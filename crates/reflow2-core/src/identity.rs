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

// ---------------------------------------------------------------------------
// Seat identity — who is *working*, as opposed to what is being worked on.
// ---------------------------------------------------------------------------

/// This session's name, minted once per process.
///
/// `req:claims-have-owners`. A claim that does not say who made it cannot be
/// told from a claim nobody is working any more, and a ghost claim makes the
/// overlap report lie — which is worse than no report, because people act on it.
///
/// Same doctrine as the design's own name (`dec:identity-out-of-band`): nothing
/// shared is read, so nothing can race at one seat or fifteen. The shape is
/// `<machine>:<pid>:<mint>`, and it is chosen to make **liveness computable**
/// rather than asserted — a later reader can ask the operating system whether
/// that process still exists instead of trusting a flag somebody set.
pub fn seat_id() -> String {
    static SEAT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SEAT.get_or_init(mint_seat).clone()
}

/// A fresh seat, not the process-wide one.
///
/// `req:seat-per-client`. One server can hold many client sessions
/// (`req:sessions-share-a-graph`), and the process-wide seat is exactly wrong
/// there: every client would report the same owner, so every claim would name
/// the same seat and the overlap report would tell six sessions they are each
/// other. A session mints its own on connect.
///
/// **Honest limit, because it is easy to misread.** The seat carries a pid, so
/// liveness answers "is the process that made this claim still running". Under
/// one server that is the right answer about *the server*, and only a proxy for
/// the session: a client that disconnects while the server lives still reads
/// `live`. Per-session liveness needs the server's own session registry, which
/// the core cannot see — recorded rather than papered over.
pub fn mint_seat() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // The address of a stack local disambiguates two mints in the same
    // nanosecond within one process — which is exactly the case a shared server
    // creates when two sessions connect at once.
    let here = &nanos as *const u128 as usize;
    let mint = format!(
        "{:08x}",
        crate::nodes::fnv1a(&format!("{nanos}|{here:x}")) & 0xffff_ffff
    );
    format!("{}:{}:{mint}", machine(), std::process::id())
}

/// This machine's name, for telling "their session died" from "their session is
/// on a different computer, and I cannot see it from here".
///
/// Public so tests can build a seat this machine will recognise, rather than
/// reimplementing the lookup and disagreeing with it somewhere subtle.
///
/// Best effort by design, and honest when it fails: an unknown machine makes
/// every foreign claim report as `Unknown` rather than as alive or dead, which
/// is the only truthful answer available.
pub fn machine() -> String {
    // Each source is trimmed and emptiness-checked BEFORE falling through, so
    // an empty HOSTNAME (set but blank, which happens in stripped environments)
    // still reaches /etc/hostname instead of short-circuiting to unknown.
    let non_empty = |s: String| Some(s.trim().to_string()).filter(|h| !h.is_empty());
    std::env::var("HOSTNAME")
        .ok()
        .and_then(non_empty)
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .and_then(non_empty)
        })
        .unwrap_or_else(|| "unknown-machine".to_string())
}

/// Is the session that made a claim still running?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// The process is still there. Somebody is probably working this.
    Live,
    /// The session that made this claim has exited. The claim is a ghost —
    /// still worth reading for what it says, never worth treating as held.
    Gone,
    /// Made on another machine, by a seat with no name, or on a machine that
    /// could not identify itself. Reported as unknown rather than guessed:
    /// calling a foreign claim dead would invite somebody to take work that is
    /// actively being done.
    Unknown,
}

/// Ask the operating system whether a seat is still running.
///
/// Computed, not remembered — nothing writes "I am alive" anywhere, so nothing
/// can be stale. Cross-machine is deliberately `Unknown`: a pid means nothing
/// on a computer that is not the one that minted it.
pub fn seat_liveness(seat: &str) -> Liveness {
    let parts: Vec<&str> = seat.split(':').collect();
    let [host, pid, ..] = parts.as_slice() else {
        return Liveness::Unknown;
    };
    if *host != machine() || *host == "unknown-machine" {
        return Liveness::Unknown;
    }
    let Ok(pid) = pid.parse::<u32>() else {
        return Liveness::Unknown;
    };
    if std::path::Path::new(&format!("/proc/{pid}")).exists() {
        return Liveness::Live;
    }
    // No /proc (macOS): ask ps. One spawn per distinct seat, on a report path
    // that runs when a person asks, never in a loop.
    if std::path::Path::new("/proc").exists() {
        // /proc exists but this pid is not in it: the process is genuinely gone.
        return Liveness::Gone;
    }
    match std::process::Command::new("ps")
        .args(["-p", &pid.to_string()])
        .output()
    {
        Ok(out) if out.status.success() => Liveness::Live,
        Ok(_) => Liveness::Gone,
        Err(_) => Liveness::Unknown,
    }
}

/// Take the identity of a design being imported into an empty store.
///
/// The case: `--import` (or `import_graph`) into a fresh graph is a *restore* —
/// same design, new store — and reflow2 says elsewhere that a copy of a design
/// **is** that design. If the empty graph kept the id it minted at open, the
/// round trip would not come back byte-identical, because `graph_id` is part of
/// the export's content hash. The project's own smoke test caught exactly that
/// the hour identity landed.
///
/// A graph that already holds a design keeps its own name, always. That is the
/// other half of the same rule, and it is what makes the stale-seat remedy safe:
/// absorbing the shared record into a working graph must never rename it.
///
/// Returns the adopted identity when it took, `None` when the graph kept its
/// own. **Call before importing** — the import writes under the current id.
pub fn adopt_on_import(
    graph_path: &str,
    document_graph_id: &str,
    holds_a_design: bool,
) -> Result<Option<DesignIdentity>, DynoError> {
    if holds_a_design || document_graph_id.is_empty() {
        return Ok(None);
    }
    let mut identity = resolve(graph_path, document_graph_id, || false)?;
    if identity.graph_id == document_graph_id {
        return Ok(None);
    }
    identity.graph_id = document_graph_id.to_string();
    identity.origin = Origin::Adopted;
    write(graph_path, &identity)?;
    Ok(Some(identity))
}
