//! Granularity — does the build separate what the design separates? (BL-182)
//!
//! **The cases that matter most here are the silences.** This reading exists
//! because Anthony spotted by eye that one file held 139 tools while every
//! structural instrument reflow2 owns reported the design healthy — and his
//! immediate objection to fixing it was the right one: *"avoid monoliths" is a
//! subjective design principle, so what exactly would detect it?*
//!
//! The answer this module gives is narrow on purpose. It does not detect
//! monoliths. It detects that **two records the design already holds disagree**
//! about how many things there are — N Capabilities in the design, one Artifact
//! in the build — and it refuses to say which side is wrong.
//!
//! So the tests below spend most of their effort proving it stays quiet when it
//! should:
//!
//! - a young design where everything lives in one file has no outlier and is
//!   told nothing, because there is nothing to be out of line *with*;
//! - a design too small to have a distribution says so rather than inventing
//!   one;
//! - "the design separates two things and the build separates neither" is true
//!   and not worth saying, so it is not said.
//!
//! That last one is not fastidiousness. Counts pile up at one, the standard
//! deviation collapses, and a purely distributional cutoff fires on noise.

use reflow2_core::granularity::{MIN_DISTINCTIONS, MIN_POPULATION, UNUSUAL_AT};
use reflow2_core::{DesignGraph, GranularityReport};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

/// Register `n` artifacts that each realize one capability — the ordinary
/// shape, and the baseline every outlier is measured against.
fn seed_ordinary(g: &mut DesignGraph, n: usize) {
    g.add_component("cmp:main", "main", "The system.", None)
        .expect("component");
    for i in 0..n {
        let cap = format!("cap:ordinary{i}");
        let art = format!("art:ordinary{i}");
        g.add_capability(&cap, &format!("Ordinary {i}"), "Does one thing.", None)
            .expect("capability");
        g.allocate(&cap, "cmp:main").expect("allocate");
        g.add_artifact(&art, &format!("ordinary{i}.rs"), Some("code"), None)
            .expect("artifact");
        g.realizes(&art, "Capability", &cap, None, None)
            .expect("realizes");
    }
}

/// One artifact that swallows `n` capabilities the design distinguishes.
fn seed_coarse(g: &mut DesignGraph, id: &str, n: usize) {
    g.add_artifact(id, "coarse.rs", Some("code"), None)
        .expect("artifact");
    for i in 0..n {
        let cap = format!("cap:{id}-{i}");
        g.add_capability(&cap, &format!("Swallowed {i}"), "Does a thing.", None)
            .expect("capability");
        g.allocate(&cap, "cmp:main").expect("allocate");
        g.realizes(id, "Capability", &cap, None, None)
            .expect("realizes");
    }
}

fn report(g: &DesignGraph) -> GranularityReport {
    g.granularity_report().expect("granularity report")
}

// ---------------------------------------------------------------------------
// The silences — what must NOT be reported.
// ---------------------------------------------------------------------------

/// **The load-bearing guarantee.** A design in its breadboard phase, where
/// every capability lives in the one file that exists, must be told nothing.
///
/// This is `dec:maturity-restructuring-delta`'s trap stated as a test: a tool
/// that reported undeclared structure as a defect would punish exactly the
/// early-phase work that is going correctly. There is no outlier here because
/// there is nothing to be out of line with.
#[test]
fn a_uniformly_coarse_design_is_not_punished() {
    let mut g = graph();
    g.add_component("cmp:main", "main", "The system.", None)
        .expect("component");
    // Eight artifacts, each swallowing four capabilities. Coarse everywhere —
    // and therefore out of line with nothing.
    for a in 0..8 {
        seed_coarse(&mut g, &format!("art:lump{a}"), 4);
    }

    let r = report(&g);

    assert!(
        r.observations.is_empty(),
        "a uniformly coarse design must produce no finding, got {:?}",
        r.observations
            .iter()
            .map(|o| &o.artifact_id)
            .collect::<Vec<_>>()
    );
    assert!(
        r.notes.iter().any(|n| n.contains("uniformly coarse")
            || n.contains("same number")
            || n.contains("ordinary answer")),
        "the silence must be explained, not blank: {:?}",
        r.notes
    );
    // And a quiet report still says what it could not see.
    assert!(!r.not_observed_about.is_empty());
}

/// Too few artifacts to have a distribution: say so rather than computing a
/// spread over three points and calling it a fact about the design.
#[test]
fn too_small_a_population_says_so_instead_of_inventing_a_spread() {
    let mut g = graph();
    seed_ordinary(&mut g, MIN_POPULATION - 3);
    seed_coarse(&mut g, "art:big", 9);

    let r = report(&g);

    assert!(r.population < MIN_POPULATION);
    assert!(r.observations.is_empty());
    assert!(
        r.notes.iter().any(|n| n.contains("below the")),
        "notes: {:?}",
        r.notes
    );
}

