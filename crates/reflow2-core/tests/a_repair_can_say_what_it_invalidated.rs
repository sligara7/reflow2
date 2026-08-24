//! A finding stops proposing work that has already been done.
//!
//! # What this pins
//!
//! dev_storyflow, 2026-08-23, and it cost a real user a real instruction. A
//! session ran `where-am-i`, read `ver:the_shardblade_walk` with status
//! `failing` and `last_run_at` of that same day, and reported its two defects
//! as the live state of the system. **Both had been fixed hours earlier**,
//! recorded properly on two Constraint nodes with commit shas. The user replied
//! "fix the forge scaling and chase the 401" — acting on the report — and the
//! first thing the session actually did was discover the work was already done.
//!
//! The graph was right. Every node was right. **The composition was wrong**,
//! because nothing joined the repair to the check that found it:
//! `describe_schema(from: Constraint, to: Verification)` returned ZERO exact
//! matches, and the nearest honest assertion available was `CONTRADICTS` with
//! `alignment: opposing` — which reads to a defect detector as a design
//! inconsistency rather than as a re-run owed.
//!
//! # Why a CLAIM and not a computation
//!
//! Inferring staleness means ordering a change against `last_run_at`. Measured
//! 2026-08-24: of 439 hand-written ChangeEvents only 37 (8%) carried a date,
//! because `add_change_event` had no parameter for one. The inference was not
//! buildable honestly. The claim needs no clock.
//!
//! # Why `INVALIDATES` and not `RESOLVES`
//!
//! A repair does not make a check pass. It makes the last RESULT untrustworthy,
//! and only a re-run can say what is true now. Every assertion below turns on
//! that distinction: the target's `status` never moves, and the check is never
//! dropped from the attention list.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, GapSource};

/// The shape of the failure as it actually happened: a failing check, and a
/// Constraint that recorded the repair.
fn a_failing_check_and_its_repair(g: &mut DesignGraph) {
    g.add_capability("cap:forge", "Forge", "casts the thing", Some("realized"))
        .unwrap();
    g.add_verification("ver:walk", "the shardblade walk", Some("test"), None, None)
        .unwrap();
    g.verifies("ver:walk", node::CAPABILITY, "cap:forge")
        .unwrap();
    g.set_verification_status("ver:walk", "failing", Some("2026-08-23"), None)
        .unwrap();
    g.add_constraint(
        "con:forge-scaling",
        "Forge scaling is bounded",
        "RESOLVED 2026-08-23 in a1b2c3d — the scaling factor is clamped.",
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
}

#[test]
fn a_repair_can_name_the_check_it_answered() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);

    let e = g
        .invalidates(
            node::CONSTRAINT,
            "con:forge-scaling",
            node::VERIFICATION,
            "ver:walk",
            Some("the clamp landed in a1b2c3d; this verdict predates it"),
            Some("2026-08-23"),
        )
        .unwrap();

    assert_eq!(e.edge_type, edge::INVALIDATES);
    assert_eq!(
        e.properties.get("note").and_then(|v| v.as_str()),
        Some("the clamp landed in a1b2c3d; this verdict predates it"),
        "the note is what makes the claim auditable rather than merely present"
    );
}

#[test]
fn the_claim_is_read_back_with_whether_a_rerun_is_owed() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    // The run happened in the morning; the repair landed after it.
    g.set_verification_status("ver:walk", "failing", Some("2026-08-23"), None)
        .unwrap();
    g.invalidates(
        node::CONSTRAINT,
        "con:forge-scaling",
        node::VERIFICATION,
        "ver:walk",
        Some("clamped"),
        Some("2026-08-24"),
    )
    .unwrap();

    let found = g.invalidated_findings().unwrap();
    assert_eq!(found.len(), 1, "one finding carries a claim");
    let f = &found[0];
    assert_eq!(f.finding_id, "ver:walk");
    assert_eq!(
        f.rerun_owed,
        Some(true),
        "the repair postdates the run, so the verdict is stale and a re-run is owed"
    );
    assert_eq!(f.claimed_by[0].claimed_by, "con:forge-scaling");
    assert_eq!(f.undated_claims, 0);
}

#[test]
fn a_run_that_postdates_the_repair_owes_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    // Somebody already re-ran it after the fix — and it still fails, which is a
    // real answer and must not be confused with a stale one.
    g.set_verification_status("ver:walk", "failing", Some("2026-08-25"), None)
        .unwrap();
    g.invalidates(
        node::CONSTRAINT,
        "con:forge-scaling",
        node::VERIFICATION,
        "ver:walk",
        Some("clamped"),
        Some("2026-08-24"),
    )
    .unwrap();

    let f = &g.invalidated_findings().unwrap()[0];
    assert_eq!(
        f.rerun_owed,
        Some(false),
        "the run already reflects the repair, so this failure is current, not stale"
    );
}

/// The third value is the point, and it is the one a reader gets wrong.
#[test]
fn an_undated_claim_says_nobody_can_tell_rather_than_no() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    g.invalidates(
        node::CONSTRAINT,
        "con:forge-scaling",
        node::VERIFICATION,
        "ver:walk",
        Some("clamped, but nobody wrote down when"),
        None,
    )
    .unwrap();

    let f = &g.invalidated_findings().unwrap()[0];
    assert_eq!(
        f.rerun_owed, None,
        "UNDATED IS REPORTED, NEVER GUESSED — null must not collapse to false, \
         which would read as 'the run already covers it'"
    );
    assert_eq!(
        f.undated_claims, 1,
        "and the reader is told what it rests on"
    );
}

