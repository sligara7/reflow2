//! Scoped detection — a team asks about its own part of a program.
//!
//! Anthony's case, 2026-07-25, and the fixture is built to match it: a satellite
//! capability with a space segment, a ground segment and a control segment; his
//! team owns the satellite; specifically it owns inter-satellite laser
//! communications. The question these tests pin is the day-to-day one — *what is
//! MY part owed* — and the two properties that keep the answer honest: it must
//! never imply the rest of the program is clean, and it must refuse a seed that
//! does not exist rather than reporting an empty region as good news.

use reflow2_core::detect::GapSource;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, LinkArtifactOptions};

/// Three segments, two of them with a hole of their own.
///
/// `cmp:laser` is the team's part: a capability allocated to it, satisfying a
/// requirement, with nothing realizing it — so the team owns exactly one gap.
/// The ground segment carries an unrelated one, which must stay visible as
/// somebody's and invisible as theirs.
fn program() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Satellite capability").unwrap();

    for (id, name, level) in [
        ("sys:space", "Space segment", "system"),
        ("sys:ground", "Ground segment", "system"),
        ("sys:control", "Control segment", "system"),
    ] {
        g.add_component(id, name, "a segment of the program", Some(level))
            .unwrap();
        g.contains("proj:sat", node::COMPONENT, id).unwrap();
    }

    // The team's part, one level down inside the space segment.
    g.add_component(
        "cmp:laser",
        "Inter-satellite laser comms",
        "crosslink terminal",
        Some("subsystem"),
    )
    .unwrap();
    g.contain_component("sys:space", "cmp:laser").unwrap();
    g.add_capability(
        "cap:crosslink",
        "Cross-link two satellites",
        "optical link between vehicles",
        None,
    )
    .unwrap();
    g.allocate("cap:crosslink", "cmp:laser").unwrap();
    g.add_requirement(
        "req:crosslink",
        "Satellites talk to each other",
        "Two satellites in the constellation must exchange data without a ground relay.",
    )
    .unwrap();
    g.satisfies("cap:crosslink", "req:crosslink").unwrap();

    // Somebody else's hole, in another segment entirely.
    g.add_capability(
        "cap:user-terminal",
        "Serve a user terminal",
        "downlink to users",
        None,
    )
    .unwrap();
    g.allocate("cap:user-terminal", "sys:ground").unwrap();
    g.add_requirement(
        "req:users",
        "Users get their data",
        "A user terminal receives its product within the latency budget.",
    )
    .unwrap();
    g.satisfies("cap:user-terminal", "req:users").unwrap();
    // The ground team has built something. That matters to the fixture: with zero
    // artifacts anywhere, the unrealized-capability detector stays quiet (nothing
    // is built yet, so complaining would be noise), and the laser team would have
    // no anchored gap of its own to find.
    // And an unbuilt one, so the fixture holds a gap that is genuinely somebody
    // else's: the out_of_scope count has to have something real to count.
    g.add_capability(
        "cap:ground-relay",
        "Relay through a ground station",
        "store and forward via the ground segment",
        None,
    )
    .unwrap();
    g.allocate("cap:ground-relay", "sys:ground").unwrap();
    g.satisfies("cap:ground-relay", "req:users").unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:ground-sw".into(),
        name: "ground segment software".into(),
        location: None,
        artifact_type: None,
        target_type: node::CAPABILITY.into(),
        target_id: "cap:user-terminal".into(),
        fragment_id: None,
        provenance: None,
        completeness: None,
        checksum: None,
    })
    .unwrap();
    g
}

#[test]
fn a_team_sees_its_own_part_and_is_told_what_it_did_not_see() {
    // The whole point: day to day the laser-comms team needs its own gaps, and a
    // program-sized gap list is how a team stops reading gap lists at all.
    let g = program();
    let all = g.detect_gaps().unwrap();
    let scoped = g.detect_gaps_in_scope("cmp:laser", 3).unwrap();

    assert_eq!(scoped.scope, "cmp:laser");
    assert_eq!(scoped.total, all.len(), "the total is the WHOLE design's");
    assert!(
        scoped.in_scope < scoped.total,
        "narrowing must narrow: {scoped:?}"
    );
    assert_eq!(
        scoped.in_scope + scoped.out_of_scope + scoped.unanchored,
        scoped.total,
        "every finding lands in exactly one bucket — none vanish"
    );
    assert!(
        scoped.region_size > 1,
        "the region must reach past the seed"
    );

    let ids: Vec<&str> = scoped
        .items
        .iter()
        .flat_map(|gp| gp.affected_ids.iter().map(String::as_str))
        .collect();
    assert!(
        ids.contains(&"cap:crosslink") || ids.contains(&"cmp:laser"),
        "the team's own findings must be there: {ids:?}"
    );
    assert!(
        !ids.contains(&"cap:user-terminal"),
        "another segment's hole is not this team's gap: {ids:?}"
    );
}

#[test]
fn the_out_of_scope_count_is_the_honesty() {
    // A scoped view that returned three gaps and said nothing about the other
    // forty would teach a team their program is healthy. That is the same silent
    // truncation rule 6 forbids, one level up from a capped read.
    let g = program();
    let scoped = g.detect_gaps_in_scope("cmp:laser", 3).unwrap();
    assert!(
        scoped.out_of_scope > 0,
        "this fixture deliberately has someone else's work in it: {scoped:?}"
    );
}

#[test]
fn a_wider_scope_never_sees_less() {
    // Depth is a radius, so growing it is monotone. If this ever fails, the
    // region computation has stopped being a radius and become something else.
    let g = program();
    let near = g.detect_gaps_in_scope("cmp:laser", 1).unwrap();
    let far = g.detect_gaps_in_scope("cmp:laser", 6).unwrap();
    assert!(far.region_size >= near.region_size);
    assert!(far.in_scope >= near.in_scope, "{near:?} vs {far:?}");
}

