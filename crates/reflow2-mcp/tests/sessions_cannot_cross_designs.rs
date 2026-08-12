//! A session attached to design X cannot reach design Y in the same process.
//!
//! `ver:sessions-cannot-cross-designs`. This check exists because
//! **`dec:one-process-many-stores` was accepted conditionally on it** (2026-08-06):
//! a hosted reflow2 holds N single-writer stores in one process, one RocksDB lock
//! each, and the directory stays the unit of a design. The condition was not
//! ceremony. Every other failure in this system announces itself — a bad export
//! fails its hash, a missing node errors, a stale seat says so. **An isolation
//! regression announces nothing.** Design A's requirement simply appears in
//! design B, or B's store answers as empty, and both look like ordinary states of
//! an ordinary design.
//!
//! These are on-disk tests and live in this crate for the same reason
//! `design_identity.rs` does: it is the one that always has the RocksDB backend.
//!
//! ## What is proven here, and what is not
//!
//! The condition named four properties. Two are about a **selection surface that
//! does not exist yet** — `cmp:registry` is `planned`, and there is today no way
//! for a session to name a `graph_id` and be given it, so "an id you were not
//! attached to is refused" and "a path is not an alternative route in" have no
//! surface to be asked of. Writing green tests for them against today's code
//! would be a vacuous pass, and a vacuous pass on the precondition of a
//! conditional decision is worse than no test: it would report the condition met.
//!
//! So those two clauses are **`ver:a-session-cannot-name-another-design`, a
//! separate Verification that is `planned` and says why.** They were originally
//! part of this check's statement, which was then set `passing` — a green node
//! standing for two clauses no test could reach, so
//! `dec:one-process-many-stores` read as fully met when it is **half** met.
//!
//! The split makes the unmet half countable in **`loop_status`**, whose
//! verification digest lists every check not currently passing. Checked, not
//! assumed: it does *not* move `detect_gaps`, because `unverified_capability`
//! fires on a Capability with no incoming `VERIFIES` and this one verifies a
//! Requirement. Edging it to `cap:select-graph-by-id` would have closed that
//! capability's gap with a check that proves nothing — informing the detector
//! and gaming it are one keystroke apart.
//!
//! [`the_registry_clauses_are_not_covered_here`] is the belt to that braces: a
//! status is a property any hand can set to `passing` in a single call, and a
//! failing test is not.
//!
//! What IS proven, today and for real:
//!
//! - Two designs open **simultaneously** in one process, and a write to either
//!   lands only in its own store. This is the clause the verification itself
//!   names as the proof, and it is the one the hosted case rests on.
//! - The **store**, not the id, is the isolation boundary — knowing another
//!   design's `graph_id` does not get you its contents.
//! - Reopening a store re-attaches to the same design, not to none and not to a
//!   neighbour.

use reflow2_core::DesignGraph;
use reflow2_core::identity;
use reflow2_core::nodes::node;

