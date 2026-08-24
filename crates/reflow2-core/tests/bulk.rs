//! BL-153 fix shape (1) and (3) — the bulk forms.
//!
//! The measurement said 52.9% of every tool transition is the same tool called
//! again. These tests pin the two properties that make a bulk form worth having
//! rather than a way to damage a design faster: **all of it or none of it**, and
//! **every failure in one round trip**.

use reflow2_core::artifact::DriftDisposition;
use reflow2_core::bulk::{ChecksumAccept, EdgeSpec, GapAck, NodeSpec};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::temporal::ChangeType;

/// A well-formed Requirement — `statement` is required, so a spec carrying only
/// a name is a validation failure, not a shortcut.
fn spec(node_type: &str, id: &str, name: &str) -> NodeSpec {
    NodeSpec::new(
        node_type,
        id,
        Props::new().set("name", name).set("statement", name),
    )
}

fn edge_spec(edge_type: &str, ft: &str, f: &str, tt: &str, t: &str) -> EdgeSpec {
    EdgeSpec {
        edge_type: edge_type.to_string(),
        from_type: ft.to_string(),
        from_id: f.to_string(),
        to_type: tt.to_string(),
        to_id: t.to_string(),
        props: Default::default(),
    }
}

/// A design with a project, a requirement, a capability, a component and two
/// artifacts — enough to exercise every bulk form.
fn seeded() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").unwrap();
    g.add_requirement("req:live", "Live scores", "scores update live")
        .unwrap();
    g.add_capability("cap:score", "Scoring", "tracks the score", None)
        .unwrap();
    g.add_component("cmp:engine", "Score engine", "computes scores", None)
        .unwrap();
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .unwrap();
    g.add_artifact("art:b", "b.rs", Some("code"), Some("src/b.rs"))
        .unwrap();
    g
}

// ---- create_nodes ----------------------------------------------------------

#[test]
fn many_nodes_land_in_one_call() {
    let mut g = seeded();
    let items = [
        spec(node::REQUIREMENT, "req:one", "One"),
        spec(node::REQUIREMENT, "req:two", "Two"),
        spec(node::REQUIREMENT, "req:three", "Three"),
    ];
    let r = g.create_nodes(&items).unwrap();

    assert!(r.applied);
    assert_eq!(r.written.len(), 3);
    assert!(r.failures.is_empty());
    for id in ["req:one", "req:two", "req:three"] {
        assert!(g.get_node(node::REQUIREMENT, id).unwrap().is_some());
    }
}

#[test]
fn one_bad_item_rejects_the_whole_batch_and_writes_nothing() {
    // THE ATOMIC GUARANTEE. A partial bulk write leaves a design in a state
    // nobody chose, and the store already has the batch HEAL's apply step uses.
    let mut g = seeded();
    let items = [
        spec(node::REQUIREMENT, "req:one", "One"),
        spec("NotAType", "x:bad", "Bad"),
        spec(node::REQUIREMENT, "req:three", "Three"),
    ];
    let r = g.create_nodes(&items).unwrap();

    assert!(!r.applied);
    assert!(r.written.is_empty());
    assert_eq!(r.failures.len(), 1);
    assert!(
        g.get_node(node::REQUIREMENT, "req:one").unwrap().is_none(),
        "the item BEFORE the failure must not survive"
    );
    assert!(
        g.get_node(node::REQUIREMENT, "req:three")
            .unwrap()
            .is_none(),
        "nor the one after it"
    );
}

#[test]
fn every_failure_comes_back_not_just_the_first() {
    // BL-118's defect, which this form must not inherit: `import_graph`
    // validation is fail-fast, one error per attempt, and BL-139 records what
    // that costs. A bulk form surfacing one error per round trip would replace
    // N writes with N retries and save nobody anything.
    let mut g = seeded();
    let items = [
        spec(node::REQUIREMENT, "req:one", "One"),
        spec("NotAType", "x:bad", "Bad"),
        spec(node::REQUIREMENT, "req:three", "Three"),
        spec("AlsoNotAType", "x:worse", "Worse"),
    ];
    let r = g.create_nodes(&items).unwrap();

    assert_eq!(r.failures.len(), 2, "BOTH bad items are reported");
    assert_eq!(r.failures[0].index, 1);
    assert_eq!(r.failures[1].index, 3);
    assert_eq!(r.failures[0].id, "x:bad");
    assert_eq!(r.failures[1].id, "x:worse");
}

