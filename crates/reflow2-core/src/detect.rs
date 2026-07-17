//! DETECT — read the graph, find where it's thin, and produce ranked gap
//! candidates (docs/gap-surfacing.md, the DIAGNOSE half of DIAGNOSE→PROMPT).
//!
//! This is the deterministic core of gap surfacing. It turns graph weaknesses
//! into [`GapCandidate`]s ranked by severity; turning a candidate into a plain-
//! language question the user answers is the **PROMPT** step (a `GapPrompt` with
//! LLM rephrase + anchoring), deferred with the rest of the LLM-reasoning ops.
//!
//! This increment implements two fully-deterministic detector groups:
//!
//! - **Traceability** — a node is missing a golden-thread link it should have
//!   (`unsatisfied_requirement`, `unallocated_capability`, `unrealized_capability`,
//!   `unverified_capability`).
//! - **Phase-coverage** — a whole lifecycle phase is absent
//!   (`concept_without_design`, `design_without_build`, `build_without_verification`,
//!   `no_deploy_operate`) — the doc's headline "you've done X but not Y".
//!
//! Two disciplines shape the design (docs/gap-surfacing.md):
//!
//! - **Detectors read computed signals, not raw filters** (discipline 1). Each
//!   detector is gated on type-population counts so it fires only when it should:
//!   phase-coverage fires at project scope when a downstream phase is *absent*;
//!   per-node traceability fires only once that phase *exists* but a specific
//!   node lacks its link — so an empty early-stage graph yields one project-level
//!   nudge, not N redundant per-node gaps.
//! - **Deterministic gap ids** (discipline 6) — `hash(source + affected ids)` so
//!   the same gap is stable across runs for dedup/caching.
//!
//! Deferred to later increments (noted so they're not mistaken for done):
//! structural gaps (need `dynograph-graph` centrality/components), quality/risk,
//! compliance (the environment layer), decomposition/matryoshka (`Component.level`),
//! SME considerations (LLM), and the whole PROMPT rephrase/anchor layer.

use dynograph_core::DynoError;

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// What a gap is about (docs/gap-surfacing.md taxonomy). Adding a detector is
/// one variant + one branch, per storyflow's convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapSource {
    // Phase-coverage
    /// Requirements/Capabilities exist, but no Components (WHERE).
    ConceptWithoutDesign,
    /// Components exist, but no Artifacts realize them.
    DesignWithoutBuild,
    /// Artifacts/Capabilities exist, but nothing verifies them.
    BuildWithoutVerification,
    /// Design/build exists, but no Release / Environment / Resource.
    NoDeployOperate,
    // Traceability
    /// A Requirement has no `SATISFIES` from any Capability.
    UnsatisfiedRequirement,
    /// A Capability is not `ALLOCATED_TO` any Component.
    UnallocatedCapability,
    /// A Capability has no `Artifact` `REALIZES`-ing it.
    UnrealizedCapability,
    /// A realized Capability/Artifact has no `Verification`.
    UnverifiedCapability,
}

impl GapSource {
    /// Stable snake_case key (used in the gap id hash and for display).
    pub fn as_str(self) -> &'static str {
        match self {
            GapSource::ConceptWithoutDesign => "concept_without_design",
            GapSource::DesignWithoutBuild => "design_without_build",
            GapSource::BuildWithoutVerification => "build_without_verification",
            GapSource::NoDeployOperate => "no_deploy_operate",
            GapSource::UnsatisfiedRequirement => "unsatisfied_requirement",
            GapSource::UnallocatedCapability => "unallocated_capability",
            GapSource::UnrealizedCapability => "unrealized_capability",
            GapSource::UnverifiedCapability => "unverified_capability",
        }
    }
}

/// The zoom level a gap is framed at (docs/gap-surfacing.md `scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapScope {
    /// Whole-project / lifecycle-level.
    Project,
    /// A lifecycle phase.
    Phase,
    /// Centered on a Component.
    Component,
    /// Centered on a Capability (or a single requirement/artifact node).
    Capability,
}

/// A detected gap, ranked for surfacing (mirrors storyflow's `ScenarioCandidate`).
///
/// The user-facing `GapPrompt` (context-setter + plain question + hints +
/// anchor) is produced later by the deferred PROMPT step; `evidence` is the
/// auditable, jargon-carrying signal that backs this candidate.
#[derive(Debug, Clone)]
pub struct GapCandidate {
    /// Deterministic id: `gap:{hash(source + sorted affected ids)}`.
    pub id: String,
    /// What kind of gap this is.
    pub gap_source: GapSource,
    /// Zoom level.
    pub scope: GapScope,
    /// Composite 0..1 — higher surfaces first.
    pub severity: f64,
    /// Short human-readable summary.
    pub title: String,
    /// Why this matters.
    pub description: String,
    /// The node ids involved.
    pub affected_ids: Vec<String>,
    /// 1..5 — how deep an answer to ask for (storyflow's "heat").
    pub suggested_depth: u8,
    /// Raw signal backing the gap, for auditing.
    pub evidence: String,
}

