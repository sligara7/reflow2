//! SEARCH — find design nodes by what they say (the `fulltext` feature).
//!
//! Two arms, like `persistence.rs`: the default build proves the feature's
//! absence fails loud (a search that silently returns nothing would read as
//! "the design says nothing about that", which is a lie), and the featured
//! build proves the real round trip. The featured arm runs in
//! `cargo test -p reflow2-core --no-default-features --features fulltext`
//! and — because reflow2-mcp enables the feature on its dependency edge —
//! its behaviour is also exercised through the surface in
//! `crates/reflow2-mcp/tests/tools.rs`.

use reflow2_core::graph::DesignGraph;

fn thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_requirement(
        "req:persist",
        "The design persists across sessions",
        "The graph survives restarts so the design outlives any one conversation.",
    )
    .expect("req");
    g.add_capability(
        "cap:score",
        "Track the score",
        "Keeps the running score of the game.",
        None,
    )
    .expect("cap");
    g
}

#[cfg(not(feature = "fulltext"))]
#[test]
fn without_the_feature_search_fails_loud_not_empty() {
    let g = thread();
    let err = g.search_design("persists", None, 10).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("fulltext") && msg.contains("cargo build"),
        "the refusal must name the feature and the fix, got: {msg}"
    );
    assert!(g.reindex_search().is_err(), "reindex refuses identically");
}

/// A graph whose requirements are worded the way real ones are — full
/// sentences — so a query can be a *question* rather than a keyword. The
/// wording deliberately mirrors two requirements from reflow2's own design that
/// a natural-language question failed to find (see
/// `a_natural_language_question_finds_the_requirement_it_asks_about`), plus a
/// distractor sharing common words so the tests prove ranking and not merely
/// retrieval.
#[cfg(feature = "fulltext")]
fn worded_like_real_requirements() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "reflow2").expect("project");
    g.add_requirement(
        "req:concurrent",
        "Several people can plan against one design without colliding",
        "A team, not a single author, must be able to work on one design at the same time, \
         with overlapping work surfaced as decidable conflicts rather than last-writer-wins.",
    )
    .expect("req");
    g.add_requirement(
        "req:upgrade",
        "A design survives a reflow2 upgrade",
        "An existing graph opens, or is refused loudly with what to do about it.",
    )
    .expect("req");
    g.add_requirement(
        "req:distractor",
        "The core is deterministic and free of model dependencies",
        "One design, one core: the runtime stays neutral to the interaction surface.",
    )
    .expect("req");
    g
}

#[cfg(feature = "fulltext")]
mod featured {
    use super::{thread, worded_like_real_requirements};

    /// The regression this whole suite exists for, and the check
    /// `ver:search-natural-language` names.
    ///
    /// Search used to AND its tokens, so a question phrased in a person's own
    /// words — inevitably containing a word the corpus never happened to use —
    /// returned NOTHING for a requirement that plainly existed. Zero hits reads
    /// as "no such thing exists", and because `capture-intent` tells the agent
    /// to search before adding and treats no-hits as licence to create, the
    /// false negative did not merely fail to answer: it manufactured duplicates.
    /// Fixed upstream in dynograph-foundation v0.11.0 by matching tokens as a
    /// ranked disjunction.
    ///
    /// Pins the CLASS, not the instance: any question whose words only
    /// partially overlap the target must still find it, and must rank it above
    /// a document sharing merely a common word.
    #[test]
    fn a_natural_language_question_finds_the_requirement_it_asks_about() {
        let g = worded_like_real_requirements();

        // Not one of these words except "people" and "time" appears in the
        // target, and "design" appears in two other requirements.
        let asked = g
            .search_design(
                "multiple people working at the same time collaboration",
                Some("Requirement"),
                10,
            )
            .expect("search");
        assert_eq!(
            asked.hits.first().map(|h| h.node_id.as_str()),
            Some("req:concurrent"),
            "a question about people working simultaneously must find the requirement \
             about it: {asked:?}"
        );

        // A different question, different target, same property.
        let upgrade = g
            .search_design(
                "upgrade an existing graph to a new version without losing it",
                Some("Requirement"),
                10,
            )
            .expect("search");
        assert_eq!(
            upgrade.hits.first().map(|h| h.node_id.as_str()),
            Some("req:upgrade"),
            "a question about upgrading without loss must find the upgrade requirement: \
             {upgrade:?}"
        );
    }

