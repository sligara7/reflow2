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

use dynograph_core::{DynoError, Value};

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
    /// A `DesignRule` whose violations are gate-blocking, with no passing
    /// `Verification` that could detect one.
    ///
    /// A rule the build enforces and nothing checks is a promise the design
    /// makes and cannot keep. It was not merely unreported before — it was
    /// UNASKABLE, because `VERIFIES` was declared `to: "*"`, which accepts a
    /// DesignRule target without modelling one, so no detector treated a rule as
    /// a verifiable thing. Enumerating that edge is what makes this question
    /// expressible; this variant is what makes it asked.
    ///
    /// Scoped to `enforced` rules ON PURPOSE. An advisory rule with no detector
    /// is guidance, and demanding a check for it would produce a list that
    /// cannot reach zero — the failure that retired `unverified_artifact` and
    /// `unexpected_coupling`. Note `enforced` DEFAULTS to true, so a rule that
    /// never said otherwise is claiming to be gate-blocking and is answered here.
    ///
    /// The counter-argument, from the same report that proposed this and worth
    /// keeping in view: *"a graph node green-washes exactly like a document"*.
    /// Attaching any Verification silences this gap, so the check must actually
    /// pass — and a passing check that tests nothing is still a lie the graph
    /// cannot see. The detector informs; it does not certify.
    UnverifiedEnforcedRule,
    /// A `DesignRule` that has not said whether breaking it is gate-blocking.
    ///
    /// The third state `enforced` gained when its default was removed
    /// (`dec:does-enforced-default-to-gate-blocking`). Absent no longer means
    /// enforced, so it must not silently mean advisory either — that would
    /// simply move the unchosen claim to the other side. It means nobody has
    /// said, and this ASKS.
    ///
    /// Deliberately a separate finding from `unverified_enforced_rule` and
    /// deliberately gentler: "decide what this rule is" costs one word, while
    /// "prove this rule" costs a detector. Collapsing them is precisely what
    /// made the pre-2026-08-08 reading wrong, where a rule nobody had thought
    /// about was billed for a check nobody agreed to.
    UnstatedRuleEnforcement,
    /// A build exists and the design records no adopted conventions at all.
    ///
    /// THE SIBLING THAT MAKES GOVERNANCE SELF-SEEDING, and without it the rest
    /// of this family cannot start. Every other rule finding fires on a rule
    /// that ALREADY EXISTS — so on a design where nobody ever wrote one down,
    /// they are all silent and governance is invisible forever. A detector that
    /// only speaks about recorded things can never ask for the thing itself;
    /// the same blind spot let a zero-edge requirement hide from HEAL until
    /// something finally pointed at it.
    ///
    /// KEYED ON ARTIFACTS, NOT COMPONENTS, and the difference is the whole
    /// argument. At genesis there are no conventions yet and asking gets a
    /// shrug — a design on paper has not chosen how it will be built. Once real
    /// FILES exist they already follow conventions, written down or not, which
    /// is exactly the adopt case: a system that exists has de facto rules living
    /// in the code that nobody recorded. So this waits for a build.
    ///
    /// Fires once, at the project level, and is acknowledgeable — "no adopted
    /// conventions" is a legitimate answer for a prototype, and a detector that
    /// cannot reach zero teaches you to skim it.
    BuildWithoutGovernance,
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
    /// Components that depend on each other with **no contract recorded between
    /// them** — the seam exists in the build and is written down nowhere.
    ///
    /// THE OPPOSITE DIRECTION FROM THE TWO ABOVE, and that is why it had to be
    /// built. [`UnprovidedInterface`](Self::UnprovidedInterface) and
    /// [`UnconsumedInterface`](Self::UnconsumedInterface) both require an
    /// `Interface` to exist already; a design that has never declared one is
    /// invisible to both. `maturity_report`'s `seams` band has always computed
    /// exactly this set — `couplings - declared` — divided it into a ratio and
    /// dropped the difference on the floor
    /// (`req:an-undeclared-coupling-is-named-not-just-counted`).
    ///
    /// IT NAMES THE PAIR AND ASKS. It never drafts the Interface: reflow2 can
    /// see *that* two components are coupled and cannot know *what* the contract
    /// is — the medium, the payload, the auth, the direction. Proposing one
    /// would be the fabrication `req:a-repair-suggestion-never-proposes-fabrication`
    /// forbids.
    ///
    /// AGGREGATE, and deliberately so: reflow2's own design would emit 73 of
    /// these individually. `chg:gap-reporting-at-corpus-scale` records the same
    /// shape biting at an order of magnitude, and
    /// [`UnexpectedCoupling`](Self::UnexpectedCoupling) was retired for
    /// flooding. One question keyed on the rule, listing the pairs, is the
    /// BL-73 answer.
    UndeclaredSeam,
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
    /// A Component contained by more than one parent — the spine is a tree.
    MultipleParents,
    /// A Component at the root of the spine declaring a level something else
    /// claims to be above, so "the top tier" has two disagreeing answers.
    LevelSpineDisagreement,
    // Decision points (axis of design space — BL-70)
    /// A *proposed* Decision holding ≥2 registered alternatives — an open fork
    /// the design has not chosen between. The "missing teeth" BL-70 named:
    /// nothing else makes a proposed Decision gate anything, so a held-open
    /// analysis of alternatives would sit undecided forever without a nudge.
    /// Compare them (`analyze_alternatives`), then `collapse_decision`.
    UndecidedDecisionPoint,
    /// Proposed Decisions carrying no relation to anything and no note saying
    /// somebody looked — **the ideas nobody has opened**.
    ///
    /// The third leg of `dec:idea-do-ideas-form-a-graph-or-only-a-list`
    /// (accepted 2026-08-21). Vocabulary reaches a user's design only with a
    /// typed tool, an instruction, AND a detector that notices its absence;
    /// this graph had the first two and 145 ideas joined by 12 edges.
    ///
    /// AGGREGATE, and not for tidiness. Per-node it would have fired 115 times
    /// on the day it shipped — every one of them correct, and the whole
    /// category filtered by the end of the week. One finding names the practice
    /// and lists the ideas.
    ///
    /// # Why it does not fire on an idea somebody judged
    ///
    /// `no_relation_note` is what separates "nobody looked" from "somebody
    /// looked and there was honestly nothing". Without that distinction this
    /// detector would report the people who did the work, which is worse than
    /// not detecting at all — it makes the careful answer indistinguishable
    /// from the missing one and then complains about both.
    ///
    /// # Why it does not fire at capture
    ///
    /// Detection is unconditional; the INVITATION waits for a boundary
    /// (`req:detecting-is-not-asking`). The brainstorm skill forbids running
    /// detect-and-ask over brainstormed nodes, because asking someone to firm
    /// up what they deliberately left soft teaches them that thinking out loud
    /// has a cost. The gap is computed always and PUT at a capture-session or
    /// an increment close.
    UnreviewedIdeas,
    // Verification vs validation (BL — edge-orthogonality)
    /// Capabilities with a passing verification-kind check but no passing
    /// validation-kind check — built to spec, but nothing confirms they meet the
    /// operational intent ("built right" without "the right thing"). The reader
    /// that earns `Verification.kind` its keep after the `VALIDATES` edge was
    /// retired (`dec:edge-orthogonality`). One project-level rollup, not N
    /// per-capability alarms (the BL-73 lesson).
    UnvalidatedCapability,
    /// A **key performance parameter** — inviolable intent — that nothing is
    /// bound to. A KPP constraining nothing is a comment: it can never be
    /// violated because it touches nothing, so it will sit green forever while
    /// asserting something important. Ranked above ordinary gaps.
    KppUnbound,
    /// A KPP whose budget rollup has gone past its threshold: the stated
    /// contributions sum to the wrong side of the limit. The one KPP violation
    /// that is arithmetic rather than judgement.
    KppBreached,
    /// An **accepted** Decision whose blast radius reaches what a KPP binds —
    /// a downstream choice that may have traded away something untradeable.
    /// Surfaced for review, never asserted as a violation: whether the decision
    /// actually costs the KPP is semantic, and calling it broken automatically
    /// would be the judgement `dec:report-dont-judge` forbids.
    KppContradicted,
    /// A `Release` with no `AT_EPOCH` edge, so it names no point on the time
    /// axis (BL-122).
    ///
    /// THE INVISIBILITY IS THE POINT. `changelog_point` resolves such a release
    /// to a position with no sequence, so the changelog window for it has no
    /// lower bound and silently widens to the beginning of the design — and a
    /// matching name plus an existing epoch node make the missing edge look
    /// exactly like a present one to every reader, human or otherwise. That is
    /// how `rel:v0190` was cut without its edge four hours before
    /// `changelog_view` needed it, and how `v0.17.0` still lacks one while
    /// `v0.18.0`'s commit message boasted of not repeating the fault.
    ///
    /// A gap rather than a defect because WHICH epoch is a judgement only a
    /// human can make, and a release genuinely cut before the epoch spine
    /// existed is a real state to accept rather than repair.
    ReleaseWithoutEpoch,
    /// A `Release` that is pinned to an epoch and deployed, and yet `INCLUDES`
    /// nothing — a release record claiming to have shipped nothing.
    ///
    /// The schema has always said what this means. `INCLUDES`' own extraction
    /// hint ends: *"The as-released view is read off these edges; a Release with
    /// none is a version number, not a manifest."* Nothing enforced it.
    ///
    /// MEASURED, AND THE COST WAS A SHIPPED RELEASE. `release_includes_all`
    /// defaults to `apply: false` — correctly, so you can read what a release is
    /// about to package before packaging it. Its reply carries `"applied": false`
    /// beside `"added": 304`, and 304 is a FORECAST. On 2026-08-21 that reply was
    /// read as an accomplishment twice, and v0.38.0 was tagged, built, published
    /// and asset-verified with 0 `INCLUDES` against v0.37.0's 275. The binaries
    /// were always right; the design's record of what they contained was empty.
    ///
    /// EVERY CHECK THAT RAN PASSED. `isError` was false both times — nothing had
    /// failed. `reflow2_check` was green. `release_report` was never called. The
    /// hole is that a dry run and a write are indistinguishable to all of them,
    /// which is `rule:success-is-read-from-the-authoritative-object` failing on a
    /// third surface after `gh pr merge`. This detector is the reading of the
    /// authoritative object, done by something that cannot forget to do it.
    ///
    /// # Why it does not consult `status`, and why that is the whole rule
    ///
    /// The sibling [`GapSource::ReleaseWithoutEpoch`] exempts a `planned`
    /// release, because an epoch is minted at the cut and asking beforehand
    /// alarms on correct work. Copying that exemption here would have missed the
    /// defect that motivated the rule: **`rel:v0380` was tagged, published and
    /// deployed while its `status` still said `planned`**, and still did when
    /// this was written. A status field records what somebody remembered to
    /// write down; `DEPLOYED_TO` records that the thing went out. This rule
    /// keys on the structure and never reads the status, so a release cannot
    /// escape it by being mislabelled — which is precisely how the one real
    /// instance would have escaped.
    ///
    /// A gap rather than a defect: what a release ships is a judgement (and a
    /// genuinely contentless release — a re-tag, a docs-only republish — is a
    /// real state to accept), so it is asked, never repaired. `apply_heal` must
    /// not invent a manifest; `cap:no-fabricated-repair` forbids exactly that.
    ReleaseWithoutManifest,
    /// A decomposed Requirement whose children have never been checked against
    /// what the parent held: what did the parent say that no child says?
    ///
    /// THE HOLE IT COVERS IS IN THE ARITHMETIC THIS PROJECT TRUSTS MOST.
    /// `report.rs` treats a parent as delivered exactly when every child is, and
    /// nothing anywhere asks whether the children, taken together, amount to the
    /// parent. So a requirement split into two children addressing a tenth of it
    /// reports `delivered` the moment both close — inside `req:completion-computed`,
    /// the number the design uses as ground truth precisely because it is computed
    /// from the golden thread rather than asserted.
    ///
    /// THE MECHANISM OF THE LOSS IS GENERAL: a decomposition by SUBJECT drops
    /// what belongs to no single subject. Cross-cutting content has no natural
    /// child to land in, so it lands in none. Measured instance (2026-07-28,
    /// reviewing reflow's `01-systems_engineering.json`): a monolithic workflow
    /// was split into 01a–01f, and `context_management` and `self_improvement` —
    /// present in all six monolithic workflows — are absent from every one of the
    /// seven children. Nothing noticed for months, because a roll-up only ever
    /// asks whether each child is done.
    ///
    /// ASKS, NEVER JUDGES (`dec:report-dont-judge`). It does not refuse a
    /// decomposition and it does not put an LLM in charge of sufficiency. It also
    /// never names what is missing: reflow2 can see THAT the question is
    /// unanswered and cannot know WHAT fell between the children, and a plausible
    /// wrong guess is worse than the question because it gets recorded as the
    /// answer (`cap:no-fabricated-repair`).
    ///
    /// DECOMPOSITION ONLY, never derivation — a DERIVED requirement adds new
    /// technical necessity and is not expected to cover anything
    /// (`req:requirement-lineage`). Keying on the `DECOMPOSES` edge gets that for
    /// free: derived requirements hang off the Decision that created them and
    /// carry no such edge.
    DecompositionCoverage,
    /// Recorded changes that never say WHICH AXIS they are on — whether the
    /// SYSTEM changed, or only the design's KNOWLEDGE of it did
    /// (`ChangeEvent.subject`).
    ///
    /// # Why an absence detector rather than a consistency one
    ///
    /// This is the leg that decides whether a piece of vocabulary ever reaches
    /// a user's design at all. A typed tool writes it and an instruction says
    /// when — and with nothing noticing its ABSENCE, the loop that exists to
    /// surface gaps never asks, so the field stays empty in every project
    /// forever. `fact:vocabulary-needs-three-legs-and-a-users-project-gets-none-of-it`
    /// measured that shape across three cases; this is the first detector
    /// written to close it deliberately.
    ///
    /// It must therefore fire where the vocabulary has NEVER been used, which
    /// is exactly what `decomposition_coverage` cannot do: that one keys on a
    /// node already carrying a `DECOMPOSES` edge, so a project that never
    /// decomposed anything reads clean. Keying on the population of
    /// ChangeEvents instead means a design that has recorded changes and never
    /// stated an axis is the loudest case rather than the silent one.
    ///
    /// # What it is not
    ///
    /// Not a claim that any change is wrong. The changes are recorded and
    /// findable; what is missing is a distinction nobody can recover later,
    /// because the person who knew it has moved on. And not a demand: a
    /// project whose events come from bulk ingest genuinely cannot know the
    /// axis, which is why one acknowledgement settles the whole practice.
    ChangeAxisUnstated,
    /// A Requirement whose every delivering artifact is declared `internal` —
    /// a stated need that nothing a CONSUMER can reach delivers.
    ///
    /// The product form of `rule:reflow2-is-built-for-other-projects-not-for-itself`,
    /// and it serves `req:work-says-whether-it-reaches-a-consumer`. The failure
    /// it names is universal and LOOKS LIKE COMPLETION: find a hole, patch it
    /// in the project's own machinery, mark the need met, and leave every
    /// consumer with the hole. The repo goes green and nothing is true of
    /// anybody else.
    ///
    /// # Never inferred from a path
    ///
    /// `Artifact.audience` is DECLARED. A first sketch classified by directory
    /// — `.github/` internal, `crates/` shipped — which encodes one project's
    /// layout and would make this detector useful to exactly one repository,
    /// which is the failure the rule behind it forbids.
    ///
    /// # It reports having nothing to run on
    ///
    /// If no artifact in the design declares an audience, this yields NO
    /// finding and the population is reported as unclassified instead. A
    /// detector answering zero over an empty population reads exactly like one
    /// that ran clean — the trap `loop_status` already refuses an unknown
    /// contributor_id to avoid.
    InternalOnlyDelivery,
    /// An Interface designated `published` or `both` that no passing check
    /// verifies — a promise others are entitled to rely on, with no evidence
    /// it holds.
    ///
    /// The sibling of `UnverifiedEnforcedRule`, one layer over. Both name an
    /// obligation nobody can observe compliance with; the rule claims the power
    /// to fail a build, and a published contract claims a consumer may depend
    /// on it. Neither can ride a carrier one hop away: an interface is not
    /// allocated anywhere, so either something checks it or nothing does.
    ///
    /// # It could not have existed before 2026-08-21
    ///
    /// `VERIFIES` could not reach an `Interface` at all until then
    /// (`fact:a-contract-is-the-one-thing-reflow2-cannot-attach-evidence-to`),
    /// so every interface in every design read as unverified and a detector
    /// would have fired on all of them, correctly and uselessly. The edge
    /// landed, and exactly ONE check has been drawn since — which is what a
    /// vocabulary with no detector looks like from the outside.
    ///
    /// # Narrow by construction
    ///
    /// It fires only on what a design has already CHOSEN to publish, so it
    /// cannot nag a design that has not started. `internal` boundaries are
    /// plumbing the owner may change freely and are never asked about here.
    UnverifiedPublishedContract,
    /// An Interface designated `published` or `both` whose AGREEMENT AXES are
    /// not all filled in — the structured fields two systems have to compare
    /// before anyone can say whether they are compatible.
    ///
    /// Serves `req:interface-spec-complete`, accepted and unimplemented until
    /// now. Its point is not completeness for its own sake: two designs cannot
    /// be checked for INCOMPATIBILITY at a seam unless the seam is described in
    /// comparable terms, so `mirror_surface` can link two designs and still not
    /// tell you they disagree.
    ///
    /// # It reports, it does not judge
    ///
    /// Some axes are genuinely meaningless for some boundaries — a library
    /// linked into its callers has no `endpoint`, a one-way data feed has no
    /// `error_model` a consumer parses. This names WHICH axes are unset and
    /// leaves applicability to the design (`dec:report-dont-judge`), because a
    /// list of required fields would be this tool deciding what a mechanical,
    /// human or physical interface must look like.
    ///
    /// # 🛑 It cannot ask for the sixth characteristic
    ///
    /// `req:interface-spec-complete` names six: protocol, data types,
    /// operations, auth, errors, and PERFORMANCE AND CONSTRAINTS — rate limits,
    /// concurrency, timeouts. The schema has fields for the first five and
    /// NONE for the sixth, so a design can satisfy this finding completely and
    /// still not say what it promises under load. Stated here because a
    /// detector that quietly checks five sixths of a requirement and reports
    /// clean is the "green gate over what it does not cover" failure.
    IncompletePublishedContract,
    /// The design has boundaries and has designated NONE of them `published` —
    /// so this project either publishes nothing on purpose, or nobody has
    /// classified its boundaries, and `Interface.designation` cannot tell those
    /// apart because it DEFAULTS to `internal`.
    ///
    /// # Why the default forces a separate finding
    ///
    /// `Artifact.audience` is undefaulted, so silence there is legible as
    /// silence. `designation` is not: an unclassified boundary and a
    /// deliberately internal one are stored identically, which the schema says
    /// in as many words and which `pair_designs` already refuses to paper over.
    /// So `UnverifiedPublishedContract` returning zero is genuinely ambiguous,
    /// and reporting it as clean would be the "nothing to run on reads like a
    /// pass" failure the loop refuses everywhere else.
    ///
    /// ONE aggregate finding, low severity, acknowledgeable — never one per
    /// boundary. The question is about the project's posture, asked once.
    NoPublishedBoundary,
}

