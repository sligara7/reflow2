//! INGEST — freeform input → graph, via the mock LLM backend.
//!
//! Each extraction pass tags its prompt with a `[pass:NAME]` marker, so the
//! scriptable mock returns per-pass canned JSON by matching that marker.

use reflow2_core::detect::GapSource;
use reflow2_core::nodes::{edge, node};
use reflow2_core::propagate::PropagateOptions;
use reflow2_core::{
    ChangeType, DesignGraph, IngestOptions, IngestStatus, MockLlmBackend, parse_snapshot_state,
};

const BRIEF: &str = "Build a widget that serves reads fast and works offline.";

/// A mock scripted for a full, clean extraction.
fn full_mock() -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:w","name":"Widget","objective":"ship it","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms","priority":"high"}]}"#,
        )
        .on_contains(
            "[pass:constraints]",
            r#"{"constraints":[{"id":"con:off","name":"Offline","statement":"no network","category":"operational"}]}"#,
        )
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads on-device"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains(
            "[pass:components]",
            r#"{"components":[{"id":"cmp:store","name":"Store","purpose":"kv store","allocated_capability_ids":["cap:cache"]}]}"#,
        )
        .on_contains(
            "[pass:satisfies]",
            r#"{"satisfies":[{"capability_id":"cap:cache","requirement_id":"req:lat"}]}"#,
        )
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

#[test]
fn full_ingest_builds_a_golden_thread_from_text() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(BRIEF, &IngestOptions::default(), &full_mock())
        .unwrap();

    assert_eq!(report.status, IngestStatus::Ok, "clean run: {report:?}");
    assert!(report.pass_errors.is_empty());
    assert!(report.dropped_edges.is_empty());

    // Nodes: Fragment + Project + Requirement + Constraint + Capability + Component.
    assert_eq!(g.count_nodes(node::PROJECT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::REQUIREMENT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::CONSTRAINT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 1);
    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 1);
    assert_eq!(g.count_nodes(node::FRAGMENT).unwrap(), 1);
    assert_eq!(report.nodes_created, 6);

    // Edges: the golden thread the passes wired.
    let sat = g.outgoing("cap:cache", Some(edge::SATISFIES)).unwrap();
    assert_eq!(sat.len(), 1);
    assert_eq!(sat[0].to_id, "req:lat");
    let alloc = g.outgoing("cap:cache", Some(edge::ALLOCATED_TO)).unwrap();
    assert_eq!(alloc.len(), 1);
    assert_eq!(alloc[0].to_id, "cmp:store");

    // Provenance: the Fragment YIELDED every created entity (5 non-fragment nodes).
    let yielded = g
        .outgoing(&report.fragment_id, Some(edge::YIELDED))
        .unwrap();
    assert_eq!(yielded.len(), 5);
    assert_eq!(yielded[0].properties["action"].as_str(), Some("created"));

    // Extracted properties survived integration.
    let req = g.get_node(node::REQUIREMENT, "req:lat").unwrap().unwrap();
    assert_eq!(req.properties["priority"].as_str(), Some("high"));
    let proj = g.get_node(node::PROJECT, "proj:w").unwrap().unwrap();
    assert_eq!(proj.properties["mode"].as_str(), Some("flexible"));
}

#[test]
fn dependencies_pass_captures_weighted_coupling_edges() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Two capabilities with a weighted dependency between them.
    let mock = MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:a","name":"A","description":"da"},{"id":"cap:b","name":"B","description":"db"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains(
            "[pass:dependencies]",
            r#"{"dependencies":[{"from_capability_id":"cap:a","to_capability_id":"cap:b","dependency_type":"data_flow","weight":0.8}]}"#,
        );

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();
    assert_eq!(report.status, IngestStatus::Ok, "clean run: {report:?}");

    let deps = g.outgoing("cap:a", Some(edge::DEPENDS_ON)).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].to_id, "cap:b");
    // The weight facet the graph-analysis work needs is captured on the edge.
    assert_eq!(deps[0].properties["weight"].as_f64(), Some(0.8));
    assert_eq!(
        deps[0].properties["weight_basis"].as_str(),
        Some("estimated")
    );
    assert_eq!(
        deps[0].properties["dependency_type"].as_str(),
        Some("data_flow")
    );
}

