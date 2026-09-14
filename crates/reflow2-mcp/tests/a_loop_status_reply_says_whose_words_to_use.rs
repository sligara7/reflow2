//! Does the loop_status reply tell the agent whose words to use?
//!
//! FIELD REPORT, 2026-09-14: a session closed with *"Remaining: 6 structural
//! findings and 1 undispositioned drift … its repair step can delete nodes, so
//! it's worth reading what it proposes rather than applying blind."* That
//! sentence is `loop_status`'s own reply — a field name and a `next` line —
//! read out to a person as if it were English. The person's brother does not
//! know the words; a baseball coach would give up.
//!
//! ROOT CAUSE, walked and measured: the lens (`cap:the-skill-response-carries-
//! the-lens`) rides `get_skill` and `list_skills` only. Every served skill ends
//! with "before moving on: `loop_status`", so the LAST reply an agent reads
//! before its closing words is the one reply on that path that carries no
//! lens — and its `next` lines are instructions written FOR THE AGENT in
//! reflow2's vocabulary. Nothing marked them as such.
//!
//! `a_skill_response_says_who_it_is_for.rs` pins the lens on the skill rail.
//! This pins it on the reply that arrives at the moment the words are chosen.

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

async fn service() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:x".into(),
        name: Some("X".to_string()),
        description: None,
        spec: None,
        decomposition_levels: None,
    })));
    s
}

async fn described(s: &ReflowService, id: &str, description: Option<&str>) {
    j!(s.add_contributor(Parameters(ContributorReq {
        id: id.into(),
        name: Some(id.to_string()),
        kind: Some("person".into()),
        handle: None,
        description: description.map(str::to_string),
    })));
}

async fn loop_status(s: &ReflowService) -> serde_json::Value {
    j!(s.loop_status(Parameters(LoopScopeReq {
        contributor_id: None,
        since_export: false,
    })))
}

/// ⭐ THE CASE THE FIELD REPORT IS. The reply carries a to-do list for the
/// agent; the same reply must say that what a person reads is said in their
/// words. When nobody's background is recorded it says so and names who could
/// be asked — absence is the signal, never an omitted field.
#[tokio::test]
async fn loop_status_says_plainly_when_nobody_s_background_is_recorded() {
    let s = service().await;
    described(&s, "who:ann", None).await;

    let out = loop_status(&s).await;
    let lens = out
        .get("lens")
        .and_then(|v| v.as_str())
        .expect("a loop_status reply carries a lens");

    assert!(
        lens.contains("NOBODY'S BACKGROUND IS RECORDED"),
        "the silent case must be stated, not implied: {lens}"
    );
    assert!(
        lens.contains("who:ann"),
        "and it must NAME who could be asked: {lens}"
    );
}

/// The ordinary case: the reply names who is described, so the agent reads the
/// right one before it writes the closing summary.
#[tokio::test]
async fn loop_status_names_the_people_the_design_can_describe() {
    let s = service().await;
    described(
        &s,
        "who:ann",
        Some("Hitting coach. Baseball. Not software."),
    )
    .await;

    let out = loop_status(&s).await;
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");

    assert!(lens.contains("who:ann"), "{lens}");
    assert!(
        !lens.contains("NOBODY'S BACKGROUND IS RECORDED"),
        "a design that CAN answer must not report itself silent: {lens}"
    );
}

/// 🛑 THE `next` LINES ARE FOR THE AGENT, AND THE REPLY HAS TO SAY SO. The
/// field report's sentence was a `next` line paraphrased to a person. The lens
/// on this reply names the to-do list as the agent's, so the agent has a reason
/// to translate rather than relay.
#[tokio::test]
async fn the_lens_on_loop_status_says_the_to_do_list_is_the_agents_not_the_readers() {
    let s = service().await;
    described(&s, "who:ann", Some("Vet.")).await;

    let out = loop_status(&s).await;
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");

    assert!(
        lens.contains("`next`"),
        "the reply must name its own to-do list as agent-facing: {lens}"
    );
}

/// 🛑 THE LENS MUST NEVER WITHHOLD THE DEBT. `next` is what the agent came for;
/// the reminder is an addition beside it, never a condition on it.
#[tokio::test]
async fn the_lens_never_withholds_the_debt() {
    let s = service().await;
    let out = loop_status(&s).await;
    assert!(out.get("next").is_some(), "`next` still arrives: {out}");
    assert!(
        out.get("lens").is_some(),
        "and the lens rides beside it: {out}"
    );
}
