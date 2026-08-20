//! `query` tools — one slice of the MCP surface.
//!
//! Split out of `service.rs` under BL-181, which had grown to 6,356 lines and
//! 139 tools in one file: the design distinguished the systems these tools
//! serve and the build did not separate them at all. That mismatch is what
//! `granularity_report` reported, and this is the answer to it.
//!
//! **Function is unchanged by construction.** Every item here moved verbatim;
//! nothing was rewritten. `rmcp` composes routers, so this module declares its
//! own and `ReflowService::new` sums them — the surface a client sees is
//! byte-identical, which `tools/toolsnap.py` is what proves rather than claims.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use tokio::sync::RwLock;

use reflow2_core::bulk::{
    AskedRecord as BulkAskedRecord, ChecksumAccept as BulkChecksumAccept, EdgeSpec as BulkEdgeSpec,
    GapAck as BulkGapAck, NodeSpec as BulkNodeSpec,
};
use reflow2_core::temporal::ChangeRecord;
use reflow2_core::{
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, DEFAULT_SCOPE_DEPTH, DesignGraph,
    Dimension, DriftDisposition, DynoError, EpochType, GapCandidate, GenesisOptions, HealOptions,
    HealProposal, HealStrategy, IngestOptions, LinkArtifactOptions, LoopStatus, ObservedArtifact,
    ObservedPath, PromptCollector, PropagateOptions, ReadinessForecast, ReadinessGate,
    ReadinessKind, ReadinessObservation, ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

#[tool_router(router = query_router, vis = "pub")]
impl ReflowService {
    // ---- Generic CRUD (deterministic) ----

    #[tool(
        description = "Create a node of any schema type with a property object. An existing id MERGES: the props you pass overwrite, every stored property you omit survives — so a partial props object edits, it does not reset the rest to defaults. READ `undeclared` IN THE REPLY: it names any property you sent that the schema does not declare for this type. The write still SUCCEEDS — the store is a property bag on purpose, so a design can record what reflow2 never anticipated — but a typo and a deliberate extension used to be indistinguishable, and this is how you tell them apart. Absent when there is nothing to say. ⚠️ EDITING SOMETHING YOU READ? PASS `expected_content_hash` — the `revision.prior_content_hash` from when you read it. The write then becomes a COMPARE-AND-SWAP and is REFUSED if the node moved in between, naming both hashes, instead of silently overwriting whoever wrote it meanwhile. Without it a shared graph loses updates by luck: measured from both sides of one real collision, the write returned a normal success and THE WINNER WAS NEVER TOLD. The `revision` block reports an overwrite AFTER the fact; this prevents it. Opt-in, because a caller who never read the node has no honest expectation to state.",
        annotations(read_only_hint = false)
    )]
    pub async fn create_node(
        &self,
        Parameters(req): Parameters<CreateNodeReq>,
    ) -> Result<CallToolResult, McpError> {
        let props = parse_props(req.props)?;
        let mut g = self.write_lock().await;
        // Computed BEFORE the write, from what the caller actually sent: an
        // upsert merges, so reading the stored node afterwards would also
        // report undeclared properties somebody else wrote earlier, and blame
        // this caller for them.
        let undeclared = g.undeclared_properties(&req.node_type, &props);
        // Read BEFORE the write so the `revision` block can report what moved —
        // and, since 2026-08-17, so it can hand back the `prior_content_hash`
        // that `expected_content_hash` needs.
        //
        // ⚠️ THIS TOOL CARRIED NO `revision` BLOCK AT ALL until the surface
        // probe went looking for it: the block was attached by the TYPED
        // constructors and never by generic `create_node`. So the
        // compare-and-swap shipped, for one commit, with its precondition
        // value unobtainable from the very tool that demanded it. The core
        // tests all passed; only driving the real surface found it, which is
        // AGENTS.md's "compiling is not the finish line" earning its place
        // again.
        let prior = g.get_node(&req.node_type, &req.id).ok().flatten();
        // COMPARE-AND-SWAP when the caller stated what they read, plain upsert
        // when they did not. The refusal is the point: `revision` already told
        // the LOSER of a collision afterwards and told the winner nothing, and
        // reporting a lost update is not the same as not losing it
        // (req:a-write-cannot-silently-lose-someone-elses-work).
        let written = match req.expected_content_hash.as_deref() {
            Some(expected) => g.upsert_node_if_unchanged(&req.node_type, &req.id, props, expected),
            None => g.upsert_node(&req.node_type, &req.id, props),
        };
        match written {
            Ok(n) => {
                let mut dto = NodeDto::from(n);
                dto.undeclared = undeclared;
                let revision = crate::tools::capture::revision_of(&g, prior.as_ref(), &dto);
                crate::tools::capture::with_capture_notes(
                    dto,
                    "loop: a generic write is still a design change — run detect_gaps \
                     (detect-and-ask) when the batch lands",
                    None,
                    revision,
                )
            }
            Err(e) => Err(node_error(&g, &req.node_type, e)),
        }
    }

    #[tool(
        description = "Create or update MANY nodes in one call — the bulk form of create_node. \
                       ALL OF IT OR NONE OF IT: every item is attempted so you learn every \
                       failure in one round trip, and if anything failed nothing is written. \
                       Upsert, like create_node, so re-running after a fix is safe.",
        annotations(read_only_hint = false)
    )]
    pub async fn create_nodes(
        &self,
        Parameters(req): Parameters<CreateNodesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut specs = Vec::with_capacity(req.nodes.len());
        for n in req.nodes {
            specs.push(BulkNodeSpec {
                node_type: n.node_type,
                id: n.id,
                props: parse_props(n.props)?,
            });
        }
        let mut g = self.write_lock().await;
        let report = g.create_nodes(&specs).map_err(dyno_err)?;
        bulk_result(report, NodeDto::from)
    }

    #[tool(
        description = "Create MANY edges in one call — the bulk form of create_edge, and so of \
                       every typed helper built on it: contains, contain_component, satisfies, \
                       allocate, realizes. Those helpers only fill in the endpoint types, so \
                       naming both types per item is the whole difference. ALL OF IT OR NONE OF \
                       IT: every item is attempted so you learn every failure at once, and if \
                       anything failed nothing is written.",
        annotations(read_only_hint = false)
    )]
    pub async fn create_edges(
        &self,
        Parameters(req): Parameters<CreateEdgesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut specs = Vec::with_capacity(req.edges.len());
        for e in req.edges {
            specs.push(BulkEdgeSpec {
                edge_type: e.edge_type,
                from_type: e.from_type,
                from_id: e.from_id,
                to_type: e.to_type,
                to_id: e.to_id,
                props: parse_props(e.props)?,
            });
        }
        let mut g = self.write_lock().await;
        let report = g.create_edges(&specs).map_err(dyno_err)?;
        bulk_result(report, EdgeDto::from)
    }

    #[tool(
        description = "Create an edge of any schema type between typed endpoints.",
        annotations(read_only_hint = false)
    )]
    pub async fn create_edge(
        &self,
        Parameters(req): Parameters<CreateEdgeReq>,
    ) -> Result<CallToolResult, McpError> {
        let props = parse_props(req.props)?;
        let mut g = self.write_lock().await;
        let edge = g.create_edge(
            &req.edge_type,
            &req.from_type,
            &req.from_id,
            &req.to_type,
            &req.to_id,
            props,
        );
        match edge {
            Ok(e) => ok_json(EdgeDto::from(e)),
            // Say what would have worked — see `edge_error`.
            Err(e) => Err(edge_error(&g, &req.from_type, &req.to_type, e)),
        }
    }

    #[tool(
        description = "Discover the design vocabulary before writing to it: which node types \
                       exist, which properties they require, and which edge types may join two \
                       given types. Call this instead of guessing at create_node / create_edge. \
                       No arguments returns everything; `node_type` focuses one type and the \
                       edges it can carry; `from` + `to` together answer 'what may connect an X \
                       to a Y?', ranking edge types that model the pair above ones that merely \
                       accept it through a `*` wildcard.",
        annotations(read_only_hint = true)
    )]
    pub async fn describe_schema(
        &self,
        Parameters(req): Parameters<DescribeSchemaReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        match (&req.node_type, &req.from, &req.to) {
            (None, None, None) => ok_json(g.describe_vocabulary()),
            (Some(t), None, None) if req.required_only => {
                ok_json(g.describe_node_type_required(t).map_err(params_err)?)
            }
            (Some(t), None, None) => ok_json(g.describe_node_type(t).map_err(params_err)?),
            (None, Some(f), Some(t)) => ok_json(g.edge_types_between(f, t).map_err(params_err)?),
            // A half-given pair is a mistake, not a request for everything.
            _ => Err(McpError::invalid_params(
                "describe_schema takes no arguments (the full vocabulary), `node_type` alone, \
                 or `from` and `to` together — not a mix."
                    .to_string(),
                None,
            )),
        }
    }

    #[tool(
        description = "Fetch a node by type and id — `{node: {...}}` when present, \
                       `{node: null}` when absent. An unknown `node_type` is REFUSED rather \
                       than answered `null`, because \"no such type\" and \"no such node\" are \
                       different facts and must not share one reply. Carries `discontinued`: \
                       true when an ACCEPTED Decision has withdrawn this node (OBSOLETES). \
                       READ IT — the stored `status` still records what was BUILT, so a \
                       withdrawn capability goes on saying `realized` and only this field \
                       tells you the thing is gone.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_node(
        &self,
        Parameters(req): Parameters<TypedIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        // A WRONG TYPE NAME and an ABSENT NODE used to answer identically: a
        // bare `null`. dev_storyflow (w-c216679a, 2026-08-09) asked for
        // `get_node("Epoch", …)` — the stored type is `DesignEpoch` — read the
        // null as "it isn't there", and their brief then told them to mint a
        // second epoch it explicitly forbids. They caught it only because they
        // distrusted the null.
        //
        // The type name is checkable against the schema for free, so answering
        // `null` for it is a fact the server HAS and declines to give. Same
        // class as `unmet_needs: 0` meaning "we never said we needed anything"
        // — a zero that cannot be told from an absence.
        if g.describe_node_type(&req.node_type).is_err() {
            let vocabulary = g.describe_vocabulary();
            let known: Vec<&str> = vocabulary
                .node_types
                .iter()
                .map(|n| n.node_type.as_str())
                .collect();
            // `Epoch` → `DesignEpoch` is the reported case, and containment
            // either way catches the whole family of dropped/added prefixes.
            let asked = req.node_type.to_ascii_lowercase();
            let near: Vec<&str> = known
                .iter()
                .copied()
                .filter(|n| {
                    let n = n.to_ascii_lowercase();
                    !asked.is_empty() && (n.contains(&asked) || asked.contains(&n))
                })
                .collect();
            let hint = if near.is_empty() {
                String::new()
            } else {
                format!(" Did you mean {}?", near.join(" or "))
            };
            return Err(McpError::invalid_params(
                format!(
                    "`{}` is not a node type in this schema, so `null` here would mean \"no \
                     such TYPE\" rather than \"no such node\" — and those are different facts.\
                     {hint}\n\nKnown node types: {}.\n\nCall `describe_schema` for the full \
                     vocabulary.",
                    req.node_type,
                    known.join(", ")
                ),
                None,
            ));
        }
        let node = g.get_node(&req.node_type, &req.id).map_err(dyno_err)?;
        // One named shape both ways (BL-57): `{node: {...}}` when present,
        // `{node: null}` when absent. Before, present returned a bare object
        // and absent returned `{value: null}` (the scalar wrap) — two shapes,
        // so an agent branching on the result read the absent case wrong.
        let node = match node {
            Some(stored) => decorate(&g, stored_to_value(stored.clone())?, &stored.node_id)?,
            None => JsonValue::Null,
        };
        self.ok_read(&g, json!({ "node": node }))
    }

    #[tool(
        description = "List nodes of a type. Answers with as many as fit in one reply and says \
                       what it left out — `total` is how many exist, `omitted` how many did not \
                       come back, `next_offset` where to resume, and `capped_by` why it stopped \
                       (`size` when the payload was full, `limit` when you asked for fewer). A \
                       cap is never silent, but it is also never a surprise: pass `brief: true` \
                       for id/name/status only when you want the shape of a large type, or \
                       `limit`/`offset` to page deliberately. On a mature design the full \
                       properties of one type can be tens of thousands of characters — read \
                       brief first, then fetch the few nodes you actually need with get_node. \
                       Every node carries `discontinued` (brief included): true when an \
                       ACCEPTED Decision has withdrawn it. The stored `status` records what was \
                       BUILT and does not move on withdrawal, so filtering a list on `status` \
                       alone will count things that no longer exist. PASS `level` TO ASK FOR ONE \
                       RUNG OF THE DECOMPOSITION LADDER — `component` / `subsystem` / `system` / \
                       `system_of_systems` / `enterprise`. That is how you ask for \"the \
                       top-level boxes\": deriving them from the CONTAINS spine instead returns \
                       leaves nobody wired to a parent, which is a different set and a \
                       confidently wrong one.",
        annotations(read_only_hint = true)
    )]
    pub async fn scan_nodes(
        &self,
        Parameters(req): Parameters<ScanReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let mut nodes = g.scan_nodes(&req.node_type).map_err(dyno_err)?;
        // `level` narrows to one rung of the decomposition ladder. Refused on
        // any other type and on an unknown rung, rather than answered with an
        // empty list: "no Components at that level" and "that is not a level"
        // are different facts, and the second one silently reads as the first.
        if let Some(level) = req.level.as_deref() {
            const LEVELS: [&str; 5] = [
                "component",
                "subsystem",
                "system",
                "system_of_systems",
                "enterprise",
            ];
            if req.node_type != reflow2_core::nodes::node::COMPONENT {
                return Err(McpError::invalid_params(
                    format!(
                        "`level` narrows the decomposition ladder and only `Component`                          carries one, so it cannot filter `{}`. Drop `level`, or scan                          `Component`.",
                        req.node_type
                    ),
                    None,
                ));
            }
            if !LEVELS.contains(&level) {
                return Err(McpError::invalid_params(
                    format!(
                        "`{}` is not a decomposition level. The ladder is: {}.",
                        level,
                        LEVELS.join(" ▸ ")
                    ),
                    None,
                ));
            }
            nodes.retain(|n| {
                n.properties
                    .get("level")
                    .and_then(|v| v.as_str())
                    // The schema defaults an unset level to `component`, so an
                    // older node with no level must still answer to it.
                    .unwrap_or("component")
                    == level
            });
        }
        let nodes = nodes;
        let total = nodes.len();
        let offset = req.offset.unwrap_or(0).min(total);
        let brief = req.brief.unwrap_or(false);

        // Render one node at a time, stopping at whichever bound bites first:
        // the caller's `limit`, or the payload budget. The budget exists because
        // an unbounded read of a mature type does not fail loudly — it arrives
        // as tens of thousands of characters that the client truncates, which is
        // the silent drop rule 6 forbids, happening outside reflow2 where
        // nothing can name it. Naming it here is the whole point.
        let mut items: Vec<JsonValue> = Vec::new();
        let mut bytes = 0usize;
        let mut capped_by: Option<&'static str> = None;
        for node in nodes.iter().skip(offset) {
            if req.limit.is_some_and(|limit| items.len() >= limit) {
                capped_by = Some("limit");
                break;
            }
            let rendered = if brief {
                brief_node(node)
            } else {
                stored_to_value(node.clone())?
            };
            // Decorated BEFORE the size is measured, so the payload budget
            // accounts for what actually goes out. Measuring the bare node and
            // then growing it is how a "bounded" read quietly exceeds its bound.
            //
            // NOT PINNED BY A TEST, and said so rather than left to look
            // verified: the ordering is reasoning, not measurement. Reversing
            // these two lines kills no test in
            // `discontinued_reaches_the_reader.rs`, because catching it needs a
            // graph sitting within ~22 bytes per node of SCAN_PAYLOAD_BUDGET_BYTES.
            // The overrun it would cause is bounded and small (one short bool
            // per returned node), which is why this is a note and not a gate.
            let rendered = decorate(&g, rendered, &node.node_id)?;
            let size = rendered.to_string().len();
            // Always return at least one node: a single node larger than the
            // whole budget must still be readable, or a big node becomes
            // unreachable rather than merely expensive.
            if !items.is_empty() && bytes + size > SCAN_PAYLOAD_BUDGET_BYTES {
                capped_by = Some("size");
                break;
            }
            bytes += size;
            items.push(rendered);
        }

        let returned = items.len();
        let next = offset + returned;
        self.ok_read(
            &g,
            json!({
                // `count` keeps its established meaning — how many came back in
                // this reply — so a caller that only reads {count, items} is
                // unaffected. `total` is the new, larger truth.
                "count": returned,
                "items": items,
                "total": total,
                "offset": offset,
                "returned": returned,
                "omitted": total.saturating_sub(next),
                "next_offset": (next < total).then_some(next),
                "capped_by": capped_by,
                "brief": brief,
            }),
        )
    }

    #[tool(
        description = "Find the reflow2 tool for a job you can describe but cannot name — \
                       'how do I record that a file implements a capability?', 'what shows me \
                       the blast radius?'. Ranked over the served surface itself (name, \
                       description and parameter names), so it can never drift from the tools \
                       that actually exist. The whole surface is too large to hold in context at \
                       once; this is its catalogue. Descriptions come back trimmed — call the \
                       tool you picked, or read its full schema, once you know its name.",
        annotations(read_only_hint = true)
    )]
    pub async fn find_tools(
        &self,
        Parameters(req): Parameters<FindToolsReq>,
    ) -> Result<CallToolResult, McpError> {
        let query = req.query.to_lowercase();
        let terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|t| !t.is_empty())
            .collect();
        let all = self.tool_router.list_all();
        let searched = all.len();
        // Document frequency over the whole served surface, computed per call
        // rather than cached: the surface is fixed at startup but small enough
        // that recomputing is cheaper than a cache that could go stale against
        // a router somebody composed differently.
        let corpus: Vec<(String, String)> = all
            .iter()
            .map(|t| {
                (
                    t.name.to_lowercase(),
                    t.description.as_deref().unwrap_or("").to_lowercase(),
                )
            })
            .collect();
        let weighted = term_weights(&terms, &corpus);

        let mut scored: Vec<(f64, JsonValue)> = all
            .iter()
            .filter_map(|tool| {
                let name = tool.name.as_ref();
                let description = tool.description.as_deref().unwrap_or("");
                let params = tool
                    .input_schema
                    .get("properties")
                    .and_then(JsonValue::as_object)
                    .map(|p| p.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                let score = score_tool(name, description, &params, &weighted);
                (score > 0.0).then(|| {
                    (
                        score,
                        json!({
                            "tool": name,
                            "score": score,
                            "summary": trim_summary(description),
                            "parameters": params,
                        }),
                    )
                })
            })
            .collect();

        // Ties broken by name so the same query answers the same way twice —
        // a ranking that reshuffles teaches an agent not to trust it.
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1["tool"].as_str().cmp(&b.1["tool"].as_str()))
        });
        let matched = scored.len();
        let limit = req.limit.unwrap_or(DEFAULT_TOOL_SEARCH_RESULTS).max(1);
        let items: Vec<JsonValue> = scored.into_iter().take(limit).map(|(_, v)| v).collect();

        ok_json(json!({
            "count": items.len(),
            "items": items,
            "matched": matched,
            "omitted": matched.saturating_sub(items.len()),
            "searched": searched,
            "query": req.query,
        }))
    }

    #[tool(
        description = "Find design nodes by what they say, when you don't know their ids — \
                       'what does the design say about persistence?', 'is there already a \
                       requirement about latency?'. BM25 keyword search over every node's \
                       name/statement/description, ranked, optionally scoped to one node type. \
                       Search BEFORE creating a node that might already exist, and to map the \
                       user's words to the node they mean. Result reports its own bounds: \
                       hits.len() == limit means there may be more, and a non-empty `stale` \
                       list means the index has drifted from the store.",
        annotations(read_only_hint = true)
    )]
    pub async fn search_design(
        &self,
        Parameters(req): Parameters<SearchDesignReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let result = g
            .search_design(
                &req.query,
                req.node_type.as_deref(),
                req.limit.unwrap_or(10),
            )
            .map_err(dyno_err)?;
        self.ok_read(&g, result)
    }

    #[tool(
        description = "Delete a node by type and id (true if it existed).",
        annotations(read_only_hint = false)
    )]
    pub async fn delete_node(
        &self,
        Parameters(req): Parameters<TypedIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        let deleted = g.delete_node(&req.node_type, &req.id).map_err(dyno_err)?;
        ok_json(json!({ "deleted": deleted }))
    }

    #[tool(
        description = "Delete one edge by type and endpoint ids (true if it existed). For \
                       retracting a link that was drawn in error — a wrongly-asserted SATISFIES, \
                       an allocation that never happened. A link that WAS true and stopped being \
                       true is design history, not an error: record it (record_change) rather \
                       than erasing it. Until this tool existed the only way to remove a wrong \
                       edge over MCP was to delete one of its endpoints.",
        annotations(read_only_hint = false)
    )]
    pub async fn delete_edge(
        &self,
        Parameters(req): Parameters<DeleteEdgeReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        // `{deleted}` rather than the bare bool the core returns: a scalar in
        // `structuredContent` is the BL-48 defect (ok_json would wrap it as an
        // anonymous `{value}`, but the field deserves its name).
        let deleted = g
            .delete_edge(&req.edge_type, &req.from_id, &req.to_id)
            .map_err(dyno_err)?;
        ok_json(json!({ "deleted": deleted }))
    }
}

