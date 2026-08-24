//! The function-preservation check (BL-180, `dec:maturity-restructuring-delta`).
//!
//! What these tests are actually defending is the *refusal* half. It is easy
//! to write a classifier that recognises `ALLOCATED_TO`, calls everything else
//! benign, and hands out green certificates for changes it never looked at —
//! BL-170's fourth quadrant wearing a different hat. So the cases below spend
//! more effort on what must NOT certify than on what must:
//!
//! - a node type the table has never heard of blocks certification;
//! - a reworded capability blocks it, because a rename and a scope change are
//!   the same bytes;
//! - a known function change beats an unknown, because more information cannot
//!   un-move a capability.
//!
//! And one case is the whole reason the classifier reads endpoints rather than
//! edge types: `DEPENDS_ON` is the functional DAG between Capabilities and
//! ordinary coupling between Components. Getting that wrong would file every
//! cross-component dependency as a function change and make the check useless
//! for the design it was built for — reflow2's own, which carries 51 of them.

use reflow2_core::Value;
use reflow2_core::compare::compare_designs;
use reflow2_core::nodes::{edge, node};
use reflow2_core::preserve::preserve_rule;
use reflow2_core::{
    DesignGraph, DivergenceClass, GraphExport, PreservationCertificate, PreservationVerdict,
    certify_preservation, classify_node_type,
};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

fn export(g: &DesignGraph) -> GraphExport {
    g.export_graph().expect("export")
}

/// A design with function in one place, so a restructuring has somewhere to
/// move it to.
fn seed(g: &mut DesignGraph) {
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_requirement("req:one", "First", "The system does the first thing.")
        .expect("requirement");
    g.add_capability("cap:one", "First capability", "Does the first thing.", None)
        .expect("capability");
    g.add_capability(
        "cap:two",
        "Second capability",
        "Does the second thing.",
        None,
    )
    .expect("capability");
    g.satisfies("cap:one", "req:one").expect("satisfies");
    g.add_component(
        "cmp:monolith",
        "monolith",
        "Holds everything, for now.",
        None,
    )
    .expect("component");
    g.allocate("cap:one", "cmp:monolith").expect("allocate one");
    g.allocate("cap:two", "cmp:monolith").expect("allocate two");
}

fn certify(base: &GraphExport, other: &GraphExport) -> PreservationCertificate {
    let diff = compare_designs(base, other, "base.json", "other.json");
    certify_preservation(&diff, base, other)
}

/// Subjects in one class, for readable assertions.
fn subjects(findings: &[reflow2_core::ClassifiedFinding]) -> Vec<&str> {
    findings.iter().map(|f| f.subject.as_str()).collect()
}

// ---------------------------------------------------------------------------
// The exhaustiveness gate — the one that fails when the schema grows.
// ---------------------------------------------------------------------------

/// Every node type the schema declares must have an explicit class.
///
/// This is the test that makes the whole module honest. Without it, adding a
/// node type to `schema/*.yaml` would silently start producing `unclassified`
/// findings — which is *safe* (nothing gets falsely certified) but useless
/// (nothing gets certified at all, and nobody would know why). With it, the
/// build stops until someone decides which layer the new type belongs to.
#[test]
fn every_schema_node_type_has_a_class() {
    let g = graph();
    // Non-vacuity, without hand-copying the schema's size into a tenth place
    // (BL-164): a test that iterates an empty set passes for the wrong reason.
    for known in [node::CAPABILITY, node::COMPONENT, node::CHANGE_EVENT] {
        assert!(
            g.schema().node_types.contains_key(known),
            "{known} is missing from the schema — this test is iterating the wrong thing"
        );
    }
    let unclassified: Vec<&String> = g
        .schema()
        .node_types
        .keys()
        .filter(|t| classify_node_type(t) == DivergenceClass::Unclassified)
        .collect();

    assert!(
        unclassified.is_empty(),
        "these node types have no function/structure/supporting class, so any design using them \
         can never be certified: {unclassified:?} — add each one to FUNCTION_TYPES, \
         STRUCTURE_TYPES or SUPPORTING_TYPES in preserve.rs after deciding what it means"
    );
}

