//! Graph report — the **SYNTHESIZE** rollup: graph → a human artifact
//! (docs/overview.md "SYNTHESIZE"; graph-analysis.md "Graph report").
//!
//! A one-shot "what should I look at?" summary that aggregates what the other
//! deterministic analyses already compute — a design snapshot, the top ranked
//! [`GapCandidate`]s (DETECT), allocation health (`evaluate_allocation`),
//! surprising couplings (`surprising_connections`), and declining quality
//! (`dimension_drifts`) — into one [`GraphReport`] that renders to Markdown.
//!
//! Pure aggregation, no LLM: it reuses the deterministic analyses and never
//! silently truncates — every capped list reports how many more there were.

use std::fmt::Write as _;

use dynograph_core::DynoError;

use crate::allocate::AllocationReport;
use crate::detect::GapCandidate;
use crate::dimensions::{DimensionDrift, DriftDirection};
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};
use crate::surprises::SurprisingConnection;

/// How many items each highlight list caps at (the rest are counted, not shown).
const TOP_N: usize = 5;

/// Design node types included in the snapshot, in lifecycle order.
const SNAPSHOT_TYPES: &[&str] = &[
    node::PROJECT,
    node::REQUIREMENT,
    node::CONSTRAINT,
    node::DESIGN_RULE,
    node::CAPABILITY,
    node::FLOW,
    node::ACTOR,
    node::COMPONENT,
    node::INTERFACE,
    node::DECISION,
    node::ARTIFACT,
    node::VERIFICATION,
    node::RELEASE,
    node::ENVIRONMENT,
    node::RESOURCE,
];

/// The `status` × `provenance` → certainty mapping, on a node already in
/// hand. Absent properties take their schema defaults (`proposed`,
/// `authored`), so a bare requirement reads as asserted, never as confirmed.
/// One string property, or `fallback` when absent — the schema default, so an
/// unset field is read the way the schema says it would be.
fn prop_str<'a>(n: &'a dynograph_storage::StoredNode, key: &str, fallback: &'a str) -> &'a str {
    n.properties
        .get(key)
        .and_then(dynograph_core::Value::as_str)
        .unwrap_or(fallback)
}

fn certainty_of(req: &dynograph_storage::StoredNode) -> RequirementCertainty {
    let status = req
        .properties
        .get("status")
        .and_then(dynograph_core::Value::as_str)
        .unwrap_or("proposed");
    match status {
        "accepted" | "met" => RequirementCertainty::UserConfirmed,
        "deferred" | "dropped" => RequirementCertainty::SettledOut,
        _ => {
            let provenance = req
                .properties
                .get("provenance")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or("authored");
            match provenance {
                "inferred" | "reconciled" | "healed" => RequirementCertainty::Recovered,
                _ => RequirementCertainty::Asserted,
            }
        }
    }
}

/// Whether a node type is design content, as opposed to the supporting layer
/// (provenance, questions, history). The same split the graph report's
/// snapshot draws — `compare` reuses it so "design vs supporting" means one
/// thing everywhere.
pub(crate) fn is_design_type(node_type: &str) -> bool {
    SNAPSHOT_TYPES.contains(&node_type)
}

/// How firmly a Requirement stands — derived from `status` × `provenance`,
/// never stored (BL-75, `dec:certainty-derived`). The two axes already span
/// the space; a third stored property could contradict them both. The
/// load-bearing doctrine that makes this derivable: an agent captures
/// requirements at `proposed`, and ONLY the user's answer moves the status —
/// `accepted`, `met`, `deferred` and `dropped` are all user-only verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementCertainty {
    /// The user said yes to this — status `accepted` or `met`.
    UserConfirmed,
    /// Someone stated it and the user has not yet confirmed the wording —
    /// status `proposed`, provenance `authored`/`planned`/`imported`.
    Asserted,
    /// Read back out of an existing system and not yet put to the user —
    /// status `proposed`, provenance `inferred`/`reconciled`/`healed`. A
    /// recovered requirement is satisfied by construction and can never
    /// contradict anything, which is exactly why its certainty must be
    /// visible.
    Recovered,
    /// The user decided it *out* — status `deferred` or `dropped`. Also
    /// their word; not uncertainty.
    SettledOut,
}

/// The certainty breakdown the snapshot renders — so no session reconstructs
/// "which of these did the user actually confirm?" in prose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CertaintyBreakdown {
    pub user_confirmed: usize,
    pub asserted: usize,
    pub recovered: usize,
    pub settled_out: usize,
}

/// How much of the design carries its own verification.
///
/// A *signal*, not a gap. An unverified Capability is asked about — nothing
/// proves that behaviour works. A file with no `VERIFIES` edge of its own is
/// merely worth knowing: demanding one per source file produced 22 of 25 gaps
/// on reflow2's own design, all on a crate whose capabilities are tested
/// (BL-23). The number is reported so anyone who does want per-file rigour can
/// see where they stand.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationCoverage {
    pub capabilities: usize,
    /// Capabilities with at least one incoming `VERIFIES`.
    pub capabilities_verified: usize,
    /// Capabilities with no passing check of their own whose allocated
    /// component carries one — verified at component granularity (BL-73).
    /// Neither `verified` nor unchecked: the state that made a tested
    /// brownfield system read as "0/20 verified" when it was invisible.
    pub capabilities_component_verified: usize,
    pub artifacts: usize,
    /// Artifacts with a `VERIFIES` edge of their own, as opposed to being
    /// covered by the capability they realize.
    pub artifacts_verified: usize,
}

impl VerificationCoverage {
    /// True when there is nothing to report — no capabilities and no artifacts.
    fn is_empty(&self) -> bool {
        self.capabilities == 0 && self.artifacts == 0
    }
}

/// How much of the design has something built for it.
///
/// The counting half of BL-42, and the same bargain as
/// [`VerificationCoverage`]: `unrealized_capability` asks only where the build
/// demonstrably arrived and skipped a capability. Capabilities in a region the
/// artifact layer has not reached at all are **counted here, never asked
/// about** — the storyflow adopt trial turned that question into 13 of 51
/// gaps, every one a consequence of modelling artifacts coarsely on purpose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RealizationCoverage {
    pub capabilities: usize,
    /// Capabilities with an artifact realizing them, or realizing a component
    /// they are allocated to (both P3 shapes, per BL-38).
    pub realized: usize,
    /// Capabilities with no artifact, whose owning component is nonetheless
    /// marked `realized` — the modeller asserts these exist and simply has
    /// not modelled a file for them. Not a gap: a statement about how much
    /// of a built system the artifact layer covers.
    pub built_but_unmodelled: usize,
}

