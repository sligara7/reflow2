//! A jot carries the tag the person's word gave it, and settles by that tag.
//!
//! `/jot` is one capture with three words (req:a-jot-captures-an-idea-in-one-
//! breath-and-sorts-it-later, settled by Anthony 2026-09-24): `/log-issue`
//! tags the note as an ISSUE, `/note` as an IDEA, and a bare `/jot` leaves it
//! untagged. The word the person typed carries the kind, so nothing is ever
//! asked at capture. The tag rides on `fact_type` — `follow_up:issue`,
//! `follow_up:idea`, plain `follow_up` — so every follow-up captured before
//! tags existed still reads, as untagged, because nobody said.
//!
//! The other half is the boundary: one list of open notes, each showing its
//! tag and where settling it goes — an issue toward root-cause, an idea toward
//! brainstorm or capture-intent, an untagged one sorted with the person.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:shop", "Shop").unwrap();
    g
}

fn jot(g: &mut DesignGraph, id: &str, fact_type: &str, words: &str, on: &str) {
    g.create_node(
        node::TEMPORAL_FACT,
        id,
        Props::new()
            .set("name", words)
            .set("statement", words)
            .set("subject_id", "prj:shop")
            .set("fact_type", fact_type)
            .set("basis", "measured")
            .set("valid_from", on),
    )
    .unwrap();
}

#[test]
fn each_word_gives_its_tag_and_its_road_to_settling() {
    let mut g = design();
    jot(
        &mut g,
        "fact:follow-up-door-sticks",
        "follow_up:issue",
        "the side door sticks when it rains",
        "2026-09-24",
    );
    jot(
        &mut g,
        "fact:follow-up-loft",
        "follow_up:idea",
        "a loft over the bench for lumber",
        "2026-09-24",
    );
    jot(
        &mut g,
        "fact:follow-up-call-the-county",
        "follow_up",
        "call the county about the setback",
        "2026-09-24",
    );

    let open = g.open_follow_ups().unwrap();
    assert_eq!(open.len(), 3, "all three words are one list: {open:?}");
    let by = |id: &str| open.iter().find(|f| f.fact_id == id).unwrap();

    let issue = by("fact:follow-up-door-sticks");
    assert_eq!(issue.tag.as_deref(), Some("issue"));
    assert!(
        issue.settle_toward.contains("root-cause"),
        "{}",
        issue.settle_toward
    );

    let idea = by("fact:follow-up-loft");
    assert_eq!(idea.tag.as_deref(), Some("idea"));
    assert!(
        idea.settle_toward.contains("brainstorm"),
        "{}",
        idea.settle_toward
    );
    assert!(
        idea.settle_toward.contains("capture-intent"),
        "{}",
        idea.settle_toward
    );

    let bare = by("fact:follow-up-call-the-county");
    assert_eq!(bare.tag, None, "a bare /jot is untagged — nobody said");
    assert!(
        bare.settle_toward.contains("the person"),
        "{}",
        bare.settle_toward
    );
}

#[test]
fn the_boundary_line_counts_open_notes_by_their_tag() {
    let mut g = design();
    jot(
        &mut g,
        "fact:follow-up-a",
        "follow_up:issue",
        "a",
        "2026-09-20",
    );
    jot(
        &mut g,
        "fact:follow-up-b",
        "follow_up:issue",
        "b",
        "2026-09-21",
    );
    jot(
        &mut g,
        "fact:follow-up-c",
        "follow_up:idea",
        "c",
        "2026-09-22",
    );
    jot(&mut g, "fact:follow-up-d", "follow_up", "d", "2026-09-23");

    let ls = g.loop_status().unwrap();
    assert_eq!(ls.follow_ups_open, 4);
    let line = ls
        .next
        .iter()
        .find(|l| l.contains("follow_ups"))
        .expect("an open note is named at the boundary");
    assert!(line.contains("2 issue"), "{line}");
    assert!(line.contains("1 idea"), "{line}");
    assert!(line.contains("1 untagged"), "{line}");
}

#[test]
fn an_untagged_row_carries_no_tag_key_and_a_tagged_one_does() {
    let mut g = design();
    jot(&mut g, "fact:follow-up-a", "follow_up", "a", "2026-09-20");
    jot(
        &mut g,
        "fact:follow-up-b",
        "follow_up:idea",
        "b",
        "2026-09-21",
    );
    let rows = serde_json::to_value(g.open_follow_ups().unwrap()).unwrap();
    assert!(
        rows[0].get("tag").is_none(),
        "absent means nobody said: {rows}"
    );
    assert_eq!(rows[1]["tag"], "idea");
    assert!(rows[0]["settle_toward"].is_string() && rows[1]["settle_toward"].is_string());
}

#[test]
fn only_the_follow_up_kind_and_its_tags_are_notes() {
    let mut g = design();
    jot(
        &mut g,
        "fact:a-finding",
        "finding",
        "not a note",
        "2026-09-20",
    );
    jot(
        &mut g,
        "fact:a-plural",
        "follow_ups",
        "not a note either",
        "2026-09-20",
    );
    jot(
        &mut g,
        "fact:follow-up-empty-tag",
        "follow_up:",
        "an empty tag is no tag",
        "2026-09-20",
    );
    let open = g.open_follow_ups().unwrap();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].fact_id, "fact:follow-up-empty-tag");
    assert_eq!(open[0].tag, None);
}
