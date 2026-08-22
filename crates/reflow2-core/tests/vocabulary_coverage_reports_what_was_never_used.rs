//! Which of the design vocabulary a design has never used — figures always,
//! list on request, grouped by domain, and parkable.
//!
//! `dec:idea-how-does-a-users-project-acquire-vocabulary-it-never-uses`, option
//! (b), in the shape a two-arm trial chose for it.
//!
//! # The trial these probes protect
//!
//! Run on reflow2 itself before any of this was built, in the owner's framing —
//! *"a doctor who develops a new medicine, but tries it on himself first"*:
//!
//! | arm | node types | edge types | flat list |
//! |-----|-----------|-----------|-----------|
//! | mature (2535 nodes) | 22/29 | 37/61 | 59 items |
//! | day one (post-genesis, 2 nodes) | 2/29 | 0/61 | **97 items** |
//!
//! The FIGURES passed both arms. The FLAT LIST failed the second outright —
//! longer for the user least able to act on it. Each probe below pins one of
//! the four decisions that came out of that, so a later session cannot quietly
//! undo one without a red test.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, vocabulary_park_decision_id};

/// A design with a little of the golden thread and none of the flow,
/// dimensions or readiness vocabulary — the ordinary shape of a real project.
fn partial_design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component("cmp:one", "One", "does a thing", None)
        .unwrap();
    g.add_capability("cap:one", "One", "a capability", None)
        .unwrap();
    g.allocate("cap:one", "cmp:one").unwrap();
    g.add_requirement("req:one", "One", "must hold").unwrap();
    g.satisfies("cap:one", "req:one").unwrap();
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:p",
        node::COMPONENT,
        "cmp:one",
        Props::new(),
    )
    .unwrap();
    // Enough nodes that the "barely started" note stands down.
    for i in 0..8 {
        g.add_requirement(&format!("req:filler{i}"), "filler", "x")
            .unwrap();
    }
    g
}

#[test]
fn the_figures_are_always_there_and_the_flat_list_never_is_unless_asked() {
    // THE DECISION THIS PINS: ship the figures, keep the flat list behind a
    // request. Measured, the list is 97 items on a day-one design and 59 on a
    // mature one — longest exactly where it is least usable.
    let g = partial_design();

    let quiet = g.vocabulary_coverage(false).unwrap();
    assert!(
        quiet.unused.is_none(),
        "the flat list is withheld by default"
    );
    assert!(quiet.node_types.total > 0 && quiet.edge_types.total > 0);
    assert!(
        quiet.edge_types.used < quiet.edge_types.total,
        "this fixture uses only a little of the vocabulary, so there is something to report"
    );

    let asked = g.vocabulary_coverage(true).unwrap();
    let list = asked.unused.expect("asked for, so returned");
    assert!(
        !list.is_empty(),
        "a design using a fraction of the vocabulary has unused vocabulary to name"
    );
    assert_eq!(
        quiet.edge_types.used, asked.edge_types.used,
        "asking for the list must not change the figures"
    );
}

#[test]
fn the_share_is_computed_so_the_reader_is_not_left_dividing() {
    let g = partial_design();
    let c = g.vocabulary_coverage(false).unwrap();
    let mut every: Vec<&reflow2_core::Coverage> = vec![
        &c.node_types,
        &c.edge_types,
        &c.properties_on_used_types,
        &c.properties_on_used_edge_types,
    ];
    every.extend(c.domains.iter().map(|d| &d.properties));
    for cov in every {
        let expected = if cov.total == 0 {
            0.0
        } else {
            cov.used as f64 / cov.total as f64
        };
        assert!(
            (cov.share - expected).abs() < 1e-9,
            "share must equal used/total: {} vs {expected}",
            cov.share
        );
        assert!(cov.used <= cov.total, "used can never exceed total");
    }
}

