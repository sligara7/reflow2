//! A session holding nothing can ask what parts the design has, and get seeds.
//!
//! `req:a-session-with-no-seed-can-still-orient`, the last of
//! `epoch:instruments-stop-overstating`.
//!
//! # The situation, in the reporter's words
//!
//! dev_storyflow's fleet, 2026-08-08: *"the moment with the most time available
//! — sitting AVAILABLE, nothing to do — is the moment the design brain is LEAST
//! USABLE, and the moment I am busiest (mid-lane) is when I am told to
//! orient."* Every scoped read wants a seed; a worker at pool check-in has no
//! lane and therefore no seed.
//!
//! # What these probes hold to
//!
//! The listing is only worth anything if the seeds it hands back WORK — so the
//! central probe does not check a shape, it takes a row and scopes to it, and
//! insists the region it lands in is the one the row measured. A directory of
//! addresses nobody can walk to is the failure mode here.
//!
//! The other four are the honesty properties, each one a way this reply could
//! quietly overstate: an empty answer that reads as a clean map, a list of
//! overlapping regions read as a partition, a coverage figure that ignores the
//! bookkeeping it never reached, and a "nobody is here" that is really "nobody
//! told me".

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DEFAULT_REGION_DEPTH, DesignGraph};

/// Two components under a project, one of them holding a gap the other cannot
/// see — the smallest graph on which "which part should I take?" has an answer.
fn two_parts() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_component(
        "cmp:ground",
        "Ground segment",
        "Receives and stores passes",
        None,
    )
    .unwrap();
    g.add_component("cmp:space", "Space segment", "Flies and transmits", None)
        .unwrap();
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:p",
        node::COMPONENT,
        "cmp:ground",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::CONTAINS,
        node::PROJECT,
        "proj:p",
        node::COMPONENT,
        "cmp:space",
        Props::new(),
    )
    .unwrap();
    // One capability allocated to the ground segment, satisfying nothing —
    // an unsatisfied-requirement-shaped hole that belongs to ONE part.
    g.add_capability("cap:downlink", "Downlink", "Receives the pass", None)
        .unwrap();
    g.allocate("cap:downlink", "cmp:ground").unwrap();
    g.add_requirement("req:pass", "Every pass is received", "No pass is dropped.")
        .unwrap();
    g.satisfies("cap:downlink", "req:pass").unwrap();
    g
}

#[test]
fn the_seed_a_region_hands_back_is_one_the_scoped_reads_accept() {
    // THE PROBE THAT MATTERS. The whole requirement is "a session can FIND its
    // seed rather than needing one to start", so a listing whose seeds do not
    // then work is a directory of addresses nobody can walk to.
    let g = two_parts();
    let listing = g.design_regions(DEFAULT_REGION_DEPTH).unwrap();
    assert!(
        !listing.regions.is_empty(),
        "a design with parts lists them"
    );

    for region in &listing.regions {
        let scoped = g
            .detect_gaps_in_scope(&region.seed_id, listing.depth)
            .unwrap_or_else(|e| panic!("seed {} was refused as a scope: {e}", region.seed_id));
        assert_eq!(
            scoped.region_size, region.region_size,
            "the listing sized {} at {} but scoping to it lands in {} nodes — a row that \
             measures a different region than the one you reach by using it is worse than no row",
            region.seed_id, region.region_size, scoped.region_size,
        );
        assert_eq!(
            scoped.in_scope, region.open_gaps,
            "the listing promised {} gaps in {} and scoping to it found {}",
            region.open_gaps, region.seed_id, scoped.in_scope,
        );
    }
}

#[test]
fn a_design_with_no_parts_says_so_instead_of_answering_empty() {
    // An empty list is the single most dangerous reply this tool can give: it
    // is indistinguishable from "your design has no problems anywhere" unless
    // it says which empty it is. Same rule as the vacuity note on a scoped
    // read, and as `swept` on a defect sweep.
    let g = DesignGraph::open_in_memory().unwrap();
    let listing = g.design_regions(DEFAULT_REGION_DEPTH).unwrap();

    assert!(listing.regions.is_empty());
    let note = listing
        .note
        .expect("a listing with no regions must say WHY in words");
    assert!(
        note.contains("VACUOUS"),
        "the note must name the emptiness rather than describe the design politely: {note}"
    );
    assert!(
        note.contains("Project") && note.contains("Component"),
        "and must name what would have produced a region, or it is a no with no remedy: {note}"
    );

    // The complement: once a part exists, the note goes away rather than
    // hanging around as decoration.
    let g = two_parts();
    assert!(
        g.design_regions(DEFAULT_REGION_DEPTH)
            .unwrap()
            .note
            .is_none(),
        "a real listing carries no vacuity note — its presence is the signal"
    );
}

