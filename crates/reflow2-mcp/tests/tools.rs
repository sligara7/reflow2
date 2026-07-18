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

// A tool returning a list sends `{"count": n, "items": [...]}` — MCP requires
// `structuredContent` to be an object. `jl!` unwraps that envelope so a test
// reads the list directly, and asserts the envelope is well formed on the way.
macro_rules! jl {
    ($call:expr) => {{
        let env = j!($call);
        assert!(
            env.get("count").is_some() && env.get("items").is_some(),
            "a list tool must return a {{count, items}} envelope, got {env}"
        );
        env["items"].clone()
    }};
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
    let gaps = jl!(s.detect_gaps());
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
async fn genesis_bootstraps_then_detect_hands_off() {
    let s = ReflowService::in_memory().expect("in-memory service");

    // Bootstrap from a brief's framing.
    let report = j!(s.genesis(Parameters(GenesisReq {
        project_id: "proj:sb".into(),
        name: "Softball Game".into(),
        domain: Some("software".into()),
        objective: Some("Physics-real softball for the nieces.".into()),
        mode: Some("flexible".into()),
        rescan: false,
    })));
    assert_eq!(report["created"], true);
    assert_eq!(report["already_initialized"], false);
    assert!(!report["next_steps"].as_array().unwrap().is_empty());

    // A second genesis is a guarded no-op (no duplicate Project).
    let again = j!(s.genesis(Parameters(GenesisReq {
        project_id: "proj:dupe".into(),
        name: "Dupe".into(),
        domain: None,
        objective: None,
        mode: None,
        rescan: false,
    })));
    assert_eq!(again["already_initialized"], true);
    assert_eq!(again["created"], false);

    // The skill's job: seed P0/P1 only (no Components), then DETECT hands off.
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
    j!(s.satisfies(Parameters(EdgePairReq {
        from_id: "cap:flight".into(),
        to_id: "req:physics".into()
    })));

    // Seeded P0/P1 with no P2 → DETECT's first-round structure gap fires.
    let gaps = jl!(s.detect_gaps());
    assert!(
        gaps.as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "concept_without_design"),
        "genesis seed depth (P0/P1, no components) should hand off to concept_without_design, \
         got {gaps}"
    );
}

#[tokio::test]
async fn link_artifact_closes_the_unrealized_capability_gap() {
    let s = ReflowService::in_memory().expect("in-memory service");
    // Two capabilities, neither realized yet.
    for (id, name) in [("cap:flight", "Ball flight"), ("cap:score", "Scoring")] {
        j!(s.add_capability(Parameters(DescribedReq {
            id: id.into(),
            name: name.into(),
            description: "…".into()
        })));
    }

    // Realize only cap:flight. Now artifacts>0, so DETECT can flag the other.
    let link = j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:ball".into(),
        name: "Ball.cs".into(),
        location: Some("src/Ball.cs".into()),
        artifact_type: Some("code".into()),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })));
    assert_eq!(link["provenance"], "authored");
    assert_eq!(link["completeness"], "complete");

    // cap:score is unrealized → the gap fires, naming it.
    let gaps = jl!(s.detect_gaps());
    let unrealized: Vec<&serde_json::Value> = gaps
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g["gap_source"] == "unrealized_capability")
        .collect();
    assert!(
        unrealized.iter().any(|g| g["affected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "cap:score")),
        "unrealized_capability should name cap:score, got {gaps}"
    );
    assert!(
        !unrealized.iter().any(|g| g["affected_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "cap:flight")),
        "cap:flight is realized — it must NOT be flagged"
    );

    // Realize cap:score too → the gap clears for it.
    j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:score".into(),
        name: "Score.cs".into(),
        location: Some("src/Score.cs".into()),
        artifact_type: Some("code".into()),
        target_type: "Capability".into(),
        target_id: "cap:score".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })));
    let gaps2 = jl!(s.detect_gaps());
    assert!(
        !gaps2
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["gap_source"] == "unrealized_capability"),
        "both capabilities realized — no unrealized_capability gap, got {gaps2}"
    );
}

#[tokio::test]
async fn gap_to_prompt_collect_then_serve() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps());
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

/// The interface layer over the surface: both sides of a contract, then the
/// two things pairing them buys — impact that crosses the boundary, and a
/// question when one side is missing.
#[tokio::test]
async fn interface_tools_pair_both_sides_of_a_contract() {
    let s = seeded().await;
    j!(s.add_component(Parameters(DescribedReq {
        id: "cmp:ui".into(),
        name: "Scoreboard UI".into(),
        description: "Shows the score.".into()
    })));
    j!(s.add_interface(Parameters(IdName {
        id: "ifc:state".into(),
        name: "Game state feed".into()
    })));
    j!(s.provides(Parameters(EdgePairReq {
        from_id: "cmp:physics".into(),
        to_id: "ifc:state".into()
    })));
    j!(s.consumes(Parameters(EdgePairReq {
        from_id: "cmp:ui".into(),
        to_id: "ifc:state".into()
    })));

    // Changing the provider must surface the consumer on the far side.
    let radius = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["cmp:physics".into()],
        max_depth: None,
    })));
    let impacted = radius["impacted"].as_array().expect("impacted array");
    assert!(
        impacted.iter().any(|n| n["node_id"] == "cmp:ui"),
        "the consumer must be in the blast radius, got {impacted:?}"
    );

    // Both sides present → no interface-pairing question.
    let gaps = jl!(s.detect_gaps());
    let sources: Vec<&str> = gaps
        .as_array()
        .expect("gaps array")
        .iter()
        .filter_map(|g| g["gap_source"].as_str())
        .collect();
    assert!(
        !sources.contains(&"unprovided_interface"),
        "a fully paired contract is not a gap, got {sources:?}"
    );
}

