//! A design knows its own name, and remembers it across opens.
//!
//! `req:design-identity`, governed by `dec:identity-out-of-band` — *names are
//! assigned with zero coordination, never derived from shared state.*
//!
//! Until now every reflow2 graph answered to the same hardcoded id, so **no
//! design could tell another design from itself**. `mirror_surface` has to
//! refuse a surface whose source is the importing graph (a filtered copy of
//! your own design would overwrite the full one), and with one constant that
//! check could never pass for anybody. Composition between designs was
//! meaningless: they all had the same name.
//!
//! ## Why the id lives beside the store and not in it
//!
//! **The graph id namespaces every stored key.** Reading anything requires
//! already knowing it, so it cannot be a node inside the design — that is a
//! chicken-and-egg, and getting it wrong is silent: a graph reopened under a
//! name it was not created with finds nothing and presents as an *empty
//! design*. So identity sits in a sibling file, exactly where the version stamp
//! already sits, and is read before the design is.
//!
//! Its own file rather than a field in `<graph>.meta.json`, for the same reason
//! the sync marker got one: `check_and_stamp` rewrites that file wholesale on
//! every open, and changing its shape would make every existing graph fail to
//! open with "the version stamp is not readable".
//!
//! ## The migration is the dangerous part, so it is the explicit part
//!
//! Every graph that exists today holds its design under the old default id.
//! Minting a fresh id for those would be a catastrophe of exactly the silent
//! kind above — the design would still be on disk, and reflow2 would open a new
//! empty one beside it and report nothing wrong. So a graph that **already has
//! design data under the default id adopts that id** as its identity, forever.
//! Only a graph with nothing under it mints.
//!
//! One consequence worth stating: `graph_id` is part of the export's content
//! hash, so adoption is also what keeps every existing export, chain link and
//! committed record valid across this change.

use std::path::{Path, PathBuf};

use dynograph_core::DynoError;
use serde::{Deserialize, Serialize};

/// How a design came by its name — recorded because the two cases have very
/// different consequences, and a later reader should not have to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Minted for a graph that had no design in it yet.
    Minted,
    /// Kept from the era when every graph shared one id, because this graph
    /// already held a design under it.
    Adopted,
}

/// What this design is called, and what it was called by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignIdentity {
    /// The storage-scoping id. Stable for the life of the design, including
    /// across machines and copies — a copy of a design *is* that design.
    pub graph_id: String,
    /// The human-facing name. A label on top of the id, changeable at will,
    /// and never load-bearing: two designs may share a label and still be
    /// distinct.
    pub label: String,
    /// Minted or adopted.
    pub origin: Origin,
    /// Which reflow2 wrote this record.
    pub minted_by: String,
}

/// `<graph-path>.id.json` — a sibling of the store, like the version stamp.
pub fn identity_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    match p.file_name().map(|n| n.to_string_lossy().to_string()) {
        Some(n) => p.with_file_name(format!("{n}.id.json")),
        None => PathBuf::from(format!("{graph_path}.id.json")),
    }
}

/// A name assigned with zero coordination — nothing shared is read, so nothing
/// can race, at one seat or a thousand (`dec:identity-out-of-band`).
///
/// Deliberately not a UUID crate: the inputs already make it unique by
/// construction — the nanosecond it was created, the process that created it,
/// and where it lives — and a dependency added for sixteen hex characters is a
/// dependency every consumer pays a rebuild for.
fn mint(graph_path: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let absolute = std::fs::canonicalize(graph_path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| graph_path.to_string());
    let seed = format!("{nanos}|{}|{absolute}", std::process::id());
    format!("{:016x}", crate::nodes::fnv1a(&seed))
}

/// A friendly default: the project directory's name, not the store's.
///
/// `<project>/.reflow2/graph` should read as "project", which is what a person
/// would call it — the two path segments below it are reflow2's plumbing.
fn default_label(graph_path: &str) -> String {
    let p = std::fs::canonicalize(graph_path).unwrap_or_else(|_| PathBuf::from(graph_path));
    let mut cursor = p.as_path();
    while let Some(name) = cursor.file_name().and_then(|n| n.to_str()) {
        if name != "graph" && name != ".reflow2" {
            return name.to_string();
        }
        match cursor.parent() {
            Some(parent) => cursor = parent,
            None => break,
        }
    }
    "design".to_string()
}

/// Read this design's identity, establishing it on first open.
///
/// `holds_default_design` is asked only when there is no identity file yet, and
/// answers the migration question: does this store already contain a design
/// under the old shared id? If it does, that id is adopted rather than
/// replaced — see the module docs for why the alternative is silent data loss.
pub fn resolve(
    graph_path: &str,
    default_id: &str,
    holds_default_design: impl FnOnce() -> bool,
) -> Result<DesignIdentity, DynoError> {
    let path = identity_path(graph_path);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            // Refused rather than defaulted: an unreadable identity file is the
            // one thing we must not paper over, because "default it" means
            // opening a different design under the same path and finding it
            // empty (req:design-identity).
            return serde_json::from_str(&text).map_err(|e| {
                DynoError::Serialization(format!(
                    "the design identity at {} is not readable ({e}). It records which design \
                     this store holds, and reflow2 will not guess: fix the file, or move it aside \
                     to have a new identity established.",
                    path.display()
                ))
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(DynoError::Storage(format!(
                "cannot read the design identity at {}: {e}",
                path.display()
            )));
        }
    }

    let identity = if holds_default_design() {
        DesignIdentity {
            graph_id: default_id.to_string(),
            label: default_label(graph_path),
            origin: Origin::Adopted,
            minted_by: env!("CARGO_PKG_VERSION").to_string(),
        }
    } else {
        DesignIdentity {
            graph_id: mint(graph_path),
            label: default_label(graph_path),
            origin: Origin::Minted,
            minted_by: env!("CARGO_PKG_VERSION").to_string(),
        }
    };
    write(graph_path, &identity)?;
    Ok(identity)
}

/// Persist an identity beside the store.
pub fn write(graph_path: &str, identity: &DesignIdentity) -> Result<(), DynoError> {
    let path = identity_path(graph_path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(identity).map_err(|e| {
        DynoError::Serialization(format!("cannot serialize the design identity: {e}"))
    })?;
    std::fs::write(&path, json + "\n").map_err(|e| {
        DynoError::Storage(format!(
            "cannot write the design identity at {}: {e}",
            path.display()
        ))
    })
}

/// Rename the design. The label is a label: the id never moves, because
/// everything stored is keyed by it and every export ever written names it.
pub fn set_label(graph_path: &str, label: &str) -> Result<DesignIdentity, DynoError> {
    let path = identity_path(graph_path);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        DynoError::Storage(format!(
            "no design identity at {} to rename ({e}) — open the graph once to establish it.",
            path.display()
        ))
    })?;
    let mut identity: DesignIdentity = serde_json::from_str(&text).map_err(|e| {
        DynoError::Serialization(format!("the design identity is not readable: {e}"))
    })?;
    identity.label = label.to_string();
    write(graph_path, &identity)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidecar_sits_beside_the_store() {
        assert_eq!(
            identity_path("/p/.reflow2/graph"),
            PathBuf::from("/p/.reflow2/graph.id.json")
        );
    }

    #[test]
    fn two_designs_minted_at_once_do_not_collide() {
        // Unique by construction: same nanosecond is possible, same path is not.
        let a = mint("/tmp/one");
        let b = mint("/tmp/two");
        assert_ne!(a, b);
        assert_eq!(a.len(), 16, "a readable, fixed-width id: {a}");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
