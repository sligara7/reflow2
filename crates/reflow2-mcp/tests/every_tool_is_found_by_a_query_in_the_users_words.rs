//! Every served tool is found by `find_tools` from a query in a USER'S words.
//!
//! # The corpus is the test — Anthony's condition, 2026-09-11
//!
//! `dec:a-tool-carries-the-users-phrasing-and-the-corpus-is-the-test`. One
//! query per tool in `fixtures/find_tools_corpus.json`, phrased as somebody
//! states the job rather than as the tool is named. A tool passes when it is
//! in the default top 5. The bar is the default because 5 is what a consumer
//! sees.
//!
//! # The trap this test cannot see, and what guards it
//!
//! A description sentence that merely COPIES its corpus query passes here and
//! still fails a user who words it differently — teaching to the test. This
//! test does not detect that. The guard is a second, held-out paraphrase
//! corpus (`docs/local/tool-sweep/findability/queries-heldout.json`, scored
//! by `score.py`), written from a different angle and never consulted while
//! the sentences were written. A tool that passes here and fails there has a
//! sentence that was tuned, not written. That corpus is deliberately NOT a CI
//! gate: it is the honesty check on this one, and folding it in would make
//! it the thing people tune to next.
//!
//! # Measured on the way here
//!
//! Baseline 2026-09-11: 114 of 180 pass, 53 miss the top 10 entirely. After
//! the scorer fix (#475): 120 pass, 42 miss. After carrying the user's
//! phrasing on the missed tools: see the assertion below — every tool.
//!
//! # A new tool joins automatically — and FAILS until it has a query
//!
//! The second test walks the served surface and refuses any tool absent from
//! the corpus, so a 181st tool cannot ship unfindable by omission.

use reflow2_mcp::service::ReflowService;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const CORPUS: &str = include_str!("fixtures/find_tools_corpus.json");

/// Tools that sit ONE place outside the top 5 for their query, with the
/// measured reason. An entry here is the ceiling of word-matching stated
/// out loud, not a pass waved through — and the second test refuses a
/// stale one.
///
/// Measured 2026-09-11 after every tool carried its phrasing: 177 of 180 in
/// the top 5, these three at rank 6, each beaten by SHORT descriptions on
/// incidental words (`reviewed_gaps` at 106 chars over `consumption_report`
/// at 1,252). Two scorer refinements were tried and measured on both
/// corpora — a 4-char floor on the name-prefix rule (neutral: 177/36) and a
/// gentler length normalisation (worse: 174/34) — and both were reverted,
/// because a change the corpus does not reward is noise.
/// Served names that are refusing STUBS for a renamed tool (2026-09-18):
/// they carry no query, rank for nothing, and go away next release.
const DEPRECATED: &[&str] = &["record_change", "manual_work_report"];

const RANK_BASELINE: &str = include_str!("fixtures/find_tools_rank_baseline.json");

const EXEMPT: &[(&str, &str)] = &[];

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
        .find_tools(Parameters(
            serde_json::from_value(json!({"query": query, "limit": 5})).unwrap(),
        ))
        .await
        .expect("find_tools")
        .structured_content
        .expect("structured");
    v["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["tool"].as_str().map(String::from))
        .collect()
}

fn is_deprecated(name: &str) -> bool {
    DEPRECATED.contains(&name)
}

fn served() -> Vec<String> {
    let mut all = ReflowService::capture_router().list_all();
    for r in [
        ReflowService::assure_router(),
        ReflowService::exchange_router(),
        ReflowService::temporal_tools_router(),
        ReflowService::ask_router(),
        ReflowService::built_router(),
        ReflowService::coherence_router(),
        ReflowService::ingest_tools_router(),
        ReflowService::operate_tools_router(),
        ReflowService::query_router(),
        ReflowService::claims_tools_router(),
        ReflowService::skills_router(),
    ] {
        all.extend(r.list_all());
    }
    all.into_iter().map(|t| t.name.to_string()).collect()
}

/// THE ACCEPTANCE TEST for criterion 1.
#[tokio::test]
async fn every_tool_is_in_the_top_five_for_its_query() {
    let s = ReflowService::in_memory().expect("service");
    let mut missed = Vec::new();
    let corpus = corpus();
    let exempt: Vec<&str> = EXEMPT.iter().map(|(t, _)| *t).collect();
    for (tool, query) in &corpus {
        if exempt.contains(&tool.as_str()) {
            continue;
        }
        let got = top5(&s, query).await;
        if !got.iter().any(|t| t == tool) {
            missed.push(format!("{tool:<32} ← {query:?}\n      top5: {got:?}"));
        }
    }
    assert!(
        missed.is_empty(),
        "{} of {} tools are not in find_tools' top 5 for a query in a user's words:\n  {}\n\n\
         A user who describes the job cannot find the tool. Carry the job in the user's \
         words ON the tool's description (\"Ask for this when you want to …\"), written \
         from what the tool DOES — not copied from the query above, which is what the \
         held-out corpus exists to catch.",
        missed.len(),
        corpus.len(),
        missed.join("\n  ")
    );
}

