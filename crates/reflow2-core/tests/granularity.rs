//! Granularity — does the build separate what the design separates? (BL-182)
//!
//! **The cases that matter most here are the silences.** This reading exists
//! because Anthony spotted by eye that one file held 139 tools while every
//! structural instrument reflow2 owns reported the design healthy — and his
//! immediate objection to fixing it was the right one: *"avoid monoliths" is a
//! subjective design principle, so what exactly would detect it?*
//!
//! The answer this module gives is narrow on purpose. It does not detect
//! monoliths. It detects that **two records the design already holds disagree**
//! about how many things there are — N Capabilities in the design, one Artifact
//! in the build — and it refuses to say which side is wrong.
//!
//! So the tests below spend most of their effort proving it stays quiet when it
//! should:
//!
//! - a young design where everything lives in one file has no outlier and is
//!   told nothing, because there is nothing to be out of line *with*;
//! - a design too small to have a distribution says so rather than inventing
//!   one;
//! - "the design separates two things and the build separates neither" is true
//!   and not worth saying, so it is not said.
//!
//! That last one is not fastidiousness. Counts pile up at one, the standard
//! deviation collapses, and a purely distributional cutoff fires on noise.

use reflow2_core::granularity::{MIN_DISTINCTIONS, MIN_POPULATION, UNUSUAL_AT};
use reflow2_core::{DesignGraph, GranularityReport};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

/// Register `n` artifacts that each realize one capability — the ordinary
/// shape, and the baseline every outlier is measured against.
fn seed_ordinary(g: &mut DesignGraph, n: usize) {
    g.add_component("cmp:main", "main", "The system.", None)
        .expect("component");
    for i in 0..n {
        let cap = format!("cap:ordinary{i}");
        let art = format!("art:ordinary{i}");
        g.add_capability(&cap, &format!("Ordinary {i}"), "Does one thing.", None)
            .expect("capability");
        g.allocate(&cap, "cmp:main").expect("allocate");
        g.add_artifact(&art, &format!("ordinary{i}.rs"), Some("code"), None)
            .expect("artifact");
        g.realizes(&art, "Capability", &cap, None)
            .expect("realizes");
    }
}

/// One artifact that swallows `n` capabilities the design distinguishes.
fn seed_coarse(g: &mut DesignGraph, id: &str, n: usize) {
    g.add_artifact(id, "coarse.rs", Some("code"), None)
        .expect("artifact");
    for i in 0..n {
        let cap = format!("cap:{id}-{i}");
        g.add_capability(&cap, &format!("Swallowed {i}"), "Does a thing.", None)
            .expect("capability");
        g.allocate(&cap, "cmp:main").expect("allocate");
        g.realizes(id, "Capability", &cap, None).expect("realizes");
    }
}

fn report(g: &DesignGraph) -> GranularityReport {
    g.granularity_report().expect("granularity report")
}

// ---------------------------------------------------------------------------
// The silences — what must NOT be reported.
// ---------------------------------------------------------------------------

/// **The load-bearing guarantee.** A design in its breadboard phase, where
/// every capability lives in the one file that exists, must be told nothing.
///
/// This is `dec:maturity-restructuring-delta`'s trap stated as a test: a tool
/// that reported undeclared structure as a defect would punish exactly the
/// early-phase work that is going correctly. There is no outlier here because
/// there is nothing to be out of line with.
#[test]
fn a_uniformly_coarse_design_is_not_punished() {
    let mut g = graph();
    g.add_component("cmp:main", "main", "The system.", None)
        .expect("component");
    // Eight artifacts, each swallowing four capabilities. Coarse everywhere —
    // and therefore out of line with nothing.
    for a in 0..8 {
        seed_coarse(&mut g, &format!("art:lump{a}"), 4);
    }

    let r = report(&g);

    assert!(
        r.observations.is_empty(),
        "a uniformly coarse design must produce no finding, got {:?}",
        r.observations
            .iter()
            .map(|o| &o.artifact_id)
            .collect::<Vec<_>>()
    );
    assert!(
        r.notes.iter().any(|n| n.contains("uniformly coarse")
            || n.contains("same number")
            || n.contains("ordinary answer")),
        "the silence must be explained, not blank: {:?}",
        r.notes
    );
    // And a quiet report still says what it could not see.
    assert!(!r.not_observed_about.is_empty());
}

