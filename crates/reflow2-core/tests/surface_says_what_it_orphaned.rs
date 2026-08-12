//! A published surface says which of the nodes it KEPT lost their container.
//!
//! # The defect, found by running the thing rather than reading it
//!
//! `export_surface` already reports what it WITHHELD — `withheld_nodes`,
//! `withheld_edges`, and a note, because "a recipient cannot tell a small design
//! from a heavily filtered one". It does not report what the withholding did to
//! what it KEPT.
//!
//! Measured 2026-08-12 on the flo2 ↔ reflow2 trial. flo2's surface exposed four
//! `subsystem` components because they provide published interfaces, and
//! withheld BOTH anchors that give them a place in the hierarchy: the
//! `system`-level `cmp:flo2-platform` that contains them, and the
//! `proj:flo2 CONTAINS <component>` edges (46 such edges in the full design, 5
//! in the surface — Interfaces only).
//!
//! Mirrored into a host graph, `hierarchy_issues` went 0 → 4, every one of them
//! `orphan_level`: *"'cmp:edge-gateway' (subsystem) is not contained by anything
//! above it and contains nothing below it"*. **That finding is false** — in flo2
//! `cmp:flo2-platform` contains it — and the detector is innocent: it reported
//! exactly what the document said. **The document lied by omission.**
//!
//! # Why this REPORTS rather than repairs
//!
//! Three fixes were considered and two were rejected for stated reasons.
//! Carrying the ancestry would disclose internal structure the surface exists to
//! withhold. Re-parenting the orphan to the Project would assert a direct
//! `CONTAINS` nobody drew, which is the fabrication
//! `req:a-repair-suggestion-never-proposes-fabrication` forbids. Dropping the
//! child would delete the provider of a published contract.
//!
//! So the surface says what it did, and the recipient decides — the same
//! discipline as `withheld_nodes` next door, and the reason `granularity` and
//! `consumption` carry `not_observed_about`.

use reflow2_core::nodes::node;
use reflow2_core::{DesignGraph, SurfaceExport};

/// flo2's real shape, reduced: a Project, a `system` component that contains
/// everything, and a `subsystem` under it that provides the published boundary.
/// Exporting the surface keeps the subsystem and withholds its parent.
fn platform_with_a_hidden_parent() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:flo2", "flo2").unwrap();

    g.add_component(
        "cmp:platform",
        "flo2 platform",
        "the whole thing",
        Some("system"),
    )
    .unwrap();
    g.contains("proj:flo2", node::COMPONENT, "cmp:platform")
        .unwrap();

    g.add_component(
        "cmp:edge",
        "Edge gateway",
        "serves the browser",
        Some("subsystem"),
    )
    .unwrap();
    // THE ONLY THING ANCHORING IT is a parent the surface will withhold.
    g.contain_component("cmp:platform", "cmp:edge").unwrap();

    g.add_interface("ifc:browser-edge", "Browser-facing edge")
        .unwrap();
    g.provides("cmp:edge", "ifc:browser-edge").unwrap();
    g.set_interface_designation("ifc:browser-edge", "published")
        .unwrap();
    g
}

fn severed(s: &SurfaceExport) -> Vec<(String, String)> {
    s.severed_containment
        .iter()
        .map(|x| (x.node_id.clone(), x.withheld_parent.clone()))
        .collect()
}

// THE DEFECT CASE. The surface keeps `cmp:edge` because it provides a published
// boundary, and withholds the only thing that contains it.
#[test]
fn a_kept_node_whose_container_was_withheld_is_named() {
    let g = platform_with_a_hidden_parent();
    let s = g.export_surface().unwrap();

    let kept: Vec<&str> = s
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    assert!(
        kept.contains(&"cmp:edge"),
        "precondition: the child is kept"
    );
    assert!(
        !kept.contains(&"cmp:platform"),
        "precondition: the parent is withheld — that is what makes this a severance"
    );

    assert_eq!(
        severed(&s),
        vec![("cmp:edge".to_string(), "cmp:platform".to_string())],
        "the surface must NAME the node it orphaned and the container it withheld"
    );
}

// COUNTERWEIGHT, and the one that decides whether this is usable: a node whose
// container IS in the surface must not be reported. A check that fired on every
// kept node would be noise, and the Project is included in every surface.
#[test]
fn a_node_whose_container_survives_is_not_reported() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:sat", "Constellation").unwrap();
    g.add_component(
        "cmp:terminal",
        "Terminal",
        "serves the link",
        Some("subsystem"),
    )
    .unwrap();
    // Contained by the PROJECT, which every surface carries.
    g.contains("proj:sat", node::COMPONENT, "cmp:terminal")
        .unwrap();
    g.add_interface("ifc:link", "Optical crosslink").unwrap();
    g.provides("cmp:terminal", "ifc:link").unwrap();
    g.set_interface_designation("ifc:link", "published")
        .unwrap();

    let s = g.export_surface().unwrap();
    assert!(
        severed(&s).is_empty(),
        "its container is in the document, so nothing was severed: {:?}",
        severed(&s)
    );
}

// NO SILENT CAPS. The field is present and EMPTY on a clean surface, so a
// recipient can tell "nothing was orphaned" from "this build does not say".
#[test]
fn a_clean_surface_reports_an_empty_list_rather_than_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_interface("ifc:only", "The only boundary").unwrap();
    g.set_interface_designation("ifc:only", "published")
        .unwrap();

    let s = g.export_surface().unwrap();
    assert!(s.severed_containment.is_empty());
}

// ⚠️ THIS REPORTS, IT DOES NOT REPAIR — pinned, because the two rejected fixes
// both change content. Carrying the ancestry would disclose internals the
// surface exists to withhold; re-parenting to the Project would assert a
// CONTAINS nobody drew. The document must be byte-identical either way.
#[test]
fn the_document_itself_is_unchanged_by_the_reporting() {
    let g = platform_with_a_hidden_parent();
    let s = g.export_surface().unwrap();

    let kept: Vec<&str> = s
        .document
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    assert!(
        !kept.contains(&"cmp:platform"),
        "the withheld parent must NOT be pulled in — that would leak internals"
    );
    let invented: Vec<_> = s
        .document
        .edges
        .iter()
        .filter(|e| e.edge_type == "CONTAINS" && e.from_id == "proj:flo2" && e.to_id == "cmp:edge")
        .collect();
    assert!(
        invented.is_empty(),
        "no CONTAINS may be invented to re-anchor the orphan — that is fabrication"
    );
}

// AND IT MUST BE SAID IN WORDS. A field nobody prints is a comment: the note is
// what a human actually reads, and it already carries the withheld counts.
#[test]
fn the_note_says_so_when_something_was_orphaned() {
    let g = platform_with_a_hidden_parent();
    let s = g.export_surface().unwrap();
    let n = s.note.to_lowercase();
    assert!(
        n.contains("container") || n.contains("contain"),
        "the note must mention the severed containment: {}",
        s.note
    );
    assert!(
        s.note.contains("cmp:edge"),
        "and name what it orphaned, not just count it: {}",
        s.note
    );
}

// A clean surface's note must NOT gain the clause — otherwise every recipient
// reads a warning about nothing and learns to skip the note.
#[test]
fn a_clean_surface_note_carries_no_severance_clause() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_interface("ifc:only", "The only boundary").unwrap();
    g.set_interface_designation("ifc:only", "published")
        .unwrap();

    let s = g.export_surface().unwrap();
    assert!(
        !s.note.to_lowercase().contains("container"),
        "a clean surface must not warn about containment: {}",
        s.note
    );
}
