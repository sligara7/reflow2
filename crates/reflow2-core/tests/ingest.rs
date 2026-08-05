//! INGEST — freeform input → graph, via the mock LLM backend.
//!
//! Each extraction pass tags its prompt with a `[pass:NAME]` marker, so the
//! scriptable mock returns per-pass canned JSON by matching that marker.

use reflow2_core::detect::GapSource;
use reflow2_core::nodes::{edge, node};
use reflow2_core::propagate::PropagateOptions;
use reflow2_core::{
    ChangeType, DesignGraph, IngestOptions, IngestStatus, MatchKind, MockLlmBackend,
    parse_snapshot_state,
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

    // Set a status the re-ingest's extraction will NOT produce. Before BL-58,
    // matched-evolved applied the edit with create_node (replace), which
    // re-materialized schema defaults over everything the text omitted —
    // silently resetting this back to `proposed`. It must survive the merge.
    g.set_requirement_status("req:lat", "accepted").unwrap();

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
    // ...and the status set between ingests survives the merge (BL-58).
    assert_eq!(
        live.properties["status"].as_str(),
        Some("accepted"),
        "a re-ingest must merge, not reset properties the text did not mention"
    );

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

/// Ingest the same pair of documents in both orders and read the name back.
///
/// Returns `(canonical_name, alias_name, capability_count, merge_count)`.
fn ingest_both(
    first: (&str, &str),
    second: (&str, &str),
) -> (String, Option<String>, usize, usize) {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:a".into(),
            ..Default::default()
        },
        &mock_cap(first.0, first.1, false),
    )
    .unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:b".into(),
                ..Default::default()
            },
            &mock_cap(second.0, second.1, false),
        )
        .unwrap();
    let name = g
        .get_node(node::CAPABILITY, first.0)
        .unwrap()
        .or_else(|| g.get_node(node::CAPABILITY, second.0).unwrap())
        .expect("one capability survives")
        .properties
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let alias = report
        .fuzzy_merges
        .first()
        .and_then(|m| m.alias_name.clone());
    (
        name,
        alias,
        g.count_nodes(node::CAPABILITY).unwrap(),
        report.fuzzy_merges.len(),
    )
}

/// `req:corpus-ingest`'s load-bearing clause: *"Ordering must not decide meaning
/// — which file happened to be read first must not determine the canonical name
/// of anything."*
///
/// It did, and a corpus is exactly where it bites, because the order is the
/// iteration order of a folder nobody chose. Measured before the fix, same two
/// documents, same design: `A then B -> "Cache Read Path"`,
/// `B then A -> "Read Path Cache"`. The merge was always right — one node, one
/// recorded merge — and the NAME followed whichever document was read last,
/// because the extracted map overwrites `name` on the survivor.
#[test]
fn the_order_two_documents_arrive_in_does_not_decide_the_canonical_name() {
    let a = ("cap:read-path-cache", "Read Path Cache");
    let b = ("cap:cache-read-path", "Cache Read Path");

    let (ab_name, ab_alias, ab_caps, ab_merges) = ingest_both(a, b);
    let (ba_name, ba_alias, ba_caps, ba_merges) = ingest_both(b, a);

    // The merge itself was never the bug and must not regress.
    assert_eq!((ab_caps, ab_merges), (1, 1), "A then B should merge to one");
    assert_eq!((ba_caps, ba_merges), (1, 1), "B then A should merge to one");

    assert_eq!(
        ab_name, ba_name,
        "the canonical name must not depend on which document was read first"
    );

    // Equal length here, so the lexicographic tiebreak decides — and the point
    // is that it decides the SAME way both times.
    assert_eq!(ab_name, "Cache Read Path");

    // Nothing is discarded in silence: the name that lost is reported, both
    // ways round, so the evidence a human chose it survives the merge.
    assert_eq!(ab_alias.as_deref(), Some("Read Path Cache"));
    assert_eq!(ba_alias.as_deref(), Some("Read Path Cache"));
}

