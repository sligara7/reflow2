//! One status contract across sibling constructors.
//!
//! Three field reports named the same shape: dev_storyflow (2026-09-02) found
//! four constructors answering "what status does this land in" four different
//! ways; bhome corroborated it (2026-09-04); flo2 F10 (2026-09-19) measured
//! `add_component` REFUSING `status` and then returning `status: planned`, and
//! `add_epoch` recording an unstarted cut as `arrived`. The relation surface had
//! been made consistent on 2026-09-06 and the constructor half was left
//! (`fact:field-report-flo2-2026-09-19-f10-…`).
//!
//! The contract, now on every constructor of a type that carries a status:
//! an optional `status`, the description naming what omitting it lands, the
//! value stored as given. `approver` stays on exactly the three constructors
//! whose status is SETTLED INTENT — Requirement, Decision, DesignRule, the
//! three cases the intent-authority gate reads — because a build status
//! (a part realized, a check passing, a release built, an epoch arrived) is a
//! measurement, not a signature.

use reflow2_core::schema::declared_defaults;
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

fn served_tools() -> Vec<rmcp::model::Tool> {
    let mut all = ReflowService::capture_router().list_all();
    for r in [
        ReflowService::assure_router(),
        ReflowService::exchange_router(),
        ReflowService::temporal_tools_router(),
        ReflowService::ask_router(),
        ReflowService::built_router(),
        ReflowService::coherence_router(),
        ReflowService::ingest_tools_router(),
        ReflowService::operate_tools_router(),
        ReflowService::query_router(),
        ReflowService::claims_tools_router(),
        ReflowService::skills_router(),
    ] {
        all.extend(r.list_all());
    }
    all
}

