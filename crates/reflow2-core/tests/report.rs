//! Graph report (SYNTHESIZE) — aggregates the deterministic analyses into one
//! "what should I look at?" artifact.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, Dimension};

fn dep(g: &mut DesignGraph, from: &str, to: &str, w: f64) {
    g.create_edge(
        edge::DEPENDS_ON,
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().set("weight", w),
    )
    .unwrap();
}

#[test]
fn report_aggregates_every_analysis_and_renders_markdown() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Intent + two capability clusters allocated to two components.
    g.create_node(
        node::REQUIREMENT,
        "req:r",
        Props::new()
            .set("name", "R")
            .set("statement", "s")
            .set("status", "accepted"),
    )
    .unwrap();
    for c in ["cap:a1", "cap:a2", "cap:b1", "cap:b2"] {
        g.add_capability(c, c, "does a thing", None).unwrap();
    }
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.satisfies("cap:a1", "req:r").unwrap();
    for (c, comp) in [
        ("cap:a1", "cmp:a"),
        ("cap:a2", "cmp:a"),
        ("cap:b1", "cmp:b"),
        ("cap:b2", "cmp:b"),
    ] {
        g.allocate(c, comp).unwrap();
    }
    dep(&mut g, "cap:a1", "cap:a2", 0.9);
    dep(&mut g, "cap:b1", "cap:b2", 0.9);
    dep(&mut g, "cap:a1", "cap:b1", 0.1); // the surprising bridge
    // A declining quality dimension.
    g.add_dimension_observation(
        "o1",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.9,
        "e01",
        None,
    )
    .unwrap();
    g.add_dimension_observation(
        "o2",
        node::COMPONENT,
        "cmp:a",
        Dimension::Maintainability,
        0.5,
        "e02",
        None,
    )
    .unwrap();

    let r = g.graph_report().unwrap();

    // Snapshot.
    assert!(r.node_counts.contains(&(node::CAPABILITY, 4)));
    assert!(r.node_counts.contains(&(node::COMPONENT, 2)));
    assert!(r.total_nodes >= 7);

    // Every analysis is represented.
    assert!(r.gap_count > 0 && !r.top_gaps.is_empty());
    let alloc = r.allocation.as_ref().expect("components exist");
    assert_eq!(alloc.component_count, 2);
    assert!(alloc.modularity > 0.9);
    assert_eq!(r.surprising.len(), 1);
    assert_eq!(r.surprising[0].from_id, "cap:a1");
    assert_eq!(r.surprising[0].to_id, "cap:b1");
    assert_eq!(r.declining.len(), 1);
    assert_eq!(r.declining[0].target_id, "cmp:a");
    assert_eq!(r.declining[0].dimension, Dimension::Maintainability);

    // Markdown renders each section.
    let md = r.to_markdown();
    for section in [
        "# Design graph report",
        "## Snapshot",
        "## Top gaps",
        "## Allocation health",
        "## Surprising couplings",
        "## Quality drift",
    ] {
        assert!(md.contains(section), "missing section: {section}");
    }
    assert!(md.contains("cmp:a"));
    assert!(md.contains("maintainability"));
}

#[test]
fn an_empty_graph_reports_empty() {
    let g = DesignGraph::open_in_memory().unwrap();
    let r = g.graph_report().unwrap();
    assert_eq!(r.total_nodes, 0);
    assert_eq!(r.gap_count, 0);
    assert!(r.allocation.is_none());
    assert!(r.to_markdown().contains("Empty graph"));
}

/// BL-43, from the storyflow adopt trial: the import wrote 122 nodes and the
/// report said 109 — the 13 missing were exactly the Fragments, because
/// `total_nodes` summed a hardcoded design-layer list. A count that silently
/// omits a node type is a quiet lie about the size of the design.
#[test]
fn the_total_counts_every_node_including_the_provenance_layer() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "R", "need it").unwrap();
    g.create_node(
        node::FRAGMENT,
        "frag:src",
        Props::new().set("title", "the note it came from"),
    )
    .unwrap();

    let rep = g.graph_report().unwrap();
    assert_eq!(rep.design_nodes, 2, "Project + Requirement");
    assert_eq!(rep.total_nodes, 3, "…and the Fragment is a node too");
    assert_eq!(
        rep.other_counts,
        vec![("Fragment".to_string(), 1)],
        "what the design-layer itemisation does not cover is named, not dropped"
    );

    // And it is visible to a reader, not just to a field.
    let md = rep.to_markdown();
    assert!(md.contains("Fragment 1"), "{md}");
    assert!(md.contains("3 nodes in total"), "{md}");
}

