//! The session learns the shared record moved — at the START, not at the write.
//!
//! # ⭐ WRITTEN BEFORE THE IMPLEMENTATION
//!
//! These cases were written from Anthony's chosen option (A) before
//! `sync_debt.rs` existed. His framing, 2026-08-11: *"It sounds like we have
//! the tool, just not used well or surfaced."*
//!
//! # The finding this is built on
//!
//! `last_synced` — the per-(graph, file) content hash this seat believes is on
//! disk — is **WRITTEN by two paths and READ by one**. `record_sync` is called
//! by `export_graph` and by `import_graph`; `last_synced()` is read inside
//! `export_graph` and nowhere else.
//!
//! So reflow2 can already tell, at the first moment of a session, that
//! somebody else's work has landed in the record. **It knows at the first
//! moment and speaks at the last** — when you try to export, hours later.
//! That is the same family as `dec:one-retire-edge`'s "a marker nothing reads
//! is a comment", except here something does read it, at the wrong moment.
//!
//! # What option A is, and what it is deliberately NOT
//!
//! **Speak, gated on the hash — never act.** The gate is the whole design:
//! silent whenever the file has not moved, which is the entirety of ordinary
//! solo work, so it fires rarely and is therefore read.
//!
//! - NOT auto-import (option B): import is an UPSERT, so an unasked one
//!   silently overwrites live session work. `dec:ask-not-repair`.
//! - NOT a refusal (option C): a refusal on READ is heavy-handed, and a
//!   session deliberately working from an older design would meet it every
//!   time. These findings carry a remedy, never a block.

use std::io::Write;

use reflow2_core::{DesignGraph, GraphExport};
use reflow2_mcp::sync_debt::{SyncDebt, SyncState, sync_debt};

/// A fresh scratch dir plus a graph-store path inside it. The sync record is a
/// sibling file of the store, so both need a home. Follows the repo's existing
/// convention (`latent_mode.rs`, `design_identity.rs`) rather than adding a
/// `tempfile` dependency for eleven tests.
struct Scratch {
    dir: std::path::PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "reflow2-syncdebt-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self { dir }
    }
    fn path(&self) -> &std::path::Path {
        &self.dir
    }
    fn graph_path(&self) -> String {
        self.dir.join("graph").display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn scratch(tag: &str) -> (Scratch, String) {
    let s = Scratch::new(tag);
    let gp = s.graph_path();
    (s, gp)
}

/// A design holding exactly the named capabilities, as an export document.
fn design(ids: &[&str]) -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    for id in ids {
        g.add_capability(id, id, "does a thing", Some("realized"))
            .unwrap();
    }
    g.export_graph().unwrap()
}

