//! HOW LONG FROM THE EXPORT DOCUMENT TO A QUERYABLE DESIGN — the measurement
//! that decides whether an embedded byte store earns its place at all.
//!
//! `dec:idea-does-reflow2-need-a-byte-store-at-all-or-only-durability` asks
//! what ANY embedded store buys over the export document alone, and closes:
//! *"WHAT IS UNMEASURED AND DECIDES IT: hydrate time. Parsing the JSON,
//! building the adjacency index and building the tantivy index, to first
//! query. Nobody has taken it."*
//!
//! `fact:hydrate-cost-is-superlinear-in-edges-and-the-byte-store-is-not-the-
//! bottleneck` took the closest earlier reading and states its own bound in as
//! many words: **the full-text index was off, so every figure on both sides of
//! that comparison is a LOWER BOUND for a hydrate that must answer
//! `search_design`.** That bound is what this instrument exists to close.
//!
//! ⚠️ RUN IT IN RELEASE, AND SAY SO WHEN QUOTING IT. A debug figure is not
//! comparable with the recorded ones, which were all release:
//!
//!   cargo test -p reflow2-core --release --no-default-features \
//!     --test hydrate_cost_from_the_document_alone -- --ignored --nocapture
//!   cargo test -p reflow2-core --release --features fulltext \
//!     --test hydrate_cost_from_the_document_alone -- --ignored --nocapture
//!
//! `#[ignore]` because it reads an 11 MB file and is a MEASUREMENT, not a
//! property: it has no pass condition and must never gate a build. What it
//! prints is the evidence; what a session does with it is a judgement.

use std::time::Instant;

use reflow2_core::{DesignGraph, GraphExport};

/// The committed export — the only document that is both realistic and stable
/// enough to quote a number against.
const EXPORT: &str = "../../docs/design/reflow2.json";

#[test]
#[ignore = "measurement, not a property: reads the 11 MB export and has no pass condition"]
fn hydrate_from_the_document_to_first_query() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXPORT);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    println!("\n=== HYDRATE FROM DOCUMENT ALONE ===");
    println!("document      : {} bytes", raw.len());
    println!(
        "fulltext      : {}",
        if cfg!(feature = "fulltext") {
            "ON"
        } else {
            "OFF"
        }
    );
    println!(
        "profile       : {}",
        if cfg!(debug_assertions) {
            "DEBUG (not comparable)"
        } else {
            "release"
        }
    );

    // ① PARSE. Separated from import because the earlier reading found 95% of
    // the cost was import rather than JSON parsing, and a combined number
    // cannot show that.
    let t = Instant::now();
    let doc: GraphExport = serde_json::from_str(&raw).expect("parse export");
    let parse = t.elapsed();

    let t = Instant::now();
    let mut g = DesignGraph::open_in_memory_as("reflow2").expect("in-memory graph");
    let open = t.elapsed();

    // ② IMPORT — nodes, edges, adjacency, and (with the feature on) the
    // full-text index, which the engine mirrors on write.
    let t = Instant::now();
    let report = g.import_graph(&doc).expect("import");
    let import = t.elapsed();

    // ③ FIRST QUERY. Two of them, because they exercise different indexes and
    // only one of them is what the `fulltext` bound is about.
    let t = Instant::now();
    let n = g.count_all_nodes().expect("count");
    let count_q = t.elapsed();

    let t = Instant::now();
    let _ = g.get_node("Capability", "cap:store").expect("get_node");
    let point_q = t.elapsed();

    println!("nodes/edges   : {} / {}", doc.nodes.len(), doc.edges.len());
    println!("imported      : {report:?}");
    println!("--");
    println!("parse         : {:>8.1} ms", parse.as_secs_f64() * 1000.0);
    println!("open graph    : {:>8.1} ms", open.as_secs_f64() * 1000.0);
    println!("import        : {:>8.1} ms", import.as_secs_f64() * 1000.0);
    println!(
        "count ({n:>5}) : {:>8.1} ms",
        count_q.as_secs_f64() * 1000.0
    );
    println!("point read    : {:>8.1} ms", point_q.as_secs_f64() * 1000.0);
    println!(
        "TOTAL to first query : {:>8.1} ms",
        (parse + open + import + point_q).as_secs_f64() * 1000.0
    );

    // ④ THE QUERY THE BOUND IS ABOUT. Only reachable with the feature on; with
    // it off `search_design` fails loud by design, and calling it would measure
    // a refusal rather than a search — the exact "a call that completed is not
    // a call that worked" trap.
    #[cfg(feature = "fulltext")]
    {
        let t = Instant::now();
        let hits = g
            .search_design("persistence store durability", None, 10)
            .expect("search_design");
        let search_q = t.elapsed();
        println!(
            "search_design : {:>8.1} ms  ({} hit(s))",
            search_q.as_secs_f64() * 1000.0,
            hits.hits.len()
        );
        println!(
            "TOTAL to first SEARCH: {:>8.1} ms",
            (parse + open + import + search_q).as_secs_f64() * 1000.0
        );
    }
    #[cfg(not(feature = "fulltext"))]
    println!("search_design : NOT BUILT — this run is a LOWER BOUND, not the answer");
    println!("=== END ===\n");
}

