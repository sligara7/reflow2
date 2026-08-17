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
        Props::new().set("name", "test a").set("status", "passing"),
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
        Props::new().set("name", "test a2").set("status", "passing"),
    )
    .unwrap();
    // Validation as well as verification: a "complete thread" is complete on
    // both axes, and a capability with only a passing verification-kind check
    // correctly raises `unvalidated_capability`.
    g.create_node(
        node::VERIFICATION,
        "ver:a-validation",
        Props::new()
            .set("name", "accepted by the user")
            .set("status", "passing")
            .set("kind", "validation"),
    )
    .unwrap();
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:a-validation",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
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

    // A complete thread has said what it is built to. Added when
    // `build_without_governance` landed and fired here: real artifacts exist, so
    // conventions exist, and recording none of them is a genuine hole rather
    // than fixture noise. ADVISORY on purpose — it answers the absence question
    // without owing a detector, so this stays an empty gap set and pins the
    // interaction between the two new rules in one place.
    g.create_node(
        node::DESIGN_RULE,
        "rule:house-style",
        Props::new()
            .set("name", "One capability per module")
            .set("statement", "A module holds exactly one capability's code.")
            .set("enforced", false),
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
        // `asserted` is the point of this test: the USER recorded it, which is
        // what makes it HEAL's to repair. A `suspected` edge would correctly
        // come back here as a question instead.
        reflow2_core::nodes::Props::new().set("basis", "asserted"),
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
        g.open_defects()
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
        conformance: None,
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
        conformance: None,
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
fn a_failing_gap_says_when_the_check_last_ran() {
    // A `status` is a measurement taken at an instant. Reporting it without its
    // timestamp presents it as a standing property of the system — and that cost
    // a real fleet twice in one shift, in both directions: a verification read
    // `passing` while the service was 100% dead, and these gaps read `failing`
    // for 26 capabilities on a run three days older than the fixes.
    let mut g = checked_thread("failing");
    g.set_verification_status("ver:a", "failing", Some("2026-07-25T18:49:11Z"))
        .unwrap();
    let gaps = g.detect_gaps().unwrap();
    let failing = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::FailingVerification)
        .expect("a red check must still be a gap");

    // The timestamp must reach the reader through the surfaces they actually
    // read — the title and the evidence — not merely exist on the node.
    assert!(
        failing.title.contains("2026-07-25T18:49:11Z"),
        "the title must carry when it last ran: {}",
        failing.title
    );
    assert!(
        failing.evidence.contains("2026-07-25T18:49:11Z"),
        "the evidence must carry the run time: {}",
        failing.evidence
    );
    // And it must stop claiming the present tense on the strength of a past run.
    assert!(
        failing.description.contains("Re-run"),
        "the description must tell the reader to re-run before treating it as current: {}",
        failing.description
    );
}

#[test]
fn a_failing_check_that_never_ran_says_so_rather_than_implying_a_measurement() {
    // `failing` with no recorded run is an ASSERTION, not a measurement, and the
    // wording must not launder one into the other. This is the same shape as the
    // paper-green that started the whole class: a status set from a transcript.
    let g = checked_thread("failing"); // fixture sets status with last_run_at = None
    let gaps = g.detect_gaps().unwrap();
    let failing = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::FailingVerification)
        .expect("a red check must still be a gap");
    assert!(
        failing.title.contains("never run"),
        "an unrun check must say so in the title: {}",
        failing.title
    );
    assert!(
        failing.evidence.contains("no last_run_at"),
        "the evidence must name the absence: {}",
        failing.evidence
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
    g.realizes("art:m", node::COMPONENT, "cmp:modelled", None, None)
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

#[test]
fn a_verified_capability_with_no_validation_is_flagged_then_cleared() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();

    // A passing verification-kind check: built to spec.
    g.add_verification("ver:spec", "spec test", Some("test"), Some("unit"))
        .unwrap();
    g.verifies("ver:spec", "Capability", "cap:a").unwrap();
    g.set_verification_status("ver:spec", "passing", None)
        .unwrap();

    // Verified against spec, nothing validates the intent → flagged.
    assert!(
        sources(&g.detect_gaps().unwrap()).contains(&GapSource::UnvalidatedCapability),
        "built right, but is it the right thing?"
    );

    // Add a passing validation-kind check → the gap clears.
    g.add_verification(
        "ver:val",
        "field validation",
        Some("review"),
        Some("acceptance"),
    )
    .unwrap();
    g.verifies("ver:val", "Capability", "cap:a").unwrap();
    g.set_verification_status("ver:val", "passing", None)
        .unwrap();
    g.set_verification_kind("ver:val", "validation").unwrap();

    assert!(
        !sources(&g.detect_gaps().unwrap()).contains(&GapSource::UnvalidatedCapability),
        "now validated"
    );
}