/// The rule catalogue is complete and free of duplicates — the same discipline
/// `changelog_rule::ALL` holds, and for the same reason: a rule that exists in
/// code but not in `ALL` is one nobody can audit.
#[test]
fn every_rule_is_listed_once() {
    let mut sorted = preserve_rule::ALL.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "duplicate rule in preserve_rule::ALL");
    assert_eq!(
        before, 11,
        "a rule was added or removed without updating ALL"
    );
}

// ---------------------------------------------------------------------------
// What must certify.
// ---------------------------------------------------------------------------

/// The case the module exists for: function stays put, structure moves. One
/// capability leaves the monolith for a new component, and the seam between
/// them gets declared for the first time.
#[test]
fn a_pure_restructuring_is_certified() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.add_component(
        "cmp:extracted",
        "extracted",
        "The second thing, on its own.",
        None,
    )
    .expect("component");
    g.delete_edge(edge::ALLOCATED_TO, "cap:two", "cmp:monolith")
        .expect("unallocate");
    g.allocate("cap:two", "cmp:extracted").expect("reallocate");
    g.add_interface("ifc:seam", "The seam between them")
        .expect("interface");
    g.provides("cmp:extracted", "ifc:seam").expect("provides");
    g.consumes("cmp:monolith", "ifc:seam").expect("consumes");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::Preserved);
    assert!(
        cert.function_changes.is_empty(),
        "function moved: {:?}",
        subjects(&cert.function_changes)
    );
    assert!(
        cert.unclassified.is_empty(),
        "unclassified: {:?}",
        subjects(&cert.unclassified)
    );
    // The restructuring's content is reported, not just its safety.
    assert!(cert.counts.structure >= 5, "counts: {:?}", cert.counts);
    assert!(subjects(&cert.structural_changes).contains(&"cmp:extracted"));
    assert!(subjects(&cert.structural_changes).contains(&"ifc:seam"));
    assert!(
        subjects(&cert.structural_changes)
            .iter()
            .any(|s| s.starts_with("ALLOCATED_TO cap:two"))
    );
    // A certificate always says what it is silent about.
    assert!(!cert.not_certified_about.is_empty());
}

/// Two identical records certify, but the certificate says plainly that no
/// restructuring happened — otherwise "preserved" would read as a claim about
/// work nobody did.
#[test]
fn identical_records_certify_but_say_nothing_moved() {
    let mut g = graph();
    seed(&mut g);
    let doc = export(&g);

    let cert = certify(&doc, &doc);

    assert_eq!(cert.verdict, PreservationVerdict::Preserved);
    assert!(
        cert.notes
            .iter()
            .any(|n| n.contains("No structural movement")),
        "notes: {:?}",
        cert.notes
    );
}

/// Provenance is bookkeeping wherever it appears, so a re-import that stamps
/// every node does not block certification.
#[test]
fn a_provenance_only_edit_is_not_a_function_change() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.set_provenance(node::CAPABILITY, "cap:one", "imported")
        .expect("provenance");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::Preserved);
    assert_eq!(cert.counts.supporting, 1);
    assert_eq!(
        cert.supporting_changes[0].rule,
        preserve_rule::PROPERTY_NEUTRAL
    );
}

// ---------------------------------------------------------------------------
// What must NOT certify.
// ---------------------------------------------------------------------------

/// A capability leaving the design is the plainest possible refutation.
#[test]
fn a_removed_capability_refutes_preservation() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.delete_edge(edge::ALLOCATED_TO, "cap:two", "cmp:monolith")
        .expect("unallocate");
    g.delete_node(node::CAPABILITY, "cap:two").expect("delete");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::NotPreserved);
    assert!(subjects(&cert.function_changes).contains(&"cap:two"));
    assert_eq!(
        cert.function_changes
            .iter()
            .find(|f| f.subject == "cap:two")
            .expect("the removed capability")
            .rule,
        preserve_rule::NODE_FUNCTION_LAYER
    );
}