/// How much of the intent has actually been **delivered** — derived from the
/// golden thread, never read from a field (BL-104).
///
/// `Requirement.status` carries a `met` value, and it is the wrong place to
/// learn this from: a hand-set "done" is a claim that outlives the truth. It
/// survives the capability regressing, the artifact drifting and the check
/// starting to fail, degrading exactly as silently as an unreconciled checksum.
/// So delivery is computed the way component-granularity verification already
/// is — a state the report works out, not a property anyone maintains — which
/// is what makes progress *fall out* of the thread rather than being asserted
/// on top of it.
///
/// Two subtleties, both load-bearing:
///
/// - **A failing check un-delivers.** Delivery requires the satisfying
///   capability to be realized *and* currently checked, so a requirement that
///   was delivered and whose test later fails stops counting. A derivation that
///   cannot go backwards is just a slower assertion.
/// - **Inference is not evidence** (`inferred_only`). A requirement recovered
///   *from the code implementing it* is satisfied by construction and can never
///   contradict anything — the schema says so on `Requirement.provenance`. If
///   those counted, a brownfield adopt would report itself fully delivered on
///   arrival, having demonstrated nothing. They are counted apart instead, so
///   the number is visible without inflating the headline.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeliveryCoverage {
    /// Requirements in scope — every requirement not `dropped`, since a
    /// withdrawn need is not unfinished work.
    pub requirements: usize,
    /// Requirements with at least one capability claiming to satisfy them.
    /// Already what `unsatisfied_requirement` asks about; repeated here so
    /// "12 delivered" can be read against "of 20 satisfied", not against silence.
    pub satisfied: usize,
    /// Satisfied by a capability that is realized AND carries a passing check,
    /// at its own or component granularity. The honest answer to "how much of
    /// this is actually done?".
    pub delivered: usize,
    /// Satisfied, but only by capabilities whose requirement was itself
    /// recovered by inference — excluded from `delivered` on purpose. See the
    /// type docs.
    pub inferred_only: usize,
}

impl DeliveryCoverage {
    /// True when there is no intent to report on.
    fn is_empty(&self) -> bool {
        self.requirements == 0
    }
}

/// The confirmation rollup — counts only; the full ledger is
/// [`DesignGraph::confirmation_ledger`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfirmationSummary {
    pub drifting: usize,
    pub confirmed: usize,
    pub unexamined: usize,
}

/// Allocation health at a glance (from `evaluate_allocation`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct AllocationSummary {
    /// Components with at least one capability.
    pub component_count: usize,
    /// Cohesion/coupling modularity (1.0 = perfectly cohesive).
    pub modularity: f64,
    /// Capabilities coupled more strongly across a boundary than within.
    pub misplaced_count: usize,
    /// Routing-hub components (selective SPOF).
    pub god_components: Vec<String>,
}

/// What a check says, and **when it last said it**.
///
/// `status` is a measurement taken at an instant; every surface that reported it
/// without its timestamp presented it as a standing property of the system. That
/// cost one fleet twice in a single shift (2026-07-27), in both directions:
///
/// * a verification read `passing` while the service behind it was 100% dead —
///   the status had been recorded from a transcript and never re-run;
/// * two others read `failing` for 24 capabilities on a run that predated the
///   fixes by three days.
///
/// The second was found in minutes because a failing check raises a gap. **The
/// first was found by accident, because a passing check raises nothing at all** —
/// so the silent half is the dangerous half, and it is the half this type exists
/// for. An audit of that graph found ELEVEN verifications sharing one batch
/// timestamp, nine of them green and covering 26 targets that no detector would
/// ever have mentioned.
///
/// **No clock is consulted.** The recorded time is surfaced verbatim and the
/// reader compares two dates. Deriving "3 days ago" would put `now` into a
/// report that callers diff and test against, and non-determinism is a worse
/// trade than a human subtraction.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationRecency {
    pub verification_id: String,
    pub name: String,
    /// `planned` / `passing` / `failing` / `skipped` / `blocked`.
    pub status: String,
    /// When it last ran. `None` means it never did — and a `passing` or
    /// `failing` with `None` is an **assertion**, not a measurement.
    pub last_run_at: Option<String>,
    /// How many nodes this check speaks for. A stale check with a large fan-out
    /// is asserting more than one about a single capability.
    pub verifies: usize,
}

/// The rolled-up state of the design graph.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphReport {
    /// `(node type, count)` for design types present, lifecycle order.
    pub node_counts: Vec<(&'static str, usize)>,
    /// `(node type, count)` for every *other* populated type — provenance
    /// (`Fragment`), questions, drift events, axis-Z machinery, dimension
    /// readings. Itemised rather than omitted: these are real nodes, and a
    /// total that skipped them made a 122-node graph report 109 (BL-43).
    pub other_counts: Vec<(String, usize)>,
    /// Design nodes only — the sum of `node_counts`.
    pub design_nodes: usize,
    /// **Every** node in the graph: `design_nodes` plus `other_counts`.
    pub total_nodes: usize,
    /// Total open gaps (DETECT).
    pub gap_count: usize,
    /// Total structural defects (HEAL).
    pub defect_count: usize,
    /// The highest-severity gaps (capped at [`TOP_N`]).
    pub top_gaps: Vec<GapCandidate>,
    /// Gaps beyond the shown top (never silently dropped).
    pub gaps_truncated: usize,
    /// Every check with its status AND when it last ran. A green report that
    /// rests on a three-day-old run should say so on its face.
    pub verifications: Vec<VerificationRecency>,
    /// Allocation health, when components exist.
    pub allocation: Option<AllocationSummary>,
    /// Where the design sits on the function-to-structure trajectory (BL-179).
    /// A position, never a verdict — and deliberately carrying no statement of
    /// where the design *should* be.
    pub maturity: crate::maturity::MaturityProfile,
    /// Artifacts whose build granularity is out of line with the design's own
    /// (BL-182). An observation, never a defect: it says the build holds as one
    /// thing what the design holds as several, and rules on neither side.
    pub granularity: Vec<crate::granularity::GranularityObservation>,
    /// The median capabilities-per-artifact the observations above are read
    /// against, so "10" is never shown without "of a median 1".
    pub granularity_median: f64,
    /// The most surprising couplings (capped).
    pub surprising: Vec<SurprisingConnection>,
    /// Surprising couplings beyond the shown top.
    pub surprising_truncated: usize,
    /// Which requirements the user actually confirmed, which are asserted,
    /// which were recovered from an artifact (BL-75). `None` when there are
    /// no requirements.
    pub requirement_certainty: Option<CertaintyBreakdown>,
    /// How much of the design carries its own verification (a signal, not a gap).
    pub verification: VerificationCoverage,
    /// How much of the design has something built for it, and how much the
    /// artifact layer does not reach (a signal, not a gap — BL-42).
    pub realization: RealizationCoverage,
    /// How much of the intent is actually **delivered**, derived from the
    /// golden thread rather than read from `Requirement.status` (BL-104).
    /// `None` when there are no requirements.
    pub delivery: Option<DeliveryCoverage>,
    /// Confirmation rollup (BL-35): of the capabilities with realizing
    /// artifacts, how many are drifting / confirmed / **unexamined** — the
    /// last being the state the original reflow died in: nobody looked, and
    /// nothing could tell.
    pub confirmation: Option<ConfirmationSummary>,
    /// Declining quality dimensions, worst first (capped).
    pub declining: Vec<DimensionDrift>,
    /// Declining dimensions beyond the shown top.
    pub declining_truncated: usize,
}

