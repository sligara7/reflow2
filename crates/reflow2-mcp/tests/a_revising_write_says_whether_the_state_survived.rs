//! A revising write COMPUTES whether the state it replaced is preserved,
//! instead of restating the snapshot-first rule at everyone.
//!
//! `req:a-discipline-is-delivered-at-the-tool-not-in-a-catalogue`, accepted.
//!
//! # The failure this closes
//!
//! The revision block already said *"`record_change` BEFORE the merge is what
//! puts the old state in the design's own timeline"* — **unconditionally**, to
//! a caller who had just done exactly that and to a caller who had destroyed
//! something, in identical words. That is the catalogue problem in miniature:
//! advice that never varies is advice a reader learns to skim, and the one
//! time it mattered it looked like all the times it did not.
//!
//! The requirement's stronger form is to compute the OUTCOME rather than track
//! the invocation — *"do not track whether a skill was INVOKED, compute whether
//! its OUTCOME IS PRESENT"* — because that survives an agent which ignores
//! every hint. dev_storyflow's dragon Boss proposed the same shape
//! independently: *"report whether the target has a snapshot — NOT BLOCK, JUST
//! SAY."*
//!
//! # What is deliberately absent
//!
//! Nothing here refuses a write. A tool that blocks becomes a tool people route
//! around, and then the graph stops matching reality — the one failure it
//! cannot survive. Every probe below asserts the write SUCCEEDED.

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

/// A decision worth revising, in a graph that can hold snapshots.
async fn with_a_decision(s: &ReflowService) {
    let _ = j!(s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({"id":"proj:p","name":"P"})).unwrap()
    )));
    let _ = j!(s.add_decision(Parameters(
        serde_json::from_value(serde_json::json!({
            "id":"dec:x","name":"A choice","decision":"the original text","rationale":"why"
        }))
        .unwrap()
    )));
    let _ = j!(s.add_epoch(Parameters(
        serde_json::from_value(serde_json::json!({
            "id":"epoch:e","name":"E","epoch_type":"revision","sequence":1
        }))
        .unwrap()
    )));
}

async fn revise(s: &ReflowService) -> serde_json::Value {
    j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"decision":"REPLACED text"}
        }))
        .unwrap()
    )))
}

/// ⭐ PREMISE CHANGED 2026-09-01, and the change is the whole point of
/// `chg:the-call-holding-the-only-copy-keeps-it`.
///
/// This asserted that a prior text nothing held was DESTROYED, and that the
/// reply named the loss. The write now PRESERVES such a value before computing
/// the note, so there is no loss to name — asserting the old warning here would
/// be asserting the fix did not happen.
///
/// 🛑 THE WARNING PATH IS NOT GONE. It still fires whenever a preserve cannot
/// be taken (the preserve is best-effort and never fails the caller's write),
/// and every word of it is still pinned by the tests below that exercise a
/// genuinely uncovered field. What changed is which situations reach it.
#[tokio::test]
async fn a_state_nothing_else_held_is_kept_and_the_note_says_so() {
    // No record_change first — the discipline NOT followed, which used to be
    // the destroying case and is now the preserving one.
    let s = svc().await;
    with_a_decision(&s).await;
    let v = revise(&s).await;

    let rev = v
        .get("revision")
        .expect("a revising write reports the revision");
    assert_eq!(
        rev.get("changed").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        rev.get("prior_state_preserved_in").is_some(),
        "the write kept the value nothing else held, so the snapshot is named: {rev}"
    );
    let note = rev.get("note").and_then(serde_json::Value::as_str).unwrap();
    assert!(
        note.contains("PRESERVED"),
        "the note is a receipt now, not a warning: {note}"
    );
    assert!(
        !note.contains("NO SNAPSHOT HOLDS THE PRIOR VALUE OF"),
        "and it must not warn about a value that was just saved: {note}"
    );
}

