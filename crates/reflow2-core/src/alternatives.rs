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

use dynograph_core::{DynoError, Value};
use dynograph_storage::StoredNode;
use serde::Serialize;

use crate::compare::{DiffSummary, compare_designs};
use crate::export::GraphExport;
use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

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

// ---------------------------------------------------------------------------
// The decision point (BL-70 rung 2): a proposed Decision holds forkable
// alternatives that CONTRADICT their siblings, until it collapses — accept the
// Decision, supersede the losers, and record the outcome in the ADR's own
// alternatives field (`dec:parallel-alternatives`). Branch-by-file: each
// alternative points at its export; comparison and collapse reuse
// analyze_alternatives and merge_designs/apply_merge.
// ---------------------------------------------------------------------------

const DECISION_STATUSES: [&str; 4] = ["proposed", "accepted", "superseded", "rejected"];

/// A pointer to one alternative design under a decision point — an `Artifact`
/// (branch-by-file), naming where its export lives.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlternativeRef {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The alternative's export document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// What collapsing a decision point did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollapseReport {
    pub decision: String,
    pub winner: String,
    /// The superseded alternatives, retired on the record (not deleted).
    pub retired: Vec<String>,
}

fn str_prop(n: &StoredNode, key: &str) -> Option<String> {
    n.properties
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

impl DesignGraph {
    /// Set a Decision's lifecycle status — the teeth that open a decision point
    /// (`proposed`) and close it. Loud on an unknown status or a missing
    /// Decision; every other property is preserved.
    pub fn set_decision_status(
        &mut self,
        decision_id: &str,
        status: &str,
    ) -> Result<StoredNode, DynoError> {
        if !DECISION_STATUSES.contains(&status) {
            return Err(DynoError::Validation {
                node_type: node::DECISION.into(),
                property: "status".into(),
                message: format!(
                    "'{status}' is not a Decision status (one of {})",
                    DECISION_STATUSES.join(", ")
                ),
            });
        }
        let Some(existing) = self.get_node(node::DECISION, decision_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DECISION.into(),
                node_id: decision_id.into(),
            });
        };
        let mut props = Props::new().set("status", status);
        for (k, v) in &existing.properties {
            if k != "status" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::DECISION, decision_id, props)
    }

    /// The alternatives under a decision point, sorted by id — the Artifacts
    /// that are `GOVERNED_BY` the Decision. Feeds `analyze_alternatives` (load
    /// each one's `location`).
    pub fn alternatives_for(&self, decision_id: &str) -> Result<Vec<AlternativeRef>, DynoError> {
        let mut out = Vec::new();
        for e in self.incoming(decision_id, Some(edge::GOVERNED_BY))? {
            if let Some(n) = self.get_node(node::ARTIFACT, &e.from_id)? {
                out.push(AlternativeRef {
                    id: e.from_id.clone(),
                    name: str_prop(&n, "name"),
                    location: str_prop(&n, "location"),
                });
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Register an alternative under a *proposed* decision point: an Artifact
    /// pointing at its export, `GOVERNED_BY` the Decision and `CONTRADICTS` its
    /// siblings. Refuses on a decision that is not proposed — you fork only an
    /// open choice.
    pub fn register_alternative(
        &mut self,
        decision_id: &str,
        artifact_id: &str,
        name: &str,
        location: &str,
    ) -> Result<AlternativeRef, DynoError> {
        let Some(dec) = self.get_node(node::DECISION, decision_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DECISION.into(),
                node_id: decision_id.into(),
            });
        };
        if str_prop(&dec, "status").as_deref() != Some("proposed") {
            return Err(DynoError::Validation {
                node_type: node::DECISION.into(),
                property: "status".into(),
                message: format!(
                    "decision '{decision_id}' is not a proposed decision point — set it to \
                     proposed before registering alternatives (you fork an open choice, not a \
                     settled one)"
                ),
            });
        }
        let siblings = self.alternatives_for(decision_id)?;

        self.begin_batch();
        let result = (|| -> Result<(), DynoError> {
            let props = Props::new()
                .set("name", name)
                .set("artifact_type", "model")
                .set("status", "planned")
                .set("location", location);
            self.create_node(node::ARTIFACT, artifact_id, props)?;
            self.create_edge(
                edge::GOVERNED_BY,
                node::ARTIFACT,
                artifact_id,
                node::DECISION,
                decision_id,
                Props::new(),
            )?;
            for sib in &siblings {
                self.create_edge(
                    edge::CONTRADICTS,
                    node::ARTIFACT,
                    artifact_id,
                    node::ARTIFACT,
                    &sib.id,
                    Props::new(),
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.commit_batch()?;
                Ok(AlternativeRef {
                    id: artifact_id.to_string(),
                    name: Some(name.to_string()),
                    location: Some(location.to_string()),
                })
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }

    /// Collapse a decision point: the winner is chosen, the Decision moves to
    /// `accepted`, the losers are superseded (`OBSOLETES`, retired on the record
    /// — not deleted), and the outcome is written into the Decision's own
    /// `alternatives` field (the ADR obituary the fork upgrades). The design
    /// content is merged separately by `apply_merge` — this records the choice.
    pub fn collapse_decision(
        &mut self,
        decision_id: &str,
        winner_id: &str,
        note: Option<&str>,
    ) -> Result<CollapseReport, DynoError> {
        let Some(dec) = self.get_node(node::DECISION, decision_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DECISION.into(),
                node_id: decision_id.into(),
            });
        };
        let alts = self.alternatives_for(decision_id)?;
        if !alts.iter().any(|a| a.id == winner_id) {
            return Err(DynoError::Validation {
                node_type: node::DECISION.into(),
                property: "winner".into(),
                message: format!(
                    "'{winner_id}' is not an alternative under decision '{decision_id}'"
                ),
            });
        }
        let losers: Vec<AlternativeRef> =
            alts.iter().filter(|a| a.id != winner_id).cloned().collect();

        // The ADR obituary: what was weighed, and how each fared.
        let obituary = serde_json::to_string(
            &alts
                .iter()
                .map(|a| {
                    let outcome = if a.id == winner_id {
                        "chosen"
                    } else {
                        "retired"
                    };
                    serde_json::json!({
                        "id": a.id,
                        "name": a.name,
                        "outcome": outcome,
                        "note": note.unwrap_or(""),
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();

        self.begin_batch();
        let result = (|| -> Result<(), DynoError> {
            let mut props = Props::new()
                .set("status", "accepted")
                .set("alternatives", obituary.as_str());
            for (k, v) in &dec.properties {
                if k != "status" && k != "alternatives" {
                    props = props.set(k, v.clone());
                }
            }
            self.create_node(node::DECISION, decision_id, props)?;
            for l in &losers {
                self.create_edge(
                    edge::OBSOLETES,
                    node::ARTIFACT,
                    winner_id,
                    node::ARTIFACT,
                    &l.id,
                    Props::new(),
                )?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.commit_batch()?;
                Ok(CollapseReport {
                    decision: decision_id.to_string(),
                    winner: winner_id.to_string(),
                    retired: losers.into_iter().map(|a| a.id).collect(),
                })
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }
}
