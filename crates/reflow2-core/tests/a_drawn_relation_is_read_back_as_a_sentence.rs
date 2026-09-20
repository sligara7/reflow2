//! A drawn relation is read back as a sentence, subject first.
//!
//! flo2, 2026-09-19: they landed two `BLOCKS` edges asserting the reverse of
//! what their own `evidence` prose said, in one session, and caught both only by
//! re-reading their own call. The direction flag lets a caller draw the edge
//! they meant; it does nothing to confirm they got it right.
//!
//! The reply used to label each edge `BLOCKS -> cmp:artifact-store`, which omits
//! the subject. A reader who has just written the call supplies the subject they
//! INTENDED rather than the one they sent, so a reversal reads as correct.
//! `req:deleting-an-artifact BLOCKS cmp:artifact-store` reads wrong immediately
//! when it is wrong.

use reflow2_core::DesignGraph;
use reflow2_core::relate::RelationLink;

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("proj:p", "P").expect("project");
    g.add_component("cmp:store", "Artifact store", "Where files live.", None)
        .expect("component");
    g.add_requirement(
        "req:deleting",
        "Deleting an artifact keeps its history",
        "A deleted artifact keeps what it was.",
    )
    .expect("requirement");
    g
}

fn link(relation: &str, other_type: &str, other_id: &str, incoming: bool) -> RelationLink {
    RelationLink {
        relation: relation.to_string(),
        other_type: other_type.to_string(),
        other_id: other_id.to_string(),
        evidence: "The store, as built, blocks the requirement.".to_string(),
        incoming,
    }
}

#[test]
fn an_outgoing_relation_reads_subject_first() {
    let mut g = design();
    let out = g
        .review_relations(
            "Requirement",
            "req:deleting",
            &[link("BLOCKS", "Component", "cmp:store", false)],
            None,
        )
        .expect("review");
    assert_eq!(
        out.drawn,
        vec!["req:deleting BLOCKS cmp:store".to_string()],
        "the echo must name the subject, the relation and the object, in that order"
    );
}

#[test]
fn an_incoming_relation_reads_with_the_other_node_as_the_subject() {
    let mut g = design();
    let out = g
        .review_relations(
            "Requirement",
            "req:deleting",
            &[link("BLOCKS", "Component", "cmp:store", true)],
            None,
        )
        .expect("review");
    // THE POINT OF THE CHANGE: the two directions now read as DIFFERENT
    // sentences. Under the old label both were "BLOCKS … cmp:store" with only
    // an arrow glyph between them, which is what let a reversal pass a reader.
    assert_eq!(
        out.drawn,
        vec!["cmp:store BLOCKS req:deleting".to_string()],
        "an inbound edge must read with the other node as the subject"
    );
}

#[test]
fn the_two_directions_do_not_read_alike() {
    let mut g = design();
    let outward = g
        .review_relations(
            "Requirement",
            "req:deleting",
            &[link("BLOCKS", "Component", "cmp:store", false)],
            None,
        )
        .expect("review")
        .drawn;
    let mut h = design();
    let inward = h
        .review_relations(
            "Requirement",
            "req:deleting",
            &[link("BLOCKS", "Component", "cmp:store", true)],
            None,
        )
        .expect("review")
        .drawn;
    assert_ne!(
        outward, inward,
        "if the two directions render the same, the echo cannot catch a reversal"
    );
    for line in outward.iter().chain(inward.iter()) {
        assert!(
            line.starts_with("req:") || line.starts_with("cmp:"),
            "every echo starts with its subject: {line}"
        );
    }
}
