//! An acknowledgement is a statement ABOUT the design, not a part of it (BL-124).
//!
//! `acknowledge_defect` writes a Decision and wires `GOVERNED_BY` from every
//! affected node to it, deliberately, so the review is reachable from the
//! design. `unthreaded_cluster` computes its id by hashing the category with
//! the affected set. For most categories those two behaviours are independent.
//! For this one the acknowledgement modifies exactly what the detector measures
//! — cluster membership — so the review joins the island it acknowledges,
//! enlarges it by one, mints a new id nobody has accepted, and the defect comes
//! back one node larger. Acknowledging again grows it again, without limit.
//!
//! That makes an entire category permanently unclosable, which is precisely the
//! failure `acknowledge_defect` exists to prevent: *"a list that can never reach
//! zero gets skimmed."* Reproduced in the field over four sessions of a real
//! project, growing 8 → 9 → 10 nodes.
//!
//! The fix excludes acknowledgement records from the **design network** rather
//! than from the defect id, because `design_network()` has three consumers —
//! `unthreaded_cluster`, betweenness centrality and
//! `surprising_connections` — and keying the id differently would fix the
//! visible symptom while leaving the other two counting review bookkeeping as
//! design structure. Measured on reflow2's own graph before the change: **four
//! of the eight most central nodes were acknowledgement records.**
//!
//! The counterweights are what keep the fix honest. Excluding *all* Decisions
//! would pass the livelock case and be badly wrong — a Decision that governs
//! real work is structure. And a cluster that is genuinely isolated must still
//! fire, or this "fix" is just a way of silencing a detector.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

/// Two capabilities wired to each other and to nothing else, beside a larger
/// connected body. The pair islands, so `unthreaded_cluster` fires on it.
fn island_beside_a_body() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();

    // The main body: a thread with enough nodes to be the largest component.
    for (id, name) in [("req:main", "Main"), ("req:second", "Second")] {
        g.add_requirement(id, name, "a requirement").unwrap();
    }
    for (id, name) in [("cap:main", "Main cap"), ("cap:other", "Other cap")] {
        g.add_capability(id, name, "a capability", None).unwrap();
    }
    g.add_component("cmp:main", "Main part", "a part", None)
        .unwrap();
    g.satisfies("cap:main", "req:main").unwrap();
    g.satisfies("cap:other", "req:second").unwrap();
    g.allocate("cap:main", "cmp:main").unwrap();
    g.allocate("cap:other", "cmp:main").unwrap();

    // The island: two capabilities depending on each other and nothing else.
    for (id, name) in [("cap:island-a", "Island A"), ("cap:island-b", "Island B")] {
        g.add_capability(id, name, "a capability", None).unwrap();
    }
    depends(&mut g, "cap:island-a", "cap:island-b");
    g
}

fn depends(g: &mut DesignGraph, from: &str, to: &str) {
    g.create_edge(
        "DEPENDS_ON",
        node::CAPABILITY,
        from,
        node::CAPABILITY,
        to,
        Props::new().build(),
    )
    .unwrap();
}

fn islands(g: &DesignGraph) -> Vec<reflow2_core::HealIssue> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .filter(|i| i.category.as_str() == "unthreaded_cluster")
        .collect()
}

/// THE LIVELOCK. Acknowledging the island must actually close it, and it must
/// STAY closed on the next detect — before the fix, the acknowledgement joined
/// the cluster and a new, larger, unaccepted defect took its place.
#[test]
fn acknowledging_an_island_closes_it_and_it_stays_closed() {
    let mut g = island_beside_a_body();

    let found = islands(&g);
    assert_eq!(found.len(), 1, "fixture must produce exactly one island");
    let defect = found[0].clone();
    let affected_before = defect.affected_ids.len();

    g.acknowledge_defect(
        &defect.id,
        &defect.affected_ids,
        "operational security is orthogonal to the functional pipeline",
    )
    .unwrap();

    let after = islands(&g);
    assert!(
        after.is_empty(),
        "the acknowledged island came back: {:?}",
        after
            .iter()
            .map(|i| (&i.id, i.affected_ids.len()))
            .collect::<Vec<_>>()
    );

    // And again — the growth was unbounded, so one re-detect is not enough.
    let again = islands(&g);
    assert!(
        again.is_empty(),
        "the island returned on a second detect, which is the livelock: {:?}",
        again
            .iter()
            .map(|i| (&i.id, i.affected_ids.len()))
            .collect::<Vec<_>>()
    );

    // The acknowledgement must not have changed what the detector measures.
    assert_eq!(
        affected_before, 2,
        "the island is the two island capabilities, and nothing else"
    );
}

