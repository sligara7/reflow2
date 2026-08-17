//! `open_questions` returning 0 must say WHICH zero it is.
//!
//! # Why this exists
//!
//! `req:a-report-says-what-it-swept-and-whether-its-checks-ran`, part (c),
//! reported by dev_storyflow's fleet 2026-08-08. `open_questions` returned 0
//! and read as an all-clear, while `loop_status` **in the very next call**
//! reported 31 other owed items — and `open_questions` is the orientation call
//! a session is told to run FIRST. Their own remedy is the one taken here:
//! naming the other non-zero counts is enough.
//!
//! # Why it could not simply reuse the existing hint
//!
//! `ok_read` already attaches a `loop_hint`, but `dec:read-hint-shape` option C
//! throttles it deliberately — a persisting debt appears once and then stays
//! quiet so reads do not nag. That reasoning is right while the reader is being
//! handed findings, and it inverts when the answer is EMPTY: the throttle then
//! removes the only sentence in the reply and leaves a zero to speak for
//! itself. So an empty answer here is exempt from the throttle, and a non-empty
//! one is not.
//!
//! The distinction this pins is the whole requirement in one line: **"nothing
//! to show you" and "nothing is owed" must stop sharing a reply.**

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

fn hint(v: &serde_json::Value) -> String {
    v.get("loop_hint")
        .and_then(|h| h.as_str())
        .unwrap_or_default()
        .to_string()
}

/// The measured case: no questions, plenty owed. The zero must not stand alone.
#[tokio::test]
async fn an_empty_answer_names_the_debt_it_is_not_reporting() {
    let s = svc().await;
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "proj:p", "name": "P"
        }))
        .unwrap(),
    ))
    .await
    .expect("project");
    // Something the loop is genuinely owed, and nothing to do with questions.
    // A check that checks nothing is a structural defect since the degree-zero
    // rule stopped being a Decision rule, so this is debt the loop counts —
    // unlike a bare Requirement, which lands at `proposed`, or a phase nudge,
    // which says what comes next rather than what is owed.
    s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "ver:loose", "name": "a check attached to nothing"
        }))
        .unwrap(),
    ))
    .await
    .expect("verification");

    let out = j!(s.open_questions());
    assert_eq!(
        out.get("count").and_then(serde_json::Value::as_u64),
        Some(0),
        "precondition: no questions have been asked"
    );

    let h = hint(&out);
    assert!(
        !h.is_empty(),
        "a zero from the FIRST call a session makes must not come back bare: {out:?}"
    );
    assert!(
        h.contains("not an all-clear"),
        "it must say which zero it is, in words: {h}"
    );
    assert!(
        h.contains("structural defect(s)"),
        "and name the other non-zero debt, which is the reporter's own remedy: {h}"
    );
}

/// The other half, and the one that keeps this from being a new nag: when
/// nothing at all is owed, the empty answer says THAT rather than going quiet.
/// A silent reply cannot be told from a throttled one, which is the failure
/// being fixed rather than a smaller version of it.
#[tokio::test]
async fn an_empty_answer_on_a_clean_loop_says_so_explicitly() {
    let s = svc().await;
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({
            "id": "proj:p", "name": "P"
        }))
        .unwrap(),
    ))
    .await
    .expect("project");

    let out = j!(s.open_questions());
    assert_eq!(
        out.get("count").and_then(serde_json::Value::as_u64),
        Some(0)
    );
    let h = hint(&out);
    assert!(
        h.contains("all-clear") && !h.contains("not an all-clear"),
        "a clean loop must be reported as a positive all-clear, not as silence: {h}"
    );
}
