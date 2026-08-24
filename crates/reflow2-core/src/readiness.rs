//! Readiness gating and the derived roadmap (BL-68).
//!
//! The problem, in the item's own words: on real programs *"people didn't
//! understand which epoch a design would be delivered on"* — roadmaps are drawn
//! as slides, disconnected from the maturity of the enabling technology, so the
//! delivery timeline is an assertion nobody can defend. Because the golden
//! thread already runs capability → component → enabling technology, and each
//! technology can carry a readiness level with a forecast, **the epoch an
//! increment can deliver on is computable rather than declared**.
//!
//! # The split this module exists to keep
//!
//! A readiness level is an **observation**; the gating threshold is a
//! **judgement**. That line is what keeps a computed roadmap inside
//! `dec:report-dont-judge` and clear of `dec:alternatives-unranked-forkable`'s
//! rule that the human does credit assignment:
//!
//! - A TRL is an input fact about a technology, the same family as a checksum.
//!   Computing over it is `detect_gaps`-shaped, not ranking-shaped.
//! - *"TRL below 5 means not buildable"* is a policy about risk appetite. It is
//!   the user's to state, so it rides the [`edge::GATED_ON`] edge and **reflow2
//!   never supplies a default**. An increment with no stated threshold reports
//!   [`ReadinessVerdict::Ungated`] — never "ready".
//!
//! The measured precedent for that refusal is `Interface.medium`, which
//! defaulted to `REST` and thereby made two silent boundaries "agree" on a value
//! neither had chosen (`ver:seam-incompatibility`, BL-129). A defaulted
//! readiness threshold is the same defect with a bigger blast radius, because it
//! would gate a roadmap rather than a seam.
//!
//! # Why the threshold is on the edge
//!
//! One increment can demand TRL 7 of one technology and TRL 4 of another, and
//! the *same* technology is legitimately demanded at different levels by a
//! demonstrator and by a fielded increment — the row's own worked example
//! (laser satellite refuelling: today's increment versus the ten-year one). A
//! property on either endpoint cannot say that. The edge can, and it is also
//! what lets the answer NAME what decided it.
//!
//! # Why a forecast is not an observation
//!
//! A projected level rides a [`node::TEMPORAL_FACT`] series carrying
//! `basis: forecast`, not a `DimensionObservation`, because `observed_at` says
//! OBSERVED and nobody observed anything in 2035
//! (`dec:readiness-forecast-is-a-temporal-fact`). Confidence on that fact is
//! **stated by the author, never computed here from horizon**: a decay curve is
//! a judgement about risk appetite, and deriving one would assert a risk model
//! nobody chose.

use crate::foundation::core::{DynoError, Value};
use crate::foundation::store::{StoredEdge, StoredNode};
use serde::{Deserialize, Serialize};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// `TemporalFact.fact_type` marking a fact as a readiness assertion.
pub const READINESS_FACT: &str = "readiness";

/// The lowest and highest rung on either ladder. Enforced HERE rather than in
/// the schema: no other `int` property in `schema/*.yaml` declares a range, so
/// enforcement at that layer is unproven, and relying on an unproven check is a
/// silent fallback by another name (AGENTS.md rule 4).
pub const MIN_RUNG: i64 = 1;
/// See [`MIN_RUNG`].
pub const MAX_RUNG: i64 = 9;

/// Which readiness ladder. They are **not** interchangeable: a technology can be
/// demonstrable and unmanufacturable, which is exactly the case a roadmap must
/// be able to state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ReadinessKind {
    /// Technology Readiness Level.
    Trl,
    /// Manufacturing Readiness Level.
    Mrl,
}

impl ReadinessKind {
    /// The schema enum string.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trl => "TRL",
            Self::Mrl => "MRL",
        }
    }

    /// Parse the schema enum string. Returns `None` for anything else — an
    /// unknown ladder is never quietly treated as TRL.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "TRL" => Some(Self::Trl),
            "MRL" => Some(Self::Mrl),
            _ => None,
        }
    }
}

