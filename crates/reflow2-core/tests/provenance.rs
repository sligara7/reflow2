//! BL-19 — which reflow2 wrote this graph, recorded beside it.
//!
//! These drive `check_and_stamp` directly against real files rather than
//! through `open_rocksdb`, so they run on the fast in-memory test path. The
//! RocksDB wiring is covered by `tools/smoke_mcp.py`.

use reflow2_core::provenance::{GraphStamp, Provenance, check_and_stamp, stamp_path};
use reflow2_core::schema::load_schema;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("reflow2-prov-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn an_unstamped_graph_is_stamped_and_says_so() {
    let d = tmpdir("unstamped");
    let g = d.join("graph");
    let schema = load_schema().unwrap();

    let v = check_and_stamp(g.to_str().unwrap(), &schema).unwrap();
    assert!(matches!(v, Provenance::Unstamped { .. }));
    assert!(
        v.note().unwrap().contains("no version stamp"),
        "an unstamped graph must say so rather than passing silently"
    );

    // The stamp lands beside the store, never inside it — RocksDB owns that dir.
    let p = stamp_path(g.to_str().unwrap());
    assert_eq!(p, d.join("graph.meta.json"));
    assert!(p.exists());

    // Second open now matches, and has nothing to report.
    let again = check_and_stamp(g.to_str().unwrap(), &schema).unwrap();
    assert!(matches!(again, Provenance::Match { .. }));
    assert_eq!(again.note(), None, "a matching graph is not worth a remark");
    std::fs::remove_dir_all(&d).ok();
}

/// The case that must **not** be refused. Schema growth is additive, so a graph
/// written before a type existed reads perfectly — refusing would lock someone
/// out of their own design over a change that cannot hurt them.
#[test]
fn an_older_graph_opens_and_reports_the_difference() {
    let d = tmpdir("older");
    let g = d.join("graph");
    let schema = load_schema().unwrap();

    let old = GraphStamp {
        reflow2_version: "0.0.1".into(),
        schema_version: 1,
        node_types: 26,
        edge_types: 52,
    };
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&old).unwrap(),
    )
    .unwrap();

    let v = check_and_stamp(g.to_str().unwrap(), &schema).unwrap();
    match &v {
        Provenance::OlderGraph { was, now } => {
            assert_eq!(was.node_types, 26);
            assert!(now.node_types >= 27);
        }
        other => panic!("expected OlderGraph, got {other:?}"),
    }
    let note = v.note().unwrap();
    assert!(
        note.contains("0.0.1") && note.contains("still reads"),
        "got {note}"
    );

    // And the stamp is refreshed, so it tracks the newest reflow2 to hold it.
    let after: GraphStamp =
        serde_json::from_str(&std::fs::read_to_string(stamp_path(g.to_str().unwrap())).unwrap())
            .unwrap();
    assert!(after.node_types >= 27);
    std::fs::remove_dir_all(&d).ok();
}

/// The one refusal: a graph written by a reflow2 that knew more of the schema.
/// Opening it would show less of the design than it holds.
#[test]
fn a_graph_from_the_future_is_refused_loudly() {
    let d = tmpdir("future");
    let g = d.join("graph");
    let schema = load_schema().unwrap();

    let future = GraphStamp {
        reflow2_version: "9.9.9".into(),
        schema_version: 1,
        node_types: 99,
        edge_types: 99,
    };
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&future).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema)
        .expect_err("a graph from the future cannot be read in full");
    let msg = err.to_string();
    assert!(msg.contains("9.9.9"), "say which reflow2 wrote it: {msg}");
    assert!(
        msg.contains("less of your design"),
        "say why it is refused, not just that it is: {msg}"
    );
    assert!(
        msg.contains("cargo build"),
        "say what to do about it: {msg}"
    );

    // Refused means untouched: the stamp is the only record of what wrote it.
    let after: GraphStamp =
        serde_json::from_str(&std::fs::read_to_string(stamp_path(g.to_str().unwrap())).unwrap())
            .unwrap();
    assert_eq!(
        after, future,
        "a refused open must not overwrite the record"
    );
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn an_unreadable_stamp_is_reported_never_overwritten() {
    let d = tmpdir("corrupt");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    std::fs::write(stamp_path(g.to_str().unwrap()), "{ not json").unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema).expect_err("must not guess");
    assert!(err.to_string().contains("not readable"), "{err}");
    assert_eq!(
        std::fs::read_to_string(stamp_path(g.to_str().unwrap())).unwrap(),
        "{ not json",
        "it may be the only record of what wrote the graph"
    );
    std::fs::remove_dir_all(&d).ok();
}
