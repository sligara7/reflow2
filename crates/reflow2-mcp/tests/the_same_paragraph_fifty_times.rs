//! The same paragraph, sent once instead of fifty times.
//!
//! `repair_is_a_judgement` is `Option<&'static str>` — a fixed literal per
//! detector branch, saying why a category of structural defect has no
//! mechanical repair. It is written per ROW, so a design with two dozen orphan
//! nodes receives the same 797-character paragraph two dozen times.
//!
//! MEASURED on reflow2's own design, 2026-08-23: `detect_defects` was 46,399
//! characters, of which 45,186 were the findings — and **52.3% of those were
//! this one field: 50 rows carrying 3 DISTINCT values.**
//!
//! This is a different shape from the two fixes before it, and the difference
//! is the point. `detect_gaps` withholds prose when a reply will not fit;
//! `graph_report` withholds a list unless asked. **Nothing is withheld here.**
//! No list is shortened, no prose truncated, no judgement made about what a
//! reader needs. The same words are simply not sent fifty times — which is why
//! this needs no flag, no budget and no note about what was lost.
//!
//! What has to hold is that the saving cannot make a reader wrong:
//!
//! 1. every row still resolves to its explanation, by
//!    `row.repair_is_a_judgement ?? map[row.category]`;
//! 2. a row whose text differs from its category's keeps its own, so the
//!    fallback is never merely *probably* right;
//! 3. it fires on the SCOPED reply too — `Scoped<T>` names its list `items`
//!    rather than `defects`, and the first version of this looked only for
//!    `defects` and was a silent no-op on every scoped call. Found by driving
//!    the built binary, which is why this case exists;
//! 4. it does not fire where it would not pay. One row carrying a paragraph
//!    costs less inline than a map entry plus the sentence explaining the map.

use reflow2_mcp::service::*;
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

/// `n` Verifications checking nothing — `n` `orphan_node` defects, every one of
/// them carrying the same repair paragraph.
///
/// ⚠️ REWRITTEN 2026-08-29, AND THE REASON IS WORTH KEEPING. This fixture used
/// to build its orphans out of TemporalFacts pointing at deleted subjects. That
/// worked only while `orphan_node` happened to own broken pointers as a side
/// effect of its degree check; the moment `dangling_reference` took that case,
/// the fixture produced ZERO orphan rows and both tests here failed — not
/// because the hoisting mechanism they exist to pin had changed, but because
/// their raw material had moved to another rule.
///
/// A test about how a SHARED PARAGRAPH IS SENT should not depend on which rule
/// happens to generate the rows. An edgeless Verification is an orphan by the
/// plainest reading of the rule — it "checks nothing", no VERIFIES edge says
/// what it is a check OF — and involves no pointer at all, so this fixture is
/// now independent of anything the reference rules do.
async fn with_orphans(n: usize) -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    for i in 0..n {
        let _ = s
            .add_verification(Parameters(
                serde_json::from_value(serde_json::json!({
                    "id": format!("ver:checks-nothing-{i}"),
                    "name": format!("A check nothing says the subject of, number {i}"),
                    "method": "test",
                    "level": "unit",
                }))
                .unwrap(),
            ))
            .await
            .expect("tool ok");
    }
    s
}

/// The list, whatever the reply calls it.
fn rows(reply: &serde_json::Value) -> Vec<serde_json::Value> {
    reply
        .get("defects")
        .or_else(|| reply.get("items"))
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default()
}

/// `row.repair_is_a_judgement ?? map[row.category]` — the rule a reader follows.
fn explanation(row: &serde_json::Value, reply: &serde_json::Value) -> Option<String> {
    if let Some(own) = row.get("repair_is_a_judgement").and_then(|t| t.as_str()) {
        return Some(own.to_string());
    }
    let cat = row.get("category")?.as_str()?;
    reply
        .get("repair_is_a_judgement")?
        .get(cat)?
        .as_str()
        .map(str::to_string)
}

#[tokio::test]
async fn every_finding_still_resolves_to_its_explanation() {
    let s = with_orphans(6).await;
    let reply = j!(s.detect_defects(Parameters(ScopeReq::default())));
    let found = rows(&reply);
    assert!(!found.is_empty(), "the fixture must produce defects");

    for row in &found {
        if row.get("category").and_then(|c| c.as_str()) != Some("orphan_node") {
            continue;
        }
        let text = explanation(row, &reply)
            .unwrap_or_else(|| panic!("no explanation reachable for {row}"));
        assert!(
            text.contains("No mechanical repair"),
            "and it must be the real paragraph, not an empty string: {text:.80}"
        );
    }
}

