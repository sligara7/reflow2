//! `reflow2-mcp` — the agent-native MCP stdio server (surface-plan.md SP-3).
//!
//! Exposes the reflow2 coherence-loop ops as MCP tools over stdio, backed by a
//! durable on-disk (RocksDB) design graph that survives across agent sessions.
//! grok build / claude code connect to it as an MCP server; the ambient agent is
//! the LLM (no external provider — IS-6).

use anyhow::Context;
use clap::Parser;
use reflow2_mcp::degraded::DegradedService;
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

    /// Directory for the content store — the bytes the design points at but
    /// cannot hold: transcripts, diagrams, drawings, ingested source documents
    /// (`cap:content-store`). Created on first write, not at startup.
    ///
    /// Deliberately NOT under `--graph-path`: that lives in `.reflow2/`, which
    /// is gitignored, and blobs are COMMITTED so they travel with the design
    /// (`dec:where-content-lives`). Point this at a directory inside the repo.
    ///
    /// NO DEFAULT, DELIBERATELY. It carried `default_value = "./reflow2-content"`
    /// until 2026-08-06, which made the refusal in `ReflowService::content_store`
    /// unreachable from this binary: every launch passed `Some(default)`, so a
    /// server nobody had configured would have created a blob directory at
    /// whatever CWD it happened to start in — the exact silent fallback that
    /// method's own doc comment forbids, citing `req:no-silent-fallback`.
    /// `ver:content-surface` asserted the refusal and passed anyway, because it
    /// tested `ReflowService::in_memory()` (where `content_path` is `None`), a
    /// state this binary could never reach. Blobs are COMMITTED, so where they
    /// live is a decision about the consumer's repo and must be theirs to make.
    /// See `fact:defect-content-store-invents-its-directory`.
    #[arg(long)]
    content_path: Option<String>,

    /// Serve over HTTP on this address instead of stdio, so SEVERAL sessions
    /// share one design (`req:sessions-share-a-graph`). One process holds the
    /// graph — the store is single-writer, and with one server there is still
    /// exactly one writer — while every client session gets its own seat.
    ///
    /// Bind to a loopback or tailnet address: there is no authentication yet,
    /// so anything that can reach the port can write the design.
    #[arg(long, value_name = "ADDR")]
    http: Option<String>,

    /// Share this design with every other session automatically — the mode a
    /// consumer's MCP config should use.
    ///
    /// `--http` already lets several sessions share one design, and it works;
    /// what it needs is somebody to start a server, choose a port, and put that
    /// port in every client's config. This does all three by itself: find the
    /// server holding this graph, start a detached one if there is none, and
    /// speak to it on this session's behalf.
    ///
    /// The point of the detour through stdio, rather than pointing the client
    /// straight at a URL: a client configured with a bare URL and nothing
    /// listening gets connection-refused, which an agent cannot tell apart from
    /// "reflow2 was never configured here". Keeping a process on stdio means
    /// there is always something able to say what happened
    /// (`req:never-silently-absent`).
    ///
    /// **No session owns the server.** It runs in its own process group, so the
    /// session that happened to start it can end — or be Ctrl-C'd — without
    /// taking anybody else's design brain with it.
    #[arg(long)]
    shared: bool,

    /// Serve the design surface only where a design has been opted into — the
    /// mode a MACHINE-WIDE registration should use, so reflow2 can be installed
    /// once instead of once per project.
    ///
    /// `--graph-path` is relative to the working directory and the store is
    /// created if absent, so a user-scope MCP registration would otherwise put a
    /// RocksDB store in every directory a session is ever opened in. With this
    /// flag, a directory whose graph (or the directory that would contain it)
    /// does not exist gets the LATENT surface instead: a server that starts,
    /// says no design has been started here, and offers the one tool that starts
    /// one. Nothing is created until somebody asks for it.
    #[arg(long = "only-if-present")]
    only_if_present: bool,

    /// Be the shared server: hold the graph and serve every session that
    /// attaches. Normally started for you by `--shared`, not run by hand.
    ///
    /// Binds loopback on an OS-assigned port and publishes where it landed to
    /// `<graph-path>.server.json`, so sessions find it without a port having to
    /// be agreed in advance.
    #[arg(long = "serve-shared")]
    serve_shared: bool,

    /// Where a `--serve-shared` server writes its diagnostics. Defaults to
    /// `<graph-path>.server.log`.
    #[arg(long = "server-log", value_name = "FILE")]
    server_log: Option<String>,

    /// Minutes a shared server stays up with no session talking to it, before
    /// exiting and releasing the store's write lock. 0 disables expiry.
    ///
    /// It expires at all because holding the lock blocks every CLI use of the
    /// graph; it expires *slowly* because restarting costs an attached session a
    /// retry. Sessions recover from expiry on their own — the proxy starts a
    /// replacement and replays the request.
    #[arg(long = "idle-timeout", value_name = "MINUTES", default_value_t = 120)]
    idle_timeout: u64,

    /// Stop the shared server holding this graph, if there is one, and exit.
    /// The way to release the write lock for maintenance without hunting a pid.
    #[arg(long = "stop-shared")]
    stop_shared: bool,

    /// A host name or `host:port` this server may be reached at, for sessions on
    /// OTHER machines. Repeatable.
    ///
    /// Needed because the transport only answers requests whose `Host` header
    /// is on an allowlist — `localhost`, `127.0.0.1` and `::1` by default. That
    /// is DNS-rebinding protection, and with no authentication it is the only
    /// thing standing between a web page you visit and your design, so reaching
    /// this server from another machine is a deliberate act rather than a
    /// side effect of binding a public address.
    #[arg(long = "http-allow-host", value_name = "HOST")]
    http_allow_host: Vec<String>,

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

    /// Propose a three-way merge and exit, printing the proposal as JSON. Takes
    /// three paths — the common ancestor (base), ours, and theirs — and never
    /// opens the graph, so it runs even while a server holds the lock.
    ///
    /// Matching the `merge_designs` tool: one-sided changes are taken, both-
    /// sides changes conflict and are surfaced as questions, and a node one
    /// side deleted and the other changed is retained and asked. This is a
    /// proposal — it writes nothing; applying it is a separate step. The exit
    /// code is 0 whenever the merge ran, whatever it found.
    #[arg(long, value_name = "BASE OURS THEIRS", num_args = 3)]
    merge: Vec<String>,

    /// Apply a three-way merge and exit, printing the merged design as a
    /// portable export document — the file-pure sibling of the `apply_merge`
    /// tool (which commits into the live graph). Takes the same three paths as
    /// `--merge` (base, ours, theirs) and needs `--resolutions`; never opens the
    /// graph, so it runs even while a server holds the lock, and records no
    /// rerere memory (that lives in the graph).
    ///
    /// The completion of the git-file workflow: `--merge` the same three files
    /// to see the conflicts and their ids, decide each in a resolutions file,
    /// then `--merge-apply` to produce the merged document you commit. Unlike
    /// `--merge`, this is an apply, not a report: it *refuses* — non-zero exit,
    /// writing no document — until every conflict is decided, and if a
    /// resolution names an id that is not a conflict here.
    #[arg(long = "merge-apply", value_name = "BASE OURS THEIRS", num_args = 3)]
    merge_apply: Vec<String>,

    /// The per-conflict decisions for `--merge-apply`: a path to a JSON object
    /// mapping each conflict id (as `--merge` prints them) to "base", "ours" or
    /// "theirs". Only meaningful with `--merge-apply`.
    #[arg(long, value_name = "FILE")]
    resolutions: Option<String>,

    /// Export a BEST-EFFORT snapshot of a graph another process is holding.
    ///
    /// The single-writer lock blocks reads as well as writes, so a peer session
    /// cannot `--export` the design a colleague's server holds — and `--export`
    /// is what the entire git-file merge workflow starts from. A StoryFlow fleet
    /// hit this with three bosses on one graph (2026-07-25): the two that lost
    /// the startup race could not so much as read the design, and one of them
    /// worked around it by hand with `cp -r` plus `rm LOCK`.
    ///
    /// That workaround gets discovered anyway, and the uncaveated version is the
    /// one that spreads — so reflow2 offers it with the caveat attached rather
    /// than leaving it as folklore. What you get: a copy taken at one instant,
    /// opened without disturbing the holder, exported, and thrown away.
    ///
    /// **It is best-effort and read-only, and it is NOT crash-consistent.** SSTs
    /// are immutable once written so a copy normally replays cleanly, but a
    /// MANIFEST or WAL captured mid-write can fail to open or silently lack the
    /// newest unflushed writes. Treat the result as "the design as of about now",
    /// never as a backup — the durable answer is RocksDB's secondary-instance
    /// open (`req:read-while-held`), which lives one layer down and is not
    /// exposed yet. If the graph is NOT locked this exports normally and says so,
    /// because a snapshot nobody needed would be a worse answer than the truth.
    #[arg(long = "export-snapshot")]
    export_snapshot: bool,

    /// Be git's merge driver for a committed design export. Takes git's three
    /// temporary files — %O (ancestor), %A (ours, and the file git reads the
    /// result back from), %B (theirs) — in that order.
    ///
    /// Why this exists: two people editing DIFFERENT parts of one design still
    /// collide in git, because the export is a single large JSON file and git
    /// merges it by lines. The divergences are not really textual, and reflow2
    /// already resolves them per node and per property against the common
    /// ancestor. Wiring that in as a driver makes disjoint work merge itself.
    ///
    /// Git's contract, followed exactly: exit 0 means "merged, the result is in
    /// %A"; non-zero means "conflicts remain, leave the path unmerged for the
    /// human". So a clean merge is written to %A and succeeds, and a real
    /// both-sides conflict exits non-zero WITHOUT touching %A, printing each
    /// conflict id, its question, and the `--merge-apply` command that finishes
    /// the job. Nothing is auto-decided: this driver only ever applies the
    /// resolutions the machine can derive from one-sided changes.
    ///
    /// Install once per clone (the pair git needs — .gitattributes names the
    /// driver, config defines it):
    ///
    ///   git config merge.reflow2.name 'reflow2 design export merge'
    ///   git config merge.reflow2.driver 'reflow2-mcp --merge-driver %O %A %B'
    #[arg(
        long = "merge-driver",
        value_name = "ANCESTOR OURS THEIRS",
        num_args = 3
    )]
    merge_driver: Vec<String>,
}

