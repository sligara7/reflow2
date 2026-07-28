//! Saying when two linked designs disagree at a boundary
//! (`req:seam-incompatibility`).
//!
//! The specification for this came from a measurement, not an opinion: with a
//! seam hand-drawn between two real designs, `compose_and_analyse` plus every
//! ordinary detector produced ZERO findings — because they check connectivity
//! and nothing compares two nodes' properties across a pair.

use std::collections::HashMap;

use dynograph_core::Value;
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
