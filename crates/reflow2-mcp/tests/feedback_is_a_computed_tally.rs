//! `usage_report` renders the ledger the server kept — the half of
//! `req:feedback-on-reflow2-is-computed-from-a-record-the-server-kept` that a
//! direct handler call can reach.
//!
//! The ledger is WRITTEN in the server's `call_tool`, which a handler method
//! called directly never passes; that half is pinned over the wire by
//! `tools/test_feedback_is_a_computed_tally.py` against the real binary. What
//! this file pins is the reading side, on a ledger written through the same
//! `usage::append` the server uses:
//!
//! 1. An in-memory design reports NO ledger, in words — never zeros that read
//!    as "nothing happened".
//! 2. On disk, the report covers the window since the last marker and leaves
//!    one, so two reports never count the same call twice; `peek` leaves none.
//! 3. A named `since` date reaches back past the marker, and a malformed one
//!    is refused with the shape it wanted.
//! 4. `never_called` is measured against the surface this server actually
//!    serves, so a 181st tool joins the denominator the day it is served.

use reflow2_mcp::service::*;
use reflow2_mcp::tools::skills_tools::UsageReportReq;
use reflow2_mcp::usage::{self, Outcome, RefusalClass, UsageLine};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("reflow2-feedback-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn report(s: &ReflowService, since: Option<&str>, peek: bool) -> Value {
    s.usage_report(Parameters(UsageReportReq {
        since: since.map(String::from),
        peek,
    }))
    .await
    .expect("usage_report ok")
    .structured_content
    .expect("structured")
}

fn call(at: u64, tool: &str, outcome: Outcome, refusal: Option<RefusalClass>) -> UsageLine {
    UsageLine {
        at,
        kind: "call".into(),
        tool: Some(tool.into()),
        outcome: Some(outcome),
        refusal,
        ms: Some(3),
        client: Some("probe".into()),
        client_version: Some("1".into()),
        seat: Some("seat:test".into()),
        skill: None,
    }
}

/// No store, no "beside", no ledger — and the reply says so instead of
/// reporting an empty tally that would read as a quiet project.
#[tokio::test]
async fn an_in_memory_design_reports_no_ledger_in_words() {
    let s = ReflowService::in_memory().expect("service");
    let r = report(&s, None, false).await;
    assert!(r["ledger"].is_null(), "{r}");
    assert!(
        r["note"].as_str().unwrap_or("").contains("in-memory"),
        "the absence is explained: {r}"
    );
    assert!(r.get("tally").is_none(), "no tally is invented: {r}");
}

/// The window closes behind a report and stays open behind a peek.
#[tokio::test]
async fn a_report_closes_its_window_and_a_peek_does_not() {
    let d = tmpdir("window");
    let gp = d.join("graph");
    let gps = gp.to_str().unwrap();
    let s = ReflowService::new(gps).expect("service on disk");
    usage::append(gps, &call(100, "add_requirement", Outcome::Ok, None));
    usage::append(
        gps,
        &call(
            101,
            "get_node",
            Outcome::Refused,
            Some(RefusalClass::MissingArgument),
        ),
    );

    let first = report(&s, None, false).await;
    assert_eq!(first["tally"]["calls"], 2, "{first}");
    assert_eq!(first["tally"]["window"]["from"], "beginning_of_ledger");
    assert_eq!(first["tally"]["refusals_by_class"]["missing_argument"], 1);
    assert_eq!(
        first["tally"]["refusals_by_tool"]["get_node"]["missing_argument"],
        1
    );
    assert_eq!(first["marker_left"], true);

    // Nothing has happened since; the window is empty and says where it began.
    let second = report(&s, None, true).await;
    assert_eq!(second["tally"]["calls"], 0, "{second}");
    assert_eq!(second["tally"]["window"]["from"], "last_report");
    assert_eq!(second["marker_left"], false);

    // A peek left no marker, so a later real report still starts at the first marker.
    usage::append(gps, &call(102, "loop_status", Outcome::Ok, None));
    let third = report(&s, None, false).await;
    assert_eq!(third["tally"]["calls"], 1, "{third}");
    assert_eq!(third["tally"]["by_tool"]["loop_status"], 1);
    let markers = usage::read_all(gps)
        .iter()
        .filter(|l| l.kind == "report")
        .count();
    assert_eq!(markers, 2, "one per real report, none for the peek");
    std::fs::remove_dir_all(&d).ok();
}

/// A named date wins over the marker, and a bad one is refused with its shape.
#[tokio::test]
async fn a_named_date_reaches_back_and_a_bad_one_is_refused() {
    let d = tmpdir("since");
    let gp = d.join("graph");
    let gps = gp.to_str().unwrap();
    let s = ReflowService::new(gps).expect("service on disk");
    // 2026-09-01 00:00 UTC = 1788566400.
    usage::append(
        gps,
        &call(1_788_566_400 + 10, "search_design", Outcome::Ok, None),
    );
    let _ = report(&s, None, false).await; // closes the window
    usage::append(
        gps,
        &call(1_788_566_400 + 20, "get_skill", Outcome::Ok, None),
    );

    let after_marker = report(&s, None, true).await;
    assert_eq!(after_marker["tally"]["calls"], 1);

    let since_first = report(&s, Some("2026-09-01"), true).await;
    assert_eq!(since_first["tally"]["calls"], 2, "{since_first}");
    assert_eq!(since_first["tally"]["window"]["from"], "named_date");

    let err = s
        .usage_report(Parameters(UsageReportReq {
            since: Some("last tuesday".into()),
            peek: true,
        }))
        .await
        .expect_err("a date the server cannot read is refused");
    assert!(err.to_string().contains("YYYY-MM-DD"), "{err}");
    std::fs::remove_dir_all(&d).ok();
}

/// The denominator of `never_called` is the served surface, read live.
#[tokio::test]
async fn never_called_is_measured_against_the_served_surface() {
    let d = tmpdir("surface");
    let gp = d.join("graph");
    let gps = gp.to_str().unwrap();
    let s = ReflowService::new(gps).expect("service on disk");
    usage::append(gps, &call(1, "loop_status", Outcome::Ok, None));
    let r = report(&s, None, true).await;
    let never: Vec<&str> = r["tally"]["never_called"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(!never.contains(&"loop_status"));
    assert!(never.contains(&"usage_report") && never.contains(&"add_requirement"));
    assert!(
        never.len() >= 170,
        "the surface serves 180+ tools and one was called: {}",
        never.len()
    );
    std::fs::remove_dir_all(&d).ok();
}
