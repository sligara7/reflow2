//! Integration tests for the reflow2-mcp tool surface (SP-3, Step 5).
//!
//! Drives the tools on an in-memory service by calling the handler methods
//! directly (they're plain async fns): build a golden thread, then exercise the
//! read/analyze, heal propose→apply, and the gap_to_prompt collect-then-serve
//! round trip. Asserts the no-envelope JSON shape and that partial fields are
//! present (no silent fallbacks).

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

// helper: unwrap a tool result into its structured JSON payload
macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn seeded() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:sb".into(),
        name: "Softball".into()
    })));
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:physics".into(),
        name: "Realistic physics".into(),
        statement: "Ball flight must be plausible.".into()
    })));
    j!(s.add_capability(Parameters(DescribedReq {
        id: "cap:flight".into(),
        name: "Ball flight".into(),
        description: "Simulate ball trajectory.".into()
    })));
    j!(s.add_component(Parameters(DescribedReq {
        id: "cmp:physics".into(),
        name: "Physics engine".into(),
        description: "Runs the sim.".into()
    })));
    j!(s.contains(Parameters(ContainsReq {
        project_id: "proj:sb".into(),
        child_type: "Requirement".into(),
        child_id: "req:physics".into()
    })));
    j!(s.satisfies(Parameters(EdgePairReq {
        from_id: "cap:flight".into(),
        to_id: "req:physics".into()
    })));
    s
}

#[tokio::test]
async fn golden_thread_and_reports() {
    let s = seeded().await;

    // The capability is unallocated → a gap should surface.
    let gaps = j!(s.detect_gaps());
    let arr = gaps.as_array().expect("gaps is a JSON array");
    assert!(
        arr.iter()
            .any(|g| g["gap_source"] == "unallocated_capability"),
        "expected an unallocated_capability gap, got {gaps}"
    );

    // graph_report is the rollup; node_counts + gap_count present.
    let report = j!(s.graph_report());
    assert!(report["total_nodes"].as_u64().unwrap() >= 4);
    assert!(report["gap_count"].as_u64().unwrap() >= 1);

    // Speculative propagate from the requirement — partial field present.
    let radius = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["req:physics".into()],
        max_depth: None
    })));
    assert!(
        radius["unknown_seeds"].is_array(),
        "partial field always present"
    );

    // Unknown seed is reported, never silently dropped.
    let radius2 = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["nope:x".into()],
        max_depth: Some(3)
    })));
    assert_eq!(radius2["unknown_seeds"][0], "nope:x");
}

#[tokio::test]
async fn heal_propose_then_apply_round_trips() {
    let s = seeded().await;
    let proposal = j!(s.propose_heal(Parameters(ProposeHealReq {
        strategy: None,
        max_operations: None
    })));
    // no-envelope: proposal fields at top level; partial field present.
    assert!(proposal["skipped_operations"].is_array());

    // Feed the proposal straight back to apply_heal.
    let report = j!(s.apply_heal(Parameters(ApplyHealReq {
        proposal: proposal.clone()
    })));
    assert!(report["applied"].is_boolean());
    assert!(report["blocked_by_mode"].is_boolean());
}

#[tokio::test]
async fn gap_to_prompt_collect_then_serve() {
    let s = seeded().await;
    let gaps = j!(s.detect_gaps());
    let gap = gaps
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["gap_source"] == "unallocated_capability")
        .expect("a gap")
        .clone();

    // Prepare pass: no answers → needs_llm + prompts.
    let prep = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: gap.clone(),
        answers: vec![]
    })));
    assert_eq!(prep["status"], "needs_llm");
    let prompts = prep["prompts"].as_array().expect("prompts array");
    assert_eq!(prompts.len(), 1);
    let prompt_id = prompts[0]["id"].as_str().unwrap().to_string();

    // Serve pass: supply the agent's answer, get the finished question.
    let served = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap,
        answers: vec![AgentAnswerReq {
            id: prompt_id,
            text: "Which component owns ball flight?".into()
        }]
    })));
    assert_eq!(served["status"], "ok");
    assert_eq!(
        served["prompt"]["question"],
        "Which component owns ball flight?"
    );
    assert_eq!(served["prompt"]["rephrase_degraded"], false);
}
