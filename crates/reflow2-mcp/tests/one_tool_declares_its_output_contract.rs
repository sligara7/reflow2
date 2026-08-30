//! The experiment that settles whether a `content`-only client is a client bug
//! or an undeclared contract on OUR side.
//!
//! # What this pins, and why it is deliberately ONE tool
//!
//! Alex reported (2026-08-29, `art:grok-shell-feedback-2026-08-29`) that almost
//! every reflow2 tool result is invisible to his agent: it sees only the
//! `content` stub saying the payload went to `structuredContent`. His client
//! negotiated MCP `2025-11-25` — the SAME revision `claude-code` negotiates and
//! reads without difficulty — which killed the leading explanation (gate the
//! duplication on protocol version) recorded as option (a) of
//! `dec:idea-how-does-a-content-only-client-get-an-answer`.
//!
//! Root-causing that turned up a candidate nobody had considered
//! (`fact:reflow2-sends-a-structured-payload-for-tools-that-declare-no-output-contract`):
//! **reflow2 returns `structuredContent` on every JSON tool and declared
//! `outputSchema` on none of them.** MCP pairs the two — the schema is what
//! tells a client what the structured payload looks like and whether to expect
//! one at all. A client that surfaces structured results ONLY for tools
//! advertising an output contract would behave exactly as he describes.
//!
//! 🛑 THE CANDIDATE IS BOUNDED AND THIS TEST DOES NOT ASSERT IT IS THE CAUSE.
//! Absence of `outputSchema` cannot be SUFFICIENT: `claude-code` reads this
//! server's `structuredContent` fine against the same undeclared surface. The
//! most that can be true is that it is NECESSARY for his client specifically.
//! Only he can take the measurement, so this file's job is to set the two arms
//! up correctly and keep them that way.
//!
//! # The two arms, and why the control matters as much as the subject
//!
//! * SUBJECT — `search_design` declares an output schema. Chosen because it is
//!   the tool he named as blocking the loop: *"'search before you add' cannot
//!   be followed from the search result."*
//! * CONTROL — every other tool declares none, so the two run against ONE
//!   server in ONE session and differ in exactly one property.
//!
//! Without the control the experiment cannot be read: if he upgrades and
//! everything starts working, an all-tools change could not distinguish "the
//! schema did it" from "something else in the release did it".
//!
//! ⚠️ IF THIS TEST FAILS BECAUSE SOMEBODY DECLARED SCHEMAS ON MORE TOOLS, THAT
//! IS THE TEST DOING ITS JOB, NOT AN OBSTACLE. Read the experiment's result
//! first; broadening the declaration before it is read destroys the control and
//! the question goes unanswered. After it is read, delete this file.
//!
//! # Safety
//!
//! Declaring `output_schema` cannot break a tool at runtime: rmcp 3.1.2
//! advertises it in `tools/list` and never validates an outgoing payload
//! against it (checked in the SDK source, not assumed). So a wrong schema
//! misinforms a client; it cannot fail a call.

use reflow2_mcp::service::ReflowService;

/// The tool carrying the declaration — the subject arm.
const SUBJECT: &str = "search_design";

#[test]
fn search_design_declares_an_output_schema() {
    let tools = ReflowService::query_router().list_all();
    let subject = tools
        .iter()
        .find(|t| t.name == SUBJECT)
        .unwrap_or_else(|| panic!("{SUBJECT} is not served by query_router"));

    let schema = subject.output_schema.as_ref().unwrap_or_else(|| {
        panic!(
            "{SUBJECT} declares no output_schema, so the experiment has no subject arm and \
             Alex's session would measure nothing"
        )
    });

    assert_eq!(
        schema.get("type").and_then(|v| v.as_str()),
        Some("object"),
        "MCP defines structuredContent as an object, so the schema's top level must say so"
    );

    let props = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .expect("the declared schema must describe properties, or it tells a client nothing");

    // The three fields SearchResult always serialises. Named individually so a
    // change to the payload shape fails HERE rather than silently shipping a
    // schema that describes a reply reflow2 no longer sends.
    for field in ["hits", "stale", "limit"] {
        assert!(
            props.contains_key(field),
            "the declared schema omits `{field}`, which search_design always returns"
        );
    }
}

/// The control arm. If everything declares a schema there is nothing to compare
/// the subject against, and the experiment cannot be read.
#[test]
fn the_rest_of_the_query_surface_stays_undeclared_so_there_is_a_control() {
    let tools = ReflowService::query_router().list_all();

    let declared: Vec<&str> = tools
        .iter()
        .filter(|t| t.output_schema.is_some())
        .map(|t| t.name.as_ref())
        .collect();

    assert_eq!(
        declared,
        vec![SUBJECT],
        "exactly one tool may declare an output schema while the experiment is running — \
         these declare one: {declared:?}. Broadening the declaration before Alex's result is \
         read destroys the control arm."
    );

    // And there must actually BE a control — a one-tool router would satisfy
    // the assertion above while leaving nothing to compare against.
    let undeclared = tools.iter().filter(|t| t.output_schema.is_none()).count();
    assert!(
        undeclared > 0,
        "no undeclared tool is left on this router, so there is no control arm"
    );
}
