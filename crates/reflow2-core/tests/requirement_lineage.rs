//! Requirement lineage — original, decomposed, derived.
//!
//! From Anthony's acquisitions background (2026-07-25). Requirements multiply in
//! two different ways and they behave differently, which is the whole reason the
//! distinction is stored rather than left to prose:
//!
//! - a DECOMPOSED child is a 1:1 split of a parent adding no new information, so
//!   satisfying every child satisfies the parent — delivery rolls UP;
//! - a DERIVED requirement is technical necessity nobody asked for, created by a
//!   design decision, so it hangs off that Decision and may lose its reason to
//!   exist if the decision is ever re-opened.
//!
//! Before this, reflow2 had ZERO requirement-to-requirement edges: a
//! systems-engineering tool holding requirements as a flat list.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// A checkout system split into three children, none of them built yet.
fn checkout() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "Shop").unwrap();
    g.add_requirement(
        "req:checkout",
        "Checkout",
        "The app must have a checkout system.",
    )
    .unwrap();
    for (id, name) in [
        ("req:card", "Enter a card"),
        ("req:discount", "Apply a discount code"),
        ("req:receipt", "Receive an email receipt"),
    ] {
        g.add_requirement(id, name, "part of checkout").unwrap();
        g.decomposes(id, "req:checkout").unwrap();
    }
    g
}

/// Build and fully deliver a capability for `req`.
fn deliver(g: &mut DesignGraph, req: &str, tag: &str) {
    let cap = format!("cap:{tag}");
    let art = format!("art:{tag}");
    let ver = format!("ver:{tag}");
    g.add_capability(&cap, tag, "does it", Some("realized"))
        .unwrap();
    g.satisfies(&cap, req).unwrap();
    g.add_artifact(&art, tag, Some("code"), Some("src/x.rs"))
        .unwrap();
    g.realizes(&art, node::CAPABILITY, &cap, None).unwrap();
    g.add_verification(&ver, tag, Some("test"), None).unwrap();
    g.verifies(&ver, node::CAPABILITY, &cap).unwrap();
    g.set_verification_status(&ver, "passing", None).unwrap();
}

#[test]
fn decomposing_records_the_edge_and_labels_the_child() {
    let g = checkout();
    assert_eq!(
        g.decomposed_children("req:checkout").unwrap(),
        vec!["req:card", "req:discount", "req:receipt"]
    );
    let child = g.get_node(node::REQUIREMENT, "req:card").unwrap().unwrap();
    assert_eq!(
        child.properties.get("lineage").and_then(|v| v.as_str()),
        Some("decomposed"),
        "the edge and the label are the same fact; letting them disagree would \
         make the classification a second thing to maintain"
    );
    // The parent is untouched — it is still whatever it was.
    let parent = g
        .get_node(node::REQUIREMENT, "req:checkout")
        .unwrap()
        .unwrap();
    assert_eq!(
        parent.properties.get("lineage").and_then(|v| v.as_str()),
        Some("original")
    );
}

#[test]
fn a_parent_is_delivered_only_when_every_child_is() {
    // THE test. "Any" would let one finished slice of a checkout system report
    // the whole thing done, which is worse than no number at all.
    let mut g = checkout();
    assert!(!g.requirement_is_delivered("req:checkout").unwrap());

    deliver(&mut g, "req:card", "card");
    assert!(
        !g.requirement_is_delivered("req:checkout").unwrap(),
        "one of three is not delivery"
    );

    deliver(&mut g, "req:discount", "discount");
    assert!(!g.requirement_is_delivered("req:checkout").unwrap());

    deliver(&mut g, "req:receipt", "receipt");
    assert!(
        g.requirement_is_delivered("req:checkout").unwrap(),
        "every child delivered, so the parent is carried by them"
    );
}

