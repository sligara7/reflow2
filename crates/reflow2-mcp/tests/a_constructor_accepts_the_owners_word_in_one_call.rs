//! The owner's word rides the same call as the status it signs.
//!
//! Before 2026-09-06 recording a decision the user had ALREADY made took
//! `add_decision` then `set_decision_status`, and the two could not be batched
//! (a harness emits a batch unordered, so the setter could run before the node
//! existed). The same day a session took the status half and forgot the
//! signature half, and CI's intent-authority gate went red
//! (`fact:an-accepted-status-written-without-its-approver-failed-the-intent-gate-in-ci`).
//!
//! The shape ruled in `dec:idea-should-a-constructor-accept-the-owners-word-in-one-call`:
//! a settling status on a CONSTRUCTOR is refused unless `approver` is named,
//! and the approver is written as the `AUTHORED_BY role=approver` edge the gate
//! reads; the SETTERS stay lenient (they have consumers) but REPORT a settling
//! status that carries nobody's name. `add_verification` takes its targets and
//! the run it just had in the same call.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value as JsonValue;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:ann".into(),
        name: Some("Ann".into()),
        kind: None,
        handle: None,
        description: None,
    })));
    s
}

async fn doc(s: &ReflowService) -> JsonValue {
    j!(s.export_graph(Parameters(ExportGraphToReq {
        path: None,
        overwrite: None,
        accept_divergence: None,
    })))
}

/// The approver edges on `from`, as (to, role, acted_at).
async fn approver_edges(s: &ReflowService, from: &str) -> Vec<(String, String, Option<String>)> {
    doc(s).await["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .filter(|e| e["edge_type"] == "AUTHORED_BY" && e["from_id"] == from)
        .map(|e| {
            (
                e["to_id"].as_str().unwrap_or("").to_string(),
                e["properties"]["role"].as_str().unwrap_or("").to_string(),
                e["properties"]["acted_at"].as_str().map(str::to_string),
            )
        })
        .collect()
}

async fn node_exists(s: &ReflowService, id: &str) -> bool {
    doc(s).await["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .any(|n| n["node_id"] == id)
}

fn decision(id: &str, status: Option<&str>, approver: Option<&str>) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some("Cumulative totals, not deltas".into()),
        decision: Some(
            "The outdoor unit sends running totals so a lost packet heals itself.".into(),
        ),
        rationale: None,
        distinct_from: None,
        kind: Some("choice".into()),
        related_to: None,
        no_relation_note: None,
        status: status.map(str::to_string),
        approver: approver.map(str::to_string),
        acted_at: Some("2026-09-06".into()),
    }
}

fn requirement(id: &str, status: Option<&str>, approver: Option<&str>) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some("A dropped reading costs nothing".into()),
        statement: Some("A lost packet must not lose a rainfall total.".into()),
        distinct_from: None,
        status: status.map(str::to_string),
        approver: approver.map(str::to_string),
        acted_at: Some("2026-09-06".into()),
        priority: None,
    }
}

// ---- constructors ----------------------------------------------------------

#[tokio::test]
async fn a_decision_lands_accepted_with_its_signature_in_one_call() {
    let s = svc().await;
    let out = j!(s.add_decision(Parameters(decision(
        "dec:totals",
        Some("accepted"),
        Some("who:ann")
    ))));
    assert_eq!(out["properties"]["status"], "accepted");
    assert_eq!(
        approver_edges(&s, "dec:totals").await,
        vec![(
            "who:ann".to_string(),
            "approver".to_string(),
            Some("2026-09-06".to_string())
        )],
        "the signature is the same edge the intent gate reads"
    );
}

