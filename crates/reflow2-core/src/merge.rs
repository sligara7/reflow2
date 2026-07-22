//! Three-way merge of two divergent designs — compare's write-side sibling.
//!
//! [`crate::compare`] reports how two as-designed records diverge. This takes
//! the *third* record they diverged from — the common ancestor — and computes
//! git's trivial-merge case table per **node** and per **property** over typed
//! values, not lines (`dec:merge-three-way`):
//!
//! - only one side changed a property → take it;
//! - both changed it the same way → take the agreed value;
//! - both changed it differently → a **conflict**, surfaced as a Question for
//!   the human, never guessed (`dec:report-dont-judge`, `req:human-decides`).
//!
//! Deletions are not symmetric with edits. When one side removes a node the
//! other changed, the merge **retains the changed node and asks** — losing a
//! node is the destructive move and must be re-justified, never the silent
//! default (`req:intent-preserved`, `dec:merge-conflict-semantics`). Edges are
//! first-class design content and get the identical rule; an edge one side
//! deliberately cut is never dropped silently (`req:no-silent-fallback`).
//!
//! This function is a **proposal**: it writes nothing. It reports what merges
//! cleanly and what needs a human decision. Applying the result — resolving the
//! conflicts and committing the merged design — is a separate, explicit step
//! (the next rung; see `docs/requirements-coverage.md`). Where the common
//! ancestor comes from is the caller's business: git supplies it
//! (`git merge-base` + the committed export at that commit), so reflow2 builds
//! no commit DAG of its own here (`dec:merge-three-way`).
//!
//! Pure and deterministic: the same three documents always produce the
//! byte-identical proposal, including the conflict ids.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::detect::fnv1a;
use crate::export::{ExportedEdge, ExportedNode, GraphExport, Props};
use crate::graph::DesignGraph;

/// Which input a resolved value came from — the audit trail on every automatic
/// decision, so a reader can see *why* the merge took what it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Both sides made the identical change (or the identical addition) — there
    /// was no real choice to make.
    Agreed,
    /// Only ours diverged from the base; theirs still matched it.
    Ours,
    /// Only theirs diverged from the base; ours still matched it.
    Theirs,
}

/// What granularity an automatic resolution acted at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeUnit {
    Node,
    Property,
    Edge,
    EdgeProperty,
}

/// What the merge did to one unit that it could resolve on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeAction {
    /// A node/edge present on one side (or identically on both) and absent from
    /// the base — added to the merge.
    Add,
    /// A node/edge removed on one side while unchanged on the other — removed.
    Remove,
    /// A property value taken: a one-sided change, or an agreed change.
    Take,
}

/// One divergence the merge resolved without needing a human — the
/// non-conflicting half of the case table, reported so the proposal is legible.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutoResolution {
    pub unit: MergeUnit,
    /// Node id, or edge identity `EDGE_TYPE from_id -> to_id`.
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    pub action: MergeAction,
    pub source: Source,
}

/// The kind of both-sides conflict, so a client can render each appropriately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Both sides changed the same node property to different values.
    Property,
    /// The same id is a different node type on the two sides.
    NodeType,
    /// One side deleted a node the other changed — retain-and-ask.
    DeleteModify,
    /// Both sides changed the same edge property to different values.
    EdgeProperty,
    /// One side removed an edge the other changed — retain-and-ask.
    EdgeRemoveModify,
}

/// One both-sides conflict — a Question in all but node type
/// (`dec:merge-conflict-semantics`). Carries the three values in dispute and a
/// plain-language prompt; the graph writes nothing until the human answers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeConflict {
    /// Deterministic id `merge:{hash(base content + target + property)}` — the
    /// same divergence against the same ancestor always gets the same id, the
    /// key a later rerere rung replays a recorded resolution against
    /// (`dec:merge-conflict-semantics`, BL-80 #5).
    pub id: String,
    pub kind: ConflictKind,
    /// Node id, or edge identity `EDGE_TYPE from_id -> to_id`.
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// Plain-language question for the human.
    pub question: String,
    /// The disputed values. `None` on a side means absent there — a deletion,
    /// or a property present on only one side. Absent and present-but-different
    /// are different facts, never collapsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<Value>,
    /// For `delete_modify` / `edge_remove_modify`: which side kept the changed
    /// node/edge. The merge retains that side pending the human's call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained: Option<Source>,
    /// The rerere key: a content fingerprint over the disputed values and the
    /// property, *independent of which node* — so the identical conflict
    /// anywhere gets the same key and one recorded resolution replays across all
    /// of them (`dec:merge-rerere`, git's model). `None` for conflicts without a
    /// clean value triple (node type, delete/modify), which v1 does not record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_key: Option<String>,
}

/// The counts first, so a caller can read "clean" or "6 conflicts" without
/// walking the listings. Every count has its full listing below it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeSummary {
    /// No conflicts: the merge resolves entirely on its own.
    pub clean: bool,
    pub auto_resolved: usize,
    pub conflicts: usize,
    /// Nodes/edges that agree across every side they appear on — reported so
    /// "6 conflicts" reads against "of 300", not against silence.
    pub nodes_unchanged: usize,
    pub edges_unchanged: usize,
}

/// A three-way merge, proposed. See the module docs for what this is and is not.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeProposal {
    pub base: String,
    pub ours: String,
    pub theirs: String,
    pub summary: MergeSummary,
    /// Divergences resolved automatically, sorted.
    pub auto: Vec<AutoResolution>,
    /// Both-sides conflicts needing a human decision, sorted.
    pub conflicts: Vec<MergeConflict>,
    /// Present when the three records were written by different reflow2 builds,
    /// carry different graph ids, or one fails its own content hash — context
    /// for reading the merge, not a conflict in itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_note: Option<String>,
}

/// A node as it appears on one side: its type and its properties.
type NodeSide<'a> = (&'a str, &'a BTreeMap<String, Value>);

