//! `add_capability` draws the golden thread in the call that creates the node.
//!
//! F-02, hxm_program: *"An `add_capability(satisfies=req:X)` parameter that
//! draws the SATISFIES edge would remove the retry AND the separate edge call."*
//!
//! ⚠️ HALF OF F-02 WAS ALREADY FIXED AND THIS DOES NOT RE-FIX IT. The report's
//! headline complaint was the duplicate guard refusing every capability written
//! after its requirement — six times in one session. `PRESCRIBED_LAYER_PAIRS`
//! landed 2026-08-31 with `("Requirement","Capability")` as its FIRST entry and
//! `("Requirement","Decision")` beside it, so both cases hxm named are exempt
//! and the refusal is gone. What remains is the ergonomics half: the thread
//! still costs three calls where the constructor could draw it in one.
//!
//! THE PRECEDENT IS ESTABLISHED, which is why this is a gap rather than a
//! design question: `add_verification` takes `verifies` ("Recording one check
//! used to take four to six calls"), and `add_decision` takes `related_to` and
//! `approver`. `add_capability` — the busiest constructor on the golden thread
//! — takes neither of its two edges.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    s.add_requirement(Parameters(
        serde_json::from_value(json!({
            "id": "req:thing",
            "name": "The thing must happen",
            "statement": "The thing must happen, reliably.",
        }))
        .unwrap(),
    ))
    .await
    .expect("add_requirement");
    s.add_component(Parameters(
        serde_json::from_value(
            json!({"id": "cmp:part", "name": "The part", "description": "The part that does it."}),
        )
        .unwrap(),
    ))
    .await
    .expect("add_component");
    s
}

/// Read the edge back through the served surface rather than trusting the
/// reply that claims to have drawn it — an echo that lies and an edge that
/// exists must not be the same assertion.
async fn ring(s: &ReflowService, seed: &str) -> Vec<(String, String)> {
    let v: Value = s
        .propagate_from(Parameters(
            serde_json::from_value(json!({"seed_ids": [seed]})).unwrap(),
        ))
        .await
        .expect("propagate_from")
        .structured_content
        .expect("structured");
    v["direct_ring"]
        .as_array()
        .expect("direct_ring")
        .iter()
        .map(|r| {
            (
                r["edge_type"].as_str().unwrap_or_default().to_string(),
                r["node_id"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// THE CASE. One call creates the capability, satisfies the requirement and
/// allocates to the component.
#[tokio::test]
async fn one_call_creates_the_capability_and_draws_both_edges() {
    let s = svc().await;
    s.add_capability(Parameters(
        serde_json::from_value(json!({
            "id": "cap:thing",
            "name": "Do the thing",
            "description": "Does the thing.",
            "satisfies": "req:thing",
            "allocated_to": "cmp:part",
        }))
        .unwrap(),
    ))
    .await
    .expect("add_capability with thread");

    let r = ring(&s, "cap:thing").await;
    assert!(
        r.contains(&("SATISFIES".into(), "req:thing".into())),
        "satisfies must draw the SATISFIES edge: {r:?}"
    );
    assert!(
        r.contains(&("ALLOCATED_TO".into(), "cmp:part".into())),
        "allocated_to must draw the ALLOCATED_TO edge: {r:?}"
    );
}

/// Omitting them changes nothing — the parameters are additive, and a capability
/// with no thread yet is a normal thing to write.
#[tokio::test]
async fn omitting_the_thread_draws_nothing_and_still_creates_the_node() {
    let s = svc().await;
    s.add_capability(Parameters(
        serde_json::from_value(json!({"id": "cap:bare", "name": "Bare", "description": "x"}))
            .unwrap(),
    ))
    .await
    .expect("add_capability");
    let r = ring(&s, "cap:bare").await;
    assert!(
        !r.iter()
            .any(|(e, _)| e == "SATISFIES" || e == "ALLOCATED_TO"),
        "no thread was asked for, so none may be drawn: {r:?}"
    );
}

/// An id that names nothing is REFUSED before the node is written, rather than
/// leaving a capability behind with a dangling half-thread. Same posture as
/// `approver` on the settling constructors: a typo must not attach the thread
/// to something that does not exist.
#[tokio::test]
async fn a_thread_end_that_names_nothing_is_refused() {
    let s = svc().await;
    let err = s
        .add_capability(Parameters(
            serde_json::from_value(json!({
                "id": "cap:typo",
                "name": "Typo",
                "description": "x",
                "satisfies": "req:no-such-thing",
            }))
            .unwrap(),
        ))
        .await
        .expect_err("must refuse an unknown satisfies target");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("req:no-such-thing"),
        "the refusal must name the id that resolved to nothing: {msg}"
    );
    let node = s
        .get_node(Parameters(
            serde_json::from_value(json!({"id": "cap:typo"})).unwrap(),
        ))
        .await
        .expect("get_node")
        .structured_content
        .expect("structured");
    assert!(
        node["node"].is_null(),
        "nothing may be written when the thread cannot be drawn: {node}"
    );
}
