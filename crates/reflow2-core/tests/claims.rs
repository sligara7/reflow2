//! Advisory claims over regions of the design (BL-44).
//!
//! The scoping primitive `dec:multi-writer-architecture` commits to. The
//! decision keeps the design as a file in each checkout with no shared server,
//! so a claim CANNOT be a lock — and the tests that matter here are the ones
//! proving it isn't. A suite that only demonstrated successful claiming would
//! read as a locking mechanism to anyone skimming it, which is the false promise
//! `dec:report-dont-judge` forbids: worse than no claims layer at all.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// Two people, and a design with two loosely-connected areas.
fn team() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_contributor("who:ann", "Ann", Some("person"), Some("@ann"), None)
        .unwrap();
    g.add_contributor("who:bob", "Bob", Some("person"), Some("@bob"), None)
        .unwrap();

    g.add_requirement("req:auth", "Auth", "Users sign in.")
        .unwrap();
    g.add_requirement("req:report", "Reports", "Users get reports.")
        .unwrap();
    for (cap, name, req) in [
        ("cap:login", "Login", "req:auth"),
        ("cap:logout", "Logout", "req:auth"),
        ("cap:export", "Export", "req:report"),
    ] {
        g.add_capability(cap, name, "does it", None).unwrap();
        g.satisfies(cap, req).unwrap();
    }
    g
}

#[test]
fn a_claim_records_who_holds_what_and_why() {
    let mut g = team();
    let c = g
        .claim_region(
            "who:ann",
            "req:auth",
            2,
            Some("adding MFA"),
            Some("2026-07-25"),
        )
        .unwrap();
    assert_eq!(c.contributor_id, "who:ann");
    assert_eq!(c.depth, 2);
    assert_eq!(c.note.as_deref(), Some("adding MFA"));

    let all = g.claims().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].seed_id, "req:auth");
}

#[test]
fn a_claim_does_not_block_anyone() {
    // THE test. The architecture forbids locks — there is no server to hold one
    // — so a second writer must be able to work the claimed region freely. If
    // this ever fails, the layer has become something the design does not
    // support and cannot honour.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None)
        .unwrap();

    // Bob edits squarely inside Ann's region. Nothing refuses him.
    g.add_capability("cap:mfa", "MFA", "second factor", None)
        .unwrap();
    g.satisfies("cap:mfa", "req:auth").unwrap();
    g.set_requirement_status("req:auth", "accepted").unwrap();

    // And the write landed, exactly as if no claim existed.
    let r = g.get_node(node::REQUIREMENT, "req:auth").unwrap().unwrap();
    assert_eq!(
        r.properties.get("status").and_then(|v| v.as_str()),
        Some("accepted")
    );
    assert!(g.get_node(node::CAPABILITY, "cap:mfa").unwrap().is_some());
}

#[test]
fn claiming_the_same_ground_is_allowed_and_reported_not_refused() {
    // Two people, one area. Both claims succeed — the overlap is information,
    // not an error, because sometimes two people genuinely must work the same
    // ground and a tool that refused would simply be routed around.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, Some("MFA"), None)
        .unwrap();
    g.claim_region("who:bob", "cap:login", 1, Some("rate limiting"), None)
        .unwrap();

    let report = g.claim_report().unwrap();
    assert_eq!(report.claims.len(), 2, "both claims stand");
    assert_eq!(report.overlaps.len(), 1, "and the collision is surfaced");

    let o = &report.overlaps[0];
    assert!(
        o.shared.contains(&"cap:login".to_string()),
        "the shared ground is named, not just counted: {:?}",
        o.shared
    );
    assert!(
        report.advisory.contains("never block"),
        "the payload itself must say an overlap is a warning, since whoever \
         reads it over the wire never sees these docs"
    );
}

#[test]
fn regions_that_do_not_touch_do_not_collide() {
    let mut g = team();
    g.claim_region("who:ann", "cap:logout", 0, None, None)
        .unwrap();
    g.claim_region("who:bob", "cap:export", 0, None, None)
        .unwrap();
    let report = g.claim_report().unwrap();
    assert_eq!(report.claims.len(), 2);
    assert!(
        report.overlaps.is_empty(),
        "separate ground is the normal case and must stay quiet: {:?}",
        report.overlaps
    );
}

#[test]
fn one_person_holding_two_overlapping_regions_is_not_a_collision() {
    // One person working two connected areas is just one person working.
    // Reporting it would train people to ignore the overlap list, which is how
    // a signal dies.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None)
        .unwrap();
    g.claim_region("who:ann", "cap:login", 2, None, None)
        .unwrap();
    let report = g.claim_report().unwrap();
    assert_eq!(report.claims.len(), 2);
    assert!(report.overlaps.is_empty());
}

#[test]
fn the_region_follows_the_design_instead_of_freezing() {
    // A claim stores a seed and a depth, never a node list. When the design
    // grows inside the claimed area, the claim covers the new work — a frozen
    // membership list would silently stop covering it the moment someone added
    // an edge, and nobody would notice.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None)
        .unwrap();
    let before = g.claimed_region(&g.claims().unwrap()[0]).unwrap();
    assert!(!before.contains("cap:mfa"));

    g.add_capability("cap:mfa", "MFA", "second factor", None)
        .unwrap();
    g.satisfies("cap:mfa", "req:auth").unwrap();

    let after = g.claimed_region(&g.claims().unwrap()[0]).unwrap();
    assert!(
        after.contains("cap:mfa"),
        "the region is computed, so new work inside it is covered: {after:?}"
    );
}

#[test]
fn releasing_a_claim_lets_the_ground_go() {
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None)
        .unwrap();
    assert!(g.release_claim("who:ann", "req:auth").unwrap());
    assert!(g.claims().unwrap().is_empty());
    assert!(
        !g.release_claim("who:ann", "req:auth").unwrap(),
        "releasing what nobody holds says so rather than pretending"
    );
}

#[test]
fn a_claim_on_something_that_does_not_exist_is_refused() {
    // A claim naming a phantom seed tells a colleague nothing and would sit in
    // the export looking authoritative.
    let mut g = team();
    assert!(
        g.claim_region("who:ann", "req:nope", 2, None, None)
            .is_err()
    );
    assert!(
        g.claim_region("who:nobody", "req:auth", 2, None, None)
            .is_err()
    );
}

#[test]
fn claims_do_not_drag_people_into_blast_radii() {
    // CLAIMS is deliberately not a traceability edge (like AUTHORED_BY): who is
    // working on something is coordination, not design structure. If it
    // propagated, every impact analysis would start reporting people.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None)
        .unwrap();
    let radius = g.propagate_from(&["req:auth"], Default::default()).unwrap();
    assert!(
        !radius.impacted.iter().any(|i| i.node_id == "who:ann"),
        "a contributor must never appear in a blast radius: {:?}",
        radius
            .impacted
            .iter()
            .map(|i| &i.node_id)
            .collect::<Vec<_>>()
    );
}
