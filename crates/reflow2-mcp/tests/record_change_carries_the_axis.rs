//! The served `record_change` can state WHICH AXIS a change is on.
//!
//! `subject` — `system` (the thing changed) or `record` (the thing did not
//! change and only the design's knowledge of it did) — reached the schema and
//! `add_change_event`, and stopped there. `record_change` is the composed
//! CHANGE step: the call a session makes when it snapshots a node before
//! editing it, and the one it is on when accepting drift. It took no `subject`,
//! so the axis was unreachable from the path that needed it most.
//!
//! ⭐ WHY THIS IS A TOOL-SURFACE TEST AND NOT ONLY A CORE ONE. The core gained
//! the field on `ChangeRecord`; nothing about that guarantees the served tool
//! exposes it, and a parameter that exists in Rust and not in the tool schema
//! is exactly the shape of the original defect — reachable in principle,
//! unreachable by the caller who actually needs it. The core test is
//! `record_change_can_state_its_axis.rs`; this one asserts the door is open
//! from outside.

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

/// An arrived epoch and a node worth changing — history cannot be recorded
/// into an epoch that has not happened.
async fn svc_with_an_epoch_and_a_target() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_epoch(Parameters(AddEpochReq {
        id: "epoch:e".into(),
        name: Some("e".into()),
        epoch_type: Some("revision".into()),
        sequence: Some(1),
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:thing".into(),
        name: Some("A thing".into()),
        description: Some("as it stands today".into()),
        status: None,
        distinct_from: None,
    })));
    s
}

async fn subject_of(s: &ReflowService, id: &str) -> Option<String> {
    let node = j!(s.get_node(Parameters(GetNodeReq {
        node_type: Some("ChangeEvent".into()),
        id: id.into(),
    })));
    node["node"]["properties"]["subject"]
        .as_str()
        .map(str::to_string)
}

#[tokio::test]
async fn the_record_axis_reaches_the_stored_event() {
    let s = svc_with_an_epoch_and_a_target().await;
    j!(s.record_change(Parameters(RecordChangeReq {
        epoch_id: "epoch:e".into(),
        change_event_id: "chg:only-our-knowledge-moved".into(),
        name: "re-synced against what was already there".into(),
        target_type: "Capability".into(),
        target_id: "cap:thing".into(),
        change_type: "resync".into(),
        subject: Some("record".into()),
        action: "modified".into(),
    })));

    assert_eq!(
        subject_of(&s, "chg:only-our-knowledge-moved")
            .await
            .as_deref(),
        Some("record"),
        "the axis a caller stated at the tool must survive to the stored event"
    );
}

/// Both values, so the test cannot pass on a hardcoded one.
#[tokio::test]
async fn the_system_axis_reaches_the_stored_event() {
    let s = svc_with_an_epoch_and_a_target().await;
    j!(s.record_change(Parameters(RecordChangeReq {
        epoch_id: "epoch:e".into(),
        change_event_id: "chg:the-thing-moved".into(),
        name: "the capability was reworded because it now does something else".into(),
        target_type: "Capability".into(),
        target_id: "cap:thing".into(),
        change_type: "scope_change".into(),
        subject: Some("system".into()),
        action: "modified".into(),
    })));

    assert_eq!(
        subject_of(&s, "chg:the-thing-moved").await.as_deref(),
        Some("system")
    );
}

/// Omitting it stays legal and stays silent. `resync` is precisely the type
/// whose axis cannot be derived, so a stored `system` here would be the tool
/// asserting something no caller said.
#[tokio::test]
async fn omitting_the_axis_is_accepted_and_writes_nothing() {
    let s = svc_with_an_epoch_and_a_target().await;
    j!(s.record_change(Parameters(RecordChangeReq {
        epoch_id: "epoch:e".into(),
        change_event_id: "chg:unstated".into(),
        name: "nobody said which axis this is on".into(),
        target_type: "Capability".into(),
        target_id: "cap:thing".into(),
        change_type: "resync".into(),
        subject: None,
        action: "modified".into(),
    })));

    assert_eq!(
        subject_of(&s, "chg:unstated").await,
        None,
        "absent must stay absent — inferring the axis is the failure the field was added to avoid"
    );
}

/// A value outside the enum is refused rather than stored. Silently keeping
/// `sistem` would put a third value into a two-value axis and every count over
/// it would quietly be wrong.
#[tokio::test]
async fn an_axis_outside_the_enum_is_refused() {
    let s = svc_with_an_epoch_and_a_target().await;
    let err = s
        .record_change(Parameters(RecordChangeReq {
            epoch_id: "epoch:e".into(),
            change_event_id: "chg:typo".into(),
            name: "a typo in the axis".into(),
            target_type: "Capability".into(),
            target_id: "cap:thing".into(),
            change_type: "resync".into(),
            subject: Some("sistem".into()),
            action: "modified".into(),
        }))
        .await
        .expect_err("an unknown subject must be refused, not stored");

    let text = format!("{err:?}");
    assert!(
        text.contains("change subject"),
        "the refusal must name what it rejected so the caller can fix it: {text}"
    );

    let node = j!(s.get_node(Parameters(GetNodeReq {
        node_type: Some("ChangeEvent".into()),
        id: "chg:typo".into(),
    })));
    assert!(
        node["node"].is_null(),
        "a refused call must write nothing at all, not a partial event"
    );
}