// ---- Loop status: the debt list, computed from state (BL-74) ----------------

#[test]
fn a_design_with_nothing_owed_reads_clean() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();

    let status = g.loop_status().unwrap();

    // A bare project draws phase nudges, and nudges are guidance, not debt.
    assert!(status.clean, "{:?}", status.next);
    assert!(status.next.is_empty());
    assert_eq!(status.unsurfaced_gaps, 0);
}

#[test]
fn an_open_decision_is_owed_only_once_somebody_has_been_asked_to_settle_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:a", "A", Some("person"), Some("a"), None)
        .unwrap();

    // A musing. add_decision lands `proposed` on purpose, and the brainstorm
    // skill records half-formed ideas exactly this way — so thinking out loud
    // must keep costing nothing.
    g.add_decision("dec:musing", "Maybe X?", "Still turning it over.", None)
        .unwrap();
    let thinking = g.loop_status().unwrap();
    assert_eq!(
        thinking.unsettled_assigned_decisions, 0,
        "a proposed decision nobody was asked to settle is thinking, not debt"
    );

    // An author is not an approver. Recording WHOSE idea it is says nothing
    // about who owes an answer, so this must stay quiet too — otherwise every
    // attributed brainstorm becomes a nag.
    g.authored_by("Decision", "dec:musing", "who:a", Some("author"), None)
        .unwrap();
    assert_eq!(
        g.loop_status().unwrap().unsettled_assigned_decisions,
        0,
        "author != approver"
    );

    // Asking a named person to decide is what creates the debt.
    g.add_decision("dec:asked", "Which way?", "Two roads, in prose.", None)
        .unwrap();
    g.authored_by("Decision", "dec:asked", "who:a", Some("approver"), None)
        .unwrap();

    let owed = g.loop_status().unwrap();
    assert_eq!(owed.unsettled_assigned_decisions, 1);
    assert!(!owed.clean);
    assert!(
        owed.next.iter().any(|l| l.contains("asked to settle")),
        "{:?}",
        owed.next
    );

    // Settling it clears the debt — the counter is state, never run history.
    g.set_decision_status("dec:asked", "accepted").unwrap();
    assert_eq!(
        g.loop_status().unwrap().unsettled_assigned_decisions,
        0,
        "an accepted decision is not owed"
    );
}

/// The count was never the hard part — finding out WHICH was.
///
/// flo2 filed this (F4): every other debt line names a tool to call next, and
/// this one left the reader to walk `AUTHORED_BY` edges by hand. Hit
/// independently in this repo the same day, where identifying two assigned
/// decisions meant `jq`-ing the committed export.
#[test]
fn the_assigned_decisions_are_listed_and_not_merely_counted() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:a", "A", Some("person"), Some("a"), None)
        .unwrap();
    g.add_decision("dec:asked", "Which way?", "Two roads.", None)
        .unwrap();
    g.authored_by("Decision", "dec:asked", "who:a", Some("approver"), None)
        .unwrap();

    let s = g.loop_status().unwrap();
    assert_eq!(s.unsettled_assigned_decisions, s.assigned_decisions.len());
    let one = &s.assigned_decisions[0];
    assert_eq!(one.decision_id, "dec:asked");
    assert_eq!(one.approver_id, "who:a");
    assert_eq!(
        one.name, "Which way?",
        "the reader needs the words, not the id"
    );
}