#[test]
fn a_bad_verification_kind_is_refused() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_verification("ver:x", "x", None, None).unwrap();
    assert!(g.set_verification_kind("ver:x", "vindication").is_err());
}

// ---- BL-122: a Release that names no point on the time axis ---------------
//
// The defect this closes was invisible by construction: `rel:v0190` was cut
// without its `AT_EPOCH` edge four hours before `changelog_view` needed it, and
// nothing anywhere reported the absence. A matching name plus an existing epoch
// node make a missing edge look exactly like a present one.

/// Build a graph with an epoch spine and `count` releases, pinning only those
/// whose index appears in `pinned`.
fn releases_and_epochs(count: usize, pinned: &[usize]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    for i in 0..count {
        g.create_node(
            node::DESIGN_EPOCH,
            &format!("epoch:v{i}"),
            Props::new()
                .set("name", format!("v0.{i}.0 cut"))
                .set("sequence", i as i64),
        )
        .unwrap();
        g.create_node(
            node::RELEASE,
            &format!("rel:v{i}"),
            // Explicit `deployed`: Release.status DEFAULTS to `planned`, and a
            // planned release is deliberately exempt, so a helper that omitted
            // it would silently test nothing.
            Props::new()
                .set("name", format!("v0.{i}.0"))
                .set("status", "deployed"),
        )
        .unwrap();
        if pinned.contains(&i) {
            g.create_edge(
                edge::AT_EPOCH,
                node::RELEASE,
                &format!("rel:v{i}"),
                node::DESIGN_EPOCH,
                &format!("epoch:v{i}"),
                Props::new(),
            )
            .unwrap();
        }
    }
    g
}

fn epochless(gaps: &[reflow2_core::GapCandidate]) -> Vec<&reflow2_core::GapCandidate> {
    gaps.iter()
        .filter(|g| g.gap_source == GapSource::ReleaseWithoutEpoch)
        .collect()
}

#[test]
fn a_release_with_no_epoch_is_reported() {
    // Two releases, only the first pinned — exactly the v0.17.0 shape.
    let g = releases_and_epochs(2, &[0]);
    let gaps = g.detect_gaps().unwrap();
    let found = epochless(&gaps);

    assert_eq!(found.len(), 1, "one unpinned release, one gap");
    assert_eq!(found[0].affected_ids, vec!["rel:v1".to_string()]);
}

#[test]
fn a_pinned_release_is_silent() {
    let g = releases_and_epochs(3, &[0, 1, 2]);
    assert!(
        epochless(&g.detect_gaps().unwrap()).is_empty(),
        "every release pinned — nothing to report"
    );
}