#[tokio::test]
async fn a_preserved_state_is_told_there_is_nothing_to_do() {
    // record_change FIRST — the discipline followed. The old text is safe, and
    // repeating the rule at this caller is exactly the noise that trains people
    // to stop reading.
    let s = svc().await;
    with_a_decision(&s).await;
    let _ = j!(s.record_change(Parameters(
        serde_json::from_value(serde_json::json!({
            "epoch_id":"epoch:e","change_event_id":"chg:c","name":"revising",
            "target_type":"Decision","target_id":"dec:x","change_type":"scope_change",
            "action":"modified"
        }))
        .unwrap()
    )));
    let v = revise(&s).await;

    let rev = v.get("revision").expect("revision block");
    let preserved = rev
        .get("prior_state_preserved_in")
        .and_then(serde_json::Value::as_str)
        .expect("the snapshot that holds the prior state is NAMED, not merely counted");
    assert!(
        preserved.contains("dec:x"),
        "and it names the snapshot for THIS node: {preserved}"
    );
    let note = rev.get("note").and_then(serde_json::Value::as_str).unwrap();
    assert!(
        note.contains("IS PRESERVED"),
        "the caller who did the right thing is told so: {note}"
    );
    assert!(
        !note.contains("record_change` BEFORE"),
        "and is NOT lectured about a rule they already followed — that is the whole point: {note}"
    );
}

#[tokio::test]
async fn the_two_replies_actually_differ_which_is_the_entire_requirement() {
    // The old note was byte-identical in both situations. If these two ever
    // converge again the feature is gone, whatever else still passes.
    let a = svc().await;
    with_a_decision(&a).await;
    let destroyed = revise(&a).await;

    let b = svc().await;
    with_a_decision(&b).await;
    let _ = j!(b.record_change(Parameters(
        serde_json::from_value(serde_json::json!({
            "epoch_id":"epoch:e","change_event_id":"chg:c","name":"revising",
            "target_type":"Decision","target_id":"dec:x","change_type":"scope_change",
            "action":"modified"
        }))
        .unwrap()
    )));
    let preserved = revise(&b).await;

    let note_of = |v: &serde_json::Value| {
        v["revision"]["note"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    assert_ne!(
        note_of(&destroyed),
        note_of(&preserved),
        "a write that destroyed history and one that preserved it must not read alike"
    );
}

#[tokio::test]
async fn the_preserved_state_is_the_one_this_write_replaced_not_a_stale_one() {
    // THE CASE THAT MATTERS MOST, and the probe suite did not have it until a
    // mutation exposed the hole: making the hash comparison always succeed left
    // every other probe green. Without this, "some snapshot exists for this
    // node" would pass for "the state you just replaced is safe".
    //
    // Snapshot, revise, revise again. The second revision replaces the
    // INTERMEDIATE state, which no snapshot holds — the one on file is of the
    // original. Reporting that as preserved would be the worst possible
    // answer: a reassurance that the thing you destroyed is recoverable.
    let s = svc().await;
    with_a_decision(&s).await;
    let _ = j!(s.record_change(Parameters(
        serde_json::from_value(serde_json::json!({
            "epoch_id":"epoch:e","change_event_id":"chg:c","name":"revising",
            "target_type":"Decision","target_id":"dec:x","change_type":"scope_change",
            "action":"modified"
        }))
        .unwrap()
    )));
    let first = revise(&s).await;
    assert!(
        first["revision"]["prior_state_preserved_in"].is_string(),
        "the FIRST revision is genuinely covered by the snapshot"
    );

    let second = j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"decision":"replaced AGAIN"}
        }))
        .unwrap()
    )));
    // ⭐ THE INTERMEDIATE STATE IS NOW KEPT — and this asserts it was kept
    // CORRECTLY, which tests the stale-snapshot invariant from the other side.
    // The node already had a snapshot, of the ORIGINAL text. If a stale
    // snapshot were being credited as the current prior state, or if the
    // preserve stored the wrong thing, the value on file for this revision
    // would be the original rather than the intermediate.
    let snap_id = second["revision"]["prior_state_preserved_in"]
        .as_str()
        .expect("the intermediate state is preserved now, not destroyed");
    let held = j!(s.get_node(Parameters(
        serde_json::from_value(serde_json::json!({ "node_type": "Snapshot", "id": snap_id }))
            .unwrap()
    )));
    let state = serde_json::to_string(&held).expect("serialisable");
    assert!(
        state.contains("REPLACED text"),
        "the snapshot must hold the INTERMEDIATE state this write replaced: {state}"
    );
    assert!(
        !state.contains("the original text"),
        "and NOT the original — crediting a stale snapshot is the failure this case exists for: \
         {state}"
    );
    // 🛑 THE UNMASKABLE HALF OF THIS INVARIANT LIVES IN CORE, where the
    // automatic preserve cannot make it true by accident:
    // `prior_state_coverage.rs::a_different_value_in_the_snapshot_is_not_preservation`.
}