fn properties_of(tool: &rmcp::model::Tool) -> Vec<String> {
    tool.input_schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// The constructor that creates each status-carrying type. `DesignEpoch` is
/// the one whose tool name is not the type's snake case.
fn constructor_for(node_type: &str) -> String {
    match node_type {
        "DesignEpoch" => "add_epoch".to_string(),
        other => {
            let mut s = String::new();
            for (i, c) in other.chars().enumerate() {
                if c.is_uppercase() && i > 0 {
                    s.push('_');
                }
                s.push(c.to_ascii_lowercase());
            }
            format!("add_{s}")
        }
    }
}

#[test]
fn every_constructor_of_a_type_that_carries_a_status_takes_it_and_names_the_landing() {
    let tools = served_tools();
    // Every type whose schema declares a `status` default, plus Artifact,
    // whose default was deliberately removed on 2026-09-16 and which still
    // carries the property.
    let mut typed: Vec<String> = declared_defaults()
        .into_iter()
        .filter(|d| d.property == "status")
        .map(|d| d.node_type)
        .collect();
    typed.push("Artifact".into());
    typed.sort();
    typed.dedup();
    let mut checked = Vec::new();
    for ty in &typed {
        let name = constructor_for(ty);
        let Some(tool) = tools.iter().find(|t| t.name == name) else {
            // Question and Fragment are minted by ask/ingest, not by a
            // constructor; a type with no `add_*` has no sibling to disagree
            // with.
            continue;
        };
        let props = properties_of(tool);
        assert!(
            props.iter().any(|p| p == "status"),
            "{name} creates a {ty}, which carries a status, and offers no `status` parameter — \
             the F10 shape (a constructor writing a status it offers no way to say)"
        );
        let desc = tool.description.as_deref().unwrap_or("");
        assert!(
            desc.contains("status") || desc.contains("Lands"),
            "{name}'s description does not say what status omitting the field lands: {desc}"
        );
        checked.push(name);
    }
    checked.sort();
    assert_eq!(
        checked,
        vec![
            "add_artifact",
            "add_capability",
            "add_component",
            "add_decision",
            "add_epoch",
            "add_project",
            "add_release",
            "add_requirement",
            "add_verification",
        ],
        "the set of constructors under the contract moved — extend the test with the new sibling"
    );
}

#[test]
fn approver_is_offered_by_exactly_the_constructors_whose_status_is_settled_intent() {
    let tools = served_tools();
    let mut with_approver: Vec<String> = tools
        .iter()
        .filter(|t| t.name.starts_with("add_"))
        .filter(|t| properties_of(t).iter().any(|p| p == "approver"))
        .map(|t| t.name.to_string())
        .collect();
    with_approver.sort();
    // The three cases `tools/check_intent_authority.py` reads: a Requirement
    // off `proposed`, a Decision `accepted`/`deferred`, a DesignRule with
    // `enforced` stated. A build status is not one of them.
    assert_eq!(
        with_approver,
        vec!["add_decision", "add_design_rule", "add_requirement"],
        "approver belongs on settled intent and nowhere else"
    );
    // And every constructor that offers `approver` offers `acted_at` beside it.
    for t in tools
        .iter()
        .filter(|t| with_approver.contains(&t.name.to_string()))
    {
        assert!(
            properties_of(t).iter().any(|p| p == "acted_at"),
            "{} takes approver without acted_at",
            t.name
        );
    }
}

fn status_of(v: &Value) -> Option<String> {
    v["properties"]["status"].as_str().map(str::to_string)
}

#[tokio::test]
async fn a_status_passed_to_the_constructor_is_stored_and_an_omitted_one_lands_as_named() {
    let s = ReflowService::in_memory().expect("in-memory service");

    // Component: the F10 row — refused the field, returned `planned`.
    let built = j!(s.add_component(Parameters(
        serde_json::from_value::<ComponentReq>(json!({
            "id": "cmp:pump",
            "name": "Pump",
            "description": "Moves the water.",
            "status": "realized"
        }))
        .expect("status is a served field")
    )));
    assert_eq!(status_of(&built).as_deref(), Some("realized"));
    let fresh = j!(s.add_component(Parameters(
        serde_json::from_value::<ComponentReq>(json!({
            "id": "cmp:valve",
            "name": "Valve",
            "description": "Stops the water."
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&fresh).as_deref(), Some("planned"));

    // Release.
    let shipped = j!(s.add_release(Parameters(
        serde_json::from_value::<ReleaseReq>(json!({
            "id": "rel:v1",
            "name": "v1",
            "status": "deployed"
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&shipped).as_deref(), Some("deployed"));

    // Project.
    let paused = j!(s.add_project(Parameters(
        serde_json::from_value::<ProjectReq>(json!({
            "id": "proj:old",
            "name": "Old",
            "status": "paused"
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&paused).as_deref(), Some("paused"));

    // Epoch: the flo2 row — "returned `status: arrived`, which is the graph
    // stating that v0.2.0 has happened".
    let next_cut = j!(s.add_epoch(Parameters(
        serde_json::from_value::<AddEpochReq>(json!({
            "id": "epoch:v0.2.0",
            "name": "v0.2.0",
            "epoch_type": "release_cut",
            "sequence": 1,
            "status": "planned"
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&next_cut).as_deref(), Some("planned"));
    let past = j!(s.add_epoch(Parameters(
        serde_json::from_value::<AddEpochReq>(json!({
            "id": "epoch:v0.1.0",
            "name": "v0.1.0",
            "epoch_type": "release_cut",
            "sequence": 0
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&past).as_deref(), Some("arrived"));

    // A revise that omits the field leaves the stored status alone.
    let revised = j!(s.add_epoch(Parameters(
        serde_json::from_value::<AddEpochReq>(json!({
            "id": "epoch:v0.2.0",
            "description": "The next cut, not yet made."
        }))
        .unwrap()
    )));
    assert_eq!(status_of(&revised).as_deref(), Some("planned"));
}

#[tokio::test]
async fn a_status_the_schema_does_not_declare_is_refused_by_name() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let err = s
        .add_component(Parameters(
            serde_json::from_value::<ComponentReq>(json!({
                "id": "cmp:x",
                "name": "X",
                "description": "x",
                "status": "shipped"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("an undeclared status is refused");
    let m = err.to_string();
    assert!(m.contains("status"), "{m}");
}
