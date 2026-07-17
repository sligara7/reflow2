//! PROPAGATE — walk the golden thread to find a change's blast radius.
//!
//! The coherence loop's second step (docs/impact-propagation.md). Given the
//! seed node(s) a change touched, this walks the traceability edges to find
//! everything the change may have broken, and — the maturation over a plain
//! undirected BFS — classifies *why* each node is impacted by the **semantic
//! direction** of the edge it was reached through.
//!
//! This increment implements the engine core from the doc's "reuse vs build"
//! table: **direction-classified bounded BFS** with an *explained* blast radius
//! (every impacted node carries its `via` edge chain) and **no silent
//! truncation** (nodes past the depth bound are counted and reported, never
//! dropped). Deferred to later increments, noted where they slot in:
//! impact-*kind* tagging (pairs with DETECT), centrality/confidence ranking
//! (needs `dynograph-graph` + inference `confidence`), and result caching.
//! Ranking here is by distance, with risk-edge crossings amplified.
//!
//! It only computes and tags — turning tags into questions is SURFACE and
//! repair is HEAL (discipline 5: feed the loop, don't fix).

use std::collections::{HashMap, HashSet, VecDeque};

use dynograph_core::DynoError;

use crate::graph::DesignGraph;
use crate::nodes::edge;

/// The semantic direction of an impact hop (docs/impact-propagation.md,
/// "Direction matters").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactDirection {
    /// Realization: "what did this node's existence justify or shape?"
    Downstream,
    /// Rationale: "what intent does this node serve, that may now be unmet?"
    Upstream,
    /// Peers/contracts: "what shares a contract or depends sideways?"
    Lateral,
    /// Inference: "what did this cause / enable / risk?"
    Causal,
}

impl ImpactDirection {
    /// A short, stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            ImpactDirection::Downstream => "downstream",
            ImpactDirection::Upstream => "upstream",
            ImpactDirection::Lateral => "lateral",
            ImpactDirection::Causal => "causal",
        }
    }
}

/// Risk edges whose crossing amplifies severity — kept verbatim from Reflow's
/// `risk_rel_types` (docs/impact-propagation.md, "Ranking the blast radius").
const RISK_EDGES: &[&str] = &["RISKS", "BLOCKS", "CONTRADICTS", "VIOLATES", "MASKS"];

/// How a structural edge propagates impact when walked **forward** (along an
/// outgoing edge) vs **backward** (along an incoming edge). `None` on a side
/// means impact does not propagate that way, so the traversal never crosses it
/// (and can therefore always explain why a node is in the blast radius).
struct EdgeRule {
    forward: Option<ImpactDirection>,
    backward: Option<ImpactDirection>,
}

/// The structural golden-thread direction table (docs/impact-propagation.md).
/// Inference edges are not here — they are classified as [`Causal`] at runtime
/// from `schema.inference_edge_types()`. Structural edges not listed (e.g.
/// SPECIFIES, DOCUMENTS, temporal bookkeeping) are intentionally not traversed
/// in this increment.
///
/// [`Causal`]: ImpactDirection::Causal
fn structural_rule(edge_type: &str) -> Option<EdgeRule> {
    use ImpactDirection::{Downstream, Lateral, Upstream};
    let (fwd, bwd) = match edge_type {
        // Note: CONTAINS (decomposition, axis Y) is deliberately *not* here.
        // It is not a traceability edge; propagating along it would make the
        // Project a hub that short-circuits every sibling to ~2 hops. The doc's
        // impact diagram omits it too.
        //
        // Traceability: Capability SATISFIES Requirement. From the requirement
        // (incoming) you reach the realizer that may now be wrong (downstream);
        // from the capability (outgoing) you reach the intent it serves (upstream).
        "SATISFIES" => (Some(Upstream), Some(Downstream)),
        // A node CONSTRAINS another it shapes.
        "CONSTRAINS" => (Some(Downstream), Some(Upstream)),
        // WHAT→WHERE: Capability ALLOCATED_TO Component.
        "ALLOCATED_TO" => (Some(Downstream), Some(Upstream)),
        // Realization: Artifact REALIZES Capability/Component/Interface.
        "REALIZES" => (Some(Upstream), Some(Downstream)),
        // Verification VERIFIES its target; a moved target staled it.
        "VERIFIES" => (Some(Upstream), Some(Downstream)),
        // Governance: source GOVERNED_BY a Decision/DesignRule.
        "GOVERNED_BY" => (Some(Upstream), Some(Downstream)),
        // Contracts / dependencies — sideways.
        "PROVIDES" | "CONSUMES" | "DEPENDS_ON" | "PART_OF_FLOW" => (Some(Lateral), Some(Lateral)),
        // Operation chain.
        "DEPLOYED_TO" | "REQUIRES_RESOURCE" => (Some(Downstream), Some(Upstream)),
        _ => return None,
    };
    Some(EdgeRule {
        forward: fwd,
        backward: bwd,
    })
}

