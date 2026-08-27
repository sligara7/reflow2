//! reflow2-mcp — the agent-native MCP surface for Reflow 2.0 (surface-plan.md SP-3).
//!
//! Library half of the crate: [`service::ReflowService`] is the MCP tool surface
//! over a single reflow2 design graph. The `reflow2-mcp` binary (`main.rs`) is a
//! thin stdio entry point over it; integration tests drive the service directly.

pub mod degraded;
pub mod dto;
pub mod latent;
pub mod mcp_http;
pub mod nudge;
pub mod proxy;
pub mod registry;
pub mod service;
pub mod shared;
pub mod skills;
pub mod sync_debt;
pub mod tools;
pub mod upstream;
