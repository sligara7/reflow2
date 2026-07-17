//! Allocation evaluator — score the *current* function→service allocation
//! (docs/graph-analysis.md, build-order step 2, the **Evaluate** mode).
//!
//! Given the capabilities allocated to components (`ALLOCATED_TO`) and the
//! weighted functional coupling between capabilities (`DEPENDS_ON` with the
//! `weight` facet), this computes how good the allocation is — **cohesion vs.
//! coupling**, which capabilities are **misplaced** (coupled more tightly across
//! a boundary than within their own component), and which components are
//! **god-components** (routing hubs the architecture can't lose). It only
//! *evaluates* — proposing a better allocation is a later step; this is DETECT
//! applied to architecture quality.
//!
//! Disciplines (graph-analysis.md):
//! - **Weights fail loud, not silently default.** An unweighted `DEPENDS_ON`
//!   counts as `1.0` *and* is tallied in [`AllocationReport::unweighted_dependencies`]
//!   so a score over mostly-`estimated`/unweighted edges is a weaker claim the
//!   caller can see.
//! - **God-components are selective** (same rule as HEAL's SPOF): a component is
//!   only flagged if removing it splits the component-coupling graph into ≥2
//!   non-trivial pieces — not merely because it's an articulation point of a
//!   tree-shaped component graph.
//! - **Candidates, not answers.** Misplacement is a *suggestion* to weigh, not a
//!   command; allocation is multi-objective and the human/SME decides.

use std::collections::{BTreeMap, HashMap, HashSet};

use dynograph_core::{DynoError, Value};
use dynograph_graph::{GraphBuilder, connected_components, cut_structure, leiden};

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// Per-component cohesion/coupling scores.
#[derive(Debug, Clone)]
pub struct ComponentScore {
    /// The component id.
    pub component_id: String,
    /// How many capabilities are allocated to it.
    pub capability_count: usize,
    /// Total weight of `DEPENDS_ON` edges *within* the component (cohesion).
    pub internal_weight: f64,
    /// Total weight of `DEPENDS_ON` edges crossing its boundary (coupling).
    pub external_weight: f64,
}

/// A capability coupled more tightly to another component than to its own — a
/// candidate to move (a *suggestion*, per the candidates-not-answers rule).
#[derive(Debug, Clone)]
pub struct MisplacedCapability {
    /// The capability id.
    pub capability_id: String,
    /// The component it is currently allocated to.
    pub current_component: String,
    /// The component it couples to most strongly instead.
    pub suggested_component: String,
    /// Its coupling weight to its current component.
    pub current_pull: f64,
    /// Its (stronger) coupling weight to the suggested component.
    pub suggested_pull: f64,
}

/// The evaluation of the current allocation.
#[derive(Debug, Clone)]
pub struct AllocationReport {
    /// Per-component scores.
    pub components: Vec<ComponentScore>,
    /// Total intra-component coupling weight (cohesion).
    pub total_internal: f64,
    /// Total inter-component coupling weight.
    pub total_external: f64,
    /// `internal / (internal + external)` — 1.0 = perfectly cohesive (no
    /// coupling crosses a boundary), 0.0 = all coupling crosses boundaries.
    /// 1.0 when there is no coupling to evaluate.
    pub modularity: f64,
    /// Capabilities coupled more strongly across a boundary than within.
    pub misplaced: Vec<MisplacedCapability>,
    /// Components whose removal would split the architecture into ≥2 non-trivial
    /// pieces — routing hubs / single points of failure.
    pub god_components: Vec<String>,
    /// `DEPENDS_ON` edges that carried no `weight` (counted as 1.0). Surfaced so
    /// a score over unweighted edges isn't mistaken for one over real weights.
    pub unweighted_dependencies: usize,
    /// Capabilities allocated to more than one component (ambiguous — the first
    /// by id is used for scoring). Surfaced, not silently collapsed.
    pub multi_allocated: Vec<String>,
}

/// One weighted functional-coupling edge between two allocated capabilities.
struct Dep {
    a: String,
    b: String,
    weight: f64,
}

