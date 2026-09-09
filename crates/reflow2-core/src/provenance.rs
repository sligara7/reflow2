//! Which reflow2 wrote this graph — recorded beside it, checked on open.
//!
//! The schema lives in the **binary** (`schema.rs` embeds the ten YAMLs via
//! `include_str!`), while the store holds only nodes and edges. Nothing was
//! written to the graph directory to say which vocabulary produced it, and
//! validation runs on write and never on read — so a graph opened by a
//! different reflow2 simply behaved differently, with no error and no marker.
//!
//! That stopped being hypothetical the moment a node type was added (BL-4 took
//! the schema from 26 types to 27): there are now two vintages in the wild.
//!
//! # What this refuses, and what it does not
//!
//! Refusing on *any* mismatch would be worse than the problem. Schema growth
//! here is additive, so a graph written before a type existed is entirely
//! readable by a binary that knows about it — refusing would lock someone out
//! of their own design over a change that cannot hurt them.
//!
//! So the line is drawn at **a graph from the future**: one written by a
//! reflow2 whose schema knew *more* than this one does. That graph can hold
//! nodes this binary has no vocabulary for, and reading it means silently
//! seeing less than is there. That is refused loudly. Everything else opens,
//! and the difference is reported rather than hidden.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::foundation::core::{DynoError, Schema};
use serde::{Deserialize, Serialize};

/// Node types this reflow2 once had and has since retired. A graph that carries
/// one in its stamp is not from the future — it predates the removal — so the
/// honest recovery is to migrate the graph, not to update the binary. This is
/// what lets a set-based stamp say "retired → migrate" rather than the
/// count-only hedge (BL-86).
///
/// `QualityGate` is the first entry, added 2026-09-07 with the removal itself
/// (dec:qualitygate-is-retired-the-phase-gate-dissolved-into-the-detectors).
/// THE ORDER MATTERS AND WAS LEARNED THE HARD WAY: cutting the type from
/// `schema/verify.yaml` without adding it here left the guard reading an
/// ordinary graph as one from the future, so the message told the operator
/// their reflow2 was BEHIND and to rebuild — the exact opposite of what to do.
/// A retirement is two edits, not one.
const RETIRED_NODE_TYPES: &[&str] = &["QualityGate"];

/// Edge types this reflow2 retired. `VALIDATES` and `ENABLES` were removed by
/// the edge-orthogonality change (55 → 53) without a version bump — the exact
/// case that made the count-only stamp ambiguous.
const RETIRED_EDGE_TYPES: &[&str] = &["VALIDATES", "ENABLES"];

/// The reflow2 that wrote a graph, as recorded beside it.
///
/// Deliberately small and boring: version facts only, no timestamps and no
/// clock. It is read before anything is trusted, so it must not depend on the
/// vocabulary it is describing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStamp {
    /// `reflow2-core`'s package version.
    pub reflow2_version: String,
    /// The merged schema's declared version.
    pub schema_version: u32,
    /// How many node types that schema had. The signal that actually moves —
    /// the declared version has never been bumped.
    pub node_types: usize,
    /// How many edge types that schema had.
    pub edge_types: usize,
    /// *Which* node types the schema carried, sorted. `None` on a legacy
    /// count-only stamp written before BL-86; present from now on. The set is
    /// what lets a removal be diagnosed precisely: a graph naming a type this
    /// binary lacks either used one this binary retired (migrate) or one it has
    /// never heard of (you are behind) — the count alone cannot tell them apart.
    #[serde(default)]
    pub node_type_names: Option<Vec<String>>,
    /// *Which* edge types the schema carried, sorted. See `node_type_names`.
    #[serde(default)]
    pub edge_type_names: Option<Vec<String>>,
}

impl GraphStamp {
    /// The stamp this binary would write.
    pub fn current(schema: &Schema) -> Self {
        let mut node_names: Vec<String> = schema.node_types.keys().cloned().collect();
        node_names.sort();
        let mut edge_names: Vec<String> = schema.edge_types.keys().cloned().collect();
        edge_names.sort();
        Self {
            reflow2_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: schema.version,
            node_types: schema.node_types.len(),
            edge_types: schema.edge_types.len(),
            node_type_names: Some(node_names),
            edge_type_names: Some(edge_names),
        }
    }