    /// The minimal pin, reduced to one variable. Under the old rule this pair
    /// was the whole bug: the bare term matched, and adding a single word the
    /// corpus never used took the result to zero.
    #[test]
    fn one_unmatched_word_does_not_erase_a_real_match() {
        let g = worded_like_real_requirements();

        let bare = g
            .search_design("upgrade", Some("Requirement"), 10)
            .expect("search");
        assert_eq!(
            bare.hits.first().map(|h| h.node_id.as_str()),
            Some("req:upgrade"),
            "the bare term must match"
        );

        let noised = g
            .search_design("upgrade zzzznotaword", Some("Requirement"), 10)
            .expect("search");
        assert_eq!(
            noised.hits.first().map(|h| h.node_id.as_str()),
            Some("req:upgrade"),
            "an unmatched extra word must lower the score, never erase the hit: {noised:?}"
        );
    }

    /// The other half of the contract, and the reason this is not simply
    /// "match anything": a disjunction must still refuse. A question sharing no
    /// vocabulary with the design has to come back empty, or "no hits" stops
    /// carrying information and the duplicate problem returns from the other
    /// direction.
    #[test]
    fn a_question_the_design_does_not_answer_still_returns_nothing() {
        let g = worded_like_real_requirements();
        let result = g
            .search_design(
                "hydraulic actuator torque specification",
                Some("Requirement"),
                10,
            )
            .expect("search");
        assert!(
            result.hits.is_empty(),
            "no shared vocabulary must still mean no hits: {result:?}"
        );
    }

    #[test]
    fn search_finds_nodes_by_their_own_words() {
        let g = thread();
        let result = g
            .search_design("persists across sessions", None, 10)
            .expect("search");
        assert!(result.stale.is_empty(), "a fresh graph has no index drift");
        assert_eq!(
            result.hits.first().map(|h| h.node_id.as_str()),
            Some("req:persist"),
            "the requirement stating those words ranks first: {result:?}"
        );
        let hit = &result.hits[0];
        assert_eq!(hit.node_type, "Requirement");
        assert_eq!(hit.name, "The design persists across sessions");
    }

    #[test]
    fn a_type_scope_narrows_without_lying() {
        let g = thread();
        let caps = g
            .search_design("score", Some("Capability"), 10)
            .expect("search");
        assert!(
            caps.hits.iter().all(|h| h.node_type == "Capability"),
            "scoped search returns only the asked-for type: {caps:?}"
        );
        assert!(!caps.hits.is_empty(), "the capability mentions score");
    }

    #[test]
    fn an_unmatched_query_returns_empty_not_error() {
        let g = thread();
        let result = g.search_design("zeppelin", None, 10).expect("search");
        assert!(result.hits.is_empty());
        assert!(result.stale.is_empty());
    }

    #[test]
    fn the_limit_is_visible_in_the_result() {
        // No silent caps: a caller can see hits.len() == limit and know the
        // list may be truncated.
        let g = thread();
        let result = g.search_design("the", None, 1).expect("search");
        assert_eq!(result.limit, 1);
        assert!(result.hits.len() <= 1);
    }

    #[test]
    fn a_revised_node_is_found_by_its_new_words_not_its_old_ones() {
        // The engine mirrors writes with replace semantics — revise-design
        // depends on search seeing the current text.
        let mut g = thread();
        g.create_node(
            "Capability",
            "cap:score",
            reflow2_core::nodes::Props::new()
                .set("name", "Track the tally")
                .set("description", "Keeps the running tally of the match.")
                .build(),
        )
        .expect("revise");

        let new = g
            .search_design("tally", Some("Capability"), 10)
            .expect("search");
        assert_eq!(
            new.hits.first().map(|h| h.node_id.as_str()),
            Some("cap:score"),
            "found by the revised wording"
        );
        let old = g
            .search_design("score", Some("Capability"), 10)
            .expect("search");
        assert!(
            old.hits.iter().all(|h| h.node_id != "cap:score"),
            "no longer found by wording it no longer carries: {old:?}"
        );
    }

    #[test]
    fn reindex_reports_how_many_nodes_it_indexed() {
        let g = thread();
        let n = g.reindex_search().expect("reindex");
        assert!(
            n >= 3,
            "project + requirement + capability at least, got {n}"
        );
        // And search still works after a rebuild.
        let result = g.search_design("persists", None, 10).expect("search");
        assert!(!result.hits.is_empty());
    }
}