#[tokio::test]
async fn a_settling_status_with_nobodys_name_is_refused_and_nothing_is_written() {
    let s = svc().await;
    let err = s
        .add_decision(Parameters(decision("dec:totals", Some("accepted"), None)))
        .await
        .expect_err("accepted with no approver is the forgery the rule forbids");
    assert!(err.to_string().contains("approver"), "{err}");
    assert!(
        !node_exists(&s, "dec:totals").await,
        "a refusal leaves no half-signed node"
    );

    let err = s
        .add_requirement(Parameters(requirement(
            "req:totals",
            Some("accepted"),
            None,
        )))
        .await
        .expect_err("a requirement past proposed is settled intent too");
    assert!(err.to_string().contains("approver"), "{err}");
    assert!(!node_exists(&s, "req:totals").await);
}

#[tokio::test]
async fn an_approver_naming_no_contributor_is_refused_before_anything_is_written() {
    let s = svc().await;
    let err = s
        .add_decision(Parameters(decision(
            "dec:totals",
            Some("accepted"),
            Some("who:nobody"),
        )))
        .await
        .expect_err("a typo must not attach the owner's authority to a name nobody can check");
    assert!(err.to_string().contains("who:nobody"), "{err}");
    assert!(!node_exists(&s, "dec:totals").await);
}

#[tokio::test]
async fn the_landing_status_still_needs_no_signature() {
    let s = svc().await;
    let out = j!(s.add_decision(Parameters(decision("dec:totals", None, None))));
    assert_eq!(out["properties"]["status"], "proposed");
    // A fresh design: the second decision would otherwise be refused as a
    // near-duplicate of the first, which is the guard working, not this rule.
    let s = svc().await;
    let out = j!(s.add_decision(Parameters(decision("dec:totals-2", Some("proposed"), None))));
    assert_eq!(out["properties"]["status"], "proposed");
    assert!(approver_edges(&s, "dec:totals-2").await.is_empty());
}

#[tokio::test]
async fn a_requirement_lands_accepted_with_its_signature_in_one_call() {
    let s = svc().await;
    let out = j!(s.add_requirement(Parameters(requirement(
        "req:totals",
        Some("accepted"),
        Some("who:ann")
    ))));
    assert_eq!(out["properties"]["status"], "accepted");
    assert_eq!(approver_edges(&s, "req:totals").await.len(), 1);
}

#[tokio::test]
async fn a_rules_power_is_settled_intent_and_needs_the_owners_name() {
    let s = svc().await;
    let rule = |enforced: Option<bool>, approver: Option<&str>| DesignRuleReq {
        id: "rule:branch-first".into(),
        name: Some("Branch before pushing".into()),
        statement: Some("Nothing lands on main directly.".into()),
        category: Some("convention".into()),
        enforced,
        distinct_from: None,
        approver: approver.map(str::to_string),
        acted_at: None,
    };
    let err = s
        .add_design_rule(Parameters(rule(Some(true), None)))
        .await
        .expect_err("stating a rule's power is the owner's act");
    assert!(err.to_string().contains("approver"), "{err}");
    assert!(!node_exists(&s, "rule:branch-first").await);

    // Unstated power is the landing state and needs nobody's name.
    j!(s.add_design_rule(Parameters(rule(None, None))));
    assert!(approver_edges(&s, "rule:branch-first").await.is_empty());

    // Advisory is a stated power too — `false` is an answer, not an absence.
    let out = j!(s.add_design_rule(Parameters(rule(Some(false), Some("who:ann")))));
    assert_eq!(out["properties"]["enforced"], false);
    assert_eq!(approver_edges(&s, "rule:branch-first").await.len(), 1);
}

// ---- setters ---------------------------------------------------------------

