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

use dynograph_core::{DynoError, Value};
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

/// A structural defect that was reviewed and accepted, with the reason given.
///
/// The mirror of [`ReviewedGap`](crate::detect::ReviewedGap), including the part
/// that matters most: an accepted defect is moved, never deleted, so the
/// judgement stays visible and re-readable when the architecture shifts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewedDefect {
    /// The defect itself, exactly as the detector reports it — absent when the
    /// shape it was accepted about no longer arises (see `retired`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defect: Option<HealIssue>,
    /// Why it was accepted.
    pub reason: String,
    /// The `Decision` node recording the review.
    pub decision_id: String,
    /// The defect id this review was made against. Always present.
    pub defect_id: String,
    /// Set when the review outlived the shape it was made about: kept, because a
    /// real judgement should not vanish because the graph moved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired: Option<String>,
}

/// The `Decision` id recording a defect's review. Namespaced under `heal:` so it
/// can never collide with a gap acknowledgement, whose ids are bare hashes.
fn defect_ack_decision_id(defect_id: &str) -> String {
    format!("decision:ack:{defect_id}")
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
    ///
    /// `None` where NO HONEST MECHANICAL REPAIR EXISTS, and
    /// [`Self::repair_is_a_judgement`] then says why in words.
    /// `req:a-repair-suggestion-never-proposes-fabrication` (accepted
    /// 2026-08-10): a suggestion may reorganise or restore, but must never
    /// assert a relationship nobody stated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix_type: Option<&'static str>,
    /// Present exactly when [`Self::suggested_fix_type`] is `None`: the sentence
    /// a reader needs instead of an operation they should not perform.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_is_a_judgement: Option<&'static str>,
    /// Node ids involved.
    pub affected_ids: Vec<String>,
    /// Nodes in `affected_ids` that ALSO appear in other findings of the same
    /// category, with how many findings each appears in. Empty is the normal
    /// case and the field is omitted from JSON entirely when so.
    ///
    /// This exists because a COUNT makes correlated findings read as
    /// independent corroboration. dev_storyflow, 2026-08-08: a scoped
    /// `detect_defects` returned `in_scope: 5`, every one a duplicate — and one
    /// Decision was in THREE of the pairs while one Requirement was in the
    /// other two. Five findings were two nodes the scorer pairs with
    /// everything. Their words, and the reason this is a field rather than a
    /// nicety: five separate warnings read as five separate judgements
    /// corroborating each other, while "one node the scorer pairs with
    /// everything" reads as what it is — a property of the SCORER, not of the
    /// design. They were mid-stand-down at the time and had to read five
    /// messages by hand to work it out.
    ///
    /// Deliberately a structured field and not a sentence in `message`: the
    /// same fleet reported, the same day, that qualifications buried in prose
    /// are qualifications nothing can act on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hubs: Vec<HubMembership>,
}

/// A node that appears in more than one finding of the same category.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HubMembership {
    /// The node appearing in several findings.
    pub node_id: String,
    /// How many findings of this category it appears in (always >= 2).
    pub in_findings: usize,
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
    ///
    /// ⚠️ READ [`Self::scope`] BESIDE THIS. `propose_heal` takes no target
    /// argument and always sweeps the whole design, so this is a LABEL and
    /// never a scope. It names the design's only Project when there is exactly
    /// one, and the GRAPH when there is more than one — because naming one of
    /// four Projects the caller never chose is what a fleet actually hit
    /// (sb-boss, 2026-08-15): the receipt said `proj:dndwright` while
    /// `issues_addressed` spanned a sibling library and a 121-node cluster.
    ///
    /// reflow2's own design holds exactly one Project, which is precisely why
    /// the self-host could never see this: here the label is accidentally
    /// correct.
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
    /// What applying this proposal would DESTROY, said before the act.
    ///
    /// One entry per merge that deletes a node, naming the doomed node and the
    /// properties that die with it — because a merge keeps only the survivor's
    /// properties, so every value the loser held and the winner does not is
    /// gone with no undo.
    ///
    /// This exists because the cost was reported in exactly the wrong place.
    /// [`HealReport::discarded`] has always said what a merge let go, but a
    /// report is the receipt of an irreversible act; the person deciding
    /// whether to apply reads the PROPOSAL, and the proposal said nothing. A
    /// disclosure that arrives after the deletion is not a disclosure, and
    /// `cap:heal` SATISFIES `req:no-silent-fallback` (accepted, critical),
    /// which this failed.
    ///
    /// It also names the case nothing else can: when both nodes carry the same
    /// provenance the survivor falls to the **smaller id** (BL-29), so the
    /// ALPHABET decides which one dies. That is fine between two equivalent
    /// nodes and indefensible between nodes that differ, and until now nothing
    /// said which had happened. Reported by dev_storyflow 2026-08-08, where the
    /// tiebreak would have deleted a `critical`/`proposed` requirement in
    /// favour of an unrelated `medium`/`accepted` one.
    ///
    /// Empty when the proposal deletes nothing.
    #[serde(default)]
    pub would_destroy: Vec<SkippedOperation>,
    /// 0..1 confidence in the proposal as a whole.
    pub confidence: f64,
    /// True whenever the proposal generates content (discipline 3) **or would
    /// destroy a node** (2026-08-08).
    ///
    /// The second clause is the fix for a defect that read as a feature: this
    /// was `!generated_content.is_empty()`, so a proposal made ENTIRELY of
    /// irreversible node deletions generated no content, reported
    /// `requires_human_review: false` and carried confidence 0.9. That is the
    /// machinery behind the served check-health skill calling merges the safe
    /// mechanical half, and behind a fleet applying ten deletions on its word.
    /// Generating a sentence has always demanded review; deleting a node did
    /// not.
    pub requires_human_review: bool,
    /// What this sweep actually covered, in words.
    ///
    /// ALWAYS the whole design: `propose_heal` takes no target argument, and
    /// `detect_defects()` under it is unscoped. This field exists because
    /// [`Self::target_id`] was read as a scope and is not one — see there.
    #[serde(default)]
    pub scope: String,
    /// How many Projects the swept design holds.
    ///
    /// The number that makes `target_id` readable: at 1 it names the only
    /// candidate, and above 1 it cannot name a chooser because nobody chose.
    #[serde(default)]
    pub projects_in_scope: usize,
    /// How many merge candidates the pair scorer was GIVEN.
    ///
    /// ⚠️ THIS IS THE FIELD THAT MAKES A ZERO READABLE, and it exists because
    /// the absence of it nearly lifted a safety gate. A fleet ran
    /// `propose_heal` as the read-only evidence step of a standing stop on
    /// `apply_heal` — which DELETES NODES — with the lift condition "it pairs
    /// none of the five control pairs". It returned `operations: []` and read
    /// exactly like a pass. It was not: no duplicate-class defect existed, so
    /// the scorer NEVER RAN, and "it paired none of them" was trivially true of
    /// nothing (reported by dev_storyflow's sb-boss, 2026-08-15).
    ///
    /// So `operations: []` with this at 0 means HAD NOTHING TO EXAMINE, and
    /// with this above 0 means EXERCISED AND PROPOSED NOTHING. Those are
    /// different claims and rendered identically before this existed.
    ///
    /// The same distinction `chg:a-null-and-a-vacuous-zero-now-say-which-they-are`
    /// gave scoped `detect_gaps`, which the same fleet named a win — generalised
    /// here by `req:a-report-says-what-it-swept-and-whether-its-checks-ran`.
    #[serde(default)]
    pub merge_candidates_considered: usize,
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