/// "What needs ME" — the asynchronous form of the loop.
#[test]
fn the_loop_answers_what_is_owed_to_one_named_person() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for (id, name) in [("who:a", "A"), ("who:b", "B")] {
        g.add_contributor(id, name, Some("person"), Some(name), None)
            .unwrap();
    }
    for (dec, who) in [("dec:for-a", "who:a"), ("dec:for-b", "who:b")] {
        g.add_decision(dec, dec, "In prose.", None).unwrap();
        g.authored_by("Decision", dec, who, Some("approver"), None)
            .unwrap();
    }

    let all = g.loop_status().unwrap();
    assert_eq!(all.unsettled_assigned_decisions, 2);
    assert!(all.scope.is_none(), "unscoped carries no scope block");

    let mine = g.loop_status_for(Some("who:a")).unwrap();
    assert_eq!(mine.unsettled_assigned_decisions, 1);
    assert_eq!(mine.assigned_decisions[0].decision_id, "dec:for-a");
    assert_eq!(mine.scope.as_ref().unwrap().contributor_id, "who:a");
    assert!(
        mine.next.iter().any(|l| l.contains("who:a")),
        "the line an agent reads aloud must name the person: {:?}",
        mine.next
    );
}

/// **The honesty test, and the reason this is not a filter.**
///
/// A person with nothing assigned must not be told the design is fine. Every
/// debt class except assignment belongs to the DESIGN, so a scoped answer names
/// those counts rather than zeroing them — otherwise `clean: true` would be the
/// tool confidently reporting "nothing is owed to you" when the truth is "I
/// cannot tell whose this is". That is the defect class this project has now
/// found four times in other guises: a value that cannot tell a real NONE from
/// an I-CANNOT-SAY.
#[test]
fn a_scoped_answer_never_reads_as_the_design_being_clean() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:idle", "Idle", Some("person"), Some("i"), None)
        .unwrap();
    // Real design debt that belongs to nobody in particular. A requirement
    // needs a capability to exist before `unsatisfied_requirement` can fire —
    // with none, "nothing satisfies it" is not yet a finding about anything.
    g.add_requirement("req:orphan", "Orphan", "Something nothing satisfies")
        .unwrap();
    g.add_capability(
        "cap:orphan",
        "Orphan capability",
        "Answers no stated need",
        None,
    )
    .unwrap();

    let design_wide = g.loop_status().unwrap();
    assert!(
        !design_wide.clean,
        "precondition: the design must actually owe something"
    );

    let theirs = g.loop_status_for(Some("who:idle")).unwrap();
    assert!(
        theirs.clean,
        "scoped, clean means nothing is assigned to this person"
    );
    let scope = theirs.scope.as_ref().unwrap();
    assert!(
        !scope.not_attributable.is_empty(),
        "the design's own debt must still be reported, not filtered to zero"
    );
    assert!(
        theirs
            .next
            .iter()
            .any(|l| l.contains("no per-person attribution")),
        "the not-attributable debt must be said in WORDS, not left in a field: {:?}",
        theirs.next
    );
    // And the design-wide counters are untouched by scoping — they are facts
    // about the design and scoping does not make them smaller.
    assert_eq!(theirs.unsurfaced_gaps, design_wide.unsurfaced_gaps);
}

/// A mistyped or renamed id must not answer "nothing is owed to you" — the most
/// reassuring reply available and the one nobody thinks to question.
#[test]
fn an_unknown_contributor_is_refused_rather_than_answered_with_an_empty_list() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:a", "A", Some("person"), Some("a"), None)
        .unwrap();

    let err = g.loop_status_for(Some("who:typo"));
    assert!(
        err.is_err(),
        "an unknown contributor must be an error, not an empty answer"
    );
    // Positive control: the same call with a real id succeeds, so the refusal
    // above is about the id and not about scoping being broken.
    assert!(g.loop_status_for(Some("who:a")).is_ok());
}

/// Scoping must not disturb the deliberate quiet around thinking out loud.
#[test]
fn scoping_does_not_make_an_unassigned_decision_somebodys_problem() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:a", "A", Some("person"), Some("a"), None)
        .unwrap();
    g.add_decision("dec:musing", "Maybe X?", "Turning it over.", None)
        .unwrap();
    // Authored by them, but nobody was ASKED.
    g.authored_by("Decision", "dec:musing", "who:a", Some("author"), None)
        .unwrap();

    let mine = g.loop_status_for(Some("who:a")).unwrap();
    assert_eq!(
        mine.unsettled_assigned_decisions, 0,
        "authoring a musing is not being asked to settle it"
    );
    assert!(mine.assigned_decisions.is_empty());
}