/// Write an export to `path` the way `export_graph` writes it.
fn put(path: &std::path::Path, export: &GraphExport) {
    let v = serde_json::to_value(export).unwrap();
    let mut f = std::fs::File::create(path).unwrap();
    writeln!(f, "{}", serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// Say this seat is in step with whatever is currently at `path`.
fn mark_synced(graph_path: &str, path: &std::path::Path, export: &GraphExport) {
    reflow2_core::provenance::record_sync(
        graph_path,
        &path.display().to_string(),
        &export.effective_content_hash(),
    );
}

fn behind(findings: &[SyncDebt]) -> Vec<&SyncDebt> {
    findings.iter().filter(|f| f.is_actionable()).collect()
}

// THE CASE THE WHOLE THING EXISTS FOR. Your brother pushed, you pulled, and the
// file now holds a brainstormed idea your graph has never seen.
#[test]
fn a_record_holding_work_this_graph_lacks_is_reported() {
    let (dir, gp) = scratch("behind");
    let file = dir.path().join("reflow2.json");

    let mine = design(&["cap:mine"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);

    // Somebody else's work arrives in the file.
    put(&file, &design(&["cap:mine", "cap:theirs"]));

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    let actionable = behind(&found);
    assert_eq!(
        actionable.len(),
        1,
        "the moved record must be reported: {found:?}"
    );
    assert!(
        actionable[0]
            .nodes_not_here
            .contains(&"cap:theirs".to_string()),
        "and it must NAME what arrived: {:?}",
        actionable[0]
    );
}

// THE GATE, AND THE REASON THIS IS USABLE AT ALL. The overwhelmingly common
// case is one person working alone: the file has not moved, so every difference
// is their own unexported work and NOTHING is said. A check that spoke here
// would fire on every session and be ignored within a week.
#[test]
fn my_own_unexported_work_is_never_reported() {
    let (dir, gp) = scratch("solo");
    let file = dir.path().join("reflow2.json");

    let exported = design(&["cap:one"]);
    put(&file, &exported);
    mark_synced(&gp, &file, &exported);

    // I keep working. My graph is now AHEAD of the file — the normal state of
    // every working session.
    let mine_now = design(&["cap:one", "cap:two", "cap:three"]);

    let found = sync_debt(&gp, 0, &|| Some(mine_now.clone()));
    assert!(
        behind(&found).is_empty(),
        "being ahead of the record is ordinary and must be silent: {found:?}"
    );
}

// The file moved — somebody else exported — but everything it holds is already
// here. Worth knowing, not worth acting on, so it must not be actionable.
#[test]
fn a_record_that_moved_but_holds_nothing_new_is_not_actionable() {
    let (dir, gp) = scratch("superset");
    let file = dir.path().join("reflow2.json");

    // Sync against a two-capability record...
    let base = design(&["cap:one", "cap:two"]);
    put(&file, &base);
    mark_synced(&gp, &file, &base);

    // ...then the file genuinely MOVES to different content that I nonetheless
    // wholly contain. Writing the same bytes back would leave the hash equal
    // and take the in_step path, which would pass this test for the wrong
    // reason — so the record must really differ from what was synced.
    put(&file, &design(&["cap:one"]));
    let mine = design(&["cap:one", "cap:two"]);

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    assert_eq!(
        found[0].state, "moved_but_current",
        "precondition: the record must actually have MOVED, or this proves nothing: {found:?}"
    );
    assert!(
        behind(&found).is_empty(),
        "I am a superset — nothing to import: {found:?}"
    );
}

// ⭐ THE COST ARGUMENT, PINNED. Exporting this graph to compare against is the
// expensive half; the hash check is the cheap half. On the in-step path — the
// one every ordinary session takes — the answer is known before any comparison
// is needed, so the export must NEVER be built. Without this test the closure
// is just a style choice; with it, the cheap path is a guarantee.
#[test]
fn the_expensive_export_is_not_built_on_the_ordinary_path() {
    let (dir, gp) = scratch("lazy");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:one"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);

    let built = std::cell::Cell::new(0usize);
    let found = sync_debt(&gp, 0, &|| {
        built.set(built.get() + 1);
        Some(mine.clone())
    });
    assert_eq!(found[0].state, "in_step");
    assert_eq!(
        built.get(),
        0,
        "the in-step path must cost no export at all"
    );

    // And when something HAS moved, it is built — once.
    put(&file, &design(&["cap:one", "cap:theirs"]));
    let built = std::cell::Cell::new(0usize);
    let found = sync_debt(&gp, 0, &|| {
        built.set(built.get() + 1);
        Some(mine.clone())
    });
    assert!(found[0].is_actionable());
    assert_eq!(built.get(), 1, "built exactly once when needed");
}

// 🛑 THE BUG THIS SHIPPED WITH FOR TWENTY MINUTES, caught end-to-end rather
// than by any unit test above. The first cut gated on
// `effective_content_hash()`, which TRUSTS the `content_hash` the document
// states about itself and computes one only when it is absent. So a record
// edited by anything other than `export_graph` — a merge, a hand-fix, another
// tool — keeps its stale stamp, and the check reported "exactly where this
// graph left it" while somebody else's work sat in the file.
//
// That is precisely the case this whole feature exists for, and it read as
// all-clear. A check must never believe a claim made by the thing it is
// checking.
#[test]
fn a_record_edited_without_restamping_is_still_caught() {
    let (dir, gp) = scratch("stale-stamp");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:mine"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);

    // Append work the way a merge or a hand-edit would: content moves, the
    // embedded content_hash does NOT.
    let mut raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let stamp_before = raw["content_hash"].clone();
    raw["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "node_id": "dec:brothers-idea",
            "node_type": "Decision",
            "properties": {
                "name": "OPEN — his brainstorm",
                "decision": "recorded as brainstorming",
                "status": "proposed"
            }
        }));
    std::fs::write(&file, serde_json::to_string_pretty(&raw).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&file).unwrap())
            .unwrap()["content_hash"],
        stamp_before,
        "precondition: the stamp must be UNCHANGED, or this tests nothing"
    );

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    assert!(
        found[0].is_actionable(),
        "content moved, so it must be caught however the file stamps itself: {found:?}"
    );
    assert!(
        found[0]
            .nodes_not_here
            .contains(&"dec:brothers-idea".to_string()),
        "and it must name what arrived: {:?}",
        found[0]
    );
    assert!(
        found[0].stamp_disagrees,
        "and the stale stamp is its own fact, not swallowed: {:?}",
        found[0]
    );
}

