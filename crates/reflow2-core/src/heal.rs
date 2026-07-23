//! HEAL — self-repair for the design graph (docs/heal-process.md).
//!
//! The coherence loop's RESOLVE/HEAL step. HEAL detects *structural* defects and
//! repairs them — but **never mutates directly**: it emits a [`HealProposal`]
//! that a separate, atomic [`apply_heal`](DesignGraph::apply_heal) executes
//! (discipline 1: propose, then apply). This split is the whole point — a
//! proposal can be reviewed, capped, and audited before anything changes.
//!
//! Distinct from DETECT/gap-surfacing: DETECT *asks the human* for meaning it
//! can't infer; HEAL *fixes structure* it can. Fixes that need generated content
//! (a resolving Decision, an owner for an orphan) are gated behind
//! `requires_human_review` and left as [`GeneratedContentStub`]s for the
//! deferred LLM healer — this increment applies only content-free structural
//! repairs.
//!
//! This increment implements HEAL's backbone with the fully-deterministic defect
//! set:
//!
//! - `orphan_node` — a Capability not `ALLOCATED_TO` (nor `PART_OF_FLOW` — a
//!   process step's anchor is its Flow, BL-37), an Artifact `REALIZES`-ing
//!   nothing, a Requirement with no `SATISFIES`. Fix needs an *owner* → generative.
//! - `contradiction` — a `CONTRADICTS` edge. Fix = a resolving Decision → generative.
//! - `unresolved_setup` — an `ANTICIPATES` edge with no follow-through → generative.
//! - `duplicate` — a `DUPLICATES` edge. Fix = **merge** (endpoints known) — the
//!   one content-free structural repair, so it is what `apply_heal` executes.
//!
//! Deferred (need `dynograph-graph` or the LLM): dead_end / unreachable /
//! disconnected_community / weak_connection / single_point_of_failure /
//! missing_link (graph algorithms), missing_embedding, and every generative
//! healer's actual content.

use std::collections::{BTreeSet, HashMap};

use dynograph_core::DynoError;
use dynograph_storage::StoredEdge;

use crate::graph::DesignGraph;
use crate::nodes::fnv1a;
use crate::nodes::{edge, node};

/// The kind of structural defect (docs/heal-process.md defect catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealCategory {
    /// A node missing a golden-thread link it should have.
    OrphanNode,
    /// Two nodes joined by `CONTRADICTS` with no resolving Decision.
    Contradiction,
    /// Two nodes joined by `DUPLICATES` (candidates to merge).
    Duplicate,
    /// An `ANTICIPATES` with no follow-through — a planned need never built.
    UnresolvedSetup,
    /// A cluster of ≥2 design nodes with no link to the rest of the design.
    DisconnectedCommunity,
    /// A node whose removal splits the design into ≥2 non-trivial subsystems.
    SinglePointOfFailure,
    /// An isolated Component — nothing depends on it and it provides nothing.
    DeadEnd,
    /// A set of parts that depend on each other in a loop, directly via
    /// `DEPENDS_ON` or through the contracts they provide and consume.
    CircularDependency,
}

impl HealCategory {
    /// Stable snake_case key.
    pub fn as_str(self) -> &'static str {
        match self {
            HealCategory::OrphanNode => "orphan_node",
            HealCategory::Contradiction => "contradiction",
            HealCategory::Duplicate => "duplicate",
            HealCategory::UnresolvedSetup => "unresolved_setup",
            HealCategory::DisconnectedCommunity => "disconnected_community",
            HealCategory::SinglePointOfFailure => "single_point_of_failure",
            HealCategory::DeadEnd => "dead_end",
            HealCategory::CircularDependency => "circular_dependency",
        }
    }
}

/// Defect severity (docs/heal-process.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealSeverity {
    /// Must fix.
    Critical,
    /// Should fix.
    Warning,
    /// Nice to fix.
    Info,
}

/// A detected structural defect.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealIssue {
    /// Deterministic id: `heal:{hash(category + sorted affected ids)}`.
    pub id: String,
    /// What kind of defect.
    pub category: HealCategory,
    /// How serious.
    pub severity: HealSeverity,
    /// Human-readable description.
    pub message: String,
    /// The suggested fix — structural (`merge`) or generative (`generate_*`).
    pub suggested_fix_type: &'static str,
    /// Node ids involved.
    pub affected_ids: Vec<String>,
}

/// How aggressively to heal (docs/heal-process.md strategies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealStrategy {
    /// CRITICAL only.
    Conservative,
    /// CRITICAL + WARNING (the default).
    #[default]
    Balanced,
    /// All, including INFO.
    Aggressive,
}

impl HealStrategy {
    /// Whether this strategy addresses a defect of the given severity.
    fn addresses(self, severity: HealSeverity) -> bool {
        match self {
            HealStrategy::Conservative => severity == HealSeverity::Critical,
            HealStrategy::Balanced => severity != HealSeverity::Info,
            HealStrategy::Aggressive => true,
        }
    }
}

/// Options for a heal run.
#[derive(Debug, Clone, Copy)]
pub struct HealOptions {
    /// Which severities to address.
    pub strategy: HealStrategy,
    /// Cap on the number of structural operations; extras are surfaced in
    /// `skipped_operations`, never silently dropped (discipline 2).
    pub max_operations: Option<usize>,
}

impl Default for HealOptions {
    fn default() -> Self {
        Self {
            strategy: HealStrategy::Balanced,
            max_operations: None,
        }
    }
}

/// A structural graph operation HEAL proposes.
///
/// `PartialEq` is load-bearing: [`apply_heal`](DesignGraph::apply_heal) compares
/// each incoming operation against the ones HEAL would produce from the graph as
/// it stands, and refuses anything that does not match.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HealOp {
    /// Create an edge between two existing nodes.
    CreateEdge {
        edge_type: String,
        from_type: String,
        from_id: String,
        to_type: String,
        to_id: String,
    },
    /// Merge `remove` into `keep` (re-point `remove`'s edges onto `keep`, then
    /// delete `remove`).
    Merge {
        keep_type: String,
        keep_id: String,
        remove_type: String,
        remove_id: String,
    },
}

/// An operation tagged with the issue it addresses (so post-repair verification
/// can check exactly the defects the operations targeted).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HealOperation {
    /// Id of the [`HealIssue`] this operation resolves.
    pub issue_id: String,
    /// The graph mutation.
    pub op: HealOp,
}