/// The coherence loop's outstanding debt — what CHANGE→DETECT→SURFACE→RESOLVE
/// steps are *owed*, computed from graph state alone (BL-74).
///
/// Deliberately state, never run-history (`dec:loop-status-state-not-history`):
/// the core takes no clock and looking is not writing, so "you haven't run
/// detect_gaps since Tuesday" is not an honest computation — but "3 open gaps
/// were never put to the user" is, and it is also the thing that actually
/// matters. The field lesson this answers: under operational load, bookkeeping
/// via the raw tools continued while the loop silently stopped, because
/// nothing cheap said what was owed. This is that cheap thing — one call, a
/// to-do list, no skill loaded.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoopStatus {
    /// Open *anchored* gaps DETECT finds right now that no Question was ever
    /// asked about — intent was captured and never surfaced to the user.
    /// Phase nudges don't count: they say what comes next, not what is owed.
    pub unsurfaced_gaps: usize,
    /// Questions put to the user, still waiting on them.
    pub unanswered_questions: usize,
    /// Questions the user answered whose gap is still open — the answer never
    /// reached the design (write it in, or acknowledge the gap).
    pub unwritten_answers: usize,
    /// Decisions left `proposed` that a NAMED person has been asked to settle —
    /// an `AUTHORED_BY` edge carrying `role=approver`.
    ///
    /// The discriminator is the approver edge, and it is the whole point. A
    /// `proposed` Decision with no approver is somebody THINKING OUT LOUD: the
    /// brainstorm skill records a musing exactly that way so the loop stays
    /// quiet while an idea is still forming, and that quiet is deliberate and
    /// worth keeping. A `proposed` Decision WITH an approver is somebody having
    /// been ASKED, and nothing else in the design will ever remind them.
    ///
    /// Before this counter the graph held both states, told them apart
    /// structurally, and reported neither — measured twice on two designs
    /// (`req:an-assigned-open-decision-is-reported`): eight open decisions on
    /// one, `detect_gaps` returning nothing about any of them, and
    /// `loop_status` with no field that could. `undecided_decision_point` does
    /// not cover this: it reasonably wants two or more REGISTERED alternatives,
    /// each with a design export behind it, and a decision whose options are
    /// prose has none — so making it fire would mean inventing file paths that
    /// do not exist.
    pub unsettled_assigned_decisions: usize,
    /// Structural defects HEAL reports right now.
    pub structural_defects: usize,
    /// Capabilities claiming `realized`/`verified` with no passing check.
    pub unproven_capabilities: usize,
    /// Recorded divergences (`DriftEvent`) awaiting a disposition.
    pub undispositioned_drift: usize,
    /// Built capabilities nobody has ever checked against reality
    /// (the confirmation ledger's `unexamined`).
    pub unexamined_claims: usize,
    /// The debt as ordered to-do lines, most blocking first. Empty when the
    /// loop is clean — and emptiness is asserted, not implied.
    pub next: Vec<String>,
    /// Every check with its status AND when it last ran, so a reader can see a
    /// stale verdict without it having to become a gap first. Deliberately does
    /// NOT feed `clean`: this is visibility, not a new nag — a counter here
    /// would make `clean` unreachable on any design whose last run was
    /// yesterday, which is the permanently-red-check failure rebuilt.
    pub verifications: Vec<VerificationRecency>,
    /// The assigned decisions THEMSELVES, not only how many.
    ///
    /// `unsettled_assigned_decisions` counted them and nothing listed them, so
    /// the one debt line with an obvious owner was the one with no obvious next
    /// call: every other line names a tool (`detect_gaps`, `detect_defects`)
    /// while this one left the reader to walk `AUTHORED_BY` edges by hand.
    /// Reported by flo2 (F4, 2026-08-09) and hit independently in this repo the
    /// same day, where finding two of them meant `jq`-ing the committed export.
    pub assigned_decisions: Vec<AssignedDecision>,
    /// Open gaps standing on ground the scoped contributor OWNS.
    ///
    /// Empty when unscoped, and empty when the contributor owns nothing — an
    /// unowned design is the ordinary state and is not a finding. See
    /// [`GapOnOwnedGround`].
    pub gaps_on_owned_ground: Vec<GapOnOwnedGround>,
    /// Set when the report was narrowed to one contributor.
    pub scope: Option<LoopScope>,
    /// Every counter zero. **When `scope` is set this means "nothing is
    /// assigned to that person"** — not that the design is clean. `scope`
    /// carries what could not be attributed, and `next` says so in words.
    pub clean: bool,
}

/// A gap standing on ground a contributor owns.
///
/// The second thing that can be honestly attributed to a person, and the reason
/// `OWNED_BY` was built: before it, a scoped answer could speak only about
/// decisions somebody had been ASKED to settle. It names WHICH owned nodes the
/// gap touches, because "a gap in your area" is only actionable if you can see
/// which part of your area it is standing on.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GapOnOwnedGround {
    pub gap_id: String,
    pub title: String,
    /// The affected nodes this contributor owns — never the gap's whole
    /// affected set, which may reach well outside their ground.
    pub owned_ids: Vec<String>,
}

/// An open Decision somebody was explicitly asked to settle.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssignedDecision {
    pub decision_id: String,
    pub name: String,
    /// The Contributor the `AUTHORED_BY role=approver` edge points at.
    pub approver_id: String,
}

/// What a contributor-scoped answer covered, and what it could not.
///
/// The scoped report exists because `loop_status` could say what a design owes
/// in total but not what it owes a PERSON (`req:the-loop-says-what-is-owed-to-a-person`),
/// which is the axis on which `req:no-idea-goes-quiet` fails for anybody who is
/// not the one person at the keyboard.
///
/// # Why only assignment is attributed
///
/// The requirement names the trap and refuses to paper over it: *what "owed to"
/// MEANS per item type* is a real question, and "a gap on a requirement Shawn
/// raised is arguably owed to whoever must judge it, not to him". So exactly one
/// relationship is attributed here — an `AUTHORED_BY role=approver` edge, which
/// is the graph saying in structure that a NAMED person was asked. Everything
/// else is reported as design-wide and named in [`Self::not_attributable`].
///
/// **Filtering the rest to zero would be the worse bug**, and it is one this
/// project has now found four times in other guises: a value that cannot tell
/// NOTHING IS OWED from I CANNOT SAY. A person reading `clean: true` while 55
/// gaps sit in the design would be reading a lie the tool told confidently.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoopScope {
    /// The contributor asked about. Verified to exist — a mistyped id is an
    /// error, never an empty answer, because "nothing is owed to you" is
    /// exactly what a typo would produce and exactly what nobody would question.
    pub contributor_id: String,
    /// Debt classes that are real design-wide but carry no per-person
    /// attribution, with their counts. Present so a scoped answer can never be
    /// read as "the design is fine".
    pub not_attributable: Vec<String>,
}

