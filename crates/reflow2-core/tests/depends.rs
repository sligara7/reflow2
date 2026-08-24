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
        // The ordinary case: a build dependency nobody has said is also a
        // design. The graph_id-bearing case has its own test below.
        graph_id: None,
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
fn a_declaration_an_accepted_decision_retired_is_skipped_and_named() {
    // THE OPPOSITE OF THE TEST ABOVE, and the distinction is the whole point:
    // a STALE declaration and a RETIRED one look identical to a reconciler that
    // only asks "is it in the build?". One is a promise about something you no
    // longer use; the other is design history about something you deliberately
    // stopped using.
    //
    // Found by dogfooding on 2026-08-24: reflow2 absorbed dynograph-foundation,
    // retired `dep:dynograph-foundation` correctly — deprecation ChangeEvent,
    // snapshot, OBSOLETES from the accepted Decision — and the design gate went
    // on failing `unobserved` anyway, because this reader never asked whether
    // the declaration had been withdrawn. A correct retirement had become a
    // permanently red gate.
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();
    g.add_decision(
        "dec:absorbed",
        "The dependency was absorbed",
        "The code is in-tree; nothing links the crate any more.",
        None,
    )
    .unwrap();
    // A Decision lands `proposed`; only the owner's word moves it, and
    // `is_discontinued` reads that status rather than the mere existence of an
    // OBSOLETES edge. Writing this test the short way — passing "accepted" to
    // `add_decision` — set the RATIONALE instead, because the fourth parameter
    // is rationale, and the assertion failed on exactly the rule it needed.
    g.set_decision_status("dec:absorbed", "accepted").unwrap();
    g.create_edge(
        "OBSOLETES",
        "Decision",
        "dec:absorbed",
        "Resource",
        "dep:dynograph-foundation",
        reflow2_core::nodes::Props::new(),
    )
    .unwrap();

    let report = g.reconcile_dependencies(&[]).unwrap();
    assert!(
        report.findings.is_empty(),
        "a withdrawn declaration must not report as stale: {:?}",
        report.findings
    );
    // SKIPPED IS NOT SILENCED. A declaration that vanishes from the report with
    // no trace is the silent-success failure this project guards against
    // everywhere else, so the reader says which ones it stepped over.
    assert_eq!(report.retired_declarations, vec!["dynograph-foundation"]);
    // And it is still DECLARED — retiring records an ending, it does not erase.
    assert_eq!(report.declared.len(), 1);
}

#[test]
fn a_declaration_retired_by_a_proposed_decision_still_reports() {
    // `is_discontinued` requires the withdrawing Decision to be ACCEPTED.
    // Somebody proposing a removal is not the same as the owner agreeing to it,
    // and the gate must keep asking until they do.
    let mut g = graph();
    g.declare_dependency(&decl()).unwrap();
    g.add_decision("dec:maybe", "Maybe drop it", "Thinking about it.", None)
        .unwrap();
    g.create_edge(
        "OBSOLETES",
        "Decision",
        "dec:maybe",
        "Resource",
        "dep:dynograph-foundation",
        reflow2_core::nodes::Props::new(),
    )
    .unwrap();

    let report = g.reconcile_dependencies(&[]).unwrap();
    assert_eq!(
        report.findings.len(),
        1,
        "a proposal does not retire anything"
    );
    assert_eq!(report.findings[0].kind, "unobserved");
    assert!(report.retired_declarations.is_empty());
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

/// A dependency that is ITSELF a reflow2 design can say so, and one that is not
/// stays silent — the distinction the whole field exists to carry.
///
/// The two facts sat side by side in this file and never touched: "my build pins
/// v0.12.0 of that" and "that is also a design I can compose with". Recording
/// the second makes a composition target derivable from a committed,
/// version-pinned manifest rather than from a per-machine config, and it keeps
/// the DIRECTION the dependency edge already carries — which a flat list of
/// graph ids cannot express.
#[test]
fn a_dependency_that_is_also_a_design_says_so_and_one_that_is_not_stays_silent() {
    let mut g = graph();

    let mut linked = decl();
    linked.graph_id = Some("dynograph-foundation".into());
    g.declare_dependency(&linked).unwrap();

    let mut plain = decl();
    plain.id = "dep:serde".into();
    plain.name = "serde".into();
    plain.source = "https://crates.io".into();
    plain.graph_id = None;
    g.declare_dependency(&plain).unwrap();

    let back = g.declared_dependencies().unwrap();
    let linked_back = back
        .iter()
        .find(|d| d.name == "dynograph-foundation")
        .unwrap();
    let plain_back = back.iter().find(|d| d.name == "serde").unwrap();
    assert_eq!(
        linked_back.graph_id.as_deref(),
        Some("dynograph-foundation")
    );
    assert_eq!(
        plain_back.graph_id, None,
        "absence must round-trip as absence: 'nobody has said' is not 'there is no design'"
    );

    let toml = g.dependency_manifest().unwrap();
    assert!(
        toml.contains("graph_id = \"dynograph-foundation\""),
        "the link must travel in the committed file:\n{toml}"
    );
    // The serde entry must carry no graph_id line at all. An always-present
    // empty field would make "has no design" and "nobody recorded one" look the
    // same, which is the distinction this field exists to preserve.
    let serde_block = toml
        .split("[dependencies.serde]")
        .nth(1)
        .expect("serde entry present");
    assert!(
        !serde_block.contains("graph_id"),
        "an undeclared graph_id must be ABSENT, not empty:\n{serde_block}"
    );
}

/// An empty string is not a graph_id. Storing one would assert that the
/// dependency has a design whose id happens to be blank.
#[test]
fn a_blank_graph_id_is_treated_as_nobody_having_said() {
    let mut g = graph();
    let mut blank = decl();
    blank.graph_id = Some("   ".into());
    g.declare_dependency(&blank).unwrap();

    let back = g.declared_dependencies().unwrap();
    assert_eq!(back[0].graph_id, None);

    // Scoped to the DEPENDENCY entry: the `[reflow2]` header always carries this
    // design's OWN graph_id, so a whole-file search for "graph_id =" would pass
    // for the wrong reason. (It did, on the first version of this assertion.)
    let toml = g.dependency_manifest().unwrap();
    let entry = toml
        .split("[dependencies.dynograph-foundation]")
        .nth(1)
        .expect("the dependency entry is present");
    assert!(
        !entry.contains("graph_id"),
        "a blank graph_id must be stored as nothing at all:\n{entry}"
    );
}
