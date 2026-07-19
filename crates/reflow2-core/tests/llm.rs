//! LlmBackend + MockLlmBackend, and the first op wired through the boundary.

use reflow2_core::{
    DesignGraph, GapSource, LlmBackend, LlmError, LlmRequest, MockLlmBackend, complete_json,
};
use serde::Deserialize;

#[test]
fn mock_resolution_order_rules_then_queue_then_default() {
    let mock = MockLlmBackend::new()
        .with_default("DEFAULT")
        .push("FIRST")
        .push("SECOND")
        .on_contains("urgent", "RULE");

    // Rule wins regardless of queue position.
    assert_eq!(
        mock.complete(&LlmRequest::new("this is urgent"))
            .unwrap()
            .text,
        "RULE"
    );
    // Then the queue, FIFO.
    assert_eq!(mock.complete(&LlmRequest::new("a")).unwrap().text, "FIRST");
    assert_eq!(mock.complete(&LlmRequest::new("b")).unwrap().text, "SECOND");
    // Then the default, once the queue is drained.
    assert_eq!(
        mock.complete(&LlmRequest::new("c")).unwrap().text,
        "DEFAULT"
    );

    assert_eq!(mock.call_count(), 4);
    assert_eq!(mock.calls()[0].prompt, "this is urgent");
}

#[test]
fn mock_with_no_response_fails_loud() {
    let mock = MockLlmBackend::new(); // no default, no queue, no rules
    assert_eq!(
        mock.complete(&LlmRequest::new("anything")),
        Err(LlmError::NoResponse)
    );
}

#[test]
fn backend_is_object_safe() {
    // The whole point: the core holds a provider-neutral trait object.
    let backend: Box<dyn LlmBackend> = Box::new(MockLlmBackend::new().with_default("ok"));
    assert_eq!(backend.name(), "mock");
    assert_eq!(backend.complete(&LlmRequest::new("x")).unwrap().text, "ok");
}

#[derive(Debug, Deserialize, PartialEq)]
struct Verdict {
    same: bool,
    reason: String,
}

#[test]
fn complete_json_parses_structured_output() {
    let mock =
        MockLlmBackend::new().with_default(r#"{"same": true, "reason": "identical intent"}"#);
    let verdict: Verdict = complete_json(&mock, &LlmRequest::new("are these the same?")).unwrap();
    assert_eq!(
        verdict,
        Verdict {
            same: true,
            reason: "identical intent".to_string()
        }
    );
    // complete_json sets the JSON hint on the request the backend saw.
    assert!(mock.calls()[0].expect_json);
}

#[test]
fn complete_json_reports_parse_failure() {
    let mock = MockLlmBackend::new().with_default("not json at all");
    let result: Result<Verdict, _> = complete_json(&mock, &LlmRequest::new("q"));
    assert!(matches!(result, Err(LlmError::Parse(_))));
}

// ---- The first LLM-reasoning op wired through the boundary: GapPrompt --------

fn graph_with_a_gap() -> DesignGraph {
    // A capability with no allocation → an `unallocated_capability` gap.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Local caching", "serve reads on-device", None)
        .unwrap();
    g.add_component("cmp:a", "Store", "kv store", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g
}

#[test]
fn gap_becomes_a_plain_question_via_the_backend() {
    let g = graph_with_a_gap();
    let gap = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|c| c.gap_source == GapSource::UnallocatedCapability)
        .expect("an unallocated-capability gap");

    let mock =
        MockLlmBackend::new().with_default("Which part of the system should own local caching?");
    let prompt = gap.to_prompt(&mock);

    assert_eq!(
        prompt.question,
        "Which part of the system should own local caching?"
    );
    assert!(!prompt.rephrase_degraded);
    assert_eq!(prompt.candidate_id, gap.id);
    // The op actually went through the backend.
    assert_eq!(mock.call_count(), 1);
}

#[test]
fn prompt_degrades_gracefully_when_the_backend_fails() {
    let g = graph_with_a_gap();
    let gap = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|c| c.gap_source == GapSource::UnallocatedCapability)
        .unwrap();

    // A dry mock fails → we must degrade, not drop, and flag it.
    let dry = MockLlmBackend::new();
    let prompt = gap.to_prompt(&dry);

    assert!(
        prompt.rephrase_degraded,
        "failure must be flagged, not hidden"
    );
    assert_eq!(
        prompt.question, gap.description,
        "falls back to raw wording"
    );
    assert_eq!(
        prompt.candidate_id, gap.id,
        "the candidate is never dropped"
    );
}