/// A proposed component (a Leiden community of tightly-coupled capabilities).
#[derive(Debug, Clone)]
pub struct ProposedComponent {
    /// A synthetic id for the cluster (`cluster:N`) — the LLM names it later.
    pub proposed_id: String,
    /// The capabilities Leiden grouped together.
    pub capability_ids: Vec<String>,
}

/// A proposed allocation — capabilities partitioned by the coupling graph
/// (Leiden community detection), with how it compares to the current allocation.
/// A **candidate**, not a command: `requires_human_review` is always true and
/// the LLM must name the clusters before it becomes real (candidates-not-answers).
#[derive(Debug, Clone)]
pub struct ProposedAllocation {
    /// The proposed components (Leiden communities).
    pub clusters: Vec<ProposedComponent>,
    /// Leiden's own modularity for the partition (its objective).
    pub leiden_modularity: f64,
    /// This crate's cohesion/coupling modularity of the *proposed* partition,
    /// over the same dependency set as `current_modularity` (comparable).
    pub proposed_modularity: f64,
    /// The same score for the *current* `ALLOCATED_TO` allocation.
    pub current_modularity: f64,
    /// The Leiden resolution used (higher → more, smaller clusters).
    pub resolution: f64,
    /// `DEPENDS_ON` edges with no `weight` (counted as 1.0) — coverage surfaced.
    pub unweighted_dependencies: usize,
    /// Always true — a proposal to review, not an applied allocation.
    pub requires_human_review: bool,
}

