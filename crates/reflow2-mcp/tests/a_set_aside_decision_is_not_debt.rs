//! A decision set aside stops counting as work somebody owes — and setting it
//! aside is the owner's act.
//!
//! `req:an-idea-that-stopped-is-not-counted-as-debt-somebody-owes`. Measured
//! 2026-09-11: 690 Decisions, ten saying DEFERRED in prose and one literally
//! named "DEFERRED, SHAPE ONLY —", against a status field that could not hold
//! it. People were recording the state where nothing could read it, and every
//! one of those sat in the same bucket as a live question somebody was waiting
//! on — so the "what needs me" list was slightly less true each time, which is
//! how a person learns to stop reading it.
//!
//! # What these pin, and why each one
//!
//! 1. **The same decision is debt while `proposed` and not while `deferred`.**
//!    Measured through the two readers the requirement names — `loop_status`'s
//!    assigned decisions and `what_next`'s open pool — on ONE node, before and
//!    after, so the test cannot pass by the readers being empty for some other
//!    reason.
//! 2. **Neither gap detector raises it once deferred.** `undecided_decision_point`
//!    and `unreviewed_ideas` both ask `== "proposed"`, and this is what makes
//!    that a contract rather than a coincidence.
//! 3. **Deferring is the owner's word.** Landing a decision straight at
//!    `deferred` with nobody's name is REFUSED, the same as `accepted`; moving
//!    one there through the setter without a name is REPORTED in the reply.
//! 4. **A deferred choice cannot be forked** — `register_alternative` asks for
//!    `proposed` and nothing else, because a set-aside question is not an open
//!    one.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn owner(s: &ReflowService) -> Value {
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:owner".into(),
        name: Some("The owner".into()),
        kind: Some("person".into()),
        handle: None,
        description: None,
    })))
}

/// Each decision gets its OWN subject, because the search-first guard refuses
/// a second node whose prose reads like an existing one — and the first
/// version of this helper gave them all the same sentence, which is exactly
/// the near-duplicate that guard exists to catch. `distinct_from` is how a
/// test that deliberately wants two says so.
fn decision(id: &str, status: Option<&str>, approver: Option<&str>) -> DecisionReq {
    let subject = id.trim_start_matches("dec:").replace('-', " ");
    DecisionReq {
        id: id.into(),
        name: Some(format!("Whether the {subject} goes left or right")),
        decision: Some(format!(
            "The {subject} question: two roads, and which one is taken is written here so it \
             is not lost between sessions."
        )),
        rationale: Some(format!(
            "Recorded because the {subject} choice has consequences."
        )),
        distinct_from: None,
        kind: Some("choice".into()),
        related_to: None,
        no_relation_note: None,
        status: status.map(str::to_string),
        approver: approver.map(str::to_string),
        acted_at: None,
    }
}

fn ids_in(v: &Value, key: &str, id_field: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get(id_field).and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// THE POINT, measured on one node before and after.
#[tokio::test]
async fn the_same_decision_is_debt_while_proposed_and_not_while_deferred() {
    let s = ReflowService::in_memory().expect("service");
    owner(&s).await;
    // Proposed AND carrying the owner's approver edge: the shape loop_status
    // counts as "somebody was asked to decide".
    j!(s.add_decision(Parameters(decision("dec:the-one", None, None))));
    j!(s.authored_by(Parameters(AuthoredByReq {
        from_id: "dec:the-one".into(),
        from_type: Some("Decision".into()),
        contributor_id: "who:owner".into(),
        role: Some("approver".into()),
        acted_at: None,
    })));

    let before = j!(s.loop_status(Parameters(LoopScopeReq {
        contributor_id: None,
        since_export: false,
    })));
    assert!(
        ids_in(&before, "assigned_decisions", "decision_id").contains(&"dec:the-one".to_string()),
        "while proposed with an approver it IS owed: {before:?}"
    );
    let open_before = j!(s.what_next(Parameters(WhatNextReq { limit: None })))["open_total"]
        .as_u64()
        .unwrap_or(0);
    assert_eq!(open_before, 1, "and what_next counts it as open");

    // The owner sets it aside.
    j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:the-one".into(),
        status: "deferred".into(),
        approver: Some("who:owner".into()),
        acted_at: Some("2026-09-12".into()),
    })));

    let after = j!(s.loop_status(Parameters(LoopScopeReq {
        contributor_id: None,
        since_export: false,
    })));
    assert!(
        !ids_in(&after, "assigned_decisions", "decision_id").contains(&"dec:the-one".to_string()),
        "THE POINT: once deferred it is no longer work somebody owes: {after:?}"
    );
    assert_eq!(after["unsettled_assigned_decisions"].as_u64(), Some(0));
    let open_after = j!(s.what_next(Parameters(WhatNextReq { limit: None })))["open_total"]
        .as_u64()
        .unwrap_or(99);
    assert_eq!(open_after, 0, "and what_next stops counting it");
}

