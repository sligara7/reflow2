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

use crate::detect::{GapCandidate, GapScope};
use crate::graph::DesignGraph;
use crate::heal::HealIssue;
use crate::nodes::edge;
use crate::propagate::PropagateOptions;

/// Default radius for a scope, in hops from the seed.
///
/// Three is the smallest number that reaches a Component's whole thread: its
/// capabilities are one hop away (ALLOCATED_TO), the requirements they satisfy
/// and the artifacts and checks that realize and verify them are two, and a
/// contained child component's capabilities are three. Deeper pulls in the
/// neighbours' neighbours, which is the program again.
pub const DEFAULT_SCOPE_DEPTH: usize = 3;

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
    pub items: Vec<T>,
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
        Ok(Scoped {
            scope: seed_id.to_string(),
            depth,
            region_size: region.len(),
            note: (region.len() == 1).then(|| {
                format!(
                    "`{seed_id}` has no edges, so this region is the seed alone and \
                     `in_scope: 0` is VACUOUS rather than clean — nothing could have been \
                     found here. Bookkeeping nodes (DesignEpoch, Fragment, Snapshot) are \
                     islands in the propagation walk; scope a Component, Capability or \
                     Project to ask a question that can have an answer."
                )
            }),
            total,
            in_scope: items.len(),
            out_of_scope: total - unanchored - items.len(),
            unanchored,
            project_level,
            items,
        })
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
        let all = self.detect_defects()?;
        let total = all.len();
        let unanchored = all.iter().filter(|d| d.affected_ids.is_empty()).count();
        let items: Vec<HealIssue> = all
            .into_iter()
            .filter(|d| !d.affected_ids.is_empty())
            .filter(|d| d.affected_ids.iter().any(|id| region.contains(id)))
            .collect();
        Ok(Scoped {
            scope: seed_id.to_string(),
            depth,
            region_size: region.len(),
            note: (region.len() == 1).then(|| {
                format!(
                    "`{seed_id}` has no edges, so this region is the seed alone and \
                     `in_scope: 0` is VACUOUS rather than clean — nothing could have been \
                     found here. Bookkeeping nodes (DesignEpoch, Fragment, Snapshot) are \
                     islands in the propagation walk; scope a Component, Capability or \
                     Project to ask a question that can have an answer."
                )
            }),
            total,
            in_scope: items.len(),
            out_of_scope: total - unanchored - items.len(),
            unanchored,
            // HealIssues carry no zoom level; the distinction is meaningless
            // here rather than zero, and reporting 0 would be a small lie.
            project_level: 0,
            items,
        })
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