/// A store directory nothing else in the suite can collide with. Tests here run
/// concurrently with each other and with `design_identity.rs`, and two of them
/// sharing a path would take the same RocksDB lock and look like a hang.
fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-isolation-{}-{}-{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn two_designs_open_at_once_and_neither_write_reaches_the_other() {
    // THE NAMED PROOF: "opening two graphs in one process, writing to each, and
    // asserting each store contains only its own nodes".
    //
    // Both handles are deliberately held open ACROSS both writes rather than
    // opened and dropped in turn. Sequential opens would pass even if the code
    // kept the current design in a process-global, which is precisely the shape
    // the registry replaces — so a sequential version of this test would go green
    // on the bug it exists to catch.
    let a_dir = tmp("alpha");
    let b_dir = tmp("beta");
    let a_path = a_dir.join("graph");
    let b_path = b_dir.join("graph");
    let (a_path, b_path) = (a_path.to_str().unwrap(), b_path.to_str().unwrap());

    let mut alpha = DesignGraph::open_rocksdb(a_path).unwrap();
    let mut beta = DesignGraph::open_rocksdb(b_path).unwrap();

    assert_ne!(
        alpha.graph_id(),
        beta.graph_id(),
        "two stores must be two designs, or nothing below distinguishes them"
    );

    alpha.add_project("proj:alpha", "Alpha").unwrap();
    alpha
        .add_requirement("req:only-in-alpha", "Alpha only", "Belongs to Alpha.")
        .unwrap();
    beta.add_project("proj:beta", "Beta").unwrap();
    beta.add_requirement("req:only-in-beta", "Beta only", "Belongs to Beta.")
        .unwrap();

    // Interleave one more write each, after both stores are non-empty. A handle
    // that had latched onto "the design" at first write would cross here.
    alpha
        .add_requirement("req:alpha-second", "Alpha second", "Still Alpha.")
        .unwrap();
    beta.add_requirement("req:beta-second", "Beta second", "Still Beta.")
        .unwrap();

    for (name, graph, mine, theirs) in [
        ("alpha", &alpha, "req:only-in-alpha", "req:only-in-beta"),
        ("beta", &beta, "req:only-in-beta", "req:only-in-alpha"),
    ] {
        assert!(
            graph.get_node(node::REQUIREMENT, mine).unwrap().is_some(),
            "{name} must hold its own write"
        );
        assert!(
            graph.get_node(node::REQUIREMENT, theirs).unwrap().is_none(),
            "{name} must NOT hold the other design's write — this is the silent \
             corruption dec:one-process-many-stores was accepted conditional on"
        );
        assert_eq!(
            graph.count_nodes(node::REQUIREMENT).unwrap(),
            2,
            "{name} must hold exactly its own two requirements, no more"
        );
        assert_eq!(
            graph.count_nodes(node::PROJECT).unwrap(),
            1,
            "{name} must hold exactly one project — its own"
        );
    }

    // And it is true on disk, not merely in the handles: the assertions above
    // would also pass if both handles were views onto one store that happened to
    // be filtering correctly.
    drop(alpha);
    drop(beta);
    let alpha = DesignGraph::open_rocksdb(a_path).unwrap();
    let beta = DesignGraph::open_rocksdb(b_path).unwrap();
    assert!(
        alpha
            .get_node(node::PROJECT, "proj:beta")
            .unwrap()
            .is_none(),
        "Beta's project must not be in Alpha's store after both are closed"
    );
    assert!(
        beta.get_node(node::PROJECT, "proj:alpha")
            .unwrap()
            .is_none(),
        "Alpha's project must not be in Beta's store after both are closed"
    );

    std::fs::remove_dir_all(&a_dir).ok();
    std::fs::remove_dir_all(&b_dir).ok();
}

#[test]
fn knowing_another_designs_graph_id_does_not_get_you_its_contents() {
    // THE STORE IS THE BOUNDARY, NOT THE ID.
    //
    // `graph_id` namespaces every stored key, which invites the assumption that
    // the id is what keeps designs apart — and if that were so, a leaked or
    // guessed id would be a way in, and the hosted case would be handing out
    // capability tokens by accident.
    //
    // It is not so, and this pins it: re-point a handle on store A at store B's
    // id and you get an EMPTY view, because B's bytes are in B's directory. The
    // id is a namespace within a store, never an address across stores.
    let a_dir = tmp("boundary-a");
    let b_dir = tmp("boundary-b");
    let a_path = a_dir.join("graph");
    let b_path = b_dir.join("graph");
    let (a_path, b_path) = (a_path.to_str().unwrap(), b_path.to_str().unwrap());

    let secret_id = {
        let mut b = DesignGraph::open_rocksdb(b_path).unwrap();
        b.add_project("proj:secret", "Not yours").unwrap();
        b.add_requirement("req:secret", "Secret", "Design B's business.")
            .unwrap();
        b.graph_id().to_string()
    };

    let mut a = DesignGraph::open_rocksdb(a_path).unwrap();
    a.add_project("proj:a", "A").unwrap();

    // The most direct attempt available in the API surface: take A's open store
    // and tell it that it is B.
    let impersonating = a.with_graph_id(secret_id.clone());

    assert_eq!(
        impersonating.graph_id(),
        secret_id,
        "the fixture must actually be re-pointed, or this proves nothing"
    );
    assert!(
        impersonating
            .get_node(node::REQUIREMENT, "req:secret")
            .unwrap()
            .is_none(),
        "naming another design's id from a different store must not serve that \
         design's nodes — if this ever passes B's data back, the id has become an \
         address and every hosted design is reachable by anyone who learns its name"
    );
    assert!(
        !impersonating.holds_a_design(),
        "and the impersonated view must be empty, not partially populated"
    );

    // The mirror of the same property: what such a handle WRITES stays in its own
    // store. A shadow design under B's name inside A's directory is confusing,
    // but it is not a breach — and B must be untouched.
    let mut impersonating = impersonating;
    impersonating
        .add_requirement("req:planted", "Planted", "Written while impersonating B.")
        .unwrap();
    drop(impersonating);

    let b = DesignGraph::open_rocksdb(b_path).unwrap();
    assert!(
        b.get_node(node::REQUIREMENT, "req:planted")
            .unwrap()
            .is_none(),
        "a write made under B's id from A's store must NOT appear in B — this is \
         the cross-design write the hosted case must make impossible"
    );
    assert_eq!(
        b.count_nodes(node::REQUIREMENT).unwrap(),
        1,
        "B must still hold exactly its own one requirement"
    );

    std::fs::remove_dir_all(&a_dir).ok();
    std::fs::remove_dir_all(&b_dir).ok();
}

#[test]
fn reopening_re_attaches_to_the_same_design_not_to_a_neighbour() {
    // The reconnect clause, at the level it can be asked today: "a reconnect
    // re-attaches to the same design rather than to none or the first in the
    // registry". There is no registry to be first in, so what is provable is the
    // half underneath it — that a store's identity is a property of the store and
    // survives the presence of other open designs in the same process.
    //
    // `design_identity.rs` already proves an id survives reopening in isolation.
    // What is new here is the neighbour: a second design open at the same time,
    // which is the only condition under which "re-attaches to the FIRST one"
    // could ever happen.
    let mine_dir = tmp("reconnect-mine");
    let other_dir = tmp("reconnect-other");
    let mine_path = mine_dir.join("graph");
    let other_path = other_dir.join("graph");
    let (mine_path, other_path) = (mine_path.to_str().unwrap(), other_path.to_str().unwrap());

    // The neighbour is opened FIRST and stays open, so "the first design this
    // process opened" and "my design" are different answers.
    let mut neighbour = DesignGraph::open_rocksdb(other_path).unwrap();
    neighbour
        .add_project("proj:neighbour", "Neighbour")
        .unwrap();
    let neighbour_id = neighbour.graph_id().to_string();

    let mine_id = {
        let mut mine = DesignGraph::open_rocksdb(mine_path).unwrap();
        mine.add_project("proj:mine", "Mine").unwrap();
        mine.graph_id().to_string()
    };
    assert_ne!(mine_id, neighbour_id);

    // The reconnect, with the neighbour still held open.
    let reconnected = DesignGraph::open_rocksdb(mine_path).unwrap();

    assert_eq!(
        reconnected.graph_id(),
        mine_id,
        "reconnecting must come back to the same design, not to the first one the \
         process opened"
    );
    assert!(
        reconnected
            .get_node(node::PROJECT, "proj:mine")
            .unwrap()
            .is_some(),
        "and the design must be READABLE on reconnect — re-attaching to the right \
         name but an empty view is the same outage to the user"
    );
    assert!(
        reconnected
            .get_node(node::PROJECT, "proj:neighbour")
            .unwrap()
            .is_none(),
        "and the neighbour must not have leaked in"
    );

    drop(neighbour);
    std::fs::remove_dir_all(&mine_dir).ok();
    std::fs::remove_dir_all(&other_dir).ok();
}

#[test]
fn a_store_whose_identity_sidecar_is_lost_is_refused_rather_than_opened_empty() {
    // WRITTEN AGAINST THE REQUIREMENT, NOT AGAINST THE CODE.
    // `cap:hosted-state-on-a-volume`: "a store without its identity sidecar is
    // refused rather than opened empty".
    //
    // The identity lives in `<path>.id.json`, a SIBLING of the store, so the two
    // can be parted. On a laptop that is unlikely. On the mounted volume the
    // hosted case runs on it is an ordinary Tuesday: a partial restore, a
    // snapshot taken mid-write, a sync tool that skips dotfiles, a volume
    // remounted with the sidecar on the other side of the mount point.
    //
    // What happens when they are parted: `identity::resolve` finds no file, probes
    // for a design under the DEFAULT id, finds none (this design is under a minted
    // id), and MINTS A NEW ONE — then writes it to the sidecar. The store's real
    // design is still on disk and is now unreachable, because the id namespaces
    // every key; the graph reports itself healthy and empty; and the original id
    // has been overwritten, so the damage is not obviously undoable by the person
    // it happens to. `holds_default_design` is a probe for the legacy migration,
    // and it cannot see a minted design by construction.
    //
    // Refusing costs an operator one clear error. Opening empty costs them the
    // design, silently, and `dec:two-sided-accept` — "silent drift-accept does not
    // exist" — is the same principle one layer down.
    let dir = tmp("lost-sidecar");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();

    let original_id = {
        let mut g = DesignGraph::open_rocksdb(path).unwrap();
        g.add_project("proj:real", "A real design").unwrap();
        g.add_requirement("req:real", "Real", "Worth not losing.")
            .unwrap();
        g.graph_id().to_string()
    };

    // The volume came back without the sidecar.
    std::fs::remove_file(identity::identity_path(path)).unwrap();

    match DesignGraph::open_rocksdb(path) {
        Err(e) => {
            let message = format!("{e}");
            assert!(
                message.contains("identity") || message.contains("id.json"),
                "the refusal must name what is missing so the operator can put it \
                 back: {message}"
            );
        }
        Ok(g) => {
            // Report the damage precisely rather than just "expected Err": the
            // point of this test is the SHAPE of the failure, and whoever reads
            // this output next should not have to re-derive it.
            let reachable = g.get_node(node::PROJECT, "proj:real").unwrap().is_some();
            let now = g.graph_id().to_string();
            panic!(
                "a store that lost its identity sidecar opened instead of being \
                 refused.\n  original id: {original_id}\n  id after reopen: {now}\n  \
                 the real design is reachable: {reachable}\n\
                 The store still holds the design; it is namespaced under an id that \
                 has now been overwritten in a freshly written sidecar. This is the \
                 'opened empty' cap:hosted-state-on-a-volume forbids."
            );
        }
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_registry_clauses_are_covered_now() {
    // THE TRIPWIRE FIRED AND WAS RETIRED, 2026-08-12 — which is the whole point
    // of having had one. `crates/reflow2-mcp/src/registry.rs` now exists, so a
    // session CAN name a design by graph_id, and the two clauses this file could
    // not reach are covered where that surface lives:
    //
    //   crates/reflow2-mcp/tests/a_session_names_its_design.rs
    //     - an_id_the_registry_does_not_hold_is_refused
    //     - a_real_design_outside_the_root_is_refused_like_any_other   (clause 1)
    //     - a_filesystem_path_is_not_an_alternative_route_in           (clause 2)
    //     - a_binding_carries_one_design_and_offers_no_way_to_ask_for_a_second
    //
    // The risk the tripwire named was never the missing code — it was that the
    // registry would land, the store-level tests above would be read as the
    // precondition met, and the clauses that motivated the condition would never
    // be written. They are written. This assertion keeps the pointer, so a
    // future reader of THIS file is sent to them rather than concluding the
    // clauses are still uncovered.
    let clauses_covered =
        std::path::Path::new("crates/reflow2-mcp/tests/a_session_names_its_design.rs").exists()
            || std::path::Path::new("../reflow2-mcp/tests/a_session_names_its_design.rs").exists();
    assert!(
        clauses_covered,
        "registry.rs exists but a_session_names_its_design.rs does not — the clauses of \
         ver:a-session-cannot-name-another-design have lost their coverage. Restore them \
         before shipping: an id you were not attached to is REFUSED rather than served, and \
         a path is not an alternative route in."
    );
}
