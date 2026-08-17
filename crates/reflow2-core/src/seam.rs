//! SEAM — say when two linked designs disagree at a boundary
//! (`req:seam-incompatibility`).
//!
//! ## Why the ordinary detectors cannot do this
//!
//! Measured, not argued (trial against dynograph-foundation, 2026-07-28).
//! `compose_and_analyse` put both designs in one graph and ran the whole
//! detector suite over it. Before the seam was drawn: one loud
//! `unthreaded_cluster`, correctly reporting two designs side by side with
//! nothing joining them. After seven `CONSUMES` edges were drawn by hand: the
//! disconnection vanished and the seam produced **zero findings of any kind**.
//!
//! That zero is the specification for this module. The existing detectors reason
//! about STRUCTURE — missing intent, orphans, cycles, single points of failure —
//! and a well-formed seam trips none of them. A contract mismatch is a
//! comparison of PROPERTIES ACROSS A PAIR of interfaces, and no ordinary
//! detector compares two nodes to each other. The silence was not the absence of
//! problems; it was the absence of anyone looking.
//!
//! ## `unspecified` is not agreement
//!
//! The rule both sides of the trial arrived at independently, from opposite
//! directions. If `unspecified` may match `unspecified` and be called
//! compatible, the false green is rebuilt with extra steps — and a false green
//! is worse than a conflict, because it is indistinguishable from having
//! checked. So an axis nobody stated is reported as **unstated**, never as
//! agreed, and it is counted separately so a caller can never read "0
//! incompatibilities" as "compatible".
//!
//! ## What this deliberately does NOT check
//!
//! `con:pairing-stops-at-the-boundary`. Every axis here is a property OF A
//! BOUNDARY. A type that CROSSES a boundary is part of the contract too, and
//! nothing here can see it: dynograph's `search_fulltext` returned a
//! `dynograph_text::TextHit` through the `storage-api` boundary, and reflow2
//! read all three of its fields while naming neither the crate nor the text
//! boundary. Every axis below would have passed that seam cleanly.
//!
//! So the report SAYS what it did not examine. A check that stays quiet about
//! its own blind spot is how a clean result becomes a lie.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::DynoError;
use dynograph_core::Value;
use dynograph_resolution::token_sort_ratio;

use crate::export::GraphExport;
use crate::graph::DesignGraph;
use crate::nodes::node;

/// One axis on which two paired boundaries were compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// How the two connect at all — `library` against `REST` cannot be wired
    /// together however well everything else matches.
    Medium,
    /// Request-response against event-driven: a consumer written for one cannot
    /// be pointed at the other.
    Paradigm,
    /// protobuf against JSON fails immediately and totally.
    PayloadFormat,
    /// Who you are. The consumer that assumed `none` gets 401s in production.
    Auth,
    /// Who can read it. A provider offering `none` against a consumer requiring
    /// `tls` is a refusal to connect, not a degradation.
    TransportSecurity,
    /// Free text: which actions are permitted.
    Operations,
    /// Free text: how failures are signalled.
    ErrorModel,
    /// Free text: where the field-level contract lives.
    PayloadSchema,
}

impl Axis {
    fn property(self) -> &'static str {
        match self {
            Axis::Medium => "medium",
            Axis::Paradigm => "paradigm",
            Axis::PayloadFormat => "payload_format",
            Axis::Auth => "auth",
            Axis::TransportSecurity => "transport_security",
            Axis::Operations => "operations",
            Axis::ErrorModel => "error_model",
            Axis::PayloadSchema => "payload_schema",
        }
    }

    /// Whether the axis is a closed vocabulary. Free-text axes can be reported
    /// as *differing*, never as *incompatible* — a machine cannot tell a real
    /// mismatch from two people describing the same thing in different words.
    fn is_enum(self) -> bool {
        matches!(
            self,
            Axis::Medium
                | Axis::Paradigm
                | Axis::PayloadFormat
                | Axis::Auth
                | Axis::TransportSecurity
        )
    }

    /// Every axis worth comparing, hardest failure first.
    fn all() -> [Axis; 8] {
        [
            Axis::Medium,
            Axis::Paradigm,
            Axis::PayloadFormat,
            Axis::Auth,
            Axis::TransportSecurity,
            Axis::Operations,
            Axis::ErrorModel,
            Axis::PayloadSchema,
        ]
    }
}

/// What the comparison found on one axis of one pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Both sides stated it and they are the same.
    Agreed,
    /// Both sides stated it, from a closed vocabulary, and they differ. The only
    /// verdict that is a genuine incompatibility.
    Incompatible,
    /// Both sides stated it in free text and the text differs — a person must
    /// read it. NOT called incompatible: two descriptions of the same contract
    /// legitimately differ in wording.
    Differs,
    /// One side or neither said anything. **Not agreement.**
    Unstated,
}

