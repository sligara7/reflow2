//! A capability's functional signature is writable — the black-box interface
//! at the capability tier.
//!
//! `req:recursive-black-box-decomposition` (accepted 2026-08-18) says every
//! element of a design is a black box with inner function AND INTERFACES,
//! nested as deep as the design needs. At the capability tier `inputs` and
//! `outputs` ARE that interface.
//!
//! # The measurement this exists because of
//!
//! `capability_type`, `inputs` and `outputs` were declared in
//! `schema/functional.yaml`, indexed, documented — and set on **0 of 170
//! capabilities**. Not unwanted: UNWRITABLE. `add_capability` writes name,
//! description and status, and nothing anywhere else in either crate touched
//! them, so no project using reflow2 could record a capability's signature by
//! any route the product offered.
//!
//! # What is deliberately NOT here
//!
//! No probe asserts that a signature-less capability raises a finding, because
//! no detector does. 170 of them lack one today, so a gap apiece would put 170
//! findings in front of a reader overnight — the wall-of-red failure the
//! vocabulary-coverage trial was run to avoid. Prompting at the right moment
//! is the instruction leg and belongs to its own increment.

use reflow2_core::DesignGraph;

fn graph_with_capability() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:downlink", "Downlink", "Receives a pass", None)
        .unwrap();
    g
}

fn prop(g: &DesignGraph, id: &str, key: &str) -> Option<String> {
    g.get_node("Capability", id).unwrap().and_then(|n| {
        n.properties
            .get(key)
            .and_then(|v| v.as_str().map(str::to_string))
    })
}

#[test]
fn a_signature_can_be_recorded_at_all_which_it_could_not_before() {
    let mut g = graph_with_capability();
    assert_eq!(
        prop(&g, "cap:downlink", "inputs"),
        None,
        "add_capability does not write a signature — that is the defect, restated as a probe"
    );

    g.set_capability_signature(
        "cap:downlink",
        Some("transform"),
        Some(&["raw_pass_telemetry".to_string()]),
        Some(&["decoded_frames".to_string(), "pass_report".to_string()]),
    )
    .unwrap();

    assert_eq!(
        prop(&g, "cap:downlink", "capability_type").as_deref(),
        Some("transform")
    );
    // The schema stores these as "JSON array of names/types", so the caller
    // passes a list and the encoding happens once, here, rather than in every
    // caller with one of them getting it wrong.
    assert_eq!(
        prop(&g, "cap:downlink", "inputs").as_deref(),
        Some(r#"["raw_pass_telemetry"]"#)
    );
    assert_eq!(
        prop(&g, "cap:downlink", "outputs").as_deref(),
        Some(r#"["decoded_frames","pass_report"]"#)
    );
}

#[test]
fn recording_one_half_never_erases_the_other() {
    // Two people describing the same capability from opposite ends must not
    // overwrite each other. Same rule `set_interface_spec` follows, and the
    // reason a partial write is a merge rather than a replace.
    let mut g = graph_with_capability();
    g.set_capability_signature("cap:downlink", None, Some(&["telemetry".to_string()]), None)
        .unwrap();
    g.set_capability_signature("cap:downlink", None, None, Some(&["frames".to_string()]))
        .unwrap();

    assert_eq!(
        prop(&g, "cap:downlink", "inputs").as_deref(),
        Some(r#"["telemetry"]"#)
    );
    assert_eq!(
        prop(&g, "cap:downlink", "outputs").as_deref(),
        Some(r#"["frames"]"#)
    );
}

#[test]
fn what_the_capability_already_said_about_itself_survives() {
    // The signature is enrichment, not replacement: name, description and
    // status were recorded by somebody else and must still be there.
    let mut g = graph_with_capability();
    g.set_capability_status("cap:downlink", "realized").unwrap();
    g.set_capability_signature("cap:downlink", Some("io"), None, None)
        .unwrap();

    assert_eq!(
        prop(&g, "cap:downlink", "name").as_deref(),
        Some("Downlink")
    );
    assert_eq!(
        prop(&g, "cap:downlink", "description").as_deref(),
        Some("Receives a pass")
    );
    assert_eq!(
        prop(&g, "cap:downlink", "status").as_deref(),
        Some("realized"),
        "a signature must not quietly walk the capability's status backwards"
    );
}

#[test]
fn an_unknown_capability_is_refused_rather_than_created() {
    // A typo must not mint a capability whose only content is a signature —
    // that would be a node with no name, no description and no intent behind
    // it, which is worse than the write failing.
    let mut g = graph_with_capability();
    let err = g
        .set_capability_signature("cap:typo", Some("io"), None, None)
        .expect_err("an unknown capability is refused");
    assert!(
        format!("{err}").contains("cap:typo"),
        "the refusal names what was not found: {err}"
    );
    assert!(
        g.get_node("Capability", "cap:typo").unwrap().is_none(),
        "and nothing was created on the way to refusing"
    );
}

#[test]
fn an_empty_list_is_a_statement_and_not_a_silence() {
    // "This capability takes nothing in" is a real design claim — a source, a
    // clock, a generator — and it must be distinguishable from nobody having
    // said. `None` leaves the field alone; an empty list records emptiness.
    let mut g = graph_with_capability();
    g.set_capability_signature("cap:downlink", None, Some(&[]), None)
        .unwrap();
    assert_eq!(
        prop(&g, "cap:downlink", "inputs").as_deref(),
        Some("[]"),
        "an explicitly empty input list is recorded, not dropped"
    );
    assert_eq!(
        prop(&g, "cap:downlink", "outputs"),
        None,
        "while the field nobody mentioned stays absent — absent means nobody said"
    );
}
