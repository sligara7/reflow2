//! A status question, asked the way a person asks it, finds a tool that READS.
//!
//! FIELD REPORT, 2026-09-14 (hxm_program): asked for a progress deck for
//! management, an agent asserted the project's status from a checklist file
//! and its own recent writes and never read the design. Challenged, one
//! `topic_report` call on the project's name gave it twenty hits across nine
//! node types. It did not know the tool was there.
//!
//! MEASURED on the way here: `find_tools` for *"put together a progress deck
//! for management on what this project has achieved so far"* returned
//! add_project, set_project_mode, schedule_for, set_violation_status,
//! dimension_drifts and contains — six tools and not one of them reads
//! status. The catalogue was written from the design's side ("one subject",
//! "where things stand") and not from the asker's ("what have we achieved",
//! "a deck for management"), so a person's words reached nothing.
//!
//! `every_tool_is_found_by_a_query_in_the_users_words.rs` pins one query per
//! tool. This pins the one query that failed in the field: the status
//! question, in the asker's words, finds the read that would have answered it.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn top5(s: &ReflowService, query: &str) -> Vec<String> {
    let v: Value = s
        .find_tools(Parameters(
            serde_json::from_value(json!({"query": query, "limit": 5})).unwrap(),
        ))
        .await
        .expect("find_tools")
        .structured_content
        .expect("structured");
    v["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["tool"].as_str().map(String::from))
        .collect()
}

/// ⭐ THE QUERY FROM THE FIELD. The exact job the user stated, in their words.
#[tokio::test]
async fn a_progress_deck_for_management_finds_the_topic_read() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let q = "put together a progress deck for management on what this project has achieved so far";
    let got = top5(&s, q).await;
    assert!(
        got.iter().any(|t| t == "topic_report"),
        "the status question must find the read that answers it; top5 for {q:?}: {got:?}"
    );
}

/// The same question the way a returning owner asks it.
#[tokio::test]
async fn where_does_this_project_stand_finds_the_topic_read() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let q = "where does this project stand, what has been achieved and what is still open";
    let got = top5(&s, q).await;
    assert!(
        got.iter().any(|t| t == "topic_report"),
        "top5 for {q:?}: {got:?}"
    );
}
