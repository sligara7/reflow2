//! A field report can be registered, parked, and never triaged — and until now
//! nothing could tell that apart from one that was read carefully.
//!
//! # The rule, and the leg it was missing
//!
//! `rule:field-feedback-issues-are-root-caused-and-ideas-are-brainstormed`
//! (Anthony, 2026-09-05) says every issue a field report names gets a
//! root-cause pass and every idea gets a brainstorm pass. It named its own
//! compliance signal from the day it was written: *"a parked field-report
//! artifact with no CAUSES edge and no exploratory Decision dated after it"*.
//!
//! Nobody built the detector, and the interesting part is why not. Measured on
//! reflow2's own graph 2026-09-09: the CAUSES edge existed in the vocabulary
//! the whole time and had been drawn **once across seventeen field reports**.
//! So the missing leg was never the detector — the INSTRUCTION never told a
//! triage to draw the edge back, compliance left no trace, and a detector over
//! a signal nobody emits would have reported every report as untriaged,
//! including the ones somebody had spent a day on. The 2026-09-09 triage drew
//! its own back-edges first; only then was there a true negative to tell from
//! the true positives.
//!
//! # ⚠️ The ruling this detector must not outrun
//!
//! Anthony ruled the rule **advisory** on 2026-09-09 when the question was put
//! to him. So this reports at `Info` and blocks nothing, and
//! `the_finding_never_blocks` is what holds it there. A detector may be
//! stricter than nothing; it must not be stricter than the rule it checks.

use reflow2_core::{DesignGraph, nodes::Props, nodes::edge, nodes::node};

/// A design holding one methodology rule and one report parked under it —
/// the shape a registered-but-untriaged field report actually has.
fn a_parked_report() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "A project").expect("project");
    g.add_design_rule(
        "rule:triage",
        "Field feedback is processed by skill",
        "Every issue gets a root-cause pass and every idea a brainstorm pass.",
        Some("methodology"),
        Some(false),
    )
    .expect("rule");
    g.add_artifact(
        "art:report-2026-09-08",
        "A field report",
        Some("document"),
        Some("docs/feedback/2026-09-08.md"),
    )
    .expect("artifact");
    g.create_edge(
        edge::GOVERNED_BY,
        node::ARTIFACT,
        "art:report-2026-09-08",
        node::DESIGN_RULE,
        "rule:triage",
        Props::new(),
    )
    .expect("governed");
    // ...AND parked, which is how a dated report is registered: an accepted
    // Decision ruling that this node deliberately hangs off nothing. Both edges
    // together are the real shape — see
    // `the_parking_that_registers_it_does_not_hide_it` for why the second one
    // must not silence the first.
    g.create_node(
        node::DECISION,
        "dec:a-dated-report-is-registered-and-parked",
        Props::new()
            .set("name", "A dated field report is registered and parked")
            .set("decision", "Register it, park it, and do not thread it.")
            .set("status", "accepted"),
    )
    .expect("parking decision");
    g.create_edge(
        edge::GOVERNED_BY,
        node::ARTIFACT,
        "art:report-2026-09-08",
        node::DECISION,
        "dec:a-dated-report-is-registered-and-parked",
        Props::new().set("ruling", "parks"),
    )
    .expect("parked");
    g
}

fn untriaged(g: &DesignGraph) -> Vec<String> {
    g.open_defects()
        .expect("defects")
        .into_iter()
        .filter(|d| d.category.as_str() == "untriaged_report")
        .flat_map(|d| d.affected_ids)
        .collect()
}

#[test]
fn a_report_that_produced_no_finding_is_reported() {
    let g = a_parked_report();
    assert_eq!(
        untriaged(&g),
        vec!["art:report-2026-09-08".to_string()],
        "a document parked under a methodology rule with nothing hanging off it \
         is the exact shape of a report nobody read"
    );
}

