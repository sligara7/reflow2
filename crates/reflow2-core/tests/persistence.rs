//! On-disk persistence via the RocksDB backend (surface-plan.md, step 1).
//!
//! The design must survive across agent sessions, so [`DesignGraph::open_rocksdb`]
//! opens a durable store where [`DesignGraph::open_in_memory`] is dev/test only.
//!
//! Two behaviours are asserted, each gated on the `rocksdb` feature:
//!
//! - Feature **off** (the default `--no-default-features` dev build): opening an
//!   on-disk graph fails loud, never silently degrading to memory (AGENTS.md
//!   rule 4). This test runs in the normal fast suite.
//! - Feature **on**: a graph written, dropped, and reopened at the same path
//!   reads back its nodes — data genuinely survived the process-local handle.
//!   This test compiles only with `--features rocksdb` (the C++ compile is
//!   opt-in), so it does not run in the default suite.

use reflow2_core::DesignGraph;

/// Without the `rocksdb` feature the on-disk backend is not compiled in, so
/// requesting it must return an actionable error rather than a memory fallback.
#[cfg(not(feature = "rocksdb"))]
#[test]
fn open_rocksdb_without_feature_fails_loud() {
    use reflow2_core::provenance::stamp_path;

    // A unique path per run, so the test is hermetic. A fixed path accumulates a
    // provenance stamp across runs, and a stale stamp written by a *different*
    // schema version used to pre-empt this very error with a "knows more of the
    // schema" refusal — the exact bug fixed in graph.rs (open before stamp).
    let path = std::env::temp_dir().join(format!("reflow2-nofeature-{}", std::process::id()));
    let path = path.to_str().expect("temp path is valid utf-8");
    let _ = std::fs::remove_file(stamp_path(path)); // clear any stale sibling stamp

    let Err(err) = DesignGraph::open_rocksdb(path) else {
        panic!("on-disk open must fail loud when the rocksdb feature is off");
    };
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("rocksdb"),
        "error should name the missing feature, got: {msg}"
    );
    // The regression this pins: a failed open must not leave a provenance stamp
    // behind. A stray stamp poisons the next open across a schema change.
    assert!(
        !stamp_path(path).exists(),
        "a feature-off open must not write a version stamp"
    );
}

/// A full write → drop → reopen round trip proves the store is durable, not just
/// process-local. Only compiled with the `rocksdb` feature.
#[cfg(feature = "rocksdb")]
#[test]
fn rocksdb_round_trips_across_reopen() {
    use reflow2_core::nodes::node;

    // Unique temp dir, cleaned up at the end. `process::id` keeps concurrent
    // test binaries from colliding without pulling in a tempfile dependency.
    let dir = std::env::temp_dir().join(format!("reflow2-rocksdb-{}", std::process::id()));
    let path = dir.to_str().expect("temp path is valid utf-8").to_string();
    let _ = std::fs::remove_dir_all(&dir); // clear any stale run

    // --- Session 1: write and close ---
    {
        let mut g = DesignGraph::open_rocksdb(&path).expect("open on-disk graph");
        g.add_project("proj:softball", "Softball Game")
            .expect("create Project");
        g.add_requirement(
            "req:physics",
            "Realistic physics",
            "Ball flight and bat-ball collision must be physically plausible.",
        )
        .expect("create Requirement");
        g.contains("proj:softball", node::REQUIREMENT, "req:physics")
            .expect("Project CONTAINS Requirement");
    } // g dropped here — the RocksDB handle is released.

    // --- Session 2: reopen the same path and read back ---
    {
        let g = DesignGraph::open_rocksdb(&path).expect("reopen on-disk graph");
        assert_eq!(g.count_nodes(node::PROJECT).unwrap(), 1);
        let req = g
            .get_node(node::REQUIREMENT, "req:physics")
            .unwrap()
            .expect("Requirement should survive the reopen");
        assert_eq!(req.properties["name"].as_str(), Some("Realistic physics"));
    }

    std::fs::remove_dir_all(&dir).expect("clean up temp store");
}
