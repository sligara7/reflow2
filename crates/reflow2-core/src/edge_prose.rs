//! Edge prose is a claim about the world, and it goes stale like any other.
//!
//! # What this exists for
//!
//! An edge's `evidence` and `note` are written at a moment in time and are
//! exactly as perishable as a node's `statement` — and until 2026-08-31 nothing
//! in reflow2 read them. Node prose was governed three ways (every `add_*`
//! warns on overwrite, `reconcile_artifacts` catches design-vs-file drift,
//! `change_axis_unstated` catches an unstated axis); edge prose had none of the
//! three.
//!
//! Reported by bhome (`fact:edge-prose-is-as-perishable-as-node-prose-and-nothing-reads-it`):
//! narrowing a climate requirement from desert-to-arctic to lower-48 city
//! climates left five `RISKS` edges still arguing about the arctic, and four
//! were repaired only because the propagation happened to list them and the
//! agent happened to remember what they said.
//!
//! # 🛑 Why this compares against the PRIOR state, not just the current one
//!
//! The obvious implementation — flag any significant term in the edge prose
//! that is absent from the node — was written first and REJECTED because it
//! fires on healthy edges. Measured on the reporter's own example: an edge
//! whose evidence still holds ("lower-48 city climates still demand an envelope
//! that operates without supplemental heating") contains `demand` and
//! `envelope`, neither of which ever appeared in the requirement. A check that
//! flags that is noise, and noise on this surface has a measured cost — the
//! near-match guard fired 15 times with zero true positives and agents learned
//! to pre-empt it, at which point it had stopped being a check at all.
//!
//! ⭐ SO THE SIGNAL IS NARROWER AND SHARPER: a term the edge prose still uses,
//! which the node USED TO say and no longer does. That is recoverable, because
//! `record_change` snapshots the prior state before an edit — so the design
//! already holds the one thing that separates "this edge outlived the node"
//! from "this edge was always about something wider".
//!
//! # And when it cannot run, it says so
//!
//! A node with no snapshot has no prior text, so nothing can be concluded — and
//! reporting that as "no stale edges" would be the failure this project has a
//! name for: silent about the question rather than clean on it. [`EdgeProseReport`]
//! carries a `coverage_note` for exactly that case.

use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;
use crate::nodes::edge;
use std::collections::{BTreeSet, HashMap};

/// Words too common to carry a claim. Deliberately tiny: the length floor does
/// most of the work, and a long stopword list is a tuning surface nobody has
/// evidence to tune.
const STOPWORDS: &[&str] = &[
    "about", "after", "again", "against", "because", "before", "being", "below", "between",
    "could", "during", "every", "from", "further", "having", "into", "itself", "least", "more",
    "most", "must", "other", "over", "same", "should", "since", "some", "such", "than", "that",
    "their", "them", "then", "there", "these", "they", "this", "those", "through", "under",
    "until", "very", "were", "what", "when", "where", "which", "while", "with", "would", "your",
];

/// Below this many characters a word is not distinctive enough to carry the
/// claim "this edge outlived the node".
const MIN_TERM_LEN: usize = 5;

/// At most this many terms are named per edge. The point is to give the reader
/// a handle, not to reproduce the diff.
const MAX_TERMS: usize = 6;

/// One edge whose prose still uses language the node it touches has dropped.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StaleEdgeEvidence {
    pub edge_type: String,
    /// The node at the OTHER end of this edge.
    pub other_id: String,
    /// `outgoing` or `incoming`, relative to the node asked about.
    pub direction: &'static str,
    /// Which prose field carries it — `evidence` or `note`.
    pub field: String,
    /// The prose itself, so the reader can judge without a second call.
    pub prose: String,
    /// Terms this prose still uses that the node used to say and no longer
    /// does. This is the whole finding — naming the edge alone would leave the
    /// reader to diff two paragraphs by eye.
    pub absent_terms: Vec<String>,
}

/// What [`DesignGraph::stale_edge_evidence`] found, and what it could not look at.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EdgeProseReport {
    pub findings: Vec<StaleEdgeEvidence>,
    /// How many incident edges actually carried prose to examine. An empty
    /// `findings` over zero of these is a different fact from an empty
    /// `findings` over twenty.
    pub edges_with_prose: usize,
    /// Present ONLY when the check could not have found anything — no prior
    /// state to compare against. A zero here means "silent", never "clean".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_note: Option<String>,
    /// What this check cannot tell, carried with every answer rather than left
    /// in the docs where a reader quoting the result will not see it.
    pub caveat: &'static str,
}

const CAVEAT: &str = "A LEXICAL CHECK, NOT A SEMANTIC ONE. It reports that an edge's prose still \
                      uses words the node has dropped — which is usually staleness and is \
                      sometimes only rewording. It never refuses a write and nothing is repaired \
                      for you: whether the evidence is now wrong is a judgement, and re-writing \
                      the edge with create_edges upserts its properties cleanly.";