#[tokio::test]
async fn a_contract_with_no_provider_surfaces_as_a_gap_over_the_surface() {
    let s = seeded().await;
    j!(s.add_interface(Parameters(IdName {
        id: "ifc:state".into(),
        name: "Game state feed".into()
    })));
    j!(s.consumes(Parameters(EdgePairReq {
        from_id: "cmp:physics".into(),
        to_id: "ifc:state".into()
    })));

    let gaps = jl!(s.detect_gaps());
    let found = gaps
        .as_array()
        .expect("gaps array")
        .iter()
        .any(|g| g["gap_source"] == "unprovided_interface");
    assert!(
        found,
        "consumed-but-unprovided must reach the agent, got {gaps:?}"
    );
}

/// As-built drift over the surface: register with a baseline, observe a change,
/// and confirm it reaches the design node the file realizes.
#[tokio::test]
async fn reconcile_surfaces_a_code_change_back_to_the_design() {
    let s = seeded().await;
    j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:flight".into(),
        name: "BallFlight.cs".into(),
        location: Some("src/BallFlight.cs".into()),
        artifact_type: Some("code".into()),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:v1".into()),
    })));

    // Unchanged: no drift.
    let clean = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v1"
        })],
        record_events: false,
        exhaustive: false,
        detected_at: None,
    })));
    assert_eq!(clean["findings"].as_array().unwrap().len(), 0);
    assert_eq!(clean["unchanged"], 1);

    // The agent edits the file; now the hash differs.
    let drifted = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v2"
        })],
        record_events: true,
        exhaustive: false,
        detected_at: Some("2026-07-18T00:00:00Z".into()),
    })));
    let findings = drifted["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["kind"], "checksum_change");
    assert_eq!(
        drifted["propagation_seeds"],
        serde_json::json!(["cap:flight"]),
        "the seeds must name the design the changed file realizes"
    );

    // Those seeds walk back up the thread to the requirement.
    let radius = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["cap:flight".into()],
        max_depth: None,
    })));
    assert!(
        radius["impacted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["node_id"] == "req:physics"),
        "a code change must reach the requirement that justified it"
    );

    // Accepting the change clears the drift.
    j!(s.set_artifact_checksum(Parameters(SetChecksumReq {
        artifact_id: "art:flight".into(),
        checksum: "sha256:v2".into(),
    })));
    let after = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v2"
        })],
        record_events: false,
        exhaustive: false,
        detected_at: None,
    })));
    assert_eq!(after["findings"].as_array().unwrap().len(), 0);
    assert_eq!(after["unchanged"], 1);
}

/// The write side over the surface: DETECT asks for a Verification and a
/// deployment, and the agent can now record both without generic create_node.
#[tokio::test]
async fn the_write_side_can_answer_what_detect_asks_for() {
    let s = seeded().await;
    j!(s.allocate(Parameters(EdgePairReq {
        from_id: "cap:flight".into(),
        to_id: "cmp:physics".into()
    })));
    j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:flight".into(),
        name: "BallFlight.cs".into(),
        location: Some("src/BallFlight.cs".into()),
        artifact_type: Some("code".into()),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:v1".into()),
    })));

    let before: Vec<String> = jl!(s.detect_gaps())
        .as_array()
        .expect("items is an array")
        .iter()
        .map(|g| g["gap_source"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        before.contains(&"build_without_verification".to_string()),
        "{before:?}"
    );
    assert!(
        before.contains(&"no_deploy_operate".to_string()),
        "{before:?}"
    );

    // Answer both, using the typed tools.
    j!(s.add_verification(Parameters(VerificationReq {
        id: "ver:flight".into(),
        name: "Ball flight tests".into(),
        method: Some("test".into()),
        level: Some("unit".into()),
    })));
    j!(s.verifies(Parameters(VerifiesReq {
        verification_id: "ver:flight".into(),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
    })));
    j!(s.add_release(Parameters(ReleaseReq {
        id: "rel:v1".into(),
        name: "Softball v1".into(),
        version: Some("1.0.0".into()),
        unit_type: Some("bundle".into()),
    })));
    j!(s.add_environment(Parameters(EnvironmentReq {
        id: "env:itch".into(),
        name: "itch.io".into(),
        env_type: Some("production".into()),
        location: None,
    })));
    j!(s.deploy_to(Parameters(DeployToReq {
        release_id: "rel:v1".into(),
        environment_id: "env:itch".into(),
        status: Some("active".into()),
    })));

    let after: Vec<String> = jl!(s.detect_gaps())
        .as_array()
        .expect("items is an array")
        .iter()
        .map(|g| g["gap_source"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !after.contains(&"build_without_verification".to_string()),
        "the verification gap must close, got {after:?}"
    );
    assert!(
        !after.contains(&"no_deploy_operate".to_string()),
        "the deploy/operate gap must close, got {after:?}"
    );

    // And a failing check reaches the requirement behind it.
    j!(s.set_verification_status(Parameters(VerificationStatusReq {
        verification_id: "ver:flight".into(),
        status: "failing".into(),
        last_run_at: None,
    })));
    let radius = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["ver:flight".into()],
        max_depth: None,
    })));
    assert!(
        radius["impacted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["node_id"] == "req:physics"),
        "a failing check must reach the requirement it ultimately protects"
    );
}
