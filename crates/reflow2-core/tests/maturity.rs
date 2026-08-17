//! Where a design sits on the function-to-structure trajectory (BL-179).
//!
//! The risk in this reading is not that the arithmetic is wrong — it is that a
//! position quietly becomes a verdict. Anthony's own framing is the guard:
//! reflow2's eight systems with no declared contract between any two of them
//! are **not debt**, they are the expected shape of a design that correctly did
//! function first. A tool that scored that as failure would punish exactly the
//! work that went right.
//!
//! So most of what follows checks restraint:
//!
//! - an early-phase design gets a frontier and no complaint;
//! - a band with nothing to measure reads as *unmeasured*, never as zero;
//! - bands scoring above the frontier are reported as **normal**, because real
//!   designs run ahead of themselves;
//! - and no profile ever states where the design *should* be.
//!
//! The frontier is deliberately relative — the lowest band — so there is no
//! threshold to default. `dec:readiness-is-an-observation-the-threshold-is-the-judgement`
//! forbids reflow2 supplying the bar, and needing none at all is stronger than
//! stating one.

use reflow2_core::{DesignGraph, MaturityProfile};

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("open in-memory graph")
}

fn report(g: &DesignGraph) -> MaturityProfile {
    g.maturity_report().expect("maturity report")
}

fn band<'a>(p: &'a MaturityProfile, name: &str) -> &'a reflow2_core::MaturityBand {
    p.bands
        .iter()
        .find(|b| b.name == name)
        .expect("band exists")
}

/// A design that has done function well and declared no structure — reflow2's
/// own shape, in miniature.
fn function_first(g: &mut DesignGraph) {
    g.add_project("proj:demo", "Demo").expect("project");
    for i in 0..4 {
        let (r, c) = (format!("req:{i}"), format!("cap:{i}"));
        g.add_requirement(&r, &format!("Need {i}"), "The system does a thing.")
            .expect("requirement");
        g.set_requirement_status(&r, "accepted").expect("accept");
        g.add_capability(&c, &format!("Cap {i}"), "Does a thing.", None)
            .expect("capability");
        g.satisfies(&c, &r).expect("satisfies");
    }
    g.add_component("cmp:a", "a", "One part.", None).expect("a");
    g.add_component("cmp:b", "b", "Another part.", None)
        .expect("b");
    for i in 0..2 {
        g.allocate(&format!("cap:{i}"), "cmp:a").expect("allocate");
    }
    for i in 2..4 {
        g.allocate(&format!("cap:{i}"), "cmp:b").expect("allocate");
    }
    // The parts talk to each other, and nobody has declared the contract.
    g.create_edge(
        reflow2_core::nodes::edge::DEPENDS_ON,
        reflow2_core::nodes::node::COMPONENT,
        "cmp:a",
        reflow2_core::nodes::node::COMPONENT,
        "cmp:b",
        [],
    )
    .expect("coupling");
}

// ---------------------------------------------------------------------------
// The restraint.
// ---------------------------------------------------------------------------

/// **The load-bearing case.** Function done, structure undeclared: the profile
/// must locate the design without calling it broken. No severity, no defect, no
/// statement of where it ought to be.
#[test]
fn a_function_first_design_is_located_not_condemned() {
    let mut g = graph();
    function_first(&mut g);

    let p = report(&g);

    assert_eq!(p.frontier, Some("seams"), "{:?}", p.bands);
    assert_eq!(band(&p, "seams").ratio, Some(0.0));
    assert_eq!(band(&p, "intent").ratio, Some(1.0));
    assert_eq!(band(&p, "function").ratio, Some(1.0));
    assert_eq!(band(&p, "allocation").ratio, Some(1.0));

    let json = serde_json::to_string(&p).expect("serialize");
    for forbidden in [
        "severity",
        "defect",
        "violation",
        "should be",
        "too low",
        "failing",
    ] {
        assert!(
            !json.contains(forbidden),
            "a position must not read as a verdict, but the profile contains {forbidden:?}"
        );
    }
    // And it says plainly that it will not supply the bar.
    assert!(
        p.not_observed_about.iter().any(|s| s.contains("SHOULD be")),
        "{:?}",
        p.not_observed_about
    );
}

/// A band with an empty population reads as *unmeasured*, with the reason — not
/// as zero. Scoring an absence would invent a deficiency.
#[test]
fn an_empty_population_is_unmeasured_not_zero() {
    let mut g = graph();
    g.add_project("proj:demo", "Demo").expect("project");
    g.add_requirement("req:one", "Need", "A thing.")
        .expect("requirement");

    let p = report(&g);

    let seams = band(&p, "seams");
    assert_eq!(seams.ratio, None, "no components, so no seam to declare");
    assert!(seams.note.as_deref().unwrap_or("").contains("absence"));
    assert_ne!(
        p.frontier,
        Some("seams"),
        "an absence cannot be the frontier"
    );

    let op = band(&p, "operation");
    assert_eq!(op.ratio, None);
    assert!(op.note.is_some());
}

