//! INGEST — freeform design material → schema-validated graph content
//! (docs/extraction-plan.md). The **CHANGE** step's content path: how a brief,
//! spec, review note, or an agent's own reasoning becomes typed nodes and edges.
//!
//! The pipeline mirrors storyflow's battle-tested shape:
//!
//! ```text
//! input → EXTRACT (multi-pass, phase-gated) → INTEGRATE (typed, provenance-stamped)
//! ```
//!
//! Every LLM-reasoning pass goes through the pluggable [`LlmBackend`] seam, so
//! INGEST runs against the [`MockLlmBackend`](crate::llm::MockLlmBackend) with no
//! provider. The storyflow disciplines that carry over and are enforced here:
//!
//! - **One shared call helper** ([`run_pass`]) — model call + JSON parse + error
//!   enveloping live in one place (discipline 1).
//! - **Never cascade-fail** — a pass that errors fills only its own slot with an
//!   empty default and a recorded [`PassError`]; siblings survive (discipline 2/4).
//! - **The discovery gate** — a classifier answers "what content is present?" as
//!   orthogonal booleans, so phase-2 passes don't hunt for structure that isn't
//!   there (discipline 5/6).
//! - **Roster threading** — phase-2/3 passes that emit edges get the phase-1
//!   rosters (id + name) so they reference real ids, not invented ones
//!   (discipline 11).
//! - **No silent drops** — an edge whose endpoint wasn't created (a phantom ref)
//!   or fails schema validation is recorded in [`IngestReport::dropped_edges`]
//!   with a reason; a node that fails validation is recorded in `warnings`;
//!   `status` goes `Partial`. Nothing is silently swallowed.
//! - **Provenance** — everything created is linked from one `Fragment` via
//!   `YIELDED`, stamped with how it entered the graph.
//! - **Time-aware resolution** — each extracted node resolves to
//!   *matched-unchanged* (no-op), *matched-evolved* (snapshot the prior state +
//!   record a `ChangeEvent`, THEN apply — never a silent overwrite), or
//!   *genuinely-new*. Re-ingesting an updated brief records the change.
//! - **Cross-id fuzzy dedup** — a new id whose name closely matches an existing
//!   same-type node (`token_sort_ratio` ≥ [`FUZZY_MATCH_THRESHOLD`], no
//!   embeddings) resolves to that node instead of duplicating; the merge is
//!   recorded in `fuzzy_merges` and edges redirect through an alias map.
//!
//! Deferred (noted so they're not mistaken for done): the **vector tiebreaker**
//! for the ambiguous middle band of `fuzzy_then_vector` — matching entities that
//! mean the same but read differently needs embeddings, kept behind an optional
//! pluggable seam (see the interaction-surface decision). Also deferred: the
//! **SME** augmentation pass,
//! real parallelism (passes run sequentially here), per-pass timeout/retry,
//! metrics, and the remaining passes (flows, actors,
//! artifacts, resources, inference, dimensions, changes). The **decisions** and
//! **verifications** passes landed 2026-07-27 — the rationale layer is what an old corpus is
//! richest in and what a codebase cannot be re-read to recover. This
//! increment implements the spine:
//! project/requirements/constraints/capabilities → components → interfaces →
//! satisfies/dependencies.

use std::collections::HashMap;

use crate::fuzzy::token_sort_ratio;
use dynograph_core::{DynoError, Value};
use dynograph_storage::StoredNode;
use serde::Deserialize;

use crate::agent::{AgentAnswer, AgentBackend, AgentPrompt, PartialBackend};
use crate::graph::DesignGraph;
use crate::llm::{LlmBackend, LlmRequest, complete_json};
use crate::nodes::{Props, edge, node};
use crate::temporal::{ChangeAction, ChangeRecord, ChangeType, EpochType};

// ---- Extraction output shapes (strict JSON per pass) -----------------------

#[derive(Debug, Default, Deserialize)]
struct ProjectIntent {
    project: Option<ExtractedProject>,
}

#[derive(Debug, Deserialize)]
struct ExtractedProject {
    id: String,
    name: String,
    #[serde(default)]
    objective: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    domain: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RequirementsOut {
    #[serde(default)]
    requirements: Vec<ExtractedRequirement>,
}

#[derive(Debug, Deserialize)]
struct ExtractedRequirement {
    id: String,
    name: String,
    statement: String,
    #[serde(default)]
    priority: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ConstraintsOut {
    #[serde(default)]
    constraints: Vec<ExtractedConstraint>,
}

#[derive(Debug, Deserialize)]
struct ExtractedConstraint {
    id: String,
    name: String,
    statement: String,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CapabilitiesOut {
    #[serde(default)]
    capabilities: Vec<ExtractedCapability>,
}

#[derive(Debug, Deserialize)]
struct ExtractedCapability {
    id: String,
    name: String,
    description: String,
}

/// The discovery gate — orthogonal booleans over what design content is present.
/// Anchor-required: `true` only when a concrete instance is named (see the doc).
///
/// `components` and `interfaces` gate passes in this increment. The other fields
/// are the classifier's full contract and gate phase-2 passes **not yet built**
/// (flows, actors, decisions, artifacts, resources). They are kept — rather
/// than narrowing the classifier — so the deferral is visible; it is recorded as
/// Deferred in `docs/requirements-coverage.md`. `#[allow(dead_code)]` marks the
/// gap explicitly instead of silently.
#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct Discovery {
    #[serde(default)]
    components: bool,
    #[serde(default)]
    interfaces: bool,
    #[serde(default)]
    actors: bool,
    #[serde(default)]
    decisions: bool,
    #[serde(default)]
    artifacts: bool,
    #[serde(default)]
    verifications: bool,
    #[serde(default)]
    flows: bool,
    #[serde(default)]
    resources: bool,
}

#[derive(Debug, Default, Deserialize)]
struct ComponentsOut {
    #[serde(default)]
    components: Vec<ExtractedComponent>,
}

#[derive(Debug, Deserialize)]
struct ExtractedComponent {
    id: String,
    name: String,
    purpose: String,
    /// Capability ids (from the phase-1 roster) allocated to this component.
    #[serde(default)]
    allocated_capability_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct InterfacesOut {
    #[serde(default)]
    interfaces: Vec<ExtractedInterface>,
}

/// A contract between parts, with both sides named in one pass — the provider
/// and consumers are what the contract *is*, so splitting them across passes
/// would invite one side to be extracted without the other.
#[derive(Debug, Deserialize)]
struct ExtractedInterface {
    id: String,
    name: String,
    #[serde(default)]
    medium: Option<String>,
    #[serde(default)]
    spec: Option<String>,
    /// Component id (from the phase-2 roster) that exposes this contract.
    #[serde(default)]
    provided_by_component_id: Option<String>,
    /// Component ids (from the phase-2 roster) that depend on this contract.
    #[serde(default)]
    consumed_by_component_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DecisionsOut {
    #[serde(default)]
    decisions: Vec<ExtractedDecision>,
}

/// A choice the source material records having been made, and why.
///
/// The pass that makes an old corpus worth ingesting at all: *why* something
/// was built the way it was is exactly what is lost when the people leave, and
/// it is the one thing a codebase cannot be re-read to recover.
///
/// **No status field, deliberately.** Everything ingest creates lands at the
/// schema default, and for `Decision` that default is `proposed` — set that way
/// (req:decision-status-not-asserted) because an `accepted` Decision is what
/// where-am-i reads back as "what you decided", what the fork layer treats as
/// binding, and what the KPP contradiction check reads as a trade already made.
/// An extraction is the agent's reading of a document, not the user's signature,
/// so reaching `accepted` stays a separate act. Requirements from ingest land
/// `proposed` for the same reason; this is consistency, not new doctrine.
#[derive(Debug, Deserialize)]
struct ExtractedDecision {
    id: String,
    name: String,
    decision: String,
    #[serde(default)]
    rationale: Option<String>,
    /// Ids from the phase-1/2 rosters that this choice shapes. Each becomes a
    /// GOVERNED_BY edge from the governed node to the Decision.
    #[serde(default)]
    governs_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct VerificationsOut {
    #[serde(default)]
    verifications: Vec<ExtractedVerification>,
}

/// A check the source material records having been made — a test run, an
/// inspection, a demonstration, a measurement.
///
/// **No status field, and that is the whole safety of this pass.** Everything
/// ingest creates lands at the schema default, which for `Verification` is
/// `planned`. A document saying "the load test passed" is a CLAIM about a
/// result; recording it as `passing` would let prose promote a capability to
/// verified, which is the exact "green while nothing was checked" failure this
/// project spent 2026-07-26 finding in its own code. The claim is preserved in
/// `description`, in the source's words, where a person can read it and decide.
///
/// This is safe to attach only because `unverified_capability` was tightened
/// the same day to require a PASSING check: before that, hanging a `planned`
/// Verification off a capability silenced the question.
#[derive(Debug, Deserialize)]
struct ExtractedVerification {
    id: String,
    name: String,
    /// What the source says about the check and its outcome, in its own terms.
    #[serde(default)]
    description: Option<String>,
    /// How it was checked, as a schema `method` value.
    #[serde(default)]
    method: Option<String>,
    /// Ids the check covers. Each becomes a VERIFIES edge.
    #[serde(default)]
    verifies_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SatisfiesOut {
    #[serde(default)]
    satisfies: Vec<SatisfiesEdge>,
}

#[derive(Debug, Deserialize)]
struct SatisfiesEdge {
    capability_id: String,
    requirement_id: String,
}

#[derive(Debug, Default, Deserialize)]
struct DependenciesOut {
    #[serde(default)]
    dependencies: Vec<ExtractedDependency>,
}

#[derive(Debug, Deserialize)]
struct ExtractedDependency {
    from_capability_id: String,
    to_capability_id: String,
    #[serde(default)]
    dependency_type: Option<String>,
    /// Coupling strength 0..1 (the graph-analysis weight facet). Extraction
    /// estimates it; integration stamps `weight_basis: estimated`.
    #[serde(default)]
    weight: Option<f64>,
}

// ---- Report shapes ---------------------------------------------------------

/// A pass that failed to produce usable output (enveloped, not fatal).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PassError {
    /// The pass name (e.g. `"requirements"`).
    pub pass: &'static str,
    /// The error (backend or parse).
    pub error: String,
}

/// An edge that could not be created, with the reason (no silent drops).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DroppedEdge {
    /// Edge type.
    pub edge_type: String,
    /// Source id.
    pub from_id: String,
    /// Target id.
    pub to_id: String,
    /// Why it was dropped.
    pub reason: String,
}

/// A cross-id fuzzy dedup: an extracted node whose id was new but whose name
/// matched an existing same-type node closely enough to be treated as the same
/// entity. Surfaced (never silent) so a wrong merge is auditable.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FuzzyMerge {
    /// The id the extraction produced.
    pub extracted_id: String,
    /// The existing canonical node it resolved to.
    pub canonical_id: String,
    /// The node type.
    pub node_type: &'static str,
    /// The fuzzy score (0–100) that cleared the threshold.
    pub score: u32,
    /// How the pair was found. Always `Fuzzy` today — a token-subset relation is
    /// reported as a candidate rather than merged, because `Auth Service` is a
    /// strict subset of `Legacy Auth Service` and those are plainly two things.
    pub match_kind: MatchKind,
    /// The name the merged node ended up carrying.
    pub canonical_name: String,
    /// The other name, when the two documents disagreed — `None` when they
    /// called it the same thing. Reported rather than dropped: a merge that
    /// silently discards one of two human-chosen names loses the only evidence
    /// that the choice was ever made.
    pub alias_name: Option<String>,
}

/// How a near-match was found. Recorded on every merge and every candidate,
/// because when one later turns out wrong the discriminator is what says whether
/// to fix a threshold or a rule (storyflow's `match_kind`, imported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// Name similarity score cleared the type's threshold.
    Fuzzy,
    /// One name's words are a strict subset of the other's. Found structurally,
    /// because similarity SCORING cannot find it: a ratio falls as the length
    /// difference grows, so `Gateway` vs `API Gateway` scores 74 — below every
    /// threshold reflow2 declares — while being the case a corpus produces most.
    /// No amount of threshold tuning reaches it; it needs a different question.
    TokenSubset,
}

/// Resolve a node's type from reflow2's typed-id convention. Returns `None` for
/// anything unrecognised, so a malformed id is reported and dropped rather than
/// written against a guessed type (rule 4).
fn node_type_from_id(id: &str) -> Option<&'static str> {
    match id.split(':').next()? {
        "req" => Some(node::REQUIREMENT),
        "cap" => Some(node::CAPABILITY),
        "cmp" | "sys" => Some(node::COMPONENT),
        "ifc" => Some(node::INTERFACE),
        "art" => Some(node::ARTIFACT),
        _ => None,
    }
}

/// Words dropped before comparing names. **Grammar only, never domain nouns.**
/// storyflow strips `the`/`of`/`a`/`an`/`and`; reflow2 must not extend that to
/// `service`, `system`, `module` or `component`, however tempting — design prose
/// is made of those words, and stripping them would collapse `Billing Service`
/// and `Auth Service` into the same two tokens.
const NAME_STOPWORDS: &[&str] = &[
    "the", "of", "a", "an", "and", "or", "for", "to", "in", "on", "&",
];

/// Lowercase, split on whitespace, trim non-alphanumeric edges, drop stopwords
/// and empties. `"The Auth Service"` → `["auth", "service"]`.
fn name_tokens(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|t| !t.is_empty() && !NAME_STOPWORDS.contains(&t.as_str()))
        .collect()
}

