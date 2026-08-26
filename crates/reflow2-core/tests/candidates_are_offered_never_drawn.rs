//! `relation_candidates` — the missing half of half-idea linking.
//!
//! `unreviewed_ideas` counts the ideas connected to nothing and
//! `review_relations` records the judgement, but until this existed nothing
//! answered the question a person working that backlog actually has: WHICH OF
//! THESE BELONG TOGETHER?
//!
//! The tests that matter most are the refusals. A suggester that offered a
//! candidate it could not explain, or that quietly returned an empty list, or
//! that re-proposed something already linked, would manufacture exactly the
//! false neighbours the brainstorm skill forbids — and a false neighbour is
//! worse than a missing one, because anything searching by neighbourhood
//! repeats it forever.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn idea(g: &mut DesignGraph, id: &str, name: &str, text: &str) {
    g.create_node(
        node::DECISION,
        id,
        Props::new()
            .set("name", name)
            .set("decision", text)
            .set("status", "proposed"),
    )
    .unwrap();
}

#[test]
fn a_shared_distinctive_term_surfaces_a_candidate_and_says_so() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    idea(
        &mut g,
        "dec:a",
        "A",
        "Leiden clustering over the allocation coupling graph.",
    );
    idea(
        &mut g,
        "dec:b",
        "B",
        "The allocation coupling graph is what Leiden needs.",
    );
    idea(
        &mut g,
        "dec:c",
        "C",
        "Sandwiches, picnics and the weather in June.",
    );

    let r = g
        .relation_candidates("Decision", "dec:a", Some("Decision"), 5)
        .unwrap();

    assert_eq!(r.candidates[0].node_id, "dec:b");
    assert!(
        r.candidates[0]
            .because
            .iter()
            .any(|b| b.contains("distinctive term")),
        "the reason must name the signal, got: {:?}",
        r.candidates[0].because
    );
    assert!(
        !r.candidates.iter().any(|c| c.node_id == "dec:c"),
        "an unrelated idea must not be offered at all"
    );
}

#[test]
fn a_shared_neighbour_outranks_shared_words() {
    // The graph asserting a common subject is a stronger claim than two
    // documents using the same word, and the ranking must say so.
    let mut g = DesignGraph::open_in_memory().unwrap();
    idea(&mut g, "dec:subject", "S", "hydrate rocksdb export durable");
    idea(&mut g, "dec:wordy", "W", "hydrate rocksdb export durable");
    idea(
        &mut g,
        "dec:structural",
        "T",
        "entirely different vocabulary here",
    );
    g.add_requirement("req:shared", "Shared", "A third node.")
        .unwrap();
    for from in ["dec:subject", "dec:structural"] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::DECISION,
            from,
            node::REQUIREMENT,
            "req:shared",
            Props::new(),
        )
        .unwrap();
    }

    let r = g
        .relation_candidates("Decision", "dec:subject", Some("Decision"), 5)
        .unwrap();

    assert_eq!(
        r.candidates[0].node_id,
        "dec:structural",
        "the structural signal must win, got: {:?}",
        r.candidates
            .iter()
            .map(|c| (&c.node_id, c.score))
            .collect::<Vec<_>>()
    );
    assert!(
        r.candidates[0]
            .because
            .iter()
            .any(|b| b.contains("both relate to")),
        "and it must say which third node, got: {:?}",
        r.candidates[0].because
    );
}

#[test]
fn something_already_related_is_excluded_and_reported_not_silently_dropped() {
    // "not offered because already linked" and "not offered because nothing
    // matched" are different facts and must never look the same.
    let mut g = DesignGraph::open_in_memory().unwrap();
    idea(
        &mut g,
        "dec:a",
        "A",
        "Leiden clustering allocation coupling.",
    );
    idea(
        &mut g,
        "dec:b",
        "B",
        "Leiden clustering allocation coupling.",
    );
    g.create_edge(
        edge::DEPENDS_ON,
        node::DECISION,
        "dec:a",
        node::DECISION,
        "dec:b",
        Props::new(),
    )
    .unwrap();

    let r = g
        .relation_candidates("Decision", "dec:a", Some("Decision"), 5)
        .unwrap();

    assert!(
        !r.candidates.iter().any(|c| c.node_id == "dec:b"),
        "an existing relation must not be re-proposed"
    );
    assert!(
        r.already_related.contains(&"dec:b".to_string()),
        "and the caller must be able to SEE it was excluded, got: {:?}",
        r.already_related
    );
}