    /// True when the recorded schema knew more *by count* than `other` does.
    /// The legacy signal, kept for the fallback when a stamp carries no type-name
    /// sets — the set-based [`unreadable_by`](Self::unreadable_by) is preferred
    /// whenever both stamps carry them.
    fn knows_more_than(&self, other: &Self) -> bool {
        self.node_types > other.node_types || self.edge_types > other.edge_types
    }

    /// Whether a graph written under `self` cannot be safely read by the binary
    /// whose stamp is `now`, and if so, the message explaining exactly why and
    /// how to recover. `None` means readable (open it).
    ///
    /// `now` is always [`GraphStamp::current`], so it carries the binary's live
    /// type-name sets. When `self` carries them too (any graph written since
    /// BL-86), the answer is exact: partition the types the graph names but this
    /// binary lacks into *retired* (migrate the graph) and *unknown* (update the
    /// binary), and name them. When `self` is a legacy count-only stamp, fall
    /// back to the count comparison — but still sharpen the message with the
    /// retired registry, since a count excess that the retired types fully
    /// explain is almost certainly a graph that predates the removal.
    /// How this graph's vocabulary differs from this binary's, partitioned by
    /// what the caller can DO about it.
    ///
    /// Returning the partition rather than a finished message is the whole
    /// point: an UNKNOWN type and a RETIRED one need opposite handling, and a
    /// single string forced them through one test. Field-reported 2026-09-08 —
    /// a graph holding ZERO instances of a retired type was refused, on the
    /// grounds that opening it "could silently show you less of your design
    /// than it holds", which is false when it holds none.
    fn vocabulary_gap(&self, now: &Self) -> VocabularyGap {
        match (
            &self.node_type_names,
            &self.edge_type_names,
            &now.node_type_names,
            &now.edge_type_names,
        ) {
            (Some(gnodes), Some(gedges), Some(nnodes), Some(nedges)) => {
                let now_nodes: BTreeSet<&str> = nnodes.iter().map(String::as_str).collect();
                let now_edges: BTreeSet<&str> = nedges.iter().map(String::as_str).collect();
                let mut retired_nodes = Vec::new();
                let mut retired = Vec::new();
                let mut unknown = Vec::new();
                for t in gnodes.iter().filter(|t| !now_nodes.contains(t.as_str())) {
                    if RETIRED_NODE_TYPES.contains(&t.as_str()) {
                        retired.push(t.clone());
                        // Only NODE types can be counted — an edge of a retired
                        // type is not addressable by a node scan, so an edge
                        // retirement keeps the old, conservative behaviour.
                        retired_nodes.push(t.clone());
                    } else {
                        unknown.push(t.clone());
                    }
                }
                for t in gedges.iter().filter(|t| !now_edges.contains(t.as_str())) {
                    if RETIRED_EDGE_TYPES.contains(&t.as_str()) {
                        retired.push(t.clone());
                    } else {
                        unknown.push(t.clone());
                    }
                }
                if retired.is_empty() && unknown.is_empty() {
                    return VocabularyGap::None; // additive — readable in full
                }
                let message = refusal_named(&retired, &unknown, self, now);
                // ⭐ THE SPLIT. An UNKNOWN type means this binary is BEHIND: it
                // cannot read what it has never heard of, the stamp is the only
                // evidence there is, and refusing is correct. A RETIRED type
                // means this binary is AHEAD — it knows the type, so it can ASK
                // THE STORE whether any survive. Mixed goes to Behind: the
                // unknown half is unanswerable either way.
                if !unknown.is_empty() {
                    VocabularyGap::Behind(message)
                } else if retired_nodes.len() == retired.len() {
                    VocabularyGap::Retired {
                        node_types: retired_nodes,
                        message,
                    }
                } else {
                    // A retired EDGE type is in the mix and cannot be counted.
                    VocabularyGap::Behind(message)
                }
            }
            // A legacy count-only stamp on at least one side (in practice `self`,
            // since `now` is always current): the names are unavailable, so there
            // is nothing to count and the conservative refusal stands.
            _ => {
                if !self.knows_more_than(now) {
                    return VocabularyGap::None;
                }
                let node_excess = self.node_types.saturating_sub(now.node_types);
                let edge_excess = self.edge_types.saturating_sub(now.edge_types);
                let retired_explains = node_excess <= RETIRED_NODE_TYPES.len()
                    && edge_excess <= RETIRED_EDGE_TYPES.len();
                VocabularyGap::Behind(refusal_by_count(retired_explains, self, now))
            }
        }
    }
}

