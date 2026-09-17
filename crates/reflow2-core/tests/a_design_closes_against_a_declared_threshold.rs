//! The closure report: five legs summed against a threshold the owner
//! declared, each saying what it swept, the first hole named, and a verdict
//! that is a report and never a gate
//! (`req:a-design-closes-against-a-declared-threshold-and-the-report-names-the-first-hole`).
//!
//! The tests that carry the weight are the refusals: no criterion reads as
//! "no closure criterion stated" rather than as any default, and a counted
//! leg with nothing to run on cannot read as closed.

use reflow2_core::DesignGraph;
use reflow2_core::closure::{CLOSURE_LEGS, ClosureVerdict};
use reflow2_core::nodes::node;

/// One requirement, one capability that satisfies it, one artifact realizing
/// the capability, one check — the delivered thread from delivery.rs.
fn threaded(status: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:ship", "It ships", "The thing must ship.")
        .unwrap();
    g.set_requirement_status("req:ship", "accepted").unwrap();
    g.add_component("cmp:engine", "Engine", "does the work", None)
        .unwrap();
    g.add_capability("cap:ship", "Ship it", "ships the thing", Some("realized"))
        .unwrap();
    g.satisfies("cap:ship", "req:ship").unwrap();
    g.allocate("cap:ship", "cmp:engine").unwrap();
    g.add_artifact(
        "art:engine",
        "engine.rs",
        Some("code"),
        Some("src/engine.rs"),
    )
    .unwrap();
    g.realizes("art:engine", node::CAPABILITY, "cap:ship", None, None)
        .unwrap();
    g.add_verification("ver:ship", "ship test", Some("test"), None, None)
        .unwrap();
    g.verifies("ver:ship", node::CAPABILITY, "cap:ship")
        .unwrap();
    g.set_verification_status("ver:ship", status, None, None)
        .unwrap();
    g
}

fn leg<'a>(
    r: &'a reflow2_core::closure::ClosureReport,
    name: &str,
) -> &'a reflow2_core::closure::ClosureLeg {
    r.legs.iter().find(|l| l.leg == name).expect(name)
}

#[test]
fn no_declared_criterion_reads_as_no_criterion_stated_never_as_a_default() {
    let g = threaded("passing");
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::NoClosureCriterionStated);
    assert!(r.criterion.is_none());
    assert!(r.first_hole.is_none());
    // The legs are still computed and shown — withholding the verdict is not
    // withholding the reading.
    assert_eq!(r.legs.len(), CLOSURE_LEGS.len());
    assert_eq!(leg(&r, "traceability").closed, 1);
    assert!(
        leg(&r, "traceability").closes.is_none(),
        "no criterion, no per-leg verdict"
    );
    assert!(r.note.contains("No closure criterion stated"), "{}", r.note);
}

#[test]
fn a_criterion_is_validated_and_carried_on_the_project() {
    let mut g = threaded("passing");
    let err = g
        .set_closure_criterion("proj:p", &["traceability", "vibes"], 100.0)
        .expect_err("an unknown leg is refused by name");
    assert!(err.to_string().contains("vibes"), "{err}");
    assert!(
        err.to_string().contains("provenance"),
        "the refusal lists the legs: {err}"
    );
    g.set_closure_criterion("proj:p", &[], 100.0)
        .expect_err("an empty criterion would make every design close");
    g.set_closure_criterion("proj:p", &["budgets"], 140.0)
        .expect_err("a share is 0 to 100");

    let p = g
        .set_closure_criterion("proj:p", &["traceability", "budgets"], 100.0)
        .unwrap();
    assert_eq!(
        p.properties.get("name").and_then(|v| v.as_str()),
        Some("P"),
        "the other properties are carried"
    );
    let r = g.closure_report().unwrap();
    let c = r.criterion.as_ref().expect("declared");
    assert_eq!(
        c.legs,
        vec!["traceability".to_string(), "budgets".to_string()]
    );
    assert_eq!(c.threshold, 100.0);
}

#[test]
fn traceability_alone_at_100_closes_on_a_delivered_thread_and_opens_when_the_check_fails() {
    let mut g = threaded("passing");
    g.set_closure_criterion("proj:p", &["traceability"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::Closes, "{}", r.note);
    assert_eq!(leg(&r, "traceability").closes, Some(true));
    assert!(
        leg(&r, "budgets").closes.is_none(),
        "an uncounted leg is shown and does not vote"
    );

    // The derivation goes backwards: the check fails, the design stops
    // closing, and the hole names the requirement.
    g.set_verification_status("ver:ship", "failing", None, None)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose);
    let hole = r.first_hole.expect("named");
    assert_eq!(hole.leg, "traceability");
    assert_eq!(hole.id.as_deref(), Some("req:ship"));
}

