//! What an Artifact node says about ITSELF — how it behaves over time
//! (`volatility`, BL-191) and whether it stands for one thing or many
//! (`granularity`, BL-188).
//!
//! Both properties exist because the graph could not tell two OPPOSITE states
//! apart, and in each case the tool reported the wrong one confidently.
//!
//! `volatility` — five coordination buses were modelled as Artifacts with
//! checksums, exactly per the **adopt** skill's *"a checksum is what makes later
//! drift detectable"*. Within minutes `reconcile_artifacts` reported five
//! `checksum_change` divergences: all correct, all meaningless, because those
//! files are appended to continuously by design. Disposing of them is a ritual
//! owed again on every reconcile forever, and the noise buries a real drift when
//! one appears — the signal-trained-to-be-ignored failure, arrived at by
//! following the skill correctly.
//!
//! `granularity` — a registration check reported *"every live doc is
//! registered"*, truthfully, because `docs/` was one Artifact and the adopt
//! skill's rule is that a directory artifact claims its subtree. 359 files were
//! individually unreferenceable behind a green tick, and nothing distinguished
//! *deliberately opaque* from *nobody has got to it yet*.
//!
//! THE COUNTERWEIGHTS ARE THE POINT, as in BL-176: declaring an artifact
//! volatile must not buy silence. Absence still fires at full severity, and a
//! `stable` artifact is unaffected — otherwise this trades a false positive for
//! a false negative, which is strictly the worse bug.

use reflow2_core::nodes::{Props, node};
use reflow2_core::{DesignGraph, DriftKind, ObservedArtifact, ObservedPath, ReconcileOptions};

fn observed(id: &str, present: bool, checksum: Option<&str>) -> ObservedArtifact {
    ObservedArtifact {
        artifact_id: id.into(),
        present,
        checksum: checksum.map(str::to_string),
        realizes: None,
    }
}

/// One registered artifact carrying a baseline, with the volatility under test.
fn graph_with(volatility: Option<&str>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:c", "C", "does c", None).unwrap();
    g.add_artifact("art:bus", "B.md", Some("document"), Some("B.md"))
        .unwrap();
    g.realizes("art:bus", node::CAPABILITY, "cap:c", None)
        .unwrap();
    let mut props = Props::new().set("checksum", "sha256:aaaa");
    if let Some(v) = volatility {
        props = props.set("volatility", v);
    }
    g.upsert_node(node::ARTIFACT, "art:bus", props).unwrap();
    g
}

fn kinds(g: &mut DesignGraph, obs: &[ObservedArtifact]) -> Vec<DriftKind> {
    g.reconcile_artifacts(obs, &ReconcileOptions::default())
        .unwrap()
        .findings
        .into_iter()
        .map(|f| f.kind)
        .collect()
}

// ---- BL-191 · volatility --------------------------------------------------

/// THE COUNTERWEIGHT, and it is first on purpose: the default must not move.
/// A source file that changed is drift, exactly as before.
#[test]
fn a_stable_artifact_still_reports_checksum_change() {
    // No volatility set. Until 2026-08-15 the SCHEMA injected `stable` here at
    // write time and this case proved the injected value behaved; since
    // music_graph F23 removed that default, nothing is stored and `drift.rs`
    // supplies `stable` via unwrap_or. The case is unchanged and still passes,
    // which is the whole argument for the removal: the safe reading never came
    // from the store.
    let mut g = graph_with(None);
    assert_eq!(
        g.get_node(node::ARTIFACT, "art:bus")
            .unwrap()
            .unwrap()
            .properties
            .get("volatility"),
        None,
        "absence must be ABSENT — if the store still holds a value nobody chose, \
         the round trip that F23 reported is still there and the case below is \
         proving the wrong thing"
    );
    assert_eq!(
        kinds(&mut g, &[observed("art:bus", true, Some("sha256:bbbb"))]),
        vec![DriftKind::ChecksumChange],
        "the default reading is unchanged — any content change is drift"
    );
}

#[test]
fn an_append_only_artifact_reports_expected_change_not_drift() {
    let mut g = graph_with(Some("append_only"));
    assert_eq!(
        kinds(&mut g, &[observed("art:bus", true, Some("sha256:bbbb"))]),
        vec![DriftKind::ExpectedChange],
        "a bus that grew is behaving as declared — surfaced, but not drift"
    );
}

#[test]
fn a_living_artifact_reports_expected_change_not_drift() {
    let mut g = graph_with(Some("living"));
    assert_eq!(
        kinds(&mut g, &[observed("art:bus", true, Some("sha256:bbbb"))]),
        vec![DriftKind::ExpectedChange]
    );
}

