//! DETECT — read the graph, find where it's thin, and produce ranked gap
//! candidates (docs/gap-surfacing.md, the DIAGNOSE half of DIAGNOSE→PROMPT).
//!
//! This is the deterministic core of gap surfacing. It turns graph weaknesses
//! into [`GapCandidate`]s ranked by severity; turning a candidate into a plain-
//! language question the user answers is the **PROMPT** step (a `GapPrompt` with
//! LLM rephrase + anchoring), deferred with the rest of the LLM-reasoning ops.
//!
//! Deterministic detector groups:
//!
//! - **Traceability** — a node is missing a golden-thread link it should have
//!   (`unsatisfied_requirement`, `unallocated_capability`, `unrealized_capability`,
//!   `unverified_capability`).
//! - **Phase-coverage** — a whole lifecycle phase is absent
//!   (`concept_without_design`, `design_without_build`, `build_without_verification`,
//!   `no_deploy_operate`) — the doc's headline "you've done X but not Y".
//! - **Graph-analysis** — findings from the design network surfaced as gaps:
//!   `unexpected_coupling` (a lateral coupling bridging distant communities, from
//!   `surprising_connections`) and `declining_dimension` (quality trending down,
//!   from `dimension_drifts`).
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
//! remaining structural gaps (`orphan_node`/`dead_end` are detected in HEAL, not
//! yet surfaced here), compliance (the environment layer), decomposition/
//! matryoshka (`Component.level`), SME considerations (LLM), and the whole
//! PROMPT rephrase/anchor layer (beyond `to_prompt`).

use dynograph_core::DynoError;

use crate::dimensions::DriftDirection;
use crate::graph::DesignGraph;
use crate::hierarchy::HierarchyIssueKind;
use crate::llm::{LlmBackend, LlmRequest};
use crate::nodes::{edge, node};

/// What a gap is about (docs/gap-surfacing.md taxonomy). Adding a detector is
/// one variant + one branch, per storyflow's convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// A Capability has no `Verification` proving the behaviour works.
    ///
    /// The key string stays `unverified_capability` even though this variant
    /// once also covered Artifacts (see [`GapSource::UnverifiedArtifact`]).
    /// Gap ids hash that string, and an acknowledgement is stored under the
    /// resulting id — so changing it would silently expire every capability
    /// acknowledgement a user has made.
    UnverifiedCapability,
    /// An `Artifact` has no `Verification` covering it.
    ///
    /// Split from [`GapSource::UnverifiedCapability`], which reported both and
    /// titled an artifact gap "Nothing verifies reading.py" — semantically
    /// right, legibly wrong. The detection is unchanged: a test proving a
    /// capability works still does not prove this particular file is the thing
    /// that does it, so both are flagged.
    UnverifiedArtifact,
    // Interface pairing (the two sides of a contract)
    /// An `Interface` something `CONSUMES` that no Component `PROVIDES` — a
    /// break between two parts of the design.
    UnprovidedInterface,
    /// An `Interface` a Component `PROVIDES` that nothing `CONSUMES` — either a
    /// deliberate public contract or a leftover.
    UnconsumedInterface,
    // Graph-analysis (from the design network)
    /// A coupling edge bridges two otherwise-distant communities — a hidden
    /// coupling worth confirming (from `surprising_connections`).
    UnexpectedCoupling,
    /// A node's quality on some dimension is trending down over epochs (from
    /// `dimension_drifts`).
    DecliningDimension,
    // Decomposition / hierarchy (axis Y — from `hierarchy_issues`)
    /// A CONTAINS/DEPENDS_ON link skips ≥2 `Component.level`s.
    MissingIntermediateLevel,
    /// A CONTAINS whose parent is not strictly above its child.
    LevelMismatch,
    /// A subsystem-or-higher component with no parent above and no child below.
    OrphanLevel,
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
            // Load-bearing: this string is hashed into the gap id, which keys
            // the acknowledgement Decision. Renaming it expires every existing
            // capability acknowledgement with nothing to tell the user why.
            GapSource::UnverifiedCapability => "unverified_capability",
            GapSource::UnverifiedArtifact => "unverified_artifact",
            GapSource::UnprovidedInterface => "unprovided_interface",
            GapSource::UnconsumedInterface => "unconsumed_interface",
            GapSource::UnexpectedCoupling => "unexpected_coupling",
            GapSource::DecliningDimension => "declining_dimension",
            GapSource::MissingIntermediateLevel => "missing_intermediate_level",
            GapSource::LevelMismatch => "level_mismatch",
            GapSource::OrphanLevel => "orphan_level",
        }
    }
}

