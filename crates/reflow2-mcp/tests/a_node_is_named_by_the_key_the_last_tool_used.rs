//! A node is named by the key the last tool used.
//!
//! dev_storyflow, 2026-09-02: "search_design hands back `node_id`; the tool
//! you reach for next rejects that exact key … every guess was informed by the
//! tool I had used immediately before, and every one was wrong." The relation
//! surface took aliases on 2026-09-06 (`dec:idea-one-way-to-name-which-node-
//! across-the-tool-surface`); flo2 F10 (2026-09-19) then met the same wall on
//! the setters — `set_decision_status` wanting `decision_id` for `id`,
//! `create_edge` wanting `props` for `properties`.
//!
//! Now every tool whose primary parameter names a design node by a typed key
//! (`decision_id`, `capability_id`, `epoch_id`, …) also accepts `id` and
//! `node_id`; the typed key stays the taught spelling in the published schema.
//! Tools whose key names a FINDING rather than a node (`gap_id`, `defect_id`)
//! are deliberately outside this: search_design never hands those back.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// Deserialize `$req` from a body that spells the primary node `$alias`, and
/// check it landed in the typed field. This is exactly the wire path: the
/// request structs carry `deny_unknown_fields`, so an unaliased spelling is a
/// refusal here too.
macro_rules! accepts {
    ($req:ty, $typed:ident, $alias:literal, $rest:tt) => {{
        let mut body = json!($rest);
        body[$alias] = json!("node:x");
        let parsed: $req = serde_json::from_value(body)
            .unwrap_or_else(|e| panic!("{} refuses `{}`: {e}", stringify!($req), $alias));
        assert_eq!(parsed.$typed, "node:x", "{} `{}`", stringify!($req), $alias);
    }};
}

#[test]
fn twenty_five_typed_keys_accept_id_and_node_id() {
    macro_rules! both {
        ($req:ty, $typed:ident, $rest:tt) => {
            accepts!($req, $typed, "id", $rest);
            accepts!($req, $typed, "node_id", $rest);
        };
    }
    both!(SetDecisionStatusReq, decision_id, {"status": "accepted"});
    both!(SetQualityTargetReq, decision_id, {"quality_target": "x"});
    both!(AlternativesForReq, decision_id, {});
    both!(CapabilityStatusReq, capability_id, {"status": "realized"});
    both!(CapabilityDeliveryReq, capability_id, {"delivery": "x"});
    both!(CapabilitySignatureReq, capability_id, {});
    both!(RequirementStatusReq, requirement_id, {"status": "accepted"});
    both!(RequirementDesignationReq, requirement_id, {"designation": "x"});
    both!(RequirementLineageReq, requirement_id, {"lineage": "x"});
    both!(VerificationStatusReq, verification_id, {"status": "passing"});
    both!(VerificationKindReq, verification_id, {"kind": "test"});
    both!(EpochStatusReq, epoch_id, {"status": "arrived"});
    both!(InterfaceDesignationReq, interface_id, {"designation": "x"});
    both!(InterfaceSpecReq, interface_id, {});
    both!(ArtifactIntentReq, artifact_id, {});
    both!(ProjectModeReq, project_id, {"mode": "x"});
    both!(SetClosureCriterionReq, project_id, {"legs": ["traceability"], "threshold": 1.0});
    both!(ReleaseReportReq, release_id, {});
    both!(ReleaseIncludesAllReq, release_id, {});
    both!(FlowReportReq, flow_id, {});
    both!(BudgetReportReq, constraint_id, {});
    both!(PropagateChangeReq, change_event_id, {});
    both!(ReadinessReportReq, subject_id, {});
    both!(ArrivalDeltaReq, target_id, {});
    both!(DimensionDriftReq, target_id, {"dimension": "mass"});
}

#[test]
fn props_accepts_properties_on_the_edge_and_node_writers() {
    let e: CreateEdgeReq = serde_json::from_value(json!({
        "edge_type": "DEPENDS_ON",
        "from_id": "cmp:a",
        "to_id": "cmp:b",
        "properties": {"evidence": "measured"}
    }))
    .expect("create_edge takes `properties` for `props`");
    assert_eq!(e.props.unwrap()["evidence"], "measured");
    let n: CreateNodeReq = serde_json::from_value(json!({
        "node_type": "Component",
        "id": "cmp:a",
        "properties": {"name": "A"}
    }))
    .expect("create_node takes `properties` for `props`");
    assert_eq!(n.props.unwrap()["name"], "A");
    let e: EdgeSpecReq = serde_json::from_value(json!({
        "edge_type": "DEPENDS_ON",
        "from_id": "cmp:a",
        "to_id": "cmp:b",
        "properties": {}
    }))
    .expect("create_edges items take `properties`");
    assert!(e.props.is_some());
    let n: NodeSpecReq = serde_json::from_value(json!({
        "node_type": "Component",
        "id": "cmp:a",
        "properties": {}
    }))
    .expect("create_nodes items take `properties`");
    assert!(n.props.is_some());
}

#[test]
fn the_published_schema_still_teaches_the_typed_spelling() {
    // An alias is a courtesy on the way in, not a second name on the way out:
    // the schema a harness reads carries the typed key only, so nothing
    // teaches two spellings.
    let tools = ReflowService::exchange_router().list_all();
    let t = tools
        .iter()
        .find(|t| t.name == "set_decision_status")
        .expect("served");
    let props = t.input_schema["properties"]
        .as_object()
        .expect("properties");
    assert!(props.contains_key("decision_id"));
    assert!(!props.contains_key("id"));
    assert!(!props.contains_key("node_id"));
}

#[tokio::test]
async fn the_key_search_design_hands_back_moves_a_capability_end_to_end() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_capability(Parameters(
        serde_json::from_value::<CapabilityReq>(json!({
            "id": "cap:pump",
            "name": "Pump water",
            "description": "Moves water uphill."
        }))
        .unwrap()
    )));
    // `node_id` is the key search_design's reply carries.
    let moved = j!(s.set_capability_status(Parameters(
        serde_json::from_value::<CapabilityStatusReq>(json!({
            "node_id": "cap:pump",
            "status": "realized"
        }))
        .unwrap()
    )));
    assert_eq!(moved["properties"]["status"], "realized");
}
