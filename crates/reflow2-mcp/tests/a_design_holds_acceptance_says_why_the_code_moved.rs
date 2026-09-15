//! A `design_holds` acceptance says WHY the code moved, or is refused.
//!
//! FIELD REPORT, 2026-09-14: `fix_without_recorded_cause` asked a project
//! about twenty-one fixes with no cause, and twenty of them were checksum
//! dispositions recorded while reconciling the design with the code — not
//! repairs at all. The agent then asked the owner whether the project should
//! record causes at all.
//!
//! ROOT CAUSE, measured: `parse_disposition` defaulted a missing `change_type`
//! on `design_holds` to `test_failure_fix`, one of the two labels that put an
//! event into the fix population. On reflow2's own design 142 of the 281
//! fix-typed ChangeEvents were minted by acceptances — 104 of them wearing the
//! default — and none carries a cause, because none is a repair. A default
//! answered a question nobody asked, and the detector then asked a person to
//! explain records nobody wrote.
//!
//! Now the question is asked at the source. An acceptance on an artifact that
//! already has a baseline must say why the code moved; a fix label is for a
//! repair, and a reconcile pass says `resync`, `refactor` or `documentation`.
//! An artifact with NO baseline is still read as a first baseline, where the
//! question does not arise.

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

async fn seeded(checksum: Option<&str>) -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:x".into(),
        name: Some("X".to_string()),
        description: None,
        spec: None,
        decomposition_levels: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:flight".into(),
        name: Some("Flight".into()),
        description: Some("ball flight".into()),
        status: None,
        tier: None,
        is_entry_point: None,
        is_exit_point: None,
        satisfies: None,
        allocated_to: None,
        distinct_from: None,
    })));
    j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:flight".into(),
        name: Some("BallFlight.cs".into()),
        description: None,
        location: Some("src/BallFlight.cs".into()),
        artifact_type: Some("code".into()),
        target_type: Some("Capability".into()),
        target_id: "cap:flight".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: checksum.map(str::to_string),
        content_ref: None,
        note_kind: None,
    })));
    s
}

fn accept(change_type: Option<&str>) -> SetChecksumReq {
    SetChecksumReq {
        artifact_id: "art:flight".into(),
        checksum: "sha256:v2".into(),
        disposition: "design_holds".into(),
        change_type: change_type.map(str::to_string),
        design_change_event_id: None,
        note: None,
        at: Some("2026-09-14".into()),
    }
}

/// ⭐ THE CASE FROM THE FIELD. The artifact has a baseline, the code moved,
/// and the caller did not say why. That is refused — and the refusal says
/// which labels are for a repair and which for a reconcile pass, because the
/// nearest label is what the field agent reached for.
#[tokio::test]
async fn a_baselined_artifact_accepted_without_a_reason_is_refused() {
    let s = seeded(Some("sha256:v1")).await;
    let err = s
        .set_artifact_checksum(Parameters(accept(None)))
        .await
        .expect_err("design_holds with no change_type on a baselined artifact must be refused");
    let msg = format!("{err:?}");
    assert!(msg.contains("change_type"), "{msg}");
    assert!(
        msg.contains("resync") && msg.contains("defect_fix"),
        "the refusal must name the reconcile labels and the repair labels: {msg}"
    );
}

/// A stated reason is recorded as given — and a non-repair label keeps the
/// event out of the fix population.
#[tokio::test]
async fn a_stated_reason_is_recorded_as_given() {
    let s = seeded(Some("sha256:v1")).await;
    let out = j!(s.set_artifact_checksum(Parameters(accept(Some("refactor")))));
    let id = out["change_event_id"]
        .as_str()
        .expect("event id")
        .to_string();
    let node = j!(s.get_node(Parameters(GetNodeReq {
        id: id.clone(),
        node_type: None,
    })));
    assert_eq!(
        node["node"]["properties"]["change_type"], "refactor",
        "the acceptance carries the reason the caller gave: {node}"
    );
}

/// A real repair accepted through the checksum path is still allowed to say
/// so — the fix labels are for repairs, not forbidden.
#[tokio::test]
async fn a_repair_may_still_be_accepted_as_a_repair() {
    let s = seeded(Some("sha256:v1")).await;
    let out = j!(s.set_artifact_checksum(Parameters(accept(Some("defect_fix")))));
    assert!(out["change_event_id"].as_str().is_some(), "{out}");
}

/// An artifact with NO baseline is read as a first baseline whatever was
/// passed (2026-09-07), and a first baseline moved nothing — so there is no
/// "why did the code move" to ask, and no refusal.
#[tokio::test]
async fn a_first_baseline_needs_no_reason() {
    let s = seeded(None).await;
    let out = j!(s.set_artifact_checksum(Parameters(accept(None))));
    assert!(
        out["change_event_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("chg:baseline-")),
        "read as a first baseline: {out}"
    );
}

/// The bulk form refuses the same way, per item, and names the item.
#[tokio::test]
async fn the_bulk_form_refuses_the_silent_item_and_names_it() {
    let s = seeded(Some("sha256:v1")).await;
    let err = s
        .set_artifact_checksums(Parameters(SetChecksumsReq {
            accepts: vec![ChecksumAcceptReq {
                artifact_id: "art:flight".into(),
                checksum: "sha256:v2".into(),
                disposition: "design_holds".into(),
                change_type: None,
                design_change_event_id: None,
                note: None,
                at: Some("2026-09-14".into()),
            }],
            check_only: false,
        }))
        .await;
    let text = match err {
        Err(e) => format!("{e:?}"),
        Ok(r) => format!("{:?}", r.structured_content),
    };
    assert!(
        text.contains("art:flight") && text.contains("change_type"),
        "the batch must name the silent item and what it lacks: {text}"
    );
}
