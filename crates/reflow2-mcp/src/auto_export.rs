//! The server keeps the working-tree export current, so a forgotten export
//! stops being a class of loss.
//!
//! `req:the-server-keeps-the-working-tree-export-current`, from two rulings on
//! 2026-09-11. **The guarantee lives in the server because the session is the
//! thing that might not come back** — the Stop hook that used to carry it fires
//! once and exists in one harness, so a consumer of reflow2 in any other client
//! never saw it at all.
//!
//! # Opt-in, by an explicit path
//!
//! Nothing happens unless the server was started with `--export-to <FILE>`.
//! Deriving the path from export history would have given every existing
//! project the guarantee for free, and would also mean that one export to a
//! scratch path silently re-targets the server's writes. Anthony's call,
//! 2026-09-12: the flag is explicit, and `reflow2_init.py` puts it in the
//! `.mcp.json` it generates so newly installed projects get it without anyone
//! having to know this module exists.
//!
//! # Debounced, because a burst is one export
//!
//! A write wakes the task; the task then waits for a QUIET period with no
//! further writes before exporting, so a capture of thirty nodes costs one
//! export rather than thirty. A continuous stream of writes would starve that
//! forever, so `MAX_WAIT` caps how long coalescing may defer a write. A full
//! export of the largest design in existence is 265 ms (measured 2026-08-25),
//! which is what makes this affordable at all.
//!
//! # It never overwrites a hand edit — it stops and says so
//!
//! The baseline is what THIS SEAT last wrote, recorded by
//! `provenance::record_sync` at the shared file-write seam and therefore moved
//! by a session's own `export_graph` too, so a manual export never looks like
//! somebody tampering. If the file on disk does not match that — a hand edit, a
//! half-resolved merge, conflict markers that will not even parse — the
//! write-through SKIPS and the reason surfaces in `loop_status`, in `next`
//! where a session actually looks. Anthony's call, 2026-09-12: never destroy a
//! hand edit or a half-resolved merge; the cost is that the export stops being
//! current until somebody reads the warning, which is why the warning is not
//! merely a log line.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::{Notify, RwLock};

use reflow2_core::DesignGraph;

/// How long the task waits for quiet before exporting. Long enough that a
/// capture batch coalesces into one write, short enough that a session which
/// stops and walks away has its work on disk almost immediately.
const QUIET: Duration = Duration::from_secs(2);

/// The ceiling on coalescing. Without it, a session writing steadily every
/// second would defer the export forever — which is precisely the loss this
/// exists to end.
const MAX_WAIT: Duration = Duration::from_secs(10);

/// What the write-through has done, for `loop_status` to report.
///
/// Counts and reasons, deliberately no timestamps: the core takes no clock, and
/// a time here would be the only one in the reply, inviting comparison with
/// dates that come from somewhere else entirely.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    /// How many times it has written since this server started.
    pub exports: u64,
    /// `created` / `changed` / `unchanged`, from the last write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_wrote: Option<String>,
    /// Why the last attempt wrote nothing. STICKY until a write succeeds,
    /// because a skip that scrolls past is a skip nobody acts on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// The last write that failed outright, as opposed to declining to write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The server's write-through: a path, a doorbell, and what it has done.
#[derive(Debug)]
pub struct AutoExport {
    /// The file kept current — exactly what `--export-to` named.
    pub path: String,
    notify: Notify,
    status: Mutex<Status>,
    /// The content hash this task last wrote, as an IN-PROCESS baseline.
    ///
    /// Belt and braces with `provenance::last_synced`, and not redundant: the
    /// recorded sync needs a `graph_path`, which a memory-backed server does
    /// not have — so without this, such a server's hand-edit guard would never
    /// fire and the write-through would silently overwrite. The recorded one
    /// survives a restart and catches a session's own `export_graph`; this one
    /// works with no store at all. A file matching EITHER is ours.
    last_written: Mutex<Option<String>>,
}

impl AutoExport {
    pub fn new(path: String) -> Arc<Self> {
        Arc::new(Self {
            path,
            notify: Notify::new(),
            status: Mutex::new(Status::default()),
            last_written: Mutex::new(None),
        })
    }

    /// A write happened. Cheap and non-blocking: this is called on the write
    /// path of every tool, so it must never do work of its own.
    pub fn poke(&self) {
        self.notify.notify_one();
    }

    /// What it has done, for the reports.
    pub fn status(&self) -> Status {
        self.status.lock().expect("auto-export status").clone()
    }

    /// Whether it is currently declining to write, and why — the part that
    /// belongs in `next` rather than in a field beside it.
    pub fn skipped(&self) -> Option<String> {
        self.status
            .lock()
            .expect("auto-export status")
            .skipped
            .clone()
    }

    /// Is the document on disk one WE wrote? True when it matches either
    /// baseline; true when there is no baseline at all, which is a server that
    /// has never written here and is not in a position to call anything a hand
    /// edit.
    fn is_ours(&self, on_disk_hash: &str, recorded: Option<&str>) -> bool {
        let in_process = self
            .last_written
            .lock()
            .expect("auto-export baseline")
            .clone();
        match (in_process.as_deref(), recorded) {
            (None, None) => true,
            (a, b) => a == Some(on_disk_hash) || b == Some(on_disk_hash),
        }
    }

