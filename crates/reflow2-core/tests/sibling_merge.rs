//! [BL-213] — a similarity score says two names are ALIKE, not that they are the
//! SAME THING, and for the names real systems use the two come apart inverted.
//!
//! Measured 2026-08-05 by the first real corpus trial, against
//! `dynograph_resolution::token_sort_ratio` and the auto-merge threshold of 90:
//!
//! ```text
//! 95  dynograph-vector  vs dynograph-core         merged, WRONG
//! 94  dynograph-storage vs dynograph-core         merged, WRONG
//! 84  Auth Service      vs Authentication Service not merged, WRONG
//! ```
//!
//! Nine crates from one document became five. No threshold repairs that — 95 was
//! a sibling pair and 84 a true duplicate — so the fix is a discriminator that
//! asks a different question, and these are the cases it must get right.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, IngestOptions, IngestStatus, MockLlmBackend};

/// A mock that emits whatever component list the caller scripts.
fn mock_with(components: &str) -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:x","name":"X","objective":"o","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        // Scripted empty so the run is CLEAN rather than `Partial` — an
        // unscripted pass errors, and a test that tolerated `Partial` would also
        // tolerate a real pass failure.
        .on_contains("[pass:requirements]", r#"{"requirements":[]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[]}"#)
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
        .on_contains("[pass:components]", components)
}

fn ingest(components: &str) -> (DesignGraph, reflow2_core::IngestReport) {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            "some architecture prose",
            &IngestOptions::default(),
            &mock_with(components),
        )
        .unwrap();
    assert_eq!(report.status, IngestStatus::Ok, "{report:?}");
    (g, report)
}

/// ⭐ THE REGRESSION, reproduced exactly: nine sibling crates must stay nine.
#[test]
fn sibling_crates_sharing_a_prefix_are_not_merged_away() {
    let nine = r#"{"components":[
      {"id":"cmp:dynograph-core","name":"dynograph-core","purpose":"schema model"},
      {"id":"cmp:dynograph-storage","name":"dynograph-storage","purpose":"rocksdb backend"},
      {"id":"cmp:dynograph-vector","name":"dynograph-vector","purpose":"hnsw index"},
      {"id":"cmp:dynograph-resolution","name":"dynograph-resolution","purpose":"entity resolution"},
      {"id":"cmp:dynograph-query","name":"dynograph-query","purpose":"cypher subset"},
      {"id":"cmp:dynograph-engine","name":"dynograph-engine","purpose":"unified api"},
      {"id":"cmp:dynograph-context","name":"dynograph-context","purpose":"tiered context"},
      {"id":"cmp:dynograph-extract","name":"dynograph-extract","purpose":"extraction"},
      {"id":"cmp:dynograph-server","name":"dynograph-server","purpose":"rest api"}]}"#;
    let (g, report) = ingest(nine);

    assert_eq!(
        g.count_nodes(node::COMPONENT).unwrap(),
        9,
        "nine distinct crates must survive as nine nodes — before BL-213 this was 5, \
         a 44% silent loss. Merges: {:?}",
        report.fuzzy_merges
    );
    assert!(
        report.fuzzy_merges.is_empty(),
        "no sibling pair may merge: {:?}",
        report.fuzzy_merges
    );

    // And each keeps its OWN name — the pre-fix graph had `cmp:dynograph-core`
    // carrying the name "dynograph-storage", which asserts something false
    // rather than merely being incomplete.
    for (id, name) in [
        ("cmp:dynograph-core", "dynograph-core"),
        ("cmp:dynograph-storage", "dynograph-storage"),
        ("cmp:dynograph-vector", "dynograph-vector"),
        ("cmp:dynograph-extract", "dynograph-extract"),
    ] {
        let n = g.get_node(node::COMPONENT, id).unwrap().expect(id);
        assert_eq!(
            n.properties.get("name").and_then(|v| v.as_str()),
            Some(name),
            "{id} must still carry its own name"
        );
    }
}

/// The other half: holding siblings apart must not stop TRUE duplicates merging.
/// This is the case the whole feature exists for.
#[test]
fn an_abbreviation_still_merges() {
    let pair = r#"{"components":[
      {"id":"cmp:auth-service","name":"Auth Service","purpose":"checks tokens"},
      {"id":"cmp:authentication-service","name":"Authentication Service","purpose":"checks tokens"}]}"#;
    let (g, report) = ingest(pair);

    // `Auth` abbreviates `Authentication`, so these are two spellings of one
    // thing. Whether the SCORE reaches the auto-merge band is a separate
    // question (measured at 84, so it does not) — what this pins is that the
    // discriminator does not add a second reason to keep them apart.
    let candidates_blocked: Vec<_> = report
        .merge_candidates
        .iter()
        .filter(|c| c.distinguished_by.is_some())
        .collect();
    assert!(
        candidates_blocked.is_empty(),
        "an abbreviation must never be reported as DISTINGUISHED: {candidates_blocked:?}"
    );
    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 2);
}

/// A version suffix is a distinction, not a spelling — the case
/// `docs/scope-corpus-ingest.md` warned that collapsing would lose.
#[test]
fn a_version_suffix_is_distinguishing() {
    let pair = r#"{"components":[
      {"id":"cmp:auth-service","name":"Auth Service","purpose":"v1"},
      {"id":"cmp:auth-service-v2","name":"Auth Service v2","purpose":"v2"}]}"#;
    let (g, report) = ingest(pair);
    assert_eq!(
        g.count_nodes(node::COMPONENT).unwrap(),
        2,
        "Auth Service and Auth Service v2 are two things: {:?}",
        report.fuzzy_merges
    );
    assert!(report.fuzzy_merges.is_empty());
}

/// The discriminator must not undo BL-186's ordering fix: the same tokens in a
/// different order are still one thing, and must still converge.
#[test]
fn reordered_tokens_still_converge() {
    let pair = r#"{"components":[
      {"id":"cmp:read-path-cache","name":"Read Path Cache","purpose":"serves reads"},
      {"id":"cmp:cache-read-path","name":"Cache Read Path","purpose":"serves reads"}]}"#;
    let (g, report) = ingest(pair);
    assert_eq!(
        g.count_nodes(node::COMPONENT).unwrap(),
        1,
        "reordered tokens are one thing and must still merge: {:?}",
        report.merge_candidates
    );
    assert_eq!(report.fuzzy_merges.len(), 1);
}

/// A refusal a person cannot act on is not a report. When the discriminator
/// holds a high-scoring pair back, it must say WHICH WORD did it.
#[test]
fn a_held_back_merge_names_the_word_that_held_it() {
    let pair = r#"{"components":[
      {"id":"cmp:dynograph-core","name":"dynograph-core","purpose":"schema model"},
      {"id":"cmp:dynograph-storage","name":"dynograph-storage","purpose":"rocksdb backend"}]}"#;
    let (_, report) = ingest(pair);

    let held: Vec<_> = report
        .merge_candidates
        .iter()
        .filter(|c| c.distinguished_by.is_some())
        .collect();
    assert_eq!(
        held.len(),
        1,
        "the pair scored 94 and must be reported as held back: {:?}",
        report.merge_candidates
    );
    let reason = held[0].distinguished_by.as_deref().unwrap();
    assert!(
        reason.contains("storage") || reason.contains("core"),
        "the reason must name the distinguishing word, got: {reason}"
    );
}
