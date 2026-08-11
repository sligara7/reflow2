//! What the design BUILT and nothing in it consumes — the surplus half of DETECT.
//!
//! # ⭐ WRITTEN BEFORE THE IMPLEMENTATION
//!
//! Every case below was written from `cap:built-and-consumed-by-nothing`'s own
//! description and `req:the-design-says-what-was-built-and-never-used`, before
//! a line of `consumption.rs` existed. Anthony raised test-first three times on
//! 2026-08-11 and it had not actually been done yet; this is the increment
//! where it was.
//!
//! It earned its keep immediately. Running the spec's three signals over
//! reflow2's own design named **100 of 110** built capabilities — and that
//! measurement, taken before any code was written, is what produced
//! [`MIN_MODELLED_RATIO`] and the whole "absence is only informative when
//! presence is the habit" rule. Implementation-first, the detector would have
//! shipped, reported 91% of the codebase as possibly-surplus, and been switched
//! off the same day.
//!
//! # The spec, quoted so a later reader can check the tests against it
//!
//! > "a Capability at `realized`/`verified` with no incoming DEPENDS_ON, no
//! > PART_OF_FLOW into any Flow, and no Actor INTERACTS_WITH — built, and
//! > nothing downstream of it."
//!
//! > "⚠️ THE WORDING IS THE FEATURE, not a caveat on it. […] the honest finding
//! > is 'NOTHING IN THIS DESIGN CONSUMES X' — never 'X is unused'. A feature
//! > real users call daily whose consumer was never modelled produces exactly
//! > the same graph shape as a dead one, and a detector that did not say so
//! > would confidently recommend deleting working code."

use std::collections::HashMap;

use reflow2_core::{
    DesignGraph,
    consumption::{MIN_MODELLED_RATIO, MIN_POPULATION},
    nodes::{Props, edge, node},
};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().unwrap()
}

/// A capability that is BUILT — the only population the spec puts in scope.
fn built(g: &mut DesignGraph, id: &str, status: &str) {
    g.add_capability(id, id, "does a thing", Some(status))
        .unwrap();
}

fn depends_on(g: &mut DesignGraph, consumer: &str, consumed: &str) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        consumer,
        node::CAPABILITY,
        consumed,
        HashMap::new(),
    )
    .unwrap();
}

/// A design that HAS the habit of recording consumption, so the ratio rule is
/// satisfied and the list is actually reported. Every consumed pair is built,
/// which keeps the population above [`MIN_POPULATION`] too.
///
/// Without this, most tests below would pass for the wrong reason — an empty
/// observation list because the design is too thin to speak about, not because
/// the capability under test was consumed.
fn a_design_that_models_consumption(g: &mut DesignGraph) {
    for i in 0..6 {
        built(g, &format!("cap:consumed-{i}"), "realized");
        built(g, &format!("cap:caller-{i}"), "realized");
        depends_on(g, &format!("cap:caller-{i}"), &format!("cap:consumed-{i}"));
        // The caller needs a consumer of its own, or half the population is
        // unconsumed by construction and the ratio never clears.
        depends_on(g, &format!("cap:consumed-{i}"), &format!("cap:caller-{i}"));
    }
}

fn observed(g: &DesignGraph) -> Vec<String> {
    g.consumption_report()
        .unwrap()
        .observations
        .into_iter()
        .map(|o| o.node_id)
        .collect()
}

// THE CASE THE REQUIREMENT WAS RAISED FOR. Anthony, 2026-08-09: "in the
// storyflow set of services, there are going to be multiple designed and
// implemented, but unused features."
#[test]
fn a_built_capability_with_nothing_downstream_is_observed() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:orphan", "realized");
    assert!(
        observed(&g).contains(&"cap:orphan".to_string()),
        "the one capability with nothing downstream must be named"
    );
}

// THE THREE CONSUMPTION SIGNALS THE SPEC NAMES, one test each so a regression
// says WHICH signal stopped being read rather than that "something broke".
#[test]
fn a_capability_something_depends_on_is_not_observed() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:used", "realized");
    built(&mut g, "cap:consumer", "realized");
    depends_on(&mut g, "cap:consumer", "cap:used");
    assert!(
        !observed(&g).contains(&"cap:used".to_string()),
        "something depends on it, so it is consumed"
    );
}

#[test]
fn a_capability_that_is_a_step_in_a_flow_is_not_observed() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:step", "realized");
    g.add_flow("flow:checkout", "Checkout", None, None, None, None)
        .unwrap();
    g.part_of_flow("cap:step", "flow:checkout", Some(1))
        .unwrap();
    assert!(
        !observed(&g).contains(&"cap:step".to_string()),
        "a flow member is consumed BY the flow"
    );
}