/// What a gate's far end is doing about the level it was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    /// A measured level already meets the demand.
    ClearsNow,
    /// No measured level meets it, but a forecast does, at this epoch.
    ClearsAt {
        /// Epoch id the clearing forecast is `VALID_FROM`.
        epoch_id: String,
        /// That epoch's name, so a report can read as prose.
        epoch_name: String,
        /// That epoch's `sequence` — the ordering key the roadmap maxes over.
        sequence: i64,
    },
    /// Nothing measured and nothing projected ever reaches the demand. NOT the
    /// same as "late": this gate has no path to clearing on the record at all,
    /// and saying so is the honest answer.
    NeverClears,
    /// The far end carries no readiness level on this ladder at all, measured or
    /// forecast. Reported rather than assumed — an unassessed technology is a
    /// question for the user, not a zero.
    Unassessed,
}

/// One `GATED_ON` edge, resolved against what the far end actually carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateFinding {
    /// The enabling technology this increment waits on.
    pub target_id: String,
    /// Which ladder the threshold was stated on.
    pub kind: String,
    /// The rung demanded.
    pub min_level: i64,
    /// The best measured level on record, if any.
    pub current_level: Option<i64>,
    /// What this gate is doing about the demand.
    pub status: GateStatus,
    /// Author-stated confidence on the clearing forecast, if it had one. Never
    /// computed here.
    pub confidence: Option<f64>,
    /// Why this increment demands this rung, if the edge said.
    pub rationale: Option<String>,
    /// The sentence a reader needs — the whole point of deriving rather than
    /// declaring a date.
    pub explanation: String,
}

/// The verdict for one increment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessVerdict {
    /// No `GATED_ON` edge states a threshold, so there is nothing to compute.
    /// **Deliberately not "ready".** Silence about a gate is not evidence there
    /// is none, and reporting an unstated threshold as satisfied is how a
    /// roadmap becomes fiction.
    Ungated,
    /// Every gate is met by a measured level today.
    AchievableNow,
    /// The earliest epoch by which every gate clears — the max over the per-gate
    /// clearing epochs, because the increment waits for its slowest dependency.
    GatedUntil {
        /// The deciding epoch.
        epoch_id: String,
        /// Its name.
        epoch_name: String,
        /// Its ordering key.
        sequence: i64,
    },
    /// At least one gate has no measured level and no clearing forecast, so no
    /// date can be derived. Reported loudly instead of being dropped from the
    /// max, which would silently return an optimistic date.
    Indeterminate,
}

/// One observed readiness level, for [`DesignGraph::add_readiness`].
///
/// A struct rather than seven positional arguments, matching the `*Options`
/// idiom already used across this crate: at a call site
/// `ReadinessKind::Trl, 7, None, None` is four values whose meaning is carried
/// entirely by position, and a roadmap is not a good place to transpose two.
#[derive(Debug, Clone)]
pub struct ReadinessObservation<'a> {
    /// Id for the new assessment node.
    pub id: &'a str,
    /// Type of the enabling technology.
    pub target_type: &'a str,
    /// Id of the enabling technology.
    pub target_id: &'a str,
    /// Which ladder.
    pub kind: ReadinessKind,
    /// The rung, 1-9.
    pub level: i64,
    /// Cited support — what was demonstrated, where.
    pub evidence: Option<&'a str>,
    /// When it was observed (reflow2 takes no clock).
    pub assessed_at: Option<&'a str>,
}

/// One stated threshold, for [`DesignGraph::gate_on`].
#[derive(Debug, Clone)]
pub struct ReadinessGate<'a> {
    /// Type of the increment being gated.
    pub subject_type: &'a str,
    /// Id of the increment being gated.
    pub subject_id: &'a str,
    /// Type of the enabling technology it waits on.
    pub target_type: &'a str,
    /// Id of the enabling technology it waits on.
    pub target_id: &'a str,
    /// Which ladder the threshold is stated on.
    pub kind: ReadinessKind,
    /// The rung demanded, 1-9. Never defaulted.
    pub min_level: i64,
    /// Why this increment demands this rung.
    pub rationale: Option<&'a str>,
}

