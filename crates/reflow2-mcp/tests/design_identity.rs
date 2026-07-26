//! A design knows its own name, and remembers it across opens.
//!
//! `req:design-identity` / `dec:identity-out-of-band`. These are on-disk tests
//! and live in this crate because it is the one that always has the RocksDB
//! backend; the behaviour under test is `reflow2_core::identity`.
//!
//! The property that matters most is not that new graphs get an id — it is that
//! **existing ones do not**. Every graph that exists today holds its design
//! under the old shared id, `graph_id` is part of the export's content hash,
//! and a graph reopened under a name it was not created with finds nothing and
//! presents as an *empty design*. Minting for those would have been silent data
//! loss dressed as a feature.

use reflow2_core::DesignGraph;
use reflow2_core::identity::{self, Origin};

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-identity-{}-{}-{name}",
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
fn a_new_design_mints_a_name_of_its_own() {
    let dir = tmp("new");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();

    {
        let mut g = DesignGraph::open_rocksdb(path).unwrap();
        g.add_project("proj:new", "New").unwrap();
        assert_ne!(
            g.graph_id(),
            reflow2_core::DEFAULT_GRAPH_ID,
            "a fresh design must not be born with the shared name"
        );
    }

    let id = identity::resolve(path, reflow2_core::DEFAULT_GRAPH_ID, || {
        panic!("must not re-probe once established")
    })
    .unwrap();
    assert_eq!(id.origin, Origin::Minted);
    assert_eq!(id.graph_id.len(), 16);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_name_survives_reopening() {
    // The whole requirement in one assertion: "remembers it across opens".
    let dir = tmp("stable");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();

    let first = {
        let mut g = DesignGraph::open_rocksdb(path).unwrap();
        g.add_project("proj:s", "Stable").unwrap();
        g.graph_id().to_string()
    };
    let second = {
        let g = DesignGraph::open_rocksdb(path).unwrap();
        g.graph_id().to_string()
    };

    assert_eq!(first, second);
    // And the design is still there, which is the failure this guards.
    let g = DesignGraph::open_rocksdb(path).unwrap();
    assert!(
        g.get_node(reflow2_core::nodes::node::PROJECT, "proj:s")
            .unwrap()
            .is_some(),
        "reopening under the remembered name must find the design"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn two_new_designs_are_told_apart() {
    // Why this exists at all: mirror_surface refuses a surface whose source is
    // the importing graph, and with one shared constant that check could never
    // pass for anybody.
    let a = tmp("a");
    let b = tmp("b");
    let ga = DesignGraph::open_rocksdb(a.join("graph").to_str().unwrap()).unwrap();
    let gb = DesignGraph::open_rocksdb(b.join("graph").to_str().unwrap()).unwrap();

    assert_ne!(ga.graph_id(), gb.graph_id());
    std::fs::remove_dir_all(&a).ok();
    std::fs::remove_dir_all(&b).ok();
}

#[test]
fn a_graph_that_already_holds_a_design_keeps_the_old_shared_name() {
    // THE migration test. Simulates every graph that exists today: design data
    // under the default id, and no identity file beside it.
    let dir = tmp("existing");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();

    // A pre-identity graph, exactly: design data written under the shared id,
    // and no identity file beside the store.
    {
        let mut legacy = DesignGraph::open_rocksdb(path)
            .unwrap()
            .with_graph_id(reflow2_core::DEFAULT_GRAPH_ID);
        legacy.add_project("proj:old", "Existing design").unwrap();
        legacy
            .add_requirement("req:old", "Old", "Captured before identity existed.")
            .unwrap();
    }
    std::fs::remove_file(identity::identity_path(path)).unwrap();

    let g = DesignGraph::open_rocksdb(path).unwrap();

    assert_eq!(
        g.graph_id(),
        reflow2_core::DEFAULT_GRAPH_ID,
        "an existing design keeps the id its data is stored under"
    );
    assert!(
        g.get_node(reflow2_core::nodes::node::REQUIREMENT, "req:old")
            .unwrap()
            .is_some(),
        "and the design is still readable — the point of adopting rather than minting"
    );
    let id = identity::resolve(path, reflow2_core::DEFAULT_GRAPH_ID, || unreachable!()).unwrap();
    assert_eq!(id.origin, Origin::Adopted, "and it says so on the record");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unreadable_identity_is_refused_rather_than_defaulted() {
    // Defaulting would open a DIFFERENT design at the same path and report
    // nothing wrong — the failure this whole file exists to prevent.
    let dir = tmp("corrupt");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();
    {
        let mut g = DesignGraph::open_rocksdb(path).unwrap();
        g.add_project("proj:c", "Corrupt").unwrap();
    }
    std::fs::write(identity::identity_path(path), "{ not json").unwrap();

    let message = match DesignGraph::open_rocksdb(path) {
        Ok(_) => panic!("an unreadable identity must not open"),
        Err(e) => format!("{e}"),
    };

    assert!(message.contains("not readable"), "{message}");
    assert!(
        message.contains("will not guess"),
        "the refusal must say why it refuses: {message}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_label_can_change_and_the_id_cannot() {
    let dir = tmp("label");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();
    let original = {
        let mut g = DesignGraph::open_rocksdb(path).unwrap();
        g.add_project("proj:l", "Labelled").unwrap();
        g.graph_id().to_string()
    };

    let renamed = identity::set_label(path, "The Satellite Program").unwrap();

    assert_eq!(renamed.label, "The Satellite Program");
    assert_eq!(
        renamed.graph_id, original,
        "renaming must not move the id — every stored key and every export names it"
    );
    let reopened = DesignGraph::open_rocksdb(path).unwrap();
    assert_eq!(reopened.graph_id(), original);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn two_on_disk_designs_can_finally_mirror_each_other() {
    // The payoff, and the reason req:design-identity was filed in the first
    // place. `mirror_surface` refuses a surface whose source id equals the
    // importing graph's — a filtered copy of your own design would overwrite the
    // full one. While every graph answered to one constant, that guard could
    // never pass for ANY pair of real designs, so composition between them was
    // impossible on disk. `open_in_memory_as` proved the mechanism in tests; it
    // could not make it work for a user.
    let space_dir = tmp("space");
    let ground_dir = tmp("ground");
    let space_path = space_dir.join("graph");
    let ground_path = ground_dir.join("graph");

    let surface = {
        let mut space = DesignGraph::open_rocksdb(space_path.to_str().unwrap()).unwrap();
        space.add_project("proj:space", "Space segment").unwrap();
        space
            .add_component("cmp:terminal", "Crosslink terminal", "flight side", None)
            .unwrap();
        space
            .add_interface("ifc:crosslink", "Optical crosslink")
            .unwrap();
        space.provides("cmp:terminal", "ifc:crosslink").unwrap();
        space
            .set_interface_designation("ifc:crosslink", "published")
            .unwrap();
        space
            .add_requirement("req:power", "Power budget", "Under 40 W.")
            .unwrap();
        space.export_surface().unwrap().document
    };

    let mut ground = DesignGraph::open_rocksdb(ground_path.to_str().unwrap()).unwrap();
    ground.add_project("proj:ground", "Ground segment").unwrap();

    let report = ground.mirror_surface(&surface, Some("2026-07-25")).unwrap();

    assert!(
        report.collisions.is_empty(),
        "two independently created designs must not read as colliding: {report:?}"
    );
    assert_ne!(
        report.mirror_of, "",
        "the mirror records WHOSE design it took"
    );
    assert!(
        ground
            .get_node(reflow2_core::nodes::node::INTERFACE, "ifc:crosslink")
            .unwrap()
            .is_some(),
        "their published contract arrived"
    );
    assert!(
        ground
            .get_node(reflow2_core::nodes::node::REQUIREMENT, "req:power")
            .unwrap()
            .is_none(),
        "and their internals did not"
    );
    std::fs::remove_dir_all(&space_dir).ok();
    std::fs::remove_dir_all(&ground_dir).ok();
}

#[test]
fn an_empty_store_takes_the_name_of_the_design_it_restores() {
    // Found by the smoke test the hour identity landed: importing a whole
    // design into a fresh graph is a RESTORE — same design, new store — and if
    // the empty graph kept the id it minted at open, the round trip would stop
    // coming back byte-identical, because graph_id is inside the content hash.
    let source_dir = tmp("restore-source");
    let restored_dir = tmp("restore-target");
    let source_path = source_dir.join("graph");
    let restored_path = restored_dir.join("graph");

    let (original_id, document) = {
        let mut g = DesignGraph::open_rocksdb(source_path.to_str().unwrap()).unwrap();
        g.add_project("proj:r", "Restorable").unwrap();
        g.add_requirement("req:r", "R", "Something.").unwrap();
        (g.graph_id().to_string(), g.export_graph().unwrap())
    };

    let mut restored = DesignGraph::open_rocksdb(restored_path.to_str().unwrap()).unwrap();
    let adopted = identity::adopt_on_import(
        restored_path.to_str().unwrap(),
        &document.graph_id,
        restored.holds_a_design(),
    )
    .unwrap()
    .expect("an empty store adopts");
    restored = restored.with_graph_id(adopted.graph_id);
    restored.import_graph(&document).unwrap();

    assert_eq!(restored.graph_id(), original_id, "same design, new store");
    assert_eq!(
        restored.export_graph().unwrap().content_hash,
        document.content_hash,
        "and the round trip is byte-identical, which is what graph_id being in the hash costs"
    );
    std::fs::remove_dir_all(&source_dir).ok();
    std::fs::remove_dir_all(&restored_dir).ok();
}

#[test]
fn a_store_that_already_holds_a_design_keeps_its_name_when_importing() {
    // The other half, and the one that protects the stale-seat remedy:
    // absorbing the shared record into a working graph must never rename it.
    let dir = tmp("import-into-live");
    let path = dir.join("graph");
    let path = path.to_str().unwrap();

    let my_id = {
        let mut mine = DesignGraph::open_rocksdb(path).unwrap();
        mine.add_project("proj:mine", "Mine").unwrap();
        let id = mine.graph_id().to_string();
        let adopted =
            identity::adopt_on_import(path, "somebody-elses-design", mine.holds_a_design())
                .unwrap();
        assert!(
            adopted.is_none(),
            "a graph with a design keeps its own name"
        );
        id
    };

    // Reopened in a separate scope: the store is single-writer, so holding the
    // first handle open would deadlock against itself.
    assert_eq!(
        DesignGraph::open_rocksdb(path).unwrap().graph_id(),
        my_id,
        "and reopening confirms it, because a rename here would be silent"
    );
    std::fs::remove_dir_all(&dir).ok();
}
