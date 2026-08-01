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
    /// What the caller observed this artifact **actually implementing** — the
    /// design node ids it would draw `REALIZES` to if it were registering the
    /// file today.
    ///
    /// This is what makes drift *directional*. A checksum says a file moved; it
    /// cannot say which way, so a file that grew a whole subsystem and a file
    /// with a typo fixed are the same signal. Comparing what was observed
    /// against what the design records separates them: more than recorded is
    /// **understatement**, less is **overstatement**.
    ///
    /// `None` means "not assessed" and is not evidence of anything — direction
    /// is simply not judged, exactly as before. An empty vec is different and
    /// means "assessed, implements nothing recognisable", which is a real claim.
    #[serde(default)]
    pub realizes: Option<Vec<String>>,
}

/// Which way the design and the build diverge — the asymmetry a checksum cannot
/// see.
///
/// Field observation that motivated this (storyflow, 2026-07-24): the docs
/// "consistently understate what's built". Drift is not symmetric noise —
/// implementation accretes capability the design never records, far more often
/// than a design overstates. Naming the direction is what turns "something
/// changed" into something a person can act on without re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDirection {
    /// The build does more than the design records — the storyflow signature.
    Understated,
    /// The design claims more than the build was observed to do.
    Overstated,
    /// Each side has something the other lacks: not a gap in one direction but
    /// a disagreement, which usually means the design moved on a different axis
    /// than the code did.
    Diverged,
}

impl DriftDirection {
    /// Stable snake_case key.
    pub fn as_str(self) -> &'static str {
        match self {
            DriftDirection::Understated => "understated",
            DriftDirection::Overstated => "overstated",
            DriftDirection::Diverged => "diverged",
        }
    }
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
    /// A **registered** artifact implements more than the design records — the
    /// same condition as `UndocumentedAddition` one level down, and the one
    /// that was invisible: a new file is noticed, a *grown* file is not.
    /// Records as `undocumented_addition`, because that is what it is.
    Understated,
    /// A registered artifact implements less than the design claims. Records as
    /// `spec_mismatch`: the spec and the thing disagree, and here the spec is
    /// the optimistic one.
    Overstated,
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
            DriftKind::Understated => "understated",
            DriftKind::Overstated => "overstated",
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
            // Deliberately mapped onto existing schema values rather than
            // adding two: a schema change costs a minor bump, an upgrade doc,
            // and real pain for anyone on an older stamp (BL-94). These fit the
            // existing vocabulary honestly, so the cost buys nothing.
            DriftKind::Understated => Some("undocumented_addition"),
            DriftKind::Overstated => Some("spec_mismatch"),
            DriftKind::NoBaseline => None,
        }
    }

    /// Schema `DriftEvent.severity`.
    fn severity(self) -> &'static str {
        match self {
            DriftKind::MissingArtifact => "high",
            DriftKind::ChecksumChange => "medium",
            DriftKind::UndocumentedAddition => "medium",
            DriftKind::Understated => "medium",
            // Higher than understatement on purpose. A design that claims
            // something the build does not do will be *relied on* — someone
            // plans against a capability that is not there. Understatement is a
            // record that is behind; overstatement is a record that is wrong.
            DriftKind::Overstated => "high",
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
    /// Which way this diverges, when the observation carried an assessment of
    /// what the artifact actually implements. `None` means direction was not
    /// judged — the caller supplied no `realizes`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<DriftDirection>,
    /// Observed to be implemented, but not recorded in the design. **This is
    /// the answer to "what does the design now claim wrongly?"** — the part a
    /// checksum could never supply, and without which accepting drift means
    /// asking someone to go and find the delta themselves.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unrecorded: Vec<String>,
    /// Recorded in the design, but not observed to be implemented. The other
    /// direction, and the more dangerous one: someone may be planning against
    /// it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unbuilt: Vec<String>,
}

