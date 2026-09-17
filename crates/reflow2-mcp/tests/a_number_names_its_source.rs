//! The served surface can say where a number came from: a limit's
//! `limit_basis` / `limit_source` / `limit_measured_at` on `add_constraint`,
//! a contribution's `source` on `constrains`, and a fact's `source` on
//! `record_finding`
//! (`req:a-quantity-carries-how-it-was-obtained-and-an-unsourced-one-is-reported`).
//!
//! Reach is the whole point: a property the core reads and no tool can write
//! is the class the vocabulary-reach requirement exists to end, and every one
//! of these three would have been that.

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
async fn a_limit_says_how_it_was_obtained_and_by_what() {
    let s = svc().await;
    let out = j!(s.add_constraint(req(serde_json::json!({
        "id": "con:floor", "name": "Interior floor area",
        "statement": "Under 288.9.", "category": "budget",
        "quantity": "floor_area", "unit": "sqft", "limit": 288.9,
        "limit_basis": "measured", "limit_source": "art:ifc-shell-model",
        "limit_measured_at": "2026-09-16"
    }))));
    assert_eq!(out["properties"]["limit_basis"], "measured", "{out:?}");
    assert_eq!(out["properties"]["limit_source"], "art:ifc-shell-model");
    assert_eq!(out["properties"]["limit_measured_at"], "2026-09-16");
}

#[tokio::test]
async fn a_contribution_names_the_tool_that_measured_it() {
    let s = svc().await;
    j!(s.add_constraint(req(serde_json::json!({
        "id": "con:floor", "name": "Interior floor area", "statement": "Under 288.9.",
        "category": "budget", "quantity": "floor_area", "unit": "sqft", "limit": 288.9
    }))));
    j!(s.add_component(req(serde_json::json!({
        "id": "cmp:tank", "name": "Fish tank", "description": "holds the fish"
    }))));
    let out = j!(s.constrains(req(serde_json::json!({
        "constraint_id": "con:floor", "target_id": "cmp:tank",
        "contribution": 44.0, "unit": "sqft", "basis": "measured",
        "source": "ifc_quantify", "measured_at": "2026-09-16"
    }))));
    assert_eq!(out["properties"]["source"], "ifc_quantify", "{out:?}");
    let report = j!(s.budget_report(req(serde_json::json!({ "constraint_id": "con:floor" }))));
    assert_eq!(
        report["unsourced"].as_array().map(Vec::len),
        Some(0),
        "{report:?}"
    );
}

#[tokio::test]
async fn a_fact_says_who_or_what_its_value_came_from() {
    let s = svc().await;
    j!(s.add_project(req(
        serde_json::json!({ "id": "prj:pod", "name": "bhome pod" })
    )));
    let out = j!(s.record_finding(req(serde_json::json!({
        "id": "fact:gross-floor-area-measured",
        "subject_id": "prj:pod",
        "name": "Gross floor area 26.8425 m²",
        "statement": "Qto_SlabBaseQuantities.GrossArea = 26.8425 m² on the finish-floor slab.",
        "fact_type": "measurement", "basis": "measured",
        "source": "ifc_quantify", "value": "{\"gross_area_m2\": 26.8425}",
        "valid_from": "2026-09-16"
    }))));
    assert_eq!(
        out["finding"]["properties"]["source"], "ifc_quantify",
        "{out:?}"
    );
}
