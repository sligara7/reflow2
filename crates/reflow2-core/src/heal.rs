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

use std::collections::HashMap;

use dynograph_core::DynoError;
use dynograph_storage::StoredEdge;

use crate::detect::fnv1a;
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// The kind of structural defect (docs/heal-process.md defect catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealCategory {
    /// A node missing a golden-thread link it should have.
    OrphanNode,
    /// Two nodes joined by `CONTRADICTS` with no resolving Decision.
    Contradiction,
    /// Two nodes joined by `DUPLICATES` (candidates to merge).
    Duplicate,
    /// An `ANTICIPATES` with no follow-through — a planned need never built.
    UnresolvedSetup,
}

impl HealCategory {
    /// Stable snake_case key.
    pub fn as_str(self) -> &'static str {
        match self {
            HealCategory::OrphanNode => "orphan_node",
            HealCategory::Contradiction => "contradiction",
            HealCategory::Duplicate => "duplicate",
            HealCategory::UnresolvedSetup => "unresolved_setup",
        }
    }
}

/// Defect severity (docs/heal-process.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealSeverity {
    /// Must fix.
    Critical,
    /// Should fix.
    Warning,
    /// Nice to fix.
    Info,
}

/// A detected structural defect.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct HealOperation {
    /// Id of the [`HealIssue`] this operation resolves.
    pub issue_id: String,
    /// The graph mutation.
    pub op: HealOp,
}

/// A description of content the LLM healer must generate (deferred). Carrying
/// the description — not the content — keeps HEAL honest: it never ships an
/// un-generated fix as if it were done.
#[derive(Debug, Clone)]
pub struct GeneratedContentStub {
    /// Issue this would resolve.
    pub for_issue: String,
    /// What kind of node/content to generate (e.g. "Decision", "owner edge").
    pub kind: &'static str,
    /// What the generator should produce.
    pub description: String,
}

/// An operation dropped from the proposal, with the reason (discipline 2).
#[derive(Debug, Clone)]
pub struct SkippedOperation {
    /// The offending reference (issue id / node id).
    pub reference: String,
    /// Why it was skipped.
    pub reason: String,
}

/// A HEAL proposal (mirrors storyflow's `HealingProposalResponse`).
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

        issues.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(issues)
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
                HealCategory::Duplicate => {
                    let keep = &issue.affected_ids[0];
                    let remove = &issue.affected_ids[1];
                    match (index.get(keep), index.get(remove)) {
                        (Some(keep_type), Some(remove_type)) => {
                            operations.push(HealOperation {
                                issue_id: issue.id.clone(),
                                op: HealOp::Merge {
                                    keep_type: keep_type.clone(),
                                    keep_id: keep.clone(),
                                    remove_type: remove_type.clone(),
                                    remove_id: remove.clone(),
                                },
                            });
                        }
                        // An endpoint that can't be resolved to a real node must
                        // never become a phantom op (discipline 2).
                        _ => skipped_operations.push(SkippedOperation {
                            reference: issue.id.clone(),
                            reason: "duplicate endpoint does not resolve to a real node".into(),
                        }),
                    }
                }
                // Everything else needs generated content → human review.
                HealCategory::OrphanNode => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "owner edge",
                    description: format!(
                        "Propose the missing golden-thread link for {}",
                        issue.message
                    ),
                }),
                HealCategory::Contradiction => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "Decision",
                    description: format!("Propose a Decision reconciling {}", issue.message),
                }),
                HealCategory::UnresolvedSetup => generated_content.push(GeneratedContentStub {
                    for_issue: issue.id.clone(),
                    kind: "entity",
                    description: format!("Propose the follow-through entity for {}", issue.message),
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

    /// Atomically apply a proposal's **structural** operations (the generative
    /// content is left for the deferred LLM healer + human review), then verify
    /// the addressed structural defects are gone (discipline 4).
    ///
    /// In `rigid` project mode nothing is applied — the proposal is returned as
    /// recorded-only (discipline 6).
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
                message: "rigid project mode: proposal recorded, not auto-applied".into(),
            });
        }

        let index = self.node_type_index()?;
        let mut applied = 0;
        for operation in &proposal.operations {
            match &operation.op {
                HealOp::Merge {
                    keep_type,
                    keep_id,
                    remove_type,
                    remove_id,
                } => {
                    self.merge_nodes(keep_type, keep_id, remove_type, remove_id, &index)?;
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

        Ok(HealReport {
            applied: applied > 0,
            blocked_by_mode: false,
            operations_applied: applied,
            verified: unresolved.is_empty(),
            unresolved_issue_ids: unresolved,
            message: format!("applied {applied} structural operation(s)"),
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
    ) -> Result<(), DynoError> {
        // Capture the edges to re-point before mutating anything.
        let outgoing = self.outgoing(remove_id, None)?;
        let incoming = self.incoming(remove_id, None)?;

        self.begin_batch();
        match self.merge_repoint(
            keep_type,
            keep_id,
            remove_type,
            remove_id,
            &outgoing,
            &incoming,
            index,
        ) {
            Ok(()) => {
                self.commit_batch()?;
                Ok(())
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
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