/// The counterweight, and the reason the rule is "longer wins" rather than
/// "lexicographically smallest wins": a longer name is the more specific one,
/// which is the same reading `token_subset_match` already applies when it
/// suggests a survivor. Order still must not matter.
#[test]
fn the_more_specific_name_survives_a_merge_whichever_way_round() {
    let short = ("cap:auth", "Auth Gateway Service");
    let long = ("cap:auth-2", "Auth Gateway Services");

    let (ab_name, ab_alias, ..) = ingest_both(short, long);
    let (ba_name, ba_alias, ..) = ingest_both(long, short);

    assert_eq!(ab_name, "Auth Gateway Services", "the longer name survives");
    assert_eq!(ba_name, ab_name, "and does so in either order");
    assert_eq!(ab_alias.as_deref(), Some("Auth Gateway Service"));
    assert_eq!(ba_alias.as_deref(), Some("Auth Gateway Service"));
}

/// The corpus half of `dec:ask-not-repair`, end to end.
///
/// That decision requires a suspected duplicate to be ASKED, never silently
/// merged, and `cap:corpus-ingest` names the consequence: *"at corpus scale the
/// asking must be batched or the feature is unusable"*. A `MergeCandidate`
/// alone cannot be batched — it lives in one document's report and is gone when
/// the caller opens the next file, so four hundred documents produce four
/// hundred separate asks to an agent that has forgotten the last one.
///
/// Persisting the suspicion as a `DUPLICATES` edge hands it to machinery that
/// already exists: HEAL's `duplicate` detector collects them across the whole
/// run, in any order, however long the run takes. **This test is the proof that
/// the handoff actually happens** — the edge alone would only be a claim.
#[test]
fn a_near_match_becomes_a_standing_question_heal_can_collect() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:one".into(),
            ..Default::default()
        },
        &mock_cap("cap:auth", "Auth Service", false),
    )
    .unwrap();

    // 84 — above Capability's fuzzy_threshold (82), below auto-merge (90), so
    // it is created AND questioned rather than merged.
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:two".into(),
                ..Default::default()
            },
            &mock_cap("cap:auth-2", "Authentication Service", false),
        )
        .unwrap();

    assert!(report.fuzzy_merges.is_empty(), "the band must not merge");
    assert_eq!(report.merge_candidates.len(), 1);
    assert_eq!(
        report.duplicates_recorded, 1,
        "the suspicion must outlive the document that raised it"
    );
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 2, "both survive");

    // The edge is drawn between the two real nodes.
    let dups = g.outgoing("cap:auth-2", Some(edge::DUPLICATES)).unwrap();
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].to_id, "cap:auth");
    assert_eq!(dups[0].properties["confidence"].as_f64(), Some(0.84));

    // THE POINT: HEAL now carries the question, without ingest telling it to.
    let issue = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .find(|i| i.category.as_str() == "duplicate")
        .expect("a persisted suspicion must reach the batched ask");
    assert!(
        issue.affected_ids.contains(&"cap:auth".to_string())
            && issue.affected_ids.contains(&"cap:auth-2".to_string()),
        "{issue:?}"
    );
}

/// The counterweight, and the reason the edge is drawn only in the ask band: at
/// or above `auto_merge_threshold` the nodes ARE merged, so there is nothing
/// left to ask. Drawing one anyway would hand HEAL a question about a node that
/// no longer exists and make every clean corpus run look ambiguous.
#[test]
fn an_auto_merge_leaves_no_question_behind() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:one".into(),
            ..Default::default()
        },
        &mock_cap("cap:cache", "Caching", false),
    )
    .unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:two".into(),
                ..Default::default()
            },
            &mock_cap("cap:cache-2", "Caching", false),
        )
        .unwrap();

    assert_eq!(report.fuzzy_merges.len(), 1, "identical names merge");
    assert_eq!(
        report.duplicates_recorded, 0,
        "a merge answers the question; it must not also ask it"
    );
    assert!(
        !g.detect_defects()
            .unwrap()
            .iter()
            .any(|i| i.category.as_str() == "duplicate"),
        "a clean convergence must not read as an outstanding duplicate"
    );
}

