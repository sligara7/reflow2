//! `add_change_event` records how a change's reason is known and which commits
//! it was — the tool boundary of `cap:a-change-says-how-its-reason-is-known`.
//!
//! The core tests pin what is stored and what reads it; these pin what the
//! JSON boundary accepts and refuses, because that is where a caller can get it
//! wrong: a branch name or a PR number passed as a commit, a workaround with
//! nothing named as the proper fix, a basis nobody declared.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

fn event(id: &str) -> AddChangeEventReq {
    AddChangeEventReq {
        description: None,
        id: id.into(),
        name: Some("Export button moved to the File menu".into()),
        change_type: Some("scope_change".into()),
        subject: Some("system".into()),
        summary: Some("Moved the export button from the toolbar to the File menu.".into()),
        rationale: Some("People hit it by accident when printing.".into()),
        affected: None,
        detected_at: Some("2020-07-14".into()),
        rationale_basis: None,
        commits: None,
        repair: None,
        stands_in_for: None,
    }
}

async fn stored(s: &ReflowService, id: &str) -> serde_json::Value {
    let out = s
        .get_node(Parameters(
            serde_json::from_value(serde_json::json!({ "id": id })).expect("request"),
        ))
        .await
        .expect("read");
    out.structured_content.expect("structured")["node"]["properties"].clone()
}

#[tokio::test]
async fn a_recalled_change_is_stored_with_its_basis_and_its_commits() {
    let s = ReflowService::in_memory().expect("service");
    s.add_change_event(Parameters(AddChangeEventReq {
        rationale_basis: Some("recalled".into()),
        commits: Some("3f2a9c1d, 77b01e2 ".into()),
        ..event("chg:recalled")
    }))
    .await
    .expect("written");
    let p = stored(&s, "chg:recalled").await;
    assert_eq!(p["rationale_basis"], "recalled");
    assert_eq!(
        p["commits"], "3f2a9c1d,77b01e2",
        "normalised to a plain list"
    );
}

#[tokio::test]
async fn a_repair_disposition_rides_the_same_call() {
    let s = ReflowService::in_memory().expect("service");
    s.add_change_event(Parameters(AddChangeEventReq {
        change_type: Some("defect_fix".into()),
        repair: Some("contained_symptom".into()),
        stands_in_for: Some("store every timestamp in UTC end to end".into()),
        ..event("chg:patch")
    }))
    .await
    .expect("written");
    let p = stored(&s, "chg:patch").await;
    assert_eq!(p["repair"], "contained_symptom");
    assert_eq!(
        p["stands_in_for"],
        "store every timestamp in UTC end to end"
    );
}

#[tokio::test]
async fn what_is_not_a_commit_is_refused_and_nothing_is_written() {
    let s = ReflowService::in_memory().expect("service");
    for bad in ["main", "#123", "3f2a9c", "", "3f2a9c1d, feature/x"] {
        let err = s
            .add_change_event(Parameters(AddChangeEventReq {
                commits: Some(bad.into()),
                ..event("chg:bad")
            }))
            .await
            .expect_err("refused");
        assert!(
            format!("{err:?}").contains("commit"),
            "{bad:?} must be refused naming commits: {err:?}"
        );
    }
    let out = s
        .get_node(Parameters(
            serde_json::from_value(serde_json::json!({ "id": "chg:bad" })).expect("request"),
        ))
        .await
        .expect("read");
    assert!(
        out.structured_content.expect("structured")["node"].is_null(),
        "a refused call writes nothing"
    );
}

#[tokio::test]
async fn a_workaround_must_name_the_fix_it_stands_in_for() {
    let s = ReflowService::in_memory().expect("service");
    let err = s
        .add_change_event(Parameters(AddChangeEventReq {
            repair: Some("contained_symptom".into()),
            ..event("chg:unnamed-patch")
        }))
        .await
        .expect_err("refused");
    assert!(format!("{err:?}").contains("stands_in_for"), "{err:?}");
}

#[tokio::test]
async fn an_undeclared_basis_is_refused_naming_the_legal_ones() {
    let s = ReflowService::in_memory().expect("service");
    let err = s
        .add_change_event(Parameters(AddChangeEventReq {
            rationale_basis: Some("remembered".into()),
            ..event("chg:odd-basis")
        }))
        .await
        .expect_err("refused");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("recalled") && msg.contains("contemporaneous"),
        "{msg}"
    );
}
