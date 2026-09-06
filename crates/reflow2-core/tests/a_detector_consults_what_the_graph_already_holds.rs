//! Three detectors asked questions the graph had already answered
//! (dev_storyflow 2026-08-23, qs 2026-09-04):
//!  - (`unsatisfied_requirement` on `deferred` requirements turned out to be
//!    ALREADY handled — a deferred requirement is asked a different, lower-ranked
//!    question; see the_surface_says_what_it_already_knew.rs. A first draft of
//!    this file silenced it and the existing pins caught that.)
//!  - `unstated_rule_enforcement` fired on DesignRules an accepted Decision had
//!    OBSOLETED — only the three capability detectors consulted `discontinued`.
//!  - one missing `allocate` edge arrived as separate findings with no thread
//!    between them.
//!
//! Written before the fixes and observed failing (compile: `group_shared_causes`
//! did not exist; behaviour: the withdrawn-rule case asserted a gap that still
//! fired).
use reflow2_core::DesignGraph;
use reflow2_core::detect::{GapCandidate, GapScope, GapSource};
use reflow2_core::foundation::core::Value;

fn g() -> DesignGraph {
    DesignGraph::open_in_memory().expect("graph")
}
fn props(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect()
}

#[test]
fn a_withdrawn_design_rule_no_longer_raises_unstated_enforcement() {
    let mut g = g();
    g.add_project("prj:t", "T").expect("project");
    g.upsert_node(
        "DesignRule",
        "rule:live",
        props(&[
            ("name", "Live"),
            ("statement", "Always X."),
            ("category", "methodology"),
        ]),
    )
    .unwrap();
    g.upsert_node(
        "DesignRule",
        "rule:gone",
        props(&[
            ("name", "Gone"),
            ("statement", "Used to Y."),
            ("category", "methodology"),
        ]),
    )
    .unwrap();
    g.upsert_node(
        "Decision",
        "dec:retire",
        props(&[
            ("name", "Retire Y"),
            ("decision", "Y is withdrawn."),
            ("rationale", "Obsolete."),
            ("status", "accepted"),
        ]),
    )
    .unwrap();
    g.create_edge(
        "OBSOLETES",
        "Decision",
        "dec:retire",
        "DesignRule",
        "rule:gone",
        std::collections::HashMap::new(),
    )
    .unwrap();
    assert!(g.is_discontinued("rule:gone").unwrap());
    let gaps = g.detect_gaps().expect("gaps");
    let unstated: Vec<&GapCandidate> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnstatedRuleEnforcement)
        .collect();
    let mentions = |id: &str| {
        unstated
            .iter()
            .any(|x| x.affected_ids.iter().any(|a| a == id) || x.description.contains(id))
    };
    assert!(
        mentions("rule:live"),
        "a live rule with `enforced` unstated is still asked: {unstated:?}"
    );
    assert!(
        !mentions("rule:gone"),
        "a WITHDRAWN rule has no right answer to 'does breaking it stop the build' and must not be asked: {unstated:?}"
    );
}

#[test]
fn findings_that_share_one_cause_name_each_other_and_the_one_call_that_clears_both() {
    let mk = |id: &str, src: GapSource, cap: &str| GapCandidate {
        id: id.into(),
        gap_source: src,
        scope: GapScope::Project,
        severity: 0.5,
        title: id.to_string(),
        description: "x".into(),
        evidence: String::new(),
        affected_ids: vec![cap.into()],
        suggested_depth: 1,
    };
    let mut gaps = vec![
        mk("gap:a", GapSource::UnallocatedCapability, "cap:x"),
        mk("gap:b", GapSource::UnrealizedCapability, "cap:x"),
        mk("gap:c", GapSource::UnallocatedCapability, "cap:other"),
    ];
    reflow2_core::detect::group_shared_causes(&mut gaps);
    let a = gaps.iter().find(|x| x.id == "gap:a").unwrap();
    let b = gaps.iter().find(|x| x.id == "gap:b").unwrap();
    let c = gaps.iter().find(|x| x.id == "gap:c").unwrap();
    assert!(
        a.description.contains("gap:b") && a.description.contains("allocate"),
        "{}",
        a.description
    );
    assert!(
        b.description.contains("gap:a") && b.description.contains("allocate"),
        "{}",
        b.description
    );
    assert_eq!(
        c.description, "x",
        "a finding with no sibling is left alone"
    );
}