/// One axis of one pair, with what each side said.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeamFinding {
    pub axis: Axis,
    pub verdict: Verdict,
    /// The boundary in this design.
    pub ours: String,
    /// The boundary in the other design.
    pub theirs: String,
    pub our_value: Option<String>,
    pub their_value: Option<String>,
    /// Why it matters, in the terms the schema already uses.
    pub detail: String,
}

/// The result of checking a set of paired boundaries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeamReport {
    /// Pairs that were compared, as given.
    pub pairs_checked: usize,
    /// Pairs named in the request whose ids could not be found, with the side
    /// that was missing. Never silently skipped — a pair that was not compared
    /// must not be mistaken for one that agreed.
    pub unresolved_pairs: Vec<String>,
    /// Genuine incompatibilities: both sides stated a closed-vocabulary value
    /// and they differ.
    pub incompatible: Vec<SeamFinding>,
    /// Free-text axes stated on both sides with differing text — for a person.
    pub differs: Vec<SeamFinding>,
    /// Axes nobody stated. **Not agreement**, and counted so a caller cannot
    /// read "0 incompatibilities" as "compatible".
    pub unstated: Vec<SeamFinding>,
    /// Axes both sides stated identically.
    pub agreed: usize,
    /// What this check cannot see, said out loud
    /// (`con:pairing-stops-at-the-boundary`).
    pub not_examined: String,
    pub note: String,
}

fn prop(props: &[(String, Value)], key: &str) -> Option<String> {
    props
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty() && s != "unspecified")
}

fn why(axis: Axis, ours: Option<&str>, theirs: Option<&str>) -> String {
    let (o, t) = (ours.unwrap_or("unstated"), theirs.unwrap_or("unstated"));
    match axis {
        Axis::Medium => format!(
            "we need `{o}`, they publish `{t}` — these cannot be connected the way either side is \
             built, however well the rest matches"
        ),
        Axis::Paradigm => format!(
            "we expect `{o}`, they publish `{t}` — a consumer written for one cannot simply be \
             pointed at the other"
        ),
        Axis::PayloadFormat => format!(
            "we expect `{o}`, they send `{t}` — this fails immediately and totally, not gradually"
        ),
        Axis::Auth => format!(
            "we assume `{o}`, they require `{t}` — the side that assumed less finds out in \
             production"
        ),
        Axis::TransportSecurity => format!(
            "we require `{o}`, they offer `{t}` — a refusal to connect rather than a degradation"
        ),
        _ => format!("ours says `{o}`, theirs says `{t}` — free text, so a person must read both"),
    }
}

