//! Function preservation — the certificate a restructuring can earn.
//!
//! [`crate::compare`] answers *what* two as-designed records disagree about.
//! This answers the question a **maturity restructuring** actually needs:
//! *did the function set survive?* (`dec:maturity-restructuring-delta`,
//! BL-180.)
//!
//! # The two deltas, and why only one of them is checkable
//!
//! A **capability increment** grows the function set — new requirements, new
//! capabilities, delivered against a schedule. reflow2 already models that:
//! `DesignEpoch`, `SCHEDULED_FOR`, `Release`/`INCLUDES`, readiness gating.
//!
//! A **maturity restructuring** is the orthogonal move. It holds the function
//! set INVARIANT and changes everything else — allocation, packaging, which
//! functions live in which component, which seams are declared. It is the
//! shape of *"knowing what I know now, would I have designed this the same
//! way?"*, and it is what a design does when it stops proving function and
//! starts earning structure.
//!
//! Such a change is **safe exactly when function is provably preserved**, and
//! that is computable rather than promised. The invariant:
//!
//! > No Capability or Requirement added or removed, and no `SATISFIES` link
//! > changed — only allocation, containment and interface edges moved.
//!
//! Which is the move [`crate::verify`] makes for tests (`dec:passing-is-verified`,
//! *verified means a check that passes, not one that exists*), applied to
//! structure: the difference between a refactor someone **hopes** is safe and
//! one the graph **checked**.
//!
//! # Unclassified never passes
//!
//! The whole risk in a classifier like this is the hidden inclusion list. A
//! rule set that recognises `ALLOCATED_TO` and `CONTAINS` and calls everything
//! else "not function" would certify a design it never examined — BL-170's
//! fourth quadrant exactly, where *neither swept nor claimed* is mentioned by
//! neither input and therefore looks clean.
//!
//! So the default here is [`DivergenceClass::Unclassified`], and an
//! unclassified finding forces [`PreservationVerdict::Indeterminate`]. A
//! vocabulary this module has not been taught cannot be certified through it.
//! `tests/preserve.rs` walks the live schema and fails the build when a node
//! type lands with no explicit class, so the omission is caught before a
//! release rather than by a false green.
//!
//! # What the graph counts and what the human judges
//!
//! A reworded `description` on a Capability is either a rename or a scope
//! change, and **nothing in the record distinguishes them**. Guessing either
//! way would be the graph asserting something it cannot know, so property
//! divergences on function-layer nodes land in `unclassified` with both values
//! attached, and the verdict says `indeterminate`. That is
//! `dec:three-party-checks` — the LLM speaks, the graph remembers and counts,
//! the human decides — and it is why this module reports a certificate rather
//! than a score.
//!
//! Deterministic and pure: same two documents, byte-identical certificate.

use std::collections::BTreeMap;

use serde::Serialize;

use dynograph_core::DynoError;

use crate::compare::{
    ChangedEdge, ChangedNode, DesignDiff, LIVE_GRAPH_LABEL, NodeRef, PropertyDivergence,
    compare_designs,
};
use crate::export::GraphExport;
use crate::graph::DesignGraph;
use crate::nodes::node;

/// Which layer a divergence belongs to, and therefore whether it bears on the
/// verdict.
///
/// Only [`Function`](DivergenceClass::Function) and
/// [`Unclassified`](DivergenceClass::Unclassified) move the verdict. The other
/// two are reported in full anyway: banding is ordering, never omission — the
/// same rule [`crate::compare`] holds itself to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceClass {
    /// Bears on WHAT the design does: the Capability/Requirement population,
    /// the flows and actors around them, and the links wholly inside that
    /// layer. Any finding here refutes preservation.
    Function,
    /// Bears on WHERE function lives and HOW it is packaged: components,
    /// interfaces, artifacts, and the allocation/containment/realization edges
    /// that place function in them. This is what a restructuring is *made of*.
    Structure,
    /// Neither: provenance, history, questions, decisions, verifications,
    /// releases. Allowed, listed, and never silently dropped.
    Supporting,
    /// No rule covers it. Reported with everything known about it, and it
    /// blocks certification — see the module docs.
    Unclassified,
}