/// Fill each issue's `hubs`: the nodes it shares with OTHER findings of the
/// same category.
///
/// Scoped per category on purpose. A node legitimately appears in a duplicate
/// finding and a disconnected-community finding at once, and saying so would be
/// noise; what misleads is several findings of the SAME kind that are really
/// one node wearing many hats. Counting across categories would turn every
/// well-connected node into a permanent warning, which is the BL-42 shape —
/// a detector that fires on almost everything stops being read.
///
/// Runs once over the collected issues rather than inside each detector, so
/// every category gets it for free and no detector can forget.
fn annotate_hubs(issues: &mut [HealIssue]) {
    // (category, node) -> how many findings of that category name it.
    let mut counts: HashMap<(&'static str, &str), usize> = HashMap::new();
    for issue in issues.iter() {
        // A node named twice by ONE finding is still one finding for it.
        let mut seen = std::collections::HashSet::new();
        for id in &issue.affected_ids {
            if seen.insert(id.as_str()) {
                *counts
                    .entry((issue.category.as_str(), id.as_str()))
                    .or_default() += 1;
            }
        }
    }
    // Collect first, then write: the borrow above is immutable and the counts
    // must be complete before any issue is annotated.
    let hubs: Vec<Vec<HubMembership>> = issues
        .iter()
        .map(|issue| {
            let mut found: Vec<HubMembership> = issue
                .affected_ids
                .iter()
                .filter_map(|id| {
                    let n = *counts.get(&(issue.category.as_str(), id.as_str()))?;
                    (n >= 2).then(|| HubMembership {
                        node_id: id.clone(),
                        in_findings: n,
                    })
                })
                .collect();
            // Deterministic: most-shared first, then id.
            found.sort_by(|a, b| {
                b.in_findings
                    .cmp(&a.in_findings)
                    .then_with(|| a.node_id.cmp(&b.node_id))
            });
            found.dedup_by(|a, b| a.node_id == b.node_id);
            found
        })
        .collect();
    for (issue, found) in issues.iter_mut().zip(hubs) {
        issue.hubs = found;
    }
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
    /// `pub(crate)` since 2026-08-08: DETECT needs the same walk to raise
    /// suspected duplicates as questions, and two implementations of "every
    /// edge of this type" could disagree about which pairs exist — which for
    /// this particular edge is the difference between a pair being asked about
    /// and a pair being reported nowhere at all.
    pub(crate) fn all_edges_of_type(
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

    /// Open structural defects — everything the detectors found that has **not**
    /// been reviewed and accepted.
    ///
    /// The gap side has worked this way since the beginning and the defect side
    /// did not, which was friction found by using reflow2 on itself
    /// (`req:reviewed-defects`, 2026-07-25): six architectural defects, every one
    /// carrying a Decision explaining why it stands, reported identically on every
    /// run. `acknowledge_gap`'s own reasoning applies word for word — "a list that
    /// can never reach zero gets skimmed" — and a genuine seventh defect would
    /// have arrived into a list nobody read carefully.
    ///
    /// Accepted defects move to [`reviewed_defects`](Self::reviewed_defects):
    /// not deleted, not hidden. And because a defect id hashes its category with
    /// its affected set, a review **expires by construction** when the shape it
    /// was made about changes — the new shape gets a new id, which nothing has
    /// accepted yet.
    pub fn detect_defects(&self) -> Result<Vec<HealIssue>, DynoError> {
        let mut open = Vec::new();
        for issue in self.all_defects()? {
            if self.defect_acknowledgement(&issue.id)?.is_none() {
                open.push(issue);
            }
        }
        Ok(open)
    }

    /// Accept a structural defect the user has judged fine, recording WHY.
    ///
    /// The mirror of [`acknowledge_gap`](Self::acknowledge_gap), and deliberately
    /// the same shape: the reason becomes a real `Decision` node so it outlives
    /// the session, and the affected nodes are linked to it so the review is
    /// reachable from the design rather than only from a list.
    ///
    /// Use it when the defect is real and the answer is "yes, and that is
    /// correct" — a single point of failure that is inherent to a single-writer
    /// store, a hub that the architecture deliberately routes through. Not for
    /// silencing something nobody has looked at: the reason is the point, and it
    /// is what a later session reads instead of re-litigating.
    pub fn acknowledge_defect(
        &mut self,
        defect_id: &str,
        affected_ids: &[String],
        reason: &str,
    ) -> Result<String, DynoError> {
        let decision_id = defect_ack_decision_id(defect_id);
        self.create_node(
            node::DECISION,
            &decision_id,
            crate::nodes::Props::new()
                .set("name", format!("Reviewed: {defect_id}"))
                .set(
                    "decision",
                    format!("Accepted the structural defect {defect_id}."),
                )
                .set("rationale", reason)
                // Explicit, because a new Decision is `proposed` since
                // req:decision-status-not-asserted — and this one IS settled: the
                // user just settled it.
                .set("status", "accepted"),
        )?;
        let index = self.node_type_index()?;
        for target in affected_ids {
            if let Some(node_type) = index.get(target) {
                self.governed_by(&node_type.clone(), target, node::DECISION, &decision_id)?;
            }
        }
        Ok(decision_id)
    }

    /// Withdraw a defect's acknowledgement, so it returns to the open list.
    ///
    /// Supersedes the Decision rather than deleting it — the judgement was real
    /// and its record survives being changed (`req:intent-preserved`). Returns
    /// `false` when there was no acknowledgement, which is a no-op and not an
    /// error.
    pub fn withdraw_defect_acknowledgement(&mut self, defect_id: &str) -> Result<bool, DynoError> {
        let decision_id = defect_ack_decision_id(defect_id);
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

    /// Structural defects that were reviewed and accepted, each with its reason.
    ///
    /// Worth re-reading when the architecture shifts. An acknowledgement keyed to
    /// a defect's shape expires when that shape changes, so a review that still
    /// appears here is one that still applies — and a review whose detector or
    /// shape has gone is reported as `retired` rather than vanishing, because
    /// silently shrinking this list would hide a judgement the user made.
    pub fn reviewed_defects(&self) -> Result<Vec<ReviewedDefect>, DynoError> {
        let mut reviewed = Vec::new();
        let mut live = std::collections::HashSet::new();
        for issue in self.all_defects()? {
            if let Some((decision_id, reason)) = self.defect_acknowledgement(&issue.id)? {
                live.insert(issue.id.clone());
                reviewed.push(ReviewedDefect {
                    defect_id: issue.id.clone(),
                    defect: Some(issue),
                    reason,
                    decision_id,
                    retired: None,
                });
            }
        }
        for d in self.scan_nodes(node::DECISION)? {
            let Some(rest) = d.node_id.strip_prefix("decision:ack:") else {
                continue;
            };
            // Only defect acknowledgements — the gap side owns the bare hashes.
            if !rest.starts_with("heal:") {
                continue;
            }
            if live.contains(rest) {
                continue;
            }
            let Some((decision_id, reason)) = self.defect_acknowledgement(rest)? else {
                continue;
            };
            reviewed.push(ReviewedDefect {
                defect_id: rest.to_string(),
                defect: None,
                reason,
                decision_id,
                retired: Some(
                    "The shape this was accepted about has changed, or no current detector \
                     raises it. The decision is kept; nothing is being suppressed by it."
                        .into(),
                ),
            });
        }
        reviewed.sort_by(|a, b| a.defect_id.cmp(&b.defect_id));
        Ok(reviewed)
    }

    /// The accepted review for a defect, if any — `(decision_id, reason)`.
    fn defect_acknowledgement(
        &self,
        defect_id: &str,
    ) -> Result<Option<(String, String)>, DynoError> {
        let decision_id = defect_ack_decision_id(defect_id);
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

    /// Detect the deterministic structural defects (the HEAL catalog subset),
    /// including ones already reviewed. [`detect_defects`](Self::detect_defects)
    /// is the filtered view callers want.
    fn all_defects(&self) -> Result<Vec<HealIssue>, DynoError> {
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
        // What stays: an Artifact attached to nothing. DETECT has no counterpart
        // (its P3 detectors ask about capabilities, not files), so dropping it
        // would lose the finding entirely.
        //
        // BL-176 — WHAT COUNTS AS ATTACHMENT, AND WHY THIS IS AN EXCLUSION LIST.
        // This rule used to be `REALIZES` and nothing else, so an Artifact filed
        // exactly the way the served link-artifacts skill prescribes — a design
        // doc linked with `DOCUMENTS`, an OpenAPI contract with `SPECIFIES` —
        // reported as an orphan. The message ("realizes nothing") was true and
        // the CATEGORY was false: an orphan is a node attached to nothing, and
        // those are attached.
        //
        // Measured in the field before it was fixed: registering 26 ADR/
        // architecture documents took structural defects 13 -> 39, +26 exactly
        // the batch size, false positives 46% -> 82%, with ~730 documents still
        // to come. The reporter STOPPED WORK rather than continue, and refused
        // the workaround of adding a bogus `REALIZES` because it would be a lie
        // at 756x scale. BL-114 had already witnessed it twice more in an
        // unrelated repo (a doc that DOCUMENTS, one PRODUCES-d by a
        // Verification, a corpus that SATISFIES a requirement).
        //
        // THE LIST IS INVERTED ON PURPOSE, and that is the load-bearing choice.
        // Naming the edges that DO attach is what broke: an inclusion list has
        // to be extended every time the vocabulary grows, and until someone
        // remembers, correctly-modelled work reads as a defect. That is this
        // bug, and BL-170's hidden inclusion list is the same shape a second
        // time. Naming the edges that do NOT attach fails the safe way instead:
        // a new design edge counts as attachment the day it is added, and only
        // a new BOOKKEEPING edge needs a line here.
        //
        // Bookkeeping means an edge drawn by the machinery rather than by
        // someone saying what this file is FOR: release membership, change
        // records, extraction provenance and time. Every one of them is present
        // on almost every artifact in a mature graph, so counting them would
        // silence the detector everywhere — which is why this is not the
        // degree-zero rule the Decision arm below uses.
        for art in self.scan_nodes(node::ARTIFACT)? {
            let attached = self
                .outgoing(&art.node_id, None)?
                .into_iter()
                .chain(self.incoming(&art.node_id, None)?)
                .any(|e| !ARTIFACT_BOOKKEEPING.contains(&e.edge_type.as_str()));
            if !attached {
                issues.push(orphan(
                    &art.node_id,
                    "Artifact",
                    "is attached to nothing — no edge says what it is for \
                     (REALIZES / DOCUMENTS / SPECIFIES / PRODUCES / SATISFIES); \
                     release, change, provenance and epoch links do not count",
                    None,
                ));
            }
        }

        // A node reachable from NOTHING — no edges at all, in either
        // direction.
        //
        // Found 2026-08-01 by running check-health and detect-and-ask on
        // reflow2's own design, getting a clean bill from every detector, and
        // then counting zero-degree nodes by hand: `dec:sanitize-spof-accepted`
        // was an ACCEPTED single-point-of-failure disposition with no edges,
        // the only one of five such dispositions not linked to what it
        // disposes. `disconnected_community` cannot see it — that reports
        // clusters of >=2, and a node joined to nothing is never a cluster.
        //
        // It is worse than untidy. A node with no edges cannot be reached by
        // propagation, so it never enters an impact analysis; and for a
        // disposition specifically it can never EXPIRE, because expiry is
        // computed from the affected set (`ver:reviewed-defects`). A
        // conditional judgement silently becomes permanent.
        //
        // ⭐ IT RAN ON `Decision` ALONE UNTIL 2026-08-16, AND THAT NARROWNESS
        // IS WHY THE PASS WHOSE JOB IS STRUCTURAL SOUNDNESS RETURNED A FALSE
        // GREEN (req:a-report-says-what-it-swept-and-whether-its-checks-ran).
        // dev_storyflow's fleet hit it from the outside: `detect_defects`
        // answered clean over a DesignEpoch carrying NO EDGES AT ALL, in two
        // separate packages, through every health call of a session. A node
        // with no edges is the most detectable structural defect there is and
        // needs no judgement to identify — unlike the modularity cluster the
        // same call did report.
        //
        // MEASURED ON REFLOW2'S OWN GRAPH THE DAY THIS WAS GENERALIZED, which
        // is what settles that the narrowness was the bug and not a scoping
        // choice: 75 of 2406 nodes are degree-zero. Only 19 were Decisions,
        // and 12 of those are `decision:ack:` review records excluded below —
        // so SEVEN were visible to this detector. Of the rest, the ones no
        // detector anywhere could see were 48 TemporalFacts naming nothing
        // they are about, 3 DesignEpochs, 3 Fragments, and
        // `ver:the-export-survives-being-read-back` — a Verification counted
        // among the 159 passing that says what it checks to nobody.
        //
        // GENERALIZING IS THE SAFE DIRECTION HERE FOR THE SAME REASON THE
        // ARTIFACT RULE ABOVE INVERTS ITS LIST. Degree-zero is self-limiting —
        // any edge at all silences it — so it cannot grow into a
        // per-convention nag whatever type it runs on, and a new node type
        // gets the check the day it is added rather than the day someone
        // remembers. Two things bound it, and both are enumerated rather than
        // inferred: `DETECT_ASKS_INSTEAD` keeps it off the types gap-surfacing
        // already asks about by name (BL-42), and `zero_degree_finding` grades
        // what the finding MEANS by type instead of flattening it. Grading is
        // not softening — every one of them is reported, and none of them can
        // be mistaken for clean.
        //
        // THE RULE IS DEGREE-ZERO, AND THAT WAS SETTLED BY MEASUREMENT, not by
        // taste. The tempting "narrow" form — an accepted Decision with no
        // incoming GOVERNED_BY — fires on SIX of reflow2's own, five of which
        // have degree 1-3: connected, merely not through that one edge type.
        // That is BL-42's shape exactly, where this same detector reported a
        // well-connected Capability missing one named link, became 20 of 31
        // defects, and had to be cut back to a single rule. Degree-zero fires
        // on ONE, and is self-limiting in a way an edge-named rule is not:
        // any edge at all silences it, so it cannot grow into a per-convention
        // nag.
        //
        // Review records are excluded deliberately, not for convenience:
        // `structure.rs` already keeps `decision:ack:` ids out of the design
        // network because they describe a judgement ABOUT the design rather
        // than how it is structured (`ver:acknowledgement-not-structure`).
        // Every one of them is `accepted` by construction, so including them
        // would fire on all twelve of reflow2's own and be pure noise.
        // Scanned type by type in sorted order, rather than by walking the
        // id→type index, so the issue order is the same in every process. The
        // index is a HashMap and its iteration order is not (BL-58).
        let mut zero_degree_types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        zero_degree_types.sort_unstable();
        for node_type in &zero_degree_types {
            // The Artifact arm above is the SAME rule with an exclusion list
            // rather than a bare degree count, because bookkeeping edges land
            // on nearly every artifact in a mature graph and counting them as
            // attachment would silence it everywhere (BL-176). Running both
            // would report the genuinely-unattached ones twice.
            if node_type == node::ARTIFACT
                || DETECT_ASKS_INSTEAD.contains(&node_type.as_str())
                || UNATTACHED_IS_A_RESTING_STATE.contains(&node_type.as_str())
            {
                continue;
            }
            for n in self.scan_nodes(node_type)? {
                // Review records are excluded deliberately, not for
                // convenience: `structure.rs` already keeps `decision:ack:`
                // ids out of the design network because they describe a
                // judgement ABOUT the design rather than how it is structured
                // (`ver:acknowledgement-not-structure`). Every one of them is
                // `accepted` by construction, so including them would fire on
                // all twelve of reflow2's own and be pure noise.
                if n.node_id.starts_with("decision:ack:") {
                    continue;
                }
                if !self.outgoing(&n.node_id, None)?.is_empty()
                    || !self.incoming(&n.node_id, None)?.is_empty()
                {
                    continue;
                }
                // NOT EVERY ATTACHMENT IS AN EDGE, and assuming it was is the
                // first thing this widening got wrong. Three types carry the
                // node they are about as a REQUIRED, INDEXED PROPERTY rather
                // than a link: `TemporalFact.subject_id`, `Snapshot.target_id`,
                // `Question.gap_id`. A fact naming its subject that way is
                // found by index every time it is needed and is not lost in any
                // sense — it simply never needed an edge.
                //
                // Measured before the correction: 48 of reflow2's own 212
                // TemporalFacts are degree-zero, and reporting them would have
                // been 48 false findings shipped inside the change whose whole
                // subject is instruments that overstate. The unit test caught
                // it, because `subject_id` is required and the fixture could
                // not be built without one.
                //
                // Resolving the id rather than naming the properties is the
                // same inverted-list choice as ARTIFACT_BOOKKEEPING: a type
                // that gains a pointer property is handled the day it does. It
                // also keeps the finding that MATTERS — a pointer to a node
                // that no longer exists resolves to nothing and is still
                // reported, which is the dangling case, not the ordinary one.
                if n.properties.values().any(|v| {
                    v.as_str()
                        .is_some_and(|s| s != n.node_id && index.contains_key(s))
                }) {
                    continue;
                }
                let accepted = n
                    .properties
                    .get("status")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == "accepted");
                let (what, severity) = zero_degree_finding(node_type, accepted);
                issues.push(orphan_at(&n.node_id, node_type, what, None, severity));
            }
        }

        // contradiction — a CONTRADICTS edge (unresolved in this increment).
        //
        // `alignment: supporting` is NOT a contradiction and must not be
        // reported as one. The schema has carried that value from the start —
        // "two decisions/requirements/claims that conflict (or, with
        // alignment=supporting, reinforce)" — and this loop read the edge TYPE
        // and never the property, so every correctly-modelled corroboration
        // came back as a structural defect.
        //
        // Found in use on 2026-07-28: `dec:commands-are-the-exception`
        // QUALIFIES `dec:skills-served` — same reasoning, narrower scope — and
        // recording that relationship the way the schema prescribes turned the
        // graph red. The damage is not the noise: it is that the only ways out
        // were to acknowledge a defect that is not one, or to stop recording
        // corroboration at all. A property the detector ignores is a property
        // that lies.
        for e in self.all_edges_of_type(edge::CONTRADICTS, &index)? {
            if e.properties
                .get("alignment")
                .and_then(Value::as_str)
                .is_some_and(|a| a == "supporting")
            {
                continue;
            }
            // AND THE SAME ARGUMENT ONE PROPERTY ACROSS. The comment above
            // ends "a property the detector ignores is a property that lies",
            // and this loop went on ignoring the ENDPOINTS' `status`.
            //
            // A `rejected` or `superseded` Decision contradicting its successor
            // is the HEALTHY case — it is what "tried in thought, refuted, here
            // is what we did instead" looks like in graph form, and there is
            // nothing to resolve. Reporting it as a structural defect made
            // recording a refutation CORRECTLY cost a warning while burying it
            // in prose cost nothing, so the tool mildly penalised the discipline
            // `rejected` exists to support — and did so at the moment a seat is
            // least likely to push back, having just been told (rightly) that
            // they should have been using `rejected` all along. The cheap wrong
            // response is to delete the node or the edge, which is the erasure
            // the record exists to prevent (dragon Boss, 2026-08-15).
            //
            // ⚠️ EXCLUDED, NOT REBANDED, AND THAT HALF IS DELIBERATELY LEFT
            // OPEN. The report offered two remedies — drop these from the
            // contradiction category, or move them to a "superseded-by" band —
            // and `req:a-detector-reads-the-properties-that-qualify-its-own-finding`
            // records the second as undecided. A new HealCategory widens the
            // defect vocabulary every consumer parses, which is a choice to
            // make deliberately rather than as a side effect of a bug fix. The
            // relationship remains fully readable in the graph: the CONTRADICTS
            // edge is untouched and both endpoints keep their status.
            let withdrawn = |id: &String| -> bool {
                index
                    .get(id)
                    .and_then(|t| self.get_node(t, id).ok().flatten())
                    .and_then(|n| {
                        n.properties
                            .get("status")
                            .and_then(Value::as_str)
                            .map(|s| s == "rejected" || s == "superseded")
                    })
                    .unwrap_or(false)
            };
            if withdrawn(&e.from_id) || withdrawn(&e.to_id) {
                continue;
            }
            let affected = vec![e.from_id.clone(), e.to_id.clone()];
            issues.push(HealIssue {
                id: issue_id(HealCategory::Contradiction, &affected),
                category: HealCategory::Contradiction,
                severity: HealSeverity::Warning,
                message: format!("'{}' and '{}' contradict each other", e.from_id, e.to_id),
                suggested_fix_type: Some("generate_decision"),
                repair_is_a_judgement: None,
                affected_ids: affected,
                hubs: Vec::new(),
            });
        }

        // duplicate — a DUPLICATES edge a HUMAN asserted (fixable by merge).
        //
        // `basis` is checked here rather than at the merge, because a merge that
        // is never proposed cannot be applied by mistake. Only an explicit
        // `asserted` qualifies: absent reads as `suspected`, so an edge that
        // cannot prove somebody meant it never reaches apply_heal's delete.
        // That is dec:ask-not-repair's precondition — "merge is safe only
        // because the endpoints were asserted" — enforced instead of assumed.
        // Mirrors how CONTRADICTS/alignment=supporting is skipped just above.
        //
        // Suspected pairs are NOT dropped: detect_gaps raises each one as a
        // possible_duplicate the user can answer or acknowledge, which is the
        // half of dec:ask-not-repair that says a suspicion is a DETECT gap.
        for e in self.all_edges_of_type(edge::DUPLICATES, &index)? {
            if !matches!(
                e.properties.get("basis").and_then(Value::as_str),
                Some("asserted")
            ) {
                continue;
            }
            let (keep, remove) = self.merge_survivor(&index, &e.from_id, &e.to_id)?;
            let affected = vec![keep, remove];
            // The score, where one was recorded, is printed WITH the verdict.
            // "cover the same ground" alone is unfalsifiable: dev_storyflow had
            // to fetch and read four node pairs to dismiss what a number beside
            // the claim would have dismissed in seconds.
            let because = match e.properties.get("confidence").and_then(Value::as_f64) {
                Some(c) => format!(" (asserted; name similarity {:.0}%)", c * 100.0),
                None => " (asserted)".to_string(),
            };
            issues.push(HealIssue {
                id: issue_id(HealCategory::Duplicate, &affected),
                category: HealCategory::Duplicate,
                severity: HealSeverity::Warning,
                message: format!(
                    "'{}' and '{}' cover the same ground{because}",
                    e.from_id, e.to_id
                ),
                suggested_fix_type: Some("merge"),
                repair_is_a_judgement: None,
                affected_ids: affected,
                hubs: Vec::new(),
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
                suggested_fix_type: Some("generate_entity"),
                repair_is_a_judgement: None,
                affected_ids: affected,
                hubs: Vec::new(),
            });
        }

        self.detect_structural_defects(&mut issues)?;

        annotate_hubs(&mut issues);
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
        //
        // ⭐ IT SPOKE THE WORDS OF UNREACHABILITY AND WALKED SOMETHING NARROWER
        // (req:a-report-says-what-it-swept-and-whether-its-checks-ran, part b).
        // The message read "disconnected from the rest of the design", and
        // dev_storyflow's report is exact: the nodes it named were all
        // REACHABLE by an undirected walk of the graph. Both halves of that are
        // true at once, because `design_network()` is not the graph — it drops
        // nine node types and every review record, and CONTAINS is not a
        // traceability edge. So a node reachable only through AUTHORED_BY, or
        // only downward through containment, is an island HERE and connected
        // THERE, and the sentence claimed the second while computing the first.
        //
        // MEASURED ON REFLOW2'S OWN GRAPH, 2026-08-17: the walk covers 1133 of
        // 2413 nodes. More than half the graph is outside the thing whose
        // absence the message called "the rest of the design".
        //
        // The fix is the message, not the computation, and deliberately not the
        // category key — `disconnected_community` is what consumers match on
        // (it is a documented HEAL category and an ility source), so renaming it
        // to be accurate would break them to fix a sentence. What the finding
        // SAYS now describes the walk that produced it, including how much of
        // the graph that walk held, so a reader can tell "cut off in the design
        // network" from "unreachable in the graph" without reading this file.
        //
        // The other half of their report — "the genuinely unreachable node was
        // never mentioned" — is answered by the degree-zero rule above rather
        // than here, since a node with no edges at all is unreachable in the
        // full graph and is now reported whatever its type.
        let total_nodes = self.node_type_index()?.len();
        let swept = net.node_count();
        let singletons = net
            .component_groups()
            .into_iter()
            .filter(|g| g.len() == 1)
            .count();
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
                        "{} nodes form a cluster with no traceability edge to the main body. \
                         SCOPE OF THIS CHECK: it walked {swept} of {total_nodes} nodes — \
                         Project, epochs, snapshots, change events, facts, fragments, drift \
                         events, dimension records and review records are not in the walk, and \
                         CONTAINS is not a traceability edge — so these nodes may still be \
                         reachable through links it does not follow, and \"cut off here\" is not \
                         \"unreachable in the graph\". {singletons} further node(s) sit alone in \
                         this walk and are not reported by this rule",
                        affected.len()
                    ),
                    // NO SUGGESTION, DELIBERATELY. `generate_bridge` used to sit
                    // here: create edges until the cluster is connected. Where the
                    // separation is CORRECT that fabricates relationships nobody
                    // stated, in order to silence a warning about a separation that
                    // is right — dev_storyflow's gap D, reproduced in this design
                    // twice on the day it was filed. Whether an island is an
                    // accident or a deliberate partition is a judgement the graph
                    // cannot make, and offering an operation implies it can.
                    suggested_fix_type: None,
                    repair_is_a_judgement: Some(
                        "No mechanical repair. Connecting this cluster would assert \
                         relationships nobody stated, which is worse than the finding: \
                         whether the separation is an accident or deliberate is a \
                         judgement, and only a person holds it.",
                    ),
                    affected_ids: affected,
                    hubs: Vec::new(),
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
                    suggested_fix_type: Some("add_redundancy"),
                    repair_is_a_judgement: None,
                    affected_ids: vec![id],
                    hubs: Vec::new(),
                });
            }
        }

        // circular_dependency — parts that depend on each other in a loop, via
        // DEPENDS_ON or through the contracts they provide/consume. Not
        // auto-fixable: breaking a cycle is a design decision (introduce an
        // interface, invert the dependency, go event-driven), so this is
        // reported for a human to resolve rather than repaired.
        for cycle in self.circular_dependencies()? {
            let mut affected = cycle.path.clone();
            affected.sort();
            let path = if cycle.path.len() == 1 {
                format!("'{}' depends on itself", cycle.path[0])
            } else {
                format!("{} → {}", cycle.path.join(" → "), cycle.path[0])
            };
            // BL-141 · say what the detector actually walked. A dependency is
            // either a DEPENDS_ON edge or a shared contract collapsed into one,
            // and until now both printed the same sentence — so a coarse
            // interface model and a tangled call graph were indistinguishable
            // at `critical`. Four false cycles in one adopt pass, zero real.
            let via = if cycle.via_interfaces.is_empty() {
                String::new()
            } else {
                format!(" via {}", cycle.via_interfaces.join(", "))
            };
            // THE DISCRIMINATOR IS THE INTERFACE COUNT, not merely whether
            // contracts were involved. A genuine service-boundary cycle also
            // runs entirely through contracts — but through TWO of them, one
            // per direction (A provides i1 that B consumes, B provides i2 that
            // A consumes). Every one of BL-141's four false cycles ran through
            // exactly ONE Interface that both parts provided *and* consumed,
            // because that node was standing for two contracts at once —
            // `ifc:midi-file` meaning both "MIDI we read" and "MIDI we emit".
            // So the single-interface loop is the case worth naming.
            let basis = match (
                cycle.contracts_only,
                cycle.via_interfaces.len(),
                cycle.via_interfaces.first(),
            ) {
                (true, 1, Some(iface)) => format!(
                    " — every hop runs through the SAME contract, '{iface}', and no DEPENDS_ON \
                     edge is involved. One Interface standing for two contracts (what is read vs \
                     what is written) produces this shape without any code depending on anything \
                     — check the model before changing code"
                ),
                // Two contracts, one per direction, is the shape of a genuine
                // service cycle — AND of BL-141's real case, which is why the
                // medium has to be said out loud. Their loop ran through
                // `ifc:midi-file` and `ifc:wav-audio`, both `data`: a renderer
                // reading MIDI and writing WAV against a transcriber doing the
                // reverse. Two programs sharing two file formats depend on each
                // other at no point in time. Structurally indistinguishable
                // from a REST cycle; only `medium` separates them.
                (true, _, _) if cycle.foundation_media_only => format!(
                    " — every hop is a contract{via}, and EVERY ONE is a library/data medium — \
                     something read or linked against, not called across at run time. Two parts \
                     that read and write the same formats form this loop without depending on \
                     each other at run time. No DEPENDS_ON edge is involved"
                ),
                (true, _, _) => {
                    format!(" — every hop is a contract{via}; no DEPENDS_ON edge is involved")
                }
                (false, 0, _) => " — every hop is a direct DEPENDS_ON edge".to_string(),
                (false, _, _) => {
                    format!(" — mixed: direct DEPENDS_ON edges and contracts{via}")
                }
            };
            // BL-141(b), Anthony's call 2026-08-01. `Critical` means MUST FIX,
            // and a loop that exists only because two parts read and write the
            // same file formats has nothing to fix — it is worth understanding,
            // not worth stopping for. Four such loops were reported `critical`
            // in a single adopt pass and none was real.
            //
            // BOTH conditions are required, and the second is the load-bearing
            // one. `contracts_only` means no hop is a real DEPENDS_ON edge —
            // one real edge anywhere in the loop is a genuine dependency and
            // keeps the whole cycle Critical. `foundation_media_only` means
            // every contract it runs through is `library` or `data`: something
            // linked against or read, not called across at run time.
            //
            // NOT SILENCED, DOWNGRADED — the distinction is the point.
            // Shared-data coupling is sometimes real (two services over one
            // table can be genuinely entangled: a schema change in one breaks
            // the other), so the finding keeps its place, its affected set and
            // its explanation, and loses only the claim that it is an
            // emergency. Deleting the case to fix a presentation problem is
            // what `ver:cycle-basis`'s mutation checks exist to catch.
            //
            // `Interface.medium` defaults to `unspecified`, which is NOT a
            // foundation medium — so silence about the medium can never earn
            // the downgrade, and a design that never classified its boundaries
            // keeps the louder answer. Pinned by its own case.
            let severity = if cycle.contracts_only && cycle.foundation_media_only {
                HealSeverity::Warning
            } else {
                HealSeverity::Critical
            };
            issues.push(HealIssue {
                id: issue_id(HealCategory::CircularDependency, &affected),
                category: HealCategory::CircularDependency,
                severity,
                message: format!("circular dependency: {path}{basis}"),
                suggested_fix_type: Some("break_cycle"),
                repair_is_a_judgement: None,
                affected_ids: affected,
                hubs: Vec::new(),
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
                    // Same rule as the island above: a component connected to
                    // nothing may be genuinely standalone, and wiring it to
                    // something to quiet the finding invents a coupling.
                    suggested_fix_type: None,
                    repair_is_a_judgement: Some(
                        "No mechanical repair. Wiring this component to something \
                         would invent a coupling; whether it is genuinely standalone \
                         or was left unwired by mistake is a judgement.",
                    ),
                    affected_ids: vec![id],
                    hubs: Vec::new(),
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
        // A LABEL, NOT A SCOPE. One Project can be named without ambiguity;
        // more than one cannot, because this tool takes no target and the
        // caller therefore chose none. Falling to the first alphabetically is
        // what made the receipt describe a sibling library's design.
        let projects = self.scan_nodes(node::PROJECT)?;
        let projects_in_scope = projects.len();
        let target_id = match projects.as_slice() {
            [only] => only.node_id.clone(),
            _ => self.graph_id().to_string(),
        };

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
        // Counted where the scorer is actually REACHED, not where duplicates
        // exist: a candidate the strategy filtered out was never examined, and
        // saying otherwise would be a second vacuous number in the same reply.
        let mut merge_candidates_considered = 0usize;

        for issue in self.detect_defects()? {
            if !options.strategy.addresses(issue.severity) {
                continue;
            }
            issues_addressed.push(issue.id.clone());

            match issue.category {
                // The one content-free structural repair.
                HealCategory::Duplicate => {
                    merge_candidates_considered += 1;
                    match merge_op_for(&issue, &index) {
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
                    }
                }
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

        // What this proposal would DESTROY, computed before anyone can apply it.
        // Deliberately derived from the operations actually in the proposal —
        // after the max_operations cap above — so it describes what would
        // happen, never what was considered.
        let mut would_destroy = Vec::new();
        for operation in &operations {
            if let HealOp::Merge {
                keep_type,
                keep_id,
                remove_type,
                remove_id,
            } = &operation.op
            {
                would_destroy.push(SkippedOperation {
                    reference: remove_id.clone(),
                    reason: self.merge_loss_note(keep_type, keep_id, remove_type, remove_id)?,
                });
            }
        }

        let requires_human_review = !generated_content.is_empty() || !would_destroy.is_empty();
        let confidence = if requires_human_review { 0.5 } else { 0.9 };
        // The summary says WHICH KIND of zero, because the fields alone are
        // read by a machine and this line is read by a person deciding whether
        // to apply an irreversible merge.
        let merges = format!(
            "{} structural op(s) from {} merge candidate(s) considered{}",
            operations.len(),
            merge_candidates_considered,
            if merge_candidates_considered == 0 {
                " (none to examine — this zero is not a pass)"
            } else if operations.is_empty() {
                " (examined, none proposed)"
            } else {
                ""
            }
        );
        let summary = format!(
            "{} issue(s) addressed across the whole design ({} Project(s)): {}, {} awaiting \
             generation, {} skipped.",
            issues_addressed.len(),
            projects_in_scope,
            merges,
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
            would_destroy,
            confidence,
            requires_human_review,
            scope: "whole design".to_string(),
            projects_in_scope,
            merge_candidates_considered,
            summary,
        })
    }

    /// What is lost when `remove_id` is merged into `keep_id`, in one sentence
    /// a person can act on before applying.
    ///
    /// Says three things, in the order that matters to someone deciding:
    /// the properties that die, whether the ALPHABET picked the victim, and
    /// nothing else. It deliberately reports long free-text fields by NAME
    /// rather than by value — dumping two statements into a proposal field
    /// would reproduce the defect dev_storyflow reported the same day, where a
    /// finding with nowhere to put its prose ends up unreadable.
    fn merge_loss_note(
        &self,
        keep_type: &str,
        keep_id: &str,
        remove_type: &str,
        remove_id: &str,
    ) -> Result<String, DynoError> {
        let keep = self.get_node(keep_type, keep_id)?;
        let doomed = self.get_node(remove_type, remove_id)?;
        let (Some(keep), Some(doomed)) = (keep, doomed) else {
            // merge_op_for refuses unresolvable endpoints before an operation
            // exists, so this is unreachable for a proposal — but a missing
            // node must not silently read as "nothing is lost".
            return Ok(format!(
                "deleting '{remove_id}': its properties could not be read, so what this destroys is UNKNOWN"
            ));
        };

        // Short, enum-like fields are quoted by value; free text by name only.
        const BY_VALUE: [&str; 6] = [
            "priority",
            "status",
            "provenance",
            "designation",
            "kind",
            "level",
        ];
        let mut valued = Vec::new();
        let mut named = Vec::new();
        let mut keys: Vec<&String> = doomed.properties.keys().collect();
        keys.sort();
        for key in keys {
            let lost = doomed.properties.get(key);
            let kept = keep.properties.get(key);
            if lost == kept {
                continue;
            }
            if BY_VALUE.contains(&key.as_str()) {
                let lost = lost.and_then(dynograph_core::Value::as_str).unwrap_or("-");
                let kept = kept.and_then(dynograph_core::Value::as_str).unwrap_or("-");
                valued.push(format!("{key} '{lost}' -> '{kept}'"));
            } else {
                named.push(key.clone());
            }
        }

        let mut note = format!("deleting '{remove_id}' keeps only '{keep_id}'s properties");
        if valued.is_empty() && named.is_empty() {
            note.push_str("; the two hold identical properties, so nothing is lost with it");
        } else {
            if !valued.is_empty() {
                note.push_str(&format!("; LOST: {}", valued.join(", ")));
            }
            if !named.is_empty() {
                note.push_str(&format!("; also replaced: {}", named.join(", ")));
            }
        }

        // Whether the choice was decided by the design or by the alphabet.
        let rank_of = |n: &crate::StoredNode| {
            provenance_rank(
                n.properties
                    .get("provenance")
                    .and_then(dynograph_core::Value::as_str),
            )
        };
        if rank_of(&keep) == rank_of(&doomed) {
            note.push_str(
                ". THE ALPHABET CHOSE THE VICTIM: both carry the same provenance, so the smaller \
                 id survived (BL-29) — nothing about the design decided which of these two lives",
            );
        }
        Ok(note)
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

/// Edges that do NOT attach an Artifact to the design (BL-176).
///
/// Bookkeeping drawn by the machinery, not by anyone saying what the file is
/// FOR: release membership, the change ledger, extraction provenance, and the
/// time axis. Almost every artifact in a mature graph carries several, so
/// counting them as attachment would silence `orphan_node` everywhere.
///
/// This list is the EXCLUSIONS on purpose. The inclusion form — naming the
/// edges that do attach — is what BL-176 was: it has to be extended whenever
/// the vocabulary grows, and until someone remembers, correct work reads as a
/// defect. Adding a design edge type needs no change here; only a new
/// bookkeeping edge does.
const ARTIFACT_BOOKKEEPING: &[&str] =
    &[edge::INCLUDES, edge::CHANGED, edge::YIELDED, edge::AT_EPOCH];

/// Types the degree-zero rule stays off because DETECT already asks about them
/// — BY NAME, in the golden-thread vocabulary, and in the degree-zero case too.
///
/// This is BL-42 held open on purpose while the rest of the rule generalizes.
/// A Capability with no `ALLOCATED_TO` and a Requirement nothing `SATISFIES`
/// were once reported here AS WELL AS by `unallocated_capability` and
/// `unsatisfied_requirement` — the same finding twice, in two lists, in two
/// vocabularies. Four independent trials complained, and on storyflow it became
/// **20 of 31 defects**, the dominant noise source in the output. An Interface
/// joins them because `unprovided_interface` / `unconsumed_interface` cover it
/// the same way.
///
/// So the generalization is deliberately not "every type": it is every type
/// whose unattachment NOBODY ELSE ASKS ABOUT. That is where the false green
/// actually lived — a DesignEpoch, a TemporalFact, a Verification that says
/// what it checks to nobody are covered by no gap detector at all, and came
/// back clean because this rule ran on `Decision` alone.
///
/// The division is the docs' own: *HEAL fills structure; gap-surfacing elicits
/// meaning.* "Who should own this?" is meaning and belongs to DETECT. "This
/// node is joined to nothing" is structure, needs no judgement, and belongs
/// here — for everything DETECT has no question for.
const DETECT_ASKS_INSTEAD: &[&str] = &[node::REQUIREMENT, node::CAPABILITY, node::INTERFACE];

/// Types whose unattached state is a legitimate resting place, not a defect.
///
/// Both of these are here because firing on them would report the NORMAL state
/// of a correct design as a problem — the exact failure
/// `req:a-deliberate-state-is-not-a-defect` is about, arriving through the
/// detector this change is widening. Widening a sweep without asking that
/// question is how the correct action starts degrading the instrument.
///
/// **Project** is the design's root. A Project alone means the design is EMPTY,
/// which is what every design looks like on its first day and what genesis
/// produces by construction — and the phase rollups (`concept_without_design`)
/// already say so, in the vocabulary of a design that has not started rather
/// than of one that is broken.
///
/// **DesignRule** can bind the PROCESS instead of a node — "we always branch
/// before pushing" governs nobody's Component and is not less of a rule for it.
/// Demanding an edge would push authors to draw a relationship nobody meant,
/// which is the forgery `repair_is_a_judgement` refuses to propose. The
/// questions a rule genuinely owes are asked by DETECT already:
/// `unstated_rule_enforcement` and `unverified_enforced_rule`.
const UNATTACHED_IS_A_RESTING_STATE: &[&str] = &[node::PROJECT, node::DESIGN_RULE];

/// What a degree-zero node of this type means, and how much it matters.
///
/// The RULE is uniform — no edges in either direction — because that is what
/// makes it self-limiting and impossible to argue with. What varies is the
/// CONSEQUENCE, and flattening that would be its own small lie: a Verification
/// that says what it checks to nobody is counted among the passing and someone
/// should look; a TemporalFact naming nothing it is about is a note that will
/// only ever be found by someone already searching for it. Both are reported.
/// The severity says which one to read first, not which one is real.
///
/// `accepted` is the node's status, and only the Decision arm reads it: an
/// accepted Decision that governs nothing CLAIMS to shape the design, where a
/// proposed one is a parked thought that correctly shapes nothing yet.
fn zero_degree_finding(node_type: &str, accepted: bool) -> (&'static str, HealSeverity) {
    match node_type {
        node::DECISION if accepted => (
            "is accepted but governs nothing — nothing links to it, so it shapes no part of the \
             design, cannot appear in any impact analysis, and if it is a disposition it can \
             never expire",
            HealSeverity::Warning,
        ),
        node::DECISION => (
            "has no links yet — a parked decision point, recorded but governing nothing",
            HealSeverity::Info,
        ),
        node::COMPONENT => (
            "is attached to nothing — it holds no capability, provides and consumes no \
             interface, and sits in no containment, so it is a part of nothing",
            HealSeverity::Warning,
        ),
        node::VERIFICATION => (
            "checks nothing — no VERIFIES edge says what it is a check OF, so whatever it \
             reports is counted among the passing and credited to no capability",
            HealSeverity::Warning,
        ),
        node::DESIGN_EPOCH => (
            "is attached to nothing — no snapshot, change or node is recorded against it, so it \
             marks a moment in which, as far as the graph can tell, nothing happened",
            HealSeverity::Info,
        ),
        node::TEMPORAL_FACT | node::SNAPSHOT => (
            "points at nothing that exists — its subject/target id resolves to no node and it \
             carries no edge either, so the record is about something the design no longer has",
            HealSeverity::Info,
        ),
        node::FRAGMENT => (
            "is attached to nothing — nothing says which artifact or corpus it came from, so it \
             cannot be traced back to its source",
            HealSeverity::Info,
        ),
        node::QUESTION | node::CHANGE_EVENT | node::DRIFT_EVENT => (
            "is attached to nothing — this is a provenance record with nothing to be provenance \
             FOR, so it documents no part of the design",
            HealSeverity::Info,
        ),
        node::CONTRIBUTOR => (
            "is attached to nothing — nobody owns, authors or approves anything through them, \
             so the design cannot say what this person is here for",
            HealSeverity::Info,
        ),
        // Everything else — Project, Constraint, Flow, Actor, Environment,
        // Resource, Release, and whatever the schema gains next. A design node
        // joined to nothing is a claim about a design it is not part of,
        // whatever its type, so the DEFAULT is the loud one and the quiet arms
        // above are the enumerated exceptions. That direction is the same
        // choice ARTIFACT_BOOKKEEPING makes one rule over, for the same
        // reason: an inclusion list goes quiet on a type nobody remembered to
        // add, and going quiet is the failure this whole requirement is about.
        _ => (
            "is attached to nothing — no edge connects it to any other part of the design, so \
             it cannot be reached by propagation or appear in any impact analysis",
            HealSeverity::Warning,
        ),
    }
}

/// Build an `orphan_node` issue.
fn orphan(id: &str, type_label: &str, what: &str, fix: Option<&'static str>) -> HealIssue {
    orphan_at(id, type_label, what, fix, HealSeverity::Warning)
}

/// Build an `orphan_node` issue at a stated severity.
///
/// Severity is a parameter because the Decision rule grades by status: an
/// `accepted` Decision reachable from nothing claims to shape the design and
/// shapes nothing, while a `proposed` one is a parked decision point and is
/// merely worth noting.
fn orphan_at(
    id: &str,
    type_label: &str,
    what: &str,
    fix: Option<&'static str>,
    severity: HealSeverity,
) -> HealIssue {
    let affected = vec![id.to_string()];
    HealIssue {
        id: issue_id(HealCategory::OrphanNode, &affected),
        category: HealCategory::OrphanNode,
        severity,
        message: format!("{type_label} '{id}' {what}"),
        suggested_fix_type: fix,
        repair_is_a_judgement: fix.is_none().then_some(
            "No mechanical repair. Linking this node to something would assert a \
             relationship nobody drew — whether it belongs somewhere, or is a parked \
             thought that correctly governs nothing yet, is a judgement.",
        ),
        affected_ids: affected,
        // Filled by annotate_hubs once every issue is collected — a single
        // orphan cannot know what else names its node.
        hubs: Vec::new(),
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
