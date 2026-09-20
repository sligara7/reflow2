//! A revising write says what it removed.
//!
//! flo2, 2026-09-19, measured on their own session: thirteen full-field
//! rewrites, **two of which silently dropped a load-bearing paragraph**. One
//! lost a clause bounding when a rule binds; one lost the paragraph explaining
//! that a requirement pulls against another, and it had to be restored in a
//! second pass. Both were caught by the author re-reading their own call.
//! Nothing reflow2 said caught either.
//!
//! Their framing is why this is not a nicety: every other entry in that report
//! costs a retry or a confusing message, and this one "degrades the record
//! permanently unless a human re-reads a four-thousand-character field
//! character by character, which nobody does twice."
//!
//! A REPORT, NOT A GATE. Refusing a shrink was rejected: deleting on purpose is
//! a legitimate edit and a permission flag would tax every honest one.

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

const LONG: &str = "The outdoor unit sends cumulative totals rather than deltas, so a lost \
                    reading heals itself on the next one. UNTIL v0.1.0 EXISTS, this rule does \
                    not bind. It pulls against the requirement that a reading is cheap to \
                    send, and that tension is deliberate.";

async fn svc() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(
        serde_json::from_value::<ProjectReq>(json!({"id": "proj:p", "name": "P"})).unwrap()
    )));
    s
}

async fn write_statement(s: &ReflowService, text: &str) -> Value {
    j!(s.add_requirement(Parameters(
        serde_json::from_value::<RequirementReq>(json!({
            "id": "req:readings",
            "name": "Readings heal themselves",
            "statement": text
        }))
        .unwrap()
    )))
}

#[tokio::test]
async fn a_rewrite_that_drops_a_paragraph_says_so_with_the_size_of_the_loss() {
    let s = svc().await;
    write_statement(&s, LONG).await;

    // The rewrite flo2 kept making: resend the field to change one part, and
    // lose an unrelated sentence on the way.
    let shorter = LONG.replace(" UNTIL v0.1.0 EXISTS, this rule does not bind.", "");
    assert!(
        shorter.len() < LONG.len(),
        "the fixture must actually shrink"
    );
    let v = write_statement(&s, &shorter).await;

    let shortened = &v["revision"]["shortened"];
    let entry = shortened
        .as_array()
        .and_then(|a| a.first())
        .unwrap_or_else(|| panic!("the reply must say a field got shorter: {v}"));
    assert_eq!(entry["field"], "statement", "{entry}");
    let removed = entry["removed_chars"].as_u64().expect("a count");
    assert_eq!(
        removed,
        (LONG.chars().count() - shorter.chars().count()) as u64,
        "the count must be the real loss: {entry}"
    );
    assert!(
        entry["before_chars"].as_u64() > entry["after_chars"].as_u64(),
        "{entry}"
    );

    // And the note must lead with it, because the rest of that note explains
    // where the prior state lives, not what left.
    let note = v["revision"]["note"].as_str().unwrap_or_default();
    assert!(
        note.contains("REMOVED") && note.contains(&removed.to_string()),
        "the note must say what was removed and how much: {note}"
    );

    // The prior value is still there to put back — that is what makes the
    // loss recoverable rather than merely visible.
    let replaced = v["revision"]["replaced"]
        .as_array()
        .and_then(|a| a.iter().find(|r| r["field"] == "statement"))
        .unwrap_or_else(|| panic!("the prior value must still be returned: {v}"));
    assert!(
        replaced["prior"]
            .as_str()
            .unwrap_or_default()
            .contains("UNTIL v0.1.0"),
        "{replaced}"
    );
}

#[tokio::test]
async fn a_write_that_adds_text_says_nothing_about_shortening() {
    let s = svc().await;
    write_statement(&s, LONG).await;
    let v = write_statement(&s, &format!("{LONG} And a new sentence.")).await;
    assert!(
        v["revision"]["shortened"].is_null(),
        "an enrichment must not be reported as a loss: {}",
        v["revision"]
    );
    let note = v["revision"]["note"].as_str().unwrap_or_default();
    assert!(
        !note.contains("REMOVED"),
        "the note must stay quiet when nothing left: {note}"
    );
}

#[tokio::test]
async fn the_shrink_is_reported_and_never_refused() {
    let s = svc().await;
    write_statement(&s, LONG).await;
    // A drastic shortening still SUCCEEDS: deleting on purpose is legitimate,
    // and the reply is where the caller learns what happened.
    let v = write_statement(&s, "Short.").await;
    assert_eq!(v["node_id"], "req:readings", "the write must land: {v}");
    assert!(
        v["revision"]["shortened"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "and it must still be reported: {}",
        v["revision"]
    );
}
