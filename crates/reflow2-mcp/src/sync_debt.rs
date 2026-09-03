//! Has the shared record moved since this seat last looked?
//!
//! # The finding this exists to fix
//!
//! `provenance::last_synced` — the content hash this seat believes is at a
//! given path — was **written by two paths and read by one**. `record_sync` is
//! called by `export_graph` and by `import_graph`; `last_synced()` was read
//! inside `export_graph` and nowhere else.
//!
//! So reflow2 could already tell, at the first moment of a session, that
//! somebody else's work had landed in the record — and said nothing until you
//! tried to export, hours later, when [`crate::sync`]'s refusal finally fired.
//! **It knew at the first moment and spoke at the last.** That is the same
//! family as `dec:one-retire-edge`'s "a marker nothing reads is a comment",
//! except here something did read it, at the wrong moment.
//!
//! # Option A, and what it deliberately is not
//!
//! Anthony chose "speak on read, gated on the hash" over three alternatives,
//! 2026-08-11 (`dec:idea-the-graph-notices-the-record-moved-without-being-asked`).
//!
//! **THE GATE IS THE DESIGN.** A seat that is merely *ahead* of the file — the
//! entire normal state of every working session — is silent, because the file
//! has not moved and every difference is the caller's own unexported work. The
//! check speaks only when the file's hash differs from what this seat recorded,
//! which means somebody else has been there. That is what keeps it rare enough
//! to be read; a check that fired on ordinary solo work would be ignored inside
//! a week.
//!
//! - NOT auto-import (option B). `import_graph` is an UPSERT, so an unasked one
//!   silently overwrites live session work with whatever the file holds, and
//!   doing it unasked makes it invisible. `dec:ask-not-repair`.
//! - NOT a refusal (option C). A refusal on READ is heavy-handed, and a session
//!   deliberately working from an older design would meet it every time. These
//!   findings carry a remedy and never block.
//!
//! # Why it lives here and not in the core
//!
//! `reflow2-core` does no file I/O, deliberately and repeatedly —
//! `reconcile_artifacts` makes the caller supply hashes, and `granularity` and
//! `consumption` both say so in their module docs. Reading the record off disk
//! therefore cannot move into `loop_status`. It belongs at the MCP layer, in
//! the same crate that already does exactly this comparison for `export_graph`.
//!
//! The comparison itself is NOT reimplemented: [`reflow2_core::sync`]'s
//! `assess_overwrite` already answers "what does the file hold that this
//! document does not", which is precisely the question, so this module supplies
//! the enumeration and the wording and borrows the judgement.

use reflow2_core::{GraphExport, sync::SyncVerdict};
use serde::Serialize;

pub use reflow2_core::provenance::SyncState;

/// How many arrived node ids to name before summarising. Enough to recognise
/// what landed, few enough that a big pull does not become a wall of ids.
pub const NAMED_ARRIVALS: usize = 8;

