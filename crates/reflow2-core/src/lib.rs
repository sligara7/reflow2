//! # reflow2-core — the deterministic, surface-agnostic core of Reflow 2.0
//!
//! Reflow 2.0 captures a design's whole lifecycle (concept → operations) in one
//! knowledge graph and keeps it coherent when anything changes. This crate is
//! the LLM-free foundation of that system: it stands up the graph **store**
//! (dynograph-foundation) configured with the reflow2 **schema** (27 node
//! types, 53 edge types across 10 domains) and exposes schema-validated CRUD
//! over the design graph.
//!
//! It is deliberately neutral to the interaction surface (MCP / CLI / hosted /
//! library) and to any LLM provider — those plug in later
//! (see `docs/interaction-surfaces.md`). This crate is step 1–2 of the build
//! order: **store + schema**, then the **deterministic core**.
//!
//! ## Quick start
//!
//! ```
//! use reflow2_core::DesignGraph;
//!
//! let mut g = DesignGraph::open_in_memory().unwrap();
//! g.add_project("proj:demo", "Demo").unwrap();
//! g.add_requirement("req:offline", "Offline", "Must run offline").unwrap();
//! g.add_capability("cap:sync", "Local sync", "Sync data on-device", None).unwrap();
//! g.contains("proj:demo", reflow2_core::nodes::node::REQUIREMENT, "req:offline").unwrap();
//! g.satisfies("cap:sync", "req:offline").unwrap();
//! assert_eq!(g.count_nodes("Requirement").unwrap(), 1);
//! ```

pub mod agent;
pub mod allocate;
pub mod alternatives;
pub mod artifact;
pub mod budget;
pub mod compare;
pub mod confirm;
pub mod detect;
pub mod dimensions;
pub mod drift;
pub mod export;
pub mod fielded;
pub mod flow;
pub mod genesis;
pub mod graph;
pub mod heal;
pub mod hierarchy;
pub mod ingest;
pub mod llm;
pub mod merge;
pub mod nodes;
pub mod operate;
pub mod propagate;
pub mod provenance;
pub mod report;
pub mod schema;
pub mod search;
pub mod structure;
pub mod surprises;
pub mod temporal;
pub mod verify;
pub mod vocabulary;

pub use agent::{AgentAnswer, AgentBackend, AgentPrompt, PromptCollector, prompt_id};
pub use allocate::{
    AllocationReport, ComponentScore, MisplacedCapability, ProposedAllocation, ProposedComponent,
};
pub use alternatives::{AoaReport, BranchMeasures, analyze_alternatives};
pub use artifact::{ArtifactLink, DriftDisposition, LinkArtifactOptions};
pub use budget::{BudgetContributor, BudgetReport, BudgetVerdict};
pub use compare::{
    ChangedEdge, ChangedNode, DesignDiff, DiffAncestry, DiffBand, DiffSummary, EdgeRef,
    LIVE_GRAPH_LABEL, NodeRef, PropertyDivergence, compare_designs,
};
pub use confirm::{ClaimConfirmation, ConfirmationLedger, ConfirmationState};
pub use detect::{AskedQuestion, AskedRecord, GapCandidate, GapPrompt, GapScope, GapSource};
pub use dimensions::{Dimension, DimensionDrift, DriftDirection};
pub use drift::{DriftFinding, DriftKind, DriftReport, ObservedArtifact, ReconcileOptions};
pub use export::{ExportedEdge, ExportedNode, GraphExport, ImportReport};
pub use fielded::{
    FieldedDriftKind, FieldedFinding, FieldedOptions, FieldedReport, ObservedEnvironment,
};
pub use flow::{FlowCycle, FlowReport, FlowStep, FlowTransition};
pub use genesis::{GENESIS_EPOCH_ID, GenesisOptions, GenesisReport};
pub use graph::{DEFAULT_GRAPH_ID, DesignGraph};
pub use heal::{
    GeneratedContentStub, HealCategory, HealIssue, HealOp, HealOperation, HealOptions,
    HealProposal, HealReport, HealSeverity, HealStrategy, SkippedOperation,
};
pub use hierarchy::{HierarchyIssue, HierarchyIssueKind, Level};
pub use ingest::{DroppedEdge, IngestOptions, IngestReport, IngestStatus, PassError};
pub use llm::{
    LlmBackend, LlmError, LlmParams, LlmRequest, LlmResponse, MockLlmBackend, complete_json,
};
pub use merge::{
    AutoResolution, ConflictKind, MergeAction, MergeApplyReport, MergeConflict, MergeError,
    MergeProposal, MergeSummary, MergeUnit, Resolution, Source, merge_designs, resolve_merge,
};
pub use propagate::{BlastRadius, Hop, ImpactDirection, ImpactedNode, PropagateOptions};
pub use provenance::{GraphStamp, Provenance};
pub use report::{
    AllocationSummary, CertaintyBreakdown, GraphReport, LoopStatus, RequirementCertainty,
};
pub use schema::load_schema;
pub use search::{SearchHit, SearchResult};
pub use surprises::SurprisingConnection;
pub use temporal::{
    ChangeAction, ChangeRecord, ChangeType, EpochType, SnapshotEdge, parse_snapshot_edges,
    parse_snapshot_state,
};
pub use verify::{
    CapabilityVerification, ObservedVerification, VerificationDriftReport, VerificationFinding,
    VerifyReconcileOptions,
};
pub use vocabulary::{
    EdgeQuery, EdgeTypeMatch, EdgeTypeSpec, EndpointMatch, NodeTypeDetail, NodeTypeSpec,
    PropertySpec, Vocabulary,
};

// Re-export the foundation types that appear in this crate's public API, so
// callers don't need a direct dependency on dynograph-core / -storage.
pub use dynograph_core::{DynoError, Schema, Value};
pub use dynograph_storage::{StoredEdge, StoredNode};
