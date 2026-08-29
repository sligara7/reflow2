//! A `proposed` Decision asserts nothing, and the defect sweep must read that.
//!
//! FOUND IN THE FIELD, not by review. hxm_program followed the `brainstorm`
//! skill EXACTLY — ideas recorded as `proposed` Decisions, related to each other
//! with `CONTRADICTS` / `ANTICIPATES` / `DEPENDS_ON` — and `detect_defects`
//! reported the `CONTRADICTS` as a contradiction, the `ANTICIPATES` as an
//! unresolved setup, and the linked ideas as an unthreaded cluster. Defects went
//! 2 -> 7 over a day of doing the right thing, and `loop_status` repeated it at
//! every hand-off. reflow2's own session hit the same class the same day.
//!
//! THE ROOT CAUSE IS NOT ANY ONE DETECTOR. The principle is already written down
//! in four places and implemented in one:
//!
//!   - `zero_degree_finding` grades a `proposed` Decision down to `info`, in as
//!     many words — "a parked thought that correctly shapes nothing yet".
//!   - `schema/inference.yaml` on OBSOLETES: "ONLY AN ACCEPTED DECISION
//!     DISCONTINUES ANYTHING; a `proposed` withdrawal has withdrawn nothing".
//!   - `loop_status` stays quiet on a `proposed` Decision with no approver,
//!     because that is somebody thinking out loud.
//!   - the `parks` ruling requires an ACCEPTED Decision, which is why it cannot
//!     reach this case at all.
//!
//! Each detector re-derives the qualifier set by hand, so `contradiction` grew
//! `alignment=supporting` and then `rejected`/`superseded` one incident at a
//! time, and `unresolved_setup` and `unthreaded_cluster` never grew anything.
//! That is the same class as `unallocated_component` never reading a parking
//! ruling though two siblings did, and the same class as F-01's replay contract
//! enforced field by field. `chg:unallocated-component-reads-the-ruling`
//! predicted this one in advance: "nothing checks that a NEW structural detector
//! reads the ruling. The next one written will have the same hole."
//!
//! So the pin is the CLASS, not the three call sites:
//! `every_category_states_whether_it_reads_proposed` is an exhaustive match, and
//! a new `HealCategory` cannot compile until its author has said where it
//! stands. It is the sibling of `every_category_is_listed_in_all`, which exists
//! for the same reason one rule over.
//!
//! ⚠️ THERE IS NO UNLABELLED DECISION. `status` carries `default: proposed`, so
//! silence was never reachable and this is NOT inert on existing designs: every
//! proposed Decision stops producing these findings. That is the intent. The
//! scope counterweight is the node TYPE — see `a_proposed_requirement_still_asserts`.

use std::collections::HashMap;

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory, Value};

/// A decision at an explicit status. `None` writes no `status` property — which
/// is NOT a silence case: the schema defaults it to `proposed`, and
/// `a_decision_has_no_unlabelled_state` is what pins that.
fn decision(g: &mut DesignGraph, id: &str, status: Option<&str>) {
    let mut props = Props::new().set("name", id).set("decision", "something");
    if let Some(s) = status {
        props = props.set("status", s);
    }
    g.create_node(node::DECISION, id, props).unwrap();
}

fn joined(status: Option<&str>, edge_type: &str) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    // A main body, so the pair below is an island rather than the whole design.
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();

    decision(&mut g, "dec:one", status);
    decision(&mut g, "dec:two", status);
    g.create_edge(
        edge_type,
        node::DECISION,
        "dec:one",
        node::DECISION,
        "dec:two",
        HashMap::<String, Value>::new(),
    )
    .unwrap();
    g
}

fn defects(g: &DesignGraph, cat: HealCategory) -> Vec<String> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == cat)
        .map(|d| d.message)
        .collect()
}

// ---- contradiction ------------------------------------------------------

#[test]
fn two_proposed_ideas_that_contradict_are_not_a_defect() {
    // hxm_program's case, exactly: brainstorming records the tension between two
    // ideas, which is what the skill asks for. Neither idea asserts anything, so
    // there is nothing to resolve and no defect to report.
    let g = joined(Some("proposed"), edge::CONTRADICTS);
    assert!(
        defects(&g, HealCategory::Contradiction).is_empty(),
        "a `proposed` Decision asserts nothing, so two of them cannot conflict: {:?}",
        defects(&g, HealCategory::Contradiction)
    );
}