fn index_nodes(doc: &GraphExport) -> BTreeMap<&str, NodeSide<'_>> {
    doc.nodes
        .iter()
        .map(|n| (n.node_id.as_str(), (n.node_type.as_str(), &n.properties)))
        .collect()
}

type EdgeKey<'a> = (&'a str, &'a str, &'a str);

fn index_edges(doc: &GraphExport) -> BTreeMap<EdgeKey<'_>, &BTreeMap<String, Value>> {
    doc.edges
        .iter()
        .map(|e| {
            (
                (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str()),
                &e.properties,
            )
        })
        .collect()
}

/// The trivial-merge verdict for one value against the base.
enum ThreeWay<T> {
    /// Both sides hold the same value (whether or not it changed from base).
    Agreed(T),
    /// Exactly one side changed from base; carries the changed value and which.
    OneSided(T, Source),
    /// Both sides changed the base value, differently.
    Conflict,
}

fn three_way<T: PartialEq + Clone>(base: &T, ours: &T, theirs: &T) -> ThreeWay<T> {
    if ours == theirs {
        ThreeWay::Agreed(ours.clone())
    } else if ours == base {
        ThreeWay::OneSided(theirs.clone(), Source::Theirs)
    } else if theirs == base {
        ThreeWay::OneSided(ours.clone(), Source::Ours)
    } else {
        ThreeWay::Conflict
    }
}

/// Render an optional value for a human-facing question.
fn render(v: &Option<Value>) -> String {
    match v {
        None => "«absent»".to_string(),
        Some(val) => serde_json::to_string(val).unwrap_or_else(|_| format!("{val:?}")),
    }
}

fn conflict_id(base_hash: &str, target: &str, property: Option<&str>) -> String {
    format!(
        "merge:{:016x}",
        fnv1a(&format!("{base_hash}|{target}|{}", property.unwrap_or("")))
    )
}

/// The rerere content fingerprint (`dec:merge-rerere`): over the property and
/// the three disputed values, deliberately *not* the node id or the ancestor
/// hash — so the same conflict shape anywhere, against any base, keys the same.
fn content_key(
    property: &str,
    base: &Option<Value>,
    ours: &Option<Value>,
    theirs: &Option<Value>,
) -> String {
    format!(
        "rr:{:016x}",
        fnv1a(&format!(
            "{property}|{}|{}|{}",
            render(base),
            render(ours),
            render(theirs)
        ))
    )
}

/// Merge one node's property bag against the base (an empty base means an
/// add/add). Appends autos and conflicts; returns nothing — the caller counts.
#[allow(clippy::too_many_arguments)]
fn merge_props(
    target: &str,
    unit_prop: MergeUnit,
    conflict_prop_kind: ConflictKind,
    base_hash: &str,
    base_props: &BTreeMap<String, Value>,
    ours_props: &BTreeMap<String, Value>,
    theirs_props: &BTreeMap<String, Value>,
    auto: &mut Vec<AutoResolution>,
    conflicts: &mut Vec<MergeConflict>,
) {
    let keys: BTreeSet<&String> = base_props
        .keys()
        .chain(ours_props.keys())
        .chain(theirs_props.keys())
        .collect();
    for k in keys {
        let bv = base_props.get(k).cloned();
        let ov = ours_props.get(k).cloned();
        let tv = theirs_props.get(k).cloned();
        match three_way(&bv, &ov, &tv) {
            ThreeWay::Agreed(v) => {
                // Only report a change; a property equal to base on both sides
                // is not a divergence.
                if v != bv {
                    auto.push(AutoResolution {
                        unit: unit_prop,
                        target: target.to_string(),
                        property: Some(k.clone()),
                        action: MergeAction::Take,
                        source: Source::Agreed,
                    });
                }
            }
            ThreeWay::OneSided(_v, source) => {
                auto.push(AutoResolution {
                    unit: unit_prop,
                    target: target.to_string(),
                    property: Some(k.clone()),
                    action: MergeAction::Take,
                    source,
                });
            }
            ThreeWay::Conflict => {
                let resolution_key = Some(content_key(k, &bv, &ov, &tv));
                conflicts.push(MergeConflict {
                    id: conflict_id(base_hash, target, Some(k)),
                    kind: conflict_prop_kind,
                    target: target.to_string(),
                    property: Some(k.clone()),
                    question: format!(
                        "{target}.{k}: base={}, ours={}, theirs={} — which value should the \
                         merge take?",
                        render(&bv),
                        render(&ov),
                        render(&tv),
                    ),
                    base: bv,
                    ours: ov,
                    theirs: tv,
                    retained: None,
                    resolution_key,
                });
            }
        }
    }
}

