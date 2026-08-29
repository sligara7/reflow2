//! A declared node reference that resolves to nothing is reported on its own
//! terms — whatever else the node is attached to.
//!
//! `fact:a-dangling-reference-is-only-detected-when-the-node-is-also-edgeless`,
//! measured on reflow2's own graph 2026-08-29 while repairing the nine dangling
//! `TemporalFact.subject_id` values #366 left for "detection over the graph as
//! it stands":
//!
//!     edges on the fact    reported by detect_defects?
//!     2, 1                 NO   (2 of 9)
//!     0  (x7)              YES  (7 of 9)
//!
//! Seven surfaced, two did not, and the two silent ones were EXACTLY the two
//! that carried edges. The cause is a conjunction rather than a coverage gap:
//! `orphan_node` fires when a subject "resolves to no node AND it carries no
//! edge either", so dangling-reference detection was a side effect of a rule
//! about ATTACHMENT and inherited that rule's precondition.
//!
//! ⚠️ WHY WIDENING THE OTHER RULE WAS THE WRONG FIX. The bias ran backwards. An
//! edgeless node is by construction the LEAST consequential place for a broken
//! pointer, because nothing else reaches it; a node with edges is embedded in
//! the design and its pointer is read down more paths. The old behaviour was
//! most reliable where it mattered least — and it LOOKED complete, having
//! produced seven correct findings on the very graph where it missed two.
//!
//! 🛑 THIS IS THE READ SIDE ONLY. #366's `create_node` guard refuses a dangling
//! value at WRITE time, which is why every fixture below must create its target
//! and then delete it: an unresolvable reference cannot be written directly.
//! That is also the honest account of how real ones arise — a target retired or
//! renamed after the fact was written.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

/// A fact about a capability, with the capability then removed — the only route
/// to a dangling reference now that the write guard exists.
///
/// `attached` decides whether the fact carries an edge, which is the whole
/// variable this file is about.
fn fact_pointing_at_a_deleted_capability(attached: bool) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.create_node(
        "Capability",
        "cap:doomed",
        Props::new()
            .set("name", "Doomed")
            .set("description", "A capability that gets retired"),
    )
    .unwrap();
    g.create_node(
        "TemporalFact",
        "fact:about-the-doomed-one",
        Props::new()
            .set("subject_id", "cap:doomed")
            .set("statement", "a measurement about a capability"),
    )
    .unwrap();
    if attached {
        g.create_node(
            "Decision",
            "dec:somewhere-to-point",
            Props::new()
                .set("name", "A decision the fact hangs off")
                .set("decision", "something was decided")
                .set("rationale", "so the fact has an edge"),
        )
        .unwrap();
        g.create_edge(
            edge::HAS_TEMPORAL_FACT,
            node::DECISION,
            "dec:somewhere-to-point",
            node::TEMPORAL_FACT,
            "fact:about-the-doomed-one",
            Props::new(),
        )
        .unwrap();
    }
    g.delete_node("Capability", "cap:doomed").unwrap();
    g
}

fn findings(g: &DesignGraph, category: HealCategory) -> Vec<String> {
    g.open_defects()
        .unwrap()
        .into_iter()
        .filter(|d| d.category == category)
        .flat_map(|d| d.affected_ids)
        .collect()
}

/// THE REGRESSION. This is the case that was silent — 2 of the 9 measured — and
/// it is the reason the rule exists.
#[test]
fn a_dangling_reference_on_an_attached_node_is_reported() {
    let g = fact_pointing_at_a_deleted_capability(true);

    // THE PRE-STATE, ASSERTED RATHER THAN DESCRIBED. This test cannot be run
    // against the old build — `HealCategory::DanglingReference` did not exist,
    // so it would not compile — and a test that only ever passes proves nothing
    // about the bug it names. What CAN be pinned is the condition that made the
    // rule necessary: `orphan_node` does not report this node, and never did,
    // because the fact carries an edge and that rule requires degree zero. If
    // this assertion ever fails, the two rules have started overlapping and the
    // de-duplication below needs rethinking.
    assert!(
        !findings(&g, HealCategory::OrphanNode).contains(&"fact:about-the-doomed-one".to_string()),
        "the attached case is invisible to orphan_node — that absence is the defect this rule \
         was added to close, and it is what makes the assertion below meaningful"
    );

    assert!(
        findings(&g, HealCategory::DanglingReference)
            .contains(&"fact:about-the-doomed-one".to_string()),
        "a fact whose subject was deleted must be reported even though it carries an edge — \
         this is the case the old orphan rule could not see"
    );
}

/// The case that DID work before, and must keep working.
#[test]
fn a_dangling_reference_on_an_isolated_node_is_still_reported() {
    let g = fact_pointing_at_a_deleted_capability(false);
    assert!(
        findings(&g, HealCategory::DanglingReference)
            .contains(&"fact:about-the-doomed-one".to_string()),
        "the edgeless case must not regress while the attached case is fixed"
    );
}

/// ONE DEFECT, ONE FINDING. The edgeless case satisfies BOTH rules' conditions,
/// and reporting it twice in two vocabularies — with two different repairs
/// suggested for one fault — is the duplication BL-176 measured a field reporter
/// stopping work over.
#[test]
fn an_isolated_dangling_node_is_not_also_reported_as_an_orphan() {
    let g = fact_pointing_at_a_deleted_capability(false);
    assert!(
        !findings(&g, HealCategory::OrphanNode).contains(&"fact:about-the-doomed-one".to_string()),
        "the pointer case now belongs to dangling_reference; orphan_node must not repeat it"
    );
}

