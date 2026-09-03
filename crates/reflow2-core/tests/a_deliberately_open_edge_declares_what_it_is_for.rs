//! A wildcard that is a DESIGN CHOICE is ranked above one that is an oversight.
//!
//! `describe_schema` ranked by how narrowly an edge is DECLARED and presented
//! that as how well the edge MODELS the caller's pair. Those come apart exactly
//! for edges whose openness is deliberate — and three reporters paid for it:
//!
//! · `proj:chama` wanted "this open decision decides whether this constraint is
//!   met". `Constraint --GOVERNED_BY--> Decision` is that edge; it sat in the
//!   alphabetical wildcard pile and they drew `BLOCKS` instead.
//! · dev_storyflow wanted "this repair invalidates that check's last run".
//!   `INVALIDATES` names Constraint→Verification in its own hint; the ranking
//!   PROMOTED TWO EDGES THEY CORRECTLY JUDGED WRONG above it, and they drew
//!   `CONTRADICTS` as an admitted stand-in.
//!
//! THE FIX IS ADVISORY, NEVER VALIDATING. Narrowing `from`/`to` is a
//! consumer-facing break — it is why the two earlier instances of this class
//! could not take the fix that closed the first two. Declaring what an edge is
//! FOR changes only the ranking and the explanation; the edge still accepts
//! exactly what it accepted before.

use reflow2_core::DesignGraph;

fn g() -> DesignGraph {
    DesignGraph::open_in_memory().expect("schema loads")
}

#[test]
fn a_deliberately_open_edge_outranks_the_wildcard_pile_for_the_pair_it_is_for() {
    // chama's case, in the direction the edge actually reads.
    let q = g().edge_types_between("Constraint", "Decision").unwrap();
    let pos = q
        .matches
        .iter()
        .position(|m| m.spec.edge_type == "GOVERNED_BY")
        .expect("GOVERNED_BY accepts this pair");
    let universal = q
        .matches
        .iter()
        .position(|m| m.spec.edge_type == "BLOCKS")
        .expect("BLOCKS accepts anything");
    assert!(
        pos < universal,
        "GOVERNED_BY declares it is FOR this pair; BLOCKS is open because \
         anything can block anything. Got GOVERNED_BY at {pos}, BLOCKS at {universal}"
    );
    assert!(
        q.modelled_open_matches > 0,
        "the pair is served by an edge whose openness is deliberate"
    );
}

#[test]
fn the_storyflow_case_stops_promoting_edges_the_reporter_judged_wrong() {
    // They ran exactly this and got CONSTRAINS and CALIBRATED_AGAINST above
    // INVALIDATES, which is the edge whose own hint names this pair.
    let q = g()
        .edge_types_between("Constraint", "Verification")
        .unwrap();
    let inv = q
        .matches
        .iter()
        .position(|m| m.spec.edge_type == "INVALIDATES")
        .expect("INVALIDATES accepts this pair");
    for wrong in ["CONSTRAINS", "CALIBRATED_AGAINST"] {
        if let Some(p) = q.matches.iter().position(|m| m.spec.edge_type == wrong) {
            assert!(
                inv < p,
                "{wrong} names one endpoint but models something else; \
                 INVALIDATES declares it is FOR repair→finding. Got INVALIDATES \
                 at {inv}, {wrong} at {p}"
            );
        }
    }
}

#[test]
fn a_genuinely_universal_edge_is_not_promoted() {
    // The discrimination has to cut. BLOCKS, CAUSES and RISKS name no types in
    // their hints because anything really can block, cause or risk anything —
    // marking them too would restore the flat pile under a new name.
    let q = g().edge_types_between("Requirement", "Capability").unwrap();
    for universal in ["BLOCKS", "CAUSES", "RISKS"] {
        let m = q
            .matches
            .iter()
            .find(|m| m.spec.edge_type == universal)
            .expect("accepts anything");
        assert!(
            !m.declared_for_this_pair,
            "{universal} is open because it is universal, not because it is FOR this pair"
        );
    }
}

#[test]
fn the_reply_tells_a_hidden_answer_apart_from_an_absent_one() {
    // The sharpest half of the finding: before this, "the answer is in the pile
    // and I am not ranking it" and "there is no answer, ask for one" rendered
    // identically — same counts, same shape, opposite required actions.
    let g = g();
    let hidden = g.edge_types_between("Constraint", "Decision").unwrap();
    let absent = g.edge_types_between("Release", "Contributor").unwrap();

    assert!(
        hidden.modelled_open_matches > 0,
        "an edge declares itself for this pair"
    );
    assert_eq!(
        absent.modelled_open_matches, 0,
        "nothing declares itself for this pair"
    );
    assert_ne!(
        hidden.note, absent.note,
        "the two situations must not read the same"
    );
}