/// The zoom level a gap is framed at (docs/gap-surfacing.md `scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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

/// A gap the user has looked at and accepted, with the reason they gave.
///
/// Acknowledgement is stored as a [`Decision`](crate::nodes::node::DECISION) —
/// the same node an engineer would write anyway — so the reason lives in the
/// graph, propagates, and survives the session that made it. Nothing is hidden:
/// a reviewed gap moves to [`reviewed_gaps`](DesignGraph::reviewed_gaps) rather
/// than disappearing, because a list that silently shrinks is its own kind of
/// dishonesty.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewedGap {
    /// The gap itself, exactly as the detector reports it.
    pub gap: GapCandidate,
    /// Why it was accepted.
    pub reason: String,
    /// The `Decision` node recording the review.
    pub decision_id: String,
}

/// A detected gap, ranked for surfacing (mirrors storyflow's `ScenarioCandidate`).
///
/// The user-facing `GapPrompt` (context-setter + plain question + hints +
/// anchor) is produced later by the deferred PROMPT step; `evidence` is the
/// auditable, jargon-carrying signal that backs this candidate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// A gap turned into a plain-language question the user actually answers
/// (docs/gap-surfacing.md, the PROMPT half of DIAGNOSE→PROMPT). Produced from a
/// [`GapCandidate`] via an [`LlmBackend`] — the first LLM-reasoning op wired
/// through the pluggable boundary.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GapPrompt {
    /// 1–2 sentences placing the user back in their own design.
    pub context_setter: String,
    /// The specific thing to answer, in plain language (no graph jargon).
    pub question: String,
    /// Optional scaffolding / examples.
    pub hints: Vec<String>,
    /// The gap this addresses.
    pub candidate_id: String,
    /// True when LLM rephrase failed and this fell back to the raw candidate
    /// text — surfaced, never silently shipped as if polished (discipline
    /// GS-16). The candidate is never dropped.
    pub rephrase_degraded: bool,
}

