//! COMPOSE — analyse two designs together without merging them
//! (`req:composed-analysis`).
//!
//! The user's framing, and it is better than the bespoke comparator this
//! started as: to check whether two projects line up, **import one design into
//! the other and run reflow2's ordinary checks over the whole**. The seam's
//! problems then surface as the gaps they already are — a contract with no
//! provider once both sides are present, a requirement nothing satisfies across
//! the join, a duplicate that is one thing named twice — instead of needing a
//! detector nobody else benefits from. It is the principle this project keeps
//! rediscovering: make the existing computation *see* more rather than write a
//! new one.
//!
//! ## Why this cannot just call `import_graph`
//!
//! `import_graph` writes every node under its **original id** with upsert
//! semantics. It exists to layer an export onto the design it came from, and
//! its own documentation says so. Point it at a *different* design and the
//! dependency's `cap:store` silently overwrites the consumer's — corruption
//! with no error, which is the worst shape a bug can take here.
//!
//! Neither of the other two composition mechanisms does this job either, and
//! the distinction is worth keeping straight:
//!
//! | Mechanism | What it composes |
//! |---|---|
//! | `mirror_surface` | the other design's **published surface only**, kept foreign |
//! | `merge_designs` | two **versions of the same design**, three-way |
//! | this | two **different designs**, for analysis only |
//!
//! ## Nothing is written to your design
//!
//! The combined graph is built **in memory and thrown away**. Your design is
//! read, never modified; the dependency is never persisted into it. That
//! disposes of the two hazards `req:composed-analysis` names — an export of
//! yours can never start shipping the dependency's internals, and there is no
//! residue to clean up — and it follows the precedent set by the ingest
//! handshake's prepare rounds, which replay against a throwaway graph for the
//! same reason.
//!
//! ## Ids are namespaced, and findings say which side they came from
//!
//! Every imported node is rewritten to `{namespace}::{id}`, edges with it, so
//! two designs that both call something `cmp:store` stay distinct. Every
//! finding is then attributed — **yours**, **theirs**, or **crosses the seam** —
//! because a consumer shown its dependency's internal gaps as if they were its
//! own will switch the whole feature off, and rightly.

use std::collections::BTreeMap;

use dynograph_core::DynoError;

use crate::export::GraphExport;
use crate::graph::DesignGraph;
use crate::heal::HealIssue;

/// Which design a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Every node it names is yours. You would have seen this without composing.
    Ours,
    /// Every node it names belongs to the imported design. Reported, because
    /// hiding it would be dishonest, but it is not yours to fix.
    Theirs,
    /// It names nodes from BOTH designs. **This is the whole point** — a finding
    /// that only exists because the two were analysed together.
    Seam,
}

/// One finding from the combined design, attributed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComposedFinding {
    pub side: Side,
    /// `gap` or `defect`.
    pub kind: &'static str,
    pub source: String,
    pub title: String,
    /// Ids as they appear in the combined graph — imported ones still carry
    /// their `{namespace}::` prefix, so a reader can always tell whose is whose.
    pub affected_ids: Vec<String>,
}

/// What analysing the two together found.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComposedReport {
    /// The imported design's own id, as its export declared it.
    pub imported_graph_id: String,
    /// The prefix its ids were given.
    pub namespace: String,
    pub imported_nodes: usize,
    pub imported_edges: usize,
    /// Edges the import could not place, with the reason — never dropped in
    /// silence.
    pub skipped_edges: Vec<String>,
    /// **Findings that span both designs.** Listed first because they are the
    /// only ones neither design could have found alone.
    pub seam_findings: Vec<ComposedFinding>,
    /// Findings wholly within your design.
    pub our_findings: Vec<ComposedFinding>,
    /// Findings wholly within theirs — reported, not yours to fix.
    pub their_findings: Vec<ComposedFinding>,
}

/// Prefix an id, leaving an already-prefixed one alone.
fn ns(namespace: &str, id: &str) -> String {
    format!("{namespace}::{id}")
}

impl DesignGraph {
    /// Analyse this design together with another, without touching either
    /// (`req:composed-analysis`).
    ///
    /// `namespace` distinguishes the imported design's ids — usually its
    /// `graph_id`. See the module docs for why this is not `import_graph` and
    /// why nothing is persisted.
    pub fn compose_and_analyse(
        &self,
        other: &GraphExport,
        namespace: &str,
    ) -> Result<ComposedReport, DynoError> {
        if namespace.trim().is_empty() {
            return Err(DynoError::Query(
                "a namespace is required: without one the two designs' ids would collide, \
                 which is the whole reason this is not import_graph"
                    .to_string(),
            ));
        }

        // Build the combined graph in memory. Ours first, unchanged.
        let mut combined = DesignGraph::open_in_memory()?;
        let ours = self.export_graph()?;
        combined.import_graph(&ours)?;
        let our_ids: std::collections::BTreeSet<String> =
            ours.nodes.iter().map(|n| n.node_id.clone()).collect();

        // Theirs, namespaced, so nothing of ours is overwritten.
        let mut theirs = other.clone();
        for n in &mut theirs.nodes {
            n.node_id = ns(namespace, &n.node_id);
        }
        for e in &mut theirs.edges {
            e.from_id = ns(namespace, &e.from_id);
            e.to_id = ns(namespace, &e.to_id);
        }
        let report = combined.import_graph(&theirs)?;

        let is_ours = |id: &str| our_ids.contains(id);
        let classify = |ids: &[String]| -> Side {
            let mine = ids.iter().any(|i| is_ours(i));
            let theirs = ids.iter().any(|i| !is_ours(i));
            match (mine, theirs) {
                (true, true) => Side::Seam,
                (false, true) => Side::Theirs,
                _ => Side::Ours,
            }
        };

        let mut buckets: BTreeMap<&'static str, Vec<ComposedFinding>> = BTreeMap::new();
        for gap in combined.detect_gaps()? {
            let side = classify(&gap.affected_ids);
            buckets
                .entry(bucket(side))
                .or_default()
                .push(ComposedFinding {
                    side,
                    kind: "gap",
                    source: serde_json::to_string(&gap.gap_source)
                        .unwrap_or_default()
                        .trim_matches('"')
                        .to_string(),
                    title: gap.title,
                    affected_ids: gap.affected_ids,
                });
        }
        for defect in combined.open_defects()? {
            let ids = defect_ids(&defect);
            let side = classify(&ids);
            buckets
                .entry(bucket(side))
                .or_default()
                .push(ComposedFinding {
                    side,
                    kind: "defect",
                    source: format!("{:?}", defect.category),
                    title: defect.message.clone(),
                    affected_ids: ids,
                });
        }

        Ok(ComposedReport {
            imported_graph_id: other.graph_id.clone(),
            namespace: namespace.to_string(),
            imported_nodes: theirs.nodes.len(),
            imported_edges: report.edges_written,
            skipped_edges: report
                .skipped_edges
                .iter()
                .map(|s| format!("{s:?}"))
                .collect(),
            seam_findings: buckets.remove("seam").unwrap_or_default(),
            our_findings: buckets.remove("ours").unwrap_or_default(),
            their_findings: buckets.remove("theirs").unwrap_or_default(),
        })
    }
}

fn bucket(side: Side) -> &'static str {
    match side {
        Side::Seam => "seam",
        Side::Ours => "ours",
        Side::Theirs => "theirs",
    }
}

/// A defect's affected ids, however the issue names them.
fn defect_ids(issue: &HealIssue) -> Vec<String> {
    issue.affected_ids.clone()
}