/// What this seat found at one target it has synced with before.
///
/// Every known target comes back, including the quiet ones: "checked three
/// records, all in step" and "checked nothing" are different facts and must not
/// share an answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SyncDebt {
    /// The file this seat has synced with before.
    pub path: String,
    /// `in_step` · `behind` · `moved_but_current` · `missing` · `unreadable`.
    /// A string rather than an enum so the served JSON is self-describing.
    pub state: String,
    /// The content hash this seat recorded for that path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// The content hash actually on disk now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<String>,
    /// Node ids the record holds that this graph does not, up to
    /// [`NAMED_ARRIVALS`]. Names what arrived, so the reader can recognise it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes_not_here: Vec<String>,
    /// How many nodes arrived in total — never truncated, so a capped list can
    /// never read as the whole set.
    pub nodes_not_here_total: usize,
    /// How many edges the record holds that this graph does not.
    pub edges_not_here_total: usize,
    /// The file's OWN embedded `content_hash` disagrees with its actual
    /// content — it was edited by something other than `export_graph`. Reported
    /// rather than hidden: it is why this check computes the hash instead of
    /// believing the one the file supplies about itself.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stamp_disagrees: bool,
    /// How many nodes the FILE holds, beside how many the LIVE graph holds.
    ///
    /// ⚠️ THESE EXIST TO MAKE `in_step` FALSIFIABLE, and the distinction they
    /// carry is the whole reason (2026-08-16, dragon Boss + reproduced here).
    /// This check answers ONE question — has the shared record moved ahead of
    /// this seat — and `ver:the-record-moved-is-surfaced` pins "ordinary
    /// unexported work is NEVER reported" as the property the design rests on.
    /// But `in_step`, in a field called `sync`, was read as ALSO meaning "my
    /// work is durable", and it never did: the cheap gate compares the file
    /// against the hash this graph last wrote it with, so a seat that has
    /// written since its last export gets a green that cannot go red.
    /// MEASURED: `in_step` with `nodes_not_here_total: 0` while two
    /// TemporalFacts sat live and absent from the export, one machine failure
    /// from gone.
    ///
    /// A caller could not tell "I compared and they match" from "I compared the
    /// file to its own recorded hash". Now they can: `live_nodes >
    /// export_nodes` means unexported work, whatever `state` says.
    ///
    /// THREE BOUNDS, stated here rather than discovered later. (1) Counts, not
    /// ids — equal counts with different ids still slip through, so this is a
    /// READING AID and not a durability guarantee. (2) NODES ONLY, and
    /// deliberately: counting edges means walking every node's adjacency, which
    /// costs what the export costs, and paying that on the path every ordinary
    /// session takes is the one thing this check was built to avoid. An
    /// edge-only divergence is therefore invisible here.
    ///
    /// (3) A PROPERTY-ONLY WRITE MOVES NEITHER COUNT, so it is invisible here
    /// too — and unlike (1) and (2) this one has been measured rather than
    /// argued. `fact:the-early-export-is-measured-and-a-property-only-write-is-
    /// invisible-to-the-net` (2026-08-31) raced a concurrent `export_graph`
    /// against a write that only changed a status: the export was early 10 times
    /// in 200, and a node-count comparison would have caught 0 of them. That is
    /// the shape of the 2026-08-30 field report — `set_artifact_checksum`, an
    /// export still carrying the old checksum — so the case this check misses is
    /// the case a user actually hit.
    ///
    /// ⭐ ALL THREE NOW REACH THE READER, which is the point. They sat in this
    /// doc comment while the served message said only "exactly where this graph
    /// left it", and a bound nobody is shown is a bound nobody applies
    /// (`fact:vocabulary-needs-three-legs-and-a-users-project-gets-none-of-it`).
    /// The in-step sentence carries the limit now, on the same reasoning that
    /// made `open_questions` speak on an empty answer: silence and an all-clear
    /// must not share a reply.
    pub export_nodes: usize,
    /// Nodes in the live graph right now. See [`Self::export_nodes`].
    pub live_nodes: usize,
}

impl SyncDebt {
    /// Is there something for the caller to actually do about this?
    ///
    /// Only `behind` — the record holds design this graph lacks. A record that
    /// moved and left this seat a superset is worth knowing and not worth
    /// acting on, and an in-step record is nothing at all.
    pub fn is_actionable(&self) -> bool {
        self.state == "behind"
    }

    /// The sentence a human needs: what happened, and what to call.
    ///
    /// Rule 4 — a finding that does not say what would fix it is a wall. This
    /// is deliberately a HINT and not a refusal: it names the remedy and never
    /// says the caller was stopped, because nothing was stopped.
    pub fn message(&self) -> String {
        match self.state.as_str() {
            "behind" => format!(
                "The shared record at {} has moved since this graph synced with it, and holds {} \
                 node(s) and {} edge(s) this graph does not{}. To take them in, call import_graph \
                 with path {} — or carry on if you meant to work from what you have.",
                self.path,
                self.nodes_not_here_total,
                self.edges_not_here_total,
                if self.nodes_not_here.is_empty() {
                    String::new()
                } else {
                    format!(
                        " ({}{})",
                        self.nodes_not_here.join(", "),
                        if self.nodes_not_here_total > self.nodes_not_here.len() {
                            format!(
                                ", +{} more",
                                self.nodes_not_here_total - self.nodes_not_here.len()
                            )
                        } else {
                            String::new()
                        }
                    )
                },
                self.path,
            ),
            "moved_but_current" => format!(
                "The shared record at {} has moved since this graph synced with it, but everything \
                 it holds is already here. Nothing to take in.",
                self.path
            ),
            "missing" => format!(
                "This seat has synced with {} before and there is no file there now.",
                self.path
            ),
            "unreadable" => format!(
                "This seat has synced with {} before and what is there now is not a readable \
                 reflow2 export.",
                self.path
            ),
            // 🛑 THE UNEXPORTED-WORK SENTENCE IS NOT HERE ANY MORE, and where
            // it used to live is the whole defect. It read
            // `self.live_nodes > self.export_nodes` — one FILE — and then said
            // "N node(s) here have never been exported", which is a claim about
            // the SEAT. On a seat with one target those coincide, and every
            // test had one target. On a seat with several they do not, and the
            // sentence is false: measured 2026-09-03 on reflow2's own graph,
            // five stale side records each claiming between 185 and 825
            // unexported nodes while the committed record held all 3662 of
            // them. It is answered once, for the seat, by [`unexported_work`].
            // THE CLEAN LINE SAYS WHAT IT CANNOT SEE. Silence here used to read
            // as "everything of mine is in that file", which this comparison has
            // never been able to promise: it counts NODES, so a write that
            // changed only a property or only an edge leaves both counts equal
            // and lands in this arm looking settled. Measured 0 of 10 caught.
            // Same remedy as `open_questions` on an empty answer — a reply that
            // cannot distinguish "nothing to report" from "nothing I can see"
            // must say which it is.
            _ => format!(
                "{} is exactly where this graph left it — as far as a NODE COUNT can see, which \
                 is blind to a write that changed only a property or only an edge (measured: 0 \
                 of 10 such early exports caught). Re-export if you have written since.",
                self.path
            ),
        }
    }
}

