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

/// A tool result as the JSON *object* a struct-carrying parameter now takes.
///
/// These tests call the handlers as Rust fns, so they never cross the JSON
/// boundary where BL-28 lived — which is exactly why they stayed green while
/// the published schema was unusable. `tools/smoke_mcp.py` asserts the schema
/// itself; this helper only keeps the round trips compiling.
fn obj(v: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    v.as_object().expect("expected a JSON object").clone()
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
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:flight".into(),
        name: "Ball flight".into(),
        description: "Simulate ball trajectory.".into(),
        status: None
    })));
    j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:physics".into(),
        name: "Physics engine".into(),
        description: "Runs the sim.".into(),
        level: None,
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
        max_depth: None,
        full: None
    })));
    assert!(
        radius["unknown_seeds"].is_array(),
        "partial field always present"
    );

    // Unknown seed is reported, never silently dropped.
    let radius2 = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["nope:x".into()],
        max_depth: Some(3),
        full: None
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
        proposal: obj(&proposal)
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
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:flight".into(),
        name: "Ball flight".into(),
        description: "Simulate ball trajectory.".into(),
        status: None
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
        j!(s.add_capability(Parameters(CapabilityReq {
            id: id.into(),
            name: name.into(),
            description: "…".into(),
            status: None
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
        gap: obj(&gap),
        answers: vec![],
        asked_at: None,
    })));
    assert_eq!(prep["status"], "needs_llm");
    let prompts = prep["prompts"].as_array().expect("prompts array");
    assert_eq!(prompts.len(), 1);
    let prompt_id = prompts[0]["id"].as_str().unwrap().to_string();

    // Serve pass: supply the agent's answer, get the finished question.
    let served = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: obj(&gap),
        answers: vec![AgentAnswerReq {
            id: prompt_id,
            text: "Which component owns ball flight?".into()
        }],
        asked_at: None,
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
    j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:ui".into(),
        name: "Scoreboard UI".into(),
        description: "Shows the score.".into(),
        level: None,
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
        full: Some(true),
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
        observed: vec![obj(&serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v1"
        }))],
        record_events: false,
        exhaustive: false,
        detected_at: None,
    })));
    assert_eq!(clean["findings"].as_array().unwrap().len(), 0);
    assert_eq!(clean["unchanged"], 1);

    // The agent edits the file; now the hash differs.
    let drifted = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![obj(&serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v2"
        }))],
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
        full: Some(true),
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
        disposition: "design_holds".into(),
        change_type: None,
        design_change_event_id: None,
        note: Some("accepted after review: no behaviour change".into()),
        at: Some("2026-07-19T12:00:00Z".into()),
    })));
    let after = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![obj(&serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v2"
        }))],
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
        full: Some(true),
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

// ---- describe_schema (BL-1) --------------------------------------------------
//
// The blind trial brute-forced fourteen edge types to connect a Release to a
// Component, settled on DEPENDS_ON "because it was the one that validated", and
// asked for exactly this tool. These assert the answer is both available and
// honest: available without guessing, honest about wildcard-only matches.

#[tokio::test]
async fn describe_schema_returns_the_whole_vocabulary() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let v = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: None,
        from: None,
        to: None,
    })));
    assert_eq!(
        v["node_types"].as_array().unwrap().len(),
        27,
        "every node type is discoverable"
    );
    assert_eq!(
        v["edge_types"].as_array().unwrap().len(),
        54,
        "every edge type is discoverable"
    );
}

#[tokio::test]
async fn describe_schema_answers_the_directed_question() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let q = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: None,
        from: Some("Capability".into()),
        to: Some("Component".into()),
    })));
    assert!(
        q["exact_matches"].as_u64().unwrap() >= 1,
        "ALLOCATED_TO models Capability -> Component, got {q}"
    );
    assert_eq!(q["matches"][0]["from_match"], "exact", "exact ranks first");
}

