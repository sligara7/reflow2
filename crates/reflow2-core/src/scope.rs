//! Scoped analysis — run the detectors over one part of the design.
//!
//! Anthony, 2026-07-25, from the satellite case: an organisation builds a
//! satellite capability with a space segment, a ground/user segment and a control
//! segment; his team owns the satellite; more specifically it owns inter-satellite
//! laser communications. He needs to understand the system of systems, but *day to
//! day* he needs to look at his own part and be told about ITS gaps — not the
//! program's.
//!
//! Every detector until now took the whole design as its subject, so a team of
//! four reading `detect_gaps` on a program-sized graph was handed everyone's work
//! at once. That is the same failure as an unbounded read (`cap:bounded-reads`)
//! one level up: technically complete, practically unusable, and the usual human
//! response is to stop looking.
//!
//! **A SCOPE IS NOT A BLAST RADIUS**, and finding that out cost a failing test
//! worth keeping. The first implementation reused the propagation radius, on the
//! reasoning that `claimed_region` ("the part I hold") and `propagate_change`
//! ("what this change touches") already agree on what a region is, so a third
//! reader should not invent a fourth definition. The test that scoped to a
//! Project and expected the whole design failed — because `CONTAINS` is
//! deliberately NOT a traceability edge (`nodes.rs`: propagating along it would
//! make the Project a hub that short-circuits every sibling to two hops). That
//! exclusion is right for impact and wrong for ownership:
//!
//! - **Impact** asks *what might now be wrong*. A change to the space segment
//!   does not implicate every screw inside it, so containment is correctly not
//!   walked.
//! - **Ownership** asks *what is mine*. A segment lead owns the subsystems inside
//!   their segment by definition — that is what containment MEANS.
//!
//! So a scope is **containment closure, then traceability radius**: descend
//! `CONTAINS` transitively from the seed (ownership has no distance limit — a
//! part three levels down is still yours), then take the propagation radius to
//! `depth` hops from everything owned (proximity does, because the thread reaches
//! outward into other people's work and has to stop somewhere).
//!
//! A consequence worth naming rather than discovering later: `claim_region` still
//! uses the radius alone, so **claiming a segment does not claim the subsystems
//! inside it**. That may well be a defect in the claims layer rather than a
//! deliberate choice — it is recorded here, not silently fixed, because changing
//! what a claim covers changes what two people believe they hold.
//!
//! Two properties this must have, both of them rule 6 applied to filtering:
//!
//! 1. **It reports what it filtered out.** A scoped view that returned three gaps
//!    and said nothing about the other forty would teach a team their program is
//!    healthy. Every scoped result carries the total, the in-scope count and the
//!    out-of-scope count.
//! 2. **It filters, it never decides.** A gap in scope is a gap whose affected set
//!    touches the region. Project-level rollups (an `unvalidated_capability`
//!    sweep, say) therefore appear in a team's view whenever they touch that
//!    team's work — and arrive carrying their own `scope: project`, so the reader
//!    can see it is the program's finding and not theirs. Hiding them would be the
//!    tool deciding what a team is allowed to worry about.
//! 3. **Findings with no location are counted, not dropped.** Some gaps are
//!    statements about the design as a whole and anchor on nothing — "nothing is
//!    verified anywhere yet" (the phase gaps). They cannot be attributed to a
//!    scope, and they are not any one team's, so they are reported as
//!    `unanchored` rather than either hidden or pushed into everybody's list. The
//!    first implementation dropped them silently and a test caught it; every
//!    finding now lands in exactly one bucket, and the four sum to `total`.

use std::collections::BTreeSet;

use dynograph_core::DynoError;

use crate::detect::{GapCandidate, GapRow, GapScope, NARROW_THE_SCOPE, ReplyBudget, budget_gaps};
use crate::graph::DesignGraph;
use crate::heal::HealIssue;
use crate::nodes::edge;
use crate::propagate::PropagateOptions;