#[test]
fn the_edge_that_records_the_reading_clears_it() {
    // THE TRUE NEGATIVE, and the half that could not exist before 2026-09-09:
    // a report that DID produce a finding must come back clean, or the detector
    // is measuring nothing but the shape of a report.
    let mut g = a_parked_report();
    g.create_node(
        node::TEMPORAL_FACT,
        "fact:what-the-report-caused",
        Props::new()
            .set("statement", "The cause the report named")
            .set("subject_id", "art:report-2026-09-08"),
    )
    .expect("fact");
    g.create_edge(
        edge::CAUSES,
        node::ARTIFACT,
        "art:report-2026-09-08",
        node::TEMPORAL_FACT,
        "fact:what-the-report-caused",
        Props::new().set("evidence", "The root-cause pass this rule requires."),
    )
    .expect("caused");
    assert!(
        untriaged(&g).is_empty(),
        "a report carrying the compliance edge the rule names is triaged, and \
         reporting it anyway would make the detector unsilenceable"
    );
}

#[test]
fn the_parking_that_registers_it_does_not_hide_it() {
    // ⭐ THE LOAD-BEARING COUNTERWEIGHT. `GOVERNED_BY ruling: parks` is how a
    // dated report gets registered, so EVERY field report in a real graph is
    // parked. A detector that honoured parking here would report zero on every
    // graph that has reports at all — and a detector reporting zero because it
    // had nothing to run on reads exactly like one that ran clean.
    //
    // Parking is a ruling about ATTACHMENT: this node deliberately hangs off
    // the golden thread. It says nothing about whether anybody read the thing.
    let g = a_parked_report();
    let sweep = g.detect_defects().expect("sweep");
    assert!(
        sweep
            .swept
            .parked
            .iter()
            .any(|p| p.contains("art:report-2026-09-08")),
        "the fixture must actually be parked, or this test proves nothing: {:?}",
        sweep.swept.parked
    );
    assert_eq!(
        untriaged(&g),
        vec!["art:report-2026-09-08".to_string()],
        "parking suppressed the finding, so no real field report would ever be reported"
    );
}

#[test]
fn an_ordinary_document_is_not_a_report() {
    // The predicate is typed, not textual: no id substring, no path prefix.
    // An Artifact in the same directory that nobody governs by a methodology
    // rule is just a file, and matching on its NAME would make this detector
    // work on reflow2's own repository and nobody else's.
    let mut g = a_parked_report();
    g.add_artifact(
        "art:feedback-notes",
        "Notes that mention feedback",
        Some("document"),
        Some("docs/feedback/notes.md"),
    )
    .expect("artifact");
    let found = untriaged(&g);
    assert!(
        !found.contains(&"art:feedback-notes".to_string()),
        "an ungoverned document was reported, so the predicate is matching words: {found:?}"
    );
}

#[test]
fn a_rule_of_another_category_does_not_make_a_document_a_report() {
    // `methodology` is doing real work in the predicate. A convention or a
    // tech-stack rule governing a document says nothing about triage, and
    // widening to "any DesignRule" would sweep in unrelated governed nodes —
    // measured on reflow2's graph 2026-09-09, dropping the category takes the
    // selection from 4 Artifacts to 31 nodes of 8 types.
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("proj:p", "A project").expect("project");
    g.add_design_rule(
        "rule:style",
        "Documents are written in British English",
        "Spelling follows OED.",
        Some("convention"),
        Some(false),
    )
    .expect("rule");
    g.add_artifact("art:doc", "A document", Some("document"), None)
        .expect("artifact");
    g.create_edge(
        edge::GOVERNED_BY,
        node::ARTIFACT,
        "art:doc",
        node::DESIGN_RULE,
        "rule:style",
        Props::new(),
    )
    .expect("governed");
    assert!(
        untriaged(&g).is_empty(),
        "a spelling convention was read as a triage obligation"
    );
}

#[test]
fn the_finding_never_blocks() {
    // The ruling of 2026-09-09 is advisory. Severity is where that ruling is
    // actually enforced, and it is the one property of this detector a later
    // change is most likely to raise without noticing it is overruling somebody.
    let g = a_parked_report();
    let issue = g
        .open_defects()
        .expect("defects")
        .into_iter()
        .find(|d| d.category.as_str() == "untriaged_report")
        .expect("the finding");
    assert_eq!(
        format!("{:?}", issue.severity),
        "Info",
        "the rule this checks is ADVISORY by Anthony's ruling; a warning or a \
         critical here would be the detector overruling the person who made it"
    );
    assert!(
        issue.repair_is_a_judgement.is_some(),
        "a mechanically drawn CAUSES edge would assert that the document was read, \
         which is the one thing the edge is evidence for"
    );
}
