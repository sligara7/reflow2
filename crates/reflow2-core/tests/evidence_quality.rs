//! The evidence-quality family — BL-106, BL-126, BL-136.
//!
//! One hole, three axes. The graph has always recorded that a check EXISTS and
//! that it PASSES, and never what its evidence actually covers:
//!
//! * **TIME** (BL-106) — the check is older than the code it covers.
//!   `cap:degraded-surface` was `verified` on a check that drove stdio only
//!   while `art:main` drifted twice underneath it, and `detect_gaps` returned
//!   zero throughout, correctly.
//! * **INPUT** (BL-126) — the check only ever ran at one point of the space it
//!   claims. A design declared a parameter arbitrary, every check in the suite
//!   pinned it, and six of eight alternative values broke the invariant.
//! * **INDEPENDENCE** (BL-136) — the thing the check validates was FITTED to
//!   the evidence the check rests on, so agreement is a fit rather than a test.
//!
//! The counterweights matter as much as the findings here, because all three
//! detectors could be "passed" by a rule that simply reports everything. Each
//! axis therefore pins both directions: the narrow case fires, and the honest
//! case stays silent.

use reflow2_core::confirm::VerificationFreshness;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g
}

/// A capability with a realizing artifact — the shape the confirmation ledger
/// needs before it will report on a claim at all.
fn built_capability(g: &mut DesignGraph, cap: &str, art: &str) {
    g.add_capability(cap, cap, "does a thing", None).unwrap();
    g.create_node(node::ARTIFACT, art, Props::new().set("name", art))
        .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        art,
        node::CAPABILITY,
        cap,
        Props::new(),
    )
    .unwrap();
}

fn passing_check(g: &mut DesignGraph, id: &str, cap: &str, ran_at: Option<&str>) {
    g.add_verification(id, id, Some("test"), Some("system"))
        .unwrap();
    g.set_verification_status(id, "passing", ran_at).unwrap();
    g.verifies(id, node::CAPABILITY, cap).unwrap();
}

/// An accepted baseline change on the artifact, dated — the "the code moved and
/// somebody signed it off" record the ledger already reads.
fn accept(g: &mut DesignGraph, id: &str, art: &str, at: &str) {
    g.create_node(
        node::CHANGE_EVENT,
        id,
        Props::new()
            .set("name", id)
            .set("change_type", "resync")
            .set("detected_at", at),
    )
    .unwrap();
    g.create_edge(
        edge::CHANGED,
        node::CHANGE_EVENT,
        id,
        node::ARTIFACT,
        art,
        Props::new().set("accepted_baseline", true),
    )
    .unwrap();
}

// ---- BL-106 · the TIME axis -------------------------------------------------

/// The BL-105 state, now visible: the code was accepted after the check last
/// ran, so the green tick is resting on evidence that predates what it covers.
#[test]
fn an_accept_dated_after_the_check_reports_stale() {
    let mut g = design();
    built_capability(&mut g, "cap:degraded", "art:main");
    passing_check(&mut g, "ver:stdio", "cap:degraded", Some("2026-07-24"));
    accept(&mut g, "chg:http", "art:main", "2026-07-26");

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:degraded")
        .unwrap();
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Stale,
        "{c:?}"
    );
    assert_eq!(c.last_verified_at.as_deref(), Some("2026-07-24"));
    assert_eq!(c.last_claim_at.as_deref(), Some("2026-07-26"));
    assert_eq!(l.stale_verification, 1);
}

/// THE COUNTERWEIGHT. A check re-run after the change is current, and must not
/// be reported — a freshness signal that fired on correct work would be the
/// BL-23 failure the whole gap workflow exists to prevent.
#[test]
fn a_check_re_run_after_the_accept_is_current() {
    let mut g = design();
    built_capability(&mut g, "cap:degraded", "art:main");
    accept(&mut g, "chg:http", "art:main", "2026-07-26");
    passing_check(&mut g, "ver:both", "cap:degraded", Some("2026-07-27"));

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:degraded")
        .unwrap();
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Current,
        "{c:?}"
    );
    assert_eq!(l.stale_verification, 0);
}

