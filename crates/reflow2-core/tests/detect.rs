//! DETECT tests — deterministic gap detectors.
//!
//! The two behaviors that matter most: phase-coverage fires at project scope
//! when a whole phase is absent, and per-node traceability fires *only once that
//! phase exists* — so an early-stage graph gets one nudge, not a flood, and a
//! complete thread yields nothing.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, Dimension, GapScope, GapSource, LinkArtifactOptions};

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

// ---- BL-27 · duplicate detection that actually computes something ----------

/// 3dtictactoe's shape: two components holding an identical capability set,
/// one of them dead code. `Board` and `GameState` each maintained their own
/// grid and their own victory check; `Board` was exported and never
/// instantiated, and its victory check was subtly wrong. `detect_defects`
/// returned eight defects and none was `duplicate`, because HEAL's rule reads a
/// DUPLICATES edge somebody has to have drawn first.
fn redundant_pair() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:play", "Play", "play a game")
        .unwrap();
    for c in ["cap:board-state", "cap:victory", "cap:draw"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
        g.satisfies(c, "req:play").unwrap();
    }
    g.add_component("cmp:board-model", "Board", "holds the grid", None)
        .unwrap();
    g.add_component("cmp:game-engine", "GameState", "holds the grid", None)
        .unwrap();
    for c in ["cap:board-state", "cap:victory", "cap:draw"] {
        g.allocate(c, "cmp:board-model").unwrap();
        g.allocate(c, "cmp:game-engine").unwrap();
    }
    g
}

#[test]
fn two_components_with_the_same_capabilities_are_reported() {
    let g = redundant_pair();
    let gaps = g.detect_gaps().unwrap();
    let dup = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::PossibleDuplicate)
        .unwrap_or_else(|| panic!("no duplicate reported, got {:?}", sources(&gaps)));

    assert_eq!(dup.affected_ids, ["cmp:board-model", "cmp:game-engine"]);
    assert!((dup.severity - 0.70).abs() < f64::EPSILON);
    assert!(
        dup.evidence.contains("3 of 3"),
        "evidence must show the overlap it measured, got: {}",
        dup.evidence
    );
}

#[test]
fn a_duplicate_the_user_already_recorded_is_left_to_heal() {
    // HEAL can actually repair a confirmed pair. Asking about it here too would
    // be the DETECT/HEAL double-count three trials have complained about.
    let mut g = redundant_pair();
    g.create_edge(
        edge::DUPLICATES,
        node::COMPONENT,
        "cmp:board-model",
        node::COMPONENT,
        "cmp:game-engine",
        reflow2_core::nodes::Props::new(),
    )
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::PossibleDuplicate),
        "got {:?}",
        sources(&gaps)
    );
    // HEAL still has it, so the fact is not lost — just owned by the half that
    // can repair it.
    assert!(
        g.detect_defects()
            .unwrap()
            .iter()
            .any(|d| d.category == reflow2_core::heal::HealCategory::Duplicate)
    );
}

#[test]
fn one_shared_capability_is_not_a_duplicate() {
    // Two components both providing the single capability they have in common
    // is ordinary design. Without the two-shared floor this fires on it.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:shared", "Shared", "used in two places", None)
        .unwrap();
    g.satisfies("cap:shared", "req:a").unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.allocate("cap:shared", "cmp:a").unwrap();
    g.allocate("cap:shared", "cmp:b").unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::PossibleDuplicate),
        "got {:?}",
        sources(&gaps)
    );
}

#[test]
fn a_big_component_containing_a_small_ones_whole_set_is_not_a_duplicate() {
    // cmp:big has everything cmp:small has and three more. The intersection is
    // cmp:small's entire set, so an intersection-only rule would accuse them;
    // Jaccard (2/5 = 0.4) is what says they are different sizes of thing.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    for c in ["cap:1", "cap:2", "cap:3", "cap:4", "cap:5"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
        g.satisfies(c, "req:a").unwrap();
        g.allocate(c, "cmp:big").ok();
    }
    g.add_component("cmp:big", "Big", "does lots", None)
        .unwrap();
    g.add_component("cmp:small", "Small", "does little", None)
        .unwrap();
    for c in ["cap:1", "cap:2", "cap:3", "cap:4", "cap:5"] {
        g.allocate(c, "cmp:big").unwrap();
    }
    for c in ["cap:1", "cap:2"] {
        g.allocate(c, "cmp:small").unwrap();
    }

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::PossibleDuplicate),
        "got {:?}",
        sources(&gaps)
    );
}

