//! Step 3 of the ordering Anthony approved on 2026-09-22: a design says
//! whether a real result has ever come back into it.
//!
//! Anthony: *"if a user designs a house and never comes back and says that it
//! failed an inspection, that is on the user and it is their choice — the
//! design should show that it never completed its loop."*
//!
//! Pins four things: an AGREEING run now leaves a trace (before, only a
//! disagreement did, so "never fed back" and "fed back and matched" were the
//! same graph); the claim itself is never edited by that trace; a failure that
//! came back and was then fixed still reads as the loop having fired; and the
//! reading is said in `loop_status.next` without counting against `clean`.

use reflow2_core::graph::DesignGraph;
use reflow2_core::loop_closure::LoopClosureState;
use reflow2_core::verify::{ObservedVerification, VerifyReconcileOptions};

fn world() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "House").expect("project");
    g.add_requirement(
        "req:safe",
        "Safe wiring",
        "The wiring must pass inspection.",
    )
    .expect("req");
    g.add_capability("cap:wiring", "Wiring", "carries power", Some("realized"))
        .expect("cap");
    g.satisfies("cap:wiring", "req:safe").expect("sat");
    g.add_verification("ver:inspect", "electrical inspection", None, None, None)
        .expect("ver");
    g.verifies("ver:inspect", "Capability", "cap:wiring")
        .expect("verifies");
    g
}

fn run(outcome: &str) -> Vec<ObservedVerification> {
    vec![ObservedVerification {
        verification_id: "ver:inspect".to_string(),
        outcome: outcome.to_string(),
    }]
}

fn recording(at: &str) -> VerifyReconcileOptions {
    VerifyReconcileOptions {
        record_events: true,
        exhaustive: false,
        detected_at: Some(at.to_string()),
    }
}

fn prop(g: &DesignGraph, k: &str) -> Option<String> {
    g.get_node("Verification", "ver:inspect")
        .expect("read")
        .expect("present")
        .properties
        .get(k)
        .and_then(|v| v.as_str().map(str::to_string))
}

#[test]
fn a_design_with_no_claimed_result_has_nothing_to_close() {
    let g = world();
    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::NothingToClose);
    let s = g.loop_status().expect("status");
    assert!(
        !s.next.iter().any(|l| l.contains("NO REAL RESULT")),
        "a design still being captured is not told it never closed: {:?}",
        s.next
    );
}

#[test]
fn a_typed_pass_with_no_run_behind_it_reads_never_closed_and_is_said_in_next() {
    let mut g = world();
    g.set_verification_status("ver:inspect", "passing", None, None)
        .expect("status");
    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::NeverClosed);
    assert_eq!(c.claiming_an_outcome, 1);
    assert_eq!(c.compared_to_a_run, 0);

    let s = g.loop_status().expect("status");
    assert!(
        s.next
            .iter()
            .any(|l| l.contains("NO REAL RESULT HAS EVER COME BACK")),
        "the fact must be in the list a session acts on: {:?}",
        s.next
    );
}

#[test]
fn never_closed_does_not_count_against_clean() {
    // The owner's choice, per Anthony — shown, never held against them.
    let mut g = world();
    g.set_verification_status("ver:inspect", "passing", None, None)
        .expect("status");
    let s = g.loop_status().expect("status");
    let other_debt: Vec<_> = s
        .next
        .iter()
        .filter(|l| !l.contains("NO REAL RESULT"))
        .collect();
    assert_eq!(s.clean, other_debt.is_empty(), "{:?}", s.next);
}

#[test]
fn an_agreeing_run_leaves_a_trace_and_does_not_touch_the_claim() {
    let mut g = world();
    g.set_verification_status("ver:inspect", "passing", None, None)
        .expect("status");
    let r = g
        .reconcile_verification(&run("passed"), &recording("2026-09-22"))
        .expect("reconcile");
    assert!(r.findings.is_empty());
    assert_eq!(r.stamped, vec!["ver:inspect"]);

    assert_eq!(prop(&g, "status").as_deref(), Some("passing"));
    assert_eq!(
        prop(&g, "last_reconciled_outcome").as_deref(),
        Some("passed")
    );
    assert_eq!(
        prop(&g, "last_reconciled_at").as_deref(),
        Some("2026-09-22")
    );

    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::ClosedWithoutAFailure);
    assert_eq!(c.compared_to_a_run, 1);
    assert_eq!(c.last_compared_at.as_deref(), Some("2026-09-22"));
}

#[test]
fn an_unrecorded_reconcile_stamps_nothing() {
    let mut g = world();
    g.set_verification_status("ver:inspect", "passing", None, None)
        .expect("status");
    let r = g
        .reconcile_verification(&run("passed"), &VerifyReconcileOptions::default())
        .expect("reconcile");
    assert!(r.stamped.is_empty());
    assert_eq!(prop(&g, "last_reconciled_outcome"), None);
    assert_eq!(
        g.loop_closure().expect("closure").state,
        LoopClosureState::NeverClosed
    );
}

#[test]
fn a_failure_that_came_back_and_was_fixed_still_reads_as_the_loop_having_fired() {
    let mut g = world();
    g.set_verification_status("ver:inspect", "passing", None, None)
        .expect("status");
    g.reconcile_verification(&run("failed"), &recording("2026-09-20"))
        .expect("reconcile");
    // The owner answers the divergence, the fix lands, the next run passes.
    g.set_verification_status("ver:inspect", "passing", Some("2026-09-22"), None)
        .expect("status");
    g.reconcile_verification(&run("passed"), &recording("2026-09-22"))
        .expect("reconcile");

    assert_eq!(
        prop(&g, "last_reconciled_outcome").as_deref(),
        Some("passed")
    );
    assert_eq!(
        prop(&g, "last_failed_run_at").as_deref(),
        Some("2026-09-20")
    );
    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::AFailureCameBack);
    assert_eq!(c.failures_fed_back, 1);
    assert_eq!(c.last_compared_at.as_deref(), Some("2026-09-22"));
}

#[test]
fn a_check_typed_as_failing_is_a_claim_not_a_result_that_came_back() {
    let mut g = world();
    g.set_verification_status("ver:inspect", "failing", None, None)
        .expect("status");
    let c = g.loop_closure().expect("closure");
    assert_eq!(c.state, LoopClosureState::NeverClosed);
    assert_eq!(c.failures_fed_back, 0);
}
