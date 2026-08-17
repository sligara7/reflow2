//! `what_next` — a bounded, deliberately approximate guide to which decisions
//! to settle next, for a design holding more open questions than anyone can
//! hold at once.
//!
//! Anthony, 2026-08-10: *"many projects have hundreds of decisions and a person
//! can only make a small subset of decisions a day… having something that says,
//! these are your top 5 priorities would be great"*, and — crucially — *"this
//! doesn't have to be an exact 'what is the most pressing decision'. getting
//! something in the general ballpark … is better than an equal set of 60+
//! decisions."* The acceptance criterion is therefore ROUGH, and the baseline
//! being beaten is no ordering at all.
//!
//! ## What these cases pin, and why each one would otherwise rot
//!
//! **The user's word is never reordered by a heuristic.** A `proposed` Decision
//! carrying `AUTHORED_BY role=approver` is durable self-prioritisation that
//! already existed and was simply not named as such. If a future scorer could
//! outrank it, the tool would be overruling the one signal it must not.
//!
//! **Ranking covers only what is NOT marked.** Ranking somebody's own marks
//! back at them says nothing they do not know, so the marked set is excluded
//! from `ranked` even when it would score highest.
//!
//! **The exploration slot is required, not decorative.** Every signal here is
//! built on connectedness, so a decision nothing points at scores zero forever
//! and is never surfaced — `dec:bl-155`'s unused-versus-unreachable trap
//! arriving through a ranking instead of a usage count. `unexplored` is the
//! exploration term `dec:attention-is-measured-behaviourally-not-lexically`
//! names as mandatory. **A test that only checked the ranking would pass while
//! the fifth slot was quietly dropped as over-engineering, which is exactly the
//! outcome the two independent derivations of it exist to prevent.**
//!
//! **`alignment: supporting` is corroboration, not conflict.** heal.rs already
//! paid for this once: reading the edge TYPE and never the property turned
//! every correctly-recorded corroboration into a structural defect. The same
//! mistake here would inflate the score of decisions that agree with the design.
//!
//! **Review records are excluded throughout.** `decision:ack:` nodes dominate
//! raw `GOVERNED_BY` counts — measured on reflow2's own design, excluding them
//! took that edge type from 867 to 255 — so leaving them in does not add noise,
//! it makes the answer confidently wrong.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, EpochType};

/// A minimal design with one contributor to assign decisions to.
fn base() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_contributor("who:ajs", "Anthony", Some("person"), None, None)
        .unwrap();
    g
}

/// Add a `proposed` Decision — the shape a brainstorm lands in.
fn open_decision(g: &mut DesignGraph, id: &str, name: &str) {
    g.add_decision(id, name, "an open question", None).unwrap();
    g.set_decision_status(id, "proposed").unwrap();
}

fn ranked_ids(w: &reflow2_core::WhatNext) -> Vec<String> {
    w.ranked.iter().map(|r| r.decision_id.clone()).collect()
}

/// THE ONE THAT MUST NEVER REGRESS: what the user marked is their word, and no
/// computed score reorders it or demotes it into the ranked band.
#[test]
fn a_marked_decision_is_never_ranked_however_it_would_score() {
    let mut g = base();
    open_decision(&mut g, "dec:marked", "I will settle this");
    g.authored_by(
        node::DECISION,
        "dec:marked",
        "who:ajs",
        Some("approver"),
        None,
    )
    .unwrap();

    // Give it every scoring signal available, so if the bands ever leaked it
    // would land at the top of `ranked` rather than merely appearing there.
    g.add_requirement("req:a", "A", "a").unwrap();
    g.add_requirement("req:b", "B", "b").unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:a",
        node::DECISION,
        "dec:marked",
        None,
    )
    .unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:b",
        node::DECISION,
        "dec:marked",
        None,
    )
    .unwrap();

    let w = g.what_next(4).unwrap();
    assert_eq!(w.marked.len(), 1, "the marked decision belongs in `marked`");
    assert_eq!(w.marked[0].decision_id, "dec:marked");
    assert_eq!(w.marked[0].approver_id, "who:ajs");
    assert!(
        !ranked_ids(&w).contains(&"dec:marked".to_string()),
        "a marked decision must never also appear in `ranked` — ranking somebody's \
         own marks back at them says nothing they do not know"
    );
}

