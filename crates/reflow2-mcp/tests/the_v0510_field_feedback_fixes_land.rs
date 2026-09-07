//! v0.51.0's field-feedback fixes, each pinned where the field met it.
//!
//! All from dev_storyflow's 2026-09-06 entries and the rule-11 facts of
//! 2026-09-06, triaged 2026-09-07 and approved as an increment the same day.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value as JsonValue, json};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_contributor(Parameters(
        serde_json::from_value(json!({"id": "who:ann", "name": "Ann"})).unwrap()
    )));
    s
}

async fn get(s: &ReflowService, id: &str) -> JsonValue {
    j!(s.get_node(Parameters(
        serde_json::from_value(json!({"id": id})).unwrap()
    )))
}

/// (1) A null in create_node's props UNSETS the property and the reply says so.
/// It used to be stored as a value that three readers read two ways.
#[tokio::test]
async fn a_null_unsets_the_property_and_the_reply_says_so() {
    let s = svc().await;
    j!(s.add_design_rule(Parameters(
        serde_json::from_value(json!({"id": "rule:x", "name": "X", "statement": "always x",
            "enforced": true, "approver": "who:ann"}))
        .unwrap()
    )));
    let out = j!(s.create_node(Parameters(
        serde_json::from_value(json!({"node_type": "DesignRule", "id": "rule:x",
            "props": {"enforced": null}}))
        .unwrap()
    )));
    assert_eq!(out["unset"], json!(["enforced"]), "{out}");
    assert!(
        out["properties"].get("enforced").is_none(),
        "the key is gone: {out}"
    );
    assert!(
        out.get("undeclared").is_none(),
        "a null is not an undeclared property: {out}"
    );
    let n = get(&s, "rule:x").await;
    assert!(
        n["node"]["properties"].get("enforced").is_none(),
        "unset survives a re-read: {n}"
    );
    assert_eq!(
        n["node"]["properties"]["statement"], "always x",
        "other properties untouched"
    );

    // A required property cannot be unset; the refusal names it.
    let err = s
        .create_node(Parameters(
            serde_json::from_value(json!({"node_type": "DesignRule", "id": "rule:x",
                "props": {"name": null}}))
            .unwrap(),
        ))
        .await
        .expect_err("a nameless rule is broken, not unstated");
    assert!(
        err.to_string().contains("`name`") && err.to_string().contains("REQUIRED"),
        "{err}"
    );
}

/// (3) The capability that answers a check's finding, beside the check that
/// found it, is reported — not refused as a duplicate.
#[tokio::test]
async fn a_capability_beside_the_check_that_found_the_need_is_not_refused() {
    let s = svc().await;
    j!(s.add_verification(Parameters(
        serde_json::from_value(json!({"id": "ver:rain-drop",
            "name": "a dropped rainfall packet must heal on the next reading"}))
        .unwrap()
    )));
    let out = j!(s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:rain-heal",
            "name": "A dropped rainfall packet heals on the next reading",
            "description": "cumulative totals so a lost reading heals itself"}))
        .unwrap()
    )));
    assert_eq!(
        out["node_id"], "cap:rain-heal",
        "created without distinct_from: {out}"
    );
}

/// (4) `test` is a legal artifact_type: the SOURCE of a check, not its run.
#[tokio::test]
async fn a_test_file_can_be_registered_as_a_test() {
    let s = svc().await;
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:seal", "name": "Sealed housing",
            "description": "keeps water out"}))
        .unwrap()
    )));
    let out = j!(s.link_artifact(Parameters(
        serde_json::from_value(
            json!({"artifact_id": "art:seal-test", "name": "seal.test.ts",
            "location": "tests/seal.test.ts", "artifact_type": "test",
            "target_type": "Capability", "target_id": "cap:seal"})
        )
        .unwrap()
    )));
    assert_eq!(out["artifact_id"], "art:seal-test", "{out}");
    let n = get(&s, "art:seal-test").await;
    assert_eq!(n["node"]["properties"]["artifact_type"], "test");
}

/// (5) A replaced executable refuses to export; a current one does not.
#[test]
fn an_export_from_a_replaced_executable_is_refused_and_names_the_stamp() {
    let e = export_stale_refusal(Some(true), "see served_by").expect("stale refuses");
    let m = e.to_string();
    assert!(
        m.contains("REFUSED") && m.contains(env!("CARGO_PKG_VERSION")),
        "{m}"
    );
    assert!(
        m.contains("--stop-shared"),
        "the refusal says how to refresh: {m}"
    );
    assert!(export_stale_refusal(Some(false), "").is_none());
    assert!(
        export_stale_refusal(None, "").is_none(),
        "cannot tell is not stale"
    );
}

/// (6) TemporalFact declares `name`: writing one no longer warns.
#[tokio::test]
async fn a_fact_can_carry_a_name_without_a_warning() {
    let s = svc().await;
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:seal", "name": "Sealed housing",
            "description": "keeps water out"}))
        .unwrap()
    )));
    let out = j!(s.create_node(Parameters(
        serde_json::from_value(json!({"node_type": "TemporalFact", "id": "fact:seal-leak",
            "props": {"name": "the seal leaked at 40 psi", "fact_type": "defect", "basis": "measured",
                      "subject_id": "cap:seal", "statement": "leaked", "valid_from": "2026-09-07"}})).unwrap()
    )));
    assert!(out.get("undeclared").is_none(), "{out}");
    assert_eq!(out["properties"]["name"], "the seal leaked at 40 psi");
}

/// (7) get_node accepts `node_id`, the key search_design hands back.
#[tokio::test]
async fn get_node_accepts_the_key_search_design_hands_back() {
    let s = svc().await;
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:seal", "name": "Sealed housing",
            "description": "keeps water out"}))
        .unwrap()
    )));
    let out = j!(s.get_node(Parameters(
        serde_json::from_value(json!({"node_id": "cap:seal"})).unwrap()
    )));
    assert_eq!(out["node"]["node_id"], "cap:seal", "{out}");
}

/// (8) An unknown-field refusal says the client's tool list may predate the
/// server, and names the server's version.
#[test]
fn an_unknown_field_refusal_names_the_stale_client_case() {
    let h = stale_client_hint("failed to deserialize parameters: unknown field `findings`");
    assert!(
        h.starts_with("failed to deserialize parameters: unknown field `findings`"),
        "{h}"
    );
    assert!(h.contains("tool list may predate the server"), "{h}");
    assert!(h.contains(env!("CARGO_PKG_VERSION")), "{h}");
}
