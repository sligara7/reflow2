//! Readiness gating and the derived roadmap (BL-68).
//!
//! The load-bearing property is NOT that a date comes out. It is that reflow2
//! refuses to invent the judgement half: an increment with no stated threshold
//! reports `Ungated` and never "ready", and a gate nobody has assessed makes the
//! whole answer `Indeterminate` rather than quietly dropping out of the max and
//! returning an optimistic date.
//!
//! The worked example throughout is the row's own — refuelling a satellite by
//! laser, where today's increment is achievable and the ten-year one waits on
//! power→light conversion maturing TRL 3 → 7.

use reflow2_core::nodes::{edge, node};
use reflow2_core::{
    ChangeAction, ChangeType, DesignGraph, EpochType, PropagateOptions, ReadinessForecast,
    ReadinessGate, ReadinessKind, ReadinessObservation, ReadinessVerdict,
};

/// The row's worked example, as a graph.
fn laser_refuelling() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:now", "today", EpochType::Baseline, 0)
        .unwrap();
    g.plan_epoch("epoch:2030", "2030", EpochType::Milestone, 10)
        .unwrap();
    g.plan_epoch("epoch:2035", "2035", EpochType::Milestone, 20)
        .unwrap();

    g.add_component("cmp:lasing", "high-power lasing", "Produce the beam", None)
        .unwrap();
    g.add_component(
        "cmp:conversion",
        "power to light conversion",
        "Turn spacecraft power into beam energy efficiently",
        None,
    )
    .unwrap();

    g.add_release("rel:v1-demo", "v1 demonstrator", Some("1.0"), None)
        .unwrap();
    g.add_release("rel:v2-fielded", "v2 fielded", Some("2.0"), None)
        .unwrap();

    // Measured today: lasing is mature, conversion is not.
    g.add_readiness(&ReadinessObservation {
        id: "trl:lasing",
        target_type: node::COMPONENT,
        target_id: "cmp:lasing",
        kind: ReadinessKind::Trl,
        level: 8,
        evidence: Some("Flight demonstration, 2024"),
        assessed_at: Some("2026-01-01"),
    })
    .unwrap();
    g.add_readiness(&ReadinessObservation {
        id: "trl:conversion",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 3,
        evidence: Some("Breadboard in a lab"),
        assessed_at: Some("2026-01-01"),
    })
    .unwrap();
    g
}

#[test]
fn the_worked_example_derives_a_date_and_names_its_reason() {
    let mut g = laser_refuelling();

    // The demonstrator tolerates an immature converter; the fielded one does not.
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v1-demo",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 3,
        rationale: Some("A demonstrator may fly a breadboard converter"),
    })
    .unwrap();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: Some("Fielded hardware needs a qualified converter"),
    })
    .unwrap();
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:conversion-2035",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        epoch_id: "epoch:2035",
        confidence: Some(0.4),
        statement: None,
    })
    .unwrap();

    // THE SAME TECHNOLOGY, TWO INCREMENTS, TWO ANSWERS — which is the whole
    // reason the threshold rides the edge rather than either endpoint.
    let demo = g.readiness_report("rel:v1-demo").unwrap();
    assert_eq!(demo.verdict, ReadinessVerdict::AchievableNow);

    let fielded = g.readiness_report("rel:v2-fielded").unwrap();
    match &fielded.verdict {
        ReadinessVerdict::GatedUntil {
            epoch_id, sequence, ..
        } => {
            assert_eq!(epoch_id, "epoch:2035");
            assert_eq!(*sequence, 20);
        }
        other => panic!("expected a derived date, got {other:?}"),
    }
    // The legibility the row says real programs lacked: the answer says WHY.
    assert_eq!(
        fielded.deciding_target_id.as_deref(),
        Some("cmp:conversion")
    );
    assert!(fielded.summary.contains("2035"), "{}", fielded.summary);
    assert!(
        fielded.summary.contains("TRL 3 today"),
        "{}",
        fielded.summary
    );
    assert!(
        fielded.summary.contains("needs TRL 7"),
        "{}",
        fielded.summary
    );
    // Author-stated confidence travels with the projection.
    let gate = &fielded.gates[0];
    assert_eq!(gate.confidence, Some(0.4));
}

#[test]
fn the_refusal_an_increment_with_no_threshold_is_ungated_never_ready() {
    // THE COUNTERWEIGHT THE WHOLE FEATURE RESTS ON
    // (dec:readiness-is-an-observation-the-threshold-is-the-judgement).
    // Silence about a gate is not evidence there is none.
    let g = laser_refuelling();
    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(r.verdict, ReadinessVerdict::Ungated);
    assert!(r.gates.is_empty());
    assert!(
        r.summary.contains("UNGATED") && r.summary.contains("never as ready"),
        "the refusal must be stated, not implied: {}",
        r.summary
    );
    // And it must not be mistakable for the achievable verdict.
    assert_ne!(r.verdict, ReadinessVerdict::AchievableNow);
}

