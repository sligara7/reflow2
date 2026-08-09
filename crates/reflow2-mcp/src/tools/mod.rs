//! The MCP tool surface, carved into the systems the design already names.
//!
//! BL-181. `service.rs` holds the service itself — the graph handle, the
//! constructors, the request shapes — and each module here holds one slice of
//! the tools, declaring its own `tool_router`. `ReflowService::new` sums them.
//!
//! The carving follows `dec:bl83a-functional-decomposition` ("reflow2's systems
//! are functional, not its file tree") rather than the crate layout, because a
//! file tree that disagrees with the design's own decomposition is exactly what
//! this split existed to fix.

pub mod ask;
pub mod assure;
pub mod built;
pub mod capture;
pub mod claims_tools;
pub mod coherence;
pub mod exchange;
pub mod ingest_tools;
pub mod operate_tools;
pub mod query;
pub mod temporal_tools;
