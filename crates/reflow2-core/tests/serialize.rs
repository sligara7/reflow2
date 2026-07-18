//! Guards the wire vocabulary of the JSON-serialized core types (SP-3, Step 1).
//!
//! The MCP surface returns these result types as JSON to the calling agent. The
//! fieldless enums serialize via `#[serde(rename_all = "snake_case")]`, which
//! must stay identical to their `as_str()` — the vocabulary the schema and the
//! rest of the system already speak. These asserts fail loud if a variant rename
//! ever lets the wire format drift from the domain vocabulary.

use reflow2_core::{ChangeType, Dimension, GapSource, ImpactDirection};

#[test]
fn enums_serialize_to_their_as_str_vocabulary() {
    // GapSource — the multi-word cases are the risky ones.
    assert_eq!(
        serde_json::to_string(&GapSource::UnallocatedCapability).unwrap(),
        "\"unallocated_capability\""
    );
    assert_eq!(
        serde_json::to_string(&GapSource::MissingIntermediateLevel).unwrap(),
        "\"missing_intermediate_level\""
    );
    // ImpactDirection, Dimension, ChangeType — one of each remaining family.
    assert_eq!(
        serde_json::to_string(&ImpactDirection::Downstream).unwrap(),
        "\"downstream\""
    );
    assert_eq!(
        serde_json::to_string(&Dimension::Maintainability).unwrap(),
        "\"maintainability\""
    );
    assert_eq!(
        serde_json::to_string(&ChangeType::RequirementCreep).unwrap(),
        "\"requirement_creep\""
    );
}

#[test]
fn heal_proposal_round_trips_through_json() {
    // apply_heal takes a HealProposal back as JSON — Serialize+Deserialize must
    // be symmetric, including the strategy enum and the generated-content stubs.
    use reflow2_core::{
        GeneratedContentStub, HealOp, HealOperation, HealProposal, HealStrategy, SkippedOperation,
    };
    let proposal = HealProposal {
        target_id: "proj:x".to_string(),
        strategy: HealStrategy::Balanced,
        issues_addressed: vec!["heal:1".to_string()],
        operations: vec![HealOperation {
            issue_id: "heal:1".to_string(),
            op: HealOp::Merge {
                keep_type: "Component".to_string(),
                keep_id: "cmp:a".to_string(),
                remove_type: "Component".to_string(),
                remove_id: "cmp:b".to_string(),
            },
        }],
        generated_content: vec![GeneratedContentStub {
            for_issue: "heal:2".to_string(),
            kind: "Decision".to_string(),
            description: "reconcile".to_string(),
        }],
        skipped_operations: vec![SkippedOperation {
            reference: "heal:3".to_string(),
            reason: "capped".to_string(),
        }],
        confidence: 0.9,
        requires_human_review: true,
        summary: "one merge".to_string(),
    };

    let json = serde_json::to_string(&proposal).unwrap();
    let back: HealProposal = serde_json::from_str(&json).unwrap();
    assert_eq!(back.strategy, HealStrategy::Balanced);
    assert_eq!(back.operations.len(), 1);
    assert_eq!(back.generated_content[0].kind, "Decision");
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
}
