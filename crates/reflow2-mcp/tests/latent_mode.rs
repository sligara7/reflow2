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

// ---- a design that ARRIVES after the surface was chosen ---------------------
//
// music_graph F24, 2026-08-16: the server was started against an empty
// directory, said so truthfully, and ninety seconds later `--import` built a
// full store underneath it. Nothing re-probes, so every design tool stayed
// absent for the rest of the session — and the ONE tool on offer was the one
// that starts a design, over the top of the one that now existed.
//
// The surface itself still cannot change mid-session: the tool router is fixed
// per service and clients cache the tool list. So the fix is that the one tool
// served REFUSES and says what happened, rather than beginning a second design.

/// THE CASE. A real design is present now; `reflow2_start_design` must not
/// offer to begin one, and must say a restart is what attaches it.
#[tokio::test]
async fn start_design_refuses_once_a_design_has_arrived() {
    let dir = scratch("arrived");
    let graph = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(graph.parent().expect("parent")).expect("marker dir");

    // A design's identity sidecar is what makes it nameable without opening it,
    // and only a real store writes one — so build one the way a restore does.
    let mut g = reflow2_core::DesignGraph::open_rocksdb(graph.to_str().unwrap()).expect("open");
    g.add_project("proj:x", "X").expect("project");
    drop(g);

    let svc = reflow2_mcp::latent::LatentService::new(graph.display().to_string());
    let out = svc
        .reflow2_start_design(rmcp::handler::server::wrapper::Parameters(
            reflow2_mcp::latent::NoArgs {},
        ))
        .await
        .expect("call");
    let v = out.structured_content.expect("structured");

    assert_eq!(
        v["started"], false,
        "it must not claim to have started anything: {v}"
    );
    assert_eq!(v["a_design_is_already_here"], true, "{v}");
    let text = v.to_string();
    assert!(
        text.contains("restart"),
        "it must name the restart that actually attaches the surface: {v}"
    );
    assert!(
        text.contains("import"),
        "and name the restore path, which is how this state is reached: {v}"
    );
}

/// COUNTERWEIGHT, and the one that decides whether this is a fix or a new bug:
/// the ORDINARY case must still work. An undesigned directory still opts in and
/// still gets the reconnect instruction — if this refusal fired here, a
/// machine-wide install could never start a design at all.
#[tokio::test]
async fn an_undesigned_directory_still_starts_normally() {
    let dir = scratch("normal-start");
    let graph = dir.join(".reflow2").join("graph");

    let svc = reflow2_mcp::latent::LatentService::new(graph.display().to_string());
    let out = svc
        .reflow2_start_design(rmcp::handler::server::wrapper::Parameters(
            reflow2_mcp::latent::NoArgs {},
        ))
        .await
        .expect("call");
    let v = out.structured_content.expect("structured");

    assert_eq!(v["started"], true, "the ordinary path must still work: {v}");
    assert!(v.get("a_design_is_already_here").is_none(), "{v}");
    assert!(
        graph.parent().expect("parent").exists(),
        "and it must actually create the opt-in directory"
    );
}

/// COUNTERWEIGHT 2, the sharp one. `design_present` deliberately treats a bare
/// `.reflow2/` directory as opted in — that is how the window between `init`
/// and the first write is covered. But OPTED IN IS NOT DESIGNED, and conflating
/// them here would make `reflow2_start_design` refuse on exactly the projects
/// that most need it: the ones set up and not yet written to.
#[tokio::test]
async fn an_opted_in_directory_with_no_store_is_not_treated_as_designed() {
    let dir = scratch("opted-in");
    let graph = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(graph.parent().expect("parent")).expect("marker dir");
    assert!(
        design_present(graph.to_str().unwrap()),
        "precondition: the marker alone reads as opted in"
    );

    let svc = reflow2_mcp::latent::LatentService::new(graph.display().to_string());
    let out = svc
        .reflow2_start_design(rmcp::handler::server::wrapper::Parameters(
            reflow2_mcp::latent::NoArgs {},
        ))
        .await
        .expect("call");
    let v = out.structured_content.expect("structured");

    assert!(
        v.get("a_design_is_already_here").is_none(),
        "opted in is not designed — refusing here would strand a project that \
         was set up and never written to: {v}"
    );
}