/// The outcome of a reconcile pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriftReport {
    /// Divergences found, ranked most severe first then by id.
    pub findings: Vec<DriftFinding>,
    /// Observations that matched their recorded checksum exactly.
    pub unchanged: usize,
    /// Artifacts stamped with a fresh `last_confirmed_at` this run — the ones
    /// that matched, when `record_events` was set (BL-158).
    ///
    /// **Why a clean pass has to write something.** Before this, `record_events`
    /// only ever recorded a *divergence*, so a sweep that checked everything and
    /// found everything correct left no trace — and the confirmation ledger,
    /// which computes `unexamined` from recorded claims, went on saying nobody
    /// had ever looked. Reproduced on reflow2's own design: 107 artifacts, 106
    /// unchanged, zero drift, and `loop_status` moved by zero. The operator who
    /// checks everything and the operator who checks nothing saw byte-identical
    /// output.
    ///
    /// Listed by id rather than counted, because the honest version of this
    /// records **what was actually observed** — a partial sweep confirms exactly
    /// the artifacts it looked at and must never read as a full one.
    pub confirmed: Vec<String>,
    /// Artifacts that matched but could **not** be confirmed, because
    /// `record_events` was set and no `detected_at` was supplied.
    ///
    /// A confirmation exists to answer *when* someone last looked, so writing an
    /// undated one would enter the ledger as evidence while being unable to say
    /// when — the flattering half of the ambiguity this whole record exists to
    /// remove. The core takes no clock, so it cannot fill the date in. It
    /// therefore skips the stamp and **says so here**: a dropped write the
    /// caller has to infer from a count that did not move is the silent-drop
    /// shape this project forbids, and it is the exact shape of the bug being
    /// fixed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unconfirmed_undated: Vec<String>,
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
        let mut matched: Vec<String> = Vec::new();

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
                    direction: None,
                    unrecorded: Vec::new(),
                    unbuilt: Vec::new(),
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
                    direction: None,
                    unrecorded: Vec::new(),
                    unbuilt: Vec::new(),
                });
                continue;
            }

            // Both sides through the canonicaliser before comparing: a bare hex
            // digest and its `sha256:`-prefixed form are the same digest, and
            // comparing them literally reports drift on a file nobody touched
            // (BL-125). The observed value is canonicalised too, not just
            // compared canonically, because it is part of a checksum_change
            // event's identity — leaving the raw form would file one divergence
            // twice under two ids depending on which dialect was supplied.
            //
            // The comparison itself is `checksums_agree` rather than `==`,
            // because LENGTH is a dialect as well as the prefix (BL-160): a
            // design registering 16 hex chars and a caller supplying all 64
            // are describing the same bytes.
            let recorded = artifact
                .properties
                .get("checksum")
                .and_then(|v| v.as_str())
                .map(crate::artifact::canonical_checksum);
            let observed_canonical = obs
                .checksum
                .as_deref()
                .map(crate::artifact::canonical_checksum);
            match (recorded, observed_canonical.as_deref()) {
                (Some(recorded), Some(current))
                    if crate::artifact::checksums_agree(&recorded, current) =>
                {
                    unchanged += 1;
                    matched.push(obs.artifact_id.clone());
                }
                (Some(_), Some(_)) => findings.push(DriftFinding {
                    artifact_id: obs.artifact_id.clone(),
                    kind: DriftKind::ChecksumChange,
                    message: format!(
                        "'{}' has changed since it was registered against the design",
                        obs.artifact_id
                    ),
                    realizes: realizes.clone(),
                    observed_checksum: observed_canonical.clone(),
                    event_id: None,
                    direction: None,
                    unrecorded: Vec::new(),
                    unbuilt: Vec::new(),
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
                        realizes: realizes.clone(),
                        observed_checksum: None,
                        event_id: None,
                        direction: None,
                        unrecorded: Vec::new(),
                        unbuilt: Vec::new(),
                    });
                }
            }

            // Direction, when the caller assessed what the file actually
            // implements. Judged INDEPENDENTLY of the checksum on purpose: a
            // design can be wrong from the day it was written, and an artifact
            // whose bytes never moved can still be described by a design that
            // understates it. Tying this to checksum_change would miss exactly
            // the long-lived files where understatement accumulates.
            if let Some(observed_targets) = &obs.realizes {
                let recorded: std::collections::BTreeSet<&str> =
                    realizes.iter().map(String::as_str).collect();
                let seen: std::collections::BTreeSet<&str> =
                    observed_targets.iter().map(String::as_str).collect();

                let unrecorded: Vec<String> =
                    seen.difference(&recorded).map(|s| s.to_string()).collect();
                let unbuilt: Vec<String> =
                    recorded.difference(&seen).map(|s| s.to_string()).collect();

                let direction = match (unrecorded.is_empty(), unbuilt.is_empty()) {
                    (true, true) => None,
                    (false, true) => Some(DriftDirection::Understated),
                    (true, false) => Some(DriftDirection::Overstated),
                    (false, false) => Some(DriftDirection::Diverged),
                };

                if let Some(direction) = direction {
                    // The message names WHAT the design has wrong. That is the
                    // whole point: telling someone their design is stale without
                    // telling them what to fix is why the fix does not happen.
                    let mut parts = Vec::new();
                    if !unrecorded.is_empty() {
                        parts.push(format!(
                            "implements {} that the design does not record",
                            unrecorded.join(", ")
                        ));
                    }
                    if !unbuilt.is_empty() {
                        parts.push(format!(
                            "does not implement {} that the design claims",
                            unbuilt.join(", ")
                        ));
                    }
                    let kind = match direction {
                        DriftDirection::Understated => DriftKind::Understated,
                        // A disagreement is reported as overstatement, the more
                        // serious half: something is claimed that is not there,
                        // and that is what someone will plan against.
                        DriftDirection::Overstated | DriftDirection::Diverged => {
                            DriftKind::Overstated
                        }
                    };
                    findings.push(DriftFinding {
                        artifact_id: obs.artifact_id.clone(),
                        kind,
                        message: format!("'{}' {}", obs.artifact_id, parts.join("; and ")),
                        realizes,
                        observed_checksum: obs.checksum.clone(),
                        event_id: None,
                        direction: Some(direction),
                        unrecorded,
                        unbuilt,
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

        let mut confirmed = Vec::new();
        let mut unconfirmed_undated = Vec::new();
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
            // A clean result is a result (BL-158). Recording only divergence is
            // what made a full sweep and no sweep at all indistinguishable, so
            // an artifact observed to still match its baseline is stamped with
            // the date it was confirmed.
            //
            // A PROPERTY, not an event, and the distinction is deliberate: a
            // confirmation is high-frequency and says nothing changed, so
            // minting a node per artifact per pass would bury axis Z — the log
            // of what actually *moved* — under non-events. This is the shape
            // `Verification.last_run_at` already uses to answer the same
            // question about a check.
            matched.sort();
            matched.dedup();
            for artifact_id in matched {
                if self.stamp_confirmed(&artifact_id, options.detected_at.as_deref())? {
                    confirmed.push(artifact_id);
                } else {
                    unconfirmed_undated.push(artifact_id);
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
            confirmed,
            unconfirmed_undated,
            unobserved,
            propagation_seeds,
            recorded_events,
        })
    }

    /// Stamp an artifact with the date it was last observed to match its
    /// baseline. Returns whether the stamp was written.
    ///
    /// Refuses to write an **undated** confirmation: `last_confirmed_at` exists
    /// to answer *when*, and a confirmation carrying no date would enter the
    /// ledger as evidence that someone looked while being unable to say when —
    /// which is the flattering half of the very ambiguity BL-158 is about. The
    /// caller is told by the artifact's absence from
    /// [`DriftReport::confirmed`], so a skipped stamp is visible rather than
    /// silent (the core takes no clock; the caller supplies `detected_at`).
    fn stamp_confirmed(&mut self, artifact_id: &str, at: Option<&str>) -> Result<bool, DynoError> {
        let Some(at) = at else {
            return Ok(false);
        };
        let Some(existing) = self.get_node(node::ARTIFACT, artifact_id)? else {
            return Ok(false);
        };
        let mut props = Props::new().set("last_confirmed_at", at);
        for (k, v) in &existing.properties {
            if k != "last_confirmed_at" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::ARTIFACT, artifact_id, props)?;
        Ok(true)
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
    // Overstatement sorts above a checksum change, and above understatement:
    // a design claiming something the build does not do is the one someone will
    // plan against. Understatement is a record that is behind; overstatement is
    // a record that is wrong. The pre-existing kinds keep their relative order.
    match kind {
        DriftKind::MissingArtifact => 0,
        DriftKind::Overstated => 1,
        DriftKind::ChecksumChange => 2,
        DriftKind::Understated => 3,
        DriftKind::UndocumentedAddition => 4,
        DriftKind::NoBaseline => 5,
    }
}