/// Render a stored node the way the read tools have always rendered it.
fn stored_to_value(node: StoredNode) -> Result<JsonValue, McpError> {
    serde_json::to_value(NodeDto::from(node)).map_err(ser_err)
}

/// Add the one fact a stored node cannot carry: **has this been withdrawn?**
///
/// # Why this exists, measured 2026-08-12
///
/// `dec:idea-discontinued-is-a-first-class-state` gave `OBSOLETES` four readers
/// — three capability detectors and delivery arithmetic — and every one of them
/// is a COMPUTATION. None was a READ. So `cap:content-store`, discontinued on
/// Anthony's word on 2026-08-09 with its code deleted, still came back from
/// `scan_nodes` as `status: "realized"`, and a session believed it. It then
/// recommended he build a surface for a feature he had personally removed three
/// days earlier.
///
/// The graph was right and the detectors were right. The reader was told
/// nothing — the same class of defect as `get_node` answering a bare `null` for
/// an unknown TYPE: a fact the server holds and declines to give.
///
/// # Derived, never stored
///
/// Computed from the edge on every read, exactly as the detectors compute it,
/// so a reader and a detector can never disagree about the same node. Nothing
/// is written back: `Capability.status` keeps recording what was BUILT, and
/// `dec:idea-does-a-capability-need-a-cancelled-state` — open, marked, and
/// Anthony's — is not settled by implementation here.
///
/// # Present and false, never absent
///
/// Emitted on every node including live ones. "Not discontinued" and "this
/// build does not report discontinuation" must not share one answer, which is
/// the rule `severed_containment` and `not_observed_about` already follow.
fn decorate(
    g: &DesignGraph,
    mut rendered: JsonValue,
    node_id: &str,
) -> Result<JsonValue, McpError> {
    let discontinued = g.is_discontinued(node_id).map_err(dyno_err)?;
    if let Some(obj) = rendered.as_object_mut() {
        obj.insert("discontinued".to_string(), json!(discontinued));
    }
    Ok(rendered)
}
