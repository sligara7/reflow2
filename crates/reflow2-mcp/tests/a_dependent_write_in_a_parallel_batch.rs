//! A call that NAMES a node it did not create says so when the node is absent —
//! and names the ordering hazard, because that is the likelier cause.
//!
//! # Why this exists
//!
//! `fact:the-decision-race-is-caller-ordering-not-read-after-write` (2026-08-09)
//! diagnosed this class from a consumer's report: read-after-write already
//! holds — a capture tool takes an exclusive write lock and drops it before
//! responding — so there is no visibility window. What is missing is ORDER.
//! reflow2 serialises ACCESS, not INTENT, so tool calls a harness emits in one
//! parallel batch are unordered and a call that references a node can win the
//! lock before the call that creates it. `Node not found` is then the CORRECT
//! answer, not a bug.
//!
//! That note named the general fix in as many words — *"THE GENERAL FIX IS A
//! DESCRIPTION, NOT A FEATURE: no tool description says that parallel batches
//! carry no ordering"* — and what shipped instead was the INSTANCE fix for one
//! pair (`add_decision` + `set_decision_status`, collapsed into one call).
//!
//! Twenty-one days later the same user hit the same class through a DIFFERENT
//! pair, `add_change_event` + `pin_at_epoch`
//! (`fact:the-parallel-batch-class-recurred-because-only-its-instance-was-fixed`).
//! Collapsing pairs cannot close an open set; the message can.
//!
//! # What this pins, and what it deliberately does not
//!
//! It pins the MESSAGE, not a race. Asserting that a concurrent batch *fails*
//! would be asserting a scheduler outcome, which is flaky by construction and
//! would eventually be deleted for flapping. The property worth holding is that
//! WHICHEVER order the lock grants, the caller is never left unable to tell a
//! no-op from a save — so the concurrent case asserts the outcome is either a
//! real success or the hinted refusal, and never a silent nothing.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn epoch(id: &str) -> AddEpochReq {
    AddEpochReq {
        id: id.into(),
        name: Some("A moment".into()),
        epoch_type: Some("milestone".into()),
        sequence: Some(1),
    }
}

fn event(id: &str) -> AddChangeEventReq {
    AddChangeEventReq {
        id: id.into(),
        name: Some("A change".into()),
        change_type: Some("defect_fix".into()),
        subject: Some("system".into()),
        summary: Some("Something moved and the design moved with it.".into()),
        rationale: None,
        affected: None,
        detected_at: None,
    }
}

fn pin(node_id: &str, epoch_id: &str) -> PinAtEpochReq {
    PinAtEpochReq {
        node_type: "ChangeEvent".into(),
        node_id: node_id.into(),
        epoch_id: epoch_id.into(),
    }
}

/// THE REGRESSION. Before this, the refusal said only "Node not found", which
/// is true and sends the reader looking for a typo — the one cause that is
/// usually NOT what happened when a harness batches its calls.
#[tokio::test]
async fn a_reference_to_an_absent_node_names_the_ordering_hazard() {
    let s = svc().await;
    s.add_epoch(Parameters(epoch("epoch:e")))
        .await
        .expect("epoch");

    let err = s
        .pin_at_epoch(Parameters(pin("chg:never-created", "epoch:e")))
        .await
        .expect_err("pinning an absent ChangeEvent must refuse");
    let msg = format!("{err:?}");

    assert!(
        msg.contains("chg:never-created"),
        "the refusal must still name the node it could not find: {msg}"
    );
    assert!(
        msg.contains("PARALLEL BATCH"),
        "and must name the ordering hazard, which is the likelier cause than a typo: {msg}"
    );
    assert!(
        msg.contains("never created at all"),
        "while still admitting the other reading, because the message cannot tell them \
         apart: {msg}"
    );
}

/// The half that proves the hazard is ORDER and not visibility: sequenced, the
/// identical pair always succeeds.
#[tokio::test]
async fn sequenced_the_same_pair_always_succeeds() {
    let s = svc().await;
    s.add_epoch(Parameters(epoch("epoch:e")))
        .await
        .expect("epoch");
    s.add_change_event(Parameters(event("chg:x")))
        .await
        .expect("create first");
    s.pin_at_epoch(Parameters(pin("chg:x", "epoch:e")))
        .await
        .expect("then reference — read-after-write already holds");
}

/// Concurrent, the outcome may be either — but it is never a silent nothing.
#[tokio::test]
async fn concurrent_the_caller_can_always_tell_which_world_it_is_in() {
    let s = svc().await;
    s.add_epoch(Parameters(epoch("epoch:e")))
        .await
        .expect("epoch");

    let (created, pinned) = tokio::join!(
        s.add_change_event(Parameters(event("chg:x"))),
        s.pin_at_epoch(Parameters(pin("chg:x", "epoch:e"))),
    );

    created.expect("the create is not order-dependent and must always land");

    match pinned {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("PARALLEL BATCH"),
                "losing the race is allowed; leaving the caller unable to tell a no-op from a \
                 save is not: {msg}"
            );
        }
    }
}
