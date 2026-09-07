//! Recording a dated finding is a TOOL CALL, and the tool demands the skill.
//!
//! # The failure this pins
//!
//! `rule:field-feedback-issues-are-root-caused-and-ideas-are-brainstormed`
//! requires the root-cause skill on every issue a field report names. On
//! 2026-09-07 the rule was broken by the agent that had recorded its own
//! enforcement ruling hours earlier, and the investigation into why is
//! `fact:the-root-cause-skill-is-demanded-by-no-tool-and-named-by-no-trigger-so-it-loads-only-by-luck`.
//!
//! The cause was not forgetfulness and not a missing rule. It was that
//! root-cause is the one skill in the set whose trigger is defined by the
//! agent's own internal state — "the moment you are about to write down a
//! cause" — so nothing observable could fire on it. Every skill the loop nudge
//! successfully names keys on something that happens: a tool called, a file
//! edited, a session started. There was no tool call for having formed an
//! explanation, because writing a dated finding had no constructor at all and
//! went through generic `create_node` with a props bag.
//!
//! Measured, from `tools/skill_lint.py`'s own comment, over 91 sessions of a
//! real project: between 19% and 24% of sessions calling a tool ever opened the
//! skill written for it, where nothing demanded it. That is the gap this closes.
//!
//! # What is pinned here, and why it is the CLASS
//!
//! Pinning "record_finding exists" would fix today and leave the class open.
//! What is pinned instead is the DEMAND CONTRACT — that the moment of recording
//! a cause is reachable as a tool call, and that the tool says the skill's name
//! at that moment. A later refactor may rename the tool; it may not quietly
//! drop the sentence that makes the skill reachable, which is the thing that
//! was missing and the thing that cost a wrong cause.
//!
//! `tools/skill_lint.py` holds the other half — root-cause declares
//! `demanded_by: [record_finding]`, and the lint fails if the golden is absent
//! or its description does not name the skill. Both were observed failing
//! before this landed.

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

async fn service_with_a_subject() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_capability(Parameters(
        serde_json::from_value(json!({
            "id": "cap:rain-sum",
            "name": "Cumulative rainfall",
            "description": "sends the running rainfall total rather than deltas"
        }))
        .unwrap()
    )));
    s
}

/// The whole point: the act of recording a cause is a tool call, and the tool
/// tells the caller to get the skill before writing one.
#[tokio::test]
async fn the_tool_that_records_a_cause_names_the_skill_that_governs_writing_one() {
    let tool = ReflowService::temporal_tools_router()
        .list_all()
        .into_iter()
        .find(|t| t.name == "record_finding")
        .expect(
            "record_finding must exist: the moment a cause is written has to be a tool call, \
             or nothing observable can trigger the skill",
        );
    let desc = tool.description.clone().unwrap_or_default().to_string();
    assert!(
        desc.contains("root-cause"),
        "record_finding's description must NAME the root-cause skill — a tool description is \
         the only thing an agent reliably reads at the moment of the work, and this sentence \
         is the trigger the skill otherwise has no way to get. Description was: {desc}"
    );
}

/// A finding about a node the design does not have is refused rather than
/// stored: a record about nothing is not a record.
#[tokio::test]
async fn a_finding_about_a_node_that_does_not_exist_is_refused() {
    let s = service_with_a_subject().await;
    let err = s
        .record_finding(Parameters(
            serde_json::from_value(json!({
                "id": "fact:about-nothing",
                "subject_id": "cap:no-such-capability",
                "statement": "the totals drift after a dropped packet"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("a finding whose subject does not resolve must be refused");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("cap:no-such-capability"),
        "the refusal must name the id that did not resolve: {msg}"
    );
}

/// Naming a cause without saying why is refused BEFORE anything is written —
/// an edge with no evidence is an assertion nobody can check or overturn.
#[tokio::test]
async fn a_cause_without_its_evidence_is_refused_and_nothing_is_written() {
    let s = service_with_a_subject().await;
    let err = s
        .record_finding(Parameters(
            serde_json::from_value(json!({
                "id": "fact:unevidenced",
                "subject_id": "cap:rain-sum",
                "statement": "the totals drift after a dropped packet",
                "caused_by": "cap:rain-sum"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("a cause with no evidence must be refused");
    assert!(
        format!("{err:?}").contains("cause_evidence"),
        "the refusal must name the field that was missing"
    );
    let got = j!(s.get_node(Parameters(
        serde_json::from_value(json!({"id": "fact:unevidenced"})).unwrap()
    )));
    assert!(
        got["node"].is_null(),
        "the refusal must happen before the write: nothing may be stored"
    );
}

/// The happy path: the finding lands, the subject carries it, and the cause
/// edge is drawn with its reason in the same call.
#[tokio::test]
async fn a_finding_lands_with_its_subject_and_its_cause() {
    let s = service_with_a_subject().await;
    let out = j!(s.record_finding(Parameters(
        serde_json::from_value(json!({
            "id": "fact:totals-drift-on-a-dropped-packet",
            "subject_id": "cap:rain-sum",
            "name": "Totals drift when a packet is dropped",
            "fact_type": "defect",
            "statement": "After a dropped reading the running total is short by that reading.",
            "valid_from": "2026-09-07",
            "caused_by": "cap:rain-sum",
            "cause_evidence": "The capability sends deltas on the retry path, so a dropped \
                               packet loses a reading that the running total never recovers."
        }))
        .unwrap()
    )));
    assert_eq!(
        out["finding"]["node_id"],
        "fact:totals-drift-on-a-dropped-packet"
    );
    assert_eq!(out["subject"]["node_id"], "cap:rain-sum");
    assert_eq!(out["subject"]["node_type"], "Capability");
    assert_eq!(out["caused_by"]["node_id"], "cap:rain-sum");
    // `basis` and `fact_type` are stated rather than left to a schema default,
    // so a reader can tell a recorded observation from a projection.
    let props = &out["finding"]["properties"];
    assert_eq!(props["fact_type"], "defect");
    assert_eq!(props["basis"], "measured");
}

/// The subject's type need not be given: it is resolved from the id by the
/// same convention every other read uses.
#[tokio::test]
async fn the_subject_type_is_resolved_from_the_id() {
    let s = service_with_a_subject().await;
    let out = j!(s.record_finding(Parameters(
        serde_json::from_value(json!({
            "id": "fact:resolved-without-a-type",
            "subject_id": "cap:rain-sum",
            "statement": "the caller did not have to know the subject's node type"
        }))
        .unwrap()
    )));
    assert_eq!(out["subject"]["node_type"], "Capability");
}