impl DesignGraph {
    /// Compare paired boundaries across a seam (`req:seam-incompatibility`).
    ///
    /// `pairs` is `(our interface id, their interface id)`, with their ids as
    /// they appear in `other` — **not** namespaced. Pairing is supplied rather
    /// than computed because the subscribe side is not declarable yet; when
    /// `req:complementary-pairing` lands it will produce these pairs instead of
    /// a person asserting them. Until then the caller's assertion is exactly as
    /// good as the caller, and the trial showed that is not very good: one of
    /// seven hand-drawn edges was wrong, and the counterparty found it.
    pub fn seam_report(
        &self,
        other: &GraphExport,
        pairs: &[(String, String)],
    ) -> Result<SeamReport, DynoError> {
        let ours = self.export_graph()?;
        let find = |doc: &GraphExport, id: &str| -> Option<Vec<(String, Value)>> {
            doc.nodes
                .iter()
                .find(|n| n.node_id == id && n.node_type == node::INTERFACE)
                .map(|n| n.properties.clone().into_iter().collect())
        };

        let mut incompatible = Vec::new();
        let mut differs = Vec::new();
        let mut unstated = Vec::new();
        let mut agreed = 0usize;
        let mut unresolved = Vec::new();
        let mut checked = 0usize;

        for (our_id, their_id) in pairs {
            let (Some(op), Some(tp)) = (find(&ours, our_id), find(other, their_id)) else {
                // Say WHICH side is missing. "Pair not found" sends someone
                // looking in the wrong design.
                let missing = match (
                    find(&ours, our_id).is_some(),
                    find(other, their_id).is_some(),
                ) {
                    (false, true) => format!("'{our_id}' is not an Interface in THIS design"),
                    (true, false) => {
                        format!("'{their_id}' is not an Interface in the OTHER design")
                    }
                    _ => format!("neither '{our_id}' nor '{their_id}' is an Interface"),
                };
                unresolved.push(missing);
                continue;
            };
            checked += 1;

            for axis in Axis::all() {
                let o = prop(&op, axis.property());
                let t = prop(&tp, axis.property());
                let finding = |verdict| SeamFinding {
                    axis,
                    verdict,
                    ours: our_id.clone(),
                    theirs: their_id.clone(),
                    our_value: o.clone(),
                    their_value: t.clone(),
                    detail: why(axis, o.as_deref(), t.as_deref()),
                };
                match (&o, &t) {
                    (Some(a), Some(b)) if a == b => agreed += 1,
                    (Some(_), Some(_)) if axis.is_enum() => {
                        incompatible.push(finding(Verdict::Incompatible))
                    }
                    (Some(_), Some(_)) => differs.push(finding(Verdict::Differs)),
                    _ => unstated.push(finding(Verdict::Unstated)),
                }
            }
        }

        let note = if checked == 0 {
            "NOTHING WAS COMPARED. No pair resolved to two Interfaces, so this result says nothing \
             about compatibility — it is not a clean bill of health."
                .to_string()
        } else if incompatible.is_empty() && differs.is_empty() {
            format!(
                "{checked} pair(s) checked: no stated value conflicts. {} axis(es) agreed, {} were \
                 stated by NOBODY — an unstated axis is not an agreed one, and this is not a claim \
                 of compatibility.",
                agreed,
                unstated.len()
            )
        } else {
            format!(
                "{checked} pair(s) checked: {} incompatibility(ies), {} free-text difference(s) \
                 for a person to read, {} axis(es) nobody stated, {} agreed.",
                incompatible.len(),
                differs.len(),
                unstated.len(),
                agreed
            )
        };

        Ok(SeamReport {
            pairs_checked: checked,
            unresolved_pairs: unresolved,
            incompatible,
            differs,
            unstated,
            agreed,
            not_examined:
                "The TYPES that cross these boundaries. Every axis here is a property OF a \
                 boundary; a struct or message that travels through one is part of the contract \
                 and is invisible to this check. A real case: a provider's storage API returned a \
                 type owned by its text crate, and the consumer read all three fields while naming \
                 neither — every axis above would have passed that seam cleanly \
                 (con:pairing-stops-at-the-boundary)."
                    .to_string(),
            note,
        })
    }
}

// ===========================================================================
// PAIRING — compute the seam instead of asserting it (`req:complementary-pairing`)
// ===========================================================================

/// One matched strand: our boundary and theirs, and how confident the name match
/// was.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PairedBoundary {
    /// Our interface id.
    pub ours: String,
    /// Their interface id, as it appears in the other document.
    pub theirs: String,
    /// Which side offers. `ours` when we publish and they require.
    pub offered_by: String,
    /// 0–100 name similarity. 100 is an exact name match, never an assertion
    /// that the contracts agree — that is what `seam_report` is for.
    pub name_score: u32,
    /// The base axes both sides stated identically, which is why they paired.
    pub agreed_on: Vec<String>,
}

/// A pair whose names matched but whose base axes cannot connect.
///
/// REPORTED, NEVER DROPPED. "You publish this, I need this, and we cannot be
/// connected the way either of us is built" is the most useful finding pairing
/// produces, and treating it as a non-match would hide it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PairingConflict {
    pub ours: String,
    pub theirs: String,
    /// EVERY axis that refused, not just the first. Reporting one at a time
    /// would send someone to fix `transport_security`, redeploy, and only then
    /// discover `auth` also refuses — the trial's public probe differs on both.
    pub refusals: Vec<AxisRefusal>,
    pub name_score: u32,
    pub detail: String,
}

/// One axis on which two otherwise-matching boundaries cannot connect.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AxisRefusal {
    /// `medium`, `transport_security` or `auth`.
    pub axis: String,
    pub ours_says: String,
    pub theirs_says: String,
}

/// A boundary with nothing on the other side.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnmatchedBoundary {
    pub id: String,
    pub name: String,
    pub reason: String,
}