impl GapSource {
    /// Stable snake_case key (used in the gap id hash and for display).
    pub fn as_str(self) -> &'static str {
        match self {
            GapSource::UnverifiedPublishedContract => "unverified_published_contract",
            GapSource::IncompletePublishedContract => "incomplete_published_contract",
            GapSource::NoPublishedBoundary => "no_published_boundary",
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
            GapSource::UnverifiedEnforcedRule => "unverified_enforced_rule",
            GapSource::UnstatedRuleEnforcement => "unstated_rule_enforcement",
            GapSource::BuildWithoutGovernance => "build_without_governance",
            GapSource::ComponentGranularityVerification => "component_granularity_verification",
            GapSource::UnverifiedArtifact => "unverified_artifact",
            GapSource::UnprovidedInterface => "unprovided_interface",
            GapSource::UnconsumedInterface => "unconsumed_interface",
            GapSource::UndeclaredSeam => "undeclared_seam",
            GapSource::UnexpectedCoupling => "unexpected_coupling",
            GapSource::DecliningDimension => "declining_dimension",
            GapSource::MissingIntermediateLevel => "missing_intermediate_level",
            GapSource::LevelMismatch => "level_mismatch",
            GapSource::OrphanLevel => "orphan_level",
            GapSource::MultipleParents => "multiple_parents",
            GapSource::LevelSpineDisagreement => "level_spine_disagreement",
            GapSource::UndecidedDecisionPoint => "undecided_decision_point",
            GapSource::UnreviewedIdeas => "unreviewed_ideas",
            GapSource::UnvalidatedCapability => "unvalidated_capability",
            GapSource::KppUnbound => "kpp_unbound",
            GapSource::KppBreached => "kpp_breached",
            GapSource::KppContradicted => "kpp_contradicted",
            GapSource::ReleaseWithoutEpoch => "release_without_epoch",
            GapSource::ReleaseWithoutManifest => "release_without_manifest",
            GapSource::DecompositionCoverage => "decomposition_coverage",
            GapSource::ChangeAxisUnstated => "change_axis_unstated",
            GapSource::InternalOnlyDelivery => "internal_only_delivery",
        }
    }

    /// Is this an AGGREGATE gap — one per project, whose `affected_ids` are the
    /// whole population the rule ranges over rather than the specific nodes the
    /// finding is about?
    ///
    /// It matters because [`gap_id`] hashes the affected set, so that a gap
    /// whose subject moved gets a fresh judgement. For a per-node gap that is
    /// exactly right. For an aggregate it is a defect: the population changes
    /// every time the design grows, so the id moves, the acknowledgement cannot
    /// carry, and the same standing judgement must be re-recorded forever
    /// (`req:set-scoped-acknowledgement-keys-on-its-rule`). `unvalidated_capability`
    /// had been re-acknowledged about twenty times — at 33, 34, 35 … 65, 67, 68
    /// capabilities — always with the same disposition.
    ///
    /// NOT keyed on [`GapScope::Project`], which is the trap: `unsatisfied_requirement`
    /// and `status_contradiction` are project-SCOPED but per-requirement, and
    /// `kpp_breached` is per-KPP. Treating scope as the test would collapse every
    /// unsatisfied requirement in the design into one gap sharing one judgement.
    ///
    /// THE TRADE-OFF, stated because it is real: a stable id means a capability
    /// added later is covered by the earlier judgement without a fresh look. That
    /// is what a STANDING disposition means — "validation is tracked in the trials
    /// programme" is a claim about the practice, not about any one capability — and
    /// the growth stays visible without the churn, because a review records the
    /// count it was made at in its reason while the live gap's title carries the
    /// count now.
    fn is_aggregate(self) -> bool {
        match self {
            GapSource::UnvalidatedCapability => true,
            // Per-rule, not per-pair: reflow2's own design has 73 undeclared
            // couplings, and every consumer arrives with a comparable set. The
            // standing judgement being accepted here is "our boundaries are
            // recorded somewhere other than the graph", which is a claim about
            // the practice, not about any one pair — and per-pair keying would
            // expire it every time a component gained a dependency.
            GapSource::UndeclaredSeam => true,
            // Aggregate for the same reason: the finding is about the PRACTICE
            // of leaving ideas unconnected, not about any one idea. Per-node
            // keying would expire the standing judgement every time somebody
            // had a thought, which is the trap unvalidated_capability fell into
            // and was re-acknowledged twenty times for.
            GapSource::UnreviewedIdeas => true,
            // Aggregate, and the reason is the same practice argument: the
            // finding is about a design that does not record the axis, not
            // about any one change. Per-event keying would expire the standing
            // judgement on every single write — which is worse here than
            // anywhere else, because ChangeEvents are the fastest-growing node
            // type in any active design.
            GapSource::ChangeAxisUnstated => true,
            // PER-REQUIREMENT, not aggregate, and the split matters: accepting
            // "this need is served by our own tooling on purpose" is a claim
            // about ONE need, and must not also accept the next requirement
            // that lands in the same state.
            GapSource::InternalOnlyDelivery => false,
            // PER-INTERFACE, same reason: "this boundary is exercised by a
            // conformance suite we run elsewhere" is a claim about ONE
            // contract, and must not silently cover the next surface this
            // design publishes.
            GapSource::UnverifiedPublishedContract => false,
            // PER-INTERFACE too: "this boundary's schema lives in the OpenAPI
            // file rather than in the graph" is a claim about ONE contract.
            GapSource::IncompletePublishedContract => false,
            // AGGREGATE — a claim about POSTURE rather than about any node.
            // "This design publishes nothing on purpose" must survive somebody
            // adding a boundary, or the acknowledgement expires on every write.
            //
            // 🛑 AND THIS DECLARATION IS CURRENTLY UNOBSERVABLE, WHICH IS
            // RECORDED RATHER THAN HIDDEN. Flipping it to `false` fails no
            // test, because the finding names NO nodes: `gap_id`'s per-node
            // branch hashes an empty list, so the id is stable either way. It
            // is kept at `true` because it states what the finding IS, and
            // because the day anyone gives it `affected_ids` the flag becomes
            // the only thing keeping the acknowledgement alive — but a reader
            // should not mistake it for protection that is being exercised
            // today. Found by mutation, like the dead guard in
            // `detect_internal_only_delivery` above.
            GapSource::NoPublishedBoundary => true,
            // Everything else names the nodes the finding is actually about, so a
            // change to that set SHOULD expire the judgement. Listed exhaustively
            // rather than with a wildcard: a new aggregate detector must come here
            // and decide, instead of silently inheriting per-node keying.
            GapSource::DesignWithoutIntent
            | GapSource::ConceptWithoutDesign
            | GapSource::DesignWithoutBuild
            | GapSource::BuildWithoutVerification
            | GapSource::NoDeployOperate
            | GapSource::UnsatisfiedRequirement
            | GapSource::UnmotivatedCapability
            | GapSource::PossibleDuplicate
            | GapSource::UnallocatedCapability
            | GapSource::UnrealizedCapability
            | GapSource::FailingVerification
            | GapSource::UnresolvedDrift
            | GapSource::UnreleasedComponent
            | GapSource::StatusContradiction
            | GapSource::UnverifiedCapability
            // Per-rule: the finding names the one rule with no detector, so
            // accepting "this rule is checked by review, not by code" must not
            // also accept the next enforced rule somebody writes.
            | GapSource::UnverifiedEnforcedRule
            | GapSource::UnstatedRuleEnforcement
            | GapSource::BuildWithoutGovernance
            | GapSource::ComponentGranularityVerification
            | GapSource::UnverifiedArtifact
            | GapSource::UnprovidedInterface
            | GapSource::UnconsumedInterface
            // (UndeclaredSeam is aggregate — matched above.)
            | GapSource::UnexpectedCoupling
            | GapSource::DecliningDimension
            | GapSource::MissingIntermediateLevel
            | GapSource::LevelMismatch
            | GapSource::OrphanLevel
            | GapSource::MultipleParents
            | GapSource::LevelSpineDisagreement
            | GapSource::UndecidedDecisionPoint
            | GapSource::KppUnbound
            | GapSource::KppBreached
            | GapSource::KppContradicted
            // Per-release: the finding names the one release missing its edge,
            // so accepting "v0.17.0 predates the epoch spine" must not also
            // accept the next release cut without one.
            | GapSource::ReleaseWithoutEpoch
            // Per-release for the same reason as its sibling above, and with a
            // sharper one: the recorded answer is "v0.36.0 really did ship
            // nothing new". That is a claim about ONE release and must never
            // carry to the next cut that forgets `apply: true`.
            | GapSource::ReleaseWithoutManifest
            // Per-decomposition, and load-bearing rather than incidental. The
            // recorded answer — "these children cover the parent, except X" — is
            // a claim about THESE children. Adding or removing one makes the
            // earlier answer an answer to a different question, so the id MUST
            // move with the child set and the question MUST be re-asked.
            | GapSource::DecompositionCoverage => false,
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
    /// The specific thing to answer, in the READER's vocabulary — their
    /// field's words kept, reflow2's own internal vocabulary dropped.
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
        // ⭐ THE INSTRUCTION USED TO SAY "for a non-engineer. No
        // graph/systems-engineering jargon" — and it contradicted this tool's
        // OWN description, which says a systems engineer wants `interface` and
        // `verification` kept. The description had learned the lesson; the
        // instruction it handed out had not, and the instruction is the half an
        // agent obeys. Corrected 2026-08-20 on Anthony's word
        // (`fact:gap-to-prompt-tells-the-agent-to-strip-the-vocabulary-its-own-description-says-to-keep`).
        //
        // WHY IT MATTERED MORE THAN ITS SIZE: this is the single point where a
        // detector's vocabulary becomes a human's. "No jargon" flattens a
        // beamline scientist, a flight-test engineer and an acquisitions officer
        // toward one generic default, producing a question that is easy to read
        // and impossible to answer precisely — put to the one person who could
        // have answered it precisely. A persona is a VOCABULARY SWAP, not a
        // difficulty dial.
        let request = LlmRequest::new(format!(
            "Rewrite this design gap as one question the reader can answer, in THEIR \
             vocabulary. Match their domain: keep the words their field uses — a systems \
             engineer wants `interface` and `verification` kept, a clinician wants theirs. \
             Drop only reflow2's own internal vocabulary (gap-source names, node and edge \
             types), because that is jargon nobody outside this tool shares. Swapping \
             vocabulary is not simplifying, and simplifying is not what is wanted here. \
             Return only the question.\n\n\
             Gap: {}\nWhy it matters: {}",
            self.title, self.description
        ))
        .with_system(
            "You help a designer fill gaps in their design by asking clear, \
             constructive questions grounded in their own work, in the register they \
             work in.",
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
///
/// An AGGREGATE gap ([`GapSource::is_aggregate`]) is keyed on its RULE alone, so
/// its id is stable while the population it ranges over grows and an
/// acknowledgement of it carries instead of expiring on every addition. Every
/// other gap keeps hashing its affected set, because there a changed subject
/// genuinely deserves a fresh judgement.
fn gap_id(source: GapSource, affected: &[String]) -> String {
    if source.is_aggregate() {
        // The literal `|aggregate` rather than an empty member list: an aggregate
        // must not collide with a hypothetical per-node gap of the same source
        // that happened to affect nothing.
        return format!(
            "gap:{:016x}",
            fnv1a(&format!("{}|aggregate", source.as_str()))
        );
    }
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
    /// Adopted conventions. Counted so the ABSENCE of governance can be asked
    /// about — every other rule detector fires on a rule that already exists,
    /// which cannot surface the case where nobody wrote one down.
    design_rules: usize,
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
            design_rules: self.count_nodes(node::DESIGN_RULE)?,
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
            self.governed_by(&node_type, target, node::DECISION, &decision_id, None)?;
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
    /// Takes EITHER the gap id or the Question's own id — whichever the caller
    /// has in hand, since `open_questions` publishes both.
    pub fn answer_question(&mut self, gap_id: &str, answer: &str) -> Result<bool, DynoError> {
        self.set_question_status(gap_id, "answered", Some(answer))
    }

    /// Withdraw a question — asked in error, or overtaken by events. Kept, not
    /// deleted: the past is not overwritten. Takes either identifier, as
    /// [`Self::answer_question`] does.
    pub fn withdraw_question(&mut self, gap_id: &str) -> Result<bool, DynoError> {
        self.set_question_status(gap_id, "withdrawn", None)
    }

    /// Every Question id currently stored, for an error that can say what it
    /// looked for AND what exists. A rejection naming only the miss costs a
    /// round trip the caller has no way to shortcut.
    pub fn known_question_ids(&self) -> Result<Vec<String>, DynoError> {
        Ok(self
            .scan_nodes(node::QUESTION)?
            .into_iter()
            .map(|n| n.node_id)
            .collect())
    }

    /// Resolve the Question a caller means, by EITHER identifier.
    ///
    /// The derived id is tried FIRST, so every caller that passes a `gap_id`
    /// behaves exactly as before. Only if that finds nothing is the argument
    /// tried as a Question id verbatim.
    ///
    /// WHY THE FALLBACK EXISTS (2026-08-16, StoryFlow fleet): the derivation is
    /// pure string formatting, so it can only ever reach a Question that
    /// `gap_to_prompt` named. A Question created any other way was
    /// PERMANENTLY unanswerable — and `open_questions` publishes its
    /// `question_id`, so the tool handed callers an identifier its sibling
    /// refused. The loop then went on reporting "follow it up rather than
    /// asking again" about something it structurally could not close, which is
    /// the exact failure that instruction exists to prevent.
    fn resolve_question_id(&self, id: &str) -> Result<Option<String>, DynoError> {
        let derived = asked_question_id(id);
        if self.get_node(node::QUESTION, &derived)?.is_some() {
            return Ok(Some(derived));
        }
        if self.get_node(node::QUESTION, id)?.is_some() {
            return Ok(Some(id.to_string()));
        }
        Ok(None)
    }

    fn set_question_status(
        &mut self,
        gap_id: &str,
        status: &str,
        answer: Option<&str>,
    ) -> Result<bool, DynoError> {
        let Some(question_id) = self.resolve_question_id(gap_id)? else {
            return Ok(false);
        };
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
            // Defect acknowledgements share the `decision:ack:` prefix but are
            // namespaced under `heal:` (req:reviewed-defects). Without this guard
            // an accepted DEFECT would surface here as a retired GAP — the two
            // lists would each report the other's judgements.
            if hash.starts_with("heal:") {
                continue;
            }
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
        self.detect_suspected_duplicate_edges(&mut gaps)?;
        self.detect_unallocated_capabilities(&pop, &mut gaps)?;
        self.detect_unrealized_capabilities(&pop, &mut gaps)?;
        self.detect_unverified_capabilities(&pop, &mut gaps)?;
        self.detect_unverified_enforced_rules(&pop, &mut gaps)?;
        self.detect_failing_verifications(&mut gaps)?;
        self.detect_unresolved_drift(&mut gaps)?;
        self.detect_unreleased_components(&mut gaps)?;
        self.detect_releases_without_epoch(&mut gaps)?;
        self.detect_releases_without_manifest(&mut gaps)?;
        self.detect_status_contradictions(&mut gaps)?;
        self.detect_interface_pairing(&pop, &mut gaps)?;
        // The other direction from interface pairing: those two need an
        // Interface to exist, this one fires where none ever has.
        self.detect_undeclared_seams(&mut gaps)?;
        // Deliberately absent: unexpected coupling. It is a *signal*, reported
        // by `graph_report` and `surprising_connections`, not a gap demanding
        // an answer — see `GapSource::UnexpectedCoupling`.
        self.detect_declining_dimensions(&mut gaps)?;
        self.detect_hierarchy_gaps(&mut gaps)?;
        self.detect_undecided_decision_points(&mut gaps)?;
        self.detect_unreviewed_ideas(&mut gaps)?;
        self.detect_unvalidated_capabilities(&mut gaps)?;
        self.detect_kpp_violations(&mut gaps)?;
        // The roll-up's blind spot: delivery climbs a decomposition without
        // anything ever asking whether the children amount to the parent.
        self.detect_decomposition_coverage(&mut gaps)?;
        // Absence, not consistency: fires loudest where the axis has NEVER
        // been stated, which is the case every other detector here misses.
        self.detect_change_axis_unstated(&mut gaps)?;
        // The product form of the third-party rule: a stated need that nothing
        // a consumer can reach delivers. Silent unless the design has actually
        // declared some audiences.
        self.detect_internal_only_delivery(&mut gaps)?;
        // A promise others may rely on, with nothing showing it holds — and,
        // when nothing is published at all, the ambiguity that a defaulted
        // `designation` creates, asked once rather than assumed away.
        self.detect_unverified_published_contracts(&mut gaps)?;
        // The other half of the same boundary: described in terms another
        // design could compare, or not.
        self.detect_incomplete_published_contracts(&mut gaps)?;

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
        // Governance-as-design (Anthony's framing, proposed with evidence by
        // dev_storyflow's api-boss 2026-08-08). Deliberately AFTER a build
        // exists: see GapSource::BuildWithoutGovernance for why artifacts and
        // not components are the trigger.
        if pop.artifacts > 0 && pop.design_rules == 0 {
            push(
                gaps,
                GapSource::BuildWithoutGovernance,
                0.45,
                3,
                "Nothing records the conventions this build follows",
                "Real files exist, so this build already follows conventions — a branching \
                 rule, a review step, a house style — but the design records none of them. \
                 Which ones has it adopted, and should breaking any of them stop the build?",
                format!(
                    "{} artifact(s) exist; 0 DesignRules. Note `enforced` defaults to TRUE, so a \
                     rule recorded without a word about enforcement is claiming to be \
                     gate-blocking.",
                    pop.artifacts
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
            // A RULING may declare this requirement correctly unsatisfied, and
            // this is the half `orphan_node` could never cover
            // (`req:a-deliberate-state-is-not-a-defect`, case b). Reported by
            // dev_storyflow: a requirement disconnected on purpose, recorded in
            // an accepted Decision saying no SATISFIES may be drawn to it — and
            // `disconnected_community` proposed `generate_bridge`, PRECISELY THE
            // FORGERY THE RULING FORBIDS, which an agent working the list
            // top-down would have performed.
            //
            // Unlike the artifact case, no edge silences this one incidentally:
            // the detector looks for incoming SATISFIES, so governance is
            // invisible to it unless it is read. That is why this must READ the
            // ruling rather than inherit a side-effect.
            if self.is_parked(&req.node_id)? {
                continue;
            }
            // A DISCONTINUED capability is not a live satisfier, so a
            // requirement whose only satisfier was withdrawn becomes
            // unsatisfied and is ASKED about again. Delivery arithmetic makes
            // the same exclusion; the two must agree or the report and the gap
            // list say different things about one requirement.
            let mut live_satisfiers = 0usize;
            for e in self.incoming(&req.node_id, Some(edge::SATISFIES))? {
                if !self.is_discontinued(&e.from_id)? {
                    live_satisfiers += 1;
                }
            }
            if live_satisfiers == 0 {
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
    /// Has this node been DISCONTINUED — built, then decided against, with
    /// nothing taking its place?
    ///
    /// # The shape, and why it needed no new vocabulary
    ///
    /// A Decision `OBSOLETES` the thing it withdrew. Putting the DECISION at
    /// the source is what makes this work: `OBSOLETES` and `SUPERSEDES` are
    /// both directional and both presume a SUCCESSOR at the source end, and a
    /// discontinued thing has no successor to put there — which is exactly why
    /// `dec:idea-discontinued-is-a-first-class-state` was opened. But a
    /// discontinuation ALWAYS has a decision behind it even when it has no
    /// replacement, so the decision is the honest source, and the edge carries
    /// the WHY by pointing at prose that can hold it.
    ///
    /// `OBSOLETES` is already `* -> *` in the schema and its hint already reads
    /// "source makes target redundant or deprecated". So no edge type is added,
    /// no enum widens, and the version stamp does not move.
    ///
    /// # Only an ACCEPTED decision discontinues anything
    ///
    /// A `proposed` decision to withdraw something has withdrawn nothing. This
    /// is `rule:design-intent-moves-only-on-the-owners-word` applied to the
    /// retirement path: an agent may draw the edge and argue for it, and the
    /// thing keeps counting until somebody accepts the decision.
    ///
    /// # This is the first READER either retirement edge has ever had
    ///
    /// `dec:one-retire-edge` measured on 2026-07-28 that "retiring something
    /// marks it and changes nothing — a retired capability still counts in
    /// every rollup, still raises its gaps, still appears in delivery
    /// arithmetic", and asked what SHOULD consult it. This is that answer for
    /// the discontinued case: the three capability detectors fall silent and
    /// delivery stops counting it. A marker nothing reads is a comment, which
    /// is the failure this project has now found in `enforced`, in `SUPERSEDES`
    /// and in `OBSOLETES` itself.
    /// # Public because the READERS needed it, not only the detectors
    ///
    /// This was `pub(crate)` until 2026-08-12, and every caller was a
    /// computation: the three capability detectors, consumption, delivery
    /// arithmetic. **None of them was a read.** So `scan_nodes` and `get_node`
    /// went on answering `status: "realized"` for a capability an accepted
    /// decision had withdrawn — which is what it says on the node, and not what
    /// a reader needs to know. It cost a wrong recommendation to the owner's
    /// face: a session read `cap:content-store` as live and proposed building a
    /// surface for a feature he had deleted three days earlier.
    ///
    /// Derived on every read rather than written onto the node, so the reader
    /// and the detectors can never disagree — and so
    /// `dec:idea-does-a-capability-need-a-cancelled-state` stays open instead of
    /// being settled by implementation.
    pub fn is_discontinued(&self, node_id: &str) -> Result<bool, DynoError> {
        for e in self.incoming(node_id, Some(edge::OBSOLETES))? {
            let Some(src) = self.get_node(node::DECISION, &e.from_id)? else {
                // Obsoleted by something that is not a Decision — a superseding
                // epoch, say. That is a different relationship and this rule
                // deliberately does not read it.
                continue;
            };
            if src
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                == Some("accepted")
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

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
            // Discontinued: built, then decided against. It is not
            // unfinished work and asking about it forever is how a gap list
            // becomes unreadable.
            if self.is_discontinued(&cap.node_id)? {
                continue;
            }
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
    /// A `DUPLICATES` edge a MACHINE proposed — asked as a question, because
    /// nobody has confirmed it.
    ///
    /// # Why this exists, and what it is the other half of
    ///
    /// `dec:ask-not-repair` splits suspicion from repair: *"possible_duplicate
    /// is a DETECT gap; HEAL merges only on a human-drawn DUPLICATES edge."*
    /// [`Self::detect_possible_duplicates`] above implements that for pairs
    /// reflow2 computes ITSELF from capability overlap. This implements it for
    /// pairs an EXTRACTION PASS proposed — corpus ingest's fuzzy name match —
    /// which land in the graph as a real edge carrying `basis: suspected`.
    ///
    /// Until 2026-08-08 those edges were bare, so HEAL could not tell them from
    /// a human's assertion and offered them as merges: measured in
    /// dev_storyflow, ten proposed node deletions from name-similarity scores of
    /// 81-85 on unrelated nodes. HEAL now skips anything not explicitly
    /// `asserted`. **This function is why that is a re-routing rather than a
    /// silent drop** — the suspicion still reaches the user, on the DETECT side
    /// where it can be answered or acknowledged, which is what the decision
    /// asked for in the first place. A defect cannot be dismissed; a gap can,
    /// and a heuristic with false positives needs the dismissible one.
    ///
    /// It also keeps the property corpus ingest wanted when it started
    /// persisting these: the ask is BATCHED. Four hundred documents raise four
    /// hundred suspicions collected into one `detect_gaps` answer, rather than
    /// one transient report per file addressed to an agent that has already
    /// forgotten the last one.
    ///
    /// The score is carried in `evidence` rather than left implicit, so a reader
    /// can dismiss a bad pair without fetching both nodes.
    fn detect_suspected_duplicate_edges(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        let index = self.node_type_index()?;
        // Resolve an id to its display name through the same index, so a gap
        // reads in the user's vocabulary rather than in ids.
        let name_of = |id: &str| -> String {
            index
                .get(id)
                .and_then(|t| self.get_node(t, id).ok().flatten())
                .map_or_else(|| id.to_string(), |n| node_name(&n))
        };
        for e in self.all_edges_of_type(edge::DUPLICATES, &index)? {
            // Absent reads as suspected, exactly as HEAL reads it: the two must
            // agree or a pair could fall between them and be reported nowhere.
            if matches!(
                e.properties.get("basis").and_then(Value::as_str),
                Some("asserted")
            ) {
                continue;
            }
            let (a, b) = ordered_pair(&e.from_id, &e.to_id);
            let a_name = name_of(&a);
            let b_name = name_of(&b);
            let score = e.properties.get("confidence").and_then(Value::as_f64);
            let how = match score {
                Some(c) => format!("their names are {:.0}% alike", c * 100.0),
                // Omitted for a structural token-subset match, where writing 0.0
                // would read as "certainly unrelated" — the opposite of what a
                // subset relation means.
                None => "one name's words are a subset of the other's".to_string(),
            };
            let affected = vec![a.clone(), b.clone()];
            gaps.push(GapCandidate {
                id: gap_id(GapSource::PossibleDuplicate, &affected),
                gap_source: GapSource::PossibleDuplicate,
                scope: GapScope::Project,
                // Deliberately below the computed-overlap rule above. That one
                // reasons about how nodes are WIRED; this one about how they are
                // SPELLED, and a name match is the weaker signal — which is the
                // whole lesson of the 81-85 scores on unrelated nodes.
                severity: 0.45,
                title: format!("Are “{a_name}” and “{b_name}” the same thing?"),
                description: format!(
                    "An extraction pass noticed that “{a_name}” and “{b_name}” look alike ({how}) and recorded the suspicion, but nobody has confirmed it. Are these two names for one thing, or two genuinely different things that happen to read similarly?"
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "'{a}' DUPLICATES '{b}' with basis=suspected — proposed by a name-similarity heuristic, not asserted by anyone. Confirm it by re-drawing the edge with basis=asserted, which is what lets HEAL merge them; acknowledge the gap to record that they are distinct."
                ),
            });
        }
        Ok(())
    }

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
            // A `required` boundary is one this design needs FROM OUTSIDE, so
            // "nothing here provides it" is its DEFINITION rather than a gap.
            //
            // Found 2026-08-09 by doing the thing dec:linked-repos-poc asked
            // for: declaring reflow2's nine required interfaces immediately
            // produced nine `unprovided_interface` gaps at severity 0.72 — the
            // detector nagging correct modelling, once per requirement, forever.
            // Any consumer who follows the same advice gets the same, which is
            // the "fires on correct work" failure dec:read-hint-shape exists to
            // prevent.
            //
            // `both` is deliberately NOT exempt: a design that publishes a
            // contract as well as needing one owes an internal provider for the
            // half it publishes.
            let required_from_outside = iface
                .properties
                .get("designation")
                .and_then(dynograph_core::Value::as_str)
                == Some("required");

            if required_from_outside {
                continue;
            }

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

    /// Component pairs that depend on each other with no contract recorded
    /// between them — the seam the build has and the design does not.
    ///
    /// THE SET WAS ALREADY BEING COMPUTED. `maturity_report`'s `seams` band
    /// divides `declared` by `couplings` on every run and discards the
    /// difference; [`DesignGraph::seam_sets`] is that computation, extracted so
    /// the band and this detector cannot disagree about what a contract is.
    ///
    /// TWO SILENCES, both load-bearing:
    ///
    /// 1. **No couplings at all → nothing to say.** `maturity` already words
    ///    this exactly right — "no two Components depend on each other, so there
    ///    is no seam to declare — an absence, not a deficiency" — and a detector
    ///    that reported a clean zero as a fault would contradict its own band.
    /// 2. **Every coupling declared → nothing to say.** The obvious one, stated
    ///    because it is the state this gap exists to reach.
    ///
    /// It names the pairs and asks. Drafting the Interface would be fabrication
    /// (`cap:no-fabricated-repair`): the graph knows two components are coupled
    /// and cannot know what runs across the boundary.
    fn detect_undeclared_seams(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let seams = self.seam_sets()?;
        if seams.couplings.is_empty() {
            return Ok(());
        }
        let undeclared = seams.undeclared();
        if undeclared.is_empty() {
            return Ok(());
        }

        // The pairs are the finding; the components are what a reader navigates
        // to, so `affected_ids` carries each component once, in a stable order.
        let mut affected: Vec<String> = Vec::new();
        for (a, b) in &undeclared {
            for id in [a, b] {
                if !affected.contains(id) {
                    affected.push(id.clone());
                }
            }
        }
        affected.sort();

        let n = undeclared.len();
        let named: Vec<String> = undeclared
            .iter()
            .map(|(a, b)| format!("{} ↔ {}", self.component_label(a), self.component_label(b)))
            .collect();
        // How many pairs to spell out in the question. The whole list goes in
        // `evidence` regardless — this is only about what a person is asked to
        // read at once, and 73 names in a prompt is a wall, not a question.
        const SHOWN: usize = 6;
        let sample = if n > SHOWN {
            format!("{}, and {} more", named[..SHOWN].join("; "), n - SHOWN)
        } else {
            named.join("; ")
        };

        gaps.push(GapCandidate {
            id: gap_id(GapSource::UndeclaredSeam, &affected),
            gap_source: GapSource::UndeclaredSeam,
            scope: GapScope::Project,
            severity: 0.45,
            title: format!(
                "{n} pair(s) of parts depend on each other with no contract written down"
            ),
            description: format!(
                "{sample} — each of these pairs is coupled in the design, and nothing records what \
                 passes between them. What is the contract on each: what crosses the boundary, in \
                 which direction, over what? Record it with add_interface plus provides/consumes, \
                 or acknowledge that these boundaries are held somewhere other than the design.",
            ),
            affected_ids: affected,
            suggested_depth: 2,
            evidence: format!(
                "Component pairs with a DEPENDS_ON edge and no Interface carrying BOTH a PROVIDES \
                 and a CONSUMES between them ({n} of {} coupling(s)): {}.",
                seams.couplings.len(),
                undeclared
                    .iter()
                    .map(|(a, b)| format!("{a} ↔ {b}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
        Ok(())
    }

    /// `req:decomposition-covers-its-parent` — the one question the roll-up
    /// never asks.
    ///
    /// Fires per DECOMPOSED PARENT, not per project: "what did this parent hold
    /// that none of its children hold?" is answerable only about one parent, and
    /// aggregating would produce a question nobody can answer. Silent on a design
    /// that has never decomposed anything, which is an absence rather than a
    /// deficiency — and is the state reflow2's own design is in, so this detector
    /// cannot be exercised by the self-host (`rule:the-self-host-always-trails-what-it-teaches`).
    fn detect_decomposition_coverage(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for parent in self.scan_nodes(node::REQUIREMENT)? {
            let children = self.decomposed_children(&parent.node_id)?;
            if children.is_empty() {
                continue;
            }

            // Parent AND children. The children are the answer's working
            // material — nobody can say what is missing without seeing what is
            // there — and anchoring on the whole set is what makes the id expire
            // when the split changes (see `is_aggregate`).
            let mut affected = vec![parent.node_id.clone()];
            affected.extend(children.iter().cloned());
            affected.sort();

            let n = children.len();
            let label = node_name(&parent);
            // The risk stops being hypothetical the moment the roll-up fires:
            // the parent is then ASSERTING delivered on the strength of children
            // nobody has checked it against.
            let delivered = self.requirement_is_delivered(&parent.node_id)?;

            gaps.push(GapCandidate {
                id: gap_id(GapSource::DecompositionCoverage, &affected),
                gap_source: GapSource::DecompositionCoverage,
                scope: GapScope::Capability,
                severity: if delivered { 0.70 } else { 0.50 },
                title: format!(
                    "'{label}' was split into {n} — nothing has checked that they cover it{}",
                    if delivered {
                        ", and it already reports delivered"
                    } else {
                        ""
                    }
                ),
                description: format!(
                    "'{label}' was split into {n} child requirement(s), and delivery rolls UP: it \
                     counts as delivered exactly when all {n} are.{} What did '{label}' hold that \
                     none of its children hold? Write the answer into the design — a child \
                     carrying what fell out — or acknowledge_gap with the reason it was left out \
                     on purpose. A narrowing you intended and a drop nobody noticed look identical \
                     until one of them is written down.",
                    if delivered {
                        format!(
                            " It already reports DELIVERED on the strength of those {n}, so \
                             anything that fell between them is being counted as done."
                        )
                    } else {
                        String::new()
                    }
                ),
                affected_ids: affected,
                suggested_depth: 3,
                evidence: format!(
                    "Requirement carrying incoming DECOMPOSES edge(s), with no coverage answer on \
                     record: {} <- {}. Rolled-up delivery: {}.",
                    parent.node_id,
                    children.join(", "),
                    if delivered {
                        "delivered"
                    } else {
                        "not yet delivered"
                    }
                ),
            });
        }
        Ok(())
    }

    /// A Component's human name, falling back to its id — the id is what a
    /// reader can act on when a component was created without a name.
    fn component_label(&self, id: &str) -> String {
        match self.get_node(node::COMPONENT, id) {
            Ok(Some(c)) => node_name(&c),
            _ => id.to_string(),
        }
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
            // Discontinued: built, then decided against. Not unfinished work.
            if self.is_discontinued(&cap.node_id)? {
                continue;
            }
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

            // WHEN the check last ran, said next to WHAT it said.
            //
            // `status` is a measurement taken at an instant; a gap that reports
            // it alone presents it as a standing property of the system. That
            // difference is not cosmetic — it cost a real fleet twice in one
            // shift, in both directions (2026-07-27):
            //
            //   * a verification read `passing` while the service behind it was
            //     100% dead — recorded from a transcript, never re-run;
            //   * these very gaps read `failing` for 26 capabilities on a run
            //     that predated the fixes by three days, asserting at MAXIMUM
            //     SEVERITY that GENESIS, extraction, self-healing and 23 others
            //     were *proven broken* when their suites were green.
            //
            // The false-red is the more corrosive of the two: it is the
            // permanently-red-check failure, and it trains every future reader
            // to skim the top of the gap list — which is exactly where a real
            // 0.8 would appear.
            //
            // No clock is consulted: the recorded timestamp is surfaced verbatim
            // and the reader judges. Introducing "now" into a detector would make
            // gap detection non-deterministic, which is a worse trade than making
            // a human compare two dates.
            let last_run = ver
                .properties
                .get("last_run_at")
                .and_then(dynograph_core::Value::as_str)
                .filter(|s| !s.is_empty());
            let when = match last_run {
                Some(t) => format!(" when it last ran, at {t}"),
                // A `failing` that has never run is an assertion, not a
                // measurement, and the wording has to stop short of claiming
                // otherwise.
                None => {
                    String::from(", though it records no run — the status was set, not measured")
                }
            };
            let recency = match last_run {
                Some(t) => format!("last_run_at={t}"),
                None => String::from("no last_run_at recorded"),
            };
            gaps.push(GapCandidate {
                id: gap_id(GapSource::FailingVerification, &affected),
                gap_source: GapSource::FailingVerification,
                scope: GapScope::Capability,
                severity: 0.8,
                title: match last_run {
                    Some(t) => format!("“{name}” is failing (last run {t})"),
                    None => format!("“{name}” is failing (never run)"),
                },
                description: format!(
                    "The check “{name}” was failing{when}, so {what} did not work as designed then. \
                     Re-run it before treating that as current — a status is a measurement at an \
                     instant, not a standing property — then fix the build or fix the design."
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "Verification '{}' has status=failing, {recency}. A failing check is reality \
                     contradicting the design rather than absence of a check — but only as of that \
                     run; if code has landed since, the gap may be stale rather than real.",
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

    /// A `Release` that names no point on the time axis (see
    /// [`GapSource::ReleaseWithoutEpoch`]).
    ///
    /// The whole computation already exists in `changelog_point`; this is the
    /// missing detector rung. Deliberately reports the edge kind it examined —
    /// BL-114's lesson, applied at birth rather than retrofitted: a finding
    /// that says "has no epoch" when it means "has no `AT_EPOCH` edge" is the
    /// class of message a user learns to distrust.
    fn detect_releases_without_epoch(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        // No epoch nodes at all means the temporal axis is simply not in use —
        // a whole-graph situation, not one gap per release. Same guard shape as
        // detect_unreleased_components' empty-`shipped` check.
        if self.scan_nodes(node::DESIGN_EPOCH)?.is_empty() {
            return Ok(());
        }
        for rel in self.scan_nodes(node::RELEASE)? {
            // A PLANNED release legitimately has no epoch yet: the epoch is
            // minted when the release is cut, so asking beforehand is an alarm
            // on correct work — the `unverified_capability` disease (BL-115),
            // which floods a gap list until people skim it. BL-122's defect is
            // specifically a release that was CUT without its edge, and this
            // still catches that the moment the status moves off `planned`.
            //
            // "When is this planned release due?" is a real and different
            // question — a schedule question, wanting `SCHEDULED_FOR` and the
            // roadmap thread, not this rule.
            //
            // 🛑 BUT A DEPLOYED RELEASE IS OUT, WHATEVER ITS STATUS SAYS, and
            // that clause was added 2026-08-21 because the exemption above had
            // a hole big enough to drive the very defect through. `rel:v0380`
            // was tagged, built, published and asset-verified while its
            // `status` still read `planned` — nobody had moved it, because
            // nothing makes you. So "status == planned" does not mean "not yet
            // cut"; it means "nobody wrote it down", and trusting it hands an
            // exemption to exactly the careless cut this rule exists to catch.
            // `DEPLOYED_TO` is the structural fact and cannot be forgotten into
            // existence: a genuinely planned release has no deployment.
            //
            // Found by the sibling rule's own tests (`release_without_manifest`,
            // which never consulted the status for this reason) — the shared
            // lesson being that a status property records what somebody
            // remembered, and an edge records what happened.
            let deployed = !self
                .outgoing(&rel.node_id, Some(edge::DEPLOYED_TO))?
                .is_empty();
            if !deployed
                && rel
                    .properties
                    .get("status")
                    .and_then(dynograph_core::Value::as_str)
                    == Some("planned")
            {
                continue;
            }
            let pinned = self
                .outgoing(&rel.node_id, Some(edge::AT_EPOCH))?
                .into_iter()
                .any(|e| matches!(self.get_node(node::DESIGN_EPOCH, &e.to_id), Ok(Some(_))));
            if pinned {
                continue;
            }
            let name = node_name(&rel);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::ReleaseWithoutEpoch,
                    std::slice::from_ref(&rel.node_id),
                ),
                gap_source: GapSource::ReleaseWithoutEpoch,
                scope: GapScope::Project,
                // Above unreleased_component's 0.5: this one makes a COMPUTED
                // answer silently wrong rather than leaving a question open.
                severity: 0.6,
                title: format!("“{name}” is not pinned to any epoch"),
                description: format!(
                    "“{name}” has no AT_EPOCH edge, so it names no point on the time axis. \
                     Anything computing a window from it — a changelog between two releases, \
                     an as-of-epoch read — gets no lower bound and silently widens to the \
                     beginning of the design. Which epoch does it belong to, or was it cut \
                     before the epoch spine existed?"
                ),
                affected_ids: vec![rel.node_id.clone()],
                suggested_depth: 1,
                evidence: format!(
                    "Release '{}' has no AT_EPOCH edge to a DesignEpoch. Only AT_EPOCH was \
                     considered — a name that matches an epoch node, or any other edge \
                     between them, does not pin it.",
                    rel.node_id
                ),
            });
        }
        Ok(())
    }

    /// A `Release` that is pinned and deployed and ships nothing (see
    /// [`GapSource::ReleaseWithoutManifest`]).
    ///
    /// Reports the edge kinds it examined, the same discipline the sibling
    /// carries: a finding that says "ships nothing" when it means "has no
    /// `INCLUDES` edge" is the class of message a user learns to distrust.
    fn detect_releases_without_manifest(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        let releases = self.scan_nodes(node::RELEASE)?;
        if releases.is_empty() {
            return Ok(());
        }
        // If NO release anywhere records its contents, release contents are
        // simply not modelled in this design — a whole-graph situation, not one
        // gap per release. Same guard shape, and the same reasoning, as
        // `detect_unreleased_components`' empty-`shipped` check: a design that
        // has never used a feature is not a design that is failing at it.
        // One pass answers both questions: whether ANY release records its
        // contents, and how many do — the second being the denominator the
        // evidence needs, so "this one is empty" always arrives beside "and the
        // other 39 are not".
        let mut with_manifest = 0usize;
        for rel in &releases {
            if !self
                .outgoing(&rel.node_id, Some(edge::INCLUDES))?
                .is_empty()
            {
                with_manifest += 1;
            }
        }
        if with_manifest == 0 {
            return Ok(());
        }
        for rel in &releases {
            if !self
                .outgoing(&rel.node_id, Some(edge::INCLUDES))?
                .is_empty()
            {
                continue;
            }
            // Both are required, and each rules out a different false alarm.
            //
            // The epoch says the release has a place on the time axis — it was
            // cut, not sketched. The deployment says it actually went out.
            // Neither alone is enough: a roadmap release planned into a future
            // epoch has the first and not the second, and asking it what it
            // ships is the `unverified_capability` disease (BL-115) that floods
            // a gap list until people skim it.
            //
            // `status` is deliberately not consulted — see the variant's docs.
            // `rel:v0380`, the one real instance, was published while its status
            // still read `planned`.
            let pinned = self
                .outgoing(&rel.node_id, Some(edge::AT_EPOCH))?
                .into_iter()
                .any(|e| matches!(self.get_node(node::DESIGN_EPOCH, &e.to_id), Ok(Some(_))));
            if !pinned {
                continue; // release_without_epoch's territory, not this one's
            }
            let deployed = !self
                .outgoing(&rel.node_id, Some(edge::DEPLOYED_TO))?
                .is_empty();
            if !deployed {
                continue; // cut but not yet out — the manifest can still land
            }
            let name = node_name(rel);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::ReleaseWithoutManifest,
                    std::slice::from_ref(&rel.node_id),
                ),
                gap_source: GapSource::ReleaseWithoutManifest,
                scope: GapScope::Project,
                // ⭐ BUILD-STOPPING, ON THE USER'S WORD (2026-08-21), and the
                // number is chosen to land there rather than tuned to it.
                //
                // `reflow2_check` fails at `--gap-threshold` (default 0.8), and
                // above that line sit the findings that say THE DESIGN ASSERTS
                // SOMETHING THAT IS NOT TRUE — failing_verification at 0.80,
                // kpp_contradicted 0.85, kpp_unbound 0.90, kpp_breached 0.95 —
                // as against the questions below it, which wait for a human.
                // This belongs with the first group: a deployed release whose
                // manifest is empty is not an open question about the future,
                // it is a false statement about the past.
                //
                // WHY A NOTE WAS NOT ENOUGH, measured: the v0.38.0 cut passed
                // `reflow2_check` GREEN with 96 notes scrolling past it, and
                // that is precisely how an empty manifest reached a published
                // tag. A finding nobody is made to read is not a finding.
                //
                // Above failing_verification's 0.80 because a failing check is
                // a signal to act BEFORE shipping, while this one can only ever
                // be true AFTER; and deliberately not sitting exactly on the
                // default threshold, so nudging `--gap-threshold` up one notch
                // does not silently drop the rule out of the gate.
                //
                // A contentless release IS a real state — a re-tag, a docs-only
                // republish — and stays acknowledgeable with a reason on the
                // record. Red until somebody says why, never red forever.
                severity: 0.85,
                title: format!("“{name}” is deployed and records shipping nothing"),
                description: format!(
                    "“{name}” is pinned to an epoch and deployed, and no INCLUDES edge says \
                     what it shipped. The as-released view is read off those edges, so \
                     release_report answers with an empty manifest and “does what we released \
                     match what we designed?” silently answers “nothing was released”. Did the \
                     manifest never get written — release_includes_all defaults to a DRY RUN and \
                     reports what it WOULD add — or did this release genuinely ship no new \
                     content?"
                ),
                affected_ids: vec![rel.node_id.clone()],
                suggested_depth: 1,
                evidence: format!(
                    "Release '{}' has an AT_EPOCH edge and at least one DEPLOYED_TO edge, and \
                     zero INCLUDES edges. Only INCLUDES was counted; its `status` property was \
                     not consulted, deliberately — rel:v0380 was tagged, published and deployed \
                     while its status still read 'planned'. {} of {} release(s) in this design \
                     record a manifest.",
                    rel.node_id,
                    with_manifest,
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

    /// A `DesignRule` the build enforces that nothing can detect a violation of.
    ///
    /// ITS OWN DETECTOR, not a clause inside the capability one, and the tests
    /// are why: `detect_unverified_capabilities` early-returns when the project
    /// has zero verifications, because "nothing is verified yet" is already said
    /// once at project level. That gate is right for a capability and WRONG for
    /// a rule — a rule that claims to fail builds is unanswerable from the
    /// moment it is written, whether or not anything else has a check yet. The
    /// first draft of this lived in that function and silently never fired on a
    /// young design; the test that expected it to fire is what found that.
    fn detect_unverified_enforced_rules(
        &self,
        pop: &Population,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        // A gate-blocking rule nobody can detect a violation of (the governance
        // proposal, dev_storyflow api-boss 2026-08-08, from Anthony's framing).
        //
        // Deliberately NOT riding a component's check the way a capability may:
        // a rule is not allocated anywhere, so there is no carrier one hop away
        // and no third state to compute. Either something checks this rule or
        // nothing does.
        let mut unstated = Vec::new();
        for n in self.scan_nodes(node::DESIGN_RULE)? {
            let enforced = n.properties.get("enforced").and_then(|v| v.as_bool());
            // ONLY AN EXPLICIT `true` IS BILLED FOR A DETECTOR.
            //
            // This read the other way round for exactly one day. `enforced`
            // defaulted to true, so absence was a claim and this detector
            // exempted only an explicit `false` — which meant a convention
            // recorded in passing was charged for a check nobody agreed to.
            // The default is gone (dec:does-enforced-default-to-gate-blocking,
            // Anthony 2026-08-08) and absence now means nobody has said.
            //
            // An unstated rule is NOT silently let off: it raises
            // `unstated_rule_enforcement` below, which asks the question
            // instead of billing for the answer. Two findings rather than one,
            // because "prove this rule" and "decide what this rule is" are
            // different questions and collapsing them is what made the old
            // reading wrong.
            if enforced != Some(true) {
                if enforced.is_none() {
                    unstated.push(n);
                }
                continue;
            }
            // `dec:passing-is-verified`: attaching a `planned` check must not
            // silence the question, or this detector becomes the green-washing
            // its own proposal warned about.
            if self.has_passing_verification(&n.node_id)? {
                continue;
            }
            let name = node_name(&n);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::UnverifiedEnforcedRule,
                    std::slice::from_ref(&n.node_id),
                ),
                gap_source: GapSource::UnverifiedEnforcedRule,
                scope: GapScope::Project,
                // Above `unverified_capability` (0.55) because this rule already
                // claims the power to fail a build. An unproven capability is
                // work not yet confirmed; an unverifiable enforced rule is an
                // obligation nobody can observe compliance with, and it is
                // stated at the project level rather than about one part.
                severity: 0.6,
                title: format!("Nothing detects a violation of “{name}”"),
                description: format!(
                    "The rule “{name}” is enforced — its violations are \
                     gate-blocking — but no passing verification could detect one. What checks \
                     it, or should it be advisory rather than enforced?"
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: {
                    let attached = self.incoming(&n.node_id, Some(edge::VERIFIES))?.len();
                    if attached == 0 {
                        format!(
                            "DesignRule '{}' has enforced=true and 0 incoming VERIFIES; project \
                             has {} verification(s).",
                            n.node_id, pop.verifications
                        )
                    } else {
                        format!(
                            "DesignRule '{}' has enforced=true and {attached} incoming VERIFIES, \
                             none of them passing; a check that has not passed cannot detect \
                             anything.",
                            n.node_id
                        )
                    }
                },
            });
        }
        for n in unstated {
            let name = node_name(&n);
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::UnstatedRuleEnforcement,
                    std::slice::from_ref(&n.node_id),
                ),
                gap_source: GapSource::UnstatedRuleEnforcement,
                scope: GapScope::Project,
                // Below build_without_governance (0.45): a rule exists and only
                // its consequence is open, which is a smaller hole than no
                // conventions at all. Well below unverified_enforced_rule
                // (0.6), because this asks for a word and that asks for a check.
                severity: 0.4,
                title: format!("“{name}” does not say whether breaking it stops the build"),
                description: format!(
                    "The rule “{name}” does not say whether its violations are \
                     gate-blocking. Absent is not read as either answer \u{2014} is breaking this \
                     rule something that should stop the build, or is it advice?"
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "DesignRule '{}' has no `enforced` property. It is neither billed for a \
                     detector nor treated as advisory until somebody says which.",
                    n.node_id
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
            // A check that has not passed is not proof. Until 2026-07-27 this
            // skipped on ANY incoming VERIFIES, so attaching a `planned`
            // Verification silenced the question — which is precisely what the
            // detect-and-ask skill already warns against ("a check left at
            // planned does not count as confirmation"). The skill said it; the
            // detector did not enforce it, and the gap between those two is
            // where a design goes quiet without getting better. Measured before
            // changing: zero capabilities on reflow2's own graph were riding a
            // non-passing check, so this tightens the rule without moving any
            // existing verdict.
            if self.has_passing_verification(&n.node_id)? {
                continue;
            }
            // Discontinued: built, then decided against. Demanding proof that
            // withdrawn work still functions is the clearest case of a gap
            // list asking a question nobody can act on.
            if self.is_discontinued(&n.node_id)? {
                continue;
            }
            // NOT STARTED: you cannot check what nobody has built.
            //
            // This is the THIRD time this detector has narrowed for the same
            // reason, and the two above say why — the per-artifact rule that
            // made 22 of 25 findings, and BL-73's 21 near-identical
            // acknowledges. Measured here before changing: 28 of the 92
            // findings on reflow2's own design were roadmap rows nobody had
            // started, and a list that cannot reach zero teaches you to skim
            // it. That is not a tidiness argument. reflow2's own v0.38.0 was
            // published with an empty manifest past a gate showing 96 notes.
            //
            // ⚠️ READING `status` ALONE IS NOT ENOUGH, and that is the whole
            // shape of this exemption. A capability marked `planned` that an
            // Artifact already realizes IS built and its status is merely
            // stale — the `rel:v0380` shape, where a published, deployed
            // release still read `planned` because nothing makes anyone move
            // it. So both must hold: the status says not started, AND nothing
            // builds it. Measured, that distinction keeps exactly one live
            // question on this design (`cap:explains-itself`, planned with a
            // file already realizing it) that trusting the status would have
            // silenced — which is the entire point.
            //
            // DIRECT realization only. The indirect path
            // (`art -REALIZES-> cmp <-ALLOCATED_TO- cap`) that
            // `unrealized_capability` rightly accepts is too loose HERE:
            // measured, it calls all 29 planned capabilities built, because
            // each is allocated to a component some file realizes, and the
            // exemption then does nothing at all.
            let not_started = n
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                == Some("planned");
            if not_started && self.incoming(&n.node_id, Some(edge::REALIZES))?.is_empty() {
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
                title: format!("Nothing verifies “{name}”"),
                description: format!(
                    "“{name}” has no verification proving it works — how will \
                     you confirm it?"
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: {
                    let attached = self.incoming(&n.node_id, Some(edge::VERIFIES))?.len();
                    if attached == 0 {
                        format!(
                            "Capability '{}' has 0 incoming VERIFIES; project has {} \
                             verification(s).",
                            n.node_id, pop.verifications
                        )
                    } else {
                        format!(
                            "Capability '{}' has {attached} incoming VERIFIES, none of them \
                             passing; a check that has not passed is not proof.",
                            n.node_id
                        )
                    }
                },
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
                     “{cmp_name}”"
                ),
                description: format!(
                    "“{cmp_name}”'s passing check is the only verification these \
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
                HierarchyIssueKind::MultipleParents => GapSource::MultipleParents,
                HierarchyIssueKind::LevelSpineDisagreement => GapSource::LevelSpineDisagreement,
            };
            // Missing-intermediate is the highest-value Y defect; rank it up.
            let severity = match issue.kind {
                HierarchyIssueKind::MissingIntermediateLevel => 0.7,
                HierarchyIssueKind::LevelMismatch => 0.6,
                HierarchyIssueKind::OrphanLevel => 0.45,
                // A box in two boxes makes every containment walk ambiguous,
                // so it outranks the floating-level finding.
                HierarchyIssueKind::MultipleParents => 0.65,
                HierarchyIssueKind::LevelSpineDisagreement => 0.5,
            };
            let title = match issue.kind {
                HierarchyIssueKind::MissingIntermediateLevel => "Missing intermediate level",
                HierarchyIssueKind::LevelMismatch => "Decomposition level mismatch",
                HierarchyIssueKind::OrphanLevel => "Floating decomposition level",
                HierarchyIssueKind::MultipleParents => "One box, two parents",
                HierarchyIssueKind::LevelSpineDisagreement => {
                    "Declared level disagrees with spine position"
                }
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

    /// The ideas nobody has opened: proposed Decisions with no relation and no
    /// note (`GapSource::UnreviewedIdeas`).
    ///
    /// One aggregate finding. The `affected_ids` are the ideas themselves, so
    /// the question can name them, but the gap is one question about a practice
    /// rather than 115 questions about 115 thoughts.
    fn detect_unreviewed_ideas(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let unreviewed = self.unreviewed_ideas()?;
        if unreviewed.is_empty() {
            return Ok(());
        }
        // The population this ranges over, so the finding can say what it is a
        // fraction OF. "115 ideas unconnected" reads very differently at 147
        // ideas and at 120 — and a detector that reports a numerator without a
        // denominator has told you almost nothing.
        let mut proposed = 0usize;
        for dec in self.scan_nodes(node::DECISION)? {
            if dec
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                == Some("proposed")
            {
                proposed += 1;
            }
        }
        let n = unreviewed.len();
        gaps.push(GapCandidate {
            id: gap_id(GapSource::UnreviewedIdeas, &unreviewed),
            gap_source: GapSource::UnreviewedIdeas,
            scope: GapScope::Project,
            // Deliberately below every finding that reports a contradiction or
            // a missing piece of the golden thread. Nothing here is WRONG — the
            // ideas are recorded and findable. They are just not reachable from
            // each other, which costs a later reader a search they should not
            // have had to run.
            severity: 0.3,
            title: format!("{n} of {proposed} open idea(s) connect to nothing"),
            description: format!(
                "{n} proposed decision(s) carry no relation to any other node and no note saying \
                 the relations were reviewed — so a search that lands on one returns it alone, and \
                 the reasoning it belongs to is not reachable from it. Relate the ones that are \
                 genuinely related with review_relations, and use the same call to record a note \
                 where nothing is honestly related. Do NOT draw an edge to clear this finding: a \
                 false neighbour is worse than a missing one, because anything that searches by \
                 neighbourhood repeats it."
            ),
            affected_ids: unreviewed.clone(),
            suggested_depth: 2,
            evidence: format!(
                "{n} of {proposed} proposed Decision(s) have no inference-layer relation in either \
                 direction and no `no_relation_note`; decision points with 2+ registered \
                 alternatives and parked nodes are excluded."
            ),
        });
        Ok(())
    }

    /// Recorded changes that never say which axis they are on
    /// (`GapSource::ChangeAxisUnstated`).
    ///
    /// One aggregate finding over every ChangeEvent in the design. The
    /// numerator is the events carrying no `subject`; the denominator is all of
    /// them, because a count without its population has said almost nothing —
    /// "12 changes unstated" reads very differently at 12 events and at 400.
    ///
    /// # Why it fires at zero usage rather than staying quiet
    ///
    /// This is the whole point of the detector and the reason it is written as
    /// an absence check. A design that has recorded changes and never once
    /// stated an axis is the case most worth asking about, and it is precisely
    /// the case a consistency check cannot see: keying on events that already
    /// HAVE a subject would report only the design that had begun using the
    /// vocabulary, and would read clean for the design that never did.
    fn detect_change_axis_unstated(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let mut unstated: Vec<String> = Vec::new();
        let mut total = 0usize;
        for ev in self.scan_nodes(node::CHANGE_EVENT)? {
            total += 1;
            // PRESENCE IS ENOUGH, and deliberately so: `subject` is a schema
            // enum, and every write path — the typed constructors and
            // `import_graph` alike — goes through `create_node`, which refuses
            // anything but `system` or `record`. A blank cannot reach the
            // store, so an emptiness guard here would be dead code pretending
            // to be a defence. `tests/change_axis_unstated.rs` pins the
            // refusal, so this stays honest if the schema ever loosens.
            if !ev.properties.contains_key("subject") {
                unstated.push(ev.node_id.clone());
            }
        }
        if unstated.is_empty() {
            return Ok(());
        }
        let n = unstated.len();
        gaps.push(GapCandidate {
            id: gap_id(GapSource::ChangeAxisUnstated, &unstated),
            gap_source: GapSource::ChangeAxisUnstated,
            scope: GapScope::Project,
            // Below every finding that reports a contradiction or a break in
            // the golden thread. Nothing here is WRONG: the changes are
            // recorded and findable. What is missing is a distinction that
            // cannot be reconstructed once the person who knew it has moved on,
            // which is a real cost and a quiet one.
            severity: 0.3,
            title: format!("{n} of {total} recorded change(s) do not say what kind of change they were"),
            description: format!(
                "{n} recorded change(s) do not say whether the SYSTEM changed or whether only the \
                 design's record of it did — a re-sync, an accepted drift, a question finally \
                 settled. Both are real and they mean opposite things to anyone later asking what \
                 actually moved, and nothing can tell them apart afterwards: only the person making \
                 the change knew. State it with `subject` on record_change or add_change_event. If \
                 this distinction is not one this project needs — or the events came from a bulk \
                 import that could not know — acknowledge this once and it will not be asked again."
            ),
            affected_ids: unstated.clone(),
            suggested_depth: 1,
            evidence: format!(
                "{n} of {total} ChangeEvent(s) carry no `subject`. The property is optional and \
                 never inferred from `change_type`, because the mapping is not total — a `resync` \
                 is honestly either axis — so an unstated one means nobody said, and this finding \
                 is what asks."
            ),
        });
        Ok(())
    }

    /// A published contract with no passing check, and the posture question
    /// when a design has published nothing at all.
    ///
    /// Two findings on purpose, because "prove this promise" and "have you
    /// decided what you publish?" are different questions — the same split
    /// `UnverifiedEnforcedRule` and `UnstatedRuleEnforcement` already draw, and
    /// collapsing them there was what made the old reading wrong.
    fn detect_unverified_published_contracts(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        let interfaces = self.scan_nodes(node::INTERFACE)?;
        if interfaces.is_empty() {
            // A design with no boundaries at all has not declined to publish
            // one; it has not got there. Same reasoning as the barely-started
            // note in `vocabulary_coverage` — an empty design must not be shown
            // a wall of red on its first read.
            return Ok(());
        }

        // `published` OFFERS the contract; `both` offers AND needs one. Both
        // are promises somebody outside may rely on, so both are billed.
        // `required` is a promise somebody ELSE made and is not this design's
        // to prove; `internal` is plumbing the owner may change freely.
        let published: Vec<_> = interfaces
            .iter()
            .filter(|n| {
                matches!(
                    n.properties
                        .get("designation")
                        .and_then(dynograph_core::Value::as_str),
                    Some("published" | "both")
                )
            })
            .collect();

        if published.is_empty() {
            let total = interfaces.len();
            gaps.push(GapCandidate {
                id: gap_id(GapSource::NoPublishedBoundary, &[]),
                gap_source: GapSource::NoPublishedBoundary,
                scope: GapScope::Project,
                // Low, and aggregate. Nothing here is wrong — publishing
                // nothing is a legitimate posture for a design with no
                // consumers. What is missing is whether anybody chose it.
                severity: 0.3,
                title: format!(
                    "{total} boundary(ies) recorded and none is designated published — deliberate, \
                     or unclassified?"
                ),
                description: format!(
                    "This design records {total} boundary(ies) and designates none of them as \
                     published, so nothing here promises anything to anyone outside. That may be \
                     exactly right. But `designation` DEFAULTS to internal, so a boundary nobody \
                     classified and one deliberately kept internal are stored identically, and \
                     this cannot tell them apart. Which is it? Mark the ones others may rely on \
                     with set_interface_designation, or acknowledge this once if this design \
                     publishes nothing on purpose."
                ),
                affected_ids: Vec::new(),
                suggested_depth: 1,
                evidence: format!(
                    "{total} Interface node(s), 0 with designation `published` or `both`. The \
                     property defaults to `internal`, so this count cannot distinguish a settled \
                     choice from an unclassified boundary — which is why the question is asked \
                     rather than answered."
                ),
            });
            return Ok(());
        }

        for n in published {
            // `dec:passing-is-verified`: a `planned` check must not silence the
            // question, or this becomes the green-washing it exists to catch.
            if self.has_passing_verification(&n.node_id)? {
                continue;
            }
            let name = node_name(n);
            let attached = self.incoming(&n.node_id, Some(edge::VERIFIES))?.len();
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::UnverifiedPublishedContract,
                    std::slice::from_ref(&n.node_id),
                ),
                gap_source: GapSource::UnverifiedPublishedContract,
                scope: GapScope::Project,
                // Level with `unverified_enforced_rule` (0.6) and above
                // `unverified_capability` (0.55), on that finding's own
                // reasoning: an unproven capability is work not yet confirmed,
                // while this is an obligation to somebody else that nobody can
                // observe compliance with. Publishing is what raises it — an
                // internal boundary with no check is not this finding.
                severity: 0.6,
                title: format!("Nothing shows the published contract “{name}” holds"),
                description: format!(
                    "“{name}” is designated published — this design offers it and others are \
                     entitled to rely on it — but no passing check verifies it. What exercises \
                     this boundary? If it is not actually a published surface, \
                     set_interface_designation says so."
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: if attached == 0 {
                    format!(
                        "Interface '{}' is designated `{}` and has 0 incoming VERIFIES. Note that \
                         VERIFIES could not reach an Interface at all before 2026-08-21, so an \
                         older design has none for a reason that is not neglect.",
                        n.node_id,
                        n.properties
                            .get("designation")
                            .and_then(dynograph_core::Value::as_str)
                            .unwrap_or("published"),
                    )
                } else {
                    format!(
                        "Interface '{}' has {attached} incoming VERIFIES, none of them passing; a \
                         check that has not passed shows nothing.",
                        n.node_id
                    )
                },
            });
        }
        Ok(())
    }

    /// The AGREEMENT AXES a published contract has left unset
    /// (`GapSource::IncompletePublishedContract`), serving
    /// `req:interface-spec-complete`.
    ///
    /// Separate from `detect_unverified_published_contracts` above even though
    /// both walk the published set, because they ask different questions and a
    /// design can be in either state independently: a fully specified contract
    /// with no check, and a checked contract nobody described. Collapsing them
    /// would make one finding answer two questions and one acknowledgement
    /// settle both.
    fn detect_incomplete_published_contracts(
        &self,
        gaps: &mut Vec<GapCandidate>,
    ) -> Result<(), DynoError> {
        // The five characteristics `req:interface-spec-complete` names that the
        // schema actually has fields for, paired with the words the requirement
        // uses — so the finding speaks the need's language rather than the
        // column names.
        const AXES: &[(&str, &str)] = &[
            ("medium", "the technology it runs over"),
            ("paradigm", "synchronous or event-driven"),
            ("payload_format", "how the payload is serialized"),
            (
                "payload_schema",
                "which fields are mandatory, and their types",
            ),
            ("endpoint", "where a request goes"),
            ("operations", "what actions are permitted"),
            ("auth", "how identity is verified"),
            ("transport_security", "how data is protected in transit"),
            ("error_model", "the failure vocabulary a consumer parses"),
        ];

        for n in self.scan_nodes(node::INTERFACE)? {
            if !matches!(
                n.properties
                    .get("designation")
                    .and_then(dynograph_core::Value::as_str),
                Some("published" | "both")
            ) {
                continue;
            }
            let missing: Vec<&(&str, &str)> = AXES
                .iter()
                .filter(|(field, _)| {
                    match n
                        .properties
                        .get(*field)
                        .and_then(dynograph_core::Value::as_str)
                    {
                        None | Some("") => true,
                        // 🛑 `unspecified` IS UNSET, AND MISSING THIS WOULD HAVE
                        // MADE THE WHOLE DETECTOR GREEN OVER THE THING IT
                        // EXISTS TO CHECK. Five of these nine axes are enums
                        // DEFAULTING to `unspecified`, and the store
                        // materialises defaults on write — so every Interface
                        // ever created already carries the word, and a
                        // presence test would have counted it as an answer.
                        // Measured before the fix: `ifc:mcp-tools-http` read
                        // 9 of 9 complete while its `medium` said
                        // `unspecified`, and `ifc:graph-export` reported four
                        // gaps instead of six.
                        Some("unspecified") => true,
                        // ...but `none` IS AN ANSWER and must never be swept up
                        // with it. `auth: none` and `transport_security: none`
                        // are what an unauthenticated local pipe honestly says,
                        // and `ifc:mcp-tools` says exactly that. Conflating a
                        // declared absence with an undeclared one is the same
                        // error `Artifact.audience` refuses by having no
                        // default at all.
                        Some(_) => false,
                    }
                })
                .collect();
            if missing.is_empty() {
                continue;
            }
            let name = node_name(&n);
            let listed = missing
                .iter()
                .map(|(f, gloss)| format!("`{f}` ({gloss})"))
                .collect::<Vec<_>>()
                .join(", ");
            gaps.push(GapCandidate {
                id: gap_id(
                    GapSource::IncompletePublishedContract,
                    std::slice::from_ref(&n.node_id),
                ),
                gap_source: GapSource::IncompletePublishedContract,
                scope: GapScope::Project,
                // Below `unverified_published_contract` (0.6) deliberately: a
                // published promise with NO evidence at all is a stronger
                // finding than one whose description has gaps. Above the
                // ordinary 0.3 notes because an undescribed seam is what makes
                // two designs uncheckable against each other.
                severity: 0.45,
                title: format!(
                    "The published contract “{name}” does not say {} of the things two systems \
                     must agree on",
                    missing.len()
                ),
                description: format!(
                    "“{name}” is designated published, so another system may be built against it \
                     — but it does not record {listed}. Two designs cannot be checked for \
                     INCOMPATIBILITY at a seam unless the seam is described in comparable terms, \
                     so an unrecorded axis is one nothing can compare. Fill in what applies with \
                     set_interface_spec. If an axis is genuinely meaningless here — a library has \
                     no endpoint, a one-way feed has no error model a consumer parses — say so \
                     once and this will not be asked again."
                ),
                affected_ids: vec![n.node_id.clone()],
                suggested_depth: 2,
                evidence: format!(
                    "Interface '{}' has {} of {} agreement axes unset: {}. NOTE THE LIMIT: \
                     `req:interface-spec-complete` names SIX characteristics and the schema has \
                     fields for five — there is nowhere to record performance and constraints \
                     (rate limits, concurrency, timeouts), so filling every axis above still \
                     leaves the sixth unsaid.",
                    n.node_id,
                    missing.len(),
                    AXES.len(),
                    missing
                        .iter()
                        .map(|(f, _)| *f)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        Ok(())
    }

    /// A Requirement delivered only by artifacts declared `internal`
    /// (`GapSource::InternalOnlyDelivery`).
    ///
    /// The thread is Requirement <-SATISFIES- Capability <-REALIZES- Artifact.
    /// A requirement fires when it HAS delivering artifacts, at least one of
    /// them declares an audience, and EVERY declared one says `internal`.
    ///
    /// # Why an undeclared artifact does not clear the finding, and does not cause it
    ///
    /// Unknown is a true answer, so an artifact with no audience is evidence of
    /// nothing in either direction: it neither proves a consumer is served nor
    /// proves one is not. Requiring every artifact to be declared would make
    /// the finding unreachable in practice; letting an undeclared one COUNT as
    /// internal would invent a claim nobody made. So the rule reads only the
    /// declared ones, and the count of undeclared siblings goes in the evidence
    /// where a reader can weigh it.
    ///
    /// # Nothing to run on is not clean — and where that is actually reported
    ///
    /// If the design declares no audience anywhere, this returns without a
    /// finding.
    ///
    /// 🛑 THIS COMMENT USED TO SAY the silence was "reported by
    /// `audience_population`". **No such thing has ever existed** — the name
    /// appeared in this sentence and nowhere else in the codebase, and it was
    /// found by grepping for it while building the sibling detector below.
    /// A doc comment naming a mechanism that does not exist is worse than one
    /// admitting a hole: it answers the reviewer's question and stops the
    /// search. Corrected 2026-08-22 rather than deleted, because the false
    /// claim is the interesting part.
    ///
    /// What is true: the silence is REPORTABLE, not REPORTED. Since #296
    /// `vocabulary_coverage` names an unused `Artifact.audience` in its
    /// `unused` list — but that list is withheld unless asked for, so nothing
    /// puts it in front of a reader unprompted. See the note in the body.
    fn detect_internal_only_delivery(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        let mut audience_of: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for art in self.scan_nodes(node::ARTIFACT)? {
            if let Some(a) = art
                .properties
                .get("audience")
                .and_then(dynograph_core::Value::as_str)
            {
                audience_of.insert(art.node_id.clone(), a.to_string());
            }
        }
        // NO "nothing to run on" GUARD HERE, AND ITS ABSENCE IS DELIBERATE.
        // One was written and a mutation proved it DEAD: with nothing declared
        // no requirement can reach a non-empty `declared_internal`, so the
        // early return changed no outcome. A surviving mutation is not always
        // a weak test — here it exposed redundant code, and the honest fix was
        // to delete it rather than keep a guard that reads as protection.
        //
        // WHERE THE SILENCE IS REPORTED, and it is deliberately NOT here.
        // `req:work-says-whether-it-reaches-a-consumer` requires that a design
        // which has declared NO audience reads as "cannot judge", never as
        // clean. This detector cannot say it: with nothing classified there is
        // no per-requirement question to ask, and inventing one would assert a
        // claim nobody made.
        //
        // `vocabulary_coverage` was the obvious home and DID NOT FIT — its
        // `unused` list named node types and edge types only, so an unused
        // PROPERTY was counted in `properties_on_used_types` and never named.
        // That is principle B, a set of named things reduced to a scalar,
        // inside reflow2's own instrument; it was found by asserting the
        // opposite in a test and watching it fail, and it was FIXED AT THE ROOT
        // on 2026-08-22 rather than special-cased for this one field. The
        // report now names the properties it counts, so an undeclared
        // `Artifact.audience` comes back as `build: node property
        // Artifact.audience` — and so does every other field no project ever
        // filled in.
        //
        // 🛑 THE LIMIT THAT REMAINS, stated because rounding it up to "closed"
        // is the easy mistake: that naming rides the FLAT LIST, which is
        // withheld unless asked for, on measured grounds. So the silence is
        // reportable rather than reported — the default reply says the domain's
        // properties are under-filled without saying which one. Pinned by
        // `tests/internal_only_delivery.rs`, which asserts both halves.

        for req in self.scan_nodes(node::REQUIREMENT)? {
            // A need the user settled OUT is not a need. Dropping or deferring
            // something is their word too.
            let status = req
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("proposed");
            if matches!(status, "dropped" | "deferred") {
                continue;
            }

            let mut declared_internal: Vec<String> = Vec::new();
            let mut declared_consumer = false;
            let mut undeclared = 0usize;
            for sat in self.incoming(&req.node_id, Some(edge::SATISFIES))? {
                for real in self.incoming(&sat.from_id, Some(edge::REALIZES))? {
                    match audience_of.get(&real.from_id).map(String::as_str) {
                        Some("consumer") => declared_consumer = true,
                        Some("internal") => declared_internal.push(real.from_id.clone()),
                        _ => undeclared += 1,
                    }
                }
            }
            if declared_consumer || declared_internal.is_empty() {
                continue;
            }

            let n = declared_internal.len();
            let mut affected = vec![req.node_id.clone()];
            affected.extend(declared_internal.iter().cloned());
            let name = req
                .properties
                .get("name")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or(&req.node_id)
                .to_string();
            gaps.push(GapCandidate {
                id: gap_id(GapSource::InternalOnlyDelivery, &affected),
                gap_source: GapSource::InternalOnlyDelivery,
                scope: GapScope::Project,
                // Above the practice-shaped findings and below anything
                // reporting a contradiction. Nothing here is proven wrong: the
                // need may be genuinely internal. What is worth asking is
                // whether a user-facing need was closed with an internal fix,
                // which is a specific and expensive mistake.
                severity: 0.45,
                title: format!("Only internal work delivers “{name}” — nothing a consumer reaches"),
                description: format!(
                    "Everything delivering this need is declared as serving the project's own \
                     machinery rather than its users: {n} internal deliverable(s) and nothing \
                     marked `consumer`. That may be exactly right — plenty of needs are \
                     internal, and reflow2 does not judge which. It is worth one look because \
                     the opposite case is a specific and expensive mistake: closing a \
                     user-facing need with a fix that only reaches your own tooling looks like \
                     completion, and the gap it leaves belongs to everybody downstream. Either \
                     mark the deliverable that a user actually reaches with \
                     `set_artifact_intent(audience: consumer)`, or acknowledge this once."
                ),
                affected_ids: affected,
                suggested_depth: 2,
                evidence: format!(
                    "Requirement '{}' is satisfied only through artifacts declared \
                     `audience: internal` ({n} of them), with none declared `consumer` and \
                     {undeclared} carrying no audience at all. An undeclared artifact is read \
                     as evidence of NEITHER side — unknown is a true answer and is never \
                     counted as internal.",
                    req.node_id
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

    /// KPP violations — inviolable intent, checked rather than remembered.
    ///
    /// A key performance parameter is a Constraint with `category: kpp`: a
    /// threshold that, if missed, fails the effort regardless of how well
    /// everything else went. The whole point is that it is COMPUTED. A KPP
    /// nobody checks is a comment, and a comment is exactly what gets traded
    /// away in the tenth iteration cycle by someone who never read it.
    ///
    /// Three findings, ranked above ordinary gaps because a breached KPP is not
    /// a thinness in the design — it is the design failing:
    ///
    /// - **unbound** — it constrains nothing, so it can never be violated. The
    ///   quietest failure: permanently green while asserting something vital.
    /// - **breached** — the budget rollup has crossed the threshold. The only
    ///   one that is arithmetic rather than judgement, and it reuses
    ///   `budget_report` wholesale.
    /// - **contradicted** — an *accepted* Decision reaches what the KPP binds.
    ///   Surfaced for review and deliberately NOT asserted as a violation:
    ///   whether that decision actually costs the KPP is semantic, and deciding
    ///   it automatically is the judgement `dec:report-dont-judge` forbids.
    fn detect_kpp_violations(&self, gaps: &mut Vec<GapCandidate>) -> Result<(), DynoError> {
        for c in self.scan_nodes(node::CONSTRAINT)? {
            if c.properties
                .get("category")
                .and_then(dynograph_core::Value::as_str)
                != Some("kpp")
            {
                continue;
            }
            let name = node_name(&c);
            let bound: Vec<String> = self
                .outgoing(&c.node_id, Some(edge::CONSTRAINS))?
                .into_iter()
                .map(|e| e.to_id)
                .collect();

            if bound.is_empty() {
                let affected = vec![c.node_id.clone()];
                gaps.push(GapCandidate {
                    id: gap_id(GapSource::KppUnbound, &affected),
                    gap_source: GapSource::KppUnbound,
                    scope: GapScope::Capability,
                    severity: 0.9,
                    title: format!("Key performance parameter “{name}” binds nothing"),
                    description: format!(
                        "“{name}” is a key performance parameter — if it is missed the effort \
                         fails — but it constrains no part of the design, so nothing can ever \
                         violate it. What must meet it?"
                    ),
                    evidence: format!(
                        "Constraint '{}' has category=kpp and 0 outgoing CONSTRAINS.",
                        c.node_id
                    ),
                    affected_ids: affected,
                    suggested_depth: 2,
                });
                // An unbound KPP cannot be breached or contradicted either —
                // both need something bound to reason about. Reporting all
                // three would be one fault counted three times.
                continue;
            }

            let report = self.budget_report(&c.node_id)?;
            if report.verdict == crate::budget::BudgetVerdict::Exceeded {
                let mut affected = vec![c.node_id.clone()];
                affected.extend(bound.iter().cloned());
                affected.sort();
                let limit = report
                    .limit
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "?".into());
                gaps.push(GapCandidate {
                    id: gap_id(GapSource::KppBreached, &affected),
                    gap_source: GapSource::KppBreached,
                    scope: GapScope::Project,
                    severity: 0.95,
                    title: format!("Key performance parameter “{name}” is breached"),
                    description: format!(
                        "“{name}” is a threshold the effort fails without, and the design has \
                         crossed it: the stated contributions total {} against a {} of {}. This \
                         is not a gap to weigh against others — either the design changes or the \
                         parameter was wrong.",
                        report.total, report.direction, limit
                    ),
                    evidence: format!(
                        "budget_report('{}') = Exceeded: total {} vs limit {} ({}), over {} \
                         contributor(s).",
                        c.node_id,
                        report.total,
                        limit,
                        report.direction,
                        report.contributors.len()
                    ),
                    affected_ids: affected,
                    suggested_depth: 2,
                });
            }

            // Accepted decisions that reach what this KPP binds. `proposed`
            // ones are excluded on purpose: an open choice has not traded
            // anything away yet, and flagging it would punish thinking out loud.
            let mut reaching: Vec<String> = Vec::new();
            for target in &bound {
                // GOVERNED_BY runs FROM the governed node TO the Decision, so
                // the decisions shaping a target are its OUTGOING edges.
                for e in self.outgoing(target, Some(edge::GOVERNED_BY))? {
                    let dec_id = e.to_id;
                    if reaching.contains(&dec_id) {
                        continue;
                    }
                    if let Some(dec) = self.get_node(node::DECISION, &dec_id)?
                        && dec
                            .properties
                            .get("status")
                            .and_then(dynograph_core::Value::as_str)
                            == Some("accepted")
                    {
                        reaching.push(dec_id);
                    }
                }
            }
            if !reaching.is_empty() {
                reaching.sort();
                let mut affected = vec![c.node_id.clone()];
                affected.extend(reaching.iter().cloned());
                gaps.push(GapCandidate {
                    id: gap_id(GapSource::KppContradicted, &affected),
                    gap_source: GapSource::KppContradicted,
                    scope: GapScope::Capability,
                    severity: 0.85,
                    title: format!(
                        "{} accepted decision(s) govern what “{name}” binds",
                        reaching.len()
                    ),
                    description: format!(
                        "“{name}” must hold no matter what else is decided, and {} accepted \
                         decision(s) shape the very parts it binds. Confirm each still leaves it \
                         intact — this is a prompt to check, not a claim that it is broken.",
                        reaching.len()
                    ),
                    evidence: format!(
                        "KPP '{}' CONSTRAINS {} node(s) governed by accepted decision(s): {}.",
                        c.node_id,
                        bound.len(),
                        reaching.join(", ")
                    ),
                    affected_ids: affected,
                    suggested_depth: 2,
                });
            }
        }
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
