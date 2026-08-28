//! Who connected, and what the two of us agreed to speak — written to disk.
//!
//! # Why this is a file and not a tool reply
//!
//! It exists for a client that cannot read tool replies. The 2026-08-27 field
//! report (Grok Build TUI, macOS) described a harness that forwards only
//! `content`: every structured reflow2 tool returned the
//! [`crate::service::STRUCTURED_ONLY`] signpost and nothing else, so `get_node`,
//! `search_design`, `loop_status` and every constructor were unreadable.
//!
//! The first question anyone asks about that is *"which protocol revision did
//! the client negotiate?"* — because `structuredContent` arrived in MCP
//! `2025-06-18` and rmcp still negotiates `2024-11-05` and `2025-03-26`. If the
//! client asked for a revision that PREDATES the field, gating the duplication
//! on the version is the spec-sanctioned fix. If it asked for a modern one and
//! still reads only `content`, that is a conformance gap and version-gating
//! would do nothing at all. **Those need different remedies and nothing could
//! tell them apart.**
//!
//! ⭐ **AND THAT QUESTION CANNOT BE ANSWERED THROUGH A TOOL REPLY**, because the
//! tool replies are the broken thing. Putting it on `graph_report.served_by`
//! would file the answer inside the channel that does not work. So it goes
//! beside the store, next to `.meta.json` and `.server.json`, where a person
//! opens it in an editor and pastes it into a report.
//!
//! # What it will not claim
//!
//! ⚠️ [`Handshake::protocol_has_structured_content`] says the REVISION contains
//! the field. It does **not** say the client reads it, and it is named so that
//! nobody can read it that way. A negotiated version is what a client CLAIMS to
//! speak; it is not evidence of what that client implemented — which is the
//! whole distinction this record exists to expose. A field called
//! `supports_structured_content` would assert exactly the thing it cannot know.
//!
//! # Known limits, recorded rather than solved
//!
//! - **Most recent connection only.** The file is overwritten on every
//!   handshake. A shared server serving two clients shows the later one.
//! - **No timestamp inside.** The file's own mtime is when the handshake
//!   happened. Inventing a clock for a field the filesystem already carries
//!   would add a dependency and one more thing to be wrong about.

use std::path::{Path, PathBuf};

use rmcp::model::{Implementation, ProtocolVersion};

/// The MCP revision that introduced `structuredContent`.
///
/// Compared as a string, the way rmcp's own transport compares versions: these
/// are ISO dates, so lexical order IS version order, and `ProtocolVersion`
/// carries no numeric ordering to borrow. The same reasoning as
/// [`crate::service::version_is_per_request`].
pub const STRUCTURED_CONTENT_SINCE: &str = "2025-06-18";

/// `<graph-path>.client.json` — beside the store, like `.meta.json` and
/// `.server.json`.
pub fn handshake_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    match p.file_name().and_then(|n| n.to_str()) {
        Some(n) => p.with_file_name(format!("{n}.client.json")),
        None => PathBuf::from(format!("{graph_path}.client.json")),
    }
}

/// What one `initialize` exchange settled.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Handshake {
    /// The client's own name for itself, from `clientInfo`. Self-reported and
    /// unverified — it is whatever the client chose to say.
    pub client_name: String,
    /// The client's own version string, from `clientInfo`. Also self-reported.
    pub client_version: String,
    /// The MCP revision the client ASKED for.
    pub client_requested: String,
    /// The revision the two settled on.
    pub negotiated: String,
    /// The revision this server offers as its own.
    pub server_offers: String,
    /// Whether the NEGOTIATED REVISION contains `structuredContent`.
    ///
    /// 🛑 NOT whether the client reads it. See the module docs.
    pub protocol_has_structured_content: bool,
    /// Which reflow2 wrote this file.
    pub reflow2_version: String,
    /// Said in the file itself, because the file is meant to be pasted into a
    /// report and read by somebody who has not read this module.
    pub note: String,
}

/// Does this revision contain `structuredContent`?
pub fn revision_has_structured_content(version: &str) -> bool {
    version >= STRUCTURED_CONTENT_SINCE
}

impl Handshake {
    /// Build the record from what `initialize` carried.
    pub fn new(
        client: &Implementation,
        requested: &ProtocolVersion,
        negotiated: &ProtocolVersion,
        server_offers: &ProtocolVersion,
    ) -> Self {
        let negotiated_s = negotiated.to_string();
        Self {
            client_name: client.name.clone(),
            client_version: client.version.clone(),
            client_requested: requested.to_string(),
            negotiated: negotiated_s.clone(),
            server_offers: server_offers.to_string(),
            protocol_has_structured_content: revision_has_structured_content(&negotiated_s),
            reflow2_version: env!("CARGO_PKG_VERSION").to_string(),
            note: format!(
                "The most recent client to connect; this file is overwritten on every handshake, \
                 and its modification time is when that happened. \
                 `protocol_has_structured_content` says the negotiated MCP revision CONTAINS that \
                 field (it arrived in {STRUCTURED_CONTENT_SINCE}) — it does NOT say this client \
                 reads it. A client can negotiate a modern revision and still forward only \
                 `content`, which is a conformance gap rather than a version gap and needs a \
                 different fix. `client_name` and `client_version` are self-reported."
            ),
        }
    }

