//! REGIONS — what parts this design has, for a session that holds nothing
//! (`cap:a-session-with-no-seed-can-find-one`,
//! `req:a-session-with-no-seed-can-still-orient`).
//!
//! ## The situation it answers, in the reporter's own words
//!
//! dev_storyflow's fleet, 2026-08-08: *"the moment with the most time available
//! — sitting AVAILABLE, nothing to do — is the moment the design brain is LEAST
//! USABLE, and the moment I am busiest (mid-lane) is when I am told to orient."*
//!
//! Every scoped read wants a seed. [`crate::scope::detect_gaps_in_scope`] takes
//! one, `claim_region` takes one, and a worker at pool check-in has no lane and
//! therefore no seed. The seedless calls that do exist are design-wide rollups
//! that their own doctrine forbids attributing to the caller — so the honest
//! options were to call nothing, or to call an unscoped tool and hedge the
//! number in a paragraph. They chose to call nothing, which is a design brain a
//! willing, idle, correctly-behaving session cannot use.
//!
//! This is the missing step before the seed: *what parts are there, how big is
//! each, and what is owed in each* — enough to pick one and scope to it.
//!
//! ## ⭐ The carve-up is the design's own, never one this module infers
//!
//! A region here is a node the design ALREADY DECLARES as a part: its Project
//! and its Components. The tempting alternative was community detection —
//! [`crate::structure::DesignNetwork::communities`] wraps Leiden and is already
//! in the crate, so "here are your design's twelve clusters" was a few lines
//! away.
//!
//! It is refused. A clustering answer would be reflow2 telling a reader *these
//! are the parts of your design* on the strength of a heuristic with a
//! resolution knob, and the reader has no way to check it. That is the exact
//! failure `epoch:instruments-stop-overstating` exists to remove, and shipping
//! it inside the epoch's last requirement would be a poor joke. What the design
//! says its parts are, reflow2 can report; what its parts REALLY are is not a
//! question this crate is entitled to answer.
//!
//! ## It reports what it does not cover, because a region list is not a map
//!
//! Named parts do not reach a whole mature design — most of one is bookkeeping
//! (ChangeEvent, Snapshot, TemporalFact, Fragment) that no Component contains.
//! Measured on reflow2's own design at depth 1, the 57 named parts between them
//! reach under a fifth of the graph. A list of regions read as a partition
//! would therefore be a false map, so [`RegionCoverage`] states how many nodes
//! lie in NO region and how many lie in MORE THAN ONE, by type.
//!
//! That is `cap:a-finding-describes-the-walk-that-produced-it` applied to a
//! region list: the same rule that made `unthreaded_cluster` name its walk.
//!
//! ## Why the default depth is 1 here and 3 there
//!
//! [`crate::scope::DEFAULT_SCOPE_DEPTH`] is 3. At that radius, on reflow2's own
//! design, every one of the 56 Components covers 595–903 nodes and returns
//! 50–60 of the 83 gaps — the regions are neither distinct nor small, so a list
//! of them tells a chooser nothing. At depth 1 the same parts cover 17–139 nodes
//! and hold 0–19 gaps, which is a choice someone can actually make.
//!
//! **That divergence is a finding, not a workaround.** It is recorded as
//! `fact:defect-a-scoped-detector-at-its-default-depth-returns-two-thirds-of-the-design`
//! and put to the owner as `dec:the-default-scope-depth-should-be-two`. Nothing
//! here changes the scoped detectors' default, because that changes what every
//! existing caller sees; this module simply does not inherit a number the
//! measurement says is wrong, and says so where a reader will meet it. Both
//! depths are parameters, so a caller who wants them to agree can say so.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::DynoError;
use dynograph_storage::Value;

use crate::graph::DesignGraph;
use crate::nodes::node;

/// Default radius for a region listing, in hops from each named part.
///
/// One, and deliberately not [`crate::scope::DEFAULT_SCOPE_DEPTH`] — see the
/// module docs. A chooser needs regions that differ from each other; a worker
/// already standing in one needs everything its thread reaches. Those are
/// different questions and the same number does not serve both.
pub const DEFAULT_REGION_DEPTH: usize = 1;

/// The node types a region can be seeded from: the design's own carve-up.
///
/// `Project` and `Component` only. Interfaces, Capabilities and Requirements
/// are things that live IN a part rather than parts themselves, and admitting
/// them would turn a list of places to stand into a second copy of the design.
pub const REGION_SEED_TYPES: [&str; 2] = [node::PROJECT, node::COMPONENT];

/// One named part of the design, sized and costed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DesignRegion {
    /// The node to pass as `scope` to the scoped reads. This is the payload:
    /// the whole point of the listing is to hand a seedless session a seed.
    pub seed_id: String,
    pub node_type: String,
    /// The part's own name, so a chooser reads words rather than ids.
    pub name: String,
    /// Nodes the region covers at the stated depth, by the same rule the scoped
    /// detectors use ([`DesignGraph::scope_region`]) — containment closure then
    /// traceability radius. Reused rather than reimplemented so that scoping to
    /// a row returns the region this row measured.
    pub region_size: usize,
    /// Open gaps whose affected set touches the region.
    pub open_gaps: usize,
    /// Structural defects whose affected set touches the region.
    pub open_defects: usize,
    /// Contributors whose claim seed falls inside this region — somebody is
    /// already working in here. Advisory, exactly as claims are: it is a reason
    /// to talk to them, never a refusal.
    pub held_by: Vec<String>,
}