/// The verdict. Three-valued on purpose: a check that cannot tell has to say
/// so, exactly as an unassessed readiness gate reads `indeterminate` rather
/// than passing (`dec:readiness-is-an-observation-the-threshold-is-the-judgement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationVerdict {
    /// Every divergence is structural or supporting. The function set is
    /// unchanged and the restructuring is certified — within the stated
    /// caveats, which are always reported.
    Preserved,
    /// At least one function-layer divergence. Whatever else this change is,
    /// it is not a pure restructuring: it moved function.
    NotPreserved,
    /// At least one divergence no rule covers, and none that refutes
    /// preservation. Nothing is certified; the unclassified list is the work.
    Indeterminate,
}

/// The named classification rules, in one place so a test can assert the full
/// set and fail when someone adds a path without deciding what it means — the
/// discipline `changelog_rule` already holds in [`crate::compare`].
pub mod preserve_rule {
    /// The node's type is in the function layer.
    pub const NODE_FUNCTION_LAYER: &str = "node.layer=function";
    /// The node's type is in the structural layer.
    pub const NODE_STRUCTURAL_LAYER: &str = "node.layer=structure";
    /// The node's type is provenance, history or governance.
    pub const NODE_SUPPORTING_LAYER: &str = "node.layer=supporting";
    /// The node's type is not in the classification table at all.
    pub const NODE_UNKNOWN_TYPE: &str = "node.type=unclassified";
    /// Every changed property on this node is one that cannot bear on
    /// function (see `NEUTRAL_PROPERTIES`).
    pub const PROPERTY_NEUTRAL: &str = "property.bears_on_function=no";
    /// A changed property on a function-layer node. A rename and a scope
    /// change are indistinguishable here, so this is for a human.
    pub const PROPERTY_UNDECIDABLE: &str = "property.bears_on_function=undecidable";
    /// The same id names two different kinds of thing across the records.
    pub const NODE_RETYPED: &str = "node.retyped";
    /// Both endpoints are function-layer nodes, so the edge lies wholly
    /// inside the function layer.
    pub const EDGE_WITHIN_FUNCTION: &str = "edge.endpoints=function+function";
    /// An endpoint is structural, and neither is supporting or unknown.
    pub const EDGE_TOUCHES_STRUCTURE: &str = "edge.endpoints=touches_structure";
    /// An endpoint is a supporting node, so the edge is bookkeeping.
    pub const EDGE_TOUCHES_SUPPORTING: &str = "edge.endpoints=touches_supporting";
    /// An endpoint names a node neither record holds, or one whose type has
    /// no class. The edge cannot be placed without inventing its meaning.
    pub const EDGE_ENDPOINT_UNKNOWN: &str = "edge.endpoint=unresolvable";

    /// Every rule, for the exhaustiveness test.
    pub const ALL: &[&str] = &[
        NODE_FUNCTION_LAYER,
        NODE_STRUCTURAL_LAYER,
        NODE_SUPPORTING_LAYER,
        NODE_UNKNOWN_TYPE,
        PROPERTY_NEUTRAL,
        PROPERTY_UNDECIDABLE,
        NODE_RETYPED,
        EDGE_WITHIN_FUNCTION,
        EDGE_TOUCHES_STRUCTURE,
        EDGE_TOUCHES_SUPPORTING,
        EDGE_ENDPOINT_UNKNOWN,
    ];
}

/// Node types whose population **is** the function set.
///
/// `Project` is here because its objective is the design's function stated at
/// the top; a restructuring that edits it has changed what the thing is for.
/// `Constraint` is here because a numeric limit is intent — a mass budget is
/// something the design must do, not somewhere it lives.
const FUNCTION_TYPES: &[&str] = &[
    node::PROJECT,
    node::REQUIREMENT,
    node::CONSTRAINT,
    node::CAPABILITY,
    node::FLOW,
    node::ACTOR,
];

/// Node types that say where function lives and how it is packaged. These are
/// what a restructuring is *allowed* to move, and what it is *made of*.
///
/// `Artifact` is structural rather than supporting on purpose: re-packaging
/// moves files, and a restructuring that splits one module into three is
/// visible here and nowhere else.
const STRUCTURE_TYPES: &[&str] = &[node::COMPONENT, node::INTERFACE, node::ARTIFACT, "Anchor"];

