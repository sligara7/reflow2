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

    /// What the `content` block of a JSON reply carries, for EVERY client:
    /// `signpost` (one sentence naming `structuredContent` — the payload
    /// once), `duplicate` (the payload pretty-printed in the text block too,
    /// for a client that hands its model only `content`), or `empty` (no text
    /// block, for a client that fills it from `structuredContent` only when it
    /// is empty). UNSET, THE POLICY IS CHOSEN PER CLIENT from the name it gives
    /// at handshake: grok* → duplicate, opencode* → empty, everything else →
    /// signpost. A `--shared` client forwards this to the daemon it starts.
    #[arg(
        long = "content-policy",
        value_name = "POLICY",
        env = "REFLOW2_CONTENT_POLICY"
    )]
    content_policy: Option<String>,

    /// Keep this file current: write the design export through to it after
    /// every change, debounced, so a forgotten export stops being a class of
    /// loss (`req:the-server-keeps-the-working-tree-export-current`).
    ///
    /// OPT-IN BY AN EXPLICIT PATH. Deriving it from export history would give
    /// every project the guarantee for free and would also mean one export to
    /// a scratch path silently re-targets the server's writes; `reflow2_init.py`
    /// puts this flag in the `.mcp.json` it generates instead.
    ///
    /// It NEVER overwrites a file that has changed since reflow2 last wrote it
    /// — a hand edit, a half-resolved merge — it declines and says so in
    /// `loop_status`. Refused together with `--read-only`.
    #[arg(long = "export-to", value_name = "FILE")]
    export_to: Option<String>,

    /// Serve over HTTP on this address instead of stdio, so SEVERAL sessions
    /// share one design (`req:sessions-share-a-graph`). One process holds the
    /// graph — the store is single-writer, and with one server there is still
    /// exactly one writer — while every client session gets its own seat.
    ///
    /// Bind to a loopback or tailnet address: there is no authentication yet,
    /// so anything that can reach the port can write the design — unless
    /// `--read-only` is also given, which is what that flag is for.
    #[arg(long, value_name = "ADDR")]
    http: Option<String>,

    /// Serve EVERY design under this directory, each selected by
    /// `/g/<graph_id>/`, instead of the one at `--graph-path`.
    ///
    /// Requires `--http`: the selection rides the URL, which is what makes it
    /// visible in logs and routable by ordinary proxies
    /// (`cap:select-graph-by-id`). Designs open ON DEMAND, so starting a server
    /// over a root of fifty designs opens none of them until somebody asks.
    ///
    /// ⭐ THE ROOT IS THE TENANT BOUNDARY
    /// (`dec:the-registry-root-is-the-tenant-boundary`). This server routes
    /// WITHIN the root and has no operation that crosses one, so an operator
    /// serving several tenants gives each its own root and its own process.
    /// Listing stays unfiltered because a filtered one would need reflow2 to
    /// know who is asking — an identity system this design has twice refused.
    ///
    /// ⚠️ THE NO-AUTHENTICATION WARNING ON `--http` APPLIES WITH MORE FORCE
    /// HERE, because what is reachable is now every design under the root
    /// rather than one. `--read-only` covers all of them.
    #[arg(long, value_name = "DIR")]
    registry_root: Option<String>,

    /// Serve a design that lives ONLY IN MEMORY and is GONE when this process
    /// stops. Nothing is written to disk and nothing is recovered.
    ///
    /// ⭐ WHAT IT IS FOR: measuring reflow2 without the store, and scratch work
    /// that is meant to be thrown away. `dec:idea-is-the-byte-backend-selectable-at-runtime`
    /// was ruled this way on 2026-09-13 — the in-memory engine already existed
    /// and several hundred tests run against it, and the only thing missing was
    /// a way to ASK for it. To measure a REAL design, start this and then load
    /// one with the `import_graph` tool.
    ///
    /// 🛑 IT IS DELIBERATELY NOT CALLED `--backend memory`. That name reads as a
    /// neutral configuration choice between two equal options, and this is not
    /// one: reflow2's whole premise is that a design outlives the session, so
    /// the name has to say what happens to your work rather than which engine
    /// is underneath. A flag that quietly turns off the memory is a foot-gun
    /// pointed at the one thing this tool is for.
    ///
    /// ⚠️ REFUSED alongside `--graph-path`, `--registry-root`, `--shared` and
    /// `--serve-shared`: each of those names a design ON DISK, and combining
    /// them with this is far more likely to be a mistake than an intention.
    /// Pair it with `--export-to` if you want the scratch design written
    /// through to a file after all.
    #[arg(long)]
    ephemeral: bool,

    /// How many designs this server may hold open at once (default 8).
    ///
    /// Each open design costs a store, its file handles and its own full-text
    /// index, so this is a resource bound rather than a policy. Past it, a
    /// request for a design that is not already open is REFUSED WITH A CLEAR
    /// ERROR naming this flag — never silent thrashing
    /// (`dec:one-process-many-stores`). Nothing is evicted to make room: idle
    /// eviction is a separate change with its own policy, and a cap that
    /// silently closed somebody's design would be worse than one that declines.
    #[arg(long, value_name = "N", default_value_t = 8)]
    registry_max_open: usize,

    /// Refuse every write. Reads, searches and reports still work; nothing can
    /// be created, changed or deleted.
    ///
    /// ⭐ THIS IS WHAT MAKES A REACHABLE SURFACE SURVIVABLE BEFORE
    /// AUTHENTICATION EXISTS (`req:the-hosted-surface-is-read-only-...`).
    /// reflow2 has no answer to "who is calling" and `--http-allow-host` is the
    /// only thing between a web page the user visits and their design. Read-only
    /// splits that exposure: INTEGRITY is answered outright, since there is no
    /// write to attribute and the caller-supplied `contributor_id` is never
    /// accepted; CONFIDENTIALITY is NOT eliminated, only relocated to the
    /// network, which is what binding to a tailnet is for.
    ///
    /// Enforced at the single point a write cannot avoid — the graph's write
    /// guard — so it covers every tool that exists and every tool added later,
    /// and a session minted for a new client inherits it.
    #[arg(long)]
    read_only: bool,

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

    /// With `--import`: load a document stamped by a NEWER reflow2 than this
    /// binary anyway. Refused by default, because this binary would write its
    /// own schema defaults onto every record the newer reflow2 left implicit
    /// and nothing would say so; the import report's `materialized` lines then
    /// say exactly what was written that the document did not state.
    #[arg(long = "accept-newer", requires = "import")]
    accept_newer: bool,

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

    /// Call ONE tool and print its reply as JSON on stdout — the door a build
    /// script, a Makefile or a CI step uses to read a report from the design
    /// without speaking MCP.
    ///
    /// Why this exists: bhome's plan-sheet generator (2026-09-18) read the
    /// budget from a hand-transcribed JSON file "because reflow2 is an MCP
    /// server and not a library the script can import", so the half of the
    /// sheet the design owned was the half with no mechanical guarantee. The
    /// CLI had --export, --import, --diff and --merge-driver, and no one-shot
    /// door to a report.
    ///
    /// The tool runs through the SAME server path a session uses (an
    /// in-process client over an in-memory pipe), so a refusal is worded the
    /// same, usage is recorded the same, and nothing here paraphrases a tool.
    /// Exit 0 with the reply's JSON on stdout; exit 1 with the refusal on
    /// stderr; exit 2 when the tool marked its own reply an error.
    ///
    /// If another process holds the graph, a READ-ONLY tool still answers,
    /// from the best-effort snapshot copy `--export-snapshot` uses, and stderr
    /// says so; a tool that WRITES refuses, because a copy is not the design.
    #[arg(long, value_name = "TOOL")]
    call: Option<String>,

    /// The arguments for `--call`, as one JSON object (default `{}`). `-`
    /// reads the object from stdin, so a script can build it with a heredoc.
    #[arg(long = "args", value_name = "JSON", default_value = "{}")]
    call_args: String,
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
/// The client `--call` speaks as, so `usage_report` names the door rather than
/// an anonymous peer.
#[derive(Clone)]
struct OneShotClient;

