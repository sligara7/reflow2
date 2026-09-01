//! "Search before you add" fires from the TOOL, and forces a CHOICE.
//!
//! # Why this exists
//!
//! The rule was already written into FIVE served skills — capture-intent,
//! revise-design, brainstorm, kpp-proposal, retire-from-design — and still only
//! fired when a session loaded one. `req:skill-use-survives-a-long-session`
//! (accepted) says skill use must be triggered by the situation and "THE USER
//! MUST NEVER BE THE TRIGGER": measured on this repo 2026-07-31, and confirmed
//! from the field 2026-08-11 by a second user whose four-step working path
//! named exactly the two skills he types and neither of the two that would have
//! fired on situations he was already in. This is
//! `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue` applied to
//! the one discipline whose absence produces near-duplicate nodes.
//!
//! # Reporting was not enough, and that is the point
//!
//! A first cut created the node and then reported what looked similar. Anthony,
//! 2026-08-11: "there needs to be a deliberate decision of either update
//! already existing node or if it is unique enough, create a new node." By the
//! time a report is read the near-duplicate already exists — so the check
//! REFUSES and names both routes, which is the two-sided accept (BL-33) that
//! `set_artifact_checksum` already uses.
//!
//! Both routes existed before this change and neither is new vocabulary:
//! SHARPEN by calling with the existing id (constructors merge, BL-183), or
//! CREATE ANYWAY with `distinct_from` naming what you read and rejected.

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

/// The pair used throughout: two statements of one idea, in different words.
const IDEA_A: &str = "A lost reading heals itself because the outdoor unit sends cumulative \
                      totals rather than deltas, so a dropped packet costs nothing.";
const IDEA_B: &str = "The outdoor unit sends cumulative totals rather than deltas so that a lost \
                      reading heals itself and a dropped packet costs nothing.";

fn req(
    id: &str,
    name: &str,
    statement: &str,
    distinct_from: Option<Vec<String>>,
) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(name.into()),
        statement: Some(statement.into()),
        distinct_from,
    }
}

async fn seed_first(s: &ReflowService) {
    j!(s.add_requirement(Parameters(req(
        "req:offline-sync",
        "The reading store heals a lost reading",
        IDEA_A,
        None
    ))));
}

// THE DEFECT CASE. Two requirements saying one thing in different words is what
// nine collisions in a single session were made of.
#[tokio::test]
async fn a_near_duplicate_is_refused_and_both_routes_are_named() {
    let s = svc().await;
    seed_first(&s).await;
    let err = s
        .add_requirement(Parameters(req(
            "req:cumulative-totals",
            "Lost readings heal themselves",
            IDEA_B,
            None,
        )))
        .await
        .expect_err("a near duplicate must not be created silently");
    let msg = err.message.to_string();
    assert!(
        msg.contains("req:offline-sync"),
        "the refusal must NAME what it found: {msg}"
    );
    assert!(
        msg.contains("SHARPEN") && msg.contains("distinct_from"),
        "the refusal must name BOTH routes, or it is a wall rather than a choice: {msg}"
    );
}

// THE ROLLBACK. The check needs the node in the index to have an in-query
// baseline, so the write happens first — which means a refusal MUST leave
// nothing behind, or the check creates the very duplicate it refused.
#[tokio::test]
async fn a_refused_create_leaves_no_node_behind() {
    let s = svc().await;
    seed_first(&s).await;
    let _ = s
        .add_requirement(Parameters(req(
            "req:cumulative-totals",
            "Lost readings heal themselves",
            IDEA_B,
            None,
        )))
        .await;
    let stored = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Requirement".into(),
        id: "req:cumulative-totals".into(),
    })));
    assert!(
        stored["node"].is_null(),
        "a refused create must leave NOTHING, got {stored}"
    );
}

// ROUTE ONE — START A NEW ONE. Saying the same thing twice in different words
// is sometimes real signal: `req:accreted-intent-becomes-a-design` exists
// because a body of intent that contradicts itself is still a design. So the
// deliberate override must work.
#[tokio::test]
async fn naming_what_was_rejected_lets_the_new_node_through() {
    let s = svc().await;
    seed_first(&s).await;
    let made = j!(s.add_requirement(Parameters(req(
        "req:cumulative-totals",
        "Lost readings heal themselves",
        IDEA_B,
        Some(vec!["req:offline-sync".into()]),
    ))));
    assert_eq!(made["node_id"], "req:cumulative-totals");
}

