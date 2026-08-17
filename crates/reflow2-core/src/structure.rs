//! Structural topology analysis — a `dynograph-graph` view of the design
//! network, powering HEAL's graph-algorithm defect detectors.
//!
//! The **design network** is the undirected graph of design nodes connected by
//! *traceability* edges (the same coupling set PROPAGATE walks — see
//! [`crate::nodes::is_traceability_edge`] — which excludes CONTAINS so the
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

use std::collections::{BTreeSet, HashMap, HashSet};

use dynograph_core::DynoError;
use dynograph_graph::{
    Graph, GraphBuilder, betweenness_centrality, connected_components, cut_structure, find_cycle,
    leiden, strongly_connected_components,
};

use crate::graph::DesignGraph;
use crate::nodes::is_traceability_edge;
use crate::nodes::{edge, node};

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
    // Provenance and observation records: they describe *how the graph came to
    // be* or *what reality looks like*, not how the design is structured.
    // Leaving them in distorts the topology they were never part of — a
    // Fragment lands as an isolated node inflating the community count, and a
    // DriftEvent's DEPENDS_ON to its Artifact is a traceability edge, so it
    // both shifts communities and can be reported as a coupling in its own
    // right.
    "Fragment",
    "DriftEvent",
];

/// Node types that operate — things whose failure at run time is a coherent
/// idea, and so the only sanctioned `single_point_of_failure` candidates
/// (`dec:operational-spof`, BL-5 second pass).
pub(crate) const OPERATIONAL_TYPES: &[&str] = &[
    node::COMPONENT,
    node::INTERFACE,
    node::RESOURCE,
    node::ENVIRONMENT,
];

/// Whether a node type belongs to the **as-built operational network** — the
/// graph `single_point_of_failure` measures connectivity on: the operational
/// types plus the Artifacts that realize them (a stranded part together with
/// its file is a real severed subsystem; a stranded sentence is not).
fn is_operational_member(node_type: &str) -> bool {
    OPERATIONAL_TYPES.contains(&node_type) || node_type == node::ARTIFACT
}

/// A contract medium a consumer *links against* rather than *calls across* at
/// run time — a shared `library`, or a `data` store everything reads. Such a
/// foundation is imported/read by everything, which makes it a perfect
/// articulation point, and you cannot run a second copy of it to survive its
/// failure: `add_redundancy` is incoherent, so it is never a runtime single
/// point of failure. `library` is F6 (the storyflow trial); `data` is the twin
/// folded in by BL-84 (the BL-83b adopt dogfood, where `data`-medium hubs still
/// fired). All other media (REST and friends) are carried at run time.
pub(crate) fn is_foundation_medium(medium: &str) -> bool {
    medium == "library" || medium == "data"
}

/// Dependency pairs plus **how each one was derived** — a direct `DEPENDS_ON`
/// edge, or collapsed out of a shared contract, or both.
struct DependencyEdges {
    /// Every `(u, v)` meaning "u depends on v", deduplicated and sorted.
    pairs: Vec<(String, String)>,
    /// Pairs backed by a real `DEPENDS_ON` edge.
    direct: HashSet<(String, String)>,
    /// Pairs produced by a shared Interface, and which Interface(s) produced
    /// them. A pair can appear here *and* in `direct`.
    contract: HashMap<(String, String), BTreeSet<String>>,
}