/// Tokens for the DISTINGUISHER, split on every non-alphanumeric rather than on
/// whitespace alone ([BL-213]).
///
/// Deliberately not [`name_tokens`]: that one splits on whitespace, so
/// `dynograph-core` is a single token and the two halves of an identifier can
/// never be compared. Real systems name sibling modules `prefix-thing`, and
/// telling `dynograph-core` from `dynograph-storage` means seeing `core` and
/// `storage` as separate words.
fn identifier_tokens(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_string)
        .filter(|t| !t.is_empty() && !NAME_STOPWORDS.contains(&t.as_str()))
        .collect()
}

/// Whether `short` is an abbreviation of `long` — `auth` of `authentication`.
///
/// Three characters minimum, because below that almost anything prefixes
/// anything and the rule would wave through the merges it exists to stop.
fn is_abbreviation_of(short: &str, long: &str) -> bool {
    short.len() >= 3 && long.len() > short.len() && long.starts_with(short)
}

/// The token pair that makes two names DIFFERENT THINGS rather than two
/// spellings of one thing — `None` when every difference is an abbreviation.
///
/// ⭐ WHY THIS EXISTS, measured 2026-08-05 by the first real corpus trial
/// ([BL-213], `dec:auto-merge-at-90-destroys-sibling-names`). A similarity score
/// says two names are ALIKE. It does not say they are the SAME THING, and for
/// the names real systems actually use the two come apart badly:
///
/// ```text
/// 95  dynograph-vector  vs dynograph-core         <- merged, and WRONG
/// 94  dynograph-storage vs dynograph-core         <- merged, and WRONG
/// 84  Auth Service      vs Authentication Service <- not merged, and WRONG
/// ```
///
/// Nine crates from one document became five: 44% of an architecture silently
/// destroyed, because sibling modules share a prefix and prefixes dominate the
/// score. No threshold fixes it — 95 was a sibling pair and 84 was a true
/// duplicate, so the ordering itself is inverted and a cutoff cannot separate
/// them.
///
/// The discriminator asks a different question. `core` and `storage` are not
/// spellings of each other, so the names denote different things however alike
/// they score. `auth` and `authentication` are, so they may merge. An extra
/// token with no counterpart at all (`Auth Service` vs `Auth Service v2`) is
/// distinguishing too — that is the case `docs/scope-corpus-ingest.md` warned
/// collapsing would lose.
///
/// Returns the offending pair for reporting, because "these two were not merged"
/// is not actionable and "not merged: `core` vs `storage`" is.
fn distinguishing_tokens(a: &str, b: &str) -> Option<String> {
    let (ta, tb) = (identifier_tokens(a), identifier_tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return None;
    }
    // Only the tokens one side has and the other lacks can distinguish; shared
    // words say nothing either way.
    let only_a: Vec<&String> = ta.iter().filter(|t| !tb.contains(t)).collect();
    let only_b: Vec<&String> = tb.iter().filter(|t| !ta.contains(t)).collect();

    let mut claimed = vec![false; only_b.len()];
    for token in &only_a {
        let partner = only_b.iter().enumerate().position(|(i, other)| {
            !claimed[i] && (is_abbreviation_of(token, other) || is_abbreviation_of(other, token))
        });
        match partner {
            Some(i) => claimed[i] = true,
            // A word on one side that nothing on the other side abbreviates.
            None => return Some(format!("`{token}` has no counterpart in \"{b}\"")),
        }
    }
    // And the reverse: an extra word on the existing side is just as
    // distinguishing as one on the new side.
    if let Some(i) = claimed.iter().position(|c| !c) {
        let token = only_b[i];
        return Some(format!("`{token}` has no counterpart in \"{a}\""));
    }
    None
}

/// True when every word of `subset` appears in `superset` AND `subset` is
/// strictly shorter. Equal token sets are excluded deliberately — those are the
/// fuzzy pass's business, and reporting them twice would be noise.
fn is_token_subset(subset: &[String], superset: &[String]) -> bool {
    if subset.is_empty() || superset.is_empty() || subset.len() >= superset.len() {
        return false;
    }
    subset.iter().all(|t| superset.contains(t))
}