// ROUTE TWO — SHARPEN. Calling with the EXISTING id is a revision, merges
// (BL-183), and must never be refused: it is the route the refusal recommends
// first, so a refusal here would make the advice impossible to follow.
#[tokio::test]
async fn sharpening_the_existing_node_is_never_refused() {
    let s = svc().await;
    seed_first(&s).await;
    // A neighbour, so the graph genuinely holds something near — without this
    // the assertion would hold whether the revision guard existed or not.
    j!(s.add_requirement(Parameters(req(
        "req:cumulative-totals",
        "Lost readings heal themselves",
        IDEA_B,
        Some(vec!["req:offline-sync".into()]),
    ))));
    let sharpened = j!(s.add_requirement(Parameters(req(
        "req:offline-sync",
        "The reading store heals a lost reading",
        "A lost reading heals itself because the outdoor unit sends cumulative totals rather \
         than deltas, so a dropped packet costs nothing. Sharpened: the window is 24 hours.",
        None,
    ))));
    assert!(
        sharpened["properties"]["statement"]
            .as_str()
            .unwrap_or_default()
            .contains("24 hours"),
        "the sharpened wording must land on the existing node, got {sharpened}"
    );
}

// THE COUNTERWEIGHT THAT DECIDES WHETHER THIS IS USABLE. An unrelated node must
// sail through. A check that refused often would be routed around, and a rule
// people route around is worse than no rule.
#[tokio::test]
async fn an_unrelated_node_is_created_without_friction() {
    let s = svc().await;
    seed_first(&s).await;
    let made = j!(s.add_requirement(Parameters(req(
        "req:paint-colour",
        "The enclosure is powder-coated",
        "The outdoor enclosure is powder-coated in a light colour to reduce solar gain on the \
         housing.",
        None,
    ))));
    assert_eq!(made["node_id"], "req:paint-colour");
    assert!(
        made.get("search_first").is_none(),
        "an unrelated node must carry no advisory at all, got {made}"
    );
}

// ACROSS TYPES on purpose: a Capability backed out of a Requirement's own words
// is `unmotivated_capability` waiting to happen, and that pair is the one most
// worth SEEING.
//
// ⭐ IT IS SEEN, AND SINCE 2026-08-31 IT IS NO LONGER REFUSED. Requirement and
// the Capability that satisfies it is the golden thread — the served
// instructions ask for both records, and it was the single most-refused pairing
// in the field: 4 of bhome's 8 false positives and 1 of musicjug's 4, with not
// one of the 15 measured refusals turning out to be a real duplicate
// (`chg:the-prescribed-layer-pairs-stop-refusing`).
//
// 🛑 SO WHAT THIS TEST NOW PINS IS THE DISTINCTION THAT CHANGE RESTS ON: the
// REPORT survives even though the REFUSAL is gone. If the advisory ever stopped
// carrying the match, suppressing the refusal would have become deleting the
// check, and nothing else in the suite would notice.
#[tokio::test]
async fn a_capability_restating_a_requirement_is_reported_but_not_refused() {
    let s = svc().await;
    seed_first(&s).await;
    let made = j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:cumulative".into(),
        name: Some("Send cumulative totals".into()),
        description: Some(IDEA_B.into()),
        status: None,
        distinct_from: None,
    })));
    assert_eq!(made["node_id"], "cap:cumulative");
    let advisory = made
        .get("search_first")
        .expect("the golden-thread pair is still REPORTED, only no longer refused");
    assert!(
        serde_json::to_string(advisory)
            .expect("serialisable")
            .contains("req:offline-sync"),
        "the cross-type match must still be named in the advisory: {advisory}"
    );
}

// THE FALSE POSITIVE CI FOUND, and the reason NEAR_MATCH_MIN_WORDS exists.
// An unrelated suite writes two requirements with three-word statements —
// "Session A wrote this." / "Session B wrote this." — and they were refused as
// near-duplicates, which is plainly wrong to any reader.
//
// The cause is an ABSENT SIGNAL rather than a bad threshold: two near-empty
// documents share most of their tokens by construction, so the ratio compares
// noise to noise and always fires. It is also the genesis case — a young design
// is all short statements, and a check that refused every early requirement
// would be switched off on its first day.
#[tokio::test]
async fn text_too_short_to_carry_a_judgement_is_not_judged() {
    let s = svc().await;
    j!(s.add_requirement(Parameters(req(
        "req:from-a",
        "From A",
        "Session A wrote this.",
        None
    ))));
    let b = j!(s.add_requirement(Parameters(req(
        "req:from-b",
        "From B",
        "Session B wrote this.",
        None
    ))));
    assert_eq!(
        b["node_id"], "req:from-b",
        "a short statement must never be refused — there is no signal to judge on, got {b}"
    );
    assert!(
        b.get("search_first").is_none(),
        "and nothing is claimed about it either way, got {b}"
    );
}