/// Resolve one node id across the three sides.
#[allow(clippy::too_many_arguments)]
fn merge_one_node(
    id: &str,
    base: Option<&NodeSide<'_>>,
    ours: Option<&NodeSide<'_>>,
    theirs: Option<&NodeSide<'_>>,
    base_hash: &str,
    auto: &mut Vec<AutoResolution>,
    conflicts: &mut Vec<MergeConflict>,
) {
    let empty: BTreeMap<String, Value> = BTreeMap::new();
    match (base, ours, theirs) {
        // Added on one side only — take it.
        (None, Some((_, _)), None) => auto.push(AutoResolution {
            unit: MergeUnit::Node,
            target: id.to_string(),
            property: None,
            action: MergeAction::Add,
            source: Source::Ours,
        }),
        (None, None, Some((_, _))) => auto.push(AutoResolution {
            unit: MergeUnit::Node,
            target: id.to_string(),
            property: None,
            action: MergeAction::Add,
            source: Source::Theirs,
        }),
        // Added on both sides (no base) — modify/modify against an empty base.
        (None, Some((o_ty, o_props)), Some((t_ty, t_props))) => {
            if o_ty != t_ty {
                conflicts.push(node_type_conflict(base_hash, id, None, o_ty, t_ty));
            } else {
                auto.push(AutoResolution {
                    unit: MergeUnit::Node,
                    target: id.to_string(),
                    property: None,
                    action: MergeAction::Add,
                    source: Source::Agreed,
                });
                merge_props(
                    id,
                    MergeUnit::Property,
                    ConflictKind::Property,
                    base_hash,
                    &empty,
                    o_props,
                    t_props,
                    auto,
                    conflicts,
                );
            }
        }
        // Present in base, both deleted — gone, no conflict.
        (Some(_), None, None) => auto.push(AutoResolution {
            unit: MergeUnit::Node,
            target: id.to_string(),
            property: None,
            action: MergeAction::Remove,
            source: Source::Agreed,
        }),
        // Present in base, one side deleted, the other still present.
        (Some(b), Some(o), None) => {
            delete_vs_side(id, base_hash, b, o, Source::Ours, auto, conflicts)
        }
        (Some(b), None, Some(t)) => {
            delete_vs_side(id, base_hash, b, t, Source::Theirs, auto, conflicts)
        }
        // Present on all three — the full three-way.
        (Some((b_ty, b_props)), Some((o_ty, o_props)), Some((t_ty, t_props))) => {
            match three_way(&b_ty.to_string(), &o_ty.to_string(), &t_ty.to_string()) {
                ThreeWay::Conflict => {
                    conflicts.push(node_type_conflict(base_hash, id, Some(b_ty), o_ty, t_ty));
                }
                ThreeWay::OneSided(_, source) => {
                    // A one-sided retype is rare but resolvable: take the
                    // changed type, then still merge the properties.
                    auto.push(AutoResolution {
                        unit: MergeUnit::Node,
                        target: id.to_string(),
                        property: Some("node_type".to_string()),
                        action: MergeAction::Take,
                        source,
                    });
                    merge_props(
                        id,
                        MergeUnit::Property,
                        ConflictKind::Property,
                        base_hash,
                        b_props,
                        o_props,
                        t_props,
                        auto,
                        conflicts,
                    );
                }
                ThreeWay::Agreed(_) => merge_props(
                    id,
                    MergeUnit::Property,
                    ConflictKind::Property,
                    base_hash,
                    b_props,
                    o_props,
                    t_props,
                    auto,
                    conflicts,
                ),
            }
        }
        // The union guarantees at least one side is present.
        (None, None, None) => unreachable!("a node id in the union exists on some side"),
    }
}

/// One side (`kept_source`) still holds a node the other side deleted. If the
/// kept side never changed it, the deletion is clean; if it changed it, that is
/// a delete/modify conflict — retain the changed node and ask.
fn delete_vs_side(
    id: &str,
    base_hash: &str,
    base: &NodeSide<'_>,
    kept: &NodeSide<'_>,
    kept_source: Source,
    auto: &mut Vec<AutoResolution>,
    conflicts: &mut Vec<MergeConflict>,
) {
    if kept == base {
        // Unchanged on the surviving side — the deletion stands.
        auto.push(AutoResolution {
            unit: MergeUnit::Node,
            target: id.to_string(),
            property: None,
            action: MergeAction::Remove,
            source: opposite(kept_source),
        });
    } else {
        let (deleter, keeper) = match kept_source {
            Source::Ours => ("theirs", "ours"),
            Source::Theirs => ("ours", "theirs"),
            Source::Agreed => unreachable!("delete_vs_side is only called for one-sided presence"),
        };
        conflicts.push(MergeConflict {
            id: conflict_id(base_hash, id, None),
            kind: ConflictKind::DeleteModify,
            target: id.to_string(),
            property: None,
            question: format!(
                "{id}: {deleter} deleted this node, {keeper} changed it — keep {keeper}'s changed \
                 node, or accept the deletion? (The merge keeps it until you decide; deletion \
                 must be re-justified.)"
            ),
            base: None,
            ours: None,
            theirs: None,
            retained: Some(kept_source),
            resolution_key: None,
        });
    }
}

fn node_type_conflict(
    base_hash: &str,
    id: &str,
    base_ty: Option<&str>,
    ours_ty: &str,
    theirs_ty: &str,
) -> MergeConflict {
    let base_clause = match base_ty {
        Some(b) => format!(" (base {b})"),
        None => " (added on both sides)".to_string(),
    };
    MergeConflict {
        id: conflict_id(base_hash, id, Some("node_type")),
        kind: ConflictKind::NodeType,
        target: id.to_string(),
        property: Some("node_type".to_string()),
        question: format!(
            "{id}: same id, different node types — ours is a {ours_ty}, theirs is a \
             {theirs_ty}{base_clause}. Which type is right?"
        ),
        base: base_ty.map(|b| Value::String(b.to_string())),
        ours: Some(Value::String(ours_ty.to_string())),
        theirs: Some(Value::String(theirs_ty.to_string())),
        retained: None,
        resolution_key: None,
    }
}

fn opposite(s: Source) -> Source {
    match s {
        Source::Ours => Source::Theirs,
        Source::Theirs => Source::Ours,
        Source::Agreed => Source::Agreed,
    }
}

fn edge_target(key: &EdgeKey<'_>) -> String {
    let (ty, from, to) = key;
    format!("{ty} {from} -> {to}")
}

