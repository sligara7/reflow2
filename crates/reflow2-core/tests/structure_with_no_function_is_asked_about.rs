//! `unallocated_component` — leaf Components no Capability is allocated to.
//!
//! The mirror of `unallocated_capability`, and the half of the allocation
//! question nothing asked until 2026-08-25. The two existing detectors are
//! gated in opposite directions — `concept_without_design` fires only at ZERO
//! components and goes silent once a design grows one, `unallocated_capability`
//! stays quiet until a component exists — so a design could carry structure
//! that no function had ever reached and every detector reported clean. On
//! reflow2's own design that was 33 of 95 components.
//!
//! The counterweights matter more than the positives here: a detector that
//! fired on parents would turn every well-formed hierarchy into a finding, and
//! one that fired on a design with nothing to allocate would be asking a
//! question that belongs to an earlier phase.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, GapScope, GapSource};

/// The design under test: `n` leaf components, of which the ones named in
/// `allocated` hold a capability.
fn design_with(components: &[&str], allocated: &[&str]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    for (i, c) in components.iter().enumerate() {
        g.add_component(c, &format!("Part {i}"), "A part", None)
            .unwrap();
    }
    for (i, c) in allocated.iter().enumerate() {
        let cap = format!("cap:{i}");
        g.add_capability(&cap, &format!("Cap {i}"), "Does a thing", None)
            .unwrap();
        g.satisfies(&cap, "req:a").unwrap();
        g.allocate(&cap, c).unwrap();
    }
    g
}

fn finding(g: &DesignGraph) -> Option<reflow2_core::GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::UnallocatedComponent)
}

#[test]
fn a_leaf_component_holding_no_function_is_reported() {
    // cmp:y owns nothing. Before this detector the design reported clean:
    // cap:0 has a home, so `unallocated_capability` is silent, and a component
    // exists, so `concept_without_design` is too.
    let g = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);

    let gap = finding(&g).expect("a leaf component owning no capability is a finding");
    assert_eq!(gap.affected_ids, ["cmp:y"]);
    assert_eq!(gap.scope, GapScope::Component);

    let srcs: Vec<GapSource> = g
        .detect_gaps()
        .unwrap()
        .iter()
        .map(|x| x.gap_source)
        .collect();
    assert!(
        !srcs.contains(&GapSource::UnallocatedCapability),
        "cap:0 is allocated — the mirror detector must stay silent, which is \
         exactly why this one had to exist"
    );
    assert!(!srcs.contains(&GapSource::ConceptWithoutDesign));
}

#[test]
fn an_allocated_leaf_is_not_reported() {
    let g = design_with(&["cmp:x"], &["cmp:x"]);
    assert!(
        finding(&g).is_none(),
        "every leaf holds a function — there is nothing to ask"
    );
}

#[test]
fn a_parent_is_allocated_through_its_children_and_is_not_a_finding() {
    // THE COUNTERWEIGHT THE FILTER EXISTS FOR. `sys:top` deliberately owns no
    // capability directly: its child does, and that is correct modelling, not a
    // hole. Without the leaf filter every well-formed hierarchy becomes a
    // finding — measured on reflow2's own design as 40 components against 33.
    let mut g = design_with(&["sys:top", "cmp:x"], &["cmp:x"]);
    g.contain_component("sys:top", "cmp:x").unwrap();

    assert!(
        finding(&g).is_none(),
        "a component holding child components is allocated through them"
    );
}

#[test]
fn a_parent_whose_children_hold_nothing_is_still_silent_but_the_children_are_not() {
    // The parent is exempt for being a parent; the exemption does not extend
    // downward. A grouping of empty boxes is still a finding about the boxes.
    let mut g = design_with(&["sys:top", "cmp:x", "cmp:y"], &["cmp:y"]);
    g.contain_component("sys:top", "cmp:x").unwrap();

    let gap = finding(&g).expect("the empty child is still asked about");
    assert_eq!(
        gap.affected_ids,
        ["cmp:x"],
        "the parent is excluded, the empty leaf is not"
    );
}

#[test]
fn a_design_with_nothing_to_allocate_is_not_asked() {
    // Components but no capabilities: there IS no allocation to have performed,
    // and asking which function a box holds before the design has said what it
    // does is the wrong question at the wrong phase. That ground belongs to
    // `design_without_intent` / `concept_without_design`.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "X", "Part X", None).unwrap();

    assert!(finding(&g).is_none());
}

#[test]
fn the_finding_says_whether_the_step_was_never_started_or_left_partial() {
    // Two different situations that need two different actions: run the
    // allocation step, versus finish the one you started.
    let never = design_with(&["cmp:x", "cmp:y"], &[]);
    // A capability exists but nothing is allocated anywhere.
    let mut never = never;
    never
        .add_capability("cap:loose", "Loose", "Unowned", None)
        .unwrap();
    let g = finding(&never).expect("nothing allocated at all is a finding");
    assert!(
        g.title.contains("allocation step looks undone"),
        "never-started must say so, got: {}",
        g.title
    );
    assert!(
        g.description
            .contains("Nothing in this design is allocated at all"),
        "and the description must say the same thing, got: {}",
        g.description
    );

    let partial = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);
    let g = finding(&partial).expect("a partial allocation is a finding");
    assert!(
        g.title.contains("1 of 2 parts"),
        "partial must count both sides, got: {}",
        g.title
    );
}

