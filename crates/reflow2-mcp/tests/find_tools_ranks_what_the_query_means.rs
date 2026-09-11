//! `find_tools` ranks the tool that ANSWERS a query in a user's words above
//! tools that merely contain more of its words.
//!
//! # Why these six
//!
//! Each was a MISS on 2026-09-11 (absent from the top 10) in the 180-query
//! criterion-1 corpus, and each is found by the corrected scorer — measured
//! over the whole corpus, not predicted: 53 misses → 42, with zero tools
//! regressing. They are the served-surface half of the fix pinned in
//! `service.rs::find_tools_scoring_invariants`.
//!
//! ⚠️ TWO FIXTURES WERE DROPPED, AND WHY IS THE HONEST PART. A Python replica
//! of the scorer predicted `satisfies` and `get_skill` would flip; the real
//! server, re-measured after the fix, still misses both. `get_skill` ←
//! "playbook" is a near-vocabulary-gap (the word is nowhere on the tool), and
//! `satisfies` loses to `add_capability`, whose description genuinely names
//! satisfying. The replica overstated the stopword half throughout (it
//! dropped a hand list; the real fix zero-weights only terms present in EVERY
//! entry). A fixture is a claim about the served surface, so it carries only
//! what the served surface does. The twelve zero-overlap misses
//! (`add_requirement`, `graph_report`, `get_node`…) are not here either —
//! no scoring change can reach them
//! (`fact:find-tools-misses-split-into-a-vocabulary-gap…`).
//!
//! Top 5 is the bar because 5 is what a consumer sees by default.

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

const FIXTURES: &[(&str, &str)] = &[
    (
        "realizes",
        "record that this artifact implements that capability",
    ),
    ("delete_edge", "remove a link between two items"),
    (
        "allocate",
        "say which component is responsible for this capability",
    ),
    (
        "deploy_to",
        "record that this release runs in that environment",
    ),
    (
        "part_of_flow",
        "add this capability as a step in that process",
    ),
    ("contains", "attach this item under the project"),
];

#[tokio::test]
async fn a_query_in_a_users_words_finds_the_tool_that_answers_it() {
    let s = ReflowService::in_memory().expect("service");
    let mut missed = Vec::new();
    for (tool, query) in FIXTURES {
        let got = top5(&s, query).await;
        if !got.iter().any(|t| t == tool) {
            missed.push(format!("{tool:<14} ← {query:?}\n      top5: {got:?}"));
        }
    }
    assert!(
        missed.is_empty(),
        "{} of {} fixtures not in the top 5:\n  {}",
        missed.len(),
        FIXTURES.len(),
        missed.join("\n  ")
    );
}
