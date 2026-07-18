//! Ambient-agent LlmBackend adapter (SP-2): the collect-then-serve handshake.

use reflow2_core::{
    AgentAnswer, AgentBackend, DesignGraph, GapSource, LlmBackend, LlmError, LlmRequest,
    PromptCollector, prompt_id,
};

#[test]
fn prompt_id_is_stable_and_content_addressed() {
    let a = LlmRequest::new("same question").with_system("framing");
    let b = LlmRequest::new("same question").with_system("framing");
    // Same semantic content → same id (reproducible across prepare/serve).
    assert_eq!(prompt_id(&a), prompt_id(&b));

    // Any of system / prompt / json-hint differing → a different id.
    assert_ne!(
        prompt_id(&a),
        prompt_id(&LlmRequest::new("other question").with_system("framing"))
    );
    assert_ne!(
        prompt_id(&a),
        prompt_id(&LlmRequest::new("same question").with_system("other framing"))
    );
    assert_ne!(
        prompt_id(&a),
        prompt_id(
            &LlmRequest::new("same question")
                .with_system("framing")
                .expecting_json()
        )
    );
}

#[test]
fn collector_records_prompts_in_order_and_dedups() {
    let collector = PromptCollector::new();
    // Running an op under it must not error — it returns a stub so the op finishes.
    collector.complete(&LlmRequest::new("first")).unwrap();
    collector.complete(&LlmRequest::new("second")).unwrap();
    collector.complete(&LlmRequest::new("first")).unwrap(); // duplicate

    let collected = collector.collected();
    assert_eq!(collected.len(), 2, "the duplicate collapses to one prompt");
    assert_eq!(collected[0].prompt, "first");
    assert_eq!(collected[1].prompt, "second");
    // The id an agent will echo back matches the request's content id.
    assert_eq!(collected[0].id, prompt_id(&LlmRequest::new("first")));
}

#[test]
fn agent_backend_serves_by_id_and_fails_loud_on_miss() {
    let req = LlmRequest::new("what owns caching?");
    let answers = vec![AgentAnswer {
        id: prompt_id(&req),
        text: "The storage subsystem.".to_string(),
    }];
    let backend = AgentBackend::from_answers(answers);

    assert_eq!(
        backend.complete(&req).unwrap().text,
        "The storage subsystem."
    );

    // A prompt the agent was never asked to fill is a desync → fail loud, not a
    // silent empty/default answer.
    let miss = backend.complete(&LlmRequest::new("unasked prompt"));
    assert!(matches!(miss, Err(LlmError::Backend(_))));
}

#[test]
fn agent_backend_reports_unused_answers() {
    let asked = LlmRequest::new("asked");
    let backend = AgentBackend::from_answers(vec![
        AgentAnswer {
            id: prompt_id(&asked),
            text: "used".to_string(),
        },
        AgentAnswer {
            id: "deadbeefdeadbeef".to_string(),
            text: "stale".to_string(),
        },
    ]);

    backend.complete(&asked).unwrap();

    // The stale answer is surfaced, not silently dropped.
    assert_eq!(
        backend.unused_answers(),
        vec!["deadbeefdeadbeef".to_string()]
    );
}

// ---- End-to-end: a real op round-tripped through the ambient agent ----------

fn graph_with_a_gap() -> DesignGraph {
    // A capability with no allocation → an `unallocated_capability` gap.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Local caching", "serve reads on-device")
        .unwrap();
    g.add_component("cmp:a", "Store", "kv store", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g
}

#[test]
fn collect_then_serve_round_trips_to_prompt() {
    let g = graph_with_a_gap();
    let gap = g
        .detect_gaps()
        .unwrap()
        .into_iter()
        .find(|c| c.gap_source == GapSource::UnallocatedCapability)
        .expect("an unallocated-capability gap");

    // --- Prepare pass: harvest the prompt the op would issue ---
    let collector = PromptCollector::new();
    let _discarded = gap.to_prompt(&collector); // result is not meaningful here
    let prompts = collector.collected();
    assert_eq!(prompts.len(), 1, "to_prompt issues exactly one LLM call");

    // --- The agent fills it in-context ---
    let answer = "Which part of the system should own local caching?";
    let answers = vec![AgentAnswer {
        id: prompts[0].id.clone(),
        text: answer.to_string(),
    }];

    // --- Serve pass: replay the SAME op; determinism means the id matches ---
    let backend = AgentBackend::from_answers(answers);
    let prompt = gap.to_prompt(&backend);

    assert_eq!(prompt.question, answer);
    assert!(
        !prompt.rephrase_degraded,
        "the agent answered, so this is not a degraded fallback"
    );
    assert_eq!(prompt.candidate_id, gap.id);
    assert!(
        backend.unused_answers().is_empty(),
        "the single answer was consumed"
    );
}