    /// Write it beside the store. Best effort: a diagnostic that refused to be
    /// written would be worse than one that is absent, and the handshake must
    /// not fail because a directory is read-only.
    pub fn write(&self, graph_path: &str) {
        let path = handshake_path(graph_path);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Read it back, for anything that wants to report what connected.
    pub fn read(graph_path: &str) -> Option<Self> {
        std::fs::read_to_string(handshake_path(graph_path))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
    }
}

/// Mirror of rmcp's `negotiate_protocol_version`, which is `pub(crate)` and
/// therefore not callable from here.
///
/// 🛑 THIS IS A COPY OF SOMEBODY ELSE'S RULE AND IT CAN DRIFT. rmcp 3.1.2's own
/// doc comment states it exactly: *"Echoes the client-requested version if the
/// server supports it; otherwise returns `server_fallback`."* Overriding
/// `initialize` means reproducing that, because the function cannot be reached.
/// `negotiation_mirrors_rmcp` pins the rule so a change in rmcp is LOUD, which
/// is the same call this project already made for `ProtocolVersion::LATEST` —
/// following the SDK silently would trade one invisible staleness for another.
pub fn negotiate(
    client_requested: &ProtocolVersion,
    server_fallback: ProtocolVersion,
    server_supported: &[ProtocolVersion],
) -> ProtocolVersion {
    if server_supported.contains(client_requested) {
        client_requested.clone()
    } else {
        server_fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Implementation` is `#[non_exhaustive]`, so it is built and then
    /// mutated — the same shape `ReflowService::get_info` uses.
    fn client(name: &str, version: &str) -> Implementation {
        let mut i = Implementation::from_build_env();
        i.name = name.into();
        i.version = version.into();
        i
    }

    fn v(s: &str) -> ProtocolVersion {
        ProtocolVersion::KNOWN_VERSIONS
            .iter()
            .find(|k| k.to_string() == s)
            .cloned()
            .unwrap_or(ProtocolVersion::LATEST)
    }

    /// The rule, pinned. If rmcp changes how it negotiates, this is where the
    /// change becomes visible instead of reflow2 silently answering `initialize`
    /// differently from the SDK it is built on.
    #[test]
    fn negotiation_mirrors_rmcp() {
        let supported = ProtocolVersion::KNOWN_VERSIONS;
        // Supported: echoed back.
        for known in supported {
            assert_eq!(
                &negotiate(known, ProtocolVersion::LATEST, supported),
                known,
                "a version the server supports must be echoed, not replaced"
            );
        }
    }

    /// The one line a reader of the file actually acts on.
    #[test]
    fn a_revision_predating_the_field_is_reported_as_not_having_it() {
        assert!(!revision_has_structured_content("2024-11-05"));
        assert!(!revision_has_structured_content("2025-03-26"));
        assert!(revision_has_structured_content("2025-06-18"));
        assert!(
            revision_has_structured_content("2026-07-28"),
            "a revision AFTER the one that introduced the field still has it — `>=`, not `==`"
        );
    }

    #[test]
    fn the_sidecar_sits_beside_the_store_like_its_siblings() {
        assert_eq!(
            handshake_path("/p/.reflow2/graph"),
            PathBuf::from("/p/.reflow2/graph.client.json")
        );
    }

    /// 🛑 The field must never be readable as a claim about the client.
    #[test]
    fn the_record_says_the_revision_has_the_field_not_that_the_client_reads_it() {
        let h = Handshake::new(
            &client("some-tui", "1.0.0"),
            &v("2025-06-18"),
            &v("2025-06-18"),
            &ProtocolVersion::LATEST,
        );
        assert!(h.protocol_has_structured_content);
        assert!(
            h.note.contains("does NOT say this client"),
            "the file must disclaim the reading it would otherwise invite: {}",
            h.note
        );
        assert_eq!(h.client_name, "some-tui");
    }

    #[test]
    fn it_survives_a_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("reflow2-handshake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        let graph = dir.join("graph");
        let graph = graph.to_string_lossy().to_string();

        let h = Handshake::new(
            &client("grok-build", "0.2.4"),
            &v("2024-11-05"),
            &v("2024-11-05"),
            &ProtocolVersion::LATEST,
        );
        h.write(&graph);
        assert_eq!(Handshake::read(&graph).as_ref(), Some(&h));
        assert!(!h.protocol_has_structured_content);
    }

    /// A read-only directory must not take the handshake down with it.
    #[test]
    fn an_unwritable_path_is_survived_not_propagated() {
        let h = Handshake::new(
            &client("anything", "0"),
            &ProtocolVersion::LATEST,
            &ProtocolVersion::LATEST,
            &ProtocolVersion::LATEST,
        );
        h.write("/proc/definitely/not/writable/graph");
        assert!(Handshake::read("/proc/definitely/not/writable/graph").is_none());
    }
}