/// Undated on either side is UNKNOWN and is reported as such — never a silent
/// pass. An unanswerable freshness question presented as freshness is the same
/// lie in a new place, and the core takes no clock with which to invent a date.
#[test]
fn an_undated_check_reports_unknown_not_current() {
    let mut g = design();
    built_capability(&mut g, "cap:a", "art:a");
    passing_check(&mut g, "ver:a", "cap:a", None); // never dated
    accept(&mut g, "chg:a", "art:a", "2026-07-26");

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Unknown,
        "{c:?}"
    );
    assert_eq!(
        l.stale_verification, 0,
        "unknown must not be counted as stale"
    );
    assert_eq!(l.unknown_verification_freshness, 1, "…nor silently dropped");
}

/// A failing check's date is not evidence whose age is worth comparing
/// (`dec:passing-is-verified`), so it cannot make a claim look fresh.
#[test]
fn a_failing_check_does_not_refresh_the_claim() {
    let mut g = design();
    built_capability(&mut g, "cap:a", "art:a");
    passing_check(&mut g, "ver:old", "cap:a", Some("2026-07-01"));
    accept(&mut g, "chg:a", "art:a", "2026-07-10");
    // A newer run that FAILED — recent, and no evidence at all.
    g.add_verification("ver:new", "ver:new", Some("test"), Some("system"))
        .unwrap();
    g.set_verification_status("ver:new", "failing", Some("2026-07-20"))
        .unwrap();
    g.verifies("ver:new", node::CAPABILITY, "cap:a").unwrap();

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    assert_eq!(c.last_verified_at.as_deref(), Some("2026-07-01"));
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Stale,
        "{c:?}"
    );
}

/// FOUND BY DOGFOODING THIS ON REFLOW2'S OWN GRAPH, before it shipped. A check
/// dated `2026-07-28` against an accept at `2026-07-28T14:52:00-04:00` is the
/// SAME DAY at two precisions, and comparing the strings whole called it stale
/// because the shorter one sorts first — an ordering nobody recorded, asserted
/// by the very report that exists to stop exactly that. Same day is Unknown:
/// deciding it needs the timestamps parsed and normalised across UTC offsets,
/// and the core takes no clock.
#[test]
fn the_same_day_at_two_precisions_is_unknown_not_stale() {
    let mut g = design();
    built_capability(&mut g, "cap:latent", "art:latent");
    passing_check(&mut g, "ver:latent", "cap:latent", Some("2026-07-28"));
    accept(
        &mut g,
        "chg:latent",
        "art:latent",
        "2026-07-28T14:52:00-04:00",
    );

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:latent")
        .unwrap();
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Unknown,
        "{c:?}"
    );
    assert_eq!(l.stale_verification, 0);
}

/// THE COUNTERWEIGHT to the rule above, and the reason it is a date compare and
/// not a shrug: a genuinely earlier DAY still reports stale however precise the
/// other side is. Widening "unknown" until nothing is ever stale would answer
/// the false positive by deleting the signal.
#[test]
fn an_earlier_day_is_still_stale_against_a_full_timestamp() {
    let mut g = design();
    built_capability(&mut g, "cap:a", "art:a");
    passing_check(&mut g, "ver:a", "cap:a", Some("2026-07-25"));
    accept(&mut g, "chg:a", "art:a", "2026-07-28T14:52:00-04:00");

    let l = g.confirmation_ledger().unwrap();
    let c = l
        .claims
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    assert_eq!(
        c.verification_freshness,
        VerificationFreshness::Stale,
        "{c:?}"
    );
    assert_eq!(l.stale_verification, 1);
}

// ---- BL-126 · the INPUT axis ------------------------------------------------

