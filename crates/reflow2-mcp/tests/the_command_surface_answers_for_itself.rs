//! Every skill says what a person would type to reach it.
//!
//! # The report
//!
//! Anthony, 2026-09-01, asking why a count was 23. It was 25 — and the answer
//! took four measurements, because reflow2's own command surface lives in five
//! places (`fact:the-command-surface-has-five-copies-and-nothing-reconciles-them`).
//!
//! Two things had drifted, both found by measuring rather than reported:
//!
//! * Four commands existed only in this repo, so every OTHER project on the
//!   machine was missing them — `/root-cause` among them, whose whole argument
//!   is that it must be reachable AT THE MOMENT of the work.
//! * `COMMAND_ALIASES` was missing three of eleven entries, so
//!   `get_skill("what-is-this")` failed with a bare list of names. That is the
//!   precise 2026-08-19 report the table was written to fix; the repair did not
//!   cover its own class.
//!
//! ⭐ AND AN AGENT WITH FULL FILESYSTEM ACCESS UNDER-REPORTED THE SURFACE BY
//! 60%, listing 11 of 28 commands, because it matched command names against
//! skill names and eight of them do not match. That is the failure this file
//! exists to make impossible: the shortcut is now something the SERVER states,
//! not something a reader has to infer from two directories.
//!
//! # What is pinned
//!
//! `list_skills` carries a `shortcut` per skill, and every skill has one. A
//! skill nobody can type is a skill nobody finds, and until now that was true
//! of two of them with nothing to say so.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

fn skills(out: &serde_json::Value) -> &Vec<serde_json::Value> {
    out["skills"].as_array().expect("a skills list")
}

async fn listed() -> serde_json::Value {
    let s = ReflowService::in_memory().expect("service");
    serde_json::to_value(
        s.list_skills(Parameters(
            serde_json::from_value(serde_json::json!({})).expect("empty request"),
        ))
        .await
        .expect("list_skills")
        .structured_content
        .expect("structured"),
    )
    .expect("serialisable")
}

/// ⭐ THE LOAD-BEARING ONE. Every served skill states what a person types to
/// reach it — so no reader ever has to derive it by comparing directories,
/// which is exactly how 17 commands went unreported.
#[tokio::test]
async fn every_skill_states_the_command_that_reaches_it() {
    let out = listed().await;
    let missing: Vec<&str> = skills(&out)
        .iter()
        .filter(|s| {
            s.get("shortcut")
                .and_then(serde_json::Value::as_str)
                .is_none()
        })
        .filter_map(|s| s.get("name").and_then(serde_json::Value::as_str))
        .collect();
    assert!(
        missing.is_empty(),
        "a skill with no stated shortcut is one a person cannot type: {missing:?}"
    );
}

/// The aliased ones are the whole point: eight skills answer to a word that is
/// not their name, and those eight are precisely the ones a name-matching
/// reader gets wrong.
#[tokio::test]
async fn an_aliased_skill_states_the_word_a_person_actually_types() {
    let out = listed().await;
    let shortcut_of = |name: &str| -> String {
        skills(&out)
            .iter()
            .find(|s| s["name"] == name)
            .and_then(|s| s.get("shortcut").and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_string()
    };

    // The two that were MISSING from COMMAND_ALIASES until 2026-09-01, which is
    // why they lead this list.
    assert_eq!(shortcut_of("help"), "/what-is-this");
    assert_eq!(shortcut_of("onboarding"), "/where-does-it-go");
    // And the six the table already carried.
    assert_eq!(shortcut_of("capture-intent"), "/req");
    assert_eq!(shortcut_of("check-health"), "/health");
    assert_eq!(shortcut_of("detect-and-ask"), "/gaps");
    assert_eq!(shortcut_of("governance-proposal"), "/rules");
    assert_eq!(shortcut_of("kpp-proposal"), "/kpp");
    assert_eq!(shortcut_of("where-am-i"), "/where");
}

/// 🛑 THE COUNTERWEIGHT. A skill whose command IS its own name must say so
/// plainly rather than being left blank — "no alias" and "no shortcut" are
/// different facts, and blanking the ordinary case would put the reader back to
/// inferring, which is the whole defect.
#[tokio::test]
async fn an_unaliased_skill_still_states_its_shortcut() {
    let out = listed().await;
    let shortcut_of = |name: &str| -> String {
        skills(&out)
            .iter()
            .find(|s| s["name"] == name)
            .and_then(|s| s.get("shortcut").and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(shortcut_of("brainstorm"), "/brainstorm");
    assert_eq!(shortcut_of("root-cause"), "/root-cause");
    assert_eq!(shortcut_of("link-ideas"), "/link-ideas");
}

/// Asking for a skill by the word a person types SERVES that skill (since
/// 2026-09-24; before, it refused and named the mapping, which cost every
/// caller a round trip with only one possible answer). The reply says what was
/// asked for, so the caller learns the skill's own name.
#[tokio::test]
async fn asking_by_the_typed_name_serves_the_skill() {
    let s = ReflowService::in_memory().expect("service");
    for (typed, served) in [
        ("what-is-this", "help"),
        ("where-does-it-go", "onboarding"),
        ("gaps", "detect-and-ask"),
        ("/gaps", "detect-and-ask"),
        ("/impact-check", "impact-check"),
    ] {
        let out = s
            .get_skill(Parameters(
                serde_json::from_value(serde_json::json!({ "name": typed })).expect("request"),
            ))
            .await
            .unwrap_or_else(|e| {
                panic!("'{typed}' names a skill and must be served: {}", e.message)
            });
        let body = out.structured_content.expect("structured");
        assert_eq!(body["name"], served, "{typed}");
        assert_eq!(
            body["requested_as"], typed,
            "the reply must say what was asked for"
        );
    }
}

/// A command that is a TOOL, not a skill, still refuses — and the refusal
/// names the tool, which is what it was written to do.
#[tokio::test]
async fn a_command_that_is_a_tool_refuses_and_names_the_tool() {
    let s = ReflowService::in_memory().expect("service");
    let err = s
        .get_skill(Parameters(
            serde_json::from_value(serde_json::json!({ "name": "/debt" })).expect("request"),
        ))
        .await
        .expect_err("/debt is not a skill");
    assert!(err.message.contains("loop_status"), "{}", err.message);
}

/// A skill asked for by its own name carries no `requested_as`.
#[tokio::test]
async fn a_skill_by_its_own_name_says_nothing_extra() {
    let s = ReflowService::in_memory().expect("service");
    let out = s
        .get_skill(Parameters(
            serde_json::from_value(serde_json::json!({ "name": "genesis" })).expect("request"),
        ))
        .await
        .expect("served");
    assert!(
        out.structured_content
            .expect("structured")
            .get("requested_as")
            .is_none()
    );
}
