//! A ChangeEvent's reasoning had nowhere to go through the front door.
//!
//! `ChangeEvent` has declared `summary` (indexed, full-text, the embedding
//! field) and `rationale` ("why the change was made") since the schema was
//! written — and the constructor took NEITHER. So the only way to record why a
//! change happened was a second write, and two projects independently reported
//! the same workaround on the same day (2026-08-19): a follow-up `create_node`
//! hanging an UNDECLARED `description` on the event.
//!
//! ⭐ THE COST WAS NOT THE EXTRA CALL. `undeclared` exists to catch typos, and
//! it was firing on every legitimate write of the field the skills tell you to
//! fill in — which trains the caller to ignore it. A flag that fires on correct
//! behaviour stops being a signal, which is the same shape as a deliberate
//! state counting as a structural defect.
//!
//! These tests pin the front door, not the storage: `upsert_node` could always
//! hold the text, and that is exactly why nothing noticed.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::{ChangeSubject, ChangeType};

fn prop(g: &DesignGraph, id: &str, key: &str) -> Option<String> {
    g.get_node(node::CHANGE_EVENT, id)
        .expect("get")
        .expect("event")
        .properties
        .get(key)
        .and_then(|v| v.as_str().map(str::to_string))
}

#[test]
fn the_constructor_records_why_without_a_second_write() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:ws-dep-regression",
        "websocket dep bumped",
        ChangeType::DefectFix,
        Some(ChangeSubject::System),
        Some("The 0.21 bump changed close-frame timing and the reconnect test went flaky."),
        Some("Pinned to 0.20 and added a close-frame assertion so the timing is checked \
              rather than tolerated."),
    )
    .expect("one call must be enough");

    assert_eq!(
        prop(&g, "chg:ws-dep-regression", "summary").as_deref(),
        Some("The 0.21 bump changed close-frame timing and the reconnect test went flaky."),
        "summary is the indexed, searchable field — it must survive the constructor"
    );
    assert!(
        prop(&g, "chg:ws-dep-regression", "rationale")
            .expect("rationale")
            .contains("Pinned to 0.20"),
        "the lesson is the part worth recording, and it must not need a second call"
    );
}

/// Both fields stay OPTIONAL. An event with nothing to add must not be forced
/// to invent prose — the old four-argument shape stays honest.
#[test]
fn text_is_optional_and_absence_writes_nothing() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event("chg:bare", "bare event", ChangeType::Resync, None, None, None)
        .expect("no text is a true answer");

    assert!(prop(&g, "chg:bare", "summary").is_none());
    assert!(prop(&g, "chg:bare", "rationale").is_none());
}

/// The merge contract holds for the new fields too: a later call that omits
/// them must not erase what an earlier one wrote. This is the property the
/// reports leaned on when they worked around the gap, and it must survive the
/// gap being closed.
#[test]
fn a_later_call_that_omits_the_text_does_not_erase_it() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_change_event(
        "chg:keep",
        "first",
        ChangeType::Refactor,
        None,
        Some("what changed"),
        Some("why it changed"),
    )
    .expect("first write");

    g.add_change_event("chg:keep", "renamed", ChangeType::Refactor, None, None, None)
        .expect("second write");

    assert_eq!(prop(&g, "chg:keep", "name").as_deref(), Some("renamed"));
    assert_eq!(
        prop(&g, "chg:keep", "summary").as_deref(),
        Some("what changed"),
        "omitting a field must keep its value — the BL-183 promise"
    );
    assert_eq!(
        prop(&g, "chg:keep", "rationale").as_deref(),
        Some("why it changed")
    );
}