/// Resolve one edge across the three sides. Edges have no independent type to
/// merge — the type is part of their identity — so this is the node case table
/// minus the retype branch (`dec:merge-conflict-semantics`).
fn merge_one_edge(
    key: &EdgeKey<'_>,
    base: Option<&&BTreeMap<String, Value>>,
    ours: Option<&&BTreeMap<String, Value>>,
    theirs: Option<&&BTreeMap<String, Value>>,
    base_hash: &str,
    auto: &mut Vec<AutoResolution>,
    conflicts: &mut Vec<MergeConflict>,
) {
    let target = edge_target(key);
    let empty: BTreeMap<String, Value> = BTreeMap::new();
    match (base, ours, theirs) {
        (None, Some(_), None) => auto.push(edge_add(&target, Source::Ours)),
        (None, None, Some(_)) => auto.push(edge_add(&target, Source::Theirs)),
        (None, Some(o), Some(t)) => {
            auto.push(edge_add(&target, Source::Agreed));
            merge_props(
                &target,
                MergeUnit::EdgeProperty,
                ConflictKind::EdgeProperty,
                base_hash,
                &empty,
                o,
                t,
                auto,
                conflicts,
            );
        }
        (Some(_), None, None) => auto.push(AutoResolution {
            unit: MergeUnit::Edge,
            target,
            property: None,
            action: MergeAction::Remove,
            source: Source::Agreed,
        }),
        (Some(b), Some(o), None) => {
            edge_remove_vs_side(&target, base_hash, b, o, Source::Ours, auto, conflicts)
        }
        (Some(b), None, Some(t)) => {
            edge_remove_vs_side(&target, base_hash, b, t, Source::Theirs, auto, conflicts)
        }
        (Some(b), Some(o), Some(t)) => merge_props(
            &target,
            MergeUnit::EdgeProperty,
            ConflictKind::EdgeProperty,
            base_hash,
            b,
            o,
            t,
            auto,
            conflicts,
        ),
        (None, None, None) => unreachable!("an edge key in the union exists on some side"),
    }
}

fn edge_add(target: &str, source: Source) -> AutoResolution {
    AutoResolution {
        unit: MergeUnit::Edge,
        target: target.to_string(),
        property: None,
        action: MergeAction::Add,
        source,
    }
}

fn edge_remove_vs_side(
    target: &str,
    base_hash: &str,
    base: &BTreeMap<String, Value>,
    kept: &BTreeMap<String, Value>,
    kept_source: Source,
    auto: &mut Vec<AutoResolution>,
    conflicts: &mut Vec<MergeConflict>,
) {
    if kept == base {
        auto.push(AutoResolution {
            unit: MergeUnit::Edge,
            target: target.to_string(),
            property: None,
            action: MergeAction::Remove,
            source: opposite(kept_source),
        });
    } else {
        let (remover, keeper) = match kept_source {
            Source::Ours => ("theirs", "ours"),
            Source::Theirs => ("ours", "theirs"),
            Source::Agreed => {
                unreachable!("edge_remove_vs_side is only called for one-sided presence")
            }
        };
        conflicts.push(MergeConflict {
            id: conflict_id(base_hash, target, None),
            kind: ConflictKind::EdgeRemoveModify,
            target: target.to_string(),
            property: None,
            question: format!(
                "{target}: {remover} removed this edge, {keeper} changed its properties — keep \
                 {keeper}'s edge, or accept the removal? (The merge keeps it until you decide.)"
            ),
            base: None,
            ours: None,
            theirs: None,
            retained: Some(kept_source),
            resolution_key: None,
        });
    }
}

/// Context the reader needs before judging any finding — mismatched builds, ids
/// or a broken self-hash on any of the three inputs.
fn provenance_notes(base: &GraphExport, ours: &GraphExport, theirs: &GraphExport) -> Vec<String> {
    let mut notes = Vec::new();
    let versions = [
        ("base", &base.stamp.reflow2_version),
        ("ours", &ours.stamp.reflow2_version),
        ("theirs", &theirs.stamp.reflow2_version),
    ];
    if versions
        .iter()
        .any(|(_, v)| **v != *base.stamp.reflow2_version)
    {
        notes.push(format!(
            "written by different reflow2 builds: base {}, ours {}, theirs {}",
            base.stamp.reflow2_version, ours.stamp.reflow2_version, theirs.stamp.reflow2_version
        ));
    }
    if ours.graph_id != base.graph_id || theirs.graph_id != base.graph_id {
        notes.push(format!(
            "different graph ids: base '{}', ours '{}', theirs '{}'",
            base.graph_id, ours.graph_id, theirs.graph_id
        ));
    }
    for (label, doc) in [("base", base), ("ours", ours), ("theirs", theirs)] {
        if doc.verify_content_hash() == Some(false) {
            notes.push(format!(
                "{label} does not match its own content_hash — edited outside reflow2 or corrupted"
            ));
        }
    }
    notes
}

/// Compare two divergent designs against their common ancestor and propose a
/// three-way merge. Pure and deterministic; writes nothing.
pub fn merge_designs(
    base: &GraphExport,
    ours: &GraphExport,
    theirs: &GraphExport,
    base_label: &str,
    ours_label: &str,
    theirs_label: &str,
) -> MergeProposal {
    let base_hash = base.effective_content_hash();
    let bn = index_nodes(base);
    let on = index_nodes(ours);
    let tn = index_nodes(theirs);

    let mut auto = Vec::new();
    let mut conflicts = Vec::new();
    let mut nodes_unchanged = 0usize;

    let node_ids: BTreeSet<&str> = bn
        .keys()
        .chain(on.keys())
        .chain(tn.keys())
        .copied()
        .collect();
    for id in node_ids {
        let before = auto.len() + conflicts.len();
        merge_one_node(
            id,
            bn.get(id),
            on.get(id),
            tn.get(id),
            &base_hash,
            &mut auto,
            &mut conflicts,
        );
        if auto.len() + conflicts.len() == before {
            nodes_unchanged += 1;
        }
    }

    let be = index_edges(base);
    let oe = index_edges(ours);
    let te = index_edges(theirs);
    let mut edges_unchanged = 0usize;
    let edge_keys: BTreeSet<EdgeKey<'_>> = be
        .keys()
        .chain(oe.keys())
        .chain(te.keys())
        .copied()
        .collect();
    for key in edge_keys {
        let before = auto.len() + conflicts.len();
        merge_one_edge(
            &key,
            be.get(&key),
            oe.get(&key),
            te.get(&key),
            &base_hash,
            &mut auto,
            &mut conflicts,
        );
        if auto.len() + conflicts.len() == before {
            edges_unchanged += 1;
        }
    }

    // Deterministic ordering: autos by (target, property), conflicts likewise.
    auto.sort_by(|a, b| (&a.target, &a.property).cmp(&(&b.target, &b.property)));
    conflicts.sort_by(|a, b| (&a.target, &a.property).cmp(&(&b.target, &b.property)));

    let notes = provenance_notes(base, ours, theirs);
    let summary = MergeSummary {
        clean: conflicts.is_empty(),
        auto_resolved: auto.len(),
        conflicts: conflicts.len(),
        nodes_unchanged,
        edges_unchanged,
    };
    MergeProposal {
        base: base_label.to_string(),
        ours: ours_label.to_string(),
        theirs: theirs_label.to_string(),
        summary,
        auto,
        conflicts,
        provenance_note: (!notes.is_empty()).then(|| notes.join("; ")),
    }
}

