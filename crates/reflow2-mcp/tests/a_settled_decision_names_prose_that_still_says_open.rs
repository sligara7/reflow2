//! A Decision settles, and the prose it governs still says the question is open.
//!
//! THE INCIDENT, on reflow2's own design, 2026-09-09. `req:a-fix-says-whether-
//! it-corrected-the-cause` ended with "NOT YET DECIDED: … whether it is
//! retro-fittable … that is his call to make explicitly rather than mine to
//! infer." The call had been made three weeks earlier — `dec:the-patch-record-
//! binds-forward-not-backward`, ACCEPTED 2026-08-17, GOVERNED_BY-linked to that
//! very requirement. A session read the stale paragraph, believed the fork was
//! open, and put a settled question to the owner a second time. He gave the
//! same answer. Recorded as `fact:a-settled-question-was-re-asked-because-the-
//! requirements-prose-outlived-its-decision`.
//!
//! ⭐ ROOT CAUSE, and it is NOT "the session should have searched harder". It
//! did search: `search_design` returned the requirement, the rule, the
//! capability, the artifact and the verification, and NOT the governing
//! Decision, whose name shares few tokens with any query about the requirement.
//! The edge was there the whole time. THE CAUSE IS THAT PROSE ASSERTING AN OPEN
//! QUESTION OUTLIVES THE DECISION THAT SETTLES IT, AND NOTHING SAYS SO — the
//! same class `prose_currency` was built for one type over, where a status
//! outruns its own description. That instrument had no equivalent for a node
//! whose prose contradicts an accepted Decision already linked to it.
//!
//! WHY A REPLY AND NOT A DETECTOR, which is the sibling module's reasoning and
//! was re-measured before building this: over the live design, 43 of 640
//! governed nodes carry an open-question marker somewhere in their prose, and
//! plenty are legitimate — a node that QUOTES an old question, or discusses one
//! elsewhere. As a sweep that is 48 findings of mixed quality and it gets
//! switched off. Fired at the moment somebody accepts the Decision or draws the
//! edge, it is one node in front of the person who just moved it.
//!
//! The counterweights matter as much as the case: this must be SILENT on a
//! status that did not settle, on a decision that was already accepted, and on
//! governed prose that asserts nothing open.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

/// A requirement carrying `statement`, governed by a `proposed` decision.
async fn svc(statement: &str) -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    s.create_node(Parameters(
        serde_json::from_value(json!({
            "node_type": "Requirement",
            "id": "req:thing",
            "props": {"name": "A thing", "statement": statement, "status": "accepted"},
        }))
        .unwrap(),
    ))
    .await
    .expect("create requirement");
    s.add_decision(Parameters(
        serde_json::from_value(json!({
            "id": "dec:thing",
            "name": "The thing is settled",
            "decision": "It binds forward only.",
        }))
        .unwrap(),
    ))
    .await
    .expect("add_decision");
    link(&s, "dec:thing").await;
    s
}

async fn link(s: &ReflowService, to: &str) -> Value {
    s.governed_by(Parameters(
        serde_json::from_value(json!({"from_id": "req:thing", "to_id": to})).unwrap(),
    ))
    .await
    .expect("governed_by")
    .structured_content
    .expect("structured")
}

async fn settle(s: &ReflowService, to: &str) -> Value {
    s.set_decision_status(Parameters(
        serde_json::from_value(json!({"decision_id": "dec:thing", "status": to})).unwrap(),
    ))
    .await
    .expect("set_decision_status")
    .structured_content
    .expect("structured")
}

const STALE: &str = "The shape is agreed. NOT YET DECIDED: whether it is retro-fittable to the events \
     already recorded, or binds only forward. That is his call to make explicitly rather \
     than mine to infer.";

