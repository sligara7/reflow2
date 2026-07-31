//! Design-vs-design comparison — the reconcile family's missing sibling.
//!
//! `reconcile_artifacts` / `reconcile_deployment` / `reconcile_verification`
//! all compare the design against *reality* — disk, a deployment, a test run —
//! and speak "drift". This module compares **two as-designed records** with
//! each other, where neither side is reality and neither side is right: the
//! committed export against the live graph (the BL-71 clobber, caught at the
//! time only by a node count dropping), one branch's export against another's
//! (BL-70's cheapest alternatives-analysis increment), or the state a claim
//! was made against versus the state now (BL-12's merge question). The word
//! for what it finds is **divergence**, not drift — "drift" stays reserved
//! for design-vs-reality (`dec:design-diff-vocabulary`).
//!
//! # Directional on purpose
//!
//! Findings are `added` / `removed` / `changed` **relative to a named base**.
//! Every real consumer has one — the committed record, the main branch, the
//! state a claim saw — and the report carries both labels so nothing is
//! implicit. What it never does is judge which side is *correct*: it reports
//! divergence and the human decides, the same doctrine as the rest of the
//! reconcile family (`dec:report-dont-judge`).
//!
//! # Banded on purpose
//!
//! Findings are grouped into **design content** (Requirements, Decisions,
//! Components, …) and the **supporting layer** (ChangeEvents, DriftEvents,
//! Fragments, Questions — provenance and history). The divergence that
//! motivated this module was three Decisions and eight Requirements buried
//! under twenty bookkeeping nodes; a flat list hides exactly the part a
//! human needs to see first. Both bands are always reported in full —
//! banding is ordering, never omission.
//!
//! Determinism is inherited and preserved: exports are sorted so two of them
//! diff cleanly, and every list this module returns is sorted, so the same
//! pair of documents always produces the byte-identical report.

use std::collections::BTreeMap;

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::export::GraphExport;
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};
use crate::report::is_design_type;

/// The label `compare_with_base` reports for the live side.
pub const LIVE_GRAPH_LABEL: &str = "live graph";

/// One property the two records disagree about. `None` on a side means the
/// property is absent there — absent and present-but-different are different
/// facts, and collapsing them would be a quiet lie.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropertyDivergence {
    pub property: String,
    pub base: Option<Value>,
    pub other: Option<Value>,
}

/// A node present on only one side, named so the report reads without a
/// second lookup.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NodeRef {
    pub node_type: String,
    pub node_id: String,
    /// The node's `name` property, when it carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A node present on both sides that does not agree with itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangedNode {
    /// The base side's type.
    pub node_type: String,
    pub node_id: String,
    /// Set when the two records disagree about the node's *type* — the same id
    /// meaning two different kinds of thing. Rare, and always worth seeing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retyped_to: Option<String>,
    /// Every property the two sides disagree about, sorted by name.
    pub properties: Vec<PropertyDivergence>,
}

/// One band of node findings — design content or the supporting layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DiffBand {
    /// In `other`, not in `base`.
    pub added: Vec<NodeRef>,
    /// In `base`, not in `other`. On a base that is the committed record, a
    /// non-empty list here is the BL-71 silent-loss signature.
    pub removed: Vec<NodeRef>,
    /// In both, disagreeing.
    pub changed: Vec<ChangedNode>,
}

impl DiffBand {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// An edge present on only one side, identified the way exports identify
/// edges: type + endpoints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EdgeRef {
    pub edge_type: String,
    pub from_id: String,
    pub to_id: String,
}

/// An edge present on both sides whose properties disagree.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangedEdge {
    pub edge_type: String,
    pub from_id: String,
    pub to_id: String,
    pub properties: Vec<PropertyDivergence>,
}

/// The counts, first — so a caller can see "identical" or "34 divergences"
/// without reading the listings. Every count has its full listing below it in
/// the same report; the summary is a table of contents, never a cap.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffSummary {
    /// No divergence at all: every node and edge agrees.
    pub identical: bool,
    pub design_added: usize,
    pub design_removed: usize,
    pub design_changed: usize,
    pub supporting_added: usize,
    pub supporting_removed: usize,
    pub supporting_changed: usize,
    pub edges_added: usize,
    pub edges_removed: usize,
    pub edges_changed: usize,
    /// Nodes that agree exactly — reported so "3 changed" can be read against
    /// "of 250", not against silence.
    pub nodes_unchanged: usize,
    pub edges_unchanged: usize,
}

