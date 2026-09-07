//! (2) An artifact with no checksum has exactly one legal disposition. A
//! `design_updated` or `design_holds` sent for it used to be refused — and
//! inside a batch, one such item discarded the rest
//! (`fact:defect-a-batch-accept-refuses-whole-for-an-artifact-whose-only-legal-disposition-is-the-first-baseline`).
//! Now it is READ as the first baseline it can only be, the note says so, and
//! no CHANGED edge is drawn — because a first baseline is "nothing moved".

use reflow2_core::artifact::DriftDisposition;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_artifact("art:seal", "seal.rs", None, Some("src/seal.rs"))
        .expect("artifact");
    g.create_node(
        node::CHANGE_EVENT,
        "chg:seal-tightened",
        Props::new()
            .set("name", "seal tightened")
            .set("change_type", "defect_fix")
            .set("subject", "system"),
    )
    .expect("event");
    g
}

#[test]
fn design_updated_on_a_checksumless_artifact_records_a_first_baseline_and_says_so() {
    let mut g = design();
    let (node, event) = g
        .set_artifact_checksum(
            "art:seal",
            "sha256:aaaa",
            DriftDisposition::DesignUpdated {
                change_event_id: "chg:seal-tightened",
            },
            Some("tightened the seal"),
            Some("2026-09-07"),
        )
        .expect("read as a first baseline, not refused");
    assert_eq!(
        node.properties.get("checksum").and_then(|v| v.as_str()),
        Some("sha256:aaaa")
    );
    assert!(
        event.starts_with("chg:baseline-"),
        "recorded as a baseline: {event}"
    );
    let ev = g
        .get_node(node::CHANGE_EVENT, &event)
        .unwrap()
        .expect("baseline event exists");
    let name = ev
        .properties
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        name.contains("FIRST BASELINE") && name.contains("tightened the seal"),
        "{name}"
    );
    // A first baseline is "nothing moved": the named change does NOT get a
    // CHANGED edge to the artifact.
    let changed = g
        .outgoing("chg:seal-tightened", Some(edge::CHANGED))
        .unwrap();
    assert!(changed.iter().all(|e| e.to_id != "art:seal"), "{changed:?}");
}

#[test]
fn once_baselined_the_same_call_is_an_ordinary_accept() {
    let mut g = design();
    g.set_artifact_checksum(
        "art:seal",
        "sha256:aaaa",
        DriftDisposition::BaselineEstablished,
        None,
        None,
    )
    .unwrap();
    let (_, event) = g
        .set_artifact_checksum(
            "art:seal",
            "sha256:bbbb",
            DriftDisposition::DesignUpdated {
                change_event_id: "chg:seal-tightened",
            },
            None,
            Some("2026-09-07"),
        )
        .unwrap();
    assert_eq!(
        event, "chg:seal-tightened",
        "a real change keeps its own event"
    );
    let changed = g
        .outgoing("chg:seal-tightened", Some(edge::CHANGED))
        .unwrap();
    assert!(changed.iter().any(|e| e.to_id == "art:seal"));
}

#[test]
fn a_baseline_that_would_move_an_existing_one_is_still_refused() {
    let mut g = design();
    g.set_artifact_checksum(
        "art:seal",
        "sha256:aaaa",
        DriftDisposition::BaselineEstablished,
        None,
        None,
    )
    .unwrap();
    let err = g
        .set_artifact_checksum(
            "art:seal",
            "sha256:bbbb",
            DriftDisposition::BaselineEstablished,
            None,
            None,
        )
        .expect_err("laundering a real change as a first baseline is refused");
    assert!(err.to_string().contains("not a first one"), "{err}");
}