/// Re-pointing the golden thread is a function change even though both nodes
/// survive — which is exactly the case a node-population check would miss.
#[test]
fn a_moved_satisfies_link_refutes_preservation() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.delete_edge(edge::SATISFIES, "cap:one", "req:one")
        .expect("unsatisfy");
    g.satisfies("cap:two", "req:one").expect("re-satisfy");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::NotPreserved);
    assert_eq!(cert.counts.function, 2, "one removed, one added");
    for f in &cert.function_changes {
        assert_eq!(f.rule, preserve_rule::EDGE_WITHIN_FUNCTION);
    }
}

/// The rename-versus-scope-change question the record cannot answer. The check
/// refuses rather than guessing, and hands the human both values.
#[test]
fn a_reworded_capability_is_undecidable_not_benign() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.create_node(
        node::CAPABILITY,
        "cap:one",
        [
            ("name".to_string(), Value::from("First capability")),
            (
                "description".to_string(),
                Value::from("Does the first thing, and also validates it."),
            ),
        ],
    )
    .expect("reword");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::Indeterminate);
    assert_eq!(cert.counts.unclassified, 1);
    let finding = &cert.unclassified[0];
    assert_eq!(finding.rule, preserve_rule::PROPERTY_UNDECIDABLE);
    // Both values travel with the finding, so settling it needs no second call.
    let desc = finding
        .properties
        .iter()
        .find(|p| p.property == "description")
        .expect("the description divergence");
    assert!(desc.base.is_some() && desc.other.is_some());
}

/// The BL-170 guard, stated as a test: a type the table has never seen must
/// not fall through into "not function". This synthesises a document rather
/// than writing one — the schema refuses unknown types at the write seam, and
/// a hand-authored or third-party export is exactly where one arrives.
#[test]
fn an_unknown_node_type_blocks_certification() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    let mut other = base.clone();
    let mut invented = other.nodes[0].clone();
    invented.node_type = "Sprocket".to_string();
    invented.node_id = "spr:mystery".to_string();
    other.nodes.push(invented);

    let cert = certify(&base, &other);

    assert_eq!(
        cert.verdict,
        PreservationVerdict::Indeterminate,
        "an unrecognised type must not be waved through as structural"
    );
    assert_eq!(cert.counts.unclassified, 1);
    assert_eq!(cert.unclassified[0].rule, preserve_rule::NODE_UNKNOWN_TYPE);
}

/// An edge whose endpoints neither record holds cannot be placed without
/// inventing what it means.
#[test]
fn an_edge_into_nowhere_blocks_certification() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    let mut other = base.clone();
    let mut dangling = other.edges[0].clone();
    dangling.from_id = "cap:ghost".to_string();
    dangling.to_id = "cmp:ghost".to_string();
    other.edges.push(dangling);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::Indeterminate);
    assert_eq!(
        cert.unclassified[0].rule,
        preserve_rule::EDGE_ENDPOINT_UNKNOWN
    );
}

/// A known function change is decisive even when something else is unknown:
/// more information cannot un-remove a capability. Ordering the two the other
/// way round would let one mystery downgrade a proven refutation to a shrug.
#[test]
fn a_known_refutation_outranks_an_unknown() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.delete_edge(edge::ALLOCATED_TO, "cap:two", "cmp:monolith")
        .expect("unallocate");
    g.delete_node(node::CAPABILITY, "cap:two").expect("delete");
    let mut other = export(&g);
    let mut invented = other.nodes[0].clone();
    invented.node_type = "Sprocket".to_string();
    invented.node_id = "spr:mystery".to_string();
    other.nodes.push(invented);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::NotPreserved);
    assert_eq!(
        cert.counts.unclassified, 1,
        "still reported, not suppressed"
    );
}

// ---------------------------------------------------------------------------
// The load-bearing subtlety: one edge type, two meanings.
// ---------------------------------------------------------------------------

