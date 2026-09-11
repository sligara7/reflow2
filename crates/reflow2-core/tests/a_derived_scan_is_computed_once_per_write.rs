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

/// THE SAME INVARIANT, ONE SCAN OVER: the whole-network scans a region walk
/// needs are computed once per graph state, not once per walk.
///
/// MEASURED 2026-09-11: `design_regions` calls `scope_region` for every Project
/// and Component — 109 seeds on reflow2's own design — and each calls
/// `propagate_from`. The tool cost >300 s twice and >150 s again, wedging the
/// shared daemon each time, while its own description bills it as *"the one
/// orientation read that asks for no seed… call it at check-in"*.
///
/// ⭐ THE COST WAS NOT WHERE IT LOOKED. Two hypotheses were measured and
/// REFUTED before the third held: the node-type index (memoised here too —
/// real, but a full build is only ~500 ms across 28 types), and the walk
/// itself (indexed prefix scans, cheap). `propagate_from` stayed at ~4.3 s on
/// EVERY call, unchanged by repetition, which is what ruled both out. The
/// cause is the last line before it returns: it rebuilt the design network and
/// re-ran an all-pairs BETWEENNESS over 4,176 nodes and 24,242 edges, purely
/// to rank the impacted set. After memoising it: `propagate_from` 4,815 ms
/// cold then ~250 ms, and `design_regions` >300 s → 29.6 s.
///
/// The argument is `con:a-sweep-builds-its-network-a-fixed-number-of-times`
/// verbatim, one tool over: *"the network has to be constructed once and
/// interrogated N times; nothing about the question requires reconstructing it
/// per candidate."* The graph does not change between seeds — only which node
/// the walk starts from.
///
/// STRUCTURE, NOT DURATION, for the reason this file's header gives.
#[test]
fn many_region_walks_build_the_node_type_index_once() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    for i in 0..12 {
        let id = format!("cmp:c{i}");
        g.add_component(&id, &id, "a part", None).unwrap();
    }
    let before = g.index_builds();
    for i in 0..12 {
        g.scope_region(&format!("cmp:c{i}"), 2).expect("region");
    }
    let built = g.index_builds() - before;
    assert!(
        built <= 1,
        "12 region walks with no write between them must share ONE node-type index, built {built}"
    );
}

/// COUNTERWEIGHT: a write invalidates it, so a walk after a write sees the new
/// node. A memo that answered from a stale index would hide a node that exists.
#[test]
fn a_write_rebuilds_the_index_and_the_next_walk_sees_the_new_node() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_component("cmp:a", "A", "does a", None).unwrap();
    g.scope_region("cmp:a", 2).expect("warm the index");

    g.add_component("cmp:b", "B", "does b", None).unwrap();
    g.depends_on("cmp:a", "cmp:b").unwrap();
    let region = g.scope_region("cmp:a", 2).expect("region after write");
    assert!(
        region.contains("cmp:b"),
        "a node added after the index was built must still be reachable: {region:?}"
    );
}

/// The betweenness memo, asserted the same structural way.
#[test]
fn many_walks_compute_network_betweenness_once() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    for i in 0..10 {
        let id = format!("cmp:n{i}");
        g.add_component(&id, &id, "a part", None).unwrap();
    }
    for i in 0..9 {
        g.depends_on(&format!("cmp:n{i}"), &format!("cmp:n{}", i + 1))
            .unwrap();
    }
    let before = g.betweenness_builds();
    for i in 0..10 {
        g.scope_region(&format!("cmp:n{i}"), 2).expect("region");
    }
    let built = g.betweenness_builds() - before;
    assert!(
        built <= 1,
        "ten region walks must share ONE betweenness computation, ran {built}"
    );
}

/// COUNTERWEIGHT: a write invalidates the betweenness too, so a walk after a
/// structural change ranks against the NEW network rather than the old one.
#[test]
fn a_write_rebuilds_the_betweenness() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_component("cmp:a", "A", "does a", None).unwrap();
    g.add_component("cmp:b", "B", "does b", None).unwrap();
    g.depends_on("cmp:a", "cmp:b").unwrap();
    g.scope_region("cmp:a", 2).expect("warm");
    let after_warm = g.betweenness_builds();

    g.add_component("cmp:c", "C", "does c", None).unwrap();
    g.depends_on("cmp:b", "cmp:c").unwrap();
    g.scope_region("cmp:a", 2).expect("after write");
    assert!(
        g.betweenness_builds() > after_warm,
        "a write must invalidate the memo and force a rebuild"
    );
}

/// THE THIRD SCAN, AND THE ONE THAT WAS THE REAL COST: the adjacency is read
/// once per graph state, not once per node VISIT.
///
/// MEASURED 2026-09-11, after the betweenness memo had already taken
/// `design_regions` from never-returning to 29.6 s. `impact_neighbors` did TWO
/// store prefix scans per visited node, and region walks OVERLAP — the tool's
/// own coverage block says 589 of 846 covered nodes lie in more than one
/// region. Counted: 108 walks made **4,502 node visits over 589 distinct
/// nodes, 7.6x redundant**, at ~4 ms a visit. So 18.0 s of the 21.6 s spent
/// walking was re-reading adjacency already read.
///
/// Reading every edge in ONE scan (the outgoing keys are prefixed by graph id)
/// and serving every visit from it: `propagate_from` 250 ms → 60 ms warm, and
/// `design_regions` 29.6 s → 12.5 s.
///
/// 🛑 THIS ASSERTION IS WHAT STOPS THE MEMO BEING REMOVED. Verified to fail
/// with the memo bypassed: rebuilding per call makes this 10, not 1.
#[test]
fn many_walks_read_the_adjacency_once() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    for i in 0..10 {
        let id = format!("cmp:adj{i}");
        g.add_component(&id, &id, "a part", None).unwrap();
    }
    for i in 0..9 {
        g.depends_on(&format!("cmp:adj{i}"), &format!("cmp:adj{}", i + 1))
            .unwrap();
    }
    let before = g.adjacency_builds();
    for i in 0..10 {
        g.scope_region(&format!("cmp:adj{i}"), 2).expect("region");
    }
    let built = g.adjacency_builds() - before;
    assert!(
        built <= 1,
        "ten region walks must share ONE adjacency read, built {built}"
    );
}

/// COUNTERWEIGHT, and the one that makes an adjacency cache safe: a new EDGE
/// must be visible to the next walk. An adjacency that outlived a write would
/// hide a real dependency, which is worse than any latency.
#[test]
fn a_new_edge_is_visible_to_the_next_walk() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_component("cmp:x", "X", "does x", None).unwrap();
    g.add_component("cmp:y", "Y", "does y", None).unwrap();
    let before = g.scope_region("cmp:x", 2).expect("region");
    assert!(!before.contains("cmp:y"), "not linked yet: {before:?}");

    g.depends_on("cmp:x", "cmp:y").unwrap();
    let after = g.scope_region("cmp:x", 2).expect("region after the edge");
    assert!(
        after.contains("cmp:y"),
        "an edge written after the adjacency was cached must still be walked: {after:?}"
    );
}
