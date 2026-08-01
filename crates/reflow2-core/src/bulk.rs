//! Bulk forms — one call where the surface measured N.
//!
//! BL-153 measured reflow2's own tool surface across 46 retained sessions and
//! 6,095 calls: **52.9% of every tool-to-tool transition is the same tool
//! called again.** Nobody calls one tool 144 times in a session because they
//! want to. The self-loops this module answers, as self-loops · total calls:
//! `set_artifact_checksum` 244 · 408, `create_node` 112 · 209, `contains`
//! 109 · 176, `acknowledge_gap` 90 · 157, `contain_component` 77 · 87,
//! `satisfies` 74 · 164. (`release_includes` 988 · 1008 was the largest and is
//! answered separately by [`DesignGraph::release_includes_all`], because that
//! manifest is DERIVABLE and a faster loop would have been the wrong fix.)
//!
//! # The rule these forms hold, and why it is not a preference
//!
//! BL-153 left one question open — *"one refusal semantics to define
//! (all-or-nothing, or per-item findings?)"* — and posed it as a choice. It is
//! not one. The graph store already has an atomic batch (`begin_batch` /
//! `commit_batch` / `discard_batch`) that HEAL's apply step and `import_graph`
//! both use, so **all-or-nothing costs nothing and is the house answer**: a
//! partial bulk write leaves a design in a state nobody chose.
//!
//! And the two options were never exclusive. Every form here does both:
//!
//! - **Every item is attempted, so every failure is collected**, not just the
//!   first. That is the defect BL-118 files against `import_graph` — *"validation
//!   is fail-fast, one error per attempt"* — and BL-139 records what it costs:
//!   the adopt loop's corrective re-import is exactly where it breaks. A bulk
//!   form that surfaced one error per round trip would replace N writes with N
//!   retries and save nobody anything.
//! - **If anything failed, nothing is written.** The batch is discarded and the
//!   whole failure list is returned.
//!
//! So the answer is *all-or-nothing **with** per-item findings*, which is
//! strictly better than either option as posed.
//!
//! # What a bulk form must not become
//!
//! `dec:two-sided-accept` ("silent drift-accept does not exist") and
//! `dec:ask-not-repair` bound bulk **dispositions** — a judgement taken about
//! each item. BL-153 stated the trap plainly: *"a batch of 144 acknowledgements
//! with one reason is exactly the erosion those decisions exist to prevent, so
//! a bulk form needs per-item dispositions or it is worse than the loop."*
//!
//! So [`ChecksumAccept`] carries its **own** disposition and [`GapAck`] its
//! **own** reason. Neither is hoisted to a call-level argument, and that is
//! deliberate rather than an oversight to tidy up later: the judgement stays
//! per item, and only the round trip is collapsed. `set_artifact_checksums`
//! makes 244 accepts cost one call and still 244 decisions.

use dynograph_core::{DynoError, Value};
use dynograph_storage::{StoredEdge, StoredNode};

use crate::artifact::DriftDisposition;
use crate::graph::DesignGraph;
use crate::nodes::Props;

/// One item's failure, with the position that identifies it in the caller's
/// list — an id alone is not enough to locate a duplicate or an empty one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BulkFailure {
    pub index: usize,
    pub id: String,
    pub error: String,
}

/// What a bulk call did. `applied` is false exactly when `failures` is
/// non-empty, and then `written` is empty — the batch was discarded.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BulkReport<T> {
    pub written: Vec<T>,
    pub failures: Vec<BulkFailure>,
    pub applied: bool,
}

impl<T> BulkReport<T> {
    fn rejected(failures: Vec<BulkFailure>) -> Self {
        Self {
            written: Vec::new(),
            failures,
            applied: false,
        }
    }
}

/// One node for [`DesignGraph::create_nodes`].
#[derive(Debug, Clone)]
pub struct NodeSpec {
    pub node_type: String,
    pub id: String,
    pub props: std::collections::HashMap<String, Value>,
}

/// One edge for [`DesignGraph::create_edges`].
#[derive(Debug, Clone)]
pub struct EdgeSpec {
    pub edge_type: String,
    pub from_type: String,
    pub from_id: String,
    pub to_type: String,
    pub to_id: String,
    pub props: std::collections::HashMap<String, Value>,
}

/// One accepted checksum, carrying **its own** disposition — see the module
/// note on why this is not a call-level argument.
#[derive(Debug, Clone)]
pub struct ChecksumAccept<'a> {
    pub artifact_id: String,
    pub checksum: String,
    pub disposition: DriftDisposition<'a>,
    pub note: Option<String>,
    pub at: Option<String>,
}

/// One question to record, for [`DesignGraph::record_asked_questions`].
#[derive(Debug, Clone)]
pub struct AskedRecord {
    pub gap_id: String,
    pub affected_ids: Vec<String>,
    pub question: String,
    pub context_setter: Option<String>,
    pub rephrase_degraded: bool,
}

/// One acknowledged gap, carrying **its own** reason.
#[derive(Debug, Clone)]
pub struct GapAck {
    pub gap_id: String,
    pub affected_ids: Vec<String>,
    pub reason: String,
}

