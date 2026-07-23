//! P5 · The as-fielded reconcile — `reconcile_deployment` (BL-9).
//!
//! The sibling of [`crate::drift`]'s `reconcile_artifacts`, one phase later:
//! that one asks *does the code match the design?*, this one asks *does what
//! is **running** match what the design declares?* Together with
//! `reconcile_verification` (BL-30, open) they are the missing feedback loops
//! the phase-coverage trial scored — the design carrying weight after the
//! build starts.
//!
//! The comparison is strictly between **declarations** (`Release DEPLOYED_TO
//! Environment`, with its `status`) and **observations** (the caller says
//! which releases are actually running in which environments). Components
//! never participate: the original reflow expected every component to appear
//! as a running thing and manufactured false drift for every library and
//! plugin (its v3.23.0 `library_plugin` fix; see reflow-audit.md's cautions).
//! Here a part that ships *inside* a release is invisible to this check by
//! construction — only Releases run, and only Environments host.
//!
//! Like its sibling it never edits the design: it *adds* `DriftEvent`s (when
//! asked to record) and it *resolves* its own prior events when a new
//! observation shows the divergence is gone — the observation is the
//! authority on the fielded side, the same way the two-sided accept is on the
//! built side (BL-33/BL-35). Deciding which side was wrong stays human.

use dynograph_core::{DynoError, Value};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// One environment's observed state: the releases actually running there.
/// An entry with an empty `running` list is a positive statement — *nothing
/// runs here* — not an absence of evidence; environments not listed at all
/// are not evidence of anything.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservedEnvironment {
    pub environment_id: String,
    #[serde(default)]
    pub running: Vec<String>,
}

/// The kind of as-fielded divergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldedDriftKind {
    /// Declared `active` in this environment, and not observed running.
    DeploymentMissing,
    /// Observed running with no `DEPLOYED_TO` declaration at all.
    DeploymentUndeclared,
    /// A declaration exists but its `status` says not-running (`planned` /
    /// `rolled_back`), and it was observed running anyway.
    DeploymentContradicted,
}

impl FieldedDriftKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldedDriftKind::DeploymentMissing => "deployment_missing",
            FieldedDriftKind::DeploymentUndeclared => "deployment_undeclared",
            FieldedDriftKind::DeploymentContradicted => "deployment_contradicted",
        }
    }

    fn severity(self) -> &'static str {
        match self {
            // Something the users depend on is not actually there.
            FieldedDriftKind::DeploymentMissing => "high",
            // Something runs that the design cannot account for.
            FieldedDriftKind::DeploymentUndeclared => "high",
            // Running, but the record says it should not be.
            FieldedDriftKind::DeploymentContradicted => "medium",
        }
    }
}

/// One divergence between a deployment declaration and observed reality.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldedFinding {
    pub environment_id: String,
    pub release_id: String,
    pub kind: FieldedDriftKind,
    pub message: String,
    /// The recorded `DriftEvent` id, when recording was on and both endpoints
    /// exist to anchor it.
    pub event_id: Option<String>,
}

/// The outcome of an as-fielded reconcile pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldedReport {
    /// Divergences, deployment_missing first, then by environment/release.
    pub findings: Vec<FieldedFinding>,
    /// Declarations that matched what was observed (active and running, or
    /// planned/rolled_back and not running).
    pub agreements: usize,
    /// Observed ids the design has never heard of — an unknown environment or
    /// release cannot be reconciled and must not be silently skipped.
    pub unknown_ids: Vec<String>,
    /// Declared-`active` deployments in environments the observation did not
    /// cover. Only populated when `exhaustive` — a partial observation is not
    /// evidence of absence.
    pub unobserved: Vec<String>,
    /// Prior deployment `DriftEvent`s this pass resolved, because the new
    /// observation shows the divergence is gone.
    pub resolved_events: Vec<String>,
    /// `DriftEvent`s recorded this run (empty unless recording).
    pub recorded_events: Vec<String>,
    /// Seeds for `propagate_from` — the releases and environments diverging.
    pub propagation_seeds: Vec<String>,
}