#[test]
fn captured_intent_owes_a_surface_pass_until_the_question_is_asked() {
    use reflow2_core::AskedQuestion;

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    // Stated intent plus a capability claiming realized, satisfying nothing,
    // verified by nothing: exactly the state a raw-tools-only capture session
    // leaves behind.
    g.add_requirement("req:r", "R", "Must do x.").unwrap();
    g.add_capability("cap:x", "X", "does x", Some("realized"))
        .unwrap();

    let before = g.loop_status().unwrap();
    assert!(!before.clean);
    assert!(
        before.unsurfaced_gaps > 0,
        "anchored gaps exist and nobody asked"
    );
    assert_eq!(before.unproven_capabilities, 1);
    assert!(
        before.next.iter().any(|l| l.contains("detect-and-ask")),
        "{:?}",
        before.next
    );
    assert!(
        before.next.iter().any(|l| l.contains("no passing check")),
        "{:?}",
        before.next
    );

    // Surfacing every anchored gap moves the debt from "unsurfaced" to
    // "waiting on the user" — the loop advanced one step.
    let gaps: Vec<_> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|gap| !gap.affected_ids.is_empty())
        .collect();
    for gap in &gaps {
        g.record_asked_question(
            &gap.id,
            &gap.affected_ids,
            "What should happen here?",
            AskedQuestion::default(),
        )
        .unwrap();
    }
    let asked = g.loop_status().unwrap();
    assert_eq!(asked.unsurfaced_gaps, 0);
    assert_eq!(asked.unanswered_questions, gaps.len());
    assert!(
        asked.next.iter().any(|l| l.contains("waiting on the user")),
        "{:?}",
        asked.next
    );

    // An answer that never reaches the design is its own named debt.
    g.answer_question(&gaps[0].id, "It should do x.").unwrap();
    let answered = g.loop_status().unwrap();
    assert_eq!(answered.unwritten_answers, 1);
    assert_eq!(answered.unanswered_questions, gaps.len() - 1);
    assert!(
        answered
            .next
            .iter()
            .any(|l| l.contains("never reached the design")),
        "{:?}",
        answered.next
    );

    // Proving the capability clears the unproven count.
    g.add_verification("ver:x", "x tests", Some("test"), None)
        .unwrap();
    g.verifies("ver:x", node::CAPABILITY, "cap:x").unwrap();
    g.set_verification_status("ver:x", "passing", None).unwrap();
    assert_eq!(g.loop_status().unwrap().unproven_capabilities, 0);
}

#[test]
fn recorded_drift_is_owed_a_disposition_until_accepted() {
    use reflow2_core::drift::{ObservedArtifact, ReconcileOptions};
    use reflow2_core::{DriftDisposition, LinkArtifactOptions};

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:x", "X", "does x", Some("realized"))
        .unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:x".into(),
        name: "x.rs".into(),
        location: Some("src/x.rs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:x".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:old".into()),
    })
    .unwrap();

    // Built, never reconciled: the ledger calls it unexamined and so do we.
    assert_eq!(g.loop_status().unwrap().unexamined_claims, 1);

    let report = g
        .reconcile_artifacts(
            &[ObservedArtifact {
                artifact_id: "art:x".into(),
                present: true,
                checksum: Some("sha256:new".into()),
                realizes: None,
            }],
            &ReconcileOptions {
                record_events: true,
                exhaustive: false,
                detected_at: Some("2026-07-21".into()),
            },
        )
        .unwrap();
    assert_eq!(report.findings.len(), 1);

    let drifted = g.loop_status().unwrap();
    assert_eq!(drifted.undispositioned_drift, 1);
    assert!(
        drifted.next.iter().any(|l| l.contains("disposition")),
        "{:?}",
        drifted.next
    );

    g.set_artifact_checksum(
        "art:x",
        "sha256:new",
        DriftDisposition::DesignHolds {
            change_type: reflow2_core::ChangeType::TestFailureFix,
        },
        None,
        Some("2026-07-21"),
    )
    .unwrap();
    let accepted = g.loop_status().unwrap();
    assert_eq!(accepted.undispositioned_drift, 0);
    assert_eq!(accepted.unexamined_claims, 0);
}