/// Significant terms of a piece of prose, lowercased and deduplicated.
fn terms(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.trim().to_lowercase())
        .filter(|w| {
            w.len() >= MIN_TERM_LEN
                && w.chars().any(|c| c.is_alphabetic())
                && !STOPWORDS.contains(&w.as_str())
        })
        .collect()
}

/// Every string property of a node, joined — what the node "says" right now.
fn text_of(props: &HashMap<String, Value>) -> String {
    let mut parts: Vec<&str> = props.values().filter_map(Value::as_str).collect();
    parts.sort_unstable();
    parts.join(" ")
}

/// The same, over a snapshot's captured `state`, which is stored as JSON text
/// rather than as live properties.
fn text_of_state(state: &str) -> Option<String> {
    let props: serde_json::Map<String, serde_json::Value> = serde_json::from_str(state).ok()?;
    let mut parts: Vec<&str> = props
        .values()
        .filter_map(serde_json::Value::as_str)
        .collect();
    parts.sort_unstable();
    Some(parts.join(" "))
}

impl DesignGraph {
    /// Edges touching `node_id` whose prose still uses language the node has
    /// since dropped.
    ///
    /// See the module docs for why this compares against the node's most recent
    /// snapshot rather than only against its current text.
    pub fn stale_edge_evidence(&self, node_id: &str) -> Result<EdgeProseReport, DynoError> {
        let index = self.node_type_index()?;
        let Some(node_type) = index.get(node_id) else {
            return Ok(EdgeProseReport {
                findings: Vec::new(),
                edges_with_prose: 0,
                coverage_note: Some(format!(
                    "'{node_id}' names no node in this design, so there was nothing to compare an \
                     edge's prose against."
                )),
                caveat: CAVEAT,
            });
        };
        let Some(current) = self.get_node(node_type, node_id)? else {
            return Ok(EdgeProseReport {
                findings: Vec::new(),
                edges_with_prose: 0,
                coverage_note: Some(format!("'{node_id}' could not be read.")),
                caveat: CAVEAT,
            });
        };
        let now = terms(&text_of(&current.properties));

        // The prior text: the most recent snapshot's captured state. Without one
        // there is no "used to say", and the honest answer is that the check
        // could not run — NOT that nothing was found.
        let prior: Option<BTreeSet<String>> = self
            .snapshots_of(node_id)?
            .last()
            .and_then(|s| s.properties.get("state").and_then(Value::as_str))
            .and_then(text_of_state)
            .map(|text| terms(&text));

        let Some(prior) = prior else {
            return Ok(EdgeProseReport {
                findings: Vec::new(),
                edges_with_prose: 0,
                coverage_note: Some(format!(
                    "No snapshot holds a PRIOR state for '{node_id}', so this check is silent \
                     about its edges rather than clean about them — with nothing to compare \
                     against, an edge whose prose outlived the node is indistinguishable from one \
                     that never matched it. `record_change` before an edit is what captures the \
                     prior state this reads."
                )),
                caveat: CAVEAT,
            });
        };

        // What the node USED to say and no longer does. Everything else is
        // either still true of the node or was never about it.
        let dropped: BTreeSet<&String> = prior.difference(&now).collect();

        let mut findings = Vec::new();
        let mut edges_with_prose = 0usize;
        let incident = self
            .outgoing(node_id, None)?
            .into_iter()
            .map(|e| (e, "outgoing"))
            .chain(
                self.incoming(node_id, None)?
                    .into_iter()
                    .map(|e| (e, "incoming")),
            );
        for (stored, direction) in incident {
            // Bookkeeping edges carry no design claim, and their prose is not
            // written by anybody making an argument.
            if stored.edge_type == edge::HAS_SNAPSHOT || stored.edge_type == edge::AT_EPOCH {
                continue;
            }
            for field in ["evidence", "note"] {
                let Some(prose) = stored.properties.get(field).and_then(Value::as_str) else {
                    continue;
                };
                if prose.trim().is_empty() {
                    continue;
                }
                edges_with_prose += 1;
                let used = terms(prose);
                let mut absent: Vec<String> = dropped
                    .iter()
                    .filter(|t| used.contains(**t))
                    .map(|t| (*t).clone())
                    .collect();
                if absent.is_empty() {
                    continue;
                }
                absent.truncate(MAX_TERMS);
                let other_id = if direction == "outgoing" {
                    stored.to_id.clone()
                } else {
                    stored.from_id.clone()
                };
                findings.push(StaleEdgeEvidence {
                    edge_type: stored.edge_type.clone(),
                    other_id,
                    direction,
                    field: field.to_string(),
                    prose: prose.to_string(),
                    absent_terms: absent,
                });
            }
        }
        findings.sort_by(|a, b| {
            (&a.edge_type, &a.other_id, &a.field).cmp(&(&b.edge_type, &b.other_id, &b.field))
        });

        Ok(EdgeProseReport {
            findings,
            edges_with_prose,
            coverage_note: None,
            caveat: CAVEAT,
        })
    }
}