/// Two documents that agree on the name must not manufacture an alias — an
/// `alias_name` on every merge would make "these two specs disagreed about what
/// to call it" unreadable, which is the only thing the field is for.
#[test]
fn agreeing_documents_record_no_alias() {
    let (name, alias, caps, merges) =
        ingest_both(("cap:cache", "Caching"), ("cap:cache-2", "Caching"));
    assert_eq!((caps, merges), (1, 1));
    assert_eq!(name, "Caching");
    assert_eq!(alias, None, "identical names are not a disagreement");
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

/// The band between a type's `fuzzy_threshold` and its `auto_merge_threshold`
/// was invisible until 2026-07-26: a name that resembled an existing node but
/// not closely enough to merge was simply created as a second node, and nothing
/// said so. That is the failure a corpus makes constantly — "Auth Service" in
/// one document and a near-variant in another, quietly becoming two components.
///
/// Capability declares `fuzzy_threshold: 82`; nothing declares an
/// `auto_merge_threshold`, so the foundation's default of 90 applies. A name
/// scoring in [82, 90) must therefore be REPORTED and NOT merged.
#[test]
fn a_near_match_below_auto_merge_is_reported_not_merged() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_cap("cap:auth", "Auth Service", false),
    )
    .unwrap();

    // "Auth Service" vs "Authentication Service" scores 84 — the canonical
    // corpus case, and squarely in the ask-band. Before this change reflow2
    // silently created two components for it.
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:auth-2", "Authentication Service", false),
        )
        .unwrap();

    assert!(
        report.fuzzy_merges.is_empty(),
        "a score below auto-merge must NOT merge: {:?}",
        report.fuzzy_merges
    );
    assert_eq!(
        report.merge_candidates.len(),
        1,
        "and it must be reported rather than silently duplicated: {report:?}"
    );
    let c = &report.merge_candidates[0];
    assert_eq!(c.extracted_id, "cap:auth-2");
    assert_eq!(c.candidate_id, "cap:auth");
    assert_eq!(
        c.auto_merge_threshold, 90,
        "the default, since none declared"
    );
    assert!(
        c.score >= 82 && c.score < 90,
        "the candidate must sit in the ask-band, got {}",
        c.score
    );

    // Both nodes exist: nothing was destroyed by a number.
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 2);
    assert!(
        g.get_node(node::CAPABILITY, "cap:auth-2")
            .unwrap()
            .is_some()
    );
}

/// The other half, and the one that guards against a regression dressed up as a
/// fix: reading the thresholds from the schema must not change what MERGES.
/// The old hardcoded 90 happened to equal the foundation's default auto-merge
/// threshold, so an identical name merges exactly as it always did.
#[test]
fn reading_thresholds_from_the_schema_does_not_change_what_merges() {
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
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:cache-2", "Caching", false),
        )
        .unwrap();

    assert_eq!(report.fuzzy_merges.len(), 1, "an exact name still merges");
    assert!(
        report.merge_candidates.is_empty(),
        "and a merge is not also reported as a question"
    );
}

/// The case similarity SCORING can never reach. A ratio falls as the length
/// difference grows, so `Gateway` vs `API Gateway` scores **74** — below the 82
/// that Capability declares — while being one of the commonest things a corpus
/// contains. No threshold tuning finds it; only a structural question does.
///
/// Reported, never merged: `Auth Service` is a strict subset of `Legacy Auth
/// Service` too, and those are plainly two different services.
#[test]
fn a_token_subset_is_found_where_scoring_cannot_reach() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_cap("cap:gw", "Gateway", false),
    )
    .unwrap();

    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:gw-2", "API Gateway", false),
        )
        .unwrap();

    assert!(
        report.fuzzy_merges.is_empty(),
        "a subset relation must never merge on its own: {:?}",
        report.fuzzy_merges
    );
    assert_eq!(
        report.merge_candidates.len(),
        1,
        "the pair scoring 74 must still be surfaced: {report:?}"
    );
    let c = &report.merge_candidates[0];
    assert_eq!(c.match_kind, MatchKind::TokenSubset, "found structurally");
    assert_eq!(c.extracted_id, "cap:gw-2");
    assert_eq!(c.candidate_id, "cap:gw");
    // The LONGER, more specific name is the suggested survivor — storyflow's
    // rule. Here that is the newly extracted "API Gateway".
    assert_eq!(c.suggested_survivor, "cap:gw-2");

    // Both nodes exist. Nothing was decided.
    assert_eq!(g.count_nodes(node::CAPABILITY).unwrap(), 2);
}