#[test]
fn discovery_gate_suppresses_phase_two_when_absent() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // Components declared absent: the components pass must not run even though a
    // rule for it exists.
    let mock = MockLlmBackend::new()
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[{"id":"req:a","name":"A","statement":"s"}]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
        .on_contains("[pass:components]", r#"{"components":[{"id":"cmp:x","name":"X","purpose":"p"}]}"#);

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 0);
    assert!(
        !mock
            .calls()
            .iter()
            .any(|c| c.prompt.contains("[pass:components]")),
        "the components pass must not run when discovery says absent"
    );
    assert_eq!(report.status, IngestStatus::Ok);
}

#[test]
fn a_failed_pass_is_enveloped_and_siblings_survive() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // No `requirements` rule and no default → that pass runs dry and errors,
    // but every other pass still lands.
    let mock = MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        );

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert!(report.pass_errors.iter().any(|e| e.pass == "requirements"));
    assert_eq!(
        g.count_nodes(node::REQUIREMENT).unwrap(),
        0,
        "failed pass yields nothing"
    );
    assert_eq!(
        g.count_nodes(node::CAPABILITY).unwrap(),
        1,
        "sibling pass survived"
    );
}

#[test]
fn phantom_edge_is_dropped_not_written() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // satisfies references a requirement id that was never created.
    let mock = MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"W"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[{"id":"req:real","name":"R","statement":"s"}]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains("[pass:capabilities]", r#"{"capabilities":[{"id":"cap:a","name":"C","description":"d"}]}"#)
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[{"capability_id":"cap:a","requirement_id":"req:ghost"}]}"#);

    let report = g.ingest(BRIEF, &IngestOptions::default(), &mock).unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert_eq!(report.dropped_edges.len(), 1);
    let dropped = &report.dropped_edges[0];
    assert_eq!(dropped.edge_type, "SATISFIES");
    assert_eq!(dropped.to_id, "req:ghost");
    assert!(dropped.reason.contains("req:ghost"));
    // No phantom edge was written.
    assert!(
        g.outgoing("cap:a", Some(edge::SATISFIES))
            .unwrap()
            .is_empty()
    );
}

/// Phase-1-only mock (no components/satisfies edges) with a given requirement
/// statement, so re-ingest tests can vary just the content that evolves.
fn mock_v(req_statement: &str) -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:w","name":"Widget","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            format!(
                r#"{{"requirements":[{{"id":"req:lat","name":"Latency","statement":"{req_statement}","priority":"high"}}]}}"#
            ),
        )
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

#[test]
fn reingest_with_changed_content_evolves_and_snapshots() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // v1: latency under 200ms.
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_v("under 200ms"),
    )
    .unwrap();

    // v2: same req:lat id, tightened statement → matched-evolved.
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                epoch_id: Some("epoch:v2".into()),
                change_type: ChangeType::RequirementCreep,
                ..Default::default()
            },
            &mock_v("under 100ms"),
        )
        .unwrap();

    // Exactly the requirement evolved; project + capability are unchanged.
    assert_eq!(report.nodes_evolved, 1);
    assert_eq!(report.nodes_unchanged, 2);
    assert_eq!(report.epoch_used.as_deref(), Some("epoch:v2"));

    // The live node holds the new statement...
    let live = g.get_node(node::REQUIREMENT, "req:lat").unwrap().unwrap();
    assert_eq!(live.properties["statement"].as_str(), Some("under 100ms"));

    // ...and the past is remembered in a snapshot pinned to the epoch.
    let snap = g
        .get_node(node::SNAPSHOT, "snap:epoch:v2:req:lat")
        .unwrap()
        .expect("a snapshot of the prior state");
    let old = parse_snapshot_state(&snap).unwrap();
    assert_eq!(old["statement"].as_str(), Some("under 200ms"));

    // A ChangeEvent of the declared type records why, wired to what it CHANGED.
    let ce = g
        .get_node(node::CHANGE_EVENT, "chg:frag:v2:req:lat")
        .unwrap()
        .expect("a change event");
    assert_eq!(
        ce.properties["change_type"].as_str(),
        Some("requirement_creep")
    );
    let changed = g
        .outgoing("chg:frag:v2:req:lat", Some(edge::CHANGED))
        .unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].to_id, "req:lat");
}

