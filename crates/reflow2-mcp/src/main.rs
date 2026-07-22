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

    /// Print the whole design to stdout as a portable document and exit,
    /// instead of serving. The same thing the `export_graph` tool returns —
    /// available here so a script can back the design up without speaking MCP.
    #[arg(long)]
    export: bool,

    /// Load a design from an exported document and exit, instead of serving.
    /// Takes a path, or `-` for stdin, so `--export` on one machine pipes
    /// straight into `--import` on another.
    ///
    /// Upsert, matching the `import_graph` tool: ids already present are
    /// overwritten and anything absent from the document is left alone. Clearing
    /// first is your decision, not a side effect of importing.
    #[arg(long, value_name = "FILE")]
    import: Option<String>,

    /// Compare two as-designed records and exit, printing the divergence
    /// report as JSON. With two paths, compares the files directly — no graph
    /// is opened, so this runs even while a server holds the lock. With one
    /// path, compares that base against the live graph at --graph-path (stop
    /// the server first).
    ///
    /// Directional, matching the `compare_designs` tool: findings are `added`
    /// / `removed` / `changed` relative to the first (base) path. Reports
    /// divergence, never judges which side is right — the exit code is 0
    /// whenever the comparison ran, whatever it found.
    #[arg(long, value_name = "BASE [OTHER]", num_args = 1..=2)]
    diff: Vec<String>,
}

/// Turn the RocksDB lock error into the sentence the operator needs.
///
/// The store is single-writer, so a running MCP server holds it exclusively —
/// and the raw error ("IO error: While lock file: … Resource temporarily
/// unavailable") does not say that, or say what to do. This is the failure a
/// script hits when it tries to restore a design into a live session.
fn explain_open_failure(err: &anyhow::Error, graph_path: &str) -> anyhow::Error {
    let text = format!("{err:#}");
    if text.contains("lock file") || text.contains("Resource temporarily unavailable") {
        return anyhow::anyhow!(
            "another process already has the design graph at {graph_path} open.\n\
             The graph is single-writer, so the MCP server holds it exclusively while it runs.\n\
             Stop that server (or close the editor session using it) and run this again."
        );
    }
    anyhow::anyhow!("failed to open design graph at {graph_path}: {text}")
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

    if cli.export && cli.import.is_some() {
        anyhow::bail!("--export and --import do the opposite things; pass one, not both");
    }
    if !cli.diff.is_empty() && (cli.export || cli.import.is_some()) {
        anyhow::bail!("--diff is its own mode; pass it without --export/--import");
    }

    // Diff-and-exit. Two files never touch the graph; one file compares
    // against the live graph, which needs the (single-writer) store.
    if !cli.diff.is_empty() {
        let read_doc = |path: &str| -> anyhow::Result<reflow2_core::GraphExport> {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read the design from {path}"))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("{path} is not a reflow2 export document"))
        };
        let base_path = &cli.diff[0];
        let base = read_doc(base_path)?;
        let diff = match cli.diff.get(1) {
            Some(other_path) => {
                let other = read_doc(other_path)?;
                reflow2_core::compare_designs(&base, &other, base_path, other_path)
            }
            None => {
                let graph = reflow2_core::DesignGraph::open_rocksdb(&cli.graph_path)
                    .map_err(|e| explain_open_failure(&e.into(), &cli.graph_path))?;
                graph
                    .compare_with_base(&base, base_path)
                    .context("failed to compare the designs")?
            }
        };
        println!("{}", serde_json::to_string_pretty(&diff)?);
        return Ok(());
    }

    // Export-and-exit runs before the server is built: a backup must be
    // possible even when the caller has no intention of serving.
    if cli.export {
        let graph = reflow2_core::DesignGraph::open_rocksdb(&cli.graph_path)
            .map_err(|e| explain_open_failure(&e.into(), &cli.graph_path))?;
        let doc = graph
            .export_graph()
            .context("failed to export the design")?;
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }

    // Import-and-exit, the sibling of --export. Without it a design could be
    // read out of a graph without speaking MCP but never written back, so a
    // committed export, a backup, or a design built on another machine could
    // only be restored by passing the whole document through the tool boundary.
    if let Some(source) = cli.import {
        let raw = if source == "-" {
            std::io::read_to_string(std::io::stdin())
                .context("failed to read the design from stdin")?
        } else {
            std::fs::read_to_string(&source)
                .with_context(|| format!("failed to read the design from {source}"))?
        };
        let doc: reflow2_core::GraphExport = serde_json::from_str(&raw).with_context(|| {
            let where_from = if source == "-" {
                "stdin"
            } else {
                source.as_str()
            };
            format!("{where_from} is not a reflow2 export document")
        })?;

        let mut graph = reflow2_core::DesignGraph::open_rocksdb(&cli.graph_path)
            .map_err(|e| explain_open_failure(&e.into(), &cli.graph_path))?;
        let report = graph
            .import_graph(&doc)
            .context("failed to import the design")?;

        // Say what landed, including what did not. An import that quietly
        // skipped half a design would be the worst kind of success.
        eprintln!(
            "reflow2: imported {} node(s) and {} edge(s) into {}",
            report.nodes_written, report.edges_written, cli.graph_path
        );
        if !report.skipped_edges.is_empty() {
            eprintln!(
                "reflow2: {} edge(s) had endpoints not in the document and not already in the \
                 graph, so they were not written:",
                report.skipped_edges.len()
            );
            for edge in &report.skipped_edges {
                eprintln!("  {edge}");
            }
        }
        return Ok(());
    }

    // The serve path is the MOST common place to hit the single-writer lock —
    // a second editor session against the same graph — so it needs the same
    // plain explanation --export/--import already get, not a raw RocksDB error
    // (BL-57).
    let (service, provenance) = ReflowService::new_reporting(&cli.graph_path)
        .map_err(|e| explain_open_failure(&e.into(), &cli.graph_path))?;

    // Say it on stderr as well as the log: an operator running this by hand
    // sees stderr, and "which reflow2 wrote this graph" is exactly the question
    // that used to have no answer at all.
    if let Some(note) = provenance {
        tracing::warn!("{note}");
        eprintln!("reflow2: {note}");
    }

    tracing::info!("reflow2-mcp serving over stdio");
    let running = service
        .serve(stdio())
        .await
        .context("failed to start MCP stdio server")?;
    running.waiting().await.context("MCP server error")?;
    Ok(())
}