/// "31 checks, all at one value." Every passing check pins `seed`, so the
/// capability is proven only at a single point of an axis the design declares
/// arbitrary — and until now that read exactly like full coverage.
#[test]
fn a_parameter_every_check_pins_is_reported_as_narrow() {
    let mut g = design();
    g.add_capability("cap:rollup", "Rollup", "aggregates", None)
        .unwrap();
    for id in ["ver:one", "ver:two", "ver:three"] {
        passing_check(&mut g, id, "cap:rollup", None);
        g.set_evidence_scope(id, node::CAPABILITY, "cap:rollup", &["seed".into()], &[])
            .unwrap();
    }

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:rollup")
        .unwrap();
    assert_eq!(c.pinned_everywhere, vec!["seed".to_string()], "{c:?}");
    assert_eq!(r.narrowly_proven, 1);
}

/// THE COUNTERWEIGHT, and the one that keeps this from being noise: ONE check
/// that actually varies the parameter ends the claim of narrowness, however
/// many others pinned it. The question is whether the claim rests on a single
/// point — not whether any individual check was broad.
#[test]
fn one_check_that_sweeps_it_ends_the_narrowness() {
    let mut g = design();
    g.add_capability("cap:rollup", "Rollup", "aggregates", None)
        .unwrap();
    for id in ["ver:one", "ver:two"] {
        passing_check(&mut g, id, "cap:rollup", None);
        g.set_evidence_scope(id, node::CAPABILITY, "cap:rollup", &["seed".into()], &[])
            .unwrap();
    }
    passing_check(&mut g, "ver:sweep", "cap:rollup", None);
    g.set_evidence_scope(
        "ver:sweep",
        node::CAPABILITY,
        "cap:rollup",
        &[],
        &["seed".into()],
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:rollup")
        .unwrap();
    assert!(c.pinned_everywhere.is_empty(), "{c:?}");
    assert_eq!(c.swept, vec!["seed".to_string()]);
    assert_eq!(r.narrowly_proven, 0);
}

/// Scope is a fact about the CLAIM, which is the whole reason it lives on the
/// edge (`dec:evidence-scope-on-the-verifies-edge`). ONE suite, TWO capabilities,
/// broad about one and narrow about the other — the case a property on the
/// Verification node would have to average away.
#[test]
fn one_suite_can_be_broad_about_one_claim_and_narrow_about_another() {
    let mut g = design();
    g.add_capability("cap:a", "A", "a", None).unwrap();
    g.add_capability("cap:b", "B", "b", None).unwrap();
    g.add_verification("ver:suite", "suite", Some("test"), Some("system"))
        .unwrap();
    g.set_verification_status("ver:suite", "passing", None)
        .unwrap();
    g.verifies("ver:suite", node::CAPABILITY, "cap:a").unwrap();
    g.verifies("ver:suite", node::CAPABILITY, "cap:b").unwrap();
    g.set_evidence_scope(
        "ver:suite",
        node::CAPABILITY,
        "cap:a",
        &[],
        &["seed".into()],
    )
    .unwrap();
    g.set_evidence_scope(
        "ver:suite",
        node::CAPABILITY,
        "cap:b",
        &["seed".into()],
        &[],
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let a = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    let b = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:b")
        .unwrap();
    assert!(a.pinned_everywhere.is_empty(), "broad about A: {a:?}");
    assert_eq!(
        b.pinned_everywhere,
        vec!["seed".to_string()],
        "narrow about B: {b:?}"
    );
    assert_eq!(
        r.narrowly_proven, 1,
        "exactly one claim is narrow, not the suite"
    );
}

/// Silence about coverage is NOT coverage. A check stating no scope is counted
/// and reported unscoped, so "0 narrowly proven" can never be read as
/// "everything is broadly proven" — the same rule `unplaced_checks` applies to
/// place.
#[test]
fn a_check_that_states_no_scope_is_unscoped_not_broad() {
    let mut g = design();
    g.add_capability("cap:a", "A", "a", None).unwrap();
    passing_check(&mut g, "ver:a", "cap:a", None); // no scope ever set

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    assert_eq!(c.unscoped_checks, 1, "{c:?}");
    assert!(
        c.pinned_everywhere.is_empty(),
        "unstated is not narrow either"
    );
    assert_eq!(r.narrowly_proven, 0);
    assert_eq!(
        r.with_unscoped_checks, 1,
        "and it must not vanish from the rollup"
    );
}

/// A parameter name carrying a comma is REFUSED rather than stored, because the
/// flat encoding would read it back as two parameters that were never checked.
#[test]
fn a_parameter_name_with_a_comma_is_refused() {
    let mut g = design();
    g.add_capability("cap:a", "A", "a", None).unwrap();
    passing_check(&mut g, "ver:a", "cap:a", None);

    let err = g
        .set_evidence_scope(
            "ver:a",
            node::CAPABILITY,
            "cap:a",
            &["seed,order".into()],
            &[],
        )
        .unwrap_err();
    assert!(format!("{err}").contains("comma"), "{err}");

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:a")
        .unwrap();
    assert!(c.pinned_everywhere.is_empty(), "nothing was stored: {c:?}");
}

/// Scoping a pair that has no VERIFIES edge is refused: silently creating one
/// would let a typo invent a verification relationship nobody asserted.
#[test]
fn scoping_a_check_that_does_not_verify_the_target_is_refused() {
    let mut g = design();
    g.add_capability("cap:a", "A", "a", None).unwrap();
    g.add_capability("cap:b", "B", "b", None).unwrap();
    passing_check(&mut g, "ver:a", "cap:a", None);

    let err = g
        .set_evidence_scope("ver:a", node::CAPABILITY, "cap:b", &["seed".into()], &[])
        .unwrap_err();
    assert!(format!("{err}").contains("does not verify"), "{err}");
}

// ---- BL-136 · the INDEPENDENCE axis -----------------------------------------

/// The QBI shape. A value fitted to an anchor, and the only check of it is
/// agreement with that same anchor: a fit, not a test. Every status still reads
/// green, which is exactly why this has to be structural.
#[test]
fn a_check_the_target_was_calibrated_against_is_consumed() {
    let mut g = design();
    g.add_capability("cap:layout", "Layout", "lays out", None)
        .unwrap();
    passing_check(&mut g, "ver:footprint", "cap:layout", None);
    g.calibrated_against(
        node::CAPABILITY,
        "cap:layout",
        node::VERIFICATION,
        "ver:footprint",
        Some("coefficient fitted to this check's anchor"),
        None,
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:layout")
        .unwrap();
    assert_eq!(c.consumed_checks.len(), 1, "{c:?}");
    assert_eq!(c.consumed_checks[0].verification_id, "ver:footprint");
    assert_eq!(c.independent_checks, 0);
    assert!(!c.independently_verified);
    assert_eq!(r.not_independently_verified, 1);
}

/// THE COUNTERWEIGHT. A second, genuinely independent check restores the claim
/// — the fit is still reported as consumed, but the capability is verified.
/// Without this the rule would condemn every calibrated design outright, which
/// is not what BL-136 asks for: it asks that the fit not be COUNTED as a test.
#[test]
fn an_independent_check_beside_the_fit_restores_the_claim() {
    let mut g = design();
    g.add_capability("cap:layout", "Layout", "lays out", None)
        .unwrap();
    passing_check(&mut g, "ver:footprint", "cap:layout", None);
    passing_check(&mut g, "ver:outside-source", "cap:layout", None);
    g.calibrated_against(
        node::CAPABILITY,
        "cap:layout",
        node::VERIFICATION,
        "ver:footprint",
        None,
        None,
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:layout")
        .unwrap();
    assert_eq!(c.consumed_checks.len(), 1, "the fit is still named: {c:?}");
    assert_eq!(c.independent_checks, 1);
    assert!(c.independently_verified);
    assert_eq!(r.not_independently_verified, 0);
}

/// THE SECOND COUNTERWEIGHT, and the one that stops this reporting everything:
/// a capability calibrated against evidence NO check of it rests on is not
/// circular. Being fitted to something is not the defect — being validated by
/// what you were fitted to is.
#[test]
fn a_calibration_against_unrelated_evidence_is_not_circular() {
    let mut g = design();
    g.add_capability("cap:layout", "Layout", "lays out", None)
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:paper",
        Props::new().set("name", "paper"),
    )
    .unwrap();
    passing_check(&mut g, "ver:independent", "cap:layout", None);
    g.calibrated_against(
        node::CAPABILITY,
        "cap:layout",
        node::ARTIFACT,
        "art:paper",
        Some("fitted to the published anchor"),
        None,
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:layout")
        .unwrap();
    assert!(c.consumed_checks.is_empty(), "not circular: {c:?}");
    assert!(c.independently_verified);
}

/// The second form of the circle: the check PRODUCED the artifact the value was
/// fitted to. Same defect one hop out, and the commoner shape in practice —
/// nobody calibrates against a test, they calibrate against its output.
#[test]
fn a_check_that_produced_the_calibration_anchor_is_consumed() {
    let mut g = design();
    g.add_capability("cap:layout", "Layout", "lays out", None)
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:run",
        Props::new().set("name", "run output"),
    )
    .unwrap();
    passing_check(&mut g, "ver:sweep", "cap:layout", None);
    g.create_edge(
        edge::PRODUCES,
        node::VERIFICATION,
        "ver:sweep",
        node::ARTIFACT,
        "art:run",
        Props::new(),
    )
    .unwrap();
    g.calibrated_against(
        node::CAPABILITY,
        "cap:layout",
        node::ARTIFACT,
        "art:run",
        None,
        None,
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:layout")
        .unwrap();
    assert_eq!(c.consumed_checks.len(), 1, "{c:?}");
    assert_eq!(c.consumed_checks[0].evidence_id, "art:run");
    assert!(!c.independently_verified);
}

/// A fitted constant lives in a FILE while the check names the capability, so a
/// calibration recorded on the realizing artifact must count too — otherwise the
/// commonest real shape is invisible.
#[test]
fn a_calibration_on_the_realizing_artifact_counts() {
    let mut g = design();
    built_capability(&mut g, "cap:layout", "art:layout-rs");
    passing_check(&mut g, "ver:footprint", "cap:layout", None);
    g.calibrated_against(
        node::ARTIFACT,
        "art:layout-rs",
        node::VERIFICATION,
        "ver:footprint",
        None,
        None,
    )
    .unwrap();

    let r = g.evidence_report().unwrap();
    let c = r
        .capabilities
        .iter()
        .find(|c| c.capability_id == "cap:layout")
        .unwrap();
    assert_eq!(c.consumed_checks.len(), 1, "{c:?}");
    assert!(!c.independently_verified);
}

/// `dec:calibration-propagates`, asked before the code was written rather than
/// after a detector complained: correcting the anchor must put every value
/// fitted to it in the blast radius. INCLUDES and SCHEDULED_FOR each reached the
/// impact table only once `disconnected_community` fired on an island they had
/// failed to join; this pins the third such edge.
#[test]
fn correcting_the_anchor_reaches_what_was_fitted_to_it() {
    let mut g = design();
    g.add_capability("cap:layout", "Layout", "lays out", None)
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:paper",
        Props::new().set("name", "paper"),
    )
    .unwrap();
    g.calibrated_against(
        node::CAPABILITY,
        "cap:layout",
        node::ARTIFACT,
        "art:paper",
        None,
        None,
    )
    .unwrap();

    let radius = g
        .propagate_from(
            &["art:paper"],
            reflow2_core::PropagateOptions { max_depth: 3 },
        )
        .unwrap();
    assert!(
        radius.impacted.iter().any(|i| i.node_id == "cap:layout"),
        "a superseded anchor must reach the value fitted to it: {radius:?}"
    );
}