/// Provenance, history, governance and assurance. Allowed to move freely, and
/// listed in full so "allowed" never means "hidden".
///
/// `Verification` sits here rather than in the function layer because dropping
/// a check does not change what the design does — it changes how well anyone
/// knows. That is a real cost, so it is raised as a caveat on the certificate
/// instead of being folded into the verdict.
const SUPPORTING_TYPES: &[&str] = &[
    node::DESIGN_RULE,
    node::DECISION,
    node::QUESTION,
    node::CONTRIBUTOR,
    node::FRAGMENT,
    node::VERIFICATION,
    node::DRIFT_EVENT,
    "QualityGate",
    node::RELEASE,
    node::ENVIRONMENT,
    node::RESOURCE,
    "EnvironmentRule",
    node::DIMENSION_ASSESSMENT,
    node::DIMENSION_OBSERVATION,
    node::READINESS_ASSESSMENT,
    node::DESIGN_EPOCH,
    node::TEMPORAL_FACT,
    node::SNAPSHOT,
    node::CHANGE_EVENT,
];

/// Properties that cannot bear on function wherever they appear.
///
/// Deliberately tiny. `provenance` records **how** a node entered the graph;
/// a Capability's `status` records **how far built** it is. Neither says
/// anything about what the design does. Everything else on a function-layer
/// node — a statement, a description, inputs, outputs, a Requirement's status
/// (`dropped` really does remove intent) — is left undecidable rather than
/// waved through.
fn is_neutral_property(node_type: &str, property: &str) -> bool {
    property == "provenance" || (node_type == node::CAPABILITY && property == "status")
}

/// Which layer a node type belongs to. `Unclassified` is the fallback and the
/// safeguard: a type this table has not been taught blocks certification
/// rather than defaulting into "not function".
pub fn classify_node_type(node_type: &str) -> DivergenceClass {
    if FUNCTION_TYPES.contains(&node_type) {
        DivergenceClass::Function
    } else if STRUCTURE_TYPES.contains(&node_type) {
        DivergenceClass::Structure
    } else if SUPPORTING_TYPES.contains(&node_type) {
        DivergenceClass::Supporting
    } else {
        DivergenceClass::Unclassified
    }
}

/// One classified divergence, carrying the rule that placed it so the
/// certificate is auditable without re-deriving it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassifiedFinding {
    pub class: DivergenceClass,
    /// `node_added` / `node_removed` / `node_changed` / `edge_added` /
    /// `edge_removed` / `edge_changed`.
    pub kind: &'static str,
    /// The node id, or `TYPE from -> to` for an edge.
    pub subject: String,
    /// The node type, or the edge type.
    pub subject_type: String,
    /// The node's `name`, when it has one and the record carried it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The named rule from [`preserve_rule`] that placed this finding.
    pub rule: &'static str,
    /// For a changed node or edge: exactly which properties disagree, with
    /// both values. This is what a human reads to settle an `undecidable`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<PropertyDivergence>,
}

/// How many findings landed in each class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct PreservationCounts {
    pub function: usize,
    pub structure: usize,
    pub supporting: usize,
    pub unclassified: usize,
}

/// The certificate. A verdict, the invariant it was measured against, every
/// finding that produced it, and — always — what it could not see.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreservationCertificate {
    /// What the findings are relative to.
    pub base: String,
    /// The side `added` findings were found on.
    pub other: String,
    pub verdict: PreservationVerdict,
    /// The invariant, in words, so the certificate states its own meaning
    /// rather than relying on the reader knowing this module.
    pub invariant: &'static str,
    pub counts: PreservationCounts,
    /// Why the verdict is not `preserved`. Empty when function survived.
    pub function_changes: Vec<ClassifiedFinding>,
    /// What the restructuring actually moved — allocation, packaging, seams.
    /// This is the change's content, not its problem.
    pub structural_changes: Vec<ClassifiedFinding>,
    /// What no rule covers, and therefore what blocks certification.
    pub unclassified: Vec<ClassifiedFinding>,
    /// Provenance, history and assurance. Listed, never hidden.
    pub supporting_changes: Vec<ClassifiedFinding>,
    /// What this check is silent about, stated on every certificate including
    /// a clean one. A green result is evidence about what it covers and says
    /// nothing about the rest.
    pub not_certified_about: Vec<String>,
    /// Anything that shaped this particular answer — an absent structural
    /// delta, dropped verifications, a provenance mismatch between records.
    pub notes: Vec<String>,
}