/// What a stamp comparison found, partitioned by what can be done about it.
enum VocabularyGap {
    /// This binary knows every type the graph names.
    None,
    /// Refuse. Either this binary is genuinely behind, or the stamp is too old
    /// to say which types differ — in both cases the population is unknowable.
    Behind(String),
    /// This binary RETIRED these node types. Whether to refuse depends on
    /// whether the graph actually holds any, which only the store can answer.
    Retired {
        node_types: Vec<String>,
        message: String,
    },
}

/// The recovery recipe for a graph that predates a type retirement.
fn migrate_recipe() -> &'static str {
    "migrate the graph: import a committed export into a fresh graph, or export it \
     with the reflow2 that wrote it and import it here. Any retired type is dropped \
     and named on import, so re-express it if the design used it"
}

/// Refusal message when the graph's stamp names its types (BL-86): say exactly
/// which types this binary lacks and the correct path for each.
fn refusal_named(
    retired: &[String],
    unknown: &[String],
    was: &GraphStamp,
    now: &GraphStamp,
) -> String {
    let mut lines = vec![format!(
        "this graph (written by reflow2 {}) names types this reflow2 ({}) cannot read, so \
         opening it could silently show you less of your design than it holds — refused.",
        was.reflow2_version, now.reflow2_version
    )];
    if !unknown.is_empty() {
        lines.push(format!(
            "\u{20}\u{2022} Your reflow2 is BEHIND: it has never heard of {} — update reflow2 (or \
             rebuild from a current checkout) and reopen.",
            unknown.join(", ")
        ));
    }
    if !retired.is_empty() {
        lines.push(format!(
            "\u{20}\u{2022} This graph predates a schema change: it uses {}, which this reflow2 \
             RETIRED, and your reflow2 is current — {}.",
            retired.join(", "),
            migrate_recipe()
        ));
    }
    lines.join("\n")
}

/// Refusal message for a legacy count-only stamp: the names are gone, so reason
/// from counts, but lead with migration when the retired registry fully explains
/// the excess (the common case: a graph from before VALIDATES/ENABLES were cut).
fn refusal_by_count(retired_explains: bool, was: &GraphStamp, now: &GraphStamp) -> String {
    let head = format!(
        "this graph was written by reflow2 {} and declares more schema types \
         ({} node / {} edge) than the reflow2 you are running ({}: {} / {}); opening it could \
         silently show you less of your design than it holds, so it is refused.",
        was.reflow2_version,
        was.node_types,
        was.edge_types,
        now.reflow2_version,
        now.node_types,
        now.edge_types
    );
    let behind = "\u{20}\u{2022} Your reflow2 is BEHIND the one that wrote this graph — update \
         reflow2 (or rebuild from a current checkout) and reopen.";
    let predates = format!(
        "\u{20}\u{2022} The graph PREDATES a schema change that retired some types, and your \
         reflow2 is current — {}.",
        migrate_recipe()
    );
    if retired_explains {
        // The excess is fully accounted for by the types this reflow2 retired —
        // most likely a pre-removal graph. Lead with migration; keep behind as
        // the alternative, since a count-only stamp cannot make it certain.
        format!(
            "{head}\nThis excess is exactly consistent with a graph written before this reflow2 \
             retired {}, so most likely:\n{}\nIf instead this graph came from a NEWER reflow2:\n{}",
            RETIRED_EDGE_TYPES.join(", "),
            predates,
            behind
        )
    } else {
        format!(
            "{head} This happens for one of two reasons, and the count alone cannot tell them apart:\n{behind}\n{predates}"
        )
    }
}

/// What opening a graph found about the reflow2 that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Provenance {
    /// Written by a reflow2 with the same vocabulary as this one.
    Match { stamp: GraphStamp },
    /// No stamp beside the graph. Either it predates this check or the file was
    /// removed; a stamp is written now, describing the *current* binary rather
    /// than pretending to know what wrote it.
    Unstamped { stamped_now: GraphStamp },
    /// Written before some of this binary's vocabulary existed. Safe: schema
    /// growth is additive, so nothing in the graph is unreadable.
    OlderGraph { was: GraphStamp, now: GraphStamp },
}

