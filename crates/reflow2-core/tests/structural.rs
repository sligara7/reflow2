//! Structural (graph-topology) HEAL detectors, powered by `dynograph-graph`.
//!
//! The subtle one is single-point-of-failure: a design's golden thread is
//! tree-shaped, so a naive articulation-point check would flag every internal
//! node. These tests pin the *selective* behavior — only a node that separates
//! ≥2 real subsystems counts.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

fn has(g: &DesignGraph, cat: HealCategory) -> bool {
    g.detect_defects()
        .unwrap()
        .iter()
        .any(|d| d.category == cat)
}

/// A linear golden thread: req—cap—cmp plus an artifact/verification. Tree-shaped.
fn linear_thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:a", "Cmp A", "part a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.allocate("cap:a", "cmp:a").unwrap();
    g
}

#[test]
fn tree_shaped_thread_flags_no_single_point_of_failure() {
    // cap:a is an articulation point (removing it isolates req and cmp), but it
    // only separates leaves, not two real subsystems — so it must NOT be flagged.
    let g = linear_thread();
    assert!(
        !has(&g, HealCategory::SinglePointOfFailure),
        "leaf-cutting on a tree is not a real SPOF"
    );
}

/// Two 2-node subsystems joined only through `hub_is_operational`'s choice of
/// bridge — each side gets its own artifact so it stays non-trivial (≥2 nodes)
/// once the bridge is removed.
fn bridged_subsystems(operational_hub: bool) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:a", "A", "part a", None).unwrap();
    g.add_component("cmp:b", "B", "part b", None).unwrap();
    g.create_node(node::ARTIFACT, "art:a", Props::new().set("name", "a.rs"))
        .unwrap();
    g.create_node(node::ARTIFACT, "art:b", Props::new().set("name", "b.rs"))
        .unwrap();
    if operational_hub {
        // The bridge is an Interface both sides meet at — a thing that runs.
        g.add_interface("ifc:hub", "Shared contract").unwrap();
        g.provides("cmp:a", "ifc:hub").unwrap();
        g.consumes("cmp:b", "ifc:hub").unwrap();
    } else {
        // The bridge is a Capability both sides host — pure intent.
        g.add_capability("cap:hub", "Hub", "central capability", None)
            .unwrap();
        g.allocate("cap:hub", "cmp:a").unwrap();
        g.allocate("cap:hub", "cmp:b").unwrap();
    }
    for (art, cmp) in [("art:a", "cmp:a"), ("art:b", "cmp:b")] {
        g.create_edge(
            edge::DEPENDS_ON,
            node::ARTIFACT,
            art,
            node::COMPONENT,
            cmp,
            Props::new(),
        )
        .unwrap();
    }
    g
}

#[test]
fn an_interface_bridging_two_subsystems_is_a_single_point_of_failure() {
    // Removing ifc:hub leaves {cmp:a, art:a} and {cmp:b, art:b}: two
    // subsystems, severed by the failure of a thing that actually runs.
    let g = bridged_subsystems(true);
    let spof: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::SinglePointOfFailure)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert_eq!(spof, ["ifc:hub"]);
}

#[test]
fn a_capability_bridging_two_subsystems_is_not_a_single_point_of_failure() {
    // Same topology, but the hub is intent rather than a running part. BL-5's
    // second pass: the suggested fix is add_redundancy, and redundancy is only
    // coherent for things that operate — a capability's failure IS its
    // component's failure, already reported there, and an intent node being an
    // articulation point is the golden thread working. On reflow2's own
    // 96-node design the topology test alone named 22 nodes, mostly
    // Requirements and Capabilities that are load-bearing because they are
    // cross-cutting.
    let g = bridged_subsystems(false);
    assert!(
        !has(&g, HealCategory::SinglePointOfFailure),
        "an intent hub must not be told to add_redundancy"
    );
}

#[test]
fn a_separate_cluster_is_a_disconnected_community() {
    // Main thread + a detached 2-node island (cap:x—cmp:x) linked to nothing else.
    let mut g = linear_thread();
    g.add_capability("cap:x", "X", "island cap", None).unwrap();
    g.add_component("cmp:x", "X part", "island part", None)
        .unwrap();
    g.allocate("cap:x", "cmp:x").unwrap(); // island internally connected, externally not

    assert!(has(&g, HealCategory::DisconnectedCommunity));
    let island: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .find(|d| d.category == HealCategory::DisconnectedCommunity)
        .unwrap()
        .affected_ids;
    assert_eq!(island, ["cap:x", "cmp:x"]);
}