/// A phase-1 mock emitting one capability (given id + name) and, optionally, a
/// SATISFIES edge from that capability id to `req:lat`.
fn mock_cap(cap_id: &str, cap_name: &str, satisfy: bool) -> MockLlmBackend {
    let sat = if satisfy {
        format!(r#"{{"satisfies":[{{"capability_id":"{cap_id}","requirement_id":"req:lat"}}]}}"#)
    } else {
        r#"{"satisfies":[]}"#.to_string()
    };
    MockLlmBackend::new()
        .on_contains("[pass:project_intent]", r#"{"project":{"id":"proj:w","name":"Widget","mode":"flexible"}}"#)
        .on_contains("[pass:requirements]", r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms"}]}"#)
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            format!(r#"{{"capabilities":[{{"id":"{cap_id}","name":"{cap_name}","description":"serve reads"}}]}}"#),
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains("[pass:satisfies]", sat)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

#[test]
fn a_new_id_with_a_matching_name_is_fuzzy_merged_and_edges_redirect() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // v1: capability cap:cache "Caching".
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_cap("cap:cache", "Caching", false),
    )
    .unwrap();

    // v2: a *different* id but the same name → resolves to the existing node
    // instead of duplicating; a SATISFIES edge on the new id redirects.
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:cache-2", "Caching", true),
        )
        .unwrap();

    // The merge happened and is recorded (never silent).
    assert_eq!(report.fuzzy_merges.len(), 1);
    assert_eq!(report.fuzzy_merges[0].extracted_id, "cap:cache-2");
    assert_eq!(report.fuzzy_merges[0].canonical_id, "cap:cache");

    // No duplicate: still one capability, and the new id is not a node.
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 1);
    assert!(
        g.get_node(node::CAPABILITY, "cap:cache-2")
            .unwrap()
            .is_none()
    );

    // The edge that named cap:cache-2 landed on the canonical cap:cache.
    let sat = g.outgoing("cap:cache", Some(edge::SATISFIES)).unwrap();
    assert_eq!(sat.len(), 1);
    assert_eq!(sat[0].to_id, "req:lat");
    assert!(
        report.dropped_edges.is_empty(),
        "the aliased edge must not be dropped"
    );
}

#[test]
fn a_new_id_with_a_dissimilar_name_is_not_merged() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_cap("cap:cache", "Caching", false),
    )
    .unwrap();

    // A genuinely different capability → new node, no merge (conservative).
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:telemetry", "Telemetry", false),
        )
        .unwrap();

    assert!(report.fuzzy_merges.is_empty());
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 2);
}

#[test]
fn reingest_identical_content_is_a_noop_no_snapshot() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_v("under 200ms"),
    )
    .unwrap();

    // Re-ingest the same content: everything resolves matched-unchanged.
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_v("under 200ms"),
        )
        .unwrap();

    assert_eq!(report.nodes_evolved, 0);
    assert_eq!(report.nodes_unchanged, 3); // project, requirement, capability
    assert_eq!(report.nodes_created, 1); // only the new provenance fragment
    assert_eq!(report.epoch_used, None, "nothing evolved → no epoch opened");
    assert_eq!(
        g.count_nodes(node::SNAPSHOT).unwrap(),
        0,
        "unchanged content must not snapshot"
    );
}

