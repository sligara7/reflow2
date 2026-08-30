//! The refusal that already fired asks one more question.
//!
//! # Why this exists
//!
//! Capturing an idea has two judgements to make about the near-matches the
//! duplicate guard surfaces: are these the SAME thing, and are any of them
//! RELATED. The guard stops the caller and makes them read the list for the
//! first question, then throws that reading away. They were only ever asked
//! half a question about a list already in their hand.
//!
//! Measured 2026-08-30 on reflow2's own graph: 140 of 207 ideas carry a
//! relation and the "nothing was related" note had been used TWICE. So for the
//! 67 unlinked ideas, "nobody looked" and "looked and found nothing" are
//! indistinguishable — which is precisely the distinction `no_relation_note`
//! exists to preserve
//! (`fact:the-linking-discipline-reproduced-its-own-finding-2026-08-30`).
//!
//! # The two things this must NOT do
//!
//! **It must not manufacture edges.** The brainstorm skill's own rule: *"Never
//! draw an edge to satisfy this step. A fabricated relation is worse than a
//! missing one."* So the note is a FULL answer, not a weaker one, and
//! `an_honest_note_is_a_complete_answer` is the case to break first when
//! doubting this suite.
//!
//! **It must not add ceremony where the hazard is not.** Scoped to
//! `exploratory`, and only when near-matches actually exist — so an idea
//! nothing resembles is captured with no extra question, and the
//! Requirement/Capability/ChangeEvent capture path a consumer reports as
//! friction is untouched.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

const TEXT: &str = "The outdoor unit sends cumulative totals rather than deltas, so a dropped \
                    reading heals itself on the next one instead of needing a retransmit.";

fn idea(id: &str) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some("OPEN — should the unit send cumulative totals rather than deltas?".into()),
        decision: Some(TEXT.into()),
        rationale: None,
        distinct_from: None,
        kind: Some("exploratory".into()),
        related_to: None,
        no_relation_note: None,
    }
}

/// Capture one idea, then a second that resembles it, acknowledging the
/// duplicate question so only the relatedness one can be left.
async fn with_a_near_match() -> (ReflowService, DecisionReq) {
    let s = svc().await;
    s.add_decision(Parameters(idea("dec:first")))
        .await
        .expect("the first idea has nothing to resemble");
    let mut second = idea("dec:second");
    second.distinct_from = Some(vec!["dec:first".into()]);
    (s, second)
}

#[tokio::test]
async fn an_idea_with_no_near_match_is_never_asked() {
    let s = svc().await;
    s.add_decision(Parameters(idea("dec:alone"))).await.expect(
        "no near matches, so no ceremony — this is the case that keeps the guard off \
                 everyday capture",
    );
}

#[tokio::test]
async fn acknowledging_the_duplicate_question_does_not_answer_the_related_one() {
    let (s, second) = with_a_near_match().await;
    let err = s
        .add_decision(Parameters(second))
        .await
        .expect_err("distinct_from settles SAMENESS; relatedness is a separate judgement");
    let msg = format!("{err:?}");
    assert!(msg.contains("dec:first"), "name what was read: {msg}");
    assert!(msg.contains("related_to"), "offer the draw route: {msg}");
    assert!(
        msg.contains("no_relation_note"),
        "and the honest-nothing route: {msg}"
    );
    assert!(
        msg.contains("NEVER INVENT A RELATION"),
        "the fabrication hazard must be named where the pressure to fabricate is: {msg}"
    );
}

#[tokio::test]
async fn an_honest_note_is_a_complete_answer() {
    let (s, mut second) = with_a_near_match().await;
    second.no_relation_note =
        Some("Read dec:first; it is about retransmit cost, this is about clock drift.".into());
    let out = s
        .add_decision(Parameters(second))
        .await
        .expect("a note is a full answer, never a weaker one")
        .structured_content
        .expect("structured content");
    assert_eq!(
        out.pointer("/properties/no_relation_note")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("clock drift")),
        Some(true),
        "and it is recorded, so a later reader can tell this was judged: {out}"
    );
}

#[tokio::test]
async fn a_drawn_relation_is_a_complete_answer_and_lands_in_the_same_call() {
    let (s, mut second) = with_a_near_match().await;
    second.related_to = Some(vec![RelationLinkReq {
        relation: "EVOLVES_INTO".into(),
        other_type: "Decision".into(),
        other_id: "dec:first".into(),
        evidence: "The earlier thought, grown up: same mechanism, stated for a second reason."
            .into(),
        incoming: Some(true),
    }]);
    s.add_decision(Parameters(second))
        .await
        .expect("drawing the relation is the other complete answer");

    // Walk from the OTHER end. If the edge landed in the same call, the
    // neighbourhood is reachable immediately — no second call, nothing to lose
    // between two.
    let reach = s
        .propagate_from(Parameters(PropagateFromReq {
            seed_ids: vec!["dec:first".into()],
            max_depth: Some(2),
            full: None,
        }))
        .await
        .expect("propagate ok")
        .structured_content
        .expect("structured content");
    assert!(
        format!("{reach}").contains("dec:second"),
        "the relation must be walkable straight away, which is the whole point of drawing it in \
         the same call: {reach}"
    );
}

/// A plain choice is untouched, whatever it resembles. This is the boundary
/// that keeps the guard away from ordinary capture.
#[tokio::test]
async fn a_choice_is_not_asked_even_when_it_has_near_matches() {
    let s = svc().await;
    s.add_decision(Parameters(idea("dec:first")))
        .await
        .expect("seed");
    let mut choice = idea("dec:a-choice");
    choice.kind = Some("choice".into());
    choice.distinct_from = Some(vec!["dec:first".into()]);
    s.add_decision(Parameters(choice))
        .await
        .expect("a choice carries no linking discipline — that is what scoping buys");
}
