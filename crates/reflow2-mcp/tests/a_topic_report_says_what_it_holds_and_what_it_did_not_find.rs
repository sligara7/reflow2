//! The /topic view: what the design holds about one subject, in one call,
//! with a mandatory not-found line.
//!
//! `dec:idea-a-topic-view-shows-what-the-design-holds-about-one-subject-read-only`
//! ruled the projection lives in the server (a view is a projection of the
//! graph, never a renderer's fill-in), that the not-found line is load-bearing
//! (a digest built on a search miss is a confident wrong answer), and that the
//! reply is bounded like detect_gaps.

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

/// A small design about rainfall: a requirement, a capability that satisfies
/// it, a component it is allocated to, a check, a dated change and a dated
/// measurement — and one unrelated component that must NOT appear.
async fn rainfall_design() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(
        serde_json::from_value(
            json!({"id": "req:rain-total", "name": "Rainfall totals survive a dropped packet",
            "statement": "A lost reading must not lose the running rainfall total."})
        )
        .unwrap()
    )));
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:rain-sum", "name": "Cumulative rainfall",
            "description": "sends the running rainfall total rather than deltas", "status": "realized"})).unwrap()
    )));
    j!(s.add_component(Parameters(
        serde_json::from_value(json!({"id": "cmp:gauge", "name": "Rain gauge", "description": "tips and counts rainfall"})).unwrap()
    )));
    j!(s.add_component(Parameters(
        serde_json::from_value(json!({"id": "cmp:display", "name": "Indoor display", "description": "shows temperature"})).unwrap()
    )));
    // A populated type that says nothing about rainfall, so the not-found
    // line has something present-but-unmatched to name.
    j!(s.add_contributor(Parameters(
        serde_json::from_value(json!({"id": "who:ann", "name": "Ann"})).unwrap()
    )));
    j!(s.satisfies(Parameters(
        serde_json::from_value(json!({"from_id": "cap:rain-sum", "to_id": "req:rain-total"}))
            .unwrap()
    )));
    j!(s.allocate(Parameters(
        serde_json::from_value(json!({"from_id": "cap:rain-sum", "to_id": "cmp:gauge"})).unwrap()
    )));
    j!(s.add_verification(Parameters(
        serde_json::from_value(
            json!({"id": "ver:rain-drop", "name": "a dropped rainfall packet heals"})
        )
        .unwrap()
    )));
    j!(s.verifies(Parameters(
        serde_json::from_value(
            json!({"verification_id": "ver:rain-drop", "target_id": "cap:rain-sum"})
        )
        .unwrap()
    )));
    j!(s.add_change_event(Parameters(
        serde_json::from_value(json!({"id": "chg:rain-older", "name": "first rainfall cut", "change_type": "new_feature",
            "subject": "system", "detected_at": "2026-08-01",
            "affected": [{"node_type": "Capability", "node_id": "cap:rain-sum"}]})).unwrap()
    )));
    j!(s.add_change_event(Parameters(
        serde_json::from_value(json!({"id": "chg:rain-newer", "name": "rainfall totals made cumulative", "change_type": "defect_fix",
            "subject": "system", "detected_at": "2026-09-01",
            "affected": [{"node_type": "Capability", "node_id": "cap:rain-sum"}]})).unwrap()
    )));
    j!(s.add_change_event(Parameters(
        serde_json::from_value(json!({"id": "chg:rain-undated", "name": "an undated rainfall tweak", "change_type": "refactor",
            "subject": "system",
            "affected": [{"node_type": "Capability", "node_id": "cap:rain-sum"}]})).unwrap()
    )));
    j!(s.create_node(Parameters(
        serde_json::from_value(json!({"node_type": "TemporalFact", "id": "fact:rain-drop-rate",
            "props": {"fact_type": "measurement", "basis": "measured", "subject_id": "cmp:gauge",
                      "statement": "rainfall packets dropped 3% in field", "valid_from": "2026-08-20"}})).unwrap()
    )));
    s
}

async fn topic(s: &ReflowService, q: &str, budget: Option<usize>) -> JsonValue {
    j!(s.topic_report(Parameters(TopicReportReq {
        query: q.into(),
        limit: None,
        budget_chars: budget,
    })))
}

fn hit<'a>(r: &'a JsonValue, id: &str) -> &'a JsonValue {
    r["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .flat_map(|g| g["hits"].as_array().expect("hits").iter())
        .find(|h| h["node_id"] == id)
        .unwrap_or_else(|| panic!("{id} not among the hits: {r}"))
}

