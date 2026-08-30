//! A brainstormed idea says it is one, in a property a computation can read.
//!
//! # Why this exists
//!
//! `dec:exploratory-staging` asked twice whether a brainstormed idea deserves a
//! kind of its own and twice judged it too small: brainstormed ideas are
//! already silent by construction, delivery counts already exclude Decisions,
//! and what remained — retirement candidates, at-a-glance readability — was
//! "real but small".
//!
//! What changed is that a COMPUTATION now needs it, which is the bar
//! `req:a-vocabulary-distinction-proves-it-is-read` sets. The brainstorm
//! skill's linking discipline must fire on ideas and stay OFF the
//! Requirement/Capability/ChangeEvent capture path that a consumer's field
//! report identifies as existing friction. Measured 2026-08-30 there was
//! nothing to fire on: 207 idea-Decisions and 363 other Decisions differed by
//! nothing readable. `status` does not separate them (32 non-idea Decisions are
//! also `proposed`), and the only remaining signal was the `dec:idea-` id
//! prefix — the defect closed that same morning in #390, where an id's spelling
//! was read as its type.
//!
//! # The state that matters most is the absent one
//!
//! There is no default. Absent means NOBODY SAID, which is a third state and
//! not a synonym for `choice` — the doctrine `Decision.quality_target` already
//! states, and the reason the 207 existing ideas stay unmarked: their only
//! evidence is the prefix being retired, so backfilling would launder it into a
//! property.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn dec(id: &str, kind: Option<&str>) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some(format!("A node called {id}")),
        decision: Some("Some text distinctive enough to avoid a near-match refusal.".into()),
        rationale: None,
        distinct_from: None,
        kind: kind.map(str::to_string),
    }
}

#[tokio::test]
async fn an_idea_records_that_it_is_exploratory() {
    let s = svc().await;
    let out = s
        .add_decision(Parameters(dec("dec:an-idea", Some("exploratory"))))
        .await
        .expect("tool ok")
        .structured_content
        .expect("structured content");
    assert_eq!(
        out.pointer("/properties/kind").and_then(|v| v.as_str()),
        Some("exploratory"),
        "the kind must be stored where a computation can read it: {out}"
    );
}

/// The third state. A default would make an unasked question indistinguishable
/// from an answered one, which is exactly what `quality_target` refuses.
#[tokio::test]
async fn omitting_the_kind_leaves_it_absent_rather_than_defaulting() {
    let s = svc().await;
    let out = s
        .add_decision(Parameters(dec("dec:unsaid", None)))
        .await
        .expect("tool ok")
        .structured_content
        .expect("structured content");
    assert!(
        out.pointer("/properties/kind").is_none(),
        "absent must stay absent — silently defaulting to `choice` would assert that somebody \
         classified this when nobody did: {out}"
    );
}

/// A wrong value is refused rather than stored, and the refusal says that
/// omitting is legal — otherwise the obvious repair is to guess `choice`.
#[tokio::test]
async fn an_unknown_kind_is_refused_and_says_omitting_is_allowed() {
    let s = svc().await;
    let err = s
        .add_decision(Parameters(dec("dec:bad", Some("brainstorm"))))
        .await
        .expect_err("an undeclared value must not be stored");
    let msg = format!("{err:?}");
    assert!(msg.contains("exploratory"), "name the legal values: {msg}");
    assert!(
        msg.contains("OMITTING IT IS ALSO VALID"),
        "and say that absent is a real answer, or the repair is to guess: {msg}"
    );
}

/// The kind is set in ONE call. A follow-up setter would be two order-dependent
/// calls, which is the hazard #392 documented and `cap:a-decision-can-be-created-already-settled`
/// was built to remove for `status`.
#[tokio::test]
async fn the_kind_needs_no_second_call() {
    let s = svc().await;
    let out = s
        .add_decision(Parameters(dec("dec:one-call", Some("choice"))))
        .await
        .expect("tool ok")
        .structured_content
        .expect("structured content");
    assert_eq!(
        out.pointer("/properties/kind").and_then(|v| v.as_str()),
        Some("choice")
    );
    assert_eq!(
        out.pointer("/properties/status").and_then(|v| v.as_str()),
        Some("proposed"),
        "and it must not disturb the status doctrine — a Decision still lands proposed: {out}"
    );
}
