//! A discontinued thing stops being unfinished work — and the marker is READ.
//!
//! # What this pins
//!
//! `dec:idea-discontinued-is-a-first-class-state` asked how a design says "we
//! built this and then decided against it", when both existing retirement edges
//! presume a SUCCESSOR at the source end and a discontinued thing has none.
//!
//! The answer needed no new vocabulary: put the **Decision** at the source. A
//! discontinuation always has a decision behind it even when it has no
//! replacement, `OBSOLETES` is already `* -> *`, and its hint already reads
//! "source makes target redundant or deprecated". No edge type, no enum, no
//! stamp move.
//!
//! # Why the reader is the whole point
//!
//! `dec:one-retire-edge` measured on 2026-07-28 that "retiring something marks
//! it and changes nothing — a retired capability still counts in every rollup,
//! still raises its gaps, still appears in delivery arithmetic", and asked what
//! SHOULD consult it. Before this, NOTHING read either retirement edge: zero
//! references to `SUPERSEDES` in the whole source, and `OBSOLETES` created in
//! two places and read in none.
//!
//! A marker nothing reads is a comment. That failure has now been found in
//! `enforced`, in `SUPERSEDES`, and in `OBSOLETES` itself — so this ships with
//! its readers or it should not ship.

use reflow2_core::{DesignGraph, EpochType, nodes::edge, nodes::node};

/// A capability that is built, checked, and satisfies a requirement — the
/// state the content store was in the day before it was withdrawn.
fn a_delivered_capability(g: &mut DesignGraph) {
    g.add_requirement(
        "req:store",
        "Bytes travel with the design",
        "The graph holds pointers and a store holds the bytes.",
    )
    .unwrap();
    g.add_capability(
        "cap:store",
        "Content store",
        "Put and get blobs.",
        Some("realized"),
    )
    .unwrap();
    g.satisfies("cap:store", "req:store").unwrap();
    g.add_verification("ver:store", "the store round-trips", Some("test"), None)
        .unwrap();
    g.verifies("ver:store", "Capability", "cap:store").unwrap();
    g.set_verification_status("ver:store", "passing", None)
        .unwrap();
    g.add_artifact(
        "art:store",
        "content.rs",
        Some("code"),
        Some("src/content.rs"),
    )
    .unwrap();
    g.realizes("art:store", "Capability", "cap:store", Some("complete"))
        .unwrap();
}

/// Withdraw it the way the content store was withdrawn: a Decision, accepted,
/// obsoleting the capability.
fn discontinue(g: &mut DesignGraph, accepted: bool) {
    g.add_decision(
        "dec:store-discontinued",
        "The content store is discontinued",
        "Built, shipped, correct, and used zero times.",
        Some("Nothing takes its place."),
    )
    .unwrap();
    if accepted {
        g.set_decision_status("dec:store-discontinued", "accepted")
            .unwrap();
    }
    g.create_edge(
        edge::OBSOLETES,
        node::DECISION,
        "dec:store-discontinued",
        node::CAPABILITY,
        "cap:store",
        std::collections::HashMap::new(),
    )
    .unwrap();
}

fn gap_sources(g: &DesignGraph, about: &str) -> Vec<String> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|c| c.affected_ids.iter().any(|i| i == about))
        .map(|c| c.gap_source.as_str().to_string())
        .collect()
}

// THE DEFECT CASE. The content store was discontinued on 2026-08-09 and its
// code deleted; two days later the graph still said the capability was
// `realized`, its check `passing`, and its requirements `accepted`. Nothing
// read the withdrawal, so the record stayed wrong.
#[test]
fn a_discontinued_capability_stops_being_asked_about() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    // Strip what made it look finished, so all three detectors would fire.
    g.delete_node("Artifact", "art:store").unwrap();
    g.set_verification_status("ver:store", "planned", None)
        .unwrap();
    let before = gap_sources(&g, "cap:store");
    assert!(
        !before.is_empty(),
        "precondition: it must be asked about while it is live, got {before:?}"
    );

    discontinue(&mut g, true);
    let after = gap_sources(&g, "cap:store");
    assert!(
        after.is_empty(),
        "a discontinued capability must raise nothing, got {after:?}"
    );
}

