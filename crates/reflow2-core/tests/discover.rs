//! DISCOVER — what design lives at a path, answered without touching it.
//!
//! The load-bearing property is NOT what it reports; it is what it does not do.
//! Opening a store to describe it writes a schema stamp and MINTS an identity
//! where there is none — so the obvious implementation would name a design by
//! the act of inspecting it, and would fail on any design another session holds.
//! Every test here is really one test: describing changes nothing.

use std::path::{Path, PathBuf};

use reflow2_core::{DesignPathState, describe_at};

/// A directory of its own per test — the same idiom as the content and
/// persistence suites, rather than a tempfile dependency for something this
/// small.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reflow2-discover-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every path under `root`, sorted — so a test can assert that describing
/// created nothing.
fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() {
                for child in tree(&p) {
                    out.push(format!("{name}/{child}"));
                }
            }
            out.push(name);
        }
    }
    out.sort();
    out
}

fn write_identity(graph_path: &Path, graph_id: &str, label: &str) {
    let sidecar = graph_path.with_file_name(format!(
        "{}.id.json",
        graph_path.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(
        sidecar,
        format!(
            r#"{{"graph_id":"{graph_id}","label":"{label}","origin":"minted","minted_by":"0.24.0"}}"#
        ),
    )
    .unwrap();
}

/// ⭐ THE PROPERTY EVERYTHING ELSE RESTS ON: describing writes nothing.
///
/// If this ever fails, a describe has started opening the store — which stamps
/// `<path>.meta.json` and mints `<path>.id.json`. The failure would look like a
/// feature (now it reports sizes!) and would mean inspecting an unnamed design
/// names it.
#[test]
fn describing_creates_nothing_and_names_nothing() {
    let dir = scratch("no-writes");
    let store = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(&store).unwrap();

    let before = tree(&dir);
    let d = describe_at(store.to_str().unwrap());
    let after = tree(&dir);

    assert_eq!(
        before, after,
        "describing must create NOTHING — an identity minted by inspection is the defect this \
         exists to prevent"
    );
    // And a store with no identity is reported as such rather than named.
    assert_eq!(d.state, DesignPathState::Unnamed, "{d:?}");
    assert!(d.graph_id.is_none());
}

/// The distinction the whole thing turns on: something-here-but-unnamed must
/// never read as nothing-here, because "no design here" is the sentence that
/// starts an unwanted one.
#[test]
fn a_store_without_an_identity_is_unnamed_not_absent() {
    let dir = scratch("unnamed");
    let store = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(&store).unwrap();

    let d = describe_at(store.to_str().unwrap());
    assert_ne!(d.state, DesignPathState::Absent, "{d:?}");
    assert_eq!(d.state, DesignPathState::Unnamed);
    assert!(
        d.reading.to_lowercase().contains("without opening"),
        "the reading must say WHY it cannot name it: {}",
        d.reading
    );
}

/// A broken identity file is a reason to stop, not a reason to start.
#[test]
fn an_unreadable_identity_is_reported_not_swallowed() {
    let dir = scratch("broken");
    let store = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.with_file_name("graph.id.json"), "{ this is not json").unwrap();

    let d = describe_at(store.to_str().unwrap());
    assert_eq!(d.state, DesignPathState::Unnamed, "{d:?}");
    assert!(
        d.reading.contains("Do NOT start a new design"),
        "a broken identity must warn against starting one: {}",
        d.reading
    );
}

/// The ordinary case: a design is named, from the sidecar alone.
#[test]
fn a_real_design_is_named_from_its_sidecar() {
    let dir = scratch("named");
    let store = dir.join(".reflow2").join("graph");
    std::fs::create_dir_all(&store).unwrap();
    write_identity(&store, "8a9ce30fedd213e5", "simulated_beamlines");

    let d = describe_at(store.to_str().unwrap());
    assert_eq!(d.state, DesignPathState::Design);
    assert_eq!(d.graph_id.as_deref(), Some("8a9ce30fedd213e5"));
    assert_eq!(d.label.as_deref(), Some("simulated_beamlines"));
    assert_eq!(d.origin.as_deref(), Some("minted"));
    assert!(
        d.reading.contains("simulated_beamlines"),
        "the reading must be usable as a menu line: {}",
        d.reading
    );
}

/// Opted in and empty is a real state, distinct from both absent and named:
/// starting a design HERE is expected, and saying so prevents a needless prompt.
#[test]
fn an_opted_in_but_empty_directory_says_so() {
    let dir = scratch("opted-in");
    std::fs::create_dir_all(dir.join(".reflow2")).unwrap();
    let store = dir.join(".reflow2").join("graph");

    let d = describe_at(store.to_str().unwrap());
    assert_eq!(d.state, DesignPathState::OptedIn, "{d:?}");
    assert!(d.graph_id.is_none());
}

/// Nothing there is nothing there — the one case where "no design" is the whole
/// truth.
#[test]
fn a_path_with_nothing_is_absent() {
    let dir = scratch("absent");
    let store = dir.join("nowhere").join("graph");
    let d = describe_at(store.to_str().unwrap());
    assert_eq!(d.state, DesignPathState::Absent, "{d:?}");
    assert_eq!(d.reading, "no design here.");
}

/// The sweep case this is built for: several paths, mixed states, one answer
/// each. A caller showing a person a menu must never lose a row.
#[test]
fn a_sweep_answers_every_path_including_the_uninteresting_ones() {
    let dir = scratch("sweep");
    let a = dir.join("root").join(".reflow2").join("graph");
    let b = dir.join("root").join("sub").join(".reflow2").join("graph");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    write_identity(&a, "37bbd9a6e0e75369", "hxm_program");
    write_identity(&b, "6771abb9b0683423", "HEX");
    let missing = dir.join("root").join("gone").join(".reflow2").join("graph");

    let results: Vec<_> = [&a, &b, &missing]
        .iter()
        .map(|p| describe_at(p.to_str().unwrap()))
        .collect();

    assert_eq!(
        results.len(),
        3,
        "every path gets a row, including absent ones"
    );
    assert_eq!(results[0].label.as_deref(), Some("hxm_program"));
    assert_eq!(results[1].label.as_deref(), Some("HEX"));
    assert_eq!(results[2].state, DesignPathState::Absent);
    // Each row carries its own path, so a caller need not track correspondence.
    assert_eq!(results[1].path, b.to_str().unwrap());
}
