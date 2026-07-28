//! The server that starts in a directory which has **not** opted into a design
//! — so reflow2 can be installed once per machine without landing a RocksDB
//! store in every folder you ever open.
//!
//! WHY THIS EXISTS. Anthony, 2026-07-28, after setting up a project and finding
//! that getting to the first design action took an installer invocation, an
//! agent restart, and a command that was not there: *"we need to make this as
//! easy as possible… there are multiple steps after just to get it working."*
//! The answer to per-project setup is to stop having any — register reflow2 once
//! at user scope, and every project has it. But `--graph-path .reflow2/graph` is
//! relative to the working directory and the store is **created if absent**, so
//! a machine-wide registration would create a design graph in every directory a
//! session is ever opened in, including the ones that will never have a design.
//! Litter in someone's repo is a worse first impression than a setup step.
//!
//! THE RULE, and it is deliberately the cheapest one that cannot be wrong: serve
//! the design surface where the graph's own directory ALREADY EXISTS, and serve
//! this instead where it does not. Creating that directory is therefore the
//! whole of "yes, design this project" — done by `reflow2 init`, by a committed
//! design being imported, or by the one tool below.
//!
//! WHAT IT IS NOT. Not the degraded surface: that one means *reflow2 is
//! configured here and could not open the graph*, which is an outage. This means
//! *reflow2 is available here and no design has been started*, which is an
//! ordinary state and by far the most common one on a machine-wide install. The
//! two must not be confused — telling an agent a design failed to open when
//! nobody ever made one would send it hunting a fault that does not exist.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Whether this directory has opted into being designed.
///
/// True when the graph exists, and also when only its parent does — a project
/// set up by `reflow2 init` has `.reflow2/` before it has `.reflow2/graph`, and
/// the store appears on the first write. Checking the parent is what makes
/// "opted in" survive the window between the two.
pub fn design_present(graph_path: &str) -> bool {
    let p = Path::new(graph_path);
    p.exists()
        || p.parent()
            .is_some_and(|d| !d.as_os_str().is_empty() && d.exists())
}

/// A server for a directory with no design, which says so without inventing one.
#[derive(Clone)]
pub struct LatentService {
    graph_path: String,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

#[tool_router(router = tool_router)]
impl LatentService {
    pub fn new(graph_path: String) -> Self {
        Self {
            graph_path,
            tool_router: Self::tool_router(),
        }
    }

    /// Opt this directory in, and say plainly what has to happen next.
    ///
    /// It creates the directory and nothing else — no store, no schema, no
    /// nodes. Opening RocksDB here would take the write lock in the very process
    /// that is about to be replaced, and the design surface this session needs
    /// is served by a *different* process; the honest thing is to make the
    /// opt-in and hand back the one instruction that completes it.
    #[tool(
        description = "Start a design for this directory. reflow2 is installed on this machine but \
                       no design has been started HERE, which is why the design tools are absent. \
                       Call this when the user asks to design, plan or capture requirements for \
                       this project — including via /genesis or /adopt. It creates the design's \
                       directory and returns the one step that finishes the job. It does NOT \
                       create requirements, and it is safe to call when unsure."
    )]
    pub async fn reflow2_start_design(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, McpError> {
        let dir = PathBuf::from(&self.graph_path)
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(&self.graph_path));

        let already = dir.exists();
        if !already && let Err(e) = std::fs::create_dir_all(&dir) {
            // Say which directory and why, rather than a bare io error: the
            // common causes are a read-only checkout and a path the session
            // cannot write, and both are the user's to fix.
            return Err(McpError::internal_error(
                format!(
                    "could not create {} for this project's design: {e}",
                    dir.display()
                ),
                None,
            ));
        }

        let payload = json!({
            "started": !already,
            "design_directory": dir.display().to_string(),
            "graph_path": self.graph_path,
            "next_step": "Ask the user to run /mcp (reconnect the reflow2 server), then continue. \
                          This session is talking to a server that was started before the design \
                          existed; the reconnected one serves the full design surface.",
            "what_this_means": "reflow2 is installed machine-wide and this directory has now opted \
                                into being designed. Nothing has been designed yet — no project, no \
                                requirements. After the reconnect, run the genesis skill for a new \
                                project or the adopt skill for code that already exists.",
            "do_not": "Do not report reflow2 as broken, missing or misconfigured, and do not write \
                       design notes into files as a substitute. The design surface is one reconnect \
                       away and the graph is where design belongs."
        });
        let text = serde_json::to_string_pretty(&payload)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut result = CallToolResult::structured(payload);
        result.content = vec![ContentBlock::text(text)];
        Ok(result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LatentService {
    fn get_info(&self) -> ServerInfo {
        // Said at handshake time, because the agent's first wrong conclusion
        // would otherwise be "reflow2 is not set up here" — which on a
        // machine-wide install is false in a way that costs the user the whole
        // design loop.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info({
                let mut info = Implementation::from_build_env();
                info.name = env!("CARGO_PKG_NAME").to_string();
                info.version = env!("CARGO_PKG_VERSION").to_string();
                info
            })
            .with_instructions(format!(
                "reflow2 IS INSTALLED AND AVAILABLE HERE, AND THIS DIRECTORY HAS NO DESIGN YET. \
                 Nothing has failed: no design graph has ever been started at {}, so the design \
                 tools are not served and exactly one tool is — `reflow2_start_design`.\n\nThis is \
                 the ordinary state of a directory on a machine where reflow2 is installed once \
                 for every project. Do NOT report reflow2 as missing, broken or misconfigured, and \
                 do NOT set up a design unasked: most directories should stay this way.\n\nWhen \
                 the user asks to design, plan, capture requirements, or runs /genesis or /adopt: \
                 call `reflow2_start_design`, then follow the single next step it returns.",
                self.graph_path
            ))
    }
}