// ---------------------------------------------------------------------------
// The apply rung: resolve the conflicts and commit the merged design.
//
// `merge_designs` proposes; this half carries it out. The human dispositions
// each conflict (`base` / `ours` / `theirs`), then an explicit apply commits —
// never before, and never a value the human did not choose (`dec:merge-three-way`).
// ---------------------------------------------------------------------------

/// A human's decision for one conflict — which side's value the merge takes.
/// For a delete/modify conflict, the deleting side means "accept the deletion"
/// and the changed side means "keep the changed node".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Take the common ancestor's value (revert the change / restore a deleted
    /// node; for an add/add, drop the node).
    Base,
    Ours,
    Theirs,
}

impl Resolution {
    /// Parse the wire form. Loud on anything else — an unrecognised choice is a
    /// mistake to surface, not a silent default.
    pub fn parse(s: &str) -> Option<Resolution> {
        match s {
            "base" => Some(Resolution::Base),
            "ours" => Some(Resolution::Ours),
            "theirs" => Some(Resolution::Theirs),
            _ => None,
        }
    }
}

/// Why a resolved merge could not be produced. Both variants are refusals, not
/// guesses — the merge never invents a value the human did not choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// Conflicts with no resolution supplied, by id. The merge refuses to pick.
    Unresolved(Vec<String>),
    /// Resolutions naming ids that are not conflicts in this merge — a typo or a
    /// stale resolution set, surfaced rather than ignored.
    UnknownResolutions(Vec<String>),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::Unresolved(ids) => write!(
                f,
                "the merge has {} unresolved conflict(s) — decide each (base/ours/theirs) before \
                 applying: {}",
                ids.len(),
                ids.join(", ")
            ),
            MergeError::UnknownResolutions(ids) => write!(
                f,
                "{} resolution(s) name ids that are not conflicts in this merge (a typo, or a \
                 stale resolution set): {}",
                ids.len(),
                ids.join(", ")
            ),
        }
    }
}

impl std::error::Error for MergeError {}

fn exported_node(id: &str, node_type: &str, properties: Props) -> ExportedNode {
    ExportedNode {
        node_type: node_type.to_string(),
        node_id: id.to_string(),
        properties,
    }
}