#[test]
fn it_is_one_rollup_not_one_alarm_per_component() {
    // BL-73, the standing anti-flood lesson: reflow2's own design raises 33 of
    // these at once, and a per-node flood is acknowledged in bulk without being
    // read.
    let g = design_with(&["cmp:a", "cmp:b", "cmp:c", "cmp:d", "cmp:x"], &["cmp:x"]);

    let all: Vec<_> = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|x| x.gap_source == GapSource::UnallocatedComponent)
        .collect();
    assert_eq!(all.len(), 1, "one finding, however many empty boxes");
    assert_eq!(all[0].affected_ids.len(), 4);
}

#[test]
fn the_acknowledgement_survives_somebody_adding_a_component() {
    // Aggregate keying. The standing judgement being recorded is "our boxes are
    // namespaces, not functional parts" — a claim about the practice. Per-node
    // keying would expire it on the next write, which is the trap
    // `unvalidated_capability` fell into and was re-acknowledged twenty times
    // for.
    let before = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);
    let id_before = finding(&before).unwrap().id;

    let mut after = before;
    after
        .add_component("cmp:z", "Z", "Another part", None)
        .unwrap();
    let after_gap = finding(&after).unwrap();

    assert_eq!(
        after_gap.affected_ids.len(),
        2,
        "the finding grew — the population really did change"
    );
    assert_eq!(
        id_before, after_gap.id,
        "the gap id must NOT move, or the acknowledgement is expired by an \
         unrelated write"
    );
}

#[test]
fn the_finding_names_the_tool_that_answers_it() {
    // The third leg. A typed tool with no instruction and no
    // detector-that-notices-absence reaches no user's design: `propose_allocation`
    // and `evaluate_allocation` were served, correct, and named in no skill.
    // This detector is where the absence became visible, so it is also where
    // the method gets named.
    let g = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);
    let gap = finding(&g).unwrap();

    assert!(
        gap.description.contains("propose_allocation"),
        "the finding must name the method, got: {}",
        gap.description
    );
    // AND IT MUST NOT PRESENT THAT TOOL AS *THE* METHOD. The quality attribute
    // a system is built for decides which grouping is right, and the four
    // disagree — a clustering answer silently picks performance
    // (`dec:idea-the-ility-chooses-the-allocation-graph`, which measured this
    // and predicted the mistake this finding first made).
    assert!(
        gap.description.contains("BEFORE ALLOCATING"),
        "the ility question must come before the method, got: {}",
        gap.description
    );
    assert!(
        gap.description.contains("silently picks performance"),
        "and the finding must say what happens if it is skipped, got: {}",
        gap.description
    );
    assert!(
        gap.description.contains("acknowledge"),
        "and the honest way out, for a design whose boxes really are namespaces"
    );
}

#[test]
fn a_box_beside_a_process_is_still_asked_what_it_holds() {
    // DELIBERATE, and pinned here so it is a decision rather than an accident.
    // `unallocated_capability` treats `PART_OF_FLOW` as a home — a step of a
    // process is owned by its Flow. The MIRROR question does not inherit that
    // exemption: if a design has drawn boxes AND a process, "which box performs
    // which step?" is a real question, and answering it is what binds the two
    // views together.
    //
    // The escape is the ordinary one: `acknowledge_gap` if the boxes are a
    // deployment view that the process deliberately does not map onto. What
    // must NOT happen is silence, which would read as "your structure is fully
    // allocated" to a design where nothing is.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_component("cmp:x", "X", "Part X", None).unwrap();
    g.add_flow("flow:main", "Main", None, None, None, None)
        .unwrap();
    g.add_capability("cap:step", "Step", "A step", None)
        .unwrap();
    g.satisfies("cap:step", "req:a").unwrap();
    g.part_of_flow("cap:step", "flow:main", Some(1)).unwrap();

    let gaps = g.detect_gaps().unwrap();
    let srcs: Vec<GapSource> = gaps.iter().map(|x| x.gap_source).collect();
    assert!(
        !srcs.contains(&GapSource::UnallocatedCapability),
        "the flow IS the capability's home — the mirror detector stays silent"
    );

    let gap = finding(&g).expect("the empty box is still asked what it holds");
    assert_eq!(gap.affected_ids, ["cmp:x"]);
}