#[tokio::test]
async fn a_subject_comes_back_grouped_with_status_connections_and_the_latest_dated_change() {
    let s = rainfall_design().await;
    let r = topic(&s, "rainfall", None).await;
    assert!(r["count"].as_u64().unwrap() >= 4, "{r}");
    let types: Vec<&str> = r["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["node_type"].as_str().unwrap())
        .collect();
    assert!(
        types.contains(&"Requirement") && types.contains(&"Capability"),
        "{types:?}"
    );
    assert!(
        !types.contains(&"Component") || hit(&r, "cmp:gauge")["node_id"] == "cmp:gauge",
        "the temperature display is not about rainfall"
    );
    assert!(
        r["groups"].as_array().unwrap().iter().all(|g| !g["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["node_id"] == "cmp:display")),
        "an unrelated node must not appear: {r}"
    );

    let cap = hit(&r, "cap:rain-sum");
    assert_eq!(cap["status"], "realized");
    let conns = cap["connections"].as_array().expect("connections");
    assert!(
        conns
            .iter()
            .any(|c| c["edge_type"] == "SATISFIES" && c["direction"] == "out"),
        "{conns:?}"
    );
    assert!(
        conns
            .iter()
            .any(|c| c["edge_type"] == "VERIFIES" && c["direction"] == "in"),
        "{conns:?}"
    );
    // Newest DATED change wins; the undated one is never offered.
    assert_eq!(cap["latest"]["kind"], "change");
    assert_eq!(cap["latest"]["id"], "chg:rain-newer");
    assert_eq!(cap["latest"]["at"], "2026-09-01");

    // A measurement is the other side of `latest`.
    let gauge = hit(&r, "cmp:gauge");
    assert_eq!(gauge["latest"]["kind"], "measurement");
    assert_eq!(gauge["latest"]["id"], "fact:rain-drop-rate");

    assert_eq!(r["budget"]["detail"], "full");
    assert!(
        r["not_found"].as_str().unwrap().contains("Matched"),
        "{}",
        r["not_found"]
    );
}

#[tokio::test]
async fn the_not_found_line_is_mandatory_and_names_what_was_not_matched() {
    let s = rainfall_design().await;
    let r = topic(&s, "rainfall", None).await;
    let nf = r["not_found"]
        .as_str()
        .expect("not_found is always a string");
    assert!(
        nf.contains("NOT matched, though present") && nf.contains("Contributor (1)"),
        "the types that were there and matched nothing are named: {nf}"
    );

    let r = topic(&s, "hydraulic actuator torque", None).await;
    assert_eq!(r["count"], 0);
    assert_eq!(r["groups"].as_array().unwrap().len(), 0);
    let nf = r["not_found"].as_str().unwrap();
    assert!(nf.contains("NOTHING MATCHED"), "{nf}");
    assert!(
        nf.contains("other words"),
        "a miss is not read as absence: {nf}"
    );

    // Cut at the limit, a type absent from the list is "not among the top
    // N", never "not matched": it may rank below the cut.
    let r = j!(s.topic_report(Parameters(TopicReportReq {
        query: "rainfall".into(),
        limit: Some(1),
        budget_chars: None,
    })));
    let nf = r["not_found"].as_str().unwrap();
    assert!(
        nf.contains("not among the top 1") && !nf.contains("NOT matched"),
        "{nf}"
    );
    assert!(nf.contains("cut at its limit of 1"), "{nf}");
}

#[tokio::test]
async fn a_tight_budget_withholds_detail_before_hits_and_never_the_counts() {
    let s = rainfall_design().await;
    let full = topic(&s, "rainfall", None).await;
    let count = full["count"].as_u64().unwrap();
    let tight = topic(&s, "rainfall", Some(600)).await;
    assert_eq!(
        tight["count"].as_u64().unwrap(),
        count,
        "count is never trimmed"
    );
    assert_eq!(
        tight["by_type"], full["by_type"],
        "by_type is never trimmed"
    );
    assert_eq!(tight["budget"]["detail"], "titles_only");
    assert!(tight["budget"]["note"].is_string(), "{tight}");
    let listed: usize = tight["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["hits"].as_array().unwrap().len())
        .sum();
    assert!(listed >= 1 && listed as u64 <= count, "{tight}");
    for g in tight["groups"].as_array().unwrap() {
        for h in g["hits"].as_array().unwrap() {
            assert!(
                h.get("connections").is_none() && h.get("latest").is_none(),
                "{h}"
            );
        }
    }
}