/// The result of pairing two designs by complementary role.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PairingReport {
    /// Matched strands, ready to hand to [`DesignGraph::seam_report`].
    pub paired: Vec<PairedBoundary>,
    /// Name-matched but structurally unconnectable — the loudest finding.
    pub conflicts: Vec<PairingConflict>,
    /// We need it; nobody publishes it. The unmet needs.
    pub unmet_needs: Vec<UnmatchedBoundary>,
    /// They publish it; nobody here needs it. Dead surface.
    pub dead_surface: Vec<UnmatchedBoundary>,
    /// Two publishers answering one need — a conflict, not a match.
    pub duplicate_providers: Vec<String>,
    /// Cleared the fuzzy bar but not the certainty bar. ASK, never auto-pair
    /// (`dec:ask-not-repair`).
    pub candidates: Vec<PairedBoundary>,
    /// Boundaries on either side carrying no external role. Counted and NAMED,
    /// because `internal` is the default and cannot distinguish "deliberately
    /// internal" from "never classified" — without this a design that did no
    /// labelling pairs with nothing and reports a clean seam.
    pub unclassified_ours: Vec<String>,
    pub unclassified_theirs: Vec<String>,
    pub note: String,
}

/// The axes that decide whether two boundaries CAN be connected at all.
///
/// Amended 2026-07-28 after the dynograph-foundation trial refuted role-plus-medium
/// from the provider's side: their design carries three `medium: REST` boundaries,
/// one of which is public and unauthenticated by design because an orchestrator's
/// liveness probe cannot hold a credential. Under medium alone a consumer
/// declaring "I require REST" pairs against all three, including that one. That
/// is not a near miss; it is the rule confidently producing a wrong and
/// security-relevant answer.
const BASE_AXES: [&str; 3] = ["medium", "transport_security", "auth"];

/// Names cleared this bar to be considered the same contract at all.
const PAIR_FUZZY_THRESHOLD: u32 = 60;
/// At or above this, the name match is certain enough to pair without asking.
const PAIR_CERTAIN_THRESHOLD: u32 = 85;

fn iface_name(props: &[(String, Value)], id: &str) -> String {
    prop(props, "name").unwrap_or_else(|| id.to_string())
}