// ---- Requirement certainty: derived, never stored (BL-75) -------------------

#[test]
fn certainty_is_derived_from_status_and_provenance() {
    use reflow2_core::RequirementCertainty;

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();

    // Captured by an agent, awaiting the user: asserted.
    g.add_requirement("req:asserted", "A", "Someone stated it.")
        .unwrap();
    // Recovered from an artifact during adopt, awaiting the user.
    g.add_requirement("req:recovered", "R", "Read out of the code.")
        .unwrap();
    g.set_provenance(node::REQUIREMENT, "req:recovered", "inferred")
        .unwrap();
    // The user said yes — including to a recovered one: provenance keeps
    // saying how it ENTERED, status records their word.
    g.add_requirement("req:confirmed", "C", "The user confirmed it.")
        .unwrap();
    g.set_requirement_status("req:confirmed", "accepted")
        .unwrap();
    g.add_requirement(
        "req:confirmed-recovered",
        "CR",
        "Recovered, then confirmed.",
    )
    .unwrap();
    g.set_provenance(node::REQUIREMENT, "req:confirmed-recovered", "inferred")
        .unwrap();
    g.set_requirement_status("req:confirmed-recovered", "accepted")
        .unwrap();
    // The user decided it out — their word too, not uncertainty.
    g.add_requirement("req:out", "O", "Not in v1.").unwrap();
    g.set_requirement_status("req:out", "dropped").unwrap();

    for (req, expected) in [
        ("req:asserted", RequirementCertainty::Asserted),
        ("req:recovered", RequirementCertainty::Recovered),
        ("req:confirmed", RequirementCertainty::UserConfirmed),
        (
            "req:confirmed-recovered",
            RequirementCertainty::UserConfirmed,
        ),
        ("req:out", RequirementCertainty::SettledOut),
    ] {
        assert_eq!(
            g.requirement_certainty(req).unwrap(),
            expected,
            "{req} should read as {expected:?}"
        );
    }

    let b = g.requirement_certainty_breakdown().unwrap();
    assert_eq!(
        (b.user_confirmed, b.asserted, b.recovered, b.settled_out),
        (2, 1, 1, 1)
    );

    // And the report says it, so no session reconstructs it in prose.
    let md = g.graph_report().unwrap().to_markdown();
    assert!(
        md.contains(
            "Requirement certainty: 2 user-confirmed · 1 asserted, awaiting the user · \
             1 recovered from the artifact, awaiting the user · 1 settled out (deferred/dropped)."
        ),
        "{md}"
    );
}

#[test]
fn a_design_with_no_requirements_makes_no_certainty_claim() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    let r = g.graph_report().unwrap();
    assert!(r.requirement_certainty.is_none());
    assert!(!r.to_markdown().contains("Requirement certainty"));
}

/// `status` without `last_run_at` is a measurement presented as a property.
///
/// Both report surfaces now carry the recency, because the two defects that
/// produced this fix were found on different surfaces: the failing one through a
/// gap, the passing one **by accident**, since a passing check raises no gap at
/// all. Fixing only the loud half would have left the dangerous half silent.
#[test]
fn both_report_surfaces_carry_when_each_check_last_ran() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_capability("cap:a", "A", "does a", Some("realized"))
        .unwrap();
    g.add_verification("ver:ran", "ran tests", Some("test"), Some("unit"))
        .unwrap();
    g.verifies("ver:ran", node::CAPABILITY, "cap:a").unwrap();
    g.set_verification_status("ver:ran", "passing", Some("2026-07-25T18:49:11Z"))
        .unwrap();
    // The asserted-not-measured case: a verdict with no run behind it.
    g.add_verification("ver:never", "never run", Some("test"), Some("unit"))
        .unwrap();
    g.set_verification_status("ver:never", "passing", None)
        .unwrap();

    for (surface, rows) in [
        ("loop_status", g.loop_status().unwrap().verifications),
        ("graph_report", g.graph_report().unwrap().verifications),
    ] {
        let ran = rows
            .iter()
            .find(|v| v.verification_id == "ver:ran")
            .unwrap_or_else(|| panic!("{surface} must list every check"));
        assert_eq!(
            ran.last_run_at.as_deref(),
            Some("2026-07-25T18:49:11Z"),
            "{surface} must carry when the check last ran"
        );
        assert_eq!(ran.verifies, 1, "{surface} must say how much it speaks for");

        let never = rows
            .iter()
            .find(|v| v.verification_id == "ver:never")
            .unwrap_or_else(|| panic!("{surface} must list the unrun check too"));
        assert!(
            never.last_run_at.is_none(),
            "{surface}: a check that never ran must report None, not a fabricated time"
        );
    }
}