/// The invariant, stated once and carried on every certificate.
pub const FUNCTION_PRESERVATION_INVARIANT: &str = "No Capability, Requirement, Flow, Actor, Constraint or Project added, removed or \
     semantically edited, and no link wholly inside that function layer (SATISFIES, the \
     capability DEPENDS_ON DAG, PART_OF_FLOW, INTERACTS_WITH) added, removed or changed. \
     Allocation, containment, interface and packaging movement is what a restructuring is \
     made of and does not refute preservation.";

/// A node's type as either record knows it, preferring the base — used to
/// classify an edge from its endpoints. `None` means neither record holds a
/// node with that id, which is a dangling endpoint and a real finding.
fn endpoint_type<'a>(
    base_types: &'a BTreeMap<&str, &str>,
    other_types: &'a BTreeMap<&str, &str>,
    id: &str,
) -> Option<&'a str> {
    base_types.get(id).or_else(|| other_types.get(id)).copied()
}

/// Classify an edge from its endpoints.
///
/// **The load-bearing subtlety, and it is not incidental.** `DEPENDS_ON` is
/// the functional DAG between two Capabilities *and* the coupling between two
/// Components — one edge type, two meanings, told apart only by what it joins.
/// Classifying by edge type alone would file every cross-component dependency
/// as a function change and make the check useless for the exact design it was
/// built for. So the rule is about the endpoints:
///
/// - both function → the edge lies inside the function layer, so it *is*
///   function;
/// - either supporting → bookkeeping;
/// - otherwise, structure;
/// - either endpoint unresolvable → unclassified, because an edge whose ends
///   are unknown cannot be placed without inventing its meaning.
fn classify_edge(
    from_class: Option<DivergenceClass>,
    to_class: Option<DivergenceClass>,
) -> (DivergenceClass, &'static str) {
    let (Some(a), Some(b)) = (from_class, to_class) else {
        return (
            DivergenceClass::Unclassified,
            preserve_rule::EDGE_ENDPOINT_UNKNOWN,
        );
    };
    if a == DivergenceClass::Unclassified || b == DivergenceClass::Unclassified {
        return (
            DivergenceClass::Unclassified,
            preserve_rule::EDGE_ENDPOINT_UNKNOWN,
        );
    }
    if a == DivergenceClass::Supporting || b == DivergenceClass::Supporting {
        return (
            DivergenceClass::Supporting,
            preserve_rule::EDGE_TOUCHES_SUPPORTING,
        );
    }
    if a == DivergenceClass::Function && b == DivergenceClass::Function {
        return (
            DivergenceClass::Function,
            preserve_rule::EDGE_WITHIN_FUNCTION,
        );
    }
    (
        DivergenceClass::Structure,
        preserve_rule::EDGE_TOUCHES_STRUCTURE,
    )
}

/// Classify a node that exists on one side only.
fn classify_presence(node_ref: &NodeRef, kind: &'static str) -> ClassifiedFinding {
    let class = classify_node_type(&node_ref.node_type);
    let rule = match class {
        DivergenceClass::Function => preserve_rule::NODE_FUNCTION_LAYER,
        DivergenceClass::Structure => preserve_rule::NODE_STRUCTURAL_LAYER,
        DivergenceClass::Supporting => preserve_rule::NODE_SUPPORTING_LAYER,
        DivergenceClass::Unclassified => preserve_rule::NODE_UNKNOWN_TYPE,
    };
    ClassifiedFinding {
        class,
        kind,
        subject: node_ref.node_id.clone(),
        subject_type: node_ref.node_type.clone(),
        name: node_ref.name.clone(),
        rule,
        properties: Vec::new(),
    }
}