#[test]
fn a_fully_connected_thread_has_no_structural_defects() {
    let g = linear_thread();
    assert!(!has(&g, HealCategory::DisconnectedCommunity));
    assert!(!has(&g, HealCategory::SinglePointOfFailure));
    assert!(!has(&g, HealCategory::DeadEnd));
}

#[test]
fn an_isolated_component_is_a_dead_end() {
    let mut g = linear_thread();
    g.add_component("cmp:orphan", "Orphan", "connected to nothing", None)
        .unwrap();
    assert!(has(&g, HealCategory::DeadEnd));
    let dead: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::DeadEnd)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert_eq!(dead, ["cmp:orphan"]);
}

#[test]
fn structural_defects_are_generative_and_gated_for_review() {
    let mut g = linear_thread();
    g.add_component("cmp:orphan", "Orphan", "connected to nothing", None)
        .unwrap();
    let proposal = g
        .propose_heal(reflow2_core::HealOptions::default())
        .unwrap();
    // The dead end has no content-free fix, so it's a review-gated generative stub.
    assert!(proposal.requires_human_review);
    assert!(
        proposal
            .generated_content
            .iter()
            .any(|s| s.kind == "bridge"),
        "dead end should propose a bridging link for review"
    );
}

/// BL-5. The SPOF test used to ask "are there ≥2 non-trivial components after
/// removing this node?" — which silently assumed the design was connected to
/// start with. One unrelated island already satisfies it, so every articulation
/// point elsewhere reported as a single point of failure with nothing about its
/// fragility changed. Measuring against the baseline is the fix.
///
/// This is the blind trial's complaint from the other side: *"all 15 defects
/// vanished at once when I added two bookkeeping edges."* Those edges attached
/// an island, the count fell below the threshold, and the list cleared.
#[test]
fn an_unrelated_island_does_not_make_everything_a_single_point_of_failure() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:hub", "Hub", "holds both", None)
        .unwrap();

    // Two real subsystems (a running part with its file), joined only
    // through cmp:hub.
    for side in ["a", "b"] {
        let c = format!("cmp:{side}");
        g.add_component(&c, &c, "a part", None).unwrap();
        g.create_edge(
            edge::DEPENDS_ON,
            node::COMPONENT,
            &c,
            node::COMPONENT,
            "cmp:hub",
            Props::new(),
        )
        .unwrap();
        let art = format!("art:{side}");
        g.create_node(node::ARTIFACT, &art, Props::new().set("name", art.as_str()))
            .unwrap();
        g.create_edge(
            edge::REALIZES,
            node::ARTIFACT,
            &art,
            node::COMPONENT,
            &c,
            Props::new(),
        )
        .unwrap();
    }
    let spofs = |g: &DesignGraph| -> Vec<String> {
        g.detect_defects()
            .unwrap()
            .iter()
            .filter(|d| d.category == HealCategory::SinglePointOfFailure)
            .flat_map(|d| d.affected_ids.clone())
            .collect()
    };
    assert_eq!(
        spofs(&g),
        ["cmp:hub"],
        "the hub genuinely holds two subsystems together"
    );

    // Add a second, entirely separate part of the design. Nothing about the
    // first part's fragility changes, so nothing new should be flagged.
    g.add_component("cmp:island", "Island", "unrelated", None)
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:island",
        Props::new().set("name", "island.rs"),
    )
    .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:island",
        node::COMPONENT,
        "cmp:island",
        Props::new(),
    )
    .unwrap();

    assert_eq!(
        spofs(&g),
        ["cmp:hub"],
        "an unrelated island must not turn other nodes into single points of failure"
    );
}

// ---- BL-69 · connectivity is measured on the operational network -----------

/// The `cmp:flow` shape from reflow2's own graph: a leaf module whose
/// capability, artifact and verification hang off it. On the full design
/// network, removing the module strands that intent cluster (≥2 nodes, so the
/// non-trivial filter passed) and the module fired as a single point of
/// failure — a healthily-modelled leaf punished for having its thread
/// recorded. The severed "subsystem" was made of sentences.
#[test]
fn an_intent_cluster_hanging_off_one_component_is_not_a_single_point_of_failure() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    // A minimal operational spine so cmp:leaf attaches to something real.
    g.add_component("cmp:hub", "Hub", "the spine", None)
        .unwrap();
    g.add_component("cmp:leaf", "Leaf", "a leaf module", None)
        .unwrap();
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:leaf",
        node::COMPONENT,
        "cmp:hub",
        Props::new(),
    )
    .unwrap();
    // The leaf's intent cluster: capability + verification + the capability's
    // artifact — connected to the rest of the design only through cmp:leaf.
    g.add_capability("cap:leaf", "Leaf cap", "what the leaf does", None)
        .unwrap();
    g.allocate("cap:leaf", "cmp:leaf").unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:leaf",
        Props::new().set("name", "leaf.rs"),
    )
    .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:leaf",
        node::CAPABILITY,
        "cap:leaf",
        Props::new(),
    )
    .unwrap();

    let spofs: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::SinglePointOfFailure)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert!(
        spofs.is_empty(),
        "stranding your own intent cluster is modelling, not fragility: {spofs:?}"
    );
}

