//! The call holding the only copy of a replaced value keeps it.
//!
//! # The report
//!
//! musicjug, 2026-08-31 (`art:musicjug-genesis-feedback-2026-08-31`, idea 2).
//! A revising write answers, correctly, that it holds THE ONLY COPY IN
//! EXISTENCE of the field it just replaced — and then offers an undo recipe:
//!
//! > Snapshot automatically (or offer `snapshot_prior: true`) and the warning
//! > becomes a receipt rather than a regret.
//!
//! It is emitted by the one caller that could still save the value, which is
//! what makes the warning-only version unsatisfying. And the order it
//! prescribes — `record_change` BEFORE the edit — is not reliably achievable
//! from a harness that emits parallel tool batches, so resting the guarantee on
//! call ORDER rests it on something an agent host can break without either
//! party choosing to.
//!
//! ⭐ IT HAPPENED TO THE SESSION THAT RECORDED IT, twice, on 2026-08-31 and
//! again on 2026-09-01 — and the second time it really did drop two paragraphs
//! of a decision, recoverable only because the prior value was echoed in the
//! tool reply and the session was still reading it.
//!
//! # 🛑 The scope, which is the whole design
//!
//! Anthony, 2026-09-01, agreeing with the stated counter-argument: snapshot
//! **only when it would actually lose something**, not on every write.
//! Snapshotting every revising write would grow the graph for no gain — most
//! revisions replace nothing, or replace something already preserved.
//!
//! The condition was already computed and already reported: `fields_at_risk` is
//! non-empty exactly when a field is being replaced and NO snapshot holds its
//! prior value. That is the trigger, so this adds no new judgement — it acts on
//! one the tool was already making and only telling you about afterwards.
//!
//! # And it is best-effort, deliberately
//!
//! A preserve that fails must never fail the caller's write. The write is what
//! was asked for; the preserve is an improvement on top of it. If it cannot be
//! taken the note reports the loss exactly as it did before, which is strictly
//! no worse than the old behaviour — so this can only ever add safety, never a
//! new way to be refused.

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

const FIRST: &str = "The outdoor unit sends cumulative totals rather than deltas, so a lost \
                     reading heals itself and a dropped packet costs nothing at all.";
const SECOND: &str = "The outdoor unit sends deltas with a sequence number, and the receiver \
                      asks again for any gap it notices in that sequence.";

fn requirement(statement: &str) -> RequirementReq {
    RequirementReq {
        id: "req:reading-transport".into(),
        name: Some("How a reading reaches the store".into()),
        statement: Some(statement.into()),
        distinct_from: None,
    }
}

/// ⭐ THE REPORTED CASE. Replace a field nothing else holds, and the value
/// survives — with the reply saying so rather than warning about it.
#[tokio::test]
async fn a_field_nothing_else_holds_is_preserved_before_it_is_replaced() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(FIRST)))
        .await
        .expect("the requirement lands");

    let out = j!(s.add_requirement(Parameters(requirement(SECOND))));
    let body = serde_json::to_string(&out).expect("serialisable");

    assert!(
        !body.contains("NO SNAPSHOT HOLDS"),
        "the value was preserved, so the reply must not still warn that nothing holds it: {body}"
    );
    assert!(
        body.contains("PRESERVED"),
        "the reply must say the replaced state survived — that is the difference between a \
         receipt and a regret: {body}"
    );
    assert!(
        body.contains("cumulative totals"),
        "and the preserved state must be findable, not merely claimed: {body}"
    );
}

/// 🛑 THE COUNTERWEIGHT, and it is what "scoped" means. A write that replaces
/// nothing must take no snapshot. If this fails, the change stopped being
/// "keep it when it would be lost" and became "snapshot everything", which is
/// the cost Anthony's condition exists to avoid.
#[tokio::test]
async fn a_write_that_replaces_nothing_preserves_nothing() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(FIRST)))
        .await
        .expect("the requirement lands");

    // Same content, second time: nothing moves.
    let out = j!(s.add_requirement(Parameters(requirement(FIRST))));
    let body = serde_json::to_string(&out).expect("serialisable");

    assert!(
        body.contains("nothing moved"),
        "an identical re-write replaces nothing and must say so: {body}"
    );
    assert!(
        !body.contains("PRESERVED"),
        "and it must not claim to have preserved anything, because there was nothing at risk: \
         {body}"
    );
}

/// The second half of the same rule: a field ALREADY preserved is not preserved
/// again. Without this, a node revised repeatedly would accumulate a snapshot
/// per write even though the history was already complete — the same
/// snapshot-everything cost arriving by a slower route.
#[tokio::test]
async fn a_field_already_preserved_is_not_preserved_twice() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(FIRST)))
        .await
        .expect("the requirement lands");
    s.add_requirement(Parameters(requirement(SECOND)))
        .await
        .expect("first revision preserves FIRST");

    // Third write, same content as the second: replaces nothing.
    let out = j!(s.add_requirement(Parameters(requirement(SECOND))));
    let body = serde_json::to_string(&out).expect("serialisable");
    assert!(
        body.contains("nothing moved"),
        "re-writing the current value replaces nothing: {body}"
    );
}