/// What the region list reaches, and what it does not.
///
/// Every field here exists so the list cannot be read as a partition of the
/// design. A caller who sees twelve regions and no coverage block will assume
/// the twelve are the design; on any real graph they are a minority of it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegionCoverage {
    /// Every node in the design.
    pub nodes: usize,
    /// Nodes inside at least one region.
    pub in_some_region: usize,
    /// Nodes inside NO region — bookkeeping, and anything the named parts have
    /// not been wired to. Scoping to any row will never mention these.
    pub in_no_region: usize,
    /// Nodes inside MORE THAN ONE region. Regions overlap by construction (a
    /// shared Capability sits in both its consumers' radii); a high figure here
    /// means the rows are not the distinct areas they look like.
    pub in_more_than_one: usize,
    /// The uncovered nodes broken down by type, commonest first in id order —
    /// so "the 1800 nodes nothing reaches" can be recognised as bookkeeping
    /// rather than feared as lost design.
    pub uncovered_by_type: BTreeMap<String, usize>,
}

/// The seedless orientation answer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DesignRegions {
    /// The radius each region was computed at, echoed so sizes can be read.
    pub depth: usize,
    /// The node types used as region seeds, named so "these are the parts" is a
    /// checkable claim about the design's own vocabulary rather than a verdict.
    pub basis: Vec<&'static str>,
    /// One row per named part, most currently owed first — see [`Self::order`].
    pub regions: Vec<DesignRegion>,
    pub coverage: RegionCoverage,
    /// Gaps across the WHOLE design, so no row can imply the rest is clean.
    pub total_gaps: usize,
    /// Structural defects across the whole design, same reason.
    pub total_defects: usize,
    /// How `regions` is sorted, stated rather than left to be inferred. An
    /// ordered list invites being read as a ranking of importance and this one
    /// is not: it is how much is open there right now, which moves every time
    /// somebody answers a question.
    pub order: &'static str,
    /// Said in WORDS when the listing could not have found anything — a design
    /// that names no parts yet. `None` when the answer is real, so its presence
    /// is the signal.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

impl DesignGraph {
    /// What parts this design has, sized and costed, without being given a seed.
    ///
    /// The one orientation read that asks nothing of the caller. See the module
    /// docs for why the carve-up is the design's own and why an empty answer
    /// still says something.
    pub fn design_regions(&self, depth: usize) -> Result<DesignRegions, DynoError> {
        let gaps = self.detect_gaps()?;
        let defects = self.open_defects()?;
        let claims = self.claims()?;

        let mut regions = Vec::new();
        // How many regions each node fell into — the overlap figure, counted
        // while the regions are being walked rather than by walking twice.
        let mut membership: BTreeMap<String, usize> = BTreeMap::new();

        for seed_type in REGION_SEED_TYPES {
            for n in self.scan_nodes(seed_type)? {
                let region = self.scope_region(&n.node_id, depth)?;
                for id in &region {
                    *membership.entry(id.clone()).or_default() += 1;
                }
                let open_gaps = gaps
                    .iter()
                    .filter(|g| g.affected_ids.iter().any(|id| region.contains(id)))
                    .count();
                let open_defects = defects
                    .iter()
                    .filter(|d| d.affected_ids.iter().any(|id| region.contains(id)))
                    .count();
                let mut held_by: Vec<String> = claims
                    .iter()
                    .filter(|c| region.contains(&c.seed_id))
                    .map(|c| c.contributor_id.clone())
                    .collect();
                held_by.sort();
                held_by.dedup();
                regions.push(DesignRegion {
                    name: n
                        .properties
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&n.node_id)
                        .to_string(),
                    seed_id: n.node_id,
                    node_type: seed_type.to_string(),
                    region_size: region.len(),
                    open_gaps,
                    open_defects,
                    held_by,
                });
            }
        }

        // Most currently owed first. Ties break on the id so the order is
        // stable across calls — a listing that reshuffles between two identical
        // reads is one nobody can compare.
        regions.sort_by(|a, b| {
            (b.open_gaps + b.open_defects)
                .cmp(&(a.open_gaps + a.open_defects))
                .then_with(|| b.region_size.cmp(&a.region_size))
                .then_with(|| a.seed_id.cmp(&b.seed_id))
        });

        let all: BTreeSet<String> = self.node_type_index()?.keys().cloned().collect();
        let mut uncovered_by_type: BTreeMap<String, usize> = BTreeMap::new();
        let index = self.node_type_index()?;
        for id in &all {
            if !membership.contains_key(id) {
                let ty = index.get(id).cloned().unwrap_or_else(|| "unknown".into());
                *uncovered_by_type.entry(ty).or_default() += 1;
            }
        }
        let in_some_region = membership.len();
        let in_more_than_one = membership.values().filter(|&&c| c > 1).count();

        let note = regions.is_empty().then(|| {
            format!(
                "This design names no parts yet — no {} nodes exist — so there is no region to \
                 stand in and `regions: []` is VACUOUS rather than a clean or complete map. Every \
                 read here is design-wide until the design is given a Project or some Components; \
                 that is a normal early state, not a fault.",
                REGION_SEED_TYPES.join(" or ")
            )
        });

        Ok(DesignRegions {
            depth,
            basis: REGION_SEED_TYPES.to_vec(),
            regions,
            coverage: RegionCoverage {
                nodes: all.len(),
                in_some_region,
                in_no_region: all.len().saturating_sub(in_some_region),
                in_more_than_one,
                uncovered_by_type,
            },
            total_gaps: gaps.len(),
            total_defects: defects.len(),
            order: "most currently open (gaps + defects) first, then larger region, then id — \
                    what is outstanding there today, NOT a ranking of importance",
            note,
        })
    }
}
