//! The frontier — where an incremental, resumable adopt stands, and the
//! deferred-derivation marker that lets a region be left for later without
//! the loop reading it as missing
//! (`req:adopt-can-be-incremental-and-resumable-with-a-frontier-and-a-deferred-derivation-marker`,
//! Alex 2026-09-17, promoted on Anthony's word the same day).
//!
//! Alex's words: adopt and ingest-corpus are whole-repo; "the graph as
//! worklist + memory + resume-state as an explicit contract; a typed
//! structural-vs-intent gap with a findable deferral marker." Two primitives:
//!
//! * A DEFERRED-DERIVATION MARKER — a `TemporalFact` with
//!   `fact_type: deferred_derivation` on the node whose intent is deliberately
//!   left for later. The same shape as the `follow_up` marker `/jot`
//!   writes: dated, keyed to a node, open until `valid_to` or an INVALIDATES
//!   edge closes it. While open it QUIETS the intent findings on its subject
//!   (`unmotivated_capability`, `unallocated_component`) — a regional pass no
//!   longer reports false gaps for what it left on purpose — and it is LISTED
//!   AS OWED at every boundary (`loop_status.deferrals`), so a deferral is
//!   never a way to make a question disappear.
//! * A FRONTIER read — the worklist and the resume point: what is captured
//!   structurally and not yet recovered as intent, what was deferred and when,
//!   and (when the caller hands in a sweep) what is adjacent and not captured
//!   at all. Nothing here is a verdict; it says where you were.
//!
//! THE BREADTH-FIRST PAYOFF IS NOT GIVEN UP. The adopt skill chose breadth
//! because every brownfield trial's payoff finding was structural and came
//! from breadth. A region mode must keep saying "here is what you have not
//! looked at", which is why the third leg exists and why it refuses to read
//! clean without a sweep: `uncaptured: None` with a note is the answer when
//! nobody swept, never an empty list.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::coverage::{ObservedPath, UnclaimedRegion};
use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// The `fact_type` that marks a node's intent as deliberately deferred.
pub const DEFERRED_DERIVATION: &str = "deferred_derivation";

