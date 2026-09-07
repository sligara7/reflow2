//! A finding another record has INVALIDATED must not read back as current.
//!
//! # The failure this pins
//!
//! `fact:a-contract-is-the-one-thing-reflow2-cannot-attach-evidence-to` claimed
//! that VERIFIES could not target an Interface, so no contract could carry a
//! check. Interface was added to that target list the very next day, and a
//! later session drew an `INVALIDATES` edge saying so in full — *"its headline
//! is now false and was false within a day of being written"*.
//!
//! And the finding went on reading as current. `search_design` returns
//! `expired` from `valid_to` alone, and the invalidation set no `valid_to`, so
//! the record came back undated-as-over while the gap detector — which reads
//! the EDGE — had correctly fallen silent about it. **Two surfaces disagreeing
//! about whether a finding is closed**, and the read surface is the one a
//! person meets first. On 2026-09-07 an agent searched, read the live-looking
//! record, and re-derived a conclusion the design had already recorded a week
//! earlier.
//!
//! # What is pinned, and why it is the CLASS
//!
//! Not "this one fact reads as expired" — that would pin the instance. What is
//! pinned is that CLOSURE IS ONE QUESTION WITH TWO ANSWERS AVAILABLE, and both
//! read surfaces give the same one: a claim ends either because its own
//! `valid_to` passed, or because a later record invalidated it, and a reader
//! must be told which. The two states stay DISTINCT on purpose — `expired` is a
//! claim that ran out on its own terms, `superseded_by` is a claim something
//! else overturned, and collapsing them would lose the id of the record that
//! did the overturning, which is the only thing a reader can follow.

use reflow2_core::{DesignGraph, nodes::Props, nodes::edge, nodes::node};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

/// Set the scene: a dated finding about a component, and a later change event
/// that says the finding no longer holds.
fn a_finding_and_the_record_that_overturned_it(g: &mut DesignGraph) {
    g.upsert_node(
        node::COMPONENT,
        "cmp:gauge",
        Props::new()
            .set("name", "Rain gauge")
            .set("purpose", "measures rainfall and reports totals"),
    )
    .expect("component");
    g.upsert_node(
        node::TEMPORAL_FACT,
        "fact:the-gauge-cannot-report-totals",
        Props::new()
            .set("name", "The gauge cannot report a running total")
            .set("fact_type", "property")
            .set("subject_id", "cmp:gauge")
            .set("statement", "The gauge sends deltas and no running total.")
            .set("valid_from", "2026-08-21"),
    )
    .expect("finding");
    g.add_change_event(
        "chg:the-gauge-reports-totals",
        "The gauge reports a running total",
        reflow2_core::ChangeType::NewFeature,
        None,
        Some("The retry path now carries the total."),
        None,
        Some("2026-08-22"),
    )
    .expect("change event");
    g.create_edge(
        edge::INVALIDATES,
        node::CHANGE_EVENT,
        "chg:the-gauge-reports-totals",
        node::TEMPORAL_FACT,
        "fact:the-gauge-cannot-report-totals",
        Props::new().set("at", "2026-08-22").set(
            "note",
            "The gauge carries the total now, so the finding's claim no longer holds.",
        ),
    )
    .expect("invalidates edge");
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SHARED COMPUTATION ITSELF, tested with no search index.
//
// 🛑 SPLIT THIS WAY BECAUSE THE FIRST CUT WAS NOT. Every test here originally
// went through `search_design`, which needs the `fulltext` feature — so the
// whole file compiled to nothing useful under `--no-default-features` and the
// core CI job FAILED on it while the workspace job passed. The local run that
// "passed" had `--features fulltext` added to get past exactly that error,
// which turned the contract into a workaround without noticing.
//
// `claim_age_of` needs no index: it reads a property bag and one incoming edge.
// So the computation is covered in EVERY job, and only the two assertions that
// genuinely require a search surface are gated below.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn the_shared_computation_names_what_overturned_a_finding() {
    let mut g = graph();
    a_finding_and_the_record_that_overturned_it(&mut g);
    let node = g
        .get_node(node::TEMPORAL_FACT, "fact:the-gauge-cannot-report-totals")
        .expect("read")
        .expect("present");
    let age = g
        .claim_age_of(&node.node_id, &node.properties, "2026-09-07")
        .expect("age");
    assert_eq!(
        age.superseded_by.as_deref(),
        Some("chg:the-gauge-reports-totals"),
        "the reader must be told WHICH record overturned it — that id is the only thing they \
         can follow"
    );
    assert!(
        !age.expired,
        "and it did NOT expire on its own terms: nothing set its valid_to, which is exactly why \
         reading only that field let it pass as current"
    );
}