/// THE INVISIBILITY, made explicit. `rel:v1` and `epoch:v1` both exist and their
/// names correspond; only the EDGE is missing. This is precisely the state that
/// reads as correct to every human reader, and the detector must not be fooled
/// by the naming convention the way the rest of us were.
#[test]
fn a_matching_epoch_name_does_not_count_as_a_pin() {
    let mut g = releases_and_epochs(2, &[0]);
    // epoch:v1 exists and is named for the release. No AT_EPOCH edge joins them.
    assert!(
        g.get_node(node::DESIGN_EPOCH, "epoch:v1")
            .unwrap()
            .is_some(),
        "the epoch node exists — that is the whole trap"
    );
    // And a DIFFERENT edge type joins them, so the evidence string's claim
    // ("any other edge between them does not pin it") is actually exercised
    // rather than merely asserted at the user.
    g.create_edge(
        edge::ANTICIPATES,
        node::RELEASE,
        "rel:v1",
        node::DESIGN_EPOCH,
        "epoch:v1",
        Props::new(),
    )
    .unwrap();
    let gaps = g.detect_gaps().unwrap();
    let found = epochless(&gaps);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].affected_ids, vec!["rel:v1".to_string()]);
}

/// MUTATION CHECK on the guard: with no epochs anywhere the temporal axis is
/// simply not in use, and one gap per release would be a flood about a
/// modelling choice nobody made. Delete the guard and this test fails.
#[test]
fn releases_alone_with_no_epoch_spine_are_not_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    for i in 0..3 {
        g.create_node(
            node::RELEASE,
            &format!("rel:v{i}"),
            Props::new().set("name", format!("v0.{i}.0")),
        )
        .unwrap();
    }
    assert!(
        epochless(&g.detect_gaps().unwrap()).is_empty(),
        "no epochs exist at all — not one gap per release"
    );
}

/// Build one epoch plus one unpinned release carrying `status`.
fn unpinned_release_with_status(status: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::DESIGN_EPOCH,
        "epoch:v0",
        Props::new().set("name", "v0 cut").set("sequence", 0i64),
    )
    .unwrap();
    g.create_node(
        node::RELEASE,
        "rel:v1",
        Props::new().set("name", "v0.1.0").set("status", status),
    )
    .unwrap();
    g
}

/// A PLANNED release has not been cut, so it has no epoch yet and that is
/// correct. Firing here would be an alarm on correct work — the shape BL-115
/// names — and would have made the gate permanently red for every release
/// sitting on the roadmap.
#[test]
fn a_planned_release_is_not_yet_expected_to_have_an_epoch() {
    let g = unpinned_release_with_status("planned");
    assert!(
        epochless(&g.detect_gaps().unwrap()).is_empty(),
        "a planned release is not late for an epoch it does not yet need"
    );
}

/// ...but a release that HAS been cut, with the edge forgotten, must be caught.
/// This is `rel:v0190`'s failure reproduced in miniature, and it is the whole
/// reason the rule exists.
#[test]
fn cutting_a_release_without_its_edge_is_caught() {
    for status in ["built", "deployed", "retired"] {
        let g = unpinned_release_with_status(status);
        let gaps = g.detect_gaps().unwrap();
        let found = epochless(&gaps);
        assert_eq!(found.len(), 1, "status {status} must be caught");
        assert_eq!(found[0].affected_ids, vec!["rel:v1".to_string()]);
    }
}

/// THE RESIDUAL TRAP, on the record rather than left implicit: `Release.status`
/// defaults to `planned`, so a release that shipped but never had its status
/// set is exempt and reads exactly like one that is genuinely still to come.
/// This rule cannot tell them apart and does not try — a status that claims
/// less than the structure shows is `status_contradiction`'s territory, not
/// this one's. Recorded as a test so the limit is checkable instead of
/// discovered later by someone it bites.
#[test]
fn a_release_with_no_status_at_all_inherits_the_planned_exemption() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::DESIGN_EPOCH,
        "epoch:v0",
        Props::new().set("name", "v0 cut").set("sequence", 0i64),
    )
    .unwrap();
    // No status set at all -> schema default `planned`.
    g.create_node(node::RELEASE, "rel:v1", Props::new().set("name", "v0.1.0"))
        .unwrap();
    assert!(
        epochless(&g.detect_gaps().unwrap()).is_empty(),
        "documents the limit: the default makes this indistinguishable from planned"
    );
}

