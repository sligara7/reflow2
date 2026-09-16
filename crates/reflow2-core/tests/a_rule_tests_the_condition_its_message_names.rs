//! Two repair rules now compute the condition their own message names.
//!
//! `contradiction` has always been documented as "two nodes joined by
//! CONTRADICTS with NO RESOLVING DECISION" and has always suggested
//! `generate_decision` as the fix — and until 2026-09-15 it never looked for
//! one. Proved by execution on 2026-08-08: a decision was written, accepted and
//! edged GOVERNED_BY from both endpoints, and `detect_defects` returned the same
//! issue id unchanged. The only exits were to acknowledge a resolved
//! disagreement as if it were unresolved, or to delete the edge and erase that
//! it happened (`fact:defect-a-contradiction-cannot-be-resolved-only-acknowledged`).
//!
//! `unresolved_setup` says "X anticipates Y but nothing follows through" and,
//! until the same day, never asked whether anything had. It exempted parked
//! ideas and reported the rest, so the moment an anticipated idea was SETTLED it
//! stopped being exempt and became a defect — the rule punished answering the
//! question (`fact:unresolved-setup-says-nothing-follows-through-and-never-checks-whether-anything-did`,
//! walked into on 2026-09-02).
//!
//! Both are one class — `fact:two-heal-rules-assert-a-condition-they-never-test`
//! — and the class's own recorded lesson, from the `alignment` fix sixty lines
//! above in the same function, is "a property the detector ignores is a
//! property that lies". A CONDITION the detector never computes lies the same
//! way.
//!
//! # The two definitions, on the owner's word (2026-09-15)
//!
//! - A contradiction is RESOLVED by an ACCEPTED Decision that BOTH endpoints
//!   are GOVERNED_BY. Both, because a decision on one side has not ruled on the
//!   disagreement; accepted, because a `proposed` one is thinking out loud.
//! - An anticipation is FOLLOWED THROUGH when its target EVOLVES_INTO
//!   something, or has arrived on its own type's terms: a Decision `accepted`,
//!   a Capability or Component `realized`/`verified`, a Requirement `met`, an
//!   epoch `arrived`.
//!
//! Each definition is pinned from both sides — the case it clears and the
//! nearest case it must NOT clear — because answering either question too
//! generously converts an over-firing rule into an under-firing one, which is
//! worse.

use std::collections::HashMap;

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::temporal::{ChangeSubject, ChangeType};
use reflow2_core::{DesignGraph, HealCategory, Value};

fn decision(g: &mut DesignGraph, id: &str, status: &str, kind: Option<&str>) {
    let mut props = Props::new()
        .set("name", id)
        .set("decision", "something")
        .set("status", status);
    if let Some(k) = kind {
        props = props.set("kind", k);
    }
    g.create_node(node::DECISION, id, props).unwrap();
}

fn link(
    g: &mut DesignGraph,
    edge_type: &str,
    from_type: &str,
    from: &str,
    to_type: &str,
    to: &str,
) {
    g.create_edge(
        edge_type,
        from_type,
        from,
        to_type,
        to,
        HashMap::<String, Value>::new(),
    )
    .unwrap();
}

fn base() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
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

/// Two accepted decisions in conflict, and a third decision ruling on it.
fn contradiction_with_ruling(ruling_status: &str, governs_both: bool) -> DesignGraph {
    let mut g = base();
    decision(&mut g, "dec:one", "accepted", None);
    decision(&mut g, "dec:two", "accepted", None);
    decision(&mut g, "dec:ruling", ruling_status, None);
    link(
        &mut g,
        edge::CONTRADICTS,
        node::DECISION,
        "dec:one",
        node::DECISION,
        "dec:two",
    );
    link(
        &mut g,
        edge::GOVERNED_BY,
        node::DECISION,
        "dec:one",
        node::DECISION,
        "dec:ruling",
    );
    if governs_both {
        link(
            &mut g,
            edge::GOVERNED_BY,
            node::DECISION,
            "dec:two",
            node::DECISION,
            "dec:ruling",
        );
    }
    g
}

#[test]
fn an_accepted_decision_governing_both_endpoints_resolves_the_contradiction() {
    let g = contradiction_with_ruling("accepted", true);
    assert!(
        defects(&g, HealCategory::Contradiction).is_empty(),
        "the resolving Decision the rule asks for was written, accepted, and drawn from both \
         endpoints — the finding must clear, or the suggested fix is a lie: {:?}",
        defects(&g, HealCategory::Contradiction)
    );
}

#[test]
fn the_contradicts_edge_survives_its_resolution() {
    let g = contradiction_with_ruling("accepted", true);
    assert_eq!(
        g.outgoing("dec:one", Some(edge::CONTRADICTS))
            .unwrap()
            .len(),
        1,
        "resolving a contradiction must not erase that it happened"
    );
}