impl DesignGraph {
    /// Current capability → component map from `ALLOCATED_TO` (smallest
    /// component id wins on multi-allocation), plus the multi-allocated ids.
    fn current_allocation_map(&self) -> Result<(HashMap<String, String>, Vec<String>), DynoError> {
        let mut cap_to_comp: HashMap<String, String> = HashMap::new();
        let mut multi_allocated = Vec::new();
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let allocs = self.outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))?;
            if allocs.len() > 1 {
                multi_allocated.push(cap.node_id.clone());
            }
            if let Some(comp) = allocs.into_iter().map(|e| e.to_id).min() {
                cap_to_comp.insert(cap.node_id, comp);
            }
        }
        multi_allocated.sort();
        Ok((cap_to_comp, multi_allocated))
    }

    /// Weighted `DEPENDS_ON` edges (each once) among the given capability set.
    /// Unweighted edges count as 1.0 and are tallied (returned) — never a silent
    /// default. Shared by the evaluator and the proposer.
    fn capability_dependencies(
        &self,
        caps: &HashSet<&str>,
    ) -> Result<(Vec<Dep>, usize), DynoError> {
        let mut deps = Vec::new();
        let mut unweighted = 0usize;
        for cap in caps {
            for e in self.outgoing(cap, Some(edge::DEPENDS_ON))? {
                if !caps.contains(e.to_id.as_str()) {
                    continue; // both endpoints must be in the capability set
                }
                let weight = match e.properties.get("weight").and_then(Value::as_f64) {
                    Some(w) => w,
                    None => {
                        unweighted += 1;
                        1.0
                    }
                };
                deps.push(Dep {
                    a: e.from_id,
                    b: e.to_id,
                    weight,
                });
            }
        }
        Ok((deps, unweighted))
    }

    /// Evaluate the current `ALLOCATED_TO` allocation. See the module docs.
    pub fn evaluate_allocation(&self) -> Result<AllocationReport, DynoError> {
        // 1. capability → component; 2. weighted DEPENDS_ON among allocated caps.
        let (cap_to_comp, multi_allocated) = self.current_allocation_map()?;
        let cap_set: HashSet<&str> = cap_to_comp.keys().map(String::as_str).collect();
        let (deps, unweighted_dependencies) = self.capability_dependencies(&cap_set)?;

        // 3. per-component cohesion/coupling + aggregated cross-component coupling.
        let mut internal: HashMap<String, f64> = HashMap::new();
        let mut external: HashMap<String, f64> = HashMap::new();
        let mut cross: HashMap<(String, String), f64> = HashMap::new();
        let mut total_internal = 0.0;
        let mut total_external = 0.0;
        for d in &deps {
            let (ca, cb) = (&cap_to_comp[&d.a], &cap_to_comp[&d.b]);
            if ca == cb {
                *internal.entry(ca.clone()).or_default() += d.weight;
                total_internal += d.weight;
            } else {
                *external.entry(ca.clone()).or_default() += d.weight;
                *external.entry(cb.clone()).or_default() += d.weight;
                total_external += d.weight;
                let key = if ca <= cb {
                    (ca.clone(), cb.clone())
                } else {
                    (cb.clone(), ca.clone())
                };
                *cross.entry(key).or_default() += d.weight;
            }
        }

        // 4. per-component scores.
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for comp in cap_to_comp.values() {
            *counts.entry(comp.as_str()).or_default() += 1;
        }
        let mut components: Vec<ComponentScore> = counts
            .iter()
            .map(|(&comp, &n)| ComponentScore {
                component_id: comp.to_string(),
                capability_count: n,
                internal_weight: internal.get(comp).copied().unwrap_or(0.0),
                external_weight: external.get(comp).copied().unwrap_or(0.0),
            })
            .collect();
        components.sort_by(|a, b| a.component_id.cmp(&b.component_id));

        let total = total_internal + total_external;
        let modularity = if total == 0.0 {
            1.0
        } else {
            total_internal / total
        };

        // 5. misplaced capabilities — pull to each component from a cap's edges.
        let mut misplaced = self.misplaced(&cap_to_comp, &deps);
        misplaced.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));

        // 6. god-components — selective articulation points of the component graph.
        let god_components = god_components(&cross, &counts);

        Ok(AllocationReport {
            components,
            total_internal,
            total_external,
            modularity,
            misplaced,
            god_components,
            unweighted_dependencies,
            multi_allocated,
        })
    }

    /// **Propose** an allocation by clustering the weighted capability-coupling
    /// graph with **Leiden** (`resolution` — higher = more, smaller clusters).
    /// Each community becomes a candidate component. Leiden guarantees connected
    /// communities, so no post-hoc split guard is needed (unlike Louvain).
    ///
    /// This only *proposes*: `requires_human_review` is always true, the clusters
    /// are unnamed (the LLM names them), and it reports how the proposal's
    /// modularity compares to the current allocation's — never auto-applies.
    pub fn propose_allocation(&self, resolution: f64) -> Result<ProposedAllocation, DynoError> {
        let caps: Vec<String> = self
            .scan_nodes(node::CAPABILITY)?
            .into_iter()
            .map(|n| n.node_id)
            .collect();
        let cap_set: HashSet<&str> = caps.iter().map(String::as_str).collect();
        let (deps, unweighted_dependencies) = self.capability_dependencies(&cap_set)?;

        // Weighted coupling graph over all capabilities.
        let mut builder = GraphBuilder::new();
        for c in &caps {
            builder.add_node(c);
        }
        for d in &deps {
            let _ = builder.add_edge(&d.a, &d.b, d.weight);
        }
        let graph = builder.build(false);

        let communities =
            leiden(&graph, resolution).map_err(|e| DynoError::Query(format!("leiden: {e}")))?;

        // capability → cluster id, and grouped clusters (sorted, deterministic).
        let mut proposed: HashMap<String, String> = HashMap::new();
        let mut grouped: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        for c in &caps {
            if let Some(idx) = graph.idx_of(c) {
                let label = communities.labels[idx];
                proposed.insert(c.clone(), format!("cluster:{label}"));
                grouped.entry(label).or_default().push(c.clone());
            }
        }
        let clusters = grouped
            .into_iter()
            .map(|(label, mut ids)| {
                ids.sort();
                ProposedComponent {
                    proposed_id: format!("cluster:{label}"),
                    capability_ids: ids,
                }
            })
            .collect();

        // Score proposed vs current over the *same* dependency set.
        let (current_map, _) = self.current_allocation_map()?;
        let (_, _, proposed_modularity) = score_modularity(&proposed, &deps);
        let (_, _, current_modularity) = score_modularity(&current_map, &deps);

        Ok(ProposedAllocation {
            clusters,
            leiden_modularity: communities.modularity,
            proposed_modularity,
            current_modularity,
            resolution,
            unweighted_dependencies,
            requires_human_review: true,
        })
    }

    /// Capabilities whose strongest coupling is to a component other than their own.
    fn misplaced(
        &self,
        cap_to_comp: &HashMap<String, String>,
        deps: &[Dep],
    ) -> Vec<MisplacedCapability> {
        // pull[cap][component] = total coupling weight from cap to that component.
        let mut pull: HashMap<&str, HashMap<&str, f64>> = HashMap::new();
        for d in deps {
            let (ca, cb) = (cap_to_comp[&d.a].as_str(), cap_to_comp[&d.b].as_str());
            *pull.entry(&d.a).or_default().entry(cb).or_default() += d.weight;
            *pull.entry(&d.b).or_default().entry(ca).or_default() += d.weight;
        }

        let mut out = Vec::new();
        for (cap, by_comp) in &pull {
            let own = cap_to_comp[*cap].as_str();
            let own_pull = by_comp.get(own).copied().unwrap_or(0.0);
            // Strongest pull to a *different* component; tie-break by smaller id
            // for determinism.
            let mut best: Option<(&str, f64)> = None;
            for (comp, w) in by_comp {
                if *comp == own {
                    continue;
                }
                let stronger = match best {
                    None => true,
                    Some((bc, bw)) => *w > bw || (*w == bw && *comp < bc),
                };
                if stronger {
                    best = Some((comp, *w));
                }
            }
            if let Some((other, other_pull)) = best
                && other_pull > own_pull
            {
                out.push(MisplacedCapability {
                    capability_id: (*cap).to_string(),
                    current_component: own.to_string(),
                    suggested_component: other.to_string(),
                    current_pull: own_pull,
                    suggested_pull: other_pull,
                });
            }
        }
        out
    }
}

