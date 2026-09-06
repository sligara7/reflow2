//! Convention (i), Anthony 2026-09-06: wherever a tool names an EXISTING node by
//! a type+id pair, the type is optional — resolved from the id, since the id
//! prefix names the type by convention — and a COLLISION (one id under two
//! types) is refused naming both, never guessed. Constructors keep the type
//! required (you cannot create a thing without saying what it is), and
//! `delete_node` keeps it required on purpose: the one destructive read.
//! fact:defect-sibling-constructors-disagree-on-how-to-name-a-node-and-what-status-it-lands-in
//! fact:defect-typed-tool-parameter-names-are-inconsistent
//! dec:idea-one-way-to-name-which-node-across-the-tool-surface
//!
//! Written first and observed failing to compile: the type fields were `String`.
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

async fn seeded() -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:fast".into(),
        name: Some("Fast".into()),
        statement: Some("Answer in under a second.".into()),
        distinct_from: None,
    })));
    j!(s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "ver:fast", "name": "Fast check", "description": "Times the answer."
        }))
        .unwrap()
    )));
    j!(s.add_decision(Parameters(serde_json::from_value(serde_json::json!({
        "id": "dec:fast", "name": "Fast is a hard budget", "decision": "One second, p50.", "rationale": "Users leave."
    })).unwrap())));
    s
}

#[tokio::test]
async fn verifies_resolves_the_target_type_from_the_id() {
    let s = seeded().await;
    let out = j!(s.verifies(Parameters(
        serde_json::from_value(serde_json::json!({
            "verification_id": "ver:fast", "target_id": "req:fast"
        }))
        .unwrap()
    )));
    assert_eq!(out["to_id"], "req:fast", "{out}");
}

#[tokio::test]
async fn governed_by_resolves_both_ends_from_their_ids() {
    let s = seeded().await;
    let out = j!(s.governed_by(Parameters(
        serde_json::from_value(serde_json::json!({
            "from_id": "req:fast", "to_id": "dec:fast"
        }))
        .unwrap()
    )));
    assert_eq!(out["edge_type"], "GOVERNED_BY", "{out}");
}

#[tokio::test]
async fn a_collision_is_refused_and_names_both_types() {
    let s = seeded().await;
    // The same id under a second type: a convention violation, but writable.
    j!(s.create_node(Parameters(serde_json::from_value(serde_json::json!({
        "node_type": "Capability", "id": "req:fast", "props": {"name": "Impostor", "description": "same id"}
    })).unwrap())));
    let e = s
        .verifies(Parameters(
            serde_json::from_value(serde_json::json!({
                "verification_id": "ver:fast", "target_id": "req:fast"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("two types hold req:fast");
    let m = format!("{e:?}");
    assert!(
        m.contains("Requirement") && m.contains("Capability") && m.contains("target_type"),
        "names both and the field to pass: {m}"
    );
    // Passing the type still works — that is the way through.
    let out = j!(s.verifies(Parameters(
        serde_json::from_value(serde_json::json!({
            "verification_id": "ver:fast", "target_type": "Requirement", "target_id": "req:fast"
        }))
        .unwrap()
    )));
    assert_eq!(out["to_id"], "req:fast");
}

#[tokio::test]
async fn an_unknown_id_says_so_rather_than_guessing_a_type() {
    let s = seeded().await;
    let e = s
        .verifies(Parameters(
            serde_json::from_value(serde_json::json!({
                "verification_id": "ver:fast", "target_id": "req:nope"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("no such node");
    let m = format!("{e:?}");
    assert!(
        m.contains("req:nope") && m.to_lowercase().contains("no node"),
        "{m}"
    );
}

#[test]
fn delete_node_still_requires_the_type_on_purpose() {
    // The one destructive read keeps the strict form: a typo'd id that happens
    // to exist under another type must not delete that other thing.
    let r: Result<TypedIdReq, _> = serde_json::from_value(serde_json::json!({ "id": "req:fast" }));
    assert!(
        r.is_err(),
        "delete_node's request must still demand node_type"
    );
}
