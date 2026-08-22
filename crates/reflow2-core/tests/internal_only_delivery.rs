//! A stated need that only the project's own machinery delivers.
//!
//! The product form of `rule:reflow2-is-built-for-other-projects-not-for-itself`,
//! serving `req:work-says-whether-it-reaches-a-consumer`. The rule binds the
//! people building reflow2 and can never bind anyone else's project; the
//! requirement is the same philosophy as something reflow2 DOES, so every
//! project inherits the question — the split `req:a-fix-says-whether-it-
//! corrected-the-cause` established.
//!
//! ⭐ THE FAILURE IS UNIVERSAL AND IT LOOKS LIKE COMPLETION: find a hole, patch
//! it in your own tooling, mark the need met, and leave every consumer with the
//! hole. The repo goes green and nothing is true of anybody downstream.
//!
//! ⚠️ NEVER INFERRED FROM A PATH. A first sketch classified artifacts by
//! directory — `.github/` internal, `crates/` shipped — which encodes ONE
//! project's layout and would make this detector useful to exactly one
//! repository. That is the failure the rule behind it forbids, so the audience
//! is DECLARED or it is unknown.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;

fn seeded() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_requirement("req:need", "A need", "Something must be true")
        .expect("req");
    g.add_capability("cap:does", "Does it", "the doing", None)
        .expect("cap");
    g.satisfies("cap:does", "req:need").expect("satisfies");
    g
}

fn artifact(g: &mut DesignGraph, id: &str, audience: Option<&str>) {
    g.add_artifact(id, id, Some("code"), Some(&format!("src/{id}.rs")))
        .expect("artifact");
    g.realizes(id, "Capability", "cap:does", None, None)
        .expect("realizes");
    if let Some(a) = audience {
        g.set_artifact_intent(id, None, None, Some(a))
            .expect("audience");
    }
}

fn finding(g: &DesignGraph) -> Option<reflow2_core::detect::GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|x| x.gap_source == GapSource::InternalOnlyDelivery)
}

/// The case the detector exists for.
#[test]
fn a_need_delivered_only_by_internal_work_is_asked_about() {
    let mut g = seeded();
    artifact(&mut g, "art:ci", Some("internal"));

    let gap = finding(&g).expect(
        "a need whose only delivery serves the project's own machinery must be put to somebody — \
         this is the shape that looks like completion and is not",
    );
    assert!(gap.affected_ids.contains(&"req:need".to_string()));
    assert!(gap.affected_ids.contains(&"art:ci".to_string()));
}

/// One consumer-facing deliverable answers it. The finding is about a need
/// NOTHING reachable delivers, not about the presence of internal work.
#[test]
fn one_consumer_facing_deliverable_settles_it() {
    let mut g = seeded();
    artifact(&mut g, "art:ci", Some("internal"));
    artifact(&mut g, "art:lib", Some("consumer"));

    assert!(
        finding(&g).is_none(),
        "internal work alongside a consumer-facing deliverable is normal and must not be \
         reported — the rule forbids MISTAKING internal work for product work, never doing it"
    );
}