/// Classify a node present on both sides that does not agree with itself.
///
/// A retype is always unclassified: the same id meaning two different kinds of
/// thing is rare, and no layer rule was written for it.
fn classify_change(changed: &ChangedNode) -> ClassifiedFinding {
    let class = classify_node_type(&changed.node_type);
    let (class, rule) = if changed.retyped_to.is_some() {
        (DivergenceClass::Unclassified, preserve_rule::NODE_RETYPED)
    } else if changed
        .properties
        .iter()
        .all(|p| is_neutral_property(&changed.node_type, &p.property))
    {
        (
            // A neutral-property edit on a structural node is still structural
            // movement; on anything else it is bookkeeping.
            if class == DivergenceClass::Structure {
                DivergenceClass::Structure
            } else {
                DivergenceClass::Supporting
            },
            preserve_rule::PROPERTY_NEUTRAL,
        )
    } else {
        match class {
            // A semantic edit to a function-layer node is the rename-versus-
            // scope-change question, which the record cannot answer.
            DivergenceClass::Function => (
                DivergenceClass::Unclassified,
                preserve_rule::PROPERTY_UNDECIDABLE,
            ),
            DivergenceClass::Structure => (
                DivergenceClass::Structure,
                preserve_rule::NODE_STRUCTURAL_LAYER,
            ),
            DivergenceClass::Supporting => (
                DivergenceClass::Supporting,
                preserve_rule::NODE_SUPPORTING_LAYER,
            ),
            DivergenceClass::Unclassified => (
                DivergenceClass::Unclassified,
                preserve_rule::NODE_UNKNOWN_TYPE,
            ),
        }
    };
    ClassifiedFinding {
        class,
        kind: "node_changed",
        subject: changed.node_id.clone(),
        subject_type: changed.node_type.clone(),
        name: None,
        rule,
        properties: changed.properties.clone(),
    }
}

/// An edge finding, rendered so the subject reads without a second lookup.
fn edge_finding(
    class: DivergenceClass,
    rule: &'static str,
    kind: &'static str,
    edge_type: &str,
    from_id: &str,
    to_id: &str,
    properties: Vec<PropertyDivergence>,
) -> ClassifiedFinding {
    ClassifiedFinding {
        class,
        kind,
        subject: format!("{edge_type} {from_id} -> {to_id}"),
        subject_type: edge_type.to_string(),
        name: None,
        rule,
        properties,
    }
}