#[test]
fn a_word_true_of_everything_carries_no_signal() {
    // Rarity is measured across the POOL, not against English. If every idea
    // says "allocation", saying "allocation" tells you nothing about which two
    // belong together.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for i in 0..8 {
        idea(
            &mut g,
            &format!("dec:{i}"),
            "N",
            "allocation allocation allocation",
        );
    }
    let r = g
        .relation_candidates("Decision", "dec:0", Some("Decision"), 5)
        .unwrap();

    assert!(
        r.candidates.is_empty(),
        "a term in every node must not manufacture 7 neighbours, got: {:?}",
        r.candidates.iter().map(|c| &c.node_id).collect::<Vec<_>>()
    );
}

#[test]
fn every_offered_candidate_can_say_why() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    idea(
        &mut g,
        "dec:a",
        "A",
        "hydrate the export into a session cache",
    );
    idea(
        &mut g,
        "dec:b",
        "B",
        "the session cache is hydrated from the export",
    );
    let r = g
        .relation_candidates("Decision", "dec:a", Some("Decision"), 5)
        .unwrap();
    for c in &r.candidates {
        assert!(
            !c.because.is_empty(),
            "{} was offered with no reason — a candidate that cannot be explained \
             is a false neighbour waiting to happen",
            c.node_id
        );
    }
}

#[test]
fn an_empty_answer_says_which_empty_it_is() {
    // Three different facts hide behind an empty list, and only one of them
    // means the node is genuinely unrelated to everything.
    let mut g = DesignGraph::open_in_memory().unwrap();

    // (a) nothing to compare against
    idea(&mut g, "dec:lonely", "L", "a thought entirely on its own");
    let r = g
        .relation_candidates("Decision", "dec:lonely", Some("Decision"), 5)
        .unwrap();
    assert_eq!(r.pool_examined, 0);
    let why = r.empty_because.expect("an empty list must explain itself");
    assert!(why.contains("nothing to compare against"), "got: {why}");

    // (c) ranked, and genuinely nothing matched
    idea(&mut g, "dec:other", "O", "sandwiches picnics weather");
    let r = g
        .relation_candidates("Decision", "dec:lonely", Some("Decision"), 5)
        .unwrap();
    assert_eq!(r.pool_examined, 1);
    let why = r.empty_because.expect("still empty, still explained");
    assert!(
        why.contains("genuinely new"),
        "the honest reading must be offered as a real answer, got: {why}"
    );
}

#[test]
fn asking_about_a_node_that_does_not_exist_is_refused() {
    let g = DesignGraph::open_in_memory().unwrap();
    let err = g
        .relation_candidates("Decision", "dec:nope", Some("Decision"), 5)
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("dec:nope"),
        "the refusal must name it, got: {msg}"
    );
}

// ---- the other half of the gap: an idea that contradicts INTENT ------------

/// Measured 2026-08-25: an idea CONTRADICTS-ing an accepted Decision scored +2
/// and surfaced in `what_next`, while one contradicting a REQUIREMENT stored
/// the edge and scored ZERO, because the set was built from Decisions alone.
/// The unread case is the more serious one — a Decision records a choice, a
/// Requirement records intent, and contradicting intent means either the intent
/// is wrong or the idea is out of scope.
#[test]
fn an_idea_contradicting_settled_intent_is_no_longer_invisible() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:settled", "Settled", "This must hold.")
        .unwrap();
    g.set_requirement_status("req:settled", "accepted").unwrap();
    idea(
        &mut g,
        "dec:clash",
        "Clash",
        "This cannot hold alongside that.",
    );
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:clash",
        node::REQUIREMENT,
        "req:settled",
        Props::new(),
    )
    .unwrap();

    let next = g.what_next(10).unwrap();
    let row = next
        .ranked
        .iter()
        .find(|d| d.decision_id == "dec:clash")
        .expect("an idea contradicting settled intent must be ranked, not score zero");

    assert!(row.score >= 2, "it must actually score, got {}", row.score);
    assert!(
        row.because
            .iter()
            .any(|b| b.contains("settled requirement")),
        "and it must say WHICH kind of clash — a choice and an intent are \
         answered differently, got: {:?}",
        row.because
    );
}

#[test]
fn contradicting_intent_the_user_settled_out_is_not_a_tension() {
    // `dropped` and `deferred` are the user's word too. Contradicting
    // something already abandoned is not a clash anybody needs to resolve.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:dropped", "Dropped", "We are not doing this.")
        .unwrap();
    g.set_requirement_status("req:dropped", "dropped").unwrap();
    idea(
        &mut g,
        "dec:clash",
        "Clash",
        "This cannot hold alongside that.",
    );
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:clash",
        node::REQUIREMENT,
        "req:dropped",
        Props::new(),
    )
    .unwrap();

    let next = g.what_next(10).unwrap();
    let scored = next
        .ranked
        .iter()
        .find(|d| d.decision_id == "dec:clash")
        .map(|d| d.score)
        .unwrap_or(0);
    assert_eq!(scored, 0, "a dropped requirement is not a live tension");
}
