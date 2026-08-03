//! Where a design sits on the trajectory from function to structure (BL-179).
//!
//! Anthony's realisation, 2026-08-02, and the reason this module exists at all:
//!
//! > *"probably all designs start immature (breadboard or engineering mockup)
//! > and then morph to a more refined structure… initially getting function
//! > right is the first objective, then you go towards getting the
//! > structure/packaging right. this is often iterative and organic."*
//!
//! **That reframes every structural number reflow2 takes.** A design with a
//! well-developed function layer and no declared seams is not in debt — it is
//! at a normal and correct point on a normal arc. So the thing worth building
//! is not a detector for bad structure but **knowledge that the arc exists and
//! where a design is along it**.
//!
//! # It reports a position, never a verdict
//!
//! This is `dec:readiness-is-an-observation-the-threshold-is-the-judgement`
//! applied to the design's own structure rather than to enabling technologies.
//! There, reflow2 computes a TRL and **refuses to supply the level at which it
//! gates**, because "below 5 means not buildable" is a policy about risk
//! appetite and a default would let reflow2 silently decide buildability.
//!
//! The same rule holds here, and it is the whole safety property: **reflow2
//! never states where a design should be.** A demonstrator may sit at
//! function-first forever and be exactly right; a fielded increment may not.
//! That judgement belongs to whoever knows what the design is for.
//!
//! # No thresholds at all, which is stronger than stated ones
//!
//! The frontier is the **lowest-scoring** band — a purely relative reading, so
//! there is no bar to default, argue with, or quietly tune. A design where
//! everything is equally undeveloped has a frontier and no complaint;
//! `granularity`'s stated cutoffs were the best available there, and needing
//! none here is better.
//!
//! # The bands are not a ladder, and saying so matters
//!
//! Real designs run ahead of themselves. reflow2's own realization and
//! assurance bands both sit far above its seam band — it has been shipping,
//! tested, for months, with no contract declared between any two of its eight
//! systems. Forcing that into a linear stage would misdescribe it, so bands
//! scoring above the frontier are reported as exactly what they are: **normal,
//! not out of order**.
//!
//! # And it puts no name on the position
//!
//! No `breadboard` / `EVT` / `production` label is emitted, on Anthony's call.
//! `dec:edge-orthogonality` says a distinction earns its keep only when a
//! computation reads it — and a stage name nothing computes over is precisely
//! that. The ladder is how people talk about the profile; it is not something
//! reflow2 asserts.
//!
//! Pure arithmetic over edges already in the graph — no file I/O, no LLM, no
//! new vocabulary, and deterministic.

use std::collections::{BTreeSet, HashMap, HashSet};

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// One band of the trajectory: a question about the design, answered as a
/// count over a population.
///
/// Carries no severity and no target. `ratio` is `None` when the population is
/// empty — a design with no components has no seams to declare, and scoring
/// that as zero would invent a deficiency out of an absence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaturityBand {
    /// Position on the trajectory, 1-based. Ordering only — **not** a rank, a
    /// score, or a claim that the bands must be done in this sequence.
    pub order: usize,
    /// Stable key: `intent`, `function`, `allocation`, `seams`,
    /// `realization`, `assurance`, `operation`.
    pub name: &'static str,
    /// What the ratio actually asks, in words, so the number is never read
    /// without its meaning.
    pub question: &'static str,
    pub present: usize,
    pub population: usize,
    /// `present / population`, or `None` when there is nothing to measure.
    pub ratio: Option<f64>,
    /// Why this band could not be measured, when it could not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The design's position on the trajectory.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaturityProfile {
    /// Every band, in trajectory order. Always all of them, measured or not.
    pub bands: Vec<MaturityBand>,
    /// The lowest-scoring measurable band — the design's current frontier.
    /// `None` when nothing could be measured at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontier: Option<&'static str>,
    /// Bands ordered AFTER the frontier that nonetheless score above it.
    /// **This is the function-first pattern showing up as data**, and it is
    /// reported as normal rather than as work done out of order.
    pub ahead_of_frontier: Vec<&'static str>,
    /// What this reading is silent about — on every profile, including a
    /// flattering one.
    pub not_observed_about: Vec<String>,
    pub notes: Vec<String>,
}

