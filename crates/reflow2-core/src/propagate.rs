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

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use dynograph_core::DynoError;

use crate::graph::DesignGraph;
use crate::nodes::structural_rule;
use crate::nodes::{edge, node};
// Re-exported from the shared vocabulary base (see the note on the enum there):
// `reflow2_core::propagate::ImpactDirection` stays a valid public path.
pub use crate::nodes::ImpactDirection;

/// Risk edges whose crossing amplifies severity — kept verbatim from Reflow's
/// `risk_rel_types` (docs/impact-propagation.md, "Ranking the blast radius").
const RISK_EDGES: &[&str] = &["RISKS", "BLOCKS", "CONTRADICTS", "VIOLATES", "MASKS"];

/// One hop in an impact chain — the edge that carried impact to a node, and how.
#[derive(Debug, Clone, serde::Serialize)]
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
#[derive(Debug, Clone, serde::Serialize)]
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
    /// The node's betweenness centrality in the design network (normalized
    /// 0..1) — how much of the golden thread routes through it. A change landing
    /// on a high-centrality node has a wider secondary blast radius, so it ranks
    /// higher among equals (IP-9).
    pub centrality: f64,
}

/// The computed blast radius of a change.
#[derive(Debug, Clone, serde::Serialize)]
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
    /// How many distinct nodes sit on the frontier **one hop past**
    /// `max_depth` — the ring the walk stopped at, not the full remainder of
    /// the graph beyond it (BL-58: this is a lower bound, an honest "there is
    /// more out here" signal, not a total). Non-zero ⇒ raise `max_depth` to
    /// see further. Reported, not hidden (discipline 1: never silently truncate).
    pub truncated_beyond_depth: usize,
}

impl BlastRadius {
    /// Whether the traversal stopped short of the full radius at the depth bound.
    pub fn was_truncated(&self) -> bool {
        self.truncated_beyond_depth > 0
    }

    /// Compress the radius to what a session reads first (BL-49): counts per
    /// distance band, the distance-1 ring, and every risk crossing. Nothing is
    /// hidden — every impacted node is counted in a band and
    /// `truncated_beyond_depth` is carried through — but the per-node `via`
    /// chains, which dominate the payload on a large design, are only in the
    /// full [`BlastRadius`].
    pub fn summarize(&self) -> BlastRadiusSummary {
        let mut bands: BTreeMap<usize, usize> = BTreeMap::new();
        for n in &self.impacted {
            *bands.entry(n.distance).or_insert(0) += 1;
        }
        let direct_ring = self
            .impacted
            .iter()
            .filter(|n| n.distance == 1)
            .map(|n| {
                let hop = n
                    .via
                    .last()
                    .expect("a distance-1 node is reached by exactly one hop");
                RingNode {
                    node_id: n.node_id.clone(),
                    node_type: n.node_type.clone(),
                    direction: n.direction,
                    edge_type: hop.edge_type.clone(),
                    is_risk: hop.is_risk,
                }
            })
            .collect();
        let risk_crossings = self
            .impacted
            .iter()
            .filter(|n| n.crosses_risk_edge)
            .map(|n| RiskCrossing {
                node_id: n.node_id.clone(),
                node_type: n.node_type.clone(),
                distance: n.distance,
            })
            .collect();
        BlastRadiusSummary {
            seeds: self.seeds.clone(),
            unknown_seeds: self.unknown_seeds.clone(),
            total_impacted: self.impacted.len(),
            counts_by_distance: bands
                .into_iter()
                .map(|(distance, count)| DistanceBand { distance, count })
                .collect(),
            direct_ring,
            risk_crossings,
            max_depth: self.max_depth,
            truncated_beyond_depth: self.truncated_beyond_depth,
        }
    }
}

/// How many impacted nodes sit `distance` hops out.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DistanceBand {
    pub distance: usize,
    pub count: usize,
}

/// A node in the distance-1 ring, with the single hop that reached it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RingNode {
    pub node_id: String,
    pub node_type: String,
    pub direction: ImpactDirection,
    pub edge_type: String,
    pub is_risk: bool,
}

/// A node whose impact chain crossed a risk edge, wherever it sits.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RiskCrossing {
    pub node_id: String,
    pub node_type: String,
    pub distance: usize,
}

/// [`BlastRadius`] compressed for reading inside a session; see
/// [`BlastRadius::summarize`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlastRadiusSummary {
    /// Seed node ids the propagation started from.
    pub seeds: Vec<String>,
    /// Seeds that were not found as real nodes (surfaced, never silently
    /// dropped).
    pub unknown_seeds: Vec<String>,
    /// Every impacted node is counted here, whatever its distance.
    pub total_impacted: usize,
    /// Node counts per hop distance, nearest first.
    pub counts_by_distance: Vec<DistanceBand>,
    /// The nodes one hop from a seed — the ring a session acts on.
    pub direct_ring: Vec<RingNode>,
    /// Every node reached across a risk edge, at any distance.
    pub risk_crossings: Vec<RiskCrossing>,
    /// The depth bound the traversal used.
    pub max_depth: usize,
    /// Nodes on the frontier one hop past `max_depth` (a lower bound, not the
    /// full remainder — see [`BlastRadius::truncated_beyond_depth`]).
    pub truncated_beyond_depth: usize,
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
                        centrality: 0.0, // filled in below
                    },
                );
                queue.push_back((nb.id, next_depth, next_via, next_crosses));
            }
        }

        // A node reached within the bound is not also "beyond" it.
        beyond_depth.retain(|id| !visited.contains_key(id));

        // Centrality-weighted ranking (IP-9): attach each impacted node's
        // betweenness in the design network — a change landing on a routing hub
        // has a wider secondary blast radius.
        let centrality = self.design_network()?.betweenness()?;
        for node in visited.values_mut() {
            node.centrality = centrality.get(&node.node_id).copied().unwrap_or(0.0);
        }

        let mut impacted: Vec<ImpactedNode> = visited.into_values().collect();
        // Deterministic ranking: nearest first, then risk-crossing paths, then
        // higher centrality, then by id for stability.
        impacted.sort_by(|a, b| {
            a.distance
                .cmp(&b.distance)
                .then(b.crosses_risk_edge.cmp(&a.crosses_risk_edge))
                .then(
                    b.centrality
                        .partial_cmp(&a.centrality)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
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
        // A nonexistent (or typo'd) ChangeEvent has no outgoing CHANGED edges,
        // so it used to yield an empty blast radius — indistinguishable from "a
        // real event that impacts nothing." That is a silent drop (BL-58): fail
        // loud so the caller knows the id was wrong, not that the change was
        // harmless.
        if self
            .get_node(node::CHANGE_EVENT, change_event_id)?
            .is_none()
        {
            return Err(DynoError::NodeNotFound {
                node_type: node::CHANGE_EVENT.to_string(),
                node_id: change_event_id.to_string(),
            });
        }
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
