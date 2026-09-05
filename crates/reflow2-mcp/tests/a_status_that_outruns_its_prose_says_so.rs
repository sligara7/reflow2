//! A status can advance past the prose in its own node, and the reply says so.
//!
//! THE FIELD REPORT, dev_storyflow 2026-09-02: they wrote a capability whose
//! description said in capitals "THE DROPLET STILL RUNS THE OLD SCRIPT ...
//! tonight's cron will capture the wrong volume again", installed the fix
//! twenty minutes later, called `set_capability_status(realized)` — and the
//! description still said the droplet ran the old script. Status said
//! delivered, prose said not started, both in one node, nothing flagged it.
//! Recorded as `fact:defect-a-status-can-advance-past-its-own-prose-and-
//! nothing-says-so`; the fix is theirs, and it is one block in a reply the
//! caller is already reading rather than a detector that finds it later.
//!
//! The counterweights matter as much as the case: this fires on a real
//! divergence and must be SILENT otherwise, or it becomes the noise that gets
//! switched off in a week.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

async fn svc_with_capability(status: &str, description: Option<&str>) -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    let mut props = json!({
        "id": "cap:thing",
        "name": "A thing",
        "status": status,
    });
    if let Some(d) = description {
        props["description"] = json!(d);
    }
    s.add_capability(Parameters(serde_json::from_value(props).unwrap()))
        .await
        .expect("add_capability");
    s
}

async fn set_status(s: &ReflowService, to: &str) -> Value {
    s.set_capability_status(Parameters(
        serde_json::from_value(json!({"capability_id": "cap:thing", "status": to})).unwrap(),
    ))
    .await
    .expect("set_capability_status")
    .structured_content
    .expect("structured")
}

/// THE CASE. Prose written under `planned`, status moved to `realized`, and the
/// reply names the divergence and quotes the prose so it can be judged here.
#[tokio::test]
async fn a_status_that_moves_past_its_description_is_reported() {
    let s = svc_with_capability(
        "planned",
        Some("THE DROPLET STILL RUNS THE OLD SCRIPT and tonight's cron will capture the wrong volume again."),
    )
    .await;

    let v = set_status(&s, "realized").await;
    let pc = v
        .get("prose_currency")
        .unwrap_or_else(|| panic!("no prose_currency block in: {v}"));

    assert_eq!(pc["field"], "description");
    assert_eq!(pc["written_under_status"], "planned");
    assert_eq!(pc["status_now"], "realized");
    assert!(
        pc["excerpt"].as_str().unwrap().contains("OLD SCRIPT"),
        "the prose must be QUOTED so it can be judged in this reply rather than \
         in another call: {pc}"
    );
    let note = pc["note"].as_str().unwrap();
    assert!(
        note.contains("planned") && note.contains("realized"),
        "the note must name both statuses, since which one the prose was written \
         under is the whole fact: {note}"
    );
}

/// COUNTERWEIGHT 1, and the one that stops this becoming a different defect: a
/// status set to what it ALREADY WAS creates no divergence and must be silent.
#[tokio::test]
async fn re_setting_the_same_status_says_nothing() {
    let s = svc_with_capability("planned", Some("Some prose that describes the thing.")).await;
    let v = set_status(&s, "planned").await;
    assert!(
        v.get("prose_currency").is_none(),
        "no status moved, so no prose can have been outrun: {v}"
    );
}

/// COUNTERWEIGHT 2: a node with no prose has nothing that can have gone stale.
///
/// NOTE THE SHAPE OF THIS TEST, learned by writing it: a Capability CANNOT be
/// created without a `description` — the required-fields guard refuses it. So
/// "no prose" is reachable only as an EMPTY or whitespace-only one, which is
/// exactly the branch `currency_note` trims for. The counterweight is real; it
/// just is not the shape it first looked like.
#[tokio::test]
async fn a_node_with_no_prose_says_nothing() {
    let s = svc_with_capability("planned", Some("   ")).await;
    let v = set_status(&s, "realized").await;
    assert!(
        v.get("prose_currency").is_none(),
        "there is no description to have outrun: {v}"
    );
}

/// AND IT FIRES ON THE REVERSE DIRECTION TOO, which is the sibling half of the
/// same field report: they also swept `planned` capabilities and found six
/// whose descriptions assert they are BUILT. A status moving BACKWARDS leaves
/// prose written under the later status, and that is just as stale.
#[tokio::test]
async fn a_status_moving_backwards_is_reported_too() {
    let s = svc_with_capability("realized", Some("Built end-to-end and shipped.")).await;
    let v = set_status(&s, "planned").await;
    let pc = v
        .get("prose_currency")
        .unwrap_or_else(|| panic!("no prose_currency block in: {v}"));
    assert_eq!(pc["written_under_status"], "realized");
    assert_eq!(pc["status_now"], "planned");
}
