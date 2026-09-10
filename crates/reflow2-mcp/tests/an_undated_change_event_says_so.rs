//! A ChangeEvent written without `detected_at` says so in the reply.
//!
//! F-06, hxm_program: *"Nine events written in one day, none dated in the field
//! (dates are in the prose). The verification digest then cannot order changes
//! against `last_run_at`. Suggest: report an undated ChangeEvent the way an
//! undated sweep is reported — as undated, not silently."*
//!
//! ⭐ THE POINT IS THE ASYMMETRY, not the missing field. This project's standing
//! posture is that an absent property means NOBODY SAID and is REPORTED rather
//! than defaulted — `invalidates` returns `rerun_owed: null` instead of false
//! when a date is missing, `repair_report` counts `unstated` as a first-class
//! number, and `detect_defects` says what it could not have found. A ChangeEvent
//! is the one place a date silently does not arrive, and the cost is downstream:
//! `changelog_view` places entries by epoch window and the verification digest
//! orders changes against `last_run_at`, both of which need the date.
//!
//! It WARNS and never refuses. The event is already written by the time this is
//! computed, and a date nobody recorded is a true state of the record — the
//! requirement is that it be visible, never that it be forbidden.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn write_event(props: Value) -> Value {
    let s = ReflowService::in_memory().expect("service");
    s.add_change_event(Parameters(serde_json::from_value(props).unwrap()))
        .await
        .expect("add_change_event")
        .structured_content
        .expect("structured")
}

/// THE CASE. No `detected_at`, so the reply says the event carries no date and
/// names what cannot be computed without one.
#[tokio::test]
async fn an_event_with_no_date_is_reported_as_undated() {
    let v = write_event(json!({
        "id": "chg:thing",
        "name": "The thing moved",
        "change_type": "new_feature",
    }))
    .await;

    let note = v
        .get("undated")
        .unwrap_or_else(|| panic!("no `undated` block in: {v}"));
    let text = note.as_str().unwrap_or_else(|| {
        note.get("note")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("undated block carries no text: {note}"))
    });
    assert!(
        text.contains("detected_at"),
        "the note must name the field that would fix it: {text}"
    );
    // It must say what the absence COSTS, not merely that it happened — a bare
    // "no date" is the silent behaviour with extra words.
    let lowered = text.to_lowercase();
    assert!(
        lowered.contains("order") || lowered.contains("window") || lowered.contains("changelog"),
        "the note must say what cannot be computed without a date: {text}"
    );
    // And it must not read as a refusal: the event is written.
    assert_eq!(v["event"]["node_id"], "chg:thing");
}

/// SILENT when the caller dated it. A block on every call is the noise this
/// family is built not to become.
#[tokio::test]
async fn an_event_that_carries_its_date_is_silent() {
    let v = write_event(json!({
        "id": "chg:dated",
        "name": "The thing moved",
        "change_type": "new_feature",
        "detected_at": "2026-09-10",
    }))
    .await;
    assert!(
        v.get("undated").is_none(),
        "a dated event has nothing to report: {v}"
    );
    assert_eq!(v["event"]["properties"]["detected_at"], "2026-09-10");
}