/// The ONE sentence a seat owes about its own unexported work — or silence.
///
/// ⭐ THE QUESTION IS ABOUT THE SEAT, AND THAT IS THE ENTIRE POINT. "Have these
/// nodes been exported" is answered by the union of the records this seat keeps,
/// not by any one of them. Asking it per file and phrasing the answer per seat
/// is how a fully-exported graph came to be told, five times in one reply, that
/// hundreds of its nodes had never been exported.
///
/// So: find the most complete record this seat is accounted for against. If it
/// already holds as much as the graph does, the work is durable and there is
/// nothing to say, however far behind every OTHER record has fallen. Otherwise
/// one line, naming that record — the one worth exporting to.
///
/// 🛑 WHY NOT "STOP TRACKING SCRATCH PATHS", which is what the symptom looks
/// like it wants. `chg:the-orientation-call-stops-rereading-stale-records` shipped
/// that rule and reverted it: fifteen tests in
/// `the_record_moved_and_the_session_is_told` failed and were right to, because a
/// hermetic test, a CI workspace and a container all put GENUINE shared records
/// under a temp dir. It also would not have worked — this seat tracks a real
/// backup outside any temp path whose stale count produces the identical false
/// sentence. Where the files live decides which ones speak; the scope error is
/// why what they say is untrue.
///
/// ⚠️ ONLY `in_step` AND `moved_but_current` RECORDS COUNT AS COVER, and the
/// exclusion is deliberately conservative. A `behind` record holds work that came
/// from somebody else, so its node count is inflated by nodes that are not this
/// seat's and cannot vouch for this seat's. Excluding it can only make this speak
/// when it need not — never stay quiet when it should speak.
///
/// The three bounds on [`SyncDebt::export_nodes`] apply unchanged: this counts
/// NODES, so equal counts with different ids, an edge-only divergence, and a
/// property-only write are all invisible to it. It is a reading aid, not a
/// durability guarantee, and the sentence says so.
pub fn unexported_work(debts: &[SyncDebt], live_nodes: usize) -> Option<String> {
    let mut best: Option<&SyncDebt> = None;
    for d in debts
        .iter()
        .filter(|d| d.state == "in_step" || d.state == "moved_but_current")
    {
        if best.is_none_or(|b| d.export_nodes > b.export_nodes) {
            best = Some(d);
        }
    }
    let best = best?;
    if best.export_nodes >= live_nodes {
        return None;
    }
    Some(format!(
        "This graph holds {} node(s), and the most complete record it is in step with — {} —          holds {}, so {} node(s) here are in no record. A record is the only copy that survives          losing the graph directory: export before you finish. (A NODE COUNT, so a write that          changed only a property or only an edge is invisible to it.)",
        live_nodes,
        best.path,
        best.export_nodes,
        live_nodes - best.export_nodes
    ))
}

