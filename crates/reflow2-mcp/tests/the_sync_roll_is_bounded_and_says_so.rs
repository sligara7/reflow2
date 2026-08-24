//! A sync roll opens a bounded number of records, freshest first, and names
//! the ones it did not open.
//!
//! # What this pins
//!
//! `sync_debt` opened EVERY record this seat had ever synced with, and that
//! list only grows. Measured on reflow2's own graph, 2026-08-24: **16 targets
//! totalling 102 MB** — the committed export, a backup, and **fourteen one-off
//! probe dumps written by past sessions**, three of them belonging to a
//! different project. Every `loop_status` re-read and re-parsed all of it.
//!
//! | call | before | after |
//! |---|---|---|
//! | `sync_status` | 28.3s | ~2.8s |
//! | `loop_status` | 40.5s | ~14.3s |
//!
//! `cap:loop-status` promises ONE CHEAP CALL and every session is instructed to
//! run it.
//!
//! # The rule that was tried first, and why it was wrong
//!
//! The first attempt refused to track anything under the OS temp directory, on
//! the reasoning that scratch is not a SHARED record. **Fifteen tests in
//! `the_record_moved_and_the_session_is_told` failed, and they were right to.**
//! A hermetic test puts a genuine shared record in a temp dir; so does a CI
//! workspace and so does a container. One of those tests is named *"the case
//! the whole thing exists for — your brother pushed, you pulled"*. The defect
//! was never WHERE the files live: it is that the list is unbounded.
//!
//! So the bound is on COUNT, ordered by the target file's own mtime, and what
//! falls off the end is reported rather than dropped.

use reflow2_mcp::sync_debt::{not_checked, sync_debt};

fn seed(dir: &std::path::Path, name: &str, hash: &str, graph_path: &str) -> std::path::PathBuf {
    let f = dir.join(name);
    std::fs::write(&f, r#"{"graph_id":"g","nodes":[],"edges":[]}"#).expect("write record");
    reflow2_core::provenance::record_sync(graph_path, &f.display().to_string(), hash);
    f
}

fn workspace(tag: &str) -> (std::path::PathBuf, String) {
    let mut d = std::env::temp_dir();
    d.push(format!("reflow2-syncroll-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("workspace");
    let gp = d.join("graph").display().to_string();
    // A temp workspace is DELIBERATE here: it is the shape a hermetic test, a
    // CI runner and a container all have, and the rule must not care.
    (d, gp)
}

#[test]
fn a_handful_of_records_is_all_checked_and_nothing_is_withheld() {
    let (dir, gp) = workspace("small");
    for i in 0..3 {
        seed(&dir, &format!("rec{i}.json"), "sha256:a", &gp);
    }

    let found = sync_debt(&gp, 0, &|| None);
    assert_eq!(
        found.len(),
        3,
        "an ordinary seat is under the bound and every record is opened"
    );
    assert!(
        not_checked(&gp).is_none(),
        "and nothing is reported as withheld, because nothing was"
    );
}

#[test]
fn a_seat_that_accumulated_records_opens_a_bounded_number_and_names_the_rest() {
    let (dir, gp) = workspace("many");
    for i in 0..16 {
        seed(&dir, &format!("rec{i:02}.json"), "sha256:a", &gp);
    }

    let found = sync_debt(&gp, 0, &|| None);
    assert!(
        found.len() < 16,
        "16 records is the measured real case; opening all of them is the cost \
         this bound exists to stop. opened: {}",
        found.len()
    );

    let skipped = not_checked(&gp).expect("what was not opened must be reported");
    assert_eq!(
        found.len() + skipped.count,
        16,
        "every record is either opened or NAMED — a roll that quietly checks \
         fewer than the seat knows about reads as all-clear while the one that \
         moved sits unopened"
    );
    assert!(
        !skipped.paths.is_empty() && skipped.note.contains("most recently modified"),
        "and the note says how to bring one back to the front"
    );
}

/// The ordering is the whole reason a bound is safe: the record somebody is
/// collaborating on is the one that moved recently.
#[test]
fn the_freshest_records_are_the_ones_opened() {
    let (dir, gp) = workspace("order");
    // Ten stale records...
    for i in 0..10 {
        let f = seed(&dir, &format!("old{i:02}.json"), "sha256:a", &gp);
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let _ = filetime_set(&f, old);
    }
    // ...and one that just moved, which is the one that matters.
    let live = seed(&dir, "live.json", "sha256:a", &gp);

    let found = sync_debt(&gp, 0, &|| None);
    let opened: Vec<&str> = found.iter().map(|d| d.path.as_str()).collect();
    assert!(
        opened.iter().any(|p| p.ends_with("live.json")),
        "the record that moved most recently must be opened, or the bound would \
         hide exactly the case sync exists for. opened: {opened:?}"
    );
    let _ = live;
}

/// Set mtime without pulling in a dependency: rewrite is not enough (it would
/// make the file NEWER), so use `utimensat` through std where available.
fn filetime_set(path: &std::path::Path, when: std::time::SystemTime) -> std::io::Result<()> {
    let f = std::fs::File::options().write(true).open(path)?;
    f.set_times(std::fs::FileTimes::new().set_modified(when))
}
