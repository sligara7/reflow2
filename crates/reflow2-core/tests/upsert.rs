//! `upsert_node` — the update half of generic CRUD (BL-46).
//!
//! The contract the revise-design skill states: an existing id **merges** —
//! the props you pass overwrite, everything else survives. `create_node`
//! alone is create-or-replace, and replacing re-materializes schema defaults
//! over everything omitted; that reset a verified capability to `planned`
//! during the 2026-07-20 self-adopt session.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

#[test]
fn a_partial_upsert_edits_without_resetting_the_rest_to_defaults() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:kit", "Install", "one command", None)
        .unwrap();
    g.set_capability_status("cap:kit", "verified").unwrap();
    g.set_provenance(node::CAPABILITY, "cap:kit", "authored")
        .unwrap();

    // The live failure: fold a longer description in, naming nothing else.
    let n = g
        .upsert_node(
            node::CAPABILITY,
            "cap:kit",
            Props::new().set("description", "one command, four harnesses"),
        )
        .unwrap();

    let get = |k: &str| {
        n.properties
            .get(k)
            .and_then(|v| v.as_str().map(String::from))
    };
    assert_eq!(
        get("description").as_deref(),
        Some("one command, four harnesses")
    );
    assert_eq!(
        get("status").as_deref(),
        Some("verified"),
        "the status the caller did not name must survive, not reset to the schema default"
    );
    assert_eq!(get("provenance").as_deref(), Some("authored"));
    assert_eq!(get("name").as_deref(), Some("Install"));
}

#[test]
fn an_upsert_of_a_new_id_creates_the_node() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    let n = g
        .upsert_node(
            node::CAPABILITY,
            "cap:new",
            Props::new().set("name", "New").set("description", "fresh"),
        )
        .unwrap();
    assert_eq!(n.node_id, "cap:new");
    assert!(g.get_node(node::CAPABILITY, "cap:new").unwrap().is_some());
}

#[test]
fn an_upsert_still_fails_loud_on_an_unknown_type() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    assert!(
        g.upsert_node("NotAType", "x:1", Props::new().set("name", "?"))
            .is_err(),
        "merge semantics must not widen the write path's validation"
    );
}
