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

/// `detect_defects` unscoped, unwrapped to its findings.
///
/// It stopped being a "list tool" on 2026-08-17: it returns `{swept, defects}`
/// so an empty answer can say whether it was EXERCISED AND FOUND NOTHING or HAD
/// NOTHING TO EXAMINE, which a bare `{count, items}` envelope cannot express.
/// `jl!` correctly refused it — that refusal is the contract check working, not
/// a test to loosen — so the sweep gets its own accessor, and the sweep block
/// is asserted present here rather than quietly skipped.
macro_rules! jd {
    ($call:expr) => {{
        let env = j!($call);
        assert!(
            env.get("swept").is_some() && env.get("defects").is_some(),
            "detect_defects must return a {{swept, defects}} sweep, got {env}"
        );
        assert!(
            env["swept"]["rules"]
                .as_array()
                .is_some_and(|r| !r.is_empty()),
            "and the sweep must name the rules that ran, or 'no findings' is taken on trust: {env}"
        );
        env["defects"].clone()
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
        name: Some("Softball".into()),
    })));
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:physics".into(),
        name: Some("Realistic physics".into()),
        statement: Some("Ball flight must be plausible.".into()),
        distinct_from: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:flight".into(),
        name: Some("Ball flight".into()),
        description: Some("Simulate ball trajectory.".into()),
        status: None,
        distinct_from: None,
    })));
    j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:physics".into(),
        name: Some("Physics engine".into()),
        description: Some("Runs the sim.".into()),
        level: None,
        distinct_from: None,
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
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
    let arr = gaps.as_array().expect("gaps is a JSON array");
    assert!(
        arr.iter()
            .any(|g| g["gap_source"] == "unallocated_capability"),
        "expected an unallocated_capability gap, got {gaps}"
    );

    // graph_report is the rollup; node_counts + gap_count present.
    let report = j!(s.graph_report(Parameters(GraphReportReq::default())));
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
        name: Some("Realistic physics".into()),
        statement: Some("Ball flight must be plausible.".into()),
        distinct_from: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:flight".into(),
        name: Some("Ball flight".into()),
        description: Some("Simulate ball trajectory.".into()),
        status: None,
        distinct_from: None,
    })));
    j!(s.satisfies(Parameters(EdgePairReq {
        from_id: "cap:flight".into(),
        to_id: "req:physics".into()
    })));

    // Seeded P0/P1 with no P2 → DETECT's first-round structure gap fires.
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
            name: Some(name.into()),
            description: Some("…".into()),
            status: None,
            distinct_from: None,
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
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })));
    assert_eq!(link["provenance"], "authored");
    assert_eq!(link["completeness"], "complete");

    // cap:score is unrealized → the gap fires, naming it.
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })));
    let gaps2 = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
        name: Some("Scoreboard UI".into()),
        description: Some("Shows the score.".into()),
        level: None,
        distinct_from: None,
    })));
    j!(s.add_interface(Parameters(IdName {
        id: "ifc:state".into(),
        name: Some("Game state feed".into()),
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
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
        name: Some("Game state feed".into()),
    })));
    j!(s.consumes(Parameters(EdgePairReq {
        from_id: "cmp:physics".into(),
        to_id: "ifc:state".into()
    })));

    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
        conformance: None,
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

/// BL-157 + BL-158 on the real surface: the third disposition, and a clean
/// sweep that leaves a trace.
#[tokio::test]
async fn the_surface_can_say_that_nothing_moved() {
    let s = seeded().await;
    // Registered with NO checksum — the state `art:detect` was found in.
    j!(s.link_artifact(Parameters(LinkArtifactReq {
        artifact_id: "art:flight".into(),
        name: "BallFlight.cs".into(),
        location: Some("src/BallFlight.cs".into()),
        artifact_type: Some("code".into()),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: None,
    })));

    // An accept is refused, and the refusal names the disposition that is right.
    let err = s
        .set_artifact_checksum(Parameters(SetChecksumReq {
            artifact_id: "art:flight".into(),
            checksum: "sha256:v1".into(),
            disposition: "design_holds".into(),
            change_type: None,
            design_change_event_id: None,
            note: None,
            at: Some("2026-08-01".into()),
        }))
        .await
        .expect_err("there is no baseline to accept a change against");
    assert!(
        format!("{err}").contains("baseline_established"),
        "the refusal must name what IS right, got: {err}"
    );

    // `change_type` alongside it is refused rather than ignored: a parameter
    // silently dropped teaches the caller it was accepted.
    assert!(
        s.set_artifact_checksum(Parameters(SetChecksumReq {
            artifact_id: "art:flight".into(),
            checksum: "sha256:v1".into(),
            disposition: "baseline_established".into(),
            change_type: Some("refactor".into()),
            design_change_event_id: None,
            note: None,
            at: Some("2026-08-01".into()),
        }))
        .await
        .is_err(),
        "baseline_established is not a change, so naming a change type is a mistake"
    );

    j!(s.set_artifact_checksum(Parameters(SetChecksumReq {
        artifact_id: "art:flight".into(),
        checksum: "sha256:v1".into(),
        disposition: "baseline_established".into(),
        change_type: None,
        design_change_event_id: None,
        note: None,
        at: Some("2026-08-01".into()),
    })));

    // A clean sweep now says what it confirmed, instead of writing nothing.
    let clean = j!(s.reconcile_artifacts(Parameters(ReconcileArtifactsReq {
        observed: vec![obj(&serde_json::json!({
            "artifact_id": "art:flight", "present": true, "checksum": "sha256:v1"
        }))],
        record_events: true,
        exhaustive: false,
        detected_at: Some("2026-08-02".into()),
    })));
    assert_eq!(clean["unchanged"], 1);
    assert_eq!(
        clean["confirmed"],
        serde_json::json!(["art:flight"]),
        "a pass that found everything correct must be distinguishable from one \
         nobody ran"
    );

    let ledger = j!(s.confirmation_ledger());
    assert_eq!(
        ledger["unexamined"], 0,
        "BL-158: the sweep has to be able to clear the debt it discharged"
    );

    // And the label cannot be applied by hand through the generic change path,
    // or the ledger's count of first baselines would measure nothing.
    assert!(
        s.add_change_event(Parameters(AddChangeEventReq {
            summary: None,
            rationale: None,
            id: "chg:fake".into(),
            name: Some("not really a baseline".into()),
            change_type: Some("baseline_established".into()),
            subject: None,
            affected: None,
        }))
        .await
        .is_err(),
        "`baseline_established` is reserved for set_artifact_checksum"
    );
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
        conformance: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:v1".into()),
    })));

    let before: Vec<String> = jl!(s.detect_gaps(Parameters(GapScopeReq::default())))
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
        name: Some("Ball flight tests".into()),
        method: Some("test".into()),
        level: Some("unit".into()),
        description: None,
    })));
    j!(s.verifies(Parameters(VerifiesReq {
        verification_id: "ver:flight".into(),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
    })));
    j!(s.add_release(Parameters(ReleaseReq {
        id: "rel:v1".into(),
        name: Some("Softball v1".into()),
        version: Some("1.0.0".into()),
        unit_type: Some("bundle".into()),
    })));
    j!(s.add_environment(Parameters(EnvironmentReq {
        id: "env:itch".into(),
        name: Some("itch.io".into()),
        env_type: Some("production".into()),
        location: None,
    })));
    j!(s.deploy_to(Parameters(DeployToReq {
        release_id: "rel:v1".into(),
        environment_id: "env:itch".into(),
        status: Some("active".into()),
    })));

    let after: Vec<String> = jl!(s.detect_gaps(Parameters(GapScopeReq::default())))
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
        findings: None,
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

/// The protocol version we advertise, pinned so a change is never silent.
///
/// `get_info` uses `ProtocolVersion::LATEST` deliberately: a hand-written
/// literal sat at `V_2024_11_05` for the project's whole life with no recorded
/// reason while rmcp's own LATEST moved on four releases, which is a claim about
/// ourselves that nothing checked. Following the SDK fixes the staleness — but
/// following it *silently* would just trade one invisible drift for another, so
/// this test records what LATEST currently resolves to.
///
/// When an rmcp bump fails this, that is the test doing its job: look at what
/// changed in the protocol, decide whether reflow2 should still speak it, and
/// update the expectation deliberately. Same discipline as the schema type
/// counts in `schema.rs` — growth must be conscious.
#[test]
fn the_advertised_protocol_version_is_the_sdks_latest_and_is_pinned() {
    use rmcp::model::ProtocolVersion;
    assert_eq!(
        ProtocolVersion::LATEST,
        ProtocolVersion::V_2025_11_25,
        "rmcp's LATEST protocol version moved — decide deliberately whether \
         reflow2 should follow it, then update this expectation"
    );
    let declared = ReflowService::describe_protocol_version();
    assert_eq!(
        declared,
        ProtocolVersion::LATEST,
        "the server must advertise the SDK's current protocol, not a literal \
         copied from an example years ago"
    );
}

