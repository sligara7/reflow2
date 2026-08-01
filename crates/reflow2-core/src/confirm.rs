//! The confirmation ledger — when was each design claim last checked against
//! reality, and what was the answer? (BL-35)
//!
//! The founding observation, from the erosion trials: an eroded design and a
//! genuinely coherent one both reported *quiet*. Structural completeness — is
//! there a Capability, does an Artifact realize it — was all that was measured,
//! and it is true in both graphs. The missing concept is **confirmation**:
//! whether anyone has checked the claim against reality, and what they said.
//!
//! Everything here is read off axis Z, from records the loop now writes:
//! `DriftEvent`s (one per divergence, `resolved` flipped by the accept that
//! answered it — BL-33) and accept `ChangeEvent`s (one per baseline accept,
//! carrying the disposition claim). The ledger computes; it never guesses. In
//! particular it does **not** try to detect a lying `design_holds` claim —
//! that is a semantic judgement no deterministic core can make. What it makes
//! impossible is the state the original reflow died in: *nobody looked, and
//! nothing could tell.*
//!
//! A **signal, not a gap** (the BL-23 lesson), with one exception: an
//! *unresolved* drift — a recorded divergence whose second question was never
//! answered — is a true, per-node, actionable gap and DETECT raises it
//! (`unresolved_drift` in `detect.rs`).

use dynograph_core::DynoError;

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// How a capability's claim currently stands against reality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationState {
    /// Reality diverged and the second question is unanswered: at least one
    /// `DriftEvent` on a realizing artifact is unresolved. The actionable state.
    Drifting,
    /// The claim has been examined: drift was observed and every occurrence was
    /// answered with a recorded disposition. Read the ledger's counts to see
    /// *how* it was answered — five `design_holds` claims with zero design
    /// edits is a very different confirmation history from one `design_updated`
    /// that moved the capability.
    Confirmed,
    /// **Nobody has ever looked.** Artifacts exist and carry baselines, but no
    /// reconcile has recorded a divergence and no accept has recorded a claim.
    /// Not the same as Confirmed, and the whole point of this ledger is that
    /// the two are no longer indistinguishable.
    Unexamined,
}

/// Whether a capability's newest check is older than the newest accepted change
/// to the code it covers (BL-106, the TIME axis of the evidence-quality family).
///
/// A **fact, never a gap** (`dec:verification-freshness-not-a-gap`): a
/// stale-looking check is a standing property of a claim rather than an event,
/// it would fire on every legitimate refactor as readily as on a real hole, and
/// an open list that can never reach zero gets skimmed — which is the failure
/// the gap workflow exists to prevent (BL-23: when a detector punishes correct
/// work, the answer is a different question, not a tuned threshold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationFreshness {
    /// The newest passing check ran no earlier than the newest accepted
    /// baseline change. The evidence keeps up with the code.
    Current,
    /// An accept is dated AFTER the newest passing check: the code moved and
    /// nothing re-checked it. The state BL-105 was in while every gate read
    /// green — `cap:degraded-surface` was `verified` on a check that drove
    /// stdio only, while `art:main` drifted twice underneath it.
    Stale,
    /// Either side is undated, so the question cannot be answered. Reported
    /// explicitly and never as a pass: an unanswerable freshness question
    /// presented as freshness is the same lie in a new place.
    Unknown,
}

/// One capability's confirmation history, computed from axis Z.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClaimConfirmation {
    pub capability_id: String,
    pub capability_name: String,
    pub state: ConfirmationState,
    /// Realizing artifacts considered (direct `REALIZES`, or via an allocated
    /// component — both P3 shapes, per BL-38).
    pub artifacts: Vec<String>,
    /// Divergences ever recorded against those artifacts.
    pub drift_events: usize,
    /// …of which the second question is still unanswered.
    pub unresolved_drift_events: usize,
    /// Baseline accepts claiming "the change carried no design meaning".
    pub design_holds_claims: usize,
    /// Baseline accepts tied to a design-side edit (the event also `CHANGED`
    /// a design node).
    pub design_updated_claims: usize,
    /// `ChangeEvent`s that `CHANGED` this capability itself — the design
    /// moving on the record.
    pub design_edits: usize,
    /// `detected_at` of the newest dated accept claim, when any accept is
    /// dated. Reported as-is; the core takes no clock and does not compare
    /// undated events.
    pub last_claim_at: Option<String>,
    /// `last_run_at` of the newest dated PASSING check on this capability
    /// (BL-106). The value `verify.rs` has written on every status set since
    /// the beginning and nothing in the core has ever read — the same shape as
    /// the temporal axis before BL-70 and the whole inviolable-intent
    /// vocabulary before BL-96.
    pub last_verified_at: Option<String>,
    /// [`last_verified_at`](Self::last_verified_at) against
    /// [`last_claim_at`](Self::last_claim_at) — is the check older than the
    /// code it covers?
    pub verification_freshness: VerificationFreshness,
}

