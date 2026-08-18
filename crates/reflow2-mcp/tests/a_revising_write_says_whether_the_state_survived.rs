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

fn req(v: serde_json::Value) -> impl serde::de::DeserializeOwned {
    serde_json::from_value::<serde_json::Value>(v).unwrap()
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

#[tokio::test]
async fn a_destroyed_state_is_named_as_destroyed() {
    // No record_change first: the prior text exists nowhere afterwards, and the
    // reply has to say that rather than offer general advice.
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
        rev.get("prior_state_preserved_in").is_none(),
        "nothing preserved it, so the field is absent rather than null — its PRESENCE is the signal"
    );
    let note = rev.get("note").and_then(serde_json::Value::as_str).unwrap();
    assert!(
        note.contains("NO SNAPSHOT HOLDS THE STATE IT REPLACED"),
        "the reply must state the fact, not the rule: {note}"
    );
    assert!(
        note.contains("checked, not assumed"),
        "and must say it LOOKED, or a reader cannot tell this from boilerplate: {note}"
    );
    assert!(
        note.contains("To undo"),
        "a finding with no remedy is a scolding — rule 4: {note}"
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
async fn a_snapshot_of_a_different_state_does_not_count_as_preservation() {
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
    assert!(
        second["revision"]["prior_state_preserved_in"].is_null()
            || second["revision"].get("prior_state_preserved_in").is_none(),
        "the intermediate state is in no snapshot, and a stale snapshot for the same node \
         must not be mistaken for it: {}",
        second["revision"]
    );
    let note = second["revision"]["note"].as_str().unwrap_or_default();
    assert!(
        note.contains("NO SNAPSHOT HOLDS THE STATE IT REPLACED"),
        "and the reply says so plainly: {note}"
    );
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