// NO SILENT CAPS. A seat that has never synced with anything must come back
// EMPTY rather than clean-looking — "nothing is owed" and "I have nothing to
// check" are different facts and must not share one answer.
#[test]
fn a_seat_that_never_synced_reports_nothing_rather_than_all_clear() {
    let (_dir, gp) = scratch("virgin");
    let found = sync_debt(&gp, 0, &|| Some(design(&["cap:one"])));
    assert!(
        found.is_empty(),
        "never synced with any file means nothing to say: {found:?}"
    );
}

// A target that has VANISHED is its own fact. Silently dropping it would let a
// deleted export read exactly like an in-step one.
#[test]
fn a_record_that_has_disappeared_is_reported_as_missing_not_skipped() {
    let (dir, gp) = scratch("gone");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:one"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);
    std::fs::remove_file(&file).unwrap();

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    assert_eq!(
        found.len(),
        1,
        "the target must still be accounted for: {found:?}"
    );
    assert!(
        found[0].state.contains("missing"),
        "and it must say WHICH fact this is: {:?}",
        found[0]
    );
}

// ⚠️ IT IS A HINT, NOT A REFUSAL — option C was considered and REJECTED.
// The remedy must name the call and the path, because a finding that does not
// say what would fix it is a wall (rule 4, the same rule the stale-seat refusal
// follows).
#[test]
fn the_finding_names_the_remedy_and_never_reads_as_a_refusal() {
    let (dir, gp) = scratch("wording");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:mine"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);
    put(&file, &design(&["cap:mine", "cap:theirs"]));

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    let msg = found[0].message();
    assert!(
        msg.contains("import_graph"),
        "it must name the call that fixes it: {msg}"
    );
    assert!(
        msg.contains(&file.display().to_string()),
        "and the path to call it on: {msg}"
    );
    let lower = msg.to_lowercase();
    for forbidden in ["refused", "error", "cannot", "blocked"] {
        assert!(
            !lower.contains(forbidden),
            "this is a hint, not a refusal — it must not say `{forbidden}`: {msg}"
        );
    }
}

// TWO TARGETS ARE INDEPENDENT. One graph legitimately publishes to more than
// one file (a full export and a published surface), and the sync record is
// keyed by path precisely so they cannot disarm each other's check.
#[test]
fn one_stale_target_does_not_hide_or_imply_the_other() {
    let (dir, gp) = scratch("two");
    let full = dir.path().join("reflow2.json");
    let surface = dir.path().join("surface.json");

    let mine = design(&["cap:mine"]);
    put(&full, &mine);
    put(&surface, &mine);
    mark_synced(&gp, &full, &mine);
    mark_synced(&gp, &surface, &mine);

    // Only ONE of them moves.
    put(&full, &design(&["cap:mine", "cap:theirs"]));

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    let actionable = behind(&found);
    assert_eq!(actionable.len(), 1, "exactly one target moved: {found:?}");
    assert!(
        actionable[0].path.contains("reflow2.json"),
        "and it must be the one that actually moved: {:?}",
        actionable[0]
    );
    assert_eq!(
        found.len(),
        2,
        "both known targets stay accounted for: {found:?}"
    );
}

// A file that is not a reflow2 export at all must not crash the check or be
// read as in-step. Same posture as export_graph, which records "not a reflow2
// export" rather than assuming a chain.
#[test]
fn a_target_that_is_not_an_export_is_reported_rather_than_assumed_fine() {
    let (dir, gp) = scratch("garbage");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:one"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);
    std::fs::write(&file, "this is not json").unwrap();

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].state.contains("unreadable"),
        "an unparseable target is its own fact: {:?}",
        found[0]
    );
}

