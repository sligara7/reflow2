//! Four places reflow2 held the answer and the surface withheld it.
//!
//! # What this pins
//!
//! Reported by dev_storyflow across its 2026-08-23 sessions and each verified
//! against the code before it was taken. The four have no subsystem in common.
//! What they share is a shape: **the process knew something, and the reply did
//! not say it**, so the session had to reconstruct by hand what was already
//! computed. That is the same failure as a vacuous zero reading as a pass, one
//! layer over — which is why they ship together.
//!
//! 1. `note` was DECLARED on `CONSTRAINS` and `GOVERNED_BY` and unreachable
//!    from their typed constructors. `describe_schema` advertised a field the
//!    write path refused.
//! 2. `unsatisfied_requirement` asked "is it covered, deferred, or dropped?"
//!    of requirements whose `status` said `deferred`.
//! 3. `rephrase_degraded` was a bare bool; the `LlmError` explaining it was
//!    discarded one line after it was produced.
//! 4. `graph_report_markdown`'s "Top gaps" section emitted whole reports
//!    inline, in the section `where-am-i` reads first.

use reflow2_core::agent::{AgentAnswer, AgentBackend, PromptCollector};
use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, GapSource};

// ---------------------------------------------------------------------------
// 1. A declared field nobody can reach is a declared field nobody writes to.
// ---------------------------------------------------------------------------

#[test]
fn constrains_stores_the_note_its_edge_type_declares() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_constraint(
        "con:mass",
        "Mass budget",
        "Under 100 kg.",
        None,
        Some("mass_kg"),
        Some(100.0),
        None,
        None,
    )
    .unwrap();
    g.add_component("cmp:bus", "Bus", "the bus", None).unwrap();

    let e = g
        .constrains(
            "con:mass",
            node::COMPONENT,
            "cmp:bus",
            Some(40.0),
            Some("measured"),
            Some("2026-08-23"),
            Some("weighed on the bench, not the flight article"),
        )
        .unwrap();

    assert_eq!(
        e.properties.get("note").and_then(|v| v.as_str()),
        Some("weighed on the bench, not the flight article"),
        "the note is the part a later reader needs, and CONSTRAINS declares it"
    );
}

#[test]
fn governed_by_stores_the_note_its_edge_type_declares() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:a", "A", "does a thing", None)
        .unwrap();
    g.add_decision("dec:d", "The decision", "We chose this.", None)
        .unwrap();

    let e = g
        .governed_by(
            node::CAPABILITY,
            "cap:a",
            node::DECISION,
            "dec:d",
            None,
            Some("binds because the decision names this capability by id"),
        )
        .unwrap();

    assert_eq!(
        e.properties.get("note").and_then(|v| v.as_str()),
        Some("binds because the decision names this capability by id"),
        "GOVERNED_BY declares `note`; the constructor could not reach it until now"
    );
}

// ---------------------------------------------------------------------------
// 2. A `deferred` requirement already answers the ordinary question.
// ---------------------------------------------------------------------------

/// One requirement, one capability so the detector switches on, and no
/// SATISFIES between them.
fn one_unsatisfied_requirement(g: &mut DesignGraph, status: &str) {
    g.add_requirement("req:r", "The thing holds", "It must hold.")
        .unwrap();
    g.set_requirement_status("req:r", status).unwrap();
    g.add_capability("cap:unrelated", "Unrelated", "elsewhere", None)
        .unwrap();
}

fn unsatisfied_gap(g: &DesignGraph) -> reflow2_core::GapCandidate {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|gap| {
            gap.gap_source == GapSource::UnsatisfiedRequirement
                && gap.affected_ids.iter().any(|id| id == "req:r")
        })
        .expect("the requirement should still raise its gap")
}

#[test]
fn a_deferred_requirement_is_asked_whether_it_is_still_parked() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g, "deferred");
    let gap = unsatisfied_gap(&g);

    // NOT SILENCED. `dropped` and `met` are finished; `deferred` is postponed,
    // and parking is a decision that expires. Dropping the row would make live
    // intent go quiet (`req:no-idea-goes-quiet`).
    assert!(
        gap.description.contains("still deferred") || gap.description.contains("time to schedule"),
        "a parked requirement should be asked whether the parking still holds, \
         not asked the question its own status answers; got: {}",
        gap.description
    );
    assert!(
        !gap.description
            .contains("is it covered, deferred, or dropped?"),
        "the generic wording asks a deferred requirement what it already says"
    );
    assert!(
        gap.evidence.contains("status=deferred"),
        "the status was the invisible fact; evidence should name it. got: {}",
        gap.evidence
    );
}

#[test]
fn an_accepted_requirement_keeps_the_original_question_and_outranks_a_parked_one() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g, "accepted");
    let accepted = unsatisfied_gap(&g);
    assert!(
        accepted
            .description
            .contains("is it covered, deferred, or dropped?"),
        "the ordinary case is unchanged"
    );

    let mut g2 = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g2, "deferred");
    let parked = unsatisfied_gap(&g2);

    assert!(
        parked.severity < accepted.severity,
        "a parked requirement is less urgent than a live unsatisfied one: \
         parked={} accepted={}",
        parked.severity,
        accepted.severity
    );
    assert_eq!(
        parked.id, accepted.id,
        "the id is a hash of source + affected ids, so an existing \
         acknowledgement must survive the rewording"
    );
}

