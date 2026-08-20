//! What counts as DELIVERED, when the deliverable is not a file.
//!
//! **Found by running the loop honestly rather than by looking for it**
//! (2026-08-20). `epoch:the-declared-walls-hold` was set arrived with both of
//! its capabilities genuinely delivered — the kernel re-placed, a module
//! dependency removed, a passing check over both — and `arrival_delta`
//! reported BOTH as `outstanding`. Delivery required a file on disk, and model
//! work produces none.
//!
//! `outstanding` is defined as *"still pointed here, not delivered, and NOBODY
//! HAS SAID which of deferred or discontinued it is — that is the question to
//! put to the user"*. So the tool asked a question about finished work, and
//! would ask it again on every run. Re-decompositions, retirements and
//! governance rulings would pile up as phantom incompletions until somebody
//! stopped scheduling that kind of work — which is most of what systems
//! engineering is.
//!
//! ⭐ THE FIX DECLARES THE KIND, NEVER THE SUCCESS. `Capability.delivery` is
//! `artifact` (default) or `model`, and **both still demand a passing check**.
//! Nothing became assertable: there is still no way to mark something done, and
//! delivery is still computed from the golden thread.
//!
//! 🛑 THE REJECTED FIX HAS ITS OWN TEST, because it is the one a later reader
//! will be tempted by: inferring `model` from the ABSENCE of an artifact. The
//! commonest reason a capability has no file is that NOBODY HAS BUILT IT YET,
//! so that rule reports unbuilt work as delivered the moment a check is
//! attached — a false green in the dangerous direction.
//! `an_unbuilt_capability_with_a_passing_check_is_not_delivered` pins it.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::temporal::{EpochType, ScheduleOutcome};

/// An arrived epoch with one capability scheduled into it.
fn scheduled(delivery: Option<&str>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Thing").expect("project");
    g.add_capability("cap:x", "A thing", "does something", None)
        .expect("capability");
    if let Some(kind) = delivery {
        g.set_capability_delivery("cap:x", kind).expect("delivery");
    }
    g.plan_epoch("epoch:1", "An increment", EpochType::Milestone, 1)
        .expect("epoch");
    g.schedule_for(
        node::CAPABILITY,
        "cap:x",
        node::DESIGN_EPOCH,
        "epoch:1",
        "expected",
        None,
    )
    .expect("schedule");
    g.set_epoch_status("epoch:1", "arrived").expect("arrived");
    g
}

/// A passing check over the capability — the evidence half, which BOTH kinds need.
fn passing_check(g: &mut DesignGraph) {
    g.add_verification("ver:x", "It works", None, None, None)
        .expect("verification");
    g.create_edge(
        edge::VERIFIES,
        node::VERIFICATION,
        "ver:x",
        node::CAPABILITY,
        "cap:x",
        Props::new(),
    )
    .expect("verifies");
    g.set_verification_status("ver:x", "passing", None, None)
        .expect("passing");
}

/// A real file realizing it — the artifact half.
fn realizing_artifact(g: &mut DesignGraph) {
    g.add_artifact("art:x", "x.rs", Some("code"), Some("src/x.rs"))
        .expect("artifact");
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:x",
        node::CAPABILITY,
        "cap:x",
        Props::new(),
    )
    .expect("realizes");
}

fn outcome(g: &DesignGraph) -> ScheduleOutcome {
    let delta = g.arrival_delta("epoch:1").expect("delta");
    delta
        .items
        .first()
        .expect("one scheduled item")
        .outcome
        .clone()
}

#[test]
fn model_work_with_a_passing_check_is_delivered() {
    // THE CASE THAT STARTED THIS. No file exists and none ever will — the
    // deliverable was the design change itself.
    let mut g = scheduled(Some("model"));
    passing_check(&mut g);
    assert_eq!(outcome(&g), ScheduleOutcome::Delivered);
}

#[test]
fn model_work_without_a_check_is_not_delivered() {
    // The declaration loosens WHICH evidence is required, never WHETHER any is.
    // Without this, `delivery: model` would be a way to mark something done —
    // exactly the assertion the whole design refuses.
    let mut g = scheduled(Some("model"));
    let _ = &mut g;
    assert_ne!(outcome(&g), ScheduleOutcome::Delivered);
}

#[test]
fn an_unbuilt_capability_with_a_passing_check_is_not_delivered() {
    // 🛑 THE REJECTED FIX, pinned so nobody re-derives it. Inferring "model"
    // from a missing artifact would make THIS delivered — a capability nobody
    // built, that merely has a check pointed at it. The commonest reason there
    // is no file is that the work has not happened.
    let mut g = scheduled(None); // no declaration → `artifact`, the default
    passing_check(&mut g);
    assert_ne!(
        outcome(&g),
        ScheduleOutcome::Delivered,
        "a passing check must not stand in for an artifact unless the author DECLARED \
         that no artifact was ever coming"
    );
}