#[test]
fn an_accepted_contradiction_is_still_a_defect() {
    // The load-bearing counterweight. If this silenced the detector outright,
    // real conflicts between settled decisions would vanish — far worse than the
    // noise being fixed.
    let g = joined(Some("accepted"), edge::CONTRADICTS);
    assert_eq!(
        defects(&g, HealCategory::Contradiction).len(),
        1,
        "two ACCEPTED decisions in conflict must still be reported"
    );
}

#[test]
fn a_decision_has_no_unlabelled_state() {
    // 🛑 THE ASSUMPTION THIS TEST EXISTS TO KILL. The first draft of the fix
    // reasoned by analogy with the `alignment` fix — "silence is not a claim, so
    // an unlabelled Decision keeps its historical meaning, so this change is
    // inert on existing designs". ALL THREE CLAUSES ARE FALSE. `status` carries
    // `default: proposed` in schema/structure.yaml, so a Decision written with
    // no status IS proposed, silence was never reachable, and every proposed
    // Decision in every existing design changes behaviour the moment this lands.
    //
    // That is the intended effect, not a side effect — but a reader who assumes
    // the narrower blast radius will mis-review this change, which is why the
    // fact is pinned rather than left in a comment.
    let g = joined(None, edge::CONTRADICTS);
    assert!(
        defects(&g, HealCategory::Contradiction).is_empty(),
        "a Decision written with no status defaults to `proposed` and therefore asserts \
         nothing — if this ever reports, the schema default moved and the blast radius of \
         `asserts_nothing` moved with it: {:?}",
        defects(&g, HealCategory::Contradiction)
    );
}

#[test]
fn a_proposed_requirement_still_asserts() {
    // ⭐ THE SCOPE COUNTERWEIGHT, and the reason `asserts_nothing` checks the
    // node TYPE. `CONTRADICTS` is `from: "*" to: "*"`, and `Requirement.status`
    // ALSO defaults to `proposed` — so a predicate reading `status` on any node
    // would silence conflicts between requirements that are merely awaiting the
    // user's word. reflow2's own design held 51 requirements in exactly that
    // state. A requirement asserted-pending-confirmation claims something real;
    // a brainstormed Decision claims nothing. Only the second is in scope.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_requirement("req:one", "One", "need one").unwrap();
    g.add_requirement("req:two", "Two", "need two").unwrap();
    g.create_edge(
        edge::CONTRADICTS,
        node::REQUIREMENT,
        "req:one",
        node::REQUIREMENT,
        "req:two",
        HashMap::<String, Value>::new(),
    )
    .unwrap();

    assert_eq!(
        defects(&g, HealCategory::Contradiction).len(),
        1,
        "two proposed REQUIREMENTS in conflict are awaiting a decision, not asserting \
         nothing — this must still be reported"
    );
}

#[test]
fn a_parked_idea_against_settled_design_is_still_reported() {
    // 🛑 THE DRAFT THIS TEST REVERSES. The first cut of the fix skipped whenever
    // EITHER side was parked, by analogy with the withdrawn rule. That broke
    // `a_proposed_decision_is_not_treated_as_withdrawn` in tests/heal.rs — an
    // existing, deliberate counterweight whose reasoning is better than the
    // analogy was: "treating 'not yet accepted' as 'no longer intended' would
    // hide exactly the disagreements a brainstorm is supposed to surface."
    //
    // The two findings are not in conflict, because they are different shapes.
    // hxm_program's field case was proposed-vs-PROPOSED, which nothing covered.
    // Against SETTLED design, one parked side still leaves a live question — so
    // a withdrawal (a positive act retiring the conflict) skips on either side,
    // and being unclaimed skips only when neither side claims anything.
    let mut g = joined(Some("accepted"), edge::CONTRADICTS);
    decision(&mut g, "dec:two", Some("proposed"));
    assert_eq!(
        defects(&g, HealCategory::Contradiction).len(),
        1,
        "a parked idea conflicting with an ACCEPTED decision is a real thing to settle"
    );
}

// ---- unresolved_setup ---------------------------------------------------

