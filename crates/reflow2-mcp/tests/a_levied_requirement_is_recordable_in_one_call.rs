//! A requirement levied by ANOTHER DESIGN is recordable through the documented
//! path, in one call.
//!
//! `dec:idea-gap-flows-to-the-dependency` states the mechanism in as many
//! words: a levied requirement "is expressible TODAY as: add_requirement
//! (lands proposed) + authored_by <them> role=author + source '<their design>'
//! + provenance imported. NOTHING IN THE SCHEMA NEEDS TO CHANGE."
//!
//! 🛑 THE SCHEMA DID NOT NEED TO CHANGE AND THE TOOL DID. Both properties are
//! declared on Requirement and `add_requirement` accepted NEITHER, so the only
//! way to finish the documented path was a second call to the generic
//! `create_node` escape hatch.
//!
//! MEASURED 2026-09-22, the day this was fixed, on reflow2's own design:
//!   · Requirements carrying `source`            1 of 263
//!   · Requirements with `provenance: imported`  1 of 263
//! Both were the SAME node — written by hand through `create_node` that
//! afternoon while following this design's own instructions, in the first live
//! exercise of the mechanism. 262 of 263 sat at the schema default.
//!
//! ⭐ THAT IS THE THREE-LEGS SHAPE WITH TWO LEGS: the vocabulary existed, the
//! instruction existed, and no typed tool could write it — so in the whole
//! history of this design the documented path had been walked exactly once,
//! and only by stepping outside it.
//!
//! OBSERVED FAILING before the fix: the request type had no such fields, so
//! this did not compile.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

#[tokio::test]
async fn the_documented_path_for_a_levied_requirement_needs_no_escape_hatch() {
    let s = ReflowService::in_memory().expect("in-memory service");

    let out = j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:they-need-this".into(),
        name: Some("Ordering is preserved across the seam".into()),
        statement: Some("it is preserved".into()),
        source: Some("their-design (graph 1a97fc1f) — ifc:the-boundary".into()),
        provenance: Some("imported".into()),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
        kind: None,
    })));

    let props = &out["properties"];
    assert_eq!(
        props["provenance"], "imported",
        "a levied requirement is not this design's own intent, and `provenance` is the field \
         that says so. Until 2026-09-22 the constructor could not write it, so 262 of 263 \
         requirements sat at the `authored` default whether or not it was true"
    );
    assert!(
        props["source"]
            .as_str()
            .is_some_and(|v| v.contains("their-design")),
        "and `source` is what names WHO levied it. One call, no escape hatch — which is what \
         the mechanism already said, three weeks before the tool could do it"
    );
    assert_eq!(
        props["status"], "proposed",
        "unchanged and load-bearing: a consumer may ask for anything and settle nothing, so a \
         levied requirement lands at `proposed` whatever else it carries"
    );
}

/// Unchanged behaviour: omitting them leaves the schema default alone. These
/// parameters add a way to SAY, never an obligation to — the same contract
/// `priority` already carries on this constructor.
#[tokio::test]
async fn omitting_them_leaves_the_default_exactly_as_it_was() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let out = j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:our-own".into(),
        name: Some("Something we want".into()),
        statement: Some("we want it".into()),
        source: None,
        provenance: None,
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
        kind: None,
    })));
    assert_eq!(out["properties"]["provenance"], "authored");
    assert!(out["properties"].get("source").is_none_or(|v| v.is_null()));
}