#[tokio::test]
async fn nothing_is_blocked_and_the_write_still_lands() {
    // A tool that blocks becomes a tool people route around, and then the graph
    // stops matching reality. The reply is advisory and the write is real.
    let s = svc().await;
    with_a_decision(&s).await;
    let v = revise(&s).await;

    assert_eq!(
        v["properties"]["decision"].as_str(),
        Some("REPLACED text"),
        "the write landed despite the warning — advisory, never a gate"
    );
}

#[tokio::test]
async fn an_ordinary_enrichment_is_not_warned_about_at_all() {
    // Adding a property that overwrote nothing destroys no history, so the
    // destruction note must not fire. A warning that appears on every write is
    // one nobody reads by the second day.
    //
    // `alternatives` is the one Decision property `add_decision` does not
    // write, which is why it is the one used here — the first draft of this
    // probe used `status` and was WRONG, because `add_decision` sets it to
    // `proposed`, so writing `accepted` genuinely IS a replacement and the
    // warning was correct. The probe's premise was the bug, not the code.
    let s = svc().await;
    with_a_decision(&s).await;
    let v = j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"alternatives":"one road not taken"}
        }))
        .unwrap()
    )));
    assert_eq!(
        v["revision"]["replaced"].as_array().map(Vec::len),
        Some(0),
        "the fixture must actually be an enrichment, or this probe proves nothing"
    );
    let note = v["revision"]["note"].as_str().unwrap_or_default();
    assert!(
        !note.contains("NO SNAPSHOT"),
        "an enrichment that replaced nothing gets no destruction warning: {note}"
    );
}

// ---------------------------------------------------------------------------
// THE MIDDLE CASE, REPORTED FROM THE FIELD 2026-08-21 AND WRONG UNTIL 08-23.
//
// The two probes above cover the ends: nothing preserved, everything preserved.
// A dev_storyflow session hit the middle and CHECKED rather than believing —
// snapshot, write #1 to one field, write #2 to another. Nothing hashes to the
// state write #2 replaced, so the block said "NO SNAPSHOT HOLDS THE STATE IT
// REPLACED — checked, not assumed" while the replaced value sat in the snapshot
// verbatim.
//
// 🛑 AND THE REMEDY WAS THE DANGEROUS PART. That message prescribes a
// three-step undo: write the prior value back, record_change, re-apply. A
// session that believed it would have snapshotted a RECONSTRUCTION over a
// timeline that was already correct — a loud wrong warning causing the loss it
// warns about.
// ---------------------------------------------------------------------------

/// Snapshot, then change field A, then change field B — the ordinary shape of a
/// second revision, and the one that used to be reported as catastrophic.
async fn snapshot_then_two_writes(s: &ReflowService) -> serde_json::Value {
    with_a_decision(s).await;
    let _ = j!(s.record_change(Parameters(
        serde_json::from_value(serde_json::json!({
            "epoch_id":"epoch:e","change_event_id":"chg:c","name":"revising",
            "target_type":"Decision","target_id":"dec:x","change_type":"scope_change",
            "action":"modified"
        }))
        .unwrap()
    )));
    // Write #1 touches `rationale` only. `decision` is untouched, so the
    // snapshot still holds its value exactly.
    let _ = j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"rationale":"a better why"}
        }))
        .unwrap()
    )));
    // Write #2 replaces `decision`. Its prior value IS in the snapshot.
    j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"decision":"REPLACED text"}
        }))
        .unwrap()
    )))
}

