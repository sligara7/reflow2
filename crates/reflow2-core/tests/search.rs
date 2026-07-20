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

#[cfg(feature = "fulltext")]
mod featured {
    use super::thread;

    #[test]
    fn search_finds_nodes_by_their_own_words() {
        let g = thread();
        let result = g.search_design("persists across sessions", None, 10).expect("search");
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

        let new = g.search_design("tally", Some("Capability"), 10).expect("search");
        assert_eq!(
            new.hits.first().map(|h| h.node_id.as_str()),
            Some("cap:score"),
            "found by the revised wording"
        );
        let old = g.search_design("score", Some("Capability"), 10).expect("search");
        assert!(
            old.hits.iter().all(|h| h.node_id != "cap:score"),
            "no longer found by wording it no longer carries: {old:?}"
        );
    }

    #[test]
    fn reindex_reports_how_many_nodes_it_indexed() {
        let g = thread();
        let n = g.reindex_search().expect("reindex");
        assert!(n >= 3, "project + requirement + capability at least, got {n}");
        // And search still works after a rebuild.
        let result = g.search_design("persists", None, 10).expect("search");
        assert!(!result.hits.is_empty());
    }
}