/// Two unrelated names must not become a candidate just because the structural
/// pass exists — a pass that fires on everything is worse than no pass.
#[test]
fn unrelated_names_produce_no_candidate() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions {
            fragment_id: "frag:v1".into(),
            ..Default::default()
        },
        &mock_cap("cap:gw", "Gateway", false),
    )
    .unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions {
                fragment_id: "frag:v2".into(),
                ..Default::default()
            },
            &mock_cap("cap:bill", "Invoice Reconciliation", false),
        )
        .unwrap();

    assert!(report.fuzzy_merges.is_empty());
    assert!(
        report.merge_candidates.is_empty(),
        "unrelated names must stay unrelated: {:?}",
        report.merge_candidates
    );
}

/// A backend whose discovery gate reports decisions present, and which returns
/// one recorded choice governing the requirement.
fn mock_with_decision(governs: &str) -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:w","name":"Widget","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms"}]}"#,
        )
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":true,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#,
        )
        .on_contains(
            "[pass:decisions]",
            format!(
                r#"{{"decisions":[{{"id":"dec:cache-aside","name":"How reads are cached","decision":"Cache-aside with a 60s TTL","rationale":"The team measured write amplification on write-through and rejected it","governs_ids":["{governs}"]}}]}}"#
            ),
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

/// The rationale layer — *why* something was built the way it was — is what an
/// old corpus is richest in and what a codebase cannot be re-read to recover.
/// Until 2026-07-27 ingest extracted none of it: the discovery gate classified
/// `decisions` and nothing consumed the flag.
#[test]
fn a_recorded_choice_is_extracted_with_its_reasoning() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions::default(),
            &mock_with_decision("req:lat"),
        )
        .unwrap();
    assert_eq!(report.status, IngestStatus::Ok, "{report:?}");

    let d = g
        .get_node(node::DECISION, "dec:cache-aside")
        .unwrap()
        .expect("the decision must be created");
    assert_eq!(
        d.properties.get("decision").and_then(|v| v.as_str()),
        Some("Cache-aside with a 60s TTL")
    );
    assert!(
        d.properties
            .get("rationale")
            .and_then(|v| v.as_str())
            .is_some_and(|r| r.contains("write amplification")),
        "the source's own reasoning must survive, not be paraphrased away"
    );

    // The requirement points at the choice that shaped it.
    let gov = g.outgoing("req:lat", Some(edge::GOVERNED_BY)).unwrap();
    assert_eq!(gov.len(), 1);
    assert_eq!(gov[0].to_id, "dec:cache-aside");
}

/// The doctrine most likely to be "fixed" wrongly by a later reader, so it is
/// pinned. An extraction is the agent's reading of somebody's document, not the
/// user's signature — and an `accepted` Decision is what where-am-i reads back
/// as "what you decided", what the fork layer treats as binding, and what the
/// KPP contradiction check reads as a trade already made.
#[test]
fn an_extracted_decision_is_proposed_never_accepted() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions::default(),
        &mock_with_decision("req:lat"),
    )
    .unwrap();

    let d = g
        .get_node(node::DECISION, "dec:cache-aside")
        .unwrap()
        .unwrap();
    assert_eq!(
        d.properties.get("status").and_then(|v| v.as_str()),
        Some("proposed"),
        "ingest must not assert that a recovered choice was ratified here"
    );
}

/// An id whose type cannot be read from its prefix is REPORTED and dropped, not
/// written against a guessed type. Rule 4 — no silent drops, no invented facts.
#[test]
fn a_governed_id_with_an_unknown_prefix_is_reported_not_guessed() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions::default(),
            &mock_with_decision("wat:mystery"),
        )
        .unwrap();

    assert!(
        g.get_node(node::DECISION, "dec:cache-aside")
            .unwrap()
            .is_some(),
        "the decision itself still lands"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("wat:mystery") && w.contains("not recognisable")),
        "the dropped edge must be named: {:?}",
        report.warnings
    );
}