#[tokio::test]
async fn describe_schema_returns_the_whole_vocabulary() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let v = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: None,
        from: None,
        to: None,
        required_only: false,
    })));
    assert_eq!(
        v["node_types"].as_array().unwrap().len(),
        29,
        "every node type is discoverable"
    );
    assert_eq!(
        v["edge_types"].as_array().unwrap().len(),
        // 61 since OWNED_BY (2026-08-09, the third "who" axis); 60 since
        // GATED_ON + HAS_READINESS (2026-08-02, BL-68); 58 before that, since
        // CALIBRATED_AGAINST (2026-08-01, req:a-fit-is-not-a-test).
        61,
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
        required_only: false,
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
        required_only: false,
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
        required_only: false,
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
        required_only: false,
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

/// BL-89 B.3: `required_only` returns the compact "what must I supply?" answer —
/// only required properties, and no edge lists — so an adopter reading many
/// types at scale is not forced back to `schema/*.yaml`.
#[tokio::test]
async fn describe_schema_required_only_is_compact() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let full = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: Some("Requirement".into()),
        from: None,
        to: None,
        required_only: false,
    })));
    let compact = j!(s.describe_schema(Parameters(DescribeSchemaReq {
        node_type: Some("Requirement".into()),
        from: None,
        to: None,
        required_only: true,
    })));

    // The full view carries the edge lists; the compact one drops them.
    assert!(full.get("outgoing").is_some(), "full view lists edges");
    assert!(
        compact.get("outgoing").is_none() && compact.get("incoming").is_none(),
        "compact view omits the edge lists, got {compact}"
    );

    // Every property returned is required, and there is at least one.
    let props = compact["properties"].as_array().unwrap();
    assert!(!props.is_empty(), "a Requirement has required properties");
    assert!(
        props.iter().all(|p| p["required"] == true),
        "compact returns only required properties, got {compact}"
    );
    // …and it is genuinely a subset: the full view has optional ones too.
    assert!(
        full["properties"].as_array().unwrap().len() > props.len(),
        "the full view carries optional properties the compact one drops"
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
            required_only: false,
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
            required_only: false,
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
        name: Some("X".into()),
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
            // Unguarded on purpose: this fixture is not testing the
            // lost-update precondition, and stating an expectation it
            // never read would be a fake one.
            expected_content_hash: None,
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
            name: Some(name.into()),
            description: Some("part".into()),
            level: Some(level.into()),
            distinct_from: None,
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
            name: Some(id.into()),
            description: Some("part".into()),
            level: Some(level.into()),
            distinct_from: None,
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
            name: Some(id.into()),
            description: Some("part".into()),
            level: None,
            distinct_from: None,
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
        name: Some("P".into()),
    })));
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:maybe".into(),
        name: Some("Maybe".into()),
        statement: Some("We might not do this.".into()),
        distinct_from: None,
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
        name: Some("Other".into()),
        description: Some("does something else".into()),
        status: None,
        distinct_from: None,
    })));
    assert!(
        flagged(&jl!(s.detect_gaps(Parameters(GapScopeReq::default())))),
        "an unsatisfied requirement is asked about — once, by DETECT"
    );
    assert!(
        !flagged(&jd!(s.detect_defects(Parameters(ScopeReq::default())))),
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

    assert!(
        !flagged(&jl!(s.detect_gaps(Parameters(GapScopeReq::default())))),
        "DETECT goes quiet"
    );
    assert!(
        !flagged(&jd!(s.detect_defects(Parameters(ScopeReq::default())))),
        "and so must HEAL"
    );
}

// ---- BL-4 · asked questions outlive the session -----------------------------

/// gap_to_prompt used to be the only tool that never touched the graph: it
/// phrased a question, returned it, and forgot. The serve pass now records what
/// it asked, so a later session can follow up instead of re-deriving.
#[tokio::test]
async fn asking_a_gap_records_the_question_it_asked() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
        gap_id: Some(gap_id.clone()),
        question_id: None,
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
            gap_id: Some("gap:never".into()),
            question_id: None,
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
    let doc = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: None,
        overwrite: None,
        accept_divergence: None,
    })));
    assert!(doc["nodes"].as_array().unwrap().len() >= 4);
    assert!(
        doc["stamp"]["node_types"].as_u64().unwrap() >= 27,
        "it says what wrote it"
    );

    // A fresh graph, loaded from the document, holds the same design.
    let fresh = ReflowService::in_memory().expect("in-memory service");
    let report = j!(fresh.import_graph(Parameters(ImportGraphReq {
        document: Some(obj(&doc)),
        path: None,
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
    let again = j!(fresh.export_graph(Parameters(ExportGraphToReq {
        path: None,
        overwrite: None,
        accept_divergence: None,
    })));
    assert_eq!(again["nodes"], doc["nodes"]);
    assert_eq!(again["edges"], doc["edges"]);

    // And it behaves the same, not merely serializes the same.
    assert_eq!(
        jl!(fresh.detect_gaps(Parameters(GapScopeReq::default())))
            .as_array()
            .unwrap()
            .len(),
        jl!(s.detect_gaps(Parameters(GapScopeReq::default())))
            .as_array()
            .unwrap()
            .len(),
        "a restored design must diagnose the same as the original"
    );
}

#[tokio::test]
async fn importing_something_that_is_not_an_export_fails_loud() {
    let s = ReflowService::in_memory().expect("in-memory service");
    assert!(
        s.import_graph(Parameters(ImportGraphReq {
            document: Some(obj(&serde_json::json!({"nodes": "not a list"}))),
            path: None,
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
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
    assert_eq!(
        existed["deleted"],
        serde_json::json!(true),
        "a scalar result carries its name — bare bools in structuredContent are the BL-48 defect"
    );

    // Both endpoints survive; only the assertion between them is gone —
    // and detect sees the thread it severed.
    assert!(
        j!(s.get_node(Parameters(TypedIdReq {
            node_type: "Requirement".into(),
            id: "req:physics".into()
        })))["node"]["node_id"]
            == "req:physics",
        "the requirement must survive the retraction"
    );
    let after = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
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
    assert_eq!(second["deleted"], serde_json::json!(false));
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
        name: Some("README.md".into()),
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
        // Unguarded on purpose: this fixture is not testing the
        // lost-update precondition, and stating an expectation it
        // never read would be a fake one.
        expected_content_hash: None,
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

    std::fs::remove_file(&path).ok(); // start clean
    let receipt = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone()),
        overwrite: None,
        accept_divergence: None,
    })));
    // The receipt reports the resolved (canonicalized) path — same file.
    assert_eq!(
        std::fs::canonicalize(receipt["path"].as_str().unwrap()).unwrap(),
        std::fs::canonicalize(&path).unwrap()
    );
    let on_disk = std::fs::read_to_string(&path).expect("file written");
    assert_eq!(receipt["bytes"].as_u64().unwrap() as usize, on_disk.len());

    // The file is the same document the payload variant returns…
    let doc: serde_json::Value = serde_json::from_str(&on_disk).expect("valid JSON");
    let payload = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: None,
        overwrite: None,
        accept_divergence: None,
    })));
    assert_eq!(
        doc["nodes"], payload["nodes"],
        "file and payload carry the same design"
    );
    assert_eq!(
        receipt["nodes"].as_u64().unwrap() as usize,
        payload["nodes"].as_array().unwrap().len()
    );

    // BL-57: a second write to the same path is REFUSED without overwrite —
    // a stray or injected path cannot silently clobber an existing file.
    assert!(
        s.export_graph(Parameters(ExportGraphToReq {
            path: Some(path_str.clone()),
            overwrite: None,
            accept_divergence: None,
        }))
        .await
        .is_err(),
        "overwriting an existing file must be refused unless opted in"
    );

    // …with overwrite, an unchanged graph writes byte-identically (diffable backups).
    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone()),
        overwrite: Some(true),
        accept_divergence: None,
    })));
    let again = std::fs::read_to_string(&path).expect("file written twice");
    assert_eq!(on_disk, again, "two exports of an unchanged graph match");

    std::fs::remove_file(&path).ok();
}