/// Counts pile up at one, so the standard deviation collapses and a purely
/// distributional cutoff fires on an artifact holding *two* capabilities. True,
/// trivial, and not worth a person's attention — so `MIN_DISTINCTIONS` stops
/// it, and this test is what keeps that floor honest if someone removes it.
#[test]
fn two_capabilities_in_one_file_is_not_worth_saying() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:slightly-coarse", 2);

    let r = report(&g);

    // The distributional cutoff alone WOULD have fired here.
    let z = (2.0 - r.mean_capabilities_per_artifact) / {
        // recompute the sd the report used, from its own reported mean
        let counts: Vec<f64> = std::iter::repeat_n(1.0, 12)
            .chain(std::iter::once(2.0))
            .collect();
        let m = r.mean_capabilities_per_artifact;
        (counts.iter().map(|c| (c - m) * (c - m)).sum::<f64>() / (counts.len() as f64 - 1.0)).sqrt()
    };
    assert!(
        z >= UNUSUAL_AT,
        "the premise of this test is that z ({z:.2}) clears the distributional bar"
    );
    assert!(
        r.observations.is_empty(),
        "MIN_DISTINCTIONS ({MIN_DISTINCTIONS}) must suppress it anyway, got {:?}",
        r.observations
    );
}

/// An artifact nobody registered cannot be reported, and the report says that
/// rather than implying full coverage.
#[test]
fn unregistered_artifacts_are_out_of_scope_and_the_report_admits_it() {
    let mut g = graph();
    seed_ordinary(&mut g, 8);

    let r = report(&g);

    assert_eq!(r.population, 8);
    assert!(
        r.not_observed_about
            .iter()
            .any(|s| s.contains("nobody registered")),
        "{:?}",
        r.not_observed_about
    );
}

// ---------------------------------------------------------------------------
// The finding itself.
// ---------------------------------------------------------------------------

/// The reflow2 case, reproduced in miniature: a design that has decomposed
/// nearly everywhere, and one artifact that did not follow.
#[test]
fn one_artifact_out_of_line_with_its_own_design_is_reported() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:swallower", 10);

    let r = report(&g);

    assert_eq!(r.observations.len(), 1, "{:?}", r.observations);
    let o = &r.observations[0];
    assert_eq!(o.artifact_id, "art:swallower");
    assert_eq!(o.realizes_capabilities, 10);
    assert_eq!(o.capability_ids.len(), 10);
    assert_eq!(o.at_or_above, 1, "it stands alone in this design");
    assert!(o.unusual >= UNUSUAL_AT);
    // Explained, in the house style — a reader can disagree without re-deriving.
    assert!(
        o.reasons.iter().any(|s| s.contains("distinguishes 10")),
        "{:?}",
        o.reasons
    );
    assert!(
        o.reasons
            .iter()
            .any(|s| s.contains("did not follow the rest")),
        "{:?}",
        o.reasons
    );
    // The cutoffs travel with the answer so they can be argued with.
    assert_eq!(r.unusual_at, UNUSUAL_AT);
    assert_eq!(r.min_distinctions, MIN_DISTINCTIONS);
}

/// **The refusal, asserted.** The observation states what is, and none of what
/// to do about it — no severity, no category, no suggested fix, and none of the
/// words that would turn a fact into an accusation. `dec:report-dont-judge`.
#[test]
fn the_observation_carries_no_verdict() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    seed_coarse(&mut g, "art:swallower", 10);

    let json = serde_json::to_string(&report(&g)).expect("serialize");

    for forbidden in [
        "severity",
        "suggested_fix",
        "monolith",
        "too big",
        "should be split",
        "violation",
        "defect",
    ] {
        assert!(
            !json.contains(forbidden),
            "the report must state a fact and refuse a verdict, but it contains {forbidden:?}"
        );
    }
    // And it must still say which side it declines to rule on.
    assert!(json.contains("That judgement is not reflow2's"));
}

/// An artifact realizing its own Component is the ordinary way to say "this
/// file is that part". Counting it would make every properly-registered
/// artifact look coarser than it is — the design discipline penalising itself,
/// which is the trap `surprises` already dodges for contracts.
#[test]
fn realizing_a_component_does_not_count_as_a_distinction() {
    let mut g = graph();
    seed_ordinary(&mut g, 12);
    g.add_artifact("art:tidy", "tidy.rs", Some("code"), None)
        .expect("artifact");
    g.add_capability("cap:tidy", "Tidy", "One thing.", None)
        .expect("capability");
    g.allocate("cap:tidy", "cmp:main").expect("allocate");
    g.realizes("art:tidy", "Capability", "cap:tidy", None, None)
        .expect("realizes cap");
    g.realizes("art:tidy", "Component", "cmp:main", None, None)
        .expect("realizes component");

    let r = report(&g);

    assert!(
        r.observations.is_empty(),
        "a component realization must not inflate the count: {:?}",
        r.observations
    );
    assert_eq!(r.population, 13, "art:tidy counts once, for its capability");
}