#[test]
fn a_failure_carries_its_position_so_a_repeated_id_is_locatable() {
    // An id alone cannot tell two identical entries apart, and a list an agent
    // generated is exactly where a duplicate turns up.
    let mut g = seeded();
    let items = [
        spec("NotAType", "x:same", "First"),
        spec("NotAType", "x:same", "Second"),
    ];
    let r = g.create_nodes(&items).unwrap();

    assert_eq!(r.failures.len(), 2);
    assert_eq!(
        (r.failures[0].index, r.failures[1].index),
        (0, 1),
        "position, not just id"
    );
}

// ---- create_edges ----------------------------------------------------------

#[test]
fn one_bulk_edge_form_covers_every_typed_helper() {
    // contains (109 self-loops), satisfies (74) and allocate all wrap
    // create_edge with the endpoint types filled in, so a bulk create_edge is
    // their bulk form too — one new tool rather than six, which BL-155's
    // unused-tool count makes the right trade.
    let mut g = seeded();
    let items = [
        edge_spec(
            edge::CONTAINS,
            node::PROJECT,
            "proj:1",
            node::REQUIREMENT,
            "req:live",
        ),
        edge_spec(
            edge::SATISFIES,
            node::CAPABILITY,
            "cap:score",
            node::REQUIREMENT,
            "req:live",
        ),
        edge_spec(
            edge::ALLOCATED_TO,
            node::CAPABILITY,
            "cap:score",
            node::COMPONENT,
            "cmp:engine",
        ),
    ];
    let r = g.create_edges(&items).unwrap();
    assert!(r.applied);
    assert_eq!(r.written.len(), 3);

    // Identical to what the typed helper produces, defaults included — a bulk
    // form that quietly dropped a schema default would be a silent difference.
    let mut helper = seeded();
    helper.allocate("cap:score", "cmp:engine").unwrap();
    let bulk_alloc = g
        .outgoing("cap:score", Some(edge::ALLOCATED_TO))
        .unwrap()
        .remove(0);
    let helper_alloc = helper
        .outgoing("cap:score", Some(edge::ALLOCATED_TO))
        .unwrap()
        .remove(0);
    assert_eq!(bulk_alloc.properties, helper_alloc.properties);
}

#[test]
fn an_edge_into_a_missing_node_rejects_the_batch() {
    let mut g = seeded();
    let items = [
        edge_spec(
            edge::CONTAINS,
            node::PROJECT,
            "proj:1",
            node::REQUIREMENT,
            "req:live",
        ),
        edge_spec(
            edge::SATISFIES,
            node::CAPABILITY,
            "cap:score",
            node::REQUIREMENT,
            "req:ghost",
        ),
    ];
    let r = g.create_edges(&items).unwrap();

    assert!(!r.applied);
    assert_eq!(r.failures.len(), 1);
    assert!(
        g.outgoing("proj:1", Some(edge::CONTAINS))
            .unwrap()
            .is_empty(),
        "the good edge is rolled back with the bad one"
    );
}

// ---- set_artifact_checksums ------------------------------------------------

/// Give an artifact its first baseline, so a later accept has something to
/// accept a change *against* (BL-157).
fn baseline(g: &mut DesignGraph, artifact_id: &str) {
    g.set_artifact_checksum(
        artifact_id,
        "sha256:000",
        DriftDisposition::BaselineEstablished,
        None,
        Some("2026-07-31"),
    )
    .unwrap();
}

