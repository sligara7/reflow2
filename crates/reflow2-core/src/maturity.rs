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

use crate::foundation::core::{DynoError, Value};
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
fn prop<'a>(n: &'a crate::foundation::store::StoredNode, key: &str) -> Option<&'a str> {
    n.properties.get(key).and_then(Value::as_str)
}

/// The two Component-pair sets the `seams` band divides — named, because the
/// difference between them is a finding and used to be thrown away.
///
/// Pairs are unordered and stored smaller-id-first, so `cmp:a`→`cmp:b` and
/// `cmp:b`→`cmp:a` are one seam rather than two.
pub(crate) struct SeamSets {
    /// Component pairs joined by a `DEPENDS_ON` edge — the seams that exist.
    pub couplings: BTreeSet<(String, String)>,
    /// Component pairs joined by a real contract — the seams that are written
    /// down. A subset of `couplings` only in practice: two components can share
    /// an Interface without depending on each other, which is why the band
    /// intersects rather than subtracts.
    pub declared: BTreeSet<(String, String)>,
    /// The `Component.level` these sets were lifted to, or `None` for the raw
    /// module-level answer.
    pub altitude: Option<String>,
    /// How many couplings existed BEFORE lifting.
    ///
    /// **THIS IS WHAT STOPS A ZERO FROM FLATTERING.** Lifted to `subsystem`,
    /// this design reports nothing undeclared — but it compared 11 pairs, not
    /// 72. Without this figure beside it, "0 undeclared" reads as "everything
    /// is contracted" when it means "everything is contracted AT THIS
    /// ALTITUDE", which is the whole family of defect this project keeps
    /// finding.
    pub raw_couplings: usize,
    /// How many contract pairs existed before lifting.
    pub raw_declared: usize,
    /// Endpoints that reached no container at the requested level and were
    /// dropped from both sets. Counted rather than silently kept at their own
    /// level, which would mix two altitudes in one answer.
    pub unreachable: usize,
}

/// One boundary that IS covered, and where the contract actually lives.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoveredSeam {
    /// The two parts, at the altitude asked for.
    pub between: (String, String),
    /// The finest-level pairs whose contract covers it. Empty at module
    /// altitude, where the pair IS the place it is declared.
    pub declared_at: Vec<String>,
}

/// Which boundaries are covered by a contract at a chosen altitude.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeamCoverage {
    /// The level this was answered at; `None` is the raw module answer.
    pub altitude: Option<String>,
    /// Couplings visible at this altitude.
    pub couplings: usize,
    /// Contract pairs visible at this altitude.
    pub declared: usize,
    /// Couplings covered by a contract at or below them.
    pub covered: usize,
    /// Couplings with no contract at or below them — the finding.
    pub uncovered: Vec<(String, String)>,
    /// Every covered boundary, naming where its contract is actually declared.
    pub covered_by: Vec<CoveredSeam>,
    /// What this answer compared, and what it therefore does NOT say.
    pub scope_note: String,
}

impl SeamSets {
    /// The couplings with no contract recorded between them — what
    /// `req:an-undeclared-coupling-is-named-not-just-counted` exists to name.
    pub fn undeclared(&self) -> Vec<(String, String)> {
        self.couplings.difference(&self.declared).cloned().collect()
    }
}

impl DesignGraph {
    /// Which Component pairs are coupled, and which of those run through a
    /// declared contract.
    ///
    /// EXTRACTED SO THERE IS ONE DEFINITION. The `seams` band divides these two
    /// sets and `detect_undeclared_seams` names their difference; computing
    /// "declared" twice would let a detector and a band disagree about what a
    /// contract is, and the band would be the one nobody could argue with.
    pub(crate) fn seam_sets(&self) -> Result<SeamSets, DynoError> {
        self.seam_sets_at(None)
    }

