//! A near-match of a DIFFERENT NODE TYPE is not a duplicate accusation.
//!
//! # The report
//!
//! Alex, 2026-08-29 (`art:grok-shell-feedback-2026-08-29`): after recording a
//! ChangeEvent for a code change, capturing the same idea as a Requirement was
//! REFUSED — and the refusal named the ChangeEvent. Then a Decision restating a
//! Capability was refused naming the Capability. Both were resolved with
//! `distinct_from`. His verdict on the message itself was that it is *"clear
//! and actionable"*; the complaint is that this is now the default path:
//!
//! > The capture loop after a build is: ChangeEvent (what we shipped) then
//! > Requirement/Decision (what must remain true / why we chose this). Those
//! > are supposed to be different records of the same idea.
//!
//! He is right, and the served instructions ask for exactly that sequence. So
//! an agent following the loop HONESTLY trips the guard every time.
//!
//! # The measured cause
//!
//! `fact:the-near-match-filter-has-no-node-type-term`. The filter in
//! `capture.rs` tests two things — not-itself, and score-above-floor — and
//! neither is the node type. `node_type` was carried into `NearMatch` and
//! printed, but nothing in the SELECTION read it.
//!
//! 🛑 SO THIS IS NOT A THRESHOLD PROBLEM AND RETUNING CANNOT FIX IT. A
//! ChangeEvent and the Requirement behind it describe one piece of work in one
//! vocabulary; they SHOULD score alike. Any ratio that separated them would
//! also stop catching the same-type duplicates the guard exists for — the nine
//! collisions in one session that produced it.
//!
//! # The repair chosen, and the one rejected
//!
//! Two shapes were available and they are not equivalent:
//!
//! * ① EXCLUDE cross-type pairs from the filter. Removes the friction, and
//!   removes a real catch when the agent genuinely was duplicating an idea
//!   across layers. REJECTED: it trades a false alarm for a silent miss, and a
//!   silent miss in the duplicate guard is the failure it was built to stop.
//! * ② KEEP the refusal, change what it SAYS when the types differ — name the
//!   layering instead of implying duplication. CHOSEN. Every real catch
//!   survives, the cost is one more round-trip, and it matches how this guard
//!   was justified: `refuse_unless_deliberate` is documented as forcing a
//!   CHOICE, never as preventing a write.
//!
//! ⚠️ WHAT IS STILL NOT ESTABLISHED, so nobody reads ② as settled: nobody has
//! counted how often a cross-type hit is a REAL duplicate versus this layered
//! capture. That count is what would justify ①, and it has not been taken.
//!
//! # What these tests pin
//!
//! The CLASS, not Alex's two instances: that the refusal's wording is a
//! function of whether the types differ, in both directions. Pinning only his
//! ChangeEvent→Requirement case would leave the same-type wording free to drift
//! into the layered phrasing, which would quietly destroy the distinction.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

/// One idea, in one vocabulary. Used verbatim for both records on purpose:
/// the layered capture is the case where the words genuinely DO match.
const SHIPPED: &str = "The client stops attaching to a shared server running a different build \
                       and elects one of its own instead, so an upgrade cannot leave the old \
                       daemon serving.";

fn requirement(id: &str, name: &str, statement: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(name.into()),
        statement: Some(statement.into()),
        distinct_from: None,
    }
}

async fn seed_change_event(s: &ReflowService) {
    s.add_change_event(Parameters(AddChangeEventReq {
        id: "chg:client-elects-its-own-build".into(),
        name: Some("A client elects a server of its own build".into()),
        change_type: Some("defect_fix".into()),
        subject: Some("system".into()),
        summary: Some(SHIPPED.into()),
        rationale: None,
        affected: None,
        detected_at: None,
    }))
    .await
    .expect("the change event records what shipped");
}

/// ⭐ THE REPORTED CASE. The prescribed loop — record what shipped, then record
/// the durable intent behind it — must not be told it is duplicating.
#[tokio::test]
async fn a_cross_type_near_match_names_the_layering_not_a_duplicate() {
    let s = svc().await;
    seed_change_event(&s).await;

    let err = s
        .add_requirement(Parameters(requirement(
            "req:a-session-serves-its-own-build",
            "A session talks only to a server of its own build",
            SHIPPED,
        )))
        .await
        .expect_err("the guard still refuses, because ② keeps the refusal");

    let msg = err.message.to_string();

    // It must still do its job: name what it found, and both ways on.
    assert!(
        msg.contains("chg:client-elects-its-own-build"),
        "the refusal must still NAME what it matched: {msg}"
    );
    assert!(
        msg.contains("distinct_from"),
        "the refusal must still name the way through: {msg}"
    );

    // ⭐ And it must say what KIND of match this is. The agent's next action
    // differs: a same-type hit invites sharpening the existing node, a
    // cross-type hit invites recording the second layer and moving on.
    assert!(
        msg.contains("ChangeEvent"),
        "the refusal must name the OTHER TYPE, or the agent cannot tell a layer from a copy: \
         {msg}"
    );
    assert!(
        msg.to_lowercase().contains("different kind of record"),
        "a cross-type match must be described as a different KIND of record rather than as a \
         near-duplicate: {msg}"
    );

    // 🛑 And it must NOT tell the agent to sharpen the ChangeEvent. Calling
    // add_requirement with a ChangeEvent's id is not a route that exists, and
    // offering it sends the agent somewhere it cannot go.
    assert!(
        !msg.contains("SHARPEN"),
        "sharpening is a same-type route and must not be offered across types: {msg}"
    );
}

/// 🛑 THE COUNTERWEIGHT, and the reason this file has two tests. If the
/// same-type wording drifted into the layered phrasing, the first test would
/// still pass while the guard stopped distinguishing anything at all.
#[tokio::test]
async fn a_same_type_near_match_is_still_a_duplicate_and_still_offers_sharpening() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(
        "req:a-session-serves-its-own-build",
        "A session talks only to a server of its own build",
        SHIPPED,
    )))
    .await
    .expect("the first requirement lands");

    let err = s
        .add_requirement(Parameters(requirement(
            "req:no-stale-daemon",
            "An upgrade never leaves the old daemon serving",
            SHIPPED,
        )))
        .await
        .expect_err("a same-type near duplicate is still refused");

    let msg = err.message.to_string();
    assert!(
        msg.contains("SHARPEN"),
        "a same-type match must still offer sharpening — that route is the whole point here: \
         {msg}"
    );
    assert!(
        !msg.to_lowercase().contains("different kind of record"),
        "a same-type match must NOT be described as a different kind of record: {msg}"
    );
}
