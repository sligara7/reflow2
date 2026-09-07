//! The two fields a user reported missing can be set by the tools that make
//! the nodes.
//!
//! # What was reported, and what it actually was
//!
//! The dev_storyflow agent, 2026-09-07: *"`add_epoch` lost `description` (it
//! had one before?) — the ordering and the must-nots had to go into the name,
//! which is now a paragraph. `add_requirement` lost `priority`. Both fine, both
//! cost a round-trip."*
//!
//! Nothing was lost. `git log -S` over the request structs shows neither
//! parameter has ever existed. Both properties were declared in the schema and
//! never offered by a typed tool, which is a different defect from a regression
//! and has a different fix — theirs.
//!
//! `DesignEpoch.description` is that type's EMBEDDING FIELD, so the field
//! `search_design` finds an epoch BY could not be written by the tool that
//! makes one, and the reporter's workaround was to put a paragraph in `name`.
//! `Requirement.priority` declares `default: medium`, so every requirement
//! carried a priority nobody chose and nobody could change through the surface.
//!
//! Both were invisible to the reachability instrument until the same day, for
//! reasons recorded in
//! `fact:defect-a-declared-property-can-be-unreachable-and-invisible-to-the-reach-instrument-when-a-default-populates-it`.
//! Making the class visible was the previous change; this is the one that
//! answers the report.
//!
//! # What is pinned
//!
//! That each parameter reaches the STORED PROPERTY — not merely that the call
//! is accepted. A tool that takes a field and drops it passes a signature test
//! and fails the user, which is the shape of the defect this pair belongs to.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// The epoch's embedding field is writable by the tool that makes an epoch.
#[tokio::test]
async fn add_epoch_takes_the_description_that_search_finds_it_by() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let out = j!(s.add_epoch(Parameters(
        serde_json::from_value(json!({
            "id": "epoch:rain-baseline",
            "name": "Rainfall baseline",
            "epoch_type": "baseline",
            "sequence": 10,
            "description": "The state of the gauge design before the retry path changed."
        }))
        .unwrap()
    )));
    let props = out
        .get("epoch")
        .and_then(|e| e.get("properties"))
        .or_else(|| out.get("properties"))
        .expect("the epoch's properties come back");
    assert_eq!(
        props["description"], "The state of the gauge design before the retry path changed.",
        "the description must reach the stored property — a tool that accepts a field and \
         drops it passes a signature check and still fails the caller"
    );
    assert_eq!(
        props["name"], "Rainfall baseline",
        "and the name must stay a short handle rather than absorbing the prose"
    );
}

/// A requirement's priority is stated by the caller rather than defaulted.
#[tokio::test]
async fn add_requirement_takes_the_priority_the_caller_means() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let out = j!(s.add_requirement(Parameters(
        serde_json::from_value(json!({
            "id": "req:rain-total",
            "name": "Rainfall totals survive a dropped packet",
            "statement": "A lost reading must not lose the running rainfall total.",
            "priority": "critical"
        }))
        .unwrap()
    )));
    let props = out
        .get("requirement")
        .and_then(|r| r.get("properties"))
        .or_else(|| out.get("properties"))
        .expect("the requirement's properties come back");
    assert_eq!(
        props["priority"], "critical",
        "the stated priority must reach the stored property; before this it was unreachable \
         and every requirement sat at the injected default"
    );
}

/// Omitting the priority still leaves the schema default in place: adding the
/// parameter must not turn an unstated value into a refusal.
#[tokio::test]
async fn omitting_the_priority_changes_nothing_about_what_lands() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let out = j!(s.add_requirement(Parameters(
        serde_json::from_value(json!({
            "id": "req:rain-quiet",
            "name": "The gauge reports quietly",
            "statement": "The gauge must not chatter when nothing has changed."
        }))
        .unwrap()
    )));
    let props = out
        .get("requirement")
        .and_then(|r| r.get("properties"))
        .or_else(|| out.get("properties"))
        .expect("the requirement's properties come back");
    assert_eq!(
        props["priority"], "medium",
        "the schema default must still apply when the caller says nothing"
    );
}

/// A value outside the declared set is refused rather than stored, and the
/// refusal says what is legal.
#[tokio::test]
async fn a_priority_outside_the_declared_set_is_refused() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let err = s
        .add_requirement(Parameters(
            serde_json::from_value(json!({
                "id": "req:rain-bogus",
                "name": "Bogus",
                "statement": "x",
                "priority": "URGENT!!"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("an undeclared priority must be refused");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("critical") || msg.contains("medium"),
        "the refusal must name the legal values: {msg}"
    );
}