/// Omitting `last_run_at` must LEAVE IT ALONE, not erase it.
///
/// dev_storyflow filed this 2026-08-08 and re-filed it after retesting on
/// 0.26.1, both times on a throwaway node created for the purpose. The key was
/// REMOVED rather than nulled, so a wiped check was byte-identical to one that
/// had never run — and it fired from the most ordinary act there is, marking a
/// check `failing` after a regression, erasing the evidence it ever ran and
/// failing toward `never_run`, the field a later session greps for unproven
/// work.
///
/// Their sharpest argument is the one this test encodes: `set_interface_spec`,
/// `set_decision_status` and `set_epoch_status` all document the opposite
/// convention, and this function's own first line said "preserving its other
/// properties" while null-writing one of them.
#[test]
fn setting_a_status_again_does_not_erase_when_the_check_last_ran() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_verification("ver:v", "a check", Some("test"), Some("unit"))
        .unwrap();

    let ran = g
        .set_verification_status("ver:v", "passing", Some("2026-08-09T10:00:00Z"))
        .unwrap();
    assert_eq!(
        ran.properties.get("last_run_at").and_then(|v| v.as_str()),
        Some("2026-08-09T10:00:00Z"),
        "precondition: the timestamp landed"
    );

    // THE REPRODUCTION, one variable: same node, omitted parameter.
    let again = g.set_verification_status("ver:v", "failing", None).unwrap();
    assert_eq!(
        again.properties.get("last_run_at").and_then(|v| v.as_str()),
        Some("2026-08-09T10:00:00Z"),
        "omitting last_run_at must leave it alone — this is the data-loss bug"
    );
    assert_eq!(
        again.properties.get("status").and_then(|v| v.as_str()),
        Some("failing"),
        "and the status the caller DID pass must still be applied"
    );

    // Supplying one still replaces it — the fix must not make the parameter
    // inert, which would be the same bug pointing the other way.
    let moved = g
        .set_verification_status("ver:v", "passing", Some("2026-08-09T18:00:00Z"))
        .unwrap();
    assert_eq!(
        moved.properties.get("last_run_at").and_then(|v| v.as_str()),
        Some("2026-08-09T18:00:00Z"),
        "an explicit timestamp must still overwrite"
    );

    // And a check that never ran still reports absence rather than a fabricated
    // time — preserving nothing is not the same as inventing something.
    g.add_verification("ver:fresh", "never run", Some("test"), Some("unit"))
        .unwrap();
    let fresh = g
        .set_verification_status("ver:fresh", "planned", None)
        .unwrap();
    assert!(
        !fresh.properties.contains_key("last_run_at"),
        "a check with no run behind it must stay empty"
    );
}

/// Visibility, not a new nag. A counter here would make `clean` unreachable on
/// any design whose last run was yesterday — the permanently-red-check failure
/// rebuilt inside the tool meant to prevent it.
#[test]
fn surfacing_recency_does_not_make_the_loop_dirty() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    let before = g.loop_status().unwrap();
    g.add_verification("ver:old", "old tests", Some("test"), Some("unit"))
        .unwrap();
    g.set_verification_status("ver:old", "passing", Some("2020-01-01T00:00:00Z"))
        .unwrap();
    let after = g.loop_status().unwrap();
    assert_eq!(
        before.clean, after.clean,
        "a stale-but-passing check must be VISIBLE without changing whether the loop is clean"
    );
    assert!(
        after
            .verifications
            .iter()
            .any(|v| v.verification_id == "ver:old"),
        "…and it must actually be visible"
    );
}