/// Decide whether a restructuring preserved function, from two as-designed
/// records and the diff between them.
///
/// Takes the [`DesignDiff`] rather than recomputing it: the diff is the
/// evidence and this is the reading of it, and a caller that wants both should
/// not pay for two walks or risk them disagreeing.
pub fn certify_preservation(
    diff: &DesignDiff,
    base: &GraphExport,
    other: &GraphExport,
) -> PreservationCertificate {
    // Node types from both records, so an edge whose endpoint was deleted on
    // one side is still resolvable from the other.
    let base_types: BTreeMap<&str, &str> = base
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), n.node_type.as_str()))
        .collect();
    let other_types: BTreeMap<&str, &str> = other
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), n.node_type.as_str()))
        .collect();

    let mut findings: Vec<ClassifiedFinding> = Vec::new();

    // Nodes. Both bands: `compare`'s design/supporting split answers a
    // different question (what a human reads first) and this one must not
    // inherit its judgement — a ChangeEvent is supporting here too, but
    // because a rule says so, not because another module already banded it.
    for band in [&diff.design, &diff.supporting] {
        for n in &band.added {
            findings.push(classify_presence(n, "node_added"));
        }
        for n in &band.removed {
            findings.push(classify_presence(n, "node_removed"));
        }
        for c in &band.changed {
            findings.push(classify_change(c));
        }
    }

    // Edges.
    let class_of = |id: &str| endpoint_type(&base_types, &other_types, id).map(classify_node_type);
    let push_edge = |findings: &mut Vec<ClassifiedFinding>,
                     kind: &'static str,
                     edge_type: &str,
                     from_id: &str,
                     to_id: &str,
                     properties: Vec<PropertyDivergence>| {
        let (class, rule) = classify_edge(class_of(from_id), class_of(to_id));
        findings.push(edge_finding(
            class, rule, kind, edge_type, from_id, to_id, properties,
        ));
    };
    for e in &diff.edges_added {
        push_edge(
            &mut findings,
            "edge_added",
            &e.edge_type,
            &e.from_id,
            &e.to_id,
            Vec::new(),
        );
    }
    for e in &diff.edges_removed {
        push_edge(
            &mut findings,
            "edge_removed",
            &e.edge_type,
            &e.from_id,
            &e.to_id,
            Vec::new(),
        );
    }
    for e in &diff.edges_changed {
        let ChangedEdge {
            edge_type,
            from_id,
            to_id,
            properties,
        } = e;
        push_edge(
            &mut findings,
            "edge_changed",
            edge_type,
            from_id,
            to_id,
            properties.clone(),
        );
    }

    // Deterministic within each class: kind, then subject.
    findings.sort_by(|a, b| (a.kind, &a.subject).cmp(&(b.kind, &b.subject)));

    let mut function_changes = Vec::new();
    let mut structural_changes = Vec::new();
    let mut supporting_changes = Vec::new();
    let mut unclassified = Vec::new();
    for f in findings {
        match f.class {
            DivergenceClass::Function => function_changes.push(f),
            DivergenceClass::Structure => structural_changes.push(f),
            DivergenceClass::Supporting => supporting_changes.push(f),
            DivergenceClass::Unclassified => unclassified.push(f),
        }
    }

    // A known function change is decisive even when other things are unknown:
    // more information cannot un-move a capability.
    let verdict = if !function_changes.is_empty() {
        PreservationVerdict::NotPreserved
    } else if !unclassified.is_empty() {
        PreservationVerdict::Indeterminate
    } else {
        PreservationVerdict::Preserved
    };

    let counts = PreservationCounts {
        function: function_changes.len(),
        structure: structural_changes.len(),
        supporting: supporting_changes.len(),
        unclassified: unclassified.len(),
    };

    let mut notes = Vec::new();
    if structural_changes.is_empty() && verdict == PreservationVerdict::Preserved {
        notes.push(
            "No structural movement was observed, so this certifies nothing about a \
             restructuring — the two records describe the same structure."
                .to_string(),
        );
    }
    let verifications_lost = supporting_changes
        .iter()
        .filter(|f| f.kind == "node_removed" && f.subject_type == node::VERIFICATION)
        .count();
    if verifications_lost > 0 {
        notes.push(format!(
            "{verifications_lost} Verification(s) left the design. Function is unaffected — but \
             the evidence for it is weaker, and `dec:passing-is-verified` means a claim with no \
             passing check is not verified."
        ));
    }
    if let Some(note) = &diff.provenance_note {
        notes.push(format!("From the diff: {note}"));
    }

    PreservationCertificate {
        base: diff.base.clone(),
        other: diff.other.clone(),
        verdict,
        invariant: FUNCTION_PRESERVATION_INVARIANT,
        counts,
        function_changes,
        structural_changes,
        unclassified,
        supporting_changes,
        not_certified_about: vec![
            "The CODE. This compares two design records; whether the implementation preserved \
             behaviour is a question for tests, and nothing here has read a line of it."
                .to_string(),
            "Function that was never captured. The function set is what the design records, so \
             a capability nobody wrote down cannot be reported as lost."
                .to_string(),
            "Whether the new structure is BETTER. Preservation is a safety property, not a \
             judgement — it says the restructuring cost nothing, never that it bought anything."
                .to_string(),
            "Renames. A reworded name or description is indistinguishable from a scope change \
             in a property diff, so both land in `unclassified` for a human rather than being \
             guessed either way."
                .to_string(),
            "What an Interface now CARRIES. Declaring and moving seams is what a restructuring \
             is made of, so interface changes read as structure — but a published contract \
             whose payload changed can break a consumer without touching a single Capability. \
             Every one is listed under `structural_changes`; read them, and check the \
             consuming side with propagate_change."
                .to_string(),
        ],
        notes,
    }
}

impl DesignGraph {
    /// Certify this live graph against a base document — "did the work in
    /// this session move structure without moving function?".
    ///
    /// The live graph is the `other` side, matching
    /// [`DesignGraph::compare_with_base`]: `added` is what the session holds
    /// that the base does not.
    pub fn certify_preservation_against(
        &self,
        base: &GraphExport,
        base_label: &str,
    ) -> Result<PreservationCertificate, DynoError> {
        let live = self.export_graph()?;
        let diff = compare_designs(base, &live, base_label, LIVE_GRAPH_LABEL);
        Ok(certify_preservation(&diff, base, &live))
    }
}
