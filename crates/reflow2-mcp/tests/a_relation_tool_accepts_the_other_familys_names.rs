//! Every relation tool accepts the OTHER family's spellings for its two ends.
//!
//! dev_storyflow's fleet counted seven spellings of "which node" across one
//! workflow and put the sharpest form as a handoff: `search_design` hands back
//! `node_id`, and the tool you reach for next rejects that exact key. Every
//! wrong guess was informed by the tool used immediately before — the
//! signature of an interface that is locally sensible and globally
//! inconsistent, not of a careless caller.
//!
//! Ruled 2026-09-06 (`dec:idea-one-way-to-name-which-node-across-the-tool-surface`):
//! one convention, taught once — a role-named end plus `target_*` when the ends
//! play different roles, `from_*` / `to_*` when they are peers — and the other
//! family's names accepted as serde ALIASES, plus `node_id` / `node_type` for
//! the from end. The schema still publishes one name; `deny_unknown_fields`
//! still catches a typo; nothing existing changes meaning.
//!
//! This test is deliberately table-shaped: every relation request struct, its
//! taught spelling, and the alias spellings, so a struct that gains an end
//! without joining the table fails here.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde::de::DeserializeOwned;
use serde_json::{Value as JsonValue, json};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// Both spellings must deserialize, and to the same request.
fn both<T: DeserializeOwned + std::fmt::Debug>(taught: JsonValue, alias: JsonValue) -> (T, T) {
    let a: T = serde_json::from_value(taught.clone())
        .unwrap_or_else(|e| panic!("taught spelling refused: {e} — {taught}"));
    let b: T = serde_json::from_value(alias.clone())
        .unwrap_or_else(|e| panic!("alias spelling refused: {e} — {alias}"));
    (a, b)
}

