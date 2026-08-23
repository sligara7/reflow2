//! The report a session reads to decide what to look at is not a roll call.
//!
//! `graph_report` answers *"what should I look at?"*, and it is what the
//! **where-am-i** skill reads. Measured on reflow2's own design on 2026-08-23,
//! after the reply had already stopped being sent twice: **166,934 characters,
//! of which 152,803 — 91.5% — were the full verification roll.** 197 checks,
//! 196 of them passing. The one read that exists to point a session somewhere
//! spent nine tenths of itself saying "196 passing, 1 planned".
//!
//! And 93% of THAT was the `name` field: 113 of the 197 names run over 25 words
//! and the longest is 654, because `description` was declared and unreachable
//! from `add_verification` for a long time, so authors wrote their reports into
//! the name.
//!
//! Two mechanisms already existed and neither had been pointed here.
//! `vocabulary_coverage` withholds its flat list unless asked
//! (`include_unused`), and `loop_status` returns a verification DIGEST with
//! names cut at 25 words and the cut announced. Reusing that digest is why the
//! truncation is not written twice.
//!
//! What is pinned here is the pair that keeps the saving honest: the roll is
//! still REACHABLE, and the pointer to it still RESOLVES. A withheld list whose
//! retrieval instruction has gone stale is worse than one that was never
//! withheld.

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

/// A design with a handful of checks on it, so the roll has something in it.
async fn with_checks() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    for (id, name) in [
        (
            "ver:brakes",
            "The vehicle stops inside 40 metres from 100 km/h",
        ),
        (
            "ver:telemetry",
            "Position reaches the ground station once a second",
        ),
        ("ver:battery", "A full charge lasts eight hours under load"),
    ] {
        let _ = s
            .add_verification(Parameters(
                serde_json::from_value(serde_json::json!({ "id": id, "name": name })).unwrap(),
            ))
            .await
            .expect("tool ok");
    }
    s
}

fn size(v: &serde_json::Value) -> usize {
    serde_json::to_string(v).expect("serializes").len()
}

#[tokio::test]
async fn the_report_does_not_carry_every_check_by_default() {
    let s = with_checks().await;
    let report = j!(s.graph_report(Parameters(GraphReportReq::default())));

    let roll = &report["verifications"];
    assert!(
        roll.is_object(),
        "the default is the digest, not the list: {roll}"
    );
    assert!(
        roll.get("by_status").is_some() && roll.get("total").is_some(),
        "the digest still says how many checks there are and how they stand: {roll}"
    );
}

#[tokio::test]
async fn the_roll_is_still_reachable_and_the_pointer_to_it_resolves() {
    // The half that matters. Withholding a list is only honest while the
    // instruction for getting it back is true — and `loop_status`'s digest
    // names this route, so a rename here breaks a pointer over there.
    let s = with_checks().await;

    let full = j!(s.graph_report(Parameters(
        serde_json::from_value(serde_json::json!({ "include_verifications": true })).unwrap()
    )));
    let roll = &full["verifications"];
    assert!(roll.is_array(), "asked for, the roll is the list: {roll}");
    assert_eq!(
        roll.as_array().expect("array").len(),
        3,
        "and it is every check, not a sample"
    );

    let digest = j!(s.graph_report(Parameters(GraphReportReq::default())))["verifications"].clone();
    let pointer = digest["full_list"]
        .as_str()
        .expect("a route back")
        .to_string();
    assert!(
        pointer.contains("include_verifications"),
        "the digest must name the flag that returns the roll, or the route is stale: {pointer}"
    );

    let loop_pointer =
        j!(s.loop_status(Parameters(LoopScopeReq::default())))["verifications"]["full_list"]
            .as_str()
            .expect("loop_status points somewhere too")
            .to_string();
    assert!(
        loop_pointer.contains("include_verifications"),
        "loop_status sends readers to graph_report for the roll; that instruction has to name \
         the flag as well: {loop_pointer}"
    );
}