/// Cohesion/coupling modularity of an allocation over a dependency set:
/// `internal / (internal + external)`, where a dependency is *internal* only
/// when both endpoints map to the same component (a cap with no component
/// therefore never counts as internal). Returns `(internal, external,
/// modularity)`; modularity is 1.0 when there is no coupling. Shared by the
/// evaluator and the proposer so both are scored the same way.
fn score_modularity(cap_to_comp: &HashMap<String, String>, deps: &[Dep]) -> (f64, f64, f64) {
    let mut internal = 0.0;
    let mut external = 0.0;
    for d in deps {
        match (cap_to_comp.get(&d.a), cap_to_comp.get(&d.b)) {
            (Some(x), Some(y)) if x == y => internal += d.weight,
            _ => external += d.weight,
        }
    }
    let total = internal + external;
    let modularity = if total == 0.0 { 1.0 } else { internal / total };
    (internal, external, modularity)
}

/// Build the undirected component-coupling graph from aggregated cross-component
/// weights, optionally excluding one component, and return its component groups.
fn component_group_count(
    cross: &HashMap<(String, String), f64>,
    all_components: &HashSet<&str>,
    exclude: Option<&str>,
) -> usize {
    let mut builder = GraphBuilder::new();
    for c in all_components {
        if exclude != Some(*c) {
            builder.add_node(c);
        }
    }
    for ((a, b), w) in cross {
        if exclude == Some(a.as_str()) || exclude == Some(b.as_str()) {
            continue;
        }
        // add_edge only fails on a non-finite weight; our weights are finite.
        let _ = builder.add_edge(a, b, *w);
    }
    let graph = builder.build(false);
    connected_components(&graph)
        .groups()
        .iter()
        .filter(|g| g.len() >= 2)
        .count()
}

/// Components whose removal splits the component-coupling graph into ≥2
/// non-trivial pieces (selective SPOF, matching HEAL's rule).
fn god_components(
    cross: &HashMap<(String, String), f64>,
    counts: &HashMap<&str, usize>,
) -> Vec<String> {
    let all: HashSet<&str> = counts.keys().copied().collect();

    // Candidate hubs: articulation points of the component graph.
    let mut builder = GraphBuilder::new();
    for c in &all {
        builder.add_node(c);
    }
    for ((a, b), w) in cross {
        let _ = builder.add_edge(a, b, *w);
    }
    let graph = builder.build(false);
    let cuts = cut_structure(&graph);

    let mut god = Vec::new();
    for idx in cuts.articulation_points {
        let id = graph.id_of(idx).to_string();
        if component_group_count(cross, &all, Some(&id)) >= 2 {
            god.push(id);
        }
    }
    god.sort();
    god
}