/// One projected level, for [`DesignGraph::forecast_readiness`].
#[derive(Debug, Clone)]
pub struct ReadinessForecast<'a> {
    /// Id for the new `TemporalFact`.
    pub id: &'a str,
    /// Type of the enabling technology.
    pub target_type: &'a str,
    /// Id of the enabling technology.
    pub target_id: &'a str,
    /// Which ladder.
    pub kind: ReadinessKind,
    /// The rung expected, 1-9.
    pub level: i64,
    /// The epoch the projection is `VALID_FROM`.
    pub epoch_id: &'a str,
    /// The AUTHOR's confidence. Never computed here from horizon.
    pub confidence: Option<f64>,
    /// Optional prose; a default is derived when absent.
    pub statement: Option<&'a str>,
}

/// A forecast that clears a gate, with the epoch it clears at.
#[derive(Debug, Clone)]
struct Clearing {
    epoch_id: String,
    epoch_name: String,
    sequence: i64,
    level: i64,
    confidence: Option<f64>,
}

/// The derived roadmap answer for one increment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// The increment asked about.
    pub subject_id: String,
    /// The computed verdict.
    pub verdict: ReadinessVerdict,
    /// Every gate, resolved.
    pub gates: Vec<GateFinding>,
    /// The gate that decided the verdict — the slowest one — if a date was
    /// derived, or the first blocking gate if the answer is indeterminate.
    pub deciding_target_id: Option<String>,
    /// One-line summary naming the reason, ready to read back to a user.
    pub summary: String,
}