/// How the two records relate through the export lineage chain
/// (`dec:export-hash-chain`) — the answer to "was this divergence made *from*
/// the base, or did the two fork earlier?". Computed from `prev_content_hash`
/// links, so it sees one generation; `unknown` honestly covers everything the
/// chain cannot show (older documents, longer histories, unrelated designs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffAncestry {
    /// `other` names `base`'s content as its predecessor — a direct
    /// successor, so its changes were made in full view of the base.
    OtherSucceedsBase,
    /// `base` names `other`'s content as its predecessor.
    BaseSucceedsOther,
    /// Both name the same predecessor — two divergent successors of one
    /// parent, the two-writer fork in its simplest form.
    SiblingsOfCommonParent,
    /// The chain does not relate them (or one side predates hashing).
    Unknown,
}

/// Two as-designed records, compared. See the module docs for what this is
/// and is not.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignDiff {
    /// What the findings are relative to — a path, a branch, a label.
    pub base: String,
    /// The side `added` nodes are found on.
    pub other: String,
    /// How the records relate through the lineage chain.
    pub ancestry: DiffAncestry,
    pub summary: DiffSummary,
    /// Present when the two records were written by different reflow2 builds
    /// or carry different graph ids — context for reading the divergence, not
    /// a divergence itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_note: Option<String>,
    /// Design content: requirements, capabilities, components, decisions, …
    pub design: DiffBand,
    /// The supporting layer: change events, drift events, fragments,
    /// questions — provenance and history.
    pub supporting: DiffBand,
    pub edges_added: Vec<EdgeRef>,
    pub edges_removed: Vec<EdgeRef>,
    pub edges_changed: Vec<ChangedEdge>,
}

fn node_ref(node_type: &str, node_id: &str, props: &BTreeMap<String, Value>) -> NodeRef {
    NodeRef {
        node_type: node_type.to_string(),
        node_id: node_id.to_string(),
        name: props
            .get("name")
            .and_then(|v| v.as_str().map(str::to_string)),
    }
}

/// Every property the two bags disagree about, sorted by name.
fn property_divergences(
    base: &BTreeMap<String, Value>,
    other: &BTreeMap<String, Value>,
) -> Vec<PropertyDivergence> {
    let mut keys: Vec<&String> = base.keys().chain(other.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| base.get(*k) != other.get(*k))
        .map(|k| PropertyDivergence {
            property: k.clone(),
            base: base.get(k).cloned(),
            other: other.get(k).cloned(),
        })
        .collect()
}