/// One hop in an impact chain — the edge that carried impact to a node, and how.
#[derive(Debug, Clone)]
pub struct Hop {
    /// Edge type traversed.
    pub edge_type: String,
    /// Direction the impact flowed.
    pub direction: ImpactDirection,
    /// Source id of the edge (graph orientation, not traversal orientation).
    pub from_id: String,
    /// Target id of the edge (graph orientation).
    pub to_id: String,
    /// Whether this edge is a risk edge (amplifies severity).
    pub is_risk: bool,
}

/// A node reached by propagation, with the explanation of why (discipline 2:
/// explain every impact).
#[derive(Debug, Clone)]
pub struct ImpactedNode {
    /// The impacted node's id.
    pub node_id: String,
    /// The impacted node's type.
    pub node_type: String,
    /// Hop count from the nearest seed (1 = direct).
    pub distance: usize,
    /// Direction of the final hop that reached it (its shortest path).
    pub direction: ImpactDirection,
    /// The edge chain from a seed to this node — the explanation.
    pub via: Vec<Hop>,
    /// Whether any hop on `via` crossed a risk edge.
    pub crosses_risk_edge: bool,
}

/// The computed blast radius of a change.
#[derive(Debug, Clone)]
pub struct BlastRadius {
    /// Seed node ids the propagation started from.
    pub seeds: Vec<String>,
    /// Seeds that were not found as real nodes (surfaced, never silently
    /// dropped).
    pub unknown_seeds: Vec<String>,
    /// Impacted nodes, ranked most-relevant-first.
    pub impacted: Vec<ImpactedNode>,
    /// The depth bound the traversal used.
    pub max_depth: usize,
    /// Count of further nodes reachable *beyond* `max_depth` that were not
    /// expanded — reported, not hidden (discipline 1: never silently truncate).
    pub truncated_beyond_depth: usize,
}

impl BlastRadius {
    /// Whether the traversal stopped short of the full radius at the depth bound.
    pub fn was_truncated(&self) -> bool {
        self.truncated_beyond_depth > 0
    }
}

/// Options for a propagation run.
#[derive(Debug, Clone, Copy)]
pub struct PropagateOptions {
    /// Maximum hop distance to expand. Beyond this, nodes are counted toward
    /// [`BlastRadius::truncated_beyond_depth`] but not walked further.
    pub max_depth: usize,
}

impl Default for PropagateOptions {
    fn default() -> Self {
        Self { max_depth: 5 }
    }
}

/// A neighbor reached across one edge, pre-classified.
struct Neighbor {
    id: String,
    hop: Hop,
}

impl DesignGraph {
    /// Build an id→type index over the whole project subgraph. Edge adjacency
    /// carries only endpoint ids; this resolves a node's type (and confirms it
    /// exists — dangling edges to absent nodes are excluded from the radius).
    ///
    /// Assumes node ids are unique across types within a graph (reflow2's typed-
    /// prefix id convention, e.g. `req:`, `cap:`); on a collision the first
    /// type scanned wins.
    fn node_type_index(&self) -> Result<HashMap<String, String>, DynoError> {
        let mut index = HashMap::new();
        let types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        for node_type in types {
            for node in self.scan_nodes(&node_type)? {
                index
                    .entry(node.node_id)
                    .or_insert_with(|| node_type.clone());
            }
        }
        Ok(index)
    }

    /// Classified neighbors of `node_id` across every propagating edge, in a
    /// deterministic order (outgoing then incoming, each in adjacency-key order).
    fn impact_neighbors(
        &self,
        node_id: &str,
        inference_edges: &HashSet<String>,
    ) -> Result<Vec<Neighbor>, DynoError> {
        let mut out = Vec::new();

        for e in self.outgoing(node_id, None)? {
            if let Some(dir) = direction_for(&e.edge_type, true, inference_edges) {
                out.push(Neighbor {
                    id: e.to_id.clone(),
                    hop: Hop {
                        is_risk: RISK_EDGES.contains(&e.edge_type.as_str()),
                        edge_type: e.edge_type,
                        direction: dir,
                        from_id: e.from_id,
                        to_id: e.to_id,
                    },
                });
            }
        }
        for e in self.incoming(node_id, None)? {
            if let Some(dir) = direction_for(&e.edge_type, false, inference_edges) {
                out.push(Neighbor {
                    id: e.from_id.clone(),
                    hop: Hop {
                        is_risk: RISK_EDGES.contains(&e.edge_type.as_str()),
                        edge_type: e.edge_type,
                        direction: dir,
                        from_id: e.from_id,
                        to_id: e.to_id,
                    },
                });
            }
        }
        Ok(out)
    }