#[test]
fn anticipation_between_proposed_ideas_is_not_an_unresolved_setup() {
    // "If we did X we would then need Y" is a thought, not a commitment with
    // missing follow-through. This detector had NO endpoint filter at all.
    let g = joined(Some("proposed"), edge::ANTICIPATES);
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "an anticipation between two parked thoughts commits to nothing: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn an_accepted_decision_anticipating_a_parked_idea_is_not_an_unresolved_setup() {
    // 🛑 FOUND BY WALKING INTO IT, twenty minutes after the first cut shipped.
    // Recording a brainstorm exactly as the `brainstorm` skill instructs — step
    // 4, relate the new idea to what is already there — drew an ANTICIPATES from
    // an ACCEPTED Decision to a parked idea, and the sweep reported it. The
    // defect count went 78 -> 80 for following the instructions, which is the
    // very complaint this file exists to answer.
    //
    // WHY EITHER SIDE IS ENOUGH HERE, where `contradiction` needs both. The
    // message is "X anticipates Y but nothing follows through", so the complaint
    // is that Y IS UNRESOLVED. When Y is a parked idea, Y being unresolved is
    // what `proposed` MEANS — the idea IS the follow-through, recorded. And when
    // X is parked, a musing that anticipates something real has committed to no
    // setup at all. Neither direction is a dangling commitment.
    let mut g = joined(Some("accepted"), edge::ANTICIPATES);
    decision(&mut g, "dec:two", Some("proposed"));
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "an accepted decision anticipating a parked idea has its follow-through — the idea: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn a_measurement_anticipating_a_parked_idea_is_not_an_unresolved_setup() {
    // The second case from the same session: a TemporalFact ANTICIPATES an idea
    // the measurement gave rise to. A Fact is not a Decision and can never be
    // parked, so a rule keyed on both endpoints could never reach this — which
    // is how a detector goes on punishing the practice it was just fixed for.
    let mut g = joined(Some("proposed"), edge::ANTICIPATES);
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:measured",
        Props::new()
            .set("name", "a measurement")
            .set("statement", "something was measured")
            .set("subject_id", "dec:one"),
    )
    .unwrap();
    g.create_edge(
        edge::ANTICIPATES,
        node::TEMPORAL_FACT,
        "fact:measured",
        node::DECISION,
        "dec:one",
        HashMap::<String, Value>::new(),
    )
    .unwrap();

    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "a measurement that gave rise to an idea is not a setup with missing follow-through: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn anticipation_between_accepted_decisions_is_still_an_unresolved_setup() {
    // The counterweight: settled work that sets something up and never follows
    // through is exactly what this detector is for.
    let g = joined(Some("accepted"), edge::ANTICIPATES);
    assert_eq!(
        defects(&g, HealCategory::UnresolvedSetup).len(),
        1,
        "an accepted decision that anticipates and never follows through must still be reported"
    );
}

// ---- unthreaded_cluster -------------------------------------------------

#[test]
fn an_island_of_only_proposed_ideas_is_not_an_unthreaded_cluster() {
    // A brainstorm produces exactly this shape: several ideas related to each
    // other and not yet wired to the design. That is what brainstorming IS, and
    // reporting it as an island punishes the skill for working.
    let g = joined(Some("proposed"), edge::DEPENDS_ON);
    assert!(
        defects(&g, HealCategory::UnthreadedCluster).is_empty(),
        "an island of parked thoughts is a brainstorm, not a severed limb: {:?}",
        defects(&g, HealCategory::UnthreadedCluster)
    );
}

#[test]
fn an_island_holding_anything_asserted_is_still_an_unthreaded_cluster() {
    // The counterweight, and the reason the rule is "every member" rather than
    // "any member": one asserting node makes the island a real part of the
    // design that nothing reaches.
    let mut g = joined(Some("proposed"), edge::DEPENDS_ON);
    decision(&mut g, "dec:two", Some("accepted"));
    assert_eq!(
        defects(&g, HealCategory::UnthreadedCluster).len(),
        1,
        "an island containing something that asserts must still be reported"
    );
}

// ---- the sweep says what it did NOT report ------------------------------