/// The `cmp:export` shape from reflow2's own graph: a genuine operational cut
/// vertex that the old test could not see, because the parts it severs stayed
/// "connected" to the rest through a SATISFIES chain — intent edges carrying
/// phantom connectivity that exists on no runtime path.
#[test]
fn a_cut_vertex_hidden_by_intent_edges_is_still_a_single_point_of_failure() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:core", "Core", "the main body", None)
        .unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:core",
        Props::new().set("name", "core.rs"),
    )
    .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:core",
        node::COMPONENT,
        "cmp:core",
        Props::new(),
    )
    .unwrap();
    // The bridge: consumers reach the core only through it.
    g.add_component("cmp:bridge", "Bridge", "sole route", None)
        .unwrap();
    g.create_edge(
        edge::DEPENDS_ON,
        node::COMPONENT,
        "cmp:bridge",
        node::COMPONENT,
        "cmp:core",
        Props::new(),
    )
    .unwrap();
    g.add_interface("ifc:door", "The bridge's contract")
        .unwrap();
    g.provides("cmp:bridge", "ifc:door").unwrap();
    g.add_component("cmp:consumer", "Consumer", "behind the door", None)
        .unwrap();
    g.consumes("cmp:consumer", "ifc:door").unwrap();
    g.create_node(
        node::ARTIFACT,
        "art:consumer",
        Props::new().set("name", "consumer.rs"),
    )
    .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:consumer",
        node::COMPONENT,
        "cmp:consumer",
        Props::new(),
    )
    .unwrap();
    // The intent bypass that used to hide the bridge: consumer and core both
    // satisfy the same requirement through their capabilities, so on the full
    // design network the consumer stays "connected" without the bridge.
    g.add_requirement("req:shared", "Shared", "one intent")
        .unwrap();
    for (cap, cmp) in [("cap:core", "cmp:core"), ("cap:consumer", "cmp:consumer")] {
        g.add_capability(cap, cap, "d", None).unwrap();
        g.allocate(cap, cmp).unwrap();
        g.satisfies(cap, "req:shared").unwrap();
    }

    let spofs: Vec<String> = g
        .detect_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::SinglePointOfFailure)
        .flat_map(|d| d.affected_ids)
        .collect();
    assert!(
        spofs.contains(&"cmp:bridge".to_string()),
        "a SATISFIES chain is not a runtime path; the bridge is a real SPOF: {spofs:?}"
    );
}

// ---- BL-38 · a pure container is not a dead end ----------------------------

#[test]
fn an_assembly_whose_only_edges_are_containment_is_not_a_dead_end() {
    // cmp:mcp on reflow2's own design: a subsystem holding modules, reported
    // "not connected to anything" because the design network excludes CONTAINS
    // (decomposition is not traceability). An assembly speaks through its
    // children.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component(
        "cmp:parent",
        "Subsystem",
        "holds modules",
        Some("subsystem"),
    )
    .unwrap();
    g.add_component("cmp:leaf", "Module", "a module", None)
        .unwrap();
    g.contain_component("cmp:parent", "cmp:leaf").unwrap();
    g.allocate("cap:a", "cmp:leaf").unwrap();

    let defects = g.detect_defects().unwrap();
    let dead: Vec<&str> = defects
        .iter()
        .filter(|d| d.category == reflow2_core::HealCategory::DeadEnd)
        .flat_map(|d| d.affected_ids.iter().map(String::as_str))
        .collect();
    assert!(
        dead.is_empty(),
        "the container's connection flows through its child, got {dead:?}"
    );
}

