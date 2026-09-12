//! A lesson a design holds is delivered at the step it concerns.
//!
//! `req:a-lesson-is-served-at-the-step-it-concerns` (increment 438). The
//! measured premise: a lesson written in prose does not change the next
//! command — the same trap was recorded three times and repeated. So a rule
//! or a dated fact may name the served skill or tool it concerns, and:
//!
//! 1. `get_skill` carries the lessons naming that skill beside its body, and
//!    a skill nobody named carries none.
//! 2. The tool list carries the lessons naming a tool on that tool's
//!    description and on no other; an empty design lists the surface
//!    unchanged (which the toolsnap goldens, taken on an empty design, pin
//!    over the wire).
//! 3. A lesson naming no step is still recorded and appears at neither.
//! 4. A step that names nothing served is REFUSED with the nearest served
//!    names, so a typo cannot file a lesson where nothing will deliver it.
//! 5. Rules take steps too, and newest first is the order.
//!
//! The listing is exercised through the same function the `list_tools`
//! override calls, because a `RequestContext` cannot be built in a test; the
//! wire shape is pinned by `tools/test_a_lesson_is_served_at_the_step.py`.

use reflow2_mcp::service::*;
use reflow2_mcp::tools::skills_tools::GetSkillReq;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

fn finding(id: &str, name: &str, steps: Option<Vec<&str>>, valid_from: &str) -> RecordFindingReq {
    RecordFindingReq {
        id: id.into(),
        subject_id: "proj:the-design".into(),
        node_type: None,
        name: Some(name.into()),
        statement: Some(format!("{name}: what bit us, and what to do instead.")),
        fact_type: Some("finding".into()),
        basis: None,
        confidence: None,
        value: None,
        valid_from: Some(valid_from.into()),
        valid_to: None,
        caused_by: None,
        caused_by_type: None,
        cause_evidence: None,
        steps: steps.map(|v| v.into_iter().map(String::from).collect()),
    }
}

async fn seeded() -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:the-design".into(),
        name: Some("The design".into()),
        description: None,
        spec: None,
        decomposition_levels: None,
    })));
    s
}

