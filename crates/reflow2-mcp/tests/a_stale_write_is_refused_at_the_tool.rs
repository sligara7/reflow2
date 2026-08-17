//! The lost-update guard works through the TOOL, not only in the engine.
//!
//! # Why this exists separately from the core suite
//!
//! `tests/lost_update.rs` proves the engine refuses a stale write.
//! That is not the same claim as *a session cannot lose an update*, and this
//! project has been burned by exactly that gap before: three home-grown test
//! layers once agreed with each other and were all wrong, because each was a
//! client we wrote. The surface a user actually touches is the MCP tool, and
//! the round trip a caller makes is READ → get `revision.prior_content_hash` →
//! WRITE with it. This exercises that round trip.
//!
//! `req:a-write-cannot-silently-lose-someone-elses-work` — the `required`
//! obligation of `epoch:instruments-stop-overstating`, and the one where
//! somebody's work actually disappears.

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

fn node_req(id: &str, decision: &str, expected: Option<&str>) -> CreateNodeReq {
    serde_json::from_value(serde_json::json!({
        "node_type": "Decision",
        "id": id,
        "props": { "name": "shared", "decision": decision },
        "expected_content_hash": expected,
    }))
    .expect("request")
}

/// The full round trip a caller makes, and the collision in the middle.
#[tokio::test]
async fn a_write_against_a_stale_read_is_refused_at_the_tool() {
    let s = svc().await;
    j!(s.create_node(Parameters(node_req("dec:shared", "first draft", None))));

    // I read it, and take the hash the tool hands back. A second write is how a
    // caller obtains `prior_content_hash` today, which is itself worth noting:
    // the value exists on the WRITE path, not the read path.
    let mine = j!(s.create_node(Parameters(node_req("dec:shared", "first draft", None))));
    let my_hash = mine["revision"]["prior_content_hash"]
        .as_str()
        .expect("the revision block carries the hash a CAS needs")
        .to_string();

    // Somebody else writes in between, unguarded and correct.
    j!(s.create_node(Parameters(node_req(
        "dec:shared",
        "their careful rewrite",
        None
    ))));

    // My edit, against what I read.
    let refused = s
        .create_node(Parameters(node_req(
            "dec:shared",
            "my edit",
            Some(&my_hash),
        )))
        .await;

    let err = refused.expect_err("a stale write must be refused at the tool");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("has changed since you read it"),
        "the refusal must reach the caller intact: {msg}"
    );

    // …and their work survived, which is the assertion the requirement is about.
    let now = j!(s.get_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type": "Decision", "id": "dec:shared"
        }))
        .unwrap()
    )));
    assert_eq!(
        now["node"]["properties"]["decision"].as_str(),
        Some("their careful rewrite"),
        "the refusal has to happen BEFORE the overwrite, not after"
    );
}

/// The counterweight: without it the guard could refuse everything and the test
/// above would still pass.
#[tokio::test]
async fn an_unraced_write_still_goes_through_at_the_tool() {
    let s = svc().await;
    j!(s.create_node(Parameters(node_req("dec:solo", "first draft", None))));
    let mine = j!(s.create_node(Parameters(node_req("dec:solo", "first draft", None))));
    let my_hash = mine["revision"]["prior_content_hash"]
        .as_str()
        .unwrap()
        .to_string();

    j!(s.create_node(Parameters(node_req(
        "dec:solo",
        "my edit, unraced",
        Some(&my_hash)
    ))));

    let now = j!(s.get_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type": "Decision", "id": "dec:solo"
        }))
        .unwrap()
    )));
    assert_eq!(
        now["node"]["properties"]["decision"].as_str(),
        Some("my edit, unraced")
    );
}

/// Omitting the expectation keeps the old behaviour. The guard is opt-in, and a
/// change that quietly made every existing writer start failing would be a
/// worse defect than the one it fixes.
#[tokio::test]
async fn a_caller_who_states_no_expectation_is_unaffected() {
    let s = svc().await;
    j!(s.create_node(Parameters(node_req("dec:legacy", "first", None))));
    j!(s.create_node(Parameters(node_req("dec:legacy", "second", None))));
    let now = j!(s.get_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type": "Decision", "id": "dec:legacy"
        }))
        .unwrap()
    )));
    assert_eq!(
        now["node"]["properties"]["decision"].as_str(),
        Some("second")
    );
}
