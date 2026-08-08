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
            None,
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
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
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
    g.claim_region("who:ann", "req:auth", 2, Some("MFA"), None, None)
        .unwrap();
    g.claim_region("who:bob", "cap:login", 1, Some("rate limiting"), None, None)
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
    g.claim_region("who:ann", "cap:logout", 0, None, None, None)
        .unwrap();
    g.claim_region("who:bob", "cap:export", 0, None, None, None)
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
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
        .unwrap();
    g.claim_region("who:ann", "cap:login", 2, None, None, None)
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
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
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
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
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
        g.claim_region("who:ann", "req:nope", 2, None, None, None)
            .is_err()
    );
    assert!(
        g.claim_region("who:nobody", "req:auth", 2, None, None, None)
            .is_err()
    );
}

#[test]
fn claims_do_not_drag_people_into_blast_radii() {
    // CLAIMS is deliberately not a traceability edge (like AUTHORED_BY): who is
    // working on something is coordination, not design structure. If it
    // propagated, every impact analysis would start reporting people.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
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

// ---------------------------------------------------------------------------
// req:claims-have-owners — a claim names its session, and liveness is computed
// ---------------------------------------------------------------------------

/// A seat on this machine whose process is definitely not running.
///
/// Built from a pid that cannot exist rather than by killing something: a test
/// that spawns and reaps a process to get a dead pid races the OS reusing it.
fn dead_seat() -> String {
    // Built from a pid that cannot be running rather than by killing something:
    // a test that spawns and reaps a process to get a dead pid races the OS
    // reusing it. The machine name comes from the same function the check uses,
    // so the two cannot disagree.
    format!("{}:0:deadbeef", reflow2_core::identity::machine())
}

#[test]
fn a_claim_records_the_session_that_made_it() {
    let mut g = team();
    let claim = g
        .claim_region("who:ann", "req:auth", 2, Some("MFA"), None, None)
        .unwrap();

    let seat = claim.seat.expect("a claim must name its seat");
    assert!(
        seat.contains(&std::process::id().to_string()),
        "defaulting to this session is what makes liveness computable: {seat}"
    );
    assert_eq!(
        claim.liveness,
        reflow2_core::identity::Liveness::Live,
        "the session that just made it is, definitionally, still running"
    );
}

#[test]
fn a_claim_from_a_session_that_exited_is_reported_as_a_ghost() {
    // THE test. A claim nobody is working must not read as held — that is the
    // lie that makes an advisory report worse than no report, because people
    // act on it and wait for somebody who left.
    let mut g = team();
    g.claim_region(
        "who:ann",
        "req:auth",
        2,
        Some("left hours ago"),
        None,
        Some(&dead_seat()),
    )
    .unwrap();

    let report = g.claim_report().unwrap();

    assert_eq!(report.claims.len(), 1);
    assert_eq!(
        report.claims[0].liveness,
        reflow2_core::identity::Liveness::Gone
    );
    assert_eq!(report.stale.len(), 1, "and it is called out: {report:?}");
    assert_eq!(
        report.stale[0].note.as_deref(),
        Some("left hours ago"),
        "the ghost keeps its note — that is usually what a colleague wants to read"
    );
}

#[test]
fn a_ghost_claim_is_not_a_collision() {
    // The consequence that matters: overlaps must mean "somebody is actually
    // working this ground", or the report trains people to ignore it.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None, Some(&dead_seat()))
        .unwrap();
    g.claim_region("who:bob", "req:auth", 2, None, None, None)
        .unwrap();

    let report = g.claim_report().unwrap();

    assert!(
        report.overlaps.is_empty(),
        "a live claim overlapping a dead one is not a collision: {:?}",
        report.overlaps
    );
    assert_eq!(report.claims.len(), 2, "but both are still listed");
    assert_eq!(report.stale.len(), 1);
}

#[test]
fn two_live_sessions_on_the_same_ground_still_collide() {
    // The guard on the guard: liveness must not quietly suppress real overlaps.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
        .unwrap();
    g.claim_region("who:bob", "req:auth", 2, None, None, None)
        .unwrap();

    let report = g.claim_report().unwrap();

    assert_eq!(report.overlaps.len(), 1, "{report:?}");
    assert!(report.stale.is_empty());
}

#[test]
fn a_claim_from_another_machine_is_unknown_rather_than_dead() {
    // Never read as free: taking work somebody is actively doing on another
    // computer is the expensive mistake, and a pid means nothing over there.
    let mut g = team();
    g.claim_region(
        "who:ann",
        "req:auth",
        2,
        None,
        None,
        Some("some-other-box:4242:aabbccdd"),
    )
    .unwrap();
    g.claim_region("who:bob", "req:auth", 2, None, None, None)
        .unwrap();

    let report = g.claim_report().unwrap();

    assert_eq!(
        report
            .claims
            .iter()
            .find(|c| c.contributor_id == "who:ann")
            .unwrap()
            .liveness,
        reflow2_core::identity::Liveness::Unknown
    );
    assert!(report.stale.is_empty(), "unknown is not gone");
    assert_eq!(
        report.overlaps.len(),
        1,
        "and it still counts as a possible collision: {report:?}"
    );
}

#[test]
fn a_claim_made_before_seats_existed_is_unknown_not_live() {
    // Legacy claims carry no seat. Guessing "live" would be the old behaviour
    // with a new label on it; guessing "gone" would invite somebody to take
    // work in progress. Unknown is the only true answer.
    let mut g = team();
    g.claim_region("who:ann", "req:auth", 2, None, None, None)
        .unwrap();
    // Strip the seat the way a pre-v0.13 claim has none.
    let edges = g.outgoing("who:ann", Some("CLAIMS")).unwrap();
    let target = edges[0].to_id.clone();
    g.delete_edge("CLAIMS", "who:ann", &target).unwrap();
    g.create_edge(
        "CLAIMS",
        "Contributor",
        "who:ann",
        "Requirement",
        &target,
        reflow2_core::nodes::Props::new().set("depth", 2i64),
    )
    .unwrap();

    let report = g.claim_report().unwrap();

    assert_eq!(report.claims[0].seat, None);
    assert_eq!(
        report.claims[0].liveness,
        reflow2_core::identity::Liveness::Unknown
    );
    assert!(report.stale.is_empty());
}

// ---- The shared-server case: liveness must be about the SESSION ------------
//
// Reported by dev_storyflow 2026-08-07 (w-613d836b) and confirmed here: under
// `--shared` every seat is minted BY THE DAEMON, so every seat carries the
// daemon's pid. The old probe asked "is that pid alive?", the answer was always
// yes because the daemon is what answered, and `Gone` became unreachable. A
// worker that died hours ago still read as holding its region.
//
// These tests pin the registry that fixes it. They run in the CORE test process,
// which is exactly the shape that made the bug invisible: here the test process
// IS the "server", so its own pid is trivially alive and only the registry can
// tell an attached session from a departed one.

#[test]
fn a_seat_this_process_serves_reads_live_and_stops_when_it_departs() {
    let seat = reflow2_core::identity::mint_seat();

    // Before anything registers, the pid probe answers — and this pid is us, so
    // it says live. That is the OLD behaviour, and it is why the bug was silent.
    assert_eq!(
        reflow2_core::identity::seat_liveness(&seat),
        reflow2_core::identity::Liveness::Live,
    );

    reflow2_core::identity::register_seat(&seat);
    assert_eq!(
        reflow2_core::identity::seat_liveness(&seat),
        reflow2_core::identity::Liveness::Live,
        "an attached session is live"
    );

    reflow2_core::identity::release_seat(&seat);
    assert_eq!(
        reflow2_core::identity::seat_liveness(&seat),
        reflow2_core::identity::Liveness::Gone,
        "THE DEFECT: this read `live` forever, because the pid in the seat is the \
         serving process's and the serving process is what answers"
    );
}

#[test]
fn a_lease_releases_its_seat_when_the_session_drops() {
    // The release is Drop rather than a step somebody remembers, so a client
    // that crashes releases its seat on the way out just like one that exits.
    let seat = {
        let lease = reflow2_core::identity::SeatLease::attach();
        let seat = lease.id().to_string();
        assert_eq!(
            reflow2_core::identity::seat_liveness(&seat),
            reflow2_core::identity::Liveness::Live,
        );
        seat
    };
    assert_eq!(
        reflow2_core::identity::seat_liveness(&seat),
        reflow2_core::identity::Liveness::Gone,
        "dropping the handler must retire the seat with nothing having to say so"
    );
}

#[test]
fn a_seat_from_a_previous_server_is_gone_not_unknown() {
    // The `--stop-shared` bounce: the replacement daemon has a new pid, so every
    // seat minted before it carries a pid that no longer exists. That case is
    // still answered by the OS probe, deliberately — the registry only takes
    // over for seats THIS process minted.
    let dead = dead_seat();
    assert_eq!(
        reflow2_core::identity::seat_liveness(&dead),
        reflow2_core::identity::Liveness::Gone,
        "a seat from a server that is no longer running is a ghost"
    );
}

#[test]
fn attached_count_answers_only_when_the_process_is_tracking() {
    // req:a-session-can-tell-it-is-not-alone leans on this: a seat that can see
    // how many sessions are attached cannot honestly report a graph-wide rollup
    // as its own result.
    // ASSERTED BY MEMBERSHIP, NOT BY CARDINALITY, and that is the whole point of
    // this comment. The first version of this test read the count, attached a
    // second lease, and asserted `before + 1`. It passed locally and on the PR
    // that introduced it, then failed in CI on the very next run: `left: Some(2),
    // right: Some(1)`. The registry is PROCESS-global and cargo runs a test
    // binary's tests as parallel threads in ONE process, so the sibling tests in
    // this file attach and drop leases of their own and the total moves
    // underneath any delta assertion. A flake, introduced by me, and exactly the
    // shared-state-read-as-private-state confusion this whole feature exists to
    // fix — committed while fixing it.
    //
    // So: the count proves only that the process is TRACKING (Some, and at least
    // our two), and the precise behaviour is asserted through the seats we own,
    // which no other test can touch.
    let lease = reflow2_core::identity::SeatLease::attach();
    let second = reflow2_core::identity::SeatLease::attach();
    let mine = second.id().to_string();

    let count = reflow2_core::identity::attached_seat_count()
        .expect("a process that has attached a seat is tracking");
    assert!(count >= 2, "both our leases must be attached; got {count}");
    assert_eq!(
        reflow2_core::identity::seat_liveness(&mine),
        reflow2_core::identity::Liveness::Live,
    );

    drop(second);
    assert_eq!(
        reflow2_core::identity::seat_liveness(&mine),
        reflow2_core::identity::Liveness::Gone,
        "a departed session must be visible as departed to the one still here"
    );
    assert!(
        reflow2_core::identity::attached_seat_count().is_some_and(|n| n >= 1),
        "the surviving lease keeps the process tracking"
    );
    drop(lease);
}
