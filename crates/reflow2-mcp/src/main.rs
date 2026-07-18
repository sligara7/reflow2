//! `reflow2-mcp` — the agent-native MCP stdio server (surface-plan.md SP-3).
//!
//! Exposes the reflow2 coherence-loop ops as MCP tools over stdio, backed by a
//! durable on-disk (RocksDB) design graph that survives across agent sessions.
//! grok build / claude code connect to it as an MCP server; the ambient agent is
//! the LLM (no external provider — IS-6).

use anyhow::Context;
use clap::Parser;
use reflow2_mcp::service::ReflowService;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

/// The reflow2 agent-native MCP server.
#[derive(Debug, Parser)]
#[command(name = "reflow2-mcp", version, about)]
struct Cli {
    /// Directory for the on-disk (RocksDB) design graph. Created if absent.
    #[arg(long, default_value = "./.reflow2/graph")]
    graph_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // JSON-RPC owns stdout; all logs go to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    tracing::info!(graph_path = %cli.graph_path, "opening reflow2 design graph");

    let service = ReflowService::new(&cli.graph_path)
        .with_context(|| format!("failed to open design graph at {}", cli.graph_path))?;

    tracing::info!("reflow2-mcp serving over stdio");
    let running = service
        .serve(stdio())
        .await
        .context("failed to start MCP stdio server")?;
    running.waiting().await.context("MCP server error")?;
    Ok(())
}
