//! What the graph can say about the quality axes (BL-184).
//!
//! Anthony's correction is the specification: *"I'm not sure a blanket yes or
//! no can be applied here. I feel like there are some areas where the graph may
//! inform or give a computed signal that indicates one of the 'ilities' are not
//! sufficiently being met by the design."*
//!
//! So the cases below defend three lines at once, and each is a way this could
//! go wrong:
//!
//! - **It must not derive a score.** Collapsing findings into a float is the
//!   lossy move `dec:readiness-is-an-observation-the-threshold-is-the-judgement`
//!   already refused for TRL.
//! - **It must not re-judge another module's findings.** A ratio, a trajectory
//!   position and a granularity observation are context; calling them adverse
//!   would overrule modules that deliberately declined to grade, and would
//!   smuggle back the thresholds they were built without.
//! - **It must not pretend.** Four axes cannot be informed by a design graph,
//!   and they say so with a reason rather than appearing clean.

use reflow2_core::ility::ility_source;
use reflow2_core::nodes::{edge, node};
use reflow2_core::{DesignGraph, IlityReport, IlitySignal};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

fn report(g: &DesignGraph) -> IlityReport {
    g.ility_report().expect("ility report")
}

fn axis<'a>(r: &'a IlityReport, name: &str) -> &'a IlitySignal {
    r.signals
        .iter()
        .find(|s| s.dimension == name)
        .unwrap_or_else(|| panic!("axis {name} is missing"))
}

/// Two components in a dependency loop — a defect its own module names.
fn seed_cycle(g: &mut DesignGraph) {
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_component("cmp:a", "a", "One part.", None).expect("a");
    g.add_component("cmp:b", "b", "Another.", None).expect("b");
    for (from, to) in [("cmp:a", "cmp:b"), ("cmp:b", "cmp:a")] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            from,
            node::COMPONENT,
            to,
            [],
        )
        .expect("coupling");
    }
}

/// Someone's stated score for an axis on a target.
fn assert_score(g: &mut DesignGraph, id: &str, target: &str, dimension: &str, score: f64) {
    g.create_node(
        node::DIMENSION_ASSESSMENT,
        id,
        [
            ("target_id".to_string(), reflow2_core::Value::from(target)),
            (
                "dimension".to_string(),
                reflow2_core::Value::from(dimension),
            ),
            ("score".to_string(), reflow2_core::Value::from(score)),
        ],
    )
    .expect("assessment");
}

// ---------------------------------------------------------------------------
// The refusals.
// ---------------------------------------------------------------------------

/// **The load-bearing case.** No score is ever derived, and no word in the
/// report reads as one. The precedent is Anthony's own: TRL was kept out of
/// this float because a ladder enters it lossily.
#[test]
fn no_score_is_ever_derived() {
    let mut g = graph();
    seed_cycle(&mut g);

    let json = serde_json::to_string(&report(&g)).expect("serialize");

    // Matched as JSON KEYS (trailing colon), not as prose: the report
    // legitimately says the words "grade" and "score" while explaining what it
    // refuses to do, and a test that tripped on its own explanation would be
    // testing the wrong thing.
    for forbidden in [
        "\"derived_score\":",
        "\"computed_score\":",
        "\"rating\":",
        "\"grade\":",
        "\"overall\":",
        "\"value\":",
    ] {
        assert!(
            !json.contains(forbidden),
            "the reading must not derive a score, but it carries the field {forbidden:?}"
        );
    }
    // The only `score` fields present are ASSERTED ones, echoed back.
    assert_eq!(
        json.matches("\"score\":").count(),
        0,
        "no score field at all when nobody asserted one"
    );
    assert!(json.contains("never derives a number"));
}