#[test]
fn unused_vocabulary_is_grouped_by_the_schemas_own_domains() {
    // THE DECISION THIS PINS: group, do not list. The trial found the unused
    // vocabulary clustering into whole subsystems, and the clusters turned out
    // to BE the schema's domains — so the axis is the design's own, never one
    // this code invented.
    let g = partial_design();
    let c = g.vocabulary_coverage(false).unwrap();

    assert!(
        c.domains.len() >= 10,
        "all eleven schema domains are reported, not only the empty ones — a \
         domain missing from the list would read as fully used: {}",
        c.domains.len()
    );
    for d in &c.domains {
        assert!(!d.domain.is_empty());
        assert!(
            d.node_types.total + d.edge_types.total > 0,
            "{} declares nothing",
            d.domain
        );
    }
    // Worst-covered first: the shape of the gap is visible without reading names.
    let scores: Vec<f64> = c
        .domains
        .iter()
        .map(|d| d.node_types.share + d.edge_types.share)
        .collect();
    assert!(
        scores.windows(2).all(|w| w[0] <= w[1] + 1e-9),
        "domains are ordered worst-covered first: {scores:?}"
    );
}

#[test]
fn a_domain_can_be_ruled_deliberately_unused_and_the_remedy_rides_on_the_finding() {
    // THE DECISION THIS PINS: make it parkable. `OWNED_BY` turned up in the
    // trial's unused list and its absence is DELIBERATE
    // (`dec:ownership-reads-claims-before-adding-an-edge`), so a report that
    // could not be told "this one is on purpose" would report a settled
    // decision as a hole forever — `req:a-deliberate-state-is-not-a-defect`.
    let mut g = partial_design();
    let target = g.vocabulary_coverage(false).unwrap().domains[0]
        .domain
        .clone();
    let park_id = vocabulary_park_decision_id(&target);

    let before = g.vocabulary_coverage(false).unwrap();
    assert_eq!(before.parked_domains, 0);
    assert!(
        before.domains.iter().all(|d| !d.parked),
        "nothing is parked until somebody rules it so"
    );
    assert!(
        before.domains.iter().any(|d| d.park_with == park_id),
        "the remedy is stated on the finding itself, whether or not it has been used — \
         a convention that rides only on a skill is one most sessions never load"
    );

    g.add_decision(
        &park_id,
        "The flow vocabulary is deliberately unused here",
        "This project models no processes.",
        Some("Nothing in this design is a workflow, so the flow types would be empty by choice."),
    )
    .unwrap();

    // A PROPOSED ruling must NOT park anything — somebody thinking out loud is
    // not a decision, the same line `ruling: parks` draws.
    let musing = g.vocabulary_coverage(false).unwrap();
    assert_eq!(
        musing.parked_domains, 0,
        "a proposed Decision is somebody thinking out loud and must not silence a finding"
    );

    g.set_decision_status(&park_id, "accepted").unwrap();
    let after = g.vocabulary_coverage(false).unwrap();
    assert_eq!(
        after.parked_domains, 1,
        "an ACCEPTED ruling parks exactly one domain"
    );
    let parked = after.domains.iter().find(|d| d.domain == target).unwrap();
    assert!(parked.parked);
    assert!(
        parked.parked_because.is_some(),
        "and carries WHY, so a later session reads the reason instead of re-litigating"
    );
    assert_eq!(
        after.edge_types.used, before.edge_types.used,
        "parking is a disposition, not a deletion — the figures do not move"
    );
}

#[test]
fn a_design_that_has_barely_started_is_told_so_instead_of_shown_a_wall_of_red() {
    // THE DECISION THIS PINS, and it is the one the trial's second arm forced:
    // straight out of genesis a design holds 2 nodes and every figure is near
    // zero. That is not vocabulary going unused, it is a design that has not
    // begun — and reporting the first as the second is a wall of red on a new
    // user's first read.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:new", "Brand new").unwrap();

    let fresh = g.vocabulary_coverage(false).unwrap();
    let note = fresh
        .note
        .expect("a barely-started design must say so in words");
    assert!(
        note.contains("BARELY STARTED"),
        "the note names the situation rather than describing the numbers: {note}"
    );

    // And it stands down once there is a design to measure.
    assert!(
        partial_design()
            .vocabulary_coverage(false)
            .unwrap()
            .note
            .is_none(),
        "the note's PRESENCE is the signal, so it must disappear on a real design"
    );
}