/// Default radius for a scope, in hops from the seed.
///
/// **Two, and it was three until 2026-08-17.** Both halves of that change are
/// worth keeping, because the old default had a stated reason and the reason
/// was wrong.
///
/// Two reaches a Component's whole thread and stops: its capabilities are one
/// hop away (ALLOCATED_TO), and the requirements they satisfy and the artifacts
/// and checks that realize and verify them are two. The third hop reaches the
/// *neighbours'* threads, which is the program again.
///
/// ## The old reason did not survive reading [`DesignGraph::scope_region`]
///
/// It argued that three was needed because "a contained child component's
/// capabilities are three" hops out. They are not. `scope_region` puts the
/// **entire containment closure into the seed set before taking the radius**,
/// unboundedly — so a child three levels down is already a seed, and its
/// capabilities are one hop from one. The third hop bought nothing the
/// docstring claimed and everything it warned against.
///
/// ## What the measurement said (`dec:the-default-scope-depth-should-be-two`)
///
/// Driving the built binary over all 56 Components of reflow2's own design
/// (2487 nodes, 83 gaps):
///
/// | depth | region_size | gaps in scope |
/// |-------|-------------|---------------|
/// | 1     | 17..139     | 0..19  (med 1)|
/// | **2** | 267..601    | 2..27  (med 4)|
/// | 3     | 595..903    | 50..60 (med 55)|
///
/// At three, **every one of the 56 returned 50-60 of the 83 gaps** — a spread
/// so narrow the answers are indistinguishable, so `in_scope: 55` told every
/// team the same thing about its own part. That is the failure this constant
/// now exists to have fixed rather than to have caused.
///
/// Depth 1 was rejected: it stops short of the requirements, so a team would
/// stop being told when its own capability satisfies nothing.
pub const DEFAULT_SCOPE_DEPTH: usize = 2;

/// The share of the design's anchored findings above which a scoped answer is
/// reported as **not meaningfully narrower** than the unscoped one.
///
/// Half, stated here and repeated in the message so a reader can disagree with
/// it rather than having to discover it. This is a threshold and therefore a
/// judgement, which `dec:report-dont-judge` normally forbids — it is admissible
/// only because the finding it produces is about **this tool's own answer**
/// rather than about the design. Saying "the narrowing you asked for did not
/// narrow" is the instrument reporting on itself, which is the one place a
/// judgement is owed rather than withheld.
pub const SCOPE_IS_BARELY_NARROWER_AT: f64 = 0.5;