impl DesignGraph {
    /// Record an OBSERVED readiness level for an enabling technology.
    ///
    /// Refuses a rung outside 1-9 loudly rather than clamping: a clamped 12 that
    /// silently becomes 9 would report a technology as mature.
    ///
    /// # Errors
    /// [`DynoError::Validation`] if `level` is outside 1-9, or if the target
    /// does not exist; storage errors propagate.
    pub fn add_readiness(
        &mut self,
        obs: &ReadinessObservation<'_>,
    ) -> Result<StoredNode, DynoError> {
        check_rung(node::READINESS_ASSESSMENT, "level", obs.level)?;
        if self.get_node(obs.target_type, obs.target_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: obs.target_type.to_string(),
                node_id: obs.target_id.to_string(),
            });
        }
        let stored = self.upsert_node(
            node::READINESS_ASSESSMENT,
            obs.id,
            Props::new()
                .set("target_id", obs.target_id)
                .set("target_type", obs.target_type)
                .set("kind", obs.kind.as_str())
                .set("level", obs.level)
                .set_opt("evidence", obs.evidence)
                .set_opt("assessed_at", obs.assessed_at),
        )?;
        self.create_edge(
            edge::HAS_READINESS,
            obs.target_type,
            obs.target_id,
            node::READINESS_ASSESSMENT,
            obs.id,
            Props::new(),
        )?;
        Ok(stored)
    }

    /// State that `subject` cannot deliver until `target` reaches `min_level`.
    ///
    /// The threshold is required and never defaulted — that refusal is the
    /// decision this whole module rests on.
    ///
    /// # Errors
    /// [`DynoError::EdgeValidation`] if `min_level` is outside 1-9; endpoint
    /// errors propagate from [`DesignGraph::create_edge`].
    pub fn gate_on(&mut self, gate: &ReadinessGate<'_>) -> Result<StoredEdge, DynoError> {
        if !(MIN_RUNG..=MAX_RUNG).contains(&gate.min_level) {
            let got = gate.min_level;
            return Err(DynoError::EdgeValidation {
                edge_type: edge::GATED_ON.to_string(),
                property: "min_level".to_string(),
                message: format!(
                    "readiness runs {MIN_RUNG}-{MAX_RUNG}; got {got}. The threshold is \
                     the user's judgement and is never defaulted or clamped."
                ),
            });
        }
        self.create_edge(
            edge::GATED_ON,
            gate.subject_type,
            gate.subject_id,
            gate.target_type,
            gate.target_id,
            Props::new()
                .set("kind", gate.kind.as_str())
                .set("min_level", gate.min_level)
                .set_opt("rationale", gate.rationale),
        )
    }

    /// Record a PROJECTED readiness level, valid from `epoch`.
    ///
    /// A `TemporalFact` carrying `basis: forecast`, not an observation — see the
    /// module docs. `confidence` is the author's; nothing here derives one.
    ///
    /// # Errors
    /// [`DynoError::Validation`] if `level` is outside 1-9; missing target or
    /// epoch propagate as [`DynoError::NodeNotFound`].
    pub fn forecast_readiness(
        &mut self,
        fc: &ReadinessForecast<'_>,
    ) -> Result<StoredNode, DynoError> {
        check_rung(node::TEMPORAL_FACT, "level", fc.level)?;
        if self.get_node(fc.target_type, fc.target_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: fc.target_type.to_string(),
                node_id: fc.target_id.to_string(),
            });
        }
        if self.get_node(node::DESIGN_EPOCH, fc.epoch_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: node::DESIGN_EPOCH.to_string(),
                node_id: fc.epoch_id.to_string(),
            });
        }
        let (kind, level, target_id) = (fc.kind.as_str(), fc.level, fc.target_id);
        let value = format!(r#"{{"kind":"{kind}","level":{level}}}"#);
        let default_statement = format!("{target_id} reaches {kind} {level}");
        let stored = self.create_node(
            node::TEMPORAL_FACT,
            fc.id,
            Props::new()
                .set("subject_id", fc.target_id)
                .set("fact_type", READINESS_FACT)
                .set("statement", fc.statement.unwrap_or(&default_statement))
                .set("value", value.as_str())
                .set("basis", "forecast")
                .set_opt("confidence", fc.confidence),
        )?;
        self.create_edge(
            edge::HAS_TEMPORAL_FACT,
            fc.target_type,
            fc.target_id,
            node::TEMPORAL_FACT,
            fc.id,
            Props::new(),
        )?;
        self.create_edge(
            edge::VALID_FROM,
            node::TEMPORAL_FACT,
            fc.id,
            node::DESIGN_EPOCH,
            fc.epoch_id,
            Props::new(),
        )?;
        Ok(stored)
    }

    /// The derived roadmap for one increment: the earliest epoch by which every
    /// technology it is gated on clears the level demanded of it, with the
    /// reason named.
    ///
    /// The answer is the **max** over the per-gate clearing epochs, because an
    /// increment waits for its slowest dependency. A gate with no path to
    /// clearing makes the whole answer [`ReadinessVerdict::Indeterminate`]
    /// rather than dropping out of the max — dropping it would return an
    /// optimistic date built by ignoring the inconvenient half of the evidence.
    ///
    /// # Errors
    /// Storage errors propagate.
    pub fn readiness_report(&self, subject_id: &str) -> Result<ReadinessReport, DynoError> {
        let gate_edges = self.outgoing(subject_id, Some(edge::GATED_ON))?;
        if gate_edges.is_empty() {
            return Ok(ReadinessReport {
                subject_id: subject_id.to_string(),
                verdict: ReadinessVerdict::Ungated,
                gates: Vec::new(),
                deciding_target_id: None,
                summary: format!(
                    "{subject_id} states no readiness threshold, so no delivery epoch can be \
                     derived. This is reported as UNGATED and never as ready: reflow2 does not \
                     supply a default threshold, because \"below level N is not buildable\" is a \
                     judgement about risk appetite and it is yours to state."
                ),
            });
        }

        let mut gates = Vec::new();
        for e in gate_edges {
            gates.push(self.resolve_gate(&e)?);
        }
        gates.sort_by(|a, b| a.target_id.cmp(&b.target_id));

        // Indeterminate wins over any date: one unknowable gate makes the whole
        // answer unknowable, and the first such gate is the one to name.
        if let Some(blocking) = gates
            .iter()
            .find(|g| matches!(g.status, GateStatus::Unassessed | GateStatus::NeverClears))
        {
            let summary = format!(
                "No delivery epoch can be derived for {subject_id}: {}",
                blocking.explanation
            );
            let deciding = blocking.target_id.clone();
            return Ok(ReadinessReport {
                subject_id: subject_id.to_string(),
                verdict: ReadinessVerdict::Indeterminate,
                gates,
                deciding_target_id: Some(deciding),
                summary,
            });
        }

        // The slowest gate decides. Ties break on target id so the answer is
        // deterministic run to run.
        let slowest = gates
            .iter()
            .filter_map(|g| match &g.status {
                GateStatus::ClearsAt {
                    epoch_id,
                    epoch_name,
                    sequence,
                } => Some((*sequence, epoch_id.clone(), epoch_name.clone(), g)),
                GateStatus::ClearsNow => None,
                _ => None,
            })
            .max_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.3.target_id.cmp(&b.3.target_id))
            });

        match slowest {
            None => {
                let summary = format!(
                    "{subject_id} is achievable now: every one of its {} readiness gate(s) is \
                     met by a measured level today.",
                    gates.len()
                );
                Ok(ReadinessReport {
                    subject_id: subject_id.to_string(),
                    verdict: ReadinessVerdict::AchievableNow,
                    gates,
                    deciding_target_id: None,
                    summary,
                })
            }
            Some((sequence, epoch_id, epoch_name, deciding)) => {
                let summary = format!(
                    "{subject_id} cannot deliver before {epoch_name} ({epoch_id}), because {}",
                    deciding.explanation
                );
                let deciding_id = deciding.target_id.clone();
                Ok(ReadinessReport {
                    subject_id: subject_id.to_string(),
                    verdict: ReadinessVerdict::GatedUntil {
                        epoch_id,
                        epoch_name,
                        sequence,
                    },
                    gates,
                    deciding_target_id: Some(deciding_id),
                    summary,
                })
            }
        }
    }

    /// Resolve one `GATED_ON` edge against what its far end carries.
    fn resolve_gate(&self, e: &StoredEdge) -> Result<GateFinding, DynoError> {
        let kind_str = prop_str(&e.properties, "kind").unwrap_or_default();
        let min_level = prop_i64(&e.properties, "min_level").unwrap_or(MAX_RUNG);
        let rationale = prop_str(&e.properties, "rationale");
        let target_id = e.to_id.clone();

        let current_level = self.best_measured_level(&target_id, &kind_str)?;

        if current_level.is_some_and(|l| l >= min_level) {
            let level = current_level.unwrap_or_default();
            return Ok(GateFinding {
                explanation: format!(
                    "{target_id} is {kind_str} {level} today and this increment needs \
                     {kind_str} {min_level}"
                ),
                target_id,
                kind: kind_str,
                min_level,
                current_level,
                status: GateStatus::ClearsNow,
                confidence: None,
                rationale,
            });
        }

        let clearing = self.earliest_clearing_forecast(&target_id, &kind_str, min_level)?;
        match clearing {
            Some(c) => {
                let (epoch_id, epoch_name, sequence, level, confidence) =
                    (c.epoch_id, c.epoch_name, c.sequence, c.level, c.confidence);
                let today = match current_level {
                    Some(l) => format!("{target_id} is {kind_str} {l} today"),
                    None => format!("{target_id} has no measured {kind_str} today"),
                };
                let conf = match confidence {
                    Some(c) => format!(" (author-stated confidence {c})"),
                    None => String::new(),
                };
                Ok(GateFinding {
                    explanation: format!(
                        "{today}, is projected {kind_str} {level} at {epoch_name}{conf}, and \
                         this increment needs {kind_str} {min_level}"
                    ),
                    target_id,
                    kind: kind_str,
                    min_level,
                    current_level,
                    status: GateStatus::ClearsAt {
                        epoch_id,
                        epoch_name,
                        sequence,
                    },
                    confidence,
                    rationale,
                })
            }
            None if current_level.is_none() => Ok(GateFinding {
                explanation: format!(
                    "{target_id} carries no {kind_str} at all, measured or forecast, and this \
                     increment needs {kind_str} {min_level} — so nothing here can say when it \
                     delivers. Record a level for it, or drop the gate."
                ),
                target_id,
                kind: kind_str,
                min_level,
                current_level,
                status: GateStatus::Unassessed,
                confidence: None,
                rationale,
            }),
            None => {
                let level = current_level.unwrap_or_default();
                Ok(GateFinding {
                    explanation: format!(
                        "{target_id} is {kind_str} {level} today and no forecast on record ever \
                         reaches the {kind_str} {min_level} this increment needs"
                    ),
                    target_id,
                    kind: kind_str,
                    min_level,
                    current_level,
                    status: GateStatus::NeverClears,
                    confidence: None,
                    rationale,
                })
            }
        }
    }

    /// The highest MEASURED level on record for one technology on one ladder.
    ///
    /// The highest rather than the most recent, deliberately: readiness is
    /// ratcheted evidence — a demonstration at TRL 7 is not undone by someone
    /// later recording a TRL 4 observation of an earlier stage — and picking by
    /// date would make the answer depend on how completely the history was
    /// backfilled.
    fn best_measured_level(&self, target_id: &str, kind: &str) -> Result<Option<i64>, DynoError> {
        let mut best: Option<i64> = None;
        for e in self.outgoing(target_id, Some(edge::HAS_READINESS))? {
            let Some(n) = self.get_node(node::READINESS_ASSESSMENT, &e.to_id)? else {
                continue;
            };
            if prop_str(&n.properties, "kind").as_deref() != Some(kind) {
                continue;
            }
            if let Some(level) = prop_i64(&n.properties, "level") {
                best = Some(best.map_or(level, |b: i64| b.max(level)));
            }
        }
        Ok(best)
    }

    /// The earliest epoch whose forecast reaches `min_level` on this ladder.
    ///
    /// The earliest is the first moment that technology is good enough.
    fn earliest_clearing_forecast(
        &self,
        target_id: &str,
        kind: &str,
        min_level: i64,
    ) -> Result<Option<Clearing>, DynoError> {
        let mut best: Option<Clearing> = None;
        for e in self.outgoing(target_id, Some(edge::HAS_TEMPORAL_FACT))? {
            let Some(fact) = self.get_node(node::TEMPORAL_FACT, &e.to_id)? else {
                continue;
            };
            if prop_str(&fact.properties, "fact_type").as_deref() != Some(READINESS_FACT) {
                continue;
            }
            // Only a forecast projects forward. A `measured` fact about the past
            // is not evidence about an epoch that has not happened.
            if prop_str(&fact.properties, "basis").as_deref() != Some("forecast") {
                continue;
            }
            let Some((fact_kind, level)) =
                parse_readiness_value(prop_str(&fact.properties, "value").as_deref())
            else {
                continue;
            };
            if fact_kind != kind || level < min_level {
                continue;
            }
            let confidence = fact.properties.get("confidence").and_then(Value::as_f64);
            for ve in self.outgoing(&fact.node_id, Some(edge::VALID_FROM))? {
                let Some(epoch) = self.get_node(node::DESIGN_EPOCH, &ve.to_id)? else {
                    continue;
                };
                let sequence = prop_i64(&epoch.properties, "sequence").unwrap_or(i64::MAX);
                let name = prop_str(&epoch.properties, "name").unwrap_or_else(|| ve.to_id.clone());
                let candidate = Clearing {
                    epoch_id: ve.to_id.clone(),
                    epoch_name: name,
                    sequence,
                    level,
                    confidence,
                };
                best = Some(match best {
                    // Earliest epoch wins; ties break on epoch id for determinism.
                    Some(cur)
                        if (cur.sequence, cur.epoch_id.as_str())
                            <= (candidate.sequence, candidate.epoch_id.as_str()) =>
                    {
                        cur
                    }
                    _ => candidate,
                });
            }
        }
        Ok(best)
    }
}

