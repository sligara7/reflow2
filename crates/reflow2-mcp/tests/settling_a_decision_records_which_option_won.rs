//! Settling a decision records which option won, without rewriting the
//! deliberation that produced it.
//!
//! flo2 F12, 2026-09-19: `set_decision_status` took the id, the status, the
//! approver and the date, and nothing else. It moved two decisions to
//! `accepted` correctly, and left both still NAMED *"OPEN — is … the one thing
//! flo2 is missing?"* with bodies ending *"Options, none chosen."*
//!
//! Their sentence is the argument: **an accepted Decision that still reads as
//! an open question is worse than an unsettled one**, because a later reader
//! trusts the prose over the status field.
//!
//! The existing route out was to re-send the whole node through `add_decision`
//! with the name and body rewritten — a four-kilobyte upsert to change a status
//! and a heading, and the exact operation measured that same week to silently
//! drop a load-bearing paragraph. `collapse_decision` does not reach it: it
//! needs REGISTERED alternatives, and these options lived in the prose, which is
//! what the brainstorm skill asks for.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

const DELIBERATION: &str = "OPEN — recorded as brainstorming. (a) Host it ourselves, which is \
                            cheapest and slowest to change. (b) Buy it, which is dearer and \
                            immediate. (c) Do neither yet. Options, none chosen.";

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:ann".into(),
        name: Some("Ann".into()),
        kind: None,
        handle: None,
        description: None,
    })));
    j!(s.add_decision(Parameters(
        serde_json::from_value::<DecisionReq>(json!({
            "id": "dec:hosting",
            "name": "OPEN — do we host it ourselves?",
            "decision": DELIBERATION,
            "kind": "exploratory"
        }))
        .unwrap()
    )));
    s
}

async fn settle(s: &ReflowService, chose: Option<&str>) -> Value {
    let mut args = json!({
        "decision_id": "dec:hosting",
        "status": "accepted",
        "approver": "who:ann",
        "acted_at": "2026-09-20"
    });
    if let Some(c) = chose {
        args["chose"] = json!(c);
    }
    j!(s.set_decision_status(Parameters(
        serde_json::from_value::<SetDecisionStatusReq>(args).unwrap()
    )))
}

#[tokio::test]
async fn the_settlement_records_what_it_chose_and_leaves_the_deliberation_alone() {
    let s = svc().await;
    let v = settle(
        &s,
        Some(
            "(b) Buy it. (a) was cheapest but we need it this quarter, and (c) defers a \
              decision the budget round will force anyway.",
        ),
    )
    .await;

    assert_eq!(v["properties"]["status"], "accepted", "{v}");
    assert!(
        v["properties"]["chose"]
            .as_str()
            .is_some_and(|c| c.contains("(b) Buy it")),
        "the settlement must record what it chose: {}",
        v["properties"]
    );
    // THE POINT: the deliberation is untouched. Recording the outcome must not
    // cost a rewrite of the reasoning that produced it.
    assert_eq!(
        v["properties"]["decision"], DELIBERATION,
        "the deliberation must survive the settlement verbatim"
    );
}

#[tokio::test]
async fn a_settlement_that_names_no_outcome_is_unchanged() {
    let s = svc().await;
    let v = settle(&s, None).await;
    assert_eq!(v["properties"]["status"], "accepted", "{v}");
    assert!(
        v["properties"]["chose"].is_null(),
        "absent must keep meaning nobody said: {}",
        v["properties"]
    );
}

#[tokio::test]
async fn the_owners_name_is_still_required_to_settle() {
    let s = svc().await;
    // `chose` records the outcome; it does not substitute for the owner's word.
    let v = j!(s.set_decision_status(Parameters(
        serde_json::from_value::<SetDecisionStatusReq>(json!({
            "decision_id": "dec:hosting",
            "status": "accepted",
            "chose": "(b) Buy it."
        }))
        .unwrap()
    )));
    assert!(
        v["carries_nobodys_name"].is_string(),
        "settling without an approver must still say so: {v}"
    );
}