/// The whole ledger plus its rollup counts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfirmationLedger {
    pub claims: Vec<ClaimConfirmation>,
    pub drifting: usize,
    pub confirmed: usize,
    pub unexamined: usize,
    /// Claims whose newest passing check predates the newest accepted change to
    /// what it covers (BL-106). A count, beside the three states — not a
    /// fourth state, because freshness is orthogonal to whether anyone looked.
    pub stale_verification: usize,
    /// Claims where one side or the other carries no date, so the question
    /// cannot be answered. Counted so that "0 stale" can never be read as
    /// "everything is current".
    pub unknown_verification_freshness: usize,
}

/// Compare a check's run date against an accept's date, on the only ground the
/// core can stand on: the **calendar-date prefix**.
///
/// FOUND BY DOGFOODING THIS ON REFLOW2'S OWN GRAPH, and it is the same class of
/// error the family exists to catch. The first version compared the two strings
/// whole, which is right while both are `YYYY-MM-DD` and wrong the moment they
/// disagree in precision: `cap:latent-surface` was checked on `2026-07-28` and
/// accepted at `2026-07-28T14:52:00-04:00`, and a lexical compare called that
/// STALE because the shorter string sorts first. The check may well have run
/// after the accept that same day — nothing in the graph says.
///
/// So: if the dates differ, they order and the answer is real. If they name the
/// SAME DAY, the answer is `Unknown`, because deciding it would need the two
/// timestamps parsed and normalised across UTC offsets, and the core takes no
/// clock and parses no dates (`dec:verification-freshness-not-a-gap`'s three
/// constraints). Reporting `Stale` there asserts an ordering nobody recorded;
/// reporting `Current` is the flattering default this whole family is against.
fn freshness_of(ran: &str, accepted: &str) -> VerificationFreshness {
    // Byte slicing is safe on ASCII-digit dates and simply yields the whole
    // string for anything shorter or oddly shaped — which then compares whole,
    // the previous behaviour, rather than panicking on a malformed value.
    fn day(s: &str) -> &str {
        s.get(..10).unwrap_or(s)
    }
    match day(ran).cmp(day(accepted)) {
        std::cmp::Ordering::Less => VerificationFreshness::Stale,
        std::cmp::Ordering::Greater => VerificationFreshness::Current,
        std::cmp::Ordering::Equal => {
            // Same day. Only claimable when both sides carry the same shape of
            // timestamp — and even then only when they share an offset, which
            // is why anything past a plain date is left Unknown.
            if ran == accepted {
                VerificationFreshness::Current
            } else {
                VerificationFreshness::Unknown
            }
        }
    }
}

impl DesignGraph {
    /// Compute the confirmation ledger — one entry per capability that has
    /// realizing artifacts. Capabilities with no artifacts are absent by
    /// design: "nothing is built yet" is `unrealized_capability`'s question,
    /// not a confirmation question.
    pub fn confirmation_ledger(&self) -> Result<ConfirmationLedger, DynoError> {
        let mut claims = Vec::new();

        for cap in self.scan_nodes(node::CAPABILITY)? {
            // Both P3 shapes (BL-38): files realizing the capability, or files
            // realizing a component it is allocated to.
            let mut artifacts: Vec<String> = self
                .incoming(&cap.node_id, Some(edge::REALIZES))?
                .into_iter()
                .map(|e| e.from_id)
                .collect();
            for alloc in self.outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))? {
                for e in self.incoming(&alloc.to_id, Some(edge::REALIZES))? {
                    artifacts.push(e.from_id);
                }
            }
            artifacts.sort();
            artifacts.dedup();
            if artifacts.is_empty() {
                continue;
            }

            let mut drift_events = 0usize;
            let mut unresolved = 0usize;
            let mut design_holds = 0usize;
            let mut design_updated = 0usize;
            let mut last_claim_at: Option<String> = None;

