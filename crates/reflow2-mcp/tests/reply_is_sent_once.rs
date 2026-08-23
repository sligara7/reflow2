//! A JSON reply goes out once, and says where it went.
//!
//! Every tool result used to carry its payload TWICE, byte for byte — once as
//! `structuredContent` and once as a text `content` block — on the reasoning
//! that a client may read either. Nobody measured what that cost until
//! 2026-08-23: unscoped `detect_gaps` was 79,566 characters of payload and
//! 157,785 bytes on the wire, and the harness refused the call. Half of it was
//! a copy no client read.
//!
//! The tax was on every reply this server has ever sent, which is why
//! `graph_report` and `loop_status` are refused as well.
//!
//! What is pinned here is the pair of properties that make the saving safe:
//! the structured payload is WHOLE, and the `content` block is a SIGNPOST
//! rather than either a duplicate or a silence. An empty block would save the
//! same bytes and leave a client reading the wrong field with nothing —
//! indistinguishable from reflow2 never having been configured, which is the
//! outage `req:never-silently-absent` exists to end.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

/// A design with something in it, so `detect_gaps` has a payload worth
/// measuring against — an empty one would make the signpost look small for the
/// wrong reason.
async fn seeded() -> ReflowService {
    // Deliberately unlike one another: the near-duplicate guard refuses a
    // requirement that reads like one already there, and twelve numbered copies
    // of one sentence is exactly what it exists to catch.
    const NEEDS: [(&str, &str); 8] = [
        (
            "brakes",
            "The vehicle stops within 40 metres from 100 km/h on dry tarmac.",
        ),
        (
            "telemetry",
            "Position is broadcast to the ground station once per second.",
        ),
        (
            "battery",
            "A full charge lasts eight hours of continuous operation.",
        ),
        ("mass", "The airframe weighs under 25 kilograms fuelled."),
        (
            "thermal",
            "Avionics stay under 70 degrees at maximum ambient.",
        ),
        (
            "comms",
            "The link survives a 30-second dropout without losing state.",
        ),
        (
            "audit",
            "Every command is recorded with the operator who issued it.",
        ),
        (
            "recovery",
            "Power loss mid-flight leaves the airframe recoverable.",
        ),
    ];
    let s = ReflowService::in_memory().expect("in-memory service");
    for (id, statement) in NEEDS {
        let _ = s
            .add_requirement(Parameters(
                serde_json::from_value(serde_json::json!({
                    "id": format!("req:{id}"),
                    "name": statement,
                    "statement": statement,
                }))
                .unwrap(),
            ))
            .await
            .expect("tool ok");
    }
    s
}

/// The text block of a JSON tool result.
fn content_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|block| block.as_text())
        .map(|text| text.text.clone())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_json_reply_does_not_carry_its_payload_twice() {
    let s = seeded().await;
    let result = s
        .detect_gaps(Parameters(GapScopeReq::default()))
        .await
        .expect("tool ok");

    let structured = result
        .structured_content
        .clone()
        .expect("the payload is the structured field");
    let payload = serde_json::to_string(&structured).expect("serializes");
    let text = content_text(&result);

    assert!(
        !text.contains("\"items\""),
        "the text block is carrying the payload again: {text:.200}"
    );
    assert!(
        text.len() < payload.len(),
        "a signpost that is not smaller than what it points at has saved nothing \
         (signpost {}, payload {})",
        text.len(),
        payload.len()
    );
}

#[tokio::test]
async fn the_text_block_says_where_the_payload_went() {
    // Not empty. A client reading the wrong field must get an instruction, not
    // a silence — silence is indistinguishable from reflow2 not being there.
    let s = seeded().await;
    let result = s
        .detect_gaps(Parameters(GapScopeReq::default()))
        .await
        .expect("tool ok");
    let text = content_text(&result);

    assert!(
        !text.is_empty(),
        "an empty content block is the silent failure"
    );
    assert!(
        text.contains("structuredContent"),
        "the signpost must NAME the field to read: {text}"
    );
}

#[tokio::test]
async fn a_prose_tool_still_returns_its_document_in_the_text_block() {
    // The saving must not reach the tools for which `content` is the ONLY
    // carrier. graph_report_markdown declares no structured output at all, so
    // a signpost there would replace the answer with a note about the answer.
    let s = seeded().await;
    let result = s.graph_report_markdown().await.expect("tool ok");

    assert!(
        result.structured_content.is_none(),
        "a Markdown document has no structure to declare"
    );
    let text = content_text(&result);
    assert!(
        text.contains('#'),
        "the document itself, not a signpost: {text:.200}"
    );
    assert!(
        !text.contains("structuredContent"),
        "a prose tool must not be signposted somewhere it never wrote: {text:.200}"
    );
}
