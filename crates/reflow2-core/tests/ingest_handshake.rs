//! SP-3b — INGEST driven by the ambient agent, with no LLM provider.
//!
//! `ingest` needs an `LlmBackend`; the agent-native surface has none, because
//! the *calling agent* is the model and cannot be reached mid-op. Until now that
//! made the whole extraction pipeline — provenance, time-aware resolution, the
//! resolution bands, the structural subset pass — unreachable from a session.

use reflow2_core::agent::AgentAnswer;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;
use reflow2_core::{IngestOptions, IngestStep};

const BRIEF: &str = "Build a widget that serves reads fast and works offline.";

/// Answer whatever the round asked, by matching on the pass tag in the prompt.
fn answer(prompt: &str) -> &'static str {
    if prompt.contains("[pass:project_intent]") {
        r#"{"project":{"id":"proj:w","name":"Widget","mode":"flexible"}}"#
    } else if prompt.contains("[pass:requirements]") {
        r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms"}]}"#
    } else if prompt.contains("[pass:capabilities]") {
        r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads"}]}"#
    } else if prompt.contains("[pass:discovery]") {
        r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#
    } else if prompt.contains("[pass:components]") {
        r#"{"components":[{"id":"cmp:store","name":"Store","purpose":"holds reads","allocated_capability_ids":["cap:cache"]}]}"#
    } else if prompt.contains("[pass:satisfies]") {
        r#"{"satisfies":[{"capability_id":"cap:cache","requirement_id":"req:lat"}]}"#
    } else {
        // Constraints, dependencies, and anything else: a valid empty answer.
        r#"{}"#
    }
}

/// Drive the handshake to completion, returning the rounds it took.
fn run(g: &mut DesignGraph) -> (usize, Box<reflow2_core::IngestReport>) {
    let mut answers: Vec<AgentAnswer> = Vec::new();
    let opts = IngestOptions::default();
    for round in 1..=10 {
        match g.ingest_step(BRIEF, &opts, answers.clone()).unwrap() {
            IngestStep::NeedsLlm { prompts, .. } => {
                assert!(!prompts.is_empty(), "NeedsLlm must name what it needs");
                for p in prompts {
                    let text = answer(&p.prompt).to_string();
                    answers.push(AgentAnswer { id: p.id, text });
                }
            }
            IngestStep::Done { report } => return (round, report),
        }
    }
    panic!("handshake did not converge");
}

#[test]
fn the_agent_can_drive_ingest_with_no_provider() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let (rounds, report) = run(&mut g);

    assert!(
        rounds > 1,
        "a content-branching pipeline needs repeated rounds"
    );
    assert!(report.nodes_created > 0, "{report:?}");

    // The design was actually written — the golden thread, not just a report.
    assert!(g.get_node(node::REQUIREMENT, "req:lat").unwrap().is_some());
    assert!(g.get_node(node::CAPABILITY, "cap:cache").unwrap().is_some());
    assert!(
        g.get_node(node::COMPONENT, "cmp:store").unwrap().is_some(),
        "a phase-2 node proves the LATER rounds reached their passes"
    );
    // And provenance came with it, which is the whole reason to route through
    // ingest rather than have the agent call add_* itself.
    assert!(
        g.get_node(node::FRAGMENT, &report.fragment_id)
            .unwrap()
            .is_some()
    );
}

/// **The property that makes the handshake safe to abandon.** A half-answered
/// run must leave NOTHING behind — otherwise an agent that stopped replying
/// would strand a partial design that looks like a real one.
#[test]
fn an_abandoned_handshake_writes_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let opts = IngestOptions::default();

    let first = g.ingest_step(BRIEF, &opts, Vec::new()).unwrap();
    assert!(matches!(first, IngestStep::NeedsLlm { .. }));
    // Answer one round, then walk away.
    let mut answers = Vec::new();
    if let IngestStep::NeedsLlm { prompts, .. } = first {
        for p in prompts {
            answers.push(AgentAnswer {
                id: p.id,
                text: answer(&p.prompt).to_string(),
            });
        }
    }
    let _second = g.ingest_step(BRIEF, &opts, answers).unwrap();

    assert!(
        g.get_node(node::REQUIREMENT, "req:lat").unwrap().is_none(),
        "a prepare round must not write to the real design"
    );
    assert_eq!(
        g.scan_nodes(node::FRAGMENT).unwrap().len(),
        0,
        "not even the provenance fragment"
    );
}

/// The handshake holds no server-side session state: each call is
/// self-contained, so the same answers replayed give the same outcome. That is
/// what lets it survive a restart and work across seats sharing one server.
#[test]
fn the_handshake_is_stateless_across_calls() {
    let mut a = DesignGraph::open_in_memory().unwrap();
    let (rounds_a, _) = run(&mut a);
    let mut b = DesignGraph::open_in_memory().unwrap();
    let (rounds_b, _) = run(&mut b);
    assert_eq!(
        rounds_a, rounds_b,
        "the same input must take the same rounds"
    );
}

/// A stale answer — left over from an earlier shape of the input — is reported,
/// never silently ignored.
#[test]
fn an_answer_nothing_asked_for_is_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let step = g
        .ingest_step(
            BRIEF,
            &IngestOptions::default(),
            vec![AgentAnswer {
                id: "deadbeefdeadbeef".into(),
                text: "{}".into(),
            }],
        )
        .unwrap();
    match step {
        IngestStep::NeedsLlm { unused_answers, .. } => {
            assert_eq!(unused_answers, vec!["deadbeefdeadbeef".to_string()]);
        }
        IngestStep::Done { .. } => panic!("should still need the real prompts"),
    }
}
