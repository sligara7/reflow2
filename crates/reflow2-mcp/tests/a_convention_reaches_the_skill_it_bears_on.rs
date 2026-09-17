//! The delivery leg, pinned where it is read: a convention recorded with
//! `steps` naming a served skill rides `get_skill` for that skill, an unknown
//! step is refused with the nearest name, and the two absences reach
//! `detect_gaps` and are answered by `documents` with `doc_kind`
//! (`req:a-repos-procedural-know-how-is-delivered-at-the-step-it-bears-on-and-the-design-knows-the-adapter-exists`).

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
async fn a_convention_with_steps_rides_get_skill_for_the_skill_it_names() {
    let s = svc().await;
    j!(s.add_project(req(serde_json::json!({ "id": "proj:p", "name": "P" }))));
    j!(s.add_design_rule(req(serde_json::json!({
        "id": "rule:tests-run-with-pytest-from-the-root",
        "name": "Tests run with pytest from the repo root",
        "statement": "Run `pytest` from the repository root; the conftest adds src/ to the path.",
        "category": "convention", "steps": ["adopt", "link-artifacts"]
    }))));
    let out = j!(s.get_skill(req(serde_json::json!({ "name": "adopt" }))));
    let carried = out["lessons"]["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .any(|l| l["id"] == "rule:tests-run-with-pytest-from-the-root")
        })
        .unwrap_or(false);
    assert!(
        carried,
        "the convention is handed over beside the skill: {}",
        out["lessons"]
    );
    let gaps = j!(s.detect_gaps(req(serde_json::json!({}))));
    assert!(
        !gaps["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "convention_delivered_nowhere"),
        "delivered, so not asked: {gaps:?}"
    );
}

#[tokio::test]
async fn an_unknown_step_is_refused_with_the_nearest_name() {
    let s = svc().await;
    let err = s
        .add_design_rule(req(serde_json::json!({
            "id": "rule:x", "name": "X", "statement": "A convention.",
            "category": "convention", "steps": ["adpot"]
        })))
        .await
        .expect_err("an unserved step is refused");
    assert!(
        err.to_string().contains("adopt"),
        "the refusal names what would have worked: {err}"
    );
}

#[tokio::test]
async fn the_adapter_is_asked_for_and_a_documents_edge_with_the_doc_kind_answers_it() {
    let s = svc().await;
    j!(s.add_project(req(serde_json::json!({ "id": "proj:p", "name": "P" }))));
    for i in 0..5 {
        j!(s.add_artifact(req(serde_json::json!({
            "id": format!("art:{i}"), "name": format!("{i}.rs"), "artifact_type": "code",
            "location": format!("src/{i}.rs")
        }))));
    }
    let gaps = j!(s.detect_gaps(req(serde_json::json!({}))));
    assert!(
        gaps["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "agent_instructions_unregistered"),
        "{gaps:?}"
    );
    j!(s.add_artifact(req(serde_json::json!({
        "id": "art:agents", "name": "AGENTS.md", "artifact_type": "document", "location": "AGENTS.md"
    }))));
    j!(s.documents(req(serde_json::json!({
        "artifact_id": "art:agents", "target_type": "Project", "target_id": "proj:p",
        "doc_kind": "agent_instructions"
    }))));
    let gaps = j!(s.detect_gaps(req(serde_json::json!({}))));
    assert!(
        !gaps["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "agent_instructions_unregistered"),
        "{gaps:?}"
    );
}
