//! As-built drift — reconcile what the design says was built against what is
//! actually there (SP-6b).
//!
//! This closes the loop the other direction. [`crate::artifact`] records that a
//! real file `REALIZES` a Capability; this module checks whether that record is
//! still true, and turns each divergence into a `DriftEvent` the design can
//! propagate from.
//!
//! ## Why the caller supplies the observations
//!
//! Reflow2's core performs **no I/O**. [`reconcile_artifacts`](DesignGraph::reconcile_artifacts)
//! takes the observed state — does this artifact still exist, and what is its
//! content hash — from whoever *can* observe it: the coding agent (which already
//! has filesystem access), a CI step, a CLI. That keeps the core deterministic
//! and testable without fixtures, and it is the same seam pattern as
//! [`LlmBackend`](crate::llm::LlmBackend): the core names the capability it
//! needs, the surface provides it. It also means an `Artifact` whose `location`
//! is a URL or a part number in a PLM system reconciles exactly like a file —
//! the hash is opaque here.
//!
//! ## Why drift propagates *backwards*
//!
//! `REALIZES` runs Artifact → Capability, and PROPAGATE classifies that forward
//! direction as **Upstream** (see [`crate::propagate`]). So seeding propagation
//! from a drifted Artifact walks *up the golden thread* — to the Capability it
//! realizes, and on to the Requirement that Capability satisfies. A change made
//! in code therefore reaches the design that justified it, which is the failure
//! the original Reflow never solved: implementation drifting without the
//! systems-engineering layer ever hearing about it.
//!
//! [`DriftReport::propagation_seeds`] carries exactly those seed ids, ready to
//! hand to [`propagate_from`](DesignGraph::propagate_from).

use dynograph_core::DynoError;

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// What the caller observed about one registered artifact.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservedArtifact {
    /// The `Artifact` node id this observation is about.
    pub artifact_id: String,
    /// Whether the artifact still exists at its recorded location.
    pub present: bool,
    /// Its current content hash, if the caller computed one. `None` means "not
    /// hashed", which is reported as [`DriftKind::NoBaseline`] rather than
    /// silently passing.
    #[serde(default)]
    pub checksum: Option<String>,
}

/// The kind of divergence found. Maps onto the schema's `DriftEvent.drift_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    /// Registered in the design, but no longer there.
    MissingArtifact,
    /// Still there, but its content changed since it was registered.
    ChecksumChange,
    /// Observed, but no such `Artifact` node exists — something was built that
    /// the design does not know about.
    UndocumentedAddition,
    /// Cannot be judged: no checksum recorded, or none observed. Surfaced rather
    /// than treated as unchanged.
    NoBaseline,
}

impl DriftKind {
    /// Stable snake_case key.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftKind::MissingArtifact => "missing_artifact",
            DriftKind::ChecksumChange => "checksum_change",
            DriftKind::UndocumentedAddition => "undocumented_addition",
            DriftKind::NoBaseline => "no_baseline",
        }
    }

    /// The schema `DriftEvent.drift_type` this records as. `NoBaseline` has no
    /// schema counterpart — it is an observability gap, not a divergence — so it
    /// is reported but never recorded as a `DriftEvent`.
    fn drift_type(self) -> Option<&'static str> {
        match self {
            DriftKind::MissingArtifact => Some("missing_artifact"),
            DriftKind::ChecksumChange => Some("checksum_change"),
            DriftKind::UndocumentedAddition => Some("undocumented_addition"),
            DriftKind::NoBaseline => None,
        }
    }

    /// Schema `DriftEvent.severity`.
    fn severity(self) -> &'static str {
        match self {
            DriftKind::MissingArtifact => "high",
            DriftKind::ChecksumChange => "medium",
            DriftKind::UndocumentedAddition => "medium",
            DriftKind::NoBaseline => "low",
        }
    }
}

/// One divergence between the design and reality.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriftFinding {
    /// The artifact this is about.
    pub artifact_id: String,
    /// What kind of divergence.
    pub kind: DriftKind,
    /// Human-readable description.
    pub message: String,
    /// Design nodes this artifact `REALIZES` — where the change lands in the
    /// design, and the seeds for backward propagation.
    pub realizes: Vec<String>,
    /// The checksum observed this pass, when the observation carried one.
    /// For a `checksum_change` this is part of the event's *identity*: the
    /// event is "the artifact became X while the design believed Y", so a
    /// later drift to a different X is a different event.
    pub observed_checksum: Option<String>,
    /// The recorded `DriftEvent` node id, when `record_events` was set and this
    /// kind has a schema counterpart.
    pub event_id: Option<String>,
}