/// A detector's answer for one part of the design, with what it left out.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Scoped<T> {
    /// The seed the region was computed from.
    pub scope: String,
    /// Hops from the seed.
    pub depth: usize,
    /// How many nodes the region covers — a one-node region usually means the
    /// seed was a leaf or a typo, and saying so beats returning nothing.
    pub region_size: usize,
    /// Said in WORDS when the scoped answer cannot mean anything.
    ///
    /// A seed with no edges makes `in_scope: 0` **vacuous rather than clean** —
    /// nothing could have been found there. The numbers already carried this
    /// (`region_size: 1` beside `in_scope: 0`) and dev_storyflow (w-c216679a,
    /// 2026-08-09) pointed out that it is exactly the shape most likely to be
    /// banked as "my area is clean": they scoped to an Epoch and to a Fragment,
    /// got `in_scope: 0` at depth 2 AND depth 5, and only caught it because
    /// they ran a positive control on a Project.
    ///
    /// `None` when the region is real — the field appears only when it has
    /// something to say, so its presence is the signal.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
    /// Findings across the WHOLE design, so a scoped view can never imply the
    /// rest is clean.
    pub total: usize,
    /// Findings inside the region (`items.len()`).
    pub in_scope: usize,
    /// Anchored findings located elsewhere in the design — someone else's to
    /// answer, but never hidden.
    pub out_of_scope: usize,
    /// Findings that anchor on nothing: statements about the design as a whole
    /// (the lifecycle-phase gaps). Not attributable to any scope and not any one
    /// team's, so counted here and read in the unscoped view.
    pub unanchored: usize,
    /// How many of the in-scope findings are project-level rollups rather than
    /// findings about this part specifically.
    pub project_level: usize,
    /// The share of the design's ANCHORED findings this answer holds, 0.0..=1.0
    /// — the comparison done FOR the reader instead of left to them.
    ///
    /// The denominator is `in_scope + out_of_scope`, deliberately not `total`:
    /// unanchored findings could never have been in any scope, so counting them
    /// against a region would flatter every scoped answer by a constant.
    ///
    /// Always present, including when it is 0.0 and when there is nothing to
    /// divide — a field that appeared only on bad news would make its absence
    /// read as good news, which is the whole family of defect this module's
    /// epoch was named after.
    pub share_of_anchored: f64,
    /// Said in WORDS when the narrowing did not narrow — when this "part of the
    /// design" holds more than [`SCOPE_IS_BARELY_NARROWER_AT`] of everything
    /// the design has to say.
    ///
    /// `req:a-scoped-answer-actually-narrows`: *"if scoping to any seed returns
    /// most of what the unscoped call returns, the tool should be able to SAY
    /// SO rather than leaving the reader to compare two numbers they were never
    /// shown together."* Measured at the old default of 3, every one of 56
    /// Components returned 50-60 of 83 gaps and nothing said a word about it.
    ///
    /// Deliberately a SECOND field rather than more prose in `note`: the two
    /// say opposite things — one that the region was too small to mean
    /// anything, the other that it was too large — and a single field carrying
    /// either would have to be read before it could be understood.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub narrowing_note: Option<String>,
    /// What this reply had to withhold from `items` in order to be readable at
    /// all — a THIRD thing an answer can be wrong about, on an axis the other
    /// two say nothing about. `note` is about the region being too small and
    /// `narrowing_note` about it being too large; this is about the ANSWER
    /// being too large, which is a fact about the reader rather than the design.
    ///
    /// `None` on a reader that does not budget its reply, so its absence means
    /// "nothing was withheld for size" and never "nobody looked".
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub budget: Option<ReplyBudget>,
    pub items: Vec<T>,
}

impl<T> Scoped<T> {
    /// Assemble a scoped answer, computing both of its self-descriptions.
    ///
    /// Exists because the vacuity note was written out TWICE, verbatim,
    /// comments and all, in the two constructors below — and the second copy
    /// was already once repaired separately from the first. A rule stated in
    /// two places is a rule that will be half-changed.
    fn new(
        scope: &str,
        depth: usize,
        region_size: usize,
        total: usize,
        unanchored: usize,
        project_level: usize,
        items: Vec<T>,
    ) -> Self {
        let in_scope = items.len();
        let out_of_scope = total.saturating_sub(unanchored).saturating_sub(in_scope);
        let anchored = in_scope + out_of_scope;
        let share_of_anchored = if anchored == 0 {
            0.0
        } else {
            in_scope as f64 / anchored as f64
        };
        Self {
            scope: scope.to_string(),
            depth,
            region_size,
            // The vacuity note is gated on the region being the seed alone AND
            // ON HAVING FOUND NOTHING — the second half added 2026-08-17, and
            // its absence was `req:a-session-with-no-seed-can-still-orient`'s
            // bug one message over.
            //
            // The note asserts "nothing could have been found here", which was
            // true of a one-node region while every rule needed at least an
            // edge to fire. The degree-zero rule stopped needing one: a node
            // attached to nothing is now a finding, and it lives in exactly the
            // one-node region this note calls vacuous. So the first scoped call
            // after that change returned `in_scope: 1`, a real finding, and a
            // note beside it saying nothing could have been found.
            //
            // Caught by driving the built binary rather than by a test — the
            // note is prose about a number, and nothing was comparing the two.
            note: (region_size == 1 && in_scope == 0).then(|| {
                format!(
                    "`{scope}` has no edges, so this region is the seed alone and \
                     `in_scope: 0` is VACUOUS rather than clean — nothing could have been \
                     found here. Bookkeeping nodes (DesignEpoch, Fragment, Snapshot) are \
                     islands in the propagation walk; scope a Component, Capability or \
                     Project to ask a question that can have an answer."
                )
            }),
            narrowing_note: (anchored > 0 && share_of_anchored > SCOPE_IS_BARELY_NARROWER_AT).then(
                || {
                    let pct = (share_of_anchored * 100.0).round() as u64;
                    format!(
                        "THIS ANSWER IS BARELY NARROWER THAN THE UNSCOPED ONE: {in_scope} of \
                         the {anchored} findings that could have been in any scope are in \
                         this one ({pct}%, over the {threshold}% at which this is reported). \
                         A region at depth {depth} that holds most of the design is not \
                         \"your part\" — every other seed will return a similar number, so \
                         the figure cannot be read as a fact about THIS part. Try a smaller \
                         `depth`, or read the unscoped answer and stop treating this as a \
                         narrowing.",
                        threshold = (SCOPE_IS_BARELY_NARROWER_AT * 100.0).round() as u64,
                    )
                },
            ),
            total,
            in_scope,
            out_of_scope,
            unanchored,
            project_level,
            share_of_anchored,
            budget: None,
            items,
        }
    }