impl Provenance {
    /// A line worth showing a user, or `None` when there is nothing to say.
    pub fn note(&self) -> Option<String> {
        match self {
            Provenance::Match { .. } => None,
            Provenance::Unstamped { stamped_now } => Some(format!(
                "this graph carried no version stamp; recording reflow2 {} \
                 ({} node types, {} edge types) from now on",
                stamped_now.reflow2_version, stamped_now.node_types, stamped_now.edge_types
            )),
            Provenance::OlderGraph { was, now } => Some(format!(
                "this graph was written by reflow2 {} ({} node types, {} edge types); \
                 you are running {} ({}, {}). Additive only — everything in it still reads.",
                was.reflow2_version,
                was.node_types,
                was.edge_types,
                now.reflow2_version,
                now.node_types,
                now.edge_types
            )),
        }
    }
}

/// Where the stamp lives: a sibling of the graph directory, never inside it.
///
/// RocksDB owns its directory; putting a file in there invites it to be tidied
/// away by a compaction or tripped over by a future format. A sibling also
/// survives being read before the store is opened, which is the whole point.
pub fn stamp_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string());
    match name {
        Some(n) => p.with_file_name(format!("{n}.meta.json")),
        None => PathBuf::from(format!("{graph_path}.meta.json")),
    }
}

/// Where this seat records the shared export it last synced with
/// (`req:stale-seat-knows`): `<graph-path>.sync.json`, a sibling of the store
/// like the stamp.
///
/// **A separate file from the stamp on purpose.** The stamp answers "which
/// reflow2 wrote this graph" and is rewritten wholesale on every open by
/// `check_and_stamp`; sync state answers "what did this seat last see of the
/// shared record", changes on a different schedule, and must survive that
/// rewrite. Folding it into the stamp would also have changed that file's
/// shape, and every existing graph would then fail to open with "the version
/// stamp is not readable" — a migration nobody asked for, to hold one string.
///
/// Machine-local, like the graph it sits beside: never committed, never shared.
pub fn sync_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string());
    match name {
        Some(n) => p.with_file_name(format!("{n}.sync.json")),
        None => PathBuf::from(format!("{graph_path}.sync.json")),
    }
}

/// What this seat last saw of each shared export it uses, keyed by the path.
///
/// Keyed by path because one graph can legitimately publish to more than one
/// file — a full export and a published surface, say — and remembering only
/// the most recent would make the two files disarm each other's check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    /// Absolute path → the `content_hash` this seat last wrote there or read
    /// from there.
    #[serde(default)]
    pub last_synced: std::collections::BTreeMap<String, String>,
    /// Absolute path → what this seat OBSERVED about the file the last time it
    /// read it in full. Enough to know from a `stat` alone whether the bytes
    /// can have changed, so an unchanged target is never re-read
    /// (`dec:an-unchanged-sync-target-is-not-re-parsed`). Absent for targets
    /// recorded before this existed; the first full read fills it in.
    #[serde(default)]
    pub observed: std::collections::BTreeMap<String, SyncObservation>,
}

/// One full read of a sync target, remembered so the next check can skip it.
///
/// `len` and `mtime_unix_nanos` are the stat gate: if both match and the
/// recorded hash is still what `last_synced` expects, the content cannot have
/// changed short of a same-size same-mtime rewrite, which is `make`'s bet too.
/// `nodes` is the count the in-step message needs, which is the ONLY reason
/// the file was being parsed at all. Measured 2026-09-05: six targets, 53.7 MB,
/// a full typed parse plus TWO canonical re-serialisations and hashes each,
/// 3.6 s per `loop_status`, five of the six being dead scratch exports that had
/// not changed in weeks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SyncObservation {
    pub hash: String,
    pub len: u64,
    pub mtime_unix_nanos: i64,
    pub nodes: usize,
}

