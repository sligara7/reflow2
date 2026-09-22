//! What the `content` block of a JSON tool result carries, PER CLIENT.
//!
//! A JSON reply's payload lives in `structuredContent`; `content` carries one
//! sentence naming where it went ([`crate::service::structured_only_signpost`]).
//! That shape was chosen on 2026-08-23 when sending every payload twice made
//! `detect_gaps` 157,785 bytes on the wire and a harness refused it outright.
//! It was chosen against "a client nobody could name" — and the field then
//! named one, three times over: Alex on Grok Build (2026-08-27), Alex on
//! grok-shell 1.0.13 (2026-08-29), and Anthony on Grok Build 1.0.30 on this
//! very repo (2026-09-15), whose session store shows the model receiving the
//! signpost sentence and nothing else for every structured tool. OpenCode
//! (2026-09-14, read from its source) forwards only `content` as well.
//!
//! **NO AUTOMATIC DETECTION IS POSSIBLE.** The negotiated protocol revision is
//! uncorrelated with whether a client READS the field — Grok negotiated the
//! newest revision reflow2 has ever seen and still dropped it — and nothing in
//! MCP asks a client the question. What the server DOES have is the client's
//! own name from `clientInfo`, replayed verbatim through the shared proxy, so
//! the policy is keyed on that, with an explicit override for a client nobody
//! has named yet.
//!
//! WHY THE COST IS NOW BEARABLE. `reply_budget` (2026-09-13) caps every JSON
//! reply at 30,000 characters by default, so duplicating the payload into
//! `content` costs at most twice a BOUNDED reply — not twice the unbounded one
//! that drove the 2026-08-23 decision. The signpost stays the default for
//! every client that reads `structuredContent`, so those pay nothing.
//!
//! MEASURED ON THE SAME GROK SESSION THAT SHOWED THE OUTAGE: two skill tools
//! already put the pretty-printed JSON in the text block as well, and both
//! reached the model in full. `duplicate` is therefore a shape proven on that
//! client, not a guess. `empty` is OpenCode's measured behaviour (it fills the
//! text from `structuredContent` only when the text block is empty) and is
//! applied to no other client until one is measured.
//!
//! This is `dec:idea-how-does-a-content-only-client-get-an-answer` — the
//! decision stays the owner's; the mechanism is what puts a choice in front
//! of him that costs the reading clients nothing.

use std::sync::OnceLock;

use rmcp::model::{CallToolResponse, CallToolResult, ContentBlock};

/// The shape of `content` on a JSON reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentPolicy {
    /// One sentence naming `structuredContent` (the default; the payload once).
    Signpost,
    /// The payload, pretty-printed, in the text block as well — for a client
    /// that hands its model only `content`.
    Duplicate,
    /// No text block at all — for a client that fills the text from
    /// `structuredContent` when, and only when, it is empty.
    Empty,
}

impl ContentPolicy {
    /// Parse the flag's value. Case-insensitive; anything else is `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "signpost" => Some(Self::Signpost),
            "duplicate" => Some(Self::Duplicate),
            "empty" => Some(Self::Empty),
            _ => None,
        }
    }

    /// The flag spelling, for forwarding to a spawned daemon.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signpost => "signpost",
            Self::Duplicate => "duplicate",
            Self::Empty => "empty",
        }
    }
}

static OVERRIDE: OnceLock<ContentPolicy> = OnceLock::new();

/// Set the process-wide override (`--content-policy` / `REFLOW2_CONTENT_POLICY`).
/// First call wins; a second is ignored, because the flag is parsed once.
pub fn set_override(policy: ContentPolicy) {
    let _ = OVERRIDE.set(policy);
}

/// The override, if the operator gave one.
pub fn override_policy() -> Option<ContentPolicy> {
    OVERRIDE.get().copied()
}

/// The policy for a client, by the name it gave at handshake — unless the
/// operator overrode it for every client.
pub fn for_client(client_name: Option<&str>) -> ContentPolicy {
    override_policy().unwrap_or_else(|| by_client_name(client_name))
}

