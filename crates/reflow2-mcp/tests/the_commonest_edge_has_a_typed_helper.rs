//! Component coupling gets a typed helper, and the name it needs is freed.
//!
//! # The report
//!
//! musicjug, 2026-08-31 (`art:musicjug-genesis-feedback-2026-08-31`, "The bad" §4):
//!
//! > I searched for a way to record that one component depends on another.
//! > `declare_dependency` matched by name and turned out to be cross-*design*
//! > version pinning — a completely different concept. I fell back to raw
//! > `create_edge` with `DEPENDS_ON`.
//!
//! Component `DEPENDS_ON` feeds cycle detection, single-point-of-failure
//! analysis and the seam gap. Nine typed helpers exist — `contains`,
//! `contain_component`, `satisfies`, `allocate`, `realizes`, `provides`,
//! `consumes`, `governed_by`, `decomposes` — and the single most common
//! structural edge in any design was not among them.
//!
//! ⭐ TWO DEFECTS THAT COMPOUND, which is why this file pins two things. The
//! ABSENCE alone costs a raw `create_edge`. The NAME COLLISION actively routes
//! a searching agent to the wrong tool first — and `find_tools` ranks by name,
//! so the misdirection is systematic rather than unlucky.
//!
//! # Why the rename happens now rather than on a deprecation path
//!
//! Anthony, 2026-09-01: *"I think we should change the declare_dependency -->
//! external_dependency."* This is a BREAKING CHANGE to a served surface, taken
//! deliberately under `rule:fix-it-properly-while-it-is-still-cheap`, whose
//! second clause makes "it would break consumers" a reason to do it NOW rather
//! than later — the deprecation discipline begins at 1.0, and reflow2 is not
//! there.
//!
//! 📌 THE CORE METHOD IS RENAMED TOO, not just the served tool:
//! `DesignGraph::declare_dependency` becomes `declare_external_dependency`.
//! Leaving the old name in core behind a tool called `external_dependency`
//! would create exactly the helper-vs-core naming disagreement that
//! `fact:defect-typed-tool-parameter-names-are-inconsistent` has already
//! counted eleven of. The core keeps its verb because it reads as Rust; the
//! served tool drops it because it reads as a noun beside `depends_on`,
//! `allocate` and `satisfies`.
//!
//! # How the rename is observed
//!
//! Through `report_manual_work`, which asks the LIVE ROUTER whether reflow2
//! serves a tool by a given name (`a_by_hand_report_names_a_real_tool.rs`).
//! That is the only behavioural way to ask the question from outside the crate,
//! and it is better than a hand-kept list for the same reason that check exists:
//! a second copy of the tool list maintained by hand is the defect class it was
//! built to stop.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({ "id": "proj:p", "name": "P" }))
            .expect("project request"),
    ))
    .await
    .expect("project");
    s
}

fn names_tool(tool: &str) -> ReportManualWorkReq {
    serde_json::from_value(serde_json::json!({
        "what": "recorded a component coupling by hand",
        "diagnosis": "tool_not_found",
        "reflow2_tool": tool,
        "at": "2026-09-01",
    }))
    .expect("request")
}

async fn component(s: &ReflowService, id: &str, name: &str) {
    s.add_component(Parameters(ComponentReq {
        id: id.into(),
        name: Some(name.into()),
        description: Some(format!("The {name} part, for a coupling fixture.")),
        level: None,
        distinct_from: None,
    }))
    .await
    .expect("component lands");
}

/// ⭐ THE REPORTED CASE. One call, two components, a real `DEPENDS_ON`.
#[tokio::test]
async fn a_component_coupling_has_one_typed_call() {
    let s = svc().await;
    component(&s, "cmp:coach", "coach").await;
    component(&s, "cmp:stage", "stage").await;

    let out = s
        .depends_on(Parameters(EdgePairReq {
            from_id: "cmp:coach".into(),
            to_id: "cmp:stage".into(),
        }))
        .await
        .expect("a component coupling is one typed call, not a raw create_edge");

    let body = serde_json::to_string(&out.structured_content.expect("structured content"))
        .expect("serialisable");
    assert!(
        body.contains("DEPENDS_ON"),
        "the helper must draw the edge the topology rules actually read: {body}"
    );
    assert!(
        body.contains("cmp:coach") && body.contains("cmp:stage"),
        "and it must name both ends: {body}"
    );
}

/// 🛑 BOTH ENDS MUST EXIST. The typed helpers fill in endpoint TYPES; they do
/// not relax the rule that an edge to a node which is not there is refused.
/// Without this, the convenience would buy a new way to point a dependency into
/// nothing — and a dangling `DEPENDS_ON` is invisible to every rollup that
/// walks the golden thread.
#[tokio::test]
async fn a_coupling_to_a_component_that_does_not_exist_is_refused() {
    let s = svc().await;
    component(&s, "cmp:coach", "coach").await;

    let err = s
        .depends_on(Parameters(EdgePairReq {
            from_id: "cmp:coach".into(),
            to_id: "cmp:nobody-made-this".into(),
        }))
        .await
        .expect_err("an edge to an absent node is refused through every typed helper");

    assert!(
        err.message.contains("cmp:nobody-made-this"),
        "the refusal must name the end that is missing: {}",
        err.message
    );
}

/// The rename, one half: the freed name is served.
#[tokio::test]
async fn the_cross_design_pin_is_served_as_external_dependency() {
    let s = svc().await;

    let out = s
        .report_manual_work(Parameters(names_tool("external_dependency")))
        .await;

    assert!(
        out.is_ok(),
        "`external_dependency` must be a tool reflow2 actually serves: {:?}",
        out.err()
    );
}

/// 🛑 The rename, other half — and the counterweight that makes the first half
/// mean something. If the old name survived alongside the new one, the
/// collision that caused the report would still be there and the test above
/// would still pass.
#[tokio::test]
async fn the_old_colliding_name_is_gone() {
    let s = svc().await;

    let out = s
        .report_manual_work(Parameters(names_tool("declare_dependency")))
        .await;

    assert!(
        out.is_err(),
        "`declare_dependency` must no longer be served — leaving it would keep the collision \
         that sent a searching agent to the wrong tool"
    );
}