// ---- BL-71 rung c · design-vs-design comparison -----------------------------

#[tokio::test]
async fn compare_designs_reports_divergence_from_a_base_export() {
    let s = seeded().await;
    let base_path =
        std::env::temp_dir().join(format!("reflow2-compare-base-{}.json", std::process::id()));
    let base_str = base_path.to_str().expect("utf8 path").to_string();
    std::fs::remove_file(&base_path).ok();

    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(base_str.clone()),
        overwrite: None,
        accept_divergence: None,
    })));

    // Identical: the live graph has not moved since the export.
    let same = j!(s.compare_designs(Parameters(CompareDesignsReq {
        base_path: base_str.clone(),
        other_path: None,
    })));
    assert_eq!(same["summary"]["identical"], true);
    assert_eq!(same["base"], base_str.as_str());
    assert_eq!(same["other"], "live graph");

    // Diverge the live graph: a new design node relative to the base.
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:catch".into(),
        name: Some("Catching".into()),
        description: Some("Field the ball.".into()),
        status: None,
        distinct_from: None,
    })));

    let diff = j!(s.compare_designs(Parameters(CompareDesignsReq {
        base_path: base_str.clone(),
        other_path: None,
    })));
    assert_eq!(diff["summary"]["identical"], false);
    assert_eq!(diff["summary"]["design_added"], 1);
    assert_eq!(diff["design"]["added"][0]["node_id"], "cap:catch");

    // Two-file mode: export the diverged state and compare file against file.
    let other_path =
        std::env::temp_dir().join(format!("reflow2-compare-other-{}.json", std::process::id()));
    let other_str = other_path.to_str().expect("utf8 path").to_string();
    std::fs::remove_file(&other_path).ok();
    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(other_str.clone()),
        overwrite: None,
        accept_divergence: None,
    })));

    let files = j!(s.compare_designs(Parameters(CompareDesignsReq {
        base_path: base_str.clone(),
        other_path: Some(other_str.clone()),
    })));
    assert_eq!(files["summary"]["design_added"], 1);
    assert_eq!(files["other"], other_str.as_str());

    // A path that does not exist is the caller's mistake, said loudly.
    assert!(
        s.compare_designs(Parameters(CompareDesignsReq {
            base_path: "/nonexistent/reflow2-compare.json".into(),
            other_path: None,
        }))
        .await
        .is_err(),
        "an unreadable base path must be refused, not read as empty"
    );

    std::fs::remove_file(&base_path).ok();
    std::fs::remove_file(&other_path).ok();
}

// ---- BL-74 · loop_status and the write-tool hints ---------------------------

#[tokio::test]
async fn loop_status_reports_debt_and_the_write_tools_point_at_the_loop() {
    let s = seeded().await;

    // A capability claiming realized with no passing check is the classic
    // raw-tools-only residue; the hint rides the write result itself.
    let cap = j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:shipped".into(),
        name: Some("Shipped".into()),
        description: Some("Claims to be built.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    assert!(
        cap["loop_hint"]
            .as_str()
            .expect("write results carry the loop hint")
            .contains("detect_gaps"),
        "{cap}"
    );

    let status = j!(s.loop_status(Parameters(Default::default())));
    assert_eq!(status["clean"], false);
    assert!(
        status["unproven_capabilities"].as_u64().unwrap() >= 1,
        "{status}"
    );
    let next: Vec<&str> = status["next"]
        .as_array()
        .expect("next is a list")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        next.iter().any(|l| l.contains("no passing check")),
        "{next:?}"
    );

    // Structural writes point at check-health instead.
    let cmp = j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:new".into(),
        name: Some("New part".into()),
        description: Some("Just added.".into()),
        level: None,
        distinct_from: None,
    })));
    assert!(
        cmp["loop_hint"].as_str().unwrap().contains("check-health"),
        "{cmp}"
    );
}

/// `cap:loop-status` promises ONE CHEAP CALL. It stopped being one: the debt
/// rollup is seven integers and a to-do list, but the per-check roll beside it
/// grew with the design until a real graph answered in 74 KB — over the harness
/// limit, so the one call you are told to fire between tasks could not be read
/// at all and had to be `jq`-ed out of a spill file.
///
/// The digest is the fix, and this is what keeps it honest: the checks that are
/// NOT passing survive in full, because those are the ones a reader acts on;
/// the passing remainder is COUNTED, not dropped in silence.
#[tokio::test]
async fn loop_status_digests_the_verification_roll_instead_of_rolling_it() {
    let s = seeded().await;

    // One check that needs attention among many that do not — the shape of any
    // mature design, and the shape that made the payload unreadable.
    for i in 0..40 {
        let id = format!("ver:bulk-{i}");
        j!(s.add_verification(Parameters(VerificationReq {
            id: id.clone(),
            // Long names are the actual bulk: these are test descriptions in
            // the real graph, hundreds of characters each.
            name: Some(format!(
                "Check {i} — {}",
                "a description long enough to matter when it is repeated once per check. "
                    .repeat(4)
            )),
            method: Some("test".into()),
            level: Some("unit".into()),
            description: None,
        })));
        j!(s.set_verification_status(Parameters(VerificationStatusReq {
            verification_id: id,
            status: if i == 7 { "failing" } else { "passing" }.into(),
            last_run_at: if i % 2 == 0 {
                Some("2026-08-04".into())
            } else {
                None
            },
            findings: None,
        })));
    }

    let status = j!(s.loop_status(Parameters(Default::default())));
    let v = &status["verifications"];

    assert!(
        v.is_object(),
        "the roll must come back as a digest, not an array — {v}"
    );
    assert_eq!(v["total"].as_u64().unwrap(), 40, "{v}");
    assert_eq!(v["by_status"]["passing"].as_u64().unwrap(), 39, "{v}");
    assert_eq!(v["by_status"]["failing"].as_u64().unwrap(), 1, "{v}");

    // The one that is not passing survives in full; the other 39 are counted.
    let attention = v["attention"].as_array().expect("attention is a list");
    assert_eq!(attention.len(), 1, "{v}");
    assert_eq!(attention[0]["verification_id"], "ver:bulk-7");
    assert_eq!(v["omitted"].as_u64().unwrap(), 39, "{v}");

    // A passing check that never ran is an assertion, not a measurement, and a
    // status tally alone cannot say so.
    assert_eq!(v["never_run"].as_u64().unwrap(), 20, "{v}");

    // Nothing is hidden: the full roll is still one call away, and the digest
    // says which one.
    assert!(
        v["full_list"].as_str().unwrap().contains("graph_report"),
        "{v}"
    );

    // The regression itself. 40 long-named checks used to serialize into the
    // reply verbatim; the digest must not scale with the roll.
    let bytes = serde_json::to_string(&status).unwrap().len();
    assert!(
        bytes < 8_000,
        "loop_status must stay cheap — {bytes} bytes for 40 checks"
    );

    // The debt rollup is untouched by the digest.
    assert!(status["clean"].is_boolean(), "{status}");
    assert!(status["next"].is_array(), "{status}");
}

// ---- dec:export-hash-chain · lineage at the file-write seam -----------------

