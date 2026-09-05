//! A derived scan runs once per graph state, not once per rollup that reports it.
//!
//! MEASURED 2026-09-05 before this existed: `open_defects()` (~5 s) had ten
//! call sites and no memo. `loop_status`, `graph_report` and `debt_since` each
//! re-ran it to print a COUNT, and the read path ran it again after every
//! write — one orientation pass paid the same scan three times.
//! `dec:derived-scans-are-memoised-per-write-generation` memoises it, keyed on
//! the engine's write generation, which every backend write moves.
//!
//! THIS ASSERTS STRUCTURE, NOT DURATION. The optimize skill is explicit: a
//! duration in a shared suite measures machine contention, and raising the
//! threshold until it passes retires the gate without anyone deciding to. So
//! the invariant pinned is a COUNT — how many times the scan actually ran —
//! which is load-independent and is exactly the thing that was broken.

use reflow2_core::DesignGraph;

fn graph_with_a_defect() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    // Two components that depend on each other: a circular_dependency, so the
    // defect scan has something real to find and the memo something to hold.
    g.add_component("cmp:a", "A", "does a", None).unwrap();
    g.add_component("cmp:b", "B", "does b", None).unwrap();
    g.depends_on("cmp:a", "cmp:b").unwrap();
    g.depends_on("cmp:b", "cmp:a").unwrap();
    g
}

/// THE CASE. Three rollups, no write between them: the defect scan runs ONCE.
#[test]
fn three_rollups_with_no_write_run_the_defect_scan_once() {
    let g = graph_with_a_defect();
    let before = g.derived_recomputes();

    let a = g.open_defects().expect("open_defects");
    let b = g.detect_defects().expect("detect_defects");
    let c = g.loop_status().expect("loop_status");

    assert!(
        !a.is_empty(),
        "the fixture must have a defect to memoise: {a:?}"
    );
    assert_eq!(
        b.defects.len(),
        a.len(),
        "the memoised sweep returns the same defects"
    );
    assert_eq!(
        c.structural_defects,
        a.len(),
        "loop_status counts the same defects"
    );
    // detect_gaps is a second memoised scan; loop_status runs it too. So the
    // ceiling across those three calls is exactly two recomputes: one defect
    // scan, one gap scan. Before the memo it was at least four.
    assert!(
        g.derived_recomputes() - before <= 2,
        "three rollups must share one defect scan and one gap scan, got {} recomputes",
        g.derived_recomputes() - before
    );
}

/// COUNTERWEIGHT 1, and the one that makes a cache safe to have: a WRITE
/// invalidates it, and the next scan sees the new world. A memo that returned
/// a stale count would be worse than no memo at all.
#[test]
fn a_write_invalidates_the_memo_and_the_next_scan_is_fresh() {
    let mut g = graph_with_a_defect();
    let first = g.open_defects().expect("first scan");
    let n0 = g.derived_recomputes();

    // Same generation: a second call is a hit, not a scan.
    let again = g.open_defects().expect("second scan");
    assert_eq!(again.len(), first.len());
    assert_eq!(g.derived_recomputes(), n0, "no write, so no recompute");

    // Break the cycle — a write, through the ordinary edge path — and the
    // defect must disappear from the NEXT scan, which must actually run.
    g.delete_edge("DEPENDS_ON", "cmp:b", "cmp:a")
        .expect("delete edge");
    let after = g.open_defects().expect("scan after write");
    assert!(
        g.derived_recomputes() > n0,
        "a write must force a recompute; the memo answered from a stale generation"
    );
    // The write BROKE the cycle, so the scan after it must see fewer defects.
    // A stale memo would have returned `first` again.
    assert!(
        after.len() < first.len(),
        "the scan after the write must reflect it: before {} defects, after {} — {after:?}",
        first.len(),
        after.len()
    );
}

/// COUNTERWEIGHT 2: the suppression counts `detect_defects` reports are
/// replayed from the memo, so a memoised sweep and a fresh one report the same
/// scope. Silently dropping them would make the second report lie by omission.
#[test]
fn a_memoised_sweep_reports_the_same_scope_as_a_fresh_one() {
    let g = graph_with_a_defect();
    let fresh = g.detect_defects().expect("fresh sweep");
    let memoised = g.detect_defects().expect("memoised sweep");
    assert_eq!(
        serde_json::to_value(&fresh.swept).unwrap(),
        serde_json::to_value(&memoised.swept).unwrap(),
        "the sweep scope must survive the memo unchanged"
    );
    assert_eq!(fresh.defects.len(), memoised.defects.len());
}