#[test]
fn a_near_identical_pair_is_asked_about_but_ranks_lower() {
    // Three of four shared (Jaccard 0.75)... below the floor. Four of five
    // (0.80) is the weakest pair that fires, and it must not outrank an
    // identical one.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    for c in ["cap:1", "cap:2", "cap:3", "cap:4", "cap:5"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
        g.satisfies(c, "req:a").unwrap();
    }
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    for c in ["cap:1", "cap:2", "cap:3", "cap:4"] {
        g.allocate(c, "cmp:a").unwrap();
        g.allocate(c, "cmp:b").unwrap();
    }
    g.allocate("cap:5", "cmp:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let dup = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::PossibleDuplicate)
        .unwrap_or_else(|| panic!("got {:?}", sources(&gaps)));
    assert!((dup.severity - 0.58).abs() < f64::EPSILON);
    assert!(dup.description.contains("nearly"));
}

#[test]
fn an_unallocated_pair_of_components_is_not_a_duplicate() {
    // Two components with no capabilities each have the empty set, which is
    // trivially "identical". They must not be accused of duplicating each other.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::PossibleDuplicate),
        "got {:?}",
        sources(&gaps)
    );
}

#[test]
fn the_duplicate_gap_can_be_acknowledged_and_stays_dismissed() {
    // The reason this is a gap and not a HEAL defect: a structural heuristic
    // will sometimes be wrong, and the user needs a way to say so once.
    let mut g = redundant_pair();
    let dup = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::PossibleDuplicate)
        .unwrap();
    g.acknowledge_gap(
        &dup.id,
        &dup.affected_ids,
        "deliberately parallel implementations",
    )
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(!sources(&gaps).contains(&GapSource::PossibleDuplicate));
    assert!(
        g.reviewed_gaps()
            .unwrap()
            .iter()
            .any(|r| r.gap_id == dup.id)
    );
}

#[test]
fn the_duplicate_gap_id_does_not_depend_on_pair_order() {
    // The id hashes the affected ids, so the pair needs one identity however it
    // was walked — otherwise an acknowledgement silently stops matching.
    let g = redundant_pair();
    let once = g.detect_gaps().unwrap();
    let twice = g.detect_gaps().unwrap();
    let id_of = |v: &Vec<_>| {
        v.iter()
            .find(|x: &&reflow2_core::detect::GapCandidate| {
                x.gap_source == GapSource::PossibleDuplicate
            })
            .unwrap()
            .id
            .clone()
    };
    assert_eq!(id_of(&once), id_of(&twice));
}

// ---- BL-38 · both P3 shapes count as "built" -------------------------------

#[test]
fn an_artifact_realizing_the_component_counts_as_building_its_capabilities() {
    // The false positive that was 11 of 33 gaps on reflow2's own design:
    // "the file realizes the module" is how code is actually organised, and
    // the path art -REALIZES-> cmp <-ALLOCATED_TO- cap was present and unwalked.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:detect", "Detect gaps", "finds gaps", Some("realized"))
        .unwrap();
    g.satisfies("cap:detect", "req:a").unwrap();
    g.add_component("cmp:detect", "detect", "the module", None)
        .unwrap();
    g.allocate("cap:detect", "cmp:detect").unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:detect".into(),
        name: "detect.rs".into(),
        location: Some("src/detect.rs".into()),
        artifact_type: Some("code".into()),
        target_type: node::COMPONENT.into(),
        target_id: "cmp:detect".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:aaa".into()),
    })
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::UnrealizedCapability),
        "a capability whose owning component is built must not be reported unbuilt, got {:?}",
        sources(&gaps)
    );
}

#[test]
fn a_capability_in_an_unbuilt_component_is_still_reported() {
    // The exemption must not swallow the true case: artifacts exist elsewhere,
    // but nothing realizes this capability OR the component that owns it.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    for (cap, cmp) in [("cap:built", "cmp:built"), ("cap:paper", "cmp:paper")] {
        g.add_capability(cap, cap, "does a thing", None).unwrap();
        g.satisfies(cap, "req:a").unwrap();
        g.add_component(cmp, cmp, "a part", None).unwrap();
        g.allocate(cap, cmp).unwrap();
    }
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:built".into(),
        name: "built.rs".into(),
        location: Some("src/built.rs".into()),
        artifact_type: Some("code".into()),
        target_type: node::COMPONENT.into(),
        target_id: "cmp:built".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:bbb".into()),
    })
    .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let unrealized: Vec<&str> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnrealizedCapability)
        .flat_map(|x| x.affected_ids.iter().map(String::as_str))
        .collect();
    assert_eq!(
        unrealized,
        ["cap:paper"],
        "only the capability whose component nothing builds"
    );
}