#[test]
fn an_unassessed_gate_is_indeterminate_not_optimistic() {
    // THE OTHER HALF OF THE REFUSAL. Dropping this gate from the max would
    // return a date built by ignoring the inconvenient half of the evidence.
    let mut g = laser_refuelling();
    g.add_component(
        "cmp:thermal",
        "thermal management",
        "Reject waste heat",
        None,
    )
    .unwrap();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:lasing",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();
    // Gated on something nobody has ever assessed.
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:thermal",
        kind: ReadinessKind::Trl,
        min_level: 6,
        rationale: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(r.verdict, ReadinessVerdict::Indeterminate);
    assert_eq!(r.deciding_target_id.as_deref(), Some("cmp:thermal"));
    assert!(
        r.summary.contains("no TRL at all"),
        "must name what is missing: {}",
        r.summary
    );
}

#[test]
fn a_gate_no_forecast_ever_clears_is_indeterminate() {
    let mut g = laser_refuelling();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 9,
        rationale: None,
    })
    .unwrap();
    // A forecast that reaches 7 — not the 9 demanded.
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:conversion-2035",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        epoch_id: "epoch:2035",
        confidence: None,
        statement: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(r.verdict, ReadinessVerdict::Indeterminate);
    assert!(
        r.summary.contains("no forecast on record ever reaches"),
        "{}",
        r.summary
    );
}

#[test]
fn the_slowest_gate_decides_and_is_the_one_named() {
    // An increment waits for its slowest dependency, so the answer is the MAX
    // over per-gate clearing epochs — not the first, not the average.
    let mut g = laser_refuelling();
    for (release, target, level) in [
        ("rel:v2-fielded", "cmp:lasing", 9),
        ("rel:v2-fielded", "cmp:conversion", 7),
    ] {
        g.gate_on(&ReadinessGate {
            subject_type: node::RELEASE,
            subject_id: release,
            target_type: node::COMPONENT,
            target_id: target,
            kind: ReadinessKind::Trl,
            min_level: level,
            rationale: None,
        })
        .unwrap();
    }
    // Lasing clears early; conversion clears late.
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:lasing-2030",
        target_type: node::COMPONENT,
        target_id: "cmp:lasing",
        kind: ReadinessKind::Trl,
        level: 9,
        epoch_id: "epoch:2030",
        confidence: None,
        statement: None,
    })
    .unwrap();
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:conversion-2035",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        epoch_id: "epoch:2035",
        confidence: None,
        statement: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    match &r.verdict {
        ReadinessVerdict::GatedUntil { epoch_id, .. } => assert_eq!(epoch_id, "epoch:2035"),
        other => panic!("expected the LATER epoch, got {other:?}"),
    }
    assert_eq!(r.deciding_target_id.as_deref(), Some("cmp:conversion"));
}

#[test]
fn the_earliest_clearing_forecast_wins_within_one_gate() {
    // Across gates the LATEST decides; within one gate the EARLIEST does — the
    // first moment that technology is good enough.
    let mut g = laser_refuelling();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:c-2035",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 8,
        epoch_id: "epoch:2035",
        confidence: None,
        statement: None,
    })
    .unwrap();
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:c-2030",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        epoch_id: "epoch:2030",
        confidence: None,
        statement: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    match &r.verdict {
        ReadinessVerdict::GatedUntil { epoch_id, .. } => assert_eq!(epoch_id, "epoch:2030"),
        other => panic!("expected the EARLIER clearing epoch, got {other:?}"),
    }
}

#[test]
fn trl_and_mrl_are_not_interchangeable() {
    // A technology can be demonstrable and unmanufacturable. If one ladder
    // satisfied the other, a roadmap could not state that case at all.
    let mut g = laser_refuelling();
    g.add_readiness(&ReadinessObservation {
        id: "mrl:conversion",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Mrl,
        level: 9,
        evidence: None,
        assessed_at: None,
    })
    .unwrap();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(
        r.verdict,
        ReadinessVerdict::Indeterminate,
        "an MRL 9 must not satisfy a TRL 7 gate"
    );
    assert_eq!(r.gates[0].current_level, Some(3), "the TRL, not the MRL");
}