/// Options for an as-fielded reconcile pass.
#[derive(Debug, Clone, Default)]
pub struct FieldedOptions {
    /// Write a `DriftEvent` per divergence, linked to the Environment and the
    /// Release it is about. Off by default: look before you write.
    pub record_events: bool,
    /// Treat the observation as covering every environment: declared-active
    /// deployments in environments not listed are reported as `unobserved`.
    pub exhaustive: bool,
    /// Timestamp for recorded events; the core takes no clock.
    pub detected_at: Option<String>,
}

impl DesignGraph {
    /// Compare what is observed running against what `DEPLOYED_TO` declares.
    ///
    /// Never edits declarations. Records divergences as `DriftEvent`s when
    /// asked, and resolves its own prior events when the divergence is no
    /// longer observed — so the persistent `unresolved_drift` gap opens and
    /// closes with reality, and the design-side answer stays a human edit
    /// (`deploy_to` with the true status) confirmed by the next reconcile.
    pub fn reconcile_deployment(
        &mut self,
        observed: &[ObservedEnvironment],
        options: &FieldedOptions,
    ) -> Result<FieldedReport, DynoError> {
        let mut findings: Vec<FieldedFinding> = Vec::new();
        let mut agreements = 0usize;
        let mut unknown_ids: Vec<String> = Vec::new();
        let mut observed_envs: Vec<String> = Vec::new();

        for obs in observed {
            if self
                .get_node(node::ENVIRONMENT, &obs.environment_id)?
                .is_none()
            {
                unknown_ids.push(obs.environment_id.clone());
                continue;
            }
            observed_envs.push(obs.environment_id.clone());

            // Declarations targeting this environment.
            let declared = self.incoming(&obs.environment_id, Some(edge::DEPLOYED_TO))?;
            let mut declared_by_release: Vec<(String, String)> = Vec::new();
            for e in &declared {
                let status = e
                    .properties
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("planned")
                    .to_string();
                declared_by_release.push((e.from_id.clone(), status));
            }

            let mut running: Vec<&str> = Vec::new();
            for rel in &obs.running {
                if self.get_node(node::RELEASE, rel)?.is_none() {
                    unknown_ids.push(rel.clone());
                } else {
                    running.push(rel);
                }
            }

            for (rel, status) in &declared_by_release {
                let is_running = running.iter().any(|r| r == rel);
                match (status.as_str(), is_running) {
                    ("active", true) => agreements += 1,
                    ("active", false) => findings.push(FieldedFinding {
                        environment_id: obs.environment_id.clone(),
                        release_id: rel.clone(),
                        kind: FieldedDriftKind::DeploymentMissing,
                        message: format!(
                            "'{rel}' is declared active in '{}' and is not running there",
                            obs.environment_id
                        ),
                        event_id: None,
                    }),
                    (_, true) => findings.push(FieldedFinding {
                        environment_id: obs.environment_id.clone(),
                        release_id: rel.clone(),
                        kind: FieldedDriftKind::DeploymentContradicted,
                        message: format!(
                            "'{rel}' is running in '{}' but its declaration says '{status}'",
                            obs.environment_id
                        ),
                        event_id: None,
                    }),
                    (_, false) => agreements += 1, // planned/rolled_back and not running
                }
            }

            for rel in &running {
                if !declared_by_release.iter().any(|(r, _)| r == rel) {
                    findings.push(FieldedFinding {
                        environment_id: obs.environment_id.clone(),
                        release_id: (*rel).to_string(),
                        kind: FieldedDriftKind::DeploymentUndeclared,
                        message: format!(
                            "'{rel}' is running in '{}' and no DEPLOYED_TO declares it",
                            obs.environment_id
                        ),
                        event_id: None,
                    });
                }
            }
        }

        // Exhaustive: a declared-active deployment in an environment the
        // observation never covered is unobservable, which must be said.
        let mut unobserved = Vec::new();
        if options.exhaustive {
            for env in self.scan_nodes(node::ENVIRONMENT)? {
                if observed_envs.contains(&env.node_id) {
                    continue;
                }
                for e in self.incoming(&env.node_id, Some(edge::DEPLOYED_TO))? {
                    let active = e
                        .properties
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s == "active");
                    if active {
                        unobserved.push(format!("{} in {}", e.from_id, env.node_id));
                    }
                }
            }
            unobserved.sort();
        }