/// Compare two export documents, directionally: what `other` added, removed
/// and changed relative to `base`. Pure and deterministic — the same pair of
/// documents always yields the byte-identical report.
pub fn compare_designs(
    base: &GraphExport,
    other: &GraphExport,
    base_label: &str,
    other_label: &str,
) -> DesignDiff {
    // Nodes by id. Exports are already sorted, but keying by BTreeMap makes
    // the walk order a property of this function, not of its input's history.
    let base_nodes: BTreeMap<&str, (&str, &BTreeMap<String, Value>)> = base
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), (n.node_type.as_str(), &n.properties)))
        .collect();
    let other_nodes: BTreeMap<&str, (&str, &BTreeMap<String, Value>)> = other
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), (n.node_type.as_str(), &n.properties)))
        .collect();

    let mut design = DiffBand::default();
    let mut supporting = DiffBand::default();
    let mut nodes_unchanged = 0usize;

    for (id, (base_ty, base_props)) in &base_nodes {
        match other_nodes.get(id) {
            None => {
                let target = if is_design_type(base_ty) {
                    &mut design
                } else {
                    &mut supporting
                };
                target.removed.push(node_ref(base_ty, id, base_props));
            }
            Some((other_ty, other_props)) => {
                let retyped = base_ty != other_ty;
                let properties = property_divergences(base_props, other_props);
                if !retyped && properties.is_empty() {
                    nodes_unchanged += 1;
                    continue;
                }
                let target = if is_design_type(base_ty) || is_design_type(other_ty) {
                    // A retype across the band boundary lands in `design`:
                    // the more visible shelf for the stranger finding.
                    &mut design
                } else {
                    &mut supporting
                };
                target.changed.push(ChangedNode {
                    node_type: base_ty.to_string(),
                    node_id: id.to_string(),
                    retyped_to: retyped.then(|| other_ty.to_string()),
                    properties,
                });
            }
        }
    }
    for (id, (other_ty, other_props)) in &other_nodes {
        if !base_nodes.contains_key(id) {
            let target = if is_design_type(other_ty) {
                &mut design
            } else {
                &mut supporting
            };
            target.added.push(node_ref(other_ty, id, other_props));
        }
    }

    // Deterministic ordering within each list: type, then id.
    for band in [&mut design, &mut supporting] {
        band.added
            .sort_by(|a, b| (&a.node_type, &a.node_id).cmp(&(&b.node_type, &b.node_id)));
        band.removed
            .sort_by(|a, b| (&a.node_type, &a.node_id).cmp(&(&b.node_type, &b.node_id)));
        band.changed
            .sort_by(|a, b| (&a.node_type, &a.node_id).cmp(&(&b.node_type, &b.node_id)));
    }

    // Edges, identified the way exports identify them: type + endpoints.
    let base_edges: BTreeMap<(&str, &str, &str), &BTreeMap<String, Value>> = base
        .edges
        .iter()
        .map(|e| {
            (
                (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str()),
                &e.properties,
            )
        })
        .collect();
    let other_edges: BTreeMap<(&str, &str, &str), &BTreeMap<String, Value>> = other
        .edges
        .iter()
        .map(|e| {
            (
                (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str()),
                &e.properties,
            )
        })
        .collect();

    let mut edges_added = Vec::new();
    let mut edges_removed = Vec::new();
    let mut edges_changed = Vec::new();
    let mut edges_unchanged = 0usize;

    for (&(ty, from, to), base_props) in &base_edges {
        match other_edges.get(&(ty, from, to)) {
            None => edges_removed.push(EdgeRef {
                edge_type: ty.to_string(),
                from_id: from.to_string(),
                to_id: to.to_string(),
            }),
            Some(other_props) => {
                let properties = property_divergences(base_props, other_props);
                if properties.is_empty() {
                    edges_unchanged += 1;
                } else {
                    edges_changed.push(ChangedEdge {
                        edge_type: ty.to_string(),
                        from_id: from.to_string(),
                        to_id: to.to_string(),
                        properties,
                    });
                }
            }
        }
    }
    for &(ty, from, to) in other_edges.keys() {
        if !base_edges.contains_key(&(ty, from, to)) {
            edges_added.push(EdgeRef {
                edge_type: ty.to_string(),
                from_id: from.to_string(),
                to_id: to.to_string(),
            });
        }
    }
    // BTreeMap iteration already sorts these by (type, from, to).

    // Different writers or different graph ids are context the reader needs
    // before judging any finding — a "changed" node under a schema bump may
    // be the migration, not an edit.
    let mut notes = Vec::new();
    if base.reflow2_version() != other.reflow2_version() {
        notes.push(format!(
            "written by different reflow2 builds: base {} vs other {}",
            base.reflow2_version(),
            other.reflow2_version()
        ));
    }
    if base.graph_id != other.graph_id {
        notes.push(format!(
            "different graph ids: base '{}' vs other '{}'",
            base.graph_id, other.graph_id
        ));
    }
    // A side whose embedded hash disagrees with its own content has been
    // edited outside reflow2 — the reader must know before trusting a single
    // finding about it.
    for (label, doc) in [("base", base), ("other", other)] {
        if doc.verify_content_hash() == Some(false) {
            notes.push(format!(
                "{label} does not match its own content_hash — edited outside reflow2 \
                 or corrupted"
            ));
        }
    }

    // Ancestry through the lineage chain: hashes are content-derived, so
    // this works even when one side predates hashing (its identity is
    // recomputed), while `prev` links only exist where a writer recorded
    // them.
    let base_hash = base.effective_content_hash();
    let other_hash = other.effective_content_hash();
    let ancestry = if other.prev_content_hash.as_deref() == Some(base_hash.as_str()) {
        DiffAncestry::OtherSucceedsBase
    } else if base.prev_content_hash.as_deref() == Some(other_hash.as_str()) {
        DiffAncestry::BaseSucceedsOther
    } else if base.prev_content_hash.is_some() && base.prev_content_hash == other.prev_content_hash
    {
        DiffAncestry::SiblingsOfCommonParent
    } else {
        DiffAncestry::Unknown
    };

    let summary = DiffSummary {
        identical: design.is_empty()
            && supporting.is_empty()
            && edges_added.is_empty()
            && edges_removed.is_empty()
            && edges_changed.is_empty(),
        design_added: design.added.len(),
        design_removed: design.removed.len(),
        design_changed: design.changed.len(),
        supporting_added: supporting.added.len(),
        supporting_removed: supporting.removed.len(),
        supporting_changed: supporting.changed.len(),
        edges_added: edges_added.len(),
        edges_removed: edges_removed.len(),
        edges_changed: edges_changed.len(),
        nodes_unchanged,
        edges_unchanged,
    };

    DesignDiff {
        base: base_label.to_string(),
        other: other_label.to_string(),
        ancestry,
        summary,
        provenance_note: (!notes.is_empty()).then(|| notes.join("; ")),
        design,
        supporting,
        edges_added,
        edges_removed,
        edges_changed,
    }
}