/// The outcome of a reconcile pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriftReport {
    /// Divergences found, ranked most severe first then by id.
    pub findings: Vec<DriftFinding>,
    /// Observations that matched their recorded checksum exactly.
    pub unchanged: usize,
    /// Registered artifacts that were **not** observed at all. Only populated
    /// when `exhaustive` is set — otherwise absence from the observation list is
    /// not evidence of anything and is left alone.
    pub unobserved: Vec<String>,
    /// Design node ids to hand to `propagate_from` — the union of every
    /// finding's `realizes`, deduplicated and sorted.
    pub propagation_seeds: Vec<String>,
    /// `DriftEvent` node ids recorded this run (empty unless `record_events`).
    pub recorded_events: Vec<String>,
}

/// Options for a reconcile pass.
#[derive(Debug, Clone, Default)]
pub struct ReconcileOptions {
    /// Write a `DriftEvent` node per divergence (linked to its Artifact by
    /// `DEPENDS_ON`, so propagation can start from the event). Off by default:
    /// observing is not the same as recording, and a caller may want to look
    /// before it writes.
    pub record_events: bool,
    /// Treat the observation list as complete — every registered `Artifact`
    /// missing from it is reported as unobserved. Off by default, because a
    /// partial scan must not be read as evidence of absence.
    pub exhaustive: bool,
    /// Timestamp stamped on recorded `DriftEvent`s. The core takes no clock, so
    /// the caller supplies it (and a test can pin it).
    pub detected_at: Option<String>,
}

impl DesignGraph {
    /// Compare observed reality against the design's `Artifact` records.
    ///
    /// Never mutates unless `record_events` is set, and then only *adds*
    /// `DriftEvent`s — it never edits or deletes the design. Deciding what a
    /// divergence means is the human's call; this only makes it visible.
    pub fn reconcile_artifacts(
        &mut self,
        observed: &[ObservedArtifact],
        options: &ReconcileOptions,
    ) -> Result<DriftReport, DynoError> {
        let mut findings = Vec::new();
        let mut unchanged = 0usize;

        for obs in observed {
            let Some(artifact) = self.get_node(node::ARTIFACT, &obs.artifact_id)? else {
                // Observed something the design has never heard of.
                findings.push(DriftFinding {
                    artifact_id: obs.artifact_id.clone(),
                    kind: DriftKind::UndocumentedAddition,
                    message: format!(
                        "'{}' exists but is not registered in the design",
                        obs.artifact_id
                    ),
                    realizes: Vec::new(),
                    observed_checksum: obs.checksum.clone(),
                    event_id: None,
                });
                continue;
            };

            let realizes = self.realized_targets(&obs.artifact_id)?;

            if !obs.present {
                findings.push(DriftFinding {
                    artifact_id: obs.artifact_id.clone(),
                    kind: DriftKind::MissingArtifact,
                    message: format!(
                        "'{}' is registered in the design but no longer exists",
                        obs.artifact_id
                    ),
                    realizes,
                    observed_checksum: None,
                    event_id: None,
                });
                continue;
            }

            let recorded = artifact
                .properties
                .get("checksum")
                .and_then(|v| v.as_str().map(str::to_string));
            match (recorded, obs.checksum.as_deref()) {
                (Some(recorded), Some(current)) if recorded == current => unchanged += 1,
                (Some(_), Some(_)) => findings.push(DriftFinding {
                    artifact_id: obs.artifact_id.clone(),
                    kind: DriftKind::ChecksumChange,
                    message: format!(
                        "'{}' has changed since it was registered against the design",
                        obs.artifact_id
                    ),
                    realizes,
                    observed_checksum: obs.checksum.clone(),
                    event_id: None,
                }),
                // Either side missing → we cannot judge. Say so; never pass silently.
                (recorded, current) => {
                    let why = match (recorded.is_some(), current.is_some()) {
                        (false, true) => "no checksum was recorded when it was registered",
                        (true, false) => "no checksum was supplied for it",
                        _ => "neither a recorded nor an observed checksum is available",
                    };
                    findings.push(DriftFinding {
                        artifact_id: obs.artifact_id.clone(),
                        kind: DriftKind::NoBaseline,
                        message: format!(
                            "'{}' cannot be checked for drift — {why}",
                            obs.artifact_id
                        ),
                        realizes,
                        observed_checksum: None,
                        event_id: None,
                    });
                }
            }
        }

        // Registered-but-unseen, only when the caller vouches for a full sweep.
        let mut unobserved = Vec::new();
        if options.exhaustive {
            let seen: std::collections::HashSet<&str> =
                observed.iter().map(|o| o.artifact_id.as_str()).collect();
            for art in self.scan_nodes(node::ARTIFACT)? {
                if !seen.contains(art.node_id.as_str()) {
                    unobserved.push(art.node_id.clone());
                }
            }
            unobserved.sort();
        }

        // Rank: most severe first, then by id for a stable order.
        findings.sort_by(|a, b| {
            severity_rank(a.kind)
                .cmp(&severity_rank(b.kind))
                .then(a.artifact_id.cmp(&b.artifact_id))
        });

        if options.record_events {
            for finding in &mut findings {
                if let Some(drift_type) = finding.kind.drift_type() {
                    let event_id = drift_event_id(
                        &finding.artifact_id,
                        finding.kind,
                        finding.observed_checksum.as_deref(),
                    );
                    self.write_drift_event(&event_id, finding, drift_type, options)?;
                    finding.event_id = Some(event_id);
                }
            }
        }

        let mut propagation_seeds: Vec<String> = findings
            .iter()
            .flat_map(|f| f.realizes.iter().cloned())
            .collect();
        propagation_seeds.sort();
        propagation_seeds.dedup();

        let recorded_events = findings.iter().filter_map(|f| f.event_id.clone()).collect();

        Ok(DriftReport {
            findings,
            unchanged,
            unobserved,
            propagation_seeds,
            recorded_events,
        })
    }