/// Same design, byte-identical report — a reading that reorders between runs
/// cannot be diffed, and this one is meant to be watched over time.
#[test]
fn the_report_is_deterministic() {
    let mut g = graph();
    seed_ordinary(&mut g, 20);
    seed_coarse(&mut g, "art:swallower", 12);
    seed_coarse(&mut g, "art:other", 9);

    let a = serde_json::to_string(&report(&g)).expect("serialize");
    let b = serde_json::to_string(&report(&g)).expect("serialize");
    assert_eq!(a, b);

    // Most out-of-line first.
    let r = report(&g);
    assert_eq!(r.observations.len(), 2, "{:?}", r.observations);
    assert_eq!(r.observations[0].artifact_id, "art:swallower");
    assert!(r.observations[0].unusual >= r.observations[1].unusual);
}

/// **Masking, asserted rather than discovered later.** Outliers inflate the
/// standard deviation they are measured against, so several coarse artifacts
/// hide each other — the classic weakness of any z-based reading, and the
/// reason this is a prompt for a person rather than a gate. Seeded here so the
/// behaviour is a recorded property instead of a surprise in the field.
#[test]
fn several_coarse_artifacts_mask_each_other() {
    let mut alone = graph();
    seed_ordinary(&mut alone, 12);
    seed_coarse(&mut alone, "art:swallower", 8);
    let solo = report(&alone);
    assert_eq!(solo.observations.len(), 1, "one outlier is visible alone");

    let mut crowded = graph();
    seed_ordinary(&mut crowded, 12);
    for i in 0..5 {
        seed_coarse(&mut crowded, &format!("art:swallower{i}"), 8);
    }
    let many = report(&crowded);

    assert!(
        many.observations.len() < 5,
        "five equally coarse artifacts should mask one another, not all be reported: {:?}",
        many.observations
            .iter()
            .map(|o| &o.artifact_id)
            .collect::<Vec<_>>()
    );
}

// ---- The module standing on its own, with no store behind it -------------
//
// Everything above builds a real DesignGraph. Everything below does not, and
// that difference is the point of `ifc:graph-read`.
//
// Before this contract existed, reflow2-core had 274 public functions on one
// struct across 43 files and exactly one trait. No module could be swapped,
// held still and measured, or tested without a store — not because anyone had
// written the wrong code, but because there was no boundary to stand outside
// of. `granularity` is the pilot: it now takes `&dyn GraphRead`, so what
// follows is the same report, computed over a design made of two vectors.

use dynograph_core::{DynoError, Value};
use dynograph_storage::{StoredEdge, StoredNode};
use reflow2_core::graph_read::GraphRead;
use reflow2_core::nodes::node;
use std::collections::HashMap;

/// A design held in two vectors. No store, no schema, no disk, no lock.
struct Vectors {
    nodes: Vec<StoredNode>,
    edges: Vec<StoredEdge>,
    /// Types this fake admits. Everything else is an ERROR, never an empty
    /// answer — the one obligation in the contract that the signatures cannot
    /// express, honoured here so the test proves the caller survives it.
    known_types: Vec<&'static str>,
}

impl Vectors {
    fn check_type(&self, node_type: &str) -> Result<(), DynoError> {
        if self.known_types.contains(&node_type) {
            Ok(())
        } else {
            Err(DynoError::UnknownNodeType(node_type.to_string()))
        }
    }
}

impl GraphRead for Vectors {
    fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError> {
        self.check_type(node_type)?;
        Ok(self
            .nodes
            .iter()
            .find(|n| n.node_type == node_type && n.node_id == id)
            .cloned())
    }

    fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError> {
        self.check_type(node_type)?;
        Ok(self
            .nodes
            .iter()
            .filter(|n| n.node_type == node_type)
            .cloned()
            .collect())
    }

    fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError> {
        Ok(self.scan_nodes(node_type)?.len())
    }

    fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        Ok(self
            .edges
            .iter()
            .filter(|e| e.from_id == from_id && edge_type.is_none_or(|t| e.edge_type == t))
            .cloned()
            .collect())
    }

    fn incoming(&self, to_id: &str, edge_type: Option<&str>) -> Result<Vec<StoredEdge>, DynoError> {
        Ok(self
            .edges
            .iter()
            .filter(|e| e.to_id == to_id && edge_type.is_none_or(|t| e.edge_type == t))
            .cloned()
            .collect())
    }
}