impl DesignGraph {
    /// Compare a base document against this live graph — "has the design in
    /// this session diverged from the record?". The live graph is the `other`
    /// side: `added` is what the session holds that the base does not.
    pub fn compare_with_base(
        &self,
        base: &GraphExport,
        base_label: &str,
    ) -> Result<DesignDiff, DynoError> {
        let live = self.export_graph()?;
        Ok(compare_designs(base, &live, base_label, LIVE_GRAPH_LABEL))
    }
}

// ---------------------------------------------------------------------------
// The changelog view — one design's delta between two of its own moments.
// ---------------------------------------------------------------------------
//
// `compare_designs` above compares two as-designed RECORDS. This compares two
// MOMENTS of one design and renders the difference in the shape the industry
// already reads (keepachangelog.com). Same family, different question.
//
// DIRECTIONALITY IS THE LOAD-BEARING CLAIM (`cap:changelog-view`, Anthony's
// formulation): "graph delta --via_agent--> human changelog". The graph delta
// is the primary, machine-readable record; the changelog is a derived
// rendering, never the other way round. When the two disagree, the graph is
// what gets interrogated and the changelog is what gets regenerated.
//
// SO THIS EMITS A DRAFT, AND SAYS SO IN THE PAYLOAD. Keep a Changelog names a
// raw commit-log dump an antipattern, and insists every entry says what a
// CONSUMER does about the change. The graph cannot know that — it holds what
// moved, not what it costs someone downstream. Rather than invent it or drop
// it silently, `needs_a_human` names the obligation and `is_draft` is
// permanently true.
//
// AND NOTHING IS GUESSED INTO A BUCKET. Every entry carries the RULE that put
// it there, mapped from vocabulary the graph already records. Anything the
// rules do not cover lands in `unmapped` with its observed values, because a
// bucket assigned by vibes and a bucket assigned by `action=removed` are
// different kinds of claim, and a changelog that cannot tell them apart is the
// commit-log dump wearing a nicer hat.

/// The five Keep a Changelog buckets, in the order that spec presents them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChangelogBucket {
    Added,
    Changed,
    Deprecated,
    Removed,
    Fixed,
}

impl ChangelogBucket {
    /// The heading exactly as Keep a Changelog spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            ChangelogBucket::Added => "Added",
            ChangelogBucket::Changed => "Changed",
            ChangelogBucket::Deprecated => "Deprecated",
            ChangelogBucket::Removed => "Removed",
            ChangelogBucket::Fixed => "Fixed",
        }
    }
}

