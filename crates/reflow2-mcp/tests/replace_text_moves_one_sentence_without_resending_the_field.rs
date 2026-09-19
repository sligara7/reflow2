//! `replace_text`: one sentence of a node's text moves without re-sending the
//! field. Three projects reported the cost (qs, dev_storyflow 2026-09-04;
//! flo2 2026-09-18): a 6 KB decision body re-sent to move one line, or a line
//! left standing false. Pinned: a unique `old` is replaced and the prior state
//! is preserved; an absent or repeated `old` is refused with the count and
//! nothing is written; omitting `old` appends; a non-text field is refused by
//! name.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

const BODY: &str = "OPEN — which store?\n\nOptions, none chosen.\n\n(a) RocksDB. (b) JSON. The word store appears twice: store.";

async fn seeded() -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    let req: DecisionReq = serde_json::from_value(json!({
        "id": "dec:store", "name": "Which store", "decision": BODY, "rationale": "r"
    }))
    .unwrap();
    s.add_decision(Parameters(req)).await.expect("decision");
    s
}

async fn edit(s: &ReflowService, body: Value) -> Result<Value, String> {
    let req: ReplaceTextReq = serde_json::from_value(body).expect("request shape");
    s.replace_text(Parameters(req))
        .await
        .map(|r| r.structured_content.expect("structured"))
        .map_err(|e| e.message.to_string())
}

async fn decision_text(s: &ReflowService) -> String {
    let req: GetNodeReq = serde_json::from_value(json!({"id": "dec:store"})).unwrap();
    s.get_node(Parameters(req))
        .await
        .unwrap()
        .structured_content
        .unwrap()["node"]["properties"]["decision"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn a_unique_sentence_is_replaced_and_the_prior_state_is_preserved() {
    let s = seeded().await;
    let out = edit(
        &s,
        json!({"node_id": "dec:store", "field": "decision", "old": "Options, none chosen.",
               "new": "SETTLED 2026-09-18: option (a), RocksDB."}),
    )
    .await
    .expect("edit");
    assert_eq!(out["edit"]["changed"], true, "{out}");
    assert_eq!(out["revision"]["replaced"][0]["field"], "decision", "{out}");
    assert!(
        out["revision"]["prior_state_preserved_in"]
            .as_str()
            .unwrap_or("")
            .contains("preserved-on-write"),
        "{out}"
    );
    let text = decision_text(&s).await;
    assert!(
        text.contains("SETTLED 2026-09-18") && !text.contains("none chosen"),
        "{text}"
    );
    assert!(
        text.starts_with("OPEN — which store?"),
        "the rest of the body is untouched: {text}"
    );
}

#[tokio::test]
async fn an_absent_or_repeated_old_is_refused_with_the_count_and_nothing_is_written() {
    let s = seeded().await;
    let err = edit(
        &s,
        json!({"node_id": "dec:store", "field": "decision", "old": "not there", "new": "x"}),
    )
    .await
    .expect_err("absent");
    assert!(err.contains("occurs 0 time(s)"), "{err}");
    let err = edit(
        &s,
        json!({"node_id": "dec:store", "field": "decision", "old": "store", "new": "x"}),
    )
    .await
    .expect_err("repeated");
    assert!(err.contains("occurs 3 time(s)"), "{err}");
    assert_eq!(decision_text(&s).await, BODY, "nothing was written");
}

#[tokio::test]
async fn omitting_old_appends_after_a_blank_line() {
    let s = seeded().await;
    let out = edit(&s, json!({"node_id": "dec:store", "field": "decision", "new": "═══ 2026-09-18 ═══\nA dated note."}))
        .await
        .expect("append");
    assert_eq!(out["edit"]["mode"], "append", "{out}");
    let text = decision_text(&s).await;
    assert!(
        text.ends_with("\n\n═══ 2026-09-18 ═══\nA dated note."),
        "{text}"
    );
    assert!(text.starts_with(BODY), "{text}");
}

#[tokio::test]
async fn a_field_that_is_not_text_is_refused_by_name() {
    let s = seeded().await;
    let err = edit(
        &s,
        json!({"node_id": "dec:store", "field": "no_such_field", "old": "a", "new": "b"}),
    )
    .await
    .expect_err("refused");
    assert!(
        err.contains("no text property `no_such_field`") && err.contains("decision"),
        "{err}"
    );
}