// ---------------------------------------------------------------------------
// 3. The flag said a fallback happened; the reason was thrown away.
// ---------------------------------------------------------------------------

#[test]
fn a_degraded_rephrase_says_why_and_strands_the_answer_that_did_not_match() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g, "accepted");
    let gap = unsatisfied_gap(&g);

    // Prepare pass: collect the prompt id the serve pass will ask for.
    let collector = PromptCollector::new();
    let _ = gap.to_prompt(&collector);
    let prepared = collector.collected();
    assert!(!prepared.is_empty(), "the prepare pass issues a prompt");

    // Serve pass with an answer keyed to an id that is not the one asked for —
    // exactly what an EDITED gap payload produces, since the id is a hash of
    // the prompt text and the prompt text is built from the gap's own prose.
    let backend = AgentBackend::from_answers([AgentAnswer {
        id: "an-id-from-a-gap-that-was-trimmed".to_string(),
        text: "a perfectly good answer to a question nobody asked".to_string(),
    }]);
    let prompt = gap.to_prompt(&backend);

    assert!(
        prompt.rephrase_degraded,
        "the mismatch degrades the phrasing"
    );
    let reason = prompt
        .degrade_reason
        .as_deref()
        .expect("a degraded prompt must say WHY — the flag alone is unactionable");
    assert!(
        reason.contains("desync") || reason.contains("no ambient-agent answer"),
        "the reason should name the prepare/serve desync; got: {reason}"
    );

    // The cheapest signal of all, and it had no caller anywhere in the MCP
    // layer: the answer that matched nothing comes back by name.
    let unused = backend.unused_answers();
    assert_eq!(
        unused,
        vec!["an-id-from-a-gap-that-was-trimmed".to_string()],
        "an answer nobody asked for is a one-read diagnosis of the desync"
    );
}

#[test]
fn a_matched_answer_degrades_nothing_and_strands_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g, "accepted");
    let gap = unsatisfied_gap(&g);

    let collector = PromptCollector::new();
    let _ = gap.to_prompt(&collector);
    let id = collector.collected()[0].id.clone();

    let backend = AgentBackend::from_answers([AgentAnswer {
        id,
        text: "Is anything actually delivering this?".to_string(),
    }]);
    let prompt = gap.to_prompt(&backend);

    assert!(!prompt.rephrase_degraded);
    assert!(
        prompt.degrade_reason.is_none(),
        "no reason on a clean pass — the field exists to explain a failure"
    );
    assert!(backend.unused_answers().is_empty());
}

// ---------------------------------------------------------------------------
// 4. The section `where-am-i` reads first was the most expensive thing in it.
// ---------------------------------------------------------------------------

#[test]
fn the_markdown_top_gaps_section_cuts_long_prose_and_announces_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // A requirement whose NAME is a report — which is how gap titles get long:
    // a gap inherits the wording of the node it fired on.
    let essay = (0..200)
        .map(|i| format!("word{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    g.create_node(
        node::REQUIREMENT,
        "req:essay",
        Props::new()
            .set("name", essay.as_str())
            .set("statement", essay.as_str())
            .set("status", "accepted"),
    )
    .unwrap();
    g.add_capability("cap:unrelated", "Unrelated", "elsewhere", None)
        .unwrap();

    let md = g.graph_report().unwrap().to_markdown();
    let top = md
        .split("## Top gaps (look here first)")
        .nth(1)
        .expect("the section exists");

    assert!(
        top.contains('…'),
        "long gap prose should be cut, not emitted whole"
    );
    assert!(
        top.contains("CUT SHORT"),
        "a silent cut reads as the whole text — announcing it is the other half"
    );
    assert!(
        top.contains("detect_gaps"),
        "say where the full text still lives"
    );
    assert!(
        !top.contains("word199"),
        "the tail of a 200-word title must not reach the roll-up"
    );
}

/// The complement, so the truncation cannot pass by always firing: an ordinary
/// design says nothing about cutting.
#[test]
fn a_short_gap_is_not_announced_as_truncated() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    one_unsatisfied_requirement(&mut g, "accepted");
    // This design says what it is BUILT FOR, which is what makes it "ordinary"
    // now. `quality_target_unstated` fires on any design that has not, and its
    // prose is far past the 40-word roll-up budget — so without this the
    // truncation notice is always present and the counterweight below could
    // never fail. Settled and governing something, because an accepted Decision
    // that governs nothing is a structural defect in its own right.
    g.add_decision(
        "dec:built-for-testability",
        "Built for testability",
        "Every part must be checkable on its own.",
        None,
    )
    .unwrap();
    g.set_quality_target("dec:built-for-testability", "testability")
        .unwrap();
    g.set_decision_status("dec:built-for-testability", "accepted")
        .unwrap();
    g.governed_by(
        node::CAPABILITY,
        "cap:unrelated",
        node::DECISION,
        "dec:built-for-testability",
        None,
        Some("Shaped by the testability trade."),
    )
    .unwrap();
    let md = g.graph_report().unwrap().to_markdown();
    if let Some(top) = md.split("## Top gaps (look here first)").nth(1) {
        assert!(
            !top.contains("CUT SHORT"),
            "nothing was long enough to cut, so nothing should claim it was"
        );
    }
}

// Keep the edge import used: the report test above builds no edges, but the
// gap helpers rely on SATISFIES being absent rather than removed.
#[allow(dead_code)]
const _SATISFIES: &str = edge::SATISFIES;