#[tokio::test]
async fn export_files_chain_by_content_hash() {
    let s = seeded().await;
    let path = std::env::temp_dir().join(format!("reflow2-chain-{}.json", std::process::id()));
    let path_str = path.to_str().expect("utf8 path").to_string();
    std::fs::remove_file(&path).ok();

    // First write: hashed, no predecessor.
    let first = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone()),
        overwrite: None,
        accept_divergence: None,
    })));
    let first_hash = first["content_hash"]
        .as_str()
        .expect("hash in receipt")
        .to_string();
    assert!(
        first["prev_content_hash"].is_null(),
        "a new file has no lineage"
    );

    // Unchanged design, rewritten: byte-identical file, chain unmoved.
    let on_disk = std::fs::read_to_string(&path).expect("written");
    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone()),
        overwrite: Some(true),
        accept_divergence: None,
    })));
    assert_eq!(
        on_disk,
        std::fs::read_to_string(&path).expect("rewritten"),
        "an unchanged design still writes byte-identical files"
    );

    // Changed design: the new file names its predecessor.
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:chain".into(),
        name: Some("Chained".into()),
        description: Some("Content moved.".into()),
        status: None,
        distinct_from: None,
    })));
    let second = j!(s.export_graph(Parameters(ExportGraphToReq {
        path: Some(path_str.clone()),
        overwrite: Some(true),
        accept_divergence: None,
    })));
    assert_eq!(
        second["prev_content_hash"].as_str(),
        Some(first_hash.as_str()),
        "the chain advances to the replaced file's content hash"
    );

    // And compare_designs reads the lineage as ancestry.
    let old_copy =
        std::env::temp_dir().join(format!("reflow2-chain-old-{}.json", std::process::id()));
    std::fs::write(&old_copy, &on_disk).expect("copy of the first export");
    let diff = j!(s.compare_designs(Parameters(CompareDesignsReq {
        base_path: old_copy.to_str().unwrap().into(),
        other_path: Some(path_str.clone()),
    })));
    assert_eq!(diff["ancestry"], "other_succeeds_base");

    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&old_copy).ok();
}

// ---- BL-50 · add_change_event declares what it changed ----------------------

#[tokio::test]
async fn change_event_declares_what_it_changed_atomically() {
    let s = seeded().await;

    // A missing affected node refuses the whole call before anything is
    // written — no event, no partial edge set.
    let refused = s
        .add_change_event(Parameters(AddChangeEventReq {
            summary: None,
            rationale: None,
            id: "chg:wind".into(),
            name: Some("Add wind".into()),
            change_type: Some("new_feature".into()),
            subject: None,
            affected: Some(vec![AffectedNodeReq {
                node_type: "Capability".into(),
                node_id: "cap:nope".into(),
                action: None,
            }]),
        }))
        .await;
    assert!(refused.is_err(), "a missing affected node refuses the call");
    let events = jl!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "ChangeEvent".into(),
        ..Default::default()
    })));
    assert!(
        events.as_array().unwrap().is_empty(),
        "nothing was written on refusal"
    );

    // A bogus action is refused the same way.
    let bad_action = s
        .add_change_event(Parameters(AddChangeEventReq {
            summary: None,
            rationale: None,
            id: "chg:wind".into(),
            name: Some("Add wind".into()),
            change_type: Some("new_feature".into()),
            subject: None,
            affected: Some(vec![AffectedNodeReq {
                node_type: "Requirement".into(),
                node_id: "req:physics".into(),
                action: Some("tweaked".into()),
            }]),
        }))
        .await;
    assert!(bad_action.is_err(), "an unknown action refuses the call");

    // The valid call draws the CHANGED edges in the same write.
    let res = j!(s.add_change_event(Parameters(AddChangeEventReq {
        summary: None,
        rationale: None,
        id: "chg:wind".into(),
        name: Some("Add wind".into()),
        change_type: Some("new_feature".into()),
        subject: None,
        affected: Some(vec![
            AffectedNodeReq {
                node_type: "Requirement".into(),
                node_id: "req:physics".into(),
                action: Some("modified".into()),
            },
            AffectedNodeReq {
                node_type: "Capability".into(),
                node_id: "cap:flight".into(),
                action: None,
            },
        ]),
    })));
    assert_eq!(res["event"]["node_id"], "chg:wind");
    let changed = res["changed"].as_array().expect("changed list");
    assert_eq!(changed.len(), 2);
    assert_eq!(
        changed[1]["action"], "modified",
        "an unstated action defaults to modified, and says so"
    );

    // The point of the edges: the event is propagatable as recorded.
    let brief = j!(s.propagate_change(Parameters(PropagateChangeReq {
        change_event_id: "chg:wind".into(),
        max_depth: None,
        full: None
    })));
    let seeds = brief["seeds"].as_array().expect("seeds");
    assert!(
        seeds.iter().any(|v| v == "req:physics") && seeds.iter().any(|v| v == "cap:flight"),
        "the affected nodes are the propagation seeds, got {seeds:?}"
    );
}

// ---- BL-62 · coverage for tools that had none -------------------------------
//
// These 14 tools were never called from tests/tools.rs or smoke_mcp.py — a tool
// nobody drives is a tool whose result contract nobody checks. One walk exercises
// the temporal, resource, realization, analysis, and delete families; the
// question and get_node cases follow.

#[tokio::test]
async fn temporal_resource_and_realization_tools_round_trip() {
    let s = seeded().await;

    // --- temporal: epochs, ordering, pinning, recorded change ---
    j!(s.add_epoch(Parameters(AddEpochReq {
        id: "epoch:v1".into(),
        name: Some("First cut".into()),
        epoch_type: Some("baseline".into()),
        sequence: Some(0),
    })));
    j!(s.add_epoch(Parameters(AddEpochReq {
        id: "epoch:v2".into(),
        name: Some("Second cut".into()),
        epoch_type: Some("revision".into()),
        sequence: Some(1),
    })));
    j!(s.precedes(Parameters(PrecedesReq {
        earlier_epoch: "epoch:v1".into(),
        later_epoch: "epoch:v2".into(),
    })));
    let pinned = j!(s.pin_at_epoch(Parameters(PinAtEpochReq {
        node_type: "Capability".into(),
        node_id: "cap:flight".into(),
        epoch_id: "epoch:v2".into(),
    })));
    assert_eq!(
        pinned["pinned"], "cap:flight",
        "pin reports what it pinned: {pinned}"
    );

    j!(s.add_change_event(Parameters(AddChangeEventReq {
        summary: None,
        rationale: None,
        id: "chg:tune".into(),
        name: Some("Tune the model".into()),
        change_type: Some("refactor".into()),
        subject: None,
        affected: None,
    })));
    // record_change snapshots the prior state before applying — the axis-Z write.
    let rec = j!(s.record_change(Parameters(RecordChangeReq {
        epoch_id: "epoch:v2".into(),
        change_event_id: "chg:tune".into(),
        name: "cap:flight description reworded".into(),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        change_type: "refactor".into(),
        subject: Some("system".into()),
        action: "modified".into(),
    })));
    assert!(
        rec.is_object(),
        "record_change returns a structured result: {rec}"
    );

    // --- resources ---
    j!(s.add_resource(Parameters(ResourceReq {
        id: "res:gpu".into(),
        name: Some("GPU pool".into()),
        provider: Some("cloud".into()),
    })));
    j!(s.require_resource(Parameters(RequireResourceReq {
        from_type: "Component".into(),
        from_id: "cmp:physics".into(),
        resource_id: "res:gpu".into(),
        criticality: Some("required".into()),
    })));
    let requires = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Resource".into(),
        id: "res:gpu".into(),
    })));
    assert_eq!(requires["node"]["node_id"], "res:gpu");

    // --- realization ---
    j!(s.add_artifact(Parameters(AddArtifactReq {
        id: "art:flight-rs".into(),
        name: Some("flight.rs".into()),
        artifact_type: Some("code".into()),
        location: Some("src/flight.rs".into()),
    })));
    j!(s.realizes(Parameters(RealizesReq {
        artifact_id: "art:flight-rs".into(),
        target_type: "Capability".into(),
        target_id: "cap:flight".into(),
        completeness: Some("complete".into()),
        conformance: None,
    })));

    // --- analysis tools (must return well-formed results, not error) ---
    let alloc = j!(s.evaluate_allocation());
    assert!(
        alloc.is_object(),
        "evaluate_allocation returns a scored object: {alloc}"
    );
    let proposal = j!(s.propose_allocation(Parameters(ProposeAllocationReq { resolution: 1.0 })));
    assert!(
        proposal.is_object(),
        "propose_allocation returns clusters: {proposal}"
    );
    let surprises = j!(s.surprising_connections());
    assert!(
        surprises.get("count").is_some() && surprises.get("items").is_some(),
        "surprising_connections returns a {{count, items}} envelope: {surprises}"
    );

    // --- dimension drift: no observations seeded (no MCP tool writes them), so
    //     the tools must report an honest "nothing to trend", never error. ---
    let drift = j!(s.dimension_drift(Parameters(DimensionDriftReq {
        target_id: "cap:flight".into(),
        dimension: "reliability".into(),
    })));
    assert!(
        drift.is_object(),
        "dimension_drift returns a result even with no data: {drift}"
    );
    let drifts = jl!(s.dimension_drifts());
    assert!(
        drifts.as_array().unwrap().is_empty(),
        "no observations → no drifts: {drifts}"
    );

    // --- delete_node: the survivor of a mistake, removed; result names it. ---
    j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:typo".into(),
        name: Some("Typo".into()),
        description: Some("created by mistake".into()),
        level: None,
        distinct_from: None,
    })));
    let deleted = j!(s.delete_node(Parameters(TypedIdReq {
        node_type: "Component".into(),
        id: "cmp:typo".into(),
    })));
    assert_eq!(
        deleted["deleted"],
        serde_json::json!(true),
        "delete_node names the outcome"
    );
    let gone = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Component".into(),
        id: "cmp:typo".into(),
    })));
    // get_node returns one named shape both ways (BL-57): `{node: null}` absent.
    assert!(
        gone["node"].is_null(),
        "absent node reads as {{node: null}}, got {gone}"
    );
}

