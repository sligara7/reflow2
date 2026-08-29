//! A property that NAMES a node id is refused when that node does not exist —
//! the guard `create_edge` has had since 2026-07-28, applied to the other shape
//! of reference.
//!
//! `fact:defect-a-property-naming-a-node-is-unguarded-while-edges-are-not`,
//! measured 2026-08-16: `create_node` was called for a TemporalFact carrying
//! `subject_id: "cap:questions-are-tracked"`. It was written and echoed back
//! with no complaint, and that capability HAS NEVER EXISTED. The slip was
//! caught only because the write was read back by habit; nothing in the tool's
//! reply revealed it.
//!
//! ⭐ THE CONTRAST IS THE POINT. An edge to a missing node is refused through
//! all sixteen typed helpers, with an atomicity case and a size-floor, because
//! `authored_by` once accepted `person:anthony` and a phantom edge sat in the
//! graph until somebody noticed by eye. The identical failure was still
//! reachable through a PROPERTY, and lands in the same place: a reference
//! nothing can walk.
//!
//! WHY IT COULD NOT SIMPLY REUSE THE EDGE GUARD, and this is what made it a
//! real change rather than an oversight to patch:
//!
//! 1. `Schema::validate_node` takes only `(node_type, properties)`. It has NO
//!    STORE ACCESS and is deliberately pure, so it cannot ask whether a node
//!    exists. The guard therefore lives in `graph.rs` beside the edge one,
//!    which is where `create_edge` already reaches `get_node`.
//! 2. An edge declares its endpoint TYPES; a property holds a bare id. There is
//!    no type-free lookup in the store — every `get_node` needs a type — so the
//!    guard resolves a bare id by asking each declared node type in turn, the
//!    same walk `count_all_nodes` already makes.
//!
//! 🛑 MEASURED SCOPE, so this test is not read as more than it is. Nine values
//! in this design's own graph dangle, and a write-time guard reaches only about
//! a third of them: three are the typo `prj:reflow2` for `proj:reflow2`, which
//! this refuses. The rest — `cap:reconcile-artifacts`, `cap:thin-install`,
//! `cap:gap-to-prompt` — were VALID WHEN WRITTEN and dangled later when the
//! node was renamed. **No write-time check can catch those**; that is a second
//! cause with its own fix, and this file does not claim it.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::Props;

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

/// The measured case, verbatim: a fact about a capability that was never created.
#[test]
fn a_fact_whose_subject_does_not_exist_is_refused() {
    let mut g = graph();
    let err = g
        .create_node(
            "TemporalFact",
            "fact:invented",
            Props::new()
                .set("subject_id", "cap:never-created")
                .set("statement", "about a node that is not there"),
        )
        .expect_err("a fact naming a node that does not exist must be refused");

    let msg = err.to_string();
    assert!(
        msg.contains("cap:never-created"),
        "the refusal must NAME the missing id, or the caller cannot fix it: {msg}"
    );
    assert!(
        msg.contains("subject_id"),
        "and must name the PROPERTY, since a node may carry more than one \
         reference: {msg}"
    );
}

/// Refused BEFORE anything is written — the same promise `create_edge` makes.
/// A node stored and then reported later is the state this guard exists to
/// prevent, so a failed write must leave no trace.
#[test]
fn a_refused_write_leaves_nothing_behind() {
    let mut g = graph();
    let _ = g.create_node(
        "TemporalFact",
        "fact:invented",
        Props::new().set("subject_id", "cap:never-created"),
    );
    assert!(
        g.get_node("TemporalFact", "fact:invented")
            .expect("read back")
            .is_none(),
        "the refused node must not be in the store"
    );
}

/// The guard must not fire on a reference that resolves. This is the case that
/// would break every existing caller if the walk over node types were wrong.
#[test]
fn a_fact_whose_subject_exists_is_accepted() {
    let mut g = graph();
    g.add_capability("cap:real", "A real one", "A real capability", None)
        .expect("capability");
    g.create_node(
        "TemporalFact",
        "fact:grounded",
        Props::new()
            .set("subject_id", "cap:real")
            .set("statement", "about something that exists"),
    )
    .expect("a fact naming an existing node must be accepted");
}

/// The subject may be ANY node type — a fact is written about capabilities,
/// components, decisions, rules and the project itself. A guard that resolved
/// only one type would refuse most legitimate writes.
#[test]
fn a_subject_may_be_any_node_type() {
    let mut g = graph();
    g.add_component("cmp:real", "part", "a real component", None)
        .expect("component");
    g.create_node(
        "TemporalFact",
        "fact:about-a-component",
        Props::new().set("subject_id", "cmp:real"),
    )
    .expect("a fact about a Component must be accepted");
}

/// A node that declares no reference property is untouched by the guard.
#[test]
fn a_node_with_no_reference_property_is_unaffected() {
    let mut g = graph();
    g.add_requirement("req:plain", "no references here", "A plain requirement.")
        .expect("a node with no declared reference must write normally");
}