/// The trial's own case, with a history. BL-1 made this tool say plainly that
/// nothing modelled Release -> Component instead of handing back the wildcard
/// edge that happened to validate; BL-34 then added `INCLUDES` — the
/// as-released containment the trial was reaching for. A still-unmodelled pair
/// keeps the honest caveat.
#[tokio::test]
async fn release_pairs_report_their_true_standing() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let q = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: None,
        from: Some("Release".into()),
        to: Some("Component".into()),
    })));
    assert_eq!(
        q["exact_matches"].as_u64().unwrap(),
        1,
        "INCLUDES models Release -> Component since BL-34"
    );
    let loose = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: None,
        from: Some("Release".into()),
        to: Some("Requirement".into()),
    })));
    assert_eq!(
        loose["exact_matches"].as_u64().unwrap(),
        0,
        "a release ships built things, not intent; if an edge is added, update this test"
    );
    let note = loose["note"].as_str().unwrap();
    assert!(
        note.contains("wildcard") || note.contains("No edge type in this schema"),
        "the caveat must be stated in words, got: {note}"
    );
}

#[tokio::test]
async fn describe_schema_focuses_one_node_type() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let d = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: Some("Component".into()),
        from: None,
        to: None,
    })));
    let outgoing = d["outgoing"].as_array().unwrap();
    assert!(
        outgoing.iter().any(|m| m["edge_type"] == "PROVIDES"),
        "Component -> Interface must be discoverable from Component"
    );
    assert!(
        d["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["required"] == true),
        "required properties must be visible before a create_node call"
    );
}

#[tokio::test]
async fn describe_schema_rejects_a_half_given_pair() {
    let s = ReflowService::in_memory().expect("in-memory service");
    // `from` without `to` is a mistake; silently dumping everything would hide it.
    assert!(
        s.describe_schema(Parameters(DescribeSchemaReq {
            node_type: None,
            from: Some("Release".into()),
            to: None,
        }))
        .await
        .is_err(),
        "a half-specified query must fail loud"
    );
    // An unknown type name must not read as "exists, but connects to nothing".
    assert!(
        s.describe_schema(Parameters(DescribeSchemaReq {
            node_type: Some("Relese".into()),
            from: None,
            to: None,
        }))
        .await
        .is_err(),
        "a typo must fail loud"
    );
}

/// "The error tells me I'm wrong without telling me what's right" — the trial's
/// sharpest complaint, and the half a discovery tool alone does not fix.
#[tokio::test]
async fn a_rejected_edge_names_the_alternatives() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:x".into(),
        name: "X".into()
    })));
    let err = s
        .create_edge(Parameters(CreateEdgeReq {
            edge_type: "PACKAGES".into(), // the trial's first guess
            from_type: "Release".into(),
            from_id: "rel:1".into(),
            to_type: "Component".into(),
            to_id: "cmp:1".into(),
            props: None,
        }))
        .await
        .expect_err("PACKAGES is not a schema edge type");
    let msg = format!("{err}");
    assert!(
        msg.contains("PACKAGES"),
        "the rejection must still name what was wrong, got: {msg}"
    );
    assert!(
        msg.contains("describe_schema"),
        "the rejection must point at the tool that answers it, got: {msg}"
    );
}

#[tokio::test]
async fn a_rejected_node_names_the_known_types() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let err = s
        .create_node(Parameters(CreateNodeReq {
            node_type: "Widget".into(),
            id: "w:1".into(),
            props: None,
        }))
        .await
        .expect_err("Widget is not a schema node type");
    let msg = format!("{err}");
    assert!(
        msg.contains("Requirement") && msg.contains("Component"),
        "an unknown node type must list the real ones, got: {msg}"
    );
}

// ---- BL-2 · the assembly hierarchy (contain_component + level) ---------------
//
// hierarchy_issues shipped as a read tool with no writer to feed it: the level
// could not be set and components could not be nested, so it returned [] for
// want of input rather than because a design was healthy. These prove the
// writer now feeds the reader, and — just as important — that a well-formed
// hierarchy stays quiet.

