//! The environment compliance layer: a design can say what rules its operating
//! environment imposes, whether it complies, and be asked when it has not said.
//!
//! # What was parked, and why it is being built now
//!
//! `schema/environment.yaml` has declared this since 2026-07-17: an
//! `EnvironmentRule` is a rule the design **cannot negotiate** — a building
//! code, a zoning ordinance, a safety standard, a physical law — as opposed to
//! a `Constraint`, which the design imposes on itself, and a `DesignRule`,
//! which is a convention it chose. Only the third is dictated by the world, and
//! reflow2 had never used it.
//!
//! Anthony parked the compliance half on 2026-08-26 (`decision:vocab:environment`,
//! option (e) of five), on the grounds that parking is the only disposition free
//! to reverse and no user had asked. The parking named its own condition: a real
//! request. On 2026-09-07 he asked — the contract-and-standards brainstorm names
//! "building codes" and "standards", both of which are values in this type's own
//! `rule_type` enum — and then ruled: build the whole leg.
//!
//! # Why the whole leg and not the write side alone
//!
//! Because a write side without a read side is the failure this project has
//! measured repeatedly: `Verification.level` exists and nothing reads it. The
//! deliberation recorded that objection against option (b) before the decision
//! was taken. So this pins all three legs together — the tools that write it,
//! the edges being read, and the two detectors that ask a human.
//!
//! # The scoping choice, stated because it is a judgement
//!
//! `unchecked_compliance` fires **once per mandatory rule nothing has answered**,
//! not once per (element, rule) pair. A design with 234 capabilities and 10 rules
//! would otherwise raise 2,340 findings, which is the hub-shaped noise that got
//! an earlier detector narrowed the same week it shipped. One finding per rule is
//! bounded by the rules a person actually wrote.

use reflow2_core::{DesignGraph, GapSource, nodes::Props, nodes::edge, nodes::node};

fn kennewick() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:house", "A house in Kennewick")
        .expect("project");
    g.add_environment(
        "env:kennewick",
        "Kennewick, WA",
        Some("field"),
        Some("Benton County, Washington"),
    )
    .expect("environment");
    g.upsert_node(
        node::COMPONENT,
        "cmp:roof",
        Props::new()
            .set("name", "Roof structure")
            .set("purpose", "carries snow and wind load"),
    )
    .expect("component");
    g
}

/// A rule the design cannot negotiate, recorded with who issues it and where it
/// applies — the fields that make it auditable rather than a note.
#[test]
fn an_externally_imposed_rule_can_be_recorded_with_its_authority() {
    let mut g = kennewick();
    let stored = g
        .add_environment_rule(
            "envr:snow-load",
            "Ground snow load",
            "Roof structures shall be designed for the ground snow load of the jurisdiction.",
            Some("building_code"),
            Some("City of Kennewick"),
            Some("Kennewick, WA"),
            Some("IBC 2021 §1608"),
            Some(true),
        )
        .expect("rule");
    let p = &stored.properties;
    assert_eq!(
        p.get("rule_type").and_then(|v| v.as_str()),
        Some("building_code")
    );
    assert_eq!(
        p.get("authority").and_then(|v| v.as_str()),
        Some("City of Kennewick")
    );
    assert_eq!(
        p.get("reference").and_then(|v| v.as_str()),
        Some("IBC 2021 §1608"),
        "the citation is what lets a reader check the claim against the source"
    );
    assert_eq!(p.get("mandatory").and_then(|v| v.as_bool()), Some(true));
}

/// The three edges that carry the claim, each drawn by a typed call.
#[test]
fn the_compliance_edges_are_writable_without_the_escape_hatch() {
    let mut g = kennewick();
    g.add_environment_rule(
        "envr:snow-load",
        "Ground snow load",
        "Design for the jurisdiction's ground snow load.",
        Some("building_code"),
        Some("City of Kennewick"),
        Some("Kennewick, WA"),
        Some("IBC 2021 §1608"),
        Some(true),
    )
    .expect("rule");

    g.operates_in("proj:house", "env:kennewick")
        .expect("operates_in");
    g.imposes("env:kennewick", "envr:snow-load")
        .expect("imposes");
    g.complies_with(
        node::COMPONENT,
        "cmp:roof",
        "envr:snow-load",
        Some(true),
        Some("Stamped calc package 2026-09-07, 40 psf."),
    )
    .expect("complies_with");

    assert_eq!(
        g.outgoing("proj:house", Some(edge::OPERATES_IN))
            .expect("read")
            .len(),
        1
    );
    assert_eq!(
        g.outgoing("env:kennewick", Some(edge::IMPOSES))
            .expect("read")
            .len(),
        1
    );
    let c = g
        .outgoing("cmp:roof", Some(edge::COMPLIES_WITH))
        .expect("read");
    assert_eq!(c.len(), 1);
    assert_eq!(
        c[0].properties.get("verified").and_then(|v| v.as_bool()),
        Some(true),
        "compliance DEMONSTRATED must be distinguishable from compliance merely asserted"
    );
}

