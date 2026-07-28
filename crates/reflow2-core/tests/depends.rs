//! Declaring which version of another design you depend on, and checking that
//! claim against the build (`req:design-dependencies-declared`).
//!
//! The trial made this load-bearing rather than convenient: without a recorded
//! pin there is nothing for a seam analysis to be taken *as of*, and the two
//! real consumers of dynograph-foundation sit two minors apart, so a surface
//! from `main` describes neither of them.

use reflow2_core::{DependencyDeclaration, DesignGraph, ObservedDependency};

fn decl() -> DependencyDeclaration {
    DependencyDeclaration {
        id: "dep:dynograph-foundation".into(),
        name: "dynograph-foundation".into(),
        source: "https://github.com/sligara7/dynograph-foundation.git".into(),
        version: "v0.11.0".into(),
        components: vec![
            "dynograph-core".into(),
            "dynograph-storage".into(),
            "dynograph-graph".into(),
            "dynograph-resolution".into(),
            "dynograph-vector".into(),
        ],
        features: vec!["rocksdb".into(), "fulltext".into()],
        declared_in: Some("Cargo.toml".into()),
        note: Some("v0.12.0 verified safe to take: built and tested against it.".into()),
    }
}

fn observed(version: &str) -> ObservedDependency {
    ObservedDependency {
        name: "dynograph-foundation".into(),
        version: version.into(),
        components: vec!["dynograph-core".into(), "dynograph-storage".into()],
        features: vec!["rocksdb".into()],
        observed_in: Some("Cargo.toml".into()),
    }
}

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.add_project("prj:p", "Consumer").unwrap();
    g
}

#[test]
fn a_declaration_survives_the_round_trip() {
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();

    let back = g.declared_dependencies().unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0], decl(), "everything declared must come back intact");
}

#[test]
fn a_declaration_without_a_version_is_refused() {
    // The version IS the declaration. Without it the document says "we depend on
    // them", which nobody needed writing down.
    let mut g = graph();
    let mut d = decl();
    d.version = "  ".into();
    let err = g.declare_dependency(&d).expect_err("must be refused");
    assert!(
        err.to_string().contains("AS OF"),
        "the refusal should say why the version matters: {err}"
    );
}

#[test]
fn a_build_taking_something_undeclared_is_reported() {
    // The dangerous state from the trial: relying on something nobody agreed to,
    // which breaks with nobody at fault.
    let g = graph();
    let report = g.reconcile_dependencies(&[observed("v0.11.0")]).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, "undeclared");
}

#[test]
fn a_pin_the_build_has_moved_past_is_reported() {
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();

    let report = g.reconcile_dependencies(&[observed("v0.12.0")]).unwrap();
    let kinds: Vec<&str> = report.findings.iter().map(|f| f.kind).collect();
    assert!(
        kinds.contains(&"version_mismatch"),
        "declared v0.11.0 against a build resolving v0.12.0 must be reported: {kinds:?}"
    );
}

#[test]
fn a_declaration_the_build_no_longer_takes_is_reported() {
    // The opposite failure, and it must not be silent either: a stale
    // declaration is a promise about something you no longer use.
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();

    let report = g.reconcile_dependencies(&[]).unwrap();
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].kind, "unobserved");
}

#[test]
fn a_feature_forwarded_by_name_but_never_declared_is_reported() {
    // Feature names are contract whether or not the provider thinks so — a
    // rename is a downstream build break no API diff would mention. This is the
    // exact reliance the trial surfaced and the provider then committed to.
    let mut g = graph();
    let mut d = decl();
    d.features = vec!["rocksdb".into()];
    g.declare_dependency(&d).unwrap();

    let mut o = observed("v0.11.0");
    o.features = vec!["rocksdb".into(), "fulltext".into()];
    let report = g.reconcile_dependencies(&[o]).unwrap();
    let kinds: Vec<&str> = report.findings.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&"undeclared_feature"), "{kinds:?}");
}

#[test]
fn agreement_between_declaration_and_build_says_so_plainly() {
    let mut g = graph();
    let mut d = decl();
    d.components = vec!["dynograph-core".into(), "dynograph-storage".into()];
    d.features = vec!["rocksdb".into()];
    g.declare_dependency(&d).unwrap();

    let report = g.reconcile_dependencies(&[observed("v0.11.0")]).unwrap();
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(report.note.contains("agrees"), "{}", report.note);
}

#[test]
fn declaring_nothing_reads_as_nobody_has_said() {
    // Same false-green rule the trial produced twice: an empty declaration set
    // must never read as "this design depends on nothing".
    let g = graph();
    let report = g.reconcile_dependencies(&[]).unwrap();
    assert!(report.declared.is_empty());
    assert!(
        report.note.contains("nobody has said"),
        "silence must be labelled as silence: {}",
        report.note
    );
}

#[test]
fn the_manifest_says_which_reflow2_wrote_it() {
    // Anthony's ask, and the same reasoning as the export's version stamp: a
    // file whose producer is unknown cannot be read safely by a tool that has
    // since changed what the fields mean.
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();

    let toml = g.dependency_manifest().unwrap();
    assert!(toml.contains("[reflow2]"), "{toml}");
    assert!(
        toml.contains(&format!("version = \"{}\"", env!("CARGO_PKG_VERSION"))),
        "the manifest must name the reflow2 that produced it:\n{toml}"
    );
    assert!(
        toml.contains("[dependencies.dynograph-foundation]"),
        "{toml}"
    );
    assert!(toml.contains("v0.11.0"), "{toml}");
    assert!(toml.contains("fulltext"), "features must travel:\n{toml}");
}

#[test]
fn an_empty_manifest_still_says_which_reflow2_wrote_it_and_that_it_is_silent() {
    let g = graph();
    let toml = g.dependency_manifest().unwrap();
    assert!(toml.contains("[reflow2]"), "{toml}");
    assert!(
        toml.contains("NOTHING DECLARED"),
        "an empty manifest must say so rather than look like a complete one:\n{toml}"
    );
}
