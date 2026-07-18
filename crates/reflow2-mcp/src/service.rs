//! `ReflowService` — the MCP tool surface over a single reflow2 design graph.
//!
//! Fine-grained, process-grouped tools (surface-plan.md SP-3): the calling agent
//! orchestrates the coherence loop by composing these, exactly as the loop
//! prescribes. Conventions mirrored from the predecessor `ir2` server:
//! - **No result envelope** — a tool returns its payload as JSON directly.
//! - **No silent fallbacks** — partial-success fields (`unknown_seeds`,
//!   `skipped_operations`, `rephrase_degraded`, …) are always present.
//!
//! The deterministic core is synchronous; each tool briefly locks the graph,
//! runs the sync op, and releases — never awaiting while the guard is held.

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use tokio::sync::Mutex;

use reflow2_core::temporal::ChangeRecord;
use reflow2_core::{
    AgentAnswer, AgentBackend, ChangeType, DesignGraph, Dimension, DynoError, EpochType,
    GapCandidate, GenesisOptions, HealOptions, HealProposal, HealStrategy, PromptCollector,
    PropagateOptions, Value,
};

use crate::dto::{EdgeDto, NodeDto};

/// The MCP service: one design graph behind a lock, plus the generated router.
#[derive(Clone)]
pub struct ReflowService {
    graph: Arc<Mutex<DesignGraph>>,
    tool_router: ToolRouter<Self>,
}

// ---- error / result helpers -------------------------------------------------

fn dyno_err(e: DynoError) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn ser_err(e: serde_json::Error) -> McpError {
    McpError::internal_error(format!("failed to serialize result: {e}"), None)
}

/// Return a payload as the tool result: structured JSON (no envelope) plus a
/// text rendering, so clients that read either `structuredContent` or `content`
/// both get the data. Returning a raw `CallToolResult` registers no output
/// schema (the wire format is the payload directly).
fn ok_json<T: serde::Serialize>(value: T) -> Result<CallToolResult, McpError> {
    let v = serde_json::to_value(value).map_err(ser_err)?;
    let text = serde_json::to_string(&v).map_err(ser_err)?;
    let mut result = CallToolResult::structured(v);
    result.content = vec![ContentBlock::text(text)];
    Ok(result)
}

/// Parse a snake_case enum key (the schema vocabulary) into a core enum.
fn parse_enum<T: serde::de::DeserializeOwned>(s: &str, what: &str) -> Result<T, McpError> {
    serde_json::from_value(JsonValue::String(s.to_string()))
        .map_err(|_| McpError::invalid_params(format!("unknown {what}: {s:?}"), None))
}

/// Convert a JSON object of properties into the core's `HashMap<String, Value>`.
fn parse_props(props: Option<JsonValue>) -> Result<HashMap<String, Value>, McpError> {
    match props {
        None | Some(JsonValue::Null) => Ok(HashMap::new()),
        Some(v) => serde_json::from_value(v)
            .map_err(|e| McpError::invalid_params(format!("invalid props object: {e}"), None)),
    }
}

