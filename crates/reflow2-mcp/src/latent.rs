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
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_router};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::service::ReflowService;

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

/// A server for a directory with no design, which says so without inventing one
/// — and which becomes the full design server IN PLACE the moment one exists.
///
/// ⭐ WHY IN PLACE, AND NOT BY A RESTART. Until 2026-09-14 the opt-in ended
/// with "ask the user to run /mcp" — Claude Code's reconnect command — and the
/// design surface arrived only when the client replaced this process. Alex,
/// on Grok Build, could not follow it: that client has `/mcps` and no
/// reconnect. His client DID re-query the tool list afterwards; this process
/// still offered two tools. The design's own rule
/// (`rule:mcp-and-the-graph-are-the-only-common-ground`) says reflow2 assumes
/// ONLY that the product can call an MCP server, so finishing opt-in may need
/// nothing outside MCP — and MCP has the mechanism: the server re-probes on
/// every call, opens the store itself, serves the full surface from then on,
/// and sends `notifications/tools/list_changed`. A client restart is the
/// fallback for a client that ignores the notification, never the way.
/// `fact:defect-the-latent-surface-never-re-probes-after-a-cli-restore`
/// (2026-08-15) asked for exactly this and got only its fallback.
#[derive(Clone)]
pub struct LatentService {
    graph_path: String,
    tool_router: ToolRouter<Self>,
    /// The full service, once the design exists. Opened at most once per
    /// process; every call re-probes the directory first.
    full: Arc<tokio::sync::RwLock<Option<ReflowService>>>,
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
            full: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// The full design service, if a design exists here NOW — re-probed on
    /// every call, opened at most once. Best-effort: a store that will not
    /// open leaves the latent surface in place and says why on stderr, rather
    /// than turning "no design yet" into an outage.
    pub(crate) async fn promoted(&self) -> Option<ReflowService> {
        if let Some(full) = self.full.read().await.as_ref() {
            return Some(full.clone());
        }
        if !design_present(&self.graph_path) {
            return None;
        }
        let mut slot = self.full.write().await;
        if slot.is_none() {
            match ReflowService::new_reporting(&self.graph_path) {
                Ok((svc, provenance)) => {
                    if let Some(note) = provenance {
                        eprintln!("reflow2: {note}");
                    }
                    eprintln!(
                        "reflow2: a design now exists at {} — this server serves the full \
                         design surface from here on",
                        self.graph_path
                    );
                    *slot = Some(svc);
                }
                Err(e) => {
                    eprintln!(
                        "reflow2: a design directory exists at {} but the store could not be \
                         opened ({e}); still serving the latent surface",
                        self.graph_path
                    );
                    return None;
                }
            }
        }
        slot.clone()
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

    /// Opt this directory in, and finish the job in this same process.
    ///
    /// It creates the directory, opens the store, and tells the client the
    /// tool list changed — so the design surface is served by THIS process from
    /// the next call on, on any MCP client. Until 2026-09-14 it created the
    /// directory only and handed back "run /mcp", which finished the opt-in on
    /// exactly one client (`fact:field-report-genesis-on-grok-build-cannot-finish-...`).
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
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (result, promoted_now) = self.start_design_detached().await?;
        if promoted_now {
            announce_tool_list_changed(&ctx).await;
        }
        Ok(result)
    }

