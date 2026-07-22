//! Graph report — the **SYNTHESIZE** rollup: graph → a human artifact
//! (docs/overview.md "SYNTHESIZE"; graph-analysis.md "Graph report").
//!
//! A one-shot "what should I look at?" summary that aggregates what the other
//! deterministic analyses already compute — a design snapshot, the top ranked
//! [`GapCandidate`]s (DETECT), allocation health (`evaluate_allocation`),
//! surprising couplings (`surprising_connections`), and declining quality
//! (`dimension_drifts`) — into one [`GraphReport`] that renders to Markdown.
//!
//! Pure aggregation, no LLM: it reuses the deterministic analyses and never
//! silently truncates — every capped list reports how many more there were.

use std::fmt::Write as _;

use dynograph_core::DynoError;

use crate::allocate::AllocationReport;
use crate::detect::GapCandidate;
use crate::dimensions::{DimensionDrift, DriftDirection};
use crate::graph::DesignGraph;
use crate::nodes::node;
use crate::surprises::SurprisingConnection;

/// How many items each highlight list caps at (the rest are counted, not shown).
const TOP_N: usize = 5;

/// Design node types included in the snapshot, in lifecycle order.
const SNAPSHOT_TYPES: &[&str] = &[
    node::PROJECT,
    node::REQUIREMENT,
    node::CONSTRAINT,
    node::DESIGN_RULE,
    node::CAPABILITY,
    node::FLOW,
    node::ACTOR,
    node::COMPONENT,
    node::INTERFACE,
    node::DECISION,
    node::ARTIFACT,
    node::VERIFICATION,
    node::RELEASE,
    node::ENVIRONMENT,
    node::RESOURCE,
];

/// The `status` × `provenance` → certainty mapping, on a node already in
/// hand. Absent properties take their schema defaults (`proposed`,
/// `authored`), so a bare requirement reads as asserted, never as confirmed.
fn certainty_of(req: &dynograph_storage::StoredNode) -> RequirementCertainty {
    let status = req
        .properties
        .get("status")
        .and_then(dynograph_core::Value::as_str)
        .unwrap_or("proposed");
    match status {
        "accepted" | "met" => RequirementCertainty::UserConfirmed,
        "deferred" | "dropped" => RequirementCertainty::SettledOut,
        _ => {
            let provenance = req
                .properties
                .get("provenance")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("authored");
            match provenance {
                "inferred" | "reconciled" | "healed" => RequirementCertainty::Recovered,
                _ => RequirementCertainty::Asserted,
            }
        }
    }
}

/// Whether a node type is design content, as opposed to the supporting layer
/// (provenance, questions, history). The same split the graph report's
/// snapshot draws — `compare` reuses it so "design vs supporting" means one
/// thing everywhere.
pub(crate) fn is_design_type(node_type: &str) -> bool {
    SNAPSHOT_TYPES.contains(&node_type)
}

/// How firmly a Requirement stands — derived from `status` × `provenance`,
/// never stored (BL-75, `dec:certainty-derived`). The two axes already span
/// the space; a third stored property could contradict them both. The
/// load-bearing doctrine that makes this derivable: an agent captures
/// requirements at `proposed`, and ONLY the user's answer moves the status —
/// `accepted`, `met`, `deferred` and `dropped` are all user-only verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementCertainty {
    /// The user said yes to this — status `accepted` or `met`.
    UserConfirmed,
    /// Someone stated it and the user has not yet confirmed the wording —
    /// status `proposed`, provenance `authored`/`planned`/`imported`.
    Asserted,
    /// Read back out of an existing system and not yet put to the user —
    /// status `proposed`, provenance `inferred`/`reconciled`/`healed`. A
    /// recovered requirement is satisfied by construction and can never
    /// contradict anything, which is exactly why its certainty must be
    /// visible.
    Recovered,
    /// The user decided it *out* — status `deferred` or `dropped`. Also
    /// their word; not uncertainty.
    SettledOut,
}

/// The certainty breakdown the snapshot renders — so no session reconstructs
/// "which of these did the user actually confirm?" in prose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CertaintyBreakdown {
    pub user_confirmed: usize,
    pub asserted: usize,
    pub recovered: usize,
    pub settled_out: usize,
}