// ---- BL-30 (S half) · a failing check is a gap, not a satisfaction ---------

/// A built, checked thread whose one verification can be flipped per test.
fn checked_thread(status: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "must work").unwrap();
    g.add_capability(
        "cap:a",
        "Charge a card",
        "charges once per key",
        Some("realized"),
    )
    .unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component("cmp:a", "A", "part", None).unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
    g.add_verification("ver:a", "charge tests", Some("test"), Some("unit"))
        .unwrap();
    g.verifies("ver:a", node::CAPABILITY, "cap:a").unwrap();
    g.set_verification_status("ver:a", status, None).unwrap();
    g
}

#[test]
fn a_failing_verification_is_surfaced_and_outranks_everything_absent() {
    // The erosion trial's headline: with status=failing, detect_gaps,
    // detect_defects and graph_report were byte-identical to the passing case.
    // The gap that asked "how will you confirm this works?" was closed by a
    // test proving it does not.
    let g = checked_thread("failing");
    let gaps = g.detect_gaps().unwrap();
    let failing = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::FailingVerification)
        .unwrap_or_else(|| panic!("a red check must be a gap, got {:?}", sources(&gaps)));

    // Anchored to the check AND what it checks — the answerer needs to know
    // what is broken, not only which test is red.
    assert_eq!(failing.affected_ids, ["cap:a", "ver:a"]);
    assert!((failing.severity - 0.8).abs() < f64::EPSILON);
    assert_eq!(
        gaps[0].gap_source,
        GapSource::FailingVerification,
        "work proven broken outranks work not started, got {:?}",
        sources(&gaps)
    );
}

#[test]
fn a_passing_verification_raises_nothing_and_a_failing_one_is_the_only_difference() {
    let pass = checked_thread("passing");
    let fail = checked_thread("failing");
    assert!(!sources(&pass.detect_gaps().unwrap()).contains(&GapSource::FailingVerification));
    // The two graphs must no longer diagnose identically.
    assert_ne!(
        sources(&pass.detect_gaps().unwrap()),
        sources(&fail.detect_gaps().unwrap()),
        "passing and failing must be distinguishable — this is the erosion trial's probe"
    );
}

#[test]
fn coverage_counts_a_check_that_passes_not_one_that_exists() {
    let pass = checked_thread("passing");
    let fail = checked_thread("failing");
    assert_eq!(
        pass.verification_coverage().unwrap().capabilities_verified,
        1
    );
    assert_eq!(
        fail.verification_coverage().unwrap().capabilities_verified,
        0,
        "a failing check must not raise coverage — counting test nodes while ignoring test results is the reflow1 failure in miniature"
    );
    // planned / skipped / blocked are "not currently confirmed", not "verified".
    for status in ["planned", "skipped", "blocked"] {
        assert_eq!(
            checked_thread(status)
                .verification_coverage()
                .unwrap()
                .capabilities_verified,
            0,
            "status={status} is not confirmation"
        );
    }
}

#[test]
fn fixing_the_build_clears_the_failing_gap() {
    // The loop the gap exists to drive: red -> fix -> green -> quiet.
    let mut g = checked_thread("failing");
    assert!(sources(&g.detect_gaps().unwrap()).contains(&GapSource::FailingVerification));
    g.set_verification_status("ver:a", "passing", None).unwrap();
    assert!(!sources(&g.detect_gaps().unwrap()).contains(&GapSource::FailingVerification));
    assert_eq!(g.verification_coverage().unwrap().capabilities_verified, 1);
}

// ---- BL-31 · a status is a claim the structure must back -------------------

#[test]
fn a_verified_claim_with_no_passing_check_is_a_contradiction() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "A", "does a", Some("verified"))
        .unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    let gaps = g.detect_gaps().unwrap();
    let hit = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::StatusContradiction)
        .unwrap_or_else(|| panic!("got {:?}", sources(&gaps)));
    assert_eq!(hit.affected_ids, ["cap:a"]);

    // A planned check does not back the claim; a passing one does.
    g.add_verification("ver:a", "checks", Some("test"), Some("unit"))
        .unwrap();
    g.verifies("ver:a", node::CAPABILITY, "cap:a").unwrap();
    assert!(
        sources(&g.detect_gaps().unwrap()).contains(&GapSource::StatusContradiction),
        "a check that has not passed proves nothing"
    );
    g.set_verification_status("ver:a", "passing", None).unwrap();
    assert!(!sources(&g.detect_gaps().unwrap()).contains(&GapSource::StatusContradiction));
}

