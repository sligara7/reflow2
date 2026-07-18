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
    node::ARTIFACT,
    node::VERIFICATION,
    node::RELEASE,
    node::ENVIRONMENT,
    node::RESOURCE,
];

/// Allocation health at a glance (from `evaluate_allocation`).
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct GraphReport {
    /// `(node type, count)` for design types present, lifecycle order.
    pub node_counts: Vec<(&'static str, usize)>,
    /// Total design nodes in the snapshot.
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
    /// Declining quality dimensions, worst first (capped).
    pub declining: Vec<DimensionDrift>,
    /// Declining dimensions beyond the shown top.
    pub declining_truncated: usize,
}

impl DesignGraph {
    /// Build the [`GraphReport`] — a one-shot aggregation of the deterministic
    /// analyses. See the module docs.
    pub fn graph_report(&self) -> Result<GraphReport, DynoError> {
        let mut node_counts = Vec::new();
        let mut total_nodes = 0;
        for &t in SNAPSHOT_TYPES {
            let n = self.count_nodes(t)?;
            if n > 0 {
                node_counts.push((t, n));
                total_nodes += n;
            }
        }

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
            total_nodes,
            gap_count,
            defect_count,
            top_gaps: gaps,
            gaps_truncated,
            allocation,
            surprising,
            surprising_truncated,
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
                self.total_nodes,
                self.node_counts.len(),
                breakdown
            );
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