/// How much of the design carries its own verification.
///
/// A *signal*, not a gap. An unverified Capability is asked about — nothing
/// proves that behaviour works. A file with no `VERIFIES` edge of its own is
/// merely worth knowing: demanding one per source file produced 22 of 25 gaps
/// on reflow2's own design, all on a crate whose capabilities are tested
/// (BL-23). The number is reported so anyone who does want per-file rigour can
/// see where they stand.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationCoverage {
    pub capabilities: usize,
    /// Capabilities with at least one incoming `VERIFIES`.
    pub capabilities_verified: usize,
    /// Capabilities with no passing check of their own whose allocated
    /// component carries one — verified at component granularity (BL-73).
    /// Neither `verified` nor unchecked: the state that made a tested
    /// brownfield system read as "0/20 verified" when it was invisible.
    pub capabilities_component_verified: usize,
    pub artifacts: usize,
    /// Artifacts with a `VERIFIES` edge of their own, as opposed to being
    /// covered by the capability they realize.
    pub artifacts_verified: usize,
}

impl VerificationCoverage {
    /// True when there is nothing to report — no capabilities and no artifacts.
    fn is_empty(&self) -> bool {
        self.capabilities == 0 && self.artifacts == 0
    }
}

/// How much of the design has something built for it.
///
/// The counting half of BL-42, and the same bargain as
/// [`VerificationCoverage`]: `unrealized_capability` asks only where the build
/// demonstrably arrived and skipped a capability. Capabilities in a region the
/// artifact layer has not reached at all are **counted here, never asked
/// about** — the storyflow adopt trial turned that question into 13 of 51
/// gaps, every one a consequence of modelling artifacts coarsely on purpose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RealizationCoverage {
    pub capabilities: usize,
    /// Capabilities with an artifact realizing them, or realizing a component
    /// they are allocated to (both P3 shapes, per BL-38).
    pub realized: usize,
    /// Capabilities with no artifact, whose owning component is nonetheless
    /// marked `realized` — the modeller asserts these exist and simply has
    /// not modelled a file for them. Not a gap: a statement about how much
    /// of a built system the artifact layer covers.
    pub built_but_unmodelled: usize,
}

/// The confirmation rollup — counts only; the full ledger is
/// [`DesignGraph::confirmation_ledger`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfirmationSummary {
    pub drifting: usize,
    pub confirmed: usize,
    pub unexamined: usize,
}

/// Allocation health at a glance (from `evaluate_allocation`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct AllocationSummary {
    /// Components with at least one capability.
    pub component_count: usize,
    /// Cohesion/coupling modularity (1.0 = perfectly cohesive).
    pub modularity: f64,
    /// Capabilities coupled more strongly across a boundary than within.
    pub misplaced_count: usize,
    /// Routing-hub components (selective SPOF).
    pub god_components: Vec<String>,
}

/// The rolled-up state of the design graph.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphReport {
    /// `(node type, count)` for design types present, lifecycle order.
    pub node_counts: Vec<(&'static str, usize)>,
    /// `(node type, count)` for every *other* populated type — provenance
    /// (`Fragment`), questions, drift events, axis-Z machinery, dimension
    /// readings. Itemised rather than omitted: these are real nodes, and a
    /// total that skipped them made a 122-node graph report 109 (BL-43).
    pub other_counts: Vec<(String, usize)>,
    /// Design nodes only — the sum of `node_counts`.
    pub design_nodes: usize,
    /// **Every** node in the graph: `design_nodes` plus `other_counts`.
    pub total_nodes: usize,
    /// Total open gaps (DETECT).
    pub gap_count: usize,
    /// Total structural defects (HEAL).
    pub defect_count: usize,
    /// The highest-severity gaps (capped at [`TOP_N`]).
    pub top_gaps: Vec<GapCandidate>,
    /// Gaps beyond the shown top (never silently dropped).
    pub gaps_truncated: usize,
    /// Allocation health, when components exist.
    pub allocation: Option<AllocationSummary>,
    /// The most surprising couplings (capped).
    pub surprising: Vec<SurprisingConnection>,
    /// Surprising couplings beyond the shown top.
    pub surprising_truncated: usize,
    /// Which requirements the user actually confirmed, which are asserted,
    /// which were recovered from an artifact (BL-75). `None` when there are
    /// no requirements.
    pub requirement_certainty: Option<CertaintyBreakdown>,
    /// How much of the design carries its own verification (a signal, not a gap).
    pub verification: VerificationCoverage,
    /// How much of the design has something built for it, and how much the
    /// artifact layer does not reach (a signal, not a gap — BL-42).
    pub realization: RealizationCoverage,
    /// Confirmation rollup (BL-35): of the capabilities with realizing
    /// artifacts, how many are drifting / confirmed / **unexamined** — the
    /// last being the state the original reflow died in: nobody looked, and
    /// nothing could tell.
    pub confirmation: Option<ConfirmationSummary>,
    /// Declining quality dimensions, worst first (capped).
    pub declining: Vec<DimensionDrift>,
    /// Declining dimensions beyond the shown top.
    pub declining_truncated: usize,
}