/// A throwaway copy of a graph directory, opened without disturbing its holder.
///
/// The lock is a filesystem lock on the directory's `LOCK` inode — it guards the
/// handle, not the bytes — so a copy with `LOCK` removed opens cleanly. That is
/// the whole trick, and its limit is stated where it is used: the copy is not
/// crash-consistent.
struct GraphSnapshot {
    dir: std::path::PathBuf,
}

impl GraphSnapshot {
    fn path(&self) -> &str {
        self.dir.to_str().unwrap_or_default()
    }

    /// Remove the copy. Called even on failure: a stale second design left on
    /// disk is something for a later session to mistake for the real one.
    ///
    /// Removes the provenance sidecar too. Opening a graph writes
    /// `<graph-path>.meta.json` BESIDE the directory, so deleting only the
    /// directory leaves a stamp behind — which is the exact sidecar trap a
    /// StoryFlow session reported on 2026-07-24, where an archived graph's
    /// leftover stamp made a brand-new graph refuse to open. It bit this code on
    /// its first run.
    fn cleanup(self) {
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            eprintln!(
                "reflow2: WARNING — could not remove the temporary snapshot at {}: {e}. Delete it \
                 by hand: a stale copy of a design is worse than no copy.",
                self.dir.display()
            );
        }
        // EVERY sidecar, enumerated rather than named one at a time. The first
        // version deleted `.meta.json` alone; adding `.id.json` for
        // req:design-identity then leaked one snapshot identity file per run —
        // and the residue test did not catch it, because it stripped the one
        // suffix it knew about. A sidecar list that has to be updated in two
        // places is a leak waiting for the next sidecar, so this globs.
        let (Some(parent), Some(prefix)) = (
            self.dir.parent(),
            self.dir.file_name().and_then(|n| n.to_str()),
        ) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(parent) else {
            return;
        };
        let sidecar_prefix = format!("{prefix}.");
        for entry in entries.flatten() {
            let name = entry.file_name();
            // `<snapshot-dir>.anything` — a sibling of the copy, named after it.
            // Never the directory itself, which is already gone.
            if name
                .to_str()
                .is_some_and(|n| n.starts_with(&sidecar_prefix))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Copy a graph directory to a temporary location and drop its lock file.
fn snapshot_dir(graph_path: &str) -> anyhow::Result<GraphSnapshot> {
    let source = std::path::Path::new(graph_path);
    if !source.is_dir() {
        anyhow::bail!("{graph_path} is not a directory, so there is nothing to snapshot");
    }
    // Distinct per process so two peers snapshotting at once cannot collide.
    let dir = std::env::temp_dir().join(format!("reflow2-snapshot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("could not create the snapshot directory {}", dir.display()))?;
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("could not read the graph directory {graph_path}"))?
    {
        let entry = entry?;
        let name = entry.file_name();
        // The lock is exactly what must not come along.
        if name == "LOCK" {
            continue;
        }
        let target = dir.join(&name);
        if entry.file_type()?.is_dir() {
            // RocksDB keeps its own files flat, so the only nested directory is
            // the full-text index — which a snapshot does not need, because
            // export reads the store rather than the index. It is rebuilt empty
            // in the copy, so do not use a snapshot for `search_design`.
            continue;
        }
        std::fs::copy(entry.path(), &target).with_context(|| {
            format!(
                "could not copy {} into the snapshot",
                entry.path().display()
            )
        })?;
    }

    // THE COPY IS THE SAME DESIGN, so it must carry the same name
    // (req:design-identity). The identity lives BESIDE the store, not in it, so
    // copying the directory alone leaves the snapshot nameless — it would then
    // mint a fresh id, look for the design under it, and find nothing. An empty
    // export, reported as a success. Caught by the degraded-server suite the
    // hour identity landed; without that test this would have started returning
    // empty designs the next time anyone reconnected.
    let source_identity = reflow2_core::identity::identity_path(graph_path);
    if source_identity.exists() {
        let target_identity = std::path::PathBuf::from(format!("{}.id.json", dir.display()));
        std::fs::copy(&source_identity, &target_identity).with_context(|| {
            format!(
                "could not copy the design identity {} into the snapshot",
                source_identity.display()
            )
        })?;
    }
    Ok(GraphSnapshot { dir })
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

/// Parse a `--merge-apply` resolutions file: a JSON object mapping each conflict
/// id (as `--merge` prints them) to `"base"`, `"ours"` or `"theirs"`. Mirrors
/// the `apply_merge` tool's `resolutions` argument, so the same decision set
/// works from the CLI or over MCP. An unrecognised choice is a mistake to
/// surface, never a silent default (`Resolution::parse`).
fn read_resolutions(
    raw: &str,
) -> anyhow::Result<std::collections::BTreeMap<String, reflow2_core::Resolution>> {
    let raw_map: std::collections::BTreeMap<String, String> = serde_json::from_str(raw).context(
        "expected a JSON object mapping each conflict id to \"base\", \"ours\" or \"theirs\"",
    )?;
    let mut out = std::collections::BTreeMap::new();
    for (id, choice) in raw_map {
        let parsed = reflow2_core::Resolution::parse(&choice).ok_or_else(|| {
            anyhow::anyhow!(
                "conflict '{id}' has resolution '{choice}', which is not one of base/ours/theirs"
            )
        })?;
        out.insert(id, parsed);
    }
    Ok(out)
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
    if !cli.merge.is_empty() && (cli.export || cli.import.is_some() || !cli.diff.is_empty()) {
        anyhow::bail!("--merge is its own mode; pass it without --export/--import/--diff");
    }
    if !cli.merge_apply.is_empty()
        && (cli.export || cli.import.is_some() || !cli.diff.is_empty() || !cli.merge.is_empty())
    {
        anyhow::bail!(
            "--merge-apply is its own mode; pass it without --export/--import/--diff/--merge"
        );
    }
    if cli.export_snapshot && (cli.export || cli.import.is_some() || !cli.diff.is_empty()) {
        anyhow::bail!(
            "--export-snapshot is its own mode; pass it without --export/--import/--diff"
        );
    }
    if !cli.merge_driver.is_empty()
        && (cli.export
            || cli.import.is_some()
            || !cli.diff.is_empty()
            || !cli.merge.is_empty()
            || !cli.merge_apply.is_empty())
    {
        anyhow::bail!(
            "--merge-driver is its own mode; pass it without --export/--import/--diff/--merge/--merge-apply"
        );
    }
    if cli.resolutions.is_some() && cli.merge_apply.is_empty() {
        anyhow::bail!("--resolutions only means something with --merge-apply");
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

    // Merge-and-exit. Three files, never the graph — so it runs while a server
    // holds the lock. It proposes; it writes nothing.
    if !cli.merge.is_empty() {
        let read_doc = |path: &str| -> anyhow::Result<reflow2_core::GraphExport> {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read the design from {path}"))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("{path} is not a reflow2 export document"))
        };
        let (base_path, ours_path, theirs_path) = (&cli.merge[0], &cli.merge[1], &cli.merge[2]);
        let base = read_doc(base_path)?;
        let ours = read_doc(ours_path)?;
        let theirs = read_doc(theirs_path)?;
        let proposal =
            reflow2_core::merge_designs(&base, &ours, &theirs, base_path, ours_path, theirs_path);
        println!("{}", serde_json::to_string_pretty(&proposal)?);
        return Ok(());
    }

    // Merge-apply-and-exit. The file-pure apply: three files plus the human's
    // decisions in, the merged document out, never the graph. resolve_merge
    // refuses (no document, non-zero exit) unless every conflict is decided.
    if !cli.merge_apply.is_empty() {
        let read_doc = |path: &str| -> anyhow::Result<reflow2_core::GraphExport> {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read the design from {path}"))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("{path} is not a reflow2 export document"))
        };
        let (base_path, ours_path, theirs_path) = (
            &cli.merge_apply[0],
            &cli.merge_apply[1],
            &cli.merge_apply[2],
        );
        let resolutions_path = cli.resolutions.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "--merge-apply needs --resolutions <FILE>: the per-conflict decisions to apply. \
                 Run --merge on the same three files first to get the conflict ids, then map each \
                 to base/ours/theirs."
            )
        })?;
        let base = read_doc(base_path)?;
        let ours = read_doc(ours_path)?;
        let theirs = read_doc(theirs_path)?;
        let raw = std::fs::read_to_string(resolutions_path)
            .with_context(|| format!("failed to read the resolutions from {resolutions_path}"))?;
        let resolutions = read_resolutions(&raw)
            .with_context(|| format!("{resolutions_path} is not a valid resolutions file"))?;
        let merged = reflow2_core::resolve_merge(&base, &ours, &theirs, &resolutions)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("could not produce the merged design")?;
        println!("{}", serde_json::to_string_pretty(&merged)?);
        return Ok(());
    }

    // Merge-driver-and-exit. Git's side of the same file-pure merge: it hands us
    // three temporary files and reads the result back out of the middle one.
    if !cli.merge_driver.is_empty() {
        let read_doc = |path: &str| -> anyhow::Result<reflow2_core::GraphExport> {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read the design from {path}"))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("{path} is not a reflow2 export document"))
        };
        let (base_path, ours_path, theirs_path) = (
            &cli.merge_driver[0],
            &cli.merge_driver[1],
            &cli.merge_driver[2],
        );
        let base = read_doc(base_path)?;
        let ours = read_doc(ours_path)?;
        let theirs = read_doc(theirs_path)?;
        let proposal =
            reflow2_core::merge_designs(&base, &ours, &theirs, base_path, ours_path, theirs_path);

        // Real conflicts stop here. Git's convention is that a non-zero exit
        // leaves the path unmerged, which is exactly right: the human decides,
        // and the message has to be actionable at a git prompt where nobody is
        // going to go hunting for the tool that produced it (rule 4).
        if !proposal.conflicts.is_empty() {
            eprintln!(
                "reflow2: {} conflict(s) in the design export need a decision — \
                 the rest merged cleanly and is NOT lost, it is recomputed when you apply.",
                proposal.conflicts.len()
            );
            for c in &proposal.conflicts {
                let property = c
                    .property
                    .as_deref()
                    .map(|p| format!(" [{p}]"))
                    .unwrap_or_default();
                eprintln!("  {} — {}{}: {}", c.id, c.target, property, c.question);
            }
            eprintln!(
                "\nDecide each id as base|ours|theirs in a JSON file, then:\n  \
                 reflow2-mcp --merge-apply {base_path} {ours_path} {theirs_path} \
                 --resolutions <FILE> > {ours_path}\n  git add <the export>"
            );
            std::process::exit(1);
        }

        // No conflicts: every divergence was one-sided, so the merge is
        // derivable with no decisions at all. resolve_merge is the same code the
        // apply path uses — the driver takes no shortcut of its own.
        let merged = reflow2_core::resolve_merge(&base, &ours, &theirs, &Default::default())
            .map_err(|e| anyhow::anyhow!("{e}"))
            .context("could not produce the merged design")?;
        let rendered = serde_json::to_string_pretty(&merged)?;
        std::fs::write(ours_path, format!("{rendered}\n"))
            .with_context(|| format!("failed to write the merged design to {ours_path}"))?;
        eprintln!(
            "reflow2: merged {} node(s) and {} edge(s) — {} divergence(s) resolved automatically, \
             no conflicts.",
            merged.nodes.len(),
            merged.edges.len(),
            proposal.auto.len()
        );
        return Ok(());
    }

    // Export-a-snapshot-and-exit: the read a peer cannot otherwise get.
    if cli.export_snapshot {
        // Probe by actually taking the handle we would use — an earlier version
        // opened the graph to test it and then opened it AGAIN to read it, which
        // deadlocked against its own first handle ("lock hold by current
        // process"). Found by this feature's own test.
        match reflow2_core::DesignGraph::open_rocksdb(&cli.graph_path) {
            // Not locked after all — the honest answer is a real export, not a
            // copy of one.
            Ok(graph) => {
                eprintln!(
                    "reflow2: the graph was not locked, so this is an ORDINARY export, not a \
                     snapshot — nothing was copied and nothing is stale."
                );
                println!("{}", serde_json::to_string_pretty(&graph.export_graph()?)?);
                return Ok(());
            }
            Err(_) => {
                let snapshot = snapshot_dir(&cli.graph_path)?;
                eprintln!(
                    "reflow2: WARNING — BEST-EFFORT SNAPSHOT. The graph at {} is held by another \
                     process, so it was COPIED and the copy opened read-only. SSTs are immutable \
                     once written, so this normally replays cleanly — but a MANIFEST or WAL caught \
                     mid-write can lack the newest unflushed writes. This is the design as of \
                     about now. It is NOT a backup and NOT crash-consistent. The durable answer is \
                     a secondary-instance open, which reflow2 cannot do yet.",
                    cli.graph_path
                );
                let result = (|| -> anyhow::Result<()> {
                    let graph = reflow2_core::DesignGraph::open_rocksdb(snapshot.path())
                        .map_err(|e| anyhow::anyhow!("{e}"))
                        .with_context(|| {
                            format!(
                                "the snapshot at {} could not be opened. That is the \
                                 crash-consistency caveat arriving: the copy caught the store \
                                 mid-write. Try again, or ask the holder to release the graph.",
                                snapshot.path()
                            )
                        })?;
                    println!("{}", serde_json::to_string_pretty(&graph.export_graph()?)?);
                    Ok(())
                })();
                // The copy is temporary by contract: leaving it behind would put
                // a second, stale design on disk for someone to mistake for the
                // real one.
                snapshot.cleanup();
                return result;
            }
        }
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
        // Importing a whole design into an EMPTY store is a restore: same
        // design, new store. It takes the document's name, or the round trip
        // would not come back byte-identical (graph_id is inside the content
        // hash). A store that already holds a design keeps its own name.
        //
        // THE RULE USED TO LIVE HERE, and that was the whole of BL-169: this
        // path adopted and `import_graph` did not, so the command and the tool
        // disagreed about what restoring a design means — a replay through the
        // tool silently renamed a design, and it was committed and pushed with
        // every gate green. It now lives in `import_graph` itself, so every
        // caller gets it and this one only has to REPORT what happened.
        let report = graph
            .import_graph(&doc)
            .context("failed to import the design")?;
        if let Some(adopted) = &report.adopted_identity {
            eprintln!(
                "reflow2: this store was empty, so it takes the imported design's name ({adopted})"
            );
        }

        // Say what landed, including what did not. An import that quietly
        // skipped half a design would be the worst kind of success.
        eprintln!(
            "reflow2: imported {} node(s) and {} edge(s) into {}",
            report.nodes_written, report.edges_written, cli.graph_path
        );
        if let Some(note) = &report.integrity_note {
            eprintln!("reflow2: WARNING — {note}");
        }
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

    // Stop-the-shared-server-and-exit. Releasing the write lock for maintenance
    // should not require hunting a pid out of `ps`.
    if cli.stop_shared {
        match reflow2_mcp::shared::read_rendezvous(&cli.graph_path) {
            None => {
                eprintln!(
                    "reflow2: no shared server is recorded for {} — nothing to stop.",
                    cli.graph_path
                );
            }
            Some(r) => {
                // SIGTERM, not SIGKILL: the server removes its rendezvous on the
                // way out, and a killed one would leave a record pointing at a
                // dead port for the next session to probe and discard.
                #[cfg(unix)]
                let stopped = std::process::Command::new("kill")
                    .arg(r.pid.to_string())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                #[cfg(not(unix))]
                let stopped = false;
                if stopped {
                    eprintln!(
                        "reflow2: asked the shared server (pid {}) at {} to stop.",
                        r.pid, r.url
                    );
                } else {
                    // It may already be gone; the stale record is the thing to
                    // clear either way, and saying which happened is the point.
                    eprintln!(
                        "reflow2: no live process at pid {} — clearing the stale record instead.",
                        r.pid
                    );
                    reflow2_mcp::shared::remove_rendezvous(&cli.graph_path);
                }
            }
        }
        return Ok(());
    }

    // Latent mode: reflow2 is installed on this machine, this directory has not
    // opted into a design, and NOTHING should be created for it.
    //
    // The check happens here — before --serve-shared and --shared, and after
    // every CLI-only mode — because both of those open or spawn something that
    // creates the store. It is deliberately a filesystem test rather than a
    // graph open: opening is the thing that would create.
    if cli.only_if_present && !reflow2_mcp::latent::design_present(&cli.graph_path) {
        eprintln!(
            "reflow2: no design has been started in this directory ({} does not exist), so the \
             design surface is not served here. This is normal on a machine-wide install; the \
             session is told so in band and offered `reflow2_start_design`.",
            cli.graph_path
        );
        let latent = reflow2_mcp::latent::LatentService::new(cli.graph_path.clone());
        let running = latent
            .serve(stdio())
            .await
            .context("failed to start the latent MCP server")?;
        running.waiting().await.context("latent MCP server error")?;
        return Ok(());
    }

    // Be-the-shared-server-and-serve. Started for a session by --shared; it is
    // an ordinary HTTP server that additionally publishes where it landed, so
    // peers can find it without a port being agreed in advance.
    if cli.serve_shared {
        let (service, provenance) = ReflowService::new_reporting(&cli.graph_path).map_err(|e| {
            // A daemon that loses the store-lock race is the NORMAL outcome when
            // several sessions start at once — exactly one wins. Say so plainly
            // in the log, because "failed to open" reads like a defect and this
            // is the mechanism working.
            let explained = explain_open_failure(&e.into(), &cli.graph_path);
            eprintln!(
                "reflow2: not becoming the shared server for {} — {explained:#}\nIf several \
                 sessions started together this is expected: the store lock picks one winner and \
                 the rest exit here. The sessions that spawned us will attach to the winner.",
                cli.graph_path
            );
            explained
        })?;
        if let Some(note) = provenance {
            eprintln!("reflow2: {note}");
        }
        let service = service.with_content_path(cli.content_path.clone());
        serve_http(
            move || Ok(service.share()),
            cli.http.as_deref().unwrap_or("127.0.0.1:0"),
            &cli.http_allow_host,
            HttpSurface::Design,
            Some(SharedServer {
                graph_path: cli.graph_path.clone(),
                idle_timeout_minutes: cli.idle_timeout,
            }),
        )
        .await?;
        return Ok(());
    }

    // Shared-session mode: attach to the server for this design (starting one if
    // there is none) and be this session's end of it.
    if cli.shared {
        let log = cli.server_log.clone().map(std::path::PathBuf::from);
        match reflow2_mcp::shared::ensure_server_async(&cli.graph_path, log.as_deref()).await {
            Ok(url) => {
                eprintln!(
                    "reflow2: sharing the design at {} through {url} — other sessions on this \
                     design are on the same server, and writes are visible to all of them \
                     immediately.",
                    cli.graph_path
                );
                return reflow2_mcp::proxy::run(&url, &cli.graph_path).await;
            }
            Err(e) => {
                // The whole point of staying on stdio: this session can still be
                // told why it has no design brain, in band, where an agent reads
                // it (`req:never-silently-absent`).
                let reason = format!("{e:#}");
                eprintln!("reflow2: {reason}");
                eprintln!(
                    "reflow2: serving a DEGRADED surface so this session can find out why — one \
                     tool, `reflow2_unavailable`, and the reason in the handshake instructions."
                );
                let degraded = DegradedService::new(reason, cli.graph_path.clone());
                let running = degraded
                    .serve(stdio())
                    .await
                    .context("failed to start the degraded MCP server")?;
                running
                    .waiting()
                    .await
                    .context("degraded MCP server error")?;
                return Ok(());
            }
        }
    }

    // The serve path is the MOST common place to hit the single-writer lock —
    // a second editor session against the same graph — so it needs the same
    // plain explanation --export/--import already get, not a raw RocksDB error
    // (BL-57).
    //
    // AND IT MUST NOT EXIT. Until 2026-07-25 a failure here ended the process
    // before the MCP handshake, so the client reported only "Connection closed"
    // and the session saw zero reflow2 tools — indistinguishable from reflow2
    // never having been configured. A three-boss StoryFlow fleet measured that
    // from both sides of the lock: the two bosses that lost the startup race ran
    // design-blind, and one of them only investigated because the user had
    // asserted the tools should be there. The diagnosis existed the whole time,
    // on stderr, where no agent reads.
    //
    // So: serve a degraded surface that carries the reason in its handshake
    // instructions and in one unmistakably-named tool. An MCP server that starts
    // and explains itself beats one that dies before it can be asked.
    match ReflowService::new_reporting(&cli.graph_path) {
        Ok((service, provenance)) => {
            let service = service.with_content_path(cli.content_path.clone());
            // Say it on stderr as well as the log: an operator running this by
            // hand sees stderr, and "which reflow2 wrote this graph" is exactly
            // the question that used to have no answer at all.
            if let Some(note) = provenance {
                tracing::warn!("{note}");
                eprintln!("reflow2: {note}");
            }

            if let Some(addr) = cli.http.clone() {
                serve_http(
                    move || Ok(service.share()),
                    &addr,
                    &cli.http_allow_host,
                    HttpSurface::Design,
                    None,
                )
                .await?;
            } else {
                tracing::info!("reflow2-mcp serving over stdio");
                let running = service
                    .serve(stdio())
                    .await
                    .context("failed to start MCP stdio server")?;
                running.waiting().await.context("MCP server error")?;
            }
        }
        Err(e) => {
            let explained = explain_open_failure(&e.into(), &cli.graph_path);
            let reason = format!("{explained:#}");
            // Still on stderr for whoever runs this by hand...
            eprintln!("reflow2: {reason}");
            eprintln!(
                "reflow2: serving a DEGRADED surface so this session can find out why — one tool, \
                 `reflow2_unavailable`, and the reason in the handshake instructions."
            );
            tracing::warn!("degraded mode: {reason}");
            // ...and in-band, where the agent will actually see it — ON THE
            // TRANSPORT THAT WAS ASKED FOR. Serving this on stdio when the
            // caller said --http put the explanation somewhere nobody was
            // listening: every session pointed at that URL got connection
            // refused, which is indistinguishable from reflow2 not being
            // configured at all, and that is the whole failure
            // `req:never-silently-absent` exists to prevent (BL-105).
            let degraded = DegradedService::new(reason, cli.graph_path.clone());
            if let Some(addr) = cli.http.clone() {
                serve_http(
                    move || Ok(degraded.clone()),
                    &addr,
                    &cli.http_allow_host,
                    HttpSurface::Degraded,
                    None,
                )
                .await?;
            } else {
                let running = degraded
                    .serve(stdio())
                    .await
                    .context("failed to start the degraded MCP server")?;
                running
                    .waiting()
                    .await
                    .context("degraded MCP server error")?;
            }
        }
    }
    Ok(())
}

/// Which surface an HTTP server is carrying, so its startup line tells the
/// truth. A degraded server is not "several sessions sharing this design" — it
/// is one tool explaining why there is no design here to share.
#[derive(Clone, Copy)]
enum HttpSurface {
    Design,
    Degraded,
}

/// The extra duties of a server that sessions are meant to FIND: publish where
/// it landed, and do not hold the store's write lock forever after everyone has
/// gone home.
struct SharedServer {
    graph_path: String,
    idle_timeout_minutes: u64,
}

/// Serve one design to many client sessions over HTTP.
///
/// `req:sessions-share-a-graph`, and the shape is the whole point: the store is
/// single-writer *per process*, so several sessions cannot each open the
/// directory — but one process holding it, with many sessions connected, still
/// has exactly one writer. rmcp builds a service per session through the
/// factory passed in; `ReflowService::share` decides what those sessions share
/// (the graph, the write generation) and what is theirs alone (their seat,
/// their read-hint memory).
///
/// Generic over the service **because the degraded surface has to come out of
/// the same door** (`req:never-silently-absent`). This took a factory of one
/// concrete type until 2026-07-26, so the failure path could only ever answer
/// on stdio: ask for `--http` against a held graph and the explanation went to
/// a transport nobody was listening on, which is the exact outage the degraded
/// surface exists to end, reintroduced on the newer transport.
///
/// **No authentication.** Bind loopback or a private tailnet: anything that can
/// reach this port can write the design. Said here and in the flag's help
/// because the failure is silent — a design does not look tampered with.
async fn serve_http<S>(
    factory: impl Fn() -> Result<S, std::io::Error> + Send + Sync + 'static,
    addr: &str,
    allow_hosts: &[String],
    surface: HttpSurface,
    shared: Option<SharedServer>,
) -> anyhow::Result<()>
where
    // rmcp v3 narrowed this from `Service<RoleServer>` to `ServerHandler`: the
    // sessionless transport builds a handler per REQUEST and has to ask it for
    // `get_info` and the tool list without a session to have cached them, which
    // the bare Service trait cannot answer. Both surfaces that come through this
    // door already implement it via `#[tool_handler]`, so this is a bound that
    // got honest, not a capability lost.
    S: rmcp::ServerHandler + Send + 'static,
{
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("could not bind {addr}"))?;
    let bound = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| addr.to_string());

    // The transport answers only requests whose Host header is allowlisted —
    // loopback by default. Extend it, never replace it, so adding a remote name
    // cannot accidentally lock out the local sessions already using this server.
    let mut config = StreamableHttpServerConfig::default();
    if !allow_hosts.is_empty() {
        let mut hosts = config.allowed_hosts.clone();
        hosts.extend(allow_hosts.iter().cloned());
        config = config.with_allowed_hosts(hosts);
    }

    // Binding a non-loopback address without naming a host is the trap this
    // warning exists for: remote sessions get an opaque 403 and nothing says
    // why. Rule 4 — say what would have worked.
    let bound_off_box = !bound.starts_with("127.") && !bound.starts_with("[::1]");
    if bound_off_box && allow_hosts.is_empty() {
        eprintln!(
            "reflow2: WARNING — bound to {bound}, which is reachable off this machine, but no \
             --http-allow-host was given. Requests from another machine will be REFUSED with 403 \
             (the Host allowlist is loopback-only by default). Pass --http-allow-host <the name or \
             address those sessions will use> to let them in."
        );
    }

    let http = StreamableHttpService::new(factory, LocalSessionManager::default().into(), config);

    // Publish AFTER the bind and the store open, never before: a rendezvous that
    // exists must mean "a server got all the way up", because that is the only
    // claim a waiting session can act on. Publishing on intent would send peers
    // at a port that may never answer.
    let activity = std::sync::Arc::new(reflow2_mcp::shared::Activity::new());
    if let Some(cfg) = &shared {
        reflow2_mcp::shared::publish_rendezvous(
            &cfg.graph_path,
            &reflow2_mcp::shared::Rendezvous {
                url: format!("http://{bound}/"),
                pid: std::process::id(),
                graph_path: cfg.graph_path.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        )?;
        eprintln!(
            "reflow2: shared server for {} is up at http://{bound}/ (pid {}). Sessions find it \
             through {}.",
            cfg.graph_path,
            std::process::id(),
            reflow2_mcp::shared::rendezvous_path(&cfg.graph_path).display()
        );

        // Clean up on the way out. Best-effort by nature — SIGKILL cannot run
        // this — which is why a stale record is designed to be survivable: a
        // session probes before trusting one.
        let graph_path = cfg.graph_path.clone();
        tokio::spawn(async move {
            if let Ok(mut term) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                term.recv().await;
                reflow2_mcp::shared::remove_rendezvous(&graph_path);
                eprintln!("reflow2: shared server stopping on SIGTERM; rendezvous removed.");
                std::process::exit(0);
            }
        });

        // Expire when nobody is using it, so the store's write lock is not held
        // against the CLI forever. Sessions recover from this on their own.
        if cfg.idle_timeout_minutes > 0 {
            let graph_path = cfg.graph_path.clone();
            let limit = std::time::Duration::from_secs(cfg.idle_timeout_minutes * 60);
            let activity = std::sync::Arc::clone(&activity);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    if activity.idle_for() >= limit {
                        reflow2_mcp::shared::remove_rendezvous(&graph_path);
                        eprintln!(
                            "reflow2: shared server for {graph_path} idle for {} minutes — \
                             exiting and releasing the store's write lock. A session that needs it \
                             again will start a replacement automatically.",
                            limit.as_secs() / 60
                        );
                        std::process::exit(0);
                    }
                }
            });
        }
    }

    match surface {
        HttpSurface::Design => eprintln!(
            "reflow2: serving over HTTP at http://{bound}/ — several sessions may share this \
             design. There is NO authentication: reach it over loopback or a private network only."
        ),
        // Say what this one is, because it looks like a working server and is
        // not: a session that connects gets the reason and one tool, and an
        // operator who reads "serving over HTTP" and walks away would be wrong.
        HttpSurface::Degraded => eprintln!(
            "reflow2: serving the DEGRADED surface over HTTP at http://{bound}/ — the design could \
             not be opened, so sessions that connect get the reason and `reflow2_unavailable`, \
             not the design. Fix the cause above and restart to serve it properly."
        ),
    }
    tracing::info!("reflow2-mcp serving over http at {bound}");

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .context("failed to accept an HTTP connection")?;
        activity.touch();
        let io = hyper_util::rt::TokioIo::new(stream);
        let svc = hyper_util::service::TowerToHyperService::new(http.clone());
        // One task per connection: a slow or stuck client must never hold up
        // the others, which is the whole reason several sessions can share this.
        tokio::spawn(async move {
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .with_upgrades()
                .await
            {
                tracing::debug!("connection from {peer} ended: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::read_resolutions;
    use reflow2_core::Resolution;

    /// The CLI must not choose where a consumer's blobs live.
    ///
    /// `ReflowService::content_store` refuses when `content_path` is `None`,
    /// citing `req:no-silent-fallback` — but until 2026-08-06 this arg carried
    /// `default_value = "./reflow2-content"`, so the binary always passed
    /// `Some(..)` and that refusal could never fire. `ver:content-surface`
    /// asserted the refusal and passed regardless, because it exercised
    /// `ReflowService::in_memory()`, a state the binary cannot reach. This test
    /// guards the CLI itself, which is where the defect actually lived:
    /// reintroducing any default here fails HERE rather than being discovered by
    /// a consumer finding blobs in whatever directory their server started in.
    ///
    /// See `fact:defect-content-store-invents-its-directory`.
    #[test]
    fn an_unconfigured_server_chooses_no_content_directory() {
        use clap::Parser;
        let cli = super::Cli::parse_from(["reflow2-mcp"]);
        assert_eq!(
            cli.content_path, None,
            "--content-path must have NO default: blobs are committed, so where \
             they live is the consumer's decision, and a default makes the \
             refusal in content_store() unreachable"
        );

        // And it is still honoured when actually given — the fix must not make
        // the flag inert, which would be the opposite defect.
        let cli = super::Cli::parse_from(["reflow2-mcp", "--content-path", "blobs"]);
        assert_eq!(cli.content_path.as_deref(), Some("blobs"));
    }

    #[test]
    fn reads_each_choice() {
        let raw = r#"{
            "merge:aaaa": "base",
            "merge:bbbb": "ours",
            "merge:cccc": "theirs"
        }"#;
        let out = read_resolutions(raw).expect("valid resolutions parse");
        assert_eq!(out.get("merge:aaaa"), Some(&Resolution::Base));
        assert_eq!(out.get("merge:bbbb"), Some(&Resolution::Ours));
        assert_eq!(out.get("merge:cccc"), Some(&Resolution::Theirs));
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn empty_object_is_no_resolutions() {
        // A merge with no conflicts needs an empty decision set, not an error.
        let out = read_resolutions("{}").expect("empty object parses");
        assert!(out.is_empty());
    }

    #[test]
    fn unknown_choice_is_surfaced_with_its_id() {
        let err = read_resolutions(r#"{"merge:dead": "mine"}"#)
            .expect_err("an unrecognised choice must be rejected, never defaulted");
        let msg = format!("{err}");
        // Names the offending conflict and the bad choice so the fix is obvious.
        assert!(msg.contains("merge:dead"), "message: {msg}");
        assert!(msg.contains("mine"), "message: {msg}");
        assert!(msg.contains("base/ours/theirs"), "message: {msg}");
    }

    #[test]
    fn non_object_json_is_rejected() {
        // A bare array or string is not a conflict-id -> choice map.
        assert!(read_resolutions("[]").is_err());
        assert!(read_resolutions(r#""ours""#).is_err());
        assert!(read_resolutions("not json at all").is_err());
    }
}