/// Too few artifacts to have a distribution: say so rather than computing a
/// spread over three points and calling it a fact about the design.
#[test]
fn too_small_a_population_says_so_instead_of_inventing_a_spread() {
    let mut g = graph();
    seed_ordinary(&mut g, MIN_POPULATION - 3);
    seed_coarse(&mut g, "art:big", 9);

    let r = report(&g);

    assert!(r.population < MIN_POPULATION);
    assert!(r.observations.is_empty());
    assert!(
        r.notes.iter().any(|n| n.contains("below the")),
        "notes: {:?}",
        r.notes
    );
}

/// Counts pile up at one, so the standard deviation collapses and a purely
/// distributional cutoff fires on an artifact holding *two* capabilities. True,
/// trivial, and not worth a person's attention — so `MIN_DISTINCTIONS` stops
/// it, and this test is what keeps that floor honest if someone removes it.
#[test]
fn two_capabilities_in_one_file_is_not_worth_saying() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:slightly-coarse", 2);

    let r = report(&g);

    // The distributional cutoff alone WOULD have fired here.
    let z = (2.0 - r.mean_capabilities_per_artifact) / {
        // recompute the sd the report used, from its own reported mean
        let counts: Vec<f64> = std::iter::repeat_n(1.0, 12)
            .chain(std::iter::once(2.0))
            .collect();
        let m = r.mean_capabilities_per_artifact;
        (counts.iter().map(|c| (c - m) * (c - m)).sum::<f64>() / (counts.len() as f64 - 1.0)).sqrt()
    };
    assert!(
        z >= UNUSUAL_AT,
        "the premise of this test is that z ({z:.2}) clears the distributional bar"
    );
    assert!(
        r.observations.is_empty(),
        "MIN_DISTINCTIONS ({MIN_DISTINCTIONS}) must suppress it anyway, got {:?}",
        r.observations
    );
}

/// An artifact nobody registered cannot be reported, and the report says that
/// rather than implying full coverage.
#[test]
fn unregistered_artifacts_are_out_of_scope_and_the_report_admits_it() {
    let mut g = graph();
    seed_ordinary(&mut g, 8);

    let r = report(&g);

    assert_eq!(r.population, 8);
    assert!(
        r.not_observed_about
            .iter()
            .any(|s| s.contains("nobody registered")),
        "{:?}",
        r.not_observed_about
    );
}

// ---------------------------------------------------------------------------
// The finding itself.
// ---------------------------------------------------------------------------

/// The reflow2 case, reproduced in miniature: a design that has decomposed
/// nearly everywhere, and one artifact that did not follow.
#[test]
fn one_artifact_out_of_line_with_its_own_design_is_reported() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:swallower", 10);

    let r = report(&g);

    assert_eq!(r.observations.len(), 1, "{:?}", r.observations);
    let o = &r.observations[0];
    assert_eq!(o.artifact_id, "art:swallower");
    assert_eq!(o.realizes_capabilities, 10);
    assert_eq!(o.capability_ids.len(), 10);
    assert_eq!(o.at_or_above, 1, "it stands alone in this design");
    assert!(o.unusual >= UNUSUAL_AT);
    // Explained, in the house style — a reader can disagree without re-deriving.
    assert!(
        o.reasons.iter().any(|s| s.contains("distinguishes 10")),
        "{:?}",
        o.reasons
    );
    assert!(
        o.reasons
            .iter()
            .any(|s| s.contains("did not follow the rest")),
        "{:?}",
        o.reasons
    );
    // The cutoffs travel with the answer so they can be argued with.
    assert_eq!(r.unusual_at, UNUSUAL_AT);
    assert_eq!(r.min_distinctions, MIN_DISTINCTIONS);
}