/// Read this seat's sync record. A missing or unreadable file is "never
/// synced", not an error: the check that consumes it treats not-knowing as a
/// reason to look harder, so losing the file costs a warning, never safety.
pub fn read_sync_state(graph_path: &str) -> SyncState {
    std::fs::read_to_string(sync_path(graph_path))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Record that this seat is in step with `hash` at `target`.
///
/// Best-effort by design: failing to write the marker must never fail an export
/// that already succeeded. The cost of losing it is one extra content check on
/// the next write, which is the safe direction to fail in.
pub fn record_sync(graph_path: &str, target: &str, hash: &str) {
    let mut state = read_sync_state(graph_path);
    let key = std::fs::canonicalize(target)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target.to_string());
    state.last_synced.insert(key, hash.to_string());
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(sync_path(graph_path), json + "\n");
    }
}

/// Remember what a full read of `target` observed, so the next check can
/// answer from a `stat`. Keyed exactly as `record_sync` keys `last_synced`.
pub fn record_sync_observation(graph_path: &str, target: &str, obs: SyncObservation) {
    let mut state = read_sync_state(graph_path);
    let key = std::fs::canonicalize(target)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target.to_string());
    state.observed.insert(key, obs);
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(sync_path(graph_path), json + "\n");
    }
}

/// What this seat believes is at `target`, if it has ever synced with it.
pub fn last_synced(graph_path: &str, target: &str) -> Option<String> {
    let state = read_sync_state(graph_path);
    let key = std::fs::canonicalize(target)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| target.to_string());
    state.last_synced.get(&key).cloned()
}

/// Read the stamp beside a graph, compare it to this binary, and refresh it.
///
/// Fails loud — and refuses to open — only when the graph was written by a
/// reflow2 that knew *more* types than this one. Reading such a graph would
/// silently show less than it holds, which is the failure this whole check
/// exists to prevent; every other difference is reported and opened.
///
/// A stamp that cannot be parsed is reported as an error rather than
/// overwritten: it may be the only record of what wrote the graph.
/// `retired_population` answers the ONE question a stamp cannot: of these
/// retired node types, which does the graph actually HOLD? It is a closure
/// rather than a store handle because this module must not depend on the
/// storage layer, and because the caller is the only place that knows which
/// design id to count under. It returns the subset with instances.
///
/// **A scan failure must propagate, never read as "none".** A retired type that
/// cannot be counted is exactly the case where the conservative refusal is
/// still right, and collapsing an I/O error to zero would open the graph the
/// guard exists to protect.
pub fn check_and_stamp(
    graph_path: &str,
    schema: &Schema,
    retired_population: impl Fn(&[String]) -> Result<Vec<String>, DynoError>,
) -> Result<Provenance, DynoError> {
    let now = GraphStamp::current(schema);
    let path = stamp_path(graph_path);

    let existing: Option<GraphStamp> = match std::fs::read_to_string(&path) {
        Ok(text) => Some(serde_json::from_str(&text).map_err(|e| {
            DynoError::Serialization(format!(
                "the version stamp at {} is not readable ({e}). It records which reflow2 \
                 wrote this graph; fix or remove it rather than leaving it unreadable.",
                path.display()
            ))
        })?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(DynoError::Storage(format!(
                "cannot read the version stamp at {}: {e}",
                path.display()
            )));
        }
    };

    let verdict = match existing {
        None => Provenance::Unstamped {
            stamped_now: now.clone(),
        },
        Some(was) if was == now => Provenance::Match { stamp: was },
        Some(was) => match was.vocabulary_gap(&now) {
            VocabularyGap::Behind(message) => return Err(DynoError::Storage(message)),
            VocabularyGap::Retired {
                node_types,
                message,
            } => {
                // ASK THE STORE. The refusal's own justification is that opening
                // "could silently show you less of your design than it holds" —
                // which is false when it holds none of them. Refuse only on a
                // population, never on a declaration.
                let populated = retired_population(&node_types)?;
                if !populated.is_empty() {
                    return Err(DynoError::Storage(format!(
                        "{message}\n \u{2022} AND THE GRAPH ACTUALLY HOLDS {}: node(s) of {} \
                         are stored here, so opening would show you less than it holds. This is \
                         the case the refusal is for.",
                        populated.join(", "),
                        if populated.len() == 1 {
                            "this type"
                        } else {
                            "these types"
                        }
                    )));
                }
                Provenance::OlderGraph {
                    was,
                    now: now.clone(),
                }
            }
            VocabularyGap::None => Provenance::OlderGraph {
                was,
                now: now.clone(),
            },
        },
    };

    // Refresh on the way through, so the stamp tracks the newest reflow2 that
    // has held this graph. Never write over an unreadable one — that path
    // returned above.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(&now).map_err(|e| {
        DynoError::Serialization(format!("cannot serialize the version stamp: {e}"))
    })?;
    std::fs::write(&path, json + "\n").map_err(|e| {
        DynoError::Storage(format!(
            "cannot write the version stamp at {}: {e}",
            path.display()
        ))
    })?;

    Ok(verdict)
}