    fn remember_written(&self, hash: Option<String>) {
        *self.last_written.lock().expect("auto-export baseline") = hash;
    }

    fn record(&self, outcome: Result<&'static str, (Option<String>, Option<String>)>) {
        let mut s = self.status.lock().expect("auto-export status");
        match outcome {
            Ok(wrote) => {
                s.exports += 1;
                s.last_wrote = Some(wrote.to_string());
                s.skipped = None;
                s.error = None;
            }
            Err((skipped, error)) => {
                if skipped.is_some() {
                    s.skipped = skipped;
                }
                s.error = error;
            }
        }
    }
}

/// Start the write-through. Call it once, after the runtime exists.
///
/// Returns immediately; the work happens on its own task, so a slow export
/// never sits in front of a tool call.
pub fn spawn(auto: Arc<AutoExport>, graph: Arc<RwLock<DesignGraph>>, graph_path: Option<String>) {
    tokio::spawn(async move {
        loop {
            // Sleep until something is actually written. No polling: an idle
            // server must cost nothing.
            auto.notify.notified().await;
            let since = Instant::now();
            loop {
                let quiet = tokio::time::sleep(QUIET);
                tokio::pin!(quiet);
                tokio::select! {
                    _ = auto.notify.notified() => {
                        // More writes arrived — keep coalescing, unless we have
                        // been deferring long enough that a steady stream would
                        // starve the export.
                        if since.elapsed() >= MAX_WAIT {
                            break;
                        }
                    }
                    _ = &mut quiet => break,
                }
            }
            write_through(&auto, &graph, graph_path.as_deref()).await;
        }
    });
}

/// One write-through attempt. Declines rather than fails wherever declining is
/// the honest answer.
async fn write_through(
    auto: &AutoExport,
    graph: &Arc<RwLock<DesignGraph>>,
    graph_path: Option<&str>,
) {
    // A replaced executable stamps the record with a version no longer on disk.
    // The manual export refuses for this reason; an automatic one has even less
    // business writing a wrong stamp, since nobody asked for it.
    let (stale, _note) = crate::service::exe_replaced_since_start();
    if stale != Some(false) {
        auto.record(Err((
            Some(
                "this server's executable has been replaced since it started, so the export would \
                 be stamped by code no longer on disk. Refresh with `--stop-shared` and any tool \
                 call; nothing was written."
                    .to_string(),
            ),
            None,
        )));
        return;
    }

    // THE HAND-EDIT GUARD. The baseline is what this seat last wrote, which the
    // shared file-write seam records — so a session's own export_graph moves it
    // too and never looks like tampering. A file that does not match is left
    // alone, whatever is in it.
    let path = auto.path.clone();
    if let Ok(raw) = std::fs::read_to_string(&path) {
        match serde_json::from_str::<reflow2_core::GraphExport>(&raw) {
            Ok(on_disk) => {
                let recorded =
                    graph_path.and_then(|g| reflow2_core::provenance::last_synced(g, &path));
                if !auto.is_ours(&on_disk.effective_content_hash(), recorded.as_deref()) {
                    auto.record(Err((
                        Some(format!(
                            "{path} has changed since reflow2 last wrote it, so the \
                                 write-through left it alone. Reconcile it — or export \
                                 deliberately with export_graph, which reports what it would \
                                 drop — and the write-through resumes."
                        )),
                        None,
                    )));
                    return;
                }
            }
            Err(e) => {
                auto.record(Err((
                    Some(format!(
                        "{path} is not a readable export ({e}), so the write-through left it \
                         alone. A half-resolved merge looks exactly like this; resolve it and the \
                         write-through resumes."
                    )),
                    None,
                )));
                return;
            }
        }
    }

    let mut export = {
        let g = graph.read().await;
        match g.export_graph() {
            Ok(e) => e,
            Err(e) => {
                auto.record(Err((
                    None,
                    Some(format!("could not build the export: {e}")),
                )));
                return;
            }
        }
    };

    // NEVER `accept_divergence`. An automatic write that discards somebody's
    // work is the one outcome that cannot be undone from the thing that caused
    // it, so a lossy write is a skip here and stays a deliberate act elsewhere.
    match crate::export_write::chain_and_write(&mut export, &path, graph_path, false) {
        Ok(w) => {
            tracing::debug!(path = %path, wrote = w.wrote, "write-through exported the design");
            auto.remember_written(export.content_hash.clone());
            auto.record(Ok(w.wrote));
        }
        Err(crate::export_write::WriteRefusal::Loss(message)) => {
            auto.record(Err((Some(message), None)));
        }
        Err(crate::export_write::WriteRefusal::Io(message)) => {
            tracing::warn!(path = %path, "write-through could not write: {message}");
            auto.record(Err((None, Some(message))));
        }
    }
}