/// The coherence loop's outstanding debt — what CHANGE→DETECT→SURFACE→RESOLVE
/// steps are *owed*, computed from graph state alone (BL-74).
///
/// Deliberately state, never run-history (`dec:loop-status-state-not-history`):
/// the core takes no clock and looking is not writing, so "you haven't run
/// detect_gaps since Tuesday" is not an honest computation — but "3 open gaps
/// were never put to the user" is, and it is also the thing that actually
/// matters. The field lesson this answers: under operational load, bookkeeping
/// via the raw tools continued while the loop silently stopped, because
/// nothing cheap said what was owed. This is that cheap thing — one call, a
/// to-do list, no skill loaded.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoopStatus {
    /// Open *anchored* gaps DETECT finds right now that no Question was ever
    /// asked about — intent was captured and never surfaced to the user.
    /// Phase nudges don't count: they say what comes next, not what is owed.
    pub unsurfaced_gaps: usize,
    /// Questions put to the user, still waiting on them.
    pub unanswered_questions: usize,
    /// Questions the user answered whose gap is still open — the answer never
    /// reached the design (write it in, or acknowledge the gap).
    pub unwritten_answers: usize,
    /// Structural defects HEAL reports right now.
    pub structural_defects: usize,
    /// Capabilities claiming `realized`/`verified` with no passing check.
    pub unproven_capabilities: usize,
    /// Recorded divergences (`DriftEvent`) awaiting a disposition.
    pub undispositioned_drift: usize,
    /// Built capabilities nobody has ever checked against reality
    /// (the confirmation ledger's `unexamined`).
    pub unexamined_claims: usize,
    /// The debt as ordered to-do lines, most blocking first. Empty when the
    /// loop is clean — and emptiness is asserted, not implied.
    pub next: Vec<String>,
    /// Every counter zero.
    pub clean: bool,
}