#[tokio::test]
async fn an_asked_question_can_be_withdrawn() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
    let gap = gaps.as_array().unwrap()[0].clone();
    let gap_id = gap["id"].as_str().unwrap().to_string();

    // Ask it (collect-then-serve records the question).
    let prep = j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: obj(&gap),
        answers: vec![],
        asked_at: None,
    })));
    let pid = prep["prompts"][0]["id"].as_str().unwrap().to_string();
    j!(s.gap_to_prompt(Parameters(GapToPromptReq {
        gap: obj(&gap),
        answers: vec![AgentAnswerReq {
            id: pid,
            text: "Who owns this?".into()
        }],
        asked_at: Some("2026-07-21T00:00:00Z".into()),
    })));
    assert_eq!(jl!(s.open_questions()).as_array().unwrap().len(), 1);

    // Withdraw it — the question leaves the open list.
    let withdrawn = j!(s.withdraw_question(Parameters(WithdrawQuestionReq {
        gap_id: gap_id.clone(),
    })));
    assert_eq!(
        withdrawn["withdrawn"],
        serde_json::json!(true),
        "withdraw reports success: {withdrawn}"
    );
    assert!(
        jl!(s.open_questions()).as_array().unwrap().is_empty(),
        "the withdrawn question is off the open list"
    );
}

// ---- BL-57 · the tool boundary tells the truth about whose fault an error is -

#[tokio::test]
async fn a_caller_mistake_is_invalid_params_not_a_server_fault() {
    // dyno_err used to map every core error to internal_error, so a typo'd id
    // read as "the server broke" instead of "you passed a bad id". A missing
    // capability is the caller's mistake — it must be invalid_params.
    let s = seeded().await;
    let err = s
        .set_capability_status(Parameters(CapabilityStatusReq {
            capability_id: "cap:ghost".into(),
            status: "verified".into(),
        }))
        .await
        .expect_err("a missing capability must be an error");
    assert_eq!(
        err.code,
        rmcp::model::ErrorCode::INVALID_PARAMS,
        "a caller's typo is invalid_params, not internal_error: {err:?}"
    );
}

#[tokio::test]
async fn export_refuses_to_overwrite_without_opt_in() {
    // BL-57: a stray or injected path must not silently clobber an existing
    // file. (The happy path + overwrite=true is covered in the deterministic
    // -file test; this pins the refusal is invalid_params, a caller matter.)
    let s = seeded().await;
    let path = std::env::temp_dir().join(format!("reflow2-guard-{}.json", std::process::id()));
    let p = path.to_str().unwrap().to_string();
    std::fs::write(&path, "pretend this is precious\n").expect("seed a file");
    let err = s
        .export_graph(Parameters(ExportGraphToReq {
            path: Some(p),
            overwrite: None,
            accept_divergence: None,
        }))
        .await
        .expect_err("overwriting an existing file must be refused");
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "pretend this is precious\n",
        "the existing file is untouched"
    );
    std::fs::remove_file(&path).ok();
}

// ---- BL-91: read-side loop_hint (dec:read-hint-shape option C) -------------

#[tokio::test]
async fn read_side_loop_hint_fires_on_debt_then_only_on_change() {
    // A seeded thread leaves the capability unallocated → the loop is owed
    // something, so an orientation read has a real debt to surface.
    let s = seeded().await;

    // First orientation read after the seeding writes surfaces the pointer once.
    let first = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    let hint = first
        .get("loop_hint")
        .and_then(|v| v.as_str())
        .expect("first orientation read on an owing loop carries a loop_hint");
    assert!(
        hint.contains("loop owes"),
        "the hint names the debt and points at loop_status, got {hint:?}"
    );

    // A second read with no write in between stays quiet — the picture has not
    // moved, and boilerplate on every read is the anti-pattern C rejects.
    let second = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    assert!(
        second.get("loop_hint").is_none(),
        "an unchanged owed-set does not repeat the hint, got {second}"
    );

    // A different orientation read in the same generation is also silent.
    let node = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Capability".into(),
        id: "cap:flight".into()
    })));
    assert!(
        node.get("loop_hint").is_none(),
        "no write since last surfaced → still quiet on get_node, got {node}"
    );

    // A write that MOVES the owed-set — a new requirement nothing satisfies —
    // re-arms the hint on the next orientation read, still naming real debt.
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:latency".into(),
        name: Some("Low latency".into()),
        statement: Some("Input to render under 50ms.".into()),
        distinct_from: None,
    })));
    let grown = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    assert!(
        grown.get("loop_hint").is_some(),
        "a write that changed the debt surfaces the hint again, got {grown}"
    );

    // Clearing the debt to nothing (allocate the capability, drop the extra
    // requirement, remove the now-connected component's defect) is exercised by
    // the clean-loop test; here the point is proven: fire once, then only when
    // the picture moves.
}

#[tokio::test]
async fn read_side_loop_hint_silent_when_the_loop_is_clean() {
    // An empty graph owes nothing.
    let s = ReflowService::in_memory().expect("in-memory service");
    let status = j!(s.loop_status(Parameters(Default::default())));
    assert_eq!(status["clean"], true, "empty graph: the loop is clean");

    // A clean loop attaches no read hint — the pointer is state-derived, not
    // static, so silence is the correct output here.
    let read = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    assert!(
        read.get("loop_hint").is_none(),
        "a clean loop attaches no loop_hint, got {read}"
    );
}

// ── Bounded reads and the tool catalogue (github-mcp-server study, 2026-07-25) ──

#[tokio::test]
async fn a_read_too_large_to_return_says_what_it_left_out() {
    // The failure this closes happened to a real session: scan_nodes over 72
    // Decisions returned 96,000 characters and the client truncated it. The
    // drop was real, silent, and outside reflow2 where nothing could name it.
    // A cap is allowed; an unnamed one is not (rule 6).
    let s = ReflowService::in_memory().expect("in-memory service");
    let prose = "x".repeat(3_000);
    for i in 0..30 {
        j!(s.add_capability(Parameters(CapabilityReq {
            id: format!("cap:{i}"),
            name: Some(format!("Capability {i}")),
            description: Some(prose.clone()),
            status: None,
            distinct_from: None,
        })));
    }

    let page = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));

    assert_eq!(
        page["total"], 30,
        "total is how many exist, not how many fit"
    );
    let returned = page["returned"].as_u64().expect("returned");
    assert!(returned < 30, "the payload budget must bite: {page}");
    assert!(returned > 0, "something must come back");
    assert_eq!(page["capped_by"], "size", "the cap must name itself");
    assert_eq!(page["omitted"], 30 - returned, "and count what it withheld");
    assert_eq!(
        page["next_offset"], returned,
        "and say where to resume, or the rest is unreachable"
    );
    assert_eq!(
        page["count"], returned,
        "count keeps meaning items.len() — an old caller reading {{count, items}} is unaffected"
    );

    // Resuming from next_offset reaches the rest: paging is real, not advice.
    let rest = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        offset: Some(returned as usize),
        ..Default::default()
    })));
    assert!(rest["returned"].as_u64().unwrap() > 0, "{rest}");
    assert_eq!(rest["offset"], returned);
}