impl DesignGraph {
    /// Compute the seam between this design and another by COMPLEMENTARY ROLE,
    /// instead of a person asserting which boundaries correspond
    /// (`req:complementary-pairing`).
    ///
    /// Each boundary declares a role on `Interface.designation`, and pairing
    /// matches complements — `published`/`both` against `required`/`both` — never
    /// like with like. A publisher paired to a publisher is the same mistake as
    /// pairing adenine with adenine.
    ///
    /// Two boundaries pair when their NAMES match (fuzzily, reusing ingest's
    /// two-band resolution rather than inventing a second matcher) AND their base
    /// axes agree. A name match whose base axes disagree is reported as a
    /// CONFLICT rather than dropped, because "we cannot be connected the way
    /// either of us is built" is exactly the finding worth having.
    ///
    /// This produces the pairs `seam_report` used to be handed by hand. It does
    /// not replace it: pairing says WHICH boundaries correspond, and
    /// `seam_report` says whether the full contracts agree once they do.
    pub fn pair_designs(&self, other: &GraphExport) -> Result<PairingReport, DynoError> {
        let ours_doc = self.export_graph()?;

        let interfaces = |doc: &GraphExport| -> Vec<(String, Vec<(String, Value)>)> {
            doc.nodes
                .iter()
                .filter(|n| n.node_type == node::INTERFACE)
                .map(|n| {
                    (
                        n.node_id.clone(),
                        n.properties.clone().into_iter().collect(),
                    )
                })
                .collect()
        };
        let role = |props: &[(String, Value)]| -> String {
            prop(props, "designation").unwrap_or_else(|| "internal".to_string())
        };
        let offers =
            |props: &[(String, Value)]| matches!(role(props).as_str(), "published" | "both");
        let needs = |props: &[(String, Value)]| matches!(role(props).as_str(), "required" | "both");

        let ours = interfaces(&ours_doc);
        let theirs = interfaces(other);

        let mut paired = Vec::new();
        let mut candidates = Vec::new();
        let mut conflicts = Vec::new();
        let mut matched_ours: BTreeSet<String> = BTreeSet::new();
        let mut matched_theirs: BTreeSet<String> = BTreeSet::new();
        let mut provider_counts: BTreeMap<String, Vec<String>> = BTreeMap::new();

        // Both directions: what we need against what they offer, and what we
        // offer against what they need. A seam is not one-way.
        for (our_id, our_props) in &ours {
            for (their_id, their_props) in &theirs {
                let our_offers_their_needs = offers(our_props) && needs(their_props);
                let our_needs_their_offers = needs(our_props) && offers(their_props);
                if !(our_offers_their_needs || our_needs_their_offers) {
                    continue;
                }
                let score = token_sort_ratio(
                    &iface_name(our_props, our_id),
                    &iface_name(their_props, their_id),
                );
                if score < PAIR_FUZZY_THRESHOLD {
                    continue;
                }

                // Names say "same contract". The base axes say whether it can be
                // connected. Disagreement here is a finding, not a non-match.
                let mut agreed = Vec::new();
                let mut refusals = Vec::new();
                for axis in BASE_AXES {
                    match (prop(our_props, axis), prop(their_props, axis)) {
                        (Some(a), Some(b)) if a == b => agreed.push(axis.to_string()),
                        (Some(a), Some(b)) => refusals.push(AxisRefusal {
                            axis: axis.to_string(),
                            ours_says: a,
                            theirs_says: b,
                        }),
                        // Unstated is NOT agreement — the rule this module was
                        // built on. It simply does not count toward pairing.
                        _ => {}
                    }
                }

                if !refusals.is_empty() {
                    let why = refusals
                        .iter()
                        .map(|r| {
                            format!(
                                "`{}` is `{}` here and `{}` there",
                                r.axis, r.ours_says, r.theirs_says
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    conflicts.push(PairingConflict {
                        ours: our_id.clone(),
                        theirs: their_id.clone(),
                        refusals,
                        name_score: score,
                        detail: format!(
                            "names match at {score}/100, so these are very likely the same \
                             contract — but {why}, so they cannot be connected as either is \
                             currently built"
                        ),
                    });
                    matched_ours.insert(our_id.clone());
                    matched_theirs.insert(their_id.clone());
                    continue;
                }

                let offered_by = if our_offers_their_needs {
                    "ours"
                } else {
                    "theirs"
                };
                let pair = PairedBoundary {
                    ours: our_id.clone(),
                    theirs: their_id.clone(),
                    offered_by: offered_by.to_string(),
                    name_score: score,
                    agreed_on: agreed,
                };
                if score >= PAIR_CERTAIN_THRESHOLD {
                    if our_needs_their_offers {
                        provider_counts
                            .entry(our_id.clone())
                            .or_default()
                            .push(their_id.clone());
                    }
                    paired.push(pair);
                } else {
                    // The middle band: worth a human's attention, never acted on.
                    candidates.push(pair);
                }
                matched_ours.insert(our_id.clone());
                matched_theirs.insert(their_id.clone());
            }
        }

        let duplicate_providers = provider_counts
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(need, providers)| {
                format!(
                    "`{need}` is answered by {} boundaries ({}) — two publishers of one need is a \
                     conflict to resolve, not a match to celebrate",
                    providers.len(),
                    providers.join(", ")
                )
            })
            .collect();

        let unmet_needs = ours
            .iter()
            .filter(|(id, p)| needs(p) && !matched_ours.contains(id))
            .map(|(id, p)| UnmatchedBoundary {
                id: id.clone(),
                name: iface_name(p, id),
                reason: "we require this and nothing in the other design publishes it — the \
                         loudest signal pairing produces"
                    .to_string(),
            })
            .collect();

        let dead_surface = theirs
            .iter()
            .filter(|(id, p)| offers(p) && !matched_theirs.contains(id))
            .map(|(id, p)| UnmatchedBoundary {
                id: id.clone(),
                name: iface_name(p, id),
                reason: "they publish this and nothing here requires it — dead surface from this \
                         design's point of view, not necessarily from theirs"
                    .to_string(),
            })
            .collect();

        let unclassified = |set: &[(String, Vec<(String, Value)>)]| -> Vec<String> {
            set.iter()
                .filter(|(_, p)| role(p) == "internal")
                .map(|(id, p)| format!("{id} ({})", iface_name(p, id)))
                .collect()
        };

        Ok(PairingReport {
            paired,
            conflicts,
            unmet_needs,
            dead_surface,
            duplicate_providers,
            candidates,
            unclassified_ours: unclassified(&ours),
            unclassified_theirs: unclassified(&theirs),
            note: format!(
                "Paired by complementary role on `Interface.designation` — published/both against \
                 required/both, never like with like. A pair needs a name match at or above \
                 {PAIR_CERTAIN_THRESHOLD}/100 AND agreement on {}; between \
                 {PAIR_FUZZY_THRESHOLD} and {PAIR_CERTAIN_THRESHOLD} it is a CANDIDATE to ask \
                 about, never an action. An axis nobody stated is not agreement and does not pair. \
                 `unclassified_*` is counted because `internal` is the DEFAULT, so it cannot tell \
                 'deliberately internal' from 'never classified' — a design that did no labelling \
                 would otherwise report a clean seam. Pairing says WHICH boundaries correspond; \
                 run seam_report on `paired` to find out whether the full contracts agree.",
                BASE_AXES.join(", ")
            ),
        })
    }
}