#[test]
fn a_forecast_on_the_other_ladder_does_not_clear_the_gate() {
    // THE SIBLING OF THE TEST ABOVE, AND IT WAS MISSING. The mutation check
    // found it: dropping the ladder filter from the FORECAST path changed
    // nothing, because every existing case only exercised the MEASURED path.
    // An MRL projection silently clearing a TRL gate would have shipped a date
    // built on the wrong evidence entirely — and a roadmap is the last place
    // that should pass unnoticed.
    let mut g = laser_refuelling();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();
    // Manufacturing matures; the technology itself does not.
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:mrl-2030",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Mrl,
        level: 9,
        epoch_id: "epoch:2030",
        confidence: None,
        statement: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(
        r.verdict,
        ReadinessVerdict::Indeterminate,
        "an MRL 9 forecast must not clear a TRL 7 gate"
    );
}

#[test]
fn a_measured_temporal_fact_is_not_a_forecast() {
    // `basis` is the whole reason the property exists: a record of the past is
    // not evidence about an epoch that has not happened.
    let mut g = laser_refuelling();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();
    // Hand-build a readiness fact that claims TRL 7 but is MEASURED, not projected.
    g.create_node(
        node::TEMPORAL_FACT,
        "tf:measured",
        reflow2_core::nodes::Props::new()
            .set("subject_id", "cmp:conversion")
            .set("fact_type", reflow2_core::READINESS_FACT)
            .set("statement", "was TRL 7 at some point")
            .set("value", r#"{"kind":"TRL","level":7}"#)
            .set("basis", "measured"),
    )
    .unwrap();
    g.create_edge(
        edge::HAS_TEMPORAL_FACT,
        node::COMPONENT,
        "cmp:conversion",
        node::TEMPORAL_FACT,
        "tf:measured",
        reflow2_core::nodes::Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::VALID_FROM,
        node::TEMPORAL_FACT,
        "tf:measured",
        node::DESIGN_EPOCH,
        "epoch:2030",
        reflow2_core::nodes::Props::new(),
    )
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(
        r.verdict,
        ReadinessVerdict::Indeterminate,
        "a measured fact must not be read as a projection"
    );
}

#[test]
fn the_highest_measured_level_wins_not_the_latest() {
    // Readiness is ratcheted evidence: a demonstration at TRL 8 is not undone by
    // someone later backfilling an observation of an earlier stage.
    let mut g = laser_refuelling();
    g.add_readiness(&ReadinessObservation {
        id: "trl:conversion-late-backfill",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        evidence: Some("Qualification campaign"),
        assessed_at: Some("2026-06-01"),
    })
    .unwrap();
    g.add_readiness(&ReadinessObservation {
        id: "trl:conversion-older-stage",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 4,
        evidence: Some("An earlier stage, recorded afterwards"),
        assessed_at: Some("2026-12-01"),
    })
    .unwrap();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();

    let r = g.readiness_report("rel:v2-fielded").unwrap();
    assert_eq!(r.verdict, ReadinessVerdict::AchievableNow);
    assert_eq!(r.gates[0].current_level, Some(7));
}

#[test]
fn a_rung_outside_one_to_nine_is_refused_at_both_ends() {
    let mut g = laser_refuelling();
    for bad in [0, 10, -1, 99] {
        assert!(
            g.add_readiness(&ReadinessObservation {
                id: &format!("trl:bad{bad}"),
                target_type: node::COMPONENT,
                target_id: "cmp:lasing",
                kind: ReadinessKind::Trl,
                level: bad,
                evidence: None,
                assessed_at: None,
            })
            .is_err(),
            "level {bad} must be refused, never clamped"
        );
        assert!(
            g.gate_on(&ReadinessGate {
                subject_type: node::RELEASE,
                subject_id: "rel:v2-fielded",
                target_type: node::COMPONENT,
                target_id: "cmp:lasing",
                kind: ReadinessKind::Trl,
                min_level: bad,
                rationale: None,
            })
            .is_err(),
            "threshold {bad} must be refused, never clamped"
        );
        assert!(
            g.forecast_readiness(&ReadinessForecast {
                id: &format!("fc:bad{bad}"),
                target_type: node::COMPONENT,
                target_id: "cmp:lasing",
                kind: ReadinessKind::Trl,
                level: bad,
                epoch_id: "epoch:2030",
                confidence: None,
                statement: None,
            })
            .is_err(),
            "forecast level {bad} must be refused"
        );
    }
    // The valid ends are accepted, so the bound is not off by one.
    assert!(
        g.add_readiness(&ReadinessObservation {
            id: "trl:one",
            target_type: node::COMPONENT,
            target_id: "cmp:lasing",
            kind: ReadinessKind::Trl,
            level: 1,
            evidence: None,
            assessed_at: None,
        })
        .is_ok()
    );
    assert!(
        g.add_readiness(&ReadinessObservation {
            id: "trl:nine",
            target_type: node::COMPONENT,
            target_id: "cmp:lasing",
            kind: ReadinessKind::Mrl,
            level: 9,
            evidence: None,
            assessed_at: None,
        })
        .is_ok()
    );
}

#[test]
fn readiness_against_a_missing_target_is_refused() {
    let mut g = laser_refuelling();
    assert!(
        g.add_readiness(&ReadinessObservation {
            id: "trl:ghost",
            target_type: node::COMPONENT,
            target_id: "cmp:does-not-exist",
            kind: ReadinessKind::Trl,
            level: 5,
            evidence: None,
            assessed_at: None,
        })
        .is_err(),
        "a level about nothing is a dangling claim"
    );
    assert!(
        g.forecast_readiness(&ReadinessForecast {
            id: "fc:ghost-epoch",
            target_type: node::COMPONENT,
            target_id: "cmp:lasing",
            kind: ReadinessKind::Trl,
            level: 5,
            epoch_id: "epoch:never-planned",
            confidence: None,
            statement: None,
        })
        .is_err(),
        "a forecast valid from a non-existent epoch is unanswerable"
    );
}

#[test]
fn gated_on_is_a_traceability_edge_so_slipping_readiness_reaches_the_roadmap() {
    // THE POINT OF PUTTING GATED_ON IN THE STRUCTURAL TABLE. Twice before, a new
    // edge type reached a Release and every traversal stepped over it. Here the
    // failure would be worse than an island: a technology could slip from TRL 7
    // to 3 and propagate_change would report nothing downstream, leaving the
    // roadmap silently stale.
    let mut g = laser_refuelling();
    g.gate_on(&ReadinessGate {
        subject_type: node::RELEASE,
        subject_id: "rel:v2-fielded",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();

    g.add_change_event(
        "chg:conversion-slipped",
        "The converter slipped",
        ChangeType::ScopeChange,
        None,
        None,
        None,
    )
    .unwrap();
    g.changed(
        "chg:conversion-slipped",
        node::COMPONENT,
        "cmp:conversion",
        ChangeAction::Modified,
    )
    .unwrap();

    let radius = g
        .propagate_change("chg:conversion-slipped", PropagateOptions::default())
        .unwrap();
    assert!(
        radius
            .impacted
            .iter()
            .any(|n| n.node_id == "rel:v2-fielded"),
        "the increment gated on a slipped technology must be in its blast radius"
    );
}

#[test]
fn the_report_is_deterministic() {
    let mut g = laser_refuelling();
    for (target, level) in [("cmp:lasing", 9), ("cmp:conversion", 7)] {
        g.gate_on(&ReadinessGate {
            subject_type: node::RELEASE,
            subject_id: "rel:v2-fielded",
            target_type: node::COMPONENT,
            target_id: target,
            kind: ReadinessKind::Trl,
            min_level: level,
            rationale: None,
        })
        .unwrap();
        g.forecast_readiness(&ReadinessForecast {
            id: &format!("fc:{target}"),
            target_type: node::COMPONENT,
            target_id: target,
            kind: ReadinessKind::Trl,
            level: 9,
            epoch_id: "epoch:2035",
            confidence: None,
            statement: None,
        })
        .unwrap();
    }
    let a = serde_json::to_string(&g.readiness_report("rel:v2-fielded").unwrap()).unwrap();
    let b = serde_json::to_string(&g.readiness_report("rel:v2-fielded").unwrap()).unwrap();
    assert_eq!(a, b, "repeated reports must be byte-identical");
}

#[test]
fn a_capability_can_be_gated_too_not_only_a_release() {
    // "Increment" is not a synonym for Release: a Capability is gated by the
    // same machinery, which is what lets the gate sit where the intent is.
    let mut g = laser_refuelling();
    g.add_capability(
        "cap:refuel",
        "Refuel by laser",
        "Transfer energy in orbit",
        None,
    )
    .unwrap();
    g.gate_on(&ReadinessGate {
        subject_type: node::CAPABILITY,
        subject_id: "cap:refuel",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        min_level: 7,
        rationale: None,
    })
    .unwrap();
    g.forecast_readiness(&ReadinessForecast {
        id: "fc:conversion-2035",
        target_type: node::COMPONENT,
        target_id: "cmp:conversion",
        kind: ReadinessKind::Trl,
        level: 7,
        epoch_id: "epoch:2035",
        confidence: None,
        statement: None,
    })
    .unwrap();

    let r = g.readiness_report("cap:refuel").unwrap();
    match &r.verdict {
        ReadinessVerdict::GatedUntil { epoch_id, .. } => assert_eq!(epoch_id, "epoch:2035"),
        other => panic!("expected a derived date for a gated capability, got {other:?}"),
    }
}
