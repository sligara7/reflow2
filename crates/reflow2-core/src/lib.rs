//! # reflow2-core — the deterministic, surface-agnostic core of Reflow 2.0
//!
//! Reflow 2.0 captures a design's whole lifecycle (concept → operations) in one
//! knowledge graph and keeps it coherent when anything changes. This crate is
//! the LLM-free foundation of that system: it stands up the graph **store**
//! (dynograph-foundation) configured with the reflow2 **schema** (29 node
//! types, 60 edge types across 11 domains) and exposes schema-validated CRUD
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
pub mod bulk;
pub mod claims;
pub mod compare;
pub mod compose;
pub mod confirm;
pub mod consumption;
pub mod corpus;
pub mod coverage;
pub mod dates;
pub mod depends;
pub mod detect;
pub mod dimensions;
pub mod discover;
pub mod drift;
pub mod export;
pub mod fielded;
pub mod flow;
pub mod genesis;
pub mod granularity;
pub mod graph;
pub mod graph_read;
pub mod heal;
pub mod hierarchy;
pub mod identity;
pub mod ility;
pub mod ingest;
pub mod llm;
pub mod maturity;
pub mod merge;
pub mod nodes;
pub mod operate;
pub mod preserve;
pub mod propagate;
pub mod provenance;
pub mod readiness;
pub mod regions;
pub mod relate;
pub mod report;
pub mod sanitize;
pub mod schema;
pub mod scope;
pub mod seam;
pub mod search;
// Absorbed from dynograph-foundation at v0.12.0 — private on purpose; see each
// module header for what was taken, what was left, and why an absorbed file
// does not widen the public surface.
// Absorbed from dynograph-foundation at v0.12.0. `foundation` is PUBLIC where
// the other three are not — its types were already in this crate's public API
// before the absorption; see its module header.
pub mod foundation;
mod fuzzy;
mod graphalg;
mod stats;
pub mod structure;
pub mod surprises;
pub mod sync;
pub mod temporal;
pub mod verify;
pub mod vocabulary;