#[test]
fn scoping_to_the_project_is_the_whole_design() {
    // The identity case, worth pinning: a scope wide enough to reach everything
    // must agree with the unscoped detector, or the filter is dropping findings
    // it should not.
    let g = program();
    let all = g.detect_gaps().unwrap();
    let scoped = g.detect_gaps_in_scope("proj:sat", 8).unwrap();
    assert_eq!(
        scoped.in_scope + scoped.unanchored,
        all.len(),
        "everything anchored is in scope, and the rest anchors on nothing: {scoped:?}"
    );
    assert_eq!(scoped.out_of_scope, 0, "nothing is elsewhere: {scoped:?}");
    assert!(
        scoped.unanchored > 0,
        "the phase gaps have no location — the point of the bucket: {scoped:?}"
    );
}

#[test]
fn an_unknown_seed_is_refused_not_answered_as_empty() {
    // "No gaps in your area" and "there is no such area" are different answers,
    // and a typo must never read as good news (rule 4).
    let g = program();
    let err = g.detect_gaps_in_scope("cmp:lasr", 3);
    assert!(err.is_err(), "a mistyped scope must fail loud");
    let message = format!("{}", err.unwrap_err());
    assert!(
        message.contains("cmp:lasr"),
        "the message must name what was not found: {message}"
    );
}

#[test]
fn a_project_level_rollup_reaches_the_team_and_is_counted_as_the_programs() {
    // Filtering must not become the tool deciding what a team may worry about.
    // A project-wide finding that touches their work appears in their view — and
    // is counted separately, so they can see it is the program's, not theirs.
    let mut g = program();
    // Give the laser capability a passing verification: that is what puts it in
    // the project-level unvalidated_capability rollup.
    g.add_verification("ver:crosslink", "Optical link bench test", None, None)
        .unwrap();
    g.verifies("ver:crosslink", node::CAPABILITY, "cap:crosslink")
        .unwrap();
    g.set_verification_status("ver:crosslink", "passing", None)
        .unwrap();

    let scoped = g.detect_gaps_in_scope("cmp:laser", 3).unwrap();
    let rollups: Vec<&GapSource> = scoped
        .items
        .iter()
        .map(|gp| &gp.gap_source)
        .filter(|s| **s == GapSource::UnvalidatedCapability)
        .collect();
    assert!(
        !rollups.is_empty(),
        "the rollup must reach them: {scoped:?}"
    );
    assert!(
        scoped.project_level > 0,
        "and be counted as the program's finding: {scoped:?}"
    );
}

#[test]
fn structural_defects_scope_the_same_way() {
    // Different question — "is my part of the architecture sound" rather than
    // "what am I owed" — same contract: filter, count, never imply the rest is
    // clean.
    let g = program();
    let all = g.detect_defects().unwrap();
    let scoped = g.detect_defects_in_scope("cmp:laser", 3).unwrap();
    assert_eq!(scoped.total, all.len());
    assert_eq!(scoped.in_scope + scoped.out_of_scope, scoped.total);
}

/// A seed with no edges makes `in_scope: 0` VACUOUS, and the result must say so
/// in words rather than leave it to be inferred from `region_size: 1`.
///
/// dev_storyflow (w-c216679a, 2026-08-09) scoped to an Epoch and to a Fragment,
/// got `in_scope: 0` at depth 2 AND at depth 5, and caught it only because they
/// ran a positive control on a Project. Their framing is the one this encodes:
/// a bare `in_scope: 0` beside `region_size: 1` is the shape most likely to be
/// banked as "my area is clean".
#[test]
fn a_seed_with_no_edges_says_its_scoped_answer_is_vacuous() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "nothing satisfies this")
        .unwrap();
    g.add_capability("cap:c", "C", "answers no stated need", None)
        .unwrap();
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:p",
        node::REQUIREMENT,
        "req:r",
        Props::new(),
    )
    .unwrap();
    // An island: recorded, real, and connected to nothing.
    g.add_epoch(
        "epoch:island",
        "an epoch",
        reflow2_core::EpochType::Revision,
        1,
    )
    .unwrap();

    let vacuous = g.detect_gaps_in_scope("epoch:island", 2).unwrap();
    assert_eq!(
        vacuous.region_size, 1,
        "precondition: the seed is an island"
    );
    assert_eq!(vacuous.in_scope, 0);
    let note = vacuous
        .note
        .as_deref()
        .expect("a one-node region must say its answer is vacuous");
    assert!(note.contains("VACUOUS"), "{note}");
    assert!(
        note.contains("epoch:island"),
        "it must name the seed the caller passed: {note}"
    );
    assert!(
        vacuous.total > 0,
        "and the design's real findings are still counted, so nothing reads as clean"
    );

    // Depth does not rescue it — theirs was vacuous at 5 too.
    assert!(
        g.detect_gaps_in_scope("epoch:island", 5)
            .unwrap()
            .note
            .is_some()
    );

    // POSITIVE CONTROL: a real region must NOT carry the note, or its presence
    // stops being a signal and becomes noise on every call.
    let real = g.detect_gaps_in_scope("proj:p", 2).unwrap();
    assert!(real.region_size > 1, "precondition: a real region");
    assert!(
        real.note.is_none(),
        "a region that can answer must stay quiet: {:?}",
        real.note
    );

    // The defect detector is scoped by the same machinery and gets the same say.
    assert!(
        g.detect_defects_in_scope("epoch:island", 2)
            .unwrap()
            .note
            .is_some()
    );
}