impl GapCandidate {
    /// Rephrase this gap into a user-facing [`GapPrompt`] via `backend`.
    ///
    /// On any backend failure it **degrades gracefully**: it returns the raw
    /// candidate wording with `rephrase_degraded = true` rather than dropping
    /// the gap or pretending the fallback is polished (docs/gap-surfacing.md
    /// discipline: graceful-degrade-with-an-explicit-flag).
    pub fn to_prompt(&self, backend: &dyn LlmBackend) -> GapPrompt {
        let request = LlmRequest::new(format!(
            "Rewrite this design gap as one plain-language question for a non-engineer. \
             No graph/systems-engineering jargon. Return only the question.\n\n\
             Gap: {}\nWhy it matters: {}",
            self.title, self.description
        ))
        .with_system(
            "You help a designer fill gaps in their design by asking clear, \
             constructive questions grounded in their own work.",
        );

        match backend.complete(&request) {
            Ok(response) => GapPrompt {
                context_setter: self.title.clone(),
                question: response.text.trim().to_string(),
                hints: Vec::new(),
                candidate_id: self.id.clone(),
                rephrase_degraded: false,
            },
            Err(_) => GapPrompt {
                context_setter: self.title.clone(),
                question: self.description.clone(),
                hints: Vec::new(),
                candidate_id: self.id.clone(),
                rephrase_degraded: true,
            },
        }
    }
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

/// The `Decision` id that records a review of `gap_id`. Derived, so any session
/// can find an existing review without an index — and so a gap whose affected
/// set changes gets a *different* id, and with it a fresh judgement.
fn ack_decision_id(gap_id: &str) -> String {
    format!(
        "decision:ack:{}",
        gap_id.strip_prefix("gap:").unwrap_or(gap_id)
    )
}

/// Population counts of the node types the detectors gate on.
struct Population {
    requirements: usize,
    capabilities: usize,
    components: usize,
    interfaces: usize,
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
            interfaces: self.count_nodes(node::INTERFACE)?,
            artifacts: self.count_nodes(node::ARTIFACT)?,
            verifications: self.count_nodes(node::VERIFICATION)?,
            operate: self.count_nodes(node::RELEASE)?
                + self.count_nodes(node::ENVIRONMENT)?
                + self.count_nodes(node::RESOURCE)?,
        })
    }

    /// Accept a gap: record *why* it is fine, and move it to the reviewed
    /// bucket so the open list reflects what still needs attention.
    ///
    /// The review is a real `Decision` node — not a suppression flag — linked by
    /// `GOVERNED_BY` to each node the gap was about, so it is reachable from the
    /// design as well as from the gap. `affected_ids` should be the gap's own
    /// `affected_ids`; endpoints that no longer exist are skipped rather than
    /// authored as dangling edges.
    ///
    /// Idempotent: acknowledging the same gap twice updates the reason.
    pub fn acknowledge_gap(
        &mut self,
        gap_id: &str,
        affected_ids: &[String],
        reason: &str,
    ) -> Result<String, DynoError> {
        let decision_id = ack_decision_id(gap_id);
        self.create_node(
            node::DECISION,
            &decision_id,
            crate::nodes::Props::new()
                .set("name", format!("Reviewed: {gap_id}"))
                .set("decision", format!("Accepted the gap {gap_id}."))
                .set("rationale", reason)
                .set("status", "accepted"),
        )?;
        for target in affected_ids {
            let Some(node_type) = self.node_type_index()?.get(target).cloned() else {
                continue; // the gap outlived the node — nothing to attach to
            };
            // A repeat acknowledgement re-creates the same edge; that is fine.
            let _ = self.governed_by(&node_type, target, node::DECISION, &decision_id);
        }
        Ok(decision_id)
    }

    /// Withdraw a previously accepted gap: the `Decision` is marked
    /// `superseded` (never deleted — the past is not overwritten) and the gap
    /// returns to the open list.
    pub fn withdraw_gap_acknowledgement(&mut self, gap_id: &str) -> Result<bool, DynoError> {
        let decision_id = ack_decision_id(gap_id);
        let Some(existing) = self.get_node(node::DECISION, &decision_id)? else {
            return Ok(false);
        };
        let mut props = crate::nodes::Props::new().set("status", "superseded");
        for (k, v) in &existing.properties {
            if k != "status" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::DECISION, &decision_id, props)?;
        Ok(true)
    }

    /// The accepted review for a gap, if there is one: `(decision id, reason)`.
    /// A `superseded` or `rejected` Decision does not count — the gap is open again.
    fn gap_acknowledgement(&self, gap_id: &str) -> Result<Option<(String, String)>, DynoError> {
        let decision_id = ack_decision_id(gap_id);
        let Some(node) = self.get_node(node::DECISION, &decision_id)? else {
            return Ok(None);
        };
        if node.properties.get("status").and_then(|v| v.as_str()) != Some("accepted") {
            return Ok(None);
        }
        let reason = node
            .properties
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("(no reason recorded)")
            .to_string();
        Ok(Some((decision_id, reason)))
    }

    /// Open gaps — everything the detectors found that has **not** been
    /// reviewed and accepted, ranked most-severe first.
    ///
    /// Gaps you have accepted move to [`reviewed_gaps`](Self::reviewed_gaps).
    /// That split is the point: a gap list that can never reach zero teaches
    /// you to skim it, and a skimmed list is the failure this whole layer
    /// exists to prevent.
    pub fn detect_gaps(&self) -> Result<Vec<GapCandidate>, DynoError> {
        let mut open = Vec::new();
        for gap in self.all_gaps()? {
            if self.gap_acknowledgement(&gap.id)?.is_none() {
                open.push(gap);
            }
        }
        Ok(open)
    }

    /// Gaps that were reviewed and accepted, with the reason given for each.
    ///
    /// Worth re-reading when the design shifts: an acknowledgement is keyed to
    /// the gap's identity, which is a hash of its source *and its affected
    /// nodes* — so if the situation changes, the id changes, the old reason no
    /// longer applies, and the gap reappears in [`detect_gaps`](Self::detect_gaps)
    /// to be judged afresh.
    pub fn reviewed_gaps(&self) -> Result<Vec<ReviewedGap>, DynoError> {
        let mut reviewed = Vec::new();
        for gap in self.all_gaps()? {
            if let Some((decision_id, reason)) = self.gap_acknowledgement(&gap.id)? {
                reviewed.push(ReviewedGap {
                    gap,
                    reason,
                    decision_id,
                });
            }
        }
        Ok(reviewed)
    }

    /// Run all deterministic detectors and return gap candidates ranked
    /// most-severe first (ties broken by id for a stable order), regardless of
    /// whether they have been reviewed.
    fn all_gaps(&self) -> Result<Vec<GapCandidate>, DynoError> {
        let pop = self.population()?;
        let mut gaps = Vec::new();

        self.detect_phase_coverage(&pop, &mut gaps);
        self.detect_unsatisfied_requirements(&pop, &mut gaps)?;
        self.detect_unallocated_capabilities(&pop, &mut gaps)?;
        self.detect_unrealized_capabilities(&pop, &mut gaps)?;
        self.detect_unverified_capabilities(&pop, &mut gaps)?;
        self.detect_interface_pairing(&pop, &mut gaps)?;
        self.detect_unexpected_couplings(&mut gaps)?;
        self.detect_declining_dimensions(&mut gaps)?;
        self.detect_hierarchy_gaps(&mut gaps)?;

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

    // ---- Interface pairing (both sides of a contract) ----------------------

    /// Both `PROVIDES` and `CONSUMES` point *at* the Interface, so an unpaired
    /// contract is a missing incoming edge of one type.
    ///
    /// Identity here is the Interface node id, not a matched name string — so
    /// this cannot fire on a naming mismatch the way a text-keyed check would.
    fn detect_interface_pairing(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        if pop.interfaces == 0 {
            return Ok(());
        }
        for iface in self.scan_nodes(node::INTERFACE)? {
            let providers = self.incoming(&iface.node_id, Some(edge::PROVIDES))?;
            let consumers = self.incoming(&iface.node_id, Some(edge::CONSUMES))?;
            let name = node_name(&iface);

            if providers.is_empty() && !consumers.is_empty() {
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnprovidedInterface,
                        std::slice::from_ref(&iface.node_id),
                    ),
                    gap_source: GapSource::UnprovidedInterface,
                    scope: GapScope::Component,
                    severity: 0.72,
                    title: format!("Nothing supplies “{name}”, but {} part(s) rely on it", consumers.len()),
                    description: format!(
                        "{} part(s) expect “{name}” to be there, but no part of the design provides it — which one should?",
                        consumers.len()
                    ),
                    affected_ids: vec![iface.node_id.clone()],
                    suggested_depth: 3,
                    evidence: format!(
                        "Interface '{}' has 0 incoming PROVIDES and {} incoming CONSUMES.",
                        iface.node_id,
                        consumers.len()
                    ),
                });
            } else if consumers.is_empty() && !providers.is_empty() {
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnconsumedInterface,
                        std::slice::from_ref(&iface.node_id),
                    ),
                    gap_source: GapSource::UnconsumedInterface,
                    scope: GapScope::Component,
                    severity: 0.35,
                    title: format!("Nothing uses “{name}”"),
                    description: format!(
                        "“{name}” is offered but nothing in the design uses it — is it for outside users, or left over?"
                    ),
                    affected_ids: vec![iface.node_id.clone()],
                    suggested_depth: 2,
                    evidence: format!(
                        "Interface '{}' has {} incoming PROVIDES and 0 incoming CONSUMES.",
                        iface.node_id,
                        providers.len()
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
        // verify phase exists — a passing test for a capability does not prove
        // that *this file* is what delivers it. They are reported under
        // separate sources so each reads in its own terms: a capability is a
        // behaviour you confirm, an artifact is a thing you cover.
        for node_type in [node::CAPABILITY, node::ARTIFACT] {
            let source = if node_type == node::ARTIFACT {
                GapSource::UnverifiedArtifact
            } else {
                GapSource::UnverifiedCapability
            };
            for n in self.scan_nodes(node_type)? {
                if self.incoming(&n.node_id, Some(edge::VERIFIES))?.is_empty() {
                    let name = node_name(&n);
                    let (title, description) = if node_type == node::ARTIFACT {
                        (
                            format!("No verification covers “{name}”"),
                            format!(
                                "Nothing checks “{name}” itself. A capability it realizes may be \
                                 proven, which does not show that this file is what delivers it."
                            ),
                        )
                    } else {
                        (
                            format!("Nothing verifies “{name}”"),
                            format!(
                                "“{name}” has no verification proving it works — how will you \
                                 confirm it?"
                            ),
                        )
                    };
                    gaps.push(GapCandidate {
                        id: gap_id(source, std::slice::from_ref(&n.node_id)),
                        gap_source: source,
                        scope: GapScope::Capability,
                        severity: 0.55,
                        title,
                        description,
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

    /// Surface hidden couplings (from `surprising_connections`) as gaps: a
    /// coupling edge bridging two otherwise-distant communities is worth
    /// confirming ("is this link intentional, or should the boundary change?").
    fn detect_unexpected_couplings(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for c in self.surprising_connections()? {
            let affected = vec![c.from_id.clone(), c.to_id.clone()];
            gaps.push(GapCandidate {
                id: gap_id(GapSource::UnexpectedCoupling, &affected),
                gap_source: GapSource::UnexpectedCoupling,
                scope: GapScope::Capability,
                // surprise is ~1..3; map into a mid-band severity.
                severity: (c.surprise / 3.0).clamp(0.3, 0.85),
                title: format!(
                    "Unexpected coupling between '{}' and '{}'",
                    c.from_id, c.to_id
                ),
                description: format!(
                    "'{}' and '{}' sit in separate parts of the design yet are directly coupled — \
                     is that intentional, or should the boundary or the link change?",
                    c.from_id, c.to_id
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "{} edge bridges communities {}→{}; {} (surprise {:.2}).",
                    c.edge_type,
                    c.from_community,
                    c.to_community,
                    c.reasons.join(", "),
                    c.surprise
                ),
            });
        }
        Ok(())
    }

    /// Surface declining quality (from `dimension_drifts`) as gaps: a node whose
    /// score on some dimension is trending down over epochs.
    fn detect_declining_dimensions(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for d in self.dimension_drifts()? {
            if d.direction != DriftDirection::Declining {
                continue;
            }
            let dim = d.dimension.as_str();
            // Distinct per (node, dimension): fold the dimension into the id hash
            // while keeping affected_ids a clean node id.
            let id = gap_id(
                GapSource::DecliningDimension,
                &[d.target_id.clone(), dim.to_string()],
            );
            gaps.push(GapCandidate {
                id,
                gap_source: GapSource::DecliningDimension,
                scope: GapScope::Capability,
                severity: (0.4 + d.slope.abs()).clamp(0.4, 0.9),
                title: format!("{dim} of '{}' is declining", d.target_id),
                description: format!(
                    "The {dim} of '{}' has slipped from {:.2} to {:.2} over {} readings — \
                     worth reviewing before it erodes further.",
                    d.target_id, d.first_score, d.last_score, d.observation_count
                ),
                affected_ids: vec![d.target_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "{dim} drift slope {:.3} over {} observations (rollup {:.2}).",
                    d.slope, d.observation_count, d.rollup_score
                ),
            });
        }
        Ok(())
    }

    /// Surface axis-Y decomposition defects (from `hierarchy_issues`) as gaps:
    /// a missing intermediate level (carburetor-to-body), an inverted/flat
    /// containment, or a floating mid-level component.
    fn detect_hierarchy_gaps(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for issue in self.hierarchy_issues()? {
            let source = match issue.kind {
                HierarchyIssueKind::MissingIntermediateLevel => GapSource::MissingIntermediateLevel,
                HierarchyIssueKind::LevelMismatch => GapSource::LevelMismatch,
                HierarchyIssueKind::OrphanLevel => GapSource::OrphanLevel,
            };
            // Missing-intermediate is the highest-value Y defect; rank it up.
            let severity = match issue.kind {
                HierarchyIssueKind::MissingIntermediateLevel => 0.7,
                HierarchyIssueKind::LevelMismatch => 0.6,
                HierarchyIssueKind::OrphanLevel => 0.45,
            };
            let title = match issue.kind {
                HierarchyIssueKind::MissingIntermediateLevel => "Missing intermediate level",
                HierarchyIssueKind::LevelMismatch => "Decomposition level mismatch",
                HierarchyIssueKind::OrphanLevel => "Floating decomposition level",
            };
            gaps.push(GapCandidate {
                id: gap_id(source, &issue.components),
                gap_source: source,
                scope: GapScope::Component,
                severity,
                title: title.to_string(),
                description: issue.message.clone(),
                affected_ids: issue.components,
                suggested_depth: 2,
                evidence: issue.message,
            });
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