#[cfg(test)]
mod tests {
    /// The message a gap would refuse with, or None when it opens. The unit
    /// tests below are about MESSAGE WORDING — which types are named, and
    /// whether the operator is told to migrate or to rebuild — so they read the
    /// message directly. Whether a RETIRED gap actually refuses now depends on
    /// the store's population, which is covered end to end in
    /// `tests/provenance.rs` rather than here.
    fn refusal_of(was: &super::GraphStamp, now: &super::GraphStamp) -> Option<String> {
        match was.vocabulary_gap(now) {
            super::VocabularyGap::None => None,
            super::VocabularyGap::Behind(m) => Some(m),
            super::VocabularyGap::Retired { message, .. } => Some(message),
        }
    }

    use super::*;

    /// A legacy count-only stamp — no type-name sets (pre-BL-86).
    fn legacy(v: &str, n: usize, e: usize) -> GraphStamp {
        GraphStamp {
            reflow2_version: v.into(),
            schema_version: 1,
            node_types: n,
            edge_types: e,
            node_type_names: None,
            edge_type_names: None,
        }
    }

    /// A set-based stamp that names its types (BL-86 and after).
    fn named(v: &str, nodes: &[&str], edges: &[&str]) -> GraphStamp {
        let nn: Vec<String> = nodes.iter().map(|s| s.to_string()).collect();
        let ne: Vec<String> = edges.iter().map(|s| s.to_string()).collect();
        GraphStamp {
            reflow2_version: v.into(),
            schema_version: 1,
            node_types: nn.len(),
            edge_types: ne.len(),
            node_type_names: Some(nn),
            edge_type_names: Some(ne),
        }
    }

    /// A RETIREMENT IS TWO EDITS, AND THIS PINS THE SECOND ONE.
    ///
    /// NOT a duplicate of `set_based_retired_type_says_migrate_not_behind`,
    /// which covers the EDGE registry. `unreadable_by` walks nodes and edges in
    /// two separate loops against two separate registries, and until 2026-09-07
    /// `RETIRED_NODE_TYPES` was empty — so the node loop's retired branch was
    /// unreachable and had never been executed by any test.
    ///
    /// Cutting a type from `schema/*.yaml` without registering it here makes the
    /// guard read every ORDINARY graph as one from the FUTURE, so the refusal
    /// tells the operator their reflow2 is BEHIND and to rebuild — the exact
    /// opposite of what to do, and they will do it, because the message is
    /// confident and specific.
    ///
    /// Observed on 2026-09-07 while retiring `QualityGate`: the schema edit
    /// landed first, the live graph stopped opening, and the message sent the
    /// operator to rebuild a binary that was already current. This test is the
    /// standing reminder that the registry entry is not bookkeeping.
    #[test]
    fn a_retired_type_says_migrate_the_graph_rather_than_update_the_binary() {
        // A graph written before the retirement: it names the type, holds no
        // instances of it, and is otherwise identical to the current schema.
        let before = named("0.54.0", &["Project", "QualityGate"], &["SATISFIES"]);
        let now = named("0.54.0", &["Project"], &["SATISFIES"]);

        let refusal =
            refusal_of(&before, &now).expect("a graph naming a type this binary lacks is refused");

        assert!(
            refusal.contains("RETIRED") && refusal.contains("migrate the graph"),
            "a RETIRED type must send the operator to the migration, not to a rebuild: {refusal}"
        );
        assert!(
            !refusal.contains("BEHIND"),
            "the binary is current; telling the operator it is behind is the failure this \
             registry exists to prevent: {refusal}"
        );
    }

    #[test]
    fn the_stamp_sits_beside_the_graph_not_inside_it() {
        assert_eq!(
            stamp_path("/p/.reflow2/graph"),
            PathBuf::from("/p/.reflow2/graph.meta.json")
        );
    }