#[test]
fn the_from_to_family_accepts_role_names_and_node_id() {
    let (a, b) = both::<AllocateReq>(
        json!({"from_id": "cap:x", "to_id": "cmp:y"}),
        json!({"capability_id": "cap:x", "component_id": "cmp:y"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (_, b) = both::<AllocateReq>(
        json!({"from_id": "cap:x", "to_id": "cmp:y"}),
        json!({"node_id": "cap:x", "to_id": "cmp:y"}),
    );
    assert_eq!(
        b.from_id, "cap:x",
        "search_design's node_id names the from end"
    );

    let (a, b) = both::<SatisfiesReq>(
        json!({"from_id": "cap:x", "to_id": "req:y"}),
        json!({"capability_id": "cap:x", "requirement_id": "req:y"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<ProvidesReq>(
        json!({"from_id": "cmp:x", "to_id": "if:y"}),
        json!({"component_id": "cmp:x", "interface_id": "if:y"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<ConsumesReq>(
        json!({"from_id": "cmp:x", "to_id": "if:y"}),
        json!({"component_id": "cmp:x", "interface_id": "if:y"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<DecomposesReq>(
        json!({"from_id": "req:p", "to_id": "req:c"}),
        json!({"parent_id": "req:p", "child_id": "req:c"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<DependsOnReq>(
        json!({"from_id": "cmp:a", "to_id": "cmp:b"}),
        json!({"dependent_id": "cmp:a", "dependency_id": "cmp:b"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<ContainComponentReq>(
        json!({"from_id": "cmp:p", "to_id": "cmp:c"}),
        json!({"parent_id": "cmp:p", "child_id": "cmp:c"}),
    );
    assert_eq!((a.from_id, a.to_id), (b.from_id, b.to_id));
    let (a, b) = both::<GovernedByReq>(
        json!({"from_id": "cap:x", "to_id": "dec:y"}),
        json!({"node_id": "cap:x", "node_type": "Capability", "to_id": "dec:y"}),
    );
    assert_eq!(a.from_id, b.from_id);
    assert_eq!(b.from_type.as_deref(), Some("Capability"));
}

#[test]
fn the_role_named_family_accepts_from_and_to() {
    let (a, b) = both::<VerifiesReq>(
        json!({"verification_id": "ver:x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "ver:x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!(
        (a.verification_id, a.target_type, a.target_id),
        (b.verification_id, b.target_type, b.target_id)
    );
    let (_, b) = both::<VerifiesReq>(
        json!({"verification_id": "ver:x", "target_id": "cap:y"}),
        json!({"node_id": "ver:x", "target_id": "cap:y"}),
    );
    assert_eq!(b.verification_id, "ver:x");

    let (a, b) = both::<RealizesReq>(
        json!({"artifact_id": "art:x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "art:x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!((a.artifact_id, a.target_id), (b.artifact_id, b.target_id));
    let (a, b) = both::<ConstrainsReq>(
        json!({"constraint_id": "con:x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "con:x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!(
        (a.constraint_id, a.target_id),
        (b.constraint_id, b.target_id)
    );
    let (a, b) = both::<DocumentsReq>(
        json!({"artifact_id": "art:x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "art:x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!((a.artifact_id, a.target_id), (b.artifact_id, b.target_id));
    let (a, b) = both::<ReleaseIncludesReq>(
        json!({"release_id": "rel:x", "target_type": "Artifact", "target_id": "art:y"}),
        json!({"from_id": "rel:x", "to_type": "Artifact", "to_id": "art:y"}),
    );
    assert_eq!((a.release_id, a.target_id), (b.release_id, b.target_id));
    let (a, b) = both::<EvidenceScopeReq>(
        json!({"verification_id": "ver:x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "ver:x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!(
        (a.verification_id, a.target_id),
        (b.verification_id, b.target_id)
    );
    let (a, b) = both::<ScheduleForReq>(
        json!({"item_type": "Requirement", "item_id": "req:x", "target_type": "DesignEpoch", "target_id": "epoch:y"}),
        json!({"from_type": "Requirement", "from_id": "req:x", "to_type": "DesignEpoch", "to_id": "epoch:y"}),
    );
    assert_eq!((a.item_id, a.target_id), (b.item_id, b.target_id));
    let (a, b) = both::<GateOnReq>(
        json!({"subject_type": "Release", "subject_id": "rel:x", "target_type": "Capability", "target_id": "cap:y", "kind": "readiness", "min_level": 3}),
        json!({"from_type": "Release", "from_id": "rel:x", "to_type": "Capability", "to_id": "cap:y", "kind": "readiness", "min_level": 3}),
    );
    assert_eq!((a.subject_id, a.target_id), (b.subject_id, b.target_id));
    let (a, b) = both::<LinkArtifactReq>(
        json!({"artifact_id": "art:x", "name": "x", "target_type": "Capability", "target_id": "cap:y"}),
        json!({"from_id": "art:x", "name": "x", "to_type": "Capability", "to_id": "cap:y"}),
    );
    assert_eq!((a.artifact_id, a.target_id), (b.artifact_id, b.target_id));
    let (a, b) = both::<DeployToReq>(
        json!({"release_id": "rel:x", "environment_id": "env:y"}),
        json!({"from_id": "rel:x", "to_id": "env:y"}),
    );
    assert_eq!(
        (a.release_id, a.environment_id),
        (b.release_id, b.environment_id)
    );
    let (a, b) = both::<PerformedInReq>(
        json!({"verification_id": "ver:x", "environment_id": "env:y"}),
        json!({"from_id": "ver:x", "to_id": "env:y"}),
    );
    assert_eq!(
        (a.verification_id, a.environment_id),
        (b.verification_id, b.environment_id)
    );
    let (a, b) = both::<PartOfFlowReq>(
        json!({"capability_id": "cap:x", "flow_id": "flow:y"}),
        json!({"from_id": "cap:x", "to_id": "flow:y"}),
    );
    assert_eq!((a.capability_id, a.flow_id), (b.capability_id, b.flow_id));
    let (a, b) = both::<RegisterAlternativeReq>(
        json!({"decision_id": "dec:x", "artifact_id": "art:y", "name": "y", "location": "alt/y.md"}),
        json!({"from_id": "dec:x", "to_id": "art:y", "name": "y", "location": "alt/y.md"}),
    );
    assert_eq!(
        (a.decision_id, a.artifact_id),
        (b.decision_id, b.artifact_id)
    );
    let (a, b) = both::<ContainsReq>(
        json!({"project_id": "proj:x", "child_type": "Component", "child_id": "cmp:y"}),
        json!({"from_id": "proj:x", "to_type": "Component", "to_id": "cmp:y"}),
    );
    assert_eq!((a.project_id, a.child_id), (b.project_id, b.child_id));
    let (a, b) = both::<PinAtEpochReq>(
        json!({"node_type": "Capability", "node_id": "cap:x", "epoch_id": "epoch:y"}),
        json!({"from_type": "Capability", "from_id": "cap:x", "to_id": "epoch:y"}),
    );
    assert_eq!((a.node_id, a.epoch_id), (b.node_id, b.epoch_id));
    let (a, b) = both::<MoveComponentReq>(
        json!({"child_id": "cmp:c", "new_parent_id": "cmp:p"}),
        json!({"from_id": "cmp:c", "parent_id": "cmp:p"}),
    );
    assert_eq!((a.child_id, a.new_parent_id), (b.child_id, b.new_parent_id));
}

#[test]
fn the_who_family_accepts_node_and_to() {
    let (a, b) = both::<AuthoredByReq>(
        json!({"from_type": "Decision", "from_id": "dec:x", "contributor_id": "who:y"}),
        json!({"node_type": "Decision", "node_id": "dec:x", "to_id": "who:y"}),
    );
    assert_eq!((a.from_id, a.contributor_id), (b.from_id, b.contributor_id));
    let (a, b) = both::<OwnedByReq>(
        json!({"from_type": "Component", "from_id": "cmp:x", "contributor_id": "who:y"}),
        json!({"node_type": "Component", "node_id": "cmp:x", "to_id": "who:y"}),
    );
    assert_eq!((a.from_id, a.contributor_id), (b.from_id, b.contributor_id));
    let (a, b) = both::<InvalidatesReq>(
        json!({"from_type": "ChangeEvent", "from_id": "chg:x", "finding_type": "TemporalFact", "finding_id": "fact:y"}),
        json!({"node_type": "ChangeEvent", "node_id": "chg:x", "to_type": "TemporalFact", "to_id": "fact:y"}),
    );
    assert_eq!((a.from_id, a.finding_id), (b.from_id, b.finding_id));
    let (a, b) = both::<CalibratedAgainstReq>(
        json!({"from_type": "Verification", "from_id": "ver:x", "evidence_type": "TemporalFact", "evidence_id": "fact:y"}),
        json!({"node_type": "Verification", "node_id": "ver:x", "to_type": "TemporalFact", "to_id": "fact:y"}),
    );
    assert_eq!((a.from_id, a.evidence_id), (b.from_id, b.evidence_id));
    let (a, b) = both::<RequireResourceReq>(
        json!({"from_type": "Component", "from_id": "cmp:x", "resource_id": "res:y"}),
        json!({"node_type": "Component", "node_id": "cmp:x", "to_id": "res:y"}),
    );
    assert_eq!((a.from_id, a.resource_id), (b.from_id, b.resource_id));
    let (a, b) = both::<AnswersReq>(
        json!({"from_type": "Decision", "from_id": "dec:x", "question_id": "question:y"}),
        json!({"node_type": "Decision", "node_id": "dec:x", "to_id": "question:y"}),
    );
    assert_eq!((a.from_id, a.question_id), (b.from_id, b.question_id));
}

/// A typo is still a typo, and both spellings of one end is a duplicate — the
/// aliases forgive the other family's name, never a wrong one.
#[test]
fn an_alias_is_not_a_loophole() {
    let e = serde_json::from_value::<VerifiesReq>(
        json!({"verifcation_id": "ver:x", "target_id": "cap:y"}),
    )
    .expect_err("a misspelling is refused");
    assert!(e.to_string().contains("unknown field"), "{e}");
    let e = serde_json::from_value::<VerifiesReq>(
        json!({"verification_id": "ver:x", "from_id": "ver:x", "target_id": "cap:y"}),
    )
    .expect_err("both spellings of one end is a duplicate, not a choice");
    assert!(e.to_string().contains("duplicate field"), "{e}");
}

/// End to end: the handoff the fleet reported. search_design's `node_id`, and
/// the last tool's `capability_id`, both land on the served surface.
#[tokio::test]
async fn the_handoff_lands_end_to_end() {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:dry".into(),
        name: Some("Stay dry".into()),
        statement: Some("The unit survives rain.".into()),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:seal".into(),
        name: Some("Sealed housing".into()),
        description: Some("keeps water out".into()),
        status: None,
        distinct_from: None,
    })));
    j!(s.add_component(Parameters(
        serde_json::from_value(
            json!({"id": "cmp:housing", "name": "Housing", "description": "the box"})
        )
        .unwrap()
    )));
    // The role names, as the last constructor would have taught them.
    let e = j!(s.allocate(Parameters(
        serde_json::from_value(json!({"capability_id": "cap:seal", "component_id": "cmp:housing"}))
            .unwrap()
    )));
    assert_eq!(e["edge_type"], "ALLOCATED_TO");
    // search_design's spelling, straight into the next tool.
    let e = j!(s.satisfies(Parameters(
        serde_json::from_value(json!({"node_id": "cap:seal", "requirement_id": "req:dry"}))
            .unwrap()
    )));
    assert_eq!(e["edge_type"], "SATISFIES");
    // And the from/to family on a role-named tool.
    j!(s.add_verification(Parameters(
        serde_json::from_value(json!({"id": "ver:rain", "name": "rain test"})).unwrap()
    )));
    let e = j!(s.verifies(Parameters(
        serde_json::from_value(json!({"from_id": "ver:rain", "to_id": "cap:seal"})).unwrap()
    )));
    assert_eq!(e["edge_type"], "VERIFIES");
}