#[test]
fn a_suppressed_finding_is_counted_rather_than_silenced() {
    // 🛑 THE DEFECT THIS FIX INTRODUCED, CAUGHT BEFORE IT MERGED. Making three
    // finding kinds read `is_parked_idea` took reflow2's own design from 89 to
    // 61 defects — 28 findings vanished and NOTHING said they had been
    // suppressed, so a reader seeing 61 could not tell it from a design that
    // genuinely has 61.
    //
    // That is exactly the vacuous zero `SweepScope.parked` exists to prevent,
    // reintroduced one rule over BY THE CHANGE THAT WAS FIXING IT. `art:heal`
    // realizes `cap:a-sweep-says-what-it-could-not-have-found`, and the sibling
    // `parks` mechanism COUNTS parked nodes rather than silencing them. This
    // rule now does the same.
    let g = joined(Some("proposed"), edge::CONTRADICTS);
    let sweep = g.detect_defects().unwrap();

    assert!(
        sweep
            .defects
            .iter()
            .all(|d| d.category != HealCategory::Contradiction),
        "the finding must still be suppressed"
    );
    assert_eq!(
        sweep.swept.suppressed_by_parked_idea.get("contradiction"),
        Some(&1),
        "a suppressed finding must be COUNTED, not silently dropped — an auditor \
         reading the defect total has to be able to reconstruct what was left out: {:?}",
        sweep.swept.suppressed_by_parked_idea
    );
}

#[test]
fn a_clean_sweep_reports_nothing_suppressed() {
    // The counterweight, and it is the one that makes the field trustworthy: an
    // EMPTY map must mean "nothing was suppressed" rather than "nobody counted".
    // Without this, a wired-but-never-incremented field reads exactly like a
    // design with no parked ideas in it — which is the failure mode being fixed,
    // wearing the new field's clothes.
    let g = joined(Some("accepted"), edge::CONTRADICTS);
    let sweep = g.detect_defects().unwrap();

    assert_eq!(
        sweep
            .defects
            .iter()
            .filter(|d| d.category == HealCategory::Contradiction)
            .count(),
        1,
        "the accepted pair must still be reported"
    );
    assert!(
        sweep.swept.suppressed_by_parked_idea.is_empty(),
        "nothing was parked here, so nothing may be claimed as suppressed: {:?}",
        sweep.swept.suppressed_by_parked_idea
    );
}

// ---- the class guard ----------------------------------------------------

#[test]
fn every_category_states_whether_it_reads_proposed() {
    // ⭐ THIS IS THE PIN THAT MATTERS. The three fixes above are call sites; this
    // is the class. The match is exhaustive, so a new `HealCategory` will not
    // compile until its author has written down where it stands on a node that
    // asserts nothing — which is precisely what nothing forced last time, and
    // why this bug reached a user.
    //
    // Sibling of `every_category_is_listed_in_all`, one rule over, for the same
    // reason: a rule maintained by hand with nothing checking it is a rule that
    // silently stops being true.
    for cat in HealCategory::ALL {
        let reads_proposed = match cat {
            // Reads it: a finding about whether something CONFLICTS, COMMITS or
            // BELONGS is a finding about an assertion, and a parked thought
            // makes none.
            HealCategory::Contradiction
            | HealCategory::UnresolvedSetup
            | HealCategory::UnthreadedCluster => true,
            // Already reads it, and did from the start — `zero_degree_finding`
            // grades a proposed Decision to `info` rather than dropping it,
            // because "recorded but governing nothing" is worth one quiet line.
            HealCategory::OrphanNode => true,
            // Does NOT read it, deliberately. A DUPLICATES edge is a claim a
            // HUMAN asserted with `basis: asserted` about two nodes covering the
            // same ground; that claim is just as true of two parked thoughts,
            // and the remedy (merge) is just as available.
            HealCategory::Duplicate => false,
            // Do NOT read it, deliberately. These three are topology over the
            // design network — reachability, articulation, cycles. They are
            // statements about the SHAPE the design has, which a node occupies
            // whatever its status claims. `unthreaded_cluster` is the exception
            // above only because an all-parked island is a brainstorm rather
            // than a severed limb.
            HealCategory::SinglePointOfFailure
            | HealCategory::DeadEnd
            | HealCategory::CircularDependency => false,
            // Does NOT read it, and the reason is stronger than for the others.
            // Every category above weighs whether a node's status changes what a
            // finding MEANS. Here it cannot: whether an id resolves is a fact
            // about the graph, not about the carrier's confidence in itself. A
            // `proposed` Decision whose reference names nothing is still a record
            // about something the design does not have, and the same repair —
            // decide what it meant — is just as available. Reading status here
            // would mean a musing could hold a broken pointer indefinitely with
            // nothing saying so, which is how the nine on reflow2's own graph
            // survived in the first place.
            HealCategory::DanglingReference => false,
        };
        // The assertion is not the value — it is that somebody wrote one down.
        // Both answers are legitimate; an unconsidered category is not.
        let _ = reads_proposed;
    }
}
