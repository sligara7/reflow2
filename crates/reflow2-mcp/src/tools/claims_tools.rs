//! `claims_tools` tools — one slice of the MCP surface.
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

#[tool_router(router = claims_tools_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Take a region of the design in hand so colleagues can see it is held: \
                       `contributor_id` claims everything within `depth` hops of `seed_id`. \
                       ADVISORY, NEVER A LOCK — it does not block anyone, nothing consults it \
                       before a write, and a second person who ignores it still gets a correct \
                       three-way merge. It is also only as fresh as the last pull, because the \
                       design lives as a file in each checkout with no shared server \
                       (dec:multi-writer-architecture). Claims reduce collisions; they do not \
                       prevent them. The region is COMPUTED from seed+depth rather than stored as \
                       a node list, so it follows the design as it changes. Overlapping an \
                       existing claim is allowed and reported by claim_report, never refused.",
        annotations(read_only_hint = false)
    )]
    pub async fn claim_region(
        &self,
        Parameters(req): Parameters<ClaimReq>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // The transport decides, per request, whether this service instance is a
        // session or a one-shot — so the check belongs here and the logic below
        // stays a plain fn the tests can drive both ways.
        self.claim_region_inner(req, identity_is_per_request(&ctx))
            .await
    }

    /// `claim_region` with the transport question already answered, so both
    /// answers are unit-testable without constructing an rmcp `Peer`.
    pub async fn claim_region_inner(
        &self,
        req: ClaimReq,
        identity_is_per_request: bool,
    ) -> Result<CallToolResult, McpError> {
        let seat = self.seat_for_claim(req.seat.as_deref(), identity_is_per_request)?;
        let mut g = self.write_lock().await?;
        ok_json(
            g.claim_region(
                &req.contributor_id,
                &req.seed_id,
                req.depth.unwrap_or(2),
                req.note.as_deref(),
                req.at.as_deref(),
                Some(&seat),
            )
            .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Mint a seat: a durable name for THIS session, to pass as `seat` on the \
                       tools that record who is working (claim_region). Call it once, keep the \
                       value, reuse it — a new seat per call is what it exists to avoid. \
                       WHEN YOU NEED IT: on the sessionless transport (MCP 2026-07-28 and later) \
                       the server builds a handler per REQUEST, so it cannot tell your second \
                       call from another client's first, and claim_region REFUSES rather than \
                       guess. On stdio and on legacy Streamable HTTP the session already supplies \
                       one and you can omit `seat` entirely — calling this anyway is harmless and \
                       works the same, which is why an agent that always mints is never wrong. \
                       Writes NOTHING: a seat is a name assigned with no coordination \
                       (dec:identity-out-of-band), never a lock, and it grants no rights.",
        annotations(read_only_hint = true)
    )]
    pub async fn mint_seat(&self) -> Result<CallToolResult, McpError> {
        let seat = reflow2_core::identity::mint_seat();
        ok_json(json!({
            "seat": seat,
            // Said in-band because the reason to keep it is not obvious from the
            // value, and an agent that re-mints per call reintroduces the bug.
            "carry_it": "Pass this as `seat` on claim_region for the rest of this session. \
                         Minting a fresh one per call is the failure mode this prevents.",
            // THIS FIELD HAS NOW BEEN WRONG IN BOTH DIRECTIONS, which is why it
            // is computed rather than written.
            //
            // Before 2026-08-08 it said liveness could not answer about the
            // session, and that was true. #100 added the registry and this text
            // was rewritten to promise the opposite — `live` while attached,
            // `gone` once not. That promise holds only for the seat the SERVICE
            // leases, and this tool does not hand that one out: it mints a bare
            // seat nothing tracks. dev_storyflow measured the consequence the
            // same day (w-aa0607ff) and it is the original defect intact.
            //
            // A correct caveat traded for an incorrect promise is strictly worse
            // than the defect it was covering: a reader who believed the old
            // text distrusted liveness and was right, and a reader who believes
            // the new text trusts it and is wrong. So the answer is now derived
            // from what this process can actually observe.
            "liveness_of_this_seat": if reflow2_core::identity::serving_many_sessions() {
                "UNOBSERVABLE for this seat, and claim_report will say `unknown` rather than \
                 `live`. This server serves many sessions, and nothing ties a minted seat to your \
                 connection — so it cannot tell that you have gone. `unknown` is never read as \
                 free, so a claim you leave behind still makes a colleague pause; it will not \
                 clear itself. RELEASE IT YOURSELF with release_claim when you are done, because \
                 here that is the only thing that clears a claim."
            } else {
                "Tracked. This process serves exactly this session, so a claim carrying this seat \
                 reads `live` while the session is running and `gone` once it is not — including \
                 if it crashes rather than exits cleanly."
            },
        }))
    }

    #[tool(
        description = "Let a claimed region go. Returns whether a claim was there to release — \
                       releasing what nobody holds says so rather than pretending it worked.",
        annotations(read_only_hint = false)
    )]
    pub async fn release_claim(
        &self,
        Parameters(req): Parameters<ReleaseClaimReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        ok_json(serde_json::json!({
            "released": g.release_claim(&req.contributor_id, &req.seed_id).map_err(dyno_err)?
        }))
    }

    #[tool(
        description = "Who holds what, and where two people are working the same ground. Read \
                       this before starting on an area someone else may already be in. Overlaps \
                       are ranked by how much they share, and two claims by the SAME person are \
                       not reported as a collision. An overlap is a WARNING, not a refusal: the \
                       merge still resolves it correctly if two people do collide.",
        annotations(read_only_hint = true)
    )]
    pub async fn claim_report(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.claim_report().map_err(dyno_err)?)
    }
}
