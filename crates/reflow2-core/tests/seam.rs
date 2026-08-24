//! Saying when two linked designs disagree at a boundary
//! (`req:seam-incompatibility`).
//!
//! The specification for this came from a measurement, not an opinion: with a
//! seam hand-drawn between two real designs, `compose_and_analyse` plus every
//! ordinary detector produced ZERO findings — because they check connectivity
//! and nothing compares two nodes' properties across a pair.

use std::collections::HashMap;

use reflow2_core::Value;
use reflow2_core::{DesignGraph, GraphExport, Verdict};

/// Build an Interface carrying exactly the properties a case needs. Uses
/// `create_node` rather than the typed setters because `medium` is set at
/// creation while the spec fields are filled in later, and a fixture that has
/// to call two APIs to describe one contract obscures what is being tested.
fn iface(g: &mut DesignGraph, id: &str, name: &str, props: &[(&str, &str)]) {
    let mut p: HashMap<String, Value> = HashMap::new();
    p.insert("name".into(), Value::from(name));
    for (k, v) in props {
        p.insert((*k).into(), Value::from(*v));
    }
    g.create_node("Interface", id, p).unwrap();
}

/// A provider design, published as a document the way a real one arrives.
fn provider(props: &[(&str, &str)]) -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:them", "Provider").unwrap();
    iface(&mut g, "ifc:theirs", "Their boundary", props);
    g.set_interface_designation("ifc:theirs", "published")
        .unwrap();
    g.export_graph().unwrap()
}

fn consumer(props: &[(&str, &str)]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:us", "Consumer").unwrap();
    iface(&mut g, "ifc:ours", "Our need", props);
    g
}

fn pairs() -> Vec<(String, String)> {
    vec![("ifc:ours".to_string(), "ifc:theirs".to_string())]
}