/// Run `op` over every item inside one atomic batch, collecting each failure
/// rather than stopping at the first, and discarding the whole batch if any
/// item failed.
///
/// The two properties are separable and both are load-bearing. Collecting all
/// failures is what stops a bulk form degenerating into N retries (BL-118).
/// Discarding on any failure is what stops it leaving a half-applied design.
fn atomic<I, T>(
    g: &mut DesignGraph,
    items: &[I],
    id_of: impl Fn(&I) -> String,
    mut op: impl FnMut(&mut DesignGraph, &I) -> Result<T, DynoError>,
) -> Result<BulkReport<T>, DynoError> {
    g.begin_batch();
    let mut written = Vec::with_capacity(items.len());
    let mut failures = Vec::new();

    for (index, item) in items.iter().enumerate() {
        match op(g, item) {
            Ok(v) => written.push(v),
            // Keep going. A later item may fail for its own reason, and the
            // caller should learn every one of them in this round trip.
            Err(e) => failures.push(BulkFailure {
                index,
                id: id_of(item),
                error: e.to_string(),
            }),
        }
    }

    if failures.is_empty() {
        g.commit_batch()?;
        Ok(BulkReport {
            written,
            failures,
            applied: true,
        })
    } else {
        g.discard_batch();
        Ok(BulkReport::rejected(failures))
    }
}

impl DesignGraph {
    /// Upsert many nodes in one call — the bulk form of `upsert_node`
    /// (`create_node` on the surface), measured at 112 self-loops.
    pub fn create_nodes(
        &mut self,
        items: &[NodeSpec],
    ) -> Result<BulkReport<StoredNode>, DynoError> {
        atomic(
            self,
            items,
            |n| n.id.clone(),
            |g, n| g.upsert_node(&n.node_type, &n.id, n.props.clone()),
        )
    }

    /// Create many edges in one call — the bulk form of `create_edge`, and so
    /// of every typed helper built on it: `contains` (109 self-loops),
    /// `contain_component` (77), `satisfies` (74), `allocate`, `realizes`.
    ///
    /// One bulk form rather than one per helper is deliberate. The helpers are
    /// thin wrappers that fill in the endpoint types and pass empty props, so a
    /// bulk `create_edge` covers all of them; and BL-155 found 40 of 132 served
    /// tools never called in the retained sample, which makes adding six tools
    /// where one will do a cost rather than a convenience.
    pub fn create_edges(
        &mut self,
        items: &[EdgeSpec],
    ) -> Result<BulkReport<StoredEdge>, DynoError> {
        atomic(
            self,
            items,
            |e| format!("{} {} -> {}", e.edge_type, e.from_id, e.to_id),
            |g, e| {
                g.create_edge(
                    &e.edge_type,
                    &e.from_type,
                    &e.from_id,
                    &e.to_type,
                    &e.to_id,
                    e.props.clone(),
                )
            },
        )
    }

    /// Accept many drift baselines in one call — the bulk form of
    /// `set_artifact_checksum`, the largest remaining offender at 244
    /// self-loops across 22 sessions.
    ///
    /// **Each item carries its own disposition.** Hoisting one disposition to
    /// the call would make this the bulk *accept* `dec:two-sided-accept` exists
    /// to prevent; see the module note.
    pub fn set_artifact_checksums(
        &mut self,
        items: &[ChecksumAccept<'_>],
    ) -> Result<BulkReport<(StoredNode, String)>, DynoError> {
        atomic(
            self,
            items,
            |c| c.artifact_id.clone(),
            |g, c| {
                g.set_artifact_checksum(
                    &c.artifact_id,
                    &c.checksum,
                    c.disposition.clone(),
                    c.note.as_deref(),
                    c.at.as_deref(),
                )
            },
        )
    }

    /// Record many asked questions in one call — the write half of a multi-gap
    /// ask, and the other half of BL-153's fix shape (3) alongside
    /// [`Self::acknowledge_gaps`].
    ///
    /// `asked_at` is shared because it is a fact about the *call* — when the
    /// batch was put to the user — not a judgement about each gap. That is the
    /// line the module note draws: a shared timestamp is a timestamp, a shared
    /// disposition would be a shared decision.
    pub fn record_asked_questions(
        &mut self,
        items: &[AskedRecord],
        asked_at: Option<&str>,
    ) -> Result<BulkReport<String>, DynoError> {
        atomic(
            self,
            items,
            |a| a.gap_id.clone(),
            |g, a| {
                g.record_asked_question(
                    &a.gap_id,
                    &a.affected_ids,
                    &a.question,
                    crate::detect::AskedQuestion {
                        prompt_id: None,
                        context_setter: a.context_setter.as_deref(),
                        asked_at,
                        rephrase_degraded: a.rephrase_degraded,
                    },
                )
            },
        )
    }

    /// Acknowledge many gaps in one call — the bulk form of `acknowledge_gap`
    /// (90 self-loops), and the write half of BL-153's fix shape (3).
    ///
    /// **Each item carries its own reason.** This is the exact case BL-153
    /// named as the one a bulk form could make worse than the loop, so the
    /// reason stays per gap and only the round trip collapses.
    pub fn acknowledge_gaps(&mut self, items: &[GapAck]) -> Result<BulkReport<String>, DynoError> {
        atomic(
            self,
            items,
            |a| a.gap_id.clone(),
            |g, a| g.acknowledge_gap(&a.gap_id, &a.affected_ids, &a.reason),
        )
    }
}

/// Convenience for callers assembling props from string pairs.
impl NodeSpec {
    pub fn new(node_type: &str, id: &str, props: Props) -> Self {
        Self {
            node_type: node_type.to_string(),
            id: id.to_string(),
            props: props.into(),
        }
    }
}