/// The per-client rule, with no override consulted.
///
/// Names are matched by prefix/containment on the lower-cased handshake name,
/// because the same product has reported under more than one: Grok Build
/// names its MCP client `grok-shell-<server>` (`grok-shell-reflow2 1.0.13` in
/// Alex's sidecar), and a test fixture once guessed `grok-build`. Both start
/// with `grok`. OpenCode sends `opencode`.
///
/// ⭐ THE LIST IS OF CLIENTS THAT HAVE BEEN MEASURED, AND THE FALLBACK IS THE
/// SHAPE THAT NEEDS NO MEASUREMENT. Until 2026-09-21 the fallback was
/// `Signpost`, and that was the defect: this rule is keyed BY NAME and cannot
/// learn, so every client nobody had measured got the one shape flo2 showed
/// fails both populations — a content-only reader gets one useless sentence,
/// and an empty-fallback reader is defeated by the text block being non-empty.
/// Every connector is unknown BY CONSTRUCTION, which is precisely the
/// population reflow2 is about to be pointed at.
///
/// `Duplicate` is the honest default because it assumes nothing: the payload
/// is in `content` for a text-only reader and in `structuredContent` for a
/// structured one, and it is never empty, so `req:never-silently-absent`
/// holds. It costs at most twice a BOUNDED reply — `reply_budget` caps a JSON
/// reply at 30,000 characters — which is the cost the 2026-08-23 decision
/// could not have paid against an unbounded one.
///
/// 🛑 SO A NAME BELONGS HERE ONLY ONCE SOMEBODY HAS MEASURED IT. Adding one on
/// a guess re-creates the defect for that client and does it quietly.
/// `claude-code` is matched NARROWLY and deliberately: it reads
/// `structuredContent` and is the daily driver here, so it pays nothing —
/// while `claude-ai` and any other Claude surface are unmeasured connectors
/// and must fall through to the default.
pub fn by_client_name(client_name: Option<&str>) -> ContentPolicy {
    let name = client_name.unwrap_or("").trim().to_ascii_lowercase();
    if name.starts_with("grok") {
        ContentPolicy::Duplicate
    } else if name.contains("opencode") {
        ContentPolicy::Empty
    } else if name.starts_with("claude-code") || name.starts_with("smoke_mcp") {
        // MEASURED READERS OF `structuredContent`. `smoke_mcp` is this repo's
        // own stdio harness: it asserts the signpost shape on the wire, and a
        // harness that silently got a different shape from the one its gates
        // are about would be testing something nobody runs.
        ContentPolicy::Signpost
    } else {
        ContentPolicy::Duplicate
    }
}

/// Is this result exactly the signpost shape — a structured payload plus the
/// one-sentence text block? Only that shape is rewritten: prose tools, tools
/// that already duplicate, and refusals (which arrive in `content` in full)
/// are left exactly as their handler built them.
fn is_signpost_shape(result: &CallToolResult) -> bool {
    result.structured_content.is_some()
        && result.is_error != Some(true)
        && result.content.len() == 1
        && result.content[0]
            .as_text()
            .is_some_and(|t| t.text == crate::service::structured_only_signpost())
}

/// Reshape one result under the policy. Returns whether anything changed.
pub fn apply(policy: ContentPolicy, result: &mut CallToolResult) -> bool {
    if policy == ContentPolicy::Signpost || !is_signpost_shape(result) {
        return false;
    }
    match policy {
        ContentPolicy::Duplicate => {
            let payload = result
                .structured_content
                .as_ref()
                .expect("is_signpost_shape checked structured_content");
            let text =
                serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string());
            result.content = vec![ContentBlock::text(text)];
        }
        ContentPolicy::Empty => {
            result.content = Vec::new();
        }
        ContentPolicy::Signpost => unreachable!("returned above"),
    }
    true
}

