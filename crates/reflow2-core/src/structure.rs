//! Structural topology analysis — a `dynograph-graph` view of the design
//! network, powering HEAL's graph-algorithm defect detectors.
//!
//! The **design network** is the undirected graph of design nodes connected by
//! *traceability* edges (the same coupling set PROPAGATE walks — see
//! [`crate::propagate::is_traceability_edge`] — which excludes CONTAINS so the
//! Project isn't an artificial hub). Bookkeeping node types (Project, temporal,
//! dimensions) are excluded: they aren't part of the design's structural
//! coupling.
//!
//! Over that network HEAL runs exact graph algorithms from `dynograph-graph`:
//! connected components (islands), articulation points (single points of
//! failure), and degree (dead ends). A caution the algorithms alone don't give
//! you: a design's golden thread is largely a **tree**, and in a tree *every*
//! internal node is an articulation point — so the SPOF detector must be
//! *selective* (see [`DesignGraph::is_single_point_of_failure`]) or it fires
//! everywhere and means nothing.

use std::collections::{HashMap, HashSet};

use dynograph_core::DynoError;
use dynograph_graph::{
    Graph, GraphBuilder, betweenness_centrality, connected_components, cut_structure,
};

use crate::graph::DesignGraph;
use crate::propagate::is_traceability_edge;

/// Node types that are *not* part of the design's structural coupling and so are
/// excluded from the design network.
const NON_DESIGN_TYPES: &[&str] = &[
    "Project",
    "DesignEpoch",
    "TemporalFact",
    "Snapshot",
    "ChangeEvent",
    "DimensionAssessment",
    "DimensionObservation",
];

/// A `dynograph-graph` view of the design network plus the id/type of each dense
/// node index (the algorithms return index-keyed results to map back).
pub(crate) struct DesignNetwork {
    graph: Graph,
    /// `meta[idx] = (node_id, node_type)`.
    meta: Vec<(String, String)>,
}

impl DesignNetwork {
    /// Number of nodes in the network.
    pub(crate) fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Id of a node index.
    pub(crate) fn id_of(&self, idx: usize) -> &str {
        &self.meta[idx].0
    }

    /// Type of a node index.
    pub(crate) fn type_of(&self, idx: usize) -> &str {
        &self.meta[idx].1
    }

    /// Undirected degree of a node index (incident traceability edges).
    pub(crate) fn degree(&self, idx: usize) -> usize {
        self.graph.out_neighbors(idx).len()
    }

    /// Connected components as groups of node indices.
    pub(crate) fn component_groups(&self) -> Vec<Vec<usize>> {
        connected_components(&self.graph).groups()
    }

    /// Articulation-point node indices (candidate single points of failure).
    pub(crate) fn articulation_points(&self) -> Vec<usize> {
        cut_structure(&self.graph).articulation_points
    }

    /// Betweenness centrality (normalized 0..1, unweighted topology) per node id
    /// — "how much of the golden thread routes through this node". Powers
    /// PROPAGATE's centrality-weighted ranking (IP-9): a change landing on a
    /// high-betweenness node has a wider secondary blast radius.
    pub(crate) fn betweenness(&self) -> Result<HashMap<String, f64>, DynoError> {
        let scores = betweenness_centrality(&self.graph, false, true)
            .map_err(|e| DynoError::Query(format!("betweenness centrality: {e}")))?;
        Ok(scores
            .iter()
            .enumerate()
            .map(|(idx, &s)| (self.id_of(idx).to_string(), s))
            .collect())
    }

    /// Count of components with ≥2 nodes — the "non-trivial subsystems".
    fn nontrivial_component_count(&self) -> usize {
        self.component_groups()
            .iter()
            .filter(|g| g.len() >= 2)
            .count()
    }
}

impl DesignGraph {
    /// Build the design network, optionally excluding one node id (used to test
    /// what a node's removal would disconnect).
    fn build_network(&self, exclude: Option<&str>) -> Result<DesignNetwork, DynoError> {
        let index = self.node_type_index()?;
        let included: HashSet<&str> = index
            .iter()
            .filter(|(id, ty)| {
                !NON_DESIGN_TYPES.contains(&ty.as_str()) && exclude != Some(id.as_str())
            })
            .map(|(id, _)| id.as_str())
            .collect();

        let mut builder = GraphBuilder::new();
        // Add every included node so isolated ones still appear as components.
        for id in &included {
            builder.add_node(id);
        }
        for id in &included {
            for e in self.outgoing(id, None)? {
                if is_traceability_edge(&e.edge_type) && included.contains(e.to_id.as_str()) {
                    builder
                        .add_edge(&e.from_id, &e.to_id, 1.0)
                        .map_err(|err| DynoError::Query(format!("design network edge: {err}")))?;
                }
            }
        }
        let graph = builder.build(false); // undirected: structural coupling is symmetric

        let mut meta = vec![(String::new(), String::new()); graph.node_count()];
        for id in &included {
            if let Some(idx) = graph.idx_of(id) {
                meta[idx] = (id.to_string(), index[*id].clone());
            }
        }
        Ok(DesignNetwork { graph, meta })
    }

    /// The design network over all design nodes.
    pub(crate) fn design_network(&self) -> Result<DesignNetwork, DynoError> {
        self.build_network(None)
    }

    /// Whether removing `node_id` would split the design into **≥2 non-trivial
    /// subsystems** (each ≥2 nodes) — the selective SPOF test that ignores the
    /// leaf-cutting every tree-internal node trivially does.
    pub(crate) fn is_single_point_of_failure(&self, node_id: &str) -> Result<bool, DynoError> {
        Ok(self
            .build_network(Some(node_id))?
            .nontrivial_component_count()
            >= 2)
    }
}
