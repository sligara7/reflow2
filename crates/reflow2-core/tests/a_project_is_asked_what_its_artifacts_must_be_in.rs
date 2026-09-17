//! `artifact_standard_undeclared` and `artifact_not_under_declared_standard`
//! — a project is asked, from the very beginning, what standard its design
//! artifacts must be in, and a drawing that does not say is named.
//!
//! Why from the beginning: bhome, 2026-08-31 — an agent drew a house plan
//! sheet in HTML because nothing in the design had asked what a plan sheet
//! must be, and the person who would have caught it is a city planner at
//! $250 an hour. Anthony, 2026-09-16: "this needs to be a question from the
//! very beginning for any domain that reflow2 is used"
//! (`req:a-project-names-its-domains-artifact-standard-from-the-beginning-and-its-absence-is-reported`).
//!
//! reflow2 opens no file. It asks by DECLARED format only; whether an IFC
//! file is valid IFC is the standard's `checker`, run outside, and its output
//! is the evidence on the compliance edge.
//!
//! The case that carries the weight is the third: "none known" is a real
//! answer and must not become "never asked again" the day a drawing appears.

use reflow2_core::foundation::core::Value;
use reflow2_core::{DesignGraph, GapCandidate, GapScope, GapSource};

fn house() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:house", "Seth's houses").unwrap();
    g
}

fn undeclared(g: &DesignGraph) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::ArtifactStandardUndeclared)
}

fn not_under(g: &DesignGraph) -> Option<GapCandidate> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .find(|x| x.gap_source == GapSource::ArtifactNotUnderDeclaredStandard)
}

fn declare_ifc(g: &mut DesignGraph) {
    g.add_environment_rule(
        "envrule:ifc4",
        "IFC 4.3",
        "Building models are exchanged as IFC.",
        Some("standard"),
        Some("buildingSMART"),
        None,
        Some("ISO 16739-1:2024"),
        Some("ifcopenshell validate"),
        None,
    )
    .unwrap();
    g.complies_with("Project", "prj:house", "envrule:ifc4", None, None)
        .unwrap();
}

fn sheet(g: &mut DesignGraph) {
    g.add_artifact(
        "art:sheet-01",
        "Plan sheet 01",
        Some("drawing"),
        Some("docs/drawings/plan-sheet-01.html"),
    )
    .unwrap();
}

#[test]
fn a_project_with_nothing_in_it_yet_is_asked() {
    let g = house();
    let gap = undeclared(&g).expect("asked from the very beginning, before any artifact exists");
    assert_eq!(gap.scope, GapScope::Project);
    assert_eq!(gap.affected_ids, vec!["prj:house".to_string()]);
    assert!(
        gap.severity < 0.5,
        "nothing is at stake yet, so it is a low question: {}",
        gap.severity
    );
    assert!(
        not_under(&g).is_none(),
        "the per-artifact question has no standard to ask against"
    );
}

#[test]
fn declaring_the_standard_ends_the_question() {
    let mut g = house();
    declare_ifc(&mut g);
    assert!(undeclared(&g).is_none(), "the project said: IFC");
    let rule = g
        .get_node("EnvironmentRule", "envrule:ifc4")
        .unwrap()
        .unwrap();
    assert_eq!(
        rule.properties.get("checker").and_then(Value::as_str),
        Some("ifcopenshell validate"),
        "the tool that checks a file is part of the declaration"
    );
}

#[test]
fn none_known_is_a_real_answer_until_a_drawing_appears() {
    let mut g = house();
    g.add_artifact("art:main", "main.rs", Some("code"), Some("src/main.rs"))
        .unwrap();
    let gap = undeclared(&g).expect("asked");
    g.acknowledge_gap(
        &gap.id,
        &gap.affected_ids,
        "Software: the code is the artifact; there is no recognized standard to declare.",
    )
    .unwrap();
    assert!(undeclared(&g).is_none(), "none known, on the record");

    g.add_artifact("art:lib", "lib.rs", Some("code"), Some("src/lib.rs"))
        .unwrap();
    assert!(
        undeclared(&g).is_none(),
        "more code changes nothing — the answer was about this kind of project"
    );

    sheet(&mut g);
    let again = undeclared(&g)
        .expect("a drawing appeared in a project that said none known — asked once more");
    assert_ne!(
        again.id, gap.id,
        "a different question, so the old acknowledgement does not cover it"
    );
    assert!(
        again.severity > gap.severity,
        "now something is at stake: {} vs {}",
        again.severity,
        gap.severity
    );
    assert!(
        again.title.contains("1 drawing/model"),
        "got: {}",
        again.title
    );
}

#[test]
fn a_drawing_that_does_not_say_it_is_in_the_standard_is_named() {
    let mut g = house();
    declare_ifc(&mut g);
    sheet(&mut g);
    g.add_artifact(
        "art:house-scad",
        "house.scad",
        Some("code"),
        Some("cad/house.scad"),
    )
    .unwrap();

    let gap = not_under(&g).expect("the sheet does not say whether it is IFC");
    assert!(gap.affected_ids.contains(&"art:sheet-01".to_string()));
    assert!(
        !gap.affected_ids.contains(&"art:house-scad".to_string()),
        "a source that generates the drawing is not asked — only drawings and models"
    );
    assert!(gap.title.contains("IFC 4.3"), "got: {}", gap.title);
    assert!(
        undeclared(&g).is_none(),
        "the project-level question is answered; this is the per-file one"
    );

    g.complies_with(
        "Artifact",
        "art:sheet-01",
        "envrule:ifc4",
        Some(true),
        Some("ifcopenshell validate: 0 errors"),
    )
    .unwrap();
    assert!(
        not_under(&g).is_none(),
        "the sheet says, with the checker's output"
    );
}

#[test]
fn a_drawing_flagged_as_outside_the_standard_has_answered() {
    let mut g = house();
    declare_ifc(&mut g);
    sheet(&mut g);
    g.violates_rule(
        "Artifact",
        "art:sheet-01",
        "envrule:ifc4",
        None,
        Some("medium"),
        Some("A hand-authored HTML sheet; not an IFC model and never will be."),
    )
    .unwrap();
    assert!(
        not_under(&g).is_none(),
        "saying what it is instead is an answer; the open violation is its own finding"
    );
}

#[test]
fn a_new_drawing_re_asks_and_an_acknowledged_set_stays_acknowledged() {
    let mut g = house();
    declare_ifc(&mut g);
    sheet(&mut g);
    let first = not_under(&g).unwrap();
    g.acknowledge_gap(
        &first.id,
        &first.affected_ids,
        "Sheet 01 is a mock-up for the family, not a submission.",
    )
    .unwrap();
    assert!(not_under(&g).is_none());

    g.add_artifact(
        "art:sheet-02",
        "Plan sheet 02",
        Some("drawing"),
        Some("docs/drawings/plan-sheet-02.html"),
    )
    .unwrap();
    let second = not_under(&g).expect("a new drawing is a new question");
    assert_ne!(second.id, first.id);
    assert!(second.affected_ids.contains(&"art:sheet-02".to_string()));
}

#[test]
fn a_mirror_of_somebody_elses_design_is_not_asked() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let props: std::collections::HashMap<String, Value> = [
        ("name".to_string(), Value::from("Their design")),
        ("mirror_of".to_string(), Value::from("theirs")),
    ]
    .into_iter()
    .collect();
    g.upsert_node("Project", "prj:theirs", props).unwrap();
    assert!(
        undeclared(&g).is_none(),
        "a mirror is somebody else's design as received; their standard is theirs to declare"
    );
}
