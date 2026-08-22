//! Can the ledger say "the thing was right and its description of itself was wrong"?
//!
//! Until 2026-08-22 it could not, and the record shows what people did instead:
//! 13 ChangeEvents on this project's own graph describe a documentation-only
//! change under THREE labels that cannot all be right — `refactor` ×8
//! ("Comment reference to the renamed category; behaviour unchanged"),
//! `test_failure_fix` ×4 ("SETUP.md aligned to the corrected wording… THE
//! DESIGN HOLDS"), and `defect_fix` ×1. That is the same evidence shape that
//! justified `defect_fix` itself, which was added after five sessions across
//! three projects each reached for a different least-wrong value.
//!
//! A FOURTH REACH CAME FROM ANOTHER PROJECT AND IS DELIBERATELY STILL UNMET:
//! dev_storyflow tried `change_type: "correction"`, pinned by
//! `a_rejected_enum_lists_the_legal_values` in reflow2-mcp. Correcting a RECORD
//! that was wrong is the `record` half of `ChangeSubject` — which exists in the
//! schema and which no served tool can write. Answering it with this variant
//! because the two are adjacent would paper over a real gap, so it is left open.

use reflow2_core::graph::DesignGraph;
use reflow2_core::{ChangeAction, ChangeRecord, ChangeType, EpochType};

fn graph_with_a_described_thing() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_epoch("epoch:e", "The epoch", EpochType::Revision, 1)
        .expect("epoch");
    g.add_artifact(
        "art:thing",
        "thing.rs",
        Some("code"),
        Some("crates/x/src/thing.rs"),
    )
    .expect("artifact");
    g
}

fn a_change(id: &'static str, change_type: ChangeType) -> ChangeRecord<'static> {
    ChangeRecord {
        epoch_id: "epoch:e",
        change_event_id: id,
        name: "the doc comment claimed the schema allows any target; it has been an enumeration since 2026-08-08",
        change_type,
        subject: None,
        target_type: "Artifact",
        target_id: "art:thing",
        action: ChangeAction::Modified,
    }
}

#[test]
fn the_ledger_can_say_a_change_was_documentation_only() {
    let mut g = graph_with_a_described_thing();
    g.record_change(a_change("chg:the-comment-lied", ChangeType::Documentation))
        .expect("a doc-only correction must be expressible without picking a least-wrong label");
}

/// The schema string and the Rust variant are one vocabulary, not two.
/// `as_str` is an exhaustive match, so a variant added without its string fails
/// to compile — but a variant added with the WRONG string compiles fine and
/// writes a value into the ledger that the schema will not accept back.
#[test]
fn the_variant_and_the_schema_agree_on_the_word() {
    assert_eq!(ChangeType::Documentation.as_str(), "documentation");

    // Serde is the other half: the MCP surface parses the caller's string
    // through this derive, so the wire word and `as_str` must be the same word.
    let parsed: ChangeType =
        serde_json::from_str("\"documentation\"").expect("the wire word must deserialise");
    assert_eq!(parsed, ChangeType::Documentation);

    let schema = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/temporal.yaml"
    ))
    .expect("read temporal.yaml");
    assert!(
        schema.contains("refactor, documentation, scope_change"),
        "temporal.yaml's change_type enum must carry the same word the Rust variant serialises to"
    );
}

/// COUNTERWEIGHT. The boundary is BEHAVIOURAL, not file-shaped, and this value
/// is only worth having if the neighbours it was carved out of still mean what
/// they meant. A structural change with no behaviour change is still a
/// `refactor`; a repair to code that disagreed with accepted intent is still a
/// `defect_fix`. And the enum must still REFUSE a near-miss — `docs` is not the
/// word, and a vocabulary that accepts anything doc-shaped has widened into the
/// catch-all this variant exists to prevent.
#[test]
fn the_labels_it_was_carved_out_of_still_mean_what_they_meant() {
    let mut g = graph_with_a_described_thing();
    for (id, ct) in [
        ("chg:moved-a-function", ChangeType::Refactor),
        ("chg:code-disagreed-with-intent", ChangeType::DefectFix),
    ] {
        g.record_change(a_change(id, ct)).unwrap_or_else(|e| {
            panic!("{} was legal before and must stay legal: {e}", ct.as_str())
        });
    }

    assert!(
        serde_json::from_str::<ChangeType>("\"docs\"").is_err(),
        "`docs` is a near-miss, not the word; admitting Documentation must not admit everything"
    );
}