    /// [`seam_sets`](Self::seam_sets), answered at a chosen ALTITUDE.
    ///
    /// `altitude` is a `Component.level` (`subsystem`, `system`, …). Each side
    /// of every coupling and every contract is LIFTED to the nearest container
    /// at that level before the two sets are compared, so the question becomes
    /// *"is this coupling covered by a contract declared at or below it?"*
    /// rather than *"do these two exact modules share one?"*.
    ///
    /// **BOTH SETS ARE LIFTED, AND THAT SYMMETRY IS THE WHOLE CORRECTNESS
    /// ARGUMENT.** `fact:coupling-and-contract-are-recorded-in-vocabularies-that-never-meet`
    /// measured the two sets as DISJOINT BY CONSTRUCTION on this design — 72
    /// couplings, 26 contract pairs, zero shared — because couplings are
    /// recorded between modules and contracts between the boxes that contain
    /// them. Lifting one side alone would not fix that; lifting both puts them
    /// in the same vocabulary for the first time.
    ///
    /// MEASURED 2026-08-23 on reflow2's own design: at module level 72
    /// couplings against 42 declared leaves 64 undeclared; lifted to
    /// `subsystem` it is 11 against 13 and **nothing** undeclared. That zero is
    /// real and it is also the reason [`SeamSets::altitude`] and
    /// [`SeamSets::raw_couplings`] exist — see their docs.
    ///
    /// A component that reaches no container at the requested level is DROPPED
    /// from both sets rather than compared against itself, and the count of
    /// what was dropped rides on the result. Silently keeping it at its own
    /// level would mix two altitudes in one answer.
    pub(crate) fn seam_sets_at(&self, altitude: Option<&str>) -> Result<SeamSets, DynoError> {
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
        let mut declared: BTreeSet<(String, String)> = BTreeSet::new();
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
        let mut couplings: BTreeSet<(String, String)> = BTreeSet::new();
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
        let raw_couplings = couplings.len();
        let raw_declared = declared.len();
        let (couplings, declared, unreachable) = match altitude {
            None => (couplings, declared, 0),
            Some(level) => {
                let mut lift_cache: HashMap<String, Option<String>> = HashMap::new();
                let mut dropped = 0usize;
                let mut lift_pairs =
                    |set: BTreeSet<(String, String)>,
                     graph: &Self,
                     dropped: &mut usize|
                     -> Result<BTreeSet<(String, String)>, DynoError> {
                        let mut out = BTreeSet::new();
                        for (a, b) in set {
                            let la = graph.lift_to_level(&a, level, &mut lift_cache)?;
                            let lb = graph.lift_to_level(&b, level, &mut lift_cache)?;
                            match (la, lb) {
                                (Some(x), Some(y)) if x != y => {
                                    out.insert(if x < y { (x, y) } else { (y, x) });
                                }
                                // Same container: the coupling is INTERNAL to one
                                // box at this altitude, so it is not a seam here.
                                (Some(_), Some(_)) => {}
                                _ => *dropped += 1,
                            }
                        }
                        Ok(out)
                    };
                let c = lift_pairs(couplings, self, &mut dropped)?;
                let d = lift_pairs(declared, self, &mut dropped)?;
                (c, d, dropped)
            }
        };
        Ok(SeamSets {
            couplings,
            declared,
            altitude: altitude.map(str::to_string),
            raw_couplings,
            raw_declared,
            unreachable,
        })
    }

    /// The nearest container of `id` at `level`, or `id` itself if it is
    /// already there. `None` when the spine runs out before reaching it.
    fn lift_to_level(
        &self,
        id: &str,
        level: &str,
        cache: &mut HashMap<String, Option<String>>,
    ) -> Result<Option<String>, DynoError> {
        if let Some(hit) = cache.get(id) {
            return Ok(hit.clone());
        }
        let mut here = id.to_string();
        let mut seen: HashSet<String> = HashSet::new();
        let answer = loop {
            if !seen.insert(here.clone()) {
                break None; // a containment cycle; refuse rather than loop
            }
            let node = self.get_node(node::COMPONENT, &here)?;
            let Some(node) = node else { break None };
            if prop(&node, "level") == Some(level) {
                break Some(here);
            }
            let parent = self
                .incoming(&here, Some(edge::CONTAINS))?
                .into_iter()
                .map(|e| e.from_id)
                .find(|p| p.starts_with("cmp:") || p.starts_with("sys:"));
            match parent {
                Some(p) => here = p,
                None => break None,
            }
        };
        cache.insert(id.to_string(), answer.clone());
        Ok(answer)
    }

