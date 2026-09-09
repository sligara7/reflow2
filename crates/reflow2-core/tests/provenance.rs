//! BL-19 — which reflow2 wrote this graph, recorded beside it.
//!
//! These drive `check_and_stamp` directly against real files rather than
//! through `open_rocksdb`, so they run on the fast in-memory test path. The
//! RocksDB wiring is covered by `tools/smoke_mcp.py`.

use reflow2_core::provenance::{GraphStamp, Provenance, check_and_stamp, stamp_path};
use reflow2_core::schema::load_schema;

/// The graph holds NO instances of any retired type — the ordinary case, and
/// the one the field report was about. Named rather than inlined so every call
/// site below states the population it assumes rather than implying one.
fn holds_none(_types: &[String]) -> Result<Vec<String>, reflow2_core::DynoError> {
    Ok(Vec::new())
}

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

    let v = check_and_stamp(g.to_str().unwrap(), &schema, holds_none).unwrap();
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
    let again = check_and_stamp(g.to_str().unwrap(), &schema, holds_none).unwrap();
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
        node_type_names: None,
        edge_type_names: None,
    };
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&old).unwrap(),
    )
    .unwrap();

    let v = check_and_stamp(g.to_str().unwrap(), &schema, holds_none).unwrap();
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
        node_type_names: None,
        edge_type_names: None,
    };
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&future).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema, holds_none)
        .expect_err("a graph from the future cannot be read in full");
    let msg = err.to_string();
    assert!(msg.contains("9.9.9"), "say which reflow2 wrote it: {msg}");
    assert!(
        msg.contains("less of your design"),
        "say why it is refused, not just that it is: {msg}"
    );
    assert!(
        msg.contains("update reflow2") && msg.contains("import"),
        "say what to do about it — both recovery paths (update the binary, or migrate the \
         graph by import): {msg}"
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

    let err =
        check_and_stamp(g.to_str().unwrap(), &schema, holds_none).expect_err("must not guess");
    assert!(err.to_string().contains("not readable"), "{err}");
    assert_eq!(
        std::fs::read_to_string(stamp_path(g.to_str().unwrap())).unwrap(),
        "{ not json",
        "it may be the only record of what wrote the graph"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// BL-86, the real @bro/StoryFlow scenario end to end: a set-based stamp that
/// names a RETIRED edge type (`VALIDATES`) is refused with the *migrate* path
/// precisely — not the count-only hedge, and not "update your binary." Uses the
/// real schema, so it also exercises `GraphStamp::current` populating the sets.
#[test]
fn a_graph_naming_a_retired_type_is_told_to_migrate() {
    let d = tmpdir("retired");
    let g = d.join("graph");
    let schema = load_schema().unwrap();

    // Today's schema, plus the retired VALIDATES edge the old graph still used.
    let now = GraphStamp::current(&schema);
    let mut edges = now.edge_type_names.clone().unwrap();
    edges.push("VALIDATES".into());
    edges.sort();
    let was = GraphStamp {
        reflow2_version: "0.9.0".into(),
        schema_version: 1,
        node_types: now.node_types,
        edge_types: edges.len(),
        node_type_names: now.node_type_names.clone(),
        edge_type_names: Some(edges),
    };
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&was).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema, holds_none)
        .expect_err("a graph using a retired type must be refused");
    let msg = err.to_string();
    assert!(msg.contains("VALIDATES"), "name the retired type: {msg}");
    assert!(
        msg.to_lowercase().contains("migrate") && !msg.contains("BEHIND"),
        "point at migration, not a binary update: {msg}"
    );
    std::fs::remove_dir_all(&d).ok();
}

// ─── The zero-instance refusal, reported from the field 2026-09-08 ──────────
//
// A user's graph, written by 0.50.0 and holding ZERO QualityGate nodes across
// 3,130, was refused by 0.55.1 with "opening it could silently show you less of
// your design than it holds". The justification is false at zero instances:
// there is nothing to show less of. It cost a full working session, and it was
// the SECOND time a version guard had bricked that user's graph — their store
// still carries a `graph.pre-0.10.1-unreadable-2026-07-24` beside it.
//
// The guard refuses on the STAMP, which records the SCHEMA the writing binary
// had, so every graph written before a retirement names the retired type
// whether or not it ever held one. That is a declaration being read as evidence
// about content — the same class as `dec:sidecar-loss-guarded-by-store-evidence`,
// which is ACCEPTED and already rules that a refusal must be conditioned on
// what the store actually holds.

