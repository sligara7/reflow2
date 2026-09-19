//! Every skill is found by a query in the user's words.
//!
//! flo2 F8 (2026-09-18): "`find_tools` exists and is good … Nothing equivalent
//! exists for skills, so the only way to find a skill you cannot name is to
//! read all 29 descriptions. That is also, verbatim, the project owner's
//! complaint about his own tool: 'I continually forget which skills are
//! available.'" `find_skills` is the counterpart, ranked over each skill's
//! name, one-line summary and trigger description, and this is its held-out
//! corpus: one query per skill in a person's words, never the skill's name.
//! The ratchet beside it (`fixtures/find_skills_rank_baseline.json`) says which
//! skills are not ranked FIRST for their own job, and may only shrink.

use reflow2_mcp::service::*;
use reflow2_mcp::skills::SKILLS;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const CORPUS: &str = include_str!("fixtures/find_skills_corpus.json");
const RANK_BASELINE: &str = include_str!("fixtures/find_skills_rank_baseline.json");

fn corpus() -> BTreeMap<String, String> {
    let v: Value = serde_json::from_str(CORPUS).expect("corpus parses");
    v["queries"]
        .as_object()
        .expect("queries")
        .iter()
        .map(|(k, q)| (k.clone(), q.as_str().expect("query").to_string()))
        .collect()
}

async fn top5(s: &ReflowService, query: &str) -> Vec<String> {
    let v: Value = s
        .find_skills(Parameters(
            serde_json::from_value(json!({"query": query, "limit": 5})).unwrap(),
        ))
        .await
        .expect("find_skills")
        .structured_content
        .expect("structured");
    v["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["name"].as_str().map(String::from))
        .collect()
}

#[test]
fn every_served_skill_has_a_query_in_the_corpus_and_nothing_else_does() {
    let corpus = corpus();
    let served: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
    let absent: Vec<&&str> = served
        .iter()
        .filter(|s| !corpus.contains_key(**s))
        .collect();
    assert!(
        absent.is_empty(),
        "{} served skill(s) have no query in tests/fixtures/find_skills_corpus.json: {absent:?}. \
         Add one, phrased as a person would state the job — not as the skill is named.",
        absent.len()
    );
    let stale: Vec<&String> = corpus
        .keys()
        .filter(|k| !served.contains(&k.as_str()))
        .collect();
    assert!(
        stale.is_empty(),
        "corpus names skills no longer served: {stale:?}"
    );
}

#[tokio::test]
async fn every_skill_is_in_the_top_five_for_its_query() {
    let s = ReflowService::in_memory().expect("service");
    let mut missed = Vec::new();
    for (skill, query) in &corpus() {
        let got = top5(&s, query).await;
        if !got.iter().any(|t| t == skill) {
            missed.push(format!("{skill:<24} ← {query:?}\n      top5: {got:?}"));
        }
    }
    assert!(
        missed.is_empty(),
        "{} skill(s) are not in find_skills' top 5 for a query in a person's words:\n  {}\n\n\
         A person who describes the job cannot find the skill. Carry the job in the person's \
         words in the skill's summary or description — written from what the skill DOES, not \
         copied from the query above, which is what the held-out corpus exists to catch.",
        missed.len(),
        missed.join("\n  ")
    );
}

#[tokio::test]
async fn every_skill_ranks_first_for_its_own_job_or_is_in_the_baseline() {
    let s = ReflowService::in_memory().expect("service");
    let baseline: Value = serde_json::from_str(RANK_BASELINE).expect("baseline parses");
    let listed: Vec<String> = baseline["not_first"]
        .as_array()
        .expect("not_first")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let mut not_first = Vec::new();
    let mut who_beat = Vec::new();
    for (skill, query) in &corpus() {
        let got = top5(&s, query).await;
        if got.first() != Some(skill) {
            not_first.push(skill.clone());
            who_beat.push(format!("{skill} ← {query:?}: {got:?}"));
        }
    }
    let new: Vec<&String> = not_first.iter().filter(|t| !listed.contains(t)).collect();
    assert!(
        new.is_empty(),
        "{} skill(s) are no longer ranked FIRST for their own job and are not in the baseline: \
         {new:?}\n  {}\nGive the skill's summary the job in the person's words, or shorten the \
         skill that now outranks it; the baseline only shrinks.",
        new.len(),
        who_beat.join("\n  ")
    );
    let stale: Vec<&String> = listed.iter().filter(|t| !not_first.contains(t)).collect();
    assert!(
        stale.is_empty(),
        "{} baseline entr(y/ies) now rank first — remove them from \
         fixtures/find_skills_rank_baseline.json so the ratchet holds: {stale:?}",
        stale.len()
    );
}

#[tokio::test]
async fn a_match_carries_what_a_person_and_an_agent_each_need() {
    let s = ReflowService::in_memory().expect("service");
    let v: Value = s
        .find_skills(Parameters(
            serde_json::from_value(json!({"query": "if I change this, what else does it affect"}))
                .unwrap(),
        ))
        .await
        .expect("find_skills")
        .structured_content
        .expect("structured");
    let first = &v["items"][0];
    assert_eq!(first["name"], "impact-check", "{v}");
    // The shortcut is what a PERSON types and may differ from the name; the
    // summary is the line they read; the audience says who it is for; the
    // agent needs the name for get_skill.
    assert!(
        first["shortcut"].as_str().unwrap_or("").starts_with('/'),
        "{first}"
    );
    assert!(
        !first["summary"].as_str().unwrap_or("").is_empty(),
        "{first}"
    );
    assert!(
        ["anyone", "operator", "agent"].contains(&first["audience"].as_str().unwrap_or("")),
        "{first}"
    );
    assert_eq!(v["searched"], SKILLS.len());
}
