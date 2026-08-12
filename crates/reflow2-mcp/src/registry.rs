//! A session names the design it wants by `graph_id`, and the server maps id to
//! path.
//!
//! `cap:select-graph-by-id`, and the second half of `req:a-session-chooses-its-design`
//! (accepted). The first half — finding out what designs exist WITHOUT opening
//! them — already shipped as `describe_designs`; this is the half that lets a
//! session be pointed at one.
//!
//! # Why an id and not a path
//!
//! `rule:a-design-is-named-by-an-id-not-a-path`, Anthony 2026-08-09: *"the id is
//! primary and the path is a storage detail. A surface that treats a filesystem
//! path as the canonical identity forecloses object storage, where the location
//! is a key and the identity rides as metadata."*
//!
//! That rule is ADVISORY, and it says exactly why: *"this clause has no
//! compliant surface at all… an honest detector would fail against the entire
//! existing tool surface on the day it was written."* It also names its own
//! trigger — *"cap:select-graph-by-id becoming realized"* — which is this
//! module. The rule can flip to enforced once a NEW surface has somewhere
//! compliant to be.
//!
//! # The two clauses this module is accountable for
//!
//! `ver:a-session-cannot-name-another-design`, and the conditions
//! `dec:one-process-many-stores` was accepted on:
//!
//! 1. **an id a session was not attached to is REFUSED rather than served**
//! 2. **a path is not an alternative route in**
//!
//! Clause 2 is why [`Registry::attach`] takes `&str` and treats it as an ID
//! ALWAYS — a path handed to it is an unknown id, never a location to open.
//! There is deliberately no `path_for(id)` and no `attach_path`: a convenience
//! overload is exactly how this property would be lost, so it is absent rather
//! than documented.
//!
//! Clause 1 is why the registry's ROOT is the boundary. A design that genuinely
//! exists elsewhere on the machine is refused identically to one that was made
//! up — knowing a real `graph_id` is not a way in.
//!
//! # A binding is the capability
//!
//! [`Registry::attach`] returns a [`Binding`], and a `Binding` exposes exactly
//! one design. There is no operation on it that takes another id, so "a session
//! cannot name another design" holds BY CONSTRUCTION rather than by a check
//! somebody must remember to keep.
//!
//! # What this deliberately does not settle
//!
//! **Who may see which designs.** [`Registry::graph_ids`] lists what the
//! OPERATOR placed under the root. That is right for one owner's own
//! neighbourhood — Anthony, 2026-08-05: *"can a reflow2 tool be to simply return
//! all graph_ids to the agent and the agent then can choose whichever graph_id
//! the user specifies"* — and it is NOT a multi-tenant policy.
//! `dec:idea-a-session-holds-several-graphs` records the distinction that
//! decides it, *"multi-tenant isolation is not the same as composition"*, and it
//! is unanswered. Nothing here forecloses either answer: a registry per tenant,
//! or a filtered listing, both remain available because the root is a parameter.
//!
//! **The transport.** `cap:select-graph-by-id` prefers an HTTP path prefix
//! (`/g/<graph_id>/`) so selection is visible in logs and routable by ordinary
//! proxies, with an MCP initialize parameter second and an attaching tool call
//! worst — *"it makes every session stateful in a way a reconnect silently
//! loses"*. This module is the resolution half only; no transport is wired to it
//! yet, and the single-`--graph-path` server is untouched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Why an attach was refused.
///
/// One variant today, and it is deliberately the only one: an id this registry
/// does not hold. A path, a stranger's real id and a typo are all the same
/// refusal, because distinguishing them would leak what exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachError {
    /// This registry holds no design under that id.
    ///
    /// The requested id is echoed because a refusal that does not say what it
    /// refused is a wall — but nothing about what DOES exist is disclosed here.
    UnknownGraphId { requested: String },
}

impl std::fmt::Display for AttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttachError::UnknownGraphId { requested } => write!(
                f,
                "no design named `{requested}` is registered here. A session names a design by \
                 its graph_id, and a filesystem path is not an alternative route in — list what \
                 this server holds and name one of those."
            ),
        }
    }
}

impl std::error::Error for AttachError {}

/// One session's attachment to one design.
///
/// Holds the resolved store path so the server can open it, and offers NO way to
/// name a second design. That absence is the isolation property, not an
/// oversight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    graph_id: String,
    graph_path: String,
}

impl Binding {
    /// The design this session is attached to.
    pub fn graph_id(&self) -> &str {
        &self.graph_id
    }

    /// Where that design's store lives — for the server that opens it, never
    /// for a client. The path is the storage detail the id exists to hide.
    pub fn graph_path(&self) -> &str {
        &self.graph_path
    }
}

/// The designs one server offers, by id.
#[derive(Debug, Clone)]
pub struct Registry {
    root: String,
    by_id: BTreeMap<String, String>,
}

impl Registry {
    /// Read every design directly under `root`, WITHOUT opening any store.
    ///
    /// Identity comes from the sidecar files beside each store, which exist to
    /// be read before opening — so discovery takes no lock, writes nothing, and
    /// a design another session holds right now enumerates fine.
    ///
    /// A directory that has opted in but carries no identity is NOT registered.
    /// Naming it would mean opening it, which MINTS an identity and thereby
    /// answers its own question — the failure `describe_designs` exists to
    /// avoid, and the one that once minted a third graph beside two populated
    /// ones.
    pub fn discover(root: &str) -> Self {
        let mut by_id = BTreeMap::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            dirs.sort();
            for dir in dirs {
                let store = dir.join(".reflow2").join("graph");
                let Some(store) = store.to_str() else {
                    continue;
                };
                let found = reflow2_core::describe_at(store);
                // TWO GUARDS, AND ONLY THE SECOND IS LOAD-BEARING TODAY —
                // measured by mutation on 2026-08-12, where removing the state
                // check changed nothing. `describe_at` sets `graph_id: Some` on
                // exactly one path, the one that reports `Design`; every other
                // state sets `None`. The state check is kept as the statement of
                // intent and as a defence if that ever stops being true, but a
                // reader should know it is belt to the `Some(id)` braces rather
                // than the thing doing the work.
                if found.state == reflow2_core::DesignPathState::Design
                    && let Some(id) = found.graph_id
                {
                    by_id.insert(id, store.to_string());
                }
            }
        }
        Registry {
            root: root.to_string(),
            by_id,
        }
    }

    /// The root this registry was built over — echoed so an empty answer reads
    /// as "nothing under here" rather than as "nobody looked".
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Every design a session may name, by id.
    ///
    /// This is the operator's offer, not a claim about the machine: designs
    /// outside the root are invisible here and unreachable through
    /// [`Registry::attach`].
    pub fn graph_ids(&self) -> Vec<String> {
        self.by_id.keys().cloned().collect()
    }

    /// Attach a session to the design named by `graph_id`.
    ///
    /// **The argument is always an ID.** A filesystem path handed here is an
    /// unknown id and is refused as one; there is no path route in, by absence
    /// rather than by a check.
    pub fn attach(&self, graph_id: &str) -> Result<Binding, AttachError> {
        match self.by_id.get(graph_id) {
            Some(path) => Ok(Binding {
                graph_id: graph_id.to_string(),
                graph_path: path.clone(),
            }),
            None => Err(AttachError::UnknownGraphId {
                requested: graph_id.to_string(),
            }),
        }
    }

    /// How many designs are registered. Stated so a caller can tell an empty
    /// registry from one it never built.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry offers nothing.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// The conventional store path under a project directory.
pub fn store_path_under(project_dir: &Path) -> PathBuf {
    project_dir.join(".reflow2").join("graph")
}
