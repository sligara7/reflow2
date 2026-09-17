//! `encoding_undecided` — several nodes of one type encoded differently under
//! no encoding decision
//! (`req:a-taxonomy-is-decided-once-before-bulk-capture-and-every-instance-cites-it`).
//!
//! Alex, 2026-09-17: "the taxonomy lives as Decisions … nothing helps you write
//! them." The finding is the leg that notices absence; the skill is the leg that
//! helps write them. The cases that carry the weight are the silences: under
//! three instances, once every unstated instance cites an accepted decision,
//! and — for a type where nobody has said — under five.

use reflow2_core::foundation::core::Value;
use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, GapCandidate, GapSource};

fn comp(g: &mut DesignGraph, id: &str, kind: Option<&str>) {
    let mut props = Props::new()
        .set("name", id)
        .set("purpose", "a part")
        .set("level", "component");
    if let Some(k) = kind {
        props = props.set("kind", k);
    }
    g.create_node(node::COMPONENT, id, props).unwrap();
}

fn find(g: &DesignGraph) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::EncodingUndecided)
}

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g
}

#[test]
fn under_three_instances_nothing_is_asked() {
    let mut g = design();
    comp(&mut g, "cmp:a", Some("service"));
    comp(&mut g, "cmp:b", None);
    assert!(find(&g).is_none(), "two instances are not yet a category");
}

#[test]
fn a_mixed_encoding_under_no_decision_is_named_with_its_counts_and_the_silent_ones() {
    let mut g = design();
    comp(&mut g, "cmp:a", Some("service"));
    comp(&mut g, "cmp:b", Some("service"));
    comp(&mut g, "cmp:c", None);
    comp(&mut g, "cmp:d", None);
    let gap = find(&g).expect("the fifth service that looks like the first four");
    assert!(gap.title.contains("2 of 4"), "{}", gap.title);
    assert_eq!(
        gap.affected_ids,
        vec!["cmp:c".to_string(), "cmp:d".to_string()]
    );
    assert!(
        gap.description.contains("establish-taxonomy"),
        "{}",
        gap.description
    );
    assert!((gap.severity - 0.45).abs() < 1e-9);

    // Keyed on the type and discriminator, so an acknowledgement sticks
    // when one more silent instance arrives.
    let before = gap.id.clone();
    comp(&mut g, "cmp:e", None);
    assert_eq!(find(&g).unwrap().id, before);
}

#[test]
fn an_instance_that_cites_an_accepted_decision_is_encoded_and_a_proposed_one_governs_nothing() {
    let mut g = design();
    comp(&mut g, "cmp:a", Some("service"));
    comp(&mut g, "cmp:b", Some("service"));
    comp(&mut g, "cmp:c", None);
    comp(&mut g, "cmp:d", None);
    g.add_decision(
        "dec:a-scan-plan-is-a-component-with-no-kind",
        "A scan plan is a Component with no kind",
        "Scan plans are recorded as plain components; kind is not used for them.",
        None,
    )
    .unwrap();
    for id in ["cmp:c", "cmp:d"] {
        g.governed_by(
            node::COMPONENT,
            id,
            node::DECISION,
            "dec:a-scan-plan-is-a-component-with-no-kind",
            None,
            None,
        )
        .unwrap();
    }
    assert!(
        find(&g).is_some(),
        "a proposed encoding governs nothing yet"
    );

    g.set_decision_status("dec:a-scan-plan-is-a-component-with-no-kind", "accepted")
        .unwrap();
    assert!(
        find(&g).is_none(),
        "every silent instance now cites an accepted decision"
    );
}

#[test]
fn stating_the_discriminator_on_every_instance_also_closes_it() {
    let mut g = design();
    comp(&mut g, "cmp:a", Some("service"));
    comp(&mut g, "cmp:b", Some("service"));
    comp(&mut g, "cmp:c", None);
    assert!(find(&g).is_some());
    comp(&mut g, "cmp:c", Some("module"));
    assert!(find(&g).is_none());
}

#[test]
fn a_type_where_nobody_has_said_is_asked_only_at_five() {
    let mut g = design();
    for id in ["cmp:a", "cmp:b", "cmp:c", "cmp:d"] {
        comp(&mut g, id, None);
    }
    assert!(
        find(&g).is_none(),
        "four silent components are not yet a practice"
    );
    comp(&mut g, "cmp:e", None);
    let gap = find(&g).expect("five is");
    assert!(gap.title.contains("none says"), "{}", gap.title);
    assert!((gap.severity - 0.3).abs() < 1e-9, "the weaker signal");
}

#[test]
fn a_materialised_unspecified_medium_reads_as_nothing_said() {
    let mut g = design();
    g.add_interface("ifc:a", "A").unwrap();
    g.add_interface("ifc:b", "B").unwrap();
    g.create_node(
        node::INTERFACE,
        "ifc:c",
        Props::new().set("name", "C").set("medium", "REST"),
    )
    .unwrap();
    let stored = g.get_node(node::INTERFACE, "ifc:a").unwrap().unwrap();
    assert_eq!(
        stored.properties.get("medium").and_then(Value::as_str),
        Some("unspecified"),
        "the store writes the default, which is exactly why presence is not an answer"
    );
    let gap = find(&g).expect("one contract says REST and two say nothing");
    assert!(gap.title.contains("Interface"), "{}", gap.title);
    assert_eq!(
        gap.affected_ids,
        vec!["ifc:a".to_string(), "ifc:b".to_string()]
    );
}
