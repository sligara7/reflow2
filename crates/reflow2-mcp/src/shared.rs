//! Every session on one design attaches to ONE server for it — automatically.
//!
//! `req:sessions-share-a-graph` was delivered in **v0.14.0** by `--http`: one process
//! holds the single-writer store, many sessions connect, and each gets its own
//! seat. That works. What it did not come with is a way for a session to *find*
//! that server, so the feature had a human-shaped hole in the middle of it:
//! somebody had to start a daemon, pick a port, and edit every client's config.
//!
//! The cost of that hole was measured rather than imagined. A StoryFlow fleet of
//! three bosses and a worker pool ran for **five days** believing the design
//! graph was single-holder-by-nature — building a `REFLOW2-HOLD`/`RELEASE`
//! convention, voting 3/3 on whether to give each seat its own graph, and
//! reading the design through `--export-snapshot` copies — while the binary they
//! were running already had `--http` — the flag is in that binary's own `--help`,
//! naming `req:sessions-share-a-graph`. Nobody was careless: the installed default
//! (`reflow2_init.py`) writes a **stdio spawn per session**, which is the
//! single-writer race, and a capability you have to reconfigure your way into is
//! one most users never reach.
//!
//! So this module makes sharing the DEFAULT and makes it automatic:
//!
//! 1. Look for a server already holding this graph (a rendezvous sidecar beside
//!    the store, the same convention as `.meta.json` / `.id.json`).
//! 2. If none answers, spawn a **detached** one and wait for it to publish.
//! 3. Either way, this session ends up a client of that server.
//!
//! **No session is special.** The server is not a session's child, so any
//! session — including whichever one started it — can end without taking the
//! others' design brain with it. That is the difference between this and simply
//! documenting `--http`: a "holder" seat is exactly the thing the fleet built a
//! whole social protocol around, and it should not exist.
//!
//! ## What arbitrates the race, and why it is not a check
//!
//! Several sessions can start in the same instant, find no server, and all spawn
//! one. That is fine and needs no coordination: **RocksDB's exclusive directory
//! lock is the arbiter.** Every spawned daemon tries to open the store; exactly
//! one succeeds and publishes, the rest fail that open and exit without
//! publishing. The losers' parents then find the winner's rendezvous and attach
//! to it.
//!
//! This is deliberate, and it is the one design note worth reading twice: there
//! is no "check whether a server exists, then start one if not". That shape is
//! check-then-act, and two sessions can both pass the check. The guard here is a
//! single kernel-arbitrated operation that cannot succeed twice — a spawned
//! process either takes the lock or it does not.

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a session waits for a server to come up before giving up and letting
/// the caller degrade. Generous: a cold daemon opens RocksDB and builds the
/// full-text index, which on a large design is seconds, not milliseconds.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How often to re-check for the rendezvous while waiting.
const POLL: Duration = Duration::from_millis(120);

/// How long to wait for somebody's server to publish before starting another.
/// Long enough that a merely-slow winner is not duplicated for nothing, short
/// enough that a winner which died before publishing does not cost the whole
/// timeout.
const RESPAWN_AFTER: Duration = Duration::from_secs(6);

/// A bound on re-spawning, so a graph that can never be opened produces a
/// handful of logged attempts and one clear error — not a spawn loop.
const MAX_SPAWNS: u32 = 3;

/// What a running shared server publishes so sessions can find it.
///
/// Written by the server AFTER it holds the store and its port is bound — never
/// before. A file that exists therefore means "a server got all the way up",
/// which is the only claim a reader can act on.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rendezvous {
    /// Where to reach it, e.g. `http://127.0.0.1:41653/`.
    pub url: String,
    /// The serving process, so a stale file can be told from a live one.
    pub pid: u32,
    /// The graph this server holds, recorded so a sidecar that has drifted from
    /// its store (copied directory, renamed path) is detectable rather than
    /// followed into the wrong design.
    pub graph_path: String,
    /// The reflow2 that published it — for the operator reading the file, and so
    /// a future version can refuse an incompatible peer rather than guess.
    pub version: String,
}