/// One circular dependency, with the evidence for how it was formed (BL-141).
///
/// The path alone cannot tell a tangled call graph from a coarse interface
/// model, and both were reported `critical` in identical words until this
/// carried the attribution.
#[derive(Debug, Clone)]
pub(crate) struct DependencyCycle {
    /// The loop, rotated to start at its smallest id and closed by `last → first`.
    pub(crate) path: Vec<String>,
    /// Interfaces any hop of this loop was collapsed out of, sorted. Empty when
    /// every hop is a direct `DEPENDS_ON`.
    pub(crate) via_interfaces: Vec<String>,
    /// **No hop of this loop is backed by a `DEPENDS_ON` edge** — every one was
    /// derived from a shared contract. Reported, never judged
    /// (`dec:report-dont-judge`): it is a fact about what the detector walked,
    /// and it is the tell that separates the four phantom cycles BL-141 found
    /// from a real tangle. A reader who sees it knows to check whether one
    /// Interface node is standing for two contracts before touching any code.
    pub(crate) contracts_only: bool,
    /// Every Interface this loop runs through is a **foundation medium** —
    /// `library` or `data`, something linked against or read rather than called
    /// across at run time ([`is_foundation_medium`]). Non-empty and true is the
    /// fact that separates BL-141's real case from a service cycle.
    ///
    /// **Measured on the reporting project's own design, not assumed:** their
    /// `cmp:fundamental-detection ⇄ cmp:midi-renderer` loop runs through
    /// `ifc:midi-file` and `ifc:wav-audio`, both `medium: data` — a renderer
    /// that reads MIDI and writes WAV against a transcriber that reads WAV and
    /// writes MIDI. Two programs sharing two file formats, with no runtime
    /// dependency in either direction. Structurally that is IDENTICAL to a
    /// genuine two-contract service cycle; only the medium tells them apart.
    ///
    /// Reported, never acted on: whether such a loop should still count as a
    /// dependency cycle at all is an open design question (BL-141(b)), and
    /// `is_foundation_medium`'s existing exemption is argued for
    /// `single_point_of_failure` on grounds that do not transfer unexamined.
    pub(crate) foundation_media_only: bool,
}

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

    /// Leiden community label per node id (connected communities). Used to spot
    /// coupling edges that bridge otherwise-distant communities.
    pub(crate) fn communities(&self, resolution: f64) -> Result<HashMap<String, usize>, DynoError> {
        let c = leiden(&self.graph, resolution)
            .map_err(|e| DynoError::Query(format!("leiden: {e}")))?;
        Ok((0..self.node_count())
            .map(|i| (self.id_of(i).to_string(), c.labels[i]))
            .collect())
    }

    /// Undirected degree of a node by id (0 if it isn't in the design network).
    pub(crate) fn degree_of(&self, id: &str) -> usize {
        self.graph.idx_of(id).map(|i| self.degree(i)).unwrap_or(0)
    }

    /// Whether a node participates in the design network.
    pub(crate) fn contains(&self, id: &str) -> bool {
        self.graph.idx_of(id).is_some()
    }

    /// Count of components with ≥2 nodes — the "non-trivial subsystems".
    fn nontrivial_component_count(&self) -> usize {
        self.component_groups()
            .iter()
            .filter(|g| g.len() >= 2)
            .count()
    }
}

/// A review record: the Decision that `acknowledge_gap` / `acknowledge_defect`
/// writes when someone accepts a finding.
///
/// Excluded from every structural network for the same reason as
/// `NON_DESIGN_TYPES` above — it describes a **judgement about** the design
/// rather than how the design is structured — but it needs an id rule rather
/// than a type rule, because `Decision` is otherwise a first-class design node
/// and a real decision genuinely does couple what it governs.
///
/// Both acknowledgement paths mint this prefix (`heal.rs`, `detect.rs`), and
/// both already strip it to recognise their own records, so this is the same
/// marker the codebase uses elsewhere rather than a new convention.
///
/// **Why it matters that these were counted (BL-124).** The acknowledgement is
/// wired `GOVERNED_BY` from every node it acknowledges, deliberately, so the
/// review stays reachable from the design. `unthreaded_cluster` hashes its
/// id from the affected set — so the review joined the island it acknowledged,
/// enlarged it by one, minted an id nobody had accepted, and the defect
/// returned one node larger every time. An entire category was unclosable,
/// which is exactly the *"a list that can never reach zero gets skimmed"*
/// failure the acknowledge tools exist to prevent. Reproduced in the field
/// across four sessions of a real project, growing 8 → 9 → 10 nodes.
///
/// The fix belongs here rather than in the defect-id hash because
/// `design_network()` has **three** consumers — `unthreaded_cluster`,
/// betweenness centrality (`propagate`) and `surprising_connections` — and the
/// other two were distorted silently. Measured on reflow2's own graph before
/// this change: 125 review records carrying 610 edges, and **four of the eight
/// most central nodes were acknowledgements**, outranking every Component and
/// Capability. Keying the id differently would have fixed the visible symptom
/// and left that untouched.
fn is_review_record(id: &str) -> bool {
    id.starts_with("decision:ack:")
}

impl DesignGraph {
    /// Build the design network, optionally excluding one node id (used to test
    /// what a node's removal would disconnect).
    fn build_network(&self, exclude: Option<&str>) -> Result<DesignNetwork, DynoError> {
        self.build_network_of(exclude, |ty| !NON_DESIGN_TYPES.contains(&ty))
    }