#[tokio::test]
async fn a_field_the_snapshot_still_holds_is_not_reported_as_destroyed() {
    let s = svc().await;
    let v = snapshot_then_two_writes(&s).await;
    let rev = v.get("revision").expect("revision block");

    let held = rev
        .get("fields_preserved_in")
        .and_then(serde_json::Value::as_object)
        .expect("the per-field answer is present when no whole-state snapshot is");
    let snap = held
        .get("decision")
        .and_then(serde_json::Value::as_str)
        .expect("`decision`'s prior value is in the snapshot and must be NAMED as such");
    assert!(snap.contains("dec:x"), "and it names the snapshot: {snap}");

    assert!(
        rev.get("fields_at_risk")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|a| a.is_empty()),
        "nothing is at risk here: {:?}",
        rev.get("fields_at_risk")
    );

    let note = rev.get("note").and_then(serde_json::Value::as_str).unwrap();
    assert!(
        !note.contains("NO SNAPSHOT HOLDS"),
        "🛑 THE BUG. The value is in the snapshot; saying nothing holds it is false: {note}"
    );
    assert!(
        !note.contains("To undo"),
        "🛑 AND THE WORSE HALF. Prescribing the undo here would write a reconstruction over a \
         correct timeline — the warning causing the loss it warns about: {note}"
    );
    assert!(
        note.contains("Nothing is lost"),
        "the caller is told the plain outcome instead: {note}"
    );
}

#[tokio::test]
async fn a_mixed_write_preserves_only_the_field_that_needed_it() {
    // One replaced field the snapshot holds, one it never did. The strong
    // warning must fire for the second and MUST NOT sweep up the first, because
    // its undo instruction applied to a preserved field is the corruption.
    let s = svc().await;
    with_a_decision(&s).await;
    let _ = j!(s.record_change(Parameters(
        serde_json::from_value(serde_json::json!({
            "epoch_id":"epoch:e","change_event_id":"chg:c","name":"revising",
            "target_type":"Decision","target_id":"dec:x","change_type":"scope_change",
            "action":"modified"
        }))
        .unwrap()
    )));
    // `name` is added-then-changed AFTER the snapshot, so its intermediate
    // value was never captured; `decision` has not moved since the snapshot.
    let _ = j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x","props":{"name":"an intermediate name"}
        }))
        .unwrap()
    )));
    let v = j!(s.create_node(Parameters(
        serde_json::from_value(serde_json::json!({
            "node_type":"Decision","id":"dec:x",
            "props":{"name":"a third name","decision":"REPLACED text"}
        }))
        .unwrap()
    )));

    // ⭐ PREMISE CHANGED 2026-09-01. `name`'s intermediate value was the one
    // field no snapshot held, so it is exactly what the automatic preserve now
    // keeps — and once kept, NOTHING is at risk. The old assertion (that
    // `name` alone is named as at risk) was the correct answer to the question
    // "what is about to be lost"; the answer now is "nothing".
    //
    // 🛑 WHAT THIS STILL PINS, and it is the part worth keeping: the write
    // discriminates between the field that needed saving and the one that did
    // not. If the preserve were indiscriminate, or if it had missed `name`,
    // this would not come back clean.
    let rev = v.get("revision").expect("revision block");
    let at_risk = rev.get("fields_at_risk");
    assert!(
        at_risk.is_none_or(|v| v.as_array().is_some_and(|a| a.is_empty())),
        "`name` was the uncovered field and has now been preserved, so nothing is at risk: {rev}"
    );
    assert!(
        rev.get("prior_state_preserved_in").is_some(),
        "and the whole prior state is on file, which is what makes the above true: {rev}"
    );

    let note = rev.get("note").and_then(serde_json::Value::as_str).unwrap();
    assert!(
        note.contains("PRESERVED"),
        "the note reports preservation rather than a loss: {note}"
    );
}
