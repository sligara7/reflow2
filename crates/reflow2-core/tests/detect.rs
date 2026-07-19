//! DETECT tests — deterministic gap detectors.
//!
//! The two behaviors that matter most: phase-coverage fires at project scope
//! when a whole phase is absent, and per-node traceability fires *only once that
//! phase exists* — so an early-stage graph gets one nudge, not a flood, and a
//! complete thread yields nothing.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, Dimension, GapScope, GapSource};

fn sources(gaps: &[reflow2_core::GapCandidate]) -> Vec<GapSource> {
    gaps.iter().map(|g| g.gap_source).collect()
}

#[test]
fn early_graph_gets_project_level_phase_nudges_not_per_node_floods() {
    // Only concept exists: one requirement + one capability, nothing downstream.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let srcs = sources(&gaps);

    // The design phase is absent → one project-level nudge.
    assert!(srcs.contains(&GapSource::ConceptWithoutDesign));
    // Downstream phase-coverage nudges also fire (no verifications either).
    assert!(srcs.contains(&GapSource::BuildWithoutVerification));

    // Crucially: NO per-node traceability gaps, because those phases don't exist
    // yet (no components → unallocated is not asked; no artifacts → unrealized
    // is not asked).
    assert!(!srcs.contains(&GapSource::UnallocatedCapability));
    assert!(!srcs.contains(&GapSource::UnrealizedCapability));
    assert!(!srcs.contains(&GapSource::UnsatisfiedRequirement)); // req:a IS satisfied

    // Phase-coverage gaps are project/phase scoped.
    for gap in &gaps {
        assert_eq!(gap.scope, GapScope::Phase);
    }
}

#[test]
fn traceability_fires_per_node_once_the_phase_exists() {
    // Components exist, so allocation is now expected. cap:b is unallocated.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A", None).unwrap();
    g.add_capability("cap:b", "Cap B", "Does B", None).unwrap();
    g.add_component("cmp:x", "X", "Part X", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.satisfies("cap:b", "req:a").unwrap();
    g.allocate("cap:a", "cmp:x").unwrap(); // cap:a allocated, cap:b not

    let gaps = g.detect_gaps().unwrap();
    let unallocated: Vec<&str> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnallocatedCapability)
        .flat_map(|x| x.affected_ids.iter().map(String::as_str))
        .collect();

    // Exactly cap:b — cap:a is allocated, so it is not flagged.
    assert_eq!(unallocated, ["cap:b"]);
    // The design phase now exists, so concept_without_design no longer fires.
    assert!(!sources(&gaps).contains(&GapSource::ConceptWithoutDesign));
}

#[test]
fn unsatisfied_requirement_ranks_by_priority() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Two unsatisfied requirements at different priorities; capabilities exist
    // (so the detector is active) but satisfy neither.
    g.create_node(
        node::REQUIREMENT,
        "req:crit",
        Props::new()
            .set("name", "Critical need")
            .set("statement", "must")
            .set("priority", "critical"),
    )
    .unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:low",
        Props::new()
            .set("name", "Nice to have")
            .set("statement", "maybe")
            .set("priority", "low"),
    )
    .unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();
    g.add_component("cmp:y", "Y", "part y", None).unwrap();
    g.allocate("cap:x", "cmp:y").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let unsat: Vec<&reflow2_core::GapCandidate> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnsatisfiedRequirement)
        .collect();
    assert_eq!(unsat.len(), 2);
    // Critical outranks low in severity, so it sorts first overall among these.
    let crit = unsat
        .iter()
        .find(|x| x.affected_ids == ["req:crit"])
        .unwrap();
    let low = unsat
        .iter()
        .find(|x| x.affected_ids == ["req:low"])
        .unwrap();
    assert!(crit.severity > low.severity);
}

#[test]
fn dropped_or_met_requirements_are_not_flagged() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:dropped",
        Props::new()
            .set("name", "Abandoned")
            .set("statement", "no")
            .set("status", "dropped"),
    )
    .unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(!sources(&gaps).contains(&GapSource::UnsatisfiedRequirement));
}

#[test]
fn complete_thread_yields_no_traceability_gaps() {
    // A full concept→operate thread: every golden-thread link present.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:a",
        Props::new()
            .set("name", "A")
            .set("statement", "need")
            .set("status", "accepted"),
    )
    .unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
    g.create_node(node::ARTIFACT, "art:a", Props::new().set("name", "a.rs"))
        .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:a",
        Props::new().set("name", "test a"),
    )
    .unwrap();
    g.create_node(node::RELEASE, "rel:a", Props::new().set("name", "v1.0"))
        .unwrap();

    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();
    // Verify both the capability and the artifact (each needs its own).
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();
    g.create_node(
        node::VERIFICATION,
        "ver:b",
        Props::new().set("name", "test a2"),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:b",
        node::ARTIFACT,
        "art:a",
        Props::new(),
    )
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let srcs = sources(&gaps);
    // No traceability gaps at all.
    assert!(!srcs.contains(&GapSource::UnsatisfiedRequirement));
    assert!(!srcs.contains(&GapSource::UnallocatedCapability));
    assert!(!srcs.contains(&GapSource::UnrealizedCapability));
    assert!(!srcs.contains(&GapSource::UnverifiedCapability));
    // And no phase-coverage gaps except deploy/operate (we added a Release, so
    // even that is covered) — expect an empty gap set.
    assert!(gaps.is_empty(), "unexpected gaps: {:?}", srcs);
}