// ---------------------------------------------------------------------------
// NAMING THE PROPERTIES, not only counting them.
//
// The four probes above pin the two-arm trial. These pin the hole the trial's
// shipped tool left behind and the rule that closing it had to obey: a set of
// named things reduced to a scalar is not a check of the set, and the fix must
// not undo the withholding decision by making the default reply longer.
// ---------------------------------------------------------------------------

/// The fixture, plus one Artifact that declares no `audience` — the case that
/// found this hole. `Artifact` HAS instances, so a zero on its properties is
/// evidence rather than a vacuous zero.
fn design_with_an_unclassified_artifact() -> DesignGraph {
    let mut g = partial_design();
    g.add_artifact("art:thing", "thing", Some("code"), Some("src/thing.rs"))
        .unwrap();
    g
}

#[test]
fn an_unused_property_is_named_and_not_merely_counted() {
    // THE HOLE THIS CLOSES. `properties_on_used_types` said "197 of 215" and
    // nothing anywhere could say WHICH 18 — so a design that had declared no
    // `Artifact.audience` was silent in every report reflow2 served, and the
    // only instrument that could name it was a hand-written script pointed at
    // reflow2's own files, which generalised to nobody.
    let g = design_with_an_unclassified_artifact();

    let list = g.vocabulary_coverage(true).unwrap().unused.unwrap();
    assert!(
        list.contains(&"build: node property Artifact.audience".to_string()),
        "an undeclared audience must be NAMED, not left inside a fraction: {list:?}"
    );
    assert!(
        list.iter()
            .any(|u| u == "core: edge property SATISFIES.coverage"),
        "the edge half counts too — an edge type with edges whose property nobody sets: {list:?}"
    );

    // AND THE WITHHOLDING DECISION SURVIVES. Extending the list rather than
    // adding a report is the whole reason this fix is allowed to exist: the
    // default reply must not have grown a wall of names.
    assert!(
        g.vocabulary_coverage(false).unwrap().unused.is_none(),
        "properties ride the list that was already withheld, never the default reply"
    );
}

#[test]
fn a_property_is_named_only_on_a_type_that_has_instances() {
    // THE VACUOUS-ZERO RULE, applied to the NAMES as strictly as it was already
    // applied to the figures. A design that has never created an
    // `EnvironmentRule` says nothing about whether `EnvironmentRule.authority`
    // is writable — and naming every property of every empty type is exactly
    // the day-one wall of text the trial killed the flat list to avoid.
    let g = design_with_an_unclassified_artifact();
    let c = g.vocabulary_coverage(true).unwrap();
    let list = c.unused.unwrap();

    assert!(
        list.contains(&"environment: node type EnvironmentRule".to_string()),
        "the empty TYPE is still named — that is the finding: {list:?}"
    );
    assert!(
        !list.iter().any(|u| u.contains("property EnvironmentRule.")),
        "but not one of its properties, because a zero on an empty type is vacuous: {list:?}"
    );
    assert!(
        !list.iter().any(|u| u.contains("edge property DECOMPOSES.")),
        "same rule one level over: DECOMPOSES has no edges, so its properties say nothing"
    );
}

#[test]
fn a_property_set_on_a_single_instance_stops_being_named() {
    // "Used" means at least one instance carries it — the same reading the
    // figures have always had. One requirement out of nine answers for the type.
    let mut g = design_with_an_unclassified_artifact();
    let named = |g: &DesignGraph, p: &str| {
        g.vocabulary_coverage(true)
            .unwrap()
            .unused
            .unwrap()
            .contains(&format!("core: node property Requirement.{p}"))
    };

    assert!(
        named(&g, "designation"),
        "nobody has designated anything yet"
    );
    assert!(named(&g, "rationale"), "and nobody has given a rationale");

    g.set_requirement_designation("req:one", "published")
        .unwrap();

    assert!(
        !named(&g, "designation"),
        "one instance carrying it makes the vocabulary reached"
    );
    assert!(
        named(&g, "rationale"),
        "and it must not take the rest of the type down with it"
    );
}

