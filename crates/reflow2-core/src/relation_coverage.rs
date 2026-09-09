//! *"Of my N things of kind X, how many carry relation R?"* — asked of any kind
//! and any relation the schema declares, without anybody writing a script.
//!
//! # Why this is a requirement and not a convenience
//!
//! `req:a-design-can-be-asked-what-fraction-of-a-kind-carries-a-relation`,
//! Anthony's word 2026-08-31, stated as a PROPERTY rather than a design. It is
//! the core traceability question of systems engineering, in the user's own
//! words rather than reflow2's: **how many of my requirements are actually
//! verified? how many of my components have a declared interface? how many of
//! my capabilities have anything that consumes them?**
//!
//! The evidence was that every session which needed this wrote its own script.
//! `fact:five-capabilities-...-one-query-shape-found-them-all` recorded five
//! instances, all hand-rolled. It kept happening: on **2026-09-09**, in the
//! session that finally built this, six more — how many field reports carry a
//! `DOCUMENTS` edge, how many nodes a `GOVERNED_BY` predicate selects, which
//! artifacts changed between two exports. Every one of those was a question
//! reflow2 should have answered about itself and instead answered through
//! `python3 - <<EOF`.
//!
//! # ⭐ Why an unknown type is REFUSED and never counted as zero
//!
//! This is the whole design risk, and it is the one this project has met
//! repeatedly: **a detector reporting zero because it had NOTHING TO RUN ON
//! reads exactly like one that ran clean.** `Requirment` (sic) against a design
//! of 208 Requirements would answer `0 of 0 — 100%` and look like good news.
//!
//! So a `node_type` or `edge_type` the schema does not declare is an error
//! naming the near-misses, never an empty result. `loop_status` already refuses
//! an unknown `contributor_id` for exactly this reason; this applies the same
//! rule one door over.
//!
//! # What it does not do
//!
//! It does not judge. A fraction is not a score and there is no threshold: a
//! design where 12% of capabilities are consumed may be perfectly healthy at
//! this stage of its life, and reflow2 has no way to know which
//! (`dec:report-dont-judge`). It reports the number and names the population it
//! came from.

use crate::foundation::core::DynoError;
use crate::graph::DesignGraph;

/// How many nodes of one kind carry one relation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelationCoverage {
    pub node_type: String,
    pub edge_type: String,
    /// `outgoing` (the node is the SOURCE) or `incoming` (the node is the
    /// TARGET). Stated back, because the same pair reads very differently each
    /// way: Requirements with an incoming VERIFIES is "how many are verified";
    /// outgoing is a question about requirements that verify things, which is
    /// not a thing.
    pub direction: String,
    /// Live nodes of this kind — the denominator, and the number that says
    /// whether the fraction means anything.
    pub population: usize,
    /// …of which some carry the relation.
    pub with_relation: usize,
    /// …and the rest do not.
    pub without: usize,
    /// `with_relation / population`, or `None` when the population is empty —
    /// never 1.0, and never 0.0, because a fraction of nothing is not a fact.
    pub fraction: Option<f64>,
    /// The ids that do NOT carry it, so the answer is actionable and not just a
    /// number. Capped; `without` is the true count.
    pub missing: Vec<String>,
    /// Set when `missing` was truncated.
    pub missing_truncated: bool,
    /// The line a reader needs first, saying which kind of empty an empty
    /// answer is.
    pub note: String,
}

/// How many `missing` ids come back before the list is cut.
const MISSING_CAP: usize = 50;

/// Which way the edge points relative to the nodes being counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The counted node is the edge's source.
    Outgoing,
    /// The counted node is the edge's target.
    Incoming,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Outgoing => "outgoing",
            Direction::Incoming => "incoming",
        }
    }
}

impl DesignGraph {
    /// See the module docs. Refuses an undeclared `node_type` or `edge_type`.
    pub fn relation_coverage(
        &self,
        node_type: &str,
        edge_type: &str,
        direction: Direction,
    ) -> Result<RelationCoverage, DynoError> {
        // ⭐ REFUSE BEFORE COUNTING. See the module docs: a typo that returned
        // `0 of 0` would be indistinguishable from an honest clean answer, and
        // the near-miss list is what makes the refusal useful rather than
        // merely correct.
        if !self.schema().node_types.contains_key(node_type) {
            return Err(DynoError::Validation {
                node_type: node_type.into(),
                property: "node_type".into(),
                message: format!(
                    "this design declares no node type '{node_type}', so counting it would answer \
                     '0 of 0' about a kind that does not exist — which reads exactly like a clean \
                     result. Did you mean one of: {}?",
                    near_misses(
                        self.schema().node_types.keys().map(String::as_str),
                        node_type
                    )
                ),
            });
        }
        if !self.schema().edge_types.contains_key(edge_type) {
            return Err(DynoError::Validation {
                node_type: node_type.into(),
                property: "edge_type".into(),
                message: format!(
                    "this design declares no edge type '{edge_type}', so every node would count as \
                     missing it and the answer would read as total absence rather than as a typo. \
                     Did you mean one of: {}?",
                    near_misses(
                        self.schema().edge_types.keys().map(String::as_str),
                        edge_type
                    )
                ),
            });
        }

        let mut with = 0usize;
        let mut missing: Vec<String> = Vec::new();
        let nodes = self.scan_live_nodes(node_type)?;
        let population = nodes.len();
        for n in nodes {
            let edges = match direction {
                Direction::Outgoing => self.outgoing(&n.node_id, Some(edge_type))?,
                Direction::Incoming => self.incoming(&n.node_id, Some(edge_type))?,
            };
            if edges.is_empty() {
                missing.push(n.node_id);
            } else {
                with += 1;
            }
        }
        missing.sort();
        let without = missing.len();
        let missing_truncated = missing.len() > MISSING_CAP;
        missing.truncate(MISSING_CAP);

        let note = if population == 0 {
            format!(
                "NOTHING TO EXAMINE: this design holds no live {node_type}, so there is no \
                 fraction to report. The type IS declared — this is an empty population, not a \
                 typo."
            )
        } else if without == 0 {
            format!("All {population} live {node_type}(s) carry {direction:?} {edge_type}.")
        } else {
            format!(
                "{with} of {population} live {node_type}(s) carry {} {edge_type}. \
                 A fraction is not a score: reflow2 has no threshold for this and does not know \
                 what yours should be.",
                direction.as_str()
            )
        };

        Ok(RelationCoverage {
            node_type: node_type.to_string(),
            edge_type: edge_type.to_string(),
            direction: direction.as_str().to_string(),
            population,
            with_relation: with,
            without,
            // A fraction of nothing is not a fact. Returning 0.0 or 1.0 here
            // would put a number in front of a reader that no data supports.
            fraction: (population > 0).then(|| with as f64 / population as f64),
            missing,
            missing_truncated,
            note,
        })
    }
}

/// The declared names closest to what was asked for, so a refusal points
/// somewhere instead of only saying no.
fn near_misses<'a>(declared: impl Iterator<Item = &'a str>, asked: &str) -> String {
    let want = asked.to_ascii_lowercase();
    let mut hits: Vec<&str> = declared
        .filter(|d| {
            let d = d.to_ascii_lowercase();
            d.starts_with(&want[..want.len().min(4)]) || d.contains(&want) || want.contains(&d)
        })
        .collect();
    hits.sort_unstable();
    hits.truncate(5);
    if hits.is_empty() {
        String::from("(no close match; call validate_schema or read the schema for the full list)")
    } else {
        hits.join(", ")
    }
}