/// ⭐ THE HONESTY CASE, AND IT HAS BEEN WRONG TWICE. Silence must mean "I could
/// not judge", never "clean" — so asserting only that the detector stays quiet
/// would enshrine exactly what `req:work-says-whether-it-reaches-a-consumer`
/// forbids in its own 🛑 clause.
///
/// FIRST ATTEMPT: asserted only the silence. A mutation exposed it — disabling
/// the detector's "nothing to run on" guard changed no result, proving the
/// guard dead and the assertion inert.
///
/// SECOND ATTEMPT: asserted that `vocabulary_coverage` names `Artifact.audience`
/// as unused, which would have made the silence reported somewhere. IT DOES
/// NOT. Its `unused` list names NODE TYPES and EDGE TYPES only; an unused
/// PROPERTY is counted in `properties_on_used_types` and never named. That is
/// principle B in reflow2's own instrument — a set of named things reduced to a
/// scalar — and it was found by writing this assertion and watching it fail.
///
/// 🛑 SO THIS IS RECORDED AS AN HONEST HOLE, NOT AS A PASS. A design that has
/// declared no audience anywhere is SILENT in every surfaced report: no
/// finding, and no report that names the field as never used. The third leg is
/// incomplete for this vocabulary and the test says so rather than pretending
/// otherwise — which is the end state the source document explicitly calls
/// legitimate: "this rule has no detector, and here is why."
#[test]
fn nothing_classified_yields_no_finding_and_nothing_yet_reports_that() {
    let mut g = seeded();
    artifact(&mut g, "art:thing", None);

    assert!(
        finding(&g).is_none(),
        "with nothing classified there is no per-requirement question to ask"
    );

    // Pinned so the hole cannot close silently: the day `vocabulary_coverage`
    // starts naming unused PROPERTIES, this goes red and the comment above —
    // and the detector's own HONEST LIMIT note — must be rewritten.
    let cov = g.vocabulary_coverage(true).expect("coverage");
    let unused = cov.unused.expect("asked for the list");
    assert!(
        !unused.iter().any(|u| u.contains("audience")),
        "vocabulary_coverage has started naming unused properties — the honest-hole note in \
         this test and in detect_internal_only_delivery is now stale and must be corrected: \
         {unused:?}"
    );
}

/// An undeclared artifact is evidence of NEITHER side. It must not silently
/// count as internal, or the finding would assert a claim nobody made.
#[test]
fn an_undeclared_artifact_is_never_counted_as_internal() {
    let mut g = seeded();
    artifact(&mut g, "art:ci", Some("internal"));
    artifact(&mut g, "art:mystery", None);

    let gap = finding(&g).expect("one declared internal, none declared consumer");
    assert!(
        !gap.affected_ids.contains(&"art:mystery".to_string()),
        "an artifact nobody classified must not be named as internal evidence"
    );
    assert!(
        gap.evidence.contains('1') && gap.evidence.contains("no audience"),
        "the undeclared count belongs in the evidence so a reader can weigh it: {}",
        gap.evidence
    );
}

/// A need the user settled OUT is not a need. Dropping something is their word.
#[test]
fn a_dropped_requirement_is_not_asked_about() {
    let mut g = seeded();
    artifact(&mut g, "art:ci", Some("internal"));
    g.set_requirement_status("req:need", "dropped")
        .expect("status");

    assert!(
        finding(&g).is_none(),
        "a requirement the user dropped must stop raising findings, like every other detector"
    );
}

/// An audience outside the enum is refused, naming both legal values — a third
/// value would make every count over this field quietly wrong.
#[test]
fn an_audience_outside_the_enum_is_refused() {
    let mut g = seeded();
    g.add_artifact("art:x", "x", Some("code"), Some("src/x.rs"))
        .expect("artifact");
    let err = g
        .set_artifact_intent("art:x", None, None, Some("customer"))
        .expect_err("'customer' is not 'consumer'");
    let text = format!("{err:?}");
    assert!(
        text.contains("consumer") && text.contains("internal"),
        "the refusal must list what IS allowed: {text}"
    );
}

/// Per-requirement, not aggregate: accepting "this need is internal on purpose"
/// is a claim about ONE need and must not pre-accept the next one.
#[test]
fn two_internal_only_needs_are_two_findings() {
    let mut g = seeded();
    artifact(&mut g, "art:ci", Some("internal"));
    g.add_requirement("req:other", "Another", "Also must be true")
        .expect("req");
    g.add_capability("cap:other", "Other", "the other doing", None)
        .expect("cap");
    g.satisfies("cap:other", "req:other").expect("satisfies");
    g.add_artifact("art:script", "script", Some("code"), Some("src/script.rs"))
        .expect("artifact");
    g.realizes("art:script", "Capability", "cap:other", None, None)
        .expect("realizes");
    g.set_artifact_intent("art:script", None, None, Some("internal"))
        .expect("audience");

    let all: Vec<_> = g
        .detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|x| x.gap_source == GapSource::InternalOnlyDelivery)
        .collect();
    assert_eq!(
        all.len(),
        2,
        "one finding per need, so a judgement about one does not settle the other"
    );
}
