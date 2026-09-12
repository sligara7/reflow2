//! The server keeps the working-tree export current, and never overwrites a
//! hand edit to do it.
//!
//! `req:the-server-keeps-the-working-tree-export-current`. The guarantee lives
//! in the server because the session is the thing that might not come back: the
//! Stop hook that used to carry it fires once and exists in one harness, so a
//! consumer of reflow2 in any other client never saw it.
//!
//! # What these pin, and why each one
//!
//! 1. **A write reaches disk with nobody asking.** The whole feature. Without
//!    it the guarantee is a claim.
//! 2. **A burst is ONE export, not one per write.** The debounce is the reason
//!    this is affordable at all, and a debounce nothing measures is a debounce
//!    that can quietly stop coalescing.
//! 3. **A hand edit is left alone, and the reason is READABLE FROM A TOOL.**
//!    Anthony's call, 2026-09-12. A guarantee that has silently stopped
//!    guaranteeing is worse than none, so the skip has to surface where a
//!    session looks — `loop_status`'s `next`, not a log line.
//! 4. **A read-only server refuses it.** The mode exists so a surface with no
//!    authentication cannot change anything; a task writing files on its behalf
//!    is exactly the exception that would make that stop meaning what it says.
//!
//! # On the sleeps
//!
//! These wait on a real debounce in a real task. They poll to a deadline rather
//! than sleeping a fixed time, so they are slow (a second or two) but not
//! flaky: a pass means the file actually arrived, and a failure means it did
//! not arrive within a window ten times the debounce.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use std::time::{Duration, Instant};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-write-through-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn req(id: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(id.into()),
        statement: Some(format!("Requirement {id} exists so the design moves.")),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
    }
}

/// A real store, because the hand-edit baseline is recorded against the graph
/// path — a memory-backed service has none, and testing the guard without one
/// would be testing a different code path than the one that ships.
fn service_on(dir: &std::path::Path) -> ReflowService {
    ReflowService::new(&dir.join("graph").display().to_string()).expect("service on a real store")
}

/// Wait for `f` to hold, to a deadline. Returns how long it took.
async fn within(limit: Duration, mut f: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if f() {
            return Some(start.elapsed());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn nodes_in(path: &std::path::Path) -> Option<usize> {
    let raw = std::fs::read_to_string(path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(doc.get("nodes")?.as_array()?.len())
}

/// THE FEATURE: a write reaches disk, and nobody called export_graph.
#[tokio::test]
async fn a_write_reaches_the_file_with_nobody_asking() {
    let dir = scratch("writes");
    let file = dir.join("design.json");
    let mut s = service_on(&dir);
    s.start_auto_export(file.display().to_string())
        .expect("write-through starts");

    assert!(!file.exists(), "nothing on disk before the first write");
    j!(s.add_requirement(Parameters(req("req:written-through"))));

    let took = within(Duration::from_secs(20), || file.exists()).await;
    assert!(
        took.is_some(),
        "the write-through never wrote {}",
        file.display()
    );
    let raw = std::fs::read_to_string(&file).expect("readable");
    assert!(
        raw.contains("req:written-through"),
        "the file must hold the design that was just written"
    );
}

/// THE DEBOUNCE: writes ARRIVING OVER TIME still cost one export, not one each.
///
/// ⚠️ THE OBVIOUS VERSION OF THIS TEST DOES NOT WORK, and it passed for three
/// minutes before that was noticed. Ten writes issued back to back coalesce
/// whatever the quiet period is — the task has not been scheduled yet and
/// `notify_one` collapses the pokes — so the test passed with the debounce set
/// to ZERO. It was measuring the scheduler, not the feature.
///
/// Spacing the writes is what makes it discriminate: 300 ms apart, a quiet
/// period of 2 s still coalesces them into one export, while no quiet period at
/// all exports after each one. Verified in both directions before it was
/// trusted.
#[tokio::test]
async fn writes_spread_over_time_still_cost_one_export() {
    let dir = scratch("burst");
    let file = dir.join("design.json");
    let mut s = service_on(&dir);
    s.start_auto_export(file.display().to_string())
        .expect("write-through starts");

    for i in 0..10 {
        j!(s.add_requirement(Parameters(req(&format!("req:burst-{i}")))));
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    assert!(
        within(Duration::from_secs(20), || nodes_in(&file).unwrap_or(0)
            >= 10)
        .await
        .is_some(),
        "all ten writes should reach the file"
    );
    let (_, status) = s.auto_export_status().expect("status");
    assert!(
        status.exports <= 2,
        "ten writes spread over three seconds must coalesce — the write-through reported {} \
         exports, which is one per write",
        status.exports
    );
}

/// THE GUARD: a hand edit is left alone, and a session can READ why.
#[tokio::test]
async fn a_hand_edit_is_left_alone_and_loop_status_says_so() {
    let dir = scratch("handedit");
    let file = dir.join("design.json");
    let mut s = service_on(&dir);
    s.start_auto_export(file.display().to_string())
        .expect("write-through starts");

    j!(s.add_requirement(Parameters(req("req:before-the-edit"))));
    assert!(
        within(Duration::from_secs(20), || file.exists())
            .await
            .is_some(),
        "the first write-through must land before the file is edited"
    );

    // Somebody edits the export by hand — or a merge leaves it mangled.
    let hand_edited = "{\"nodes\": [], \"edges\": [], \"graph_id\": \"hand-edited-by-a-person\"}\n";
    std::fs::write(&file, hand_edited).expect("hand edit");

    j!(s.add_requirement(Parameters(req("req:after-the-edit"))));
    // Long enough that a write-through WOULD have happened.
    tokio::time::sleep(Duration::from_secs(5)).await;

    assert_eq!(
        std::fs::read_to_string(&file).expect("readable"),
        hand_edited,
        "THE POINT: the hand edit must survive untouched"
    );

    let status = j!(s.loop_status(Parameters(LoopScopeReq {
        contributor_id: None,
        since_export: false,
    })));
    let skipped = status
        .get("auto_export")
        .and_then(|a| a.get("skipped"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        skipped.contains("changed since reflow2 last wrote it"),
        "loop_status must carry the reason, got {skipped:?}"
    );
    let next = status
        .get("next")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        next.iter().any(|n| n
            .as_str()
            .unwrap_or_default()
            .contains("NO LONGER BEING KEPT CURRENT")),
        "a guarantee that has stopped must be in `next`, where a session acts — got {next:?}"
    );
}

/// A read-only server refuses the write-through rather than quietly taking it.
#[tokio::test]
async fn a_read_only_server_refuses_to_write_through() {
    let dir = scratch("readonly");
    let mut s = service_on(&dir).into_read_only();
    let refusal = s
        .start_auto_export(dir.join("design.json").display().to_string())
        .expect_err("a read-only server must refuse");
    assert!(
        refusal.contains("read-only"),
        "the refusal must name the reason, got {refusal:?}"
    );
    assert!(
        s.auto_export_status().is_none(),
        "a refused write-through must not report a status"
    );
}