/// Check every record this seat has synced with against what is on disk now.
///
/// Returns one [`SyncDebt`] per known target, in path order. An empty answer
/// means this seat has never synced with anything — never that everything is
/// fine.
///
/// `mine` is a CLOSURE and that is a cost decision, not a style one. Exporting
/// this graph to compare against is the expensive half; the hash check is the
/// cheap half, and on the overwhelmingly common in-step path the answer is
/// known before any comparison is needed. So the export is built only if some
/// record has actually moved, and never on the path every ordinary session
/// takes. It is called at most once however many targets are stale.
/// How many synced records one roll will actually open.
///
/// 📐 MEASURED, 2026-08-24, and this is why a bound exists at all. reflow2's own
/// seat had accumulated **16 targets totalling 102 MB** — the committed export,
/// a backup, and **fourteen one-off probe dumps written by past sessions**,
/// three of them belonging to a different project. Every `loop_status` re-read
/// and re-parsed all of it: `sync_status` measured **28.3s**, inside the call
/// `cap:loop-status` promises is CHEAP and every session is told to run.
///
/// ⭐ THE DEFECT IS UNBOUNDED ACCUMULATION, NOT WHERE THE FILES LIVE. The first
/// attempt at this fix refused to track anything under the OS temp directory,
/// on the reasoning that a scratch file is not a SHARED record. **Fifteen tests
/// in `the_record_moved_and_the_session_is_told` failed and were right to**: a
/// hermetic test puts a genuine shared record in a temp dir, and so does a CI
/// workspace and a container. One of them is named *"the case the whole thing
/// exists for — your brother pushed, you pulled"*. A rule that silences the
/// feature's own reason for existing is the wrong rule, however convenient the
/// paths on this machine made it look.
const MAX_RECORDS_CHECKED: usize = 6;

/// Records this roll did not open, with why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncNotChecked {
    pub count: usize,
    pub paths: Vec<String>,
    pub note: String,
}

/// Order the targets a roll should open, freshest first, and say what is left.
///
/// Freshest by the TARGET FILE's own mtime — an observation of a file, not a
/// clock the core invented — because the record somebody is actually
/// collaborating on is the one that moved most recently, and a probe dump from
/// five sessions ago is exactly what should fall off the end.
fn ordered_targets(state: &reflow2_core::provenance::SyncState) -> (Vec<String>, Vec<String>) {
    let mut all: Vec<(std::time::SystemTime, String)> = state
        .last_synced
        .keys()
        .map(|p| {
            let m = std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (m, p.clone())
        })
        .collect();
    // Freshest first; ties by path so the answer is deterministic.
    all.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut checked: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for (i, (_, path)) in all.into_iter().enumerate() {
        if i < MAX_RECORDS_CHECKED {
            checked.push(path);
        } else {
            skipped.push(path);
        }
    }
    checked.sort();
    (checked, skipped)
}

/// The targets a roll left unopened — for a caller that must say so out loud.
pub fn not_checked(graph_path: &str) -> Option<SyncNotChecked> {
    let state = reflow2_core::provenance::read_sync_state(graph_path);
    let (_, skipped) = ordered_targets(&state);
    if skipped.is_empty() {
        return None;
    }
    Some(SyncNotChecked {
        count: skipped.len(),
        note: format!(
            "{} record(s) this seat has synced with were NOT opened by this roll. A roll opens the \
             {MAX_RECORDS_CHECKED} most recently modified, because every one costs a full document \
             read and this list only ever grows — one seat had 16 targets totalling 102 MB, mostly \
             one-off exports from past sessions. If one of these is a record you actually \
             collaborate on, touch it or re-export to it and it returns to the front. Named here \
             rather than dropped: a roll that quietly checks fewer than it knows about is the \
             silent truncation this project refuses.",
            skipped.len()
        ),
        paths: skipped,
    })
}