/// The three signals are additive and order the head of the list. Deliberately
/// asserts the ORDER, not the absolute numbers: the weights are a guide and may
/// be tuned, but blocking scheduled work must keep outranking governing alone.
#[test]
fn the_score_is_additive_over_governed_scheduled_and_contradicted() {
    let mut g = base();
    open_decision(&mut g, "dec:governs-one", "governs one thing");
    open_decision(&mut g, "dec:blocks-plan", "blocks scheduled work");
    open_decision(&mut g, "dec:conflicts", "conflicts with a settled choice");

    // +1: one requirement waits on it.
    g.add_requirement("req:plain", "Plain", "p").unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:plain",
        node::DECISION,
        "dec:governs-one",
        None,
    )
    .unwrap();

    // +1 +2: the requirement waiting on it is scheduled into an increment.
    g.add_requirement("req:sched", "Scheduled", "s").unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:sched",
        node::DECISION,
        "dec:blocks-plan",
        None,
    )
    .unwrap();
    g.plan_epoch("epoch:next", "Next increment", EpochType::Milestone, 1)
        .unwrap();
    g.schedule_for(
        node::REQUIREMENT,
        "req:sched",
        node::DESIGN_EPOCH,
        "epoch:next",
        "expected",
        None,
    )
    .unwrap();

    // +2: it contradicts something already accepted, so the design is
    // internally inconsistent until somebody settles it.
    g.add_decision("dec:settled", "Settled", "we chose this", None)
        .unwrap();
    g.set_decision_status("dec:settled", "accepted").unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:conflicts",
        node::DECISION,
        "dec:settled",
        Props::new(),
    )
    .unwrap();

    let w = g.what_next(4).unwrap();
    let ids = ranked_ids(&w);
    assert_eq!(
        ids,
        vec![
            "dec:blocks-plan".to_string(),
            "dec:conflicts".to_string(),
            "dec:governs-one".to_string()
        ],
        "blocking planned work (3) outranks a bare contradiction (2), which \
         outranks governing one thing (1)"
    );
    assert!(
        w.ranked[0].because.iter().any(|r| r.contains("scheduled")),
        "the reason must NAME what raised it — a ranking nobody can argue with \
         is a ranking nobody can correct"
    );
}

/// heal.rs's scar, reproduced here before it could be re-earned: a `supporting`
/// CONTRADICTS is corroboration, and must not raise the score.
#[test]
fn a_supporting_contradicts_edge_is_corroboration_and_does_not_score() {
    let mut g = base();
    open_decision(&mut g, "dec:agrees", "agrees with a settled choice");
    g.add_decision("dec:settled", "Settled", "we chose this", None)
        .unwrap();
    g.set_decision_status("dec:settled", "accepted").unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::DECISION,
        "dec:agrees",
        node::DECISION,
        "dec:settled",
        Props::new().set("alignment", "supporting"),
    )
    .unwrap();

    let w = g.what_next(4).unwrap();
    assert!(
        ranked_ids(&w).is_empty(),
        "corroboration must not read as conflict"
    );
    assert_eq!(
        w.unranked_pool, 1,
        "it scores zero and belongs in the exploration pool"
    );
}

/// THE EXPLORATION TERM. A decision nothing points at scores zero forever; if
/// the bottom slot is ever dropped, this fails.
#[test]
fn a_decision_nothing_points_at_is_still_surfaced() {
    let mut g = base();
    open_decision(&mut g, "dec:lonely", "nothing points at this yet");

    let w = g.what_next(4).unwrap();
    assert!(
        ranked_ids(&w).is_empty(),
        "it scores zero, so it cannot be in the ranked band"
    );
    let u = w.unexplored.as_ref().expect(
        "the exploration slot must surface it — a ranking built on \
                 connectedness would otherwise bury it permanently",
    );
    assert_eq!(u.decision_id, "dec:lonely");
    assert!(
        u.because.is_empty(),
        "the unexplored draw has no reasons by construction; that is the point"
    );
    assert_eq!(w.unranked_pool, 1);
    assert!(
        w.note.contains("not the least important"),
        "the answer must SAY the bottom slot is a deliberate sample, or a reader \
         will take it for a wooden spoon"
    );
}

