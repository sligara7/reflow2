//! Writing over a design record that moved while you were working.
//!
//! `req:stale-seat-knows`. The scenario, which is the one the collaboration
//! guide had to teach a workaround for: two people share one design through a
//! committed export. She pulls his work into the file, you export from a graph
//! that never caught up, and your document — internally perfect, simply older —
//! replaces it. The merge driver finds no conflict because **there is none**: a
//! stale export is a complete document. His requirements are gone with nothing
//! in the diff that looks like an error.
//!
//! What is pinned here is the line between loud and quiet, because a check that
//! fires on every ordinary export would be passed by habit within a day and
//! would then protect nobody:
//!
//! - the file is where you left it → silent
//! - the file moved but nothing of it is lost → allowed, and said
//! - the file moved and the write would drop what it holds → refused

use reflow2_core::sync::{SyncVerdict, assess_overwrite};
use reflow2_core::{DesignGraph, GraphExport};

/// The shared design, as it stands before anyone diverges.
fn shared() -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();
    g.add_requirement("req:range", "Crosslink range", "Close at 5,000 km.")
        .unwrap();
    g.export_graph().unwrap()
}

/// The same design, plus what the other seat added while you were working.
fn shared_plus_theirs() -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();
    g.add_requirement("req:range", "Crosslink range", "Close at 5,000 km.")
        .unwrap();
    g.add_requirement(
        "req:theirs",
        "Their requirement",
        "Written by the other seat.",
    )
    .unwrap();
    g.add_capability(
        "cap:theirs",
        "Their capability",
        "and how they meet it",
        None,
    )
    .unwrap();
    g.satisfies("cap:theirs", "req:theirs").unwrap();
    g.export_graph().unwrap()
}

/// Your graph: the shared design plus your own work, and nothing of theirs.
fn shared_plus_mine() -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();
    g.add_requirement("req:range", "Crosslink range", "Close at 5,000 km.")
        .unwrap();
    g.add_requirement("req:mine", "My requirement", "Written by this seat.")
        .unwrap();
    g.export_graph().unwrap()
}

#[test]
fn the_ordinary_export_is_silent() {
    // The file is exactly what this seat last wrote. This runs on essentially
    // every export in a working session, and it must cost nothing and say
    // nothing — a check that chatters is a check that gets ignored.
    let on_disk = shared();
    let hash = on_disk.effective_content_hash();
    let mine = shared_plus_mine();

    let verdict = assess_overwrite(Some(&on_disk), &mine, Some(&hash));
    assert_eq!(verdict, SyncVerdict::Clear);
    assert!(verdict.message("design.json").is_none());
}

#[test]
fn a_first_write_to_a_new_path_is_clear() {
    assert_eq!(
        assess_overwrite(None, &shared_plus_mine(), None),
        SyncVerdict::Clear
    );
}

#[test]
fn writing_over_their_work_is_refused_and_names_it() {
    // THE test. You never synced with what is on disk, and your export lacks
    // their requirement, their capability and the edge between them.
    let theirs = shared_plus_theirs();
    let mine = shared_plus_mine();

    let verdict = assess_overwrite(Some(&theirs), &mine, None);

    let SyncVerdict::WouldDrop {
        dropped_nodes,
        dropped_edges,
        ..
    } = &verdict
    else {
        panic!("must refuse: {verdict:?}");
    };
    assert!(
        dropped_nodes.contains(&"req:theirs".to_string()),
        "{dropped_nodes:?}"
    );
    assert!(
        dropped_nodes.contains(&"cap:theirs".to_string()),
        "{dropped_nodes:?}"
    );
    assert!(
        dropped_edges.iter().any(|e| e.contains("SATISFIES")),
        "the edge they drew must count as loss too: {dropped_edges:?}"
    );
    assert!(verdict.is_loss());

    let message = verdict.message("docs/design/reflow2.json").unwrap();
    assert!(message.contains("REFUSED"), "{message}");
    assert!(
        message.contains("req:theirs"),
        "name what would go: {message}"
    );
    assert!(
        message.contains("import_graph") && message.contains("compare_designs"),
        "rule 4 — say what would work instead: {message}"
    );
    assert!(
        message.contains("accept_divergence"),
        "and how to override deliberately: {message}"
    );
    assert!(
        message.contains("git will not catch it"),
        "the reason this is not a normal conflict is the whole point: {message}"
    );
}

#[test]
fn a_stale_seat_that_pulled_first_is_allowed_and_told() {
    // The remedy working: you imported their file, so your graph is now a
    // superset. Nothing is lost, so nothing is refused — but the movement is
    // still reported, because "somebody else has been here" is worth knowing.
    let theirs = shared_plus_theirs();
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.import_graph(&theirs).unwrap();
    g.add_requirement("req:mine", "My requirement", "Written after importing.")
        .unwrap();
    let mine = g.export_graph().unwrap();

    let verdict = assess_overwrite(Some(&theirs), &mine, None);

    assert!(!verdict.is_loss(), "{verdict:?}");
    let message = verdict.message("design.json").unwrap();
    assert!(message.contains("nothing in it would be lost"), "{message}");
}

#[test]
fn an_unchanged_design_is_clear_whoever_wrote_the_file() {
    // Byte-identical content cannot lose anything, so it never stops — even
    // with no sync marker at all, which is the state of every graph the first
    // time it runs a reflow2 that has this check.
    let on_disk = shared();
    let same = shared();
    assert_eq!(
        assess_overwrite(Some(&on_disk), &same, None),
        SyncVerdict::Clear
    );
}

#[test]
fn an_empty_graph_writing_over_a_real_design_is_refused() {
    // The worst version, and the reason the check cannot rest on the marker
    // alone: a fresh clone runs the installer, gets an empty graph, exports —
    // and would replace the whole committed design with nothing. No marker
    // exists, and none is needed to see the harm.
    let on_disk = shared_plus_theirs();
    let empty = DesignGraph::open_in_memory()
        .unwrap()
        .export_graph()
        .unwrap();

    let verdict = assess_overwrite(Some(&on_disk), &empty, None);

    assert!(verdict.is_loss(), "{verdict:?}");
    let SyncVerdict::WouldDrop { dropped_nodes, .. } = &verdict else {
        unreachable!()
    };
    assert!(
        dropped_nodes.len() >= 4,
        "everything on disk is at risk here: {dropped_nodes:?}"
    );
}

#[test]
fn the_message_caps_what_it_lists_and_says_it_capped() {
    // Rule 6: a refusal listing four hundred ids is a refusal nobody reads,
    // and a silent truncation is a lie about scope.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:big", "Big").unwrap();
    for i in 0..40 {
        g.add_requirement(&format!("req:{i}"), &format!("R{i}"), "…")
            .unwrap();
    }
    let on_disk = g.export_graph().unwrap();
    let empty = DesignGraph::open_in_memory()
        .unwrap()
        .export_graph()
        .unwrap();

    let message = assess_overwrite(Some(&on_disk), &empty, None)
        .message("design.json")
        .unwrap();
    assert!(
        message.contains("and 3"),
        "the remainder must be counted: {message}"
    );
    assert!(message.contains("more"), "{message}");
}