#[tokio::test]
async fn a_well_formed_hierarchy_reports_no_issues() {
    let s = ReflowService::in_memory().expect("in-memory service");
    for (id, name, level) in [
        ("cmp:sys", "Station", "system"),
        ("cmp:sub", "Sensor suite", "subsystem"),
        ("cmp:leaf", "Thermometer", "component"),
    ] {
        j!(s.add_component(Parameters(ComponentReq {
            id: id.into(),
            name: name.into(),
            description: "part".into(),
            level: Some(level.into()),
        })));
    }
    j!(s.contain_component(Parameters(EdgePairReq {
        from_id: "cmp:sys".into(),
        to_id: "cmp:sub".into(),
    })));
    j!(s.contain_component(Parameters(EdgePairReq {
        from_id: "cmp:sub".into(),
        to_id: "cmp:leaf".into(),
    })));

    let issues = jl!(s.hierarchy_issues());
    assert_eq!(
        issues.as_array().unwrap().len(),
        0,
        "a clean system>subsystem>component spine has nothing to report, got {issues}"
    );
}

#[tokio::test]
async fn skipping_a_level_is_reported() {
    let s = ReflowService::in_memory().expect("in-memory service");
    for (id, level) in [("cmp:sys", "system"), ("cmp:leaf", "component")] {
        j!(s.add_component(Parameters(ComponentReq {
            id: id.into(),
            name: id.into(),
            description: "part".into(),
            level: Some(level.into()),
        })));
    }
    j!(s.contain_component(Parameters(EdgePairReq {
        from_id: "cmp:sys".into(),
        to_id: "cmp:leaf".into(),
    })));

    let issues = jl!(s.hierarchy_issues());
    let arr = issues.as_array().unwrap();
    assert_eq!(
        arr.len(),
        1,
        "a system containing a part directly skips a level, got {issues}"
    );
    assert_eq!(arr[0]["kind"], "missing_intermediate_level");
}

/// The regression that makes BL-2 worth doing carefully: exposing
/// contain_component *without* a way to set level would have flagged every
/// containment as a level_mismatch, because everything defaults to `component`.
#[tokio::test]
async fn nesting_two_defaulted_components_is_a_mismatch_not_silence() {
    let s = ReflowService::in_memory().expect("in-memory service");
    for id in ["cmp:a", "cmp:b"] {
        j!(s.add_component(Parameters(ComponentReq {
            id: id.into(),
            name: id.into(),
            description: "part".into(),
            level: None,
        })));
    }
    j!(s.contain_component(Parameters(EdgePairReq {
        from_id: "cmp:a".into(),
        to_id: "cmp:b".into(),
    })));

    let arr = jl!(s.hierarchy_issues());
    assert_eq!(
        arr.as_array().unwrap()[0]["kind"],
        "level_mismatch",
        "same-level nesting must be called out — this is why level is on add_component"
    );
}

// ---- BL-3 · requirement status ----------------------------------------------