/// Ownership is the second thing that can honestly be attributed to a person,
/// and the reason OWNED_BY was built.
///
/// Before it, a scoped answer could speak only about decisions somebody had been
/// ASKED to settle, and reported every gap as "I cannot tell whose this is".
/// `dec:ownership-reads-claims-before-adding-an-edge` set the condition — decide
/// on an edge once claims are shown insufficient — and named the disqualifying
/// evidence in advance: claims are transient work-in-hand, ownership is durable.
#[test]
fn gaps_standing_on_ground_you_own_are_attributed_to_you() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for (id, name) in [("who:mine", "Mine"), ("who:theirs", "Theirs")] {
        g.add_contributor(id, name, Some("person"), Some(name), None)
            .unwrap();
    }
    // Two capabilities that each raise a gap (nothing asked for them).
    g.add_requirement("req:r", "R", "so gaps can fire at all")
        .unwrap();
    g.add_capability("cap:mine", "Mine", "in my area", None)
        .unwrap();
    g.add_capability("cap:theirs", "Theirs", "in their area", None)
        .unwrap();

    let before = g.loop_status_for(Some("who:mine")).unwrap();
    assert!(
        before.gaps_on_owned_ground.is_empty(),
        "owning nothing must attribute nothing — an unowned design is ordinary"
    );

    g.owned_by(
        "Capability",
        "cap:mine",
        "who:mine",
        Some("the ingest half, not the export half"),
        Some("2026-08-09"),
    )
    .unwrap();

    let mine = g.loop_status_for(Some("who:mine")).unwrap();
    assert!(
        !mine.gaps_on_owned_ground.is_empty(),
        "a gap on ground I own must be attributed to me"
    );
    assert!(
        mine.gaps_on_owned_ground
            .iter()
            .all(|x| x.owned_ids.contains(&"cap:mine".to_string())),
        "each attributed gap must name WHICH owned node it stands on: {:?}",
        mine.gaps_on_owned_ground
    );
    assert!(
        mine.gaps_on_owned_ground
            .iter()
            .all(|x| !x.owned_ids.contains(&"cap:theirs".to_string())),
        "and must never claim ground I do not own"
    );
    assert!(
        !mine.clean,
        "a gap on your own ground is owed by you, so scoped clean must be false"
    );
    assert!(
        mine.next.iter().any(|l| l.contains("ground who:mine owns")),
        "the line an agent reads aloud must say it: {:?}",
        mine.next
    );

    // POSITIVE CONTROL: the other contributor owns nothing, so the identical
    // call must attribute nothing. Without this, an implementation that
    // attributed every gap to everybody would pass every assertion above.
    let theirs = g.loop_status_for(Some("who:theirs")).unwrap();
    assert!(
        theirs.gaps_on_owned_ground.is_empty(),
        "ownership must be per-person, not global: {:?}",
        theirs.gaps_on_owned_ground
    );

    // And the design-wide view is untouched — ownership narrows, never shrinks.
    assert_eq!(
        g.loop_status().unwrap().unsurfaced_gaps,
        mine.unsurfaced_gaps,
        "scoping must not make the design's own gap count smaller"
    );
    assert!(
        g.loop_status().unwrap().gaps_on_owned_ground.is_empty(),
        "unscoped, there is no person to attribute to"
    );
}

/// Ownership must not propagate. Owning something says who ANSWERS for it, not
/// that a change to it changes them — the same exclusion `AUTHORED_BY` and
/// `CLAIMS` already carry, and this is the third of the kind.
#[test]
fn owning_something_does_not_drag_a_person_into_a_blast_radius() {
    use reflow2_core::propagate::PropagateOptions;

    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:a", "A", Some("person"), Some("a"), None)
        .unwrap();
    g.add_capability("cap:a", "A", "does a thing", None)
        .unwrap();
    g.owned_by("Capability", "cap:a", "who:a", None, None)
        .unwrap();

    let blast = g
        .propagate_from(&["cap:a"], PropagateOptions { max_depth: 5 })
        .unwrap();
    assert!(
        !blast.impacted.iter().any(|i| i.node_id == "who:a"),
        "a Contributor must never appear in a blast radius: {:?}",
        blast
            .impacted
            .iter()
            .map(|i| &i.node_id)
            .collect::<Vec<_>>()
    );
}
