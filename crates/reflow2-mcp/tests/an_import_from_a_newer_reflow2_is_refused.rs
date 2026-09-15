//! An import knows its own age, and a server behind its record says so.
//!
//! ROOT CAUSE, measured 2026-09-14 (`fact:root-cause-the-853-is-an-older-binary-
//! writing-the-old-role-default-onto-a-newer-document-not-the-export-leaving-a-
//! default-implicit`): the launcher served a 0.59.0 release binary while the
//! committed record had been written by 0.60.2. Import compared no versions,
//! the open-time guard compared only vocabulary (identical), and the old binary
//! wrote its `role: author` default onto 853 AUTHORED_BY edges the newer
//! reflow2 had deliberately left implicit. Nothing said so anywhere.
//!
//! Three legs, on Anthony's word:
//! 1. `import_graph` REFUSES a document stamped by a newer reflow2 unless
//!    `accept_newer` — the mirror of the stale-export refusal.
//! 2. The import report names what it MATERIALISED that the document did not
//!    state, by `Type.property`.
//! 3. `served_by.behind_record` says when this server is older than the last
//!    reflow2 that wrote its store — a question `served_by.stale` (was my
//!    executable replaced since I started?) could never answer.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Map, Value, json};

fn a_document_from(version: &str) -> Map<String, Value> {
    json!({
        "graph_id": "t",
        "nodes": [{"node_type": "Project", "node_id": "proj:x", "properties": {"name": "X"}}],
        "edges": [],
        "stamp": {
            "reflow2_version": version,
            "schema_version": 1,
            "node_types": 28,
            "edge_types": 65
        }
    })
    .as_object()
    .cloned()
    .expect("object")
}

/// Leg 1, on the served tool: refused, and the refusal says both versions,
/// the direction, and the way through.
#[tokio::test]
async fn the_tool_refuses_a_document_from_the_future_and_names_the_way_through() {
    let s = ReflowService::in_memory().expect("service");
    let err = s
        .import_graph(Parameters(ImportGraphReq {
            document: Some(a_document_from("99.0.0")),
            path: None,
            accept_newer: None,
        }))
        .await
        .expect_err("refused by default");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("99.0.0") && msg.contains("BEHIND") && msg.contains("accept_newer"),
        "{msg}"
    );
}

/// Legs 1 and 2 together: accepted on purpose, and the report names what this
/// binary materialised that the document never stated.
#[tokio::test]
async fn accepted_on_purpose_the_report_names_what_was_materialised() {
    let s = ReflowService::in_memory().expect("service");
    let out = s
        .import_graph(Parameters(ImportGraphReq {
            document: Some(a_document_from("99.0.0")),
            path: None,
            accept_newer: Some(true),
        }))
        .await
        .expect("accepted")
        .structured_content
        .expect("structured");
    assert_eq!(out["nodes_written"], 1, "{out}");
    let materialized = out["materialized"]
        .as_object()
        .expect("a Project stated only its name, so its defaults were materialised");
    assert!(
        materialized.keys().all(|k| k.starts_with("Project.")),
        "counted by Type.property: {materialized:?}"
    );
    assert!(!materialized.is_empty(), "{out}");
}

/// A document from this version, or an older one, needs no flag.
#[tokio::test]
async fn a_document_from_this_or_an_older_reflow2_imports_as_before() {
    let s = ReflowService::in_memory().expect("service");
    for v in [env!("CARGO_PKG_VERSION"), "0.1.0"] {
        s.import_graph(Parameters(ImportGraphReq {
            document: Some(a_document_from(v)),
            path: None,
            accept_newer: None,
        }))
        .await
        .unwrap_or_else(|e| panic!("{v}: {e:?}"));
    }
}

/// Leg 3: the served block is absent when current, and names the newer
/// writer, this version and the hazard when behind.
#[test]
fn served_by_says_when_this_server_is_behind_the_record_it_holds() {
    assert!(
        behind_record(None).is_none(),
        "absent, not false, when current"
    );
    let b = behind_record(Some("99.0.0")).expect("behind");
    assert_eq!(b["written_by"], "99.0.0");
    assert_eq!(b["running"], env!("CARGO_PKG_VERSION"));
    let note = b["note"].as_str().unwrap();
    assert!(note.contains("BEHIND") && note.contains("853"), "{note}");
    assert!(
        BEHIND_NEXT.contains("behind_record") && BEHIND_NEXT.contains("WRITE"),
        "the next line points at the block and names the hazard: {BEHIND_NEXT}"
    );
}