    /// The opt-in itself, without a client to notify — what the tool above
    /// does before it sends `tools/list_changed`, and what a test with no
    /// peer can drive. Returns the reply and whether this call promoted the
    /// process to the full surface.
    pub async fn start_design_detached(&self) -> Result<(CallToolResult, bool), McpError> {
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
            let promoted_now = self.promoted().await.is_some();
            let payload = json!({
                "started": false,
                "a_design_is_already_here": true,
                "graph_path": self.graph_path,
                "design": found,
                "what_happened": "A design exists at this path NOW. This server was started when \
                                  it did not; it has re-probed, opened that design, and serves \
                                  the full design surface from here on.",
                "next_step": served_next_step(&self.graph_path, promoted_now),
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
            return Ok((result, promoted_now));
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

        // THE OPT-IN FINISHES HERE, INSIDE MCP. The directory exists, so the
        // design is present by the same test the launcher uses; open the
        // store in this process and tell the client its tool list changed.
        // Nothing the user has to type, on any client.
        let promoted_now = self.promoted().await.is_some();
        let payload = json!({
            "started": !already,
            "design_directory": dir.display().to_string(),
            "graph_path": self.graph_path,
            "surface": if promoted_now { "full" } else { "latent" },
            "next_step": served_next_step(&self.graph_path, promoted_now),
            "what_this_means": "reflow2 is installed machine-wide and this directory has now opted \
                                into being designed. Nothing has been designed yet — no project, no \
                                requirements. Run the genesis skill (get_skill genesis) for a new \
                                project or the adopt skill for code that already exists.",
            "do_not": "Do not report reflow2 as broken, missing or misconfigured, and do not write \
                       design notes into files as a substitute. The design surface is served now \
                       and the graph is where design belongs.",
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
        Ok((result, promoted_now))
    }
}

/// The one instruction that finishes the opt-in — written for ANY MCP client.
///
/// It names no client command as the way: the surface is served by this same
/// process and the client has been told the list changed. The restart is the
/// fallback, and the per-product hints ride it as ADDITIONS, which is what
/// `rule:mcp-and-the-graph-are-the-only-common-ground` allows and no more.
fn served_next_step(graph_path: &str, promoted: bool) -> String {
    if promoted {
        format!(
            "The full design surface is SERVED NOW by this same server: it opened the design at \
             {graph_path} and sent the client `notifications/tools/list_changed`. Continue in \
             this session — `get_skill` genesis for a new project, adopt for code that already \
             exists — and read the tool list again if your client caches it. Only if the design \
             tools still do not appear, ask the user to restart this client's reflow2 server \
             (Claude Code: `/mcp` then reconnect; Grok Build: `/mcps`, toggle reflow2 off and on, \
             or restart the TUI; any other client: restart it). Nothing else is needed."
        )
    } else {
        format!(
            "The design directory exists but this server could not open the store at \
             {graph_path} (see its stderr log). Ask the user to restart this client's reflow2 \
             server (Claude Code: `/mcp` then reconnect; Grok Build: `/mcps`, toggle reflow2 off \
             and on, or restart the TUI; any other client: restart it); the fresh server will \
             report the open error in band if it persists."
        )
    }
}

/// Tell the client the tool list changed — the MCP mechanism for exactly this.
/// Best-effort: a client that does not take notifications is not an error,
/// and the next `tools/list` serves the full surface regardless.
async fn announce_tool_list_changed(ctx: &RequestContext<RoleServer>) {
    if let Err(e) = ctx.peer.notify_tool_list_changed().await {
        eprintln!("reflow2: could not send tools/list_changed to the client: {e}");
    }
}

impl ServerHandler for LatentService {
    /// The latent tools, or the full surface once a design exists here.
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        if let Some(full) = self.promoted().await {
            return full.list_tools(request, context).await;
        }
        Ok(ListToolsResult::with_all_items(self.tool_router.list_all()))
    }

    /// `reflow2_start_design` is always answered here (it re-probes and says
    /// whether the surface is served); everything else goes to the full
    /// service once there is one.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name != "reflow2_start_design"
            && let Some(full) = self.promoted().await
        {
            return full.call_tool(request, context).await;
        }
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    fn get_info(&self) -> ServerInfo {
        // Said at handshake time, because the agent's first wrong conclusion
        // would otherwise be "reflow2 is not set up here" — which on a
        // machine-wide install is false in a way that costs the user the whole
        // design loop.
        // `list_changed` is DECLARED, because this server sends it: the moment a
        // design exists here it serves the full surface and says so the MCP way.
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
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