/// One part whose intent was deliberately left for later.
#[derive(Debug, Clone, Serialize)]
pub struct Deferral {
    pub fact_id: String,
    /// The node it was recorded against.
    pub subject_id: String,
    /// Why it was deferred, in the words it was captured in.
    pub statement: String,
    /// When, when the capture said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// One node captured structurally with no intent behind it yet.
#[derive(Debug, Clone, Serialize)]
pub struct FrontierItem {
    pub node_id: String,
    pub node_type: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrontierReport {
    /// Structure with no intent: capabilities no requirement asks for and
    /// leaf components nothing is allocated to — the same rules the two
    /// detectors apply — minus anything under an open deferral, which is
    /// listed under `deferred` instead.
    pub structure_without_intent: Vec<FrontierItem>,
    /// Parts whose intent was deliberately left for later, oldest first.
    pub deferred: Vec<Deferral>,
    /// Adjacent and not captured at all: the unclaimed regions of the sweep
    /// the caller handed in. `None` when no sweep was handed in — never an
    /// empty list, because an empty list would read as "everything is
    /// captured" when the truth is "nobody looked".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncaptured: Option<Vec<UnclaimedRegion>>,
    /// The most recent deferral — "you were here".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_point: Option<Deferral>,
    pub note: String,
}

fn prop(n: &crate::foundation::store::StoredNode, k: &str) -> Option<String> {
    n.properties
        .get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl DesignGraph {
    /// Every open deferred-derivation marker, oldest first. Open means no
    /// `valid_to` and no INVALIDATES edge pointing at it — the same rule the
    /// follow-up read applies.
    pub fn open_deferrals(&self) -> Result<Vec<Deferral>, DynoError> {
        let mut out = Vec::new();
        for n in self.scan_nodes(node::TEMPORAL_FACT)? {
            if prop(&n, "fact_type").as_deref() != Some(DEFERRED_DERIVATION) {
                continue;
            }
            if prop(&n, "valid_to").is_some() {
                continue;
            }
            if !self
                .incoming(&n.node_id, Some(edge::INVALIDATES))?
                .is_empty()
            {
                continue;
            }
            out.push(Deferral {
                fact_id: n.node_id.clone(),
                subject_id: prop(&n, "subject_id").unwrap_or_default(),
                statement: prop(&n, "statement")
                    .or_else(|| prop(&n, "name"))
                    .unwrap_or_default(),
                since: prop(&n, "valid_from"),
            });
        }
        out.sort_by(|a, b| a.since.cmp(&b.since).then(a.fact_id.cmp(&b.fact_id)));
        Ok(out)
    }

    /// The subjects under an open deferral — what the intent detectors skip.
    pub fn deferred_subjects(&self) -> Result<BTreeSet<String>, DynoError> {
        Ok(self
            .open_deferrals()?
            .into_iter()
            .map(|d| d.subject_id)
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Where an incremental adopt stands. See the module docs.
    pub fn frontier(
        &self,
        observed: Option<&[ObservedPath]>,
        exclusions: &[String],
    ) -> Result<FrontierReport, DynoError> {
        let deferred = self.open_deferrals()?;
        let under_deferral: BTreeSet<&str> =
            deferred.iter().map(|d| d.subject_id.as_str()).collect();

        let mut items: Vec<FrontierItem> = Vec::new();
        let mut caps = self.scan_live_nodes(node::CAPABILITY)?;
        caps.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        for cap in caps {
            if under_deferral.contains(cap.node_id.as_str())
                || self.is_discontinued(&cap.node_id)?
            {
                continue;
            }
            if self
                .outgoing(&cap.node_id, Some(edge::SATISFIES))?
                .is_empty()
            {
                items.push(FrontierItem {
                    node_id: cap.node_id.clone(),
                    node_type: node::CAPABILITY.to_string(),
                    why: "no requirement asks for it".to_string(),
                });
            }
        }
        let mut cmps = self.scan_live_nodes(node::COMPONENT)?;
        cmps.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        for cmp in cmps {
            if under_deferral.contains(cmp.node_id.as_str()) {
                continue;
            }
            // Leaves only: a parent is carried by what it contains, as the
            // detector already reads it.
            let mut has_child = false;
            for e in self.outgoing(&cmp.node_id, Some(edge::CONTAINS))? {
                if self.get_node(node::COMPONENT, &e.to_id)?.is_some() {
                    has_child = true;
                    break;
                }
            }
            if has_child {
                continue;
            }
            if self
                .incoming(&cmp.node_id, Some(edge::ALLOCATED_TO))?
                .is_empty()
            {
                items.push(FrontierItem {
                    node_id: cmp.node_id.clone(),
                    node_type: node::COMPONENT.to_string(),
                    why: "no capability is allocated to it".to_string(),
                });
            }
        }

        let uncaptured = match observed {
            Some(paths) => Some(
                self.coverage_report(paths, exclusions, None)?
                    .unclaimed_regions,
            ),
            None => None,
        };
        let resume_point = deferred.last().cloned();

        let note = format!(
            "{} node(s) captured structurally with no intent yet; {} deferred on purpose; {}. {}",
            items.len(),
            deferred.len(),
            match &uncaptured {
                Some(r) => format!("{} unclaimed region(s) in the sweep handed in", r.len()),
                None =>
                    "no sweep handed in, so what is adjacent and uncaptured is NOT known — hand \
                         `observed` paths (git ls-files, minus named exclusions) to see it"
                        .to_string(),
            },
            match &resume_point {
                Some(d) => format!(
                    "Resume at '{}' (deferred {}).",
                    d.subject_id,
                    d.since.as_deref().unwrap_or("undated")
                ),
                None =>
                    "No deferral recorded; nothing says where a previous pass stopped.".to_string(),
            }
        );

        Ok(FrontierReport {
            structure_without_intent: items,
            deferred,
            uncaptured,
            resume_point,
            note,
        })
    }
}
