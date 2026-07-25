//! The server that starts when the graph cannot be opened — so a session that
//! has no design brain can find out **why**.
//!
//! From the StoryFlow fleet, 2026-07-25, measured from both sides of a lock. A
//! three-boss fleet pointed every session at one graph; the first to start won
//! the exclusive lock and the rest died at startup, before any tool existed. What
//! the losing sessions saw was not an error — it was *nothing*:
//!
//! ```text
//! reflow2: ✘ Failed to connect — -32000: MCP error -32000: Connection closed
//! ```
//!
//! Zero `reflow2__*` tools, none deferred, and — in the words of the api-boss who
//! wrote it up — *"nothing distinguished this from 'reflow2 was never configured
//! for this project'"*. reflow2's own excellent diagnosis ("another process
//! already has the design graph open… stop that server") went to stderr and died
//! with the process. Recovering it took hand-piping an `initialize` frame into the
//! binary, which is a diagnosis path no ordinary session will ever find.
//!
//! The same silence had already been reached from a different cause on 2026-07-24:
//! a graph refused for schema-version skew also exits at startup. Two causes, one
//! failure class — which is why the fix belongs here, at the plumbing, rather than
//! in each skill.
//!
//! **So: never exit silently.** Complete the MCP handshake, put the diagnosis in
//! the server instructions (which the agent reads as part of its context), and
//! serve exactly one tool whose name is unmistakable. An MCP server that starts
//! and explains itself beats one that dies before it can be asked.
//!
//! What this deliberately is NOT: a read-only mode. Serving the read tools from a
//! locked graph needs RocksDB's secondary-instance open, which lives one layer
//! down in dynograph-storage and is not exposed yet (`req:read-while-held`). This
//! costs nothing to ship and stops the outage being invisible today.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde_json::json;

/// A server with no graph, which knows exactly why and says so.
#[derive(Clone)]
pub struct DegradedService {
    /// The plain-language reason the graph could not be opened — already
    /// translated for a human by `explain_open_failure`.
    reason: String,
    /// The path that was attempted, so the reader can tell WHICH graph failed
    /// when a machine holds several.
    graph_path: String,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

#[tool_router(router = tool_router)]
impl DegradedService {
    pub fn new(reason: String, graph_path: String) -> Self {
        Self {
            reason,
            graph_path,
            tool_router: Self::tool_router(),
        }
    }

    /// The whole point: a tool whose NAME is the diagnosis.
    ///
    /// A session listing tools sees `reflow2_unavailable` and cannot mistake it
    /// for a configuration problem. Calling it returns the reason and the
    /// remedies — in-band, where an agent can act on them.
    #[tool(
        description = "reflow2 is UNAVAILABLE in this session and this is the only tool served. \
                       Call it for the reason and what to do about it. The design graph could not \
                       be opened — most often because another session holds it (the store is \
                       single-writer), or because it was written by a different reflow2. This tool \
                       existing at all means reflow2 IS configured here: do not report the design \
                       brain as missing or unconfigured.",
        annotations(read_only_hint = true)
    )]
    pub async fn reflow2_unavailable(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, McpError> {
        let payload = json!({
            "available": false,
            "graph_path": self.graph_path,
            "reason": self.reason,
            "what_this_means": "reflow2 is configured for this project but this session has no \
                                design graph. Every other reflow2 tool is absent for that reason \
                                alone — not because the project has no design.",
            "remedies": [
                "If another session holds the graph: that session is the writer. Either work \
                 through it, or take your own seat — copy the committed design export into your \
                 own graph path and merge back with the git merge driver (see the parallel-work \
                 skill and AGENTS.md).",
                "If the reason names a version or type mismatch: the graph was written by a \
                 different reflow2. Export it with the build that wrote it, or import a committed \
                 export into a fresh path — and move the sidecar `<graph-path>.meta.json` too, \
                 because the version gate is read from there.",
                "Either way: tell the user the reason above verbatim. It is the one thing they \
                 cannot get from inside this session."
            ],
            "do_not": "Do not report that reflow2 is not installed, not configured, or that the \
                       project has no design graph. All three would be false, and a design-first \
                       process would then be skipped for a reason nobody recorded."
        });
        let text = serde_json::to_string_pretty(&payload)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut result = CallToolResult::structured(payload);
        result.content = vec![ContentBlock::text(text)];
        Ok(result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DegradedService {
    fn get_info(&self) -> ServerInfo {
        // The instructions are the real fix. A client puts them in the agent's
        // context at handshake time, so the reason arrives BEFORE the agent
        // wonders where the tools went — which is the difference between a
        // self-explaining outage and an invisible one.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info({
                let mut info = Implementation::from_build_env();
                info.name = env!("CARGO_PKG_NAME").to_string();
                info.version = env!("CARGO_PKG_VERSION").to_string();
                info
            })
            .with_instructions(format!(
                "reflow2 IS CONFIGURED FOR THIS PROJECT BUT UNAVAILABLE IN THIS SESSION. The \
                 design graph at {} could not be opened:\n\n{}\n\nOnly one tool is served \
                 (`reflow2_unavailable`); every other reflow2 tool is absent for this reason \
                 alone. DO NOT conclude that this project has no design graph, or that reflow2 is \
                 not installed — both would be false, and a design-first process would be skipped \
                 on a false premise. Tell the user the reason above verbatim, and if another \
                 session holds the graph, either work through that session or take your own seat \
                 (own graph path + the git merge driver; see the parallel-work skill).",
                self.graph_path, self.reason
            ))
    }
}