/// FIRST DETECTOR. A mandatory rule nothing has answered — neither complied
/// with nor violated — is a question for a human, because a design that has not
/// said whether it meets a code has not met it.
#[test]
fn a_mandatory_rule_nobody_has_answered_is_asked_about() {
    let mut g = kennewick();
    g.add_environment_rule(
        "envr:egress",
        "Means of egress",
        "Every sleeping room shall have an emergency escape opening.",
        Some("building_code"),
        Some("City of Kennewick"),
        Some("Kennewick, WA"),
        Some("IRC 2021 §R310"),
        Some(true),
    )
    .expect("rule");
    g.operates_in("proj:house", "env:kennewick")
        .expect("operates_in");
    g.imposes("env:kennewick", "envr:egress").expect("imposes");

    let gaps = g.detect_gaps().expect("gaps");
    let hit = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::UncheckedCompliance)
        .expect("a mandatory rule with no answer is reported");
    assert!(
        hit.affected_ids.iter().any(|i| i == "envr:egress"),
        "the finding must name the rule: {:?}",
        hit.affected_ids
    );

    // Answering it — either way — closes the question.
    g.complies_with(
        node::COMPONENT,
        "cmp:roof",
        "envr:egress",
        Some(false),
        None,
    )
    .expect("complies");
    let after = g.detect_gaps().expect("gaps");
    assert!(
        !after
            .iter()
            .any(|x| x.gap_source == GapSource::UncheckedCompliance),
        "once the design has said something about the rule, it is no longer unanswered"
    );
}

/// An ADVISORY rule is not asked about. Only what the design cannot negotiate
/// carries this obligation.
#[test]
fn an_advisory_rule_is_not_asked_about() {
    let mut g = kennewick();
    g.add_environment_rule(
        "envr:guidance",
        "Preferred cladding",
        "Fiber cement is preferred in this district.",
        Some("standard"),
        Some("Design review board"),
        Some("Kennewick, WA"),
        None,
        Some(false),
    )
    .expect("rule");
    g.operates_in("proj:house", "env:kennewick")
        .expect("operates_in");
    g.imposes("env:kennewick", "envr:guidance")
        .expect("imposes");
    let gaps = g.detect_gaps().expect("gaps");
    assert!(
        !gaps
            .iter()
            .any(|x| x.gap_source == GapSource::UncheckedCompliance),
        "an advisory rule is guidance; asking about it every sweep is how a finding becomes noise"
    );
}

/// SECOND DETECTOR. A violation nobody has triaged is an open question: it is
/// neither an accepted variance nor a defect somebody owns.
#[test]
fn a_violation_nobody_has_triaged_is_asked_about() {
    let mut g = kennewick();
    g.add_environment_rule(
        "envr:setback",
        "Front setback",
        "No structure within 20 feet of the front lot line.",
        Some("zoning"),
        Some("City of Kennewick"),
        Some("Kennewick, WA"),
        Some("KMC 18.12"),
        Some(true),
    )
    .expect("rule");
    g.operates_in("proj:house", "env:kennewick")
        .expect("operates_in");
    g.imposes("env:kennewick", "envr:setback").expect("imposes");
    g.violates_rule(
        node::COMPONENT,
        "cmp:roof",
        "envr:setback",
        None,
        Some("high"),
        Some("The porch overhang sits 18 feet from the lot line."),
    )
    .expect("violation");

    let gaps = g.detect_gaps().expect("gaps");
    let hit = gaps
        .iter()
        .find(|x| x.gap_source == GapSource::OpenViolation)
        .expect("an untriaged violation is reported");
    assert!(hit.affected_ids.iter().any(|i| i == "envr:setback"));

    // A GRANTED VARIANCE closes it. The violation is kept and documented — that
    // is the whole point of the lifecycle, and deleting it would lose the audit.
    g.set_violation_status(
        "cmp:roof",
        "envr:setback",
        "confirmed",
        Some("Variance V-2026-14 granted."),
    )
    .expect("variance");
    let after = g.detect_gaps().expect("gaps");
    assert!(
        !after
            .iter()
            .any(|x| x.gap_source == GapSource::OpenViolation),
        "a triaged violation is settled, whether the answer was a waiver or a fix"
    );
    assert_eq!(
        g.outgoing("cmp:roof", Some(edge::VIOLATES_RULE))
            .expect("read")
            .len(),
        1,
        "and the record of it SURVIVES — a confirmed variance is documented, never deleted"
    );
}