// ---------------------------------------------------------------------------
// THE IMPORT HALF. A restore is not a write, and the guard above had to learn
// the difference the hard way.
//
// MEASURED 2026-08-29, on this design's own 3388-node export: with the guard
// running on the import walk, restoring it failed with 23 faults and wrote
// NOTHING. Only nine were real. Three were nodes present in the very same
// document — at indices 3342, 3360 and 3169, referenced from 2854, 3167 and
// 3063 — refused because an export is ordered by node type and `TemporalFact`
// sorts before `Verification`. One more was a node refused above cascading onto
// its referrer, and ten were edges orphaned by the thirteen.
//
// ⭐ SO THE CAUSE IS NOT "SOME REFERENCES DANGLE". It is that the guard resolved
// a reference against the store AS IT STOOD MID-WALK, while the import is
// all-or-nothing — the document is the unit that becomes true, so the document
// is what a reference must be judged against. A design with NO dangling
// references at all would still have failed this way, which is what makes it a
// class rather than this graph's data problem.

use reflow2_core::export::GraphExport;

/// A fact whose subject is declared LATER in the same document imports cleanly.
/// The regression test for the ordering bug: `TemporalFact` sorts before
/// `Verification`, so this is a forward reference in every export ever written.
#[test]
fn a_forward_reference_within_one_document_is_not_a_dangling_one() {
    let mut source = graph();
    source
        .add_verification("ver:sorts-after-the-fact", "later", None, None, None)
        .expect("verification");
    source
        .create_node(
            "TemporalFact",
            "fact:names-a-later-node",
            Props::new()
                .set("subject_id", "ver:sorts-after-the-fact")
                .set("statement", "about a node that sorts after it"),
        )
        .expect("write-time: the subject exists, so this is accepted");

    let doc = source.export_graph().expect("export");
    let fact = doc
        .nodes
        .iter()
        .position(|n| n.node_id == "fact:names-a-later-node")
        .expect("fact in document");
    let subject = doc
        .nodes
        .iter()
        .position(|n| n.node_id == "ver:sorts-after-the-fact")
        .expect("subject in document");
    assert!(
        fact < subject,
        "the premise of this test is that the reference points FORWARD ({fact} -> {subject}); \
         if export ordering changed, this no longer covers the bug it was written for"
    );

    let mut restored = graph();
    let report = restored
        .import_graph(&doc)
        .expect("a document whose references resolve WITHIN IT must import");
    assert!(
        report.dangling_node_refs.is_empty(),
        "a forward reference is not dangling: {:?}",
        report.dangling_node_refs
    );
    assert!(
        restored
            .get_node("TemporalFact", "fact:names-a-later-node")
            .expect("read back")
            .is_some(),
        "the fact must be in the restored graph"
    );
}

/// A reference that resolves to nothing is REPORTED and the node is still
/// written. The sibling of `skipped_edges`, and the reason is that a restore
/// reproduces a state that already existed rather than asserting a new one —
/// refusing would leave a graph holding one bad reference with an unrestorable
/// backup, remediable only by hand-editing the export.
#[test]
fn a_genuinely_dangling_reference_is_reported_not_refused_on_import() {
    let mut source = graph();
    source
        .add_verification("ver:real", "real", None, None, None)
        .expect("verification");
    source
        .create_node(
            "TemporalFact",
            "fact:will-be-orphaned",
            Props::new()
                .set("subject_id", "ver:real")
                .set("statement", "written while its subject existed"),
        )
        .expect("fact");

    // Break the reference the only way a caller now can: in the document. The
    // write path refuses to make one, which is why this is built by editing the
    // export rather than by calling `create_node`.
    let doc = source.export_graph().expect("export");
    // Only the REFERENCE, never the node's own id — a blanket replace renames
    // the target too and the reference goes on resolving, which is exactly how
    // the first version of this test passed while proving nothing.
    let json = serde_json::to_string(&doc).expect("serialize").replace(
        "\"subject_id\":\"ver:real\"",
        "\"subject_id\":\"ver:renamed-away\"",
    );
    assert!(
        json.contains("ver:renamed-away") && json.contains("\"node_id\":\"ver:real\""),
        "the document must now hold a reference to a node it does not contain"
    );
    let doc: GraphExport = serde_json::from_str(&json).expect("deserialize");

    let mut restored = graph();
    let report = restored
        .import_graph(&doc)
        .expect("a restore must not refuse a design that already held this");

    assert_eq!(
        report.dangling_node_refs.len(),
        1,
        "the unresolvable reference must be named: {:?}",
        report.dangling_node_refs
    );
    assert!(
        report.dangling_node_refs[0].contains("fact:will-be-orphaned")
            && report.dangling_node_refs[0].contains("subject_id"),
        "the report must name the node AND the property: {:?}",
        report.dangling_node_refs
    );
    assert!(
        restored
            .get_node("TemporalFact", "fact:will-be-orphaned")
            .expect("read back")
            .is_some(),
        "the node is WRITTEN — reported, not refused"
    );
}

/// The write path is unchanged by any of the above. Stated as its own test
/// because the import fix moves where the check runs, and the failure mode of
/// such a move is that it quietly stops running anywhere.
#[test]
fn moving_the_check_to_import_did_not_disarm_it_on_write() {
    let mut g = graph();
    g.create_node(
        "TemporalFact",
        "fact:still-guarded",
        Props::new().set("subject_id", "cap:still-never-created"),
    )
    .expect_err("the interactive write path must still refuse");
}
