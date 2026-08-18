//! The seedless orientation read is reachable, and answers with no arguments.
//!
//! `req:a-session-with-no-seed-can-still-orient`.
//!
//! # Why an MCP-level probe, when the core is already covered
//!
//! Because the last four defects in this epoch were all the same shape: a
//! capability that existed in the engine and could not be reached from a
//! client. `Verification.description` was declared, fulltext and unwritable —
//! one use in 164. `SUPERSEDES` was declared and had zero edges. `create_node`
//! shipped a compare-and-swap whose precondition value it did not return, and
//! every core test passed while the very first MCP call failed.
//!
//! A tool that needs no arguments is exactly where that family hides, because
//! there is nothing to get wrong in the signature and therefore nothing a
//! typed test would catch. So this drives the surface: **the call takes an
//! empty argument object**, which is the requirement's whole claim.

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

#[tokio::test]
async fn the_orientation_read_takes_an_empty_argument_object() {
    // A session at check-in has no seed, no scope and no topic. If this call
    // needed any of them the requirement would be unmet however good the engine
    // underneath is.
    let s = svc().await;
    let v = j!(s.design_regions(Parameters(
        serde_json::from_value(serde_json::json!({})).unwrap()
    )));

    assert!(
        v.get("regions").is_some(),
        "the seedless call must answer with regions, arguments or not: {v:?}"
    );
    assert!(
        v.get("coverage").is_some(),
        "and with what it did not cover, or the rows read as a partition"
    );
    assert_eq!(
        v.get("depth").and_then(serde_json::Value::as_u64),
        Some(1),
        "the depth it used must be in the reply — sizes are unreadable without it"
    );
}

#[tokio::test]
async fn an_empty_design_still_answers_in_words() {
    // The reply a brand-new project gets. `regions: []` alone is
    // indistinguishable from "your design has no problems"; the note is the
    // whole difference.
    let s = svc().await;
    let v = j!(s.design_regions(Parameters(
        serde_json::from_value(serde_json::json!({})).unwrap()
    )));

    let note = v
        .get("note")
        .and_then(serde_json::Value::as_str)
        .expect("an empty listing must carry its note through the surface, not just in the core");
    assert!(
        note.contains("VACUOUS"),
        "and the note must name the emptiness: {note}"
    );
}

#[tokio::test]
async fn the_depth_the_caller_asks_for_is_the_depth_it_gets() {
    // The parameter is the escape hatch from this tool's default of 1, which
    // deliberately differs from the scoped detectors' 3
    // (`dec:the-default-scope-depth-should-be-two`). A depth that were silently
    // ignored would make every size in the reply a number about a radius the
    // caller did not choose.
    let s = svc().await;
    let v = j!(s.design_regions(Parameters(
        serde_json::from_value(serde_json::json!({"depth": 4})).unwrap()
    )));
    assert_eq!(
        v.get("depth").and_then(serde_json::Value::as_u64),
        Some(4),
        "the reply echoes the depth actually used: {v:?}"
    );
}