// ---- request shapes ---------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GenesisReq {
    /// Stable Project id (e.g. `proj:softball`).
    pub project_id: String,
    /// Project name.
    pub name: String,
    /// Optional domain hint (software / hardware / document / …).
    #[serde(default)]
    pub domain: Option<String>,
    /// Optional one-line "what success looks like".
    #[serde(default)]
    pub objective: Option<String>,
    /// Project mode: `flexible` (default) or `rigid`.
    #[serde(default)]
    pub mode: Option<String>,
    /// Bootstrap over an existing Project instead of a guarded no-op.
    #[serde(default)]
    pub rescan: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IdName {
    /// Stable node id (e.g. `req:offline`).
    pub id: String,
    /// Human-readable name.
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RequirementReq {
    pub id: String,
    pub name: String,
    /// The requirement statement.
    pub statement: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DescribedReq {
    pub id: String,
    pub name: String,
    /// Capability description / component purpose.
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ContainsReq {
    pub project_id: String,
    /// Child node type (e.g. `Requirement`, `Capability`, `Component`).
    pub child_type: String,
    pub child_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EdgePairReq {
    pub from_id: String,
    pub to_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateNodeReq {
    pub node_type: String,
    pub id: String,
    /// Property object; validated against the schema.
    #[serde(default)]
    pub props: Option<JsonValue>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEdgeReq {
    pub edge_type: String,
    pub from_type: String,
    pub from_id: String,
    pub to_type: String,
    pub to_id: String,
    #[serde(default)]
    pub props: Option<JsonValue>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypedIdReq {
    pub node_type: String,
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanReq {
    pub node_type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PropagateFromReq {
    /// Seed node ids to propagate impact from.
    pub seed_ids: Vec<String>,
    /// Max traversal depth (default 5).
    #[serde(default)]
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PropagateChangeReq {
    pub change_event_id: String,
    #[serde(default)]
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeHealReq {
    /// `conservative` | `balanced` | `aggressive` (default `balanced`).
    #[serde(default)]
    pub strategy: Option<String>,
    /// Cap on structural operations; extras surface in `skipped_operations`.
    #[serde(default)]
    pub max_operations: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApplyHealReq {
    /// A `HealProposal` previously returned by `propose_heal`.
    pub proposal: JsonValue,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeAllocationReq {
    /// Leiden resolution (higher = more, smaller clusters).
    pub resolution: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DimensionDriftReq {
    pub target_id: String,
    /// Quality dimension key (e.g. `reliability`, `security`).
    pub dimension: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddEpochReq {
    pub id: String,
    pub name: String,
    /// `baseline` | `revision` | `milestone` | `incident_response` | `release_cut`.
    pub epoch_type: String,
    pub sequence: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddChangeEventReq {
    pub id: String,
    pub name: String,
    /// Change type key (e.g. `new_feature`, `scope_change`).
    pub change_type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordChangeReq {
    pub epoch_id: String,
    pub change_event_id: String,
    pub name: String,
    pub target_type: String,
    pub target_id: String,
    /// Change type key (e.g. `new_feature`).
    pub change_type: String,
    /// `added` | `modified` | `removed`.
    pub action: String,
}

/// One filled answer from the ambient agent (mirrors core `AgentAnswer` with a
/// JsonSchema for the tool boundary).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentAnswerReq {
    /// The `AgentPrompt.id` this answers.
    pub id: String,
    /// The answer text (JSON string when the prompt expected JSON).
    pub text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GapToPromptReq {
    /// A `GapCandidate` previously returned by `detect_gaps`.
    pub gap: JsonValue,
    /// Answers to a prior `needs_llm` round. Empty on the first (prepare) call.
    #[serde(default)]
    pub answers: Vec<AgentAnswerReq>,
}

// ---- tools ------------------------------------------------------------------

#[tool_router(router = tool_router)]
impl ReflowService {
    /// Open an on-disk (RocksDB) design graph at `path`.
    pub fn new(path: &str) -> Result<Self, DynoError> {
        Ok(Self {
            graph: Arc::new(Mutex::new(DesignGraph::open_rocksdb(path)?)),
            tool_router: Self::tool_router(),
        })
    }

    /// Open an in-memory design graph (tests / dry runs; not persisted).
    pub fn in_memory() -> Result<Self, DynoError> {
        Ok(Self {
            graph: Arc::new(Mutex::new(DesignGraph::open_in_memory()?)),
            tool_router: Self::tool_router(),
        })
    }

    // ---- GENESIS (bootstrap the graph from a brief) ----

    #[tool(
        description = "Bootstrap the design graph: create the Project + a genesis Epoch anchor \
                       and return a next-steps checklist. Guarded and idempotent — a no-op that \
                       reports already_initialized if a Project exists (unless rescan). Call this \
                       first, then seed the brief into Requirements/Capabilities via the add_* \
                       tools and run detect_gaps."
    )]
    pub async fn genesis(
        &self,
        Parameters(req): Parameters<GenesisReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = GenesisOptions {
            project_id: req.project_id,
            name: req.name,
            domain: req.domain,
            objective: req.objective,
            mode: req.mode,
            rescan: req.rescan,
        };
        let mut g = self.graph.lock().await;
        ok_json(g.genesis(opts).map_err(dyno_err)?)
    }

    // ---- DETECT / analyze (deterministic, read-only) ----

    #[tool(description = "Find gaps in the design to ask the human about (DETECT).")]
    pub async fn detect_gaps(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.detect_gaps().map_err(dyno_err)?)
    }

    #[tool(description = "Blast radius of a recorded ChangeEvent along the golden thread.")]
    pub async fn propagate_change(
        &self,
        Parameters(req): Parameters<PropagateChangeReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let g = self.graph.lock().await;
        ok_json(
            g.propagate_change(&req.change_event_id, opts)
                .map_err(dyno_err)?,
        )
    }

    #[tool(description = "Speculative blast radius from seed node ids (what would this touch?).")]
    pub async fn propagate_from(
        &self,
        Parameters(req): Parameters<PropagateFromReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let seeds: Vec<&str> = req.seed_ids.iter().map(String::as_str).collect();
        let g = self.graph.lock().await;
        ok_json(g.propagate_from(&seeds, opts).map_err(dyno_err)?)
    }

    #[tool(description = "The 'what should I look at?' rollup report (SYNTHESIZE).")]
    pub async fn graph_report(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.graph_report().map_err(dyno_err)?)
    }

    #[tool(description = "The graph report rendered as Markdown.")]
    pub async fn graph_report_markdown(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        let report = g.graph_report().map_err(dyno_err)?;
        ok_json(report.to_markdown())
    }

    #[tool(description = "Detect structural defects the machine can repair (HEAL).")]
    pub async fn detect_defects(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.detect_defects().map_err(dyno_err)?)
    }

    #[tool(description = "Propose a HEAL plan (never mutates; review then apply_heal).")]
    pub async fn propose_heal(
        &self,
        Parameters(req): Parameters<ProposeHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let strategy: HealStrategy = match req.strategy.as_deref() {
            None => HealStrategy::default(),
            Some(s) => parse_enum(s, "heal strategy")?,
        };
        let opts = HealOptions {
            strategy,
            max_operations: req.max_operations,
        };
        let g = self.graph.lock().await;
        ok_json(g.propose_heal(opts).map_err(dyno_err)?)
    }

    #[tool(description = "Evaluate how capabilities are allocated across components.")]
    pub async fn evaluate_allocation(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.evaluate_allocation().map_err(dyno_err)?)
    }

    #[tool(description = "Propose a capability→component allocation via Leiden clustering.")]
    pub async fn propose_allocation(
        &self,
        Parameters(req): Parameters<ProposeAllocationReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.propose_allocation(req.resolution).map_err(dyno_err)?)
    }

    #[tool(description = "Decomposition/hierarchy issues (matryoshka level checks).")]
    pub async fn hierarchy_issues(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.hierarchy_issues().map_err(dyno_err)?)
    }

    #[tool(description = "Surprising cross-community couplings (mined from the graph).")]
    pub async fn surprising_connections(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.surprising_connections().map_err(dyno_err)?)
    }

    #[tool(description = "All declining quality dimensions across the design, worst first.")]
    pub async fn dimension_drifts(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        ok_json(g.dimension_drifts().map_err(dyno_err)?)
    }

    #[tool(description = "Quality-dimension drift for one target node.")]
    pub async fn dimension_drift(
        &self,
        Parameters(req): Parameters<DimensionDriftReq>,
    ) -> Result<CallToolResult, McpError> {
        let dim: Dimension = parse_enum(&req.dimension, "dimension")?;
        let g = self.graph.lock().await;
        ok_json(g.dimension_drift(&req.target_id, dim).map_err(dyno_err)?)
    }

    // ---- Golden-thread constructors (deterministic, mutating) ----

    #[tool(description = "Create a Project node.")]
    pub async fn add_project(
        &self,
        Parameters(req): Parameters<IdName>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_project(&req.id, &req.name).map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Create a Requirement node.")]
    pub async fn add_requirement(
        &self,
        Parameters(req): Parameters<RequirementReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_requirement(&req.id, &req.name, &req.statement)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Create a Capability node.")]
    pub async fn add_capability(
        &self,
        Parameters(req): Parameters<DescribedReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_capability(&req.id, &req.name, &req.description)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Create a Component node.")]
    pub async fn add_component(
        &self,
        Parameters(req): Parameters<DescribedReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_component(&req.id, &req.name, &req.description)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Link a Capability to a Requirement it SATISFIES.")]
    pub async fn satisfies(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(EdgeDto::from(
            g.satisfies(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Allocate a Capability to a Component (ALLOCATED_TO).")]
    pub async fn allocate(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(EdgeDto::from(
            g.allocate(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Link a Project to a child node it CONTAINS.")]
    pub async fn contains(
        &self,
        Parameters(req): Parameters<ContainsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(EdgeDto::from(
            g.contains(&req.project_id, &req.child_type, &req.child_id)
                .map_err(dyno_err)?,
        ))
    }

    // ---- Generic CRUD (deterministic) ----

    #[tool(description = "Create a node of any schema type with a property object.")]
    pub async fn create_node(
        &self,
        Parameters(req): Parameters<CreateNodeReq>,
    ) -> Result<CallToolResult, McpError> {
        let props = parse_props(req.props)?;
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.create_node(&req.node_type, &req.id, props)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Create an edge of any schema type between typed endpoints.")]
    pub async fn create_edge(
        &self,
        Parameters(req): Parameters<CreateEdgeReq>,
    ) -> Result<CallToolResult, McpError> {
        let props = parse_props(req.props)?;
        let mut g = self.graph.lock().await;
        ok_json(EdgeDto::from(
            g.create_edge(
                &req.edge_type,
                &req.from_type,
                &req.from_id,
                &req.to_type,
                &req.to_id,
                props,
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Fetch a node by type and id (null if absent).")]
    pub async fn get_node(
        &self,
        Parameters(req): Parameters<TypedIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        let node = g.get_node(&req.node_type, &req.id).map_err(dyno_err)?;
        ok_json(node.map(NodeDto::from))
    }

    #[tool(description = "List all nodes of a type.")]
    pub async fn scan_nodes(
        &self,
        Parameters(req): Parameters<ScanReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.lock().await;
        let nodes = g.scan_nodes(&req.node_type).map_err(dyno_err)?;
        ok_json(nodes.into_iter().map(NodeDto::from).collect::<Vec<_>>())
    }

    #[tool(description = "Delete a node by type and id (true if it existed).")]
    pub async fn delete_node(
        &self,
        Parameters(req): Parameters<TypedIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.graph.lock().await;
        ok_json(g.delete_node(&req.node_type, &req.id).map_err(dyno_err)?)
    }

    #[tool(description = "Apply a reviewed HealProposal atomically (rigid mode = no-op).")]
    pub async fn apply_heal(
        &self,
        Parameters(req): Parameters<ApplyHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let proposal: HealProposal = serde_json::from_value(req.proposal)
            .map_err(|e| McpError::invalid_params(format!("invalid HealProposal: {e}"), None))?;
        let mut g = self.graph.lock().await;
        ok_json(g.apply_heal(&proposal).map_err(dyno_err)?)
    }

    // ---- Temporal / CHANGE (deterministic, mutating) ----

    #[tool(description = "Create an Epoch (a point on the time axis).")]
    pub async fn add_epoch(
        &self,
        Parameters(req): Parameters<AddEpochReq>,
    ) -> Result<CallToolResult, McpError> {
        let epoch_type: EpochType = parse_enum(&req.epoch_type, "epoch type")?;
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_epoch(&req.id, &req.name, epoch_type, req.sequence)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Create a ChangeEvent (seed for propagate_change).")]
    pub async fn add_change_event(
        &self,
        Parameters(req): Parameters<AddChangeEventReq>,
    ) -> Result<CallToolResult, McpError> {
        let change_type: ChangeType = parse_enum(&req.change_type, "change type")?;
        let mut g = self.graph.lock().await;
        ok_json(NodeDto::from(
            g.add_change_event(&req.id, &req.name, change_type)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(description = "Record a change to a node in an epoch (snapshots the prior state).")]
    pub async fn record_change(
        &self,
        Parameters(req): Parameters<RecordChangeReq>,
    ) -> Result<CallToolResult, McpError> {
        let change_type: ChangeType = parse_enum(&req.change_type, "change type")?;
        let action = parse_enum(&req.action, "change action")?;
        let rec = ChangeRecord {
            epoch_id: &req.epoch_id,
            change_event_id: &req.change_event_id,
            name: &req.name,
            target_type: &req.target_type,
            target_id: &req.target_id,
            change_type,
            action,
        };
        let mut g = self.graph.lock().await;
        let (prior, current) = g.record_change(rec).map_err(dyno_err)?;
        ok_json(json!({
            "prior_snapshot": prior.map(NodeDto::from),
            "current": NodeDto::from(current),
        }))
    }

    // ---- LLM handshake (SP-2 collect-then-serve) ----

    #[tool(
        description = "Phrase a gap as a plain question via the ambient agent. \
                       Call with empty `answers` to get {status:needs_llm, prompts}; \
                       fill them and call again with `answers` to get {status:ok, prompt}."
    )]
    pub async fn gap_to_prompt(
        &self,
        Parameters(req): Parameters<GapToPromptReq>,
    ) -> Result<CallToolResult, McpError> {
        let gap: GapCandidate = serde_json::from_value(req.gap)
            .map_err(|e| McpError::invalid_params(format!("invalid GapCandidate: {e}"), None))?;

        if req.answers.is_empty() {
            // Prepare pass: harvest the prompt the op would issue.
            let collector = PromptCollector::new();
            let _discarded = gap.to_prompt(&collector);
            return ok_json(json!({
                "status": "needs_llm",
                "prompts": collector.collected(),
            }));
        }

        // Serve pass: replay the op with the agent's answers.
        let answers = req.answers.into_iter().map(|a| AgentAnswer {
            id: a.id,
            text: a.text,
        });
        let backend = AgentBackend::from_answers(answers);
        let prompt = gap.to_prompt(&backend);
        ok_json(json!({ "status": "ok", "prompt": prompt }))
    }
}

// ---- ServerHandler ----------------------------------------------------------

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ReflowService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "reflow2 is the persistent, coherent design brain. The loop: capture intent as \
                 Requirements/Capabilities/Components via the add_* / create_* tools; run \
                 detect_gaps and ask the human the gaps (gap_to_prompt); build only what the \
                 graph specifies; on any change, add_change_event + propagate_change to see the \
                 blast radius BEFORE editing; use graph_report to decide what to look at.",
            )
    }
}