/// `<graph-path>.server.json`, beside the store rather than inside it.
///
/// Inside would be neater to clean up and is wrong: the store directory is
/// RocksDB's, the lock is taken on it, and a file written into a directory a
/// second process is opening is a race with the store's own file handling. The
/// sidecar convention already exists here (`identity_path`), so this follows it.
pub fn rendezvous_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    match p.file_name().map(|n| n.to_string_lossy().to_string()) {
        Some(n) => p.with_file_name(format!("{n}.server.json")),
        None => PathBuf::from(format!("{graph_path}.server.json")),
    }
}

/// Read the rendezvous, if there is one and it parses.
///
/// A malformed file reads as absent rather than as an error: the recovery is the
/// same (start a server) and a session blocked on a corrupt sidecar would be a
/// worse outcome than one that re-elects. The file is disposable state, not the
/// design.
pub fn read_rendezvous(graph_path: &str) -> Option<Rendezvous> {
    let raw = std::fs::read_to_string(rendezvous_path(graph_path)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Publish, atomically. Write a temp file and rename it into place, so a reader
/// never sees a half-written record — `rename(2)` within a directory is atomic,
/// a `write` is not.
pub fn publish_rendezvous(graph_path: &str, r: &Rendezvous) -> anyhow::Result<()> {
    let final_path = rendezvous_path(graph_path);
    let tmp = final_path.with_extension(format!("json.tmp{}", std::process::id()));
    let body = serde_json::to_string_pretty(r).context("could not render the rendezvous record")?;
    std::fs::write(&tmp, format!("{body}\n"))
        .with_context(|| format!("could not write {}", tmp.display()))?;
    std::fs::rename(&tmp, &final_path)
        .with_context(|| format!("could not publish {}", final_path.display()))?;
    Ok(())
}

/// Remove the rendezvous. Best-effort by design: a server that is killed with
/// SIGKILL cannot run this, so a stale file must be survivable anyway — which is
/// what `probe` is for. Deleting on the way out is a courtesy that makes the
/// common case clean, not the mechanism that makes it correct.
pub fn remove_rendezvous(graph_path: &str) {
    let _ = std::fs::remove_file(rendezvous_path(graph_path));
}

/// Is something answering at this URL, and is it reflow2 serving THIS graph?
///
/// A TCP connect alone is not enough and the difference is not academic: the
/// port is OS-assigned, so a stale rendezvous can name a port some unrelated
/// process has since been given. Attaching to that would produce a session whose
/// tools fail in ways that look like reflow2 bugs. So the probe completes an MCP
/// `initialize` and checks the server identifies as `reflow2-mcp`.
pub async fn probe(url: &str) -> bool {
    crate::proxy::probe_server_async(url).await.unwrap_or(false)
}

/// The same store, however the two paths are spelled?
///
/// Compared canonically because the two sides are written by different people:
/// an MCP config says `.reflow2/graph` (relative, so two worktrees do not
/// collide) while the rendezvous records whatever that resolved to. A string
/// compare would call those different and re-elect a server on every start.
/// `canonicalize` needs the path to exist, so an un-created graph falls back to
/// the literal — at which point there is no server to attach to anyway.
fn same_store(a: &str, b: &str) -> bool {
    let resolve = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    resolve(a) == resolve(b)
}

/// Is this rendezvous about the graph we are opening?
///
/// **The check the record was written for, and for a while did not perform.**
/// `Rendezvous.graph_path` documented itself as the guard against a sidecar that
/// has drifted from its store, and nothing read it — found by a reviewer who
/// grepped for readers of the field and got none, with a positive control
/// proving the grep worked (`w-74c2989e`, 2026-07-27).
///
/// The failure it prevents is the expensive kind. `cp -r` a project, restore a
/// backup, or make a worktree with `.reflow2/` in it, and `<graph>.server.json`
/// travels with the copy — naming a live server that is holding the ORIGINAL
/// design. Probing only proves *a reflow2* is there, never that it holds *your*
/// graph, so the copy's session would attach and write its design into somebody
/// else's store. Nothing errors, and a design does not look tampered with.
fn is_for_this_graph(r: &Rendezvous, graph_path: &str) -> bool {
    // An older record without the field cannot be verified, and unverifiable is
    // not the same as wrong — but it is not something to write a design through
    // either. Re-electing costs a second; attaching to the wrong store is
    // unbounded, so the tie breaks toward re-electing.
    !r.graph_path.is_empty() && same_store(&r.graph_path, graph_path)
}

/// Attach this session to a server for `graph_path`, starting one if needed.
///
/// Returns the URL to proxy to. The error case is deliberately informative
/// rather than silent — the caller turns it into the degraded surface, and a
/// session that cannot get a design brain must be able to say why
/// (`req:never-silently-absent`).
pub async fn ensure_server_async(
    graph_path: &str,
    log_to: Option<&Path>,
) -> anyhow::Result<String> {
    // Already up? Attach and we are done — the overwhelmingly common case once
    // the first session of the day has started.
    if let Some(r) = read_rendezvous(graph_path) {
        if !is_for_this_graph(&r, graph_path) {
            // Loud, because this is the copied-project case and a session that
            // silently re-elected would leave the operator with two servers and
            // no idea why.
            tracing::warn!(
                recorded = %r.graph_path,
                opening = %graph_path,
                "ignoring a rendezvous that belongs to a DIFFERENT design — it was probably copied \
                 along with the project; electing a server for this graph instead"
            );
        } else if probe(&r.url).await {
            tracing::info!(url = %r.url, pid = r.pid, "attaching to the shared reflow2 server");
            return Ok(r.url);
        }
    }

    // Either no rendezvous, or one nothing answers at. Both mean "start one",
    // and the spawned process settles the race by taking the store lock or not.
    // A stale file is left alone rather than deleted first: deleting it is a
    // write that races with a server publishing a good one.
    let spawned = spawn_daemon(graph_path, log_to)
        .context("could not start a shared reflow2 server for this design")?;

    let deadline = Instant::now() + READY_TIMEOUT;
    let mut last_seen_pid = None;
    let mut attempts = 1u32;
    let mut since_last_spawn = Duration::ZERO;
    while Instant::now() < deadline {
        if let Some(r) = read_rendezvous(graph_path)
            && is_for_this_graph(&r, graph_path)
        {
            last_seen_pid = Some(r.pid);
            if probe(&r.url).await {
                tracing::info!(url = %r.url, pid = r.pid, "shared reflow2 server is up");
                return Ok(r.url);
            }
        }
        tokio::time::sleep(POLL).await;
        since_last_spawn += POLL;

        // Waiting out the deadline when OUR spawn exits is right — losing the
        // store-lock race is the NORMAL outcome when several sessions start
        // together, and we are waiting for the WINNER's rendezvous.
        //
        // But that reasoning quietly assumes a winner exists. If the one that
        // took the lock dies before publishing (a panic during index build, an
        // OOM, a stray kill), no rendezvous ever appears, **the store lock is
        // free again**, and every session would sit out the full timeout and
        // degrade when a single re-spawn would have succeeded at once. So retry
        // on a slow cadence: cheap when a winner is merely slow (it publishes
        // and we attach), decisive when there is no winner at all.
        // (`w-74c2989e`, 2026-07-27 — the code's own comment justified the wait
        // without noticing the assumption underneath it.)
        if since_last_spawn >= RESPAWN_AFTER && attempts < MAX_SPAWNS {
            since_last_spawn = Duration::ZERO;
            attempts += 1;
            tracing::warn!(
                attempt = attempts,
                "no shared server has published yet; starting another (the previous one may have \
                 died before it could publish)"
            );
            // A failure here is not fatal: a peer may still publish, and the
            // deadline is what decides.
            if let Err(e) = spawn_daemon(graph_path, log_to) {
                tracing::warn!("re-spawn attempt {attempts} failed: {e:#}");
            }
        }
    }

    anyhow::bail!(
        "no shared reflow2 server came up for {graph_path} within {}s.\n\
         A server was started (pid {spawned}){}. Its log is at {}.\n\
         The usual cause is that something else already holds the store's write lock — an older \
         reflow2 running in stdio mode, or a stray process — in which case that holder must be \
         stopped before any session can share this design.",
        READY_TIMEOUT.as_secs(),
        match last_seen_pid {
            Some(p) => format!(", and a rendezvous named pid {p} but nothing answered there"),
            None => String::from(", and no rendezvous was published"),
        },
        daemon_log_path(graph_path, log_to).display(),
    )
}

/// Where a spawned server's stderr goes.
///
/// It must go somewhere. The daemon is detached, so its diagnostics are not
/// attached to any session's terminal, and the one question an operator will
/// have — "why did it not come up?" — is answered only by that output. A
/// detached process whose failure output goes to `/dev/null` is the silent
/// failure this project refuses everywhere else.
pub fn daemon_log_path(graph_path: &str, log_to: Option<&Path>) -> PathBuf {
    match log_to {
        Some(p) => p.to_path_buf(),
        None => {
            let p = Path::new(graph_path);
            match p.file_name().map(|n| n.to_string_lossy().to_string()) {
                Some(n) => p.with_file_name(format!("{n}.server.log")),
                None => PathBuf::from(format!("{graph_path}.server.log")),
            }
        }
    }
}

/// Start a detached server for this graph and return its pid.
///
/// Detached on purpose, in both senses: its own process group, so a Ctrl-C in
/// the session that happened to start it does not take the shared server down
/// with it; and no stdio inherited, so it cannot write a stray byte into a
/// session's JSON-RPC channel.
fn spawn_daemon(graph_path: &str, log_to: Option<&Path>) -> anyhow::Result<u32> {
    let exe = std::env::current_exe().context("could not find the running reflow2-mcp binary")?;
    let log_path = daemon_log_path(graph_path, log_to);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("could not open the server log at {}", log_path.display()))?;

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--graph-path")
        .arg(graph_path)
        .arg("--serve-shared")
        // Pass the log through so a caller that redirected it (the test suite)
        // gets the daemon's diagnostics in the same place it is watching.
        .args(match log_to {
            Some(p) => vec![String::from("--server-log"), p.display().to_string()],
            None => Vec::new(),
        })
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group. Without this the server is in the session's
        // group and a terminal signal reaches it, which would make "the shared
        // server" quietly a child of whichever session started it — the exact
        // holder relationship this module exists to remove.
        cmd.process_group(0);
    }

    let child = cmd
        .spawn()
        .context("could not spawn the shared reflow2 server")?;
    Ok(child.id())
}

