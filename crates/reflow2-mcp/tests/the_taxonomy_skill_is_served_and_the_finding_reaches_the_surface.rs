//! The three legs of the taxonomy requirement reach the served surface: the
//! establish-taxonomy skill is served with its command, add_component can say
//! a part's kind, and encoding_undecided arrives through detect_gaps
//! (`req:a-taxonomy-is-decided-once-before-bulk-capture-and-every-instance-cites-it`).

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
async fn the_skill_is_served_and_names_the_finding_it_answers() {
    let s = svc().await;
    let out = j!(s.get_skill(req(serde_json::json!({ "name": "establish-taxonomy" }))));
    let body = out["body"].as_str().expect("a body");
    assert!(
        body.contains("encoding_undecided"),
        "the skill names its finding"
    );
    assert!(
        body.contains("governed_by"),
        "and the edge that makes the taxonomy real"
    );
    let listed = j!(s.list_skills(req(serde_json::json!({}))));
    let names: Vec<&str> = listed["skills"]
        .as_array()
        .expect("skills")
        .iter()
        .filter_map(|x| x["name"].as_str())
        .collect();
    assert!(names.contains(&"establish-taxonomy"), "{names:?}");
}

#[tokio::test]
async fn a_part_can_say_its_kind_on_the_typed_constructor() {
    let s = svc().await;
    let out = j!(s.add_component(req(serde_json::json!({
        "id": "svc:queue", "name": "Queue service", "description": "holds the work",
        "kind": "service"
    }))));
    assert_eq!(out["properties"]["kind"], "service", "{out:?}");
}

#[tokio::test]
async fn a_mixed_encoding_reaches_the_surface_as_encoding_undecided() {
    let s = svc().await;
    j!(s.add_project(req(serde_json::json!({ "id": "proj:p", "name": "P" }))));
    for (id, kind) in [
        ("svc:a", Some("service")),
        ("svc:b", Some("service")),
        ("svc:c", None),
        ("svc:d", None),
    ] {
        let mut v =
            serde_json::json!({ "id": id, "name": id, "description": "a part that does its job" });
        if let Some(k) = kind {
            v["kind"] = serde_json::Value::String(k.into());
        }
        j!(s.add_component(req(v)));
    }
    let gaps = j!(s.detect_gaps(req(serde_json::json!({}))));
    let hit = gaps["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|g| g["gap_source"] == "encoding_undecided")
        .expect("the finding is served");
    assert_eq!(hit["affected_ids"], serde_json::json!(["svc:c", "svc:d"]));
}