#[tokio::test]
async fn a_single_node_larger_than_the_budget_is_still_returned() {
    // Otherwise a big node becomes unreachable rather than merely expensive,
    // and an unreachable node is a silent drop by another name.
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:huge".into(),
        name: Some("Huge".into()),
        description: Some("y".repeat(60_000)),
        status: None,
        distinct_from: None,
    })));

    let page = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    assert_eq!(page["returned"], 1, "{}", page["capped_by"]);
    assert_eq!(page["omitted"], 0);
}

#[tokio::test]
async fn brief_gives_the_shape_without_the_prose() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:one".into(),
        name: Some("The one".into()),
        description: Some("z".repeat(5_000)),
        status: None,
        distinct_from: None,
    })));

    let page = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        brief: Some(true),
        ..Default::default()
    })));

    assert_eq!(page["brief"], true);
    let item = &page["items"][0];
    assert_eq!(item["node_id"], "cap:one");
    assert_eq!(item["name"], "The one");
    assert_eq!(
        item["status"], "planned",
        "status is what orientation needs"
    );
    assert!(
        item.get("properties").is_none(),
        "brief must not carry the prose it exists to avoid: {item}"
    );
}

#[tokio::test]
async fn an_explicit_limit_is_reported_as_the_reason_it_stopped() {
    // A caller-imposed bound and a payload bound are different facts, and an
    // agent deciding whether to page needs to know which one it hit.
    let s = ReflowService::in_memory().expect("in-memory service");
    for i in 0..5 {
        j!(s.add_capability(Parameters(CapabilityReq {
            id: format!("cap:{i}"),
            name: Some(format!("Cap {i}")),
            description: Some("small".into()),
            status: None,
            distinct_from: None,
        })));
    }

    let page = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        limit: Some(2),
        ..Default::default()
    })));
    assert_eq!(page["returned"], 2);
    assert_eq!(page["capped_by"], "limit");
    assert_eq!(page["omitted"], 3);

    // Unbounded and small: nothing is capped, and the fields say so plainly.
    let all = j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        ..Default::default()
    })));
    assert_eq!(all["returned"], 5);
    assert_eq!(all["capped_by"], serde_json::Value::Null);
    assert_eq!(all["next_offset"], serde_json::Value::Null);
    assert_eq!(all["omitted"], 0);
}

#[tokio::test]
async fn a_common_word_does_not_outrank_a_rare_one_in_the_catalogue() {
    // Found by BREAKING IT: adding `set_capability_signature` on 2026-08-18
    // evicted `link_artifact` from the five results for "register a file that
    // realizes a capability". The scores were 28/27/26/26/25/24 — a one-point
    // near-tie — because every term was worth the same regardless of how many
    // tools mentioned it. `capability` appears in dozens of descriptions;
    // `file` in a handful.
    //
    // So the catalogue was UNSTABLE UNDER ITS OWN GROWTH: any new tool
    // mentioning a common word could displace the right answer for an
    // unrelated query, silently, and `req:agent-native`'s promise that every
    // capability is reachable over one surface only holds if the agent can
    // find the tool.
    let s = ReflowService::in_memory().expect("in-memory service");
    let found = j!(s.find_tools(Parameters(FindToolsReq {
        query: "register a file that realizes a capability".into(),
        limit: Some(8),
    })));
    let names: Vec<String> = found["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["tool"].as_str().unwrap_or_default().to_string())
        .collect();

    let rank = |n: &str| names.iter().position(|x| x == n);
    let link = rank("link_artifact").expect("link_artifact is found at all");
    // A tool that merely shares the COMMON word must not outrank the one that
    // matches the rare words too. This is the property, not the position.
    for merely_capability in ["set_capability_status", "add_capability"] {
        if let Some(other) = rank(merely_capability) {
            assert!(
                link < other,
                "`link_artifact` matches register/file/realizes/capability; \
                 `{merely_capability}` matches only the commonest of those, and must not \
                 outrank it: {names:?}"
            );
        }
    }
}

#[tokio::test]
async fn the_tool_catalogue_finds_a_tool_by_the_job_it_does() {
    // req:agent-native promises every capability is reachable over one surface.
    // With a surface this large that is only true if the agent can find the
    // tool, so the catalogue is part of the promise, not a convenience.
    let s = ReflowService::in_memory().expect("in-memory service");

    let found = j!(s.find_tools(Parameters(FindToolsReq {
        query: "register a file that realizes a capability".into(),
        limit: None,
    })));
    let names: Vec<String> = found["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["tool"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "link_artifact"),
        "expected link_artifact among {names:?}"
    );
    assert!(
        found["searched"].as_u64().unwrap() > 50,
        "it must search the real served surface, not a hand-kept list"
    );
    assert!(
        found["items"][0]["summary"]
            .as_str()
            .expect("summary")
            .len()
            < 400,
        "summaries are trimmed — a catalogue that costs as much as the surface is pointless"
    );

    // An exact tool name is not a guess: it must rank first.
    let exact = j!(s.find_tools(Parameters(FindToolsReq {
        query: "propagate_change".into(),
        limit: Some(3),
    })));
    assert_eq!(exact["items"][0]["tool"], "propagate_change");

    // A query matching nothing says so rather than returning the surface.
    let empty = j!(s.find_tools(Parameters(FindToolsReq {
        query: "zzzzqqqxx".into(),
        limit: None,
    })));
    assert_eq!(empty["count"], 0);
    assert_eq!(empty["matched"], 0);
    assert!(empty["searched"].as_u64().unwrap() > 50, "it still looked");
}

// ---------------------------------------------------------------------------
// The stateless seat handle (dec:stateless-seat-handle, option (a) + backstop).
//
// These drive `claim_region_inner`, which is `claim_region` with the transport
// question already answered, so BOTH answers are exercised without constructing
// an rmcp `Peer`. The complement assertion below —- one client, one seat -— is
// the one whose absence let the rmcp v3 upgrade pass every gate while seat
// identity was already broken.
// ---------------------------------------------------------------------------

/// A seat to claim with, and something to claim.
async fn claimable() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:seat".into(),
        name: Some("Seat".into()),
    })));
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:ann".into(),
        name: Some("Ann".into()),
        kind: None,
        handle: None,
        description: None,
    })));
    s
}

fn claim_of(seat: Option<&str>) -> ClaimReq {
    ClaimReq {
        contributor_id: "who:ann".into(),
        seed_id: "proj:seat".into(),
        depth: Some(1),
        note: None,
        at: Some("2026-07-30".into()),
        seat: seat.map(str::to_owned),
    }
}

#[tokio::test]
async fn a_claim_with_no_seat_is_refused_when_identity_is_per_request() {
    let s = claimable().await;
    let err = s
        .claim_region_inner(claim_of(None), true)
        .await
        .expect_err("a claim whose owner would change per request must be refused");
    let said = format!("{err:?}");
    // Rule 4: the refusal has to say what WOULD have worked, or it is just a no.
    assert!(
        said.contains("mint_seat"),
        "the refusal must name the tool that fixes it, got: {said}"
    );
    assert!(
        said.contains("2026-07-28"),
        "the refusal must say which protocol revision it is about, got: {said}"
    );
}

#[tokio::test]
async fn a_minted_seat_carries_a_claim_on_a_sessionless_transport() {
    let s = claimable().await;
    let minted = j!(s.mint_seat());
    let seat = minted["seat"].as_str().expect("mint_seat returns a seat");
    assert!(!seat.is_empty());

    let claimed = j!(s.claim_region_inner(claim_of(Some(seat)), true));
    assert_eq!(
        claimed["seat"].as_str(),
        Some(seat),
        "the claim must be owned by the seat the caller carried, verbatim"
    );
}

#[tokio::test]
async fn in_a_session_a_claim_still_needs_no_seat() {
    let s = claimable().await;
    let claimed = j!(s.claim_region_inner(claim_of(None), false));
    assert!(
        claimed["seat"].as_str().is_some_and(|v| !v.is_empty()),
        "in a session the service's own seat identifies the client, as it always did"
    );
}

/// THE COMPLEMENT of test_shared_sessions' "two clients get two seats", and the
/// assertion nothing made until 2026-07-30.
#[tokio::test]
async fn two_claims_from_one_session_report_one_seat() {
    let s = claimable().await;
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:bob".into(),
        name: Some("Bob".into()),
        kind: None,
        handle: None,
        description: None,
    })));
    let first = j!(s.claim_region_inner(claim_of(None), false));
    let mut second = claim_of(None);
    second.contributor_id = "who:bob".into();
    let second = j!(s.claim_region_inner(second, false));
    assert_eq!(
        first["seat"], second["seat"],
        "one session is one seat: a seat per claim is what makes claim_report \
         report one client as several owners"
    );
}