/// FNV-1a 64-bit — a small, stable, dependency-free hash so gap ids are
/// reproducible across runs and platforms (`std`'s `DefaultHasher` is not
/// guaranteed stable). Discipline 6.
pub(crate) fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Deterministic gap id from source + affected ids (order-independent).
fn gap_id(source: GapSource, affected: &[String]) -> String {
    let mut ids = affected.to_vec();
    ids.sort();
    format!(
        "gap:{:016x}",
        fnv1a(&format!("{}|{}", source.as_str(), ids.join(",")))
    )
}

/// Population counts of the node types the detectors gate on.
struct Population {
    requirements: usize,
    capabilities: usize,
    components: usize,
    artifacts: usize,
    verifications: usize,
    operate: usize, // Release + Environment + Resource
}

impl DesignGraph {
    fn population(&self) -> Result<Population, DynoError> {
        Ok(Population {
            requirements: self.count_nodes(node::REQUIREMENT)?,
            capabilities: self.count_nodes(node::CAPABILITY)?,
            components: self.count_nodes(node::COMPONENT)?,
            artifacts: self.count_nodes(node::ARTIFACT)?,
            verifications: self.count_nodes(node::VERIFICATION)?,
            operate: self.count_nodes(node::RELEASE)?
                + self.count_nodes(node::ENVIRONMENT)?
                + self.count_nodes(node::RESOURCE)?,
        })
    }

    /// Run all deterministic detectors and return gap candidates ranked
    /// most-severe first (ties broken by id for a stable order).
    pub fn detect_gaps(&self) -> Result<Vec<GapCandidate>, DynoError> {
        let pop = self.population()?;
        let mut gaps = Vec::new();

        self.detect_phase_coverage(&pop, &mut gaps);
        self.detect_unsatisfied_requirements(&pop, &mut gaps)?;
        self.detect_unallocated_capabilities(&pop, &mut gaps)?;
        self.detect_unrealized_capabilities(&pop, &mut gaps)?;
        self.detect_unverified_capabilities(&pop, &mut gaps)?;

        gaps.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        Ok(gaps)
    }

    // ---- Phase-coverage (project-scope, "you've done X but not Y") ---------

    fn detect_phase_coverage(&self, pop: &Population, gaps: &mut Vec<GapCandidate>) {
        let push = |gaps: &mut Vec<GapCandidate>,
                    source: GapSource,
                    severity: f64,
                    depth: u8,
                    title: &str,
                    description: &str,
                    evidence: String| {
            gaps.push(GapCandidate {
                id: gap_id(source, &[]),
                gap_source: source,
                scope: GapScope::Phase,
                severity,
                title: title.to_string(),
                description: description.to_string(),
                affected_ids: Vec::new(),
                suggested_depth: depth,
                evidence,
            });
        };

        if pop.requirements + pop.capabilities > 0 && pop.components == 0 {
            push(
                gaps,
                GapSource::ConceptWithoutDesign,
                0.70,
                3,
                "Concept defined, but no structure yet",
                "You've defined what it does, but nothing about how it's structured into buildable parts.",
                format!(
                    "{} requirement(s) + {} capability(ies) exist; 0 Components.",
                    pop.requirements, pop.capabilities
                ),
            );
        }
        if pop.components > 0 && pop.artifacts == 0 {
            push(
                gaps,
                GapSource::DesignWithoutBuild,
                0.60,
                3,
                "Design laid out, but nothing built yet",
                "Your design is laid out, but nothing actually gets built to realize it.",
                format!("{} Component(s) exist; 0 Artifacts.", pop.components),
            );
        }
        if pop.artifacts + pop.capabilities > 0 && pop.verifications == 0 {
            push(
                gaps,
                GapSource::BuildWithoutVerification,
                0.65,
                2,
                "Nothing confirms it works",
                "There's a design/build, but no way to confirm any of it actually works.",
                format!(
                    "{} artifact(s) + {} capability(ies) exist; 0 Verifications.",
                    pop.artifacts, pop.capabilities
                ),
            );
        }
        if pop.components + pop.artifacts > 0 && pop.operate == 0 {
            push(
                gaps,
                GapSource::NoDeployOperate,
                0.50,
                4,
                "No plan to deploy and operate it",
                "You have a concept and design — but nothing about how to deploy and operate it.",
                format!(
                    "{} component(s) + {} artifact(s) exist; 0 Release/Environment/Resource.",
                    pop.components, pop.artifacts
                ),
            );
        }
    }