#[tokio::test]
async fn the_default_stops_growing_with_the_check_count_and_the_roll_does_not() {
    // NOT "the default is smaller" — on a three-check design it is BIGGER, and
    // that is correct rather than a bug: the digest carries a fixed sentence
    // saying how to get the roll back, which outweighs three short rows. On a
    // design that small nothing is struggling to be read.
    //
    // The property that actually makes this a fix is that the digest is FLAT in
    // the number of checks while the roll is LINEAR. That is what turns 166,934
    // characters into 15,480 on a design with 197 of them, and it is what a
    // regression would break.
    let s = with_checks().await;
    let small_default = size(&j!(s.graph_report(Parameters(GraphReportReq::default()))));
    let small_full = size(&j!(s.graph_report(Parameters(
        serde_json::from_value(serde_json::json!({ "include_verifications": true })).unwrap()
    ))));

    // PASSING on purpose, and the reason is the limit of this fix. The digest
    // keeps every check that is NOT currently passing in full, because that is
    // what a reader acts on — so it is flat in the PASSING remainder, not in the
    // check count as such. On reflow2's own design 196 of 197 pass and the
    // saving is 166,934 → 15,480; on a design mid-build with two hundred
    // `planned` checks this report would be large again. That case is real,
    // unmeasured, and deliberately not built for
    // (`fact:a-report-is-not-a-list-of-every-check`).
    for i in 0..40 {
        let id = format!("ver:extra-{i}");
        let _ = s
            .add_verification(Parameters(
                serde_json::from_value(serde_json::json!({
                    "id": id,
                    "name": format!("Check number {i} of the padding, which exists to make the \
                                     roll grow while the digest does not"),
                }))
                .unwrap(),
            ))
            .await
            .expect("tool ok");
        let _ = s
            .set_verification_status(Parameters(
                serde_json::from_value(serde_json::json!({
                    "verification_id": id, "status": "passing", "last_run_at": "2026-08-23"
                }))
                .unwrap(),
            ))
            .await
            .expect("tool ok");
    }

    let big_default = size(&j!(s.graph_report(Parameters(GraphReportReq::default()))));
    let big_full = size(&j!(s.graph_report(Parameters(
        serde_json::from_value(serde_json::json!({ "include_verifications": true })).unwrap()
    ))));

    let default_growth = big_default.saturating_sub(small_default);
    let full_growth = big_full.saturating_sub(small_full);
    assert!(
        full_growth > default_growth * 4,
        "40 more PASSING checks grew the roll by {full_growth} and the digest by \
         {default_growth}; the digest is supposed to be flat in the passing remainder"
    );
    assert!(
        big_default < big_full,
        "past a handful of checks the default must be the cheaper one ({big_default} vs \
         {big_full})"
    );
}

#[tokio::test]
async fn nothing_else_in_the_report_moved() {
    // The roll is the only field this touches. A rollup that quietly lost a
    // section while getting smaller would be the wrong kind of fix.
    let s = with_checks().await;
    let default = j!(s.graph_report(Parameters(GraphReportReq::default())));
    let full = j!(s.graph_report(Parameters(
        serde_json::from_value(serde_json::json!({ "include_verifications": true })).unwrap()
    )));

    // `loop_hint` is deliberately excluded: `dec:read-hint-shape` option C
    // throttles it so a persisting debt appears once and then stays quiet, so
    // the SECOND read in this test legitimately does not carry it. Comparing it
    // would pin the throttle rather than the report.
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v
            .as_object()
            .expect("object")
            .keys()
            .filter(|k| *k != "loop_hint")
            .cloned()
            .collect();
        k.sort();
        k
    };
    let a = keys(&default);
    assert_eq!(a, keys(&full), "both forms carry the same sections");

    for key in &a {
        if key == "verifications" {
            continue;
        }
        let key = key.as_str();
        assert_eq!(
            default[key], full[key],
            "`{key}` differs between the two forms, and only `verifications` should"
        );
    }
}