#[test]
fn a_leaf_with_no_traceability_is_still_a_dead_end_even_inside_a_hierarchy() {
    // The exemption is for assemblies only. A contained leaf that hosts
    // nothing and provides nothing is the true case and must keep firing.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "A", "need a").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.satisfies("cap:a", "req:a").unwrap();
    g.add_component(
        "cmp:parent",
        "Subsystem",
        "holds modules",
        Some("subsystem"),
    )
    .unwrap();
    g.add_component("cmp:busy", "Busy", "hosts the capability", None)
        .unwrap();
    g.add_component("cmp:idle", "Idle", "hosts nothing", None)
        .unwrap();
    g.contain_component("cmp:parent", "cmp:busy").unwrap();
    g.contain_component("cmp:parent", "cmp:idle").unwrap();
    g.allocate("cap:a", "cmp:busy").unwrap();

    let defects = g.detect_defects().unwrap();
    let dead: Vec<&str> = defects
        .iter()
        .filter(|d| d.category == reflow2_core::HealCategory::DeadEnd)
        .flat_map(|d| d.affected_ids.iter().map(String::as_str))
        .collect();
    assert_eq!(dead, ["cmp:idle"], "the idle leaf, not the assembly");
}

// ---- F6 · a library is not a runtime single point of failure ---------------

/// From the storyflow trial: a shared library is imported by every service,
/// which makes it a perfect articulation point and a nonsense candidate —
/// you cannot run a second copy of a library to survive its failure, so the
/// suggested `add_redundancy` is not merely unhelpful but incoherent.
///
/// Topology cannot tell a library API from a service API; the discriminator
/// is a statement the modeller makes. `Interface.medium` defaults to a
/// runtime medium, so a design that says nothing behaves exactly as before —
/// silence has to be earned by an explicit `library`.
#[test]
fn a_library_hub_is_not_a_runtime_single_point_of_failure() {
    // The hub provides two contracts, each with its own group of consumers,
    // so the hub COMPONENT is the cut vertex. (With one shared contract the
    // Interface is the articulation point instead — a distinction the
    // fixture has to get right to be testing what it claims.)
    let build = |medium: &str| {
        let mut g = DesignGraph::open_in_memory().unwrap();
        g.add_project("proj:p", "P").unwrap();
        g.add_component("cmp:hub", "Hub", "the shared thing", None)
            .unwrap();
        for side in ["a", "b"] {
            let ifc = format!("ifc:{side}");
            g.create_node(
                node::INTERFACE,
                &ifc,
                Props::new()
                    .set("name", format!("{side} contract"))
                    .set("medium", medium),
            )
            .unwrap();
            g.provides("cmp:hub", &ifc).unwrap();
            for i in 0..2 {
                let c = format!("cmp:{side}{i}");
                g.add_component(&c, &c, "a consumer", None).unwrap();
                g.consumes(&c, &ifc).unwrap();
            }
        }
        g
    };
    let flagged = |g: &DesignGraph| -> Vec<String> {
        g.detect_defects()
            .unwrap()
            .into_iter()
            .filter(|d| d.category == HealCategory::SinglePointOfFailure)
            .flat_map(|d| d.affected_ids)
            .collect()
    };

    // Carried at run time (the default medium): a true single point of failure.
    assert!(
        flagged(&build("REST")).contains(&"cmp:hub".to_string()),
        "a service every path routes through is genuinely fragile"
    );

    // The same topology, stated as a library: linked into its consumers, so
    // it cannot fail on its own and redundancy means nothing.
    let linked = flagged(&build("library"));
    assert!(
        !linked.contains(&"cmp:hub".to_string()),
        "you cannot run two copies of a library: {linked:?}"
    );
}

#[test]
fn a_release_and_its_environment_are_not_an_island() {
    // Found modelling v0.4.0: DEPLOYED_TO joined the Release to its
    // Environment and INCLUDES joined it to nothing, so every release was a
    // 2-node disconnected community by construction. INCLUDES is traceability
    // (the as-released packaging of the artifact), and the pair must join the
    // design network through it.
    let mut g = linear_thread();
    g.create_node(node::ARTIFACT, "art:a", Props::new().set("name", "a.rs"))
        .unwrap();
    g.create_edge(
        edge::REALIZES,
        node::ARTIFACT,
        "art:a",
        node::CAPABILITY,
        "cap:a",
        Props::new(),
    )
    .unwrap();
    g.create_node(node::RELEASE, "rel:v1", Props::new().set("name", "v1"))
        .unwrap();
    g.create_node(
        node::ENVIRONMENT,
        "env:dev",
        Props::new().set("name", "dev"),
    )
    .unwrap();
    g.create_edge(
        edge::INCLUDES,
        node::RELEASE,
        "rel:v1",
        node::ARTIFACT,
        "art:a",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::DEPLOYED_TO,
        node::RELEASE,
        "rel:v1",
        node::ENVIRONMENT,
        "env:dev",
        Props::new(),
    )
    .unwrap();

    assert!(
        !has(&g, HealCategory::DisconnectedCommunity),
        "a release shipping a design artifact is part of the design network"
    );
}