#[tokio::test]
async fn a_supplied_seat_wins_even_inside_a_session() {
    let s = claimable().await;
    let claimed = j!(s.claim_region_inner(claim_of(Some("fleet-worker-7")), false));
    assert_eq!(
        claimed["seat"].as_str(),
        Some("fleet-worker-7"),
        "a durable handle the caller owns is the mechanism, on every transport"
    );
}

#[tokio::test]
async fn an_empty_seat_is_refused_rather_than_quietly_defaulted() {
    let s = claimable().await;
    for blank in ["", "   "] {
        let err = s
            .claim_region_inner(claim_of(Some(blank)), false)
            .await
            .expect_err("an empty owner is not a seat");
        assert!(
            format!("{err:?}").contains("empty"),
            "say that the seat was empty, not something generic"
        );
    }
}

/// `mint_seat` is a name, not a claim: it must leave the design untouched.
#[tokio::test]
async fn minting_a_seat_writes_nothing() {
    let s = claimable().await;
    let before = j!(s.graph_report(Parameters(GraphReportReq::default())));
    let a = j!(s.mint_seat());
    let b = j!(s.mint_seat());
    let after = j!(s.graph_report(Parameters(GraphReportReq::default())));
    assert_eq!(
        before["node_count"], after["node_count"],
        "minting a seat must not touch the graph"
    );
    assert_ne!(
        a["seat"], b["seat"],
        "each mint is a distinct name — that is why one must be KEPT, not re-minted"
    );
}

// ---------------------------------------------------------------------------
// Governance mode (req:mode-is-chosen-and-changeable).
//
// The mode decides whether apply_heal APPLIES structural repairs or only
// proposes them. Until 2026-07-30 it could be set only at genesis, so every
// design carried the `flexible` default and could never move off it.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_project_mode_can_be_chosen_after_genesis() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:m".into(),
        name: Some("Modey".into()),
    })));

    let set = j!(s.set_project_mode(Parameters(ProjectModeReq {
        project_id: "proj:m".into(),
        mode: "rigid".into(),
    })));
    assert_eq!(
        set["properties"]["mode"].as_str(),
        Some("rigid"),
        "the mode a project is governed by must be changeable, not frozen at genesis"
    );
}

#[tokio::test]
async fn choosing_a_mode_preserves_everything_else_about_the_project() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:m".into(),
        name: Some("Modey".into()),
    })));
    j!(s.set_project_mode(Parameters(ProjectModeReq {
        project_id: "proj:m".into(),
        mode: "rigid".into(),
    })));
    let after = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Project".into(),
        id: "proj:m".into()
    })));
    assert_eq!(
        after["node"]["properties"]["name"].as_str(),
        Some("Modey"),
        "a setter writes one property; create_node REPLACES props, so a naive \
         implementation would silently wipe the rest of the node"
    );
}

#[tokio::test]
async fn an_unknown_mode_fails_loud_rather_than_leaving_the_old_one() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:m".into(),
        name: Some("Modey".into()),
    })));
    j!(s.set_project_mode(Parameters(ProjectModeReq {
        project_id: "proj:m".into(),
        mode: "rigid".into(),
    })));

    s.set_project_mode(Parameters(ProjectModeReq {
        project_id: "proj:m".into(),
        mode: "advisory".into(),
    }))
    .await
    .expect_err("a mode outside the enum must be refused, not accepted quietly");

    let after = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Project".into(),
        id: "proj:m".into()
    })));
    assert_eq!(
        after["node"]["properties"]["mode"].as_str(),
        Some("rigid"),
        "a refused write must leave the previous choice intact"
    );
}

#[tokio::test]
async fn setting_the_mode_of_a_project_that_is_not_there_fails_loud() {
    let s = ReflowService::in_memory().expect("in-memory service");
    s.set_project_mode(Parameters(ProjectModeReq {
        project_id: "proj:ghost".into(),
        mode: "rigid".into(),
    }))
    .await
    .expect_err("no silent creation of a project as a side effect of governing it");
}

// ---- The content store on the surface (cap:content-store) -------------------
//
// The store itself is proven in reflow2-core's tests/content.rs. These are the
// SURFACE cases — the ones that only exist because the store is reachable from
// a session, which is what `req:the-store-is-reachable-from-a-session` says was
// missing when the store was first marked realized.

// ---- BL-153 fix shapes (1) and (3) — the bulk forms over the surface -------

/// A rejected bulk write comes back as an ERROR carrying every failure, not as
/// a payload with `applied: false`. A tool result reads as success, and "we
/// wrote nothing" dressed as a result is the silent-failure shape this project
/// forbids — so the error is the signal and the list is the content.
#[tokio::test]
async fn a_rejected_bulk_write_errors_and_still_names_every_failure() {
    let s = seeded().await;
    let err = s
        .create_nodes(Parameters(CreateNodesReq {
            nodes: vec![
                NodeSpecReq {
                    node_type: "NotAType".into(),
                    id: "x:bad".into(),
                    props: None,
                },
                NodeSpecReq {
                    node_type: "AlsoNotAType".into(),
                    id: "x:worse".into(),
                    props: None,
                },
            ],
        }))
        .await
        .expect_err("a rejected batch must not read as success");

    let data = err.data.expect("failures ride along in the error data");
    let failures = data["failures"].as_array().expect("failure list");
    assert_eq!(failures.len(), 2, "BOTH failures, not just the first");
    assert_eq!(failures[0]["id"], "x:bad");
    assert_eq!(failures[1]["id"], "x:worse");
}

/// THE COUNTERWEIGHT for the multi-gap ask. Two gaps whose prompts carry the
/// same id must not cross-contaminate: answers are grouped per gap, so each is
/// replayed against a backend built from its own answers and never sees the
/// other's. Without the grouping this is where a batched handshake would put
/// one gap's question on another gap's record.
#[tokio::test]
async fn each_gap_is_replayed_against_only_its_own_answers() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
    let all = gaps.as_array().unwrap().clone();
    assert!(all.len() >= 2, "need two gaps to prove they stay separate");
    let (a, b) = (all[0].clone(), all[1].clone());

    let prep = j!(s.gaps_to_prompts(Parameters(GapsToPromptsReq {
        gaps: vec![
            GapPromptReq {
                gap: obj(&a),
                answers: vec![]
            },
            GapPromptReq {
                gap: obj(&b),
                answers: vec![]
            },
        ],
        asked_at: None,
    })));
    assert_eq!(prep["status"], "needs_llm");
    let per_gap = prep["gaps"].as_array().expect("grouped by gap");
    assert_eq!(per_gap.len(), 2, "prompts come back grouped per gap");
    let id_a = per_gap[0]["prompts"][0]["id"].as_str().unwrap().to_string();
    let id_b = per_gap[1]["prompts"][0]["id"].as_str().unwrap().to_string();

    let served = j!(s.gaps_to_prompts(Parameters(GapsToPromptsReq {
        gaps: vec![
            GapPromptReq {
                gap: obj(&a),
                answers: vec![AgentAnswerReq {
                    id: id_a,
                    text: "QUESTION FOR THE FIRST GAP".into()
                }]
            },
            GapPromptReq {
                gap: obj(&b),
                answers: vec![AgentAnswerReq {
                    id: id_b,
                    text: "QUESTION FOR THE SECOND GAP".into()
                }]
            },
        ],
        asked_at: Some("2026-08-01".into()),
    })));
    assert_eq!(served["status"], "ok");
    let out = served["gaps"].as_array().unwrap();
    assert_eq!(out[0]["prompt"]["question"], "QUESTION FOR THE FIRST GAP");
    assert_eq!(out[1]["prompt"]["question"], "QUESTION FOR THE SECOND GAP");
    assert_ne!(
        out[0]["question_id"], out[1]["question_id"],
        "two gaps must not collapse onto one question record"
    );

    // Both are on the record, with the wording each was actually given.
    let open = jl!(s.open_questions());
    let asked: Vec<&str> = open
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|q| q["question"].as_str())
        .collect();
    assert!(asked.contains(&"QUESTION FOR THE FIRST GAP"));
    assert!(asked.contains(&"QUESTION FOR THE SECOND GAP"));
}

