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
//!   `declining_dimension` (quality trending down, from `dimension_drifts`).
//!   Cross-community coupling is deliberately *not* here: it is reported as a
//!   signal by `graph_report`, because a gap demands an answer and that one
//!   fires on correct architecture (see [`GapSource::UnexpectedCoupling`]).
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

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::DynoError;

use crate::dimensions::DriftDirection;
use crate::graph::DesignGraph;
use crate::hierarchy::HierarchyIssueKind;
use crate::llm::{LlmBackend, LlmRequest};
use crate::nodes::{edge, fnv1a, node};

/// What a gap is about (docs/gap-surfacing.md taxonomy). Adding a detector is
/// one variant + one branch, per storyflow's convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapSource {
    // Phase-coverage
    /// Capabilities/Components exist, but not one Requirement says why —
    /// the pure brownfield starting state (BL-27): structure recorded from
    /// code, intent never stated, so nothing can ever contradict anything.
    DesignWithoutIntent,
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
    /// A Capability `SATISFIES` no Requirement — something exists that nothing
    /// asked for.
    ///
    /// The mirror of [`GapSource::UnsatisfiedRequirement`], and the direction
    /// DETECT was blind in. Capabilities are normally created *from*
    /// requirements, so in greenfield an orphan is usually a half-finished
    /// thought. Reading a system backwards inverts that: the capability is the
    /// thing that indisputably exists, and one nothing justifies is either a
    /// missing requirement or dead code. Both are worth a question, and finding
    /// them is much of what an adoption exercise is *for*.
    UnmotivatedCapability,
    /// Two Components carry the same set of Capabilities — probably two
    /// implementations of one thing.
    ///
    /// Asked rather than repaired, and the distinction is load-bearing. HEAL's
    /// `duplicate` fires on a `DUPLICATES` edge, which is a human's assertion,
    /// and merge is safe *because* the endpoints were asserted. This is a
    /// heuristic over allocation sets, and merging on a heuristic would delete a
    /// component the machine merely suspects. So it asks — and if the user
    /// confirms by drawing the `DUPLICATES` edge, HEAL's existing merge takes it
    /// from there.
    PossibleDuplicate,
    /// A Capability is not `ALLOCATED_TO` any Component.
    UnallocatedCapability,
    /// A Capability has no `Artifact` `REALIZES`-ing it.
    UnrealizedCapability,
    /// A `Verification` whose status says the thing it checks does not work.
    ///
    /// The one gap where reality has directly contradicted the design, which is
    /// why it outranks everything that merely reports absence. Before it
    /// existed, a failing test *satisfied* `build_without_verification` — the
    /// gap asks "how will you confirm this works?" and was answered by a test
    /// proving it does not — and `detect_gaps`, `detect_defects` and
    /// `graph_report` were byte-identical between the passing and failing
    /// cases. The later phases counted test nodes and ignored test results,
    /// which is the reflow1 failure in miniature (BL-30, the phase-coverage
    /// trial's headline).
    FailingVerification,
    /// A recorded divergence whose second question was never answered: a
    /// `DriftEvent` with `resolved: false`. Reality moved, the movement was
    /// *observed and written down* — and then nobody said what it meant.
    /// Persistent on purpose: the session that reconciled may not be the
    /// session that answers, and before this gap the open question lived only
    /// in a tool result that scrolled away.
    UnresolvedDrift,
    /// A built Component that no Release includes — designed, built, and
    /// shipping in nothing.
    ///
    /// Gated twice: releases must exist, and at least one `INCLUDES` edge must
    /// exist somewhere — i.e. release contents are actually being modelled.
    /// Without the second gate every built component would fire the day the
    /// first Release node appeared, which is the mid-construction flood ophyd
    /// A14 warns about: a graph whose release manifest simply has not been
    /// entered yet is not a graph full of unshipped work.
    UnreleasedComponent,
    /// A lifecycle status making a claim the graph's own structure denies:
    /// a Capability `verified` that no passing check verifies, or a
    /// Requirement `met` that nothing satisfies.
    ///
    /// The design disagreeing with itself — distinct from absence (nothing
    /// recorded yet) and from reality-contradiction (a check failing). BL-27
    /// made these fields easy to write for the first time, which is exactly
    /// when unchecked claims start accumulating; and a `met` requirement is
    /// otherwise *invisible* — that status silences `unsatisfied_requirement`
    /// on purpose, so nothing else can catch it lying.
    StatusContradiction,
    /// A Capability has no `Verification` proving the behaviour works.
    ///
    /// The key string stays `unverified_capability` even though this variant
    /// once also covered Artifacts (see [`GapSource::UnverifiedArtifact`]).
    /// Gap ids hash that string, and an acknowledgement is stored under the
    /// resulting id — so changing it would silently expire every capability
    /// acknowledgement a user has made.
    UnverifiedCapability,
    /// Capabilities with no check of their own, riding a passing check on the
    /// component they are allocated to (BL-73). One gap per carrying
    /// component, at 0.35 — the question is "is component granularity enough
    /// for these?", asked once, instead of N per-capability alarms on a
    /// system whose suite genuinely passes. Neither `verified` nor
    /// unchecked: the third state, computed, never written
    /// (`dec:component-verified-computed`).
    ComponentGranularityVerification,
    /// **Retired as a gap.** Per-file verification coverage is counted by
    /// [`DesignGraph::verification_coverage`] and reported by `graph_report`.
    ///
    /// The reasoning for flagging it was sound — proving a capability works
    /// does not prove *this file* is what delivers it — and the demand was
    /// still wrong: one `VERIFIES` edge per source file is bookkeeping nobody
    /// writes. Modelling reflow2's own design made it 22 of 25 gaps, on a crate
    /// whose capabilities are all tested. A list that cannot reach zero teaches
    /// you to skim it, which is the failure this layer exists to prevent.
    ///
    /// Kept, like [`GapSource::UnexpectedCoupling`], because acknowledgement
    /// ids hash the key string.
    UnverifiedArtifact,
    // Interface pairing (the two sides of a contract)
    /// An `Interface` something `CONSUMES` that no Component `PROVIDES` — a
    /// break between two parts of the design.
    UnprovidedInterface,
    /// An `Interface` a Component `PROVIDES` that nothing `CONSUMES` — either a
    /// deliberate public contract or a leftover.
    UnconsumedInterface,
    // Graph-analysis (from the design network)
    /// **Retired as a gap.** A coupling edge bridging two otherwise-distant
    /// communities is a *signal*, not a question: `graph_report` lists it under
    /// "Surprising couplings" and `surprising_connections` returns it whole.
    ///
    /// It was never in the gap taxonomy — docs/gap-surfacing.md names
    /// `orphan_node`, `dead_end`, `disconnected_cluster` and
    /// `single_point_of_failure`, not this — and demanding an answer for it went
    /// badly twice. Both blind trials reported the same thing: it fires on
    /// correct architecture. An `Interface` joins two clusters *by
    /// construction*, so modelling contracts as the docs instruct made the
    /// detector penalise every one. Ten of thirteen gaps in one trial were this;
    /// the other put it plainly — *"that coupling **is** the product"*.
    ///
    /// The variant and its key string stay because acknowledgement ids hash
    /// them: removing them would strand every review someone has already made
    /// (see [`DesignGraph::reviewed_gaps`], which reports those as retired).
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
    // Decision points (axis of design space — BL-70)
    /// A *proposed* Decision holding ≥2 registered alternatives — an open fork
    /// the design has not chosen between. The "missing teeth" BL-70 named:
    /// nothing else makes a proposed Decision gate anything, so a held-open
    /// analysis of alternatives would sit undecided forever without a nudge.
    /// Compare them (`analyze_alternatives`), then `collapse_decision`.
    UndecidedDecisionPoint,
    // Verification vs validation (BL — edge-orthogonality)
    /// Capabilities with a passing verification-kind check but no passing
    /// validation-kind check — built to spec, but nothing confirms they meet the
    /// operational intent ("built right" without "the right thing"). The reader
    /// that earns `Verification.kind` its keep after the `VALIDATES` edge was
    /// retired (`dec:edge-orthogonality`). One project-level rollup, not N
    /// per-capability alarms (the BL-73 lesson).
    UnvalidatedCapability,
}