/// The `call_tool` hook: shape a finished response for the client that asked.
/// Errors and non-`Complete` responses pass through untouched.
pub fn shape(
    policy: ContentPolicy,
    answer: Result<CallToolResponse, rmcp::ErrorData>,
) -> Result<CallToolResponse, rmcp::ErrorData> {
    answer.map(|response| match response {
        CallToolResponse::Complete(mut result) => {
            apply(policy, &mut result);
            CallToolResponse::Complete(result)
        }
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn signposted(payload: serde_json::Value) -> CallToolResult {
        crate::service::json_result(payload).expect("json_result")
    }

    /// The three named populations, and the default for everyone else.
    #[test]
    fn the_policy_is_keyed_on_the_handshake_name() {
        assert_eq!(
            by_client_name(Some("grok-shell-reflow2")),
            ContentPolicy::Duplicate
        );
        assert_eq!(by_client_name(Some("Grok-Build")), ContentPolicy::Duplicate);
        assert_eq!(by_client_name(Some("opencode")), ContentPolicy::Empty);
        assert_eq!(by_client_name(Some("claude-code")), ContentPolicy::Signpost);
        assert_eq!(by_client_name(Some("smoke_mcp")), ContentPolicy::Signpost);
    }

    /// ⭐ AN UNMEASURED CLIENT GETS THE SHAPE THAT WORKS WITHOUT KNOWING HOW IT
    /// RENDERS A RESULT — the default is `Duplicate`, not `Signpost`.
    ///
    /// Until 2026-09-21 the fallback was `Signpost`, and
    /// `fact:the-per-client-reply-shape-exists-and-its-DEFAULT-is-the-one-shape-\
    /// that-serves-neither-population-and-every-new-connector-is-unknown`
    /// measured what that cost: the policy is keyed BY NAME and cannot learn,
    /// so every client not called grok or opencode got the one shape flo2
    /// showed fails both populations — a content-only client reads one useless
    /// sentence, and an empty-fallback client is defeated by the text block
    /// being non-empty. Every connector is unknown BY CONSTRUCTION: Claude,
    /// ChatGPT and any future one have never handshaked here.
    ///
    /// `Duplicate` is the only shape that needs no knowledge of the client: the
    /// payload is in `content` for a text-only reader AND in
    /// `structuredContent` for a structured one, and it is never empty, so
    /// `req:never-silently-absent` still holds. It costs at most twice a
    /// BOUNDED reply, because `reply_budget` caps a JSON reply at 30,000
    /// characters.
    #[test]
    fn an_unmeasured_client_gets_the_payload_not_a_signpost() {
        for name in [
            Some("claude-ai"),
            Some("chatgpt"),
            Some("some-connector-nobody-has-measured"),
            None,
        ] {
            assert_eq!(
                by_client_name(name),
                ContentPolicy::Duplicate,
                "an unmeasured client ({name:?}) must get the payload in the text block: the \
                 policy is keyed by name and cannot learn, so a signpost default is what every \
                 new connector gets, and it is the one shape that serves neither population"
            );
        }
    }

    /// The other half, and the reason this is not a blanket flip: a client
    /// MEASURED to read `structuredContent` still pays nothing. Claude Code is
    /// the daily driver on this project and reads the structured half, so
    /// duplicating every payload into its text block would double the bytes
    /// for no reader. The narrow match matters — `claude-code` is measured,
    /// `claude-ai` is a connector nobody has measured, and the test above
    /// requires them to differ.
    #[test]
    fn a_measured_structured_reader_still_pays_nothing() {
        assert_eq!(by_client_name(Some("claude-code")), ContentPolicy::Signpost);
        assert_eq!(
            by_client_name(Some("Claude-Code 2.1")),
            ContentPolicy::Signpost
        );
    }

    /// `duplicate` puts the SAME payload in the text block, byte-for-byte as
    /// JSON — what a content-only client's model then reads is the answer.
    #[test]
    fn duplicate_puts_the_payload_in_the_text_block() {
        let payload = json!({"answer": 42, "items": ["a", "b"]});
        let mut r = signposted(payload.clone());
        assert!(apply(ContentPolicy::Duplicate, &mut r));
        let text = r.content[0].as_text().expect("text").text.clone();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            payload
        );
        assert_eq!(
            r.structured_content,
            Some(payload),
            "structuredContent is kept"
        );
    }

    /// `empty` sends no text block, and the structured payload still.
    #[test]
    fn empty_sends_no_text_block() {
        let mut r = signposted(json!({"k": 1}));
        assert!(apply(ContentPolicy::Empty, &mut r));
        assert!(r.content.is_empty());
        assert!(r.structured_content.is_some());
    }

    /// Only the signpost shape is touched: prose, already-duplicated and
    /// error results keep the content their handler chose.
    #[test]
    fn only_the_signpost_shape_is_rewritten() {
        let mut prose = crate::service::ok_markdown("# a document".into());
        assert!(!apply(ContentPolicy::Duplicate, &mut prose));
        assert_eq!(prose.content[0].as_text().unwrap().text, "# a document");

        let mut dup = CallToolResult::structured(json!({"k": 1}));
        dup.content = vec![ContentBlock::text("{\"k\":1}")];
        assert!(!apply(ContentPolicy::Empty, &mut dup));
        assert_eq!(dup.content.len(), 1);

        let mut err = CallToolResult::error(vec![ContentBlock::text("missing field `id`")]);
        err.structured_content = Some(json!({"why": "x"}));
        assert!(!apply(ContentPolicy::Duplicate, &mut err));
        assert_eq!(err.content[0].as_text().unwrap().text, "missing field `id`");

        let mut sign = signposted(json!({"k": 1}));
        assert!(!apply(ContentPolicy::Signpost, &mut sign));
    }

    #[test]
    fn the_flag_value_parses_case_insensitively_and_refuses_the_rest() {
        assert_eq!(
            ContentPolicy::parse("Duplicate"),
            Some(ContentPolicy::Duplicate)
        );
        assert_eq!(ContentPolicy::parse(" empty "), Some(ContentPolicy::Empty));
        assert_eq!(
            ContentPolicy::parse("signpost"),
            Some(ContentPolicy::Signpost)
        );
        assert_eq!(ContentPolicy::parse("both"), None);
    }
}