impl DesignGraph {
    /// Derive a requirement's [`RequirementCertainty`] from its stored
    /// `status` and `provenance`. Pure derivation — see the enum for the
    /// mapping and the doctrine it rests on.
    pub fn requirement_certainty(
        &self,
        requirement_id: &str,
    ) -> Result<RequirementCertainty, DynoError> {
        let Some(req) = self.get_node(node::REQUIREMENT, requirement_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::REQUIREMENT.to_string(),
                node_id: requirement_id.to_string(),
            });
        };
        Ok(certainty_of(&req))
    }

    /// Count the certainty breakdown across every Requirement.
    pub fn requirement_certainty_breakdown(&self) -> Result<CertaintyBreakdown, DynoError> {
        let mut b = CertaintyBreakdown {
            user_confirmed: 0,
            asserted: 0,
            recovered: 0,
            settled_out: 0,
        };
        for req in self.scan_nodes(node::REQUIREMENT)? {
            match certainty_of(&req) {
                RequirementCertainty::UserConfirmed => b.user_confirmed += 1,
                RequirementCertainty::Asserted => b.asserted += 1,
                RequirementCertainty::Recovered => b.recovered += 1,
                RequirementCertainty::SettledOut => b.settled_out += 1,
            }
        }
        Ok(b)
    }

    /// Compute the loop's outstanding debt. See [`LoopStatus`].
    pub fn loop_status(&self) -> Result<LoopStatus, DynoError> {
        let questions = self.open_questions()?;
        let surfaced: std::collections::BTreeSet<&str> =
            questions.iter().map(|q| q.gap_id.as_str()).collect();
        // Acknowledged gaps are already absent from detect_gaps, so what
        // remains unsurfaced is: open right now, anchored to real nodes, and
        // never asked about. Anchored only — a phase nudge says what comes
        // next, not what is owed (dec:anchored-first), and counting nudges as
        // debt would make `clean` unreachable on a healthy design.
        let unsurfaced_gaps = self
            .detect_gaps()?
            .iter()
            .filter(|g| !g.affected_ids.is_empty() && !surfaced.contains(g.id.as_str()))
            .count();
        let unanswered_questions = questions.iter().filter(|q| q.status == "asked").count();
        let unwritten_answers = questions.iter().filter(|q| q.status == "answered").count();

        let structural_defects = self.detect_defects()?.len();

        // A component-granularity check clears this debt (BL-73): the claim
        // HAS a passing check, one hop away — the coverage line says at which
        // granularity, and the per-component gap asks whether that is enough.
        let mut unproven_capabilities = 0usize;
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let claims_built = cap
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .map(|s| s == "realized" || s == "verified")
                .unwrap_or(false);
            if claims_built
                && self.capability_verification(&cap.node_id)?
                    == crate::verify::CapabilityVerification::Unchecked
            {
                unproven_capabilities += 1;
            }
        }

        let undispositioned_drift = self
            .scan_nodes(node::DRIFT_EVENT)?
            .into_iter()
            .filter(|d| {
                !d.properties
                    .get("resolved")
                    .and_then(dynograph_core::Value::as_bool)
                    .unwrap_or(false)
            })
            .count();

        let unexamined_claims = self.confirmation_ledger()?.unexamined;

        let mut next = Vec::new();
        if unanswered_questions > 0 {
            next.push(format!(
                "{unanswered_questions} question(s) are waiting on the user — follow up, \
                 don't re-ask (open_questions)"
            ));
        }
        if unwritten_answers > 0 {
            next.push(format!(
                "{unwritten_answers} answered question(s) never reached the design — write \
                 the answer in, or acknowledge the gap"
            ));
        }
        if unsurfaced_gaps > 0 {
            next.push(format!(
                "{unsurfaced_gaps} open gap(s) have never been put to the user — run \
                 detect-and-ask"
            ));
        }
        if structural_defects > 0 {
            next.push(format!(
                "{structural_defects} structural defect(s) outstanding — run check-health \
                 (detect_defects)"
            ));
        }
        if unproven_capabilities > 0 {
            next.push(format!(
                "{unproven_capabilities} capability(ies) claim realized/verified with no \
                 passing check — add or run their Verification"
            ));
        }
        if undispositioned_drift > 0 {
            next.push(format!(
                "{undispositioned_drift} recorded divergence(s) await a disposition \
                 (set_artifact_checksum)"
            ));
        }
        if unexamined_claims > 0 {
            next.push(format!(
                "{unexamined_claims} built capability(ies) never checked against reality \
                 (reconcile_artifacts)"
            ));
        }

        Ok(LoopStatus {
            unsurfaced_gaps,
            unanswered_questions,
            unwritten_answers,
            structural_defects,
            unproven_capabilities,
            undispositioned_drift,
            unexamined_claims,
            clean: next.is_empty(),
            next,
        })
    }

    /// Count how much of the design carries its own verification.
    ///
    /// Deliberately a count and not a detector. Capabilities without a check
    /// are a real gap and DETECT still raises one; artifacts without their own
    /// check are worth *knowing* and not worth *asking* about, because the
    /// answer is usually "the capability's tests cover it" (BL-23).
    pub fn verification_coverage(&self) -> Result<VerificationCoverage, DynoError> {
        let mut v = VerificationCoverage {
            capabilities: 0,
            capabilities_verified: 0,
            capabilities_component_verified: 0,
            artifacts: 0,
            artifacts_verified: 0,
        };
        // "Verified" means a check that PASSES, not a check that exists.
        // Counting mere existence let a failing test raise coverage — the
        // design counting test nodes while ignoring test results, which is
        // the reflow1 failure in miniature (BL-30). `planned`, `failing`,
        // `skipped` and `blocked` all mean "not currently confirmed".
        for n in self.scan_nodes(node::CAPABILITY)? {
            v.capabilities += 1;
            match self.capability_verification(&n.node_id)? {
                crate::verify::CapabilityVerification::Verified => v.capabilities_verified += 1,
                crate::verify::CapabilityVerification::ComponentVerified => {
                    v.capabilities_component_verified += 1;
                }
                crate::verify::CapabilityVerification::Unchecked => {}
            }
        }
        for n in self.scan_nodes(node::ARTIFACT)? {
            v.artifacts += 1;
            if self.has_passing_verification(&n.node_id)? {
                v.artifacts_verified += 1;
            }
        }
        Ok(v)
    }

    /// Count how much of the design has something built for it — and how much
    /// the artifact layer simply does not reach. See [`RealizationCoverage`].
    pub fn realization_coverage(&self) -> Result<RealizationCoverage, DynoError> {
        let mut c = RealizationCoverage {
            capabilities: 0,
            realized: 0,
            built_but_unmodelled: 0,
        };
        for cap in self.scan_nodes(node::CAPABILITY)? {
            c.capabilities += 1;
            if self.capability_is_realized(&cap.node_id)? {
                c.realized += 1;
            } else if self.owner_claims_built(&cap.node_id)? {
                c.built_but_unmodelled += 1;
            }
        }
        Ok(c)
    }

    /// Build the [`GraphReport`] — a one-shot aggregation of the deterministic
    /// analyses. See the module docs.
    pub fn graph_report(&self) -> Result<GraphReport, DynoError> {
        let mut node_counts = Vec::new();
        let mut design_nodes = 0;
        for &t in SNAPSHOT_TYPES {
            let n = self.count_nodes(t)?;
            if n > 0 {
                node_counts.push((t, n));
                design_nodes += n;
            }
        }

        // Everything the design-layer itemisation above does not cover:
        // provenance (`Fragment`), the asked-question record, drift events,
        // axis-Z machinery, dimension readings. Counted from the *schema*
        // rather than a second hardcoded list, so a node type added later
        // cannot go missing from the total the way `Fragment` did.
        //
        // The storyflow adopt trial imported 122 nodes and was told 109
        // (BL-43): `total_nodes` summed the snapshot list only, so the whole
        // provenance ledger — the thing that makes a recovered claim
        // checkable — was invisible to the surface an agent reads first. A
        // count that silently omits a type is a quiet lie about the size of
        // the design, which is rule 6 (no silent caps) applied to reporting.
        let mut other_counts = Vec::new();
        let mut other_nodes = 0;
        let mut schema_types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        schema_types.sort();
        for t in schema_types {
            if SNAPSHOT_TYPES.contains(&t.as_str()) {
                continue;
            }
            let n = self.count_nodes(&t)?;
            if n > 0 {
                other_nodes += n;
                other_counts.push((t, n));
            }
        }
        let total_nodes = design_nodes + other_nodes;

        let verification = self.verification_coverage()?;
        let requirement_certainty = if self.count_nodes(node::REQUIREMENT)? > 0 {
            Some(self.requirement_certainty_breakdown()?)
        } else {
            None
        };
        let realization = self.realization_coverage()?;
        let ledger = self.confirmation_ledger()?;
        let confirmation = if ledger.claims.is_empty() {
            None
        } else {
            Some(ConfirmationSummary {
                drifting: ledger.drifting,
                confirmed: ledger.confirmed,
                unexamined: ledger.unexamined,
            })
        };

        let mut gaps = self.detect_gaps()?;
        let gap_count = gaps.len();
        let gaps_truncated = gap_count.saturating_sub(TOP_N);
        gaps.truncate(TOP_N);

        let defect_count = self.detect_defects()?.len();

        let allocation = if self.count_nodes(node::COMPONENT)? > 0 {
            let a: AllocationReport = self.evaluate_allocation()?;
            Some(AllocationSummary {
                component_count: a.components.len(),
                modularity: a.modularity,
                misplaced_count: a.misplaced.len(),
                god_components: a.god_components,
            })
        } else {
            None
        };

        let mut surprising = self.surprising_connections()?;
        let surprising_truncated = surprising.len().saturating_sub(TOP_N);
        surprising.truncate(TOP_N);

        let mut declining: Vec<DimensionDrift> = self
            .dimension_drifts()?
            .into_iter()
            .filter(|d| d.direction == DriftDirection::Declining)
            .collect();
        let declining_truncated = declining.len().saturating_sub(TOP_N);
        declining.truncate(TOP_N);

        Ok(GraphReport {
            node_counts,
            other_counts,
            design_nodes,
            realization,
            total_nodes,
            gap_count,
            defect_count,
            top_gaps: gaps,
            gaps_truncated,
            allocation,
            surprising,
            surprising_truncated,
            requirement_certainty,
            verification,
            confirmation,
            declining,
            declining_truncated,
        })
    }
}

