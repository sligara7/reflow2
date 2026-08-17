//! A state a RULING declares correct is reported as parked, not as a defect —
//! and is COUNTED rather than silenced.
//!
//! `req:a-deliberate-state-is-not-a-defect`.
//!
//! # The failure, measured by dev_storyflow's fleet
//!
//! Defects moved **88 → 97 across ten artifact writes**, eight of them
//! deliberate no-claim registrations a standing ruling prescribed, with 31 more
//! documents owed in the same genre. Their framing is the requirement: *the
//! disposition rule and the defect detector make each other unreadable, and the
//! direction is the bad one — THE CORRECT ACTION DEGRADES THE INSTRUMENT.* It
//! is self-reinforcing: a later seat watching the count climb has an incentive
//! to stop registering documents, which is the worse state.
//!
//! # Why counting, not silencing — the finding that shaped this
//!
//! Reproduced live BEFORE any of this was designed: an Artifact carrying **any**
//! `GOVERNED_BY` edge already escaped `orphan_node`, because the detector saw an
//! EDGE and never a RULING. So the deliberate ones could already be hidden and
//! could not be COUNTED — "deliberately parked" and "never looked at" gave the
//! identical answer, which is a vacuous zero in different clothes.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

/// A graph holding a superseded scope proposal: registered on purpose, with no
/// claim edges, exactly as the fleet's ruling prescribes.
fn fleet_case(park_it: bool, accept_the_ruling: bool) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_artifact(
        "art:superseded",
        "2026-07-scope-proposal.md",
        Some("document"),
        Some("docs/proposals/2026-07.md"),
    )
    .unwrap();
    g.add_decision(
        "dec:no-claims-from-shipped-scope",
        "A superseded scope proposal is registered with no claim edges",
        "A DOCUMENTS edge would count a shipped proposal as coverage in the ingest ledger, \
         and absence would read as a backlog somebody re-mines. Registered, no doctrine, \
         on purpose is the correct end state.",
        None,
    )
    .unwrap();
    if accept_the_ruling {
        g.set_decision_status("dec:no-claims-from-shipped-scope", "accepted")
            .unwrap();
    }
    if park_it {
        g.create_edge(
            edge::GOVERNED_BY,
            node::ARTIFACT,
            "art:superseded",
            node::DECISION,
            "dec:no-claims-from-shipped-scope",
            Props::new().set("ruling", "parks"),
        )
        .unwrap();
    }
    g
}

fn orphans(g: &DesignGraph) -> Vec<String> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == HealCategory::OrphanNode)
        .flat_map(|d| d.affected_ids)
        .collect()
}

/// Without a ruling the finding stands — the counterweight that keeps every
/// assertion below meaningful.
#[test]
fn an_unruled_registration_is_still_a_defect() {
    let g = fleet_case(false, false);
    assert!(
        orphans(&g).contains(&"art:superseded".to_string()),
        "an artifact attached to nothing, with nothing saying why, is still a finding"
    );
}

/// The requirement's actual ask: skipped as a defect AND counted as parked.
#[test]
fn a_parked_node_is_counted_rather_than_silenced() {
    let g = fleet_case(true, true);
    let sweep = g.detect_defects().unwrap();

    assert!(
        !sweep
            .defects
            .iter()
            .filter(|d| d.category == HealCategory::OrphanNode)
            .any(|d| d.affected_ids.contains(&"art:superseded".to_string())),
        "a ruling declares this state correct, so it is not a defect"
    );
    assert_eq!(
        sweep.swept.parked,
        vec!["art:superseded".to_string()],
        "…and it must be COUNTED, or 'deliberately parked' and 'never looked at' are the \
         same answer — the vacuous zero this increment exists to end"
    );
}

/// A PROPOSED ruling is somebody thinking out loud. Parking on one would let a
/// musing suppress a finding, so the authority has to be settled.
#[test]
fn parking_on_an_unsettled_ruling_does_not_count() {
    let g = fleet_case(true, false);
    let sweep = g.detect_defects().unwrap();
    assert!(
        sweep.swept.parked.is_empty(),
        "a `proposed` Decision is a brainstorm, not an authority: {:?}",
        sweep.swept.parked
    );
    assert!(
        orphans(&g).contains(&"art:superseded".to_string()),
        "…so the finding stands until somebody actually rules"
    );
}

/// Case (b), and the half no edge could ever silence incidentally: a
/// requirement deliberately left unsatisfied. `unsatisfied_requirement` looks
/// for incoming SATISFIES, so governance is invisible to it unless READ.
///
/// dev_storyflow's report is what makes this urgent: the detector's suggested
/// fix was `generate_bridge` — precisely the forgery the ruling forbids — and
/// an agent working the defect list top-down would have performed it.
#[test]
fn a_requirement_ruled_intentionally_unsatisfied_stops_being_asked_about() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement(
        "req:by-ruling",
        "Deliberately unsatisfied",
        "Nothing may satisfy this.",
    )
    .unwrap();
    g.set_requirement_status("req:by-ruling", "accepted")
        .unwrap();
    // A capability must exist or the detector stays quiet for another reason.
    g.add_capability("cap:other", "Other", "does something else", None)
        .unwrap();

    let asked = |g: &DesignGraph| -> bool {
        g.detect_gaps()
            .unwrap()
            .into_iter()
            .any(|c| c.affected_ids.contains(&"req:by-ruling".to_string()))
    };
    assert!(
        asked(&g),
        "precondition: it is asked about while nothing rules"
    );

    g.add_decision(
        "dec:no-satisfier",
        "No SATISFIES may be drawn to req:by-ruling",
        "It states an intent the design deliberately does not deliver.",
        None,
    )
    .unwrap();
    g.set_decision_status("dec:no-satisfier", "accepted")
        .unwrap();
    g.create_edge(
        edge::GOVERNED_BY,
        node::REQUIREMENT,
        "req:by-ruling",
        node::DECISION,
        "dec:no-satisfier",
        Props::new().set("ruling", "parks"),
    )
    .unwrap();

    assert!(
        !asked(&g),
        "a ruling that forbids a satisfier must stop the tool demanding one"
    );
    assert_eq!(
        g.detect_defects().unwrap().swept.parked,
        vec!["req:by-ruling".to_string()],
        "and it stays visible as parked rather than vanishing"
    );
}

/// Ordinary governance must NOT park anything, or every governed node in a
/// mature design would quietly stop being checked.
#[test]
fn plain_governance_does_not_park() {
    let mut g = fleet_case(false, true);
    g.create_edge(
        edge::GOVERNED_BY,
        node::ARTIFACT,
        "art:superseded",
        node::DECISION,
        "dec:no-claims-from-shipped-scope",
        Props::new(), // no `ruling` — this shapes that, nothing more
    )
    .unwrap();

    assert!(
        g.detect_defects().unwrap().swept.parked.is_empty(),
        "an unmarked GOVERNED_BY is plain governance and must never park"
    );
}