pub use agent::{AgentAnswer, AgentBackend, AgentPrompt, PromptCollector, prompt_id};
pub use allocate::{
    AllocationReport, ComponentScore, MisplacedCapability, ProposedAllocation, ProposedComponent,
};
pub use alternatives::{
    AlternativeRef, AoaReport, BranchMeasures, CollapseReport, analyze_alternatives,
};
pub use artifact::{ArtifactLink, DriftDisposition, LinkArtifactOptions};
pub use budget::{BudgetContributor, BudgetReport, BudgetVerdict};
pub use compare::{
    ChangedEdge, ChangedNode, ChangelogBucket, ChangelogDraft, ChangelogEntry, DesignDiff,
    DiffAncestry, DiffBand, DiffSummary, EdgeRef, LIVE_GRAPH_LABEL, ManifestDelta, NodeRef,
    PropertyDivergence, UnmappedChange, changelog_rule, compare_designs,
};
pub use compose::{ComposedFinding, ComposedReport, Side};
pub use confirm::{ClaimConfirmation, ConfirmationLedger, ConfirmationState};
pub use consumption::{ConsumptionObservation, ConsumptionReport};
pub use corpus::{
    CorpusDocument, CorpusOptions, CorpusReport, CorpusStep, DocumentOutcome, DocumentStatus,
};
pub use coverage::{CoverageReport, ObservedPath, UnclaimedRegion};
pub use depends::{DependencyDeclaration, DependencyFinding, DependencyReport, ObservedDependency};
pub use detect::{
    AFFECTED_CAP, AskedQuestion, AskedRecord, DEFAULT_REPLY_BUDGET_CHARS, GapCandidate, GapPrompt,
    GapReport, GapRow, GapScope, GapSource, NARROW_THE_SCOPE, NARROW_WITH_SCOPE, ReplyBudget,
    ReplyDetail, budget_gaps,
};
pub use dimensions::{Dimension, DimensionDrift, DriftDirection};
pub use discover::{DesignAtPath, DesignPathState, describe_at};
pub use drift::{DriftFinding, DriftKind, DriftReport, ObservedArtifact, ReconcileOptions};
pub use export::{
    ExportedEdge, ExportedNode, GraphExport, ImportReport, MirrorRef, MirrorReport,
    SeveredContainment, SurfaceExport,
};
pub use fielded::{
    FieldedDriftKind, FieldedFinding, FieldedOptions, FieldedReport, ObservedEnvironment,
};
pub use flow::{FlowCycle, FlowReport, FlowStep, FlowTransition};
pub use genesis::{GENESIS_EPOCH_ID, GenesisOptions, GenesisReport};
pub use granularity::{GranularityObservation, GranularityReport};
pub use graph::{DEFAULT_GRAPH_ID, DesignGraph, node_content_hash};
pub use heal::{
    DefectSweep, GeneratedContentStub, HealCategory, HealIssue, HealOp, HealOperation, HealOptions,
    HealProposal, HealReport, HealSeverity, HealStrategy, ReviewedDefect, SkippedOperation,
    SweepScope,
};
pub use hierarchy::{HierarchyIssue, HierarchyIssueKind, Level};
pub use ility::{AssertedScore, IlityEvidence, IlityReport, IlitySignal};
pub use ingest::{
    DroppedEdge, FuzzyMerge, IngestOptions, IngestReport, IngestStatus, IngestStep, MatchKind,
    MergeCandidate, PassError,
};
pub use llm::{
    LlmBackend, LlmError, LlmParams, LlmRequest, LlmResponse, MockLlmBackend, complete_json,
};
pub use maturity::{CoveredSeam, MaturityBand, MaturityProfile, SeamCoverage};
pub use merge::{
    AutoResolution, ConflictKind, MergeAction, MergeApplyReport, MergeConflict, MergeError,
    MergeProposal, MergeSummary, MergeUnit, Resolution, Source, merge_designs, resolve_merge,
};
pub use preserve::{
    ClassifiedFinding, DivergenceClass, FUNCTION_PRESERVATION_INVARIANT, PreservationCertificate,
    PreservationCounts, PreservationVerdict, certify_preservation, classify_node_type,
};
pub use propagate::{BlastRadius, Hop, ImpactDirection, ImpactedNode, PropagateOptions};
pub use provenance::{GraphStamp, Provenance};
pub use readiness::{
    GateFinding, GateStatus, READINESS_FACT, ReadinessForecast, ReadinessGate, ReadinessKind,
    ReadinessObservation, ReadinessReport, ReadinessVerdict,
};
pub use regions::{
    DEFAULT_REGION_DEPTH, DesignRegion, DesignRegions, REGION_SEED_TYPES, RegionCoverage,
};
pub use report::{
    AllocationSummary, CertaintyBreakdown, GraphReport, LoopStatus, RankedDecision,
    RequirementCertainty, ShapingDecision, WhatNext,
};
pub use sanitize::{SanitizeReport, sanitize_text};
pub use schema::load_schema;
pub use scope::{DEFAULT_SCOPE_DEPTH, SCOPE_IS_BARELY_NARROWER_AT, Scoped};
pub use seam::{Axis, SeamFinding, SeamReport, Verdict};
pub use search::{SearchHit, SearchResult};
pub use surprises::SurprisingConnection;
pub use temporal::{
    ArrivalDelta, BaselineSource, ChangeAction, ChangeRecord, ChangeSubject, ChangeType, EpochType,
    PlanRevision, PriorStateCoverage, ScheduleOutcome, ScheduledItem, SnapshotEdge,
    parse_snapshot_edges, parse_snapshot_state,
};
pub use verify::{
    CapabilityVerification, InvalidatedFinding, InvalidationClaim, ObservedVerification,
    UnclaimedFinding, UnclaimedFindings, VerificationDriftReport, VerificationFinding,
    VerifyReconcileOptions,
};
pub use vocabulary::{
    Coverage, DomainCoverage, EdgeQuery, EdgeTypeMatch, EdgeTypeSpec, EndpointMatch,
    NodeTypeDetail, NodeTypeSpec, PropertySpec, Vocabulary, VocabularyCoverage,
    vocabulary_park_decision_id,
};

// The foundation types that appear in this crate's public API. Until
// 2026-08-24 these were re-exported FROM dynograph-core / -storage so callers
// did not need a direct dependency on them; they are now reflow2's own, and the
// re-export stays because `reflow2-mcp` names `DynoError` 35 times and
// `StoredNode` 21 times — removing it would be a breaking change dressed as
// tidiness. See `crate::foundation` for the provenance.
pub use crate::foundation::core::{DynoError, Schema, Value};
pub use crate::foundation::store::{StoredEdge, StoredNode};