#[test]
fn gap_ids_are_deterministic_across_runs() {
    let build = || {
        let mut g = DesignGraph::open_in_memory().unwrap();
        g.add_requirement("req:a", "A", "need").unwrap();
        g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
        g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
        // cap:a unallocated on purpose.
        g.detect_gaps().unwrap()
    };
    let first = build();
    let second = build();
    let ids1: Vec<&str> = first.iter().map(|g| g.id.as_str()).collect();
    let ids2: Vec<&str> = second.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids1, ids2, "same graph state must yield identical gap ids");
    assert!(first.iter().all(|g| g.id.starts_with("gap:")));
}

#[test]
fn a_cross_community_coupling_is_a_signal_not_a_gap() {
    // Two tightly-coupled triangles joined by one lateral bridge. The bridge is
    // a real finding — but a signal to report, not a question to answer, so it
    // belongs in graph_report and not in the gap list (BL-6b).
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a1", "cap:a2", "cap:a3", "cap:b1", "cap:b2", "cap:b3"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
    }
    let dep = |g: &mut DesignGraph, from: &str, to: &str, w: f64| {
        g.create_edge(
            edge::DEPENDS_ON,
            node::CAPABILITY,
            from,
            node::CAPABILITY,
            to,
            Props::new().set("weight", w),
        )
        .unwrap();
    };
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:a2", "cap:a3", 0.9);
    dep(&mut g, "cap:a1", "cap:a3", 0.9);
    dep(&mut g, "cap:b1", "cap:b2", 0.9);
    dep(&mut g, "cap:b2", "cap:b3", 0.9);
    dep(&mut g, "cap:b1", "cap:b3", 0.9);
    dep(&mut g, "cap:a1", "cap:b1", 0.1); // the bridge

    // Not a gap: it fires on correct architecture. Both blind trials reported
    // it doing so, and an Interface bridges two clusters by construction — so
    // modelling contracts as the docs instruct made the detector penalise every
    // one of them.
    assert!(
        !g.detect_gaps()
            .unwrap()
            .iter()
            .any(|x| x.gap_source == GapSource::UnexpectedCoupling),
        "a cross-community coupling must not demand an answer"
    );

    // Still reported, in full, where it informs instead of interrupting.
    let report = g.graph_report().unwrap();
    assert_eq!(report.surprising.len(), 1, "the signal itself must survive");
    assert_eq!(report.surprising[0].from_id, "cap:a1");
    assert_eq!(report.surprising[0].to_id, "cap:b1");
}

#[test]
fn a_declining_dimension_is_surfaced_as_a_gap_but_an_improving_one_is_not() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "X", "part", None).unwrap();
    g.add_component("cmp:y", "Y", "part", None).unwrap();
    // cmp:x maintainability sliding; cmp:y reliability improving.
    g.add_dimension_observation(
        "o1",
        node::COMPONENT,
        "cmp:x",
        Dimension::Maintainability,
        0.9,
        "e01",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "o2",
        node::COMPONENT,
        "cmp:x",
        Dimension::Maintainability,
        0.5,
        "e02",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "r1",
        node::COMPONENT,
        "cmp:y",
        Dimension::Reliability,
        0.4,
        "e01",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "r2",
        node::COMPONENT,
        "cmp:y",
        Dimension::Reliability,
        0.9,
        "e02",
        None,
    )
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let declining: Vec<&reflow2_core::GapCandidate> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::DecliningDimension)
        .collect();
    assert_eq!(declining.len(), 1, "only the declining dimension is a gap");
    assert_eq!(declining[0].affected_ids, ["cmp:x"]);
    assert!(declining[0].title.contains("maintainability"));
}

// ---- BL-27 · ranking: "broken now" outranks "what comes next" --------------

#[test]
fn a_named_gap_outranks_a_phase_nudge_that_scores_higher() {
    // The brownfield shape, reproduced at fixture scale. GENESIS seeds P0/P1
    // and stops, so `concept_without_design` fires at its literal 0.70 while
    // `unsatisfied_requirement` computes 0.5 + 0.10 (default `medium`) = 0.60.
    // Ordering on severity alone put the artifact of seeding order on top and
    // the actionable finding below it — three trials reported that, and the
    // cost is that an agent working the list top-down does the useless thing
    // first.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:orphan", "Track authorization", "someone must sign off")
        .unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();

    let gaps = g.detect_gaps().unwrap();
    let first = &gaps[0];

    assert_eq!(
        first.gap_source,
        GapSource::UnsatisfiedRequirement,
        "the anchored gap must come first, got {:?}",
        sources(&gaps)
    );
    assert!(
        first.severity < gaps.last().unwrap().severity,
        "and it must win despite scoring lower — that is the whole point"
    );
    assert!(
        sources(&gaps).contains(&GapSource::ConceptWithoutDesign),
        "the phase nudge is demoted, never suppressed"
    );
}