/// The counterweight that keeps every assertion above meaningful: a reference
/// that RESOLVES is not a finding, however isolated the node is otherwise.
#[test]
fn a_resolving_reference_is_not_a_finding() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    g.create_node(
        "Capability",
        "cap:alive",
        Props::new()
            .set("name", "Alive")
            .set("description", "A capability that stays"),
    )
    .unwrap();
    g.create_node(
        "TemporalFact",
        "fact:about-the-live-one",
        Props::new()
            .set("subject_id", "cap:alive")
            .set("statement", "a measurement about a capability that exists"),
    )
    .unwrap();
    assert!(
        findings(&g, HealCategory::DanglingReference).is_empty(),
        "a reference that resolves is not a defect, and a rule that fired here would make \
         correct work cost a warning"
    );
}

/// ⚠️ PARKING IS ABOUT ATTACHMENT, AND A BROKEN POINTER IS NOT AN ATTACHMENT
/// STATE. `GOVERNED_BY ruling: parks` lets a design say a node is deliberately
/// linked to nothing — something a person can genuinely hold. Nobody can decide
/// that a record is ABOUT a node the design does not have, so a park must not
/// silence this. Asserted rather than left to be discovered, because the same
/// ruling DOES silence `orphan_node` one rule over.
#[test]
fn a_park_does_not_silence_a_broken_pointer() {
    let mut g = fact_pointing_at_a_deleted_capability(false);
    g.create_node(
        "Decision",
        "dec:this-fact-is-deliberately-loose",
        Props::new()
            .set("name", "The fact is deliberately attached to nothing")
            .set("decision", "this fact is deliberately linked to nothing")
            .set(
                "rationale",
                "to prove a park cannot silence a broken pointer",
            ),
    )
    .unwrap();
    g.set_decision_status("dec:this-fact-is-deliberately-loose", "accepted")
        .unwrap();
    g.create_edge(
        edge::GOVERNED_BY,
        node::TEMPORAL_FACT,
        "fact:about-the-doomed-one",
        node::DECISION,
        "dec:this-fact-is-deliberately-loose",
        Props::new().set("ruling", "parks"),
    )
    .unwrap();

    assert!(
        findings(&g, HealCategory::DanglingReference)
            .contains(&"fact:about-the-doomed-one".to_string()),
        "a park says 'attached to nothing on purpose'; it cannot say 'about a node that does \
         not exist on purpose', so it must not suppress this rule"
    );
}

/// THE SELF-LIMITING PROPERTY. The population is declared REFERENCES, not nodes,
/// so a rule that inspected almost nothing cannot report a large sweep — and a
/// graph with no references at all reports zero, which the sweep's existing
/// starved-rule note then says out loud.
#[test]
fn the_population_counts_references_rather_than_nodes() {
    let mut g = DesignGraph::open_in_memory().expect("in-memory graph");
    for i in 0..4 {
        g.create_node(
            "Capability",
            &format!("cap:{i}"),
            Props::new()
                .set("name", format!("Capability {i}"))
                .set("description", "a capability with no reference property"),
        )
        .unwrap();
    }
    let examined = |g: &DesignGraph| {
        g.detect_defects()
            .unwrap()
            .swept
            .rule_populations
            .into_iter()
            .find(|r| r.rule == HealCategory::DanglingReference.as_str())
            .expect("the rule reports its population")
            .examined
    };

    assert_eq!(
        examined(&g),
        0,
        "four nodes and no node-reference property between them must examine ZERO references — \
         reporting 4 would be the overstating instrument this rule was added to stop being"
    );

    g.create_node(
        "TemporalFact",
        "fact:one",
        Props::new()
            .set("subject_id", "cap:0")
            .set("statement", "one reference"),
    )
    .unwrap();
    assert_eq!(examined(&g), 1, "one declared reference now exists");
}

/// The finding must NAME the property and the unresolvable value. A message
/// saying only "this node has a bad reference" leaves the reader to find which
/// of its properties, on a node type that may declare several.
#[test]
fn the_finding_names_the_property_and_the_missing_target() {
    let g = fact_pointing_at_a_deleted_capability(true);
    let message = g
        .open_defects()
        .unwrap()
        .into_iter()
        .find(|d| d.category == HealCategory::DanglingReference)
        .expect("the finding exists")
        .message;

    assert!(
        message.contains("subject_id"),
        "the message must name the property: {message}"
    );
    assert!(
        message.contains("cap:doomed"),
        "the message must name the id that resolves to nothing: {message}"
    );
}

/// No repair is proposed, and the reason is stated. Guessing the intended target
/// would fabricate the same class of false record the rule exists to find.
#[test]
fn no_repair_is_proposed_and_the_finding_says_why() {
    let g = fact_pointing_at_a_deleted_capability(true);
    let issue = g
        .open_defects()
        .unwrap()
        .into_iter()
        .find(|d| d.category == HealCategory::DanglingReference)
        .expect("the finding exists");

    assert!(
        issue.suggested_fix_type.is_none(),
        "there is no mechanical repair for a broken pointer"
    );
    assert!(
        issue.repair_is_a_judgement.is_some(),
        "and the finding must say so rather than leaving the absence to be inferred"
    );
}
