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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeDesignsReq {
    /// Store paths to describe — the same value `--graph-path` takes, e.g.
    /// `/repo/.reflow2/graph`. YOU find these by walking the tree (reflow2 does
    /// no file navigation); pass every candidate at once rather than one per
    /// call, because the point is to show a person a menu.
    pub paths: Vec<String>,
}

/// The shared body of `describe_designs`, served on both the latent surface and
/// the full one. A session with NO design is exactly where this matters most —
/// it is the moment someone is about to create one — so it cannot live only on
/// the surface you get after a design already exists.
pub(crate) fn describe_designs_payload(paths: &[String]) -> serde_json::Value {
    let found: Vec<reflow2_core::DesignAtPath> =
        paths.iter().map(|p| reflow2_core::describe_at(p)).collect();
    let named = found
        .iter()
        .filter(|d| d.state == reflow2_core::DesignPathState::Design)
        .count();
    json!({
        "described": found.len(),
        "designs_found": named,
        "results": found,
        "how_to_read_this": "state `design` means a real design lives there and can be named. \
                             `unnamed` means something is there whose identity could not be read \
                             WITHOUT opening the store — and opening would mint one, so it is not \
                             opened. `opted_in` means the directory exists and nothing is written \
                             yet. `absent` means nothing is there.",
        "nothing_was_opened": "This read only the sidecar files beside each store. No store was \
                               opened, no lock taken, and nothing was written — so it is safe \
                               against a design another session is holding right now, and it \
                               cannot create the thing it was asked to look for.",
        "no_sizes": "Node counts are deliberately absent: counting means opening the store, which \
                     writes a schema stamp and mints an identity when there is none. Naming a \
                     design by the act of inspecting it is the failure this exists to prevent."
    })
}

#[tool_router(router = tool_router)]
impl LatentService {
    pub fn new(graph_path: String) -> Self {
        Self {
            graph_path,
            tool_router: Self::tool_router(),
        }
    }

    /// What design lives at each of these paths — without opening any of them.
    #[tool(
        description = "Say what design lives at each given path, WITHOUT opening or writing \
                       anything. Call this BEFORE reflow2_start_design, every time. YOU find the \
                       candidate paths — `find . -maxdepth 3 -name .reflow2` and the same upward \
                       — because reflow2 does no file navigation; this answers what each one IS. \
                       WHY IT EXISTS: a session opened at a repo root was told 'no design here' \
                       and started a THIRD design while two populated ones sat one and two \
                       directories below. Nothing could say what they were. Returns the design's \
                       stable id, its label, whether it was minted or adopted, and the schema \
                       stamp — enough to put a menu in front of the user. It reads only the \
                       sidecar files beside each store: no lock is taken, nothing is written, and \
                       a design another session is holding right now describes fine. Node counts \
                       are deliberately absent because counting would mean opening, and opening \
                       MINTS an identity where there is none — naming a design by the act of \
                       looking at it is the very failure this prevents."
    )]
    pub async fn describe_designs(
        &self,
        Parameters(req): Parameters<DescribeDesignsReq>,
    ) -> Result<CallToolResult, McpError> {
        if req.paths.is_empty() {
            return Err(McpError::invalid_params(
                "describe_designs needs at least one path. Walk the tree first — \
                 `find . -maxdepth 3 -name .reflow2` — and pass what you found; an empty sweep \
                 reported as 'nothing here' is the answer that starts an unwanted design."
                    .to_string(),
                None,
            ));
        }
        let payload = describe_designs_payload(&req.paths);
        let text = serde_json::to_string_pretty(&payload)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let mut result = CallToolResult::structured(payload);
        result.content = vec![ContentBlock::text(text)];
        Ok(result)
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
                       create requirements. ⚠️ LOOK BEFORE YOU START: run \
                       `find . -maxdepth 3 -name .reflow2` and the same upward from here, then \
                       describe_designs on whatever you find, and put any existing design to the \
                       user BEFORE calling this. 'No design HERE' is not 'no design NEARBY' — a \
                       session that skipped this started a third design on a repo that already \
                       had two, one and two directories down, and nobody noticed until later. \
                       Starting one is cheap to do and awkward to undo, so the check is not \
                       optional even when the user sounds certain."
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

        // ⚠️ RE-PROBE FIRST. The surface was chosen when this process started,
        // and a design can APPEAR under it afterwards — the documented restore
        // path does exactly that: `reflow2-mcp --graph-path … --import …` builds
        // a full store in a directory this server was told was empty ninety
        // seconds earlier (music_graph F24, 2026-08-16). Everything the server
        // said at handshake was true when printed and false by the time anyone
        // acted on it, and the ONE tool on offer was the one that starts a
        // design — over the top of the one that now exists.
        //
        // This reads only the sidecar files (`describe_at` opens no store and
        // takes no lock), so it is safe against a design another session is
        // holding, and it cannot mint an identity by looking.
        let found = reflow2_core::describe_at(&self.graph_path);
        if matches!(
            found.state,
            reflow2_core::DesignPathState::Design | reflow2_core::DesignPathState::Unnamed
        ) {
            let payload = json!({
                "started": false,
                "a_design_is_already_here": true,
                "graph_path": self.graph_path,
                "design": found,
                "what_happened": "A design exists at this path NOW. This server was started when \
                                  it did not, which is why it is serving the one-tool surface — \
                                  the surface is chosen once at startup and does not re-probe.",
                "next_step": "Ask the user to fully restart this client (a /mcp reconnect is not \
                              always enough — the stdio server must be replaced). The new server \
                              will see the design and serve the full surface.",
                "nothing_was_created": "This call did NOT create or modify anything. Starting a \
                                        design over one that already exists is the mistake this \
                                        refusal exists to prevent.",
                "if_you_just_restored_this": "This is the expected state after \
                                              `reflow2-mcp --graph-path <path> --import <export>`. \
                                              The restore worked; only this session's view of it \
                                              is stale. Do NOT re-import, and do NOT report the \
                                              restore as failed."
            });
            let text = serde_json::to_string_pretty(&payload)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let mut result = CallToolResult::structured(payload);
            result.content = vec![ContentBlock::text(text)];
            return Ok(result);
        }

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
                       away and the graph is where design belongs.",
            "if_you_did_not_check_nearby": "Say so to the user NOW, before going further. This \
                                            call only created a directory — nothing has been \
                                            designed — so if a design already exists one or two \
                                            levels away, the recovery is to point this project at \
                                            THAT one and remove the directory just made. Waiting \
                                            until work has been captured makes it a merge instead \
                                            of a deletion."
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
                 call `reflow2_start_design`, then follow the single next step it returns.\n\n\
                 ⚠️ THIS SENTENCE HAS A SHELF LIFE. It was true when this server started and \
                 nothing re-checks it. A design can appear underneath — restoring one with \
                 `reflow2-mcp --graph-path {} --import <export>` builds a full store in seconds, \
                 and this server goes on serving the one-tool surface and reporting no design. If \
                 you have just restored, or anything else has written here since: the restore \
                 worked, this view is stale, and a full client restart is what attaches the design \
                 surface. Do not re-import and do not report the restore as failed. \
                 `reflow2_start_design` re-probes and will tell you if this has happened.",
                self.graph_path, self.graph_path
            ))
    }
}