#[test]
fn the_phase_nudge_still_leads_when_nothing_specific_is_wrong() {
    // The greenfield day-one case the aidrone trial recorded as working:
    // GENESIS seeds P0/P1, every requirement is satisfied, and the productive
    // first question really is "how should this be structured?". Demoting the
    // nudge must not cost us that.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert_eq!(
        gaps[0].gap_source,
        GapSource::ConceptWithoutDesign,
        "with nothing anchored to report, the nudge is still the first thing asked, got {:?}",
        sources(&gaps)
    );
}

#[test]
fn ranking_is_stable_across_runs() {
    // Gap ids are deterministic and the sort must be too, or a session-to-session
    // diff of the list is noise.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:x", "X", "need x").unwrap();
    g.add_requirement("req:y", "Y", "need y").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();

    let once = sources(&g.detect_gaps().unwrap());
    let twice = sources(&g.detect_gaps().unwrap());
    assert_eq!(once, twice);
}

// ---- BL-27 · the direction DETECT was blind in -----------------------------

#[test]
fn a_capability_nothing_asked_for_is_reported() {
    // 3dtictactoe's probe, verbatim in shape: the code detects draws but no
    // requirement in description.txt ever asks for it. Four gaps came back and
    // none was about the orphan. Ophyd ran the same probe on a service graph
    // (cap:qserver-auth, no SATISFIES) and got 13 unsatisfied_requirement gaps
    // and silence about the dangling capability.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:play", "Play a game", "two players take turns")
        .unwrap();
    g.add_capability("cap:turns", "Turn taking", "alternates players", None)
        .unwrap();
    g.add_capability("cap:draw", "Draw detection", "spots a full board", None)
        .unwrap();
    g.satisfies("cap:turns", "req:play").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let orphans: Vec<&str> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnmotivatedCapability)
        .flat_map(|x| x.affected_ids.iter().map(String::as_str))
        .collect();

    // Exactly cap:draw — cap:turns satisfies something, so it is not flagged.
    assert_eq!(orphans, ["cap:draw"]);
}

#[test]
fn an_inferred_orphan_outranks_an_unsatisfied_requirement() {
    // Ophyd asked for this to outrank unsatisfied_requirement "on a brownfield
    // graph". A capability read out of running code that no requirement
    // justifies is a feature in production nobody asked for; that is the
    // highest-value thing an adoption pass surfaces, so it leads the list.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "Stated need", "someone asked for this")
        .unwrap();
    g.add_capability(
        "cap:ghost",
        "Undocumented auth",
        "authorises requests",
        None,
    )
    .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:ghost", "inferred")
        .unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert_eq!(
        gaps[0].gap_source,
        GapSource::UnmotivatedCapability,
        "got {:?}",
        sources(&gaps)
    );
    assert!((gaps[0].severity - 0.70).abs() < f64::EPSILON);
}

#[test]
fn an_authored_orphan_ranks_below_the_requirement_gaps() {
    // The greenfield reading of the same structure. A capability someone wrote
    // down that satisfies nothing is a half-finished thought, not a discovery,
    // and must not push the requirement gaps down the list.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "Stated need", "someone asked for this")
        .unwrap();
    g.add_capability("cap:half", "Half a thought", "does something", None)
        .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let orphan = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::UnmotivatedCapability)
        .expect("still reported, just not first");
    let unsat = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::UnsatisfiedRequirement)
        .expect("req:a is satisfied by nothing");

    assert!((orphan.severity - 0.55).abs() < f64::EPSILON);
    assert!(
        orphan.severity < unsat.severity,
        "an authored orphan must not outrank a real requirement gap"
    );
}

#[test]
fn no_orphan_capability_gaps_before_any_requirement_exists() {
    // A graph seeded from code with no intent captured yet would otherwise emit
    // one gap per capability — the per-node flood the project-level nudges exist
    // to replace. The missing-intent case is a phase gap nothing reports yet.
    let mut g = DesignGraph::open_in_memory().unwrap();
    for c in ["cap:a", "cap:b", "cap:c"] {
        g.add_capability(c, c, "read out of the code", None)
            .unwrap();
    }

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::UnmotivatedCapability),
        "got {:?}",
        sources(&gaps)
    );
}

#[test]
fn a_complete_thread_reports_no_orphan_capability() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(!sources(&gaps).contains(&GapSource::UnmotivatedCapability));
}
