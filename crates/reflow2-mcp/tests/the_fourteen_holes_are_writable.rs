//! The fourteen declared properties nothing could write can be written.
//!
//! # What these are, and why fourteen rather than forty-three
//!
//! The reachability instrument reported 43 properties the served surface could
//! not write. Split by intent on 2026-09-07, that number was two thirds wrong
//! as a work list: 15 are written by an OPERATION, where a caller parameter
//! would be actively harmful — a hand-set creation timestamp or mirror hash
//! lets somebody forge a fact the system computes — and 13 are declared and
//! carried by nothing at all.
//!
//! The remaining **14 are holes**: a user would reasonably want to state them
//! and no tool would take them. Most were being written through the generic
//! `create_node` escape hatch, which is how they stayed invisible — 147 of 234
//! capabilities carry a flow-endpoint flag nothing on the surface could set,
//! 154 of 207 requirements carry a cross-cutting concern, and every constraint
//! carries a priority.
//!
//! `fact:six-constructors-cannot-write-any-prose-because-the-class-was-fixed-one-report-at-a-time-and-never-swept`
//! is the same class one step earlier; this is the rest of it.
//!
//! # The one that is not a parameter
//!
//! `Actor` had no typed constructor AT ALL, so its two properties could not be
//! given parameters without first making an `add_actor`. It was one of three
//! types in that state, and the other two never reached the unreachable list
//! because they had no instances anywhere. Both have since been resolved in
//! opposite directions on the same day: EnvironmentRule got its constructor
//! when the compliance layer was built whole, and QualityGate was retired from
//! the schema outright.
//!
//! # What is pinned
//!
//! That each value REACHES THE STORED PROPERTY, not merely that the call is
//! accepted. A tool that takes a field and drops it passes a signature check
//! and still fails the caller, which is the shape of defect this whole family
//! belongs to. The sweep in `every_constructor_can_say_what_the_thing_is.rs`
//! guards the class going forward; this pins the fourteen that were open.

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

fn props(out: &JsonValue) -> &JsonValue {
    for key in [
        "actor",
        "capability",
        "component",
        "constraint",
        "flow",
        "interface",
        "project",
        "requirement",
        "node",
        "artifact",
    ] {
        if let Some(v) = out.get(key).and_then(|v| v.get("properties")) {
            return v;
        }
    }
    out.get("properties").expect("properties come back")
}

/// Actor had no constructor at all. Both of its properties are holes for that
/// one reason, so the fix is a tool rather than a parameter.
#[tokio::test]
async fn an_actor_can_be_created_and_says_what_kind_it_is() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_actor(Parameters(
        serde_json::from_value(json!({
            "id": "act:beamline-scientist",
            "name": "Beamline scientist",
            "actor_type": "operator",
            "description": "Runs the experiment and reads the detector output."
        }))
        .unwrap()
    )));
    let p = props(&out);
    assert_eq!(p["name"], "Beamline scientist");
    assert_eq!(p["actor_type"], "operator");
    assert_eq!(
        p["description"], "Runs the experiment and reads the detector output.",
        "the description must reach the stored property — it is the type's embedding field"
    );
}

/// A capability's tier and its two flow-endpoint flags.
#[tokio::test]
async fn a_capability_states_its_tier_and_whether_it_starts_or_ends_a_flow() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_capability(Parameters(
        serde_json::from_value(json!({
            "id": "cap:accept-a-sample",
            "name": "Accept a sample",
            "description": "takes a sample into the queue",
            "tier": "operational",
            "is_entry_point": true,
            "is_exit_point": false
        }))
        .unwrap()
    )));
    let p = props(&out);
    assert_eq!(p["tier"], "operational");
    assert_eq!(
        p["is_entry_point"], true,
        "147 of 234 capabilities carried this flag written through the escape hatch"
    );
    assert_eq!(p["is_exit_point"], false);
}

/// A component's tier.
#[tokio::test]
async fn a_component_states_its_tier() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_component(Parameters(
        serde_json::from_value(json!({
            "id": "cmp:sample-stage",
            "name": "Sample stage",
            "description": "holds and positions the sample",
            "tier": "tactical"
        }))
        .unwrap()
    )));
    assert_eq!(props(&out)["tier"], "tactical");
}