    /// Replace the items with a budgeted rendering of them, recording what that
    /// cost. Separate from [`Scoped::new`] because only some readers budget.
    fn budgeted<U>(self, items: Vec<U>, budget: ReplyBudget) -> Scoped<U> {
        Scoped {
            scope: self.scope,
            depth: self.depth,
            region_size: self.region_size,
            note: self.note,
            total: self.total,
            in_scope: self.in_scope,
            out_of_scope: self.out_of_scope,
            unanchored: self.unanchored,
            project_level: self.project_level,
            share_of_anchored: self.share_of_anchored,
            narrowing_note: self.narrowing_note,
            budget: Some(budget),
            items,
        }
    }
}

impl DesignGraph {
    /// The nodes a scope covers: everything the seed OWNS, plus everything the
    /// golden thread reaches from any of it within `depth` hops.
    ///
    /// Computed, never stored, so it follows the design instead of freezing a
    /// list that goes stale. See the module docs for why this is not the
    /// propagation radius on its own.
    pub fn scope_region(&self, seed_id: &str, depth: usize) -> Result<BTreeSet<String>, DynoError> {
        let owned = self.containment_closure(seed_id)?;
        let seeds: Vec<&str> = owned.iter().map(String::as_str).collect();
        let radius = self.propagate_from(&seeds, PropagateOptions { max_depth: depth })?;
        let mut ids = owned;
        ids.extend(radius.impacted.into_iter().map(|i| i.node_id));
        Ok(ids)
    }

    /// Everything at or beneath `seed_id` on the containment spine — the axis-Y
    /// matryoshka, walked to the bottom.
    ///
    /// Unbounded by design: ownership does not attenuate with distance. A part
    /// three levels down inside your subsystem is still yours, and a depth limit
    /// here would mean a team stopped being told about its own deepest parts —
    /// exactly the silence this whole feature exists to remove.
    fn containment_closure(&self, seed_id: &str) -> Result<BTreeSet<String>, DynoError> {
        let mut owned = BTreeSet::new();
        owned.insert(seed_id.to_string());
        let mut queue = vec![seed_id.to_string()];
        while let Some(id) = queue.pop() {
            for edge in self.outgoing(&id, Some(edge::CONTAINS))? {
                if owned.insert(edge.to_id.clone()) {
                    queue.push(edge.to_id);
                }
            }
        }
        Ok(owned)
    }