fn fake_node(node_type: &str, id: &str) -> StoredNode {
    StoredNode {
        graph_id: "fake".into(),
        node_type: node_type.into(),
        node_id: id.into(),
        properties: HashMap::from([("name".to_string(), Value::String(id.to_string()))]),
    }
}

fn realizes(art: &str, cap: &str) -> StoredEdge {
    StoredEdge {
        graph_id: "fake".into(),
        edge_type: "REALIZES".into(),
        from_id: art.into(),
        to_id: cap.into(),
        properties: HashMap::new(),
    }
}

/// One fat artifact against nine lean ones — the shape the reading exists to
/// notice, built here without a store.
fn one_outlier_among_many() -> Vectors {
    let mut nodes = vec![fake_node("Artifact", "art:fat")];
    let mut edges = Vec::new();
    for i in 0..8 {
        nodes.push(fake_node("Capability", &format!("cap:fat{i}")));
        edges.push(realizes("art:fat", &format!("cap:fat{i}")));
    }
    for i in 0..9 {
        let (a, c) = (format!("art:lean{i}"), format!("cap:lean{i}"));
        nodes.push(fake_node("Artifact", &a));
        nodes.push(fake_node("Capability", &c));
        edges.push(realizes(&a, &c));
    }
    Vectors {
        nodes,
        edges,
        known_types: vec!["Artifact", "Capability"],
    }
}

#[test]
fn the_reading_runs_against_a_design_that_is_only_two_vectors() {
    let report = reflow2_core::granularity::granularity_report(&one_outlier_among_many())
        .expect("no store required");

    assert_eq!(report.population, 10, "ten artifacts realize capabilities");
    let flagged: Vec<&str> = report
        .observations
        .iter()
        .map(|o| o.artifact_id.as_str())
        .collect();
    assert_eq!(
        flagged,
        vec!["art:fat"],
        "the one artifact holding eight distinctions is the finding; the nine lean ones are not"
    );
}

/// ⭐ SUBSTITUTABILITY, DEMONSTRATED RATHER THAN ASSERTED.
///
/// The same function, the same assertions, over an implementation that shares
/// no code with `DesignGraph` — no RocksDB, no schema, no `.reflow2/` on disk.
/// If `granularity` had reached past the contract for anything at all, this
/// could not compile, let alone agree.
#[test]
fn the_same_module_cannot_tell_which_implementation_it_is_reading() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_artifact("art:fat", "fat.rs", Some("code"), Some("src/fat.rs"))
        .unwrap();
    for i in 0..8 {
        g.add_capability(&format!("cap:fat{i}"), "c", "c", None)
            .unwrap();
        g.realizes(
            "art:fat",
            node::CAPABILITY,
            &format!("cap:fat{i}"),
            None,
            None,
        )
        .unwrap();
    }
    for i in 0..9 {
        let (a, c) = (format!("art:lean{i}"), format!("cap:lean{i}"));
        g.add_artifact(&a, "lean.rs", Some("code"), Some("src/lean.rs"))
            .unwrap();
        g.add_capability(&c, "c", "c", None).unwrap();
        g.realizes(&a, node::CAPABILITY, &c, None, None).unwrap();
    }

    let from_store = reflow2_core::granularity::granularity_report(&g).unwrap();
    let from_vectors =
        reflow2_core::granularity::granularity_report(&one_outlier_among_many()).unwrap();

    assert_eq!(from_store.population, from_vectors.population);
    assert_eq!(
        from_store.mean_capabilities_per_artifact,
        from_vectors.mean_capabilities_per_artifact
    );
    let ids = |r: &reflow2_core::granularity::GranularityReport| {
        r.observations
            .iter()
            .map(|o| o.artifact_id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&from_store), ids(&from_vectors));
}

/// The obligation the signatures cannot carry: an UNKNOWN node type is an
/// error, not an empty answer. A conforming implementation must fail loud, and
/// the caller must let that failure through rather than reading it as "nothing
/// there" — which is how a typo would otherwise answer reassuringly forever.
#[test]
fn an_unknown_node_type_reaches_the_caller_as_an_error() {
    // The control, and without it this test would be vacuous: an EMPTY design
    // whose types are known is a fine answer, not a failure.
    let empty_but_known = Vectors {
        nodes: vec![],
        edges: vec![],
        known_types: vec!["Artifact", "Capability"],
    };
    let ok = reflow2_core::granularity::granularity_report(&empty_but_known)
        .expect("no artifacts is a design state, not an error");
    assert_eq!(ok.population, 0);

    // The same emptiness, but the store cannot answer at all. One thing
    // changed; the outcome must flip.
    let cannot_answer = Vectors {
        nodes: vec![],
        edges: vec![],
        known_types: vec![], // admits nothing — even "Artifact" is unknown here
    };
    assert!(
        reflow2_core::granularity::granularity_report(&cannot_answer).is_err(),
        "the store could not answer, and that must not arrive as an empty design"
    );
}