#[test]
fn a_counted_leg_with_nothing_to_run_on_cannot_read_as_closed() {
    // A delivered thread and NO budget modelled. "budgets: 0 open" beside
    // "budgets: 0 modelled" must not read as clean.
    let mut g = threaded("passing");
    g.set_closure_criterion("proj:p", &["traceability", "budgets"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose, "{}", r.note);
    let b = leg(&r, "budgets");
    assert_eq!(b.swept, 0);
    assert!(b.share.is_none());
    assert!(b.closes.is_none());
    assert!(b.swept_note.contains("0 modelled"), "{}", b.swept_note);
    let hole = r.first_hole.expect("the hole is the leg itself");
    assert_eq!(hole.leg, "budgets");
    assert!(hole.id.is_none());
    assert!(hole.why.contains("nothing to run on"), "{}", hole.why);
}

#[test]
fn the_threshold_is_the_owners_and_fifty_percent_is_a_legal_declaration() {
    let mut g = threaded("passing");
    // A second requirement nobody has delivered.
    g.add_requirement("req:log", "It logs", "Every run is logged.")
        .unwrap();
    g.set_requirement_status("req:log", "accepted").unwrap();

    g.set_closure_criterion("proj:p", &["traceability"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose);
    assert_eq!(leg(&r, "traceability").share, Some(50.0));

    g.set_closure_criterion("proj:p", &["traceability"], 50.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(
        r.verdict,
        ClosureVerdict::Closes,
        "half is what the owner asked for"
    );
}

#[test]
fn a_budget_inside_its_limit_but_not_its_declared_margin_is_a_hole() {
    let mut g = threaded("passing");
    g.add_constraint(
        "con:mass",
        "Mass",
        "Under 100 kg.",
        Some("budget"),
        Some("mass"),
        Some(100.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.set_constraint_unit("con:mass", "kg").unwrap();
    g.set_constraint_provenance("con:mass", Some("asserted"), Some("who:ajs"), None)
        .unwrap();
    g.constrains_in(
        "con:mass",
        "Component",
        "cmp:engine",
        Some(95.0),
        Some("kg"),
        Some("measured"),
        Some("scale"),
        Some("2026-09-17"),
        None,
    )
    .unwrap();
    g.set_closure_criterion("proj:p", &["budgets"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(
        r.verdict,
        ClosureVerdict::Closes,
        "within the limit, no margin declared: {}",
        r.note
    );
    assert!(
        leg(&r, "budgets")
            .swept_note
            .contains("1 modelled, 0 with a declared margin")
    );

    g.set_constraint_margin("con:mass", 10.0).unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose);
    let hole = r.first_hole.expect("named");
    assert_eq!(hole.id.as_deref(), Some("con:mass"));
    assert!(hole.why.contains("margin"), "{}", hole.why);
}

#[test]
fn the_first_hole_follows_the_declared_order_not_the_reports() {
    // Provenance is broken (an unsourced limit) AND traceability is broken
    // (a failing check). Declared order "provenance, traceability" names the
    // provenance hole first; the reverse names traceability first.
    let mut g = threaded("failing");
    g.add_constraint(
        "con:mass",
        "Mass",
        "Under 100 kg.",
        Some("budget"),
        Some("mass"),
        Some(100.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.set_closure_criterion("proj:p", &["provenance", "traceability"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(
        r.first_hole.as_ref().map(|h| h.leg.as_str()),
        Some("provenance")
    );
    assert_eq!(
        r.first_hole.as_ref().and_then(|h| h.id.as_deref()),
        Some("con:mass")
    );

    g.set_closure_criterion("proj:p", &["traceability", "provenance"], 100.0)
        .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(
        r.first_hole.as_ref().map(|h| h.leg.as_str()),
        Some("traceability")
    );
}

#[test]
fn scheduled_work_governed_by_an_open_decision_is_the_decisions_hole() {
    let mut g = threaded("passing");
    g.plan_epoch(
        "epoch:next",
        "Next increment",
        reflow2_core::temporal::EpochType::Milestone,
        10,
    )
    .unwrap();
    g.add_decision("dec:store", "Which store?", "Undecided.", None)
        .unwrap();
    g.governed_by(
        node::CAPABILITY,
        "cap:ship",
        node::DECISION,
        "dec:store",
        None,
        None,
    )
    .unwrap();
    g.set_closure_criterion("proj:p", &["decisions"], 100.0)
        .unwrap();

    // Nothing scheduled yet: nothing to run on, and it says so.
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose);
    assert!(
        leg(&r, "decisions")
            .swept_note
            .contains("0 item(s) scheduled")
    );

    g.schedule_for(
        node::CAPABILITY,
        "cap:ship",
        node::DESIGN_EPOCH,
        "epoch:next",
        "expected",
        None,
    )
    .unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::DoesNotClose);
    let hole = r.first_hole.expect("named");
    assert_eq!(hole.leg, "decisions");
    assert_eq!(hole.id.as_deref(), Some("dec:store"));

    g.set_decision_status("dec:store", "accepted").unwrap();
    let r = g.closure_report().unwrap();
    assert_eq!(r.verdict, ClosureVerdict::Closes, "{}", r.note);
}

#[test]
fn closure_is_a_report_and_never_a_gate() {
    // A design that does not close can still be released; the release
    // report carries the closure verdict beside its own answer.
    let mut g = threaded("failing");
    g.set_closure_criterion("proj:p", &["traceability"], 100.0)
        .unwrap();
    g.add_release("rel:1", "1.0", Some("1.0"), Some("binary"))
        .unwrap();
    let rr = g
        .release_report("rel:1")
        .expect("a release report is produced regardless");
    assert_eq!(
        rr.closure.as_ref().map(|c| c.verdict),
        Some(ClosureVerdict::DoesNotClose)
    );
}
