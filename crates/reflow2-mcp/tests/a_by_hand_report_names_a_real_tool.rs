//! A by-hand report may only name a tool reflow2 actually serves.
//!
//! `req:a-session-says-what-it-did-by-hand-that-reflow2-already-serves`.
//!
//! # Why this test is HERE and not in the core suite
//!
//! The check asks "does reflow2 serve a tool by this name?", and the CORE does
//! not know what the surface serves. Answering it there would have meant a
//! second copy of the tool list maintained by hand — the defect class this
//! project spent 2026-08-26 fixing three times over (`unallocated_component`
//! and the parking ruling; F-01's replay contract; three detectors not reading
//! `proposed`). At the tool, `tool_router.has_route()` asks the LIVE ROUTER,
//! so there is nothing to maintain and nothing that can drift.
//!
//! # Why the check matters at all
//!
//! `tool_not_found` asserts the tool EXISTS and the session missed it — a
//! DISCOVERABILITY failure, whose repair is to surface the tool. `tool_missing`
//! asserts nothing does it — whose repair is to build one. Those are different
//! pieces of work, and `dec:bl-155`'s central finding is that reflow2 otherwise
//! **cannot tell unused from unreachable**. A report naming a tool that does not
//! exist quietly claims the first while meaning the second, and would sit in the
//! one table somebody reads to decide what to improve.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

fn req(tool: Option<&str>) -> ReportManualWorkReq {
    serde_json::from_value(serde_json::json!({
        "what": "compared two layers of one design with a hand-written script",
        "diagnosis": if tool.is_some() { "tool_not_found" } else { "tool_missing" },
        "reflow2_tool": tool,
        "at": "2026-08-26",
    }))
    .expect("request")
}

#[tokio::test]
async fn a_report_naming_a_tool_that_is_not_served_is_refused() {
    let s = ReflowService::in_memory().expect("service");

    let out = s
        .report_manual_work(Parameters(req(Some("compare_the_layers"))))
        .await;

    assert!(
        out.is_err(),
        "a phantom tool name must be refused, not stored — it claims a \
         discoverability failure while meaning a missing feature"
    );
}

#[tokio::test]
async fn a_report_naming_a_real_tool_is_kept() {
    // The counterweight. If the check were keyed on a stale hand-written list,
    // or were simply always-refuse, this is what would say so.
    let s = ReflowService::in_memory().expect("service");
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({ "id": "prj:p", "name": "P" })).unwrap(),
    ))
    .await
    .expect("project");

    let out = s
        .report_manual_work(Parameters(req(Some("search_design"))))
        .await;

    assert!(
        out.is_ok(),
        "`search_design` IS served, so naming it must be accepted: {:?}",
        out.err()
    );
}

#[tokio::test]
async fn a_report_with_no_tool_named_is_kept() {
    // `tool_missing` is the commonest case and names nothing by design — the
    // whole point is that no tool does the work. It must not require one.
    let s = ReflowService::in_memory().expect("service");
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({ "id": "prj:p", "name": "P" })).unwrap(),
    ))
    .await
    .expect("project");

    let out = s.report_manual_work(Parameters(req(None))).await;

    assert!(
        out.is_ok(),
        "a `tool_missing` report names no tool and must still be accepted: {:?}",
        out.err()
    );
}