fn mock_with_verification(covers: &str, method: &str) -> MockLlmBackend {
    MockLlmBackend::new()
        .on_contains(
            "[pass:project_intent]",
            r#"{"project":{"id":"proj:w","name":"Widget","mode":"flexible"}}"#,
        )
        .on_contains(
            "[pass:requirements]",
            r#"{"requirements":[{"id":"req:lat","name":"Latency","statement":"under 200ms"}]}"#,
        )
        .on_contains("[pass:constraints]", r#"{"constraints":[]}"#)
        .on_contains(
            "[pass:capabilities]",
            r#"{"capabilities":[{"id":"cap:cache","name":"Caching","description":"serve reads"}]}"#,
        )
        .on_contains(
            "[pass:discovery]",
            r#"{"components":false,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":true,"flows":false,"resources":false}"#,
        )
        .on_contains(
            "[pass:verifications]",
            format!(
                r#"{{"verifications":[{{"id":"ver:load","name":"Load test at 5k rps","description":"Ran 2024-03-12; p99 measured at 140ms, which the report calls a pass","method":"{method}","verifies_ids":["{covers}"]}}]}}"#
            ),
        )
        .on_contains("[pass:satisfies]", r#"{"satisfies":[]}"#)
        .on_contains("[pass:dependencies]", r#"{"dependencies":[]}"#)
}

/// Test evidence is the other third of what a body of documents holds, and
/// ingest recorded none of it before 2026-07-27 — the discovery gate classified
/// `verifications` and nothing consumed the flag.
#[test]
fn a_recorded_check_is_extracted_with_its_method() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions::default(),
            &mock_with_verification("cap:cache", "measurement"),
        )
        .unwrap();
    assert_eq!(report.status, IngestStatus::Ok, "{report:?}");

    let v = g
        .get_node(node::VERIFICATION, "ver:load")
        .unwrap()
        .expect("the check must be created");
    assert_eq!(
        v.properties.get("method").and_then(|x| x.as_str()),
        Some("measurement")
    );
    assert!(
        v.properties
            .get("description")
            .and_then(|x| x.as_str())
            .is_some_and(|d| d.contains("140ms")),
        "the source's own account of the outcome must survive"
    );

    let cov = g.outgoing("ver:load", Some(edge::VERIFIES)).unwrap();
    assert_eq!(cov.len(), 1);
    assert_eq!(cov[0].to_id, "cap:cache");
}

/// **The one that matters.** A document saying a test passed is a CLAIM, not
/// reflow2 watching it pass. Landing it `passing` would let prose promote a
/// capability to verified — the exact "green while nothing was checked" failure
/// this project found in its own code the day before. The claim is kept as
/// text; the status is not asserted, and the gap keeps asking.
#[test]
fn an_extracted_check_is_planned_and_does_not_silence_the_gap() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.ingest(
        BRIEF,
        &IngestOptions::default(),
        &mock_with_verification("cap:cache", "test"),
    )
    .unwrap();

    let v = g.get_node(node::VERIFICATION, "ver:load").unwrap().unwrap();
    assert_eq!(
        v.properties.get("status").and_then(|x| x.as_str()),
        Some("planned"),
        "a document's claim is not an observed outcome"
    );

    let still_asking = g.detect_gaps().unwrap().into_iter().any(|gap| {
        gap.gap_source == GapSource::UnverifiedCapability
            && gap.affected_ids.contains(&"cap:cache".to_string())
    });
    assert!(
        still_asking,
        "attaching an unproven check must NOT silence unverified_capability"
    );
}

/// An unknown method warns and falls back rather than taking the node down —
/// the same bargain the interface `medium` makes.
#[test]
fn an_unknown_verification_method_warns_and_defaults() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let report = g
        .ingest(
            BRIEF,
            &IngestOptions::default(),
            &mock_with_verification("cap:cache", "vibes"),
        )
        .unwrap();

    let v = g.get_node(node::VERIFICATION, "ver:load").unwrap().unwrap();
    assert_eq!(
        v.properties.get("method").and_then(|x| x.as_str()),
        Some("test"),
        "the schema default stands in"
    );
    assert!(
        report.warnings.iter().any(|w| w.contains("vibes")),
        "and the substitution is reported: {:?}",
        report.warnings
    );
}