/// The named mapping rules. These are constants rather than inline strings so
/// a test can assert the full set and fail when someone adds a sixth bucket
/// path without deciding what it means — the same discipline
/// `GapSource::is_aggregate`'s exhaustive match enforces on the detect side.
pub mod changelog_rule {
    /// The target appeared in the later release's `INCLUDES` manifest.
    pub const MANIFEST_APPEARED: &str = "manifest.includes.appeared";
    /// The target left the later release's `INCLUDES` manifest.
    pub const MANIFEST_LEFT: &str = "manifest.includes.left";
    /// A `ChangeEvent CHANGED` edge with `action=added`.
    pub const ACTION_ADDED: &str = "change_event.action=added";
    /// A `ChangeEvent CHANGED` edge with `action=modified`.
    pub const ACTION_MODIFIED: &str = "change_event.action=modified";
    /// `action=removed` on an event whose `change_type` is `deprecation` —
    /// retirement WITH the intent recorded, which is Deprecated, not Removed.
    pub const ACTION_REMOVED_DEPRECATION: &str =
        "change_event.action=removed+change_type=deprecation";
    /// `action=removed` with any other `change_type`.
    pub const ACTION_REMOVED: &str = "change_event.action=removed";
    /// A drift accept (`accepted_baseline=true`) whose event is a
    /// `test_failure_fix` — the design held and the code was repaired.
    pub const ACCEPT_TEST_FAILURE_FIX: &str =
        "changed.accepted_baseline=true+change_type=test_failure_fix";

    /// Every rule, for the exhaustiveness test.
    pub const ALL: &[&str] = &[
        MANIFEST_APPEARED,
        MANIFEST_LEFT,
        ACTION_ADDED,
        ACTION_MODIFIED,
        ACTION_REMOVED_DEPRECATION,
        ACTION_REMOVED,
        ACCEPT_TEST_FAILURE_FIX,
    ];
}

/// One drafted entry. It names WHAT moved and WHY it is in this bucket, and
/// deliberately says nothing about what a consumer should do — that is the
/// half a person writes, and claiming it here would be the graph asserting
/// something it cannot know.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangelogEntry {
    pub bucket: ChangelogBucket,
    pub subject_id: String,
    pub subject_type: String,
    pub subject_name: String,
    /// The named rule from [`changelog_rule`] that placed this entry.
    pub rule: String,
    /// The concrete values observed, so the mapping is auditable without
    /// re-deriving it.
    pub evidence: String,
}

/// A change inside the window that matched no rule. Reported rather than
/// dropped: silent truncation reads as "covered everything" when it did not
/// (AGENTS.md engineering principle 6).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnmappedChange {
    pub change_event_id: String,
    pub subject_id: String,
    pub action: Option<String>,
    pub change_type: Option<String>,
    pub why: String,
}

/// What the later release's manifest gained and lost. Empty when either side
/// is not a Release — an epoch has no manifest, and inventing one would be a
/// fabricated fact.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ManifestDelta {
    pub appeared: Vec<String>,
    pub left: Vec<String>,
}

/// A derived changelog draft. Never stored: storing it would create a second
/// source of truth about what changed, able to disagree with the graph.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangelogDraft {
    /// `[Unreleased]` or the target's version in Keep a Changelog form.
    pub heading: String,
    /// The base point, if one was resolved.
    pub from: Option<String>,
    /// The target point. `None` is the `[Unreleased]` case.
    pub to: Option<String>,
    /// The epoch sequence window actually used, exclusive of `from`.
    pub from_sequence: Option<i64>,
    pub to_sequence: Option<i64>,
    pub entries: Vec<ChangelogEntry>,
    pub manifest: ManifestDelta,
    pub unmapped: Vec<UnmappedChange>,
    /// Always true. A draft is what this produces; a changelog is what a
    /// person edits it into.
    pub is_draft: bool,
    /// The obligations the graph cannot discharge, named so they are not
    /// mistaken for absent.
    pub needs_a_human: Vec<String>,
    /// Anything that limited the answer — a release with no epoch, an
    /// unresolvable point. Loud, never silent.
    pub notes: Vec<String>,
}

/// One end of the window, resolved to the epoch ordering that drives it.
struct ChangelogPoint {
    id: String,
    release_id: Option<String>,
    sequence: Option<i64>,
}