/// Resolve one node's merged property bag, consulting the human's decisions for
/// conflicts. Records unresolved conflict ids and every conflict id it touched.
#[allow(clippy::too_many_arguments)]
fn resolve_props(
    target: &str,
    base_hash: &str,
    base_props: &Props,
    ours_props: &Props,
    theirs_props: &Props,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Props {
    let keys: BTreeSet<&String> = base_props
        .keys()
        .chain(ours_props.keys())
        .chain(theirs_props.keys())
        .collect();
    let mut out = Props::new();
    for k in keys {
        let bv = base_props.get(k).cloned();
        let ov = ours_props.get(k).cloned();
        let tv = theirs_props.get(k).cloned();
        let chosen = match three_way(&bv, &ov, &tv) {
            ThreeWay::Agreed(v) | ThreeWay::OneSided(v, _) => v,
            ThreeWay::Conflict => {
                let cid = conflict_id(base_hash, target, Some(k));
                touched.insert(cid.clone());
                match resolutions.get(&cid) {
                    None => {
                        unresolved.push(cid);
                        continue;
                    }
                    Some(Resolution::Base) => bv,
                    Some(Resolution::Ours) => ov,
                    Some(Resolution::Theirs) => tv,
                }
            }
        };
        if let Some(val) = chosen {
            out.insert(k.clone(), val);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn resolve_one_node(
    id: &str,
    base: Option<NodeSide<'_>>,
    ours: Option<NodeSide<'_>>,
    theirs: Option<NodeSide<'_>>,
    base_hash: &str,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Option<ExportedNode> {
    let empty = Props::new();
    match (base, ours, theirs) {
        (None, Some((ty, p)), None) | (None, None, Some((ty, p))) => {
            Some(exported_node(id, ty, p.clone()))
        }
        (None, Some((oty, op)), Some((tty, tp))) => {
            if oty != tty {
                resolve_node_type(
                    id,
                    base_hash,
                    None,
                    oty,
                    op,
                    tty,
                    tp,
                    resolutions,
                    unresolved,
                    touched,
                )
            } else {
                let props = resolve_props(
                    id,
                    base_hash,
                    &empty,
                    op,
                    tp,
                    resolutions,
                    unresolved,
                    touched,
                );
                Some(exported_node(id, oty, props))
            }
        }
        (Some(_), None, None) => None,
        (Some((bty, bp)), Some((oty, op)), None) => resolve_delete_modify(
            id,
            (bty, bp),
            (oty, op),
            Source::Ours,
            base_hash,
            resolutions,
            unresolved,
            touched,
        ),
        (Some((bty, bp)), None, Some((tty, tp))) => resolve_delete_modify(
            id,
            (bty, bp),
            (tty, tp),
            Source::Theirs,
            base_hash,
            resolutions,
            unresolved,
            touched,
        ),
        (Some((bty, bp)), Some((oty, op)), Some((tty, tp))) => {
            match three_way(&bty.to_string(), &oty.to_string(), &tty.to_string()) {
                ThreeWay::Agreed(ty) | ThreeWay::OneSided(ty, _) => {
                    let props =
                        resolve_props(id, base_hash, bp, op, tp, resolutions, unresolved, touched);
                    Some(exported_node(id, &ty, props))
                }
                ThreeWay::Conflict => resolve_node_type(
                    id,
                    base_hash,
                    Some((bty, bp)),
                    oty,
                    op,
                    tty,
                    tp,
                    resolutions,
                    unresolved,
                    touched,
                ),
            }
        }
        (None, None, None) => unreachable!("a node id in the union exists on some side"),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_node_type(
    id: &str,
    base_hash: &str,
    base: Option<NodeSide<'_>>,
    ours_ty: &str,
    ours_props: &Props,
    theirs_ty: &str,
    theirs_props: &Props,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Option<ExportedNode> {
    let cid = conflict_id(base_hash, id, Some("node_type"));
    touched.insert(cid.clone());
    match resolutions.get(&cid) {
        None => {
            unresolved.push(cid);
            None
        }
        Some(Resolution::Base) => base.map(|(bty, bp)| exported_node(id, bty, bp.clone())),
        Some(Resolution::Ours) => Some(exported_node(id, ours_ty, ours_props.clone())),
        Some(Resolution::Theirs) => Some(exported_node(id, theirs_ty, theirs_props.clone())),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_delete_modify(
    id: &str,
    base: NodeSide<'_>,
    kept: NodeSide<'_>,
    kept_source: Source,
    base_hash: &str,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Option<ExportedNode> {
    if kept == base {
        // Unchanged on the surviving side — the deletion stands.
        return None;
    }
    let cid = conflict_id(base_hash, id, None);
    touched.insert(cid.clone());
    match resolutions.get(&cid) {
        None => {
            unresolved.push(cid);
            None
        }
        Some(Resolution::Base) => Some(exported_node(id, base.0, base.1.clone())),
        // The kept side's value is the changed node; the other side deleted it.
        Some(choice) if *choice == source_to_resolution(kept_source) => {
            Some(exported_node(id, kept.0, kept.1.clone()))
        }
        // The opposite side: accept the deletion.
        Some(_) => None,
    }
}

fn source_to_resolution(s: Source) -> Resolution {
    match s {
        Source::Ours => Resolution::Ours,
        Source::Theirs => Resolution::Theirs,
        Source::Agreed => Resolution::Ours, // unreachable in delete/modify
    }
}

fn exported_edge(key: &EdgeKey<'_>, properties: Props) -> ExportedEdge {
    let (ty, from, to) = key;
    ExportedEdge {
        edge_type: ty.to_string(),
        from_id: from.to_string(),
        to_id: to.to_string(),
        properties,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_one_edge(
    key: &EdgeKey<'_>,
    base: Option<&Props>,
    ours: Option<&Props>,
    theirs: Option<&Props>,
    base_hash: &str,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Option<ExportedEdge> {
    let target = edge_target(key);
    let empty = Props::new();
    match (base, ours, theirs) {
        (None, Some(p), None) | (None, None, Some(p)) => Some(exported_edge(key, p.clone())),
        (None, Some(o), Some(t)) => {
            let props = resolve_props(
                &target,
                base_hash,
                &empty,
                o,
                t,
                resolutions,
                unresolved,
                touched,
            );
            Some(exported_edge(key, props))
        }
        (Some(_), None, None) => None,
        (Some(b), Some(o), None) => resolve_edge_remove(
            key,
            &target,
            b,
            o,
            Source::Ours,
            base_hash,
            resolutions,
            unresolved,
            touched,
        ),
        (Some(b), None, Some(t)) => resolve_edge_remove(
            key,
            &target,
            b,
            t,
            Source::Theirs,
            base_hash,
            resolutions,
            unresolved,
            touched,
        ),
        (Some(b), Some(o), Some(t)) => {
            let props = resolve_props(
                &target,
                base_hash,
                b,
                o,
                t,
                resolutions,
                unresolved,
                touched,
            );
            Some(exported_edge(key, props))
        }
        (None, None, None) => unreachable!("an edge key in the union exists on some side"),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_edge_remove(
    key: &EdgeKey<'_>,
    target: &str,
    base: &Props,
    kept: &Props,
    kept_source: Source,
    base_hash: &str,
    resolutions: &BTreeMap<String, Resolution>,
    unresolved: &mut Vec<String>,
    touched: &mut BTreeSet<String>,
) -> Option<ExportedEdge> {
    if kept == base {
        return None;
    }
    let cid = conflict_id(base_hash, target, None);
    touched.insert(cid.clone());
    match resolutions.get(&cid) {
        None => {
            unresolved.push(cid);
            None
        }
        Some(Resolution::Base) => Some(exported_edge(key, base.clone())),
        Some(choice) if *choice == source_to_resolution(kept_source) => {
            Some(exported_edge(key, kept.clone()))
        }
        Some(_) => None,
    }
}

/// Resolve a proposed merge into a single merged design, using the human's
/// per-conflict decisions. Pure and deterministic. Refuses — never guesses —
/// when a conflict has no decision, or a decision names a non-conflict.
///
/// The merged document's lineage points at `ours` (`prev_content_hash`) and it
/// carries `ours`'s stamp: it is *ours, with theirs merged in*.
pub fn resolve_merge(
    base: &GraphExport,
    ours: &GraphExport,
    theirs: &GraphExport,
    resolutions: &BTreeMap<String, Resolution>,
) -> Result<GraphExport, MergeError> {
    let base_hash = base.effective_content_hash();
    let bn = index_nodes(base);
    let on = index_nodes(ours);
    let tn = index_nodes(theirs);

    let mut unresolved = Vec::new();
    let mut touched = BTreeSet::new();
    let mut nodes = Vec::new();

    let node_ids: BTreeSet<&str> = bn
        .keys()
        .chain(on.keys())
        .chain(tn.keys())
        .copied()
        .collect();
    for id in node_ids {
        if let Some(n) = resolve_one_node(
            id,
            bn.get(id).copied(),
            on.get(id).copied(),
            tn.get(id).copied(),
            &base_hash,
            resolutions,
            &mut unresolved,
            &mut touched,
        ) {
            nodes.push(n);
        }
    }

    let be = index_edges(base);
    let oe = index_edges(ours);
    let te = index_edges(theirs);
    let mut edges = Vec::new();
    let edge_keys: BTreeSet<EdgeKey<'_>> = be
        .keys()
        .chain(oe.keys())
        .chain(te.keys())
        .copied()
        .collect();
    for key in edge_keys {
        if let Some(e) = resolve_one_edge(
            &key,
            be.get(&key).copied(),
            oe.get(&key).copied(),
            te.get(&key).copied(),
            &base_hash,
            resolutions,
            &mut unresolved,
            &mut touched,
        ) {
            edges.push(e);
        }
    }

    if !unresolved.is_empty() {
        unresolved.sort();
        unresolved.dedup();
        return Err(MergeError::Unresolved(unresolved));
    }
    let unknown: Vec<String> = resolutions
        .keys()
        .filter(|k| !touched.contains(*k))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(MergeError::UnknownResolutions(unknown));
    }

    // Sorted the way exports are, so the merged document is canonical.
    nodes.sort_by(|a, b| {
        a.node_type
            .cmp(&b.node_type)
            .then(a.node_id.cmp(&b.node_id))
    });
    edges.sort_by(|a, b| {
        a.edge_type
            .cmp(&b.edge_type)
            .then(a.from_id.cmp(&b.from_id))
            .then(a.to_id.cmp(&b.to_id))
    });

    let mut merged = GraphExport {
        stamp: ours.stamp.clone(),
        content_hash: None,
        prev_content_hash: None,
        graph_id: ours.graph_id.clone(),
        nodes,
        edges,
    };
    merged.content_hash = Some(merged.compute_content_hash());
    merged.prev_content_hash = Some(ours.effective_content_hash());
    Ok(merged)
}

/// What an applied merge did to the live graph — every count, and every edge it
/// could not carry, named rather than dropped in silence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeApplyReport {
    pub nodes_added: usize,
    pub nodes_changed: usize,
    pub nodes_removed: usize,
    pub edges_added: usize,
    pub edges_changed: usize,
    pub edges_removed: usize,
    /// How many conflict decisions were applied.
    pub conflicts_resolved: usize,
    /// How many of those decisions came from recorded resolutions (rerere),
    /// rather than being passed explicitly (`dec:merge-rerere`).
    pub resolutions_recalled: usize,
    /// How many resolutions were recorded for future reuse.
    pub resolutions_recorded: usize,
    /// Merged edges whose endpoints were not in the merged design, so they were
    /// not written — a dangling edge is reported, never silently kept or dropped.
    pub skipped_edges: Vec<String>,
}

fn merge_err(e: MergeError) -> DynoError {
    DynoError::Validation {
        node_type: "merge".into(),
        property: "resolutions".into(),
        message: e.to_string(),
    }
}

impl DesignGraph {
    /// Record how a conflict was resolved, keyed by its content fingerprint
    /// (`dec:merge-rerere`) — stored as an answered `Question` whose id *is* the
    /// rerere key, so it travels with the design in the export and reuses the
    /// answer machinery. The same conflict shape anywhere recalls this decision.
    pub fn record_merge_resolution(
        &mut self,
        resolution_key: &str,
        choice: Resolution,
    ) -> Result<(), DynoError> {
        let answer = match choice {
            Resolution::Base => "base",
            Resolution::Ours => "ours",
            Resolution::Theirs => "theirs",
        };
        let props = crate::nodes::Props::new()
            .set("question", "recorded merge-conflict resolution (rerere)")
            .set("gap_id", resolution_key)
            .set("status", "answered")
            .set("answer", answer);
        self.create_node(crate::nodes::node::QUESTION, resolution_key, props)?;
        Ok(())
    }

    /// Recall a recorded resolution by its content fingerprint, if one exists.
    /// Advisory: the caller decides whether to reuse it (`dec:merge-rerere`).
    pub fn recall_merge_resolution(
        &self,
        resolution_key: &str,
    ) -> Result<Option<Resolution>, DynoError> {
        let Some(n) = self.get_node(crate::nodes::node::QUESTION, resolution_key)? else {
            return Ok(None);
        };
        Ok(n.properties
            .get("answer")
            .and_then(Value::as_str)
            .and_then(Resolution::parse))
    }

    /// Bulk recall — for each rerere key, any recorded resolution. This is the
    /// advisory surfacing the merge *report* cannot do itself: it is pure over
    /// files, while the resolution memory lives in the graph.
    pub fn recall_resolutions(
        &self,
        resolution_keys: &[String],
    ) -> Result<BTreeMap<String, Resolution>, DynoError> {
        let mut out = BTreeMap::new();
        for k in resolution_keys {
            if let Some(r) = self.recall_merge_resolution(k)? {
                out.insert(k.clone(), r);
            }
        }
        Ok(out)
    }

    /// Apply a resolved three-way merge into this graph — the write side of
    /// `merge_designs`. This graph is *ours*: given the common ancestor and
    /// *theirs*, it makes the live design equal the merged result, atomically.
    ///
    /// Refuses before writing anything when a conflict has no decision
    /// (`dec:merge-three-way`): the batch is only committed once the whole
    /// merged design is known. With `use_recorded`, conflicts left undecided are
    /// filled from recorded resolutions (rerere) where one exists — the human
    /// opts in by passing the flag (`dec:merge-rerere`). Every applied
    /// resolution is recorded for future reuse.
    pub fn apply_merge(
        &mut self,
        base: &GraphExport,
        theirs: &GraphExport,
        resolutions: &BTreeMap<String, Resolution>,
        use_recorded: bool,
    ) -> Result<MergeApplyReport, DynoError> {
        let ours = self.export_graph()?;

        // The proposal gives each conflict's id and its rerere key — what recall
        // and recording key on.
        let proposal = merge_designs(base, &ours, theirs, "base", "ours", "theirs");
        let mut effective = resolutions.clone();
        let mut recalled = 0usize;
        if use_recorded {
            for c in &proposal.conflicts {
                let Some(key) = &c.resolution_key else {
                    continue;
                };
                if effective.contains_key(&c.id) {
                    continue;
                }
                if let Some(r) = self.recall_merge_resolution(key)? {
                    effective.insert(c.id.clone(), r);
                    recalled += 1;
                }
            }
        }
        let merged = resolve_merge(base, &ours, theirs, &effective).map_err(merge_err)?;

        // Every applied resolution that carries a rerere key, to record for reuse.
        let to_record: Vec<(String, Resolution)> = proposal
            .conflicts
            .iter()
            .filter_map(|c| Some((c.resolution_key.clone()?, *effective.get(&c.id)?)))
            .collect();
        let conflicts_resolved = effective.len();
        let resolutions_recorded = to_record.len();

        // What ours holds now, to diff against the merged target.
        let ours_nodes: BTreeMap<&str, (&str, &Props)> = ours
            .nodes
            .iter()
            .map(|n| (n.node_id.as_str(), (n.node_type.as_str(), &n.properties)))
            .collect();
        let ours_edges: BTreeMap<EdgeKey<'_>, &Props> = ours
            .edges
            .iter()
            .map(|e| {
                (
                    (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str()),
                    &e.properties,
                )
            })
            .collect();
        let merged_node_ids: BTreeSet<&str> =
            merged.nodes.iter().map(|n| n.node_id.as_str()).collect();
        let merged_types: BTreeMap<&str, &str> = merged
            .nodes
            .iter()
            .map(|n| (n.node_id.as_str(), n.node_type.as_str()))
            .collect();
        let merged_edge_keys: BTreeSet<EdgeKey<'_>> = merged
            .edges
            .iter()
            .map(|e| (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str()))
            .collect();

        self.begin_batch();
        let result = (|| -> Result<MergeApplyReport, DynoError> {
            let mut report = MergeApplyReport {
                nodes_added: 0,
                nodes_changed: 0,
                nodes_removed: 0,
                edges_added: 0,
                edges_changed: 0,
                edges_removed: 0,
                conflicts_resolved,
                resolutions_recalled: recalled,
                resolutions_recorded,
                skipped_edges: Vec::new(),
            };

            // Upsert only what actually differs.
            for n in &merged.nodes {
                let props: std::collections::HashMap<String, Value> =
                    n.properties.clone().into_iter().collect();
                match ours_nodes.get(n.node_id.as_str()) {
                    None => {
                        self.create_node(&n.node_type, &n.node_id, props)?;
                        report.nodes_added += 1;
                    }
                    Some((oty, op)) => {
                        if *oty != n.node_type || **op != n.properties {
                            self.create_node(&n.node_type, &n.node_id, props)?;
                            report.nodes_changed += 1;
                        }
                    }
                }
            }
            for e in &merged.edges {
                let (from_ty, to_ty) = (
                    merged_types.get(e.from_id.as_str()).copied(),
                    merged_types.get(e.to_id.as_str()).copied(),
                );
                let (Some(from_ty), Some(to_ty)) = (from_ty, to_ty) else {
                    report.skipped_edges.push(format!(
                        "{} {} -> {} (an endpoint is not in the merged design)",
                        e.edge_type, e.from_id, e.to_id
                    ));
                    continue;
                };
                let key: EdgeKey<'_> = (e.edge_type.as_str(), e.from_id.as_str(), e.to_id.as_str());
                let props: std::collections::HashMap<String, Value> =
                    e.properties.clone().into_iter().collect();
                match ours_edges.get(&key) {
                    None => {
                        self.create_edge(
                            &e.edge_type,
                            from_ty,
                            &e.from_id,
                            to_ty,
                            &e.to_id,
                            props,
                        )?;
                        report.edges_added += 1;
                    }
                    Some(op) => {
                        if **op != e.properties {
                            self.create_edge(
                                &e.edge_type,
                                from_ty,
                                &e.from_id,
                                to_ty,
                                &e.to_id,
                                props,
                            )?;
                            report.edges_changed += 1;
                        }
                    }
                }
            }

            // Remove what the merge dropped. Edges first; deleting a node then
            // cascades any of its edges that survived this loop.
            for (ty, from, to) in ours_edges.keys() {
                if !merged_edge_keys.contains(&(*ty, *from, *to)) {
                    self.delete_edge(ty, from, to)?;
                    report.edges_removed += 1;
                }
            }
            for (id, (ty, _)) in &ours_nodes {
                if !merged_node_ids.contains(id) {
                    self.delete_node(ty, id)?;
                    report.nodes_removed += 1;
                }
            }

            // Record each applied resolution for reuse (rerere), in the same
            // atomic batch as the merge it came from.
            for (key, choice) in &to_record {
                self.record_merge_resolution(key, *choice)?;
            }
            Ok(report)
        })();

        match result {
            Ok(report) => {
                self.commit_batch()?;
                Ok(report)
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }
}