/// Per-release keying: accepting "v0.17.0 predates the epoch spine" must not
/// also accept the next release cut without one. Distinct ids are what make
/// that true, and `is_aggregate` returning true would collapse them.
#[test]
fn each_unpinned_release_gets_its_own_gap_id() {
    let g = releases_and_epochs(3, &[0]);
    let gaps = g.detect_gaps().unwrap();
    let found = epochless(&gaps);
    assert_eq!(found.len(), 2);
    assert_ne!(
        found[0].id, found[1].id,
        "one judgement per release, not one for the class"
    );
}

/// BL-114 applied at birth: the finding must name the edge kind it examined,
/// so nobody has to guess whether "no epoch" meant "no edge" or "no node".
#[test]
fn the_finding_names_the_edge_it_considered() {
    let g = releases_and_epochs(2, &[0]);
    let gaps = g.detect_gaps().unwrap();
    let found = epochless(&gaps);
    assert!(
        found[0].description.contains("AT_EPOCH"),
        "description must name the edge kind: {}",
        found[0].description
    );
    assert!(
        found[0].evidence.contains("AT_EPOCH"),
        "evidence must name the edge kind: {}",
        found[0].evidence
    );
}

// ---------------------------------------------------------------------------
// undeclared_seam — the coupling nobody wrote a contract for
//
// `req:an-undeclared-coupling-is-named-not-just-counted`. The set was already
// computed by `maturity_report`'s seams band and thrown away; these tests hold
// the two silences and the one thing the finding must never do.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

fn depends_on(g: &mut DesignGraph, from: &str, to: &str) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        from,
        node::COMPONENT,
        to,
        HashMap::new(),
    )
    .unwrap();
}

/// Two components that depend on each other with nothing recorded between them,
/// plus enough thread that the phase detectors are not the only thing firing.
fn coupled_pair() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component("cmp:service", "Service", "the service", None)
        .unwrap();
    g.add_component("cmp:store", "Store", "the store", None)
        .unwrap();
    g.allocate("cap:a", "cmp:service").unwrap();
    depends_on(&mut g, "cmp:service", "cmp:store");
    g
}

fn seam_gap(g: &DesignGraph) -> Option<reflow2_core::GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::UndeclaredSeam)
}

#[test]
fn a_coupling_with_no_contract_is_named() {
    let g = coupled_pair();
    let gap =
        seam_gap(&g).unwrap_or_else(|| panic!("got {:?}", sources(&g.detect_gaps().unwrap())));

    // It names the PAIR — both ends, by name in the question and by id in the
    // evidence, because a reader needs one and a tool needs the other.
    assert!(
        gap.description.contains("Service") && gap.description.contains("Store"),
        "both ends must be named: {}",
        gap.description
    );
    assert!(
        gap.evidence.contains("cmp:service") && gap.evidence.contains("cmp:store"),
        "evidence must carry the ids: {}",
        gap.evidence
    );
    assert_eq!(
        gap.affected_ids,
        vec!["cmp:service".to_string(), "cmp:store".to_string()]
    );
    assert_eq!(gap.scope, GapScope::Project);
}

/// The whole constraint of `req:an-undeclared-coupling-is-named-not-just-counted`:
/// reflow2 can see THAT two parts are coupled and cannot know WHAT crosses the
/// boundary. Naming a medium, a payload or a direction would be exactly the
/// fabrication `cap:no-fabricated-repair` exists to prevent — so the finding
/// asks, and every word of it stays interrogative.
#[test]
fn the_finding_asks_for_the_contract_and_never_drafts_one() {
    let g = coupled_pair();
    let gap = seam_gap(&g).unwrap();
    assert!(
        gap.description.contains('?'),
        "it must ask, not assert: {}",
        gap.description
    );
    // No invented contract vocabulary. If any of these ever appear, something
    // has started guessing what runs across a boundary it cannot see.
    for fabricated in ["HTTP", "REST", "JSON", "gRPC", "TCP", "queue", "protobuf"] {
        assert!(
            !gap.description.contains(fabricated) && !gap.evidence.contains(fabricated),
            "the finding must not propose a contract, found {fabricated:?} in: {} / {}",
            gap.description,
            gap.evidence
        );
    }
}