/// An exemption must still be TRUE: an exempt tool that has climbed into the
/// top 5 is a stale excuse, and one that has fallen out of the top 10 is a
/// regression hiding behind one.
#[tokio::test]
async fn every_exemption_is_still_one_place_off() {
    let s = ReflowService::in_memory().expect("service");
    let corpus = corpus();
    for (tool, why) in EXEMPT {
        let query = corpus
            .get(*tool)
            .unwrap_or_else(|| panic!("exempt {tool} has no query"));
        let got = top5(&s, query).await;
        assert!(
            !got.iter().any(|t| t == tool),
            "{tool} is now IN the top 5 — remove its exemption ({why})"
        );
    }
}

/// A tool with no query is unfindable by omission. A new tool fails here
/// until somebody writes down how a user would ask for it.
#[test]
fn every_served_tool_has_a_query_in_the_corpus() {
    let corpus = corpus();
    let absent: Vec<String> = served()
        .into_iter()
        .filter(|t| !is_deprecated(t))
        .filter(|t| !corpus.contains_key(t))
        .collect();
    assert!(
        absent.is_empty(),
        "{} served tool(s) have no query in tests/fixtures/find_tools_corpus.json: {}\n\
         Add one, phrased as a user would state the job — not as the tool is named.",
        absent.len(),
        absent.join(", ")
    );
    let stale: Vec<&String> = corpus.keys().filter(|t| !served().contains(t)).collect();
    assert!(
        stale.is_empty(),
        "corpus names {} tool(s) no longer served: {:?}",
        stale.len(),
        stale
    );
}

/// THE RANK RATCHET (dec:idea-prune-the-tool-surface-to-an-orthogonal-essential-set,
/// Anthony 2026-09-18: "Go with your recommendation" — words before deletion).
///
/// The top-5 test above passes a tool even when ANOTHER tool owns its words;
/// measured 2026-09-17, 73 of 185 were not ranked FIRST for their own job.
/// A description pass took that to 25, every one beaten on a NAME match
/// (five times a description hit). Those 25 are the baseline, and the list
/// may only shrink: a new tool must rank first for its job, and an entry that
/// has become first must leave the baseline.
#[tokio::test]
async fn every_tool_ranks_first_for_its_own_job_or_is_in_the_baseline() {
    let s = ReflowService::in_memory().expect("service");
    let baseline: Value = serde_json::from_str(RANK_BASELINE).expect("baseline parses");
    let listed: Vec<String> = baseline["not_first"]
        .as_array()
        .expect("not_first")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let corpus = corpus();
    let mut not_first = Vec::new();
    for (tool, query) in &corpus {
        let got = top5(&s, query).await;
        if got.first() != Some(tool) {
            not_first.push(tool.clone());
        }
    }
    let new: Vec<&String> = not_first.iter().filter(|t| !listed.contains(t)).collect();
    assert!(
        new.is_empty(),
        "{} tool(s) are no longer ranked FIRST for their own job and are not in the \
         baseline: {new:?}. Give the tool's description the job in the user's words (not the \
         corpus sentence), or shorten the tool that now outranks it; the baseline only shrinks.",
        new.len()
    );
    let stale: Vec<&String> = listed.iter().filter(|t| !not_first.contains(t)).collect();
    assert!(
        stale.is_empty(),
        "{} baseline entry(ies) now rank FIRST — remove them from \
         fixtures/find_tools_rank_baseline.json so the ratchet holds: {stale:?}",
        stale.len()
    );
}

/// The orthogonality half: no NEW pair of tools may become mutually
/// confusable (each in the other's top 5 for its own job). The recorded pairs
/// are the ceiling; a pair that comes apart leaves the baseline.
#[tokio::test]
async fn no_new_mutually_confusable_pair() {
    let s = ReflowService::in_memory().expect("service");
    let baseline: Value = serde_json::from_str(RANK_BASELINE).expect("baseline parses");
    let listed: Vec<(String, String)> = baseline["mutual"]
        .as_array()
        .expect("mutual")
        .iter()
        .filter_map(|p| {
            let a = p[0].as_str()?;
            let b = p[1].as_str()?;
            Some((a.to_string(), b.to_string()))
        })
        .collect();
    let corpus = corpus();
    let mut top: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (tool, query) in &corpus {
        top.insert(tool.clone(), top5(&s, query).await);
    }
    let mut mutual: Vec<(String, String)> = Vec::new();
    for (t, list) in &top {
        for u in list {
            if u == t || !corpus.contains_key(u) {
                continue;
            }
            if top[u].contains(t) {
                let pair = if t < u {
                    (t.clone(), u.clone())
                } else {
                    (u.clone(), t.clone())
                };
                if !mutual.contains(&pair) {
                    mutual.push(pair);
                }
            }
        }
    }
    let new: Vec<&(String, String)> = mutual.iter().filter(|p| !listed.contains(p)).collect();
    assert!(
        new.is_empty(),
        "{} NEW mutually confusable pair(s): {new:?}. Two tools now answer the same job \
         sentence; sharpen the first sentence of one, or fold them. The baseline only shrinks.",
        new.len()
    );
    let stale: Vec<&(String, String)> = listed.iter().filter(|p| !mutual.contains(p)).collect();
    assert!(
        stale.is_empty(),
        "{} baseline pair(s) have come apart — remove them from \
         fixtures/find_tools_rank_baseline.json: {stale:?}",
        stale.len()
    );
}