#[tokio::test]
async fn marking_a_requirement_dropped_stops_the_nagging() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:p".into(),
        name: "P".into()
    })));
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:maybe".into(),
        name: "Maybe".into(),
        statement: "We might not do this.".into()
    })));

    let flagged = |v: &serde_json::Value| {
        v.as_array().unwrap().iter().any(|c| {
            c["affected_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "req:maybe")
        })
    };
    // Asserted through DETECT, which is now the only side that asks: BL-42
    // removed HEAL's duplicate orphan scan over requirements (the same finding
    // in two lists, 20 of 31 defects on the storyflow trial). Per-node
    // traceability is gated on the relevant phase existing, so a capability
    // has to exist for the question to be meaningful at all.
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:other".into(),
        name: "Other".into(),
        description: "does something else".into(),
        status: None,
    })));
    assert!(
        flagged(&jl!(s.detect_gaps())),
        "an unsatisfied requirement is asked about — once, by DETECT"
    );
    assert!(
        !flagged(&jl!(s.detect_defects())),
        "and never doubled as a HEAL defect"
    );

    let updated = j!(s.set_requirement_status(Parameters(RequirementStatusReq {
        requirement_id: "req:maybe".into(),
        status: "dropped".into(),
    })));
    assert_eq!(updated["properties"]["status"], "dropped");
    assert_eq!(
        updated["properties"]["statement"], "We might not do this.",
        "a status change must not cost the statement"
    );

    assert!(!flagged(&jl!(s.detect_gaps())), "DETECT goes quiet");
    assert!(!flagged(&jl!(s.detect_defects())), "and so must HEAL");
}

// ---- BL-4 · asked questions outlive the session -----------------------------

/// gap_to_prompt used to be the only tool that never touched the graph: it
/// phrased a question, returned it, and forgot. The serve pass now records what
/// it asked, so a later session can follow up instead of re-deriving.
#[tokio::test]
async fn asking_a_gap_records_the_question_it_asked() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps());
    let gap = gaps.as_array().unwrap()[0].clone();
    let gap_id = gap["id"].as_str().unwrap().to_string();
    let gap_affected: Vec<String> = gap["affected_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    // Nothing recorded before the question is put.
    assert!(jl!(s.open_questions()).as_array().unwrap().is_empty());

    let prep = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: obj(&gap),
        answers: vec![],
        asked_at: None,
    })));
    let pid = prep["prompts"][0]["id"].as_str().unwrap().to_string();
    let served = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: obj(&gap),
        answers: vec![AgentAnswerReq {
            id: pid,
            text: "Which part should own this?".into()
        }],
        asked_at: Some("2026-07-18T10:00:00Z".into()),
    })));
    assert!(
        served["question_id"].is_string(),
        "the record is reported back"
    );

    let open = jl!(s.open_questions());
    let arr = open.as_array().unwrap();
    assert_eq!(
        arr.len(),
        1,
        "the question is now on the record, got {open}"
    );
    assert_eq!(arr[0]["gap_id"], gap_id.as_str());
    assert_eq!(
        arr[0]["question"], "Which part should own this?",
        "the wording the user saw is what survives"
    );
    assert_eq!(arr[0]["asked_at"], "2026-07-18T10:00:00Z");

    // Answering records the reply. The question stays visible while its gap is
    // still open, now marked `answered` and carrying what they said — otherwise
    // a later session sees a bare open gap and asks all over again (BL-25).
    j!(s.answer_question(Parameters(AnswerQuestionReq {
        gap_id: gap_id.clone(),
        answer: "The physics engine.".into(),
    })));
    let after = jl!(s.open_questions());
    let a = after.as_array().unwrap();
    assert_eq!(
        a.len(),
        1,
        "still outstanding while the gap is open, got {after}"
    );
    assert_eq!(a[0]["status"], "answered");
    assert_eq!(a[0]["answer"], "The physics engine.");

    // Acknowledging the gap is what settles it; then there is nothing left.
    j!(s.acknowledge_gap(Parameters(AcknowledgeGapReq {
        gap_id: gap_id.clone(),
        affected_ids: gap_affected.clone(),
        reason: "the physics engine owns it".into(),
    })));
    assert!(
        jl!(s.open_questions()).as_array().unwrap().is_empty(),
        "a settled gap leaves nothing outstanding"
    );

    // Answering one nobody asked fails loud rather than inventing a record.
    assert!(
        s.answer_question(Parameters(AnswerQuestionReq {
            gap_id: "gap:never".into(),
            answer: "…".into(),
        }))
        .await
        .is_err()
    );
}

// ---- BL-20 · the design as a portable document -----------------------------