impl GapSource {
    /// Stable snake_case key (used in the gap id hash and for display).
    pub fn as_str(self) -> &'static str {
        match self {
            GapSource::DesignWithoutIntent => "design_without_intent",
            GapSource::ConceptWithoutDesign => "concept_without_design",
            GapSource::DesignWithoutBuild => "design_without_build",
            GapSource::BuildWithoutVerification => "build_without_verification",
            GapSource::NoDeployOperate => "no_deploy_operate",
            GapSource::UnsatisfiedRequirement => "unsatisfied_requirement",
            GapSource::UnmotivatedCapability => "unmotivated_capability",
            GapSource::PossibleDuplicate => "possible_duplicate",
            GapSource::UnallocatedCapability => "unallocated_capability",
            GapSource::UnrealizedCapability => "unrealized_capability",
            // Load-bearing: this string is hashed into the gap id, which keys
            // the acknowledgement Decision. Renaming it expires every existing
            // capability acknowledgement with nothing to tell the user why.
            GapSource::FailingVerification => "failing_verification",
            GapSource::UnresolvedDrift => "unresolved_drift",
            GapSource::UnreleasedComponent => "unreleased_component",
            // Must match the serde snake_case of the variant: clients match on
            // the serialized name, and gap ids hash this string.
            GapSource::StatusContradiction => "status_contradiction",
            GapSource::UnverifiedCapability => "unverified_capability",
            GapSource::ComponentGranularityVerification => "component_granularity_verification",
            GapSource::UnverifiedArtifact => "unverified_artifact",
            GapSource::UnprovidedInterface => "unprovided_interface",
            GapSource::UnconsumedInterface => "unconsumed_interface",
            GapSource::UnexpectedCoupling => "unexpected_coupling",
            GapSource::DecliningDimension => "declining_dimension",
            GapSource::MissingIntermediateLevel => "missing_intermediate_level",
            GapSource::LevelMismatch => "level_mismatch",
            GapSource::OrphanLevel => "orphan_level",
            GapSource::UndecidedDecisionPoint => "undecided_decision_point",
            GapSource::UnvalidatedCapability => "unvalidated_capability",
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
    /// The gap itself, exactly as the detector reports it — absent when the
    /// detector that raised it has since been retired (see `retired`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<GapCandidate>,
    /// Why it was accepted.
    pub reason: String,
    /// The `Decision` node recording the review.
    pub decision_id: String,
    /// The gap id this review was made against. Always present, including when
    /// no live detector produces it any more.
    pub gap_id: String,
    /// Set when the review outlived its detector: the judgement was real and is
    /// kept, but nothing raises that gap now, so there is no candidate to show.
    ///
    /// Reported rather than dropped. Silently omitting these would shrink the
    /// reviewed list for a reason the user cannot see — the same dishonesty
    /// this type exists to avoid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired: Option<String>,
}

/// The optional detail carried alongside a question when it is recorded.
///
/// A struct rather than five positional arguments, because all of it is
/// optional and a call site with five bare `None`s says nothing.
#[derive(Debug, Clone, Default)]
pub struct AskedQuestion<'a> {
    /// Id of the LLM request that phrased it, so the same phrasing is
    /// recognisable across sessions ([`crate::prompt_id`]).
    pub prompt_id: Option<&'a str>,
    /// The 1-2 sentences that placed the user back in their own design.
    pub context_setter: Option<&'a str>,
    /// When it was put to the user.
    pub asked_at: Option<&'a str>,
    /// True when phrasing fell back to the raw gap text. Recorded rather than
    /// hidden: the question was still asked, and this says how well.
    pub rephrase_degraded: bool,
}