/// THE OTHER SIDE OF THE COMPARISON — opening the EXISTING RocksDB store and
/// asking it the same first question.
///
/// Without this arm the document figure means nothing: `dec:idea-does-reflow2-
/// need-a-byte-store-at-all-or-only-durability` asks what a store BUYS, and that
/// is a difference, not an absolute. The node's own text flags the trap it is
/// avoiding — an earlier 2.4-second reading was "an EXISTING store opening with
/// its index already on disk — THE CHEAP HALF, and not this number" — so the two
/// halves have to be taken on one machine, one design, one sitting.
///
/// 🛑 THE SHARED SERVER MUST BE STOPPED FIRST or this fails on the lock:
///   ./target/debug/reflow2-mcp --graph-path ./.reflow2/graph --stop-shared
///
///   cargo test -p reflow2-core --release --features "rocksdb fulltext" \
///     --test hydrate_cost_from_the_document_alone -- --ignored --nocapture
#[test]
#[ignore = "measurement; needs the shared server stopped so the store lock is free"]
#[cfg(feature = "rocksdb")]
fn open_the_existing_store_to_first_query() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.reflow2/graph");

    println!("\n=== OPEN THE EXISTING STORE ===");
    println!(
        "fulltext      : {}",
        if cfg!(feature = "fulltext") {
            "ON"
        } else {
            "OFF"
        }
    );

    let t = Instant::now();
    let g = match DesignGraph::open_rocksdb(path.to_str().expect("path")) {
        Ok(g) => g,
        Err(e) => {
            println!("could not open {}: {e}", path.display());
            println!("(is the shared server still holding the lock?)");
            return;
        }
    };
    let open = t.elapsed();

    let t = Instant::now();
    let n = g.count_all_nodes().expect("count");
    let count_q = t.elapsed();

    let t = Instant::now();
    let _ = g.get_node("Capability", "cap:store").expect("get_node");
    let point_q = t.elapsed();

    println!("open store    : {:>8.1} ms", open.as_secs_f64() * 1000.0);
    println!(
        "count ({n:>5}) : {:>8.1} ms",
        count_q.as_secs_f64() * 1000.0
    );
    println!("point read    : {:>8.1} ms", point_q.as_secs_f64() * 1000.0);
    println!(
        "TOTAL to first query : {:>8.1} ms",
        (open + point_q).as_secs_f64() * 1000.0
    );

    #[cfg(feature = "fulltext")]
    {
        let t = Instant::now();
        let hits = g
            .search_design("persistence store durability", None, 10)
            .expect("search_design");
        println!(
            "search_design : {:>8.1} ms  ({} hit(s))",
            t.elapsed().as_secs_f64() * 1000.0,
            hits.hits.len()
        );
        println!(
            "TOTAL to first SEARCH: {:>8.1} ms",
            (open + t.elapsed()).as_secs_f64() * 1000.0
        );
    }
    println!("=== END ===\n");
}
