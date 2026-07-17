//! # reflow2-core — the deterministic, surface-agnostic core of Reflow 2.0
//!
//! Reflow 2.0 captures a design's whole lifecycle (concept → operations) in one
//! knowledge graph and keeps it coherent when anything changes. This crate is
//! the LLM-free foundation of that system: it stands up the graph **store**
//! (dynograph-foundation) configured with the reflow2 **schema** (26 node
//! types, 52 edge types across 10 domains) and exposes schema-validated CRUD
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
//! g.add_capability("cap:sync", "Local sync", "Sync data on-device").unwrap();
//! g.contains("proj:demo", reflow2_core::nodes::node::REQUIREMENT, "req:offline").unwrap();
//! g.satisfies("cap:sync", "req:offline").unwrap();
//! assert_eq!(g.count_nodes("Requirement").unwrap(), 1);
//! ```

pub mod allocate;
pub mod detect;
pub mod graph;
pub mod heal;
pub mod ingest;
pub mod llm;
pub mod nodes;
pub mod propagate;
pub mod schema;
pub mod structure;
pub mod temporal;

pub use allocate::{AllocationReport, ComponentScore, MisplacedCapability};
pub use detect::{GapCandidate, GapPrompt, GapScope, GapSource};
pub use graph::{DEFAULT_GRAPH_ID, DesignGraph};
pub use heal::{
    GeneratedContentStub, HealCategory, HealIssue, HealOp, HealOperation, HealOptions,
    HealProposal, HealReport, HealSeverity, HealStrategy, SkippedOperation,
};
pub use ingest::{DroppedEdge, IngestOptions, IngestReport, IngestStatus, PassError};
pub use llm::{
    LlmBackend, LlmError, LlmParams, LlmRequest, LlmResponse, MockLlmBackend, complete_json,
};
pub use propagate::{BlastRadius, Hop, ImpactDirection, ImpactedNode, PropagateOptions};
pub use schema::load_schema;
pub use temporal::{ChangeAction, ChangeRecord, ChangeType, EpochType, parse_snapshot_state};

// Re-export the foundation types that appear in this crate's public API, so
// callers don't need a direct dependency on dynograph-core / -storage.
pub use dynograph_core::{DynoError, Schema, Value};
pub use dynograph_storage::{StoredEdge, StoredNode};