// COUNTERWEIGHT, and the one that keeps this from becoming a silencer: only an
// ACCEPTED decision discontinues anything. An agent may draw the edge and argue
// for it; the thing keeps counting until somebody with the authority says so.
// This is rule:design-intent-moves-only-on-the-owners-word on the retire path.
#[test]
fn a_proposed_decision_discontinues_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    g.delete_node("Artifact", "art:store").unwrap();
    g.set_verification_status("ver:store", "planned", None)
        .unwrap();

    discontinue(&mut g, false); // left `proposed`
    let after = gap_sources(&g, "cap:store");
    assert!(
        !after.is_empty(),
        "a PROPOSED withdrawal has withdrawn nothing, got {after:?}"
    );
}

// THE CASCADE, and it is deliberate rather than a side effect. Withdrawing the
// thing that met a need does not meet the need — so the requirement drops back
// to unsatisfied and is ASKED about ("covered, deferred, or dropped?"). That is
// what stops a discontinuation being invisible one hop away.
#[test]
fn the_requirement_it_served_becomes_unsatisfied_and_is_asked_about() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    assert!(
        !gap_sources(&g, "req:store").contains(&"unsatisfied_requirement".to_string()),
        "precondition: the requirement is satisfied while the capability is live"
    );

    discontinue(&mut g, true);
    assert!(
        gap_sources(&g, "req:store").contains(&"unsatisfied_requirement".to_string()),
        "the need outlives the withdrawn capability and must be asked about again"
    );
}

// AND THE NUMBERS MUST SAY WHY THEY MOVED. A discontinuation that made delivery
// fall with no mention is the silent-drop failure this project forbids
// everywhere else: the count drops, nothing explains it, and a reader concludes
// the design regressed.
#[test]
fn delivery_reports_what_it_excluded_rather_than_shrinking_quietly() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    let before = g
        .graph_report()
        .unwrap()
        .delivery
        .expect("delivery present");
    assert_eq!(
        before.delivered, 1,
        "precondition: it is delivered while live"
    );
    assert_eq!(before.satisfied_only_by_discontinued, 0);

    discontinue(&mut g, true);
    let after = g
        .graph_report()
        .unwrap()
        .delivery
        .expect("delivery present");
    assert_eq!(
        after.delivered, 0,
        "a withdrawn capability delivers nothing"
    );
    assert_eq!(
        after.satisfied_only_by_discontinued, 1,
        "and the report must SAY the edge was skipped, not just shrink"
    );
}

// COUNTERWEIGHT: an ordinary live capability is untouched. A rule that silenced
// everything would pass the tests above and destroy the gap layer.
#[test]
fn a_live_capability_beside_a_discontinued_one_is_unaffected() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    g.add_capability(
        "cap:other",
        "Something else",
        "Still wanted.",
        Some("planned"),
    )
    .unwrap();
    discontinue(&mut g, true);

    let other = gap_sources(&g, "cap:other");
    assert!(
        other.contains(&"unmotivated_capability".to_string()),
        "the neighbour must still be asked about, got {other:?}"
    );
}

// COUNTERWEIGHT: OBSOLETES from something that is NOT a Decision is a different
// relationship — a superseding epoch, say — and this rule deliberately does not
// read it. Without this, any obsoleting edge would silence a capability.
#[test]
fn obsoleted_by_something_other_than_a_decision_does_not_discontinue() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    a_delivered_capability(&mut g);
    g.delete_node("Artifact", "art:store").unwrap();
    g.set_verification_status("ver:store", "planned", None)
        .unwrap();
    g.add_epoch("epoch:later", "a later epoch", EpochType::Revision, 2)
        .unwrap();
    g.create_edge(
        edge::OBSOLETES,
        node::DESIGN_EPOCH,
        "epoch:later",
        node::CAPABILITY,
        "cap:store",
        std::collections::HashMap::new(),
    )
    .unwrap();

    let after = gap_sources(&g, "cap:store");
    assert!(
        !after.is_empty(),
        "only a Decision discontinues, got {after:?}"
    );
}