#[test]
fn each_accepted_checksum_keeps_its_own_disposition() {
    // THE COUNTERWEIGHT THAT MATTERS. BL-153 named this exact trap: "a batch of
    // 144 acknowledgements with one reason is exactly the erosion those
    // decisions exist to prevent". Hoisting one disposition to the call would
    // make this the bulk accept dec:two-sided-accept exists to forbid. Two
    // items, two DIFFERENT dispositions, both honoured.
    let mut g = seeded();
    // Both artifacts start FROM a baseline: `design_holds`/`design_updated`
    // claim something about a movement, and there is no movement without one
    // (BL-157). Establishing it is a separate, earlier act.
    baseline(&mut g, "art:a");
    baseline(&mut g, "art:b");
    g.add_change_event(
        "chg:real",
        "A real design change",
        ChangeType::NewFeature,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let items = [
        ChecksumAccept {
            artifact_id: "art:a".into(),
            checksum: "sha256:aaa".into(),
            disposition: DriftDisposition::DesignHolds {
                change_type: ChangeType::Refactor,
            },
            note: Some("no design meaning".into()),
            at: Some("2026-08-01".into()),
        },
        ChecksumAccept {
            artifact_id: "art:b".into(),
            checksum: "sha256:bbb".into(),
            disposition: DriftDisposition::DesignUpdated {
                change_event_id: "chg:real",
            },
            note: None,
            at: Some("2026-08-01".into()),
        },
    ];
    let r = g.set_artifact_checksums(&items).unwrap();

    assert!(r.applied);
    assert_eq!(r.written.len(), 2);
    let (_, a_event) = &r.written[0];
    let (_, b_event) = &r.written[1];
    assert_ne!(
        a_event, b_event,
        "two dispositions must not collapse into one record"
    );
    assert_eq!(
        b_event, "chg:real",
        "design_updated points at the ChangeEvent the caller named"
    );
    assert_ne!(
        a_event, "chg:real",
        "design_holds mints its own dated claim and never borrows the other's event"
    );

    for (id, sum) in [("art:a", "sha256:aaa"), ("art:b", "sha256:bbb")] {
        let n = g.get_node(node::ARTIFACT, id).unwrap().unwrap();
        assert_eq!(n.properties["checksum"].as_str(), Some(sum));
    }
}

#[test]
fn one_unknown_artifact_rejects_every_accept_in_the_batch() {
    let mut g = seeded();
    baseline(&mut g, "art:a");
    let items = [
        ChecksumAccept {
            artifact_id: "art:a".into(),
            checksum: "sha256:aaa".into(),
            disposition: DriftDisposition::DesignHolds {
                change_type: ChangeType::Refactor,
            },
            note: None,
            at: None,
        },
        ChecksumAccept {
            artifact_id: "art:ghost".into(),
            checksum: "sha256:zzz".into(),
            disposition: DriftDisposition::DesignHolds {
                change_type: ChangeType::Refactor,
            },
            note: None,
            at: None,
        },
    ];
    let r = g.set_artifact_checksums(&items).unwrap();

    assert!(!r.applied);
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].id, "art:ghost");
    let a = g.get_node(node::ARTIFACT, "art:a").unwrap().unwrap();
    assert_eq!(
        a.properties["checksum"].as_str(),
        Some("sha256:000"),
        "a baseline must not move because a LATER item in the list was wrong"
    );
}

// ---- acknowledge_gaps ------------------------------------------------------

#[test]
fn each_acknowledged_gap_keeps_its_own_reason() {
    // The other half of the same rule. BL-153 flagged acknowledge_gap as the
    // case where a bulk form could be worse than the loop; the reason stays per
    // gap and only the round trip collapses.
    let mut g = seeded();
    let items = [
        GapAck {
            gap_id: "gap:one".into(),
            affected_ids: vec!["req:live".into()],
            reason: "accepted because the first thing is fine".into(),
            approver: None,
            acted_at: None,
        },
        GapAck {
            gap_id: "gap:two".into(),
            affected_ids: vec!["cap:score".into()],
            reason: "accepted for an entirely different reason".into(),
            approver: None,
            acted_at: None,
        },
    ];
    let r = g.acknowledge_gaps(&items).unwrap();

    assert!(r.applied);
    assert_eq!(r.written.len(), 2);

    let reviewed = g.reviewed_gaps().unwrap();
    let reasons: Vec<&str> = reviewed.iter().map(|x| x.reason.as_str()).collect();
    assert!(reasons.contains(&"accepted because the first thing is fine"));
    assert!(reasons.contains(&"accepted for an entirely different reason"));
    assert_ne!(
        reasons[0], reasons[1],
        "two gaps must not end up sharing one rationale"
    );
}
