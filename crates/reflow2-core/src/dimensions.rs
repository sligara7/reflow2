//! Depth axis — dimensional quality, and how it drifts over time
//! (docs/three-axes.md "depth", graph-analysis.md step 5).
//!
//! Every design node can carry a quality profile across axes (reliability,
//! maintainability, security, …). Each axis is measured as immutable, per-epoch
//! [`DimensionObservation`]s (`schema/dimensions.yaml`), which this module
//! **rolls up** into a current score and — the point of keeping them immutable —
//! lets quality **drift** across epochs be computed ("maintainability has been
//! sliding since v1.1"). The math is pure `dynograph-vector` stats
//! (`mean`, `linear_regression_slope`) — no embeddings, no LLM.
//!
//! Ordering: observations are ordered by their `observed_at` string, so a caller
//! that stamps it monotonically (an epoch sequence, or an ISO-8601 timestamp)
//! gets a true time series. Drift is the least-squares slope of score vs.
//! observation index — negative = declining.
//!
//! [`DimensionObservation`]: crate::nodes::node::DIMENSION_OBSERVATION

use dynograph_core::{DynoError, Value};
use dynograph_storage::StoredNode;
use dynograph_vector::{linear_regression_slope, mean};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// A quality axis — mirrors `dimensions.yaml`'s `dimension` enum exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Reliability,
    Performance,
    Maintainability,
    Security,
    Scalability,
    Observability,
    Testability,
    Coupling,
    Maturity,
}

impl Dimension {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Reliability => "reliability",
            Dimension::Performance => "performance",
            Dimension::Maintainability => "maintainability",
            Dimension::Security => "security",
            Dimension::Scalability => "scalability",
            Dimension::Observability => "observability",
            Dimension::Testability => "testability",
            Dimension::Coupling => "coupling",
            Dimension::Maturity => "maturity",
        }
    }

    /// Parse a stored dimension string (as produced by [`as_str`](Self::as_str))
    /// back into the enum.
    pub fn from_key(s: &str) -> Option<Dimension> {
        Some(match s {
            "reliability" => Dimension::Reliability,
            "performance" => Dimension::Performance,
            "maintainability" => Dimension::Maintainability,
            "security" => Dimension::Security,
            "scalability" => Dimension::Scalability,
            "observability" => Dimension::Observability,
            "testability" => Dimension::Testability,
            "coupling" => Dimension::Coupling,
            "maturity" => Dimension::Maturity,
            _ => return None,
        })
    }
}

/// Which way a node's quality is trending on an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDirection {
    /// Slope clearly positive.
    Improving,
    /// Slope within the flat band.
    Stable,
    /// Slope clearly negative — the one to surface.
    Declining,
}

/// A node's drift on one dimension, rolled up from its observations.
#[derive(Debug, Clone)]
pub struct DimensionDrift {
    /// The assessed node.
    pub target_id: String,
    /// The axis.
    pub dimension: Dimension,
    /// How many observations backed this.
    pub observation_count: usize,
    /// Rolled-up current score = mean of observations (0..1).
    pub rollup_score: f64,
    /// Least-squares slope of score vs. observation index (per step).
    pub slope: f64,
    /// Classification of `slope` against the flat band.
    pub direction: DriftDirection,
    /// Earliest observation's score.
    pub first_score: f64,
    /// Latest observation's score.
    pub last_score: f64,
}

/// Slopes within ±this of zero are treated as `Stable` (not drifting).
const DRIFT_EPSILON: f64 = 0.01;

impl DesignGraph {
    /// Record an immutable per-epoch [`DimensionObservation`] and wire it to its
    /// target (`HAS_OBSERVATION`) and, if given, its source Fragment
    /// (`OBSERVED_IN`). `observed_at` should be monotonic (epoch sequence / ISO
    /// timestamp) so drift reads as a true time series.
    #[allow(clippy::too_many_arguments)]
    pub fn add_dimension_observation(
        &mut self,
        id: &str,
        target_type: &str,
        target_id: &str,
        dimension: Dimension,
        score: f64,
        observed_at: &str,
        source_fragment_id: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        let node = self.create_node(
            node::DIMENSION_OBSERVATION,
            id,
            Props::new()
                .set("target_id", target_id)
                .set("target_type", target_type)
                .set("dimension", dimension.as_str())
                .set("score", score)
                .set("observed_at", observed_at)
                .set_opt("source_fragment_id", source_fragment_id),
        )?;
        self.create_edge(
            edge::HAS_OBSERVATION,
            target_type,
            target_id,
            node::DIMENSION_OBSERVATION,
            id,
            Props::new(),
        )?;
        if let Some(frag) = source_fragment_id {
            self.create_edge(
                edge::OBSERVED_IN,
                node::DIMENSION_OBSERVATION,
                id,
                node::FRAGMENT,
                frag,
                Props::new(),
            )?;
        }
        Ok(node)
    }