/// A shared server's activity clock, so it can expire when nobody is using it.
///
/// Why a shared server should ever expire: it holds the store's exclusive write
/// lock, and every CLI use of the graph — `--import`, a `--diff` against the
/// live design, another tool's direct open — is blocked while it does. An
/// immortal daemon would trade one lock problem for another. The session proxy
/// starts a replacement on demand (`Upstream::send`), so expiry costs an
/// attached session one retry rather than its design brain.
#[derive(Debug)]
pub struct Activity {
    last: std::sync::Mutex<Instant>,
}

impl Activity {
    pub fn new() -> Self {
        Self {
            last: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// Called on every accepted connection.
    pub fn touch(&self) {
        if let Ok(mut g) = self.last.lock() {
            *g = Instant::now();
        }
    }

    pub fn idle_for(&self) -> Duration {
        self.last
            .lock()
            .map(|g| g.elapsed())
            // A poisoned clock must never be read as "idle forever" — that would
            // expire a server that is in active use. Report zero idle time, so
            // the failure keeps the server alive rather than killing it.
            .unwrap_or(Duration::ZERO)
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rv_for(graph_path: &str) -> Rendezvous {
        Rendezvous {
            url: "http://127.0.0.1:1/".into(),
            pid: 1,
            graph_path: graph_path.to_string(),
            version: "test".into(),
        }
    }

    #[test]
    fn a_rendezvous_for_another_design_is_refused() {
        // THE COPIED-PROJECT CASE. `cp -r` a project and `<graph>.server.json`
        // travels with it, naming a live server that holds the ORIGINAL design.
        // Probing only proves *a reflow2* is listening, so without this check the
        // copy's session would write its design into somebody else's store —
        // silently, because a design does not look tampered with.
        assert!(!is_for_this_graph(
            &rv_for("/home/me/original/.reflow2/graph"),
            "/home/me/copy/.reflow2/graph"
        ));
    }

    #[test]
    fn a_rendezvous_for_this_design_is_accepted() {
        assert!(is_for_this_graph(
            &rv_for("/home/me/proj/.reflow2/graph"),
            "/home/me/proj/.reflow2/graph"
        ));
    }

    #[test]
    fn the_same_store_spelled_two_ways_still_matches() {
        // The two sides are written by different people: an MCP config says
        // `.reflow2/graph` (relative, so two worktrees do not collide) and the
        // rendezvous records what that resolved to. A string compare would
        // re-elect a server on every single start.
        let dir = std::env::temp_dir().join(format!("reflow2-same-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("graph")).unwrap();
        let absolute = dir.join("graph");
        let dotted = dir.join(".").join("graph");
        assert!(same_store(
            absolute.to_str().unwrap(),
            dotted.to_str().unwrap()
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rendezvous_with_no_recorded_graph_is_not_trusted() {
        // Unverifiable is not the same as wrong — but it is not something to
        // write a design through either. Re-electing costs a second; attaching
        // to the wrong store is unbounded.
        assert!(!is_for_this_graph(
            &rv_for(""),
            "/home/me/proj/.reflow2/graph"
        ));
    }

    #[test]
    fn a_fresh_activity_clock_is_not_idle() {
        let a = Activity::new();
        assert!(a.idle_for() < Duration::from_secs(1));
        a.touch();
        assert!(a.idle_for() < Duration::from_secs(1));
    }

    #[test]
    fn rendezvous_sits_beside_the_store_not_inside_it() {
        let p = rendezvous_path("/tmp/proj/.reflow2/graph");
        assert_eq!(
            p,
            PathBuf::from("/tmp/proj/.reflow2/graph.server.json"),
            "the sidecar must be a SIBLING of the store directory: a file written inside it races \
             with RocksDB's own file handling"
        );
    }

    #[test]
    fn log_sits_beside_the_store_too() {
        assert_eq!(
            daemon_log_path("/tmp/proj/.reflow2/graph", None),
            PathBuf::from("/tmp/proj/.reflow2/graph.server.log")
        );
    }

    #[test]
    fn publish_then_read_round_trips() {
        let dir = std::env::temp_dir().join(format!("reflow2-rv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = dir.join("graph");
        let graph = graph.to_str().unwrap();

        assert!(
            read_rendezvous(graph).is_none(),
            "nothing published yet must read as absent"
        );
        let r = Rendezvous {
            url: "http://127.0.0.1:41653/".into(),
            pid: 4242,
            graph_path: graph.to_string(),
            version: "test".into(),
        };
        publish_rendezvous(graph, &r).unwrap();
        let back = read_rendezvous(graph).expect("a published rendezvous must read back");
        assert_eq!(back.url, r.url);
        assert_eq!(back.pid, 4242);

        remove_rendezvous(graph);
        assert!(read_rendezvous(graph).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_rendezvous_reads_as_absent_rather_than_erroring() {
        // Disposable state, not the design: the recovery for "unreadable" and
        // for "not there" is the same, and a session must never be blocked out
        // of its design brain by a damaged sidecar.
        let dir = std::env::temp_dir().join(format!("reflow2-rv-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = dir.join("graph");
        let graph = graph.to_str().unwrap();
        std::fs::write(rendezvous_path(graph), "{ this is not json").unwrap();
        assert!(read_rendezvous(graph).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publishing_twice_replaces_rather_than_appends() {
        // The rename is what makes this true; a plain write could leave the tail
        // of a longer previous record behind.
        let dir = std::env::temp_dir().join(format!("reflow2-rv-twice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let graph = dir.join("graph");
        let graph = graph.to_str().unwrap();
        for (pid, url) in [(1u32, "http://127.0.0.1:1/"), (2, "http://127.0.0.1:2/")] {
            publish_rendezvous(
                graph,
                &Rendezvous {
                    url: url.into(),
                    pid,
                    graph_path: graph.to_string(),
                    version: "test".into(),
                },
            )
            .unwrap();
        }
        let back = read_rendezvous(graph).unwrap();
        assert_eq!(back.pid, 2);
        assert_eq!(back.url, "http://127.0.0.1:2/");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
