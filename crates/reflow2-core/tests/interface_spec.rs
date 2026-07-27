//! `req:interface-spec-complete` — a published contract carries what two
//! systems must AGREE on, in terms a computation can compare.
//!
//! Before this, `Interface` recorded the technology and put everything else in
//! one free-text `spec` string, so two designs could be linked and reflow2 still
//! could not say they disagreed.

use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

fn design() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_interface("ifc:reads", "Read API").unwrap();
    g
}

fn prop(g: &DesignGraph, id: &str, key: &str) -> Option<String> {
    g.get_node(node::INTERFACE, id).unwrap().and_then(|n| {
        n.properties
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}

#[test]
fn a_contract_records_what_two_sides_must_agree_on() {
    let mut g = design();
    g.set_interface_spec(
        "ifc:reads",
        Some("asynchronous"),
        Some("protobuf"),
        Some("schemas/read.proto"),
        Some("tcp://bus:9000/reads"),
        Some("Subscribe, Ack"),
        Some("mtls"),
        Some("tls"),
        Some("status enum + retryable flag"),
    )
    .unwrap();

    assert_eq!(
        prop(&g, "ifc:reads", "paradigm").as_deref(),
        Some("asynchronous")
    );
    assert_eq!(
        prop(&g, "ifc:reads", "payload_format").as_deref(),
        Some("protobuf")
    );
    assert_eq!(prop(&g, "ifc:reads", "auth").as_deref(), Some("mtls"));
    assert_eq!(
        prop(&g, "ifc:reads", "transport_security").as_deref(),
        Some("tls")
    );
    assert_eq!(
        prop(&g, "ifc:reads", "endpoint").as_deref(),
        Some("tcp://bus:9000/reads")
    );
}

/// **Silence must not read as a claim.** An unrecorded authentication scheme is
/// `unspecified`, never `none` — the flattering default would tell a consumer
/// the contract is open when nobody has said anything at all.
#[test]
fn unrecorded_fields_are_unspecified_not_none() {
    let g = design();
    assert_eq!(
        prop(&g, "ifc:reads", "auth").as_deref(),
        Some("unspecified")
    );
    assert_eq!(
        prop(&g, "ifc:reads", "transport_security").as_deref(),
        Some("unspecified")
    );
    assert_eq!(
        prop(&g, "ifc:reads", "paradigm").as_deref(),
        Some("unspecified")
    );
}

/// Filling in one field must not erase another. A spec gets completed over
/// time and by different people; a setter that reset the rest would quietly
/// destroy somebody else's work.
#[test]
fn filling_one_field_leaves_the_others_alone() {
    let mut g = design();
    g.set_interface_spec(
        "ifc:reads",
        None,
        None,
        None,
        None,
        None,
        Some("oauth2"),
        None,
        None,
    )
    .unwrap();
    g.set_interface_spec(
        "ifc:reads",
        Some("synchronous"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(
        prop(&g, "ifc:reads", "auth").as_deref(),
        Some("oauth2"),
        "not erased"
    );
    assert_eq!(
        prop(&g, "ifc:reads", "paradigm").as_deref(),
        Some("synchronous")
    );
    assert_eq!(
        prop(&g, "ifc:reads", "name").as_deref(),
        Some("Read API"),
        "nor the name"
    );
}

/// An invented value is refused, not stored — the enum is a vocabulary two
/// designs will be compared in, and a typo'd `auth` that quietly persisted
/// would make a seam look mismatched when it is not.
#[test]
fn an_invented_enum_value_is_refused() {
    let mut g = design();
    let bad = g.set_interface_spec(
        "ifc:reads",
        None,
        Some("yaml-ish"),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert!(bad.is_err(), "an unknown payload format must be refused");
}

/// Rate limits and timeouts stay `Constraint`s. This pins the decision NOT to
/// add parallel properties: `CONSTRAINS` already accepts an Interface, and a
/// second place to say the same number would drift from the first.
#[test]
fn performance_limits_bind_the_interface_as_constraints() {
    let mut g = design();
    g.create_node(
        node::CONSTRAINT,
        "con:rate",
        Props::new()
            .set("name", "Read rate limit")
            .set("statement", "100 requests per minute")
            .set("quantity", "requests_per_minute")
            .set("limit", 100.0)
            .set("direction", "maximum"),
    )
    .unwrap();
    g.create_edge(
        edge::CONSTRAINS,
        node::CONSTRAINT,
        "con:rate",
        node::INTERFACE,
        "ifc:reads",
        Props::new(),
    )
    .unwrap();

    let bound = g.outgoing("con:rate", Some(edge::CONSTRAINS)).unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].to_id, "ifc:reads");
}