#[tokio::test]
async fn a_design_round_trips_through_export_and_import() {
    let s = seeded().await;
    let doc = j!(s.export_graph(Parameters(ExportGraphToReq { path: None })));
    assert!(doc["nodes"].as_array().unwrap().len() >= 4);
    assert!(
        doc["stamp"]["node_types"].as_u64().unwrap() >= 27,
        "it says what wrote it"
    );

    // A fresh graph, loaded from the document, holds the same design.
    let fresh = ReflowService::in_memory().expect("in-memory service");
    let report = j!(fresh.import_graph(Parameters(ImportGraphReq {
        document: obj(&doc),
    })));
    assert_eq!(
        report["nodes_written"].as_u64().unwrap(),
        doc["nodes"].as_array().unwrap().len() as u64
    );
    assert!(
        report["skipped_edges"].as_array().unwrap().is_empty(),
        "a self-contained document imports whole, got {report}"
    );

    // Exporting it again gives the same document — the property that makes a
    // backup directory diffable rather than a pile of fresh blobs.
    let again = j!(fresh.export_graph(Parameters(ExportGraphToReq { path: None })));
    assert_eq!(again["nodes"], doc["nodes"]);
    assert_eq!(again["edges"], doc["edges"]);

    // And it behaves the same, not merely serializes the same.
    assert_eq!(
        jl!(fresh.detect_gaps()).as_array().unwrap().len(),
        jl!(s.detect_gaps()).as_array().unwrap().len(),
        "a restored design must diagnose the same as the original"
    );
}

#[tokio::test]
async fn importing_something_that_is_not_an_export_fails_loud() {
    let s = ReflowService::in_memory().expect("in-memory service");
    assert!(
        s.import_graph(Parameters(ImportGraphReq {
            document: obj(&serde_json::json!({"nodes": "not a list"})),
        }))
        .await
        .is_err(),
        "a malformed document must be rejected, not partly applied"
    );
}

#[tokio::test]
async fn a_wrong_edge_can_be_retracted_without_deleting_its_endpoints() {
    // Until delete_edge existed the only way to remove a mis-drawn link over
    // MCP was to delete one of its endpoint nodes — destroying a real design
    // node to fix a wrong assertion about it.
    let s = seeded().await;

    // The seeded SATISFIES edge is visible to detect: no unsatisfied gap.
    let gaps = jl!(s.detect_gaps());
    let unsatisfied = |gaps: &serde_json::Value| {
        gaps.as_array()
            .unwrap()
            .iter()
            .filter(|g| g["gap_source"] == "unsatisfied_requirement")
            .count()
    };
    assert_eq!(unsatisfied(&gaps), 0, "the thread starts intact");

    let existed = j!(s.delete_edge(Parameters(DeleteEdgeReq {
        edge_type: "SATISFIES".into(),
        from_id: "cap:flight".into(),
        to_id: "req:physics".into(),
    })));
    assert_eq!(existed, serde_json::json!(true));

    // Both endpoints survive; only the assertion between them is gone —
    // and detect sees the thread it severed.
    assert!(
        j!(s.get_node(Parameters(TypedIdReq {
            node_type: "Requirement".into(),
            id: "req:physics".into()
        })))
        .is_object(),
        "the requirement must survive the retraction"
    );
    let after = jl!(s.detect_gaps());
    assert_eq!(
        unsatisfied(&after),
        1,
        "with the edge retracted the requirement is unsatisfied again"
    );

    // Retracting an edge that is not there says so, without inventing work.
    let second = j!(s.delete_edge(Parameters(DeleteEdgeReq {
        edge_type: "SATISFIES".into(),
        from_id: "cap:flight".into(),
        to_id: "req:physics".into(),
    })));
    assert_eq!(second, serde_json::json!(false));
}