impl GraphReport {
    /// Render the report as Markdown — the shareable "what should I look at?"
    /// artifact.
    pub fn to_markdown(&self) -> String {
        let mut m = String::new();

        let _ = writeln!(m, "# Design graph report\n");

        // Snapshot.
        let _ = writeln!(m, "## Snapshot\n");
        if self.node_counts.is_empty() {
            let _ = writeln!(m, "_Empty graph — nothing designed yet._\n");
        } else {
            let breakdown = self
                .node_counts
                .iter()
                .map(|(t, n)| format!("{t} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                m,
                "{} design nodes across {} type(s): {}.\n",
                self.design_nodes,
                self.node_counts.len(),
                breakdown
            );
            if !self.other_counts.is_empty() {
                let other = self
                    .other_counts
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    m,
                    "Plus {} supporting node(s) — provenance, questions, history: {}. \
                     **{} nodes in total.**\n",
                    self.total_nodes - self.design_nodes,
                    other,
                    self.total_nodes
                );
            }
            // Which requirements the user actually confirmed (BL-75) — said
            // here so no session reconstructs certainty in prose. Zero
            // categories are omitted; a wholly-confirmed set reads as one
            // clean clause.
            if let Some(c) = &self.requirement_certainty {
                let mut parts = Vec::new();
                for (n, label) in [
                    (c.user_confirmed, "user-confirmed"),
                    (c.asserted, "asserted, awaiting the user"),
                    (
                        c.recovered,
                        "recovered from the artifact, awaiting the user",
                    ),
                    (c.settled_out, "settled out (deferred/dropped)"),
                ] {
                    if n > 0 {
                        parts.push(format!("{n} {label}"));
                    }
                }
                if !parts.is_empty() {
                    let _ = writeln!(m, "Requirement certainty: {}.\n", parts.join(" · "));
                }
            }
            let _ = writeln!(
                m,
                "{} open gap(s), {} structural defect(s).\n",
                self.gap_count, self.defect_count
            );
        }

        // Top gaps.
        if !self.top_gaps.is_empty() {
            let _ = writeln!(m, "## Top gaps (look here first)\n");
            for g in &self.top_gaps {
                let _ = writeln!(
                    m,
                    "- **[{:.2}]** {} — {}",
                    g.severity, g.title, g.description
                );
            }
            if self.gaps_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.gaps_truncated);
            }
            let _ = writeln!(m);
        }

        // Allocation health.
        if let Some(a) = &self.allocation {
            let _ = writeln!(m, "## Allocation health\n");
            let _ = writeln!(
                m,
                "Modularity **{:.2}** across {} component(s); {} misplaced capability(ies).",
                a.modularity, a.component_count, a.misplaced_count
            );
            if a.god_components.is_empty() {
                let _ = writeln!(m, "No god-components.\n");
            } else {
                let _ = writeln!(m, "God-component(s): {}.\n", a.god_components.join(", "));
            }
        }

        if let Some(c) = &self.confirmation {
            let _ = writeln!(m, "## Confirmation\n");
            let _ = writeln!(
                m,
                "{} drifting · {} confirmed · {} unexamined (capabilities with built artifacts; \
                 unexamined = nobody has ever checked the claim against reality)\n",
                c.drifting, c.confirmed, c.unexamined
            );
        }

        // Verification coverage — reported, never demanded. Component
        // granularity is its own clause (BL-73): folding it into "verified"
        // would overstate, folding it into silence read a tested system as
        // 0/20.
        if !self.verification.is_empty() {
            let v = &self.verification;
            let _ = writeln!(m, "## Verification coverage\n");
            let component_clause = if v.capabilities_component_verified > 0 {
                format!(
                    " ({} more at component granularity)",
                    v.capabilities_component_verified
                )
            } else {
                String::new()
            };
            let _ = writeln!(
                m,
                "{}/{} capability(ies) verified{}; {}/{} artifact(s) carry a check of their own.\n",
                v.capabilities_verified,
                v.capabilities,
                component_clause,
                v.artifacts_verified,
                v.artifacts
            );
        }

        // Surprising couplings.
        if !self.surprising.is_empty() {
            let _ = writeln!(m, "## Surprising couplings\n");
            for s in &self.surprising {
                let _ = writeln!(
                    m,
                    "- `{}` → `{}` ({}): {}. _[surprise {:.2}]_",
                    s.from_id,
                    s.to_id,
                    s.edge_type,
                    s.reasons.join(", "),
                    s.surprise
                );
            }
            if self.surprising_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.surprising_truncated);
            }
            let _ = writeln!(m);
        }

        // Declining quality.
        if !self.declining.is_empty() {
            let _ = writeln!(m, "## Quality drift (declining)\n");
            for d in &self.declining {
                let _ = writeln!(
                    m,
                    "- **{}** of `{}`: {:.2} → {:.2} over {} reading(s) _(slope {:.3})_",
                    d.dimension.as_str(),
                    d.target_id,
                    d.first_score,
                    d.last_score,
                    d.observation_count,
                    d.slope
                );
            }
            if self.declining_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.declining_truncated);
            }
            let _ = writeln!(m);
        }

        m
    }
}
