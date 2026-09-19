//! The lens line names the person beside their id, so a session can match the
//! git author it can see against the readers the design records.
//!
//! Anthony, 2026-09-14, on flo2 — one repo shared with his brother: *"When I
//! am looking at the project, it should be viewed from my lens/persona. When
//! he is looking at the project, it should be viewed from his lens … 'assuming
//! you are AJ; if not, please state your name and some background'."*
//!
//! The graph cannot say who is at the keyboard and never will; the harness
//! can see a git author. With two recorded readers, the agent's only join is
//! NAME against NAME — and the lens line printed ids alone, so the match was a
//! hunch that needed a fetch per person. Now it is a lookup over text already
//! in front of the agent. No new identity signal is recorded: names are
//! already in the public export, and nothing else is.

use reflow2_mcp::service::*;
use reflow2_mcp::tools::skills_tools::GetSkillReq;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn service() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(ProjectReq {
        id: "proj:x".into(),
        name: Some("X".to_string()),
        description: None,
        decomposition_levels: None,
        status: None,
    })));
    s
}

async fn person(s: &ReflowService, id: &str, name: &str, description: Option<&str>) {
    j!(s.add_contributor(Parameters(ContributorReq {
        id: id.into(),
        name: Some(name.to_string()),
        kind: Some("person".into()),
        handle: None,
        description: description.map(str::to_string),
    })));
}

/// ⭐ THE CASE IT EXISTS FOR: two recorded readers, and the line carries both
/// names, so "git author = Anthony Sligar" resolves to who:ajs without a fetch.
#[tokio::test]
async fn two_recorded_readers_are_named_beside_their_ids() {
    let s = service().await;
    person(&s, "who:ajs", "Anthony Sligar", Some("Systems engineer.")).await;
    person(
        &s,
        "who:alex",
        "Alex",
        Some("Software engineer; biology degree."),
    )
    .await;

    let out = j!(s.get_skill(Parameters(GetSkillReq {
        name: "where-am-i".into(),
    })));
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");

    assert!(
        lens.contains("who:ajs (Anthony Sligar)"),
        "the id must carry the name beside it: {lens}"
    );
    assert!(lens.contains("who:alex (Alex)"), "{lens}");
}

/// The silent case names who could be asked — by name as well, since the
/// agent may be able to tell from the git author which of them is present.
#[tokio::test]
async fn the_askable_people_are_named_too() {
    let s = service().await;
    person(&s, "who:ann", "Ann Example", None).await;

    let out = j!(s.get_skill(Parameters(GetSkillReq {
        name: "where-am-i".into(),
    })));
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");

    assert!(lens.contains("NOBODY'S BACKGROUND IS RECORDED"), "{lens}");
    assert!(lens.contains("who:ann (Ann Example)"), "{lens}");
}

/// A person with no `name` is still listed — by id alone, never dropped.
#[tokio::test]
async fn a_person_with_no_name_is_still_listed() {
    let s = service().await;
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:nameless".into(),
        name: None,
        kind: Some("person".into()),
        handle: None,
        description: Some("Vet.".to_string()),
    })));

    let out = j!(s.get_skill(Parameters(GetSkillReq {
        name: "where-am-i".into(),
    })));
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");
    assert!(lens.contains("who:nameless"), "{lens}");
    assert!(
        !lens.contains("who:nameless ("),
        "no empty parenthesis: {lens}"
    );
}

/// The same line rides loop_status, so the join is available at the moment
/// the closing words are chosen as well.
#[tokio::test]
async fn loop_status_carries_the_names_as_well() {
    let s = service().await;
    person(&s, "who:ajs", "Anthony Sligar", Some("Systems engineer.")).await;

    let out = j!(s.loop_status(Parameters(LoopScopeReq {
        contributor_id: None,
        since_export: false,
    })));
    let lens = out.get("lens").and_then(|v| v.as_str()).expect("lens");
    assert!(lens.contains("who:ajs (Anthony Sligar)"), "{lens}");
}