#[test]
fn a_proposed_decision_resolves_nothing() {
    let g = contradiction_with_ruling("proposed", true);
    assert_eq!(
        defects(&g, HealCategory::Contradiction).len(),
        1,
        "a `proposed` Decision is somebody thinking out loud; it must not silence a live conflict"
    );
}

#[test]
fn a_decision_governing_one_side_has_not_ruled_on_the_disagreement() {
    let g = contradiction_with_ruling("accepted", false);
    assert_eq!(
        defects(&g, HealCategory::Contradiction).len(),
        1,
        "a Decision touching one endpoint is not a ruling on the pair"
    );
}

#[test]
fn a_resolved_contradiction_is_counted_in_the_sweep_not_dropped() {
    let g = contradiction_with_ruling("accepted", true);
    let report = g.detect_defects().unwrap();
    let swept = serde_json::to_value(&report).unwrap();
    let suppressed = swept
        .pointer("/swept/suppressed_by_resolution/contradiction")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    assert_eq!(
        suppressed, 1,
        "a finding cleared by a resolution is reported as such, never silently omitted: {swept}"
    );
}

/// An accepted decision anticipating a target of the given type and status.
fn anticipation(target_type: &str, target_status: &str, kind: Option<&str>) -> DesignGraph {
    let mut g = base();
    decision(&mut g, "dec:setup", "accepted", None);
    match target_type {
        node::DECISION => decision(&mut g, "dec:target", target_status, kind),
        node::CAPABILITY => {
            g.add_capability("cap:target", "Target", "anticipated", None)
                .unwrap();
            g.upsert_node(
                node::CAPABILITY,
                "cap:target",
                Props::new().set("status", target_status),
            )
            .unwrap();
        }
        node::REQUIREMENT => {
            g.add_requirement("req:target", "Target", "anticipated need")
                .unwrap();
            g.upsert_node(
                node::REQUIREMENT,
                "req:target",
                Props::new().set("status", target_status),
            )
            .unwrap();
        }
        other => panic!("no fixture for {other}"),
    }
    let target_id = match target_type {
        node::DECISION => "dec:target",
        node::CAPABILITY => "cap:target",
        _ => "req:target",
    };
    link(
        &mut g,
        edge::ANTICIPATES,
        node::DECISION,
        "dec:setup",
        target_type,
        target_id,
    );
    g
}

#[test]
fn an_anticipated_decision_that_was_accepted_is_followed_through() {
    let g = anticipation(node::DECISION, "accepted", None);
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "the anticipated decision was made; demanding more punishes answering: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn an_anticipated_choice_nobody_made_is_still_an_unresolved_setup() {
    // `kind: choice` + `proposed` is a FACED choice, not a parked musing, so
    // the brainstorm exemption does not apply — and nobody has decided it.
    let g = anticipation(node::DECISION, "proposed", Some("choice"));
    assert_eq!(
        defects(&g, HealCategory::UnresolvedSetup).len(),
        1,
        "a committed setup whose anticipated choice was never made must still be reported"
    );
}

#[test]
fn an_anticipated_capability_that_was_built_is_followed_through() {
    let g = anticipation(node::CAPABILITY, "realized", None);
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "the anticipated capability exists and is built: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn an_anticipated_capability_still_planned_is_an_unresolved_setup() {
    let g = anticipation(node::CAPABILITY, "planned", None);
    assert_eq!(
        defects(&g, HealCategory::UnresolvedSetup).len(),
        1,
        "existing is not following through — the edge already points at it; being BUILT is"
    );
}

#[test]
fn an_anticipated_requirement_that_was_met_is_followed_through() {
    let g = anticipation(node::REQUIREMENT, "met", None);
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "{:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}

#[test]
fn an_accepted_requirement_is_a_need_agreed_not_a_need_met() {
    let g = anticipation(node::REQUIREMENT, "accepted", None);
    assert_eq!(
        defects(&g, HealCategory::UnresolvedSetup).len(),
        1,
        "`accepted` on a Requirement is the user's word that the need is real, not that it was delivered"
    );
}

#[test]
fn a_target_that_evolved_into_recorded_work_is_followed_through() {
    // The exact shape walked into on 2026-09-02: the anticipated idea was
    // settled and EVOLVES_INTO the ChangeEvent that implemented it. Status is
    // left `proposed` here on purpose, so the edge alone has to carry it.
    let mut g = anticipation(node::DECISION, "proposed", Some("choice"));
    g.add_change_event(
        "chg:did-it",
        "did it",
        ChangeType::NewFeature,
        Some(ChangeSubject::System),
        Some("the anticipated thing was built"),
        None,
        Some("2026-09-15"),
    )
    .unwrap();
    link(
        &mut g,
        edge::EVOLVES_INTO,
        node::DECISION,
        "dec:target",
        node::CHANGE_EVENT,
        "chg:did-it",
    );
    assert!(
        defects(&g, HealCategory::UnresolvedSetup).is_empty(),
        "work recorded against the target is the strongest follow-through the graph can hold: {:?}",
        defects(&g, HealCategory::UnresolvedSetup)
    );
}