#[test]
fn a_stated_conflict_on_a_closed_vocabulary_is_an_incompatibility() {
    let g = consumer(&[("payload_format", "protobuf")]);
    let them = provider(&[("payload_format", "json")]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert_eq!(r.incompatible.len(), 1, "{:?}", r.incompatible);
    assert_eq!(r.incompatible[0].verdict, Verdict::Incompatible);
    assert!(
        r.incompatible[0].detail.contains("immediately and totally"),
        "the finding should say why it matters: {}",
        r.incompatible[0].detail
    );
}

#[test]
fn unspecified_is_never_reported_as_agreement() {
    // THE load-bearing case. Both sides silent must read as "nobody has said".
    // If unspecified may match unspecified and be called compatible, the false
    // green is rebuilt with extra steps.
    let g = consumer(&[]);
    let them = provider(&[]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert_eq!(r.incompatible.len(), 0);
    assert_eq!(r.agreed, 0, "silence is not agreement");
    assert_eq!(r.unstated.len(), 8, "every axis should read as unstated");
    assert!(
        r.note.contains("stated by NOBODY") && r.note.contains("not a claim of compatibility"),
        "the note must refuse to be read as a clean bill: {}",
        r.note
    );
}

#[test]
fn one_side_silent_is_also_unstated_not_agreement() {
    // The subtler half: a consumer that stated `auth: none` and a provider that
    // said nothing have NOT agreed on `none`.
    let g = consumer(&[("auth", "none")]);
    let them = provider(&[]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert_eq!(r.agreed, 0);
    assert!(
        r.unstated
            .iter()
            .any(|f| f.our_value.as_deref() == Some("none"))
    );
}

#[test]
fn free_text_differences_are_reported_but_never_called_incompatible() {
    // A machine cannot tell a real mismatch from two people describing the same
    // contract in different words. Calling that "incompatible" would train
    // people to ignore the finding.
    let g = consumer(&[("operations", "GET, POST")]);
    let them = provider(&[("operations", "read and create")]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(
        r.incompatible.is_empty(),
        "free text must not be an incompatibility"
    );
    assert_eq!(r.differs.len(), 1);
    assert_eq!(r.differs[0].verdict, Verdict::Differs);
}

#[test]
fn agreement_is_agreement() {
    let g = consumer(&[("paradigm", "synchronous"), ("payload_format", "json")]);
    let them = provider(&[("paradigm", "synchronous"), ("payload_format", "json")]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert_eq!(r.agreed, 2);
}

#[test]
fn a_medium_mismatch_is_reported_because_it_cannot_be_wired_at_all() {
    // The provider's own objection during the trial: a consumer needing a
    // linked library cannot be handed a REST endpoint, however well the rest
    // matches.
    let g = consumer(&[("medium", "library")]);
    let them = provider(&[("medium", "REST")]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert_eq!(r.incompatible.len(), 1);
    assert!(r.incompatible[0].detail.contains("cannot be connected"));
}

#[test]
fn an_unresolvable_pair_says_which_side_is_missing() {
    // "Pair not found" sends someone looking in the wrong design.
    let g = consumer(&[]);
    let them = provider(&[]);

    let r = g
        .seam_report(&them, &[("ifc:ours".into(), "ifc:nope".into())])
        .unwrap();
    assert_eq!(r.pairs_checked, 0);
    assert_eq!(r.unresolved_pairs.len(), 1);
    assert!(
        r.unresolved_pairs[0].contains("OTHER design"),
        "must name the side: {}",
        r.unresolved_pairs[0]
    );
}

#[test]
fn comparing_nothing_is_not_a_clean_bill_of_health() {
    let g = consumer(&[]);
    let them = provider(&[]);

    let r = g
        .seam_report(&them, &[("ifc:missing".into(), "ifc:nope".into())])
        .unwrap();
    assert!(
        r.note.contains("NOTHING WAS COMPARED") && r.note.contains("not a clean bill of health"),
        "an empty comparison must refuse to look like a pass: {}",
        r.note
    );
}

#[test]
fn the_report_says_what_it_did_not_examine() {
    // con:pairing-stops-at-the-boundary. A check that stays quiet about its own
    // blind spot is how a clean result becomes a lie.
    let g = consumer(&[("payload_format", "json")]);
    let them = provider(&[("payload_format", "json")]);

    let r = g.seam_report(&them, &pairs()).unwrap();
    assert!(r.incompatible.is_empty());
    assert!(
        r.not_examined.contains("TYPES that cross"),
        "even a clean seam must name its blind spot: {}",
        r.not_examined
    );
}

// ===========================================================================
// PAIRING — computing the seam instead of asserting it
// (`req:complementary-pairing`, `dec:pairing-role-placement`)
// ===========================================================================

/// A design that NEEDS a boundary — the half that did not exist before 2026-07-30.
fn needer(id: &str, name: &str, props: &[(&str, &str)]) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:us", "Us").unwrap();
    iface(&mut g, id, name, props);
    g.set_interface_designation(id, "required").unwrap();
    g
}

/// A design that OFFERS one, as the document a counterparty publishes.
fn offerer(id: &str, name: &str, props: &[(&str, &str)]) -> GraphExport {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:them", "Them").unwrap();
    iface(&mut g, id, name, props);
    g.set_interface_designation(id, "published").unwrap();
    g.export_graph().unwrap()
}

const REST_JWT_TLS: [(&str, &str); 3] = [
    ("medium", "REST"),
    ("auth", "jwt"),
    ("transport_security", "tls"),
];

#[test]
fn a_need_and_an_offer_pair_by_complementary_role() {
    let us = needer("ifc:auth-api", "Auth API", &REST_JWT_TLS);
    let them = offerer("ifc:auth-api", "Auth API", &REST_JWT_TLS);

    let r = us.pair_designs(&them).unwrap();
    assert_eq!(r.paired.len(), 1, "one need, one offer, one strand: {r:?}");
    assert_eq!(r.paired[0].offered_by, "theirs");
    assert!(r.unmet_needs.is_empty());
    assert!(r.dead_surface.is_empty());
}

/// The complement rule, and the whole point of the RNA analogy: a base pairs
/// with its complement, never with a copy of itself.
#[test]
fn two_publishers_never_pair_with_each_other() {
    let mut us = DesignGraph::open_in_memory().unwrap();
    us.add_project("prj:us", "Us").unwrap();
    iface(&mut us, "ifc:auth-api", "Auth API", &REST_JWT_TLS);
    us.set_interface_designation("ifc:auth-api", "published")
        .unwrap();
    let them = offerer("ifc:auth-api", "Auth API", &REST_JWT_TLS);

    let r = us.pair_designs(&them).unwrap();
    assert!(
        r.paired.is_empty() && r.conflicts.is_empty(),
        "publisher-to-publisher is like-with-like and must not pair: {r:?}"
    );
    assert_eq!(r.dead_surface.len(), 1, "their offer answers no need here");
}

/// THE CASE THAT REFUTED THE FIRST DRAFT OF THE KEY, from the real
/// dynograph-foundation trial: an orchestrator's liveness probe is public and
/// unauthenticated BY DESIGN. Under role-plus-medium alone, "I require REST"
/// pairs against it — a wrong and security-relevant answer.
#[test]
fn a_consumer_requiring_auth_does_not_pair_with_a_public_probe() {
    let us = needer("ifc:auth-api", "Auth API", &REST_JWT_TLS);
    let them = offerer(
        "ifc:auth-api",
        "Auth API",
        &[
            ("medium", "REST"),
            ("auth", "none"),
            ("transport_security", "none"),
        ],
    );

    let r = us.pair_designs(&them).unwrap();
    assert!(
        r.paired.is_empty(),
        "matching on medium alone would have paired these: {r:?}"
    );
    assert_eq!(r.conflicts.len(), 1, "and it must be REPORTED, not dropped");
    let axes: Vec<&str> = r.conflicts[0]
        .refusals
        .iter()
        .map(|x| x.axis.as_str())
        .collect();
    assert!(
        axes.contains(&"auth") && axes.contains(&"transport_security"),
        "the probe refuses on BOTH axes and both must be named, or the caller \
         fixes one and rediscovers the other: {axes:?}"
    );
    assert!(r.conflicts[0].detail.contains("cannot be connected"));
}

/// A mismatch is a finding, never silence.
#[test]
fn a_name_match_that_cannot_connect_is_a_conflict_not_a_non_match() {
    let us = needer("ifc:events", "Events", &[("medium", "REST")]);
    let them = offerer("ifc:events", "Events", &[("medium", "gRPC")]);

    let r = us.pair_designs(&them).unwrap();
    assert!(r.paired.is_empty());
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].refusals[0].axis, "medium");
    assert!(
        r.unmet_needs.is_empty(),
        "a reported conflict must not ALSO be counted as an unmet need — one \
         situation, one finding"
    );
}

#[test]
fn a_need_nobody_publishes_is_the_loudest_finding() {
    let us = needer("ifc:billing", "Billing", &REST_JWT_TLS);
    let them = offerer("ifc:auth-api", "Auth API", &REST_JWT_TLS);

    let r = us.pair_designs(&them).unwrap();
    assert_eq!(r.unmet_needs.len(), 1);
    assert_eq!(r.unmet_needs[0].id, "ifc:billing");
    assert_eq!(r.dead_surface.len(), 1, "and their offer is dead surface");
}

/// The middle band is asked about, never acted on (`dec:ask-not-repair`).
#[test]
fn a_fuzzy_name_match_is_a_candidate_rather_than_a_pair() {
    let us = needer("ifc:gateway", "Gateway", &REST_JWT_TLS);
    let them = offerer("ifc:api-gateway", "API Gateway Service", &REST_JWT_TLS);

    let r = us.pair_designs(&them).unwrap();
    assert!(
        r.paired.is_empty() && r.candidates.len() == 1,
        "an uncertain name must be a candidate to ask about, not an action: {r:?}"
    );
}

/// `internal` is the DEFAULT, so it cannot mean "deliberately internal" — an
/// unlabelled design must not report a clean seam.
#[test]
fn unclassified_boundaries_are_counted_and_named() {
    let mut us = DesignGraph::open_in_memory().unwrap();
    us.add_project("prj:us", "Us").unwrap();
    iface(&mut us, "ifc:quiet", "Quiet thing", &REST_JWT_TLS);
    let them = offerer("ifc:auth-api", "Auth API", &REST_JWT_TLS);

    let r = us.pair_designs(&them).unwrap();
    assert_eq!(
        r.unclassified_ours.len(),
        1,
        "a boundary nobody classified must be named, not silently skipped"
    );
    assert!(r.unclassified_ours[0].contains("ifc:quiet"));
    assert!(
        r.note.contains("DEFAULT"),
        "and the report must say why that matters"
    );
}

/// `both` is on the Interface precisely so it stays rare and meaningful.
#[test]
fn both_offers_and_needs() {
    let mut us = DesignGraph::open_in_memory().unwrap();
    us.add_project("prj:us", "Us").unwrap();
    iface(&mut us, "ifc:relay", "Relay", &REST_JWT_TLS);
    us.set_interface_designation("ifc:relay", "both").unwrap();

    assert!(us.published_interfaces().unwrap().contains("ifc:relay"));
    assert!(us.required_interfaces().unwrap().contains("ifc:relay"));

    let them = offerer("ifc:relay", "Relay", &REST_JWT_TLS);
    let r = us.pair_designs(&them).unwrap();
    assert_eq!(r.paired.len(), 1, "both pairs against a publisher: {r:?}");
}

#[test]
fn two_publishers_of_one_need_is_a_conflict_to_resolve() {
    let us = needer("ifc:auth-api", "Auth API", &REST_JWT_TLS);
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:them", "Them").unwrap();
    for id in ["ifc:auth-a", "ifc:auth-b"] {
        iface(&mut g, id, "Auth API", &REST_JWT_TLS);
        g.set_interface_designation(id, "published").unwrap();
    }
    let them = g.export_graph().unwrap();

    let r = us.pair_designs(&them).unwrap();
    assert_eq!(
        r.duplicate_providers.len(),
        1,
        "two publishers answering one need must be reported: {r:?}"
    );
    assert!(r.duplicate_providers[0].contains("conflict"));
}

#[test]
fn an_unknown_designation_is_refused_by_name() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:us", "Us").unwrap();
    iface(&mut g, "ifc:x", "X", &[]);
    let err = g
        .set_interface_designation("ifc:x", "subscriber")
        .expect_err("a role outside the enum must be refused");
    let said = format!("{err:?}");
    assert!(
        said.contains("required") && said.contains("published"),
        "and the refusal must say what would have worked, got: {said}"
    );
}