#[test]
fn a_met_requirement_nothing_satisfies_is_caught_by_the_only_detector_that_can() {
    // `met` silences unsatisfied_requirement on purpose, so before BL-31 a
    // lying `met` was invisible to everything.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::REQUIREMENT,
        "req:met",
        Props::new()
            .set("name", "Done, allegedly")
            .set("statement", "must work")
            .set("status", "met"),
    )
    .unwrap();
    g.add_capability("cap:x", "X", "does x", None).unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !sources(&gaps).contains(&GapSource::UnsatisfiedRequirement),
        "met suppresses the absence gap — that is the design"
    );
    let hit = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::StatusContradiction)
        .unwrap_or_else(|| panic!("got {:?}", sources(&gaps)));
    assert_eq!(hit.affected_ids, ["req:met"]);

    g.satisfies("cap:x", "req:met").unwrap();
    assert!(!sources(&g.detect_gaps().unwrap()).contains(&GapSource::StatusContradiction));
}

/// The pure brownfield starting state (BL-27): structure seeded from code,
/// zero requirements — which previously reported nothing at all, because
/// `unmotivated_capability` is gated on requirements existing. One nudge,
/// not one gap per capability, and it yields the moment intent is stated.
#[test]
fn structure_with_zero_requirements_raises_one_intent_nudge() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "Adopted").unwrap();
    for i in 0..4 {
        g.add_capability(
            &format!("cap:{i}"),
            "C",
            "found in the code",
            Some("realized"),
        )
        .unwrap();
    }
    g.add_component("cmp:core", "Core", "the code", None)
        .unwrap();

    let gaps = g.detect_gaps().unwrap();
    let hits: Vec<_> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::DesignWithoutIntent)
        .collect();
    assert_eq!(hits.len(), 1, "one project-level nudge, never one per node");

    g.add_requirement("req:why", "Why it exists", "From the README, not the code.")
        .unwrap();
    assert!(
        !sources(&g.detect_gaps().unwrap()).contains(&GapSource::DesignWithoutIntent),
        "stated intent answers the nudge"
    );
}

#[test]
fn an_empty_graph_has_no_intent_to_miss() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "Empty").unwrap();
    assert!(!sources(&g.detect_gaps().unwrap()).contains(&GapSource::DesignWithoutIntent));
}

/// BL-42, from the storyflow adopt trial: a system that is entirely built,
/// modelled with deliberately coarse artifacts, must not be asked "what
/// builds this?" once per capability. The signal is the modeller's own claim
/// — a component marked `realized` asserts it exists — not a guess from
/// topology, and the number survives as `graph_report.realization`.
#[test]
fn a_component_claiming_to_be_built_is_not_asked_what_builds_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "Adopted").unwrap();
    // One modelled artifact somewhere, so the detector is switched on at all.
    g.add_component("cmp:modelled", "Modelled", "has a file", None)
        .unwrap();
    g.add_capability("cap:modelled", "M", "modelled", None)
        .unwrap();
    g.allocate("cap:modelled", "cmp:modelled").unwrap();
    g.add_artifact("art:m", "m.rs", Some("code"), Some("src/m.rs"))
        .unwrap();
    g.realizes("art:m", node::COMPONENT, "cmp:modelled", None)
        .unwrap();

    // A shipped component whose files were never modelled.
    g.create_node(
        node::COMPONENT,
        "cmp:shipped",
        Props::new()
            .set("name", "Shipped")
            .set("purpose", "in production")
            .set("status", "realized"),
    )
    .unwrap();
    g.add_capability("cap:shipped", "S", "already ships", None)
        .unwrap();
    g.allocate("cap:shipped", "cmp:shipped").unwrap();

    let unrealized: Vec<String> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|x| x.gap_source == GapSource::UnrealizedCapability)
        .flat_map(|x| x.affected_ids)
        .collect();
    assert!(
        unrealized.is_empty(),
        "a component asserting it is built states coverage, not a gap: {unrealized:?}"
    );

    let coverage = g.realization_coverage().unwrap();
    assert_eq!(coverage.capabilities, 2);
    assert_eq!(coverage.realized, 1);
    assert_eq!(
        coverage.built_but_unmodelled, 1,
        "the question is dropped but the number is kept"
    );

    // …and the moment the same component is only *planned*, the question is
    // right again and comes back.
    g.create_node(
        node::COMPONENT,
        "cmp:shipped",
        Props::new()
            .set("name", "Shipped")
            .set("purpose", "in production")
            .set("status", "planned"),
    )
    .unwrap();
    let unrealized: Vec<String> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|x| x.gap_source == GapSource::UnrealizedCapability)
        .flat_map(|x| x.affected_ids)
        .collect();
    assert_eq!(unrealized, ["cap:shipped"]);
}