#[tokio::test]
async fn search_finds_the_design_by_its_own_words() {
    // The retrieval gap the surface carried from day one: get_node needs the
    // id, scan_nodes reads a whole type — nothing answered "which node talks
    // about X?". That made finding-by-content the LLM's job, the seat-swap
    // partnership.md forbids.
    let s = seeded().await;

    let result = j!(s.search_design(Parameters(SearchDesignReq {
        query: "ball flight plausible".into(),
        node_type: None,
        limit: None,
    })));
    let hits = result["hits"].as_array().expect("hits list");
    assert!(
        hits.iter().any(|h| h["node_id"] == "req:physics"),
        "the requirement stating those words is found: {result}"
    );
    assert!(
        result["stale"].as_array().unwrap().is_empty(),
        "a live graph has no index drift"
    );
    assert_eq!(result["limit"], 10, "the default bound is visible");

    // Scoped to a type it narrows; scoped to the wrong type it is honestly empty.
    let caps = j!(s.search_design(Parameters(SearchDesignReq {
        query: "flight".into(),
        node_type: Some("Capability".into()),
        limit: Some(5),
    })));
    let cap_hits = caps["hits"].as_array().unwrap();
    assert!(!cap_hits.is_empty(), "cap:flight mentions flight: {caps}");
    assert!(cap_hits.iter().all(|h| h["node_type"] == "Capability"));

    let none = j!(s.search_design(Parameters(SearchDesignReq {
        query: "zeppelin".into(),
        node_type: None,
        limit: None,
    })));
    assert!(none["hits"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn documents_links_a_doc_over_the_surface_and_refuses_a_ghost() {
    // BL-26's write side: the coherence failure it exists for is two
    // instruction files disagreeing about the build — uncatchable while no
    // graph knows the files exist.
    let s = seeded().await;
    j!(s.add_artifact(Parameters(AddArtifactReq {
        id: "art:readme".into(),
        name: "README.md".into(),
        artifact_type: Some("document".into()),
        location: Some("README.md".into()),
    })));

    let edge = j!(s.documents(Parameters(DocumentsReq {
        artifact_id: "art:readme".into(),
        target_type: "Project".into(),
        target_id: "proj:sb".into(),
        doc_kind: Some("readme".into()),
    })));
    assert_eq!(edge["edge_type"], "DOCUMENTS");
    assert_eq!(edge["from_id"], "art:readme");
    assert_eq!(edge["to_id"], "proj:sb");

    // A missing endpoint is refused by name, never a dangling edge.
    let err = s
        .documents(Parameters(DocumentsReq {
            artifact_id: "art:ghost".into(),
            target_type: "Project".into(),
            target_id: "proj:sb".into(),
            doc_kind: None,
        }))
        .await
        .expect_err("a ghost artifact must be refused");
    assert!(
        err.to_string().contains("art:ghost"),
        "the refusal must name the missing node, got: {err}"
    );
}

/// BL-46: the create_node tool's documented contract (the revise-design
/// skill's "an existing id merges") — a partial props object edits the named
/// properties and must not reset the rest to schema defaults, which is what
/// silently downgraded a verified capability to `planned` in the 2026-07-20
/// self-adopt session.
#[tokio::test]
async fn create_node_on_an_existing_id_merges_instead_of_resetting() {
    let s = seeded().await;
    j!(s.set_capability_status(Parameters(CapabilityStatusReq {
        capability_id: "cap:flight".into(),
        status: "verified".into(),
    })));

    let mut props = serde_json::Map::new();
    props.insert(
        "description".into(),
        serde_json::Value::String("Simulate ball trajectory, with drag.".into()),
    );
    let n = j!(s.create_node(Parameters(CreateNodeReq {
        node_type: "Capability".into(),
        id: "cap:flight".into(),
        props: Some(props),
    })));

    assert_eq!(
        n["properties"]["description"],
        "Simulate ball trajectory, with drag."
    );
    assert_eq!(
        n["properties"]["status"], "verified",
        "a property the caller did not name must survive the edit"
    );
    assert_eq!(n["properties"]["name"], "Ball flight");
}

// ---- BL-48 · a prose tool must not put a string in structuredContent -------
//
// MCP defines `structuredContent` as an object; a bare string is rejected by a
// spec-compliant client, which made graph_report_markdown — the report a
// session reads first — unreachable from Claude Code while every Rust-side
// test stayed green.

#[tokio::test]
async fn markdown_report_is_text_content_with_no_structured_payload() {
    let s = seeded().await;
    let result = s.graph_report_markdown().await.expect("tool ok");
    assert!(
        result.structured_content.is_none(),
        "a Markdown document has no structure to declare"
    );
    let text = &result.content[0].as_text().expect("text content").text;
    assert!(
        text.contains('#'),
        "the rendered report should be Markdown, got {text:?}"
    );
}

// ---- BL-49 · a blast radius must be readable inside the loop ----------------

#[tokio::test]
async fn propagate_defaults_to_a_summary_that_counts_everything() {
    let s = seeded().await;
    let summary = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["req:physics".into()],
        max_depth: None,
        full: None
    })));
    assert!(
        summary.get("impacted").is_none(),
        "the default result must not carry per-node hop chains"
    );
    let total = summary["total_impacted"].as_u64().expect("total_impacted");
    let banded: u64 = summary["counts_by_distance"]
        .as_array()
        .expect("counts_by_distance")
        .iter()
        .map(|b| b["count"].as_u64().unwrap())
        .sum();
    assert_eq!(total, banded, "every impacted node is counted in a band");
    let ring = summary["direct_ring"].as_array().expect("direct_ring");
    assert!(
        ring.iter().any(|n| n["node_id"] == "cap:flight"),
        "the capability satisfying the seed requirement sits one hop out, got {ring:?}"
    );
    assert!(
        ring.iter().all(|n| n["edge_type"].is_string()),
        "each ring node names the edge that reached it"
    );
    assert!(summary["risk_crossings"].is_array(), "field always present");
    assert!(
        summary["truncated_beyond_depth"].is_u64(),
        "truncation stays reported in the summary"
    );

    // The full dump stays reachable, explicitly.
    let radius = j!(s.propagate_from(Parameters(PropagateFromReq {
        seed_ids: vec!["req:physics".into()],
        max_depth: None,
        full: Some(true)
    })));
    let impacted = radius["impacted"].as_array().expect("impacted");
    assert_eq!(impacted.len() as u64, total, "same radius, both shapes");
    assert!(
        impacted.iter().all(|n| n["via"].is_array()),
        "the full dump explains every impact"
    );
}