impl DesignGraph {
    /// Resolve a Release or DesignEpoch id to its position on the time axis.
    /// A Release finds its epoch through `AT_EPOCH`; if it has none, the point
    /// still resolves but carries no sequence, and the caller reports that
    /// rather than quietly computing an empty window.
    fn changelog_point(&self, id: &str) -> Result<Option<ChangelogPoint>, DynoError> {
        if self.get_node(node::DESIGN_EPOCH, id)?.is_some() {
            let sequence = self
                .get_node(node::DESIGN_EPOCH, id)?
                .and_then(|e| e.properties.get("sequence").and_then(Value::as_i64));
            return Ok(Some(ChangelogPoint {
                id: id.to_string(),
                release_id: None,
                sequence,
            }));
        }
        if self.get_node(node::RELEASE, id)?.is_some() {
            let mut sequence = None;
            for e in self.outgoing(id, Some(edge::AT_EPOCH))? {
                if let Some(ep) = self.get_node(node::DESIGN_EPOCH, &e.to_id)? {
                    sequence = ep.properties.get("sequence").and_then(Value::as_i64);
                    break;
                }
            }
            return Ok(Some(ChangelogPoint {
                id: id.to_string(),
                release_id: Some(id.to_string()),
                sequence,
            }));
        }
        Ok(None)
    }