#[test]
fn a_capability_an_actor_interacts_with_is_not_observed() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:ui", "realized");
    g.create_node(node::ACTOR, "act:user", Props::new().set("name", "A user"))
        .unwrap();
    g.create_edge(
        edge::INTERACTS_WITH,
        node::ACTOR,
        "act:user",
        node::CAPABILITY,
        "cap:ui",
        HashMap::new(),
    )
    .unwrap();
    assert!(
        !observed(&g).contains(&"cap:ui".to_string()),
        "a person is downstream of it"
    );
}

// POPULATION. The spec scopes this to `realized`/`verified`: something not yet
// built cannot be surplus, and reporting it would duplicate the gaps that
// already ask what builds it.
#[test]
fn a_planned_capability_is_not_observed_because_it_is_not_built_yet() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:future", "planned");
    assert!(
        !observed(&g).contains(&"cap:future".to_string()),
        "unbuilt is not surplus — unrealized_capability already asks about it"
    );
}

#[test]
fn a_verified_capability_counts_as_built() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:proven", "verified");
    assert!(
        observed(&g).contains(&"cap:proven".to_string()),
        "the spec names realized AND verified"
    );
}

// A WITHDRAWN capability is not surplus, it is withdrawn — and
// `dec:idea-discontinued-is-a-first-class-state` already covers it. Without
// this, discontinuing something would move it from one report to another
// instead of out of both.
#[test]
fn a_discontinued_capability_is_not_observed() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:withdrawn", "realized");
    assert!(
        observed(&g).contains(&"cap:withdrawn".to_string()),
        "precondition: it is observed while live"
    );

    g.add_decision(
        "dec:withdraw",
        "Withdrawn",
        "Built, shipped, correct, and used zero times.",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:withdraw", "accepted").unwrap();
    g.create_edge(
        edge::OBSOLETES,
        node::DECISION,
        "dec:withdraw",
        node::CAPABILITY,
        "cap:withdrawn",
        HashMap::new(),
    )
    .unwrap();
    assert!(
        !observed(&g).contains(&"cap:withdrawn".to_string()),
        "a withdrawn capability is not surplus, it is withdrawn"
    );
}

// AND A WITHDRAWN CALLER IS NOT A CALLER. Without this, discontinuing one
// capability quietly props up the next one along: the consumer is gone, the
// DEPENDS_ON edge remains, and the thing it used to call keeps reading as
// consumed forever. That is the same "a marker nothing reads is a comment"
// failure the discontinued reader was built to fix, one hop downstream.
#[test]
fn a_consumer_that_has_itself_been_discontinued_does_not_count() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:propped-up", "realized");
    built(&mut g, "cap:doomed-caller", "realized");
    depends_on(&mut g, "cap:doomed-caller", "cap:propped-up");
    assert!(
        !observed(&g).contains(&"cap:propped-up".to_string()),
        "precondition: while the caller is live, it is consumed"
    );

    g.add_decision(
        "dec:withdraw-caller",
        "The caller is withdrawn",
        "Gone.",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:withdraw-caller", "accepted")
        .unwrap();
    g.create_edge(
        edge::OBSOLETES,
        node::DECISION,
        "dec:withdraw-caller",
        node::CAPABILITY,
        "cap:doomed-caller",
        HashMap::new(),
    )
    .unwrap();
    assert!(
        observed(&g).contains(&"cap:propped-up".to_string()),
        "its only consumer was withdrawn, so nothing consumes it any more"
    );
}

// ⚠️ THE WORDING IS THE FEATURE. Modelled on granularity's
// `the_observation_carries_no_verdict`, because this report has the same
// obligation and a stronger reason: acting on a wrong reading here means
// deleting working code.
#[test]
fn the_report_says_nothing_consumes_it_never_that_it_is_unused() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:orphan", "realized");
    let report = g.consumption_report().unwrap();
    assert!(
        !report.observations.is_empty(),
        "precondition: something to word"
    );

    // ⚠️ Scanned over the OBSERVATIONS, not the whole report — a distinction
    // this test learned the hard way. `not_observed_about` legitimately says
    // "looks exactly like one that has been dead since it shipped" and "genuine
    // surplus", because naming the failure mode is how a caveat prevents it.
    // Banning those words everywhere would delete the very sentences that stop
    // the misreading. What must never carry a verdict is the FINDING.
    let findings = serde_json::to_string(&report.observations)
        .unwrap()
        .to_lowercase();
    for forbidden in [
        "unused",
        "dead",
        "delete",
        "surplus",
        "severity",
        "suggested_fix",
        "should be removed",
    ] {
        assert!(
            !findings.contains(forbidden),
            "the finding must not say `{forbidden}` — a consumer nobody modelled looks identical \
             to a dead one: {findings}"
        );
    }
    assert!(
        findings.contains("records nothing that consumes it"),
        "and it must say what it DOES mean, as a fact about the RECORD: {findings}"
    );
}