#[test]
fn a_declared_seam_is_not_reported() {
    let mut g = coupled_pair();
    g.add_interface("iface:reads", "Reads").unwrap();
    g.provides("cmp:store", "iface:reads").unwrap();
    g.consumes("cmp:service", "iface:reads").unwrap();

    assert!(
        seam_gap(&g).is_none(),
        "got {:?}",
        sources(&g.detect_gaps().unwrap())
    );
}

/// One-sided is exactly the unrecorded contract the capture skill warns about,
/// and `maturity`'s seams band has always counted it as undeclared. The detector
/// must agree with the band it was extracted from.
#[test]
fn a_one_sided_interface_does_not_close_the_seam() {
    let mut g = coupled_pair();
    g.add_interface("iface:reads", "Reads").unwrap();
    g.provides("cmp:store", "iface:reads").unwrap();
    // Nobody CONSUMES it.

    assert!(seam_gap(&g).is_some(), "a half-written contract is not one");
}

/// `maturity` already words this exactly right — "no two Components depend on
/// each other, so there is no seam to declare — an absence, not a deficiency".
/// A detector that reported a clean zero as a fault would contradict its own band.
#[test]
fn a_design_with_no_component_dependencies_stays_silent() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();

    assert!(
        seam_gap(&g).is_none(),
        "got {:?}",
        sources(&g.detect_gaps().unwrap())
    );
}

/// The BL-73 lesson, and the reason this is an aggregate: reflow2's own design
/// would emit 73 of these individually, and `unexpected_coupling` was retired
/// for flooding on correct architecture.
#[test]
fn many_undeclared_couplings_are_one_question_not_many() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    for i in 0..10 {
        g.add_component(&format!("cmp:{i}"), &format!("Part {i}"), "a part", None)
            .unwrap();
    }
    g.allocate("cap:a", "cmp:0").unwrap();
    for i in 0..9 {
        depends_on(&mut g, &format!("cmp:{i}"), &format!("cmp:{}", i + 1));
    }

    let seams: Vec<_> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|x| x.gap_source == GapSource::UndeclaredSeam)
        .collect();
    assert_eq!(seams.len(), 1, "one question keyed on the rule, not nine");
    assert!(
        seams[0].title.contains('9'),
        "the count belongs in the title: {}",
        seams[0].title
    );
    // The question shows a readable sample; the evidence carries all nine, so
    // nothing is silently dropped.
    assert!(seams[0].description.contains("and 3 more"));
    for i in 0..10 {
        assert!(
            seams[0].evidence.contains(&format!("cmp:{i}")),
            "evidence must list every pair"
        );
    }
}

/// Aggregate keying (`req:set-scoped-acknowledgement-keys-on-its-rule`): the
/// standing judgement is about the practice, so adding a coupling must not
/// expire it. This is the failure `unvalidated_capability` was re-acknowledged
/// about twenty times for.
#[test]
fn the_seam_gap_id_survives_a_new_coupling() {
    let mut g = coupled_pair();
    let before = seam_gap(&g).unwrap().id;

    g.add_component("cmp:third", "Third", "another part", None)
        .unwrap();
    depends_on(&mut g, "cmp:service", "cmp:third");

    let after = seam_gap(&g).unwrap();
    assert_eq!(
        before, after.id,
        "an aggregate is keyed on its rule, so the acknowledgement carries"
    );
    assert!(
        after.title.contains('2'),
        "but the count moves: {}",
        after.title
    );
}

