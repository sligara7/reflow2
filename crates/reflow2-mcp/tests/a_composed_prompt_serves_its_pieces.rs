//! A composed prompt serves the pieces it was composed from, and a gap that has
//! closed refuses in a way a consumer can branch on.
//!
//! flo2 F18 and F22, 2026-09-19, under the black-box rule the owner stated while
//! cutting v0.65.0: a consumer should never have to read reflow2's prose to work.
//!
//! F18: `gap_to_prompt` builds its prompt as
//! `format!("… Gap: {title}\nWhy it matters: {description}")` and returned only
//! the joined string, so flo2 — which wants the CONTEXT folded into a model turn
//! it is already paying for, not a second turn buying a phrased question — split
//! that string on the literal `"\nWhy it matters:"`. That marker is prose in no
//! schema and rewording the template is legitimate for any release. It was
//! flo2's single declared breach of the rule. The server held both halves and
//! discarded the seam in the same statement that made it.
//!
//! F22: the refusal for a gap that has closed is good prose and nothing else, so
//! telling it apart meant matching that prose. flo2 instead re-ran `detect_gaps`
//! before every send — a whole graph computation per click — purely to know
//! whether a chip was stale. The refusal now carries `reason: "gap_closed"`.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// A design with one capability nothing realizes, which raises a gap.
async fn design_with_a_gap() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(
        serde_json::from_value::<ProjectReq>(json!({"id": "proj:g", "name": "Greenhouse"}))
            .unwrap()
    )));
    j!(s.add_capability(Parameters(
        serde_json::from_value::<CapabilityReq>(json!({
            "id": "cap:clarify-water",
            "name": "Clarify the water",
            "description": "Ultraviolet clarifier on the return line."
        }))
        .unwrap()
    )));
    s
}

async fn first_gap(s: &ReflowService) -> Value {
    let v: Value = j!(s.detect_gaps(Parameters(
        serde_json::from_value(json!({})).expect("no arguments")
    )));
    v["gaps"]
        .as_array()
        .or_else(|| v["items"].as_array())
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| panic!("the design raises no gap to ask about: {v}"))
}

#[tokio::test]
async fn the_first_reply_carries_the_pieces_the_prompt_was_composed_from() {
    let s = design_with_a_gap().await;
    let gap = first_gap(&s).await;
    let v: Value = j!(s.gap_to_prompt(Parameters(
        serde_json::from_value(json!({"gap": gap})).unwrap()
    )));
    assert_eq!(v["status"], "needs_llm", "{v}");

    let pieces = &v["gap"];
    assert!(
        pieces.is_object(),
        "the reply must carry the gap's own pieces, not only the composed prompt: {v}"
    );
    for field in ["id", "title", "why"] {
        assert!(
            pieces[field].as_str().is_some_and(|s| !s.is_empty()),
            "`gap.{field}` must be served: {pieces}"
        );
    }

    // THE POINT: the pieces are the values the prompt was built from, so a
    // consumer never has to recover them by splitting the prose.
    let composed = v["prompts"]
        .as_array()
        .and_then(|a| a.first())
        .map(|p| p["prompt"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    let title = pieces["title"].as_str().unwrap();
    let why = pieces["why"].as_str().unwrap();
    assert!(
        composed.contains(title),
        "the served title is not the one in the prompt:\n  title: {title}\n  prompt: {composed}"
    );
    assert!(
        composed.contains(why),
        "the served why is not the one in the prompt:\n  why: {why}\n  prompt: {composed}"
    );
    assert!(
        composed.contains("Why it matters:"),
        "this test is worthless if the marker it replaces is gone: {composed}"
    );
}

#[tokio::test]
async fn a_gap_that_has_closed_refuses_with_a_reason_a_consumer_can_branch_on() {
    let s = design_with_a_gap().await;
    let mut gap = first_gap(&s).await;

    // A STALE CHIP, which is the case flo2 actually meets: the consumer holds a
    // gap object from an earlier read and the gap is no longer open. Taking a
    // real gap and changing only its id reproduces that exactly, and keeps the
    // payload a valid GapCandidate so the refusal under test is the one about
    // the gap being gone rather than one about the shape.
    gap["id"] = json!("gap:closed-since-you-took-it");

    let err = s
        .gap_to_prompt(Parameters(
            serde_json::from_value(json!({"gap": gap})).unwrap(),
        ))
        .await
        .expect_err("a gap that is no longer open must be refused, not served from the echo");

    let data = err.data.clone().unwrap_or(Value::Null);
    assert_eq!(
        data["reason"], "gap_closed",
        "the refusal must say WHY in data a consumer can branch on, not only in prose: \
         message={:?} data={data}",
        err.message
    );
    assert_eq!(
        data["gap_id"], "gap:closed-since-you-took-it",
        "and it must name which gap: {data}"
    );
    assert!(
        err.message.contains("CLOSED or ACKNOWLEDGED"),
        "the prose explanation must survive beside the machine-readable reason: {}",
        err.message
    );
}