#[tokio::test]
async fn the_paragraph_is_sent_once_not_once_per_row() {
    let s = with_orphans(6).await;
    let reply = j!(s.detect_defects(Parameters(ScopeReq::default())));
    let orphans: Vec<_> = rows(&reply)
        .into_iter()
        .filter(|r| r.get("category").and_then(|c| c.as_str()) == Some("orphan_node"))
        .collect();
    assert!(
        orphans.len() > 1,
        "the saving needs more than one row to be about"
    );
    assert!(
        orphans
            .iter()
            .all(|r| r.get("repair_is_a_judgement").is_none()),
        "rows sharing their category's text should not each carry a copy"
    );
    assert!(
        reply["repair_is_a_judgement"]["orphan_node"].is_string(),
        "and the one copy has to be somewhere: {}",
        reply["repair_is_a_judgement"]
    );
}

/// A reply with `n` findings of one category, all carrying `text`.
fn reply_with(list_key: &str, n: usize, text: &str) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            serde_json::json!({
                "id": format!("heal:{i}"),
                "category": "orphan_node",
                "repair_is_a_judgement": text,
            })
        })
        .collect();
    serde_json::json!({ list_key: rows })
}

#[tokio::test]
async fn it_fires_on_the_scoped_shape_as_well_as_the_unscoped_one() {
    // `Scoped<T>` names its list `items`; the unscoped reply names it
    // `defects`. The first version of this looked only for `defects` and did
    // NOTHING AT ALL on a scoped call — a silent no-op, which is the failure
    // mode that looks exactly like working.
    //
    // Pinned directly on both shapes rather than through a fixture: the
    // categories that carry a repair note are the ones whose findings are
    // disconnected by definition, so a synthetic region holding several of them
    // is awkward to build — and the first attempt at this test QUIETLY PASSED
    // against the bug because its scope contained no such rows at all.
    const TEXT: &str = "No mechanical repair. This would assert a relationship nobody drew.";

    for list_key in ["defects", "items"] {
        let mut reply = reply_with(list_key, 4, TEXT);
        reflow2_mcp::tools::coherence::lift_repair_notes(&mut reply);

        assert_eq!(
            reply["repair_is_a_judgement"]["orphan_node"], TEXT,
            "`{list_key}`: the one copy has to be somewhere: {reply}"
        );
        let rows = reply[list_key]
            .as_array()
            .expect("the list survives")
            .clone();
        assert_eq!(rows.len(), 4, "`{list_key}`: no finding is dropped");
        assert!(
            rows.iter()
                .all(|r| r.get("repair_is_a_judgement").is_none()),
            "`{list_key}`: rows sharing their category's text should not each carry a copy"
        );
        for row in &rows {
            assert!(
                explanation(row, &reply).is_some(),
                "`{list_key}`: every row must still resolve: {row}"
            );
        }
    }
}

#[tokio::test]
async fn a_row_whose_text_differs_keeps_its_own() {
    // The fallback must never be merely PROBABLY right. If one category ever
    // carries two explanations, the odd row keeps its own rather than being
    // silently handed the other one.
    const SHARED: &str = "No mechanical repair. The usual explanation for this category.";
    let mut reply = reply_with("defects", 3, SHARED);
    reply["defects"][2]["repair_is_a_judgement"] =
        serde_json::json!("A different explanation entirely, for the same category.");
    reflow2_mcp::tools::coherence::lift_repair_notes(&mut reply);

    let rows = reply["defects"].as_array().expect("list").clone();
    for row in &rows {
        let text = explanation(row, &reply).expect("every row resolves");
        let expected = row["id"].as_str() == Some("heal:2");
        assert_eq!(
            text.starts_with("A different"),
            expected,
            "row {} resolved to the wrong explanation: {text:.60}",
            row["id"]
        );
    }
}

#[tokio::test]
async fn one_of_a_kind_keeps_its_text_inline() {
    // A map entry plus the sentence explaining the map costs more than one
    // paragraph inline. A mechanism that fires where it does not pay is how a
    // reply grows while claiming to shrink.
    let s = with_orphans(1).await;
    let reply = j!(s.detect_defects(Parameters(ScopeReq::default())));
    let single: Vec<_> = rows(&reply)
        .into_iter()
        .filter(|r| r.get("category").and_then(|c| c.as_str()) == Some("orphan_node"))
        .collect();
    assert_eq!(single.len(), 1, "one orphan, one finding");
    assert!(
        single[0].get("repair_is_a_judgement").is_some(),
        "a lone row keeps its own text rather than being lifted into a map for one entry"
    );
    assert!(
        reply.get("repair_is_a_judgement").is_none(),
        "and no map is built for it: {reply:.400}"
    );
}