// The state map is readable on its own, so a caller can distinguish "checked
// three targets, all in step" from "checked nothing".
#[test]
fn every_known_target_is_accounted_for_even_when_all_are_in_step() {
    let (dir, gp) = scratch("accounted");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:one"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);

    let found = sync_debt(&gp, 0, &|| Some(mine.clone()));
    assert_eq!(
        found.len(),
        1,
        "the in-step target is still listed: {found:?}"
    );
    assert!(!found[0].is_actionable());
    assert!(
        found[0].state.contains("in_step"),
        "a quiet answer must still say what it checked: {:?}",
        found[0]
    );
}

// The sync record is machine-local state, and reading it must not require a
// live graph — this is what lets the check run at the very start of a session.
#[test]
fn the_sync_record_is_readable_without_opening_the_graph() {
    let (dir, gp) = scratch("standalone");
    let file = dir.path().join("reflow2.json");
    let mine = design(&["cap:one"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);

    let state: SyncState = reflow2_core::provenance::read_sync_state(&gp);
    let _ = &state;
    assert_eq!(state.last_synced.len(), 1);
}

// ---------------------------------------------------------------------------
// The finding rides on the LOOP HINT, not only on loop_status
//
// `dec:idea-feedback-arrives-by-git-push-and-pull` option D, Anthony's word
// 2026-08-13. He asked whether his brother could clone, write feedback into the
// graph, push, and have it arrive on the next pull. That is the design's
// intended path — and of its four steps, the pull-side import is the only LOUD
// one. It was loud only inside `loop_status`, a call nobody makes on the way
// past, so a session learned a colleague's work had landed only if it thought
// to ask.
//
// Still NOT an auto-import. The hint SAYS and names the remedy; taking it in
// stays a conscious act (`dec:ask-not-repair`).
// ---------------------------------------------------------------------------

use reflow2_mcp::service::ReflowService;

/// A service over a real store, plus that store's path. `in_memory` will not do:
/// the finding is gated on a `graph_path`, which an in-memory service has not
/// got — and a test that passed without one would be asserting nothing.
fn service_on_disk(tag: &str) -> (Scratch, ReflowService, String) {
    let (scratch, graph_path) = scratch(tag);
    let svc = ReflowService::new(&graph_path).expect("service over a real store");
    (scratch, svc, graph_path)
}

/// Drive an ordinary orientation READ and hand back whatever hint a client
/// would have seen. Deliberately through a real tool rather than the internal:
/// what matters is that the finding reaches somebody who never asked for it.
async fn hint_from_a_read(svc: &ReflowService) -> Option<String> {
    let result = svc.graph_report().await.expect("tool ok");
    let out = result
        .structured_content
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .expect("structured content present");
    out.get("loop_hint")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[tokio::test]
async fn a_moved_record_reaches_an_ordinary_read_without_anyone_calling_loop_status() {
    let (scratch, svc, graph_path) = service_on_disk("hint-behind");
    let record = scratch.path().join("record.json");

    // This seat has seen the record, and the record then gained work.
    let seen = design(&["req:mine"]);
    put(&record, &seen);
    mark_synced(&graph_path, &record, &seen);
    put(&record, &design(&["req:mine", "req:theirs"]));

    let hint = hint_from_a_read(&svc)
        .await
        .expect("a record holding work this graph lacks must reach an ordinary read");

    assert!(
        hint.contains("import_graph"),
        "the hint must name the remedy, not merely report a state: {hint}"
    );
}

#[tokio::test]
async fn an_untouched_record_says_nothing_which_is_the_whole_gate() {
    // The gate IS the design: silent whenever the file has not moved, which is
    // the entirety of ordinary solo work. A hint that fired every read would be
    // read by nobody.
    let (scratch, svc, graph_path) = service_on_disk("hint-in-step");
    let record = scratch.path().join("record.json");
    let seen = design(&["req:mine"]);
    put(&record, &seen);
    mark_synced(&graph_path, &record, &seen);

    let hint = hint_from_a_read(&svc).await;
    assert!(
        hint.as_deref().is_none_or(|h| !h.contains("import_graph")),
        "nothing moved, so nothing about the record may be said: {hint:?}"
    );
}

// ---- the OTHER direction: work this seat has not exported ------------------
//
// Everything above asks "has the shared record moved ahead of me". Nothing did
// — or could — ask "am I holding work the record has never seen", and
// `in_step` was read as if it answered both. dragon Boss reported a live node
// absent from the export with sync green (2026-08-16); the same shape was then
// reproduced on reflow2's own graph, where two TemporalFacts sat unexported
// behind an `in_step` that could not go red.
//
// The counts do NOT change what `sync` decides. They make its green readable.

/// THE CASE. The file is exactly where this graph left it — and this graph has
/// grown since. The state is still `in_step`, correctly, and the line now says
/// what the hash cannot.
#[test]
fn unexported_work_is_named_on_the_in_step_line() {
    let (dir, gp) = scratch("unexported");
    let file = dir.path().join("reflow2.json");

    let exported = design(&["cap:one"]);
    put(&file, &exported);
    mark_synced(&gp, &file, &exported);

    // Two nodes live, one in the file: the seat wrote and never exported.
    let found = sync_debt(&gp, 3, &|| Some(exported.clone()));
    assert_eq!(found.len(), 1);
    let d = &found[0];

    assert_eq!(d.state, "in_step", "the file genuinely has not moved");
    assert_eq!(d.live_nodes, 3);
    assert_eq!(d.export_nodes, 1);
    let msg = d.message();
    assert!(
        msg.contains('3') && msg.contains('1') && msg.contains("never been exported"),
        "the line must say how much is unexported, not merely that all is well: {msg}"
    );
}

/// COUNTERWEIGHT, and the one that stops this becoming a different bug: a seat
/// that HAS exported must get the quiet answer. A reading aid that fires on
/// correct work is a false alarm on the path every ordinary session takes, and
/// it would be switched off inside a day.
#[test]
fn a_fully_exported_graph_says_nothing_extra() {
    let (dir, gp) = scratch("exported");
    let file = dir.path().join("reflow2.json");

    let exported = design(&["cap:one", "cap:two"]);
    put(&file, &exported);
    mark_synced(&gp, &file, &exported);

    let live = exported.nodes.len();
    let found = sync_debt(&gp, live, &|| Some(exported.clone()));
    let d = &found[0];

    assert_eq!(d.state, "in_step");
    assert_eq!(d.live_nodes, d.export_nodes);
    let msg = d.message();
    assert!(
        !msg.contains("never been exported"),
        "an in-step, fully-exported record must stay quiet: {msg}"
    );
}

/// COUNTERWEIGHT 2: unexported work is NOT actionable sync debt. `behind` means
/// somebody else's work is in the file and you should import it; exporting your
/// own is a different act with a different remedy, and collapsing the two would
/// tell a session to `import_graph` over its own unsaved work.
#[test]
fn unexported_work_is_not_reported_as_behind() {
    let (dir, gp) = scratch("notbehind");
    let file = dir.path().join("reflow2.json");

    let exported = design(&["cap:one"]);
    put(&file, &exported);
    mark_synced(&gp, &file, &exported);

    let found = sync_debt(&gp, 99, &|| Some(exported.clone()));
    assert!(
        behind(&found).is_empty(),
        "holding unexported work is not the record moving ahead of you: {found:?}"
    );
}

/// COUNTERWEIGHT 3: a seat BEHIND the record still reports that, and still
/// names what arrived. The counts must not shadow the finding that has a
/// remedy.
#[test]
fn the_counts_do_not_mask_a_record_that_moved_ahead() {
    let (dir, gp) = scratch("bothways");
    let file = dir.path().join("reflow2.json");

    let mine = design(&["cap:mine"]);
    put(&file, &mine);
    mark_synced(&gp, &file, &mine);
    put(&file, &design(&["cap:mine", "cap:theirs"]));

    let found = sync_debt(&gp, 50, &|| Some(mine.clone()));
    let actionable = behind(&found);
    assert_eq!(actionable.len(), 1, "{found:?}");
    assert!(
        actionable[0]
            .nodes_not_here
            .contains(&"cap:theirs".to_string()),
        "still names what arrived: {:?}",
        actionable[0]
    );
}

// ---------------------------------------------------------------------------
// AND THE SENTENCE HAS TO REACH THE CALLER.
//
// The two probes above prove `message()` says the right thing. It then reached
// nobody: `loop_status` surfaced `message()` only for `is_actionable()` debts,
// and `in_step` is not actionable, so the unexported-work line was written,
// tested, and filtered out of everything served.
//
// Reported twice from dev_storyflow — 2026-08-16, then 2026-08-22 after a
// session re-exported as a control and got `wrote: "changed"` against a green
// verdict. Reproduced here the same day: `export_nodes: 2897` beside
// `live_nodes: 2899`, verdict `in_step`, read and passed over.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn loop_status_puts_unexported_work_in_the_list_a_session_reads() {
    use rmcp::handler::server::wrapper::Parameters;

    // An ON-DISK service: the sync check is gated on `graph_path`, so an
    // in-memory one has no record to be in step with and this could not fail.
    let dir = Scratch::new("loopstatus-unexported");
    let file = dir.path().join("reflow2.json");
    let svc = reflow2_mcp::service::ReflowService::new(&dir.graph_path()).expect("service");

    // A design, exported — so the seat has a record it is in step with.
    svc.add_project(Parameters(
        serde_json::from_value(serde_json::json!({"id":"proj:p","name":"P"})).unwrap(),
    ))
    .await
    .expect("project");
    svc.export_graph(Parameters(
        serde_json::from_value(serde_json::json!({"path": file.to_str().unwrap()})).unwrap(),
    ))
    .await
    .expect("export");

    // ...and then work that never reached it. This is the ordinary mid-session
    // state, and the one that used to read as entirely clean.
    svc.add_requirement(Parameters(
        serde_json::from_value(
            serde_json::json!({"id":"req:unexported","name":"Late","statement":"must hold"}),
        )
        .unwrap(),
    ))
    .await
    .expect("requirement");

    let v = svc
        .loop_status(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await
        .expect("loop_status")
        .structured_content
        .expect("structured");

    let next = v
        .get("next")
        .and_then(serde_json::Value::as_array)
        .expect("`next` is the list a session reads before standing down")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join(" | ");

    assert!(
        next.contains("never been exported"),
        "🛑 THE BUG. Unexported work must appear in `next`, not only in a `sync` row a reader \
         skims — the sentence existed and reached nobody: {next}"
    );

    // COUNTERWEIGHT: the verdict itself must NOT have moved. Making `in_step`
    // go red here would fire on almost every session mid-flight, which is the
    // noise `ver:the-record-moved-is-surfaced` exists to exclude.
    let state = v["sync"][0]["state"].as_str().unwrap_or_default();
    assert_eq!(
        state, "in_step",
        "the record genuinely has not moved ahead of this seat; only the reading aid was missing"
    );
}

#[tokio::test]
async fn a_fully_exported_graph_adds_nothing_to_next() {
    use rmcp::handler::server::wrapper::Parameters;

    let dir = Scratch::new("loopstatus-exported");
    let file = dir.path().join("reflow2.json");
    let svc = reflow2_mcp::service::ReflowService::new(&dir.graph_path()).expect("service");

    svc.add_project(Parameters(
        serde_json::from_value(serde_json::json!({"id":"proj:p","name":"P"})).unwrap(),
    ))
    .await
    .expect("project");
    svc.export_graph(Parameters(
        serde_json::from_value(serde_json::json!({"path": file.to_str().unwrap()})).unwrap(),
    ))
    .await
    .expect("export");

    let v = svc
        .loop_status(Parameters(
            serde_json::from_value(serde_json::json!({})).unwrap(),
        ))
        .await
        .expect("loop_status")
        .structured_content
        .expect("structured");

    let next = serde_json::to_string(&v["next"]).unwrap_or_default();
    assert!(
        !next.contains("never been exported"),
        "a seat that HAS exported must stay quiet — an aid that fires on correct work is a false \
         alarm on the path every session takes, and would be switched off inside a day: {next}"
    );
    // 🛑 AND NOTHING AT ALL ABOUT THE RECORD, not merely a missing phrase.
    // Checking only for the phrase let a mutation through: surfacing EVERY
    // target's message pushes "X is exactly where this graph left it" into
    // `next` for every in-step record, which is a to-do list entry for work
    // nobody has to do. The quiet message hid the noisy behaviour.
    assert!(
        !next.contains("reflow2.json") && !next.contains("exactly where this graph left it"),
        "`next` is what the loop OWES; an in-step record owes nothing and must not appear in it \
         at all: {next}"
    );
}
