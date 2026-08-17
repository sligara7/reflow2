//! `loop_status` stays cheap to READ, not only cheap to CALL — and says when
//! it shortened something.
//!
//! # Why this exists
//!
//! dev_storyflow, 2026-08-08: `loop_status` is documented as "one cheap call"
//! and is cheap to call and expensive to read —
//! `verifications.attention[0].name` came back as a single ~450-word paragraph
//! holding a full graded walk report. Their diagnosis is the requirement:
//! *"workers write the full report into the `name` because there is no other
//! durable place for it on the node — a missing field being paid for in every
//! future read of that node."*
//!
//! Measured on reflow2's own graph the same week: 164 Verifications, median
//! `name` 76 words, longest 654.
//!
//! # The half that matters most here
//!
//! **The truncation announces itself.** A silently shortened name reads as
//! "that is the whole name", which is the same defect one layer over as a
//! vacuous zero reading as a pass — and it is the sixth of the user's own
//! engineering principles, *no silent caps or truncation*. So `name_truncated`,
//! `name_words` and a top-level `names_truncated` note are the point, not
//! decoration.
//!
//! `req:a-finding-has-somewhere-to-put-its-evidence`.

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

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

/// A long name is cut short in the rollup, and the reply SAYS it was cut.
#[tokio::test]
async fn a_long_name_is_shortened_and_the_shortening_is_announced() {
    let s = svc().await;
    let long_name = (0..120)
        .map(|i| format!("word{i}"))
        .collect::<Vec<_>>()
        .join(" ");

    s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "ver:verbose", "name": long_name,
        }))
        .unwrap(),
    ))
    .await
    .expect("verification");

    // `planned` keeps it in `attention`, which is the roll that was unreadable.
    let status = j!(s.loop_status(Parameters(
        serde_json::from_value(serde_json::json!({})).unwrap()
    )));
    let att = &status["verifications"]["attention"];
    let item = att
        .as_array()
        .and_then(|a| a.iter().find(|v| v["verification_id"] == "ver:verbose"))
        .expect("the check needing attention is listed");

    assert_eq!(
        item["name_truncated"], true,
        "a shortened name must say it was shortened: {item}"
    );
    assert_eq!(
        item["name_words"], 120,
        "…and say how long the real one is: {item}"
    );
    let shown = item["name"].as_str().unwrap();
    assert!(
        shown.split_whitespace().count() < 40,
        "the rollup must actually be shorter: {} words",
        shown.split_whitespace().count()
    );
    assert!(
        status["verifications"]["names_truncated"]
            .as_str()
            .is_some_and(|n| n.contains("CUT SHORT")),
        "and the rollup must say so at the top level too: {status}"
    );
}

/// The counterweight, and without it the code could truncate everything and
/// still pass: a short name is left exactly alone and claims no truncation.
#[tokio::test]
async fn a_short_name_is_untouched_and_claims_no_truncation() {
    let s = svc().await;
    s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "ver:terse", "name": "the schema merges",
        }))
        .unwrap(),
    ))
    .await
    .expect("verification");

    let status = j!(s.loop_status(Parameters(
        serde_json::from_value(serde_json::json!({})).unwrap()
    )));
    let item = status["verifications"]["attention"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["verification_id"] == "ver:terse"))
        .expect("listed");

    assert_eq!(item["name"], "the schema merges");
    assert!(
        item.get("name_truncated").is_none(),
        "a name that was not cut must not carry the flag: {item}"
    );
    assert!(
        status["verifications"].get("names_truncated").is_none(),
        "and the rollup must not claim a truncation that did not happen"
    );
}

/// The evidence has somewhere to go at the TOOL, which is the half that makes
/// the truncation fair: shortening the name is only reasonable if there is
/// another place to write.
#[tokio::test]
async fn description_and_findings_are_reachable_from_the_tools() {
    let s = svc().await;
    s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "ver:paired",
            "name": "a short label",
            "description": "what this check actually is, at whatever length it needs",
        }))
        .unwrap(),
    ))
    .await
    .expect("verification");

    s.set_verification_status(Parameters(
        serde_json::from_value(serde_json::json!({
            "verification_id": "ver:paired",
            "status": "passing",
            "last_run_at": "2026-08-17",
            "findings": "42 cases, 0 failures, mutation-checked two ways",
        }))
        .unwrap(),
    ))
    .await
    .expect("status");

    let node = j!(s.get_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type": "Verification", "id": "ver:paired"
        }))
        .unwrap()
    )));
    let p = &node["node"]["properties"];
    assert_eq!(p["name"], "a short label");
    assert_eq!(
        p["description"], "what this check actually is, at whatever length it needs",
        "the constructor must be able to reach `description` — it was declared and unreachable"
    );
    assert_eq!(
        p["findings"], "42 cases, 0 failures, mutation-checked two ways",
        "and a run must be able to record what it found"
    );
}