/// Four axes cannot be informed by a design graph. They must say so with a
/// reason — an honest silence, not an absent entry and not a clean bill.
#[test]
fn the_uncomputable_axes_say_why_rather_than_reading_clean() {
    let mut g = graph();
    seed_cycle(&mut g);

    let r = report(&g);

    assert_eq!(r.signals.len(), 9, "every axis appears");
    for name in ["performance", "security", "scalability", "observability"] {
        let s = axis(&r, name);
        assert!(!s.informed, "{name} must not claim to be informed");
        assert!(
            s.not_informed_because.is_some(),
            "{name} must give its reason"
        );
        assert!(s.evidence.is_empty(), "{name} must carry no evidence");
    }
    for name in [
        "reliability",
        "maintainability",
        "testability",
        "coupling",
        "maturity",
    ] {
        let s = axis(&r, name);
        assert!(s.informed, "{name} should be informed");
        assert!(s.not_informed_because.is_none());
    }
}

/// **Adverse is inherited, never re-judged.** A ratio, a trajectory position
/// and a granularity observation are context — the modules that produce them
/// deliberately refuse to grade, and relabelling them here would overrule that
/// and reintroduce the thresholds they were built without.
#[test]
fn context_from_a_module_that_refuses_to_judge_is_not_counted_against_an_axis() {
    let mut g = graph();
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_component("cmp:a", "a", "One part.", None).expect("a");

    let r = report(&g);

    let coupling = axis(&r, "coupling");
    assert!(
        coupling
            .evidence
            .iter()
            .any(|e| e.source == ility_source::MODULARITY),
        "modularity should be reported"
    );
    for e in &coupling.evidence {
        if e.source == ility_source::MODULARITY || e.source == ility_source::SEAMS_BAND {
            assert!(!e.adverse, "{} must be context, not a charge", e.source);
        }
    }
    let maturity = axis(&r, "maturity");
    assert!(
        maturity.evidence.iter().all(|e| !e.adverse),
        "a position on the trajectory is never adverse"
    );
}

/// A defect category mapped to no axis contributes nothing — `orphan_node` is a
/// real finding about the design and says nothing about any quality axis, so a
/// parked decision must not read as a maintainability problem. This design
/// carries two such decisions.
///
/// **What this does NOT prove, found by mutation-checking and worth saying:**
/// removing the `HealSeverity::Info` guard in `ility.rs` does not fail any test
/// here, because all four *mapped* categories are Warning-or-Critical by
/// construction and `orphan_node` is filtered earlier, by having no axis at
/// all. That guard is therefore unreachable today and kept as defence for the
/// day someone maps a category that can be `info` — see the comment at its
/// site. Naming the test for what it proves rather than for what it looks like
/// it proves is the point.
#[test]
fn an_unmapped_defect_category_is_not_evidence_against_an_axis() {
    let mut g = graph();
    g.add_project("proj:demo", "Demo").expect("project");
    // A parked decision: recorded, governing nothing — orphan_node at `info`.
    g.add_decision("dec:parked", "Parked", "An open question.", None)
        .expect("decision");

    let r = report(&g);

    for s in &r.signals {
        assert_eq!(
            s.adverse_findings, 0,
            "{} counted an info-level finding against itself",
            s.dimension
        );
    }
}

// ---------------------------------------------------------------------------
// The signal itself.
// ---------------------------------------------------------------------------

/// A dependency loop is a defect by its own module's reckoning, and it lands on
/// every axis it genuinely bears on — you cannot build, test or reason about
/// either part alone.
#[test]
fn a_dependency_cycle_counts_against_the_axes_it_actually_harms() {
    let mut g = graph();
    seed_cycle(&mut g);

    let r = report(&g);

    for name in ["maintainability", "testability", "coupling"] {
        let s = axis(&r, name);
        assert!(
            s.adverse_findings > 0,
            "a cycle must count against {name}: {:?}",
            s.evidence
        );
        assert!(
            s.evidence
                .iter()
                .any(|e| e.source == ility_source::CIRCULAR_DEPENDENCY && e.adverse),
            "{name} should name the cycle as its source"
        );
    }
    // And nowhere it does not bear on.
    assert_eq!(axis(&r, "reliability").adverse_findings, 0);
}