// COUNTERWEIGHT: an ordinary consumed capability beside an unconsumed one is
// untouched. A report that named everything would pass most tests above and be
// worthless.
#[test]
fn a_consumed_capability_beside_an_unconsumed_one_is_not_named() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:orphan", "realized");
    let seen = observed(&g);
    assert_eq!(
        seen,
        vec!["cap:orphan".to_string()],
        "exactly the one thing with nothing downstream, and none of its twelve neighbours"
    );
}

// ⭐ THE RULE THE MEASUREMENT FORCED, and the reason this module is not just a
// list. On reflow2's own design (2026-08-11) the three signals named 100 of 110
// built capabilities, because that design holds twelve consumption edges and
// zero Flow nodes. Consumption is not something it models — so absence of
// consumption says nothing about any individual capability, and a hundred
// findings would bury the one true finding, which is the ratio.
#[test]
fn a_design_that_does_not_model_consumption_gets_the_ratio_not_a_list() {
    let mut g = graph();
    // Ten built capabilities, one consumed pair — 20% modelled, reflow2's own
    // shape in miniature.
    for i in 0..8 {
        built(&mut g, &format!("cap:lonely-{i}"), "realized");
    }
    built(&mut g, "cap:a", "realized");
    built(&mut g, "cap:b", "realized");
    depends_on(&mut g, "cap:a", "cap:b");
    depends_on(&mut g, "cap:b", "cap:a");

    let r = g.consumption_report().unwrap();
    assert_eq!(r.population, 10);
    assert_eq!(r.consumption_modelled, 2);
    assert!(
        r.observations.is_empty(),
        "below the ratio the list is withheld, got {:?}",
        r.observations
    );
    let note = r.notes.join(" ");
    assert!(
        note.contains("2 of 10") && note.contains("not this design's habit"),
        "and the ratio must be REPORTED, never silence: {note}"
    );
    assert!(
        note.contains("8 capability(s) with no recorded consumer are NOT listed"),
        "no silent caps — it must say what it withheld and how much: {note}"
    );
}

// THE OTHER SIDE OF THAT RULE. A design that DOES have the habit gets its list,
// or the ratio rule would be a way of never reporting anything.
#[test]
fn a_design_that_models_consumption_does_get_its_list() {
    let mut g = graph();
    a_design_that_models_consumption(&mut g);
    built(&mut g, "cap:orphan", "realized");
    let r = g.consumption_report().unwrap();
    assert_eq!(r.population, 13);
    assert_eq!(r.consumption_modelled, 12);
    assert!(
        (r.consumption_modelled as f64 / r.population as f64) >= MIN_MODELLED_RATIO,
        "precondition: this design has the habit"
    );
    assert_eq!(r.observations.len(), 1);
}

// Too few built capabilities to compute a ratio over — the same floor and the
// same reasoning as granularity's MIN_POPULATION, and reported as a note rather
// than as silence.
#[test]
fn too_small_a_population_is_said_out_loud_rather_than_answered_quietly() {
    let mut g = graph();
    built(&mut g, "cap:only", "realized");
    let r = g.consumption_report().unwrap();
    assert!(r.observations.is_empty());
    assert!(
        r.notes
            .join(" ")
            .contains(&format!("below the {MIN_POPULATION}")),
        "it must say WHY it is quiet: {:?}",
        r.notes
    );
}

// The boundary is stated, not trusted. TRIGGERS and GATED_ON can also land on a
// Capability and are deliberately NOT read as consumption; a reader can only
// argue with that if the report says what it counted.
#[test]
fn the_report_names_the_edges_it_read_as_consumption() {
    let g = graph();
    let signals = g.consumption_report().unwrap().signals_read.join(" ");
    for named in ["DEPENDS_ON", "PART_OF_FLOW", "INTERACTS_WITH"] {
        assert!(
            signals.contains(named),
            "the boundary must be visible: {signals}"
        );
    }
}

// AND WHAT IT CANNOT SEE IS ON EVERY REPORT, including a clean one — the same
// discipline granularity's `not_observed_about` keeps. This is the field that
// stops "no observations" being read as "nothing is surplus".
#[test]
fn every_report_carries_what_it_is_blind_to() {
    let g = graph();
    let blind = g.consumption_report().unwrap().not_observed_about.join(" ");
    assert!(
        blind.contains("never a running system"),
        "the runtime blindness must be stated on every report: {blind}"
    );
}