#[tokio::test]
async fn export_graph_writes_a_deterministic_file_when_asked() {
    let s = seeded().await;
    let path =
        std::env::temp_dir().join(format!("reflow2-export-test-{}.json", std::process::id()));
    let path_str = path.to_str().expect("utf8 path").to_string();

    let receipt = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone())
    })));
    assert_eq!(receipt["path"], path_str.as_str());
    let on_disk = std::fs::read_to_string(&path).expect("file written");
    assert_eq!(receipt["bytes"].as_u64().unwrap() as usize, on_disk.len());

    // The file is the same document the payload variant returns…
    let doc: serde_json::Value = serde_json::from_str(&on_disk).expect("valid JSON");
    let payload = j!(s.export_graph(Parameters(ExportGraphToReq { path: None })));
    assert_eq!(
        doc["nodes"], payload["nodes"],
        "file and payload carry the same design"
    );
    assert_eq!(
        receipt["nodes"].as_u64().unwrap() as usize,
        payload["nodes"].as_array().unwrap().len()
    );

    // …and writing an unchanged graph again is byte-identical (diffable backups).
    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone())
    })));
    let again = std::fs::read_to_string(&path).expect("file written twice");
    assert_eq!(on_disk, again, "two exports of an unchanged graph match");

    std::fs::remove_file(&path).ok();
}