impl rmcp::ClientHandler for OneShotClient {
    fn get_info(&self) -> rmcp::model::ClientConfig {
        let mut cfg = rmcp::model::ClientConfig::default();
        cfg.client_info.name = "reflow2-mcp --call".to_string();
        cfg.client_info.version = env!("CARGO_PKG_VERSION").to_string();
        cfg
    }
}

/// Run one served tool against the graph at `--graph-path` and print its
/// reply. Returns the process exit code: 0 for a reply, 1 for a refusal (on
/// stderr), 2 when the tool marked its own reply an error.
async fn call_one_tool(cli: &Cli, tool: &str) -> anyhow::Result<i32> {
    // Arguments first: a bad object should not touch the graph.
    let raw = if cli.call_args == "-" {
        std::io::read_to_string(std::io::stdin()).context("failed to read --args from stdin")?
    } else {
        cli.call_args.clone()
    };
    let parsed: serde_json::Value = serde_json::from_str(raw.trim())
        .with_context(|| format!("--args is not JSON: {}", raw.trim()))?;
    let Some(arguments) = parsed.as_object().cloned() else {
        anyhow::bail!(
            "--args must be one JSON object ({{\"field\": value, …}}) — the tool's parameters by \
             name — not {}",
            raw.trim()
        );
    };

    // Is the tool served, and does it only read? Decided from the served list
    // itself, so this verb can never disagree with what a session is offered.
    let probe = ReflowService::in_memory().context("could not build the tool list")?;
    let tools = probe.tools_with_lessons().await;
    let Some(served) = tools.iter().find(|t| t.name == tool) else {
        anyhow::bail!(
            "no tool named `{tool}` is served. --call takes a served tool name; `--call \
             find_tools --args '{{\"query\":\"…\"}}'` finds one from a sentence in your own words, \
             and `--call list_skills` names the skills."
        );
    };
    let read_only = served
        .annotations
        .as_ref()
        .and_then(|a| a.read_only_hint)
        .unwrap_or(false);

    let (service, snapshot) = match ReflowService::new(&cli.graph_path) {
        Ok(s) => (s, None),
        Err(e) if read_only && is_lock_contention(&format!("{e:#}")) => {
            let snapshot = snapshot_dir(&cli.graph_path)?;
            eprintln!(
                "reflow2: WARNING — BEST-EFFORT SNAPSHOT. The graph at {} is held by another \
                 process, so `{tool}` reads a COPY: the design as of about now, which can lack \
                 the newest unflushed writes. A read-only tool is answered this way rather than \
                 refused; nothing was written.",
                cli.graph_path
            );
            let s = ReflowService::new(snapshot.path())
                .map_err(|e| anyhow::anyhow!("{e}"))
                .with_context(|| {
                    format!(
                        "the snapshot at {} could not be opened: the copy caught the store \
                         mid-write. Try again, or ask the holder to release the graph.",
                        snapshot.path()
                    )
                })?;
            (s.into_read_only(), Some(snapshot))
        }
        Err(e) => {
            let why = explain_open_failure(&e.into(), &cli.graph_path);
            if read_only {
                return Err(why);
            }
            return Err(why.context(format!(
                "`{tool}` writes, so it needs the graph itself and not a snapshot copy"
            )));
        }
    };

    let outcome = call_over_pipe(service, tool, arguments).await;
    if let Some(snapshot) = snapshot {
        snapshot.cleanup();
    }
    let result = match outcome.with_context(|| format!("`{tool}` did not answer"))? {
        Ok(r) => r,
        Err(refusal) => {
            eprintln!("reflow2: `{tool}` refused — {}", refusal.message);
            return Ok(1);
        }
    };
    let body = match result.structured_content {
        Some(v) => v,
        None => {
            // A tool with no structured reply answers in text blocks; hand
            // them over as one JSON string each, so stdout is still JSON.
            let texts: Vec<serde_json::Value> = result
                .content
                .iter()
                .filter_map(|c| {
                    c.as_text()
                        .map(|t| serde_json::Value::String(t.text.clone()))
                })
                .collect();
            if texts.len() == 1 {
                texts.into_iter().next().unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Array(texts)
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(if result.is_error.unwrap_or(false) {
        2
    } else {
        0
    })
}

/// Serve `service` to an in-process client over an in-memory pipe, make one
/// call, and take the server down. The call goes through `call_tool` exactly
/// as a session's would — usage recorded, refusals hinted — because the point
/// of this door is that it opens onto the same room.
async fn call_over_pipe(
    service: ReflowService,
    tool: &str,
    arguments: rmcp::model::JsonObject,
) -> anyhow::Result<Result<rmcp::model::CallToolResult, rmcp::ErrorData>> {
    let (server_rx, client_tx) = tokio::io::duplex(1 << 22);
    let (client_rx, server_tx) = tokio::io::duplex(1 << 22);
    let server = tokio::spawn(async move {
        match service.serve((server_rx, server_tx)).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => eprintln!("reflow2: the in-process server did not start — {e}"),
        }
    });
    let client = OneShotClient
        .serve((client_rx, client_tx))
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("the in-process handshake failed")?;
    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new(tool.to_string()).with_arguments(arguments),
        )
        .await;
    let _ = client.cancel().await;
    let _ = server.await;
    match result {
        Ok(r) => Ok(Ok(r)),
        Err(rmcp::service::ServiceError::McpError(e)) => Ok(Err(e)),
        Err(e) => Err(anyhow::anyhow!("{e}")),
    }
}

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
/// Is this open failure the single-writer lock being held by somebody else?
///
/// Named rather than inlined because two callers must agree on it and they draw
/// OPPOSITE conclusions: `explain_open_failure` uses it to phrase the message,
/// and the `--serve-shared` daemon uses it to decide whether its exit means
/// "another process won, wait for them" or "nobody can fix this, stop waiting".
/// Getting those two out of step is exactly how a deliberate refusal reached a
/// user as `CONNECT_TIMEOUT`.
fn is_lock_contention(text: &str) -> bool {
    text.contains("lock file") || text.contains("Resource temporarily unavailable")
}

fn explain_open_failure(err: &anyhow::Error, graph_path: &str) -> anyhow::Error {
    let text = format!("{err:#}");
    if is_lock_contention(&text) {
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

/// The default tracing filter: quiet for everything, `info` for reflow2's own
/// crates. Named rather than inlined so the test below can hold it to that
/// shape — a bare `info` here is the state this replaced, and it is an easy
/// thing to restore by accident while debugging.
const DEFAULT_LOG_FILTER: &str = "warn,reflow2_mcp=info,reflow2_core=info";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // FIRST, before anything can replace the binary under us: remember what
    // this executable looked like at start, so currency has an answer on
    // platforms with no /proc (see shared::startup_fingerprint).
    let _ = reflow2_mcp::shared::startup_fingerprint();
    // JSON-RPC owns stdout; all logs go to stderr.
    //
    // THE DEFAULT IS `warn` FOR EVERYTHING AND `info` FOR OUR OWN CRATES, not a
    // bare `info`. Measured 2026-09-08 on this project's own server log: 368 of
    // 473 lines — 78% — were `tantivy`'s per-commit and garbage-collect chatter,
    // against 11 lines from reflow2 itself. In a container that stream is stderr,
    // and Docker's default `json-file` driver has NO SIZE CAP, so a long-running
    // server grows an unbounded log almost entirely out of a dependency's
    // internal bookkeeping. A filled disk from a container log is what prompted
    // this (a user's laptop, though not from this image).
    //
    // Stated as "quiet everything, then raise our own" rather than as
    // `tantivy=warn`, deliberately: silencing the one dependency that happens to
    // be noisy today leaves the next one to be discovered the same way. The
    // rule that holds is that an operator wants OUR narrative at info and a
    // dependency's only when something is wrong.
    //
    // RUST_LOG overrides all of it, including back to `info` for everything.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER)),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    if let Some(raw) = cli.content_policy.as_deref() {
        match reflow2_mcp::content_policy::ContentPolicy::parse(raw) {
            Some(p) => reflow2_mcp::content_policy::set_override(p),
            None => {
                eprintln!(
                    "reflow2-mcp: --content-policy must be signpost, duplicate or empty, got '{raw}'"
                );
                std::process::exit(2);
            }
        }
    }
    // ⚠️ SAY WHAT IS ACTUALLY BEING OPENED. This logged `--graph-path` for every
    // invocation, including ones that never touch it — an ephemeral design opens
    // no directory at all, and a registry server opens whichever design is asked
    // for later, not this one. A log line that names a path nothing opened is a
    // claim nothing checks, which is the drift class this project exists to catch.
    if cli.ephemeral {
        tracing::info!("opening an EPHEMERAL design — in memory only, no directory");
    } else if let Some(root) = cli.registry_root.as_deref() {
        tracing::info!(registry_root = %root, "serving designs from a registry root");
    } else {
        tracing::info!(graph_path = %cli.graph_path, "opening reflow2 design graph");
    }

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

    // ---- serve a design that will NOT survive this process -------------------
    //
    // Placed before the registry and single-graph paths for the same reason the
    // registry branch is: it replaces the premise those rest on. There is no
    // directory, so `--graph-path` means nothing here and opening one on the way
    // past would create a store nobody asked for.
    if cli.ephemeral {
        // ⚠️ REFUSE THE COMBINATIONS THAT NAME A DESIGN ON DISK. Each of these
        // is far more likely to be a mistake than an intention, and the cost of
        // guessing wrong is somebody's design quietly not being saved. Rule 4
        // throughout: say what would have worked.
        //
        // `--graph-path` is detected from the ARGUMENTS rather than the parsed
        // value, because it carries a default — a parsed value cannot tell "the
        // user asked for this directory" from "clap filled it in". The one case
        // this misses is somebody passing the default path explicitly, which is
        // harmless: they get the ephemeral design they asked for.
        if std::env::args().any(|a| a == "--graph-path" || a.starts_with("--graph-path=")) {
            anyhow::bail!(
                "--ephemeral serves a design that is GONE when this process stops, and \
                 --graph-path names one that is meant to persist — passing both is almost \
                 certainly a mistake, so nothing was opened. Drop --graph-path for a scratch \
                 design, or drop --ephemeral to work on the one at that path."
            );
        }
        if cli.registry_root.is_some() {
            anyhow::bail!(
                "--ephemeral and --registry-root disagree: a registry root is a directory of \
                 designs ON DISK, and an ephemeral design has no directory at all. Pass one."
            );
        }
        if cli.shared || cli.serve_shared {
            anyhow::bail!(
                "--ephemeral cannot be shared. --shared and --serve-shared find or start the \
                 server holding a design AT A PATH, and an ephemeral design has no path for \
                 anyone to find it by. Use --http to let several sessions reach this one."
            );
        }

        let service = ReflowService::in_memory().context("could not open an in-memory design")?;
        let service = if cli.read_only {
            service.into_read_only()
        } else {
            service
        };

        // SAY IT ON THE WAY UP, and say what happens rather than which engine
        // is underneath. The handshake says it too, because an AGENT connecting
        // here never reads stderr.
        eprintln!(
            "reflow2: ⚠️  EPHEMERAL — this design lives only in memory and is GONE when this \
             process stops. Nothing is written to disk and nothing will be recovered."
        );
        if let Some(export_to) = cli.export_to.clone() {
            eprintln!(
                "reflow2: ...except that --export-to {export_to} is set, so the design is written \
                 through to that file after every change. That file is the only durable record."
            );
        } else {
            eprintln!(
                "reflow2: to keep anything, either pass --export-to <FILE> or call export_graph \
                 with a path before you stop. To measure a REAL design, load one with import_graph."
            );
        }

        let mut service = service;
        if let Some(export_to) = cli.export_to.clone() {
            match service.start_auto_export(export_to.clone()) {
                Ok(()) => {}
                Err(why) => eprintln!("reflow2: NOT keeping {export_to} current — {why}"),
            }
        }

        if let Some(addr) = cli.http.clone() {
            serve_http(
                |cfg| http_service_of(move || Ok(service.share()), cfg),
                &addr,
                &cli.http_allow_host,
                HttpSurface::Design,
                None,
                false,
            )
            .await?;
        } else {
            tracing::info!("reflow2-mcp serving an EPHEMERAL design over stdio");
            let running = service
                .serve(stdio())
                .await
                .context("failed to start MCP stdio server")?;
            running.waiting().await.context("MCP server error")?;
        }
        return Ok(());
    }

    // ---- serve MANY designs, selected by /g/<graph_id>/ ---------------------
    //
    // Placed before every single-graph path because it replaces the premise
    // those paths rest on: there is no ONE design to open, and `--graph-path`
    // means nothing here. Taking this branch late would mean opening a design
    // nobody asked for on the way past.
    if let Some(root) = cli.registry_root.clone() {
        let Some(addr) = cli.http.clone() else {
            // RULE 4 — say what would have worked. The selection rides the URL,
            // so without a URL there is nowhere to put it, and stdio has one
            // session that could never name a second design.
            anyhow::bail!(
                "--registry-root serves SEVERAL designs and each is addressed as \
                 /g/<graph_id>/, so it needs --http to put them on. Add --http \
                 127.0.0.1:<port>, or drop --registry-root to serve the single design at \
                 --graph-path over stdio."
            );
        };

        let registry = reflow2_mcp::registry::Registry::discover(&root);
        let ids = registry.graph_ids();
        // SAY WHAT IS THERE BEFORE SERVING IT. An empty root is not an error —
        // a design created later is picked up without a restart, because the
        // listing is re-read per request — but an operator who meant to point
        // at a populated directory should find that out now rather than from a
        // 404 later.
        if ids.is_empty() {
            eprintln!(
                "reflow2: WARNING — no designs found under {root}. Serving anyway: the root is \
                 re-read on every request, so a design created there later is reachable without \
                 a restart. A design is a directory containing a reflow2 store."
            );
        } else {
            eprintln!(
                "reflow2: serving {} design(s) from {root} — {}",
                ids.len(),
                ids.join(", ")
            );
        }
        eprintln!(
            "reflow2: address a design as http://<addr>/g/<graph_id>/ . Designs open ON DEMAND \
             and at most {} are held at once (--registry-max-open). THE ROOT IS THE TENANT \
             BOUNDARY: this server routes within {root} and has no operation that crosses it. \
             There is NO authentication — reach it over loopback or a private network only.",
            cli.registry_max_open
        );

        let read_only = cli.read_only;
        let max_open = cli.registry_max_open;
        serve_http(
            move |cfg| reflow2_mcp::registry_http::GraphRouter::new(root, read_only, max_open, cfg),
            &addr,
            &cli.http_allow_host,
            HttpSurface::Design,
            None,
            true,
        )
        .await?;
        return Ok(());
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

    // One tool, one reply, and exit: the door a build uses. Before the server
    // is built for the same reason --export is.
    if let Some(tool) = cli.call.clone() {
        let code = call_one_tool(&cli, &tool).await?;
        std::process::exit(code);
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
            .import_graph_with(
                &doc,
                reflow2_core::export::ImportOptions {
                    accept_newer: cli.accept_newer,
                },
            )
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
        // What the store now holds that the document did not say. Schema
        // defaults and migrations are legitimate; an import that does not name
        // them is how 853 edges once gained a property nobody chose.
        if !report.materialized.is_empty() {
            let total: usize = report.materialized.values().sum();
            eprintln!(
                "reflow2: this import materialised {total} value(s) the document did not state — \
                 reflow2 {}'s schema defaults and migrations, by Type.property:",
                env!("CARGO_PKG_VERSION")
            );
            for (key, n) in &report.materialized {
                eprintln!("  {n}x {key}");
            }
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
        // The other shape of an unresolvable reference, and it gets the other
        // half of the same sentence: a property naming a node that is neither in
        // the document nor in the graph. The node IS written — a restore
        // reproduces what already existed rather than asserting it afresh — so
        // saying so here is the only thing standing between "reported" and
        // "silently accepted".
        if !report.dangling_node_refs.is_empty() {
            eprintln!(
                "reflow2: {} node reference(s) in this document resolve to nothing. The node(s) \
                 were written — an import restores what a design already held — but the \
                 reference cannot be walked:",
                report.dangling_node_refs.len()
            );
            for r in &report.dangling_node_refs {
                eprintln!("  {r}");
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
        // BEFORE the service is built, because building one mints a seat and the
        // flag decides how that seat's siblings are answered. From here on, a
        // seat carrying this pid that we never leased reads `unknown` rather
        // than borrowing our liveness — see identity::SERVES_MANY_SESSIONS.
        reflow2_core::identity::declare_serving_many_sessions();
        let (mut service, provenance) = ReflowService::new_reporting(&cli.graph_path)
            .map(|(svc, prov)| {
                (
                    if cli.read_only {
                        svc.into_read_only()
                    } else {
                        svc
                    },
                    prov,
                )
            })
            .map_err(|e| {
                let raw: anyhow::Error = e.into();
                let text = format!("{raw:#}");
                let explained = explain_open_failure(&raw, &cli.graph_path);
                if is_lock_contention(&text) {
                    // A daemon that loses the store-lock race is the NORMAL outcome when
                    // several sessions start at once — exactly one wins. Say so plainly
                    // in the log, because "failed to open" reads like a defect and this
                    // is the mechanism working.
                    eprintln!(
                        "reflow2: not becoming the shared server for {} — {explained:#}\nIf \
                         several sessions started together this is expected: the store lock picks \
                         one winner and the rest exit here. The sessions that spawned us will \
                         attach to the winner.",
                        cli.graph_path
                    );
                } else {
                    // 🛑 NO PEER CAN FIX THIS ONE. A version-guard refusal, a corrupt
                    // store, an unreadable path — every session that spawns us will
                    // fail the same way, so the spawning session must be told rather
                    // than left waiting out a deadline for a winner that cannot
                    // exist. Before 2026-09-09 this branch exited as silently as the
                    // lock-race one and the session's ONLY signal was 30 s of
                    // nothing, which its MCP client reported as `CONNECT_TIMEOUT` —
                    // sending a real user hunting networking while this exact
                    // sentence sat in the server log (`shared::Refusal`).
                    eprintln!(
                        "reflow2: REFUSING to become the shared server for {} — {explained:#}\n\
                         This is not a lost lock race: no other process can resolve it, so the \
                         session that spawned us is being told now rather than at the timeout.",
                        cli.graph_path
                    );
                    if let Err(e) = reflow2_mcp::shared::publish_refusal(
                        &cli.graph_path,
                        &format!("{explained:#}"),
                    ) {
                        eprintln!(
                            "reflow2: could not record that refusal for the spawning session, so \
                             it will wait out its timeout instead: {e:#}"
                        );
                    }
                }
                explained
            })?;
        // THE SERVER'S OWN GUARANTEE, started before it serves anyone. One task
        // per server, never per session — the write-through is a property of the
        // server (one file, one writer), the way the graph is.
        if let Some(export_to) = cli.export_to.clone() {
            match service.start_auto_export(export_to.clone()) {
                Ok(()) => eprintln!(
                    "reflow2: keeping {export_to} current — the design is written through after \
                     every change, debounced."
                ),
                Err(why) => eprintln!("reflow2: NOT keeping {export_to} current — {why}"),
            }
        }
        // We are about to become the server, so any refusal recorded against this
        // graph describes a world that no longer holds.
        reflow2_mcp::shared::remove_refusal(&cli.graph_path);
        if let Some(note) = provenance {
            eprintln!("reflow2: {note}");
        }
        serve_http(
            |cfg| http_service_of(move || Ok(service.share()), cfg),
            cli.http.as_deref().unwrap_or("127.0.0.1:0"),
            &cli.http_allow_host,
            HttpSurface::Design,
            Some(SharedServer {
                graph_path: cli.graph_path.clone(),
                idle_timeout_minutes: cli.idle_timeout,
            }),
            false,
        )
        .await?;
        return Ok(());
    }

    // Shared-session mode: attach to the server for this design (starting one if
    // there is none) and be this session's end of it.
    if cli.shared {
        let log = cli.server_log.clone().map(std::path::PathBuf::from);
        match reflow2_mcp::shared::ensure_server_async(
            &cli.graph_path,
            log.as_deref(),
            cli.export_to.as_deref(),
        )
        .await
        {
            Ok(url) => {
                eprintln!(
                    "reflow2: sharing the design at {} through {url} — other sessions on this \
                     design are on the same server, and writes are visible to all of them \
                     immediately.",
                    cli.graph_path
                );
                return reflow2_mcp::proxy::run(&url, &cli.graph_path, cli.export_to.as_deref())
                    .await;
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
    match ReflowService::new_reporting(&cli.graph_path).map(|(svc, prov)| {
        (
            if cli.read_only {
                svc.into_read_only()
            } else {
                svc
            },
            prov,
        )
    }) {
        Ok((mut service, provenance)) => {
            // Say it on stderr as well as the log: an operator running this by
            // hand sees stderr, and "which reflow2 wrote this graph" is exactly
            // the question that used to have no answer at all.
            if let Some(note) = provenance {
                tracing::warn!("{note}");
                eprintln!("reflow2: {note}");
            }
            // THE SERVER'S OWN GUARANTEE, started before it serves anyone. One task
            // per server, never per session — the write-through is a property of the
            // server (one file, one writer), the way the graph is.
            if let Some(export_to) = cli.export_to.clone() {
                match service.start_auto_export(export_to.clone()) {
                    Ok(()) => eprintln!(
                        "reflow2: keeping {export_to} current — the design is written through after \
                     every change, debounced."
                    ),
                    Err(why) => eprintln!("reflow2: NOT keeping {export_to} current — {why}"),
                }
            }

            if let Some(addr) = cli.http.clone() {
                serve_http(
                    |cfg| http_service_of(move || Ok(service.share()), cfg),
                    &addr,
                    &cli.http_allow_host,
                    HttpSurface::Design,
                    None,
                    false,
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
                    |cfg| http_service_of(move || Ok(degraded.clone()), cfg),
                    &addr,
                    &cli.http_allow_host,
                    HttpSurface::Degraded,
                    None,
                    false,
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
/// Build the single-design transport: one `StreamableHttpService` over one
/// handler factory.
///
/// Exists so `serve_http` can take a MAKER of tower services — which is what
/// lets the `--registry-root` path hand it a router instead — without every
/// single-graph call site having to name rmcp's transport types.
fn http_service_of<S>(
    factory: impl Fn() -> Result<S, std::io::Error> + Send + Sync + 'static,
    config: rmcp::transport::streamable_http_server::StreamableHttpServerConfig,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<
    S,
    rmcp::transport::streamable_http_server::session::local::LocalSessionManager,
>
where
    // rmcp v3 narrowed this from `Service<RoleServer>` to `ServerHandler`: the
    // sessionless transport builds a handler per REQUEST and has to ask it for
    // `get_info` and the tool list without a session to have cached them, which
    // the bare Service trait cannot answer.
    S: rmcp::ServerHandler + Send + 'static,
{
    rmcp::transport::streamable_http_server::StreamableHttpService::new(
        factory,
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default()
            .into(),
        config,
    )
}

async fn serve_http<Svc>(
    // ⭐ A MAKER OF THE TOWER SERVICE, not a factory of handlers, since
    // 2026-09-13. The single-graph path still passes a handler factory — it
    // wraps it one line below — but the registry path (`--registry-root`) serves
    // a ROUTER that owns one StreamableHttpService per design and cannot be
    // expressed as a single handler. Taking the maker here keeps the bind, the
    // Host-allowlist config, the off-box warning, the rendezvous publish and the
    // accept loop in ONE place: duplicating them for the second path is how the
    // two would drift, and the off-box warning is exactly the kind of thing that
    // gets fixed in one copy.
    //
    // The config is handed IN because the router needs it too: each design it
    // opens gets its own StreamableHttpService built with the same allowlist.
    make: impl FnOnce(rmcp::transport::streamable_http_server::StreamableHttpServerConfig) -> Svc,
    addr: &str,
    allow_hosts: &[String],
    surface: HttpSurface,
    shared: Option<SharedServer>,
    // True when this server holds MANY designs, so the banner does not claim to
    // hold one. Nothing else in this function differs.
    many_designs: bool,
) -> anyhow::Result<()>
where
    Svc: tower_service::Service<
            http::Request<hyper::body::Incoming>,
            Response = http::Response<
                http_body_util::combinators::BoxBody<bytes::Bytes, std::convert::Infallible>,
            >,
            Error = std::convert::Infallible,
        > + Clone
        + Send
        + 'static,
    Svc::Future: Send + 'static,
{
    use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;

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

    let http = make(config);

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
                // ABSOLUTE, not as typed: every reader resolves this against
                // its own cwd, so a relative path meant a different store to
                // a client outside the project (`resolved_graph_path`).
                graph_path: reflow2_mcp::shared::resolved_graph_path(&cfg.graph_path),
                version: env!("CARGO_PKG_VERSION").to_string(),
                // WHICH BUILD is about to serve, captured now so a later client
                // can tell this daemon apart from a rebuild of the same version.
                exe_fingerprint: reflow2_mcp::shared::exe_fingerprint(),
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
        // ⚠️ SAYS "THIS DESIGN" — SO THE REGISTRY SURFACE MUST NOT REACH IT.
        // A multi-design server already printed what it serves and how to
        // address it; this line would then claim it holds one design, which is
        // the kind of banner an operator reads and believes.
        HttpSurface::Design if !many_designs => eprintln!(
            "reflow2: serving over HTTP at http://{bound}/ — several sessions may share this \
             design. There is NO authentication: reach it over loopback or a private network only."
        ),
        HttpSurface::Design => eprintln!(
            "reflow2: serving over HTTP at http://{bound}/ — address a design as /g/<graph_id>/."
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
    /// A LOG THAT NOBODY CAPS CAN FILL A DISK, and the default filter sets the
    /// rate. Measured 2026-09-08 on this project's own server log: 368 of 473
    /// lines were `tantivy`'s per-commit and garbage-collect bookkeeping,
    /// against 11 from reflow2 itself — and in a container that stream goes to
    /// Docker's `json-file` driver, which has no size limit.
    ///
    /// Pinned as a SHAPE rather than as an exact string: what must hold is that
    /// the global default is quiet and reflow2's own crates are raised above
    /// it. Asserting the literal would make any future addition a test edit
    /// rather than a decision.
    #[test]
    fn the_default_log_filter_is_quiet_for_dependencies_and_loud_for_us() {
        let f = super::DEFAULT_LOG_FILTER;
        assert!(
            f.starts_with("warn"),
            "the GLOBAL default must be quiet, or a dependency's info-level \
             bookkeeping is back in the log: {f}"
        );
        assert!(
            f.contains("reflow2_mcp=info") && f.contains("reflow2_core=info"),
            "reflow2's own narrative must still reach an operator at info: {f}"
        );
        assert!(
            f.parse::<tracing_subscriber::EnvFilter>().is_ok(),
            "the default filter must be a valid EnvFilter directive set: {f}"
        );
    }

    use super::read_resolutions;
    use reflow2_core::Resolution;

    /// The CLI must not choose where a consumer's blobs live.
    ///

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
