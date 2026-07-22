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

/// How a capability's claim to work is checked — three-valued on purpose
/// (BL-73, from the first extensive field trial). A brownfield adopt with a
/// real per-service test suite read as "0/20 capabilities verified": the
/// suites were registered against *components*, and nothing on the read side
/// knew what that meant for the capabilities allocated to them. "Verified at
/// component granularity" is neither "verified" nor "unverified" — collapsing
/// it into either understates a tested system or overstates a wholesale claim
/// (`dec:component-verified-computed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityVerification {
    /// A passing `Verification` checks this capability itself.
    Verified,
    /// No passing check of its own, but a component it is allocated to
    /// carries one — the capability rides its component's suite. Derived,
    /// never written: the graph records exactly what was checked (the
    /// component), and this state is what that fact means one hop away.
    ComponentVerified,
    /// No passing check anywhere in sight.
    Unchecked,
}

impl DesignGraph {
    /// Whether a node has at least one incoming `VERIFIES` from a passing
    /// `Verification`. "Verified means a check that passes, not one that
    /// exists" (`dec:passing-is-verified`).
    pub(crate) fn has_passing_verification(&self, node_id: &str) -> Result<bool, DynoError> {
        for e in self.incoming(node_id, Some(edge::VERIFIES))? {
            let passing = self
                .get_node(node::VERIFICATION, &e.from_id)?
                .and_then(|v| {
                    v.properties
                        .get("status")
                        .and_then(dynograph_core::Value::as_str)
                        .map(|s| s == "passing")
                })
                .unwrap_or(false);
            if passing {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Compute a capability's [`CapabilityVerification`] state. See the enum
    /// for why this is three-valued.
    pub fn capability_verification(
        &self,
        capability_id: &str,
    ) -> Result<CapabilityVerification, DynoError> {
        if self.has_passing_verification(capability_id)? {
            return Ok(CapabilityVerification::Verified);
        }
        for e in self.outgoing(capability_id, Some(edge::ALLOCATED_TO))? {
            if self.has_passing_verification(&e.to_id)? {
                return Ok(CapabilityVerification::ComponentVerified);
            }
        }
        Ok(CapabilityVerification::Unchecked)
    }
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

// ---- The P4 reconcile (BL-30's M half) -------------------------------------
//
// The last of the three feedback loops: `reconcile_artifacts` asks *does the
// code match the design?* (P3), `reconcile_deployment` asks *does what runs
// match what is declared?* (P5), and this asks *does the recorded outcome
// match what the test run actually reported?* — the exact hole the erosion
// trial fell through, where a status written once was believed forever.
// Adoption's dynamic-analysis step lands here too: run the found system's
// tests, feed the outcomes in, and the graph says where its beliefs diverge.

/// One observed check outcome from a real run. `outcome` is what the runner
/// reported: `passed` / `failed` / `skipped`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservedVerification {
    pub verification_id: String,
    pub outcome: String,
}

/// One divergence between a recorded status and an observed outcome.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationFinding {
    pub verification_id: String,
    /// What the design believed (`Verification.status`).
    pub declared: String,
    /// What the run reported.
    pub observed: String,
    pub message: String,
    /// What this check verifies — where the divergence lands in the design.
    pub verifies: Vec<String>,
    pub event_id: Option<String>,
}

/// The outcome of a P4 reconcile pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationDriftReport {
    /// Divergences, believed-proven-actually-broken first.
    pub findings: Vec<VerificationFinding>,
    /// Observations matching the recorded status exactly.
    pub agreements: usize,
    /// Observed ids the design has never heard of.
    pub unknown_ids: Vec<String>,
    /// Observations refused by name (an outcome that is not
    /// passed/failed/skipped) — the rest of the batch still processes.
    pub rejected: Vec<String>,
    /// Recorded `passing`/`failing` claims the observation did not cover.
    /// Only under `exhaustive` — a partial run is not evidence of absence.
    pub unobserved: Vec<String>,
    /// Prior events this pass resolved: the divergence is no longer observed.
    pub resolved_events: Vec<String>,
    /// `DriftEvent`s recorded this run (empty unless recording).
    pub recorded_events: Vec<String>,
    /// Seeds for `propagate_from` — the diverging checks' VERIFIES targets.
    pub propagation_seeds: Vec<String>,
}

/// Options for a P4 reconcile pass (the same shape as its two siblings).
#[derive(Debug, Clone, Default)]
pub struct VerifyReconcileOptions {
    pub record_events: bool,
    pub exhaustive: bool,
    pub detected_at: Option<String>,
}

/// declared status ↔ observed outcome agreement.
fn agrees(declared: &str, observed: &str) -> bool {
    matches!(
        (declared, observed),
        ("passing", "passed") | ("failing", "failed") | ("skipped", "skipped")
    )
}

impl DesignGraph {
    /// Compare what a real run reported against what the design records.
    ///
    /// Never edits the design — the answer to a divergence is
    /// [`set_verification_status`](Self::set_verification_status) with what
    /// the run actually said (or fixing the thing under test), confirmed by
    /// the next reconcile, which resolves the event on agreement. Recording
    /// is optional; resolution of this pass's own prior events is not, since
    /// a divergence no longer observed is answered by definition.
    pub fn reconcile_verification(
        &mut self,
        observed: &[ObservedVerification],
        options: &VerifyReconcileOptions,
    ) -> Result<VerificationDriftReport, DynoError> {
        let mut findings: Vec<VerificationFinding> = Vec::new();
        let mut agreements = 0usize;
        let mut unknown_ids = Vec::new();
        let mut rejected = Vec::new();
        let mut covered: Vec<String> = Vec::new();

        for obs in observed {
            if !matches!(obs.outcome.as_str(), "passed" | "failed" | "skipped") {
                rejected.push(format!(
                    "{}: outcome '{}' is not one of passed/failed/skipped",
                    obs.verification_id, obs.outcome
                ));
                continue;
            }
            let Some(ver) = self.get_node(node::VERIFICATION, &obs.verification_id)? else {
                unknown_ids.push(obs.verification_id.clone());
                continue;
            };
            covered.push(obs.verification_id.clone());
            let declared = ver
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("planned")
                .to_string();
            if agrees(&declared, &obs.outcome) {
                agreements += 1;
                continue;
            }
            let verifies: Vec<String> = self
                .outgoing(&obs.verification_id, Some(edge::VERIFIES))?
                .into_iter()
                .map(|e| e.to_id)
                .collect();
            findings.push(VerificationFinding {
                verification_id: obs.verification_id.clone(),
                declared: declared.clone(),
                observed: obs.outcome.clone(),
                message: format!(
                    "'{}' is recorded as '{declared}' and the run reported '{}'",
                    obs.verification_id, obs.outcome
                ),
                verifies,
                event_id: None,
            });
        }

        // Believed-proven-actually-broken is the reflow1 failure in miniature
        // and sorts first; then by id for determinism.
        findings.sort_by_key(|f| {
            (
                u8::from(!(f.declared == "passing" && f.observed == "failed")),
                f.verification_id.clone(),
            )
        });

        let mut unobserved = Vec::new();
        if options.exhaustive {
            for ver in self.scan_nodes(node::VERIFICATION)? {
                if covered.contains(&ver.node_id) {
                    continue;
                }
                let status = ver
                    .properties
                    .get("status")
                    .and_then(dynograph_core::Value::as_str)
                    .unwrap_or("planned");
                // Only run-outcome claims can be contradicted by a run that
                // did not include them; planned/skipped/blocked are not
                // claims about a run.
                if status == "passing" || status == "failing" {
                    unobserved.push(ver.node_id.clone());
                }
            }
            unobserved.sort();
        }

        // Resolve prior events for checks this pass observed, where the
        // divergence is no longer among the current findings.
        let current: std::collections::BTreeSet<String> = findings
            .iter()
            .map(|f| verification_event_id(&f.verification_id, &f.declared, &f.observed))
            .collect();
        let mut resolved_events = Vec::new();
        for ev in self.scan_nodes(node::DRIFT_EVENT)? {
            let is_status = ev
                .properties
                .get("drift_type")
                .and_then(dynograph_core::Value::as_str)
                == Some("status_mismatch");
            let resolved = ev
                .properties
                .get("resolved")
                .and_then(dynograph_core::Value::as_bool)
                .unwrap_or(false);
            if !is_status || resolved || current.contains(&ev.node_id) {
                continue;
            }
            let about_covered = self
                .outgoing(&ev.node_id, Some(edge::DEPENDS_ON))?
                .iter()
                .any(|e| covered.contains(&e.to_id));
            if about_covered {
                let mut props = Props::new().set("resolved", true);
                for (k, v) in &ev.properties {
                    if k != "resolved" {
                        props = props.set(k, v.clone());
                    }
                }
                self.create_node(node::DRIFT_EVENT, &ev.node_id, props)?;
                resolved_events.push(ev.node_id.clone());
            }
        }
        resolved_events.sort();

        let mut recorded_events = Vec::new();
        if options.record_events {
            for f in &mut findings {
                let id = verification_event_id(&f.verification_id, &f.declared, &f.observed);
                if self.get_node(node::DRIFT_EVENT, &id)?.is_none() {
                    let severity = if f.declared == "passing" && f.observed == "failed" {
                        "high" // believed proven, actually broken
                    } else {
                        "medium"
                    };
                    self.create_node(
                        node::DRIFT_EVENT,
                        &id,
                        Props::new()
                            .set("name", format!("{} status drift", f.verification_id))
                            .set("summary", f.message.as_str())
                            .set("drift_type", "status_mismatch")
                            .set("severity", severity)
                            .set_opt("detected_at", options.detected_at.as_deref()),
                    )?;
                    self.create_edge(
                        edge::DEPENDS_ON,
                        node::DRIFT_EVENT,
                        &id,
                        node::VERIFICATION,
                        &f.verification_id,
                        Props::new(),
                    )?;
                }
                recorded_events.push(id.clone());
                f.event_id = Some(id);
            }
        }

        let mut propagation_seeds: Vec<String> = findings
            .iter()
            .flat_map(|f| f.verifies.iter().cloned())
            .collect();
        propagation_seeds.sort();
        propagation_seeds.dedup();
        unknown_ids.sort();
        rejected.sort();

        Ok(VerificationDriftReport {
            findings,
            agreements,
            unknown_ids,
            rejected,
            unobserved,
            resolved_events,
            recorded_events,
            propagation_seeds,
        })
    }
}

/// Deterministic event id: the divergence is "check X reported OBSERVED while
/// the design believed DECLARED", so re-observing the same pair is the same
/// unresolved event and a different pair is a new one — the flapping history
/// stays visible, per axis Z.
fn verification_event_id(verification_id: &str, declared: &str, observed: &str) -> String {
    format!(
        "drift:{:016x}",
        crate::detect::fnv1a(&format!(
            "status_mismatch|{verification_id}|{declared}|{observed}"
        ))
    )
}