/// A question already put to the user, as a later session finds it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AskedRecord {
    pub question_id: String,
    /// The gap it was asked about. Re-derivable, so it survives a restart.
    pub gap_id: String,
    /// The wording the user actually saw.
    pub question: String,
    pub context_setter: String,
    pub asked_at: String,
    pub rephrase_degraded: bool,
    /// `asked` (still waiting) or `answered` (they replied, and the gap is
    /// still open — so their answer has not been written into the design, or
    /// the gap needs acknowledging).
    pub status: String,
    /// What they said, when `status` is `answered`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub answer: String,
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
/// The `Question` id recording that `gap_id` was put to the user. Derived from
/// the gap id for the same reason as [`ack_decision_id`]: a later session finds
/// it without an index, and a gap whose affected set changes gets a different
/// id — so a question about a situation that has moved on does not suppress the
/// fresh one.
fn asked_question_id(gap_id: &str) -> String {
    format!("question:{}", gap_id.strip_prefix("gap:").unwrap_or(gap_id))
}

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
    flows: usize,
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
            flows: self.count_nodes(node::FLOW)?,
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
            // A repeat acknowledgement re-creates the same edge harmlessly
            // (create_edge upserts on (graph, type, from, to)), so there is no
            // benign error to swallow here — a failure is a real storage/schema
            // fault and must surface, not leave the Decision unlinked from what
            // it governs (BL-58).
            self.governed_by(&node_type, target, node::DECISION, &decision_id)?;
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

    /// Record that a gap was actually put to the user, and in what words.
    ///
    /// `gap_to_prompt` phrases a question and returns it; until now nothing
    /// kept it. The next session re-derived the same gap, re-phrased it, and
    /// asked again — *"the stateless-agent problem reflow2 is supposed to
    /// solve"*, in the blind trial's words. It worked around this by copying
    /// questions into a Markdown file by hand.
    ///
    /// Stored as a real `Question` node at a derived id, `ASKS_ABOUT` the nodes
    /// the gap concerned, so it is reachable from the design and not only from
    /// the gap. Idempotent: asking again updates the wording rather than
    /// stacking duplicates.
    ///
    /// This records that a question was *asked*, not that it was answered —
    /// see [`answer_question`](Self::answer_question).
    pub fn record_asked_question(
        &mut self,
        gap_id: &str,
        affected_ids: &[String],
        question: &str,
        opts: AskedQuestion<'_>,
    ) -> Result<String, DynoError> {
        let question_id = asked_question_id(gap_id);
        // Asking again must not erase an answer already given.
        let existing = self.get_node(node::QUESTION, &question_id)?;
        let mut props = crate::nodes::Props::new()
            .set("question", question)
            .set("gap_id", gap_id)
            .set("rephrase_degraded", opts.rephrase_degraded)
            .set_opt("prompt_id", opts.prompt_id)
            .set_opt("context_setter", opts.context_setter)
            .set_opt("asked_at", opts.asked_at);
        props = match existing.as_ref().and_then(|n| n.properties.get("status")) {
            Some(v) if v.as_str() == Some("answered") => {
                let answer = existing
                    .as_ref()
                    .and_then(|n| n.properties.get("answer"))
                    .and_then(dynograph_core::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                props.set("status", "answered").set("answer", answer)
            }
            _ => props.set("status", "asked"),
        };
        self.create_node(node::QUESTION, &question_id, props)?;

        for target in affected_ids {
            let Some(node_type) = self.node_type_index()?.get(target).cloned() else {
                continue; // the gap outlived the node — nothing to attach to
            };
            // Upsert, so a re-asked question re-draws the same edge harmlessly;
            // a real failure must surface rather than leave the Question
            // unlinked from what it asks about (BL-58).
            self.create_edge(
                edge::ASKS_ABOUT,
                node::QUESTION,
                &question_id,
                &node_type,
                target,
                crate::nodes::Props::new(),
            )?;
        }
        Ok(question_id)
    }

    /// Record what the user said, closing an asked question.
    ///
    /// The answer text is kept verbatim. Whatever design nodes it produces are
    /// written separately by the caller — this is the record that the question
    /// was settled and by what, not a substitute for the design itself.
    pub fn answer_question(&mut self, gap_id: &str, answer: &str) -> Result<bool, DynoError> {
        self.set_question_status(gap_id, "answered", Some(answer))
    }

    /// Withdraw a question — asked in error, or overtaken by events. Kept, not
    /// deleted: the past is not overwritten.
    pub fn withdraw_question(&mut self, gap_id: &str) -> Result<bool, DynoError> {
        self.set_question_status(gap_id, "withdrawn", None)
    }

    fn set_question_status(
        &mut self,
        gap_id: &str,
        status: &str,
        answer: Option<&str>,
    ) -> Result<bool, DynoError> {
        let question_id = asked_question_id(gap_id);
        let Some(existing) = self.get_node(node::QUESTION, &question_id)? else {
            return Ok(false);
        };
        let mut props = crate::nodes::Props::new()
            .set("status", status)
            .set_opt("answer", answer);
        for (k, v) in &existing.properties {
            if k != "status" && !(k == "answer" && answer.is_some()) {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::QUESTION, &question_id, props)?;
        Ok(true)
    }

    /// Questions already put to the user that still bear on something open.
    ///
    /// Two kinds, distinguished by `status`:
    ///
    /// - `asked` — they have not replied yet. Follow it up; do not ask again.
    /// - `answered` — they replied, **and the gap is still open**. Their answer
    ///   has not been written into the design, or the gap needs acknowledging.
    ///
    /// The second kind exists because of what the self-host probe found
    /// immediately after questions became persistent: answer a question in a way
    /// that does not change the design — *"it is a library you build from
    /// source; no deploy layer is intended"* — and the gap stays open while the
    /// question goes quiet. A later session then saw a bare open gap with no
    /// sign it had ever been asked, and asked again. That is the same failure
    /// this whole item exists to prevent, displaced by one step.
    ///
    /// A question whose gap has since closed or been acknowledged is not
    /// returned: there is nothing left to act on. It stays in the graph.
    ///
    /// Sorted by id, so the order is stable across sessions.
    pub fn open_questions(&self) -> Result<Vec<AskedRecord>, DynoError> {
        let still_open: std::collections::HashSet<String> =
            self.detect_gaps()?.into_iter().map(|g| g.id).collect();

        let mut out = Vec::new();
        for n in self.scan_nodes(node::QUESTION)? {
            let get = |k: &str| {
                n.properties
                    .get(k)
                    .and_then(dynograph_core::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let status = get("status");
            let gap_id = get("gap_id");
            let live = match status.as_str() {
                "asked" => true,
                // Answered, but the thing it was about is still outstanding.
                "answered" => still_open.contains(&gap_id),
                // withdrawn, or anything a later version adds
                _ => false,
            };
            if !live {
                continue;
            }
            out.push(AskedRecord {
                question_id: n.node_id.clone(),
                gap_id,
                question: get("question"),
                context_setter: get("context_setter"),
                asked_at: get("asked_at"),
                rephrase_degraded: n
                    .properties
                    .get("rephrase_degraded")
                    .and_then(dynograph_core::Value::as_bool)
                    .unwrap_or(false),
                answer: get("answer"),
                status,
            });
        }
        out.sort_by(|a, b| a.question_id.cmp(&b.question_id));
        Ok(out)
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
        let mut live = std::collections::HashSet::new();
        for gap in self.all_gaps()? {
            if let Some((decision_id, reason)) = self.gap_acknowledgement(&gap.id)? {
                live.insert(gap.id.clone());
                reviewed.push(ReviewedGap {
                    gap_id: gap.id.clone(),
                    gap: Some(gap),
                    reason,
                    decision_id,
                    retired: None,
                });
            }
        }

        // Acknowledgements whose detector no longer exists. `unexpected_coupling`
        // was retired as a gap, and at least one trial had already accepted one —
        // that judgement is real and stays visible, rather than vanishing because
        // the code changed underneath it.
        for d in self.scan_nodes(node::DECISION)? {
            let Some(hash) = d.node_id.strip_prefix("decision:ack:") else {
                continue;
            };
            let gap_id = format!("gap:{hash}");
            if live.contains(&gap_id) {
                continue;
            }
            let Some((decision_id, reason)) = self.gap_acknowledgement(&gap_id)? else {
                continue;
            };
            reviewed.push(ReviewedGap {
                gap: None,
                reason,
                decision_id,
                gap_id,
                retired: Some(
                    "No current detector raises this gap. The decision is kept; nothing is \
                     being suppressed by it."
                        .to_string(),
                ),
            });
        }

        reviewed.sort_by(|a, b| a.gap_id.cmp(&b.gap_id));
        Ok(reviewed)
    }

    /// Run all deterministic detectors and return gap candidates ranked
    /// **anchored gaps first, then most-severe** (ties broken by id for a stable
    /// order), regardless of whether they have been reviewed.
    ///
    /// # Why anchoring outranks severity
    ///
    /// [`gap-surfacing.md`] names two modes: *retroactive* (gap-driven — "fix
    /// what's thin") and *proactive* ("you're at the design stage; here's what
    /// comes next"), and puts the phase-coverage nudges in the proactive one. A
    /// gap that names nodes is a statement about something wrong **now**; a
    /// phase nudge is a statement about what comes **next**. Ranking "next"
    /// above "broken" is what an agent working the list top-down pays for.
    ///
    /// Ordering on severity alone did exactly that, because the two kinds are
    /// not on a comparable scale. `concept_without_design` is the literal 0.70;
    /// `unsatisfied_requirement` is computed as `0.5 + priority_bump`, which for
    /// the default `medium` priority is 0.60 — and until BL-28 no client on one
    /// major harness could write `priority` at all, so the losing number was a
    /// default nobody chose. Three brownfield trials reported the consequence
    /// independently at a 20× size difference: the top gap was an artifact of
    /// seeding order, and the actionable one sat below it.
    ///
    /// This deliberately does **not** suppress the phase detectors, which would
    /// break the case the [aidrone trial] recorded as working: GENESIS seeds
    /// P0/P1 and stops, `concept_without_design` fires, "the skill and the
    /// detector agree, the gap arrives as a question rather than a complaint."
    /// On a graph with nothing anchored yet it is still the first thing the user
    /// sees. It only yields once there is something specific to say.
    ///
    /// [`gap-surfacing.md`]: https://github.com/sligara7/reflow2/blob/main/docs/gap-surfacing.md
    /// [aidrone trial]: https://github.com/sligara7/reflow2/blob/main/docs/trials/2026-07-18-greenfield-aidrone.md
    fn all_gaps(&self) -> Result<Vec<GapCandidate>, DynoError> {
        let pop = self.population()?;
        let mut gaps = Vec::new();

        self.detect_phase_coverage(&pop, &mut gaps);
        self.detect_unsatisfied_requirements(&pop, &mut gaps)?;
        self.detect_unmotivated_capabilities(&pop, &mut gaps)?;
        self.detect_possible_duplicates(&pop, &mut gaps)?;
        self.detect_unallocated_capabilities(&pop, &mut gaps)?;
        self.detect_unrealized_capabilities(&pop, &mut gaps)?;
        self.detect_unverified_capabilities(&pop, &mut gaps)?;
        self.detect_failing_verifications(&mut gaps)?;
        self.detect_unresolved_drift(&mut gaps)?;
        self.detect_unreleased_components(&mut gaps)?;
        self.detect_status_contradictions(&mut gaps)?;
        self.detect_interface_pairing(&pop, &mut gaps)?;
        // Deliberately absent: unexpected coupling. It is a *signal*, reported
        // by `graph_report` and `surprising_connections`, not a gap demanding
        // an answer — see `GapSource::UnexpectedCoupling`.
        self.detect_declining_dimensions(&mut gaps)?;
        self.detect_hierarchy_gaps(&mut gaps)?;
        self.detect_undecided_decision_points(&mut gaps)?;
        self.detect_unvalidated_capabilities(&mut gaps)?;

        gaps.sort_by(|a, b| {
            // `false` sorts before `true`, so "has anchors" comes first.
            a.affected_ids
                .is_empty()
                .cmp(&b.affected_ids.is_empty())
                .then(
                    b.severity
                        .partial_cmp(&a.severity)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
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

        // The pure brownfield starting state (BL-27, ophyd finding 1): a graph
        // seeded from what exists holds capabilities and components and not
        // one requirement — and before this fired nothing at all, because
        // `unmotivated_capability` is gated on requirements > 0 to avoid one
        // gap per capability. One project-level nudge, not N: the first gap on
        // an adopted system should be about missing intent, not missing
        // structure. Requirements must come from OUTSIDE the implementation
        // (a requirement inferred from the code it describes is satisfied by
        // construction), which is what the wording asks for.
        if pop.capabilities + pop.components > 0 && pop.requirements == 0 {
            push(
                gaps,
                GapSource::DesignWithoutIntent,
                0.72,
                3,
                "Structure recorded, but no stated intent",
                "The graph knows what this system has and does, but not one requirement says \
                 what it is for or what must be true of it. Record intent from sources outside \
                 the implementation — what people asked for, READMEs, tests, issues, configs — \
                 so the structure has something to be checked against.",
                format!(
                    "{} capability(ies) + {} component(s) exist; 0 Requirements.",
                    pop.capabilities, pop.components
                ),
            );
        }
        // A Flow counts as structure: a *process* design's WHERE is the flow
        // its capabilities form, and it never grows Components at all. Without
        // this, modelling reflow2's own coherence loop raised "no structure
        // yet" over a fully-structured process — the phase detectors assuming
        // every subject is a product (BL-37; the wider question is BL-16).
        if pop.requirements + pop.capabilities > 0 && pop.components == 0 && pop.flows == 0 {
            push(
                gaps,
                GapSource::ConceptWithoutDesign,
                0.70,
                3,
                "Concept defined, but no structure yet",
                "You've defined what it does, but nothing about how it's structured into buildable parts.",
                format!(
                    "{} requirement(s) + {} capability(ies) exist; 0 Components, 0 Flows.",
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

    /// The mirror of [`Self::detect_unsatisfied_requirements`]: a Capability
    /// that satisfies no Requirement.
    ///
    /// # Why severity reads `provenance`
    ///
    /// The ophyd trial asked for this to outrank `unsatisfied_requirement`
    /// *"on a brownfield graph"* — and a fixed number cannot honour that
    /// qualifier, because the same structure means different things on the two
    /// paths. An `authored` capability nothing asked for is a half-finished
    /// thought, worth mentioning after the requirement gaps. An `inferred` one
    /// is a feature **in production** that no stated requirement justifies —
    /// either a requirement nobody wrote down or dead code, and the single
    /// highest-value thing an adoption pass can surface.
    ///
    /// `provenance` is what tells those apart, so the bump keys on it: 0.55
    /// normally, 0.70 when inferred, which clears `unsatisfied_requirement`'s
    /// 0.60 default exactly on the graph where the trial wanted it to and
    /// nowhere else.
    fn detect_unmotivated_capabilities(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        // Only meaningful once requirements exist to be motivated *by*. A graph
        // with capabilities and no requirements at all is a different situation
        // — intent has not been captured yet — and reporting it once per
        // capability would be the per-node flood this layer exists to avoid.
        // Nothing currently reports that project-level case; recorded in BL-27.
        if pop.requirements == 0 {
            return Ok(());
        }
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if self
                .outgoing(&cap.node_id, Some(edge::SATISFIES))?
                .is_empty()
            {
                let name = node_name(&cap);
                let inferred = cap
                    .properties
                    .get("provenance")
                    .and_then(dynograph_core::Value::as_str)
                    == Some("inferred");
                gaps.push(GapCandidate {
                    id: gap_id(
                        GapSource::UnmotivatedCapability,
                        std::slice::from_ref(&cap.node_id),
                    ),
                    gap_source: GapSource::UnmotivatedCapability,
                    scope: GapScope::Capability,
                    severity: if inferred { 0.70 } else { 0.55 },
                    title: format!("Nothing asked for capability “{name}”"),
                    description: if inferred {
                        format!(
                            "“{name}” is built and running, but no requirement justifies it — is there a need nobody wrote down, or is this dead code?"
                        )
                    } else {
                        format!(
                            "“{name}” satisfies no requirement — what need does it serve, or should it go?"
                        )
                    },
                    affected_ids: vec![cap.node_id.clone()],
                    suggested_depth: 2,
                    evidence: format!(
                        "Capability '{}' (provenance={}) has 0 outgoing SATISFIES; project has {} requirement(s).",
                        cap.node_id,
                        if inferred { "inferred" } else { "authored" },
                        pop.requirements
                    ),
                });
            }
        }
        Ok(())
    }

    /// Two Components allocated the same (or nearly the same) Capabilities.
    ///
    /// # Why this is computed here and not in HEAL
    ///
    /// HEAL already has a `duplicate` category, and it fires on a `DUPLICATES`
    /// edge — which means it reports a conclusion somebody already reached and
    /// recorded. It computes nothing, so it cannot fire on a duplicate nobody
    /// has found yet, which is every duplicate an adoption pass exists to
    /// discover. 3dtictactoe modelled two components holding an identical set of
    /// three capabilities, one of them dead code, and `detect_defects` returned
    /// eight defects with no `duplicate` among them. That is
    /// [gap-surfacing.md]'s first discipline exactly — *detectors read computed
    /// signals, not raw edge-name filters* — the trap it says was storyflow's
    /// biggest.
    ///
    /// The computed half lands in DETECT rather than HEAL for three reasons:
    ///
    /// 1. **Merge is only safe because the endpoints were asserted.** HEAL maps
    ///    `duplicate` straight to an applicable [`HealOp::Merge`], which
    ///    `apply_heal` executes — it deletes a node and re-points its edges.
    ///    Feeding a heuristic into that path would let the machine delete a
    ///    component it merely suspects is redundant.
    /// 2. **A HEAL issue cannot be dismissed.** Gaps can be acknowledged and drop
    ///    out of the open list; defects cannot. Any structural heuristic has
    ///    false positives — two components legitimately sharing a capability set
    ///    is a real design — and [`GapSource::UnexpectedCoupling`] is the
    ///    cautionary tale of a detector that fired on correct architecture with
    ///    no way to make it stop.
    /// 3. **"Are these the same thing?" is meaning, not structure**, which is the
    ///    division the docs draw: HEAL fills structure, gap-surfacing elicits
    ///    meaning.
    ///
    /// So the two compose rather than duplicate: this asks, the user confirms by
    /// drawing the `DUPLICATES` edge, and HEAL's existing merge — whose "endpoints
    /// known" precondition now genuinely holds — repairs it. A pair already
    /// joined by that edge is skipped here, so nothing is reported twice.
    ///
    /// # The rule, and why it is this one
    ///
    /// [heal-process.md] plans duplicate detection on dynograph's
    /// `resolution: fuzzy_then_vector` — semantic similarity over names and
    /// descriptions. That needs the `EmbeddingBackend`, a deliberate deferral, and
    /// it would find a different population: things *described* alike. The
    /// structural rule needs nothing deferred and finds things *wired* alike,
    /// which is what the trial actually hit. They are complements, not rivals;
    /// this is the deterministic half.
    ///
    /// Two guards against the obvious false positive. A pair must share **at
    /// least two** capabilities, because two components both providing the one
    /// capability they have in common is ordinary design, not redundancy; and
    /// their sets must be at least 80% alike by Jaccard overlap, so a large
    /// component that happens to contain a small one's whole set is not accused.
    ///
    /// Scoped to Components on purpose. Two Capabilities satisfying the same
    /// Requirement is *decomposition* — the normal case, and a rule there would
    /// fire on almost every correct design. Duplicate capabilities need the
    /// semantic path.
    ///
    /// [gap-surfacing.md]: https://github.com/sligara7/reflow2/blob/main/docs/gap-surfacing.md
    /// [heal-process.md]: https://github.com/sligara7/reflow2/blob/main/docs/heal-process.md
    /// [`HealOp::Merge`]: crate::heal::HealOp::Merge
    fn detect_possible_duplicates(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        /// Below this many shared capabilities, an overlap is ordinary design.
        const MIN_SHARED: usize = 2;
        /// Jaccard overlap below which two sets are merely related, not alike.
        const MIN_JACCARD: f64 = 0.8;

        if pop.components < 2 {
            return Ok(());
        }

        // component id -> (display name, capabilities allocated to it). Sorted
        // throughout so the pair walk below is deterministic. ALLOCATED_TO runs
        // Capability -> Component, so the component is the `to` side.
        let mut by_component: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
        for cmp in self.scan_nodes(node::COMPONENT)? {
            let caps: BTreeSet<String> = self
                .incoming(&cmp.node_id, Some(edge::ALLOCATED_TO))?
                .into_iter()
                .map(|e| e.from_id)
                .collect();
            by_component.insert(cmp.node_id.clone(), (node_name(&cmp), caps));
        }

        // Pairs the user has already called duplicates belong to HEAL, which can
        // actually repair them. Reporting them here as a question too would be
        // the DETECT/HEAL double-count the trials have complained about.
        let mut already_known: BTreeSet<(String, String)> = BTreeSet::new();
        for id in by_component.keys() {
            for e in self.outgoing(id, Some(edge::DUPLICATES))? {
                already_known.insert(ordered_pair(&e.from_id, &e.to_id));
            }
            for e in self.incoming(id, Some(edge::DUPLICATES))? {
                already_known.insert(ordered_pair(&e.from_id, &e.to_id));
            }
        }

        let components: Vec<(&String, &(String, BTreeSet<String>))> = by_component.iter().collect();
        for (i, (a_id, (a_name, a_caps))) in components.iter().enumerate() {
            for (b_id, (b_name, b_caps)) in components.iter().skip(i + 1) {
                let shared = a_caps.intersection(b_caps).count();
                if shared < MIN_SHARED {
                    continue;
                }
                let union = a_caps.union(b_caps).count();
                #[allow(clippy::cast_precision_loss)]
                let jaccard = shared as f64 / union as f64;
                if jaccard < MIN_JACCARD {
                    continue;
                }
                let pair = ordered_pair(a_id, b_id);
                if already_known.contains(&pair) {
                    continue;
                }
                let (keep, other) = pair;

                let identical = a_caps == b_caps;
                let affected = vec![keep.clone(), other.clone()];
                gaps.push(GapCandidate {
                    id: gap_id(GapSource::PossibleDuplicate, &affected),
                    gap_source: GapSource::PossibleDuplicate,
                    scope: GapScope::Component,
                    // An identical set is the strong signal the trial hit; a
                    // near-identical one is worth asking about but should not
                    // outrank a requirement nothing satisfies.
                    severity: if identical { 0.70 } else { 0.58 },
                    title: format!("“{a_name}” and “{b_name}” may be the same thing"),
                    description: format!(
                        "“{a_name}” and “{b_name}” carry {} the same capabilities — are these two implementations of one thing, or genuinely separate?",
                        if identical { "exactly" } else { "nearly" }
                    ),
                    affected_ids: affected,
                    suggested_depth: 2,
                    evidence: format!(
                        "Components '{keep}' and '{other}' share {shared} of {union} allocated capabilities (Jaccard {jaccard:.2}); no DUPLICATES edge joins them."
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
        // A Flow is structure too (BL-37), so a process design can ask this
        // question without ever growing a Component. Before BL-42 removed
        // HEAL's duplicate orphan check, a loose capability on a flow-only
        // graph was covered there; now this is the only place that asks, so
        // the gate has to admit it.
        if pop.components == 0 && pop.flows == 0 {
            return Ok(());
        }
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if self
                .outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))?
                .is_empty()
                // A step of a process is owned by its Flow — that IS its home.
                && self
                    .outgoing(&cap.node_id, Some(edge::PART_OF_FLOW))?
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
                        "Capability '{}' has 0 outgoing ALLOCATED_TO and 0 PART_OF_FLOW; project has {} component(s) and {} flow(s).",
                        cap.node_id, pop.components, pop.flows
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

    /// A Capability nothing builds — where "builds" accepts **both** shapes the
    /// schema allows at P3.
    ///
    /// `REALIZES` is declared `from: Artifact, to: "*"`, and `link_artifact`
    /// takes any `target_type`, so a modeller can honestly say either *this
    /// file realizes the capability* or *this file realizes the module* — the
    /// second being how code is actually organised. This detector used to
    /// accept only the first, which silently mandated one of two equally valid
    /// modellings and flooded anyone who picked the other: 11 of 33 gaps on
    /// reflow2's own design were "Nothing builds capability X" for capabilities
    /// shipping in the binary that reported them.
    ///
    /// So a capability now also counts as realized when an artifact realizes a
    /// **Component it is allocated to**: the path
    /// `art -REALIZES-> cmp <-ALLOCATED_TO- cap` was present in every false
    /// positive and simply not walked. The indirect form is the coarser claim —
    /// the file builds the part that owns the capability, not the capability
    /// itself — which is exactly the granularity BL-23 pushes designs toward
    /// (one artifact per module, never per behaviour).
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
                && !self.realized_via_component(&cap.node_id)?
                // …and nobody has already *asserted* that the owning component
                // is built. This is the BL-42 fix, and the signal is a claim
                // the modeller made rather than a guess from topology.
                //
                // "What gets built for this?" is a real forward-looking
                // question while a component is `planned` or `in_progress`.
                // Once a component is marked `realized`, the modeller has said
                // *this already exists* — an absent artifact then describes
                // the coverage of the artifact layer, not a hole in the
                // design. The storyflow adopt trial made that distinction
                // expensive: 13 of 51 gaps, every one produced by following
                // the adopt skill's own instruction to model artifacts
                // coarsely over a system that is entirely built.
                //
                // Same bargain as BL-23: the question is dropped and the
                // number is kept (`realization` in `graph_report`), so
                // per-file rigour is still visible to anyone who wants it. No
                // threshold and no proportion — BL-5's lesson was that a loud
                // detector needs a different *question*, not a tuned number.
                && !self.owner_claims_built(&cap.node_id)?
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
                        "Capability '{}' has 0 incoming REALIZES, and no artifact realizes any component it is allocated to; project has {} artifact(s).",
                        cap.node_id, pop.artifacts
                    ),
                });
            }
        }
        Ok(())
    }

    /// Has the modeller already asserted that this capability's owning
    /// component exists — `status: realized` or `verified`?
    ///
    /// An unallocated capability is `false`: there is no owner to have made
    /// the claim, and `unallocated_capability` asks the prior question anyway.
    pub(crate) fn owner_claims_built(&self, cap_id: &str) -> Result<bool, DynoError> {
        for alloc in self.outgoing(cap_id, Some(edge::ALLOCATED_TO))? {
            let claimed = self
                .get_node(node::COMPONENT, &alloc.to_id)?
                .and_then(|c| {
                    c.properties
                        .get("status")
                        .and_then(dynograph_core::Value::as_str)
                        .map(|s| s == "realized" || s == "verified")
                })
                .unwrap_or(false);
            if claimed {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Is this capability built, in either P3 shape (BL-38)?
    pub(crate) fn capability_is_realized(&self, cap_id: &str) -> Result<bool, DynoError> {
        Ok(!self.incoming(cap_id, Some(edge::REALIZES))?.is_empty()
            || self.realized_via_component(cap_id)?)
    }

    /// Does any artifact realize a Component this capability is allocated to?
    fn realized_via_component(&self, cap_id: &str) -> Result<bool, DynoError> {
        for alloc in self.outgoing(cap_id, Some(edge::ALLOCATED_TO))? {
            if !self
                .incoming(&alloc.to_id, Some(edge::REALIZES))?
                .is_empty()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A `Verification` whose recorded status is `failing` — reality has
    /// contradicted the design, which no absence-shaped gap can say.
    ///
    /// No population gate on purpose: a failing check is worth surfacing even
    /// if it is the only Verification in the graph — *especially* then, since
    /// its mere existence is what closes `build_without_verification`. That
    /// closure is correct (the "how will you confirm this?" question *is*
    /// answered) but it used to be the end of the story, leaving the failure
    /// invisible everywhere. Now the silence is filled with the right signal
    /// instead of the phase nudge staying open.
    ///
    /// Severity 0.8, above every absence gap: a requirement nothing satisfies
    /// is work not started, but a failing verification is work *proven broken*,
    /// and an agent working the list top-down should see it first.
    fn detect_failing_verifications(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let index = self.node_type_index()?;
        for ver in self.scan_nodes(node::VERIFICATION)? {
            let status = ver
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("planned");
            if status != "failing" {
                continue;
            }
            let name = node_name(&ver);
            // Anchor the gap to what the check was checking, not only the
            // check itself — the person answering needs to know what is broken.
            let mut affected = vec![ver.node_id.clone()];
            let mut target_names = Vec::new();
            for e in self.outgoing(&ver.node_id, Some(edge::VERIFIES))? {
                if let Some(t) = index.get(&e.to_id)
                    && let Some(n) = self.get_node(t, &e.to_id)?
                {
                    target_names.push(node_name(&n));
                }
                affected.push(e.to_id);
            }
            affected.sort();
            let what = if target_names.is_empty() {
                "what it checks".to_string()
            } else {
                target_names.sort();
                format!("“{}”", target_names.join("”, “"))
            };
            gaps.push(GapCandidate {
                id: gap_id(GapSource::FailingVerification, &affected),
                gap_source: GapSource::FailingVerification,
                scope: GapScope::Capability,
                severity: 0.8,
                title: format!("“{name}” is failing"),
                description: format!(
                    "The check “{name}” is failing, so {what} is proven not to work as designed — fix the build, or the design no longer describes reality."
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "Verification '{}' has status=failing; a failing check is reality contradicting the design, not absence of a check.",
                    ver.node_id
                ),
            });
        }
        Ok(())
    }

    /// A `DriftEvent` still marked `resolved: false` — observed divergence
    /// with no recorded answer. Severity 0.75: reality-contradiction family
    /// (just below a failing check at 0.8, above every absence gap). Clears
    /// when `set_artifact_checksum` accepts the artifact, which resolves its
    /// open events (BL-33/BL-35).
    fn detect_unresolved_drift(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for ev in self.scan_nodes(node::DRIFT_EVENT)? {
            let resolved = ev
                .properties
                .get("resolved")
                .and_then(dynograph_core::Value::as_bool)
                .unwrap_or(false);
            if resolved {
                continue;
            }
            let mut affected = vec![ev.node_id.clone()];
            for e in self.outgoing(&ev.node_id, Some(edge::DEPENDS_ON))? {
                affected.push(e.to_id);
            }
            affected.sort();
            let summary = ev
                .properties
                .get("summary")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("reality diverged from the design");
            // The answer differs by which reality diverged: build-side drift
            // is answered by the two-sided accept; fielded drift (BL-9) by
            // correcting the DEPLOYED_TO declaration — or the deployment —
            // and reconciling again, which resolves the event on agreement.
            let drift_type = ev
                .properties
                .get("drift_type")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("");
            let advice = if drift_type.starts_with("deployment_") {
                "the fielded state and the declaration disagree and nobody has said which is right. Fix the declaration (deploy_to with the true status) or fix the deployment, then reconcile_deployment again."
            } else if drift_type == "status_mismatch" {
                "the recorded outcome and the real run disagree. Set the status to what the run actually reported (set_verification_status), or fix the thing under test, then reconcile_verification again."
            } else {
                "the code moved and nobody has said what it means. Accept the new baseline with a disposition (design_holds or design_updated), or fix the build back."
            };
            gaps.push(GapCandidate {
                id: gap_id(GapSource::UnresolvedDrift, &affected),
                gap_source: GapSource::UnresolvedDrift,
                scope: GapScope::Capability,
                severity: 0.75,
                title: "A recorded divergence is waiting for its answer".to_string(),
                description: format!("{summary} — {advice}"),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "DriftEvent '{}' has resolved=false; the divergence was observed and written down, and the second question is unanswered.",
                    ev.node_id
                ),
            });
        }
        Ok(())
    }

    /// A built Component no Release includes (see
    /// [`GapSource::UnreleasedComponent`] for the double gate).
    fn detect_unreleased_components(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let releases = self.scan_nodes(node::RELEASE)?;
        if releases.is_empty() {
            return Ok(());
        }
        let mut shipped: BTreeSet<String> = BTreeSet::new();
        for rel in &releases {
            for e in self.outgoing(&rel.node_id, Some(edge::INCLUDES))? {
                shipped.insert(e.to_id);
            }
        }
        if shipped.is_empty() {
            // Releases exist but contents are not modelled — a different, whole-
            // graph situation, not one gap per component.
            return Ok(());
        }
        // A release that includes an assembly ships its parts: expand the
        // shipped set down every CONTAINS edge, so including a subsystem covers
        // its modules without an explicit INCLUDES per leaf (BL-89 E.1 — the
        // same "an assembly speaks through its children" lesson dead_end and the
        // community detector already carry).
        let mut frontier: Vec<String> = shipped.iter().cloned().collect();
        while let Some(id) = frontier.pop() {
            for e in self.outgoing(&id, Some(edge::CONTAINS))? {
                if shipped.insert(e.to_id.clone()) {
                    frontier.push(e.to_id);
                }
            }
        }
        for cmp in self.scan_nodes(node::COMPONENT)? {
            if shipped.contains(&cmp.node_id) {
                continue;
            }
            // Built = an artifact realizes the component, or realizes a
            // capability allocated to it (both P3 shapes, per BL-38).
            let mut built_by: Vec<String> = self
                .incoming(&cmp.node_id, Some(edge::REALIZES))?
                .into_iter()
                .map(|e| e.from_id)
                .collect();
            for alloc in self.incoming(&cmp.node_id, Some(edge::ALLOCATED_TO))? {
                for e in self.incoming(&alloc.from_id, Some(edge::REALIZES))? {
                    built_by.push(e.from_id);
                }
            }
            if built_by.is_empty() {
                continue; // not built — design_without_build's territory
            }
            if built_by.iter().any(|a| shipped.contains(a)) {
                continue; // its build ships, even if the component node is unlisted
            }
            let name = node_name(&cmp);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::UnreleasedComponent,
                    std::slice::from_ref(&cmp.node_id),
                ),
                gap_source: GapSource::UnreleasedComponent,
                scope: GapScope::Component,
                severity: 0.5,
                title: format!("“{name}” is built but ships in nothing"),
                description: format!(
                    "“{name}” is built, and no release includes it or anything that realizes it — is it part of a future release, or dead weight?"
                ),
                affected_ids: vec![cmp.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "Component '{}' has realizing artifacts; {} release(s) exist and model their contents, and none includes it.",
                    cmp.node_id,
                    releases.len()
                ),
            });
        }
        Ok(())
    }

    /// Statuses whose claims the structure denies (see
    /// [`GapSource::StatusContradiction`]). Scoped to the two unambiguous
    /// cases — `verified` without a passing check, `met` with nothing
    /// satisfying — because weaker claims (`realized` without an artifact) are
    /// already absence gaps, and double-reporting them would be the
    /// DETECT/HEAL double-count in a new costume.
    fn detect_status_contradictions(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for cap in self.scan_nodes(node::CAPABILITY)? {
            if cap
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                != Some("verified")
            {
                continue;
            }
            // A component-granularity check counts as proof for the status
            // claim too (BL-73): the modeller who marked a capability
            // `verified` because its component's suite passes is not
            // overstating — the 0.35 component_granularity_verification gap
            // asks the depth question, and asking it twice at 0.70 recreates
            // the 21-acknowledge pile this item exists to remove.
            if self.capability_verification(&cap.node_id)?
                != crate::verify::CapabilityVerification::Unchecked
            {
                continue;
            }
            let name = node_name(&cap);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::StatusContradiction,
                    std::slice::from_ref(&cap.node_id),
                ),
                gap_source: GapSource::StatusContradiction,
                scope: GapScope::Capability,
                severity: 0.70,
                title: format!("“{name}” claims verified, and nothing proves it"),
                description: format!(
                    "“{name}” has status `verified`, but no passing check verifies it — either run and record the check, or the status is overstating what is known."
                ),
                affected_ids: vec![cap.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "Capability '{}' has status=verified and no incoming VERIFIES from a Verification with status=passing.",
                    cap.node_id
                ),
            });
        }
        for req in self.scan_nodes(node::REQUIREMENT)? {
            if req
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                != Some("met")
            {
                continue;
            }
            if !self
                .incoming(&req.node_id, Some(edge::SATISFIES))?
                .is_empty()
            {
                continue;
            }
            let name = node_name(&req);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::StatusContradiction,
                    std::slice::from_ref(&req.node_id),
                ),
                gap_source: GapSource::StatusContradiction,
                scope: GapScope::Project,
                severity: 0.70,
                title: format!("“{name}” claims met, and nothing satisfies it"),
                description: format!(
                    "“{name}” has status `met`, but no capability satisfies it — and `met` silences the unsatisfied-requirement check, so nothing else can catch this. Link what meets it, or the status is a claim with nothing behind it."
                ),
                affected_ids: vec![req.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "Requirement '{}' has status=met and 0 incoming SATISFIES; `met` suppresses unsatisfied_requirement by design, so this is the only detector that can see it.",
                    req.node_id
                ),
            });
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
        // Capabilities only. An Artifact realizing a verified capability was
        // once flagged too, on the reasoning that proving the behaviour does
        // not prove *this file* delivers it. True, and unhelpful: the rule
        // demanded one VERIFIES edge per source file, which nobody writes.
        // Modelling reflow2's own design made it 22 of 25 gaps — 88% of the
        // list, on a crate whose capabilities are all tested — and a list that
        // cannot reach zero teaches you to skim it.
        //
        // The coverage is still counted, by `verification_coverage`, and
        // reported by `graph_report`. It informs rather than demands, the same
        // resolution `unexpected_coupling` reached (BL-6b).
        // Component granularity (BL-73, from the first extensive field
        // trial): a brownfield adopt with a real per-service suite read as
        // "0/20 capabilities verified" and cost 21 near-identical
        // acknowledges, because the per-capability gap could not see a
        // passing check one hop away. A capability riding its component's
        // suite is not unchecked — it gets ONE per-component question at 0.35
        // ("is component granularity enough for these?") instead of N
        // per-capability alarms at 0.55 (`dec:component-verified-computed`).
        let mut riding: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for n in self.scan_nodes(node::CAPABILITY)? {
            if !self.incoming(&n.node_id, Some(edge::VERIFIES))?.is_empty() {
                continue;
            }
            let mut carrier = None;
            for e in self.outgoing(&n.node_id, Some(edge::ALLOCATED_TO))? {
                if self.has_passing_verification(&e.to_id)? {
                    carrier = Some(e.to_id);
                    break;
                }
            }
            if let Some(component) = carrier {
                riding.entry(component).or_default().push(n.node_id.clone());
                continue;
            }
            let name = node_name(&n);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::UnverifiedCapability,
                    std::slice::from_ref(&n.node_id),
                ),
                gap_source: GapSource::UnverifiedCapability,
                scope: GapScope::Capability,
                severity: 0.55,
                title: format!("Nothing verifies \u{201c}{name}\u{201d}"),
                description: format!(
                    "\u{201c}{name}\u{201d} has no verification proving it works — how will \
                     you confirm it?"
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "Capability '{}' has 0 incoming VERIFIES; project has {} verification(s).",
                    n.node_id, pop.verifications
                ),
            });
        }
        for (component, mut caps) in riding {
            caps.sort();
            let count = caps.len();
            let cmp_name = self
                .get_node(node::COMPONENT, &component)?
                .map(|c| node_name(&c))
                .unwrap_or_else(|| component.clone());
            let listed = caps.join(", ");
            let mut affected = vec![component.clone()];
            affected.extend(caps);
            gaps.push(GapCandidate {
                id: gap_id(GapSource::ComponentGranularityVerification, &affected),
                gap_source: GapSource::ComponentGranularityVerification,
                scope: GapScope::Component,
                severity: 0.35,
                title: format!(
                    "{count} capability(ies) verified only at component granularity via \
                     \u{201c}{cmp_name}\u{201d}"
                ),
                description: format!(
                    "\u{201c}{cmp_name}\u{201d}'s passing check is the only verification these \
                     capabilities have: {listed}. That is a real check, one hop away — deepen \
                     with per-capability verifications where the behaviour deserves its own \
                     proof, or accept component granularity here once."
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "Component '{component}' has a passing VERIFIES; {count} capability(ies) \
                     allocated to it have 0 VERIFIES edges of their own."
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
            // Fold the producing edge into the id so a CONTAINS and a
            // DEPENDS_ON missing-intermediate over the same pair get DISTINCT
            // gap ids (BL-58) — else one acknowledgement suppresses both. The
            // discriminant is a hash input only; `affected_ids` stays the real
            // component ids.
            let mut id_input = issue.components.clone();
            if let Some(rel) = issue.relation {
                id_input.push(format!("via:{rel}"));
            }
            gaps.push(GapCandidate {
                id: gap_id(source, &id_input),
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

    /// A proposed Decision holding ≥2 registered alternatives — an open fork
    /// (BL-70's "missing teeth": a proposed Decision that otherwise gates
    /// nothing). One question per decision point, anchored on the Decision and
    /// its alternatives, so acknowledging it survives only while that exact fork
    /// stands. A fork of one road is not a choice, so ≥2 is the threshold.
    fn detect_undecided_decision_points(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        for dec in self.scan_nodes(node::DECISION)? {
            if dec
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                != Some("proposed")
            {
                continue;
            }
            let alts = self.alternatives_for(&dec.node_id)?;
            if alts.len() < 2 {
                continue;
            }
            let name = node_name(&dec);
            let mut affected = vec![dec.node_id.clone()];
            affected.extend(alts.iter().map(|a| a.id.clone()));
            gaps.push(GapCandidate {
                id: gap_id(GapSource::UndecidedDecisionPoint, &affected),
                gap_source: GapSource::UndecidedDecisionPoint,
                scope: GapScope::Capability,
                severity: 0.6,
                title: format!("Decision “{name}” has {} alternatives, undecided", alts.len()),
                description: format!(
                    "“{name}” is a proposed decision point with {} alternatives held open — which do you choose? Compare them with analyze_alternatives, then collapse_decision to settle it.",
                    alts.len()
                ),
                affected_ids: affected,
                suggested_depth: 3,
                evidence: format!(
                    "Decision '{}' is proposed with {} alternative(s) GOVERNED_BY it.",
                    dec.node_id,
                    alts.len()
                ),
            });
        }
        Ok(())
    }

    /// Capabilities proven against their spec (a passing verification-kind
    /// check) but with no validation-kind check confirming they meet the intent
    /// — "built right" without "the right thing". Reads `Verification.kind`, the
    /// reader that earns it its keep (`dec:edge-orthogonality`). One rollup, not
    /// N per-capability alarms: the design tracks verification but not validation
    /// (the BL-73 anti-flood lesson).
    fn detect_unvalidated_capabilities(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        let mut unvalidated: Vec<(String, String)> = Vec::new();
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let mut has_verification = false;
            let mut has_validation = false;
            for e in self.incoming(&cap.node_id, Some(edge::VERIFIES))? {
                let Some(ver) = self.get_node(node::VERIFICATION, &e.from_id)? else {
                    continue;
                };
                if ver
                    .properties
                    .get("status")
                    .and_then(dynograph_core::Value::as_str)
                    != Some("passing")
                {
                    continue;
                }
                match ver
                    .properties
                    .get("kind")
                    .and_then(dynograph_core::Value::as_str)
                {
                    Some("validation") => has_validation = true,
                    _ => has_verification = true, // default kind is verification
                }
            }
            if has_verification && !has_validation {
                unvalidated.push((cap.node_id.clone(), node_name(&cap)));
            }
        }
        if unvalidated.is_empty() {
            return Ok(());
        }
        let affected: Vec<String> = unvalidated.iter().map(|(id, _)| id.clone()).collect();
        let names: Vec<String> = unvalidated.iter().map(|(_, n)| n.clone()).collect();
        let n = unvalidated.len();
        gaps.push(GapCandidate {
            id: gap_id(GapSource::UnvalidatedCapability, &affected),
            gap_source: GapSource::UnvalidatedCapability,
            scope: GapScope::Project,
            severity: 0.35,
            title: format!(
                "{n} verified capabilit{} not validated",
                if n == 1 { "y" } else { "ies" }
            ),
            description: format!(
                "{n} capabilit{} proven against spec, but no validation check confirms {} meet the operational intent — built right, but the right thing? Add a Verification of kind=validation, or acknowledge that validation is tracked elsewhere.",
                if n == 1 { "y is" } else { "ies are" },
                if n == 1 { "it" } else { "they" }
            ),
            affected_ids: affected,
            suggested_depth: 2,
            evidence: format!(
                "Capabilities with a passing verification-kind VERIFIES and no passing validation-kind VERIFIES: {}.",
                names.join(", ")
            ),
        });
        Ok(())
    }
}

/// The `name` property, falling back to the id.
/// Order two ids so a pair has one identity regardless of which side it was
/// found from — the gap id hashes them, so `(a, b)` and `(b, a)` must not be
/// two different gaps about the same fact.
fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

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