/// Review records are judgements ABOUT the design, not part of it — excluded
/// both as decisions and as things that count toward a score.
#[test]
fn review_records_are_excluded_as_decisions_and_as_governed_sources() {
    let mut g = base();
    open_decision(&mut g, "dec:real", "a real open question");

    // A review record that is itself `proposed` must not appear as an open
    // decision...
    g.add_decision("decision:ack:gap1", "Reviewed: gap1", "accepted", None)
        .unwrap();
    g.set_decision_status("decision:ack:gap1", "proposed")
        .unwrap();
    // ...and must not inflate the real decision's score by governing it.
    g.governed_by(
        node::DECISION,
        "decision:ack:gap1",
        node::DECISION,
        "dec:real",
        None,
    )
    .unwrap();

    let w = g.what_next(4).unwrap();
    assert_eq!(
        w.open_total, 1,
        "the review record is not an open decision the user must settle"
    );
    assert!(
        ranked_ids(&w).is_empty(),
        "a review record governing it must not raise its score above zero"
    );
}

/// Two readings of an unchanged design agree, and the slot MOVES when a
/// decision is settled — the rotation is derived from graph state because the
/// core takes no clock and no RNG.
#[test]
fn the_unexplored_pick_is_stable_and_advances_when_a_decision_is_settled() {
    let mut g = base();
    for i in 0..4 {
        open_decision(&mut g, &format!("dec:pool-{i}"), &format!("open {i}"));
    }

    let first = g.what_next(4).unwrap();
    let again = g.what_next(4).unwrap();
    assert_eq!(
        first.unexplored.as_ref().unwrap().decision_id,
        again.unexplored.as_ref().unwrap().decision_id,
        "an unchanged design must read the same twice"
    );

    // Settling something advances the rotation, so the next reading offers a
    // different unexplored decision rather than re-showing the same one.
    g.add_decision("dec:now-settled", "Settled", "chosen", None)
        .unwrap();
    g.set_decision_status("dec:now-settled", "accepted")
        .unwrap();
    let after = g.what_next(4).unwrap();
    assert_eq!(after.rotation, first.rotation + 1);
    assert_ne!(
        first.unexplored.as_ref().unwrap().decision_id,
        after.unexplored.as_ref().unwrap().decision_id,
        "make a decision, see a different unexplored one — that is the cadence \
         the rotation is derived to produce"
    );
}

/// A five-item answer must never read as the whole set.
#[test]
fn everything_not_displayed_is_counted() {
    let mut g = base();
    for i in 0..10 {
        open_decision(&mut g, &format!("dec:o-{i}"), &format!("open {i}"));
    }
    let w = g.what_next(4).unwrap();
    let shown = w.marked.len() + w.ranked.len() + usize::from(w.unexplored.is_some());
    assert_eq!(w.open_total, 10);
    assert_eq!(
        w.not_shown,
        10 - shown,
        "`not_shown` must account for every open decision the answer omitted"
    );
}

// ---------------------------------------------------------------------------
// Band four — which few decisions shape everything else.
//
// `dec:orientation-is-computed-not-written` named this as the one genuinely
// missing thing a newcomer needs, and insisted it be a COMPUTATION rather than
// another paragraph in a start-here file. `cap:what-next` then recorded the trap
// BEFORE anything was built: raw `GOVERNED_BY` in-degree's top hit was a pruning
// decision governing ten DROPPED requirements — high-degree, and the last thing
// a newcomer should read. The first case below IS that trap, as a fixture.
// ---------------------------------------------------------------------------