/// THE CASE. The decision settles; the requirement it governs still says the
/// question is open; the reply names it and quotes the prose.
#[tokio::test]
async fn a_decision_that_settles_names_governed_prose_that_still_reads_open() {
    let s = svc(STALE).await;

    let v = settle(&s, "accepted").await;
    let block = v
        .get("settled_question_prose")
        .unwrap_or_else(|| panic!("no settled_question_prose block in: {v}"));

    let governs = block["governs"].as_array().expect("governs array");
    assert_eq!(
        governs.len(),
        1,
        "one governed node still reads open: {block}"
    );
    let hit = &governs[0];
    assert_eq!(hit["node_id"], "req:thing");
    assert_eq!(hit["node_type"], "Requirement");
    assert_eq!(hit["field"], "statement");
    assert!(
        hit["markers"]
            .as_array()
            .expect("markers")
            .iter()
            .any(|m| m == "not yet decided"),
        "the phrase that matched must be named, so a reader can see WHY this fired: {hit}"
    );
    assert!(
        hit["excerpt"].as_str().unwrap().contains("NOT YET DECIDED"),
        "the prose must be QUOTED so it can be judged in this reply rather than in \
         another call: {hit}"
    );

    let note = block["note"].as_str().unwrap();
    assert!(
        note.contains("accepted"),
        "the note must say the decision settled: {note}"
    );
    // dec:report-dont-judge. It cannot read English and must not claim the
    // prose is wrong — only that it was written while the question was open.
    //
    // ASSERTED AS A POSITIVE PROPERTY, and the first attempt at this test shows
    // why: it blacklisted substrings like "is wrong", and failed on the note's
    // own disclaimer — "nothing here CLAIMS THE TEXT IS WRONG". A blacklist
    // cannot tell an assertion from its denial, which is the same limit the
    // feature under test has and the reason it quotes rather than concludes.
    let lowered = note.to_lowercase();
    assert!(
        note.contains('?'),
        "the block must ASK rather than conclude: {note}"
    );
    assert!(
        lowered.contains("only a person"),
        "the block must say plainly that the judgement is not its own: {note}"
    );
}

/// SILENT when the decision did not actually settle — `proposed` is somebody
/// thinking out loud, and a musing must not accuse anyone's prose.
#[tokio::test]
async fn a_decision_that_does_not_settle_is_silent() {
    let s = svc(STALE).await;
    let v = settle(&s, "rejected").await;
    assert!(
        v.get("settled_question_prose").is_none(),
        "rejecting retires rather than settles; nothing was decided: {v}"
    );
}

/// SILENT on a re-set that moves nothing. A block on every call is the noise
/// this family is explicitly built not to become.
#[tokio::test]
async fn a_decision_already_accepted_is_silent_on_re_set() {
    let s = svc(STALE).await;
    let first = settle(&s, "accepted").await;
    assert!(
        first.get("settled_question_prose").is_some(),
        "precondition: the first settle fires"
    );
    let again = settle(&s, "accepted").await;
    assert!(
        again.get("settled_question_prose").is_none(),
        "accepted -> accepted creates no new divergence: {again}"
    );
}

/// SILENT when the governed prose asserts nothing open. The overwhelmingly
/// common case, and the one that decides whether this stays switched on.
#[tokio::test]
async fn governed_prose_that_reads_settled_is_silent() {
    let s = svc("The outdoor unit sends cumulative totals, so a lost reading heals itself.").await;
    let v = settle(&s, "accepted").await;
    assert!(
        v.get("settled_question_prose").is_none(),
        "no marker, nothing to ask about: {v}"
    );
}

/// THE OTHER MOMENT THE DIVERGENCE IS CREATED. The decision was already
/// accepted and the EDGE is drawn afterwards — which is the ordering the
/// incident actually had available, and a hook only on the status setter would
/// miss it entirely.
#[tokio::test]
async fn linking_open_prose_to_an_already_accepted_decision_is_reported() {
    let s = ReflowService::in_memory().expect("service");
    s.create_node(Parameters(
        serde_json::from_value(json!({
            "node_type": "Requirement",
            "id": "req:thing",
            "props": {"name": "A thing", "statement": STALE, "status": "accepted"},
        }))
        .unwrap(),
    ))
    .await
    .expect("create requirement");
    s.add_decision(Parameters(
        serde_json::from_value(json!({
            "id": "dec:already",
            "name": "Already settled",
            "decision": "It binds forward only.",
        }))
        .unwrap(),
    ))
    .await
    .expect("add_decision");
    s.set_decision_status(Parameters(
        serde_json::from_value(json!({"decision_id": "dec:already", "status": "accepted"}))
            .unwrap(),
    ))
    .await
    .expect("settle");

    let v = link(&s, "dec:already").await;
    let block = v
        .get("settled_question_prose")
        .unwrap_or_else(|| panic!("no settled_question_prose block in: {v}"));
    assert_eq!(block["governs"][0]["node_id"], "req:thing");
    assert!(
        block["note"].as_str().unwrap().contains("accepted"),
        "the note names the settled decision: {block}"
    );
}

/// SILENT when the edge points at a decision nobody has accepted. Linking to a
/// musing says nothing about whether the prose is current.
#[tokio::test]
async fn linking_open_prose_to_a_proposed_decision_is_silent() {
    let s = svc(STALE).await;
    let v = link(&s, "dec:thing").await;
    assert!(
        v.get("settled_question_prose").is_none(),
        "a proposed decision has settled nothing: {v}"
    );
}
