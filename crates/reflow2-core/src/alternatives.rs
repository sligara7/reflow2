//! Analysis of alternatives — compare parallel design branches on the same
//! measures (BL-70, `dec:parallel-alternatives`).
//!
//! Alternatives are **branch-by-file**: each is a separate exported design (a
//! git branch's `reflow2.json`, or an export file), held in design *space* —
//! sibling roads that coexist until a decision collapses them, not points in
//! time. This loads N of them, runs the same rollup on each (`graph_report`),
//! and lays the decision-relevant measures side by side — plus each branch's
//! structural divergence from a named baseline (`compare_designs`). The point
//! is to make alternatives comparable **on measures, not on advocacy**.
//!
//! Each alternative is analysed in its own single-world graph, so the merge and
//! comparison machinery BL-80 built is reused whole and no detector has to
//! learn about worlds. Collapsing the winner (merge into the baseline) and
//! retiring the losers reuse `merge_designs` / `retire-from-design`.

use dynograph_core::DynoError;
use serde::Serialize;

use crate::compare::{DiffSummary, compare_designs};
use crate::export::GraphExport;
use crate::graph::DesignGraph;

/// One alternative's decision-relevant measures, taken from its own
/// `graph_report`, plus how it structurally diverges from the baseline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BranchMeasures {
    pub label: String,
    pub design_nodes: usize,
    pub total_nodes: usize,
    pub open_gaps: usize,
    pub structural_defects: usize,
    /// Allocation modularity (1.0 = perfectly cohesive), when components exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modularity: Option<f64>,
    pub capabilities: usize,
    pub capabilities_verified: usize,
    /// Divergence from the baseline branch (added / removed / changed counts).
    /// `None` for the baseline itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divergence_from_baseline: Option<DiffSummary>,
}

/// N alternatives, compared on the same measures. The first is the baseline the
/// others' divergence is reported against.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AoaReport {
    pub baseline: String,
    pub branches: Vec<BranchMeasures>,
}

/// Load each alternative into a fresh in-memory graph, run the rollup, and lay
/// the measures side by side. The first alternative is the baseline; the rest
/// carry their structural divergence from it. Pure of the live graph — it opens
/// its own throwaway graphs, so it runs while a server holds the store.
pub fn analyze_alternatives(
    alternatives: &[(String, GraphExport)],
) -> Result<AoaReport, DynoError> {
    let baseline_label = alternatives
        .first()
        .map(|(l, _)| l.clone())
        .unwrap_or_default();
    let baseline_export = alternatives.first().map(|(_, e)| e);

    let mut branches = Vec::new();
    for (i, (label, export)) in alternatives.iter().enumerate() {
        let mut g = DesignGraph::open_in_memory()?;
        g.import_graph(export)?;
        let report = g.graph_report()?;

        let divergence_from_baseline = if i == 0 {
            None
        } else {
            baseline_export
                .map(|base| compare_designs(base, export, &baseline_label, label).summary)
        };

        branches.push(BranchMeasures {
            label: label.clone(),
            design_nodes: report.design_nodes,
            total_nodes: report.total_nodes,
            open_gaps: report.gap_count,
            structural_defects: report.defect_count,
            modularity: report.allocation.as_ref().map(|a| a.modularity),
            capabilities: report.verification.capabilities,
            capabilities_verified: report.verification.capabilities_verified,
            divergence_from_baseline,
        });
    }

    Ok(AoaReport {
        baseline: baseline_label,
        branches,
    })
}