// ---------------------------------------------------------------------------
// decomposition_coverage — what the parent held that no child holds
//
// `req:decomposition-covers-its-parent`, `cap:decomposition-coverage-is-asked`.
// reflow2 rolls delivery UP a decomposition (`report.rs`: a parent is delivered
// exactly when every child is) and never asks whether the children AMOUNT TO
// the parent. A parent split into two children addressing a tenth of it reports
// `delivered` the moment both close — in the number this project treats as
// ground truth (`req:completion-computed`).
//
// The field instance is not hypothetical: reflow's monolithic
// 01-systems_engineering was split into 01a–01f, and two cross-cutting blocks
// (`context_management`, `self_improvement`) were present in all six originals
// and absent from all seven children. Nothing noticed for months. A
// decomposition BY SUBJECT drops what belongs to no single subject.
//
// It ASKS and never judges (`dec:report-dont-judge`): no refusal, no LLM ruling
// on sufficiency, and — the test at the bottom — no guess at what is missing.
// ---------------------------------------------------------------------------

/// A parent split into three children, the shape `requirement_lineage` uses.
fn split_checkout() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement(
        "req:checkout",
        "Checkout",
        "The app must have a checkout system, and it must log every attempt.",
    )
    .unwrap();
    for (id, name) in [
        ("req:card", "Enter a card"),
        ("req:discount", "Apply a discount code"),
    ] {
        g.add_requirement(id, name, "part of checkout").unwrap();
        g.decomposes(id, "req:checkout").unwrap();
    }
    g
}

fn coverage_gaps(g: &DesignGraph) -> Vec<reflow2_core::GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|x| x.gap_source == GapSource::DecompositionCoverage)
        .collect()
}

#[test]
fn a_decomposition_is_asked_what_it_dropped() {
    let g = split_checkout();
    let found = coverage_gaps(&g);
    assert_eq!(
        found.len(),
        1,
        "one question per decomposition, got {:?}",
        sources(&g.detect_gaps().unwrap())
    );
    let gap = &found[0];

    // The parent is what the reader has to re-read, so it is named.
    assert!(
        gap.description.contains("Checkout"),
        "the parent must be named: {}",
        gap.description
    );
    // Anchored on the whole decomposition: parent AND children. The children are
    // the answer's working material — you cannot say what is missing without
    // knowing what is there.
    for id in ["req:checkout", "req:card", "req:discount"] {
        assert!(
            gap.affected_ids.iter().any(|a| a == id),
            "{id} must be anchored: {:?}",
            gap.affected_ids
        );
    }
    assert!(
        gap.evidence.contains("DECOMPOSES"),
        "evidence must name the edge kind it ranged over: {}",
        gap.evidence
    );
}

#[test]
fn a_requirement_nobody_split_is_silent() {
    // Absence is not deficiency: an undecomposed requirement has no coverage
    // question, and firing on one would make the detector a tax on flat designs.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:solo", "Solo", "one thing").unwrap();
    assert!(coverage_gaps(&g).is_empty());
}

#[test]
fn only_decomposes_counts_not_every_incoming_edge() {
    // The boundary that matters for `req:requirement-lineage`: a DERIVED
    // requirement adds new technical necessity and is not expected to cover
    // anything, and satisfaction is not decomposition either. Counting any
    // incoming edge would turn every satisfied requirement into a coverage
    // question.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:parent", "Parent", "the need")
        .unwrap();
    for (cap, name) in [("cap:one", "One"), ("cap:two", "Two")] {
        g.add_capability(cap, name, "does it", None).unwrap();
        g.satisfies(cap, "req:parent").unwrap();
    }
    g.add_requirement("req:derived", "Derived", "technical necessity")
        .unwrap();
    g.set_requirement_lineage("req:derived", "derived").unwrap();

    assert!(
        coverage_gaps(&g).is_empty(),
        "no DECOMPOSES edge exists, so there is no decomposition to check"
    );
}

#[test]
fn the_answer_is_recorded_and_the_question_stops() {
    // The whole point: a deliberate narrowing and an accidental drop look
    // identical today. Recording the answer is what separates them, and the
    // acknowledgement's reason is where it lives.
    let mut g = split_checkout();
    let gap = coverage_gaps(&g).remove(0);
    let affected = gap.affected_ids.clone();
    g.acknowledge_gap(
        &gap.id,
        &affected,
        "Deliberate: logging moved to req:audit-trail, tracked separately.",
    )
    .unwrap();

    assert!(
        coverage_gaps(&g).is_empty(),
        "an answered question must stop being asked"
    );
    let reviewed = g.reviewed_gaps().unwrap();
    assert!(
        reviewed
            .iter()
            .any(|r| r.gap_id == gap.id && r.reason.contains("req:audit-trail")),
        "the answer must survive where a later reader finds it"
    );
}

