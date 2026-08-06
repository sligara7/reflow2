//! DISCOVER — say what design lives at a path, without binding to it
//! (`cap:describe-design-at-path`, `dec:describe-a-design-without-binding-to-it`).
//!
//! The half of discovery only reflow2 can answer. The AGENT walks the tree and
//! finds `.reflow2` directories — that is file navigation and the ambient agent
//! does it well (`dec:agent-navigates-content`). This answers the other
//! question: *given a path, what design is that?*
//!
//! ## Why it exists
//!
//! Measured on a real repo, 2026-08-05: a session opened at the root of
//! `hxm_program` was told "this directory has no design yet" and minted a THIRD
//! graph, while two populated designs sat one and two directories down. Nothing
//! could say what they were without opening a session against each. One listing
//! would have prevented it.
//!
//! ## ⭐ It never opens the store, and that is the load-bearing property
//!
//! The obvious implementation — open the graph and count its nodes — is WRONG,
//! and not for performance. [`DesignGraph::open_rocksdb_with_provenance`] has
//! two side effects on the way in: `provenance::check_and_stamp` WRITES
//! `<path>.meta.json`, and `identity::resolve` MINTS an identity file when the
//! store has none. **A describe that opened the store could therefore create
//! state in a design it was only meant to look at** — and on a store with no
//! identity yet, inspecting it would name it.
//!
//! It would also take RocksDB's exclusive lock, so describing a design another
//! session is holding would fail — and a failure that reads as "no design here"
//! is precisely the defect this exists to prevent.
//!
//! So everything here is read from the two sidecar files that already sit
//! beside the store — `<path>.id.json` and `<path>.meta.json` — plus whether
//! the store directory is there at all. That makes this **lock-free,
//! side-effect-free, and safe against a design that is being written right
//! now**, which no store-opening version could be.
//!
//! ## What that costs, stated rather than hidden
//!
//! **No node counts.** `dec:describe-a-design-without-binding-to-it` said the
//! report would carry "node counts by type"; it cannot, because counting means
//! opening. Identity, origin and the mint stamp answer *which design is this and
//! how old is its schema* — which is what choosing between designs actually
//! needs. Size would be nice and is not worth minting an identity to get.

use std::path::Path;

use crate::identity::{DesignIdentity, identity_path};
use crate::provenance::{GraphStamp, stamp_path};

/// What is at a path, as far as can be told without opening anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignPathState {
    /// An identity file is there: this path holds a design and it can be named.
    Design,
    /// A store directory is there with no identity beside it. Something is
    /// here, but saying WHICH design would mean opening it — which would mint
    /// the identity and thereby answer its own question. Reported as unnamed
    /// rather than opened.
    Unnamed,
    /// The `.reflow2` directory exists but holds no store and no identity — a
    /// project that has opted in and not yet written anything.
    OptedIn,
    /// Nothing here.
    Absent,
}

/// One path, described.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DesignAtPath {
    /// The graph path asked about, echoed so a caller sweeping many paths need
    /// not track the correspondence.
    pub path: String,
    /// What is there.
    pub state: DesignPathState,
    /// The design's stable id — the thing that survives copies and renames.
    pub graph_id: Option<String>,
    /// Its human-facing label. Never load-bearing: two designs may share one.
    pub label: Option<String>,
    /// `minted` or `adopted`.
    pub origin: Option<String>,
    /// Which reflow2 established the identity.
    pub minted_by: Option<String>,
    /// The schema stamp beside the store — which reflow2 last wrote it and how
    /// many types that schema had. `None` when the store has never been opened
    /// by a version that stamps.
    pub stamp: Option<GraphStamp>,
    /// What a reader should conclude, in a sentence. Present on every state,
    /// because a bare enum makes the caller invent the meaning.
    pub reading: String,
}

/// Describe the design at `graph_path` without opening or writing anything.
///
/// `graph_path` is the store path — the same value `--graph-path` takes, e.g.
/// `/repo/.reflow2/graph`. Never fails: an unreadable or malformed identity is
/// reported as [`DesignPathState::Unnamed`] with the reason in `reading`,
/// because a caller sweeping a tree wants an answer per path rather than one
/// bad path aborting the sweep.
pub fn describe_at(graph_path: &str) -> DesignAtPath {
    let store_exists = Path::new(graph_path).exists();
    let identity = read_identity(graph_path);
    let stamp = read_stamp(graph_path);

    match identity {
        Some(Ok(id)) => DesignAtPath {
            path: graph_path.to_string(),
            state: DesignPathState::Design,
            reading: format!(
                "holds the design '{}' ({}){}",
                id.label,
                id.graph_id,
                if store_exists {
                    ""
                } else {
                    " — identity recorded, but the store itself is not here"
                }
            ),
            graph_id: Some(id.graph_id),
            label: Some(id.label),
            origin: Some(format!("{:?}", id.origin).to_lowercase()),
            minted_by: Some(id.minted_by),
            stamp,
        },
        // Present and broken. Named as such rather than treated as absent —
        // "no design here" is the sentence that mints an unwanted graph.
        Some(Err(why)) => DesignAtPath {
            path: graph_path.to_string(),
            state: DesignPathState::Unnamed,
            graph_id: None,
            label: None,
            origin: None,
            minted_by: None,
            stamp,
            reading: format!(
                "something is here but its identity file cannot be read ({why}). \
                 Do NOT start a new design at this path until that is resolved."
            ),
        },
        None if store_exists => DesignAtPath {
            path: graph_path.to_string(),
            state: DesignPathState::Unnamed,
            graph_id: None,
            label: None,
            origin: None,
            minted_by: None,
            stamp,
            reading: "a store is here with no identity beside it. Which design it holds cannot \
                      be answered without opening it, and opening would MINT an identity — so \
                      it is reported unnamed rather than named by the act of asking."
                .to_string(),
        },
        None => {
            let opted_in = Path::new(graph_path)
                .parent()
                .is_some_and(|d| !d.as_os_str().is_empty() && d.exists());
            DesignAtPath {
                path: graph_path.to_string(),
                state: if opted_in {
                    DesignPathState::OptedIn
                } else {
                    DesignPathState::Absent
                },
                graph_id: None,
                label: None,
                origin: None,
                minted_by: None,
                stamp,
                reading: if opted_in {
                    "opted in but empty: the .reflow2 directory exists and nothing has been \
                     written yet. Starting a design here is expected."
                        .to_string()
                } else {
                    "no design here.".to_string()
                },
            }
        }
    }
}

/// `Some(Ok)` when the identity file parses, `Some(Err)` when it is there and
/// broken, `None` when it is absent. The three cases are deliberately distinct:
/// absent and unreadable must never collapse into one answer.
fn read_identity(graph_path: &str) -> Option<Result<DesignIdentity, String>> {
    let path = identity_path(graph_path);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(serde_json::from_str::<DesignIdentity>(&text).map_err(|e| e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(Err(e.to_string())),
    }
}

/// The schema stamp, when one is there and readable. Absent is ordinary — a
/// store written before stamping, or never opened — so this never reports why.
fn read_stamp(graph_path: &str) -> Option<GraphStamp> {
    let text = std::fs::read_to_string(stamp_path(graph_path)).ok()?;
    serde_json::from_str(&text).ok()
}
