//! Latent mode: reflow2 installed once per machine must cost an undesigned
//! directory NOTHING.
//!
//! Both halves are load-bearing and pull against each other, so both are pinned
//! here. If latent mode stops firing, a machine-wide install drops a RocksDB
//! store into every directory the user ever opens a session in. If it fires too
//! eagerly, a real project silently loses its design surface — which is the
//! `req:never-silently-absent` failure, arriving by a new route.

use reflow2_mcp::latent::design_present;

/// The crate has no dev-dependency on `tempfile` and this test needs no
/// cleanup guarantees beyond a unique path, so it follows the convention the
/// other suites here use: a pid-and-name-keyed directory under the system temp.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("reflow2-latent-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn an_undesigned_directory_is_latent() {
    let dir = scratch("undesigned");
    let graph = dir.join(".reflow2").join("graph");
    assert!(
        !design_present(graph.to_str().unwrap()),
        "a directory with no .reflow2 has not opted into a design"
    );
}

#[test]
fn the_marker_directory_alone_is_enough() {
    // `reflow2 init` creates `.reflow2/` before anything creates the store, and
    // the store only appears on the first write. Requiring the store itself
    // would leave a freshly-initialised project latent — set up, and still
    // served no design tools.
    let dir = scratch("marker");
    let reflow2 = dir.join(".reflow2");
    std::fs::create_dir_all(&reflow2).expect("mkdir");
    let graph = reflow2.join("graph");
    assert!(
        design_present(graph.to_str().unwrap()),
        "`.reflow2/` present means this project opted in, store or no store"
    );
}

#[test]
fn an_existing_graph_is_never_latent() {
    let dir = scratch("existing");
    let graph = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(&graph).expect("mkdir");
    assert!(
        design_present(graph.to_str().unwrap()),
        "a graph that exists must always be served — latent mode must never \
         take the design surface away from a project that has one"
    );
}

#[test]
fn a_bare_relative_path_does_not_read_as_present() {
    // `--graph-path graph` has an empty parent, and `Path::new("").exists()` is
    // false — but relying on that would be relying on an accident. An empty
    // parent must mean "no marker", not "the current directory, which always
    // exists", or every bare path would read as opted-in.
    assert!(
        !design_present("definitely-not-here-graph"),
        "a bare path with no parent and no store is not a design"
    );
}