#[tokio::test]
async fn a_setter_draws_the_signature_when_given_one_and_reports_its_absence_when_not() {
    let s = svc().await;
    j!(s.add_decision(Parameters(decision("dec:totals", None, None))));

    let out = j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:totals".into(),
        status: "accepted".into(),
        approver: None,
        acted_at: None,
    })));
    assert_eq!(
        out["properties"]["status"], "accepted",
        "the setter stays lenient"
    );
    assert!(
        out["carries_nobodys_name"]
            .as_str()
            .is_some_and(|n| n.contains("approver")),
        "but says so: {out}"
    );

    let out = j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:totals".into(),
        status: "accepted".into(),
        approver: Some("who:ann".into()),
        acted_at: Some("2026-09-06".into()),
    })));
    assert!(out.get("carries_nobodys_name").is_none(), "{out}");
    assert_eq!(approver_edges(&s, "dec:totals").await.len(), 1);

    // Retiring is not settling: superseding without a name is not flagged.
    let out = j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:totals".into(),
        status: "superseded".into(),
        approver: None,
        acted_at: None,
    })));
    assert!(out.get("carries_nobodys_name").is_none(), "{out}");

    j!(s.add_requirement(Parameters(requirement("req:totals", None, None))));
    let out = j!(s.set_requirement_status(Parameters(RequirementStatusReq {
        requirement_id: "req:totals".into(),
        status: "accepted".into(),
        approver: None,
        acted_at: None,
    })));
    assert!(out["carries_nobodys_name"].is_string(), "{out}");
    let err = s
        .set_requirement_status(Parameters(RequirementStatusReq {
            requirement_id: "req:totals".into(),
            status: "accepted".into(),
            approver: Some("who:nobody".into()),
            acted_at: None,
        }))
        .await
        .expect_err("an unknown approver is refused on the setter too");
    assert!(err.to_string().contains("who:nobody"), "{err}");
}

// ---- add_verification ------------------------------------------------------

#[tokio::test]
async fn a_check_records_its_targets_and_the_run_it_just_had_in_one_call() {
    let s = svc().await;
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:totals".into(),
        name: Some("Running totals".into()),
        description: Some("sends cumulative totals".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    let out = j!(s.add_verification(Parameters(VerificationReq {
        id: "ver:totals-heal".into(),
        name: Some("a dropped packet heals on the next reading".into()),
        method: Some("test".into()),
        level: None,
        description: None,
        verifies: Some(vec![VerifyTargetReq {
            target_type: None,
            target_id: "cap:totals".into(),
        }]),
        status: Some("passing".into()),
        findings: Some("12 of 12 passed".into()),
        last_run_at: Some("2026-09-06".into()),
    })));
    assert_eq!(out["properties"]["status"], "passing");
    assert_eq!(out["properties"]["findings"], "12 of 12 passed");
    assert_eq!(out["properties"]["last_run_at"], "2026-09-06");
    assert_eq!(out["verifies"], serde_json::json!(["cap:totals"]));
    let d = doc(&s).await;
    assert!(
        d["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["edge_type"] == "VERIFIES"
                && e["from_id"] == "ver:totals-heal"
                && e["to_id"] == "cap:totals"),
        "the VERIFIES edge was drawn in the same call"
    );
}

#[tokio::test]
async fn a_finding_without_a_run_is_refused_and_an_unknown_target_refuses_the_whole_call() {
    let s = svc().await;
    let base = || VerificationReq {
        id: "ver:totals-heal".into(),
        name: Some("heals".into()),
        method: None,
        level: None,
        description: None,
        verifies: None,
        status: None,
        findings: None,
        last_run_at: None,
    };
    let mut r = base();
    r.findings = Some("12 of 12".into());
    let err = s
        .add_verification(Parameters(r))
        .await
        .expect_err("a finding belongs to a run, and a run has an outcome");
    assert!(err.to_string().contains("status"), "{err}");
    assert!(!node_exists(&s, "ver:totals-heal").await);

    let mut r = base();
    r.verifies = Some(vec![VerifyTargetReq {
        target_type: None,
        target_id: "cap:does-not-exist".into(),
    }]);
    let err = s
        .add_verification(Parameters(r))
        .await
        .expect_err("an unresolvable target refuses the create, not just the edge");
    assert!(err.to_string().contains("cap:does-not-exist"), "{err}");
    assert!(
        !node_exists(&s, "ver:totals-heal").await,
        "no half-wired check left behind"
    );
}
