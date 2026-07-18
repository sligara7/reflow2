//! P4 · Verification — the write side of the verify domain (WS-1).
//!
//! [`detect.rs`](crate::detect) raises two gaps that ask the user for a
//! `Verification`: `build_without_verification` ("you've built things but
//! nothing checks them") and `unverified_capability` ("this realized capability
//! has no check"). Until now neither could be answered with a typed call —
//! `Verification` was counted and reported but never constructible, so the gap
//! could be raised and not closed.
//!
//! A `Verification` is deliberately broad: a unit test, a design review, a
//! simulation, a physical inspection, a measurement. `method` and `level` carry
//! that distinction rather than the type name, so a hardware inspection and a
//! `cargo test` run are the same kind of node with different properties — which
//! is what lets the same coverage gap work across domains.

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

impl DesignGraph {
    /// P4 · Verification — a check that something meets its intent. `name` is
    /// required; `method` (default `test`), `level` (default `unit`),
    /// `location` and `status` (default `planned`) are optional.
    ///
    /// `status` is what makes a Verification more than an inventory entry: a
    /// `failing` check on a realized Capability is a live signal, not a record.
    pub fn add_verification(
        &mut self,
        id: &str,
        name: &str,
        method: Option<&str>,
        level: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::VERIFICATION,
            id,
            Props::new()
                .set("name", name)
                .set_opt("method", method)
                .set_opt("level", level),
        )
    }

    /// Set a `Verification`'s outcome, preserving its other properties.
    /// `status` ∈ `planned` / `passing` / `failing` / `skipped` / `blocked`.
    ///
    /// Kept separate from creation because the outcome changes far more often
    /// than the check itself, and a re-run should not have to restate what the
    /// check *is*.
    pub fn set_verification_status(
        &mut self,
        verification_id: &str,
        status: &str,
        last_run_at: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::VERIFICATION, verification_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::VERIFICATION.to_string(),
                node_id: verification_id.to_string(),
            });
        };
        let mut props = Props::new()
            .set("status", status)
            .set_opt("last_run_at", last_run_at);
        for (k, v) in &existing.properties {
            if k != "status" && k != "last_run_at" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::VERIFICATION, verification_id, props)
    }

    /// `Verification VERIFIES target` — the check and the thing it checks.
    /// `target_type` is required because the schema allows any target.
    ///
    /// PROPAGATE reads this edge as Upstream from the Verification, so a failing
    /// check reaches the Capability it covers and the Requirement behind it.
    pub fn verifies(
        &mut self,
        verification_id: &str,
        target_type: &str,
        target_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::VERIFIES,
            node::VERIFICATION,
            verification_id,
            target_type,
            target_id,
            Props::new(),
        )
    }
}