/// A description of content the LLM healer must generate (deferred). Carrying
/// the description — not the content — keeps HEAL honest: it never ships an
/// un-generated fix as if it were done.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeneratedContentStub {
    /// Issue this would resolve.
    pub for_issue: String,
    /// What kind of node/content to generate (e.g. "Decision", "owner edge").
    pub kind: String,
    /// What the generator should produce.
    pub description: String,
}

/// An operation dropped from the proposal, with the reason (discipline 2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkippedOperation {
    /// The offending reference (issue id / node id).
    pub reference: String,
    /// Why it was skipped.
    pub reason: String,
}

/// A HEAL proposal (mirrors storyflow's `HealingProposalResponse`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealProposal {
    /// Project (or graph) being healed.
    pub target_id: String,
    /// Strategy used.
    pub strategy: HealStrategy,
    /// Ids of issues this proposal targets.
    pub issues_addressed: Vec<String>,
    /// Structural operations to apply.
    pub operations: Vec<HealOperation>,
    /// Generative fills awaiting the LLM healer + human review.
    pub generated_content: Vec<GeneratedContentStub>,
    /// Operations dropped, with reasons.
    pub skipped_operations: Vec<SkippedOperation>,
    /// 0..1 confidence in the proposal as a whole.
    pub confidence: f64,
    /// True whenever the proposal generates content (discipline 3).
    pub requires_human_review: bool,
    /// Human-readable summary.
    pub summary: String,
}

/// The outcome of applying a proposal.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealReport {
    /// Whether any operations were applied.
    pub applied: bool,
    /// True if apply was refused because the project is in `rigid` mode.
    pub blocked_by_mode: bool,
    /// How many structural operations were applied.
    pub operations_applied: usize,
    /// Whether post-repair verification confirmed the addressed defects are gone.
    pub verified: bool,
    /// Structural issue ids still detected after apply (should be empty when
    /// `verified`).
    pub unresolved_issue_ids: Vec<String>,
    /// Everything a merge could not carry onto the survivor, with the reason.
    ///
    /// A merge keeps the survivor's own properties and re-points the removed
    /// node's edges; it cannot keep both nodes' versions of the same thing. What
    /// it therefore lets go — the removed node's properties, an edge whose other
    /// endpoint is unknown, an edge triple both nodes already had, a
    /// non-DUPLICATES edge joining the merging pair (re-pointing it would make a
    /// self-loop) — used to go unreported, which is the silent drop rule 4
    /// forbids. Empty on a merge that lost nothing.
    pub discarded: Vec<SkippedOperation>,
    /// Human-readable outcome.
    pub message: String,
}

/// Deterministic issue id from category + affected ids (order-independent).
fn issue_id(category: HealCategory, affected: &[String]) -> String {
    let mut ids = affected.to_vec();
    ids.sort();
    format!(
        "heal:{:016x}",
        fnv1a(&format!("{}|{}", category.as_str(), ids.join(",")))
    )
}

/// The merge a `duplicate` issue implies, or the reason it cannot be built.
///
/// Shared by [`propose_heal`](DesignGraph::propose_heal) and
/// [`apply_heal`](DesignGraph::apply_heal) deliberately. Apply validates by
/// re-deriving what HEAL would propose and matching against it, so if the two
/// computed the operation separately they could drift, and a drift would make
/// apply refuse legitimate proposals — or worse, sanction ones HEAL never made.
fn merge_op_for(issue: &HealIssue, index: &HashMap<String, String>) -> Result<HealOp, String> {
    let (keep, remove) = (&issue.affected_ids[0], &issue.affected_ids[1]);
    // `x DUPLICATES x` is schema-valid (`* -> *`) and used to build a merge
    // whose re-pointing skips every edge ("already on the survivor") and whose
    // final delete then removed the survivor itself — a sanctioned self-merge
    // deleted the node and all its edges with no undo, reporting success
    // (BL-53). This guard covers propose AND apply: both derive through here.
    if keep == remove {
        return Err(format!(
            "'{keep}' cannot duplicate itself — a self-loop DUPLICATES edge is a \
             modelling error to delete (delete_edge), not a merge to apply"
        ));
    }
    let (Some(keep_type), Some(remove_type)) = (index.get(keep), index.get(remove)) else {
        // An endpoint that can't be resolved to a real node must never become a
        // phantom op (discipline 2).
        return Err("duplicate endpoint does not resolve to a real node".into());
    };
    // `DUPLICATES` is declared `from: "*" to: "*"`, so `Requirement DUPLICATES
    // Component` is schema-valid. Merging across types would re-point one type's
    // edges onto another and be rejected mid-batch by edge validation, after
    // earlier operations in the same proposal had already committed.
    if keep_type != remove_type {
        return Err(format!(
            "cannot merge across node types ({keep_type} and {remove_type}) — a DUPLICATES edge joins two different kinds of thing"
        ));
    }
    Ok(HealOp::Merge {
        keep_type: keep_type.clone(),
        keep_id: keep.clone(),
        remove_type: remove_type.clone(),
        remove_id: remove.clone(),
    })
}

/// Rank of a `provenance` value for the merge-survivor choice: lower survives.
///
/// The ordering encodes how directly a human stands behind the node's text —
/// because a merge keeps only the survivor's properties, so this choice decides
/// whose words are kept and whose go to `discarded`. `authored` and `planned`
/// are things a person actually said; `imported` came through a found document
/// (trusted per the ophyd caution — its PDR omitted the system's central
/// invariant); `reconciled` was written back from observed reality by a
/// machine; `inferred` is the machine's guess from the implementation; `healed`
/// is machine-generated fill. The machine's guess must never delete the
/// human's words.
///
/// `None` is a node **without** the property. Schema defaults materialize on
/// create, so only a node written before the property existed lacks it — a
/// pre-provenance vintage. It is probably a human's words, so it outranks
/// every machine provenance; but an explicit `authored` outranks *it*, because
/// ranking the two equal sent the choice to the id tiebreak and the alphabet
/// nearly deleted an authored, verified node in favour of its vintage stub
/// (BL-47, the 2026-07-20 self-adopt session).
fn provenance_rank(provenance: Option<&str>) -> u8 {
    match provenance {
        Some("authored") => 0,
        None => 1,
        Some("planned") => 2,
        Some("imported") => 3,
        Some("reconciled") => 4,
        Some("inferred") => 5,
        Some("healed") => 6,
        // The schema validates the enum, so this arm is unreachable for stored
        // values — but an unknown word must never outrank a known one.
        Some(_) => u8::MAX,
    }
}

