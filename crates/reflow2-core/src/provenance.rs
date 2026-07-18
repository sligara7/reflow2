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

use std::path::{Path, PathBuf};

use dynograph_core::{DynoError, Schema};
use serde::{Deserialize, Serialize};

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
}

impl GraphStamp {
    /// The stamp this binary would write.
    pub fn current(schema: &Schema) -> Self {
        Self {
            reflow2_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: schema.version,
            node_types: schema.node_types.len(),
            edge_types: schema.edge_types.len(),
        }
    }

    /// True when the recorded schema knew more than `other` does — the case
    /// that cannot be read safely.
    fn knows_more_than(&self, other: &Self) -> bool {
        self.node_types > other.node_types || self.edge_types > other.edge_types
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

/// Read the stamp beside a graph, compare it to this binary, and refresh it.
///
/// Fails loud — and refuses to open — only when the graph was written by a
/// reflow2 that knew *more* types than this one. Reading such a graph would
/// silently show less than it holds, which is the failure this whole check
/// exists to prevent; every other difference is reported and opened.
///
/// A stamp that cannot be parsed is reported as an error rather than
/// overwritten: it may be the only record of what wrote the graph.
pub fn check_and_stamp(graph_path: &str, schema: &Schema) -> Result<Provenance, DynoError> {
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
        Some(was) if was.knows_more_than(&now) => {
            return Err(DynoError::Storage(format!(
                "this graph was written by reflow2 {} ({} node types, {} edge types), which \
                 knows more of the schema than the reflow2 you are running ({}: {}, {}). \
                 Opening it would silently show you less of your design than it holds.\n\
                 Rebuild from a current checkout:  cargo build -p reflow2-mcp --release",
                was.reflow2_version,
                was.node_types,
                was.edge_types,
                now.reflow2_version,
                now.node_types,
                now.edge_types
            )));
        }
        Some(was) => Provenance::OlderGraph {
            was,
            now: now.clone(),
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
    use super::*;

    fn stamp(v: &str, n: usize, e: usize) -> GraphStamp {
        GraphStamp {
            reflow2_version: v.into(),
            schema_version: 1,
            node_types: n,
            edge_types: e,
        }
    }

    #[test]
    fn the_stamp_sits_beside_the_graph_not_inside_it() {
        assert_eq!(
            stamp_path("/p/.reflow2/graph"),
            PathBuf::from("/p/.reflow2/graph.meta.json")
        );
    }

    #[test]
    fn a_graph_that_knows_more_is_the_only_refusal() {
        let old = stamp("0.1.0", 26, 52);
        let new = stamp("0.2.0", 27, 53);
        assert!(
            new.knows_more_than(&old),
            "a graph from the future cannot be read in full"
        );
        assert!(
            !old.knows_more_than(&new),
            "an older graph is additive and entirely readable — refusing would lock \
             someone out of their own design over a change that cannot hurt them"
        );
        assert!(!new.knows_more_than(&new));
    }
}