#[test]
fn the_listing_says_what_no_region_reaches() {
    // A region list read as a partition is a false map. On any real design most
    // nodes are bookkeeping no Component contains, so the reply has to hand the
    // reader that number rather than let them assume the rows are the design.
    let mut g = two_parts();
    // A ChangeEvent belongs to no component — exactly the shape that dominates
    // a mature graph.
    g.create_node(
        node::CHANGE_EVENT,
        "chg:unrelated",
        Props::new()
            .set("name", "something happened")
            .set("change_type", "new_feature"),
    )
    .unwrap();

    let listing = g.design_regions(DEFAULT_REGION_DEPTH).unwrap();
    let c = &listing.coverage;

    assert_eq!(
        c.nodes,
        c.in_some_region + c.in_no_region,
        "every node lands in exactly one of covered/uncovered, or the coverage block \
         is arithmetic nobody can check"
    );
    assert!(
        c.in_no_region > 0,
        "the ChangeEvent is in no region and the reply must admit it"
    );
    assert_eq!(
        c.uncovered_by_type.get(node::CHANGE_EVENT).copied(),
        Some(1),
        "and must say WHICH KIND of thing it did not reach, so a big uncovered count \
         can be recognised as bookkeeping instead of feared as lost design: {:?}",
        c.uncovered_by_type,
    );
    assert_eq!(
        c.uncovered_by_type.values().sum::<usize>(),
        c.in_no_region,
        "the breakdown must account for every uncovered node"
    );
}

#[test]
fn overlapping_regions_are_counted_rather_than_presented_as_a_partition() {
    // Regions overlap by construction — a shared Capability sits in the radius
    // of everything that reaches it. Reporting rows without the overlap lets a
    // chooser believe they picked a distinct area when they picked a slice of
    // the same one, which is `detect_gaps` at depth 3 all over again.
    let mut g = two_parts();
    // One capability allocated to BOTH segments: genuinely shared ground.
    g.allocate("cap:downlink", "cmp:space").unwrap();

    let listing = g.design_regions(DEFAULT_REGION_DEPTH).unwrap();
    assert!(
        listing.coverage.in_more_than_one > 0,
        "a node reachable from two parts must be counted as shared, not silently \
         attributed to whichever region was walked first"
    );
    assert!(
        listing.coverage.in_more_than_one <= listing.coverage.in_some_region,
        "shared nodes are a subset of covered ones"
    );

    // And the order is stated, so an ordered list is not mistaken for a ranking
    // of importance.
    assert!(
        listing.order.contains("NOT a ranking of importance"),
        "the sort order has to disclaim what it is not: {}",
        listing.order
    );
}

#[test]
fn a_region_says_who_is_already_working_in_it() {
    // "Nobody is here" and "nobody told me" are different facts, and a chooser
    // walking into somebody else's lane is the collision claims exist to
    // reduce. The listing is the moment that information is worth anything —
    // afterwards they have already picked.
    let mut g = two_parts();
    assert!(
        g.design_regions(DEFAULT_REGION_DEPTH)
            .unwrap()
            .regions
            .iter()
            .all(|r| r.held_by.is_empty()),
        "an unclaimed design shows nobody holding anything"
    );

    g.add_contributor("who:a", "A", None, None, None).unwrap();
    g.claim_region(
        "who:a",
        "cmp:ground",
        1,
        Some("mid-lane"),
        None,
        Some("2026-08-17T00:00:00Z"),
    )
    .unwrap();

    let listing = g.design_regions(DEFAULT_REGION_DEPTH).unwrap();
    let ground = listing
        .regions
        .iter()
        .find(|r| r.seed_id == "cmp:ground")
        .expect("the claimed part is still listed — a claim is advisory, never a filter");
    assert_eq!(
        ground.held_by,
        vec!["who:a".to_string()],
        "the claim on this part reaches the chooser"
    );
    let space = listing
        .regions
        .iter()
        .find(|r| r.seed_id == "cmp:space")
        .unwrap();
    assert!(
        space.held_by.is_empty(),
        "and does not bleed onto the part nobody claimed"
    );
}