#[test]
fn a_claim_that_ran_out_is_not_a_claim_something_refuted() {
    let mut g = graph();
    a_finding_and_the_record_that_overturned_it(&mut g);
    g.upsert_node(
        node::TEMPORAL_FACT,
        "fact:the-gauge-was-offline-in-august",
        Props::new()
            .set("name", "The gauge was offline through August")
            .set("fact_type", "observation")
            .set("subject_id", "cmp:gauge")
            .set("statement", "No readings arrived.")
            .set("valid_from", "2026-08-01")
            .set("valid_to", "2026-08-31"),
    )
    .expect("lapsed finding");
    let node = g
        .get_node(node::TEMPORAL_FACT, "fact:the-gauge-was-offline-in-august")
        .expect("read")
        .expect("present");
    let age = g
        .claim_age_of(&node.node_id, &node.properties, "2026-09-07")
        .expect("age");
    assert!(age.expired, "its own window closed");
    assert!(
        age.superseded_by.is_none(),
        "and nothing overturned it — the two kinds of closure must stay distinguishable"
    );
}

#[test]
fn a_live_finding_is_untouched() {
    let mut g = graph();
    a_finding_and_the_record_that_overturned_it(&mut g);
    g.upsert_node(
        node::TEMPORAL_FACT,
        "fact:the-gauge-drifts-in-frost",
        Props::new()
            .set("name", "The gauge under-reports in frost")
            .set("fact_type", "property")
            .set("subject_id", "cmp:gauge")
            .set("statement", "Tips stick below freezing.")
            .set("valid_from", "2026-08-21"),
    )
    .expect("live finding");
    let node = g
        .get_node(node::TEMPORAL_FACT, "fact:the-gauge-drifts-in-frost")
        .expect("read")
        .expect("present");
    let age = g
        .claim_age_of(&node.node_id, &node.properties, "2026-09-07")
        .expect("age");
    assert!(age.superseded_by.is_none() && !age.expired);
}

/// THE READ SURFACE, which is the half that actually misled a session — and the
/// half that needs an index, so it is gated rather than dropped.
#[cfg(feature = "fulltext")]
mod on_the_read_surface {
    use super::*;
    #[test]
    fn search_says_a_finding_was_superseded_and_names_what_superseded_it() {
        let mut g = graph();
        a_finding_and_the_record_that_overturned_it(&mut g);
        g.reindex_search().expect("reindex");
        let result = g
            .search_design("running total gauge", None, 10)
            .expect("search");
        let hit = result
            .hits
            .iter()
            .find(|h| h.node_id == "fact:the-gauge-cannot-report-totals")
            .expect("the finding is found");
        assert_eq!(
            hit.age.superseded_by.as_deref(),
            Some("chg:the-gauge-reports-totals"),
            "a finding another record has invalidated must not read back as current — and the \
             reader must be told WHICH record overturned it, because that id is the only thing \
             they can follow. This is the surface that misled a session into re-deriving a \
             conclusion the design already held."
        );
    }

    /// The two states are distinct. A claim that ran out on its own terms is not
    /// the same as one something else overturned, and a reader needs to tell them
    /// apart.
    #[test]
    fn expiring_on_its_own_terms_is_not_the_same_as_being_superseded() {
        let mut g = graph();
        a_finding_and_the_record_that_overturned_it(&mut g);
        g.upsert_node(
            node::TEMPORAL_FACT,
            "fact:the-gauge-was-offline-in-august",
            Props::new()
                .set("name", "The gauge was offline through August")
                .set("fact_type", "observation")
                .set("subject_id", "cmp:gauge")
                .set("statement", "No readings arrived.")
                .set("valid_from", "2026-08-01")
                .set("valid_to", "2026-08-31"),
        )
        .expect("expired finding");
        g.reindex_search().expect("reindex");
        let result = g.search_design("gauge", None, 20).expect("search");
        let find = |id: &str| {
            result
                .hits
                .iter()
                .find(|h| h.node_id == id)
                .unwrap_or_else(|| panic!("{id} is found"))
                .clone()
        };

        let overturned = find("fact:the-gauge-cannot-report-totals");
        assert!(
            overturned.age.superseded_by.is_some(),
            "the overturned one is superseded"
        );
        assert!(
            !overturned.age.expired,
            "and it did NOT expire on its own terms — nothing set its valid_to, which is exactly \
             why reading only that field let it pass as current"
        );

        let lapsed = find("fact:the-gauge-was-offline-in-august");
        assert!(lapsed.age.expired, "the dated one expired on its own terms");
        assert!(
            lapsed.age.superseded_by.is_none(),
            "and nothing overturned it — a claim that ran out is not a claim that was refuted"
        );
    }

    /// A live finding is unaffected: this must not make ordinary records read as
    /// closed.
    #[test]
    fn a_finding_nothing_has_overturned_still_reads_as_current() {
        let mut g = graph();
        a_finding_and_the_record_that_overturned_it(&mut g);
        g.upsert_node(
            node::TEMPORAL_FACT,
            "fact:the-gauge-drifts-in-frost",
            Props::new()
                .set("name", "The gauge under-reports in frost")
                .set("fact_type", "property")
                .set("subject_id", "cmp:gauge")
                .set("statement", "Tips stick below freezing.")
                .set("valid_from", "2026-08-21"),
        )
        .expect("live finding");
        g.reindex_search().expect("reindex");
        let result = g.search_design("frost gauge", None, 10).expect("search");
        let hit = result
            .hits
            .iter()
            .find(|h| h.node_id == "fact:the-gauge-drifts-in-frost")
            .expect("found");
        assert!(hit.age.superseded_by.is_none() && !hit.age.expired);
    }
}