            for art in &artifacts {
                for e in self.incoming(art, Some(edge::DEPENDS_ON))? {
                    let Some(ev) = self.get_node(node::DRIFT_EVENT, &e.from_id)? else {
                        continue;
                    };
                    drift_events += 1;
                    let resolved = ev
                        .properties
                        .get("resolved")
                        .and_then(dynograph_core::Value::as_bool)
                        .unwrap_or(false);
                    if !resolved {
                        unresolved += 1;
                    }
                }
                for e in self.incoming(art, Some(edge::CHANGED))? {
                    // Only accept claims count; ordinary change history on the
                    // artifact (a record_change) is not a disposition.
                    let is_claim = e
                        .properties
                        .get("accepted_baseline")
                        .and_then(dynograph_core::Value::as_bool)
                        .unwrap_or(false);
                    if !is_claim {
                        continue;
                    }
                    let Some(ev) = self.get_node(node::CHANGE_EVENT, &e.from_id)? else {
                        continue;
                    };
                    // Which kind of claim is this accept? A design-moving event
                    // also CHANGED a non-Artifact design node.
                    let mut moved_design = false;
                    for t in self.outgoing(&ev.node_id, Some(edge::CHANGED))? {
                        if t.to_id != *art && self.get_node(node::ARTIFACT, &t.to_id)?.is_none() {
                            moved_design = true;
                            break;
                        }
                    }
                    if moved_design {
                        design_updated += 1;
                    } else {
                        design_holds += 1;
                    }
                    if let Some(at) = ev
                        .properties
                        .get("detected_at")
                        .and_then(dynograph_core::Value::as_str)
                    {
                        // ISO-8601 strings order lexically; the caller supplies
                        // them (the core takes no clock).
                        if last_claim_at.as_deref().is_none_or(|prev| at > prev) {
                            last_claim_at = Some(at.to_string());
                        }
                    }
                }
            }

            let design_edits = self.incoming(&cap.node_id, Some(edge::CHANGED))?.len();

            // BL-106 · the TIME axis. The newest dated run across this
            // capability's PASSING checks — passing only, because
            // `dec:passing-is-verified` means a failing check is not evidence
            // whose age is worth comparing.
            let mut last_verified_at: Option<String> = None;
            for e in self.incoming(&cap.node_id, Some(edge::VERIFIES))? {
                let Some(v) = self.get_node(node::VERIFICATION, &e.from_id)? else {
                    continue;
                };
                if v.properties
                    .get("status")
                    .and_then(dynograph_core::Value::as_str)
                    != Some("passing")
                {
                    continue;
                }
                if let Some(at) = v
                    .properties
                    .get("last_run_at")
                    .and_then(dynograph_core::Value::as_str)
                {
                    // ISO-8601 orders lexically; the caller supplies it (the
                    // core takes no clock), exactly as last_claim_at above.
                    if last_verified_at.as_deref().is_none_or(|prev| at > prev) {
                        last_verified_at = Some(at.to_string());
                    }
                }
            }

            // Undated on either side is Unknown, never a pass.
            let verification_freshness = match (&last_verified_at, &last_claim_at) {
                (Some(ran), Some(accepted)) => freshness_of(ran, accepted),
                _ => VerificationFreshness::Unknown,
            };

            let state = if unresolved > 0 {
                ConfirmationState::Drifting
            } else if drift_events + design_holds + design_updated + design_edits > 0 {
                ConfirmationState::Confirmed
            } else {
                ConfirmationState::Unexamined
            };

            claims.push(ClaimConfirmation {
                capability_id: cap.node_id.clone(),
                capability_name: cap
                    .properties
                    .get("name")
                    .and_then(dynograph_core::Value::as_str)
                    .unwrap_or(&cap.node_id)
                    .to_string(),
                state,
                artifacts,
                drift_events,
                unresolved_drift_events: unresolved,
                design_holds_claims: design_holds,
                design_updated_claims: design_updated,
                design_edits,
                last_claim_at,
                last_verified_at,
                verification_freshness,
            });
        }

        claims.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let count = |s: ConfirmationState| claims.iter().filter(|c| c.state == s).count();
        let fresh = |f: VerificationFreshness| {
            claims
                .iter()
                .filter(|c| c.verification_freshness == f)
                .count()
        };
        Ok(ConfirmationLedger {
            drifting: count(ConfirmationState::Drifting),
            confirmed: count(ConfirmationState::Confirmed),
            unexamined: count(ConfirmationState::Unexamined),
            stale_verification: fresh(VerificationFreshness::Stale),
            unknown_verification_freshness: fresh(VerificationFreshness::Unknown),
            claims,
        })
    }
}