/// `DEPENDS_ON` between two Capabilities is the functional DAG; between two
/// Components it is packaging. Classifying by edge type alone would file every
/// cross-component dependency as a function change — and reflow2's own design
/// carries 51 of them, so the check would be worthless on the first design
/// anyone pointed it at.
#[test]
fn depends_on_is_read_from_its_endpoints_not_its_type() {
    let mut g = graph();
    seed(&mut g);
    g.add_component("cmp:other", "other", "Somewhere else.", None)
        .expect("component");
    let base = export(&g);

    // Capability -> Capability: the functional DAG moved.
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        "cap:one",
        node::CAPABILITY,
        "cap:two",
        [],
    )
    .expect("functional dependency");
    // Component -> Component: packaging moved.
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:monolith",
        node::COMPONENT,
        "cmp:other",
        [],
    )
    .expect("structural dependency");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::NotPreserved);
    assert_eq!(cert.counts.function, 1);
    assert_eq!(
        cert.function_changes[0].subject,
        "DEPENDS_ON cap:one -> cap:two"
    );
    assert_eq!(
        cert.function_changes[0].rule,
        preserve_rule::EDGE_WITHIN_FUNCTION
    );
    assert!(
        subjects(&cert.structural_changes).contains(&"DEPENDS_ON cmp:monolith -> cmp:other"),
        "the component-to-component dependency must read as structure: {:?}",
        subjects(&cert.structural_changes)
    );
}

// ---------------------------------------------------------------------------
// Assurance is not function — but losing it is worth saying.
// ---------------------------------------------------------------------------

/// Dropping a check does not change what the design does, so it does not
/// refute preservation. It does weaken the evidence, and a certificate that
/// stayed silent about it would be the kind of green gate this project keeps
/// warning about.
#[test]
fn a_dropped_verification_certifies_but_is_called_out() {
    let mut g = graph();
    seed(&mut g);
    g.add_verification("ver:one", "Checks the first thing", None, None, None)
        .expect("verification");
    g.verifies("ver:one", node::CAPABILITY, "cap:one")
        .expect("verifies");
    let base = export(&g);

    g.delete_edge(edge::VERIFIES, "ver:one", "cap:one")
        .expect("unverify");
    g.delete_node(node::VERIFICATION, "ver:one")
        .expect("delete verification");
    let other = export(&g);

    let cert = certify(&base, &other);

    assert_eq!(cert.verdict, PreservationVerdict::Preserved);
    assert!(
        cert.notes.iter().any(|n| n.contains("Verification")),
        "a lost check must be named: {:?}",
        cert.notes
    );
}

// ---------------------------------------------------------------------------
// Determinism and the live-graph door.
// ---------------------------------------------------------------------------

/// Same inputs, byte-identical certificate — inherited from `compare_designs`
/// and worth pinning, because a report that reorders between runs cannot be
/// diffed in CI.
#[test]
fn the_certificate_is_deterministic() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);
    g.add_component("cmp:extracted", "extracted", "Elsewhere.", None)
        .expect("component");
    g.delete_edge(edge::ALLOCATED_TO, "cap:two", "cmp:monolith")
        .expect("unallocate");
    g.allocate("cap:two", "cmp:extracted").expect("reallocate");
    let other = export(&g);

    let a = serde_json::to_string(&certify(&base, &other)).expect("serialize");
    let b = serde_json::to_string(&certify(&base, &other)).expect("serialize");
    assert_eq!(a, b);
}

/// The live-graph form answers "did this session move structure without moving
/// function?" — the same question against the record the repo commits.
#[test]
fn the_live_graph_certifies_against_a_base_document() {
    let mut g = graph();
    seed(&mut g);
    let base = export(&g);

    g.add_component("cmp:extracted", "extracted", "Elsewhere.", None)
        .expect("component");
    g.delete_edge(edge::ALLOCATED_TO, "cap:two", "cmp:monolith")
        .expect("unallocate");
    g.allocate("cap:two", "cmp:extracted").expect("reallocate");

    let cert = g
        .certify_preservation_against(&base, "committed.json")
        .expect("certify");

    assert_eq!(cert.verdict, PreservationVerdict::Preserved);
    assert_eq!(cert.base, "committed.json");
    assert_eq!(cert.other, reflow2_core::LIVE_GRAPH_LABEL);
}
