//! `capture` tools — one slice of the MCP surface.
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

#[tool_router(router = capture_router, vis = "pub")]
impl ReflowService {
    // ---- GENESIS (bootstrap the graph from a brief) ----

    #[tool(
        description = "Bootstrap the design graph: create the Project + a genesis Epoch anchor \
                       and return a next-steps checklist. Guarded and idempotent — a no-op that \
                       reports already_initialized if a Project exists (unless rescan). Call this \
                       first, then seed the brief into Requirements/Capabilities via the add_* \
                       tools and run detect_gaps.",
        annotations(read_only_hint = false)
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
        let mut g = self.write_lock().await;
        ok_json(g.genesis(opts).map_err(dyno_err)?)
    }

    // ---- Golden-thread constructors (deterministic, mutating) ----

    #[tool(
        description = "Create a Project node.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_project(
        &self,
        Parameters(req): Parameters<IdName>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_project(&req.id, &req.name).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Requirement node. A new one lands at `proposed`; only the \
                       user's word moves it off, through set_requirement_status. CALLING THIS \
                       AGAIN WITH AN EXISTING ID REVISES that node: what you pass overwrites, \
                       and every field you do NOT pass keeps its current value instead of \
                       reverting to a default — so rewording a requirement never silently \
                       un-confirms it (BL-183).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_requirement(
        &self,
        Parameters(req): Parameters<RequirementReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        with_loop_hint(
            NodeDto::from(
                g.add_requirement(&req.id, &req.name, &req.statement)
                    .map_err(dyno_err)?,
            ),
            "loop: when this capture batch lands, run detect_gaps (detect-and-ask) — \
             loop_status says what's owed",
        )
    }

    #[tool(
        description = "Create a Capability node. `status` defaults to `planned`; set it when \
                       recording something that already exists, so adopting a running system \
                       does not describe it as entirely unbuilt. CALLING THIS AGAIN WITH AN \
                       EXISTING ID REVISES that node: what you pass overwrites, and every field \
                       you do NOT pass keeps its current value instead of reverting to a default \
                       — so sharpening a description never silently unbuilds a verified \
                       capability (BL-183).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_capability(
        &self,
        Parameters(req): Parameters<CapabilityReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        with_loop_hint(
            NodeDto::from(
                g.add_capability(&req.id, &req.name, &req.description, req.status.as_deref())
                    .map_err(dyno_err)?,
            ),
            "loop: wire satisfies to the requirement this serves, then run detect_gaps when \
             the capture batch lands (detect-and-ask)",
        )
    }

    #[tool(
        description = "Set a Requirement's lifecycle status: `proposed` (the default) / \
                       `accepted` / `deferred` / `dropped` / `met`. Every move off `proposed` \
                       records the USER's word, never your own judgment: capture at `proposed` \
                       and move the status only when the user has actually confirmed, deferred \
                       or dropped it — certainty is derived from this status, so promoting it \
                       yourself forges their signature (dec:certainty-derived). A `dropped` or \
                       `met` requirement stops raising unsatisfied_requirement.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_status(
        &self,
        Parameters(req): Parameters<RequirementStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_status(&req.requirement_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Choose how much this project lets a machine change its design on its own: \
                       `flexible` (apply_heal applies structural repairs) or `rigid` (apply_heal \
                       proposes them and stops, so a human decides). That one gate is ALL the \
                       mode currently changes — said plainly because the older schema wording, \
                       \"design is the source of truth\", promised a breadth the code does not \
                       implement. ASK THE USER; do not pick for them. Until 2026-07-30 the mode \
                       could only be set at genesis, so every design ever made carried the \
                       `flexible` DEFAULT and could never move off it — a governance choice \
                       nobody made and nobody could revisit. The default records that nobody \
                       has chosen, not that flexible was chosen (req:mode-is-chosen-and-changeable).",
        annotations(read_only_hint = false)
    )]
    pub async fn set_project_mode(
        &self,
        Parameters(req): Parameters<ProjectModeReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_project_mode(&req.project_id, &req.mode)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set a Capability's lifecycle status: `planned` (the default) / \
                       `in_progress` / `realized` / `verified`. Use it as a capability moves \
                       through its life; to record one that already ships, pass `status` to \
                       add_capability instead and save a write.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_capability_status(
        &self,
        Parameters(req): Parameters<CapabilityStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_capability_status(&req.capability_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record how a node entered the graph: `authored` (the default, someone \
                       stated it) / `planned` / `inferred` (read back out of an existing system) \
                       / `healed` / `reconciled` / `imported`. Accepted on Requirement, \
                       Capability, Component and Interface. Mark inferred requirements as such — \
                       a requirement backed out of the code that implements it is satisfied by \
                       construction and cannot contradict anything, and a reader has no other way \
                       to tell. For bulk adoption prefer import_graph, which carries this at \
                       create time.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_provenance(
        &self,
        Parameters(req): Parameters<ProvenanceReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_provenance(&req.node_type, &req.node_id, &req.provenance)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Component node. Pass `level` when the part is an assembly \
                       rather than a leaf (`subsystem`, `system`, `system_of_systems`, \
                       `enterprise`; default `component`), then use contain_component to nest \
                       it — that pair is what gives hierarchy_issues something to check.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_component(
        &self,
        Parameters(req): Parameters<ComponentReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        with_loop_hint(
            NodeDto::from(
                g.add_component(&req.id, &req.name, &req.description, req.level.as_deref())
                    .map_err(dyno_err)?,
            ),
            "loop: structural change — run detect_defects (check-health) when the batch lands",
        )
    }

    #[tool(
        description = "Nest one Component inside another (parent CONTAINS child) — the assembly \
                       spine. The parent should sit exactly one level above the child: nesting \
                       two components at the same level is reported as a level_mismatch, and \
                       skipping a level as a missing_intermediate_level. Set `level` on both via \
                       add_component first, or every containment looks like a mismatch.",
        annotations(read_only_hint = false)
    )]
    pub async fn contain_component(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.contain_component(&req.from_id, &req.to_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a Capability to a Requirement it SATISFIES.",
        annotations(read_only_hint = false)
    )]
    pub async fn satisfies(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.satisfies(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Split a Requirement into a smaller one: `from_id` DECOMPOSES `to_id`. Use \
                       when a child is a 1:1 piece of its parent adding NO new information (\"the \
                       app must have a checkout system\" → enter-a-card, apply-a-discount, \
                       receive-a-receipt). Delivery rolls UP this edge: the parent is delivered \
                       when EVERY child is, so a decomposed parent needs no capability of its own. \
                       Do NOT use for a requirement that adds new technical necessity nobody asked \
                       for — that is *derived*, it belongs to the Decision that forced it \
                       (set_requirement_lineage `derived` + governed_by), and re-opening that \
                       decision may remove its reason to exist. Marks the child `decomposed`. \
                       Refuses a cycle: a tree that contains itself has no leaves and could never \
                       roll up.",
        annotations(read_only_hint = false)
    )]
    pub async fn decomposes(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.decomposes(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Set where a Requirement came from — `original` (the stakeholder's own \
                       word), `decomposed` (a 1:1 split of a parent, normally set for you by \
                       `decomposes`), or `derived` (technical necessity nobody asked for, created \
                       by a design decision — pair it with governed_by to that Decision). Distinct \
                       from `provenance`, which says how the node entered the graph rather than \
                       where the need came from. The classes behave differently: delivery rolls up \
                       a decomposition, and a derived requirement may lose its reason to exist if \
                       the decision behind it is re-opened.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_lineage(
        &self,
        Parameters(req): Parameters<RequirementLineageReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_lineage(&req.requirement_id, &req.lineage)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Allocate a Capability to a Component (ALLOCATED_TO).",
        annotations(read_only_hint = false)
    )]
    pub async fn allocate(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.allocate(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create an Interface node — a contract between parts (an API, event, \
                       data feed, CLI, library boundary, or physical/human connection point). \
                       Model one whenever two Components talk to each other, then pair it with \
                       `provides` and `consumes`: that pairing is what makes a change on one \
                       side of a boundary surface the other side.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_interface(
        &self,
        Parameters(req): Parameters<IdName>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        with_loop_hint(
            NodeDto::from(g.add_interface(&req.id, &req.name).map_err(dyno_err)?),
            "loop: structural change — wire provides/consumes, then run detect_defects \
             (check-health) when the batch lands",
        )
    }

    #[tool(
        description = "Create a Flow — an ordered process linking Capabilities end to end (a \
                       user journey, an assembly sequence, an operating loop). Attach each step \
                       with `part_of_flow` (+ step_order); join steps with TRIGGERS edges via \
                       `create_edge`, giving each a `role` property saying what the transition \
                       means ('feeds', 'forces resync') — in a process the backward edges are \
                       the point, and without a role they are indistinguishable from forward \
                       ones. Read it back with `flow_report`.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_flow(
        &self,
        Parameters(req): Parameters<AddFlowReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_flow(
                &req.id,
                &req.name,
                req.description.as_deref(),
                req.flow_type.as_deref(),
                req.entry_point.as_deref(),
                req.exit_point.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Capability is a step of a Flow (PART_OF_FLOW), with its \
                       position (`step_order`). A step without one is listed after the ordered \
                       steps, and `flow_report` says so rather than inventing an order.",
        annotations(read_only_hint = false)
    )]
    pub async fn part_of_flow(
        &self,
        Parameters(req): Parameters<PartOfFlowReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.part_of_flow(&req.capability_id, &req.flow_id, req.step_order)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Read a Flow back as facts: steps in stated order, the TRIGGERS \
                       transitions among them with their roles, and the cycles. Cycles are \
                       REPORTED, never judged — a process's loops are its design, so they do \
                       not appear in detect_defects (whose circular_dependency stays scoped to \
                       DEPENDS_ON and contracts, where a cycle really is a defect). Anything \
                       the model left unstated (an unmatched entry/exit point, steps without \
                       step_order, transitions without a role) is confessed by name.",
        annotations(read_only_hint = true)
    )]
    pub async fn flow_report(
        &self,
        Parameters(req): Parameters<FlowReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.flow_report(&req.flow_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Record that a Component PROVIDES an Interface — it is the side that \
                       implements the contract. `from_id` is the Component, `to_id` the Interface.",
        annotations(read_only_hint = false)
    )]
    pub async fn provides(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.provides(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Component CONSUMES an Interface — it is the side that \
                       depends on the contract. `from_id` is the Component, `to_id` the \
                       Interface. Once both sides are recorded, `propagate_change` on either \
                       Component reaches the other, and `detect_gaps` reports a contract that \
                       is consumed but never provided.",
        annotations(read_only_hint = false)
    )]
    pub async fn consumes(
        &self,
        Parameters(req): Parameters<EdgePairReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.consumes(&req.from_id, &req.to_id).map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a Project to a child node it CONTAINS.",
        annotations(read_only_hint = false)
    )]
    pub async fn contains(
        &self,
        Parameters(req): Parameters<ContainsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.contains(&req.project_id, &req.child_type, &req.child_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Create a Constraint — a limit the design must respect, vs a Requirement \
                       which is a goal to achieve. For a numeric budget (BL-11) set `quantity` \
                       (unit-bearing name like mass_kg / latency_ms / cost_usd), `limit`, and \
                       `direction` (maximum = stay at or under, the default). Then attach the \
                       spenders with `constrains` and read the rollup with `budget_report`. \
                       `category: kpp` marks a KEY PERFORMANCE PARAMETER — inviolable intent, a \
                       threshold that if missed fails the whole effort — and its violations are \
                       computed and ranked above ordinary gaps. On a kpp, `limit` is the \
                       threshold and `objective` is what success looks like. Never set kpp on \
                       your own reading of the wording: criticality is a claim about \
                       consequence, so ask the user first (the kpp-proposal skill).",
        annotations(read_only_hint = false)
    )]
    pub async fn add_constraint(
        &self,
        Parameters(req): Parameters<AddConstraintReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_constraint(
                &req.id,
                &req.name,
                &req.statement,
                req.category.as_deref(),
                req.quantity.as_deref(),
                req.limit,
                req.objective,
                req.direction.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Give an Interface its external ROLE, which is what makes composition \
                       computable: `published` (this design OFFERS the contract and others may \
                       rely on it), `required` (this design NEEDS one of these FROM OUTSIDE), \
                       `both` (rare, and therefore meaningful), or `internal` (plumbing its owner \
                       may change freely). An Interface is internal until someone says otherwise, \
                       because publishing is a commitment. `published` is the distinction a \
                       systems-engineering ICD publishes and that MOSA calls a modular system \
                       interface. THE ROLE IS ON THE INTERFACE, NOT THE COMPONENT: a component \
                       both publishes and subscribes, so a per-node role collapses to `both` and \
                       pairs with everything (dec:pairing-role-placement). It is READ, not just \
                       stored: propagate reports which published boundaries a change crosses so \
                       \"is this part severable\" is computed instead of asserted, and pair_designs \
                       matches `published`/`both` against `required`/`both` to compute a seam. \
                       NOT a claim the boundary has held; whether it stayed stable is its drift \
                       history.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_interface_designation(
        &self,
        Parameters(req): Parameters<InterfaceDesignationReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_interface_designation(&req.interface_id, &req.designation)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Designate a Requirement as a PROMISE THIS DESIGN PUBLISHES — a behavioural \
                       commitment a consumer may rely on — or back to INTERNAL intent nobody \
                       outside sees. Use it for the things an ICD states in prose and no \
                       structural export can carry: 'a missing store fails loud rather than \
                       falling back', 'ordering is preserved', 'an empty result means no match, \
                       not an error'. Published requirements travel with export_surface; \
                       everything else is still withheld and still counted. Internal until \
                       someone says otherwise, because publishing is a commitment — the same rule \
                       as set_interface_designation. It is NOT a claim the promise is kept; \
                       whether it held is its verification and drift history.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_requirement_designation(
        &self,
        Parameters(req): Parameters<RequirementDesignationReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_requirement_designation(&req.requirement_id, &req.designation)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record that a Constraint CONSTRAINS a target, with the target's \
                       `contribution` to the budget (in the Constraint's quantity unit) and the \
                       `basis` for the number (estimated/evidence/measured). An edge without a \
                       contribution is reported by budget_report as unstated — never treated as \
                       zero.",
        annotations(read_only_hint = false)
    )]
    pub async fn constrains(
        &self,
        Parameters(req): Parameters<ConstrainsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.constrains(
                &req.constraint_id,
                &req.target_type,
                &req.target_id,
                req.contribution,
                req.basis.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Roll a budget Constraint up (BL-11): total of stated contributions vs \
                       the limit, the worst dependency path among contributors (the \
                       path-cumulative rollup — end-to-end latency, mass down a chain), basis \
                       coverage (estimated vs measured), and an honest verdict — `incomplete` \
                       when any contribution is unstated, because a partial sum passed off as a \
                       total is how budgets lie. Contributors with no stated number are listed, \
                       never zeroed.",
        annotations(read_only_hint = true)
    )]
    pub async fn budget_report(
        &self,
        Parameters(req): Parameters<BudgetReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.budget_report(&req.constraint_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Record a Decision and why it was made (an ADR). Use this whenever the user \
                       chooses between real alternatives — the rationale is what stops the choice \
                       being silently reversed later. Link it with `governed_by`. It lands \
                       `proposed`: recording a choice is not the same as settling it, so reaching \
                       `accepted` is a separate act (`set_decision_status`, or `collapse_decision` \
                       when a fork is chosen). That is deliberate — an accepted Decision is what \
                       where-am-i reads back to the user as \"what you decided\", so asserting it \
                       on their behalf would be the forgery dec:certainty-derived forbids for \
                       requirement status. BEHAVIOUR CHANGED 2026-07-25: this used to default to \
                       `accepted`.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_decision(
        &self,
        Parameters(req): Parameters<DecisionReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_decision(&req.id, &req.name, &req.decision, req.rationale.as_deref())
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link a node to the Decision or DesignRule that shapes it (GOVERNED_BY).",
        annotations(read_only_hint = false)
    )]
    pub async fn governed_by(
        &self,
        Parameters(req): Parameters<GovernedByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.governed_by(&req.from_type, &req.from_id, &req.to_type, &req.to_id)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record a Contributor — who authors and decides the DESIGN \
                       itself: a person, an automated coding agent, or an \
                       organization. Distinct from an Actor (add via create_node), \
                       which is who the designed system SERVES. Create one per \
                       session for whoever is driving, then attribute their design \
                       nodes with authored_by — the structured 'who' behind \
                       provenance's 'how'.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_contributor(
        &self,
        Parameters(req): Parameters<ContributorReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.add_contributor(
                &req.id,
                &req.name,
                req.kind.as_deref(),
                req.handle.as_deref(),
                req.description.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Attribute a design node to a Contributor (AUTHORED_BY) — \
                       whose word this Decision/Requirement/… is. `role` is \
                       author (default), reviewer, or approver. This is the \
                       structured author behind a node; it is deliberately not a \
                       traceability edge, so it never enlarges a blast radius. \
                       Record it when a decision is MADE, not at session end — \
                       captured-when-decided is what keeps the authorship honest.",
        annotations(read_only_hint = false)
    )]
    pub async fn authored_by(
        &self,
        Parameters(req): Parameters<AuthoredByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.authored_by(
                &req.from_type,
                &req.from_id,
                &req.contributor_id,
                req.role.as_deref(),
                req.acted_at.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Record whose AREA a node is (OWNED_BY) — durable, standing, and never \
                       released. THE THIRD 'WHO' AXIS: `authored_by` says who WROTE it (past \
                       tense, never changes), `claim_region` says who is IN it RIGHT NOW \
                       (transient, advisory, released at checkout), and this says whose ground \
                       it is, which survives every session. Use it for the ordinary case of two \
                       people splitting a design — these parts are mine, those are yours. DO NOT \
                       use a claim for this: claims are session-scoped by their own description \
                       and never expire on a shared server, so standing ones would drown the \
                       report that shows who is actively working where. Deliberately NOT a \
                       traceability edge, so ownership never enlarges a blast radius — owning \
                       something says who ANSWERS for it, not that changing it changes them. \
                       `note` is what is actually owned and any bound on it. AN UNOWNED NODE IS \
                       NOT A GAP: most of a mature design legitimately has no owner, so absence \
                       is never reported. Once recorded, `loop_status` with a `contributor_id` \
                       lists the open gaps standing on that person's ground.",
        annotations(read_only_hint = false)
    )]
    pub async fn owned_by(
        &self,
        Parameters(req): Parameters<OwnedByReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(EdgeDto::from(
            g.owned_by(
                &req.from_type,
                &req.from_id,
                &req.contributor_id,
                req.note.as_deref(),
                req.since.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Fill in what a consumer of this contract must AGREE with — the paradigm \
                       (sync/async), the payload format, the field-level schema, the endpoint and \
                       permitted operations, authentication, transport security, and the error \
                       model. Structured rather than prose because prose cannot be compared: two \
                       designs can be linked and still not be checkable for disagreement unless \
                       the seam is described in comparable terms. Every field is optional and \
                       omitting one LEAVES IT ALONE, so a spec can be filled in over time by \
                       different people. Unset reads as `unspecified`, never a flattering default \
                       — silence about authentication must not read as `none`. Rate limits, \
                       timeouts and concurrency do NOT belong here: they are numeric limits with \
                       a unit and a direction, so record them as a `Constraint` and point it at \
                       this interface with `constrains`.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_interface_spec(
        &self,
        Parameters(req): Parameters<InterfaceSpecReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await;
        ok_json(NodeDto::from(
            g.set_interface_spec(
                &req.interface_id,
                req.medium.as_deref(),
                req.paradigm.as_deref(),
                req.payload_format.as_deref(),
                req.payload_schema.as_deref(),
                req.endpoint.as_deref(),
                req.operations.as_deref(),
                req.auth.as_deref(),
                req.transport_security.as_deref(),
                req.error_model.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }
}