    /// The most recently deployed Release, by epoch sequence. This is what
    /// bounds `[Unreleased]` — deliberately `deployed` and not merely the
    /// highest version, because a release that was cut but never reached
    /// anyone has not yet drawn a line under anything.
    fn last_deployed_release(&self) -> Result<Option<ChangelogPoint>, DynoError> {
        let mut best: Option<ChangelogPoint> = None;
        for rel in self.scan_nodes(node::RELEASE)? {
            let deployed = rel
                .properties
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|s| s == "deployed");
            if !deployed {
                continue;
            }
            let Some(point) = self.changelog_point(&rel.node_id)? else {
                continue;
            };
            let better = match (&best, point.sequence) {
                (None, _) => true,
                (Some(b), Some(s)) => b.sequence.is_none_or(|bs| s > bs),
                (Some(_), None) => false,
            };
            if better {
                best = Some(point);
            }
        }
        Ok(best)
    }

    /// The `INCLUDES` manifest of a Release, as a sorted set of target ids.
    fn manifest_of(
        &self,
        release_id: &str,
    ) -> Result<std::collections::BTreeSet<String>, DynoError> {
        Ok(self
            .outgoing(release_id, Some(edge::INCLUDES))?
            .into_iter()
            .map(|e| e.to_id)
            .collect())
    }

    /// Derive a Keep a Changelog draft between two moments of this design.
    ///
    /// `to = None` is the standing `[Unreleased]` case: everything after the
    /// last DEPLOYED release, which makes "what would this increment's
    /// changelog say?" answerable BEFORE cutting it.
    ///
    /// An empty window produces an EMPTY draft — no entries, `is_draft` still
    /// true. Inventing a "no changes" entry would be the graph asserting
    /// something nobody recorded.
    pub fn changelog_view(
        &self,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<ChangelogDraft, DynoError> {
        let mut notes = Vec::new();

        let to_point = match to {
            Some(id) => {
                let p = self.changelog_point(id)?;
                if p.is_none() {
                    notes.push(format!(
                        "target '{id}' is neither a Release nor a DesignEpoch — no window computed"
                    ));
                }
                p
            }
            None => None,
        };

        // The base: explicit, or the last deployed release for [Unreleased].
        let from_point = match from {
            Some(id) => {
                let p = self.changelog_point(id)?;
                if p.is_none() {
                    notes.push(format!(
                        "base '{id}' is neither a Release nor a DesignEpoch — no window computed"
                    ));
                }
                p
            }
            None => {
                let p = self.last_deployed_release()?;
                match &p {
                    Some(b) => notes.push(format!(
                        "base not given; using the last DEPLOYED release '{}'",
                        b.id
                    )),
                    None => notes.push(
                        "base not given and no DEPLOYED release exists — the window is open from \
                         the beginning of the design"
                            .to_string(),
                    ),
                }
                p
            }
        };

        for p in [from_point.as_ref(), to_point.as_ref()]
            .into_iter()
            .flatten()
        {
            if p.sequence.is_none() {
                notes.push(format!(
                    "'{}' has no epoch (no AT_EPOCH edge), so it contributes no ordering — the \
                     change window is wider than it should be",
                    p.id
                ));
            }
        }

        let from_seq = from_point.as_ref().and_then(|p| p.sequence);
        let to_seq = to_point.as_ref().and_then(|p| p.sequence);

        // ---- the change window ------------------------------------------
        let mut entries: Vec<ChangelogEntry> = Vec::new();
        let mut unmapped: Vec<UnmappedChange> = Vec::new();

        let mut epochs_in_window: Vec<String> = Vec::new();
        for ep in self.scan_nodes(node::DESIGN_EPOCH)? {
            let Some(seq) = ep.properties.get("sequence").and_then(Value::as_i64) else {
                continue;
            };
            let after_base = from_seq.is_none_or(|f| seq > f);
            let up_to_target = to_seq.is_none_or(|t| seq <= t);
            if after_base && up_to_target {
                epochs_in_window.push(ep.node_id);
            }
        }
        epochs_in_window.sort();

        for epoch_id in &epochs_in_window {
            for pin in self.incoming(epoch_id, Some(edge::AT_EPOCH))? {
                let Some(ev) = self.get_node(node::CHANGE_EVENT, &pin.from_id)? else {
                    continue; // Snapshots and Releases pin here too
                };
                let change_type = ev
                    .properties
                    .get("change_type")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                for ch in self.outgoing(&ev.node_id, Some(edge::CHANGED))? {
                    let action = ch
                        .properties
                        .get("action")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let accepted = ch
                        .properties
                        .get("accepted_baseline")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);

                    let mapped =
                        classify_change(action.as_deref(), change_type.as_deref(), accepted);
                    match mapped {
                        Some((bucket, rule)) => {
                            let (subject_type, subject_name) = self.describe_subject(&ch.to_id)?;
                            entries.push(ChangelogEntry {
                                bucket,
                                subject_id: ch.to_id.clone(),
                                subject_type,
                                subject_name,
                                rule: rule.to_string(),
                                evidence: format!(
                                    "{} action={} change_type={} accepted_baseline={}",
                                    ev.node_id,
                                    action.as_deref().unwrap_or("<none>"),
                                    change_type.as_deref().unwrap_or("<none>"),
                                    accepted
                                ),
                            });
                        }
                        None => unmapped.push(UnmappedChange {
                            change_event_id: ev.node_id.clone(),
                            subject_id: ch.to_id.clone(),
                            action: action.clone(),
                            change_type: change_type.clone(),
                            why: "no bucket rule covers this action/change_type combination"
                                .to_string(),
                        }),
                    }
                }
            }
        }

        // ---- the manifest delta -----------------------------------------
        let mut manifest = ManifestDelta::default();
        let from_rel = from_point.as_ref().and_then(|p| p.release_id.clone());
        let to_rel = to_point.as_ref().and_then(|p| p.release_id.clone());
        if let (Some(a), Some(b)) = (&from_rel, &to_rel) {
            let before = self.manifest_of(a)?;
            let after = self.manifest_of(b)?;
            manifest.appeared = after.difference(&before).cloned().collect();
            manifest.left = before.difference(&after).cloned().collect();
            for id in &manifest.appeared {
                let (subject_type, subject_name) = self.describe_subject(id)?;
                entries.push(ChangelogEntry {
                    bucket: ChangelogBucket::Added,
                    subject_id: id.clone(),
                    subject_type,
                    subject_name,
                    rule: changelog_rule::MANIFEST_APPEARED.to_string(),
                    evidence: format!("in {b}'s INCLUDES, not in {a}'s"),
                });
            }
            for id in &manifest.left {
                let (subject_type, subject_name) = self.describe_subject(id)?;
                entries.push(ChangelogEntry {
                    bucket: ChangelogBucket::Removed,
                    subject_id: id.clone(),
                    subject_type,
                    subject_name,
                    rule: changelog_rule::MANIFEST_LEFT.to_string(),
                    evidence: format!("in {a}'s INCLUDES, not in {b}'s"),
                });
            }
        } else if from_rel.is_some() || to_rel.is_some() {
            notes.push(
                "only one end of the window is a Release, so no manifest delta was computed — an \
                 epoch has no INCLUDES manifest"
                    .to_string(),
            );
        }

        // Deterministic: same design, same window, byte-identical draft.
        entries.sort_by(|a, b| {
            (a.bucket, &a.subject_id, &a.rule).cmp(&(b.bucket, &b.subject_id, &b.rule))
        });
        entries.dedup();
        unmapped.sort_by(|a, b| {
            (&a.change_event_id, &a.subject_id).cmp(&(&b.change_event_id, &b.subject_id))
        });

        let heading = match &to_point {
            Some(p) => self
                .get_node(node::RELEASE, &p.id)?
                .and_then(|r| {
                    r.properties
                        .get("version")
                        .and_then(Value::as_str)
                        .map(|v| format!("[{v}]"))
                })
                .unwrap_or_else(|| format!("[{}]", p.id)),
            None => "[Unreleased]".to_string(),
        };

        let mut needs_a_human = vec![
            "Every entry needs what a CONSUMER does about it — upgrade steps, whether anything \
             locks out, what to change. The graph holds what moved, never what it costs \
             downstream, so that half is written by a person."
                .to_string(),
        ];
        if !entries.is_empty() {
            needs_a_human.push(format!(
                "{} drafted entr{} to curate: Keep a Changelog is FOR HUMANS, so merge, reword \
                 and drop what does not earn its line.",
                entries.len(),
                if entries.len() == 1 { "y" } else { "ies" }
            ));
        }
        if !unmapped.is_empty() {
            needs_a_human.push(format!(
                "{} change(s) matched no bucket rule and are reported unfiled rather than \
                 dropped — decide where they belong, or whether they belong at all.",
                unmapped.len()
            ));
        }

        Ok(ChangelogDraft {
            heading,
            from: from_point.as_ref().map(|p| p.id.clone()),
            to: to_point.as_ref().map(|p| p.id.clone()),
            from_sequence: from_seq,
            to_sequence: to_seq,
            entries,
            manifest,
            unmapped,
            is_draft: true,
            needs_a_human,
            notes,
        })
    }

    /// A subject's type and readable name, so the draft reads without a second
    /// lookup. Unknown ids come back as `<unknown>` rather than being skipped —
    /// a dangling CHANGED target is a fact worth seeing.
    fn describe_subject(&self, id: &str) -> Result<(String, String), DynoError> {
        for t in self.schema().node_types.keys() {
            if let Some(n) = self.get_node(t, id)? {
                let name = n
                    .properties
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_string();
                return Ok((n.node_type, name));
            }
        }
        Ok(("<unknown>".to_string(), id.to_string()))
    }
}