/// **The output worth reading**: somebody scored an axis good on a target that
/// a detector found something against. Two records pointing different ways —
/// the same shape as `compare_designs`, and reflow2 rules on neither.
#[test]
fn an_asserted_score_against_adverse_evidence_is_flagged() {
    let mut g = graph();
    seed_cycle(&mut g);
    assert_score(&mut g, "dim:a", "cmp:a", "maintainability", 0.9);

    let r = report(&g);
    let m = axis(&r, "maintainability");

    assert!(
        m.worth_weighing.contains(&"cmp:a".to_string()),
        "an asserted 0.9 over a cycle must be flagged: {:?}",
        m.worth_weighing
    );
    assert_eq!(m.asserted.len(), 1);
    assert_eq!(m.asserted[0].score, 0.9);
    assert!(
        r.notes
            .iter()
            .any(|n| n.contains("disagreement between two records"))
    );
}

/// A low score over adverse evidence agrees with it — nothing to weigh.
#[test]
fn an_asserted_score_that_agrees_is_not_flagged() {
    let mut g = graph();
    seed_cycle(&mut g);
    assert_score(&mut g, "dim:a", "cmp:a", "maintainability", 0.2);

    let r = report(&g);

    assert!(axis(&r, "maintainability").worth_weighing.is_empty());
}

/// **The asymmetry, asserted deliberately.** A low score with NO adverse
/// finding is not flagged, because "no detector fired" is an absence of
/// evidence — not evidence the score is wrong. Flagging it would be the
/// reading claiming a clean axis is healthy, which it cannot know.
#[test]
fn a_pessimistic_score_over_a_quiet_axis_is_not_flagged() {
    let mut g = graph();
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_component("cmp:a", "a", "One part.", None).expect("a");
    assert_score(&mut g, "dim:a", "cmp:a", "maintainability", 0.1);

    let r = report(&g);
    let m = axis(&r, "maintainability");

    assert_eq!(m.adverse_findings, 0);
    assert!(
        m.worth_weighing.is_empty(),
        "absence of findings must not be read as evidence against a low score"
    );
    assert!(
        r.not_observed_about
            .iter()
            .any(|s| s.contains("absence of evidence")),
        "the asymmetry must be stated: {:?}",
        r.not_observed_about
    );
}

/// A score on a target the evidence never mentions is not flagged — the
/// disagreement has to be about the same thing.
#[test]
fn a_score_on_an_unrelated_target_is_not_flagged() {
    let mut g = graph();
    seed_cycle(&mut g);
    g.add_component("cmp:elsewhere", "elsewhere", "Uninvolved.", None)
        .expect("component");
    assert_score(&mut g, "dim:x", "cmp:elsewhere", "maintainability", 0.95);

    let r = report(&g);

    assert!(axis(&r, "maintainability").worth_weighing.is_empty());
}

// ---------------------------------------------------------------------------
// Housekeeping.
// ---------------------------------------------------------------------------

/// Every source is listed once, so a reader can audit where a signal came from
/// — the discipline `changelog_rule` and `preserve_rule` already hold.
#[test]
fn every_source_is_listed_once() {
    let mut sorted = ility_source::ALL.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "duplicate in ility_source::ALL");
    assert_eq!(before, 13, "a source was added without updating ALL");
}

/// Same design, byte-identical report.
#[test]
fn the_report_is_deterministic() {
    let mut g = graph();
    seed_cycle(&mut g);
    assert_score(&mut g, "dim:a", "cmp:a", "maintainability", 0.9);

    let a = serde_json::to_string(&report(&g)).expect("serialize");
    let b = serde_json::to_string(&report(&g)).expect("serialize");
    assert_eq!(a, b);
}