/// The load-bearing restraint: this says a verdict is STALE, never that it
/// turned. Only a run may move a status.
#[test]
fn invalidating_a_check_never_moves_its_status() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    g.invalidates(
        node::CONSTRAINT,
        "con:forge-scaling",
        node::VERIFICATION,
        "ver:walk",
        Some("clamped"),
        Some("2026-08-24"),
    )
    .unwrap();

    let v = g.get_node(node::VERIFICATION, "ver:walk").unwrap().unwrap();
    assert_eq!(
        v.properties.get("status").and_then(|p| p.as_str()),
        Some("failing"),
        "a repair is not a measurement; reflow2 must not assert an outcome nobody took"
    );
    let f = &g.invalidated_findings().unwrap()[0];
    assert_eq!(
        f.status.as_deref(),
        Some("failing"),
        "and the reader is shown the unchanged verdict beside the claim"
    );
}

/// It generalises: the same relation closes a stale MEASUREMENT, which is where
/// this project measured the problem independently six days earlier.
#[test]
fn the_same_edge_closes_a_stale_temporal_fact() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:service", "Service", "the service", None)
        .unwrap();
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:orientation-is-slow",
        reflow2_core::nodes::Props::new()
            .set("subject_id", "cmp:service")
            .set(
                "statement",
                "open_questions takes 11.7s. NOTHING WAS OPTIMISED.",
            )
            .set("basis", "measured")
            .set("valid_from", "2026-08-09"),
    )
    .unwrap();
    g.add_change_event(
        "chg:optimised",
        "The orientation reads got fast",
        reflow2_core::ChangeType::PerformanceOptimization,
        None,
        Some("open_questions 11.7s -> 2.95s"),
        None,
        Some("2026-08-20"),
    )
    .unwrap();

    g.invalidates(
        node::CHANGE_EVENT,
        "chg:optimised",
        node::TEMPORAL_FACT,
        "fact:orientation-is-slow",
        Some("the measurement was taken and the fix landed; these numbers are gone"),
        Some("2026-08-20"),
    )
    .unwrap();

    let found = g.invalidated_findings().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].finding_id, "fact:orientation-is-slow");
    assert_eq!(
        found[0].finding_type, "TemporalFact",
        "one relation covers both carriers — a check's run and a fact's measurement"
    );
    assert_eq!(
        found[0].rerun_owed, None,
        "a TemporalFact has no last_run_at, so nobody can order the two: reported, not guessed"
    );
}

/// A finding nobody has claimed must not appear — otherwise the reader cannot
/// tell a closed finding from an open one, which is the whole point.
#[test]
fn a_finding_with_no_claim_is_not_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    assert!(
        g.invalidated_findings().unwrap().is_empty(),
        "silence here means nobody has claimed anything, and that is a real answer"
    );
}

/// `add_change_event` can now date a change — the field that was declared,
/// written by the reconcile paths, and unreachable from the ordinary path.
#[test]
fn a_hand_written_change_can_carry_its_date() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let e = g
        .add_change_event(
            "chg:dated",
            "A dated change",
            reflow2_core::ChangeType::DefectFix,
            None,
            Some("what changed"),
            Some("why"),
            Some("2026-08-24"),
        )
        .unwrap();
    assert_eq!(
        e.properties.get("detected_at").and_then(|v| v.as_str()),
        Some("2026-08-24"),
        "8% of hand-written events carried a date because this parameter did not exist"
    );
}

/// The edge must not quietly enlarge a blast radius. Asserted BEHAVIOURALLY —
/// the only path from the repair to the check is the INVALIDATES edge, so if
/// propagation reached the check it could only have travelled along it.
///
/// It is a statement ABOUT two records, like AUTHORED_BY and OWNED_BY, not a
/// path a change travels: a repair reaching into every check it answered would
/// make repairs into hubs and put findings in the impact set of the very work
/// that answered them.
#[test]
fn invalidates_never_enlarges_a_blast_radius() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // An ISOLATED check — nothing verifies anything, so there is no other route.
    g.add_verification("ver:lonely", "a lonely check", Some("test"), None, None)
        .unwrap();
    g.set_verification_status("ver:lonely", "failing", Some("2026-08-23"), None)
        .unwrap();
    g.add_change_event(
        "chg:repair",
        "The repair",
        reflow2_core::ChangeType::DefectFix,
        None,
        Some("fixed it"),
        None,
        Some("2026-08-24"),
    )
    .unwrap();
    g.invalidates(
        node::CHANGE_EVENT,
        "chg:repair",
        node::VERIFICATION,
        "ver:lonely",
        Some("answered"),
        Some("2026-08-24"),
    )
    .unwrap();

    let impact = g
        .propagate_change("chg:repair", reflow2_core::PropagateOptions::default())
        .unwrap();
    assert!(
        !impact.impacted.iter().any(|i| i.node_id == "ver:lonely"),
        "the check must not be in the repair's blast radius: the claim says a \
         verdict is stale, it does not make the check depend on the repair"
    );
}

/// Guard against the detector regressing: a design where the repair is recorded
/// must still raise its ordinary gaps. Closing a finding is not silencing one.
#[test]
fn recording_a_repair_does_not_suppress_unrelated_gaps() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_failing_check_and_its_repair(&mut g);
    g.add_requirement("req:open", "Something unmet", "It must hold.")
        .unwrap();
    g.set_requirement_status("req:open", "accepted").unwrap();
    g.invalidates(
        node::CONSTRAINT,
        "con:forge-scaling",
        node::VERIFICATION,
        "ver:walk",
        Some("clamped"),
        Some("2026-08-24"),
    )
    .unwrap();

    let still_open = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .any(|gap| gap.gap_source == GapSource::UnsatisfiedRequirement);
    assert!(
        still_open,
        "an INVALIDATES edge closes ONE finding; it must not quiet the gap list"
    );
}