/// The whole bucket mapping, in one place and in rule order. `None` means no
/// rule covers it — the caller reports that rather than picking a bucket.
///
/// ORDER MATTERS AND IS NOT INCIDENTAL: a drift accept writes
/// `action=modified`, so the accept rule has to be tried before the modified
/// rule or every `test_failure_fix` would be filed as Changed.
fn classify_change(
    action: Option<&str>,
    change_type: Option<&str>,
    accepted_baseline: bool,
) -> Option<(ChangelogBucket, &'static str)> {
    if accepted_baseline && change_type == Some("test_failure_fix") {
        return Some((
            ChangelogBucket::Fixed,
            changelog_rule::ACCEPT_TEST_FAILURE_FIX,
        ));
    }
    match action {
        Some("added") => Some((ChangelogBucket::Added, changelog_rule::ACTION_ADDED)),
        Some("modified") => Some((ChangelogBucket::Changed, changelog_rule::ACTION_MODIFIED)),
        Some("removed") if change_type == Some("deprecation") => Some((
            ChangelogBucket::Deprecated,
            changelog_rule::ACTION_REMOVED_DEPRECATION,
        )),
        Some("removed") => Some((ChangelogBucket::Removed, changelog_rule::ACTION_REMOVED)),
        _ => None,
    }
}