    // ---- Traceability (per-node, gated on the phase existing) --------------

    fn detect_unsatisfied_requirements(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        // Only meaningful once capabilities exist to satisfy them.
        if pop.capabilities == 0 {
            return Ok(());
        }
        for req in self.scan_nodes(node::REQUIREMENT)? {
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
                let name = node_name(&req);
                let priority = req
                    .properties
                    .get("priority")
                    .and_then(dynograph_core::Value::as_str)
                    .unwrap_or("medium");
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnsatisfiedRequirement,
                        std::slice::from_ref(&req.node_id),
                    ),
                    gap_source: GapSource::UnsatisfiedRequirement,
                    scope: GapScope::Project,
                    severity: (0.5 + priority_bump(priority)).min(1.0),
                    title: format!("Nothing satisfies requirement “{name}”"),
                    description: format!(
                        "The requirement “{name}” has no capability delivering it — is it covered, deferred, or dropped?"
                    ),
                    affected_ids: vec![req.node_id.clone()],
                    suggested_depth: if priority == "critical" { 3 } else { 2 },
                    evidence: format!(
                        "Requirement '{}' (priority={priority}) has 0 incoming SATISFIES; project has {} capability(ies).",
                        req.node_id, pop.capabilities
                    ),
                });
            }
        }
        Ok(())
    }

    fn detect_unallocated_capabilities(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        if pop.components == 0 {
            return Ok(());
        }
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if self
                .outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))?
                .is_empty()
            {
                let name = node_name(&cap);
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnallocatedCapability,
                        std::slice::from_ref(&cap.node_id),
                    ),
                    gap_source: GapSource::UnallocatedCapability,
                    scope: GapScope::Capability,
                    severity: 0.50,
                    title: format!("Capability “{name}” isn't assigned to any part"),
                    description: format!(
                        "“{name}” isn't allocated to a component that will provide it — which part owns it?"
                    ),
                    affected_ids: vec![cap.node_id.clone()],
                    suggested_depth: 2,
                    evidence: format!(
                        "Capability '{}' has 0 outgoing ALLOCATED_TO; project has {} component(s).",
                        cap.node_id, pop.components
                    ),
                });
            }
        }
        Ok(())
    }

    fn detect_unrealized_capabilities(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        if pop.artifacts == 0 {
            return Ok(());
        }
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if self
                .incoming(&cap.node_id, Some(edge::REALIZES))?
                .is_empty()
            {
                let name = node_name(&cap);
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnrealizedCapability,
                        std::slice::from_ref(&cap.node_id),
                    ),
                    gap_source: GapSource::UnrealizedCapability,
                    scope: GapScope::Capability,
                    severity: 0.45,
                    title: format!("Nothing builds capability “{name}”"),
                    description: format!(
                        "“{name}” has no artifact realizing it — what actually gets built for it?"
                    ),
                    affected_ids: vec![cap.node_id.clone()],
                    suggested_depth: 2,
                    evidence: format!(
                        "Capability '{}' has 0 incoming REALIZES; project has {} artifact(s).",
                        cap.node_id, pop.artifacts
                    ),
                });
            }
        }
        Ok(())
    }

    fn detect_unverified_capabilities(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        if pop.verifications == 0 {
            return Ok(());
        }
        // Both Capabilities and Artifacts should carry a Verification once the
        // verify phase exists.
        for node_type in [node::CAPABILITY, node::ARTIFACT] {
            for n in self.scan_nodes(node_type)? {
                if self.incoming(&n.node_id, Some(edge::VERIFIES))?.is_empty() {
                    let name = node_name(&n);
                    gaps.push(GapCandidate {
                        id: gap_id(
                            GapSource::UnverifiedCapability,
                            std::slice::from_ref(&n.node_id),
                        ),
                        gap_source: GapSource::UnverifiedCapability,
                        scope: GapScope::Capability,
                        severity: 0.55,
                        title: format!("Nothing verifies “{name}”"),
                        description: format!(
                            "“{name}” has no verification proving it works — how will you confirm it?"
                        ),
                        affected_ids: vec![n.node_id.clone()],
                        suggested_depth: 2,
                        evidence: format!(
                            "{node_type} '{}' has 0 incoming VERIFIES; project has {} verification(s).",
                            n.node_id, pop.verifications
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The `name` property, falling back to the id.
fn node_name(n: &dynograph_storage::StoredNode) -> String {
    n.properties
        .get("name")
        .and_then(dynograph_core::Value::as_str)
        .unwrap_or(&n.node_id)
        .to_string()
}

/// Severity contribution of a requirement's priority.
fn priority_bump(priority: &str) -> f64 {
    match priority {
        "critical" => 0.40,
        "high" => 0.25,
        "medium" => 0.10,
        _ => 0.0,
    }
}
