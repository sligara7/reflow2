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
//! - `orphan_node` — a Capability not `ALLOCATED_TO`, an Artifact `REALIZES`-ing
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

use crate::detect::fnv1a;
use crate::graph::DesignGraph;
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
    /// endpoint is unknown, an edge triple both nodes already had — used to go
    /// unreported, which is the silent drop rule 4 forbids. Empty on a merge that
    /// lost nothing.
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

/// Order a pair of ids canonically so the smaller is kept on a merge — makes the
/// choice deterministic regardless of which way the `DUPLICATES` edge points.
fn canonical_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

impl DesignGraph {
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

        // orphan_node — missing golden-thread links.
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if self
                .outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))?
                .is_empty()
            {
                issues.push(orphan(
                    &cap.node_id,
                    "Capability",
                    "is not allocated to any component",
                    "generate_owner",
                ));
            }
        }
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
        for req in self.scan_nodes(node::REQUIREMENT)? {
            // A requirement the user has dropped or already met is not an
            // orphan to repair. DETECT skips these for the same reason
            // (`detect_unsatisfied_requirements`); without the check here, one
            // half of the system would go quiet while the other kept nagging
            // about the same requirement — which reads as a broken tool.
            let status = req
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("proposed");
            if status == "dropped" || status == "met" {
                continue;
            }
            if self
                .incoming(&req.node_id, Some(edge::SATISFIES))?
                .is_empty()
            {
                issues.push(orphan(
                    &req.node_id,
                    "Requirement",
                    "is satisfied by nothing",
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
            let (keep, remove) = canonical_pair(&e.from_id, &e.to_id);
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
            for island in &clusters[1..] {
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
        // ≥2 subsystems (not the leaf-cutting every tree-internal node does).
        for ap in net.articulation_points() {
            let id = net.id_of(ap).to_string();
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

        for issue in self.detect_defects()? {
            if !options.strategy.addresses(issue.severity) {
                continue;
            }
            issues_addressed.push(issue.id.clone());

            match issue.category {
                // The one content-free structural repair.
                HealCategory::Duplicate => match merge_op_for(&issue, &index) {
                    Ok(op) => operations.push(HealOperation {
                        issue_id: issue.id.clone(),
                        op,
                    }),
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

        let index = self.node_type_index()?;
        let mut applied = 0;
        let mut discarded: Vec<SkippedOperation> = Vec::new();
        for operation in &proposal.operations {
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
                        &index,
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

    /// Merge `remove` into `keep`: re-point `remove`'s edges onto `keep`, then
    /// delete `remove`. Atomic (one batch). `DUPLICATES` edges and edges to
    /// `keep`/`remove` themselves are skipped so no self-loop or dangling edge
    /// is produced.
    fn merge_nodes(
        &mut self,
        keep_type: &str,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
        index: &HashMap<String, String>,
    ) -> Result<Vec<SkippedOperation>, DynoError> {
        // Capture the edges to re-point before mutating anything.
        let outgoing = self.outgoing(remove_id, None)?;
        let incoming = self.incoming(remove_id, None)?;

        let mut discarded =
            self.merge_losses(keep_id, remove_type, remove_id, &outgoing, &incoming)?;

        self.begin_batch();
        match self.merge_repoint(
            keep_type,
            keep_id,
            remove_type,
            remove_id,
            &outgoing,
            &incoming,
            index,
            &mut discarded,
        ) {
            Ok(()) => {
                self.commit_batch()?;
                Ok(discarded)
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }

    /// What this merge will not be able to carry across, computed before it runs.
    ///
    /// Two kinds, both previously silent. The removed node's **properties** are
    /// never carried — only its edges are — so its name, description and status
    /// go with it. And where both nodes already had the same edge type to the
    /// same neighbour, `create_edge` is an upsert keyed on
    /// `(graph, type, from, to)`, so the removed node's edge properties land on
    /// top of the survivor's rather than beside them.
    fn merge_losses(
        &self,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
        outgoing: &[StoredEdge],
        incoming: &[StoredEdge],
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

        for e in outgoing {
            if e.edge_type != edge::DUPLICATES
                && !e.properties.is_empty()
                && existing_out.contains(&(e.edge_type.clone(), e.to_id.clone()))
            {
                discarded.push(SkippedOperation {
                    reference: format!("{remove_id} -{}-> {}", e.edge_type, e.to_id),
                    reason: format!(
                        "'{keep_id}' already has this edge; the merged edge's properties overwrite its own"
                    ),
                });
            }
        }
        for e in incoming {
            if e.edge_type != edge::DUPLICATES
                && !e.properties.is_empty()
                && existing_in.contains(&(e.edge_type.clone(), e.from_id.clone()))
            {
                discarded.push(SkippedOperation {
                    reference: format!("{} -{}-> {remove_id}", e.from_id, e.edge_type),
                    reason: format!(
                        "'{keep_id}' already has this edge; the merged edge's properties overwrite its own"
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
        index: &HashMap<String, String>,
        discarded: &mut Vec<SkippedOperation>,
    ) -> Result<(), DynoError> {
        for e in outgoing {
            if e.edge_type == edge::DUPLICATES {
                continue;
            }
            let other = &e.to_id;
            if other == keep_id || other == remove_id {
                continue; // avoid self-loop / edge to the node being deleted
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
            if e.edge_type == edge::DUPLICATES {
                continue;
            }
            let other = &e.from_id;
            if other == keep_id || other == remove_id {
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