#[test]
fn allocating_only_to_parents_is_not_reported_as_never_having_started() {
    // THE CLAIM MUST MATCH THE MEASUREMENT. `never_started` used to be derived
    // from "every leaf is empty", which is a different statement wearing the
    // same words: this design has allocated its capability — to the parent —
    // and telling its author "nothing in this design is allocated at all"
    // would be false in the one sentence they are most likely to act on.
    //
    // The empty leaf is still a finding. Only the never-started claim is wrong.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "Need A").unwrap();
    g.add_component("sys:top", "Top", "A grouping", None)
        .unwrap();
    g.add_component("cmp:x", "X", "Part X", None).unwrap();
    g.add_component("cmp:y", "Y", "Part Y", None).unwrap();
    g.contain_component("sys:top", "cmp:x").unwrap();
    g.contain_component("sys:top", "cmp:y").unwrap();
    g.add_capability("cap:a", "Cap A", "Does A", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "sys:top").unwrap(); // the PARENT holds it

    let gap = finding(&g).expect("both leaves are empty — still a finding");
    assert_eq!(gap.affected_ids, ["cmp:x", "cmp:y"]);
    assert!(
        !gap.description
            .contains("Nothing in this design is allocated at all"),
        "the parent IS allocated — this design plainly did the step, got: {}",
        gap.description
    );
    assert!(
        gap.title.contains("2 of 2 parts"),
        "it is a partial finding, not a never-started one, got: {}",
        gap.title
    );
}

/// Park `component` under an accepted Decision, the way a ruling reaches a
/// structural detector: the edge carries the claim, so a deliberate state can
/// never be asserted without a Decision that says why.
fn park(g: &mut DesignGraph, component: &str) {
    g.add_decision(
        "dec:surface-holds-no-function",
        "Surface slices hold no function of their own",
        "A slice exposes what the module behind it implements.",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:surface-holds-no-function", "accepted")
        .unwrap();
    g.create_edge(
        edge::GOVERNED_BY,
        node::COMPONENT,
        component,
        node::DECISION,
        "dec:surface-holds-no-function",
        Props::new().set("ruling", "parks"),
    )
    .unwrap();
}

#[test]
fn a_leaf_parked_by_an_accepted_ruling_is_not_a_finding() {
    // THE COUNTERWEIGHT THIS DETECTOR SHIPPED WITHOUT. `unsatisfied_requirement`
    // has read a parking ruling since `req:a-deliberate-state-is-not-a-defect`
    // and `unreviewed_ideas` excludes parked nodes — this one did not, so on
    // reflow2's own design twelve tool-surface slices ruled deliberately empty
    // by an accepted Decision kept reporting as defects. Recording the correct
    // judgement made the instrument worse, which is the exact incentive
    // `dec:reflow2-is-built-for-observability` exists to remove.
    //
    // Nothing silences this incidentally: the detector looks for incoming
    // ALLOCATED_TO, so governance is invisible unless it is READ.
    let mut g = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);
    park(&mut g, "cmp:y");

    assert!(
        finding(&g).is_none(),
        "an accepted ruling declares cmp:y correctly empty — it is not a hole"
    );
}

#[test]
fn a_proposed_ruling_parks_nothing() {
    // A musing must not suppress a finding. `proposed` is somebody thinking out
    // loud, and only the owner's word moves a Decision to `accepted` — so the
    // status check is what keeps parking from becoming a way to silence the
    // instrument by writing a node.
    let mut g = design_with(&["cmp:x", "cmp:y"], &["cmp:x"]);
    g.add_decision("dec:musing", "Maybe empty is fine", "Thinking aloud.", None)
        .unwrap();
    g.create_edge(
        edge::GOVERNED_BY,
        node::COMPONENT,
        "cmp:y",
        node::DECISION,
        "dec:musing",
        Props::new().set("ruling", "parks"),
    )
    .unwrap();

    let gap = finding(&g).expect("a proposed decision has parked nothing");
    assert_eq!(gap.affected_ids, ["cmp:y"]);
}

#[test]
fn parked_leaves_are_counted_in_the_evidence_never_silently_dropped() {
    // COUNTED, NEVER SILENCED — the half that keeps this from being silent
    // truncation. A reader must be able to tell a design with no empty parts
    // from one whose empty parts were ruled deliberate, and the finding is the
    // only place that distinction can reach them.
    let mut g = design_with(&["cmp:x", "cmp:y", "cmp:z"], &["cmp:x"]);
    park(&mut g, "cmp:z");

    let gap = finding(&g).expect("cmp:y is still an unruled hole");
    assert_eq!(
        gap.affected_ids,
        ["cmp:y"],
        "only the unruled leaf is asked"
    );
    assert!(
        gap.evidence.contains("PARKED"),
        "the parked leaf must still be reported, got: {}",
        gap.evidence
    );
    assert!(
        gap.evidence.contains("1 empty leaf"),
        "and the count must be there, got: {}",
        gap.evidence
    );
    assert!(
        gap.title.contains("1 of 3 parts"),
        "the leaf total still counts every leaf; only the finding shrinks, got: {}",
        gap.title
    );
}
