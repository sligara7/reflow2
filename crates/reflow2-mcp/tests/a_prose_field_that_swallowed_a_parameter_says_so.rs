//! A prose field that swallowed the next parameter says so — and the write still lands.
//!
//! # Why this exists
//!
//! `dec:idea-a-prose-field-silently-absorbed-tool-call-markup` (ACCEPTED
//! 2026-08-31, Anthony's word). A capture call whose tool-call markup is
//! malformed at the tail puts the NEXT parameter's opening tag inside the
//! CURRENT parameter's prose, and that parameter then never arrives. The write
//! SUCCEEDS. What lands is a node whose body ends in `</decision>` and which has
//! no `rationale` at all — and nothing noticed: not the schema (a string is a
//! string), not `undeclared` (the property is declared and simply absent), not
//! the revision block (it faithfully reports replacing one property and cannot
//! know the replacement is corrupt), not the dedup guard, not `detect_defects`.
//!
//! Seven observed instances across two projects and two graphs: five in
//! reflow2's own design, and two in `proj:bhome` — a consumer project with no
//! connection to reflow2's subject matter, which hit it on `add_decision` and
//! `add_change_event` and independently proposed this exact fix.
//!
//! # The shape it keys on, and why not the obvious one
//!
//! 🛑 IT DOES NOT MATCH THE MARKUP APPEARING AT ALL. The decision's own
//! counter-argument to that is fatal and was measured before any code: three
//! nodes in this design legitimately quote `</decision>` and `<parameter name=`
//! on purpose — including the decision node that documents the bug and the two
//! findings behind it. A rule matching the strings would fire on exactly the
//! records that explain the defect.
//!
//! So it keys on the LOSS, not the syntax: a prose field names a parameter
//! **that this tool declares** and **that this call did not supply**. Measured
//! against the committed export before implementing:
//!
//! ```text
//!   naive "contains the markup at all"             3 false positives
//!   "names a parameter the node lacks"             2   (both TemporalFacts, a type with no `rationale`)
//!   type-scoped to the tool's own declared params  0   across 1,776 nodes
//! ```
//!
//! # It WARNS, it never refuses
//!
//! A property bag accepting whatever arrives is arguably correct behaviour, and
//! a refusal here would be a new way to lose a write. The reply carries an
//! advisory block beside `search_first` and `revision`; the node lands either
//! way. That is option B's severity on option A's keying, which is what the
//! decision records as accepted.
//!
//! ⚠️ OUT OF SCOPE BY DESIGN: `create_node`. Its `props` is one JSON object with
//! no declared prose parameters, so "was the named parameter supplied?" has no
//! meaning there — and that scoping is exactly what took the middle row of the
//! table above from 2 false positives to 0. Measured 2026-08-31: zero instances
//! of the damage shape from any writer across 3,528 nodes, and `create_node`
//! already carries `undeclared` plus a JSON parse that refuses structural
//! corruption loudly. Option C, a CI scan over the committed export, remains
//! available and is the right home for that case if it is ever wanted.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn decision(id: &str, decision: &str, rationale: Option<&str>) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some("A choice".into()),
        decision: Some(decision.into()),
        rationale: rationale.map(Into::into),
        kind: None,
        distinct_from: None,
        related_to: None,
        no_relation_note: None,
    }
}

fn block(v: &serde_json::Value) -> Option<&serde_json::Value> {
    v.get("absorbed_markup")
}

/// THE REGRESSION, in the `<parameter name=` form — the shape observed on
/// `dec:idea-parks-shipped-...` and on both of bhome's instances.
#[tokio::test]
async fn a_swallowed_parameter_is_named_in_the_reply() {
    let s = svc().await;
    let body = "We chose the cheap option because it is reversible.\
                </decision><parameter name=\"rationale\">Because reversibility \
                is worth more than elegance here.";

    let out = s
        .add_decision(Parameters(decision("dec:swallowed", body, None)))
        .await
        .expect("the write must still LAND — this warns, it never refuses");
    let v = out.structured_content.expect("structured content");

    let b = block(&v).expect("a swallowed parameter must be reported: {v}");
    let text = b.to_string();
    assert!(
        text.contains("rationale"),
        "the block must name the parameter that never arrived: {text}"
    );
    assert!(
        text.contains("decision"),
        "and the field it was swallowed into: {text}"
    );
}

/// THE SECOND OBSERVED FORM — `</decision>` then a bare `<rationale>` rather
/// than a `<parameter name=` tag. Same loss, different emission.
#[tokio::test]
async fn the_abbreviated_tag_form_is_caught_too() {
    let s = svc().await;
    let body = "Ship the narrow fix.</decision><rationale>It is cheaper and \
                reversible.</rationale></invoke>";

    let out = s
        .add_decision(Parameters(decision("dec:abbrev", body, None)))
        .await
        .expect("still lands");
    let v = out.structured_content.expect("structured content");
    assert!(
        block(&v).is_some(),
        "the abbreviated form loses the same parameter and must be reported: {v}"
    );
}

/// 🛑 THE COUNTERWEIGHT, AND IT IS THE WHOLE REASON THIS KEYS ON LOSS RATHER
/// THAN SYNTAX. A node may legitimately quote the markup — three in this design
/// do, including the decision that documents the defect. Quoting it while
/// SUPPLYING the parameter is correct work and must stay silent.
#[tokio::test]
async fn quoting_the_markup_while_supplying_the_field_is_not_a_finding() {
    let s = svc().await;
    let body = "The defect looks like this: a body ending \
                `</decision><parameter name=\"rationale\">` with no rationale at all.";

    let out = s
        .add_decision(Parameters(decision(
            "dec:documents-the-bug",
            body,
            Some("Recorded so the shape is on the record."),
        )))
        .await
        .expect("ok");
    let v = out.structured_content.expect("structured content");
    assert!(
        block(&v).is_none(),
        "quoting the markup AND supplying the parameter is correct work, not a \
         finding — this is the false positive the naive rule would produce: {v}"
    );
}

/// The same loss on the other tool with an observed instance.
#[tokio::test]
async fn add_change_event_reports_it_too() {
    let s = svc().await;
    let out = s
        .add_change_event(Parameters(AddChangeEventReq {
            id: "chg:swallowed".into(),
            name: Some("Something moved".into()),
            change_type: Some("defect_fix".into()),
            subject: Some("system".into()),
            summary: Some(
                "The guard now warns.</summary><parameter name=\"rationale\">And \
                 here is why."
                    .into(),
            ),
            rationale: None,
            affected: None,
            detected_at: None,
        }))
        .await
        .expect("still lands");
    let v = out.structured_content.expect("structured content");
    assert!(
        block(&v).is_some(),
        "add_change_event lost its rationale the same way and must say so: {v}"
    );
}

/// An ordinary write says nothing. A block present and empty on every capture is
/// the noise the sibling advisory blocks are explicitly built not to become.
#[tokio::test]
async fn an_ordinary_write_carries_no_block() {
    let s = svc().await;
    let out = s
        .add_decision(Parameters(decision(
            "dec:ordinary",
            "Take the reversible option.",
            Some("It costs less to undo."),
        )))
        .await
        .expect("ok");
    let v = out.structured_content.expect("structured content");
    assert!(
        block(&v).is_none(),
        "nothing was swallowed, so nothing should be said: {v}"
    );
}