#[test]
fn a_decomposed_parent_counts_as_satisfied_without_a_capability_of_its_own() {
    // Splitting a requirement adds no new information, so demanding the parent
    // carry its own capability would punish exactly the practice systems
    // engineering asks for — a properly decomposed design would read as a wall
    // of unsatisfied parents.
    let g = checkout();
    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.requirements, 4);
    assert_eq!(
        d.satisfied, 1,
        "the parent is carried by its children; the children themselves are \
         not yet satisfied by anything"
    );
    assert_eq!(d.delivered, 0);
}

#[test]
fn the_whole_tree_rolls_up_in_the_coverage_number() {
    let mut g = checkout();
    for (req, tag) in [
        ("req:card", "card"),
        ("req:discount", "discount"),
        ("req:receipt", "receipt"),
    ] {
        deliver(&mut g, req, tag);
    }
    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.requirements, 4);
    assert_eq!(d.satisfied, 4);
    assert_eq!(
        d.delivered, 4,
        "three leaves delivered, and the parent with them — without anyone \
         setting a status anywhere"
    );
}

#[test]
fn a_failing_child_un_delivers_the_parent() {
    // Roll-up must inherit the property that made delivery a derivation rather
    // than an assertion: it has to go backwards. A parent that stays green when
    // one of its children regresses is a stored claim wearing a tree.
    let mut g = checkout();
    for (req, tag) in [
        ("req:card", "card"),
        ("req:discount", "discount"),
        ("req:receipt", "receipt"),
    ] {
        deliver(&mut g, req, tag);
    }
    assert!(g.requirement_is_delivered("req:checkout").unwrap());

    g.set_verification_status("ver:discount", "failing", None)
        .unwrap();
    assert!(
        !g.requirement_is_delivered("req:checkout").unwrap(),
        "a child's check now fails, so the parent is no longer delivered"
    );
}

#[test]
fn decomposition_cannot_be_circular() {
    // A tree that contains itself has no leaves, so "satisfy every child" is
    // unsatisfiable by construction and delivery could never terminate
    // honestly. Cheaper to refuse than to detect later as a defect nobody can
    // act on.
    let mut g = checkout();
    let err = g
        .decomposes("req:checkout", "req:card")
        .expect_err("must refuse to close the loop");
    let msg = err.to_string();
    assert!(
        msg.contains("circular"),
        "the refusal must say why, got: {msg}"
    );

    assert!(
        g.decomposes("req:card", "req:card").is_err(),
        "and a self-loop is refused too"
    );
}

#[test]
fn a_derived_requirement_is_labelled_and_hangs_off_its_decision() {
    // The other class. Nobody asked for it; a design decision forced it, so it
    // traces to the Decision rather than to a parent goal — and that link is
    // what would let a re-opened decision name the requirements it orphans.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "Car").unwrap();
    g.add_requirement(
        "req:range",
        "500 mile range",
        "The car must travel 500 miles.",
    )
    .unwrap();
    g.add_decision(
        "dec:powertrain",
        "Petrol powertrain",
        "Use a petrol engine.",
        Some("Cheapest path to the range target."),
    )
    .unwrap();
    g.add_requirement(
        "req:mass",
        "Body under 3000 lb",
        "The car body must weigh under 3000 pounds.",
    )
    .unwrap();
    g.set_requirement_lineage("req:mass", "derived").unwrap();
    g.governed_by(
        node::REQUIREMENT,
        "req:mass",
        node::DECISION,
        "dec:powertrain",
    )
    .unwrap();

    let r = g.get_node(node::REQUIREMENT, "req:mass").unwrap().unwrap();
    assert_eq!(
        r.properties.get("lineage").and_then(|v| v.as_str()),
        Some("derived")
    );
    // It is NOT a decomposition of the range requirement: it adds new technical
    // information rather than splitting an existing goal.
    assert!(g.decomposed_children("req:range").unwrap().is_empty());
}

#[test]
fn an_unknown_lineage_is_refused_loudly() {
    let mut g = checkout();
    let err = g
        .set_requirement_lineage("req:card", "invented")
        .expect_err("must refuse");
    assert!(err.to_string().contains("original"));
}