// ---------------------------------------------------------------------------
// AND THE VOCABULARY HAS TO SAY SO, or the mechanism above is unreachable.
//
// Alex's fleet, 2026-08-09: "Requirements: deferred / dropped. Capabilities: no
// equivalent — agents stuff DROPPED into description text." The mechanism above
// already existed when that was reported and worked correctly. What did NOT
// exist was any way to FIND it.
//
// MEASURED 2026-08-14 via `describe_schema`, which the served routing table
// tells an agent to use for exactly this lookup: on Capability, `provenance`,
// `capability_type`, `inputs`, `outputs`, `is_entry_point` and `is_exit_point`
// all carried teaching descriptions and **`status` carried none** — four bare
// values and no fifth. The other half of the pair was as bare: `OBSOLETES`'s
// whole hint read "Source makes target redundant or deprecated", which names
// neither the retirement job, nor the Decision as source, nor `discontinued`.
//
// So an agent looked up the four statuses, found nothing about abandonment, and
// put the fact in the only place left — prose the graph cannot compute on.
// `dec:idea-does-a-capability-need-a-cancelled-state`: the answer was never a
// fifth enum value (that would be a SECOND way to say what the graph already
// computes, and one that can disagree with it). It was that the vocabulary
// never connected the two halves.
//
// Pinned here rather than trusted, for the reason the standing rule is pinned
// in every skill: a description nothing reads back is one edit away from
// silently disappearing, and this one is load-bearing precisely because it is
// the only signpost between a stored status and an edge.
// ---------------------------------------------------------------------------

#[test]
fn the_status_enum_says_where_abandonment_actually_lives() {
    let g = DesignGraph::open_in_memory().unwrap();
    let detail = g.describe_node_type(node::CAPABILITY).unwrap();
    let status = detail
        .spec
        .properties
        .iter()
        .find(|p| p.name == "status")
        .expect("Capability must have a status property");

    let text = status.description.as_deref().expect(
        "Capability.status must carry a description — without one an agent \
                 sees four bare values and no hint that abandonment lives on an edge",
    );

    assert!(
        text.contains("OBSOLETES"),
        "it must name the edge that records abandonment: {text}"
    );
    assert!(
        text.contains("discontinued"),
        "it must name the field the edge computes, which is what a reader checks: {text}"
    );
    // The negative half, and the one an agent needs most: there is no fifth
    // value, and saying so is what stops the search ending in `description`.
    let lowered = text.to_lowercase();
    assert!(
        lowered.contains("cancelled") || lowered.contains("dropped"),
        "it must name the value an agent came looking for, to say it is absent \
         ON PURPOSE rather than merely missing: {text}"
    );
}

#[test]
fn the_obsoletes_hint_says_it_is_the_retirement_mechanism() {
    let g = DesignGraph::open_in_memory().unwrap();
    let detail = g.describe_node_type(node::CAPABILITY).unwrap();
    let obsoletes = detail
        .incoming
        .iter()
        .find(|e| e.spec.edge_type == edge::OBSOLETES)
        .expect("OBSOLETES must be offered as an incoming edge on Capability");

    let hint = obsoletes
        .spec
        .hint
        .as_deref()
        .expect("OBSOLETES must carry a hint");

    assert!(
        hint.contains("Decision"),
        "the source end is the Decision that withdrew the thing, and that is the \
         non-obvious half — a retirement edge normally presumes a successor: {hint}"
    );
    assert!(
        hint.contains("discontinued"),
        "it must name what the edge computes, or it reads as bookkeeping: {hint}"
    );
    assert!(
        hint.contains("ACCEPTED") || hint.contains("accepted"),
        "only an accepted Decision discontinues anything — a proposed withdrawal \
         has withdrawn nothing: {hint}"
    );
    assert!(
        hint.contains("status"),
        "it must say the target's own status does NOT move, or an agent will \
         'helpfully' edit it and create the second source of truth this avoids: {hint}"
    );
}