/// Real designs run ahead of themselves — reflow2 shipped and tested for months
/// with no contract declared between any two of its systems. Bands above the
/// frontier are reported as normal, not as work done out of order.
#[test]
fn bands_ahead_of_the_frontier_are_reported_as_normal() {
    let mut g = graph();
    function_first(&mut g);
    // Assurance runs ahead of seams, exactly as in the real design.
    g.add_verification("ver:one", "Checks cap 0", None, None, None)
        .expect("verification");
    g.verifies("ver:one", reflow2_core::nodes::node::CAPABILITY, "cap:0")
        .expect("verifies");
    g.set_verification_status("ver:one", "passing", None, None)
        .expect("passing");

    let p = report(&g);

    assert_eq!(p.frontier, Some("seams"));
    assert!(
        p.ahead_of_frontier.contains(&"assurance"),
        "{:?}",
        p.ahead_of_frontier
    );
    assert!(
        p.notes.iter().any(|n| n.contains("NORMAL")),
        "the pattern must be named as normal: {:?}",
        p.notes
    );
}

/// An empty design is empty, not immature — and the profile says which.
#[test]
fn an_empty_design_is_not_reported_as_immature() {
    let g = graph();

    let p = report(&g);

    assert_eq!(p.frontier, None);
    assert!(p.bands.iter().all(|b| b.ratio.is_none()));
    assert!(
        p.notes
            .iter()
            .any(|n| n.contains("empty design, not an immature one")),
        "{:?}",
        p.notes
    );
}

// ---------------------------------------------------------------------------
// The reading itself.
// ---------------------------------------------------------------------------

/// Declaring the contract moves the frontier — the reading responds to the work
/// it exists to make visible, which is what stops it being decoration.
#[test]
fn declaring_a_seam_moves_the_frontier() {
    let mut g = graph();
    function_first(&mut g);
    assert_eq!(report(&g).frontier, Some("seams"));

    g.add_interface("ifc:seam", "The contract between a and b")
        .expect("interface");
    g.provides("cmp:a", "ifc:seam").expect("provides");
    g.consumes("cmp:b", "ifc:seam").expect("consumes");

    let p = report(&g);
    assert_eq!(band(&p, "seams").ratio, Some(1.0));
    assert_ne!(p.frontier, Some("seams"), "{:?}", p.bands);
}

/// A contract with only one side recorded does not count. An unrecorded
/// counterparty is precisely the invisible coupling the capture skill warns
/// about, and crediting it would let a design look connected while nothing
/// tells the other side it broke.
#[test]
fn a_one_sided_contract_does_not_count_as_a_declared_seam() {
    let mut g = graph();
    function_first(&mut g);
    g.add_interface("ifc:seam", "Half a contract")
        .expect("interface");
    g.provides("cmp:a", "ifc:seam").expect("provides");
    // No consumer recorded.

    let p = report(&g);

    assert_eq!(
        band(&p, "seams").ratio,
        Some(0.0),
        "one side is not a declared seam"
    );
    assert_eq!(p.frontier, Some("seams"));
}

/// Assurance counts a check that PASSES, not one that exists
/// (`dec:passing-is-verified`). A planned check is inventory, not evidence.
#[test]
fn assurance_counts_passing_checks_not_existing_ones() {
    let mut g = graph();
    function_first(&mut g);
    g.add_verification("ver:one", "Planned check", None, None, None)
        .expect("verification");
    g.verifies("ver:one", reflow2_core::nodes::node::CAPABILITY, "cap:0")
        .expect("verifies");

    assert_eq!(band(&report(&g), "assurance").ratio, Some(0.0));

    g.set_verification_status("ver:one", "passing", None, None)
        .expect("passing");
    assert_eq!(band(&report(&g), "assurance").ratio, Some(0.25));
}

/// Requirements the user settled OUT are not counted against intent — dropping
/// something is their word too, not a gap in confirmation.
#[test]
fn dropped_requirements_leave_the_intent_population() {
    let mut g = graph();
    function_first(&mut g);
    g.add_requirement("req:gone", "Abandoned", "Not wanted.")
        .expect("requirement");
    g.set_requirement_status("req:gone", "dropped")
        .expect("drop");

    let p = report(&g);

    assert_eq!(
        band(&p, "intent").population,
        4,
        "the dropped requirement leaves the population entirely"
    );
    assert_eq!(band(&p, "intent").ratio, Some(1.0));
}

/// Every band carries the question it answers, so a number is never read
/// without its meaning — and the caveats travel on every profile.
#[test]
fn the_profile_explains_itself() {
    let mut g = graph();
    function_first(&mut g);

    let p = report(&g);

    assert_eq!(p.bands.len(), 7);
    for (i, b) in p.bands.iter().enumerate() {
        assert_eq!(b.order, i + 1, "bands are in trajectory order");
        assert!(!b.question.is_empty(), "{} has no question", b.name);
    }
    assert!(p.not_observed_about.len() >= 4);
}

/// Same design, byte-identical profile — this is meant to be watched over time,
/// and a reading that reorders between runs cannot be diffed.
#[test]
fn the_profile_is_deterministic() {
    let mut g = graph();
    function_first(&mut g);

    let a = serde_json::to_string(&report(&g)).expect("serialize");
    let b = serde_json::to_string(&report(&g)).expect("serialize");
    assert_eq!(a, b);
}