    /// Design node ids an artifact `REALIZES`, sorted.
    fn realized_targets(&self, artifact_id: &str) -> Result<Vec<String>, DynoError> {
        let mut targets: Vec<String> = self
            .outgoing(artifact_id, Some(edge::REALIZES))?
            .into_iter()
            .map(|e| e.to_id)
            .collect();
        targets.sort();
        Ok(targets)
    }

    /// Record one `DriftEvent`, linked to the artifact it is about so PROPAGATE
    /// can start from the event and walk back into the design.
    fn write_drift_event(
        &mut self,
        event_id: &str,
        finding: &DriftFinding,
        drift_type: &str,
        options: &ReconcileOptions,
    ) -> Result<(), DynoError> {
        if self.get_node(node::DRIFT_EVENT, event_id)?.is_some() {
            return Ok(()); // Same divergence, same id — recorded once.
        }
        self.create_node(
            node::DRIFT_EVENT,
            event_id,
            Props::new()
                .set("name", format!("{} drift", finding.artifact_id))
                .set("summary", finding.message.as_str())
                .set("drift_type", drift_type)
                .set("severity", finding.kind.severity())
                .set_opt("detected_at", options.detected_at.as_deref()),
        )?;
        // The event is *about* this artifact. DEPENDS_ON is lateral in PROPAGATE,
        // so seeding from the event reaches the artifact, then upstream via
        // REALIZES into the design.
        //
        // But an `undocumented_addition` is a file on disk that is NOT a
        // registered Artifact node — so this edge would point at a node that
        // does not exist, a dangling edge the event could never propagate from
        // and whose phantom id then leaked into `unresolved_drift`'s affected
        // set (BL-58). Draw it only when the artifact is really in the graph.
        if finding.kind != DriftKind::UndocumentedAddition
            && self
                .get_node(node::ARTIFACT, &finding.artifact_id)?
                .is_some()
        {
            self.create_edge(
                edge::DEPENDS_ON,
                node::DRIFT_EVENT,
                event_id,
                node::ARTIFACT,
                &finding.artifact_id,
                Props::new(),
            )?;
        }
        Ok(())
    }
}

/// Deterministic `DriftEvent` id, so re-running a reconcile over the same
/// unresolved divergence does not pile up duplicates — while a **new**
/// divergence gets a new event.
///
/// The line between those two is what the first version got wrong: with no
/// discriminator, five successive drifts on one artifact collapsed into one
/// `DriftEvent`, so "drifted once" and "drifted five times, capability never
/// revisited" were the same graph — erasing exactly the accumulation that
/// reveals erosion, and violating axis Z's *never overwrite the past* on the
/// as-built side (BL-33; `temporal.rs` honours it for design edits).
///
/// For a `checksum_change` the observed checksum is part of the identity: the
/// event is "the artifact became X while the design believed Y", so observing
/// the same X twice is one event and a later drift to X′ is another. The
/// state-shaped kinds (`missing_artifact`, `undocumented_addition`) stay keyed
/// on artifact + kind alone — "still missing" re-observed is the same
/// unresolved divergence, not a new one.
fn drift_event_id(artifact_id: &str, kind: DriftKind, observed_checksum: Option<&str>) -> String {
    let discriminator = match kind {
        DriftKind::ChecksumChange => observed_checksum.unwrap_or(""),
        _ => "",
    };
    format!(
        "drift:{:016x}",
        crate::nodes::fnv1a(&format!(
            "{}|{}|{}",
            kind.as_str(),
            artifact_id,
            discriminator
        ))
    )
}

fn severity_rank(kind: DriftKind) -> u8 {
    match kind {
        DriftKind::MissingArtifact => 0,
        DriftKind::ChecksumChange => 1,
        DriftKind::UndocumentedAddition => 2,
        DriftKind::NoBaseline => 3,
    }
}