/// A batch is the prepare pass or the serve pass. Serving half of it would
/// record some questions and silently drop the rest, so it is refused.
#[tokio::test]
async fn a_half_answered_ask_batch_is_refused() {
    let s = seeded().await;
    let gaps = jl!(s.detect_gaps(Parameters(GapScopeReq::default())));
    let all = gaps.as_array().unwrap().clone();
    let (a, b) = (all[0].clone(), all[1].clone());

    let err = s
        .gaps_to_prompts(Parameters(GapsToPromptsReq {
            gaps: vec![
                GapPromptReq {
                    gap: obj(&a),
                    answers: vec![AgentAnswerReq {
                        id: "whatever".into(),
                        text: "answered".into(),
                    }],
                },
                GapPromptReq {
                    gap: obj(&b),
                    answers: vec![],
                },
            ],
            asked_at: None,
        }))
        .await
        .expect_err("a mixed batch is refused");
    assert!(
        err.message.contains("1 of 2"),
        "the refusal says which half, got: {}",
        err.message
    );

    // And nothing was recorded — the refusal is before any write.
    let open = jl!(s.open_questions());
    assert!(open.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn an_empty_ask_batch_is_refused_rather_than_treated_as_a_no_op() {
    let s = seeded().await;
    assert!(
        s.gaps_to_prompts(Parameters(GapsToPromptsReq {
            gaps: vec![],
            asked_at: None,
        }))
        .await
        .is_err()
    );
}

// ---- 2026-08-08 · add_contributor forgives claim_region's field name -------

#[test]
fn add_contributor_accepts_contributor_id_as_an_alias_for_id() {
    // dev_storyflow, 2026-08-07: a worker passed `contributor_id` to
    // add_contributor because that is what `claim_region` calls the SAME handle
    // one step later in the documented sequence, and lost a round trip to
    // `unknown field 'contributor_id'`. The asymmetry carries no meaning.
    let via_alias: ContributorReq = serde_json::from_value(serde_json::json!({
        "contributor_id": "who:alias",
        "name": "Aliased",
    }))
    .expect("contributor_id must be accepted as an alias for id");
    assert_eq!(via_alias.id, "who:alias");

    // The documented name still works and is still the one the schema teaches.
    let via_id: ContributorReq = serde_json::from_value(serde_json::json!({
        "id": "who:canonical",
        "name": "Canonical",
    }))
    .expect("id must keep working");
    assert_eq!(via_id.id, "who:canonical");

    // deny_unknown_fields is NOT loosened by the alias — a genuinely unknown
    // field must still be refused, or this fix would have bought forgiveness
    // by removing a guard.
    let bogus = serde_json::from_value::<ContributorReq>(serde_json::json!({
        "id": "who:x",
        "name": "X",
        "contribtuor_id": "typo",
    }));
    assert!(
        bogus.is_err(),
        "an unknown field must still be refused; the alias forgives one known mistake, \
         not every mistake"
    );
}

/// A WRONG TYPE NAME and an ABSENT NODE must not answer identically.
///
/// dev_storyflow (w-c216679a, 2026-08-09) called `get_node("Epoch", …)` — the
/// stored type is `DesignEpoch` — got a bare `null`, read it as "it isn't
/// there", and their brief then told them to mint a second epoch it explicitly
/// forbids. They caught it only because they distrusted the null.
///
/// The type name is checkable against the schema for free, so answering `null`
/// for it is a fact the server HAS and declines to give.
#[tokio::test]
async fn an_unknown_node_type_is_refused_rather_than_answered_null() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_epoch(Parameters(AddEpochReq {
        id: "epoch:real".into(),
        name: Some("a real epoch".into()),
        epoch_type: Some("revision".into()),
        sequence: Some(1),
    })));

    // THE REPRODUCTION: the type name they used.
    let err = s
        .get_node(Parameters(TypedIdReq {
            node_type: "Epoch".into(),
            id: "epoch:real".into(),
        }))
        .await
        .expect_err("an unknown node type must be refused, not answered null");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not a node type"),
        "it must say the TYPE is unknown: {msg}"
    );
    assert!(
        msg.contains("DesignEpoch"),
        "and point at the one they meant: {msg}"
    );

    // POSITIVE CONTROL, both directions — the refusal must be about the type
    // and nothing else.
    let found = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "DesignEpoch".into(),
        id: "epoch:real".into()
    })));
    assert_eq!(found["node"]["node_id"], "epoch:real");

    // A REAL type with an absent id still answers null — that is the honest
    // "no such node", and it must not have been collateral damage.
    let absent = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "DesignEpoch".into(),
        id: "epoch:nope".into()
    })));
    assert!(
        absent["node"].is_null(),
        "an absent node under a real type is still null: {absent}"
    );
}

/// `scan_nodes(level:)` is how you ask for one rung of the decomposition.
///
/// THE DEFECT IT CLOSES: `Component.level` was indexed and populated and
/// nothing served it, so callers derived the top tier by walking CONTAINS and
/// taking the parentless nodes — which returns leaves nobody wired to a parent.
/// Measured on reflow2's own design 2026-08-18: 8 subsystems by declared level,
/// 2 leaves by spine position. Two reasonable queries, two answers.
#[tokio::test]
async fn scan_nodes_filters_by_decomposition_level() {
    let s = ReflowService::in_memory().expect("in-memory service");
    for (id, level) in [
        ("cmp:sub-a", Some("subsystem")),
        ("cmp:sub-b", Some("subsystem")),
        ("cmp:leaf", Some("component")),
        // No level at all: the schema defaults it to `component`, so it must
        // still answer to that filter rather than vanishing from both answers.
        ("cmp:unset", None),
    ] {
        j!(s.add_component(Parameters(ComponentReq {
            id: id.into(),
            name: Some(id.into()),
            description: Some("x".into()),
            level: level.map(str::to_string),
            distinct_from: None,
        })));
    }

    let subs = j!(s.scan_nodes(Parameters(ScanReq {
        node_type: "Component".into(),
        level: Some("subsystem".into()),
        limit: None,
        offset: None,
        brief: Some(true),
    })));
    assert_eq!(subs["total"], 2, "{subs}");

    let comps = j!(s.scan_nodes(Parameters(ScanReq {
        node_type: "Component".into(),
        level: Some("component".into()),
        limit: None,
        offset: None,
        brief: Some(true),
    })));
    assert_eq!(
        comps["total"], 2,
        "an unset level must answer to `component`: {comps}"
    );

    // Unfiltered still returns everything — the filter adds a question, it
    // does not change the default answer.
    let all = j!(s.scan_nodes(Parameters(ScanReq {
        node_type: "Component".into(),
        level: None,
        limit: None,
        offset: None,
        brief: Some(true),
    })));
    assert_eq!(all["total"], 4, "{all}");
}

/// An unknown rung and a wrong node type are REFUSED, not answered empty.
///
/// "No Components at that rung" and "that is not a rung" are different facts,
/// and an empty list says the first while meaning the second — which is the
/// silent-wrong-answer this whole filter exists to end.
#[tokio::test]
async fn a_bad_level_is_refused_rather_than_answered_empty() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_component(Parameters(ComponentReq {
        id: "cmp:x".into(),
        name: Some("x".into()),
        description: Some("x".into()),
        level: Some("subsystem".into()),
        distinct_from: None,
    })));

    let bad_level = s
        .scan_nodes(Parameters(ScanReq {
            node_type: "Component".into(),
            level: Some("susbystem".into()), // typo
            limit: None,
            offset: None,
            brief: Some(true),
        }))
        .await;
    let err = bad_level
        .expect_err("a typo'd rung must be refused")
        .to_string();
    assert!(err.contains("not a decomposition level"), "{err}");
    // Rule 4: the refusal names what WOULD have worked.
    assert!(err.contains("subsystem"), "{err}");

    let wrong_type = s
        .scan_nodes(Parameters(ScanReq {
            node_type: "Requirement".into(),
            level: Some("subsystem".into()),
            limit: None,
            offset: None,
            brief: Some(true),
        }))
        .await;
    let err = wrong_type
        .expect_err("only Component carries a level")
        .to_string();
    assert!(err.contains("Component"), "{err}");
}