    /// Speculative PROPAGATE: compute the blast radius of changing `seed_ids`,
    /// without recording anything ("what if I change X?").
    pub fn propagate_from(
        &self,
        seed_ids: &[&str],
        opts: PropagateOptions,
    ) -> Result<BlastRadius, DynoError> {
        let index = self.node_type_index()?;
        let inference_edges: HashSet<String> = self
            .schema()
            .inference_edge_types()
            .into_iter()
            .map(str::to_string)
            .collect();

        let seeds: Vec<String> = seed_ids.iter().map(|s| s.to_string()).collect();
        let seed_set: HashSet<&str> = seed_ids.iter().copied().collect();
        let unknown_seeds: Vec<String> = seeds
            .iter()
            .filter(|s| !index.contains_key(*s))
            .cloned()
            .collect();

        // BFS. `visited` maps id -> its best (shortest, first-found) impact.
        let mut visited: HashMap<String, ImpactedNode> = HashMap::new();
        let mut queue: VecDeque<(String, usize, Vec<Hop>, bool)> = VecDeque::new();
        let mut beyond_depth: HashSet<String> = HashSet::new();

        for s in &seeds {
            // Only walk from seeds that exist; unknown seeds are already reported.
            if index.contains_key(s) {
                queue.push_back((s.clone(), 0, Vec::new(), false));
            }
        }

        while let Some((id, depth, via, crosses_risk)) = queue.pop_front() {
            let neighbors = self.impact_neighbors(&id, &inference_edges)?;
            for nb in neighbors {
                // Skip edges to nodes that don't exist, and back to seeds.
                let Some(node_type) = index.get(&nb.id) else {
                    continue;
                };
                if seed_set.contains(nb.id.as_str()) {
                    continue;
                }
                if visited.contains_key(&nb.id) {
                    continue; // BFS: first visit is the shortest path.
                }

                let next_depth = depth + 1;
                let next_crosses = crosses_risk || nb.hop.is_risk;

                if next_depth > opts.max_depth {
                    // Reachable but beyond the bound: count it, don't expand.
                    beyond_depth.insert(nb.id.clone());
                    continue;
                }

                let mut next_via = via.clone();
                let direction = nb.hop.direction;
                next_via.push(nb.hop);

                visited.insert(
                    nb.id.clone(),
                    ImpactedNode {
                        node_id: nb.id.clone(),
                        node_type: node_type.clone(),
                        distance: next_depth,
                        direction,
                        via: next_via.clone(),
                        crosses_risk_edge: next_crosses,
                    },
                );
                queue.push_back((nb.id, next_depth, next_via, next_crosses));
            }
        }

        // A node reached within the bound is not also "beyond" it.
        beyond_depth.retain(|id| !visited.contains_key(id));

        let mut impacted: Vec<ImpactedNode> = visited.into_values().collect();
        // Deterministic ranking: nearest first, risk-crossing paths amplified,
        // then by id for stability.
        impacted.sort_by(|a, b| {
            a.distance
                .cmp(&b.distance)
                .then(b.crosses_risk_edge.cmp(&a.crosses_risk_edge))
                .then(a.node_id.cmp(&b.node_id))
        });

        Ok(BlastRadius {
            seeds,
            unknown_seeds,
            impacted,
            max_depth: opts.max_depth,
            truncated_beyond_depth: beyond_depth.len(),
        })
    }

    /// Reactive PROPAGATE: the blast radius of an already-recorded
    /// [`ChangeEvent`](crate::nodes::node::CHANGE_EVENT). Its `CHANGED` targets
    /// are the seeds. This is the automatic path the vision describes.
    pub fn propagate_change(
        &self,
        change_event_id: &str,
        opts: PropagateOptions,
    ) -> Result<BlastRadius, DynoError> {
        let seeds: Vec<String> = self
            .outgoing(change_event_id, Some(edge::CHANGED))?
            .into_iter()
            .map(|e| e.to_id)
            .collect();
        let seed_refs: Vec<&str> = seeds.iter().map(String::as_str).collect();
        self.propagate_from(&seed_refs, opts)
    }
}

/// Resolve the impact direction for an edge walked forward/backward, treating
/// any inference edge as [`Causal`](ImpactDirection::Causal).
fn direction_for(
    edge_type: &str,
    forward: bool,
    inference_edges: &HashSet<String>,
) -> Option<ImpactDirection> {
    if inference_edges.contains(edge_type) {
        return Some(ImpactDirection::Causal);
    }
    let rule = structural_rule(edge_type)?;
    if forward { rule.forward } else { rule.backward }
}