/// A stamp naming a type this binary RETIRED, on a graph holding none of them.
fn stamp_naming(
    extra_node_type: &str,
    schema: &reflow2_core::foundation::core::Schema,
) -> GraphStamp {
    let mut s = GraphStamp::current(schema);
    let mut names = s.node_type_names.clone().unwrap_or_default();
    names.push(extra_node_type.to_string());
    names.sort();
    s.node_types = names.len();
    s.node_type_names = Some(names);
    s.reflow2_version = "0.50.0".into();
    s
}

/// THE REPORTED BUG. A retired type with no instances must OPEN.
#[test]
fn a_retired_type_with_no_instances_opens() {
    let d = tmpdir("retired-zero");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&stamp_naming("QualityGate", &schema)).unwrap(),
    )
    .unwrap();

    let verdict = check_and_stamp(g.to_str().unwrap(), &schema, holds_none);
    assert!(
        verdict.is_ok(),
        "a graph holding ZERO instances of a retired type must OPEN: the refusal's own \
         justification — that opening could show less than the graph holds — is false when \
         the graph holds none. Got: {:?}",
        verdict.err()
    );
    std::fs::remove_dir_all(&d).ok();
}

/// THE CONTROL THAT STOPS THE FIX GOING TOO FAR. A type this binary has never
/// heard of still refuses: there the binary IS behind, the stamp is the only
/// evidence available, and reading really would show less than the graph holds.
#[test]
fn an_unknown_type_still_refuses_even_with_no_instances() {
    let d = tmpdir("unknown-zero");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&stamp_naming("SomeTypeFromTheFuture", &schema)).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema, holds_none)
        .expect_err("a graph from the future must still be refused");
    assert!(
        err.to_string().contains("BEHIND"),
        "and it must still say the BINARY is behind, not that the graph should migrate: {err}"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// THE CONTROL THAT STOPS THE FIX BECOMING "ALWAYS OPEN". When the graph DOES
/// hold instances of the retired type, the refusal is exactly right and must
/// still fire — that is the case the whole guard exists for.
#[test]
fn a_retired_type_that_is_actually_populated_still_refuses() {
    let d = tmpdir("retired-populated");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&stamp_naming("QualityGate", &schema)).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema, |types| {
        Ok(types.to_vec()) // every retired type has instances
    })
    .expect_err("a graph that HOLDS the retired type must still be refused");
    let msg = err.to_string();
    assert!(
        msg.contains("ACTUALLY HOLDS") && msg.contains("QualityGate"),
        "and it must say the graph really holds them, which is the fact that makes this \
         refusal correct rather than the declaration: {msg}"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// A COUNT THAT CANNOT BE TAKEN IS NOT A COUNT OF ZERO. If the store cannot be
/// scanned, the conservative refusal must stand — collapsing an I/O error to
/// "no instances" would open the graph the guard exists to protect, which is
/// the failure this project has recorded as `no silent fallback`.
#[test]
fn a_population_that_cannot_be_counted_still_refuses() {
    let d = tmpdir("retired-uncountable");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        stamp_path(g.to_str().unwrap()),
        serde_json::to_string(&stamp_naming("QualityGate", &schema)).unwrap(),
    )
    .unwrap();

    let err = check_and_stamp(g.to_str().unwrap(), &schema, |_| {
        Err(reflow2_core::DynoError::Storage("the scan failed".into()))
    })
    .expect_err("an uncountable population must not read as an empty one");
    assert!(
        err.to_string().contains("the scan failed"),
        "and the real reason must survive rather than being reported as a clean open: {err}"
    );
    std::fs::remove_dir_all(&d).ok();
}