/// The bands, in trajectory order, with the question each one asks.
const BANDS: &[(&str, &str)] = &[
    (
        "intent",
        "Of the requirements still live, how many carry the user's own word — accepted or met \
         rather than merely asserted?",
    ),
    (
        "function",
        "Of the requirements still live, how many have a Capability that SATISFIES them?",
    ),
    (
        "allocation",
        "Of the capabilities, how many are placed in a Component?",
    ),
    (
        "seams",
        "Of the couplings between two Components, how many run through a declared Interface that \
         one provides and the other consumes?",
    ),
    (
        "realization",
        "Of the capabilities, how many have an Artifact that REALIZES them?",
    ),
    (
        "assurance",
        "Of the capabilities, how many carry a Verification that is actually PASSING?",
    ),
    (
        "operation",
        "Of the releases, how many reached an Environment?",
    ),
];

/// A node's string property, or `None`.
fn prop<'a>(n: &'a dynograph_storage::StoredNode, key: &str) -> Option<&'a str> {
    n.properties.get(key).and_then(Value::as_str)
}

impl DesignGraph {
    /// Read where this design sits on the function-to-structure trajectory.
    ///
    /// See the module docs for what this reports and — more importantly — what
    /// it refuses to say.
    pub fn maturity_report(&self) -> Result<MaturityProfile, DynoError> {
        // ---- populations -------------------------------------------------
        let reqs = self.scan_nodes(node::REQUIREMENT)?;
        let live: Vec<_> = reqs
            .iter()
            .filter(|r| !matches!(prop(r, "status"), Some("dropped") | Some("deferred")))
            .collect();
        let confirmed = live
            .iter()
            .filter(|r| matches!(prop(r, "status"), Some("accepted") | Some("met")))
            .count();

        let caps = self.scan_nodes(node::CAPABILITY)?;
        let cap_ids: BTreeSet<String> = caps.iter().map(|c| c.node_id.clone()).collect();

        let mut satisfied: HashSet<String> = HashSet::new();
        let mut allocated: HashSet<String> = HashSet::new();
        let mut realized: HashSet<String> = HashSet::new();
        for c in &caps {
            for e in self.outgoing(&c.node_id, Some(edge::SATISFIES))? {
                satisfied.insert(e.to_id);
            }
            if !self
                .outgoing(&c.node_id, Some(edge::ALLOCATED_TO))?
                .is_empty()
            {
                allocated.insert(c.node_id.clone());
            }
            for e in self.incoming(&c.node_id, Some(edge::REALIZES))? {
                if self.get_node(node::ARTIFACT, &e.from_id)?.is_some() {
                    realized.insert(c.node_id.clone());
                }
            }
        }

        // Assurance counts only checks that PASS: `dec:passing-is-verified`
        // — a check that exists is inventory, not evidence.
        let mut assured: HashSet<String> = HashSet::new();
        for v in self.scan_nodes(node::VERIFICATION)? {
            if prop(&v, "status") != Some("passing") {
                continue;
            }
            for e in self.outgoing(&v.node_id, Some(edge::VERIFIES))? {
                if cap_ids.contains(&e.to_id) {
                    assured.insert(e.to_id);
                }
            }
        }

        // ---- seams: which component pairs are joined by a real contract ----
        let mut provided: HashMap<String, HashSet<String>> = HashMap::new();
        let mut consumed: HashMap<String, HashSet<String>> = HashMap::new();
        for i in self.scan_nodes(node::INTERFACE)? {
            for e in self.incoming(&i.node_id, Some(edge::PROVIDES))? {
                provided
                    .entry(i.node_id.clone())
                    .or_default()
                    .insert(e.from_id);
            }
            for e in self.incoming(&i.node_id, Some(edge::CONSUMES))? {
                consumed
                    .entry(i.node_id.clone())
                    .or_default()
                    .insert(e.from_id);
            }
        }
        // A contract counts only when BOTH sides are recorded: one-sided is
        // exactly the "unrecorded contract" the capture skill warns about.
        let mut declared: HashSet<(String, String)> = HashSet::new();
        for (iid, ps) in &provided {
            if let Some(cs) = consumed.get(iid) {
                for p in ps {
                    for c in cs {
                        if p != c {
                            let (a, b) = if p < c { (p, c) } else { (c, p) };
                            declared.insert((a.clone(), b.clone()));
                        }
                    }
                }
            }
        }
        let mut couplings: HashSet<(String, String)> = HashSet::new();
        for c in self.scan_nodes(node::COMPONENT)? {
            for e in self.outgoing(&c.node_id, Some(edge::DEPENDS_ON))? {
                if self.get_node(node::COMPONENT, &e.to_id)?.is_some() && e.to_id != c.node_id {
                    let (a, b) = if c.node_id < e.to_id {
                        (c.node_id.clone(), e.to_id.clone())
                    } else {
                        (e.to_id.clone(), c.node_id.clone())
                    };
                    couplings.insert((a, b));
                }
            }
        }
        let covered = couplings.intersection(&declared).count();

        let releases = self.scan_nodes(node::RELEASE)?;
        let mut deployed = 0usize;
        for r in &releases {
            if !self
                .outgoing(&r.node_id, Some(edge::DEPLOYED_TO))?
                .is_empty()
            {
                deployed += 1;
            }
        }

        let measured: [(usize, usize); 7] = [
            (confirmed, live.len()),
            (
                live.iter()
                    .filter(|r| satisfied.contains(&r.node_id))
                    .count(),
                live.len(),
            ),
            (allocated.len(), caps.len()),
            (covered, couplings.len()),
            (realized.len(), caps.len()),
            (assured.len(), caps.len()),
            (deployed, releases.len()),
        ];

        let empty_reason = |name: &str| -> String {
            match name {
                "seams" => "no two Components depend on each other, so there is no seam to \
                            declare — an absence, not a deficiency"
                    .to_string(),
                "operation" => "no Release is recorded, so nothing has had the chance to reach an \
                                environment"
                    .to_string(),
                _ => format!(
                    "nothing of the kind this band measures is recorded, so {name} is \
                              unmeasured rather than zero"
                ),
            }
        };

        let mut bands = Vec::with_capacity(BANDS.len());
        for (i, ((name, question), (present, population))) in
            BANDS.iter().zip(measured.iter()).enumerate()
        {
            bands.push(MaturityBand {
                order: i + 1,
                name,
                question,
                present: *present,
                population: *population,
                ratio: (*population > 0).then(|| *present as f64 / *population as f64),
                note: (*population == 0).then(|| empty_reason(name)),
            });
        }

        // ---- the frontier: lowest measurable band, earliest on a tie ------
        let frontier = bands
            .iter()
            .filter(|b| b.ratio.is_some())
            .min_by(|a, b| {
                a.ratio
                    .partial_cmp(&b.ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.order.cmp(&b.order))
            })
            .map(|b| b.name);

        let frontier_ratio =
            frontier.and_then(|f| bands.iter().find(|b| b.name == f).and_then(|b| b.ratio));
        let frontier_order = frontier
            .and_then(|f| bands.iter().find(|b| b.name == f).map(|b| b.order))
            .unwrap_or(0);
        let ahead_of_frontier: Vec<&'static str> = bands
            .iter()
            .filter(|b| {
                b.order > frontier_order
                    && match (b.ratio, frontier_ratio) {
                        (Some(r), Some(f)) => r > f,
                        _ => false,
                    }
            })
            .map(|b| b.name)
            .collect();

        let mut notes = Vec::new();
        match frontier {
            Some(f) => notes.push(format!(
                "The frontier is `{f}` — the lowest-scoring band. It is a RELATIVE reading: there \
                 is no threshold here to default, because where this design SHOULD be is not \
                 reflow2's to say."
            )),
            None => notes.push(
                "Nothing could be measured: this design records none of the populations the \
                 bands count. That is an empty design, not an immature one."
                    .to_string(),
            ),
        }
        if !ahead_of_frontier.is_empty() {
            notes.push(format!(
                "{} band(s) score above the frontier: {}. That is NORMAL and is the pattern this \
                 reading exists to show — designs get function right first and structure later, \
                 iteratively and organically. It is not work done out of order.",
                ahead_of_frontier.len(),
                ahead_of_frontier.join(", ")
            ));
        }

        Ok(MaturityProfile {
            bands,
            frontier,
            ahead_of_frontier,
            not_observed_about: vec![
                "Where this design SHOULD be. A demonstrator may sit at function-first forever \
                 and be exactly right; a fielded increment may not. That is a judgement about \
                 what the design is for, and reflow2 refuses to default it — the same rule \
                 `dec:readiness-is-an-observation-the-threshold-is-the-judgement` holds for TRL."
                    .to_string(),
                "Whether these are the right seven questions. The bands are a claim about how \
                 designs mature, not a measurement of one."
                    .to_string(),
                "Anything unrecorded. A seam that exists in the code but was never declared is \
                 invisible here, and so is a requirement nobody wrote down — this reads the \
                 design, never the subject system."
                    .to_string(),
                "Quality. Every band counts whether something EXISTS, not whether it is any \
                 good: a confirmed requirement may be vague, and a passing check may be weak."
                    .to_string(),
            ],
            notes,
        })
    }
}
