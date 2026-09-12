//! The one place an export document becomes a file on disk.
//!
//! # Why it is a module rather than a block inside the tool
//!
//! Two callers now write the export: `export_graph`, when a session asks, and
//! the write-through in [`crate::auto_export`], which the server does on its
//! own (`req:the-server-keeps-the-working-tree-export-current`). They must agree
//! on all four of the things that happen at this seam — where the lineage
//! anchors, whether the write would drop design the file already holds, what
//! `wrote` reports, and recording that this seat is now in step — because a
//! second copy that drifts is the failure this project keeps meeting:
//! *a mechanism is built, wired into the one place that motivated it, and the
//! siblings are left alone.* The version guard, the `unknown field`
//! interception and the adjacency memo were all that shape.
//!
//! So the tool keeps what is genuinely its own — the overwrite guard on a
//! caller-supplied path, the receipt, `accept_divergence` — and the seam lives
//! here.

use std::path::Path;

use reflow2_core::GraphExport;

/// What the write did, for the caller to report.
#[derive(Debug, Clone)]
pub(crate) struct Written {
    /// `created` / `changed` / `unchanged` — the fact the hashes cannot tell
    /// you, because `chain_after` gives an unchanged export the predecessor's
    /// own `prev` and the two receipts come out identical.
    pub wrote: &'static str,
    /// Where the lineage anchored: `origin/main@<sha>`, `disk`, or `nothing`.
    pub chained_from: String,
    /// Why the committed anchor was not used, when it was not.
    pub chain_note: Option<&'static str>,
    /// What the divergence assessment said, when it said anything.
    pub sync_note: Option<String>,
    /// Bytes written.
    pub bytes: usize,
}

/// Why nothing was written. Both are refusals rather than failures: the caller
/// decides whether that is an error to return or a skip to report.
#[derive(Debug, Clone)]
pub(crate) enum WriteRefusal {
    /// Writing would drop design the file on disk already holds — somebody
    /// else's work, or a hand edit. Carries the message to show.
    Loss(String),
    /// The file could not be written.
    Io(String),
}

impl WriteRefusal {
    pub(crate) fn message(&self) -> &str {
        match self {
            WriteRefusal::Loss(m) | WriteRefusal::Io(m) => m,
        }
    }
}

/// Anchor `export`'s lineage, check the write is not lossy, and write it.
///
/// `accept_divergence` is the caller's opt-in to overwrite work the file holds
/// and the graph does not. The write-through never passes it: an automatic
/// write that discards somebody's edit is the one outcome that cannot be undone
/// from the thing that caused it.
pub(crate) fn chain_and_write(
    export: &mut GraphExport,
    path: &str,
    graph_path: Option<&str>,
    accept_divergence: bool,
) -> Result<Written, WriteRefusal> {
    let target = Path::new(path);

    // WHERE THE LINEAGE ANCHORS, asked once and used by every branch below.
    //
    // The chain used to grow from the file being replaced, which made every
    // intermediate save a hop and put `dec:export-once-per-pr` on the author to
    // remember. Anchored at the merge-base with the default branch instead, any
    // number of commits on a branch each chain from the same committed ancestor
    // and a squash-merge lands exactly one hop per PR. Outside a git repository
    // nothing changes: `Err` carries the reason and every branch falls back to
    // the file on disk.
    let committed = crate::git::committed_predecessor(target);
    let mut chained_from = match &committed {
        Ok(c) => c.source.clone(),
        Err(_) => "disk".to_string(),
    };
    let mut chain_note = None;
    let mut sync_note = None;
    let mut wrote = "created";

    if target.exists() {
        match std::fs::read_to_string(target)
            .ok()
            .and_then(|raw| serde_json::from_str::<GraphExport>(&raw).ok())
        {
            Some(predecessor) => {
                // req:stale-seat-knows. Before the lineage link, the question
                // git answers with a non-fast-forward refusal: would writing
                // this drop design the file already holds? Only the lossy case
                // stops — see reflow2_core::sync.
                let last = graph_path.and_then(|g| reflow2_core::provenance::last_synced(g, path));
                let verdict = reflow2_core::sync::assess_overwrite(
                    Some(&predecessor),
                    export,
                    last.as_deref(),
                );
                if verdict.is_loss() && !accept_divergence {
                    return Err(WriteRefusal::Loss(
                        verdict.message(path).unwrap_or_default(),
                    ));
                }
                sync_note = verdict.message(path);
                wrote = if predecessor.effective_content_hash() == export.effective_content_hash() {
                    "unchanged"
                } else {
                    "changed"
                };
                match &committed {
                    Ok(c) => export.chain_after(&c.doc),
                    Err(reason) => {
                        export.chain_after(&predecessor);
                        chain_note = Some(reason.reason());
                    }
                }
            }
            None => {
                wrote = "changed";
                // The file on disk is not an export — but the COMMITTED version
                // of this path may still be one, and it is the honest ancestor
                // either way.
                match &committed {
                    Ok(c) => export.chain_after(&c.doc),
                    Err(_) => {
                        chained_from = "nothing".to_string();
                        chain_note = Some(
                            "the file being replaced was not a reflow2 export — no lineage \
                             recorded",
                        );
                    }
                }
            }
        }
    } else {
        // Nothing at this path — but a committed version can still exist
        // (somebody deleted it, or this is a fresh checkout). Writing a record
        // whose ancestry says "none" when git knows otherwise is the silent
        // wrong answer the committed anchor exists to remove.
        match &committed {
            Ok(c) => export.chain_after(&c.doc),
            Err(_) => chained_from = "nothing".to_string(),
        }
    }

    // Through `serde_json::Value` so keys serialize sorted (its object is a
    // BTreeMap) — the same convention as the committed design export, so a file
    // this writes diffs cleanly against one written before it.
    let v = serde_json::to_value(&*export)
        .map_err(|e| WriteRefusal::Io(format!("cannot serialize the export: {e}")))?;
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&v)
            .map_err(|e| WriteRefusal::Io(format!("cannot serialize the export: {e}")))?
    );
    std::fs::write(target, &text)
        .map_err(|e| WriteRefusal::Io(format!("cannot write export to {path}: {e}")))?;

    // This seat is now in step with what it just wrote — so the next export
    // takes the one-hash fast path instead of comparing documents, and a file
    // that moves after this is detectable (req:stale-seat-knows).
    if let (Some(gp), Some(hash)) = (graph_path, &export.content_hash) {
        reflow2_core::provenance::record_sync(gp, path, hash);
    }

    Ok(Written {
        wrote,
        chained_from,
        chain_note,
        sync_note,
        bytes: text.len(),
    })
}