pub fn sync_debt(
    graph_path: &str,
    live_nodes: usize,
    mine: &dyn Fn() -> Option<GraphExport>,
) -> Vec<SyncDebt> {
    let mut built: Option<Option<GraphExport>> = None;
    let state = reflow2_core::provenance::read_sync_state(graph_path);
    let (checked, _) = ordered_targets(&state);
    let mut out = Vec::new();

    for (path, expected) in state
        .last_synced
        .iter()
        .filter(|(p, _)| checked.contains(p))
    {
        let target = std::path::Path::new(path);
        if !target.exists() {
            out.push(bare(
                path,
                "missing",
                Some(expected.clone()),
                None,
                live_nodes,
                0,
            ));
            continue;
        }
        let on_disk = std::fs::read_to_string(target)
            .ok()
            .and_then(|raw| serde_json::from_str::<GraphExport>(&raw).ok());
        let Some(on_disk) = on_disk else {
            out.push(bare(
                path,
                "unreadable",
                Some(expected.clone()),
                None,
                live_nodes,
                0,
            ));
            continue;
        };
        // ⚠️ COMPUTED, NEVER the hash the file states about itself.
        // `effective_content_hash` TRUSTS the embedded `content_hash` and only
        // computes when it is absent — so a document edited by anything other
        // than `export_graph` (a merge, a hand-fix, another tool) keeps its old
        // stamp and would read as "exactly where this graph left it" while its
        // content had moved. Caught end-to-end on 2026-08-11 by simulating the
        // very case this feature exists for: work appended to the record.
        // The document is already parsed, so computing costs nothing extra.
        let found = on_disk.compute_content_hash();
        let stamp_disagrees = on_disk.verify_content_hash() == Some(false);

        let export_nodes = on_disk.nodes.len();

        // THE CHEAP GATE, and the path every ordinary session takes: the file
        // is exactly where this graph left it, so any difference is the
        // caller's own unexported work. Answered without exporting anything.
        //
        // ⚠️ AND THAT IS EXACTLY WHY THE COUNTS RIDE ALONG. This branch cannot
        // go red for unexported work — it compares the file against the hash
        // this graph last wrote it with, so a seat that has written since its
        // last export lands here every time. `live_nodes` vs `export_nodes` is
        // the only thing on this path that CAN disagree, which is what makes
        // the green falsifiable rather than merely reassuring.
        if &found == expected {
            let mut d = bare(
                path,
                "in_step",
                Some(expected.clone()),
                Some(found),
                live_nodes,
                export_nodes,
            );
            d.stamp_disagrees = stamp_disagrees;
            out.push(d);
            continue;
        }

        // Something moved. NOW the export is worth building — once, however
        // many targets are stale.
        let mine = built.get_or_insert_with(mine);
        let Some(mine) = mine.as_ref() else {
            out.push(bare(
                path,
                "unreadable",
                Some(expected.clone()),
                Some(found),
                live_nodes,
                export_nodes,
            ));
            continue;
        };

        // The judgement itself is borrowed whole from the write path rather
        // than reimplemented — assess_overwrite already answers "what does the
        // file hold that this document does not".
        //
        // ⚠️ `last_synced` IS PASSED AS `None` DELIBERATELY. Its only role in
        // there is a fast path that compares against `effective_content_hash`,
        // which believes the stamp the document states about itself; we have
        // already made that comparison above with a COMPUTED hash and know the
        // content moved, so handing it the shortcut would let a stale stamp
        // send it straight back to `Clear`. Passing None asks for the full
        // document comparison, which is the whole reason we got here.
        match reflow2_core::sync::assess_overwrite(Some(&on_disk), mine, None) {
            SyncVerdict::Clear => {
                let mut d = bare(
                    path,
                    "in_step",
                    Some(expected.clone()),
                    Some(found),
                    live_nodes,
                    export_nodes,
                );
                d.stamp_disagrees = stamp_disagrees;
                out.push(d)
            }
            SyncVerdict::MovedButNothingLost { .. } => {
                let mut d = bare(
                    path,
                    "moved_but_current",
                    Some(expected.clone()),
                    Some(found),
                    live_nodes,
                    export_nodes,
                );
                d.stamp_disagrees = stamp_disagrees;
                out.push(d)
            }
            SyncVerdict::WouldDrop {
                dropped_nodes,
                dropped_edges,
                ..
            } => out.push(SyncDebt {
                path: path.clone(),
                state: "behind".into(),
                expected: Some(expected.clone()),
                found: Some(found),
                nodes_not_here: dropped_nodes.iter().take(NAMED_ARRIVALS).cloned().collect(),
                nodes_not_here_total: dropped_nodes.len(),
                edges_not_here_total: dropped_edges.len(),
                stamp_disagrees,
                export_nodes,
                live_nodes,
            }),
        }
    }
    out
}

fn bare(
    path: &str,
    state: &str,
    expected: Option<String>,
    found: Option<String>,
    live_nodes: usize,
    export_nodes: usize,
) -> SyncDebt {
    SyncDebt {
        path: path.to_string(),
        state: state.to_string(),
        expected,
        found,
        nodes_not_here: Vec::new(),
        nodes_not_here_total: 0,
        edges_not_here_total: 0,
        stamp_disagrees: false,
        export_nodes,
        live_nodes,
    }
}