#[test]
fn changing_the_split_re_asks() {
    // `is_aggregate = false` buys this, and it is the reason it must stay false:
    // the judgement was about THESE children. Add one and the earlier answer is
    // no longer an answer to the current question.
    let mut g = split_checkout();
    let first = coverage_gaps(&g).remove(0);
    let affected = first.affected_ids.clone();
    g.acknowledge_gap(&first.id, &affected, "covered, minus logging")
        .unwrap();
    assert!(coverage_gaps(&g).is_empty());

    g.add_requirement(
        "req:receipt",
        "Receive an email receipt",
        "part of checkout",
    )
    .unwrap();
    g.decomposes("req:receipt", "req:checkout").unwrap();

    let again = coverage_gaps(&g);
    assert_eq!(
        again.len(),
        1,
        "a changed decomposition is a fresh question"
    );
    assert_ne!(
        again[0].id, first.id,
        "the id must move with the child set, or the stale answer silences it"
    );
}

#[test]
fn a_parent_already_reporting_delivered_outranks_one_still_open() {
    // The risk stops being hypothetical the moment the roll-up fires: the parent
    // now asserts `delivered` in the coverage number, and nothing has ever asked
    // whether its children amount to it.
    let open = split_checkout();
    let open_sev = coverage_gaps(&open)[0].severity;

    let mut done = split_checkout();
    for (req, tag) in [("req:card", "card"), ("req:discount", "discount")] {
        let (cap, art, ver) = (
            format!("cap:{tag}"),
            format!("art:{tag}"),
            format!("ver:{tag}"),
        );
        done.add_capability(&cap, tag, "does it", Some("realized"))
            .unwrap();
        done.satisfies(&cap, req).unwrap();
        done.add_artifact(&art, tag, Some("code"), Some("src/x.rs"))
            .unwrap();
        done.realizes(&art, node::CAPABILITY, &cap, None, None)
            .unwrap();
        done.add_verification(&ver, tag, Some("test"), None)
            .unwrap();
        done.verifies(&ver, node::CAPABILITY, &cap).unwrap();
        done.set_verification_status(&ver, "passing", None).unwrap();
    }
    assert!(
        done.requirement_is_delivered("req:checkout").unwrap(),
        "precondition: the roll-up has fired"
    );

    let done_gap = &coverage_gaps(&done)[0];
    assert!(
        done_gap.severity > open_sev,
        "a delivered parent must outrank an open one: {} vs {open_sev}",
        done_gap.severity
    );
    assert!(
        done_gap.description.contains("delivered") || done_gap.title.contains("delivered"),
        "and it must SAY the claim is already being made: {} / {}",
        done_gap.title,
        done_gap.description
    );
}

#[test]
fn it_never_says_what_is_missing() {
    // The seams detector's constraint, in its own shape: reflow2 can see THAT a
    // decomposition might not cover its parent and cannot know WHAT fell out.
    // Naming the missing content would be fabrication (`cap:no-fabricated-repair`),
    // and a plausible wrong answer is worse than the question, because it gets
    // recorded as the answer.
    let g = split_checkout();
    let gap = &coverage_gaps(&g)[0];
    let text = format!("{} {} {}", gap.title, gap.description, gap.evidence);
    let lowered = text.to_lowercase();
    for invented in ["log", "missing:", "should add", "you forgot", "suggest"] {
        assert!(
            !lowered.contains(invented),
            "must not guess the dropped content ({invented:?}): {text}"
        );
    }
    assert!(
        gap.description.contains('?'),
        "it is a question, not a verdict: {}",
        gap.description
    );
}