impl DesignGraph {
    /// Derive a requirement's [`RequirementCertainty`] from its stored
    /// `status` and `provenance`. Pure derivation — see the enum for the
    /// mapping and the doctrine it rests on.
    pub fn requirement_certainty(
        &self,
        requirement_id: &str,
    ) -> Result<RequirementCertainty, DynoError> {
        let Some(req) = self.get_node(node::REQUIREMENT, requirement_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::REQUIREMENT.to_string(),
                node_id: requirement_id.to_string(),
            });
        };
        Ok(certainty_of(&req))
    }

    /// Count the certainty breakdown across every Requirement.
    pub fn requirement_certainty_breakdown(&self) -> Result<CertaintyBreakdown, DynoError> {
        let mut b = CertaintyBreakdown {
            user_confirmed: 0,
            asserted: 0,
            recovered: 0,
            settled_out: 0,
        };
        for req in self.scan_nodes(node::REQUIREMENT)? {
            match certainty_of(&req) {
                RequirementCertainty::UserConfirmed => b.user_confirmed += 1,
                RequirementCertainty::Asserted => b.asserted += 1,
                RequirementCertainty::Recovered => b.recovered += 1,
                RequirementCertainty::SettledOut => b.settled_out += 1,
            }
        }
        Ok(b)
    }

    /// Compute the loop's outstanding debt. See [`LoopStatus`].
    /// Every check with its status and the time it last ran, sorted so the
    /// output is stable to diff.
    ///
    /// One computation feeding both `loop_status` and `graph_report`: the whole
    /// point is that a reader sees recency wherever they look, and two
    /// implementations would eventually disagree about which surface tells the
    /// truth.
    pub fn verification_recency(&self) -> Result<Vec<VerificationRecency>, DynoError> {
        let mut out = Vec::new();
        for v in self.scan_nodes(node::VERIFICATION)? {
            let verifies = self.outgoing(&v.node_id, Some(edge::VERIFIES))?.len();
            out.push(VerificationRecency {
                verification_id: v.node_id.clone(),
                name: prop_str(&v, "name", &v.node_id).to_string(),
                status: prop_str(&v, "status", "planned").to_string(),
                last_run_at: v
                    .properties
                    .get("last_run_at")
                    .and_then(dynograph_core::Value::as_str)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string),
                verifies,
            });
        }
        // Failing first, then by id: the loud ones lead, and a stable order
        // means a diff of two reports shows what CHANGED, not what moved.
        out.sort_by(|a, b| {
            let rank = |s: &str| match s {
                "failing" => 0,
                "blocked" => 1,
                "skipped" => 2,
                "passing" => 3,
                _ => 4,
            };
            rank(&a.status)
                .cmp(&rank(&b.status))
                .then_with(|| a.verification_id.cmp(&b.verification_id))
        });
        Ok(out)
    }

    pub fn loop_status(&self) -> Result<LoopStatus, DynoError> {
        self.loop_status_for(None)
    }

    /// The loop's debt, optionally narrowed to what one CONTRIBUTOR was asked
    /// to settle.
    ///
    /// `None` is the whole design and is byte-identical to what `loop_status`
    /// always returned, plus the new `assigned_decisions` listing.
    ///
    /// `Some(id)` answers "what needs ME" — the screen a person working
    /// asynchronously actually wants, and the thing that today can only be
    /// produced by a human opening the design and reading it. It follows
    /// `detect_gaps`'s established shape: narrow the answer, and always say
    /// what was left out.
    ///
    /// **A scoped answer attributes assignment and nothing else** — see
    /// [`LoopScope`] for why that restraint is the design rather than a
    /// shortcut.
    pub fn loop_status_for(&self, contributor: Option<&str>) -> Result<LoopStatus, DynoError> {
        // A contributor we cannot find is an ERROR, not an empty answer. The
        // failure mode being avoided is specific: a mistyped or renamed id
        // silently produces "nothing is owed to you", which is both the most
        // reassuring possible response and the one nobody thinks to question.
        if let Some(id) = contributor
            && self.get_node(node::CONTRIBUTOR, id)?.is_none()
        {
            return Err(DynoError::NodeNotFound {
                node_type: node::CONTRIBUTOR.to_string(),
                node_id: id.to_string(),
            });
        }
        let questions = self.open_questions()?;
        let surfaced: std::collections::BTreeSet<&str> =
            questions.iter().map(|q| q.gap_id.as_str()).collect();
        // Acknowledged gaps are already absent from detect_gaps, so what
        // remains unsurfaced is: open right now, anchored to real nodes, and
        // never asked about. Anchored only — a phase nudge says what comes
        // next, not what is owed (dec:anchored-first), and counting nudges as
        // debt would make `clean` unreachable on a healthy design.
        let unsurfaced: Vec<_> = self
            .detect_gaps()?
            .into_iter()
            .filter(|g| !g.affected_ids.is_empty() && !surfaced.contains(g.id.as_str()))
            .collect();
        let unsurfaced_gaps = unsurfaced.len();

        // Gaps standing on ground this contributor OWNS. This is the second
        // thing that can honestly be attributed to a person, and the reason
        // OWNED_BY was built: before it, a scoped answer could speak only about
        // decisions somebody had been ASKED to settle, and reported every gap
        // as "I cannot tell whose this is".
        //
        // Ownership makes it answerable without guessing — if a gap's affected
        // set touches a node you own, it is on your ground. That is a fact, not
        // an inference, which is why this is safe where the reverse question
        // ("what has NO owner?") is not: dec:idea-detect-ownership-orphans is
        // open precisely because absence of an owner is ordinary.
        let owned = match contributor {
            Some(id) => self.owned_by_contributor(id)?,
            None => Default::default(),
        };
        let gaps_on_owned_ground: Vec<GapOnOwnedGround> = if owned.is_empty() {
            Vec::new()
        } else {
            unsurfaced
                .iter()
                .filter(|g| g.affected_ids.iter().any(|id| owned.contains(id)))
                .map(|g| GapOnOwnedGround {
                    gap_id: g.id.clone(),
                    title: g.title.clone(),
                    owned_ids: g
                        .affected_ids
                        .iter()
                        .filter(|id| owned.contains(*id))
                        .cloned()
                        .collect(),
                })
                .collect()
        };
        let unanswered_questions = questions.iter().filter(|q| q.status == "asked").count();
        let unwritten_answers = questions.iter().filter(|q| q.status == "answered").count();

        // A Decision that is still `proposed` AND carries an approver edge:
        // somebody was asked to decide and nothing else says so. The approver
        // edge is the discriminator — without it a `proposed` Decision is a
        // brainstorm, and staying quiet about those is deliberate.
        let mut assigned_decisions = Vec::new();
        for dec in self.scan_nodes(node::DECISION)? {
            let proposed = dec
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                == Some("proposed");
            if !proposed {
                continue;
            }
            for edge in self.outgoing(&dec.node_id, Some(edge::AUTHORED_BY))? {
                if edge
                    .properties
                    .get("role")
                    .and_then(dynograph_core::Value::as_str)
                    != Some("approver")
                {
                    continue;
                }
                // Scoping happens HERE, on the one edge that names a person,
                // and nowhere else in this function.
                if contributor.is_some_and(|want| want != edge.to_id) {
                    continue;
                }
                assigned_decisions.push(AssignedDecision {
                    decision_id: dec.node_id.clone(),
                    name: dec
                        .properties
                        .get("name")
                        .and_then(dynograph_core::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    approver_id: edge.to_id.clone(),
                });
            }
        }
        let unsettled_assigned_decisions = assigned_decisions.len();

        let structural_defects = self.detect_defects()?.len();

        // A component-granularity check clears this debt (BL-73): the claim
        // HAS a passing check, one hop away — the coverage line says at which
        // granularity, and the per-component gap asks whether that is enough.
        let mut unproven_capabilities = 0usize;
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let claims_built = cap
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .map(|s| s == "realized" || s == "verified")
                .unwrap_or(false);
            if claims_built
                && self.capability_verification(&cap.node_id)?
                    == crate::verify::CapabilityVerification::Unchecked
            {
                unproven_capabilities += 1;
            }
        }

        let undispositioned_drift = self
            .scan_nodes(node::DRIFT_EVENT)?
            .into_iter()
            .filter(|d| {
                !d.properties
                    .get("resolved")
                    .and_then(dynograph_core::Value::as_bool)
                    .unwrap_or(false)
            })
            .count();

        let unexamined_claims = self.confirmation_ledger()?.unexamined;

        let mut next = Vec::new();
        if unanswered_questions > 0 {
            next.push(format!(
                "{unanswered_questions} question(s) are waiting on the user — follow up, \
                 don't re-ask (open_questions)"
            ));
        }
        if unwritten_answers > 0 {
            next.push(format!(
                "{unwritten_answers} answered question(s) never reached the design — write \
                 the answer in, or acknowledge the gap"
            ));
        }
        if unsettled_assigned_decisions > 0 {
            next.push(format!(
                "{unsettled_assigned_decisions} open decision(s) name someone who was asked \
                 to settle them — decide (set_decision_status), or drop the approver if \
                 nobody was actually asked"
            ));
        }
        if unsurfaced_gaps > 0 {
            next.push(format!(
                "{unsurfaced_gaps} open gap(s) have never been put to the user — run \
                 detect-and-ask"
            ));
        }
        if structural_defects > 0 {
            // The IMPERATIVE names the read-only tool; the skill is named as
            // context, and its destructive step is named with it.
            //
            // This line used to read "run check-health (detect_defects)", which
            // is ambiguous in the dangerous direction: `check-health` is the
            // SKILL and its step 3 applies merges, while `detect_defects` in the
            // brackets only reads. dev_storyflow reported it five times across
            // four seats in two days — one of them filed deliberately as a
            // duplicate so recurrence would be visible — because `next` is the
            // most authoritative-feeling string in the payload to a session that
            // has just arrived, and a session that follows it literally, which
            // is what `next` is FOR, walked into ten irreversible node
            // deletions. Two of their people read past it in their own sessions
            // and said so. They ended up posting a fleet-wide stop-order at the
            // top of their worker board to counteract one string this tool
            // emits into a call every worker is required to make.
            //
            // The general rule, worth applying to any future entry here: when a
            // skill contains an irreversible step, the loop's own to-do list
            // must not name that skill as the thing to do. Note the contrast
            // with the `detect-and-ask` line above, which names a skill quite
            // deliberately — that skill only reads and asks.
            next.push(format!(
                "{structural_defects} structural defect(s) outstanding — run detect_defects to \
                 read them (the check-health skill is the wider workflow; its repair step can \
                 delete nodes, so read what it proposes before applying)"
            ));
        }
        if unproven_capabilities > 0 {
            next.push(format!(
                "{unproven_capabilities} capability(ies) claim realized/verified with no \
                 passing check — add or run their Verification"
            ));
        }
        if undispositioned_drift > 0 {
            next.push(format!(
                "{undispositioned_drift} recorded divergence(s) await a disposition \
                 (set_artifact_checksum)"
            ));
        }
        if unexamined_claims > 0 {
            next.push(format!(
                "{unexamined_claims} built capability(ies) never checked against reality \
                 (reconcile_artifacts)"
            ));
        }

        // Narrowing, and saying what the narrowing left out. Everything above
        // except `assigned_decisions` and `gaps_on_owned_ground` is a fact about
        // the DESIGN, not about a person, so a scoped answer names those counts
        // rather than zeroing them — the difference between "nothing is owed to
        // you" and "I cannot tell whose this is", which must never be the same
        // answer.
        //
        // The gap line now subtracts what ownership DID attribute, so the
        // not-attributable count shrinks as a design records more ownership.
        // That is the whole payoff of OWNED_BY on this surface.
        let gaps_not_attributable = unsurfaced_gaps.saturating_sub(gaps_on_owned_ground.len());
        let scope = contributor.map(|id| {
            let mut not_attributable = Vec::new();
            for (n, what) in [
                (gaps_not_attributable, "open gap(s) on ground nobody owns"),
                (unanswered_questions, "question(s) waiting on the user"),
                (
                    unwritten_answers,
                    "answered question(s) not yet written into the design",
                ),
                (structural_defects, "structural defect(s)"),
                (
                    unproven_capabilities,
                    "capability(ies) claiming built with no passing check",
                ),
                (
                    undispositioned_drift,
                    "recorded divergence(s) awaiting a disposition",
                ),
                (
                    unexamined_claims,
                    "built capability(ies) never checked against reality",
                ),
            ] {
                if n > 0 {
                    not_attributable.push(format!("{n} {what}"));
                }
            }
            LoopScope {
                contributor_id: id.to_string(),
                not_attributable,
            }
        });

        if let Some(s) = &scope {
            next.clear();
            if unsettled_assigned_decisions > 0 {
                next.push(format!(
                    "{unsettled_assigned_decisions} open decision(s) name {} as the person \
                     asked to settle them — decide (set_decision_status), or drop the \
                     approver if nobody was actually asked. They are listed in \
                     `assigned_decisions`.",
                    s.contributor_id
                ));
            } else {
                next.push(format!("Nothing is assigned to {}.", s.contributor_id));
            }
            if !gaps_on_owned_ground.is_empty() {
                next.push(format!(
                    "{} open gap(s) stand on ground {} owns — listed in \
                     `gaps_on_owned_ground` with the owned nodes each one touches.",
                    gaps_on_owned_ground.len(),
                    s.contributor_id
                ));
            }
            if !s.not_attributable.is_empty() {
                // Said in words rather than left to the reader to infer from a
                // field, because `next` is the line an agent reads aloud.
                next.push(format!(
                    "The rest of the loop's debt carries no per-person attribution and is \
                     NOT counted as {}'s: {}. Call loop_status without a contributor for \
                     the design-wide picture.",
                    s.contributor_id,
                    s.not_attributable.join(", ")
                ));
            }
        }

        // Scoped, `clean` means "nothing is owed by this person" — it is
        // deliberately NOT `next.is_empty()`, because a scoped answer always
        // carries the not-attributable line and would otherwise never be clean.
        //
        // Both attributable kinds count: a gap on your own ground is owed by you
        // as surely as a decision you were asked to settle, and a `clean: true`
        // that ignored it would be the same confident lie in a new place.
        let clean = match &scope {
            Some(_) => unsettled_assigned_decisions == 0 && gaps_on_owned_ground.is_empty(),
            None => next.is_empty(),
        };

        Ok(LoopStatus {
            unsurfaced_gaps,
            unanswered_questions,
            unwritten_answers,
            unsettled_assigned_decisions,
            structural_defects,
            unproven_capabilities,
            undispositioned_drift,
            unexamined_claims,
            verifications: self.verification_recency()?,
            assigned_decisions,
            gaps_on_owned_ground,
            scope,
            clean,
            next,
        })
    }

    /// Count how much of the design carries its own verification.
    ///
    /// Deliberately a count and not a detector. Capabilities without a check
    /// are a real gap and DETECT still raises one; artifacts without their own
    /// check are worth *knowing* and not worth *asking* about, because the
    /// answer is usually "the capability's tests cover it" (BL-23).
    pub fn verification_coverage(&self) -> Result<VerificationCoverage, DynoError> {
        let mut v = VerificationCoverage {
            capabilities: 0,
            capabilities_verified: 0,
            capabilities_component_verified: 0,
            artifacts: 0,
            artifacts_verified: 0,
        };
        // "Verified" means a check that PASSES, not a check that exists.
        // Counting mere existence let a failing test raise coverage — the
        // design counting test nodes while ignoring test results, which is
        // the reflow1 failure in miniature (BL-30). `planned`, `failing`,
        // `skipped` and `blocked` all mean "not currently confirmed".
        for n in self.scan_nodes(node::CAPABILITY)? {
            v.capabilities += 1;
            match self.capability_verification(&n.node_id)? {
                crate::verify::CapabilityVerification::Verified => v.capabilities_verified += 1,
                crate::verify::CapabilityVerification::ComponentVerified => {
                    v.capabilities_component_verified += 1;
                }
                crate::verify::CapabilityVerification::Unchecked => {}
            }
        }
        for n in self.scan_nodes(node::ARTIFACT)? {
            v.artifacts += 1;
            if self.has_passing_verification(&n.node_id)? {
                v.artifacts_verified += 1;
            }
        }
        Ok(v)
    }

    /// Count how much of the design has something built for it — and how much
    /// the artifact layer simply does not reach. See [`RealizationCoverage`].
    pub fn realization_coverage(&self) -> Result<RealizationCoverage, DynoError> {
        let mut c = RealizationCoverage {
            capabilities: 0,
            realized: 0,
            built_but_unmodelled: 0,
        };
        for cap in self.scan_nodes(node::CAPABILITY)? {
            c.capabilities += 1;
            if self.capability_is_realized(&cap.node_id)? {
                c.realized += 1;
            } else if self.owner_claims_built(&cap.node_id)? {
                c.built_but_unmodelled += 1;
            }
        }
        Ok(c)
    }

    /// Whether one requirement is delivered — satisfied by a realized, checked
    /// capability, or (for a decomposed parent) by every one of its children
    /// being delivered.
    ///
    /// `depth` bounds the recursion. `decomposes` refuses to create a cycle, so
    /// a well-formed tree terminates on its own; the bound exists for a graph
    /// that arrived by `import_graph` from somewhere less careful, where
    /// refusing to answer beats recursing forever.
    fn requirement_is_delivered_within(
        &self,
        requirement_id: &str,
        depth: usize,
    ) -> Result<bool, DynoError> {
        if depth == 0 {
            return Ok(false);
        }
        for e in self.incoming(requirement_id, Some(edge::SATISFIES))? {
            let cap = &e.from_id;
            let checked = !matches!(
                self.capability_verification(cap)?,
                crate::verify::CapabilityVerification::Unchecked
            );
            if checked && self.capability_is_realized(cap)? {
                return Ok(true);
            }
        }
        let children = self.decomposed_children(requirement_id)?;
        if children.is_empty() {
            return Ok(false);
        }
        for child in &children {
            if !self.requirement_is_delivered_within(child, depth - 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether one requirement is delivered, rolling up a decomposition tree.
    /// See [`DeliveryCoverage`].
    pub fn requirement_is_delivered(&self, requirement_id: &str) -> Result<bool, DynoError> {
        self.requirement_is_delivered_within(requirement_id, 64)
    }

    /// Work out how much intent has actually been delivered, from the thread
    /// itself. See [`DeliveryCoverage`] for why this is computed rather than
    /// read from `Requirement.status`.
    ///
    /// A requirement is DELIVERED when some capability satisfies it and that
    /// capability is both realized and currently checked. "Currently" is the
    /// whole point: [`capability_verification`](Self::capability_verification)
    /// only reports `Verified`/`ComponentVerified` for checks that PASS
    /// (`dec:passing-is-verified`), so a requirement stops being delivered the
    /// moment its check starts failing, with nobody editing anything.
    ///
    /// A requirement whose own provenance is `inferred` never counts, however
    /// good its thread looks — it was read back out of the thing that
    /// implements it, so its satisfaction is a tautology rather than evidence.
    pub fn delivery_coverage(&self) -> Result<DeliveryCoverage, DynoError> {
        let mut d = DeliveryCoverage {
            requirements: 0,
            satisfied: 0,
            delivered: 0,
            inferred_only: 0,
        };
        for req in self.scan_nodes(node::REQUIREMENT)? {
            // A dropped requirement is a withdrawn need, not unfinished work.
            // Counting it would make abandoning something look like failing to
            // deliver it.
            if prop_str(&req, "status", "proposed") == "dropped" {
                continue;
            }
            d.requirements += 1;

            let satisfiers: Vec<String> = self
                .incoming(&req.node_id, Some(edge::SATISFIES))?
                .into_iter()
                .map(|e| e.from_id)
                .collect();

            // A decomposed parent is carried by its children, not by a
            // capability of its own: splitting a requirement adds no new
            // information, so satisfying every child IS satisfying the parent.
            // Without this a properly decomposed design reads as a wall of
            // unsatisfied parents, which punishes exactly the practice
            // systems engineering asks for.
            let children = self.decomposed_children(&req.node_id)?;
            let carried_by_children = !children.is_empty();

            if satisfiers.is_empty() && !carried_by_children {
                continue;
            }
            d.satisfied += 1;

            let mut thread_complete = false;
            for cap in &satisfiers {
                let checked = !matches!(
                    self.capability_verification(cap)?,
                    crate::verify::CapabilityVerification::Unchecked
                );
                if checked && self.capability_is_realized(cap)? {
                    thread_complete = true;
                    break;
                }
            }
            // EVERY child, not any: a parent half-delivered is not delivered,
            // and "any" would let one finished slice of a checkout system
            // report the whole thing done.
            if !thread_complete && carried_by_children {
                thread_complete = true;
                for child in &children {
                    if !self.requirement_is_delivered(child)? {
                        thread_complete = false;
                        break;
                    }
                }
            }
            if !thread_complete {
                continue;
            }

            // The thread is complete — but if the requirement itself was
            // recovered from the code that satisfies it, the thread proves
            // nothing. Counted apart rather than silently dropped, so the
            // number stays visible (rule 6: no silent caps).
            if prop_str(&req, "provenance", "authored") == "inferred" {
                d.inferred_only += 1;
            } else {
                d.delivered += 1;
            }
        }
        Ok(d)
    }

    /// Build the [`GraphReport`] — a one-shot aggregation of the deterministic
    /// analyses. See the module docs.
    pub fn graph_report(&self) -> Result<GraphReport, DynoError> {
        let mut node_counts = Vec::new();
        let mut design_nodes = 0;
        for &t in SNAPSHOT_TYPES {
            let n = self.count_nodes(t)?;
            if n > 0 {
                node_counts.push((t, n));
                design_nodes += n;
            }
        }

        // Everything the design-layer itemisation above does not cover:
        // provenance (`Fragment`), the asked-question record, drift events,
        // axis-Z machinery, dimension readings. Counted from the *schema*
        // rather than a second hardcoded list, so a node type added later
        // cannot go missing from the total the way `Fragment` did.
        //
        // The storyflow adopt trial imported 122 nodes and was told 109
        // (BL-43): `total_nodes` summed the snapshot list only, so the whole
        // provenance ledger — the thing that makes a recovered claim
        // checkable — was invisible to the surface an agent reads first. A
        // count that silently omits a type is a quiet lie about the size of
        // the design, which is rule 6 (no silent caps) applied to reporting.
        let mut other_counts = Vec::new();
        let mut other_nodes = 0;
        let mut schema_types: Vec<String> = self.schema().node_types.keys().cloned().collect();
        schema_types.sort();
        for t in schema_types {
            if SNAPSHOT_TYPES.contains(&t.as_str()) {
                continue;
            }
            let n = self.count_nodes(&t)?;
            if n > 0 {
                other_nodes += n;
                other_counts.push((t, n));
            }
        }
        let total_nodes = design_nodes + other_nodes;

        let verification = self.verification_coverage()?;
        let requirement_certainty = if self.count_nodes(node::REQUIREMENT)? > 0 {
            Some(self.requirement_certainty_breakdown()?)
        } else {
            None
        };
        let realization = self.realization_coverage()?;
        let delivery = {
            let d = self.delivery_coverage()?;
            if d.is_empty() { None } else { Some(d) }
        };
        let ledger = self.confirmation_ledger()?;
        let confirmation = if ledger.claims.is_empty() {
            None
        } else {
            Some(ConfirmationSummary {
                drifting: ledger.drifting,
                confirmed: ledger.confirmed,
                unexamined: ledger.unexamined,
            })
        };

        let mut gaps = self.detect_gaps()?;
        let gap_count = gaps.len();
        let gaps_truncated = gap_count.saturating_sub(TOP_N);
        gaps.truncate(TOP_N);

        let defect_count = self.detect_defects()?.len();

        let allocation = if self.count_nodes(node::COMPONENT)? > 0 {
            let a: AllocationReport = self.evaluate_allocation()?;
            Some(AllocationSummary {
                component_count: a.components.len(),
                modularity: a.modularity,
                misplaced_count: a.misplaced.len(),
                god_components: a.god_components,
            })
        } else {
            None
        };

        // Granularity: the build against the design's own decomposition. Read
        // in full here rather than capped — the reading caps itself, by only
        // ever naming distributional outliers.
        let granularity_reading = self.granularity_report()?;
        let maturity = self.maturity_report()?;

        let mut surprising = self.surprising_connections()?;
        let surprising_truncated = surprising.len().saturating_sub(TOP_N);
        surprising.truncate(TOP_N);

        let mut declining: Vec<DimensionDrift> = self
            .dimension_drifts()?
            .into_iter()
            .filter(|d| d.direction == DriftDirection::Declining)
            .collect();
        let declining_truncated = declining.len().saturating_sub(TOP_N);
        declining.truncate(TOP_N);

        Ok(GraphReport {
            node_counts,
            other_counts,
            design_nodes,
            realization,
            delivery,
            total_nodes,
            gap_count,
            defect_count,
            top_gaps: gaps,
            gaps_truncated,
            verifications: self.verification_recency()?,
            allocation,
            maturity,
            granularity: granularity_reading.observations,
            granularity_median: granularity_reading.median_capabilities_per_artifact,
            surprising,
            surprising_truncated,
            requirement_certainty,
            verification,
            confirmation,
            declining,
            declining_truncated,
        })
    }
}

impl GraphReport {
    /// Render the report as Markdown — the shareable "what should I look at?"
    /// artifact.
    pub fn to_markdown(&self) -> String {
        let mut m = String::new();

        let _ = writeln!(m, "# Design graph report\n");

        // Snapshot.
        let _ = writeln!(m, "## Snapshot\n");
        if self.node_counts.is_empty() {
            let _ = writeln!(m, "_Empty graph — nothing designed yet._\n");
        } else {
            let breakdown = self
                .node_counts
                .iter()
                .map(|(t, n)| format!("{t} {n}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                m,
                "{} design nodes across {} type(s): {}.\n",
                self.design_nodes,
                self.node_counts.len(),
                breakdown
            );
            if !self.other_counts.is_empty() {
                let other = self
                    .other_counts
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    m,
                    "Plus {} supporting node(s) — provenance, questions, history: {}. \
                     **{} nodes in total.**\n",
                    self.total_nodes - self.design_nodes,
                    other,
                    self.total_nodes
                );
            }
            // Which requirements the user actually confirmed (BL-75) — said
            // here so no session reconstructs certainty in prose. Zero
            // categories are omitted; a wholly-confirmed set reads as one
            // clean clause.
            if let Some(c) = &self.requirement_certainty {
                let mut parts = Vec::new();
                for (n, label) in [
                    (c.user_confirmed, "user-confirmed"),
                    (c.asserted, "asserted, awaiting the user"),
                    (
                        c.recovered,
                        "recovered from the artifact, awaiting the user",
                    ),
                    (c.settled_out, "settled out (deferred/dropped)"),
                ] {
                    if n > 0 {
                        parts.push(format!("{n} {label}"));
                    }
                }
                if !parts.is_empty() {
                    let _ = writeln!(m, "Requirement certainty: {}.\n", parts.join(" · "));
                }
            }
            // Delivery, derived — never read from `status`. Phrased against its
            // denominator so "3 delivered" cannot be mistaken for "3 left".
            if let Some(d) = &self.delivery {
                let _ = write!(
                    m,
                    "Delivered: **{}** of {} requirement(s) ({} satisfied) — computed from the \
                     thread (something satisfies it, and that capability is built and currently \
                     passing), not from a status field.",
                    d.delivered, d.requirements, d.satisfied
                );
                if d.inferred_only > 0 {
                    let _ = write!(
                        m,
                        " {} further requirement(s) have a complete thread but were recovered by \
                         inference, so they are excluded: a requirement read back out of the code \
                         satisfying it proves nothing.",
                        d.inferred_only
                    );
                }
                let _ = writeln!(m, "\n");
            }
            let _ = writeln!(
                m,
                "{} open gap(s), {} structural defect(s).\n",
                self.gap_count, self.defect_count
            );
        }

        // Top gaps.
        if !self.top_gaps.is_empty() {
            let _ = writeln!(m, "## Top gaps (look here first)\n");
            for g in &self.top_gaps {
                let _ = writeln!(
                    m,
                    "- **[{:.2}]** {} — {}",
                    g.severity, g.title, g.description
                );
            }
            if self.gaps_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.gaps_truncated);
            }
            let _ = writeln!(m);
        }

        // Allocation health.
        if let Some(a) = &self.allocation {
            let _ = writeln!(m, "## Allocation health\n");
            let _ = writeln!(
                m,
                "Modularity **{:.2}** across {} component(s); {} misplaced capability(ies).",
                a.modularity, a.component_count, a.misplaced_count
            );
            if a.god_components.is_empty() {
                let _ = writeln!(m, "No god-components.\n");
            } else {
                let _ = writeln!(m, "God-component(s): {}.\n", a.god_components.join(", "));
            }
        }

        if let Some(c) = &self.confirmation {
            let _ = writeln!(m, "## Confirmation\n");
            let _ = writeln!(
                m,
                "{} drifting · {} confirmed · {} unexamined (capabilities with built artifacts; \
                 unexamined = nobody has ever checked the claim against reality)\n",
                c.drifting, c.confirmed, c.unexamined
            );
        }

        // Verification coverage — reported, never demanded. Component
        // granularity is its own clause (BL-73): folding it into "verified"
        // would overstate, folding it into silence read a tested system as
        // 0/20.
        if !self.verification.is_empty() {
            let v = &self.verification;
            let _ = writeln!(m, "## Verification coverage\n");
            let component_clause = if v.capabilities_component_verified > 0 {
                format!(
                    " ({} more at component granularity)",
                    v.capabilities_component_verified
                )
            } else {
                String::new()
            };
            let _ = writeln!(
                m,
                "{}/{} capability(ies) verified{}; {}/{} artifact(s) carry a check of their own.\n",
                v.capabilities_verified,
                v.capabilities,
                component_clause,
                v.artifacts_verified,
                v.artifacts
            );
        }

        // Where the design sits on the function-to-structure trajectory.
        if let Some(frontier) = self.maturity.frontier {
            let _ = writeln!(m, "## Trajectory — function first, structure later\n");
            for b in &self.maturity.bands {
                let mark = if b.name == frontier {
                    "  ← frontier"
                } else {
                    ""
                };
                match b.ratio {
                    Some(r) => {
                        let _ = writeln!(
                            m,
                            "- **{}** {:.0}% ({}/{}){}",
                            b.name,
                            r * 100.0,
                            b.present,
                            b.population,
                            mark
                        );
                    }
                    None => {
                        let _ = writeln!(m, "- **{}** — not measurable yet", b.name);
                    }
                }
            }
            if !self.maturity.ahead_of_frontier.is_empty() {
                let _ = writeln!(
                    m,
                    "\n_{} band(s) run ahead of the frontier ({}). That is the normal shape of a \
                     design that got function right first — not work done out of order._",
                    self.maturity.ahead_of_frontier.len(),
                    self.maturity.ahead_of_frontier.join(", ")
                );
            }
            let _ = writeln!(
                m,
                "\n_Where this design SHOULD be is deliberately not stated: a demonstrator may sit \
                 here forever and be right. `maturity_report` carries the full reading._\n"
            );
        }

        // Granularity — the build against the design's own decomposition.
        if !self.granularity.is_empty() {
            let _ = writeln!(m, "## Granularity — what the build does not separate\n");
            for o in &self.granularity {
                let _ = writeln!(
                    m,
                    "- `{}`{}: realizes **{}** capabilities the design distinguishes; the median \
                     artifact realizes {:.0}. _[unusual {:.2}]_",
                    o.artifact_id,
                    o.location
                        .as_ref()
                        .map(|l| format!(" ({l})"))
                        .unwrap_or_default(),
                    o.realizes_capabilities,
                    self.granularity_median,
                    o.unusual
                );
            }
            let _ = writeln!(
                m,
                "\n_Not a defect and not a size judgement — the build holds as one thing what \
                 the design holds as several. Which side is wrong is yours to say; \
                 `granularity_report` carries the full reading._\n"
            );
        }

        // Surprising couplings.
        if !self.surprising.is_empty() {
            let _ = writeln!(m, "## Surprising couplings\n");
            for s in &self.surprising {
                let _ = writeln!(
                    m,
                    "- `{}` → `{}` ({}): {}. _[surprise {:.2}]_",
                    s.from_id,
                    s.to_id,
                    s.edge_type,
                    s.reasons.join(", "),
                    s.surprise
                );
            }
            if self.surprising_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.surprising_truncated);
            }
            let _ = writeln!(m);
        }

        // Declining quality.
        if !self.declining.is_empty() {
            let _ = writeln!(m, "## Quality drift (declining)\n");
            for d in &self.declining {
                let _ = writeln!(
                    m,
                    "- **{}** of `{}`: {:.2} → {:.2} over {} reading(s) _(slope {:.3})_",
                    d.dimension.as_str(),
                    d.target_id,
                    d.first_score,
                    d.last_score,
                    d.observation_count,
                    d.slope
                );
            }
            if self.declining_truncated > 0 {
                let _ = writeln!(m, "- _…and {} more._", self.declining_truncated);
            }
            let _ = writeln!(m);
        }

        m
    }
}