async fn skill_lessons(s: &ReflowService, skill: &str) -> Vec<String> {
    let r = j!(s.get_skill(Parameters(GetSkillReq { name: skill.into() })));
    r["lessons"]["items"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| l["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// THE POINT: the lesson is in the reply for its skill, and in no other's.
#[tokio::test]
async fn get_skill_carries_the_lessons_naming_that_skill_and_no_others() {
    let s = seeded().await;
    j!(s.record_finding(Parameters(finding(
        "fact:orient-before-you-touch-code",
        "Orient before you touch code",
        Some(vec!["where-am-i"]),
        "2026-09-12",
    ))));
    let r = j!(s.get_skill(Parameters(GetSkillReq {
        name: "where-am-i".into()
    })));
    assert!(
        r["body"].as_str().is_some_and(|b| !b.is_empty()),
        "the skill itself still comes"
    );
    assert_eq!(
        skill_lessons(&s, "where-am-i").await,
        vec!["fact:orient-before-you-touch-code".to_string()]
    );
    assert!(
        r["lessons"]["note"]
            .as_str()
            .unwrap_or("")
            .contains("THIS DESIGN"),
        "the note says whose lessons these are: {r}"
    );
    assert!(
        skill_lessons(&s, "brainstorm").await.is_empty(),
        "a skill nobody named carries none"
    );
}

/// The tool list carries a lesson on the tool it names, and only there.
#[tokio::test]
async fn the_tool_list_carries_a_lesson_on_its_tool_and_nowhere_else() {
    let s = seeded().await;
    let before: Vec<rmcp::model::Tool> = s.tools_with_lessons_for_test().await;
    let plain: Vec<rmcp::model::Tool> = ReflowService::capture_router().list_all();
    let base_of = |name: &str| {
        plain
            .iter()
            .chain(before.iter())
            .find(|t| t.name == name)
            .and_then(|t| t.description.clone())
            .unwrap_or_default()
            .to_string()
    };
    assert!(
        before.iter().all(|t| !t
            .description
            .as_deref()
            .unwrap_or("")
            .contains("LESSONS THIS DESIGN HOLDS")),
        "an empty design lists the surface unchanged"
    );

    j!(s.record_finding(Parameters(finding(
        "fact:format-first",
        "Format before the export",
        Some(vec!["export_graph"]),
        "2026-09-12",
    ))));
    let after = s.tools_with_lessons_for_test().await;
    let desc = |name: &str| {
        after
            .iter()
            .find(|t| t.name == name)
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
            .to_string()
    };
    assert!(
        desc("export_graph").contains("LESSONS THIS DESIGN HOLDS FOR `export_graph`")
            && desc("export_graph").contains("fact:format-first"),
        "{}",
        desc("export_graph")
    );
    assert!(
        desc("export_graph").starts_with(&base_of("export_graph")),
        "the served description is kept whole; the lessons are appended"
    );
    let touched = after
        .iter()
        .filter(|t| {
            t.description
                .as_deref()
                .unwrap_or("")
                .contains("LESSONS THIS DESIGN HOLDS")
        })
        .count();
    assert_eq!(touched, 1, "exactly one tool carries it");
}

/// A lesson with no step is recorded and delivered nowhere — not an error.
#[tokio::test]
async fn a_lesson_naming_no_step_is_kept_and_delivered_nowhere() {
    let s = seeded().await;
    j!(s.record_finding(Parameters(finding(
        "fact:a-thing-we-noticed",
        "A thing we noticed",
        None,
        "2026-09-12",
    ))));
    let n = j!(s.get_node(Parameters(GetNodeReq {
        id: "fact:a-thing-we-noticed".into(),
        node_type: None,
    })));
    assert!(!n["node"].is_null(), "recorded: {n}");
    assert!(skill_lessons(&s, "where-am-i").await.is_empty());
    let tools = s.tools_with_lessons_for_test().await;
    assert!(tools.iter().all(|t| {
        !t.description
            .as_deref()
            .unwrap_or("")
            .contains("a-thing-we-noticed")
    }));
}

/// A typo is refused, naming what would have worked, and nothing is written.
#[tokio::test]
async fn an_unserved_step_is_refused_with_the_nearest_names() {
    let s = seeded().await;
    let err = s
        .record_finding(Parameters(finding(
            "fact:typo",
            "A typo'd step",
            Some(vec!["export-graph"]),
            "2026-09-12",
        )))
        .await
        .expect_err("a step nothing serves must be refused");
    let msg = err.to_string();
    assert!(
        msg.contains("export-graph") && msg.contains("export_graph"),
        "{msg}"
    );
    let n = j!(s.get_node(Parameters(GetNodeReq {
        id: "fact:typo".into(),
        node_type: None,
    })));
    assert!(n["node"].is_null(), "nothing was written: {n}");
}

/// Rules take steps too, and the newest lesson comes first.
#[tokio::test]
async fn rules_take_steps_and_the_newest_lesson_comes_first() {
    let s = seeded().await;
    j!(s.add_design_rule(Parameters(DesignRuleReq {
        id: "rule:never-export-twice".into(),
        name: Some("Never export twice on one branch".into()),
        statement: Some("The lineage lives inside the file; export once, last.".into()),
        category: Some("convention".into()),
        enforced: None,
        approver: None,
        acted_at: None,
        distinct_from: None,
        steps: Some(vec!["export_graph".into(), "ci-gate".into()]),
    })));
    j!(s.record_finding(Parameters(finding(
        "fact:older",
        "An older lesson on exporting",
        Some(vec!["export_graph"]),
        "2026-06-01",
    ))));
    j!(s.record_finding(Parameters(finding(
        "fact:newer",
        "A newer lesson on exporting",
        Some(vec!["export_graph"]),
        "2026-09-10",
    ))));
    assert_eq!(
        skill_lessons(&s, "ci-gate").await,
        vec!["rule:never-export-twice".to_string()]
    );
    let tools = s.tools_with_lessons_for_test().await;
    let d = tools
        .iter()
        .find(|t| t.name == "export_graph")
        .and_then(|t| t.description.as_deref())
        .unwrap_or("")
        .to_string();
    let pos = |id: &str| d.find(id).unwrap_or(usize::MAX);
    assert!(pos("fact:newer") < pos("fact:older"), "newest first: {d}");
    assert!(d.contains("rule:never-export-twice"));
    let _: Value = Value::Null;
}
