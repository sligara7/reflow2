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

use dynograph_core::Value;
use serde::Serialize;

use crate::detect::fnv1a;
use crate::export::GraphExport;

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
