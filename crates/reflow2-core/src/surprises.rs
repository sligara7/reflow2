//! Surprising connections — coupling edges that bridge otherwise-distant parts
//! of the design (adapted from graphify's `surprising_connections`; see
//! docs/graph-analysis.md "Concepts to mine from graphify").
//!
//! Leiden groups the design network into communities of tightly-coupled nodes.
//! A traceability edge whose two endpoints land in **different** communities is
//! *surprising*: the communities are otherwise structurally distant, yet this
//! edge ties them together. That's either a **hidden coupling** worth flagging
//! (DETECT) or, read the other way, a **creative-link** the design leans on
//! (chain_reflow) — and it's exactly the kind of thing HEAL's creative-bridge
//! healer would propose.
//!
//! In graphify's spirit, every finding is *explained* (`reasons`) and ranked by
//! a surprise score that amplifies:
//! - **rarity** — the *sole* bridge between two communities is more surprising
//!   than one of many;
//! - **peripheral→hub** — a low-degree node unexpectedly reaching a high-degree
//!   hub (which also ties to the matryoshka missing-intermediate smell).
//!
//! Pure `dynograph-graph` (Leiden + degree) — no embeddings, no LLM.

use std::collections::HashMap;

use dynograph_core::DynoError;

use crate::graph::DesignGraph;

/// Only **lateral coupling** edges can be "surprising" — the *vertical* golden-
/// thread edges (SATISFIES / ALLOCATED_TO / REALIZES / VERIFIES …) are the
/// design's intended cross-layer structure, so a cross-community one of those is
/// expected, not surprising. Communities are still detected over the *full*
/// design network (see [`DesignGraph::design_network`]); only which edges we
/// *flag* is narrowed here.
const LATERAL_COUPLING: &[&str] = &["DEPENDS_ON", "PROVIDES", "CONSUMES", "PART_OF_FLOW"];

/// A coupling edge bridging two Leiden communities of the design network.
#[derive(Debug, Clone)]
pub struct SurprisingConnection {
    /// Source id of the edge (graph orientation).
    pub from_id: String,
    /// Target id of the edge (graph orientation).
    pub to_id: String,
    /// The coupling edge type (e.g. `DEPENDS_ON`).
    pub edge_type: String,
    /// Community of `from_id`.
    pub from_community: usize,
    /// Community of `to_id`.
    pub to_community: usize,
    /// Composite surprise score — higher = more surprising. Ranks the list.
    pub surprise: f64,
    /// Plain-language reasons this edge is surprising (graphify's "explained").
    pub reasons: Vec<&'static str>,
}

/// Leiden resolution for community detection (higher → more, smaller communities).
const RESOLUTION: f64 = 1.0;
/// Degree-asymmetry at/above which an edge counts as peripheral→hub.
const ASYMMETRY_REASON: f64 = 0.4;

impl DesignGraph {
    /// Find coupling edges that bridge two otherwise-distant communities, ranked
    /// most-surprising first. See the module docs.
    pub fn surprising_connections(&self) -> Result<Vec<SurprisingConnection>, DynoError> {
        let net = self.design_network()?;
        let community = net.communities(RESOLUTION)?;

        // Pass 1: collect cross-community coupling edges (each undirected edge once).
        struct Candidate {
            from_id: String,
            to_id: String,
            edge_type: String,
            from_c: usize,
            to_c: usize,
            lo_deg: usize,
            hi_deg: usize,
        }
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut seen: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();
        let mut pair_bridges: HashMap<(usize, usize), usize> = HashMap::new();

        for (id, &c_from) in &community {
            for e in self.outgoing(id, None)? {
                if !LATERAL_COUPLING.contains(&e.edge_type.as_str()) || !net.contains(&e.to_id) {
                    continue;
                }
                let Some(&c_to) = community.get(&e.to_id) else {
                    continue;
                };
                if c_from == c_to {
                    continue; // same community — not surprising
                }
                // Canonical key so an A→B / B→A pair isn't double-counted.
                let (a, b) = if e.from_id <= e.to_id {
                    (e.from_id.clone(), e.to_id.clone())
                } else {
                    (e.to_id.clone(), e.from_id.clone())
                };
                if !seen.insert((a.clone(), b.clone(), e.edge_type.clone())) {
                    continue;
                }
                let (d_from, d_to) = (net.degree_of(&e.from_id), net.degree_of(&e.to_id));
                let pair = if c_from <= c_to {
                    (c_from, c_to)
                } else {
                    (c_to, c_from)
                };
                *pair_bridges.entry(pair).or_default() += 1;
                candidates.push(Candidate {
                    from_id: e.from_id,
                    to_id: e.to_id,
                    edge_type: e.edge_type,
                    from_c: c_from,
                    to_c: c_to,
                    lo_deg: d_from.min(d_to),
                    hi_deg: d_from.max(d_to),
                });
            }
        }

        // Pass 2: score + explain.
        let mut out: Vec<SurprisingConnection> = candidates
            .into_iter()
            .map(|c| {
                let mut reasons = vec!["bridges separate communities"];
                let mut surprise = 1.0;

                // Peripheral→hub: low-degree endpoint reaching a high-degree one.
                let total = (c.lo_deg + c.hi_deg) as f64;
                let asymmetry = if total > 0.0 {
                    (c.hi_deg - c.lo_deg) as f64 / total
                } else {
                    0.0
                };
                surprise += asymmetry;
                if c.lo_deg <= 2 && asymmetry >= ASYMMETRY_REASON {
                    reasons.push("peripheral node reaches a hub");
                }

                // Rarity: the sole bridge between these communities.
                let pair = if c.from_c <= c.to_c {
                    (c.from_c, c.to_c)
                } else {
                    (c.to_c, c.from_c)
                };
                let bridges = pair_bridges.get(&pair).copied().unwrap_or(1);
                surprise += 1.0 / bridges as f64;
                if bridges == 1 {
                    reasons.push("sole bridge between these communities");
                }

                SurprisingConnection {
                    from_id: c.from_id,
                    to_id: c.to_id,
                    edge_type: c.edge_type,
                    from_community: c.from_c,
                    to_community: c.to_c,
                    surprise,
                    reasons,
                }
            })
            .collect();

        // Rank most-surprising first; deterministic tie-break by endpoints.
        out.sort_by(|a, b| {
            b.surprise
                .partial_cmp(&a.surprise)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.from_id.cmp(&b.from_id))
                .then(a.to_id.cmp(&b.to_id))
        });
        Ok(out)
    }
}