    /// Compute a node's drift on one dimension from its observations
    /// (`HAS_OBSERVATION`). `Ok(None)` when it has none.
    pub fn dimension_drift(
        &self,
        target_id: &str,
        dimension: Dimension,
    ) -> Result<Option<DimensionDrift>, DynoError> {
        // (observed_at, score) for this target+dimension, via HAS_OBSERVATION.
        let mut series: Vec<(String, f64)> = Vec::new();
        for e in self.outgoing(target_id, Some(edge::HAS_OBSERVATION))? {
            let Some(obs) = self.get_node(node::DIMENSION_OBSERVATION, &e.to_id)? else {
                continue;
            };
            if obs.properties.get("dimension").and_then(Value::as_str) != Some(dimension.as_str()) {
                continue;
            }
            if let Some(score) = obs.properties.get("score").and_then(Value::as_f64) {
                let at = obs
                    .properties
                    .get("observed_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                series.push((at, score));
            }
        }
        if series.is_empty() {
            return Ok(None);
        }
        // Order by observed_at, then take scores as a time series.
        series.sort_by(|a, b| a.0.cmp(&b.0));
        let scores: Vec<f64> = series.iter().map(|(_, s)| *s).collect();

        let rollup_score = mean(&scores).unwrap_or(0.0);
        let points: Vec<(f64, f64)> = scores
            .iter()
            .enumerate()
            .map(|(i, &s)| (i as f64, s))
            .collect();
        // <2 points → slope undefined → treat as no drift.
        let slope = linear_regression_slope(&points).unwrap_or(0.0);
        let direction = if slope > DRIFT_EPSILON {
            DriftDirection::Improving
        } else if slope < -DRIFT_EPSILON {
            DriftDirection::Declining
        } else {
            DriftDirection::Stable
        };

        Ok(Some(DimensionDrift {
            target_id: target_id.to_string(),
            dimension,
            observation_count: scores.len(),
            rollup_score,
            slope,
            direction,
            first_score: scores[0],
            last_score: *scores.last().expect("non-empty"),
        }))
    }

    /// Drift for every (node, dimension) with at least one observation, ranked
    /// **most-declining first** (steepest negative slope) so the worst quality
    /// erosion surfaces first; ties broken by id + dimension for determinism.
    pub fn dimension_drifts(&self) -> Result<Vec<DimensionDrift>, DynoError> {
        // Distinct (target_id, dimension) pairs across all observations.
        let mut seen: std::collections::HashSet<(String, Dimension)> =
            std::collections::HashSet::new();
        for obs in self.scan_nodes(node::DIMENSION_OBSERVATION)? {
            let (Some(t), Some(d)) = (
                obs.properties.get("target_id").and_then(Value::as_str),
                obs.properties
                    .get("dimension")
                    .and_then(Value::as_str)
                    .and_then(Dimension::from_key),
            ) else {
                continue;
            };
            seen.insert((t.to_string(), d));
        }

        let mut out = Vec::new();
        for (target_id, dimension) in seen {
            if let Some(drift) = self.dimension_drift(&target_id, dimension)? {
                out.push(drift);
            }
        }
        out.sort_by(|a, b| {
            a.slope
                .partial_cmp(&b.slope)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.target_id.cmp(&b.target_id))
                .then(a.dimension.as_str().cmp(b.dimension.as_str()))
        });
        Ok(out)
    }

    /// Materialize the current [`DimensionAssessment`] for a node+dimension by
    /// rolling up its observations (score = mean, evidence_count = how many).
    /// Returns `Ok(None)` when there is nothing to roll up. This is the
    /// observation→assessment rollup of docs/three-axes.md (3AX-8).
    ///
    /// [`DimensionAssessment`]: crate::nodes::node::DIMENSION_ASSESSMENT
    pub fn rollup_assessment(
        &mut self,
        assessment_id: &str,
        target_type: &str,
        target_id: &str,
        dimension: Dimension,
    ) -> Result<Option<StoredNode>, DynoError> {
        let Some(drift) = self.dimension_drift(target_id, dimension)? else {
            return Ok(None);
        };
        let assessment = self.create_node(
            node::DIMENSION_ASSESSMENT,
            assessment_id,
            Props::new()
                .set("target_id", target_id)
                .set("target_type", target_type)
                .set("dimension", dimension.as_str())
                .set("score", drift.rollup_score)
                .set("evidence_count", drift.observation_count as i64),
        )?;
        self.create_edge(
            edge::ASSESSED_ON,
            node::DIMENSION_ASSESSMENT,
            assessment_id,
            target_type,
            target_id,
            Props::new(),
        )?;
        Ok(Some(assessment))
    }
}