        findings.sort_by_key(|f| {
            (
                match f.kind {
                    FieldedDriftKind::DeploymentMissing => 0,
                    FieldedDriftKind::DeploymentUndeclared => 1,
                    FieldedDriftKind::DeploymentContradicted => 2,
                },
                f.environment_id.clone(),
                f.release_id.clone(),
            )
        });

        // Resolve prior events whose divergence the new observation no longer
        // shows — only for environments this pass actually observed.
        let current: std::collections::BTreeSet<String> = findings
            .iter()
            .map(|f| fielded_event_id(f.kind, &f.environment_id, &f.release_id))
            .collect();
        let mut resolved_events = Vec::new();
        for ev in self.scan_nodes(node::DRIFT_EVENT)? {
            let dt = ev.properties.get("drift_type").and_then(Value::as_str);
            let is_fielded = matches!(
                dt,
                Some("deployment_missing" | "deployment_undeclared" | "deployment_contradicted")
            );
            let resolved = ev
                .properties
                .get("resolved")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !is_fielded || resolved || current.contains(&ev.node_id) {
                continue;
            }
            let about_observed_env = self
                .outgoing(&ev.node_id, Some(edge::DEPENDS_ON))?
                .iter()
                .any(|e| observed_envs.contains(&e.to_id));
            if about_observed_env {
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
                let id = fielded_event_id(f.kind, &f.environment_id, &f.release_id);
                if self.get_node(node::DRIFT_EVENT, &id)?.is_none() {
                    self.create_node(
                        node::DRIFT_EVENT,
                        &id,
                        Props::new()
                            .set("name", format!("{} / {}", f.environment_id, f.release_id))
                            .set("summary", f.message.as_str())
                            .set("drift_type", f.kind.as_str())
                            .set("severity", f.kind.severity())
                            .set_opt("detected_at", options.detected_at.as_deref()),
                    )?;
                    // The event is about this environment and this release, when
                    // they exist to point at — PROPAGATE walks DEPENDS_ON.
                    for (t, tid) in [
                        (node::ENVIRONMENT, f.environment_id.as_str()),
                        (node::RELEASE, f.release_id.as_str()),
                    ] {
                        if self.get_node(t, tid)?.is_some() {
                            self.create_edge(
                                edge::DEPENDS_ON,
                                node::DRIFT_EVENT,
                                &id,
                                t,
                                tid,
                                Props::new(),
                            )?;
                        }
                    }
                }
                recorded_events.push(id.clone());
                f.event_id = Some(id);
            }
        }

        // A loop, not a `.filter()` with `.ok().flatten()`: the old form
        // swallowed a storage error as "id not found" and silently dropped the
        // propagation seed (BL-58). `?` surfaces a real read failure instead.
        let mut propagation_seeds: Vec<String> = Vec::new();
        for f in &findings {
            for id in [&f.environment_id, &f.release_id] {
                let is_seed = self.get_node(node::ENVIRONMENT, id)?.is_some()
                    || self.get_node(node::RELEASE, id)?.is_some();
                if is_seed {
                    propagation_seeds.push(id.clone());
                }
            }
        }
        propagation_seeds.sort();
        propagation_seeds.dedup();

        unknown_ids.sort();
        unknown_ids.dedup();

        Ok(FieldedReport {
            findings,
            agreements,
            unknown_ids,
            unobserved,
            resolved_events,
            recorded_events,
            propagation_seeds,
        })
    }
}

/// Deterministic event id. State-shaped like `missing_artifact`: the same
/// divergence re-observed is the same unresolved event, and resolution comes
/// from a later observation showing agreement — not from re-keying.
fn fielded_event_id(kind: FieldedDriftKind, environment_id: &str, release_id: &str) -> String {
    format!(
        "drift:{:016x}",
        crate::nodes::fnv1a(&format!(
            "{}|{}|{}",
            kind.as_str(),
            environment_id,
            release_id
        ))
    )
}