// ---- Interfaces: the contract between two components ------------------------
//
// The seam the original Reflow lost track of — a change lands on one side of a
// service boundary and the other side is never revisited. Extraction has to
// produce both sides for PROPAGATE to be able to cross.

/// A mock scripted through the interfaces pass, parameterised on the discovery
/// gate, the components pass, and the interfaces pass.
fn iface_mock(discovery: &str, components: &str, interfaces: &str) -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:s","name":"Scoreboard","objective":"show scores","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            r#"{"requirements":[{"id":"req:live","name":"Live scores","statement":"scores update live","priority":"high"}]}"#,
        )
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:score","name":"Scoring","description":"track the score"}]}"#,
        )
        .on_contains("[pass:discovery]", discovery)
        .on_contains("[pass:components]", components)
        .on_contains("[pass:interfaces]", interfaces)
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

const DISCOVERY_WITH_INTERFACES: &str = r#"{"components":true,"interfaces":true,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#;
const TWO_COMPONENTS: &str = r#"{"components":[{"id":"cmp:api","name":"Score API","purpose":"serve scores","allocated_capability_ids":["cap:score"]},{"id":"cmp:ui","name":"Scoreboard UI","purpose":"show scores","allocated_capability_ids":[]}]}"#;

#[test]
fn interfaces_pass_extracts_both_sides_of_a_contract() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            "The Scoreboard UI reads from the Score API over REST.",
            &IngestOptions::default(),
            &iface_mock(
                DISCOVERY_WITH_INTERFACES,
                TWO_COMPONENTS,
                r#"{"interfaces":[{"id":"ifc:scores","name":"Scores endpoint","medium":"REST","spec":"GET /scores","provided_by_component_id":"cmp:api","consumed_by_component_ids":["cmp:ui"]}]}"#,
            ),
        )
        .unwrap();

    assert_eq!(report.status, IngestStatus::Ok, "clean run: {report:?}");
    assert_eq!(g.count_nodes(node::INTERFACE).unwrap(), 1);

    let provides = g.outgoing("cmp:api", Some(edge::PROVIDES)).unwrap();
    assert_eq!(provides.len(), 1);
    assert_eq!(provides[0].to_id, "ifc:scores");

    let consumes = g.outgoing("cmp:ui", Some(edge::CONSUMES)).unwrap();
    assert_eq!(consumes.len(), 1);
    assert_eq!(consumes[0].to_id, "ifc:scores");

    // A paired contract is not a gap.
    let gaps = g.detect_gaps().unwrap();
    assert!(
        !gaps.iter().any(|c| matches!(
            c.gap_source,
            GapSource::UnprovidedInterface | GapSource::UnconsumedInterface
        )),
        "both sides were extracted, so nothing to ask about"
    );
}

#[test]
fn an_extracted_contract_carries_impact_across_the_boundary() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        "The Scoreboard UI reads from the Score API over REST.",
        &IngestOptions::default(),
        &iface_mock(
            DISCOVERY_WITH_INTERFACES,
            TWO_COMPONENTS,
            r#"{"interfaces":[{"id":"ifc:scores","name":"Scores endpoint","medium":"REST","provided_by_component_id":"cmp:api","consumed_by_component_ids":["cmp:ui"]}]}"#,
        ),
    )
    .unwrap();

    // The whole point: from prose alone, changing the provider now surfaces the
    // consumer that the original Reflow would have left behind.
    let radius = g
        .propagate_from(&["cmp:api"], PropagateOptions::default())
        .unwrap();
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cmp:ui"),
        "the far side of an extracted contract must be in the blast radius"
    );
}

#[test]
fn interfaces_pass_is_gated_on_the_discovery_classifier() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let no_ifaces = r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#;
    g.ingest(
        "Two components, no contracts described.",
        &IngestOptions::default(),
        &iface_mock(
            no_ifaces,
            TWO_COMPONENTS,
            r#"{"interfaces":[{"id":"ifc:ghost","name":"Should not be extracted","provided_by_component_id":"cmp:api","consumed_by_component_ids":["cmp:ui"]}]}"#,
        ),
    )
    .unwrap();

    assert_eq!(
        g.count_nodes(node::INTERFACE).unwrap(),
        0,
        "the classifier said no interfaces; the pass must not run"
    );
}

