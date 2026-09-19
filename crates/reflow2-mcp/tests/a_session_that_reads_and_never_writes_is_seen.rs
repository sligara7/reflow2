//! A session that reads the design at length and writes nothing is the one
//! session every loop signal cannot see — all of them count nodes that exist.
//! flo2, 2026-09-18: a document review produced six design findings in chat
//! and recorded none; `loop_status` was clean. Now the loop counts this
//! session's reads and writes and, past a threshold with no write, says so in
//! `next` — for a session that could write. Pinned here: the count is per
//! session, the line appears past the threshold, one write settles it, and a
//! read-only surface is never told it owes a finding.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn loop_status(s: &ReflowService) -> Value {
    let req: LoopScopeReq = serde_json::from_value(json!({})).unwrap();
    s.loop_status(Parameters(req))
        .await
        .expect("loop_status")
        .structured_content
        .expect("structured")
}

#[tokio::test]
async fn twenty_five_reads_and_no_write_puts_a_line_in_next_and_one_write_removes_it() {
    let s = ReflowService::in_memory().expect("service");
    for _ in 0..25 {
        s.note_call_for_test("search_design");
    }
    let ls = loop_status(&s).await;
    assert_eq!(ls["session"]["writes"], 0, "{ls}");
    assert_eq!(ls["session"]["reads"], 25, "{ls}");
    let next = ls["next"].to_string();
    assert!(
        next.contains("THIS SESSION") && next.contains("no write"),
        "{next}"
    );

    s.note_call_for_test("add_capability");
    let ls = loop_status(&s).await;
    assert_eq!(ls["session"]["writes"], 1, "{ls}");
    assert!(!ls["next"].to_string().contains("THIS SESSION"), "{ls}");
}

#[tokio::test]
async fn below_the_threshold_nothing_is_said_and_the_count_is_still_there() {
    let s = ReflowService::in_memory().expect("service");
    for _ in 0..5 {
        s.note_call_for_test("get_node");
    }
    let ls = loop_status(&s).await;
    assert_eq!(ls["session"]["reads"], 5, "{ls}");
    assert!(!ls["next"].to_string().contains("THIS SESSION"), "{ls}");
}

#[tokio::test]
async fn a_read_only_surface_is_never_told_it_owes_a_finding() {
    let s = ReflowService::in_memory()
        .expect("service")
        .into_read_only();
    for _ in 0..40 {
        s.note_call_for_test("search_design");
    }
    let ls = loop_status(&s).await;
    assert_eq!(ls["session"]["read_only"], true, "{ls}");
    assert!(!ls["next"].to_string().contains("THIS SESSION"), "{ls}");
}