/// COUNTERWEIGHT. Excluding every Decision would pass the test above and be
/// badly wrong: a Decision that governs real design work is structure, and
/// removing it would island the things it governs.
#[test]
fn an_ordinary_decision_still_counts_as_structure() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:main", "Main", "a requirement")
        .unwrap();
    g.add_capability("cap:main", "Main cap", "a capability", None)
        .unwrap();
    g.add_component("cmp:main", "Main part", "a part", None)
        .unwrap();
    g.satisfies("cap:main", "req:main").unwrap();
    g.allocate("cap:main", "cmp:main").unwrap();

    // A second cluster joined to the body ONLY through an ordinary Decision.
    g.add_capability("cap:far-a", "Far A", "a capability", None)
        .unwrap();
    g.add_capability("cap:far-b", "Far B", "a capability", None)
        .unwrap();
    depends(&mut g, "cap:far-a", "cap:far-b");
    g.add_decision(
        "dec:real",
        "A real decision",
        "we chose this",
        Some("because"),
    )
    .unwrap();
    g.governed_by(
        node::CAPABILITY,
        "cap:far-a",
        node::DECISION,
        "dec:real",
        None,
    )
    .unwrap();
    g.governed_by(
        node::CAPABILITY,
        "cap:main",
        node::DECISION,
        "dec:real",
        None,
    )
    .unwrap();

    assert!(
        islands(&g).is_empty(),
        "an ordinary Decision joins what it governs; dropping it would island them"
    );
}

/// COUNTERWEIGHT. A genuinely isolated cluster must still fire — otherwise this
/// change is not a fix, it is a way of turning a detector off.
#[test]
fn a_genuinely_isolated_cluster_still_fires() {
    let g = island_beside_a_body();
    let found = islands(&g);
    assert_eq!(found.len(), 1, "the detector must still find a real island");
    assert_eq!(found[0].affected_ids.len(), 2);
}

/// Withdrawal must still reopen the defect. The review layer's existing bargain
/// — acceptance is reversible and supersedes rather than deletes — has to
/// survive a change to what the network counts.
#[test]
fn withdrawing_the_acknowledgement_reopens_the_island() {
    let mut g = island_beside_a_body();
    let defect = islands(&g)[0].clone();

    g.acknowledge_defect(&defect.id, &defect.affected_ids, "accepted for now")
        .unwrap();
    assert!(islands(&g).is_empty(), "acknowledged");

    assert!(
        g.withdraw_defect_acknowledgement(&defect.id).unwrap(),
        "withdrawal reports that it acted"
    );
    let reopened = islands(&g);
    assert_eq!(
        reopened.len(),
        1,
        "withdrawing the review must put the island back on the list"
    );
    assert_eq!(
        reopened[0].id, defect.id,
        "and it must be the SAME defect, not a differently-shaped one"
    );
}

/// The acknowledgement is still reachable from the design — the property the
/// `GOVERNED_BY` wiring exists for. Excluding it from the *network* must not
/// mean deleting it or orphaning it in the *graph*.
#[test]
fn the_acknowledgement_is_still_recorded_and_still_linked() {
    let mut g = island_beside_a_body();
    let defect = islands(&g)[0].clone();
    let decision_id = g
        .acknowledge_defect(&defect.id, &defect.affected_ids, "a reason worth keeping")
        .unwrap();

    let node = g
        .get_node(node::DECISION, &decision_id)
        .unwrap()
        .expect("the review Decision survives");
    assert_eq!(
        node.properties.get("rationale").and_then(|v| v.as_str()),
        Some("a reason worth keeping"),
        "the reason is the point of recording it"
    );
    assert!(
        g.incoming(&decision_id, Some("GOVERNED_BY"))
            .unwrap()
            .iter()
            .any(|e| e.from_id == "cap:island-a"),
        "the review stays reachable from what it acknowledges"
    );
}