#[test]
fn a_defaulted_property_can_never_be_named_and_that_limit_is_real() {
    // 🛑 THE HONEST LIMIT, pinned so a later reader does not take this list for
    // more than it is. The store MATERIALISES schema defaults on write, so
    // every instance carries a defaulted property and it reads as used whether
    // or not anybody chose the value. What this list names is the UNDEFAULTED
    // optional — which is the class that matters, because
    // `req:defaults-do-not-assert` is why the fields worth asking about have no
    // default in the first place.
    let g = design_with_an_unclassified_artifact();
    let list = g.vocabulary_coverage(true).unwrap().unused.unwrap();

    assert!(
        !list.iter().any(|u| u.ends_with("Requirement.priority")),
        "`priority` defaults to `medium`, so it is stored on every requirement and cannot be \
         reported unused — a reading of 'nobody varied from the default' is a different \
         question and one the store cannot tell from a deliberate choice: {list:?}"
    );
    assert!(
        list.iter().any(|u| u.ends_with("Requirement.rationale")),
        "while the undefaulted optional beside it is named"
    );
}

#[test]
fn the_per_domain_property_figures_account_for_both_totals() {
    // The domains partition the whole schema — `domain_membership` refuses to
    // answer unless they do — so the per-domain figures must sum to the two
    // top-level ones. They are accumulated from the same walk rather than
    // recomputed, which makes this an assertion about the invariant holding
    // rather than about two computations happening to agree.
    let g = design_with_an_unclassified_artifact();
    let c = g.vocabulary_coverage(false).unwrap();

    let used: usize = c.domains.iter().map(|d| d.properties.used).sum();
    let total: usize = c.domains.iter().map(|d| d.properties.total).sum();
    assert_eq!(
        used,
        c.properties_on_used_types.used + c.properties_on_used_edge_types.used
    );
    assert_eq!(
        total,
        c.properties_on_used_types.total + c.properties_on_used_edge_types.total
    );

    // And the depth figure is REPORTED BY DEFAULT, because without it a domain
    // reads fully covered on types while every optional field on those types
    // goes unfilled — which is the state `Artifact.audience` was in.
    let build = c.domains.iter().find(|d| d.domain == "build").unwrap();
    assert!(
        build.properties.total > 0 && build.properties.share < 1.0,
        "the build domain has an artifact with fields nobody filled: {:?}",
        build.properties
    );
}

#[test]
fn property_names_come_out_sorted_within_their_type() {
    // The schema holds properties in a `HashMap`, so naming them put this
    // module's byte-identical-output promise at the mercy of hash order.
    //
    // ⚠️ COMPARING TWO CALLS WOULD NOT CATCH THAT, and the first version of
    // this probe did exactly that. The schema is a process-wide `OnceLock`, so
    // both calls walk the SAME map with the SAME hash seed and agree whether or
    // not anything is sorted — an assertion that can only pass. The order has
    // to be asserted against something outside the map, so this checks the
    // property it actually promises.
    let g = design_with_an_unclassified_artifact();
    let list = g.vocabulary_coverage(true).unwrap().unused.unwrap();

    let key = |s: &str| -> Option<(String, String, String)> {
        let (head, qualified) = s
            .split_once(": node property ")
            .or_else(|| s.split_once(": edge property "))?;
        let (ty, prop) = qualified.split_once('.')?;
        Some((head.to_string(), ty.to_string(), prop.to_string()))
    };
    let props: Vec<(String, String, String)> = list.iter().filter_map(|s| key(s)).collect();
    assert!(
        props.len() > 5,
        "the fixture must produce enough property names for this to mean anything: {}",
        props.len()
    );
    for w in props.windows(2) {
        if w[0].0 == w[1].0 && w[0].1 == w[1].1 {
            assert!(
                w[0].2 < w[1].2,
                "{}.{} then {} is out of order — an unsorted walk of the schema's HashMap \
                 makes this module's output differ between processes",
                w[0].1,
                w[0].2,
                w[1].2
            );
        }
    }
}
