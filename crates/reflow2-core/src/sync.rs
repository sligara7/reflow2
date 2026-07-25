//! Writing over a shared design record that moved while you were working.
//!
//! `req:stale-seat-knows`, accepted 2026-07-25. Git refuses a push that does
//! not contain what is already published — *non-fast-forward* — and that
//! refusal is the single property that makes two people on one repository safe.
//! reflow2 had no equivalent, and its absence is worse here than in git,
//! because of one detail:
//!
//! **A stale export is not a conflicting export. It is a complete one.**
//!
//! A session's graph is a long-lived copy of the committed design. Pull your
//! partner's work, export from a graph that never caught up, and you write a
//! document that is internally perfect and simply older. The merge driver sees
//! no conflict — there is none to see — and their requirements are gone with no
//! marker, no warning, and nothing in the diff that looks like an error.
//!
//! ## What is checked, and why it is not "did the file change"
//!
//! The naive rule — refuse if the file moved since you last wrote — fires
//! constantly in normal work and teaches people to pass the override by habit,
//! which is worse than no check. The rule here is narrower and answers the
//! question that actually matters: **would this write DROP something the file
//! has?**
//!
//! - The file is exactly where this graph left it → write, silently. The
//!   overwhelmingly common case, and it costs one hash comparison.
//! - The file moved, but everything in it survives the write → allowed, with
//!   the movement *named* in the receipt. You are a superset; nothing is lost.
//! - The file moved and the write would remove nodes or edges it holds →
//!   **refused**, naming what would have gone and what to do instead.
//!
//! So the loud case is exactly the lossy case. Nothing else is.

use std::collections::BTreeSet;

use crate::export::GraphExport;

/// What writing `mine` over the document already at the target would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncVerdict {
    /// Nothing is there, or the file is exactly what this graph last synced
    /// with. Write it.
    Clear,
    /// The file moved since this graph last synced, but every node and edge it
    /// holds survives the write. Allowed — and said out loud, because "someone
    /// else has been here" is worth knowing even when it costs you nothing.
    MovedButNothingLost {
        /// The content hash found on disk.
        found: String,
        /// What this graph believed the file was, if it had ever synced.
        expected: Option<String>,
    },
    /// The write would remove design the file holds. This is the data loss
    /// `req:stale-seat-knows` exists to stop.
    WouldDrop {
        found: String,
        expected: Option<String>,
        /// Node ids on disk that the new document does not contain.
        dropped_nodes: Vec<String>,
        /// Edges on disk (`TYPE from -> to`) the new document does not contain.
        dropped_edges: Vec<String>,
    },
}

impl SyncVerdict {
    /// Is this a refusal?
    pub fn is_loss(&self) -> bool {
        matches!(self, SyncVerdict::WouldDrop { .. })
    }

    /// The sentence a human needs: what happened, what it costs, what to do.
    ///
    /// Rule 4 — a refusal that does not say what would have worked is a wall.
    /// The remedy is spelled out in the order it must actually be done,
    /// because the intuitive order (export, then pull) is the one that loses
    /// the work.
    pub fn message(&self, path: &str) -> Option<String> {
        match self {
            SyncVerdict::Clear => None,
            SyncVerdict::MovedButNothingLost { found, expected } => Some(format!(
                "{path} moved since this graph last synced with it ({}, now {}), but nothing in \
                 it would be lost — your design contains everything the file holds. Written.",
                expected.as_deref().unwrap_or("never synced"),
                short(found)
            )),
            SyncVerdict::WouldDrop {
                found,
                expected,
                dropped_nodes,
                dropped_edges,
            } => Some(format!(
                "REFUSED: writing this design over {path} would DELETE {} node(s) and {} edge(s) \
                 that the file holds and your graph does not — almost certainly somebody else's \
                 work, pulled into the file after you last synced. The file is at {}, this graph \
                 last saw {}.\n\nDropped: {}{}\n\nThis is not a merge conflict and git will not \
                 catch it: a stale export is a COMPLETE document, so it merges cleanly and the \
                 missing work simply vanishes.\n\nDo this instead:\n  1. `git pull --rebase` (you \
                 may already have).\n  2. Bring the file into your graph — `import_graph` from \
                 {path}, or `compare_designs` against it first to see exactly what differs.\n  3. \
                 Export again; it will be a superset and go through.\nIf you genuinely mean to \
                 discard that work, pass accept_divergence=true.",
                dropped_nodes.len(),
                dropped_edges.len(),
                short(found),
                expected
                    .as_deref()
                    .unwrap_or("nothing — it has never synced with this file"),
                sample(dropped_nodes),
                if dropped_edges.is_empty() {
                    String::new()
                } else {
                    format!("; edges: {}", sample(dropped_edges))
                }
            )),
        }
    }
}

/// First 12 characters of a hash, enough to recognise and short enough to read.
fn short(hash: &str) -> String {
    hash.chars().take(19).collect()
}

/// At most five ids, then a count — a refusal nobody can read is a refusal
/// nobody acts on (rule 6: the cap is stated, never silent).
fn sample(ids: &[String]) -> String {
    if ids.len() <= 5 {
        return ids.join(", ");
    }
    format!("{}, … and {} more", ids[..5].join(", "), ids.len() - 5)
}

/// Assess writing `mine` over `target`.
///
/// `last_synced` is the content hash this graph believes the file carries —
/// recorded when it last exported to it or imported from it. `None` means this
/// graph has never met the file, which is not itself an error: a fresh clone
/// that has been working locally is in exactly that state, and it is only in
/// trouble if the file holds something it lacks.
pub fn assess_overwrite(
    target: Option<&GraphExport>,
    mine: &GraphExport,
    last_synced: Option<&str>,
) -> SyncVerdict {
    let Some(target) = target else {
        return SyncVerdict::Clear;
    };
    let found = target.effective_content_hash();

    // The fast path, and the one that runs on nearly every export: the file is
    // exactly where this graph left it.
    if last_synced == Some(found.as_str()) {
        return SyncVerdict::Clear;
    }
    // Writing what is already there changes nothing, whoever wrote it.
    if mine.effective_content_hash() == found {
        return SyncVerdict::Clear;
    }

    let my_nodes: BTreeSet<&str> = mine.nodes.iter().map(|n| n.node_id.as_str()).collect();
    let dropped_nodes: Vec<String> = target
        .nodes
        .iter()
        .filter(|n| !my_nodes.contains(n.node_id.as_str()))
        .map(|n| n.node_id.clone())
        .collect();

    let my_edges: BTreeSet<String> = mine.edges.iter().map(edge_key).collect();
    let dropped_edges: Vec<String> = target
        .edges
        .iter()
        .map(edge_key)
        .filter(|k| !my_edges.contains(k))
        .collect();

    if dropped_nodes.is_empty() && dropped_edges.is_empty() {
        SyncVerdict::MovedButNothingLost {
            found,
            expected: last_synced.map(str::to_string),
        }
    } else {
        SyncVerdict::WouldDrop {
            found,
            expected: last_synced.map(str::to_string),
            dropped_nodes,
            dropped_edges,
        }
    }
}

fn edge_key(e: &crate::export::ExportedEdge) -> String {
    format!("{} {} -> {}", e.edge_type, e.from_id, e.to_id)
}