/// Refuse a rung outside 1-9, loudly.
fn check_rung(node_type: &str, property: &str, level: i64) -> Result<(), DynoError> {
    if (MIN_RUNG..=MAX_RUNG).contains(&level) {
        return Ok(());
    }
    Err(DynoError::Validation {
        node_type: node_type.to_string(),
        property: property.to_string(),
        message: format!(
            "readiness runs {MIN_RUNG}-{MAX_RUNG}; got {level}. Refused rather than clamped — a \
             clamped 12 silently becomes 9 and reports a technology as mature."
        ),
    })
}

/// `{"kind":"TRL","level":7}` → `("TRL", 7)`. Deliberately tolerant of nothing:
/// an unparseable value is skipped, never guessed at.
fn parse_readiness_value(raw: Option<&str>) -> Option<(String, i64)> {
    let parsed: serde_json::Value = serde_json::from_str(raw?).ok()?;
    let kind = parsed.get("kind")?.as_str()?.to_string();
    let level = parsed.get("level")?.as_i64()?;
    Some((kind, level))
}

fn prop_str(props: &std::collections::HashMap<String, Value>, key: &str) -> Option<String> {
    props.get(key).and_then(Value::as_str).map(str::to_string)
}

fn prop_i64(props: &std::collections::HashMap<String, Value>, key: &str) -> Option<i64> {
    props.get(key).and_then(Value::as_i64)
}