/// A constraint's cross-cutting concern and how much it matters.
#[tokio::test]
async fn a_constraint_states_its_concern_and_priority() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_constraint(Parameters(
        serde_json::from_value(json!({
            "id": "con:dose-limit",
            "name": "Dose limit",
            "statement": "Sample dose must stay below the damage threshold.",
            "concern": "safety",
            "priority": "critical"
        }))
        .unwrap()
    )));
    let p = props(&out);
    assert_eq!(p["concern"], "safety");
    assert_eq!(p["priority"], "critical");
}

/// A flow's tier.
#[tokio::test]
async fn a_flow_states_its_tier() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_flow(Parameters(
        serde_json::from_value(json!({
            "id": "flw:run-an-experiment",
            "name": "Run an experiment",
            "description": "sample in, data out",
            "tier": "strategic"
        }))
        .unwrap()
    )));
    assert_eq!(props(&out)["tier"], "strategic");
}

/// An interface's free-text spec — the detail the structured fields do not
/// carry, which 13 of 22 interfaces held with no way to set it.
#[tokio::test]
async fn an_interface_carries_the_detail_its_structured_fields_do_not() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_interface(Parameters(
        serde_json::from_value(json!({
            "id": "ifc:detector-feed",
            "name": "Detector feed",
            "description": "frames off the detector",
            "spec": "Frames arrive as little-endian u16; see the vendor note for the header layout."
        }))
        .unwrap()
    )));
    assert!(
        props(&out)["spec"]
            .as_str()
            .unwrap_or_default()
            .contains("little-endian"),
        "the free-text spec must reach the stored property"
    );
}

/// THE SHARPEST HOLE: hierarchy.rs reads the design's own decomposition ladder
/// on every level check, and no tool could set it.
#[tokio::test]
async fn a_project_states_its_own_decomposition_ladder() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_project(Parameters(
        serde_json::from_value(json!({
            "id": "proj:beamline",
            "name": "Beamline",
            "description": "the endstation and its control stack",
            "decomposition_levels": ["component", "subsystem", "system", "facility"]
        }))
        .unwrap()
    )));
    let ladder = &props(&out)["decomposition_levels"];
    assert_eq!(
        ladder.as_array().map(|a| a.len()),
        Some(4),
        "the ladder must land as a list, ordered bottom-first: {ladder}"
    );
    assert_eq!(ladder[0], "component", "index 0 is the finest grain");
    assert_eq!(ladder[3], "facility");
}

/// A requirement's cross-cutting concern — 154 of 207 carried one.
#[tokio::test]
async fn a_requirement_states_its_cross_cutting_concern() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_requirement(Parameters(
        serde_json::from_value(json!({
            "id": "req:sample-survives",
            "name": "The sample survives the scan",
            "statement": "A scan shall not exceed the damage threshold.",
            "concern": "safety"
        }))
        .unwrap()
    )));
    assert_eq!(props(&out)["concern"], "safety");
}

/// The two provenance-fragment fields, set where the fragment is minted.
#[tokio::test]
async fn a_registered_file_can_point_at_its_source_and_name_its_voice() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({
            "id": "cap:read-frames", "name": "Read frames",
            "description": "pulls frames off the detector"
        }))
        .unwrap()
    )));
    let out = j!(s.link_artifact(Parameters(
        serde_json::from_value(json!({
            "artifact_id": "art:frames-rs",
            "name": "frames.rs",
            "location": "src/frames.rs",
            "artifact_type": "code",
            "target_type": "Capability",
            "target_id": "cap:read-frames",
            "content_ref": "sha256:deadbeef",
            "note_kind": "reviewer"
        }))
        .unwrap()
    )));
    let frag = out["fragment_id"].as_str().expect("a fragment was minted");
    let got = j!(s.get_node(Parameters(
        serde_json::from_value(json!({"id": frag, "node_type": "Fragment"})).unwrap()
    )));
    let p = &got["node"]["properties"];
    assert_eq!(p["content_ref"], "sha256:deadbeef");
    assert_eq!(p["note_kind"], "reviewer");
}

/// Omitting any of them changes nothing: these add a way to SAY, never an
/// obligation to.
#[tokio::test]
async fn omitting_them_leaves_the_node_exactly_as_it_was() {
    let s = ReflowService::in_memory().expect("service");
    let out = j!(s.add_capability(Parameters(
        serde_json::from_value(json!({
            "id": "cap:quiet", "name": "Quiet", "description": "says nothing extra"
        }))
        .unwrap()
    )));
    let p = props(&out);
    assert!(
        p.get("is_entry_point").is_none() && p.get("is_exit_point").is_none(),
        "an unstated flag must stay unstated — absence means nobody said"
    );
}