    /// Build a network over the node types `member` admits, optionally
    /// excluding one node id. Edges are the traceability set, restricted to
    /// pairs whose endpoints are both members.
    fn build_network_of(
        &self,
        exclude: Option<&str>,
        member: impl Fn(&str) -> bool,
    ) -> Result<DesignNetwork, DynoError> {
        let index = self.node_type_index()?;
        let included: HashSet<&str> = index
            .iter()
            .filter(|(id, ty)| {
                member(ty.as_str())
                    && !is_review_record(id.as_str())
                    && exclude != Some(id.as_str())
            })
            .map(|(id, _)| id.as_str())
            .collect();

        // Insertion order decides the dense node indices, and those decide how
        // Leiden breaks ties between equal-modularity moves. `included` is a
        // HashSet, and Rust seeds its hasher per *process* — so iterating it
        // directly makes the community assignment (and therefore every gap
        // derived from it) differ between runs on an unchanged graph. Sort.
        let mut ordered: Vec<&str> = included.iter().copied().collect();
        ordered.sort_unstable();

        let mut builder = GraphBuilder::new();
        // Add every included node so isolated ones still appear as components.
        for id in &ordered {
            builder.add_node(id);
        }
        for id in &ordered {
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
        for id in &ordered {
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

    /// The **as-built operational network**: operational nodes (Components,
    /// Interfaces, Resources, Environments) plus the Artifacts realizing them,
    /// joined by the traceability edges that hold between such nodes
    /// (`DEPENDS_ON`, `PROVIDES`/`CONSUMES`, `REALIZES`, `DEPLOYED_TO`,
    /// `REQUIRES_RESOURCE`). This is the graph on which "what does this
    /// node's failure sever?" is a coherent question — intent nodes are out,
    /// because a Requirement provides no connectivity at run time.
    pub(crate) fn operational_network(
        &self,
        exclude: Option<&str>,
    ) -> Result<DesignNetwork, DynoError> {
        self.build_network_of(exclude, is_operational_member)
    }

    /// The **dependency pairs** `(u, v)` meaning "u depends on v", deduplicated
    /// and deterministically ordered.
    ///
    /// Only *homogeneous dependency* relations count. Mixing the golden thread's
    /// other traceability edges (SATISFIES, ALLOCATED_TO, REALIZES, VERIFIES)
    /// into one directed graph would manufacture "cycles" that are just the
    /// thread closing on itself — Requirement ← Capability → Component is not a
    /// circular dependency. This is the same selectivity lesson as
    /// [`Self::is_single_point_of_failure`]: an unselective topology detector
    /// fires everywhere and means nothing.
    ///
    /// Two relations qualify:
    /// - a direct `DEPENDS_ON` edge, and
    /// - a contract: if `c CONSUMES i` and `p PROVIDES i` then `c` depends on
    ///   `p`. The `Interface` is the medium, so it collapses into a direct
    ///   dependency between the two parts rather than appearing as a hop.
    ///
    /// **The collapse is why a cycle needs attribution** (BL-141). Discarding
    /// the Interface here is what made every contract-derived cycle read
    /// identically to a real one: an adopt pass over a research repo produced
    /// four `critical` cycles and *none* of them were real — each was one
    /// Interface node standing for two contracts (`ifc:midi-file` meaning both
    /// "MIDI we read" and "MIDI we emit"), so a reader and a writer of the same
    /// file format looked like mutual dependency. So the medium is kept beside
    /// the pair rather than thrown away, and the finding can say which edge
    /// kinds it actually walked.
    fn dependency_pairs(&self) -> Result<DependencyEdges, DynoError> {
        let index = self.node_type_index()?;
        let mut ids: Vec<&str> = index.keys().map(String::as_str).collect();
        ids.sort_unstable();

        let mut pairs: HashSet<(String, String)> = HashSet::new();
        let mut direct: HashSet<(String, String)> = HashSet::new();
        let mut contract: HashMap<(String, String), BTreeSet<String>> = HashMap::new();

        for id in &ids {
            for e in self.outgoing(id, Some(edge::DEPENDS_ON))? {
                pairs.insert((e.from_id.clone(), e.to_id.clone()));
                direct.insert((e.from_id.clone(), e.to_id.clone()));
            }
        }
        // Contracts: every consumer of an interface depends on its provider(s).
        for id in &ids {
            if index[*id] != node::INTERFACE {
                continue;
            }
            let providers = self.incoming(id, Some(edge::PROVIDES))?;
            let consumers = self.incoming(id, Some(edge::CONSUMES))?;
            for c in &consumers {
                for p in &providers {
                    if c.from_id != p.from_id {
                        let pair = (c.from_id.clone(), p.from_id.clone());
                        pairs.insert(pair.clone());
                        contract.entry(pair).or_default().insert((*id).to_string());
                    }
                }
            }
        }
        let mut pairs: Vec<(String, String)> = pairs.into_iter().collect();
        pairs.sort();
        Ok(DependencyEdges {
            pairs,
            direct,
            contract,
        })
    }

    /// Circular dependencies: one representative cycle per independent cluster.
    ///
    /// Uses strongly-connected components rather than enumerating every
    /// elementary cycle — enumeration is exponential in the worst case, and a
    /// cluster of mutually-dependent parts is one defect to break, not one per
    /// loop through it. Each returned path `[a, b, c]` is closed by `c → a`, and
    /// is rotated to start at its lexicographically smallest id so the output is
    /// stable run to run.
    pub(crate) fn circular_dependencies(&self) -> Result<Vec<DependencyCycle>, DynoError> {
        let edges = self.dependency_pairs()?;
        let pairs = &edges.pairs;
        if pairs.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = GraphBuilder::new();
        for (from, to) in pairs {
            builder.add_node(from);
            builder.add_node(to);
        }
        for (from, to) in pairs {
            builder
                .add_edge(from, to, 1.0)
                .map_err(|e| DynoError::Query(format!("dependency network edge: {e}")))?;
        }
        let graph = builder.build(true); // directed: a dependency has a direction

        let mut meta = vec![String::new(); graph.node_count()];
        for (from, to) in pairs {
            for id in [from, to] {
                if let Some(idx) = graph.idx_of(id) {
                    meta[idx] = id.clone();
                }
            }
        }

        let mut cycles = Vec::new();

        // A node that depends on itself is a degenerate cycle Tarjan reports as
        // a singleton SCC — catch it explicitly rather than losing it.
        for (from, to) in pairs {
            if from == to {
                cycles.push(vec![from.clone()]);
            }
        }

        for group in strongly_connected_components(&graph).groups() {
            if group.len() < 2 {
                continue;
            }
            let members: HashSet<usize> = group.iter().copied().collect();
            // Rebuild just this cluster so `find_cycle` returns a path inside it.
            let mut sub = GraphBuilder::new();
            let mut sub_ids: Vec<&str> = group.iter().map(|&i| meta[i].as_str()).collect();
            sub_ids.sort_unstable();
            for id in &sub_ids {
                sub.add_node(id);
            }
            for &i in &group {
                for &(j, _) in graph.out_neighbors(i) {
                    if members.contains(&j) {
                        sub.add_edge(&meta[i], &meta[j], 1.0).map_err(|e| {
                            DynoError::Query(format!("dependency subgraph edge: {e}"))
                        })?;
                    }
                }
            }
            let sub_graph = sub.build(true);
            if let Some(path) = find_cycle(&sub_graph) {
                let mut ids: Vec<String> = path
                    .iter()
                    .map(|&i| {
                        sub_ids
                            .iter()
                            .find(|id| sub_graph.idx_of(id) == Some(i))
                            .map(|id| id.to_string())
                            .unwrap_or_default()
                    })
                    .collect();
                // Rotate to start at the smallest id — same cycle, stable text.
                if let Some(start) = ids
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.cmp(b.1))
                    .map(|(i, _)| i)
                {
                    ids.rotate_left(start);
                }
                cycles.push(ids);
            }
        }

        cycles.sort();

        // Attribute each loop: which Interfaces its hops were collapsed out of,
        // and whether ANY hop is a real DEPENDS_ON edge. A loop closes, so the
        // hops are every consecutive pair plus `last → first` (and a
        // self-cycle's single hop is `a → a`).
        Ok(cycles
            .into_iter()
            .map(|path| {
                let mut via: BTreeSet<String> = BTreeSet::new();
                let mut any_direct = false;
                for i in 0..path.len() {
                    let hop = (path[i].clone(), path[(i + 1) % path.len()].clone());
                    if let Some(ifaces) = edges.contract.get(&hop) {
                        via.extend(ifaces.iter().cloned());
                    }
                    if edges.direct.contains(&hop) {
                        any_direct = true;
                    }
                }
                let foundation_media_only = !via.is_empty()
                    && via.iter().all(|id| {
                        self.get_node(node::INTERFACE, id)
                            .ok()
                            .flatten()
                            .and_then(|n| {
                                n.properties
                                    .get("medium")
                                    .and_then(dynograph_core::Value::as_str)
                                    .map(is_foundation_medium)
                            })
                            .unwrap_or(false)
                    });
                DependencyCycle {
                    path,
                    via_interfaces: via.into_iter().collect(),
                    contracts_only: !any_direct,
                    foundation_media_only,
                }
            })
            .collect())
    }

    /// Whether removing `node_id` **creates** a split — leaving more non-trivial
    /// subsystems (≥2 nodes each) than the design already had — measured on the
    /// **as-built operational network**, not the full design network.
    ///
    /// Measured against the baseline, not against a fixed count. Asking "are
    /// there ≥2 non-trivial components after removal?" silently assumed the
    /// design was connected to begin with. It usually is not: one unrelated
    /// island of two nodes already satisfies the test, so **every** articulation
    /// point anywhere else in the graph reports as a single point of failure
    /// while nothing about its fragility is different.
    ///
    /// This is the defect the blind trial described from the other side — *"all
    /// 15 defects vanished at once when I added two bookkeeping edges; nothing
    /// about actual fragility changed."* Those edges attached the island. The
    /// count fell below the threshold and the whole list cleared, which is the
    /// same bug wearing the opposite sign.
    ///
    /// Modelling reflow2's own design showed it directly: two capabilities that
    /// were correctly *not* flagged became single points of failure the moment a
    /// disconnected second crate was added beside them.
    ///
    /// **Why the operational network (BL-69, the fourth pass).** Connectivity
    /// used to be measured on the full design network, where intent edges are
    /// wrong in both directions at once. They donate *mass*: a component whose
    /// capability, artifact and verification hang off it strands that intent
    /// cluster on removal, so a healthily-modelled leaf module fired — the
    /// severed "subsystem" was made of sentences. And they donate *phantom
    /// connectivity*: a genuine operational cut vertex stayed silent because
    /// the parts it severs remained "connected" through a SATISFIES chain — a
    /// path that carries nothing at run time. Found dispositioning reflow2's
    /// own two warnings (2026-07-21): `cmp:flow` fired on the first defect
    /// while `cmp:export`, a true cut vertex, hid behind the second.
    /// Does this component couple to the rest of the design **only** through
    /// contracts carried by a library — that is, linked into its consumers
    /// rather than called across a boundary at run time?
    ///
    /// The third pass at `single_point_of_failure`, and the same lesson one
    /// level down. BL-5's second pass scoped candidates to types that operate,
    /// because redundancy is meaningless for a sentence. But `Component`
    /// covers both a running service and a shared library, and on storyflow —
    /// 15 components, 7 flagged — the false ones were exactly the libraries:
    /// `dag_transport` and `type_registry` are imported by every service, so
    /// they are perfect articulation points, and *you cannot run two copies of
    /// a library to survive its failure*. The suggested fix, `add_redundancy`,
    /// is not merely unhelpful there; it is incoherent.
    ///
    /// Topology cannot tell the two apart: a library API and a service API are
    /// the same shape in the graph. The discriminator is a statement the
    /// modeller makes — `Interface.medium` — and it defaults to `REST`, so a
    /// design that says nothing keeps today's behaviour exactly. **Silence has
    /// to be earned by an explicit `library`**, which is the right direction
    /// for a detector: never quiet by default (sharpening.md §4).
    ///
    /// A component providing a mix still counts: if anything it exposes is
    /// carried at run time, it is a thing that can fail at run time.
    pub(crate) fn couples_only_as_a_library(&self, node_id: &str) -> Result<bool, DynoError> {
        let provided = self.outgoing(node_id, Some(edge::PROVIDES))?;
        if provided.is_empty() {
            return Ok(false);
        }
        for e in provided {
            let by_library = self
                .get_node(node::INTERFACE, &e.to_id)?
                .and_then(|i| {
                    i.properties
                        .get("medium")
                        .and_then(dynograph_core::Value::as_str)
                        .map(is_foundation_medium)
                })
                .unwrap_or(false);
            if !by_library {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether an `Interface` node is *itself* a `library`/`data` foundation.
    /// The Interface-node twin of [`Self::couples_only_as_a_library`], which
    /// keys on a *component's* provided contracts: when two subsystems meet at a
    /// single shared foundation contract, the `Interface` is the articulation
    /// point rather than the provider, and it is no more a runtime failure point
    /// than the library component is (BL-84). A missing or non-foundation medium
    /// is a run-time contract, so it stays a candidate — silence is earned by an
    /// explicit `library`/`data`, never the default.
    pub(crate) fn interface_is_foundation(&self, node_id: &str) -> Result<bool, DynoError> {
        Ok(self
            .get_node(node::INTERFACE, node_id)?
            .and_then(|i| {
                i.properties
                    .get("medium")
                    .and_then(dynograph_core::Value::as_str)
                    .map(is_foundation_medium)
            })
            .unwrap_or(false))
    }

    pub(crate) fn is_single_point_of_failure(&self, node_id: &str) -> Result<bool, DynoError> {
        let baseline = self.operational_network(None)?.nontrivial_component_count();
        Ok(self
            .operational_network(Some(node_id))?
            .nontrivial_component_count()
            > baseline)
    }
}