/// THE RECORDED CAVEAT, REPRODUCED: a decision whose whole footprint is retired
/// must not lead the list, however high its raw degree.
#[test]
fn a_decision_governing_only_retired_work_does_not_shape_anything() {
    let mut g = base();
    g.add_decision(
        "dec:pruning",
        "Nine proposed requirements not accepted",
        "we dropped them",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:pruning", "accepted").unwrap();
    for i in 0..9 {
        let r = format!("req:dropped-{i}");
        g.add_requirement(&r, "Dropped", "no").unwrap();
        g.set_requirement_status(&r, "dropped").unwrap();
        g.governed_by(node::REQUIREMENT, &r, node::DECISION, "dec:pruning", None)
            .unwrap();
    }
    // A genuinely shaping decision with a SMALLER raw degree.
    g.add_decision(
        "dec:shapes",
        "Systems are functional",
        "not the file tree",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:shapes", "accepted").unwrap();
    for i in 0..3 {
        let c = format!("cmp:part-{i}");
        g.add_component(&c, "Part", "a part", None).unwrap();
        g.governed_by(node::COMPONENT, &c, node::DECISION, "dec:shapes", None)
            .unwrap();
    }

    let w = g.what_next(4).unwrap();
    let top = w.shaping.first().expect("something shapes the design");
    assert_eq!(
        top.decision_id, "dec:shapes",
        "raw in-degree would put the pruning decision first (9 > 3); liveness is \
         the whole refinement and this is the case that proves it"
    );
    let pruning = w.shaping.iter().find(|s| s.decision_id == "dec:pruning");
    assert!(
        pruning.is_none(),
        "a decision governing only retired work shapes nothing live and must drop out"
    );
}

/// Retired work is COUNTED and reported, never silently dropped — a decision
/// half of whose footprint was pruned is still a fact the reader may want.
#[test]
fn retired_governed_nodes_are_reported_rather_than_hidden() {
    let mut g = base();
    g.add_decision("dec:mixed", "Half pruned", "some stuck", None)
        .unwrap();
    g.set_decision_status("dec:mixed", "accepted").unwrap();
    g.add_requirement("req:live", "Live", "yes").unwrap();
    g.add_requirement("req:gone", "Gone", "no").unwrap();
    g.set_requirement_status("req:gone", "dropped").unwrap();
    for r in ["req:live", "req:gone"] {
        g.governed_by(node::REQUIREMENT, r, node::DECISION, "dec:mixed", None)
            .unwrap();
    }

    let s = &g.what_next(4).unwrap().shaping[0];
    assert_eq!(s.governs_live, 1);
    assert_eq!(
        s.governs_retired, 1,
        "the pruned half is reported, not hidden"
    );
}

/// BREADTH WAS TRIED AND REJECTED ON MEASUREMENT. Weighting by how many distinct
/// node types a decision governs demotes exactly the exemplars `cap:what-next`
/// names as the right sort of thing — functional decomposition governs eight
/// Components, the three-party division seven Capabilities: one type each.
/// `shapes` is reported for the reader and must stay OUT of the ranking.
#[test]
fn breadth_of_governed_types_is_reported_but_never_ranks() {
    let mut g = base();
    // Deep and single-typed — the shape of the real exemplars.
    g.add_decision("dec:deep", "Governs many of one kind", "d", None)
        .unwrap();
    g.set_decision_status("dec:deep", "accepted").unwrap();
    for i in 0..4 {
        let c = format!("cmp:deep-{i}");
        g.add_component(&c, "C", "c", None).unwrap();
        g.governed_by(node::COMPONENT, &c, node::DECISION, "dec:deep", None)
            .unwrap();
    }
    // Broad but shallow — two types, fewer nodes.
    g.add_decision("dec:broad", "Governs a couple of kinds", "b", None)
        .unwrap();
    g.set_decision_status("dec:broad", "accepted").unwrap();
    g.add_requirement("req:b", "B", "b").unwrap();
    g.add_capability("cap:b", "Cap B", "b", None).unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:b",
        node::DECISION,
        "dec:broad",
        None,
    )
    .unwrap();
    g.governed_by(node::CAPABILITY, "cap:b", node::DECISION, "dec:broad", None)
        .unwrap();

    let w = g.what_next(4).unwrap();
    assert_eq!(
        w.shaping[0].decision_id, "dec:deep",
        "governing four Components IS shaping — single-typedness is not shallowness, \
         and a breadth weighting would invert this"
    );
    assert_eq!(w.shaping[0].shapes, vec!["Component".to_string()]);
    assert_eq!(
        w.shaping[1].shapes,
        vec!["Capability".to_string(), "Requirement".to_string()],
        "breadth is still REPORTED, for the reader to use"
    );
}

/// The shaping band answers a different question from the other three: it holds
/// SETTLED decisions that need nothing from anybody, and must never be confused
/// with the to-do list.
#[test]
fn the_shaping_band_holds_settled_decisions_not_open_ones() {
    let mut g = base();
    open_decision(&mut g, "dec:still-open", "not settled");
    g.add_requirement("req:x", "X", "x").unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:x",
        node::DECISION,
        "dec:still-open",
        None,
    )
    .unwrap();

    let w = g.what_next(4).unwrap();
    assert!(
        w.shaping.is_empty(),
        "an OPEN decision is a to-do, not an explanation of why the design looks \
         as it does — it belongs in `ranked`, and it is there"
    );
    assert_eq!(ranked_ids(&w), vec!["dec:still-open".to_string()]);
}