/// Neither detector treats a set-aside decision as an open question.
#[tokio::test]
async fn no_gap_detector_raises_a_deferred_decision() {
    let s = ReflowService::in_memory().expect("service");
    owner(&s).await;
    j!(s.add_decision(Parameters(decision("dec:parked", None, None))));
    // An exploratory idea, which unreviewed_ideas would otherwise ask about.
    let mut idea = decision("dec:idea-parked", None, None);
    idea.kind = Some("exploratory".into());
    idea.distinct_from = Some(vec!["dec:parked".into()]);
    // An exploratory idea beside a near-match must say, in the same call,
    // whether it is related — the linking discipline, which this test is not
    // about and answers honestly rather than routes around.
    idea.no_relation_note = Some(
        "Read dec:parked: a different question that happens to share this test's \
         phrasing. Not related; the two exist so both detectors have something to \
         raise."
            .into(),
    );
    j!(s.add_decision(Parameters(idea)));

    let before = j!(s.detect_gaps(Parameters(GapScopeReq {
        scope: None,
        depth: None,
        budget_chars: None,
    })));
    let names = |v: &Value| -> Vec<String> {
        v["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .flat_map(|g| {
                        g["affected_ids"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let touched_before = names(&before);
    assert!(
        touched_before
            .iter()
            .any(|id| id == "dec:parked" || id == "dec:idea-parked"),
        "while proposed at least one detector must be asking about them, or this test \
         proves nothing: {touched_before:?}"
    );

    for id in ["dec:parked", "dec:idea-parked"] {
        j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
            decision_id: id.into(),
            status: "deferred".into(),
            approver: Some("who:owner".into()),
            acted_at: None,
        })));
    }
    let after = j!(s.detect_gaps(Parameters(GapScopeReq {
        scope: None,
        depth: None,
        budget_chars: None,
    })));
    let touched_after = names(&after);
    assert!(
        !touched_after
            .iter()
            .any(|id| id == "dec:parked" || id == "dec:idea-parked"),
        "once deferred, no detector may raise them: {touched_after:?}"
    );
}

/// Deferring is the owner's act, exactly as accepting is.
#[tokio::test]
async fn deferring_wants_the_owners_name() {
    let s = ReflowService::in_memory().expect("service");
    owner(&s).await;

    // Landing straight at `deferred` with nobody's name: REFUSED.
    let err = s
        .add_decision(Parameters(decision("dec:nameless", Some("deferred"), None)))
        .await
        .expect_err("a decision past proposed with no approver must be refused");
    assert!(
        err.to_string().contains("approver"),
        "and the refusal must say what is missing: {err}"
    );

    // With the name: accepted, and it lands deferred.
    let mut named = decision("dec:named", Some("deferred"), Some("who:owner"));
    named.distinct_from = Some(vec!["dec:nameless".into()]);
    let ok = j!(s.add_decision(Parameters(named)));
    assert_eq!(
        ok["properties"]["status"].as_str(),
        Some("deferred"),
        "{ok:?}"
    );

    // Moving one there through the setter without a name is REPORTED, not
    // silently accepted — the setter has consumers and does not refuse, but the
    // reply must carry the note the intent gate would otherwise raise later.
    let mut moved_req = decision("dec:moved", None, None);
    moved_req.distinct_from = Some(vec!["dec:named".into()]);
    j!(s.add_decision(Parameters(moved_req)));
    let moved = j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:moved".into(),
        status: "deferred".into(),
        approver: None,
        acted_at: None,
    })));
    let text = moved.to_string();
    assert!(
        text.contains("NOBODY'S NAME"),
        "a deferral with no approver must carry the nobody's-name note: {moved:?}"
    );
}

/// A set-aside question is not an open one: it cannot be forked.
#[tokio::test]
async fn a_deferred_decision_cannot_have_alternatives_registered() {
    let s = ReflowService::in_memory().expect("service");
    owner(&s).await;
    j!(s.add_decision(Parameters(decision(
        "dec:shelved",
        Some("deferred"),
        Some("who:owner")
    ))));
    let err = s
        .register_alternative(Parameters(RegisterAlternativeReq {
            decision_id: "dec:shelved".into(),
            artifact_id: "art:road-a".into(),
            name: "Road A".into(),
            location: "designs/alt-a.json".into(),
        }))
        .await
        .expect_err("a deferred decision is not an open decision point");
    assert!(
        err.to_string().contains("proposed"),
        "and the refusal names the status it wanted: {err}"
    );
}
