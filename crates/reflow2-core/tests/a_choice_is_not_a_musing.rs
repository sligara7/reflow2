//! `proposed` is two different states, and the detectors could not tell them apart.
//!
//! # The report
//!
//! ophyd-service, 2026-08-31 (`fact:the-fork-ask-is-a-deferred-branch-whose-reopening-condition-is-now-met`,
//! item ③). They objected that a `CONTRADICTS` from an exploratory idea lands in
//! the defect count, and proposed keying on *"a CONTRADICTS whose source is an
//! OPEN `kind: exploratory` Decision"* rather than on `status` alone.
//!
//! **Their discriminator is better than anything available when the rule was
//! written.** `is_parked_idea` keyed on `status == "proposed"` and nothing else,
//! which cannot tell an IDEA BEING TURNED OVER from a CHOICE somebody actually
//! faced and has not settled. `Decision.kind` is exactly that distinction and it
//! did not exist in August.
//!
//! # 🛑 What this does NOT do, said first because it is the likelier misreading
//!
//! It does not give ophyd what they asked for. Their ask — suppress a
//! contradiction whenever the SOURCE is exploratory, even against a settled
//! decision — would overturn
//! `dec:idea-a-proposed-decision-asserts-nothing-so-what-may-a-structural-detector-say-about-it`,
//! accepted on Anthony's word 2026-08-26, whose own counterweight test records
//! that *"a parked idea that conflicts with an accepted decision is a real thing
//! to settle"*. `proj:bhome`, reporting the same day on the same behaviour,
//! called it CORRECT. Two field reports disagree; the settled rule stands
//! untouched here.
//!
//! What changes is only the PREDICATE's sharpness, in the direction that adds
//! findings rather than silencing them.
//!
//! # The fallback, which is the design
//!
//! `kind` is read when set; `status` is the fallback when it is not. So:
//!
//! * no retro-classification of existing decisions — reading `kind` alone would
//!   have silently unparked every design in the world, because `kind` is unset
//!   on 205 of reflow2's own 225 proposed Decisions. That is the same
//!   reconstruction trap that keeps `lineage` at 0 of 207 and that this project
//!   refused for requirement provenance a day earlier.
//! * strictly sharper from the first write, and sharper still as `kind` fills.
//!
//! ⚠️ MEASURED, AND HONEST ABOUT IT: on reflow2's own design the day this
//! landed, ZERO proposed Decisions carried `kind: choice`, so this moves no
//! finding here at all. It is correct in shape and inert in effect until designs
//! carry the distinction. A change whose whole benefit is deferred should say so
//! rather than be reported as a fix.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

/// Two decisions joined by CONTRADICTS. `contradiction` is the finding that
/// skips only when BOTH ends are parked, so it is the one that shows whether a
/// given node counts as parked.
fn pair(a_kind: Option<&str>, a_status: &str, b_kind: Option<&str>, b_status: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "P").expect("project");
    for (id, kind, status) in [("dec:a", a_kind, a_status), ("dec:b", b_kind, b_status)] {
        g.add_decision(id, id, "a decision", None)
            .expect("decision");
        let mut props = Props::new().set("status", status);
        if let Some(k) = kind {
            props = props.set("kind", k);
        }
        g.upsert_node(node::DECISION, id, props).expect("props");
    }
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:a",
        node::DECISION,
        "dec:b",
        Props::new(),
    )
    .expect("the contradiction");
    g
}

fn contradiction_reported(g: &DesignGraph) -> bool {
    g.detect_defects()
        .expect("sweep")
        .defects
        .iter()
        .any(|i| format!("{:?}", i.category).contains("Contradiction"))
}

/// The unchanged majority, and the reason there is a fallback at all: `kind`
/// unset still means parked, so no existing design has to be reclassified
/// before it behaves as it did yesterday.
#[test]
fn two_unlabelled_proposed_decisions_are_still_both_parked() {
    let g = pair(None, "proposed", None, "proposed");
    assert!(
        !contradiction_reported(&g),
        "with kind unset the predicate must fall back to status, exactly as before"
    );
}

/// The same, said explicitly. An idea being turned over asserts nothing —
/// which is what `exploratory` was added to state.
#[test]
fn two_exploratory_ideas_are_both_parked() {
    let g = pair(
        Some("exploratory"),
        "proposed",
        Some("exploratory"),
        "proposed",
    );
    assert!(
        !contradiction_reported(&g),
        "two ideas that assert nothing cannot contradict each other into a defect"
    );
}

/// ⭐ THE SHARPENING. A `choice` is a decision somebody FACED and has not
/// settled — not a musing. Two of those in conflict is a real thing to settle,
/// and until `kind` existed the detector could not tell it from two half-formed
/// thoughts.
#[test]
fn two_unsettled_choices_are_not_parked_and_the_conflict_is_reported() {
    let g = pair(Some("choice"), "proposed", Some("choice"), "proposed");
    assert!(
        contradiction_reported(&g),
        "a conflict between two choices somebody actually faced must surface — this is the \
         distinction `status` alone could never make"
    );
}

/// 🛑 THE COUNTERWEIGHT THAT MATTERS MOST: `kind` sharpens `status`, it does not
/// replace it. An ACCEPTED decision is settled whatever kind it wears, and if
/// this ever went quiet the predicate would be reading kind INSTEAD of status —
/// which would park settled design and silence real conflicts.
#[test]
fn an_accepted_exploratory_decision_is_not_parked() {
    let g = pair(
        Some("exploratory"),
        "accepted",
        Some("exploratory"),
        "accepted",
    );
    assert!(
        contradiction_reported(&g),
        "accepted is settled whatever the kind says; kind must sharpen status, never override it"
    );
}

/// And the asymmetric case the settled rule turns on, unchanged by this work:
/// one parked side against one settled side still reports, because a parked
/// idea conflicting with settled design is a real thing to settle
/// (`dec:idea-a-proposed-decision-asserts-nothing-so-what-may-a-structural-detector-say-about-it`).
/// This is the assertion ophyd asked to have reversed; it is pinned here so the
/// reversal cannot happen by accident while sharpening the predicate.
#[test]
fn an_idea_against_settled_design_still_reports() {
    let g = pair(Some("exploratory"), "proposed", None, "accepted");
    assert!(
        contradiction_reported(&g),
        "ONE parked side is not enough to silence a contradiction — overturning this needs \
         Anthony's word, not a predicate change"
    );
}