#[test]
fn interfaces_pass_is_gated_on_components_existing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let no_components = r#"{"components":false,"interfaces":true,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#;
    g.ingest(
        "An early brief with no parts named yet.",
        &IngestOptions::default(),
        &iface_mock(
            no_components,
            r#"{"components":[]}"#,
            r#"{"interfaces":[{"id":"ifc:premature","name":"Premature"}]}"#,
        ),
    )
    .unwrap();

    assert_eq!(
        g.count_nodes(node::INTERFACE).unwrap(),
        0,
        "a contract needs two sides; without components it must not be extracted"
    );
}

#[test]
fn an_ungrounded_provider_stays_unpaired_and_becomes_a_question() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            "The Scoreboard UI reads scores from somewhere.",
            &IngestOptions::default(),
            &iface_mock(
                DISCOVERY_WITH_INTERFACES,
                TWO_COMPONENTS,
                // No provider named — extraction correctly declines to guess.
                r#"{"interfaces":[{"id":"ifc:scores","name":"Scores endpoint","consumed_by_component_ids":["cmp:ui"]}]}"#,
            ),
        )
        .unwrap();

    assert_eq!(report.status, IngestStatus::Ok, "omitting is not an error");
    assert_eq!(g.count_nodes(node::INTERFACE).unwrap(), 1);
    assert!(
        g.incoming("ifc:scores", Some(edge::PROVIDES))
            .unwrap()
            .is_empty()
    );

    let gaps = g.detect_gaps().unwrap();
    assert!(
        gaps.iter()
            .any(|c| c.gap_source == GapSource::UnprovidedInterface
                && c.affected_ids == vec!["ifc:scores"]),
        "the missing side must come back as a question, not a guess"
    );
}

#[test]
fn an_unknown_interface_medium_warns_rather_than_dropping_the_contract() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            "The UI talks to the API by carrier pigeon.",
            &IngestOptions::default(),
            &iface_mock(
                DISCOVERY_WITH_INTERFACES,
                TWO_COMPONENTS,
                r#"{"interfaces":[{"id":"ifc:scores","name":"Scores endpoint","medium":"carrier_pigeon","provided_by_component_id":"cmp:api","consumed_by_component_ids":["cmp:ui"]}]}"#,
            ),
        )
        .unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert!(
        report.warnings.iter().any(|w| w.contains("carrier_pigeon")),
        "the bad enum must be surfaced, got {:?}",
        report.warnings
    );
    assert_eq!(
        g.count_nodes(node::INTERFACE).unwrap(),
        1,
        "a bad medium must not cost us the contract itself"
    );
    assert_eq!(
        g.outgoing("cmp:api", Some(edge::PROVIDES)).unwrap().len(),
        1
    );
}

#[test]
fn a_phantom_component_in_a_contract_is_dropped_not_written() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            "The Score API serves something unnamed.",
            &IngestOptions::default(),
            &iface_mock(
                DISCOVERY_WITH_INTERFACES,
                TWO_COMPONENTS,
                r#"{"interfaces":[{"id":"ifc:scores","name":"Scores endpoint","provided_by_component_id":"cmp:api","consumed_by_component_ids":["cmp:ghost"]}]}"#,
            ),
        )
        .unwrap();

    assert_eq!(report.status, IngestStatus::Partial);
    assert!(
        report
            .dropped_edges
            .iter()
            .any(|d| d.edge_type == edge::CONSUMES && d.to_id == "ifc:scores"),
        "an edge to a component that was never created must be reported, got {:?}",
        report.dropped_edges
    );
    assert!(
        g.incoming("ifc:scores", Some(edge::CONSUMES))
            .unwrap()
            .is_empty(),
        "no phantom edge may be written"
    );
}
