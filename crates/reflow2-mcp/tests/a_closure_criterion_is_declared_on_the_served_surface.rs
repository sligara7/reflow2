//! The served surface can declare what closure means and read whether the
//! design closes: `set_closure_criterion` on the Project, `closure_report`
//! as one read, and `margin` on `add_constraint`
//! (`req:a-design-closes-against-a-declared-threshold-and-the-report-names-the-first-hole`).
//!
//! Driven through the JSON request shapes so a renamed or unreachable field
//! fails here rather than in front of somebody.

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

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn req<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Parameters<T> {
    Parameters(serde_json::from_value(v).expect("request shape"))
}

#[tokio::test]
async fn no_criterion_reads_as_no_criterion_stated_and_a_declared_one_is_read_back() {
    let s = svc().await;
    j!(s.add_project(req(
        serde_json::json!({ "id": "proj:pod", "name": "bhome pod" })
    )));
    let out = j!(s.closure_report(req(serde_json::json!({}))));
    assert_eq!(out["verdict"], "no_closure_criterion_stated", "{out:?}");
    assert_eq!(out["legs"].as_array().map(Vec::len), Some(5));

    let p = j!(s.set_closure_criterion(req(serde_json::json!({
        "project_id": "proj:pod", "legs": ["traceability", "budgets"], "threshold": 100
    }))));
    assert_eq!(p["properties"]["closure_threshold"], 100.0, "{p:?}");
    let out = j!(s.closure_report(req(serde_json::json!({}))));
    assert_eq!(out["criterion"]["legs"][1], "budgets");
    // Nothing recorded yet: both counted legs have nothing to run on, and
    // the design does not close on that account rather than closing on
    // silence.
    assert_eq!(out["verdict"], "does_not_close", "{out:?}");
    assert_eq!(out["first_hole"]["leg"], "traceability");
}

#[tokio::test]
async fn an_unknown_leg_is_refused_by_name() {
    let s = svc().await;
    j!(s.add_project(req(
        serde_json::json!({ "id": "proj:pod", "name": "bhome pod" })
    )));
    let err = s
        .set_closure_criterion(req(serde_json::json!({
            "project_id": "proj:pod", "legs": ["vibes"], "threshold": 100
        })))
        .await
        .expect_err("refused");
    assert!(err.to_string().contains("vibes"), "{err}");
}

#[tokio::test]
async fn a_budget_carries_its_declared_margin() {
    let s = svc().await;
    let out = j!(s.add_constraint(req(serde_json::json!({
        "id": "con:floor", "name": "Interior floor area", "statement": "Under 288.9.",
        "category": "budget", "quantity": "floor_area", "unit": "sqft", "limit": 288.9,
        "margin": 10.0
    }))));
    assert_eq!(out["properties"]["margin"], 10.0, "{out:?}");
}
