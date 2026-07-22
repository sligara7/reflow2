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

/// Whether a node type is design content, as opposed to the supporting layer
/// (provenance, questions, history). The same split the graph report's
/// snapshot draws — `compare` reuses it so "design vs supporting" means one
/// thing everywhere.
pub(crate) fn is_design_type(node_type: &str) -> bool {
    SNAPSHOT_TYPES.contains(&node_type)
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

impl DesignGraph {
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
            artifacts: 0,
            artifacts_verified: 0,
        };
        for (node_type, total, verified) in [
            (
                node::CAPABILITY,
                &mut v.capabilities,
                &mut v.capabilities_verified,
            ),
            (node::ARTIFACT, &mut v.artifacts, &mut v.artifacts_verified),
        ] {
            for n in self.scan_nodes(node_type)? {
                *total += 1;
                // "Verified" means a check that PASSES, not a check that exists.
                // Counting mere existence let a failing test raise coverage —
                // the design counting test nodes while ignoring test results,
                // which is the reflow1 failure in miniature (BL-30). `planned`,
                // `failing`, `skipped` and `blocked` all mean "not currently
                // confirmed".
                for e in self.incoming(&n.node_id, Some(crate::nodes::edge::VERIFIES))? {
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
                        *verified += 1;
                        break;
                    }
                }
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

        // Verification coverage — reported, never demanded.
        if !self.verification.is_empty() {
            let v = &self.verification;
            let _ = writeln!(m, "## Verification coverage\n");
            let _ = writeln!(
                m,
                "{}/{} capability(ies) verified; {}/{} artifact(s) carry a check of their own.\n",
                v.capabilities_verified, v.capabilities, v.artifacts_verified, v.artifacts
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