/// A near-match that was NOT merged: it cleared the type's `fuzzy_threshold`
/// but fell short of its `auto_merge_threshold`, so it is reported for a human
/// to judge rather than decided by a number.
///
/// This band was invisible before 2026-07-26 — a name scoring 85 against an
/// existing node was simply created as a second node, and nothing said so. That
/// is the failure a corpus makes constantly: `Auth Service` in one document and
/// `Auth  Service` in another, quietly becoming two components.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MergeCandidate {
    /// The id the extraction produced — created as a NEW node, not merged.
    pub extracted_id: String,
    /// The existing node it resembles.
    pub candidate_id: String,
    /// The node type.
    pub node_type: &'static str,
    /// The fuzzy score (0–100) it reached.
    pub score: u32,
    /// The score it would have had to reach to merge without asking.
    pub auto_merge_threshold: u32,
    /// How the pair was found — a score, or a structural subset relation.
    pub match_kind: MatchKind,
    /// Which side is the more SPECIFIC name, and so the one a merge should keep.
    /// storyflow's rule, and it is the non-obvious half: the longer name wins
    /// regardless of edge count, because the naive "keep whichever has more
    /// edges" collapses the specific into the vague and is hard to undo.
    /// Reported, never acted on — reflow2 asks (`dec:ask-not-repair`).
    pub suggested_survivor: String,
    /// Set when the pair scored high enough to merge and was held back anyway,
    /// naming the word that makes them different things ([BL-213]).
    ///
    /// This is the difference between "these two were not merged" and "not
    /// merged: `core` has no counterpart in \"dynograph-storage\"" — only the
    /// second is something a person can rule on. `None` means the score simply
    /// fell short, which needs no explanation.
    pub distinguished_by: Option<String>,
}

/// Whether the ingest ran fully clean or degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestStatus {
    /// All passes and integrations succeeded.
    Ok,
    /// At least one pass errored, node failed validation, or edge was dropped.
    Partial,
}

/// The outcome of an ingest.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IngestReport {
    /// The provenance Fragment created for this input.
    pub fragment_id: String,
    /// Genuinely-new nodes created this run (includes the provenance Fragment).
    pub nodes_created: usize,
    /// Matched-evolved nodes: an existing node whose content changed — snapshotted
    /// and re-recorded with a `ChangeEvent`, never silently overwritten.
    pub nodes_evolved: usize,
    /// Matched-unchanged nodes: already present, identical content → left as-is.
    pub nodes_unchanged: usize,
    /// Cross-id fuzzy dedups: a new id resolved to an existing node by name
    /// similarity instead of creating a duplicate. Auditable, never silent.
    pub fuzzy_merges: Vec<FuzzyMerge>,
    /// Near-matches deliberately NOT merged — above the type's fuzzy threshold,
    /// below its auto-merge threshold. Created as new nodes AND reported, so the
    /// ambiguous band is a question put to a person rather than a coin flip
    /// resolved by a constant (`dec:ask-not-repair`).
    pub merge_candidates: Vec<MergeCandidate>,
    /// How many of those suspicions were persisted as `DUPLICATES` edges. The
    /// candidates above are this document's answer; the edges are what lets HEAL
    /// collect the same question across a whole corpus, in any order.
    pub duplicates_recorded: usize,
    /// Edges created this run.
    pub edges_created: usize,
    /// The `DesignEpoch` matched-evolved snapshots were pinned to (`Some` only
    /// when at least one node evolved).
    pub epoch_used: Option<String>,
    /// Pass-level failures (enveloped; siblings survived).
    pub pass_errors: Vec<PassError>,
    /// Node-level problems (e.g. a bad enum), recorded not fatal.
    pub warnings: Vec<String>,
    /// Edges dropped rather than emitted as phantoms.
    pub dropped_edges: Vec<DroppedEdge>,
    /// Overall status.
    pub status: IngestStatus,
}

/// Options for an ingest run.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// Id for the provenance Fragment.
    pub fragment_id: String,
    /// Title for the provenance Fragment.
    pub fragment_title: String,
    /// How this content entered the graph (`authored`/`planned`/`imported`/…).
    pub provenance: String,
    /// The active `DesignEpoch` this ingest happens in, if any (wired via
    /// `OCCURS_DURING`). Matched-evolved snapshots are pinned here; if unset and
    /// a node evolves, ingest opens `epoch:{fragment_id}` and reports it.
    pub epoch_id: Option<String>,
    /// The change type recorded on the `ChangeEvent` for every matched-evolved
    /// node this run (why you re-ingested). Per-node auto-classification is the
    /// deferred `changes` pass (EX-Z2); until then the caller declares it.
    pub change_type: ChangeType,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            fragment_id: "frag:ingest".to_string(),
            fragment_title: "Ingested design input".to_string(),
            provenance: "authored".to_string(),
            epoch_id: None,
            change_type: ChangeType::ScopeChange,
        }
    }
}

/// Run one extraction pass through the shared LLM seam. On any failure it
/// records a [`PassError`] and returns `T::default()` (empty) — one bad pass
/// never cancels the others (discipline 2).
fn run_pass<T: serde::de::DeserializeOwned + Default>(
    backend: &dyn LlmBackend,
    pass: &'static str,
    prompt: String,
    errors: &mut Vec<PassError>,
) -> T {
    let request = LlmRequest::new(prompt).with_system(
        "You extract structured design entities from freeform input and return ONLY \
         strict JSON in the shape the instruction specifies. Lists are always arrays.",
    );
    match complete_json::<T>(backend, &request) {
        Ok(value) => value,
        Err(e) => {
            errors.push(PassError {
                pass,
                error: e.to_string(),
            });
            T::default()
        }
    }
}

/// Build a pass prompt with the (unchanging) input FIRST for prefix-cache
/// sharing (discipline 7), then the pass instruction, then any roster context.
fn pass_prompt(input: &str, instruction: &str, roster: Option<&str>) -> String {
    let mut p = format!("INPUT:\n{input}\n\n{instruction}");
    if let Some(r) = roster {
        p.push_str("\n\nKNOWN ENTITIES (reference these ids exactly):\n");
        p.push_str(r);
    }
    p
}