    /// Which boundaries are covered by a contract, answered at the altitude
    /// you asked the question at.
    ///
    /// `req:an-undeclared-coupling-is-named-not-just-counted` is answered at
    /// module level today, and that is the only level at which it can be
    /// answered — so a design that DECLARES ITS CONTRACTS AT THE SUBSYSTEM
    /// BOUNDARY reads as having none at all. Anthony, 2026-08-23:
    /// *"this should be defined at the lowest level that actually defines the
    /// interface and the rest is rolled up so that if somebody asked at a high
    /// level, 'is there an interface between subsystem_A and subsystem_B?' the
    /// view … would say 'yes', but the graph would actually show it is
    /// subsystem_A.component_2 <-> subsystem_B.component_5 that the contract is
    /// actually defined."*
    ///
    /// THIS IS A PROJECTION AND NOTHING IS WRITTEN BACK. The answer is derived
    /// from `CONTAINS` + `PROVIDES` + `CONSUMES` on every call. Storing a
    /// rolled-up edge between two subsystems would make the graph assert a
    /// contract nobody declared, which `dec:views-are-projections` forbids.
    ///
    /// `covered_by` names the LEAF pair for every covered boundary, because
    /// "yes there is an interface" without saying where it is declared is the
    /// half-answer that sends a reader hunting.
    pub fn seam_coverage(&self, altitude: Option<&str>) -> Result<SeamCoverage, DynoError> {
        let sets = self.seam_sets_at(altitude)?;
        let mut covered_by: Vec<CoveredSeam> = Vec::new();
        let raw = self.seam_sets_at(None)?;
        for (a, b) in sets.couplings.intersection(&sets.declared) {
            // Where it is ACTUALLY declared: the finest-level contract pairs
            // that lift into this one.
            let mut at: Vec<String> = Vec::new();
            if altitude.is_some() {
                let mut cache = HashMap::new();
                for (p, c) in &raw.declared {
                    let (lp, lc) = (
                        self.lift_to_level(p, altitude.unwrap_or_default(), &mut cache)?,
                        self.lift_to_level(c, altitude.unwrap_or_default(), &mut cache)?,
                    );
                    if let (Some(x), Some(y)) = (lp, lc) {
                        let pair = if x < y { (x, y) } else { (y, x) };
                        if pair == (a.clone(), b.clone()) {
                            at.push(format!("{p} ↔ {c}"));
                        }
                    }
                }
            }
            at.sort();
            at.dedup();
            covered_by.push(CoveredSeam {
                between: (a.clone(), b.clone()),
                declared_at: at,
            });
        }
        Ok(SeamCoverage {
            altitude: sets.altitude.clone(),
            couplings: sets.couplings.len(),
            declared: sets.declared.len(),
            covered: covered_by.len(),
            uncovered: sets.undeclared(),
            covered_by,
            scope_note: match altitude {
                None => format!(
                    "Module level, nothing lifted: {} coupling(s) compared against {} contract \
                     pair(s). A design that declares its contracts at a HIGHER boundary than it \
                     records its dependencies will read as uncovered here and be fully covered one \
                     altitude up — pass `altitude` to ask at the level the contracts live.",
                    sets.raw_couplings, sets.raw_declared,
                ),
                Some(level) => format!(
                    "Answered at `{level}`: {} coupling(s) compared, lifted from {} at module \
                     level{}. A ZERO HERE MEANS EVERY COUPLING VISIBLE AT THIS ALTITUDE IS \
                     COVERED — it says nothing about the {} finer-grained couplings underneath, \
                     which are a different question asked at a different level.",
                    sets.couplings.len(),
                    sets.raw_couplings,
                    if sets.unreachable > 0 {
                        format!(
                            "; {} endpoint(s) reached no container at this level and were dropped \
                             from both sides",
                            sets.unreachable
                        )
                    } else {
                        String::new()
                    },
                    sets.raw_couplings,
                ),
            },
        })
    }

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
        // `seam_sets` is shared with `detect_undeclared_seams`, which names the
        // difference this line discards.
        let seams = self.seam_sets()?;
        let covered = seams.couplings.intersection(&seams.declared).count();
        let couplings = seams.couplings;

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