/// The ritual this exists to end: an expected change must not land in the drift
/// ledger, or the disposition is owed again on every reconcile forever.
#[test]
fn an_expected_change_is_never_written_to_the_drift_ledger() {
    let mut g = graph_with(Some("append_only"));
    let before = g.count_nodes(node::DRIFT_EVENT).unwrap();
    let report = g
        .reconcile_artifacts(
            &[observed("art:bus", true, Some("sha256:bbbb"))],
            &ReconcileOptions {
                record_events: true,
                detected_at: Some("2026-08-04".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(report.findings.len(), 1, "still reported to the caller");
    assert_eq!(
        g.count_nodes(node::DRIFT_EVENT).unwrap(),
        before,
        "but nothing is recorded — an expected change is not a divergence"
    );
    assert!(
        report.findings[0].event_id.is_none(),
        "and the finding says so rather than pointing at an event"
    );
}

/// THE COUNTERWEIGHT THAT KEEPS THIS HONEST. Declaring an artifact volatile says
/// its CONTENT moves. It never says the file may vanish — a missing bus is a
/// real finding at full severity, and a fix that silenced it would have traded a
/// false positive for a false negative.
#[test]
fn a_missing_append_only_artifact_still_fires() {
    let mut g = graph_with(Some("append_only"));
    assert_eq!(
        kinds(&mut g, &[observed("art:bus", false, None)]),
        vec![DriftKind::MissingArtifact],
        "absence is not an expected change, whatever the volatility"
    );
}

/// An append-only file that did NOT change must read as unchanged, not as an
/// expected change — otherwise the report says something happened every time.
#[test]
fn an_unchanged_append_only_artifact_is_still_unchanged() {
    let mut g = graph_with(Some("append_only"));
    let report = g
        .reconcile_artifacts(
            &[observed("art:bus", true, Some("sha256:aaaa"))],
            &ReconcileOptions::default(),
        )
        .unwrap();
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(report.unchanged, 1);
}

// ---- BL-188 · granularity -------------------------------------------------

fn graph_with_granularity(granularity: Option<&str>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_artifact("art:docs", "docs", Some("document"), Some("docs"))
        .unwrap();
    if let Some(v) = granularity {
        g.upsert_node(
            node::ARTIFACT,
            "art:docs",
            Props::new().set("granularity", v),
        )
        .unwrap();
    }
    g
}

fn swept() -> Vec<ObservedPath> {
    ["docs/a.md", "docs/b.md", "docs/c.md"]
        .iter()
        .map(|p| ObservedPath {
            path: (*p).into(),
            mass: 1,
        })
        .collect()
}

/// The reading that could not be produced from the graph at all: a directory
/// artifact claims its whole subtree, so coverage is green — and now the report
/// says on whose authority.
#[test]
fn coverage_names_the_placeholders_behind_a_green_tick() {
    let g = graph_with_granularity(Some("pending_expansion"));
    let r = g.coverage_report(&swept(), &[], None).unwrap();

    assert_eq!(r.claimed, 3, "the directory still claims its subtree");
    assert_eq!(r.unclaimed, 0, "so coverage still reads green");
    assert_eq!(
        r.pending_expansion,
        vec!["art:docs".to_string()],
        "but the green is qualified — this node stands in for files nobody has \
         registered individually"
    );
    assert!(r.opaque_claims.is_empty());
}

/// The two states are OPPOSITE and used to read identically. A settled archive
/// claimed on purpose must not be reported as unfinished work.
#[test]
fn a_deliberately_opaque_claim_is_reported_apart_from_an_unfinished_one() {
    let g = graph_with_granularity(Some("opaque"));
    let r = g.coverage_report(&swept(), &[], None).unwrap();

    assert_eq!(r.opaque_claims, vec!["art:docs".to_string()]);
    assert!(
        r.pending_expansion.is_empty(),
        "deliberately opaque is a decision, not a backlog item"
    );
}

/// THE COUNTERWEIGHT: the ordinary case stays quiet. Every artifact in a normal
/// design is atomic, and a report that named them all would be noise.
#[test]
fn an_ordinary_artifact_is_not_reported_as_a_placeholder() {
    // Same shape as the volatility case above: nothing is stored since F23, and
    // `coverage.rs` supplies `atomic` via unwrap_or.
    let g = graph_with_granularity(None);
    assert_eq!(
        g.get_node(node::ARTIFACT, "art:docs")
            .unwrap()
            .unwrap()
            .properties
            .get("granularity"),
        None,
        "absence must be ABSENT — a stored `atomic` nobody chose is what made \
         35 of music_graph's 35 Artifacts change across a round trip"
    );
    let r = g.coverage_report(&swept(), &[], None).unwrap();

    assert!(r.pending_expansion.is_empty());
    assert!(r.opaque_claims.is_empty());
}
