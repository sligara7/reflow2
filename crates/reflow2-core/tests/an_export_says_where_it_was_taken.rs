//! An export document can say which working tree it was taken in, and an
//! older one that cannot still reads.
//!
//! The coordinate lives on the EXPORT, not on nodes — the grain at which the
//! phantom-drift incident was actually a question: the graph does not branch
//! with git, so a plain export on one branch carries another branch's writes,
//! and the only thing that could have said so is the export itself
//! (`dec:idea-should-a-node-carry-its-git-coordinate`, option ②, on Anthony's
//! word 2026-09-16). The core takes no clock and does no I/O, so the field is
//! caller-supplied and optional; here only the document contract is pinned —
//! it survives a round trip, it is absent rather than defaulted on an old
//! document, and it is NOT part of the content hash, because two exports of one
//! design taken on different branches are the same design.

use reflow2_core::{DesignGraph, GraphExport, TakenAt};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g
}

#[test]
fn the_coordinate_survives_a_round_trip_through_json() {
    let mut export = design().export_graph().unwrap();
    export.taken_at = Some(TakenAt {
        branch: Some("feature".into()),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        dirty: true,
    });
    let text = serde_json::to_string(&export).unwrap();
    let back: GraphExport = serde_json::from_str(&text).unwrap();
    assert_eq!(back.taken_at, export.taken_at);
}

#[test]
fn an_older_document_reads_with_no_coordinate_rather_than_a_default() {
    let export = design().export_graph().unwrap();
    let mut v = serde_json::to_value(&export).unwrap();
    v.as_object_mut().unwrap().remove("taken_at");
    let back: GraphExport = serde_json::from_value(v).unwrap();
    assert_eq!(
        back.taken_at, None,
        "absent means nobody said — never 'clean on main'"
    );
}

#[test]
fn a_detached_head_is_a_commit_with_no_branch() {
    let text = r#"{"branch":null,"commit":"abc","dirty":false}"#;
    let t: TakenAt = serde_json::from_str(text).unwrap();
    assert_eq!(t.branch, None);
    let text = r#"{"commit":"abc"}"#;
    let t: TakenAt = serde_json::from_str(text).unwrap();
    assert_eq!((t.branch, t.dirty), (None, false));
}

#[test]
fn the_coordinate_is_not_part_of_the_content_hash() {
    let mut a = design().export_graph().unwrap();
    let mut b = a.clone();
    a.taken_at = Some(TakenAt {
        branch: Some("main".into()),
        commit: "aaaa".into(),
        dirty: false,
    });
    b.taken_at = Some(TakenAt {
        branch: Some("feature".into()),
        commit: "bbbb".into(),
        dirty: true,
    });
    assert_eq!(
        a.compute_content_hash(),
        b.compute_content_hash(),
        "the same design taken on two branches is the same design"
    );
}