impl DesignGraph {
    /// Which of a duplicate pair a merge keeps: **stronger provenance survives;
    /// equal provenance falls back to the smaller id** (the BL-29 survivor
    /// decision, taken by the user 2026-07-20 — option 2 of the recorded
    /// alternatives). Returns `(keep, remove)`.
    ///
    /// Provenance is what the choice is *for*: a merge keeps only the
    /// survivor's properties, and before this the lexicographically smaller id
    /// won regardless — so on an adopted graph an `inferred` stub could delete
    /// an `authored` node's words. The fallback keeps the choice fully
    /// deterministic regardless of which way the `DUPLICATES` edge points; a
    /// node without the property (a pre-provenance vintage, or a type that
    /// does not carry it) ranks just below an explicit `authored` and above
    /// everything else — so a vintage pair still ties and falls to the id,
    /// leaving pre-provenance graphs exactly as before, while an explicitly
    /// authored node beats its vintage twin instead of racing it on the
    /// alphabet (BL-47).
    fn merge_survivor(
        &self,
        index: &HashMap<String, String>,
        a: &str,
        b: &str,
    ) -> Result<(String, String), DynoError> {
        let rank_of = |id: &str| -> Result<u8, DynoError> {
            let Some(node_type) = index.get(id) else {
                // Unresolvable endpoint: rank is moot — merge_op_for refuses
                // the pair before an operation is built.
                return Ok(provenance_rank(None));
            };
            let stored = self.get_node(node_type, id)?;
            Ok(provenance_rank(
                stored
                    .as_ref()
                    .and_then(|n| n.properties.get("provenance"))
                    .and_then(dynograph_core::Value::as_str),
            ))
        };
        let (rank_a, rank_b) = (rank_of(a)?, rank_of(b)?);
        let a_survives = match rank_a.cmp(&rank_b) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => a <= b,
        };
        if a_survives {
            Ok((a.to_string(), b.to_string()))
        } else {
            Ok((b.to_string(), a.to_string()))
        }
    }

    /// The project's `mode` (`flexible` / `rigid`), or `flexible` if unset. In
    /// `rigid` mode HEAL only proposes; it never auto-applies (discipline 6).
    fn project_mode(&self) -> Result<String, DynoError> {
        Ok(self
            .scan_nodes(node::PROJECT)?
            .first()
            .and_then(|p| p.properties.get("mode"))
            .and_then(dynograph_core::Value::as_str)
            .unwrap_or("flexible")
            .to_string())
    }

    /// Every edge of `edge_type` in the graph, each returned once (from the
    /// out-side). Adjacency stores an edge once per direction, so scanning
    /// outgoing edges across all nodes enumerates each edge exactly once.
    fn all_edges_of_type(
        &self,
        edge_type: &str,
        index: &HashMap<String, String>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        let mut edges = Vec::new();
        for id in index.keys() {
            edges.extend(self.outgoing(id, Some(edge_type))?);
        }
        Ok(edges)
    }

    /// Detect the deterministic structural defects (the HEAL catalog subset).
    pub fn detect_defects(&self) -> Result<Vec<HealIssue>, DynoError> {
        let index = self.node_type_index()?;
        let mut issues = Vec::new();

        // orphan_node — missing golden-thread links, scoped to the ones DETECT
        // does not already ask about.
        //
        // A Capability with no `ALLOCATED_TO` and a Requirement with nothing
        // `SATISFIES`-ing it used to be reported here *as well as* by
        // `unallocated_capability` and `unsatisfied_requirement` — the same
        // finding twice, in two lists, in two vocabularies. Four independent
        // trials complained (ophyd 15, 3dtictactoe 10, the self-host run, and
        // storyflow where it became **20 of 31 defects** — the dominant noise
        // source in the output, BL-42).
        //
        // Removing them here rather than there follows the docs' own division:
        // *HEAL fills structure; gap-surfacing elicits meaning.* "Who should
        // own this?" and "what asked for this?" are meaning, they are
        // questions for a human, and they were never repairable — both mapped
        // to a `generate_owner` stub that `apply_heal` can never apply, so
        // they only ever inflated the defect count and the
        // awaiting-generation pile.
        //
        // What stays: an Artifact realizing nothing. DETECT has no counterpart
        // (its P3 detectors ask about capabilities, not files), so dropping it
        // would lose the finding entirely.
        for art in self.scan_nodes(node::ARTIFACT)? {
            if self
                .outgoing(&art.node_id, Some(edge::REALIZES))?
                .is_empty()
            {
                issues.push(orphan(
                    &art.node_id,
                    "Artifact",
                    "realizes nothing",
                    "generate_owner",
                ));
            }
        }

        // contradiction — a CONTRADICTS edge (unresolved in this increment).
        for e in self.all_edges_of_type(edge::CONTRADICTS, &index)? {
            let affected = vec![e.from_id.clone(), e.to_id.clone()];
            issues.push(HealIssue {
                id: issue_id(HealCategory::Contradiction, &affected),
                category: HealCategory::Contradiction,
                severity: HealSeverity::Warning,
                message: format!("'{}' and '{}' contradict each other", e.from_id, e.to_id),
                suggested_fix_type: "generate_decision",
                affected_ids: affected,
            });
        }

        // duplicate — a DUPLICATES edge (fixable by merge).
        for e in self.all_edges_of_type(edge::DUPLICATES, &index)? {
            let (keep, remove) = self.merge_survivor(&index, &e.from_id, &e.to_id)?;
            let affected = vec![keep, remove];
            issues.push(HealIssue {
                id: issue_id(HealCategory::Duplicate, &affected),
                category: HealCategory::Duplicate,
                severity: HealSeverity::Warning,
                message: format!("'{}' and '{}' cover the same ground", e.from_id, e.to_id),
                suggested_fix_type: "merge",
                affected_ids: affected,
            });
        }

        // unresolved_setup — an ANTICIPATES edge (info).
        for e in self.all_edges_of_type(edge::ANTICIPATES, &index)? {
            let affected = vec![e.from_id.clone(), e.to_id.clone()];
            issues.push(HealIssue {
                id: issue_id(HealCategory::UnresolvedSetup, &affected),
                category: HealCategory::UnresolvedSetup,
                severity: HealSeverity::Info,
                message: format!(
                    "'{}' anticipates '{}' but nothing follows through",
                    e.from_id, e.to_id
                ),
                suggested_fix_type: "generate_entity",
                affected_ids: affected,
            });
        }

        self.detect_structural_defects(&mut issues)?;

        issues.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(issues)
    }

    /// Graph-topology defects over the design network (via `dynograph-graph`):
    /// disconnected communities, selective single points of failure, dead ends.
    fn detect_structural_defects(&self, issues: &mut Vec<HealIssue>) -> Result<(), DynoError> {
        let net = self.design_network()?;

        // disconnected_community — islands of ≥2 nodes cut off from the main
        // body. Singletons are orphans/dead-ends, handled elsewhere; flag every
        // non-largest cluster of size ≥2.
        let mut clusters: Vec<Vec<usize>> = net
            .component_groups()
            .into_iter()
            .filter(|g| g.len() >= 2)
            .collect();
        if clusters.len() > 1 {
            // Keep the largest as "the main design"; the rest are islands. Sort
            // by size desc, then by first-member id for determinism.
            clusters.sort_by(|a, b| {
                b.len()
                    .cmp(&a.len())
                    .then(net.id_of(a[0]).cmp(net.id_of(b[0])))
            });
            let main_ids: BTreeSet<&str> = clusters[0].iter().map(|&i| net.id_of(i)).collect();
            for island in &clusters[1..] {
                // A cluster reachable from the main design through CONTAINS is a
                // decomposition scaffold, not an orphan. The design network
                // excludes CONTAINS on purpose (decomposition is not
                // traceability), so a subsystem grouping whose modules live in
                // the main body islands by construction — several subsystems tie
                // to each other through the Decision that governs them and reach
                // the body only downward through containment. dead_end already
                // exempts such an assembly ("an assembly speaks through its
                // children"); the community detector needs the same lesson
                // (BL-84, surfaced by BL-83a on reflow2's own self-model). A
                // genuinely disconnected cluster has no containment crossing its
                // boundary to the body and still fires.
                let island_ids: BTreeSet<&str> = island.iter().map(|&i| net.id_of(i)).collect();
                if self.island_attached_by_containment(&island_ids, &main_ids)? {
                    continue;
                }
                let mut affected: Vec<String> =
                    island.iter().map(|&i| net.id_of(i).to_string()).collect();
                affected.sort();
                issues.push(HealIssue {
                    id: issue_id(HealCategory::DisconnectedCommunity, &affected),
                    category: HealCategory::DisconnectedCommunity,
                    severity: HealSeverity::Warning,
                    message: format!(
                        "{} nodes form a cluster disconnected from the rest of the design",
                        affected.len()
                    ),
                    suggested_fix_type: "generate_bridge",
                    affected_ids: affected,
                });
            }
        }

        // single_point_of_failure — articulation points that actually separate
        // ≥2 subsystems (not the leaf-cutting every tree-internal node does),
        // and that name something which can *fail*.
        //
        // The candidate filter is what keeps this meaningful at real scale. A
        // golden thread converges on intent by design — every Requirement is
        // supposed to be the hub of what satisfies it — so on a 96-node design
        // the topology test alone named 22 nodes, most of them Requirements and
        // Capabilities that are load-bearing *because* they are cross-cutting.
        // The suggested fix is `add_redundancy`, and redundancy is only a
        // coherent idea for things that operate: a second copy of a sentence
        // adds no resilience, and a capability's failure *is* its component's
        // failure, already reported there. Intent nodes being articulation
        // points is the thread working, not a defect (BL-5, second pass — the
        // first fixed the island false-positive at fixture scale, and this
        // shape only appears above it).
        //
        // Candidates and connectivity both come from the *operational* network
        // (BL-69, the fourth pass): intent nodes not only must not be flagged,
        // they must not participate in the connectivity being measured. On the
        // full design network they donated mass (a component's own intent
        // cluster counted as a severed "subsystem") and phantom connectivity (a
        // real cut vertex stayed silent because its severed parts remained
        // joined through a SATISFIES chain). Artifacts are members of that
        // network — a stranded part with its file is a real severed subsystem —
        // but never candidates: the operational thing to make redundant is the
        // part, not the file.
        let op_net = self.operational_network(None)?;
        for ap in op_net.articulation_points() {
            let ty = op_net.type_of(ap);
            if !crate::structure::OPERATIONAL_TYPES.contains(&ty) {
                continue;
            }
            let id = op_net.id_of(ap).to_string();
            // An Interface that is itself a library/data foundation — linked
            // into or read by everything, so a perfect articulation point you
            // cannot make redundant — is the Interface twin of the library
            // component handled just below (BL-84). When two subsystems meet at
            // one shared foundation contract, the Interface is the cut vertex
            // rather than its provider.
            if ty == node::INTERFACE && self.interface_is_foundation(&id)? {
                continue;
            }
            // …and among components, only the ones that can fail *at run time*.
            // A shared library is imported by everything, which makes it a
            // perfect articulation point and a nonsense candidate: you cannot
            // run a second copy of a library to survive its failure. Keyed on
            // the contract's stated medium, which defaults to a runtime one —
            // see `couples_only_as_a_library` (F6, the storyflow trial).
            if self.couples_only_as_a_library(&id)? {
                continue;
            }
            if self.is_single_point_of_failure(&id)? {
                issues.push(HealIssue {
                    id: issue_id(HealCategory::SinglePointOfFailure, std::slice::from_ref(&id)),
                    category: HealCategory::SinglePointOfFailure,
                    severity: HealSeverity::Warning,
                    message: format!(
                        "every path between subsystems routes through '{id}' — a single point of failure"
                    ),
                    suggested_fix_type: "add_redundancy",
                    affected_ids: vec![id],
                });
            }
        }

        // circular_dependency — parts that depend on each other in a loop, via
        // DEPENDS_ON or through the contracts they provide/consume. Not
        // auto-fixable: breaking a cycle is a design decision (introduce an
        // interface, invert the dependency, go event-driven), so this is
        // reported for a human to resolve rather than repaired.
        for cycle in self.circular_dependencies()? {
            let mut affected = cycle.clone();
            affected.sort();
            let path = if cycle.len() == 1 {
                format!("'{}' depends on itself", cycle[0])
            } else {
                format!("{} → {}", cycle.join(" → "), cycle[0])
            };
            issues.push(HealIssue {
                id: issue_id(HealCategory::CircularDependency, &affected),
                category: HealCategory::CircularDependency,
                severity: HealSeverity::Critical,
                message: format!("circular dependency: {path}"),
                suggested_fix_type: "break_cycle",
                affected_ids: affected,
            });
        }

        // dead_end — an isolated Component (no traceability edges at all).
        //
        // "Isolated" is judged in the design network, which excludes CONTAINS
        // on purpose (decomposition is not traceability) — so a pure container,
        // the standard way to express a subsystem, has degree 0 here while
        // being exactly what it should be. An assembly speaks through its
        // children: if they are disconnected they are flagged individually, and
        // if they are connected the assembly is doing its one job. So a
        // component that CONTAINS other components is exempt; a *leaf* with no
        // traceability is a real dead end even inside a healthy hierarchy.
        for idx in 0..net.node_count() {
            if net.type_of(idx) == node::COMPONENT && net.degree(idx) == 0 {
                let id = net.id_of(idx).to_string();
                let mut is_assembly = false;
                for e in self.outgoing(&id, Some(edge::CONTAINS))? {
                    if self.get_node(node::COMPONENT, &e.to_id)?.is_some() {
                        is_assembly = true;
                        break;
                    }
                }
                if is_assembly {
                    continue;
                }
                issues.push(HealIssue {
                    id: issue_id(HealCategory::DeadEnd, std::slice::from_ref(&id)),
                    category: HealCategory::DeadEnd,
                    severity: HealSeverity::Warning,
                    message: format!("component '{id}' is not connected to anything"),
                    suggested_fix_type: "generate_bridge",
                    affected_ids: vec![id],
                });
            }
        }
        Ok(())
    }

    /// Whether any node in `island` reaches the main design `body` through a
    /// CONTAINS (decomposition) edge — the one traceability edge the design
    /// network excludes. Such an island is a subsystem grouping attached to the
    /// design through the hierarchy, not a true orphan (BL-84); the check keys
    /// on containment crossing the island boundary *to the body*, so an island
    /// with only internal or dangling containment is still genuinely
    /// disconnected and stays flagged.
    fn island_attached_by_containment(
        &self,
        island: &BTreeSet<&str>,
        body: &BTreeSet<&str>,
    ) -> Result<bool, DynoError> {
        for &id in island {
            for e in self.outgoing(id, Some(edge::CONTAINS))? {
                if body.contains(e.to_id.as_str()) {
                    return Ok(true);
                }
            }
            for e in self.incoming(id, Some(edge::CONTAINS))? {
                if body.contains(e.from_id.as_str()) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Produce a heal proposal for the current defects under `options`. Computes
    /// only — nothing is mutated (discipline 1).
    pub fn propose_heal(&self, options: HealOptions) -> Result<HealProposal, DynoError> {
        let index = self.node_type_index()?;
        let target_id = self
            .scan_nodes(node::PROJECT)?
            .first()
            .map(|p| p.node_id.clone())
            .unwrap_or_else(|| self.graph_id().to_string());

        let mut issues_addressed = Vec::new();
        let mut operations = Vec::new();
        let mut generated_content = Vec::new();
        let mut skipped_operations = Vec::new();
        // Nodes already committed to a merge in THIS proposal. A chained
        // duplicate (a↔b, b↔c) implies two merges sharing a node, and applying
        // both in one pass writes to a node the earlier merge deleted — so the
        // second link is deferred to the next propose/apply round instead.
        let mut merge_kept: BTreeSet<String> = BTreeSet::new();
        let mut merge_removed: BTreeSet<String> = BTreeSet::new();

        for issue in self.detect_defects()? {
            if !options.strategy.addresses(issue.severity) {
                continue;
            }
            issues_addressed.push(issue.id.clone());

            match issue.category {
                // The one content-free structural repair.
                HealCategory::Duplicate => match merge_op_for(&issue, &index) {
                    Ok(op) => {
                        let HealOp::Merge {
                            keep_id, remove_id, ..
                        } = &op
                        else {
                            unreachable!("merge_op_for only builds Merge ops")
                        };
                        let overlap = [keep_id, remove_id]
                            .into_iter()
                            .find(|id| merge_removed.contains(*id))
                            .or_else(|| merge_kept.contains(remove_id).then_some(remove_id));
                        if let Some(node_id) = overlap {
                            skipped_operations.push(SkippedOperation {
                                reference: issue.id.clone(),
                                reason: format!(
                                    "chained duplicate: '{node_id}' is already part of another merge \
                                     in this proposal — apply this proposal, then re-run propose_heal \
                                     for the rest of the chain"
                                ),
                            });
                        } else {
                            merge_kept.insert(keep_id.clone());
                            merge_removed.insert(remove_id.clone());
                            operations.push(HealOperation {
                                issue_id: issue.id.clone(),
                                op,
                            });
                        }
                    }
                    Err(reason) => skipped_operations.push(SkippedOperation {
                        reference: issue.id.clone(),
                        reason,
                    }),
                },
                // Everything else needs generated content → human review.
                HealCategory::OrphanNode => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "owner edge".to_string(),
                    description: format!(
                        "Propose the missing golden-thread link for {}",
                        issue.message
                    ),
                }),
                HealCategory::Contradiction => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "Decision".to_string(),
                    description: format!("Propose a Decision reconciling {}", issue.message),
                }),
                HealCategory::UnresolvedSetup => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "entity".to_string(),
                    description: format!("Propose the follow-through entity for {}", issue.message),
                }),
                HealCategory::DisconnectedCommunity | HealCategory::DeadEnd => generated_content
                    .push(GeneratedContentStub {
                        for_issue: issue.id.clone(),
                        kind: "bridge".to_string(),
                        description: format!("Propose a bridging link for {}", issue.message),
                    }),
                HealCategory::SinglePointOfFailure => {
                    generated_content.push(GeneratedContentStub {
                        for_issue: issue.id.clone(),
                        kind: "redundancy".to_string(),
                        description: format!("Propose redundancy for {}", issue.message),
                    })
                }
                // Breaking a cycle is a design decision, not a mechanical edit —
                // which edge to invert, whether to introduce an interface, whether
                // to go event-driven. Always human-reviewed, never auto-applied.
                HealCategory::CircularDependency => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "cycle break".to_string(),
                    description: format!(
                        "Propose how to break the loop for {} — invert one dependency, \
                         introduce an interface, or make the link event-driven",
                        issue.message
                    ),
                }),
            }
        }

        // Cap structural operations; surface the overflow, don't drop it.
        if let Some(cap) = options.max_operations {
            while operations.len() > cap {
                let extra = operations.pop().expect("len > cap implies non-empty");
                skipped_operations.push(SkippedOperation {
                    reference: extra.issue_id,
                    reason: format!("max_operations cap ({cap}) reached"),
                });
            }
        }

        let requires_human_review = !generated_content.is_empty();
        let confidence = if requires_human_review { 0.5 } else { 0.9 };
        let summary = format!(
            "{} issue(s) addressed: {} structural op(s), {} awaiting generation, {} skipped.",
            issues_addressed.len(),
            operations.len(),
            generated_content.len(),
            skipped_operations.len()
        );

        Ok(HealProposal {
            target_id,
            strategy: options.strategy,
            issues_addressed,
            operations,
            generated_content,
            skipped_operations,
            confidence,
            requires_human_review,
            summary,
        })
    }

    /// Every structural operation HEAL sanctions for the graph as it stands.
    ///
    /// Deliberately ignores strategy and `max_operations`: those decide which
    /// subset of legitimate operations a *proposal* carries, and validation only
    /// asks whether an operation is legitimate at all.
    fn sanctioned_operations(&self) -> Result<Vec<HealOperation>, DynoError> {
        let index = self.node_type_index()?;
        let mut ops = Vec::new();
        for issue in self.detect_defects()? {
            if issue.category == HealCategory::Duplicate
                && let Ok(op) = merge_op_for(&issue, &index)
            {
                ops.push(HealOperation {
                    issue_id: issue.id.clone(),
                    op,
                });
            }
        }
        Ok(ops)
    }

    /// Atomically apply a proposal's **structural** operations (the generative
    /// content is left for the deferred LLM healer + human review), then verify
    /// the addressed structural defects are gone (discipline 4).
    ///
    /// In `rigid` project mode nothing is applied — the proposal is returned as
    /// recorded-only (discipline 6).
    ///
    /// # The proposal is checked, not trusted
    ///
    /// Every operation must match one HEAL would produce from the graph as it
    /// stands — same issue id, same operation. Anything else is refused **before
    /// a single write**, so a rejected proposal leaves the graph untouched.
    ///
    /// This was not always so, and the gap was not theoretical: a hand-written
    /// proposal carrying a made-up issue id and a `Merge` naming two capabilities
    /// that no detector had called duplicates was applied, and deleted one of
    /// them. `apply_heal` reads caller JSON straight off the MCP surface, so any
    /// client could do it, and a merge has no snapshot and no undo.
    ///
    /// Propose-then-apply is described as the whole point — a proposal can be
    /// reviewed, capped and audited before anything changes — but nothing bound
    /// the applied proposal to one HEAL actually made. Note also that
    /// `requires_human_review` is computed per *proposal* and is not consulted
    /// here; it reports that generative stubs are present, and has never been a
    /// gate on applying the structural half.
    ///
    /// Re-deriving costs one `detect_defects` pass and is what makes the
    /// operation's meaning — *this defect is real right now* — true at the moment
    /// of writing rather than at the moment of proposing.
    ///
    /// Sanctioning is per-operation, so it cannot see that two individually
    /// legitimate merges share a node — the chained-duplicate shape a↔b, b↔c.
    /// A separate guard refuses such a proposal outright; `propose_heal` never
    /// emits one, so the chain resolves one propose/apply round per link.
    pub fn apply_heal(&mut self, proposal: &HealProposal) -> Result<HealReport, DynoError> {
        if self.project_mode()? == "rigid" {
            return Ok(HealReport {
                applied: false,
                blocked_by_mode: true,
                operations_applied: 0,
                verified: false,
                unresolved_issue_ids: proposal
                    .operations
                    .iter()
                    .map(|o| o.issue_id.clone())
                    .collect(),
                discarded: Vec::new(),
                message: "rigid project mode: proposal recorded, not auto-applied".into(),
            });
        }

        // A node a merge deletes must not appear in any other operation of the
        // same proposal. Each operation can be individually sanctioned — on a
        // chain a↔b, b↔c both merges are — yet applying both writes to a node
        // the earlier merge deleted. The storage layer accepts the dangling
        // edge, so the graph corrupts silently while the report says
        // `verified` (reproduced before this guard existed: `cap:c`'s edges
        // re-pointed onto the already-deleted `cap:b`). `propose_heal` defers
        // the second link of a chain; this refuses the hand-built proposal
        // that carries both anyway.
        for (i, a) in proposal.operations.iter().enumerate() {
            let HealOp::Merge { remove_id, .. } = &a.op else {
                continue;
            };
            for (j, b) in proposal.operations.iter().enumerate() {
                if i == j {
                    continue;
                }
                let touches = match &b.op {
                    HealOp::Merge {
                        keep_id: k,
                        remove_id: r,
                        ..
                    } => k == remove_id || r == remove_id,
                    HealOp::CreateEdge { from_id, to_id, .. } => {
                        from_id == remove_id || to_id == remove_id
                    }
                };
                if touches {
                    return Err(DynoError::Validation {
                        node_type: remove_id.clone(),
                        property: "operation".into(),
                        message: format!(
                            "two operations in this proposal touch '{remove_id}', which one of them \
                             deletes — the later one would write to a node that no longer exists. \
                             Apply one link of the chain, then re-run propose_heal. Nothing was changed."
                        ),
                    });
                }
            }
        }

        // Refuse the whole proposal before mutating anything, so a rejected one
        // never leaves the graph half-changed.
        let sanctioned = self.sanctioned_operations()?;
        for operation in &proposal.operations {
            if !sanctioned.iter().any(|s| s == operation) {
                let subject = match &operation.op {
                    HealOp::Merge { remove_id, .. } => remove_id.clone(),
                    HealOp::CreateEdge { from_id, .. } => from_id.clone(),
                };
                return Err(DynoError::Validation {
                    node_type: subject,
                    property: "operation".into(),
                    message: format!(
                        "operation for issue '{}' is not one HEAL proposes for this graph — \
                         re-run propose_heal and apply that. Nothing was changed.",
                        operation.issue_id
                    ),
                });
            }
        }

        // All operations land together or not at all (BL-58). Previously each
        // merge/create was its own write, so a failure in operation N committed
        // 1..N-1 while returning a bare Err that implied nothing happened — and
        // a merge has no snapshot and no undo. `merge_nodes` captures its edges
        // up front (BL-29), and the pre-write guard above forbids two merges
        // sharing a node, so no operation reads another's buffered write:
        // batching is safe.
        let index = self.node_type_index()?;
        self.begin_batch();
        let (applied, discarded) = match self.apply_heal_operations(&proposal.operations, &index) {
            Ok(result) => {
                self.commit_batch()?;
                result
            }
            Err(e) => {
                self.discard_batch();
                return Err(e);
            }
        };

        // Post-repair verification: only the issues the OPERATIONS targeted.
        let op_issue_ids: std::collections::HashSet<&str> = proposal
            .operations
            .iter()
            .map(|o| o.issue_id.as_str())
            .collect();
        let remaining: std::collections::HashSet<String> =
            self.detect_defects()?.into_iter().map(|i| i.id).collect();
        let unresolved: Vec<String> = op_issue_ids
            .iter()
            .filter(|id| remaining.contains(**id))
            .map(|id| id.to_string())
            .collect();

        let message = if discarded.is_empty() {
            format!("applied {applied} structural operation(s)")
        } else {
            format!(
                "applied {applied} structural operation(s); {} thing(s) could not be carried across — see `discarded`",
                discarded.len()
            )
        };
        Ok(HealReport {
            applied: applied > 0,
            blocked_by_mode: false,
            operations_applied: applied,
            verified: unresolved.is_empty(),
            unresolved_issue_ids: unresolved,
            discarded,
            message,
        })
    }

    /// Run every operation, assuming the caller holds an open batch (BL-58).
    /// Any error propagates so the caller discards the batch — all-or-nothing.
    fn apply_heal_operations(
        &mut self,
        operations: &[HealOperation],
        index: &HashMap<String, String>,
    ) -> Result<(usize, Vec<SkippedOperation>), DynoError> {
        let mut applied = 0;
        let mut discarded: Vec<SkippedOperation> = Vec::new();
        for operation in operations {
            match &operation.op {
                HealOp::Merge {
                    keep_type,
                    keep_id,
                    remove_type,
                    remove_id,
                } => {
                    discarded.extend(self.merge_nodes(
                        keep_type,
                        keep_id,
                        remove_type,
                        remove_id,
                        index,
                    )?);
                    applied += 1;
                }
                HealOp::CreateEdge {
                    edge_type,
                    from_type,
                    from_id,
                    to_type,
                    to_id,
                } => {
                    self.create_edge(
                        edge_type,
                        from_type,
                        from_id,
                        to_type,
                        to_id,
                        crate::nodes::Props::new(),
                    )?;
                    applied += 1;
                }
            }
        }
        Ok((applied, discarded))
    }

    /// Merge `remove` into `keep`: re-point `remove`'s edges onto `keep`, then
    /// delete `remove`. Batch-free — the caller holds one batch across all
    /// operations. Edges between the pair themselves
    /// are dropped so no self-loop is produced — the pair's own `DUPLICATES`
    /// edge silently (resolving it is the merge's purpose), anything else with
    /// a `discarded` entry. A `DUPLICATES` edge to a *third* node is re-pointed
    /// like any other edge: on a chain a↔b, b↔c, merging b away must leave
    /// a↔c behind, or the user's still-unresolved duplicate claim about c
    /// would vanish with b.
    fn merge_nodes(
        &mut self,
        keep_type: &str,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
        index: &HashMap<String, String>,
    ) -> Result<Vec<SkippedOperation>, DynoError> {
        // Capture the edges to re-point — and the survivor's own, which win
        // any collision — before mutating anything.
        let outgoing = self.outgoing(remove_id, None)?;
        let incoming = self.incoming(remove_id, None)?;
        let existing_out: BTreeSet<(String, String)> = self
            .outgoing(keep_id, None)?
            .into_iter()
            .map(|e| (e.edge_type, e.to_id))
            .collect();
        let existing_in: BTreeSet<(String, String)> = self
            .incoming(keep_id, None)?
            .into_iter()
            .map(|e| (e.edge_type, e.from_id))
            .collect();

        let mut discarded = self.merge_losses(
            keep_id,
            remove_type,
            remove_id,
            &outgoing,
            &incoming,
            &existing_out,
            &existing_in,
        )?;

        // No batch here: `apply_heal` — the only caller — wraps the whole
        // operation list in ONE batch, so a failure in any operation rolls the
        // entire apply back (BL-58). A batch opened here would nest, and the
        // engine's `begin_batch` auto-commits the outer batch on nesting, which
        // would defeat that atomicity. All reads above are captured before any
        // write, so `merge_repoint` is pure mutation.
        self.merge_repoint(
            keep_type,
            keep_id,
            remove_type,
            remove_id,
            &outgoing,
            &incoming,
            &existing_out,
            &existing_in,
            index,
            &mut discarded,
        )?;
        Ok(discarded)
    }

    /// What this merge will not be able to carry across, computed before it runs.
    ///
    /// Two kinds, both previously silent. The removed node's **properties** are
    /// never carried — only its edges are — so its name, description and status
    /// go with it. And where both nodes already had the same edge type to the
    /// same neighbour, the survivor's edge is kept and the removed node's edge
    /// properties are dropped — `merge_repoint` skips the collision, because
    /// `create_edge` is an upsert keyed on `(graph, type, from, to)` and
    /// re-pointing would land the removed node's properties on top of the
    /// survivor's own (report-then-clobber was BL-47's second finding; a merge
    /// keeps the survivor's words on edges for the same reason it does on the
    /// node).
    #[allow(clippy::too_many_arguments)]
    fn merge_losses(
        &self,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
        outgoing: &[StoredEdge],
        incoming: &[StoredEdge],
        existing_out: &BTreeSet<(String, String)>,
        existing_in: &BTreeSet<(String, String)>,
    ) -> Result<Vec<SkippedOperation>, DynoError> {
        let mut discarded = Vec::new();

        if let Some(gone) = self.get_node(remove_type, remove_id)? {
            let mut names: Vec<&String> = gone.properties.keys().collect();
            names.sort();
            if !names.is_empty() {
                discarded.push(SkippedOperation {
                    reference: remove_id.to_string(),
                    reason: format!(
                        "properties not carried onto '{keep_id}' (a merge keeps the survivor's own): {}",
                        names
                            .iter()
                            .map(|k| k.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }
        }

        for e in outgoing {
            // Pair-joining edges are never re-pointed (see merge_repoint), so
            // they cannot collide; everything else — DUPLICATES to a third
            // node included — moves and can.
            if e.to_id != keep_id
                && e.to_id != remove_id
                && !e.properties.is_empty()
                && existing_out.contains(&(e.edge_type.clone(), e.to_id.clone()))
            {
                discarded.push(SkippedOperation {
                    reference: format!("{remove_id} -{}-> {}", e.edge_type, e.to_id),
                    reason: format!(
                        "'{keep_id}' already has this edge, and a merge keeps the survivor's own: the merged edge's properties are dropped"
                    ),
                });
            }
        }
        for e in incoming {
            if e.from_id != keep_id
                && e.from_id != remove_id
                && !e.properties.is_empty()
                && existing_in.contains(&(e.edge_type.clone(), e.from_id.clone()))
            {
                discarded.push(SkippedOperation {
                    reference: format!("{} -{}-> {remove_id}", e.from_id, e.edge_type),
                    reason: format!(
                        "'{keep_id}' already has this edge, and a merge keeps the survivor's own: the merged edge's properties are dropped"
                    ),
                });
            }
        }
        Ok(discarded)
    }

    #[allow(clippy::too_many_arguments)]
    fn merge_repoint(
        &mut self,
        keep_type: &str,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
        outgoing: &[StoredEdge],
        incoming: &[StoredEdge],
        existing_out: &BTreeSet<(String, String)>,
        existing_in: &BTreeSet<(String, String)>,
        index: &HashMap<String, String>,
        discarded: &mut Vec<SkippedOperation>,
    ) -> Result<(), DynoError> {
        for e in outgoing {
            let other = &e.to_id;
            if other == keep_id || other == remove_id {
                // The edge joins the merging pair (or loops), so it cannot be
                // re-pointed without becoming a self-loop. The pair's
                // DUPLICATES edge is what this merge resolves; anything else
                // was a real relationship and must not vanish silently.
                if e.edge_type != edge::DUPLICATES {
                    discarded.push(SkippedOperation {
                        reference: format!("{remove_id} -{}-> {other}", e.edge_type),
                        reason: format!(
                            "the edge joins the merging pair, so re-pointing it would make a self-loop on '{keep_id}'; it is not kept"
                        ),
                    });
                }
                continue;
            }
            if existing_out.contains(&(e.edge_type.clone(), other.clone())) {
                // The survivor already has this edge, and create_edge is an
                // upsert keyed on (graph, type, from, to): re-pointing would
                // land the removed node's properties on top of the survivor's
                // own. The survivor's version is kept; merge_losses reported
                // the drop if there was anything to lose.
                continue;
            }
            if let Some(to_type) = index.get(other) {
                self.create_edge(
                    &e.edge_type,
                    keep_type,
                    keep_id,
                    to_type,
                    other,
                    e.properties.clone(),
                )?;
            } else {
                // The other endpoint is not a node we know the type of, so the
                // edge cannot be recreated. Dropping it silently would lose a
                // relationship with nothing to say so.
                discarded.push(SkippedOperation {
                    reference: format!("{remove_id} -{}-> {other}", e.edge_type),
                    reason: format!(
                        "'{other}' is not a known node, so the edge could not be moved"
                    ),
                });
            }
        }
        for e in incoming {
            let other = &e.from_id;
            if other == keep_id || other == remove_id {
                if e.edge_type != edge::DUPLICATES {
                    discarded.push(SkippedOperation {
                        reference: format!("{other} -{}-> {remove_id}", e.edge_type),
                        reason: format!(
                            "the edge joins the merging pair, so re-pointing it would make a self-loop on '{keep_id}'; it is not kept"
                        ),
                    });
                }
                continue;
            }
            if existing_in.contains(&(e.edge_type.clone(), other.clone())) {
                // Same collision, incoming side: the survivor's edge wins.
                continue;
            }
            if let Some(from_type) = index.get(other) {
                self.create_edge(
                    &e.edge_type,
                    from_type,
                    other,
                    keep_type,
                    keep_id,
                    e.properties.clone(),
                )?;
            } else {
                discarded.push(SkippedOperation {
                    reference: format!("{other} -{}-> {remove_id}", e.edge_type),
                    reason: format!(
                        "'{other}' is not a known node, so the edge could not be moved"
                    ),
                });
            }
        }
        // Deletes remove and every edge still attached to it (incl. DUPLICATES).
        self.delete_node(remove_type, remove_id)?;
        Ok(())
    }
}

/// Build an `orphan_node` issue.
fn orphan(id: &str, type_label: &str, what: &str, fix: &'static str) -> HealIssue {
    let affected = vec![id.to_string()];
    HealIssue {
        id: issue_id(HealCategory::OrphanNode, &affected),
        category: HealCategory::OrphanNode,
        severity: HealSeverity::Warning,
        message: format!("{type_label} '{id}' {what}"),
        suggested_fix_type: fix,
        affected_ids: affected,
    }
}

#[cfg(test)]
mod tests {
    use super::provenance_rank;

    /// BL-47. A node without the property cannot be built through today's
    /// public API — schema defaults materialize on create — so the vintage
    /// slot is pinned here, at the function seam; the live reproduction is
    /// the 2026-07-20 self-adopt trial record.
    #[test]
    fn unset_provenance_sits_between_authored_and_everything_else() {
        assert!(
            provenance_rank(Some("authored")) < provenance_rank(None),
            "an explicit `authored` must beat a vintage node, not tie into the id lottery"
        );
        for machine_or_weaker in ["planned", "imported", "reconciled", "inferred", "healed"] {
            assert!(
                provenance_rank(None) < provenance_rank(Some(machine_or_weaker)),
                "a vintage node is probably a human's words; `{machine_or_weaker}` must not delete them"
            );
        }
    }

    #[test]
    fn the_explicit_order_is_unchanged_and_unknown_words_rank_last() {
        let explicit = [
            "authored",
            "planned",
            "imported",
            "reconciled",
            "inferred",
            "healed",
        ];
        for pair in explicit.windows(2) {
            assert!(
                provenance_rank(Some(pair[0])) < provenance_rank(Some(pair[1])),
                "`{}` must outrank `{}`",
                pair[0],
                pair[1]
            );
        }
        assert!(provenance_rank(Some("healed")) < provenance_rank(Some("word-not-in-the-enum")));
    }
}