/// **The refusal, asserted.** The observation states what is, and none of what
/// to do about it — no severity, no category, no suggested fix, and none of the
/// words that would turn a fact into an accusation. `dec:report-dont-judge`.
#[test]
fn the_observation_carries_no_verdict() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:swallower", 10);

    let json = serde_json::to_string(&report(&g)).expect("serialize");

    for forbidden in [
        "severity",
        "suggested_fix",
        "monolith",
        "too big",
        "should be split",
        "violation",
        "defect",
    ] {
        assert!(
            !json.contains(forbidden),
            "the report must state a fact and refuse a verdict, but it contains {forbidden:?}"
        );
    }
    // And it must still say which side it declines to rule on.
    assert!(json.contains("That judgement is not reflow2's"));
}

/// An artifact realizing its own Component is the ordinary way to say "this
/// file is that part". Counting it would make every properly-registered
/// artifact look coarser than it is — the design discipline penalising itself,
/// which is the trap `surprises` already dodges for contracts.
#[test]
fn realizing_a_component_does_not_count_as_a_distinction() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    g.add_artifact("art:tidy", "tidy.rs", Some("code"), None)
        .expect("artifact");
    g.add_capability("cap:tidy", "Tidy", "One thing.", None)
        .expect("capability");
    g.allocate("cap:tidy", "cmp:main").expect("allocate");
    g.realizes("art:tidy", "Capability", "cap:tidy", None)
        .expect("realizes cap");
    g.realizes("art:tidy", "Component", "cmp:main", None)
        .expect("realizes component");

    let r = report(&g);

    assert!(
        r.observations.is_empty(),
        "a component realization must not inflate the count: {:?}",
        r.observations
    );
    assert_eq!(r.population, 13, "art:tidy counts once, for its capability");
}

/// Same design, byte-identical report — a reading that reorders between runs
/// cannot be diffed, and this one is meant to be watched over time.
#[test]
fn the_report_is_deterministic() {
    let mut g = graph();
    seed_ordinary(&mut g, 20);
    seed_coarse(&mut g, "art:swallower", 12);
    seed_coarse(&mut g, "art:other", 9);

    let a = serde_json::to_string(&report(&g)).expect("serialize");
    let b = serde_json::to_string(&report(&g)).expect("serialize");
    assert_eq!(a, b);

    // Most out-of-line first.
    let r = report(&g);
    assert_eq!(r.observations.len(), 2, "{:?}", r.observations);
    assert_eq!(r.observations[0].artifact_id, "art:swallower");
    assert!(r.observations[0].unusual >= r.observations[1].unusual);
}

/// **Masking, asserted rather than discovered later.** Outliers inflate the
/// standard deviation they are measured against, so several coarse artifacts
/// hide each other — the classic weakness of any z-based reading, and the
/// reason this is a prompt for a person rather than a gate. Seeded here so the
/// behaviour is a recorded property instead of a surprise in the field.
#[test]
fn several_coarse_artifacts_mask_each_other() {
    let mut alone = graph();
    seed_ordinary(&mut alone, 12);
    seed_coarse(&mut alone, "art:swallower", 8);
    let solo = report(&alone);
    assert_eq!(solo.observations.len(), 1, "one outlier is visible alone");

    let mut crowded = graph();
    seed_ordinary(&mut crowded, 12);
    for i in 0..5 {
        seed_coarse(&mut crowded, &format!("art:swallower{i}"), 8);
    }
    let many = report(&crowded);

    assert!(
        many.observations.len() < 5,
        "five equally coarse artifacts should mask one another, not all be reported: {:?}",
        many.observations
            .iter()
            .map(|o| &o.artifact_id)
            .collect::<Vec<_>>()
    );
}
