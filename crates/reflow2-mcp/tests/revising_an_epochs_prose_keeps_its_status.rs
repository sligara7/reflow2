//! Revising an epoch's prose does not move it back to `planned`.
//!
//! `fact:plan-epoch-on-an-existing-epoch-resets-its-status-to-planned`,
//! measured on every arrival of increments 430–434 (2026-09-11/12): calling
//! `plan_epoch` again with the same id — the constructor's own "call again to
//! revise" path — put an ARRIVED epoch back to `planned`, so every revised
//! description had to be followed by a second `set_epoch_status(arrived)`.
//!
//! The constructor's contract says omitted fields keep their stored value.
//! `status` is not a parameter of `plan_epoch` at all, so a revise has no
//! business touching it: `planned` is what a NEW epoch lands in, and only
//! `set_epoch_status` moves it. This is BL-183's class — a revise silently
//! un-confirming a status — on a second type.
//!
//! Shown to fail before the fix: the first assertion below read `planned`.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

fn plan(id: &str, description: &str) -> AddEpochReq {
    AddEpochReq {
        id: id.into(),
        name: Some("Increment 900 — the one being revised".into()),
        epoch_type: Some("revision".into()),
        sequence: Some(900),
        description: Some(description.into()),
        checksum: None,
    }
}

async fn status_of(s: &ReflowService, id: &str) -> String {
    let n = j!(s.get_node(Parameters(GetNodeReq {
        id: id.into(),
        node_type: None,
    })));
    n["node"]["properties"]["status"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

/// THE POINT: arrive, revise the prose, still arrived.
#[tokio::test]
async fn revising_an_arrived_epochs_prose_keeps_it_arrived() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.plan_epoch(Parameters(plan(
        "epoch:planned-nine-hundred",
        "IN: the plan."
    ))));
    assert_eq!(status_of(&s, "epoch:planned-nine-hundred").await, "planned");
    j!(s.set_epoch_status(Parameters(EpochStatusReq {
        epoch_id: "epoch:planned-nine-hundred".into(),
        status: "arrived".into(),
    })));
    assert_eq!(status_of(&s, "epoch:planned-nine-hundred").await, "arrived");

    // The revise: same id, new prose, nothing said about status.
    let r = j!(s.plan_epoch(Parameters(plan(
        "epoch:planned-nine-hundred",
        "IN: the plan, and what it turned out to mean."
    ))));
    assert_eq!(
        r["properties"]["description"].as_str(),
        Some("IN: the plan, and what it turned out to mean."),
        "the prose moved: {r}"
    );
    assert_eq!(
        status_of(&s, "epoch:planned-nine-hundred").await,
        "arrived",
        "a revise of the prose must not un-arrive the epoch"
    );
}

/// The other direction still holds: a NEW epoch lands `planned`, and a revise
/// of a still-planned one leaves it planned.
#[tokio::test]
async fn a_new_epoch_lands_planned_and_a_planned_one_stays_planned_when_revised() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.plan_epoch(Parameters(plan("epoch:planned-nine-oh-one", "first prose"))));
    assert_eq!(status_of(&s, "epoch:planned-nine-oh-one").await, "planned");
    j!(s.plan_epoch(Parameters(plan(
        "epoch:planned-nine-oh-one",
        "second prose"
    ))));
    assert_eq!(status_of(&s, "epoch:planned-nine-oh-one").await, "planned");
}