#[test]
fn an_artifact_capability_still_needs_both_halves() {
    // The default path is unchanged, which is most of every design.
    let mut g = scheduled(Some("artifact"));
    realizing_artifact(&mut g);
    assert_ne!(outcome(&g), ScheduleOutcome::Delivered, "file but no check");

    let mut g = scheduled(Some("artifact"));
    realizing_artifact(&mut g);
    passing_check(&mut g);
    assert_eq!(outcome(&g), ScheduleOutcome::Delivered, "file and check");
}

#[test]
fn an_undeclared_capability_keeps_the_stricter_rule() {
    // Absent reads as `artifact`, the schema default. Every capability written
    // before this property existed keeps the rule it was written under rather
    // than quietly loosening — a silent loosening would re-report a backlog of
    // unbuilt work as delivered on the day of the upgrade.
    let mut g = scheduled(None);
    passing_check(&mut g);
    realizing_artifact(&mut g);
    assert_eq!(outcome(&g), ScheduleOutcome::Delivered);
}

#[test]
fn a_delivery_kind_that_is_not_one_is_refused_and_names_the_legal_values() {
    let mut g = scheduled(None);
    let err = g
        .set_capability_delivery("cap:x", "modelled")
        .expect_err("refused");
    let text = format!("{err}");
    assert!(text.contains("artifact"), "{text}");
    assert!(text.contains("model"), "{text}");
}

#[test]
fn declaring_delivery_on_a_capability_that_does_not_exist_is_refused() {
    let mut g = scheduled(None);
    let err = g
        .set_capability_delivery("cap:nope", "model")
        .expect_err("refused");
    assert!(format!("{err}").contains("cap:nope"), "{err}");
}

#[test]
fn declaring_delivery_preserves_every_other_property() {
    // The setter rebuilds the property bag; a capability's status, description
    // and provenance must survive it. This is the BL-183 shape — a sharpening
    // write that silently unbuilds a verified capability.
    let mut g = scheduled(None);
    g.set_capability_status("cap:x", "realized")
        .expect("status");
    g.set_capability_delivery("cap:x", "model")
        .expect("delivery");

    let n = g
        .get_node(node::CAPABILITY, "cap:x")
        .expect("get")
        .expect("present");
    assert_eq!(
        n.properties.get("status").and_then(|v| v.as_str()),
        Some("realized")
    );
    assert_eq!(
        n.properties.get("delivery").and_then(|v| v.as_str()),
        Some("model")
    );
    assert_eq!(
        n.properties.get("name").and_then(|v| v.as_str()),
        Some("A thing")
    );
}

#[test]
fn an_outstanding_item_with_a_check_but_no_file_names_the_remedy() {
    // The finding that started this was not that the rule was wrong — it was
    // that `outstanding` asks "deferred or dropped?" about finished work, and
    // a reader has no way to learn that a declaration would fix it. Fixing the
    // rule without saying so anywhere would leave the mechanism built, tested
    // and unreachable from where the person is stuck — which was the same
    // failure four separate times on 2026-08-20.
    let mut g = scheduled(None);
    passing_check(&mut g);

    let delta = g.arrival_delta("epoch:1").expect("delta");
    assert_eq!(delta.items[0].outcome, ScheduleOutcome::Outstanding);

    let note = delta
        .notes
        .iter()
        .find(|n| n.contains("set_capability_delivery"))
        .expect("the remedy must be named where the outcome is reported");
    assert!(note.contains("cap:x"), "{note}");
    // And it must not accuse: unbuilt work is a real possibility and the note
    // says so rather than assuming the reader is in the other case.
    assert!(note.contains("simply unbuilt"), "{note}");
}

#[test]
fn an_outstanding_item_with_no_check_at_all_gets_no_such_note() {
    // Nothing has been checked, so there is no evidence to compute delivery
    // from under EITHER kind. Declaring `model` would not help, and suggesting
    // it would be noise — the note has to be silent where it is not the answer.
    let g = scheduled(None);

    let delta = g.arrival_delta("epoch:1").expect("delta");
    assert_eq!(delta.items[0].outcome, ScheduleOutcome::Outstanding);
    assert!(
        !delta
            .notes
            .iter()
            .any(|n| n.contains("set_capability_delivery")),
        "{:?}",
        delta.notes
    );
}
