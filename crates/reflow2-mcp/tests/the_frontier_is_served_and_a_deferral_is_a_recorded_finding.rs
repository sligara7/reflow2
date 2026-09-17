//! The incremental-adopt primitives reach the served surface: `frontier` is a
//! tool, a deferral is `record_finding` with `fact_type: deferred_derivation`,
//! and `loop_status` lists open deferrals
//! (`req:adopt-can-be-incremental-and-resumable-with-a-frontier-and-a-deferred-derivation-marker`).

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

async fn seeded() -> ReflowService {
    let s = svc().await;
    j!(s.add_project(req(serde_json::json!({ "id": "proj:p", "name": "P" }))));
    j!(s.add_requirement(req(serde_json::json!({
        "id": "req:scan", "name": "Scan a sample", "statement": "The beamline scans a sample."
    }))));
    j!(s.add_capability(req(serde_json::json!({
        "id": "cap:scan", "name": "Scan", "description": "runs a scan", "satisfies": "req:scan"
    }))));
    j!(s.add_capability(req(serde_json::json!({
        "id": "cap:archive", "name": "Archive", "description": "writes runs to the archive"
    }))));
    s
}

#[tokio::test]
async fn the_frontier_is_served_and_says_when_no_sweep_was_handed_in() {
    let s = seeded().await;
    let f = j!(s.frontier(req(serde_json::json!({}))));
    assert_eq!(
        f["structure_without_intent"][0]["node_id"], "cap:archive",
        "{f:?}"
    );
    assert!(f.get("uncaptured").is_none(), "no sweep, no list: {f:?}");
    j!(s.add_artifact(req(serde_json::json!({
        "id": "art:scanner", "name": "scanner.py", "artifact_type": "code",
        "location": "src/scanner.py"
    }))));
    let f = j!(s.frontier(req(serde_json::json!({
        "observed": [
            {"path": "src/scanner.py", "mass": 10},
            {"path": "src/archive/writer.py", "mass": 30}
        ]
    }))));
    assert_eq!(
        f["uncaptured"][0]["path"], "src/archive",
        "the shallowest directory none of whose contents are claimed: {f:?}"
    );
}

#[tokio::test]
async fn a_deferral_is_a_recorded_finding_that_the_boundary_lists_and_the_frontier_resumes_at() {
    let s = seeded().await;
    j!(s.record_finding(req(serde_json::json!({
        "id": "fact:defer-archive", "subject_id": "cap:archive",
        "fact_type": "deferred_derivation",
        "statement": "archive: structure captured, intent deferred to the next pass",
        "valid_from": "2026-09-17"
    }))));
    let ls = j!(s.loop_status(req(serde_json::json!({}))));
    assert_eq!(ls["deferrals_open"], 1, "{ls:?}");
    assert_eq!(ls["deferrals"][0]["subject_id"], "cap:archive");
    let f = j!(s.frontier(req(serde_json::json!({}))));
    assert_eq!(f["resume_point"]["subject_id"], "cap:archive", "{f:?}");
    assert!(
        f["structure_without_intent"].as_array().unwrap().is_empty(),
        "{f:?}"
    );
    let gaps = j!(s.detect_gaps(req(serde_json::json!({}))));
    assert!(
        !gaps["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "unmotivated_capability"),
        "deferred on purpose is not asked as a gap: {gaps:?}"
    );
}
