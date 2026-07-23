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
