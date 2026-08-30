//! A typed constructor that lands on an existing node SAYS WHAT IT REPLACED.
//!
//! # Why this exists
//!
//! Constructors merge (BL-183), and `search_first` deliberately goes quiet on a
//! revision because a node's resemblance to itself is noise. Nothing filled that
//! silence: a merge onto an existing id and a fresh create returned **the same
//! shape**, with no signal that anything was replaced and no prior value.
//!
//! Reported four times, by three agents, across two versions and three projects
//! before this was written:
//!
//! - `add_constraint` called twice on one id overwrote a multi-paragraph
//!   statement — *"I lost the prior text of two constraints and could not
//!   honestly reconstruct it."*
//! - A `record_change` snapshot taken after a sibling merge stored the NEW
//!   statement as the prior one — *"the timeline for that revision is a lie."*
//! - An `accepted` Decision was widened from a debugging hypothesis; the user
//!   had to walk it back.
//! - A malformed payload replaced a Decision's text while replying exactly like
//!   a create.
//!
//! Every one was caught — when caught at all — by an agent reading the echoed
//! properties by hand. That is the check this makes structural.
//!
//! # The direction that matters
//!
//! The dangerous failure is echoing the value the node holds NOW and calling it
//! the prior one, because that reads as history and is wrong. `the_prior_value_is_the_old_text_not_the_new_one`
//! is the case that would pass against a naive implementation reading the node
//! after the write, so it is the one to break first when doubting this suite.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value as JsonValue;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

const FIRST: &str = "A dropped packet costs nothing because the outdoor unit sends cumulative \
                     totals rather than deltas, so a lost reading heals itself on the next one.";
const SECOND: &str = "Every reading carries the running total, and the receiver keeps the highest \
                      it has seen, so a gap in the series is closed without a retransmit.";

fn req(id: &str, name: &str, statement: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(name.into()),
        statement: Some(statement.into()),
        distinct_from: None,
    }
}

fn decision(id: &str, name: &str, text: &str, rationale: Option<&str>) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some(name.into()),
        decision: Some(text.into()),
        rationale: rationale.map(str::to_string),
        distinct_from: None,
        kind: None,
    }
}

/// A create has nothing to report, and must not carry an empty block. A section
/// present and empty on every single create is the noise `search_first` already
/// refuses to become.
#[tokio::test]
async fn a_create_says_nothing_about_revision() {
    let s = svc().await;
    let out = j!(s.add_requirement(Parameters(req(
        "req:heals-itself",
        "A lost reading heals itself",
        FIRST
    ))));
    assert!(
        out.get("revision").is_none(),
        "a create must not carry a revision block: {out:?}"
    );
}

/// THE DEFECT CASE. Second call, different statement, same id.
#[tokio::test]
async fn a_replaced_statement_is_named_and_its_prior_value_returned() {
    let s = svc().await;
    j!(s.add_requirement(Parameters(req(
        "req:heals-itself",
        "A lost reading heals itself",
        FIRST
    ))));
    let out = j!(s.add_requirement(Parameters(req(
        "req:heals-itself",
        "A lost reading heals itself",
        SECOND
    ))));

    let rev = out.get("revision").expect("a revision must be reported");
    assert_eq!(rev.get("changed"), Some(&JsonValue::Bool(true)));

    let replaced = rev["replaced"].as_array().expect("replaced is a list");
    assert_eq!(
        replaced.len(),
        1,
        "only `statement` moved — `name` was re-sent unchanged: {replaced:?}"
    );
    assert_eq!(replaced[0]["field"], "statement");
    assert_eq!(
        replaced[0]["prior"], FIRST,
        "the reply must carry the text that was lost, not a hash of it"
    );
    assert!(
        rev["prior_content_hash"]
            .as_str()
            .is_some_and(|h| h.starts_with("sha256:")),
        "a caller needs a fingerprint to prove a restore landed: {rev:?}"
    );
}

/// THE MUTATION-CHECK. An implementation that reads the node AFTER the write
/// and calls that the prior value would pass every other case in this file and
/// be exactly the bug reported from the field.
#[tokio::test]
async fn the_prior_value_is_the_old_text_not_the_new_one() {
    let s = svc().await;
    j!(s.add_requirement(Parameters(req("req:heals-itself", "Heals", FIRST))));
    let out = j!(s.add_requirement(Parameters(req("req:heals-itself", "Heals", SECOND))));
    let prior = &out["revision"]["replaced"][0]["prior"];
    assert_eq!(prior, FIRST, "prior must be the OLD statement");
    assert_ne!(
        prior, SECOND,
        "echoing the value just written as the PRIOR one is the reported defect, \
         not a near miss: it reads as history and is false"
    );
}

/// Enrichment is not overwriting. A second call that only ADDS a property must
/// not be dressed up as a replacement, or every ordinary sharpening cries wolf
/// and the block gets ignored — which is how a warning stops working.
#[tokio::test]
async fn adding_a_property_is_reported_as_added_not_replaced() {
    let s = svc().await;
    j!(s.add_decision(Parameters(decision(
        "dec:cumulative-totals",
        "Send cumulative totals",
        FIRST,
        None
    ))));
    let out = j!(s.add_decision(Parameters(decision(
        "dec:cumulative-totals",
        "Send cumulative totals",
        FIRST,
        Some("Deltas would need a retransmit path nobody wants to own.")
    ))));

    let rev = out.get("revision").expect("still a revision");
    assert_eq!(rev.get("changed"), Some(&JsonValue::Bool(true)));
    assert!(
        rev["replaced"].as_array().expect("list").is_empty(),
        "nothing was overwritten: {rev:?}"
    );
    assert_eq!(
        rev["added"].as_array().expect("list").len(),
        1,
        "rationale arrived: {rev:?}"
    );
    assert_eq!(rev["added"][0], "rationale");
}

/// "I wrote nothing" and "I wrote something" must not be the same reply. This is
/// the same ambiguity reported against `export_graph`'s no-op in the same week,
/// and it is cheap to answer here.
#[tokio::test]
async fn a_revision_that_changed_nothing_says_so() {
    let s = svc().await;
    let call = || req("req:heals-itself", "A lost reading heals itself", FIRST);
    j!(s.add_requirement(Parameters(call())));
    let out = j!(s.add_requirement(Parameters(call())));

    let rev = out
        .get("revision")
        .expect("a no-op revision is still a revision");
    assert_eq!(
        rev.get("changed"),
        Some(&JsonValue::Bool(false)),
        "an identical re-send changed nothing and must say so: {rev:?}"
    );
    assert!(rev["replaced"].as_array().expect("list").is_empty());
    assert!(rev["added"].as_array().expect("list").is_empty());
}

/// The note is what an agent actually reads at call time, so it must name the
/// ordering that makes `record_change` honest — snapshot BEFORE the merge. A
/// snapshot taken afterwards stores the replacement, which is one of the four
/// reports this work came from.
#[tokio::test]
async fn the_note_names_the_record_change_ordering() {
    let s = svc().await;
    j!(s.add_requirement(Parameters(req("req:heals-itself", "Heals", FIRST))));
    let out = j!(s.add_requirement(Parameters(req("req:heals-itself", "Heals", SECOND))));
    let note = out["revision"]["note"].as_str().expect("a note");
    assert!(
        note.contains("record_change") && note.contains("BEFORE"),
        "the remedy must be named at call time, not only in a skill: {note}"
    );
}