    #[test]
    fn a_graph_that_knows_more_by_count_is_the_only_refusal() {
        let old = legacy("0.1.0", 26, 52);
        let new = legacy("0.2.0", 27, 53);
        assert!(new.knows_more_than(&old));
        assert!(
            !old.knows_more_than(&new),
            "an older graph is additive and entirely readable"
        );
        assert!(!new.knows_more_than(&new));
    }

    #[test]
    fn set_based_subset_opens() {
        // The graph names a strict subset of the binary's types — additive.
        let graph = named("0.9.0", &["Requirement", "Capability"], &["SATISFIES"]);
        let now = named(
            "0.10.0",
            &["Requirement", "Capability", "Release"],
            &["SATISFIES", "INCLUDES"],
        );
        assert!(refusal_of(&graph, &now).is_none());
    }

    #[test]
    fn set_based_retired_type_says_migrate_not_behind() {
        // The graph uses VALIDATES, which this reflow2 retired: it is not from
        // the future — migrate the graph.
        let graph = named("0.9.0", &["Capability"], &["SATISFIES", "VALIDATES"]);
        let now = named("0.10.0", &["Capability"], &["SATISFIES"]);
        let msg = refusal_of(&graph, &now).expect("must refuse");
        assert!(msg.contains("VALIDATES"), "names the retired type: {msg}");
        assert!(msg.contains("RETIRED") && msg.to_lowercase().contains("migrate"));
        assert!(
            !msg.contains("BEHIND"),
            "a purely-retired case must not tell the user to update: {msg}"
        );
    }

    #[test]
    fn set_based_unknown_type_says_behind() {
        // The graph uses a type this reflow2 has never heard of: the binary is
        // behind — update it.
        let graph = named("0.11.0", &["Capability"], &["SATISFIES", "FUTURE_EDGE"]);
        let now = named("0.10.0", &["Capability"], &["SATISFIES"]);
        let msg = refusal_of(&graph, &now).expect("must refuse");
        assert!(
            msg.contains("FUTURE_EDGE") && msg.contains("BEHIND"),
            "{msg}"
        );
    }

    #[test]
    fn set_based_mixed_names_both_paths() {
        let graph = named("0.11.0", &["Capability"], &["VALIDATES", "FUTURE_EDGE"]);
        let now = named("0.10.0", &["Capability"], &["SATISFIES"]);
        let msg = refusal_of(&graph, &now).expect("must refuse");
        assert!(
            msg.contains("VALIDATES") && msg.contains("FUTURE_EDGE"),
            "{msg}"
        );
        assert!(msg.contains("BEHIND") && msg.to_lowercase().contains("migrate"));
    }

    #[test]
    fn legacy_count_excess_matching_retired_leads_with_migrate() {
        // The count-only case that motivated BL-86: 2 extra edge types, exactly
        // the number this reflow2 retired.
        let graph = legacy("0.9.0", 3, 5);
        let now = named("0.10.0", &["A", "B", "C"], &["X", "Y", "Z"]); // 3 / 3
        let msg = refusal_of(&graph, &now).expect("must refuse");
        assert!(
            msg.contains("VALIDATES") && msg.to_lowercase().contains("most likely"),
            "excess explained by the retired types → lead with migration: {msg}"
        );
    }

    #[test]
    fn legacy_count_excess_beyond_retired_stays_hedged() {
        let graph = legacy("0.9.0", 3, 8); // 5 extra edge types — more than retired
        let now = named("0.10.0", &["A", "B", "C"], &["X", "Y", "Z"]);
        let msg = refusal_of(&graph, &now).expect("must refuse");
        assert!(
            msg.contains("cannot tell them apart"),
            "an unexplained excess keeps the honest hedge: {msg}"
        );
    }

    #[test]
    fn a_legacy_stamp_still_deserializes_without_the_name_fields() {
        let old =
            r#"{"reflow2_version":"0.9.0","schema_version":1,"node_types":28,"edge_types":55}"#;
        let s: GraphStamp = serde_json::from_str(old).expect("legacy stamp parses");
        assert_eq!(s.node_type_names, None);
        assert_eq!(s.edge_types, 55);
    }
}
