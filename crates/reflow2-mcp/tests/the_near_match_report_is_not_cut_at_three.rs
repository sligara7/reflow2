//! A crowded near-match band is not cut at three.
//!
//! MEASURED 2026-09-21 (`fact:the-near-match-guard-shows-three-of-a-nine-way-
//! tie-so-relevance-is-decided-by-score-noise`): an idea was captured that
//! substantially duplicated one open here since 2026-08-26, and the guard
//! reported nothing about it. Replaying the guard's own query showed why —
//! NINE non-self hits cleared the floor, three were shown, and the missed node
//! sat at rank 4, six THOUSANDTHS of a point behind rank 5 on a 245-point
//! score.
//!
//! The cause is two constants interacting at scale, not a broken check. The
//! floor is relative to the new node's OWN score, and a node scored against
//! its own text sits structurally far above everything else (474 against a
//! next-best 283 in the measurement). So the floor lands in the MIDDLE of a
//! dense band rather than at the edge of relevance, and `.take(3)` then picks
//! three members of what is effectively a nine-way tie. On a 5,000-node design
//! a crowded band is the normal case.
//!
//! WHAT THIS PINS IS THE CLASS, NOT THE CONSTANT. It asserts that a band of
//! eight close nodes is not reported as three, and that the report is still
//! BOUNDED — because "report everything" is the other way to make this test
//! green and it is the failure the original comment on `NEAR_MATCH_LIMIT`
//! named: a list nobody reads every time is worse than no list. Tuning the
//! limit between those two bounds stays free.
//!
//! ⚠️ THE FILLER CORPUS IS LOAD-BEARING, and a first draft of this test that
//! left it out failed for the wrong reason. With the band as the WHOLE corpus
//! and every member carrying identical text, BM25's IDF collapses: a term in
//! every document distinguishes nothing, so the shared body scored ~0, the
//! probe's self-score came only from its unique title words, and NOTHING
//! cleared the floor — the guard reported no block at all and the test panicked
//! before reaching the assertion it exists for. The band has to be a crowded
//! neighbourhood inside a larger design, which is what it is in the wild.
//!
//! OBSERVED FAILING before the fix, against `NEAR_MATCH_LIMIT = 3`, and
//! passing after it.

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

/// The shared topic. Every band member says this and then adds its own clause,
/// so they are CLOSE rather than identical — close is what the guard is for,
/// and identical is what breaks the scoring (see the header note).
const TOPIC: &str = "The export document records who settled each decision and when, so a later \
                     reader can tell an intent somebody confirmed from one an agent proposed.";

/// One distinguishing clause per band member. Enough to keep the documents
/// distinct without moving any of them out of the neighbourhood.
const CLAUSES: [&str; 8] = [
    "The approver edge carries the date the owner acted.",
    "A settling status without a named approver is refused outright.",
    "The confirmation ledger reads the signature back for a reviewer.",
    "Certainty is derived from status rather than stored beside it.",
    "An agent may propose anything and may settle nothing at all.",
    "The gate fails the build on a settled node carrying no name.",
    "Authorship survives approval instead of being displaced by it.",
    "A published decision keeps the approver across the crossing.",
];

/// Unrelated material, so the shared terms above are not in every document and
/// IDF still means something. Twenty-four is not a magic number — it is simply
/// enough that the band is a neighbourhood rather than the whole world.
const FILLER: [&str; 6] = [
    "Rainfall totals arrive as cumulative counts so a lost reading heals itself on the next one.",
    "The outdoor unit sleeps between samples and wakes on a timer rather than on a request.",
    "Mirror figure error is measured at three milliradians across the whole illuminated length.",
    "The container image runs unprivileged and keeps no state of its own between restarts.",
    "Spreadsheet imports are rejected when a column header repeats anywhere in the first row.",
    "Backup runs nightly and the restore is exercised quarterly against a scratch machine.",
];

/// The window the index is asked for, from `capture.rs`. The report can never
/// exceed it, so it is the honest upper bound for "still bounded".
const WINDOW: usize = 12;

fn decision(id: &str, name: &str, body: &str) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some(name.into()),
        decision: Some(body.into()),
        rationale: None,
        distinct_from: None,
        kind: None,
        related_to: None,
        no_relation_note: None,
        status: None,
        approver: None,
        acted_at: None,
    }
}

#[tokio::test]
async fn a_crowded_near_match_band_is_not_cut_at_three() {
    let svc = ReflowService::in_memory().expect("in-memory service");

    // Unrelated corpus first, so the band's shared vocabulary is distinctive
    // rather than universal.
    for (i, text) in FILLER.iter().enumerate() {
        let id = format!("dec:filler-{i}");
        j!(svc.add_decision(Parameters(decision(
            &id,
            &format!("Unrelated matter {i}"),
            text
        ))));
    }

    // The band. Each is created against a design that already holds the
    // previous ones, so the earlier writes meet the guard themselves —
    // `distinct_from` names every id seeded so far, which is the deliberate
    // "I read them and they differ" route and keeps the seeding from being the
    // thing under test.
    let mut band: Vec<String> = Vec::new();
    for (i, clause) in CLAUSES.iter().enumerate() {
        let id = format!("dec:band-member-{i}");
        let mut req = decision(
            &id,
            &format!("Settlement record {i}"),
            &format!("{TOPIC} {clause}"),
        );
        req.distinct_from = Some(band.clone());
        j!(svc.add_decision(Parameters(req)));
        band.push(id);
    }

    // The node that should be told what it resembles. It names every band
    // member as distinct so the write is not refused — the refusal is a
    // different behaviour, covered elsewhere; what is under test here is the
    // ADVISORY BLOCK, attached whenever there are near matches at all.
    // Its name follows the band's own pattern and its body is the shared TOPIC
    // with nothing added. That is deliberate: the probe must not carry
    // vocabulary the band lacks, or its own score climbs, the floor climbs with
    // it (the floor is half the probe's score) and the band drops below a bar
    // that only the probe can reach. An earlier draft gave the probe an extra
    // sentence and only TWO of eight cleared — the cap was never reached, so
    // the test could not measure it.
    let mut req = decision(
        "dec:the-one-that-should-see-the-band",
        "Settlement record 8",
        TOPIC,
    );
    req.distinct_from = Some(band.clone());
    let out = j!(svc.add_decision(Parameters(req)));

    let near = out["search_first"]["near_matches"]
        .as_array()
        .expect("the guard reported near matches at all — without them this test proves nothing");

    assert!(
        near.len() > 3,
        "a crowded band must not be cut at three: {} nodes were seeded in one neighbourhood and \
         the guard reported {}. Three of a tie is a lottery, not a relevance filter — see \
         fact:the-near-match-guard-shows-three-of-a-nine-way-tie-so-relevance-is-decided-by-\
         score-noise",
        CLAUSES.len(),
        near.len()
    );

    assert!(
        near.len() <= WINDOW,
        "the report must stay BOUNDED: it returned {} against a window of {WINDOW}. Removing the \
         cap is the other way to make the assertion above pass, and it is the failure the \
         original NEAR_MATCH_LIMIT comment named — a list nobody reads every time is worse than \
         no list",
        near.len()
    );
}