    /// `detect_gaps`, narrowed to one part of the design.
    ///
    /// Refuses an unknown seed rather than returning an empty region, because
    /// "no gaps here" and "there is no such node" are different answers and a
    /// typo must not read as good news.
    pub fn detect_gaps_in_scope(
        &self,
        seed_id: &str,
        depth: usize,
    ) -> Result<Scoped<GapCandidate>, DynoError> {
        let region = self.require_scope(seed_id, depth)?;
        let all = self.detect_gaps()?;
        let total = all.len();
        let unanchored = all.iter().filter(|g| g.affected_ids.is_empty()).count();
        let items: Vec<GapCandidate> = all
            .into_iter()
            .filter(|g| !g.affected_ids.is_empty())
            .filter(|g| g.affected_ids.iter().any(|id| region.contains(id)))
            .collect();
        let project_level = items
            .iter()
            .filter(|g| g.scope == GapScope::Project)
            .count();
        Ok(Scoped::new(
            seed_id,
            depth,
            region.len(),
            total,
            unanchored,
            project_level,
            items,
        ))
    }

    /// [`detect_gaps_in_scope`](Self::detect_gaps_in_scope), in a reply that
    /// fits in `budget_chars`.
    ///
    /// Scoping is what the unscoped reader is TOLD to do when its own answer
    /// will not fit, so a scoped answer that will not fit either would make
    /// that advice a dead end. Measured on reflow2's own design, a Component at
    /// depth 3 holds 50–60 of the 83 gaps — most of the way back to the reply
    /// that was refused in the first place.
    ///
    /// `in_scope` counts what is IN the region; `budget.listed` counts what is
    /// in this reply. They are different numbers whenever the tail was dropped,
    /// and both are reported for that reason.
    pub fn detect_gaps_in_scope_within(
        &self,
        seed_id: &str,
        depth: usize,
        budget_chars: usize,
    ) -> Result<Scoped<GapRow>, DynoError> {
        let mut scoped = self.detect_gaps_in_scope(seed_id, depth)?;
        let gaps = std::mem::take(&mut scoped.items);
        let report = budget_gaps(gaps, budget_chars, NARROW_THE_SCOPE);
        Ok(scoped.budgeted(report.items, report.budget))
    }

    /// `detect_defects`, narrowed the same way.
    ///
    /// Structural defects are about shape rather than meaning, so scoping them
    /// answers a different question than scoping gaps: not "what is my team
    /// owed" but "is my part of the architecture sound" — a cycle wholly inside
    /// one subsystem is that subsystem's problem to fix.
    pub fn detect_defects_in_scope(
        &self,
        seed_id: &str,
        depth: usize,
    ) -> Result<Scoped<HealIssue>, DynoError> {
        let region = self.require_scope(seed_id, depth)?;
        let all = self.open_defects()?;
        let total = all.len();
        let unanchored = all.iter().filter(|d| d.affected_ids.is_empty()).count();
        let items: Vec<HealIssue> = all
            .into_iter()
            .filter(|d| !d.affected_ids.is_empty())
            .filter(|d| d.affected_ids.iter().any(|id| region.contains(id)))
            .collect();
        Ok(Scoped::new(
            seed_id,
            depth,
            region.len(),
            total,
            unanchored,
            // HealIssues carry no zoom level; the distinction is meaningless
            // here rather than zero, and reporting 0 would be a small lie.
            0,
            items,
        ))
    }

    /// Resolve a scope seed, refusing one that does not exist.
    fn require_scope(&self, seed_id: &str, depth: usize) -> Result<BTreeSet<String>, DynoError> {
        if !self.node_type_index()?.contains_key(seed_id) {
            return Err(DynoError::NodeNotFound {
                node_type: "any".into(),
                node_id: seed_id.into(),
            });
        }
        self.scope_region(seed_id, depth)
    }
}
