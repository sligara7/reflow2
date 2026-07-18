//! reflow2-mcp — the agent-native MCP surface for Reflow 2.0 (surface-plan.md SP-3).
//!
//! Library half of the crate: [`service::ReflowService`] is the MCP tool surface
//! over a single reflow2 design graph. The `reflow2-mcp` binary (`main.rs`) is a
//! thin stdio entry point over it; integration tests drive the service directly.

pub mod dto;
pub mod service;