/// A compact `id — name` roster for threading into edge passes.
fn roster<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    items
        .into_iter()
        .map(|(id, name)| format!("- {id} — {name}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One turn of the agent-native ingest handshake (SP-3b).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IngestStep {
    /// The run needs these answered before it can go further. Answer them and
    /// call again with the SAME input and options, passing every answer gathered
    /// so far — earlier ones included, because the run is replayed from the top
    /// rather than resumed.
    NeedsLlm {
        /// Prompts newly reachable this round, in the order the pipeline asked.
        prompts: Vec<AgentPrompt>,
        /// Answers supplied that no pass requested — stale, and reported rather
        /// than ignored, because a leftover answer usually means the input
        /// changed underneath the handshake.
        unused_answers: Vec<String>,
    },
    /// Every prompt was answered and the design was written.
    Done { report: Box<IngestReport> },
}

impl DesignGraph {
    /// Drive INGEST with the ambient agent as the model (SP-3b).
    ///
    /// `ingest` needs an [`LlmBackend`], and the agent-native surface has no
    /// provider: the *calling agent* is the model, and it cannot be reached
    /// mid-op because it is the outer caller. This is the collect-then-serve
    /// handshake from `agent.rs`, driven for an op whose prompt sequence
    /// **branches on earlier answers** — phase-2 passes are gated on a phase-1
    /// discovery classifier and threaded with phase-1 rosters, so the rounds
    /// cannot be collapsed into one.
    ///
    /// Call with no answers; answer what comes back; call again with everything
    /// gathered so far. Repeat until `Done`. Typically three rounds.
    ///
    /// **Nothing is written until the last one.** The prepare rounds replay the
    /// whole pipeline against a throwaway in-memory graph, which is safe because
    /// every prompt is issued before INGEST's integrate phase begins — so a
    /// half-answered run cannot leave half a design behind. It also means the
    /// handshake holds **no server-side session state**: each call is
    /// self-contained, so it survives a restart, works across seats sharing one
    /// server, and cannot leak an abandoned run.
    pub fn ingest_step(
        &mut self,
        input: &str,
        options: &IngestOptions,
        answers: Vec<AgentAnswer>,
    ) -> Result<IngestStep, DynoError> {
        // Prepare: replay against a scratch graph to see what is still needed.
        let probe = PartialBackend::new(answers.clone());
        let mut scratch = DesignGraph::open_in_memory()?;
        // The scratch run's own result is discarded; only what it ASKED matters.
        // Its errors are the expected consequence of stubbed answers, not a
        // failure of this call.
        let _ = scratch.ingest(input, options, &probe);
        let prompts = probe.outstanding();
        if !prompts.is_empty() {
            return Ok(IngestStep::NeedsLlm {
                prompts,
                unused_answers: probe.unused_answers(),
            });
        }

        // Serve: nothing outstanding, so replay for real against this design.
        let backend = AgentBackend::from_answers(answers);
        let report = self.ingest(input, options, &backend)?;
        Ok(IngestStep::Done {
            report: Box::new(report),
        })
    }
}

impl DesignGraph {
    /// EXTRACT freeform `input` into the graph and INTEGRATE it, stamped with
    /// provenance. Runs against any [`LlmBackend`]. See the module docs for the
    /// disciplines enforced and what's deferred.
    pub fn ingest(
        &mut self,
        input: &str,
        options: &IngestOptions,
        backend: &dyn LlmBackend,
    ) -> Result<IngestReport, DynoError> {
        // Each ingest run is a distinct extraction event and owns a distinct
        // Fragment (and, by default, its own `epoch:{fragment_id}`). Reusing a
        // fragment_id — easy to do accidentally via `IngestOptions::default()`,
        // whose id is the fixed `frag:ingest` — would overwrite the prior run's
        // Fragment and, worse, reopen its epoch and overwrite its snapshots,
        // violating axis Z's never-overwrite-the-past (BL-58). Refuse it up
        // front, before any write.
        if self
            .get_node(node::FRAGMENT, &options.fragment_id)?
            .is_some()
        {
            return Err(DynoError::Validation {
                node_type: node::FRAGMENT.to_string(),
                property: "id".to_string(),
                message: format!(
                    "ingest fragment '{}' already exists — each ingest run needs a unique \
                     fragment_id (IngestOptions::default() reuses 'frag:ingest'; set your own)",
                    options.fragment_id
                ),
            });
        }

        let mut errors = Vec::new();

        // ---- EXTRACT · Phase 1 (always run, read input only) ----
        let project = run_pass::<ProjectIntent>(
            backend,
            "project_intent",
            pass_prompt(
                input,
                r#"[pass:project_intent] Return JSON {"project":{"id":"proj:<slug>","name":"...","objective":"...","mode":"flexible|rigid","domain":"..."}}."#,
                None,
            ),
            &mut errors,
        )
        .project;
        let requirements = run_pass::<RequirementsOut>(
            backend,
            "requirements",
            pass_prompt(
                input,
                r#"[pass:requirements] Return JSON {"requirements":[{"id":"req:<slug>","name":"...","statement":"...","priority":"low|medium|high|critical"}]}."#,
                None,
            ),
            &mut errors,
        )
        .requirements;
        let constraints = run_pass::<ConstraintsOut>(
            backend,
            "constraints",
            pass_prompt(
                input,
                r#"[pass:constraints] Return JSON {"constraints":[{"id":"con:<slug>","name":"...","statement":"...","category":"technical|business|operational|physical|regulatory|budget|schedule"}]}."#,
                None,
            ),
            &mut errors,
        )
        .constraints;
        let capabilities = run_pass::<CapabilitiesOut>(
            backend,
            "capabilities",
            pass_prompt(
                input,
                r#"[pass:capabilities] Return JSON {"capabilities":[{"id":"cap:<slug>","name":"...","description":"..."}]}."#,
                None,
            ),
            &mut errors,
        )
        .capabilities;
        let discovery = run_pass::<Discovery>(
            backend,
            "discovery",
            pass_prompt(
                input,
                r#"[pass:discovery] Classify what design content is present. Return JSON with booleans {"components":bool,"interfaces":bool,"actors":bool,"decisions":bool,"artifacts":bool,"verifications":bool,"flows":bool,"resources":bool}. Return true ONLY when a concrete named instance is described acting as a unit — not when merely alluded to."#,
                None,
            ),
            &mut errors,
        );

        // ---- EXTRACT · Phase 2 (gated by discovery, roster-threaded) ----
        let cap_roster = roster(
            capabilities
                .iter()
                .map(|c| (c.id.as_str(), c.name.as_str())),
        );
        let components = if discovery.components {
            run_pass::<ComponentsOut>(
                backend,
                "components",
                pass_prompt(
                    input,
                    r#"[pass:components] Return JSON {"components":[{"id":"cmp:<slug>","name":"...","purpose":"...","allocated_capability_ids":["cap:..."]}]}. Allocate capabilities only from the known ids."#,
                    Some(&cap_roster),
                ),
                &mut errors,
            )
            .components
        } else {
            Vec::new()
        };

        // Interfaces are gated on components as well as on discovery: a contract
        // needs two sides to connect, and both PROVIDES and CONSUMES land on a
        // Component. Extracting them earlier would only manufacture unpaired
        // contracts that DETECT would then ask about.
        let cmp_roster = roster(components.iter().map(|c| (c.id.as_str(), c.name.as_str())));
        let interfaces = if discovery.interfaces && !components.is_empty() {
            run_pass::<InterfacesOut>(
                backend,
                "interfaces",
                pass_prompt(
                    input,
                    r#"[pass:interfaces] Which contracts connect the components — APIs, events, data feeds, physical or human connection points? Return JSON {"interfaces":[{"id":"ifc:<slug>","name":"...","medium":"REST|gRPC|json_rpc|event|graphql|cli|library|data|mechanical|electrical|human","spec":"endpoint path, signature, or protocol detail","provided_by_component_id":"cmp:...","consumed_by_component_ids":["cmp:..."]}]} using only known component ids. Omit a side you cannot ground in the text rather than guessing it."#,
                    Some(&cmp_roster),
                ),
                &mut errors,
            )
            .interfaces
        } else {
            Vec::new()
        };

        // Decisions: the rationale layer. Gated on discovery like every other
        // phase-2 pass, and given the rosters built so far so `governs_ids`
        // names real nodes instead of inventing them. Deliberately NOT gated on
        // components existing — a document can record why a requirement was
        // written long before anything was built to serve it, and that is
        // exactly the material an old corpus is richest in.
        let decisions = if discovery.decisions {
            let governable = format!(
                "Requirements:\n{}\n\nCapabilities:\n{cap_roster}\n\nComponents:\n{cmp_roster}",
                roster(
                    requirements
                        .iter()
                        .map(|r| (r.id.as_str(), r.name.as_str()))
                )
            );
            run_pass::<DecisionsOut>(
                backend,
                "decisions",
                pass_prompt(
                    input,
                    r#"[pass:decisions] What choices does this text record having been MADE, and why? Return JSON {"decisions":[{"id":"dec:<slug>","name":"the question that was settled","decision":"what was chosen","rationale":"why, in the source's own terms","governs_ids":["req:...","cap:...","cmp:..."]}]} using only ids from the roster. Extract only choices the text states were taken, with the reasoning it gives — never infer a rationale the source does not offer, and never record an option someone merely considered as a decision. Omit governs_ids you cannot ground."#,
                    Some(&governable),
                ),
                &mut errors,
            )
            .decisions
        } else {
            Vec::new()
        };

        // Verifications: the evidence layer. Gated on discovery and on there
        // being something to check — a check with nothing to verify is an
        // orphan, and DETECT would only ask about it.
        let verifications = if discovery.verifications
            && !(capabilities.is_empty() && components.is_empty())
        {
            let checkable = format!("Capabilities:\n{cap_roster}\n\nComponents:\n{cmp_roster}");
            run_pass::<VerificationsOut>(
                backend,
                "verifications",
                pass_prompt(
                    input,
                    r#"[pass:verifications] What checks does this text record — tests run, inspections, demonstrations, measurements, analyses? Return JSON {"verifications":[{"id":"ver:<slug>","name":"what was checked","description":"what the source says about the check AND its outcome, in its own words","method":"test|analysis|inspection|demonstration|measurement|observation|review|simulation","verifies_ids":["cap:...","cmp:..."]}]} using only ids from the roster. Record what the source claims; do NOT state an outcome the text does not give, and do not treat an intention to test as a test that happened. Omit verifies_ids you cannot ground."#,
                    Some(&checkable),
                ),
                &mut errors,
            )
            .verifications
        } else {
            Vec::new()
        };

        // ---- EXTRACT · Phase 3 (edge passes over rosters) ----
        let req_roster = roster(
            requirements
                .iter()
                .map(|r| (r.id.as_str(), r.name.as_str())),
        );
        let satisfies = if !capabilities.is_empty() && !requirements.is_empty() {
            run_pass::<SatisfiesOut>(
                backend,
                "satisfies",
                pass_prompt(
                    input,
                    r#"[pass:satisfies] Which capability satisfies which requirement? Return JSON {"satisfies":[{"capability_id":"cap:...","requirement_id":"req:..."}]} using only known ids."#,
                    Some(&format!("Capabilities:\n{cap_roster}\n\nRequirements:\n{req_roster}")),
                ),
                &mut errors,
            )
            .satisfies
        } else {
            Vec::new()
        };
        // dependencies: the functional coupling graph, carrying weights — the
        // signal the graph-analysis allocation work (graph-analysis.md) needs.
        let dependencies = if !capabilities.is_empty() {
            run_pass::<DependenciesOut>(
                backend,
                "dependencies",
                pass_prompt(
                    input,
                    r#"[pass:dependencies] Which capabilities depend on which? Return JSON {"dependencies":[{"from_capability_id":"cap:...","to_capability_id":"cap:...","dependency_type":"function_call|data_flow|control_flow|error_flow|physical","weight":0.7}]} using only known ids. `weight` is coupling strength 0..1 (higher = tighter). Keep the dependency graph acyclic."#,
                    Some(&cap_roster),
                ),
                &mut errors,
            )
            .dependencies
        } else {
            Vec::new()
        };

        // ---- INTEGRATE (resolve → typed, provenance-stamped, time-aware) ----
        let effective_epoch = options
            .epoch_id
            .clone()
            .unwrap_or_else(|| format!("epoch:{}", options.fragment_id));
        let mut st = Integration::new(&options.fragment_id, effective_epoch, options.change_type);

        // Provenance fragment first (a new fragment per ingest).
        let mut frag_props = Props::new()
            .set("title", options.fragment_title.as_str())
            .set("fragment_type", "design")
            .set("provenance", options.provenance.as_str());
        if !PROVENANCE_VALUES.contains(&options.provenance.as_str()) {
            st.warnings.push(format!(
                "provenance '{}' not a schema value; using 'authored'",
                options.provenance
            ));
            frag_props = frag_props.set("provenance", "authored");
        }
        match self.create_node(node::FRAGMENT, &options.fragment_id, frag_props) {
            Ok(_) => st.nodes_created += 1,
            Err(e) => st
                .warnings
                .push(format!("fragment '{}': {e}", options.fragment_id)),
        }
        // Honor a caller-named epoch up front so provenance-in-time is valid.
        if options.epoch_id.is_some() {
            self.ensure_epoch(&mut st);
            self.link_fragment_epoch(&mut st);
        }

        // Nodes (resolved: unchanged / evolved / new).
        if let Some(p) = &project {
            let mut props = Props::new().set("name", p.name.as_str());
            props = props.set_opt("objective", p.objective.as_deref());
            props = props.set_opt("domain", p.domain.as_deref());
            if let Some(m) = p.mode.as_deref()
                && (m == "flexible" || m == "rigid")
            {
                props = props.set("mode", m);
            }
            self.integrate_node(&mut st, node::PROJECT, &p.id, props);
        }
        for r in &requirements {
            let mut props = Props::new()
                .set("name", r.name.as_str())
                .set("statement", r.statement.as_str());
            if let Some(pr) = r.priority.as_deref()
                && PRIORITY_VALUES.contains(&pr)
            {
                props = props.set("priority", pr);
            }
            self.integrate_node(&mut st, node::REQUIREMENT, &r.id, props);
        }
        for c in &constraints {
            let mut props = Props::new()
                .set("name", c.name.as_str())
                .set("statement", c.statement.as_str());
            if let Some(cat) = c.category.as_deref()
                && CONSTRAINT_CATEGORIES.contains(&cat)
            {
                props = props.set("category", cat);
            }
            self.integrate_node(&mut st, node::CONSTRAINT, &c.id, props);
        }
        for c in &capabilities {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("description", c.description.as_str());
            self.integrate_node(&mut st, node::CAPABILITY, &c.id, props);
        }
        for c in &components {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("purpose", c.purpose.as_str());
            self.integrate_node(&mut st, node::COMPONENT, &c.id, props);
        }
        for d in &decisions {
            // `status` is left unset on purpose — the schema default is
            // `proposed`, and asserting `accepted` here would forge the user's
            // signature on someone else's reasoning.
            let mut props = Props::new()
                .set("name", d.name.as_str())
                .set("decision", d.decision.as_str());
            if let Some(r) = d.rationale.as_deref() {
                props = props.set("rationale", r);
            }
            self.integrate_node(&mut st, node::DECISION, &d.id, props);
        }
        for v in &verifications {
            // `status` is left unset: the schema default is `planned`, and a
            // document's claim that something passed is not reflow2 observing
            // it pass. The claim itself survives in `description`.
            let mut props = Props::new().set("name", v.name.as_str());
            if let Some(d) = v.description.as_deref() {
                props = props.set("description", d);
            }
            if let Some(m) = v.method.as_deref() {
                if VERIFICATION_METHODS.contains(&m) {
                    props = props.set("method", m);
                } else {
                    st.warnings.push(format!(
                        "verification '{}' method '{m}' not a schema value; using the default",
                        v.id
                    ));
                }
            }
            self.integrate_node(&mut st, node::VERIFICATION, &v.id, props);
        }
        for i in &interfaces {
            let mut props = Props::new().set("name", i.name.as_str());
            if let Some(m) = i.medium.as_deref() {
                if MEDIUM_VALUES.contains(&m) {
                    props = props.set("medium", m);
                } else {
                    st.warnings.push(format!(
                        "interface '{}' medium '{m}' not a schema value; using the default",
                        i.id
                    ));
                }
            }
            if let Some(s) = i.spec.as_deref() {
                props = props.set("spec", s);
            }
            self.integrate_node(&mut st, node::INTERFACE, &i.id, props);
        }

        // Edges: ALLOCATED_TO (capability -> component), SATISFIES (capability -> requirement).
        for c in &components {
            for cap_id in &c.allocated_capability_ids {
                self.integrate_edge(
                    &mut st,
                    edge::ALLOCATED_TO,
                    node::CAPABILITY,
                    cap_id,
                    node::COMPONENT,
                    &c.id,
                    Props::new(),
                );
            }
        }
        // GOVERNED_BY — the governed node points at the choice that shaped it.
        // The endpoint type is resolved from the id prefix rather than asked
        // for, because a pass that had to name both the id and its type would
        // give an extraction two ways to be inconsistent instead of one.
        for d in &decisions {
            for governed in &d.governs_ids {
                let Some(from_type) = node_type_from_id(governed) else {
                    st.warnings.push(format!(
                        "decision '{}' governs '{governed}', whose type is not recognisable from \
                         its id; edge dropped rather than guessed",
                        d.id
                    ));
                    continue;
                };
                self.integrate_edge(
                    &mut st,
                    edge::GOVERNED_BY,
                    from_type,
                    governed,
                    node::DECISION,
                    &d.id,
                    Props::new(),
                );
            }
        }

        // VERIFIES — the check points at what it covers.
        for v in &verifications {
            for target in &v.verifies_ids {
                let Some(to_type) = node_type_from_id(target) else {
                    st.warnings.push(format!(
                        "verification '{}' covers '{target}', whose type is not recognisable \
                         from its id; edge dropped rather than guessed",
                        v.id
                    ));
                    continue;
                };
                self.integrate_edge(
                    &mut st,
                    edge::VERIFIES,
                    node::VERIFICATION,
                    &v.id,
                    to_type,
                    target,
                    Props::new(),
                );
            }
        }

        // PROVIDES / CONSUMES — both sides of each contract. An interface whose
        // provider or consumers the extraction could not ground stays unpaired
        // on purpose: DETECT raises it as a question rather than ingest guessing.
        for i in &interfaces {
            if let Some(provider) = i.provided_by_component_id.as_deref() {
                self.integrate_edge(
                    &mut st,
                    edge::PROVIDES,
                    node::COMPONENT,
                    provider,
                    node::INTERFACE,
                    &i.id,
                    Props::new(),
                );
            }
            for consumer in &i.consumed_by_component_ids {
                self.integrate_edge(
                    &mut st,
                    edge::CONSUMES,
                    node::COMPONENT,
                    consumer,
                    node::INTERFACE,
                    &i.id,
                    Props::new(),
                );
            }
        }
        for s in &satisfies {
            self.integrate_edge(
                &mut st,
                edge::SATISFIES,
                node::CAPABILITY,
                &s.capability_id,
                node::REQUIREMENT,
                &s.requirement_id,
                Props::new(),
            );
        }
        for d in &dependencies {
            let mut props = Props::new().set("weight_basis", "estimated");
            if let Some(w) = d.weight
                && (0.0..=1.0).contains(&w)
            {
                props = props.set("weight", w);
            }
            if let Some(dt) = d.dependency_type.as_deref()
                && DEPENDENCY_TYPE_VALUES.contains(&dt)
            {
                props = props.set("dependency_type", dt);
            }
            self.integrate_edge(
                &mut st,
                edge::DEPENDS_ON,
                node::CAPABILITY,
                &d.from_capability_id,
                node::CAPABILITY,
                &d.to_capability_id,
                props,
            );
        }

        // If we lazily opened an epoch for evolutions, tie the fragment to it too.
        if options.epoch_id.is_none() && st.nodes_evolved > 0 {
            self.link_fragment_epoch(&mut st);
        }
        let epoch_used = if st.nodes_evolved > 0 || options.epoch_id.is_some() {
            Some(st.epoch_id.clone())
        } else {
            None
        };
        let status = if errors.is_empty() && st.warnings.is_empty() && st.dropped_edges.is_empty() {
            IngestStatus::Ok
        } else {
            IngestStatus::Partial
        };
        Ok(IngestReport {
            fragment_id: options.fragment_id.clone(),
            nodes_created: st.nodes_created,
            nodes_evolved: st.nodes_evolved,
            nodes_unchanged: st.nodes_unchanged,
            fuzzy_merges: st.fuzzy_merges,
            merge_candidates: st.merge_candidates,
            duplicates_recorded: st.duplicates_recorded,
            edges_created: st.edges_created,
            epoch_used,
            pass_errors: errors,
            warnings: st.warnings,
            dropped_edges: st.dropped_edges,
            status,
        })
    }

    /// Ensure the effective `DesignEpoch` node exists — created lazily the first
    /// time a matched-evolved node needs somewhere to pin its snapshot.
    fn ensure_epoch(&mut self, st: &mut Integration) {
        if st.epoch_ready {
            return;
        }
        st.epoch_ready = true;
        // A *read error* is not "the epoch exists" (BL-58): the old
        // `matches!(_, Ok(None))` skipped both creation and any warning on
        // `Err`, so the change events that pin to this epoch would land on
        // nothing, silently.
        match self.get_node(node::DESIGN_EPOCH, &st.epoch_id) {
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Err(e) = self.add_epoch(&st.epoch_id, "ingest epoch", EpochType::Revision, 0)
                {
                    st.warnings
                        .push(format!("open epoch '{}': {e}", st.epoch_id));
                }
            }
            Err(e) => st
                .warnings
                .push(format!("check epoch '{}': {e}", st.epoch_id)),
        }
    }

    /// `Fragment OCCURS_DURING epoch` — provenance-in-time.
    fn link_fragment_epoch(&mut self, st: &mut Integration) {
        match self.create_edge(
            edge::OCCURS_DURING,
            node::FRAGMENT,
            st.fragment_id,
            node::DESIGN_EPOCH,
            &st.epoch_id,
            Props::new(),
        ) {
            Ok(_) => st.edges_created += 1,
            // The module header promises nothing is silently swallowed (BL-58).
            Err(e) => st
                .warnings
                .push(format!("link fragment to epoch '{}': {e}", st.epoch_id)),
        }
    }

    /// Resolve one extracted node against the graph and integrate it:
    /// **genuinely-new** → create; **matched-unchanged** → leave as-is (no write,
    /// no snapshot); **matched-evolved** → snapshot the prior state + record a
    /// `ChangeEvent` (via [`record_change`](DesignGraph::record_change)) THEN
    /// apply the edit — never a silent overwrite (extraction-plan.md, "a
    /// matched-evolved result that lands with no Snapshot is an integrity
    /// breach"). Every resolved node is registered so later edges can reference
    /// it, and linked from the provenance fragment.
    fn integrate_node(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        props: Props,
    ) {
        let mut new_map = sanitize_extracted(st, node_type, id, props.build());
        match self.get_node(node_type, id) {
            Err(e) => st.warnings.push(format!("resolve {node_type} '{id}': {e}")),
            // Direct id hit → resolve against that node.
            Ok(Some(_)) => self.integrate_existing(st, node_type, id, new_map),
            // Id miss → try cross-id fuzzy dedup before creating a duplicate.
            Ok(None) => match self.fuzzy_match(node_type, &new_map, id) {
                Err(e) => {
                    st.warnings
                        .push(format!("fuzzy-match {node_type} '{id}': {e}"));
                    self.integrate_new(st, node_type, id, new_map);
                }
                // The band decides the ACT. At or above auto-merge this is not a
                // suspicion and merging is right; below it, merging would be a
                // number overruling a person, so the node is created and the
                // near-match reported instead (`dec:ask-not-repair`).
                Ok(Some((candidate, score))) => {
                    let (_, auto_merge) = self.resolution_thresholds(node_type);
                    // A score says the names are ALIKE; it does not say they are
                    // the same thing. For prefixed sibling names — how nearly
                    // every real system names its modules — the two come apart,
                    // and measured they come apart INVERTED: `dynograph-vector`
                    // scored 95 against `dynograph-core` while the canonical
                    // duplicate `Auth Service` ~ `Authentication Service` scored
                    // 84. So the band alone cannot be trusted to act, and no
                    // threshold repairs it ([BL-213]).
                    let distinguished =
                        self.existing_name(node_type, &candidate)
                            .and_then(|existing| {
                                new_map
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .and_then(|new_name| distinguishing_tokens(new_name, &existing))
                            });
                    if score >= auto_merge && distinguished.is_none() {
                        st.aliases.insert(id.to_string(), candidate.clone());
                        // `req:corpus-ingest` — "ordering must not decide
                        // meaning". The merge is right at this score; the NAME
                        // is what used to follow whichever document was read
                        // last, because the extracted map overwrites it. Two
                        // specs naming one thing "Read Path Cache" and "Cache
                        // Read Path" produced a different canonical name
                        // depending on directory order, which for a corpus is
                        // the read order of a folder nobody chose.
                        let (canonical_name, alias_name) =
                            self.settle_merged_name(node_type, &candidate, &mut new_map);
                        st.fuzzy_merges.push(FuzzyMerge {
                            extracted_id: id.to_string(),
                            canonical_id: candidate.clone(),
                            node_type,
                            score,
                            match_kind: MatchKind::Fuzzy,
                            canonical_name,
                            alias_name,
                        });
                        self.integrate_existing(st, node_type, &candidate, new_map);
                    } else {
                        st.merge_candidates.push(MergeCandidate {
                            extracted_id: id.to_string(),
                            candidate_id: candidate.clone(),
                            node_type,
                            score,
                            auto_merge_threshold: auto_merge,
                            match_kind: MatchKind::Fuzzy,
                            // A score says the two are alike, not which is more
                            // specific; the existing node is suggested because it
                            // is the one already carrying edges and history.
                            suggested_survivor: candidate.clone(),
                            distinguished_by: distinguished,
                        });
                        self.integrate_new(st, node_type, id, new_map);
                        self.suspect_duplicate(st, node_type, id, &candidate, Some(score));
                    }
                }
                // Nothing by score. Try the structural question before concluding
                // this is a genuinely new thing — `Gateway` vs `API Gateway`
                // scores 74, below every threshold reflow2 declares, and is the
                // case a corpus produces most.
                Ok(None) => {
                    // Drawn AFTER integrate_new below: create_edge refuses a
                    // dangling endpoint, and the node does not exist yet here.
                    let mut pending_subset: Option<String> = None;
                    match self.token_subset_match(node_type, &new_map, id) {
                        Err(e) => st
                            .warnings
                            .push(format!("token-subset match {node_type} '{id}': {e}")),
                        Ok(Some((candidate, existing_is_longer))) => {
                            let (_, auto_merge) = self.resolution_thresholds(node_type);
                            let survivor = if existing_is_longer {
                                candidate.clone()
                            } else {
                                id.to_string()
                            };
                            pending_subset = Some(candidate.clone());
                            st.merge_candidates.push(MergeCandidate {
                                extracted_id: id.to_string(),
                                candidate_id: candidate,
                                node_type,
                                // No score cleared anything — this was found
                                // structurally, and saying 0 would read as "not
                                // similar" when the truth is "not measured".
                                score: 0,
                                auto_merge_threshold: auto_merge,
                                match_kind: MatchKind::TokenSubset,
                                suggested_survivor: survivor,
                                // A subset match never reached the merge
                                // decision, so nothing held it back.
                                distinguished_by: None,
                            });
                        }
                        Ok(None) => {}
                    }
                    self.integrate_new(st, node_type, id, new_map);
                    if let Some(candidate) = pending_subset {
                        // No score cleared anything, so no confidence is
                        // asserted — absence reads as "not measured", which is
                        // the same reason the candidate carries score 0.
                        self.suspect_duplicate(st, node_type, id, &candidate, None);
                    }
                }
            },
        }
    }

    /// Create a genuinely-new node + its provenance link.
    fn integrate_new(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        new_map: HashMap<String, Value>,
    ) {
        match self.create_node(node_type, id, new_map) {
            Ok(_) => {
                st.created_ids.insert(id.to_string(), node_type);
                st.nodes_created += 1;
                self.yield_edge(st, node_type, id, "created");
            }
            Err(e) => st.warnings.push(format!("skipped {node_type} '{id}': {e}")),
        }
    }

    /// Resolve an extracted node against an existing one (`id` is a real node —
    /// a direct id hit or a fuzzy-matched canonical): matched-unchanged →
    /// no-op; matched-evolved → snapshot + `ChangeEvent` THEN apply.
    fn integrate_existing(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        new_map: HashMap<String, Value>,
    ) {
        let existing = match self.get_node(node_type, id) {
            Ok(Some(n)) => n,
            Ok(None) => return, // vanished between resolve and integrate — nothing to do
            Err(e) => {
                st.warnings.push(format!("resolve {node_type} '{id}': {e}"));
                return;
            }
        };
        if node_unchanged(&existing, &new_map) {
            st.created_ids.insert(id.to_string(), node_type);
            st.nodes_unchanged += 1;
            return;
        }
        // matched-evolved: remember the past, then apply the edit.
        self.ensure_epoch(st);
        let ce_id = format!("chg:{}:{id}", st.fragment_id);
        let name = format!("Re-ingest updated {node_type} {id}");
        let rec = ChangeRecord {
            epoch_id: &st.epoch_id,
            change_event_id: &ce_id,
            name: &name,
            change_type: st.change_type,
            // UNSTATED, and this caller is the reason the field is an Option.
            // A re-ingest genuinely cannot tell the two axes apart: the source
            // document may describe a system that moved, or it may just be a
            // better description of one that did not, and nothing reaching
            // this point distinguishes them. Saying nothing is the true answer.
            subject: None,
            target_type: node_type,
            target_id: id,
            action: ChangeAction::Modified,
        };
        match self.record_change(rec) {
            Ok(_) => {
                // Merge, don't replace (BL-58, the BL-46 failure on the ingest
                // path): the extraction produced only the fields it found in
                // the text, so `create_node` would re-materialize schema
                // defaults over everything it omitted — silently resetting a
                // status or provenance the re-ingest never mentioned. The
                // prior state is already snapshotted by `record_change` above.
                if let Err(e) = self.upsert_node(node_type, id, new_map) {
                    st.warnings
                        .push(format!("apply evolved {node_type} '{id}': {e}"));
                }
                st.created_ids.insert(id.to_string(), node_type);
                st.nodes_evolved += 1;
                self.yield_edge(st, node_type, id, "updated");
            }
            Err(e) => st
                .warnings
                .push(format!("snapshot evolved {node_type} '{id}': {e}")),
        }
    }

    /// Persist a near-match as a `DUPLICATES` edge, so the question survives the
    /// document that raised it.
    ///
    /// **This is what makes `dec:ask-not-repair` affordable at corpus scale.**
    /// That decision requires suspected duplicates to be asked rather than
    /// silently merged, and `cap:corpus-ingest` notes the consequence: *"at
    /// corpus scale the asking must be batched or the feature is unusable"*. A
    /// `MergeCandidate` alone cannot be batched — it lives in one document's
    /// [`IngestReport`] and is gone the moment the caller moves to the next
    /// file. Four hundred documents produce four hundred separate asks, each
    /// addressed to an agent that has already forgotten the last one.
    ///
    /// The batching machinery already exists and needed no new vocabulary: the
    /// edge turns a transient suspicion into a standing question that survives
    /// the run, in any order, however long it takes.
    ///
    /// # `basis: suspected` is load-bearing, and it was missing until 2026-08-08
    ///
    /// This function originally wrote a BARE `DUPLICATES` edge, and reasoned
    /// that HEAL's `duplicate` detector would collect it as a standing question.
    /// That reasoning was right about batching and wrong about safety: HEAL maps
    /// `duplicate` to an applicable `Merge`, so what was written as *a question*
    /// was read downstream as *a human's assertion that these are the same
    /// thing* — the exact precondition `dec:ask-not-repair` says merge is safe
    /// only because of. Nothing in the edge distinguished the two.
    ///
    /// Measured in dev_storyflow on 2026-08-07: `token_sort_ratio` scored six
    /// pairs of semantically unrelated design nodes at 81-85, inside this ASK
    /// band; `propose_heal` then offered TEN node deletions, and the served
    /// check-health skill classed them as the mechanical half safe to apply
    /// without review. Three bosses stood their fleet down from check-health
    /// rather than run it. Nobody's graph was destroyed only because the
    /// pairings looked implausible enough that a human read them.
    ///
    /// So the edge now says who decided. `suspected` keeps every batching
    /// property the original design wanted — the suspicion is durable, HEAL and
    /// DETECT can both find it later, and `detect_gaps` raises it as a
    /// `possible_duplicate` the user can answer or acknowledge — while making it
    /// impossible for a name-similarity score to reach `apply_heal`'s delete.
    ///
    /// **Drawn only in the ASK band.** At or above `auto_merge_threshold` the
    /// nodes are merged and there is nothing left to ask; below the type's
    /// `fuzzy_threshold` nothing was suspected at all. `confidence` carries the
    /// score where one was measured and is OMITTED for a structural
    /// (token-subset) match — writing 0.0 would read as "certainly unrelated",
    /// which is the opposite of what a subset relation means.
    ///
    /// Never cascade-fails: a refused edge is a warning, because losing one
    /// suspicion must not cost the document that carried it.
    fn suspect_duplicate(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        extracted_id: &str,
        candidate_id: &str,
        score: Option<u32>,
    ) {
        if extracted_id == candidate_id {
            return;
        }
        // A heuristic proposed this pair, so it is a question and never a merge
        // licence. Written explicitly rather than left to the schema default:
        // the value is the whole safety property, and a reader of the stored
        // edge should not have to know what the default was on the day it was
        // written to know whether a machine or a person decided.
        let mut props = Props::new().set("basis", "suspected");
        if let Some(s) = score {
            props = props.set("confidence", f64::from(s) / 100.0);
        }
        match self.create_edge(
            edge::DUPLICATES,
            node_type,
            extracted_id,
            node_type,
            candidate_id,
            props,
        ) {
            Ok(_) => st.duplicates_recorded += 1,
            Err(e) => st.warnings.push(format!(
                "record duplicate suspicion {node_type} '{extracted_id}' ~ '{candidate_id}': {e}"
            )),
        }
    }

    /// Decide which name a fuzzy-merged node keeps, **without consulting the
    /// order the documents arrived in**, and say which name lost.
    ///
    /// `req:corpus-ingest` names this as its load-bearing clause: *"Ordering
    /// must not decide meaning — which file happened to be read first must not
    /// determine the canonical name of anything."* It did. The extracted
    /// property map overwrites `name` on the survivor, so of two specs calling
    /// one thing `Read Path Cache` and `Cache Read Path`, whichever was read
    /// LAST named it — and for a corpus that is the iteration order of a folder
    /// nobody chose. Measured before the fix: the same two documents produced
    /// `"Cache Read Path"` one way round and `"Read Path Cache"` the other.
    ///
    /// The rule is **longer name wins, ties broken lexicographically**. Longer
    /// is not a guess about quality — it is the same instinct
    /// [`token_subset_match`](Self::token_subset_match) already encodes when it
    /// suggests the longer side as survivor, on the reading that the more
    /// specific name carries more of what the author meant (`Auth Service` over
    /// `Auth`). The lexicographic tiebreak exists only to make the rule TOTAL:
    /// without it, two equal-length names would fall back to arrival order and
    /// rebuild the bug for the narrow case.
    ///
    /// **This picks a name; it does not rule on which is better.** The loser is
    /// returned and recorded on the [`FuzzyMerge`] as `alias_name`, because a
    /// merge that silently discards one of two human-chosen names destroys the
    /// only evidence a person ever chose it — and `dec:ask-not-repair` governs
    /// this capability. Deliberately NOT scoped to the direct-id path:
    /// re-ingesting the SAME id with a new name is *matched-evolved*, where the
    /// newer document updating its own name is the correct reading.
    fn settle_merged_name(
        &self,
        node_type: &str,
        canonical_id: &str,
        new_map: &mut HashMap<String, Value>,
    ) -> (String, Option<String>) {
        let incoming = new_map
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let existing = match self.get_node(node_type, canonical_id) {
            Ok(Some(n)) => n
                .properties
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            // No existing name to weigh: whatever arrived stands, and there is
            // no second name to report as an alias.
            _ => return (incoming, None),
        };
        if incoming.is_empty() || incoming == existing {
            new_map.remove("name");
            return (existing, None);
        }
        // Total order, computed from the two strings alone — nothing here can
        // see which document arrived first.
        let incoming_wins = (incoming.chars().count(), existing.as_str())
            > (existing.chars().count(), incoming.as_str());
        if incoming_wins {
            (incoming, Some(existing))
        } else {
            // Drop the losing name from the map rather than writing it: the
            // rest of the extraction still applies to the survivor.
            new_map.remove("name");
            (existing, Some(incoming))
        }
    }

    /// The two thresholds this node type asks for, read from the schema rather
    /// than from a constant.
    ///
    /// `dynograph-core` has parsed both onto every node type all along —
    /// `fuzzy_threshold` (worth considering) and `auto_merge_threshold` (certain
    /// enough to act) — and until 2026-07-26 ingest read neither, using one
    /// hardcoded 90 instead. That 90 happens to equal the foundation's DEFAULT
    /// auto-merge threshold, so the merging half was accidentally right; what
    /// was missing was the band BELOW it, where a near-match should be reported
    /// and was instead silently created as a second node.
    ///
    /// A type that declares no `resolution` block gets the foundation's defaults
    /// (70 / 90) rather than an invented pair, so the fallback is stated
    /// upstream instead of here.
    fn resolution_thresholds(&self, node_type: &str) -> (u32, u32) {
        self.schema()
            .node_types
            .get(node_type)
            .and_then(|def| def.resolution.as_ref())
            .map_or(
                (DEFAULT_FUZZY_THRESHOLD, DEFAULT_AUTO_MERGE_THRESHOLD),
                |r| (r.fuzzy_threshold, r.auto_merge_threshold),
            )
    }

    /// Cross-id dedup: find an existing same-type node whose `name` matches the
    /// extracted node's name at or above this type's `fuzzy_threshold`
    /// (token-order- and case-insensitive, no embeddings). Returns the best
    /// candidate id + score; the CALLER decides whether that score is a merge or
    /// a question, because the two bands are different acts.
    fn fuzzy_match(
        &self,
        node_type: &'static str,
        new_map: &HashMap<String, Value>,
        extracted_id: &str,
    ) -> Result<Option<(String, u32)>, DynoError> {
        let Some(new_name) = new_map.get("name").and_then(Value::as_str) else {
            return Ok(None);
        };
        let (fuzzy_threshold, _) = self.resolution_thresholds(node_type);
        let mut best: Option<(String, u32)> = None;
        for n in self.scan_nodes(node_type)? {
            if n.node_id == extracted_id {
                continue;
            }
            if let Some(existing_name) = n.properties.get("name").and_then(Value::as_str) {
                let score = token_sort_ratio(new_name, existing_name);
                if score >= fuzzy_threshold && best.as_ref().is_none_or(|(_, b)| score > *b) {
                    best = Some((n.node_id.clone(), score));
                }
            }
        }
        Ok(best)
    }

    /// The structural pass that scoring cannot do: find an existing same-type
    /// node whose name tokens are a strict subset of the extracted node's, or
    /// vice versa. Returns the candidate's id and which side is more specific.
    ///
    /// Runs only when the fuzzy pass found nothing, so a pair already reported by
    /// score is not reported twice. Best = the smallest token-count difference,
    /// which is the closest qualification rather than the most distant one.
    /// The `name` an existing node carries, if it has one. Used by the
    /// distinguisher, which must compare the two NAMES rather than the ids.
    fn existing_name(&self, node_type: &'static str, id: &str) -> Option<String> {
        self.get_node(node_type, id).ok().flatten().and_then(|n| {
            n.properties
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    }

    fn token_subset_match(
        &self,
        node_type: &'static str,
        new_map: &HashMap<String, Value>,
        extracted_id: &str,
    ) -> Result<Option<(String, bool)>, DynoError> {
        let Some(new_name) = new_map.get("name").and_then(Value::as_str) else {
            return Ok(None);
        };
        let new_tokens = name_tokens(new_name);
        if new_tokens.is_empty() {
            return Ok(None);
        }
        let mut best: Option<(String, bool, usize)> = None;
        for n in self.scan_nodes(node_type)? {
            if n.node_id == extracted_id {
                continue;
            }
            let Some(existing_name) = n.properties.get("name").and_then(Value::as_str) else {
                continue;
            };
            let existing_tokens = name_tokens(existing_name);
            // `existing_is_longer` says which id to suggest as the survivor.
            let (matched, existing_is_longer) = if is_token_subset(&new_tokens, &existing_tokens) {
                (true, true)
            } else if is_token_subset(&existing_tokens, &new_tokens) {
                (true, false)
            } else {
                (false, false)
            };
            if !matched {
                continue;
            }
            let gap = new_tokens.len().abs_diff(existing_tokens.len());
            if best.as_ref().is_none_or(|(_, _, b)| gap < *b) {
                best = Some((n.node_id.clone(), existing_is_longer, gap));
            }
        }
        Ok(best.map(|(id, longer, _)| (id, longer)))
    }

    /// `Fragment YIELDED node {action}` — provenance link.
    fn yield_edge(&mut self, st: &mut Integration, node_type: &str, id: &str, action: &str) {
        match self.create_edge(
            edge::YIELDED,
            node::FRAGMENT,
            st.fragment_id,
            node_type,
            id,
            Props::new().set("action", action),
        ) {
            Ok(_) => st.edges_created += 1,
            // A failed provenance edge means a node with no trail back to the
            // work that made it — surfaced, never silent (BL-58).
            Err(e) => st
                .warnings
                .push(format!("provenance edge for {node_type} '{id}': {e}")),
        }
    }

    /// Create one edge, but only between endpoints resolved this run — a
    /// reference to an unknown id is dropped with a reason, never a phantom edge.
    #[allow(clippy::too_many_arguments)]
    fn integrate_edge(
        &mut self,
        st: &mut Integration,
        edge_type: &str,
        from_type: &'static str,
        from_id: &str,
        to_type: &'static str,
        to_id: &str,
        props: Props,
    ) {
        // Redirect endpoints through any fuzzy-merge aliases, so an edge that
        // referenced a merged-away id lands on the canonical node.
        let from = st
            .aliases
            .get(from_id)
            .cloned()
            .unwrap_or_else(|| from_id.to_string());
        let to = st
            .aliases
            .get(to_id)
            .cloned()
            .unwrap_or_else(|| to_id.to_string());

        let mut drop = |reason: String| {
            st.dropped_edges.push(DroppedEdge {
                edge_type: edge_type.to_string(),
                from_id: from.clone(),
                to_id: to.clone(),
                reason,
            });
        };
        if st.created_ids.get(from.as_str()) != Some(&from_type) {
            drop(format!("source '{from}' not a resolved {from_type}"));
            return;
        }
        if st.created_ids.get(to.as_str()) != Some(&to_type) {
            drop(format!("target '{to}' not a resolved {to_type}"));
            return;
        }
        match self.create_edge(edge_type, from_type, &from, to_type, &to, props) {
            Ok(_) => st.edges_created += 1,
            Err(e) => drop(format!("schema rejected: {e}")),
        }
    }
}

/// Minimum `token_sort_ratio` (0–100) for a cross-id fuzzy dedup. High on
/// purpose: below this, resolution creates a new node rather than risk a wrong
/// merge — the uncertain band is the deferred LLM/vector tiebreaker's job.
/// Fallbacks for a node type that declares no `resolution` block. Both mirror
/// `dynograph-core`'s own defaults deliberately — a second opinion about what
/// "close enough" means, held in two places, is how the two drift apart.
const DEFAULT_FUZZY_THRESHOLD: u32 = 70;
const DEFAULT_AUTO_MERGE_THRESHOLD: u32 = 90;

/// The ingress trust boundary for INGEST: every extracted node's text passes
/// through here on its way into the graph.
///
/// This is the one choke point for extraction — the prose came out of a
/// codebase or a document nobody in this session wrote, so it is foreign text
/// by definition, and the smuggling channels it may carry are invisible to
/// whoever reviews the result. Stripping is not the whole job: the removal is
/// pushed onto `warnings` naming the node and the field, because a design whose
/// statements were silently rewritten on the way in is a design nobody can
/// audit (rule 6). See [`crate::sanitize`].
fn sanitize_extracted(
    st: &mut Integration<'_>,
    node_type: &'static str,
    id: &str,
    map: HashMap<String, Value>,
) -> HashMap<String, Value> {
    map.into_iter()
        .map(|(field, value)| match value {
            Value::String(text) => {
                let (clean, report) = crate::sanitize::sanitize_text(&text);
                if !report.is_clean() {
                    st.warnings.push(format!(
                        "sanitized {node_type} '{id}' field '{field}': removed {}",
                        report.describe()
                    ));
                    return (field, Value::String(clean.into_owned()));
                }
                (field, Value::String(text))
            }
            other => (field, other),
        })
        .collect()
}

/// Mutable accumulators for one integration pass — bundled so the integration
/// methods keep small, stable signatures (per the modular-code principle).
struct Integration<'a> {
    fragment_id: &'a str,
    epoch_id: String,
    change_type: ChangeType,
    epoch_ready: bool,
    created_ids: HashMap<String, &'static str>,
    /// extracted id → canonical id, for edges that referenced a fuzzy-merged id.
    aliases: HashMap<String, String>,
    nodes_created: usize,
    nodes_evolved: usize,
    nodes_unchanged: usize,
    fuzzy_merges: Vec<FuzzyMerge>,
    merge_candidates: Vec<MergeCandidate>,
    duplicates_recorded: usize,
    edges_created: usize,
    warnings: Vec<String>,
    dropped_edges: Vec<DroppedEdge>,
}

impl<'a> Integration<'a> {
    fn new(fragment_id: &'a str, epoch_id: String, change_type: ChangeType) -> Self {
        Self {
            fragment_id,
            epoch_id,
            change_type,
            epoch_ready: false,
            created_ids: HashMap::new(),
            aliases: HashMap::new(),
            nodes_created: 0,
            nodes_evolved: 0,
            nodes_unchanged: 0,
            fuzzy_merges: Vec::new(),
            merge_candidates: Vec::new(),
            duplicates_recorded: 0,
            edges_created: 0,
            warnings: Vec::new(),
            dropped_edges: Vec::new(),
        }
    }
}

/// Whether `existing` already holds every property the extraction produced
/// (compared only over the extracted keys, so schema defaults don't read as a
/// change). Equal ⇒ matched-unchanged; differing ⇒ matched-evolved.
fn node_unchanged(existing: &StoredNode, new_map: &HashMap<String, Value>) -> bool {
    new_map
        .iter()
        .all(|(k, v)| existing.properties.get(k) == Some(v))
}

// Schema enum value sets (single source of truth is schema/*.yaml; kept here for
// loud-skip validation of LLM output. A drift here fails a node into `warnings`
// rather than silently dropping it).
const PROVENANCE_VALUES: &[&str] = &[
    "authored",
    "planned",
    "inferred",
    "healed",
    "reconciled",
    "imported",
];
const PRIORITY_VALUES: &[&str] = &["low", "medium", "high", "critical"];
const MEDIUM_VALUES: &[&str] = &[
    "REST",
    "gRPC",
    "json_rpc",
    "event",
    "graphql",
    "cli",
    "library",
    "data",
    "mechanical",
    "electrical",
    "human",
];

/// Schema `method` values. Mirrors schema/verify.yaml; an unknown value is
/// warned about and dropped to the default rather than written, because a
/// method that failed validation would take the whole node down with it.
const VERIFICATION_METHODS: &[&str] = &[
    "test",
    "review",
    "simulation",
    "inspection",
    "measurement",
    "analysis",
    "demonstration",
    "observation",
];
const DEPENDENCY_TYPE_VALUES: &[&str] = &[
    "function_call",
    "data_flow",
    "control_flow",
    "error_flow",
    "physical",
];
const CONSTRAINT_CATEGORIES: &[&str] = &[
    "technical",
    "business",
    "operational",
    "physical",
    "regulatory",
    "budget",
    "schedule",
];
