//! `Interface.medium` has a value for a contract between institutions.
//!
//! Reported by the US-government genesis session on 2026-08-15: `medium:
//! "process"` was refused, and seventeen check-and-balance boundaries — a
//! presentment-and-veto procedure among them — were filed under `human` for
//! want of a word. `medium` is READ, not merely stored (`library`/`data` earn
//! the single-point-of-failure exemption; `seam_report` compares it across
//! paired designs), so a procedural boundary labelled `human` is being reasoned
//! about as a human-factors touchpoint by every computation that reads the
//! field (`fact:the-medium-enum-has-no-word-for-an-institutional-or-legal-contract`).
//!
//! One value, `procedural`, on the owner's word 2026-09-15: a rule-governed
//! exchange between institutions or roles, where the contract is a procedure
//! rather than a protocol or a touchpoint. Enum values do not move the schema
//! stamp, so this is additive.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

#[test]
fn a_procedural_medium_is_accepted() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:gov", "A government").unwrap();
    let stored = g
        .create_node(
            node::INTERFACE,
            "ifc:presentment-and-veto",
            Props::new()
                .set("name", "Presentment and veto")
                .set("medium", "procedural"),
        )
        .expect("a constitutional procedure is a boundary the schema can name");
    assert_eq!(
        stored.properties.get("medium").and_then(|v| v.as_str()),
        Some("procedural")
    );
}

#[test]
fn the_word_the_reporter_reached_for_is_still_refused_and_names_the_right_one() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:gov", "A government").unwrap();
    let err = g
        .create_node(
            node::INTERFACE,
            "ifc:veto",
            Props::new().set("name", "Veto").set("medium", "process"),
        )
        .expect_err("`process` was never a value; the refusal is what makes the fix one retry");
    let text = err.to_string();
    assert!(
        text.contains("procedural"),
        "the refusal must list the value that would have worked: {text}"
    );
}
