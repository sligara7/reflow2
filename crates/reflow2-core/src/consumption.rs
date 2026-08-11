//! What the design built, and nothing in the design consumes.
//!
//! **This module reports a fact and refuses a verdict.** It never says
//! "unused", never says "dead code", never says "delete it", and carries no
//! severity. It says one thing: *this design records nothing downstream of
//! capability X* — and leaves what to do about it to the agent and the human
//! (`dec:report-dont-judge`, `dec:three-party-checks`).
//!
//! # Why the wording is the feature and not a caveat on it
//!
//! reflow2 reads a design, never a running system. So a capability that real
//! users call every day, whose consumer nobody ever modelled, produces
//! **exactly the same graph shape** as one that has been dead since the day it
//! shipped. Those two are indistinguishable here and always will be.
//!
//! That is why the finding is phrased as a statement about the *record* —
//! nothing in this design consumes X — rather than about the *system*. A
//! detector that collapsed the two would confidently recommend deleting working
//! code, which is the most expensive wrong answer this codebase can give.
//!
//! # Why absence is only informative when presence is the habit
//!
//! The measurement that shaped this module: run over reflow2's own design on
//! 2026-08-11, the three signals below would have named **100 of 110** built
//! capabilities. Not because reflow2 is 91% surplus, but because this design
//! holds twelve consumption edges in total and zero `Flow` nodes — consumption
//! is simply not something it models.
//!
//! So the same rule [`crate::granularity`] already applies to size applies here
//! to consumption: measure against **this design's own habit**, never against
//! an absolute bar. If a design records consumption for most of what it built,
//! the few exceptions stand out and are worth a look. If it barely records
//! consumption at all, the honest finding is *that* — reported as a note — and
//! the list is withheld, because a hundred findings nobody can act on is how a
//! check gets switched off on its first day.
//!
//! This is the second instance of `dec:idea-distributional-thresholds`, and the
//! cutoff is a public constant stated in every report for the same reason
//! [`crate::granularity::UNUSUAL_AT`] is: a threshold nobody can see is one
//! nobody can argue with.
//!
//! Pure arithmetic over edges already in the graph — no file I/O, no LLM, and
//! deterministic: the same design always yields the byte-identical report.

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// The smallest population this will speak about at all.
///
/// Below it, "three of four things have no consumer" describes the sample
/// rather than the design. Reported as a note, never as silence — the same
/// floor and the same reasoning as [`crate::granularity::MIN_POPULATION`].
pub const MIN_POPULATION: usize = 5;

/// How much of the built population must already record a consumer before the
/// absence of one is worth reporting.
///
/// **A distributional cutoff, not an absolute one.** At or above this,
/// recording consumption is evidently this design's habit and a capability
/// without it is out of line with its own neighbours. Below it, not recording
/// consumption is the norm, so absence carries no signal and the list would be
/// a census of the design's modelling style rather than a finding about what
/// was built.
///
/// Set at half deliberately: the habit has to be the *majority* practice before
/// a departure from it means anything.
pub const MIN_MODELLED_RATIO: f64 = 0.5;

/// The statuses that count as built. Something still `planned` cannot be
/// surplus, and `unrealized_capability` already asks what will build it.
const BUILT: [&str; 2] = ["realized", "verified"];

/// One built capability with nothing recorded downstream of it.
///
/// Deliberately carries **no** severity, no category and no suggested fix.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsumptionObservation {
    pub node_id: String,
    /// The capability's `name`, when it carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `realized` or `verified` — which kind of built this is.
    pub status: String,
    /// Plain-language statement of what was observed. Phrased as a fact about
    /// the record, never about the running system.
    pub reasons: Vec<String>,
}

/// The consumption reading for a whole design.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsumptionReport {
    /// Built capabilities with nothing recorded downstream. Empty is a
    /// perfectly ordinary answer, and is also what a design below
    /// [`MIN_MODELLED_RATIO`] gets — read `notes` to tell those apart.
    pub observations: Vec<ConsumptionObservation>,
    /// Built, live capabilities — the population every figure is relative to.
    pub population: usize,
    /// How many of the population have a consumer recorded. The headline
    /// number: on a design that models consumption thinly this is the finding,
    /// and the list above is withheld rather than guessed at.
    pub consumption_modelled: usize,
    /// The edge types read as consumption, named so a reader can see the
    /// boundary rather than trust it — and argue that one is missing.
    pub signals_read: Vec<String>,
    /// The cutoff actually applied, echoed so it can be argued with.
    pub min_modelled_ratio: f64,
    /// What this reading is silent about. Present on every report, including
    /// an empty one — a quiet report is evidence about what it covers and says
    /// nothing about the rest.
    pub not_observed_about: Vec<String>,
    /// Anything that shaped this particular answer: too small a population,
    /// consumption modelled too thinly, nothing built yet.
    pub notes: Vec<String>,
}

impl DesignGraph {
    /// Read what this design built and records no consumer for.
    ///
    /// See the module docs for what this does and — more importantly — what it
    /// refuses to do.
    pub fn consumption_report(&self) -> Result<ConsumptionReport, DynoError> {
        let signals_read = vec![
            "DEPENDS_ON (incoming — something needs it)".to_string(),
            "PART_OF_FLOW (outgoing — it is a step of a process)".to_string(),
            "INTERACTS_WITH (incoming — an actor reaches it)".to_string(),
        ];
        let not_observed_about = vec![
            "Whether anything actually runs it. reflow2 reads a design, never a running system: a \
             capability real users call daily whose consumer nobody modelled looks exactly like \
             one that has been dead since it shipped."
                .to_string(),
            "Consumers outside this design. A capability consumed by another team's system, or by \
             a script nobody registered, is indistinguishable here from one consumed by nothing."
                .to_string(),
            "Whether being unconsumed is a problem. A capability built ahead of its consumer, a \
             deliberate extension point, and genuine surplus all produce this same shape. That \
             judgement is not reflow2's."
                .to_string(),
        ];

        let mut population: Vec<(String, Option<String>, String)> = Vec::new();
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let status = cap
                .properties
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !BUILT.contains(&status.as_str()) {
                continue;
            }
            // A withdrawn capability is not surplus — it is withdrawn, and
            // `dec:idea-discontinued-is-a-first-class-state` already covers it.
            if self.is_discontinued(&cap.node_id)? {
                continue;
            }
            let name = cap
                .properties
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string);
            population.push((cap.node_id, name, status));
        }

        let mut unconsumed: Vec<ConsumptionObservation> = Vec::new();
        let mut modelled = 0usize;
        for (node_id, name, status) in &population {
            if self.has_a_recorded_consumer(node_id)? {
                modelled += 1;
                continue;
            }
            unconsumed.push(ConsumptionObservation {
                node_id: node_id.clone(),
                name: name.clone(),
                status: status.clone(),
                reasons: vec![format!(
                    "This design is `{status}` for this capability and records nothing that \
                     consumes it: no capability depends on it, it is a step of no flow, and no \
                     actor interacts with it."
                )],
            });
        }

        let population_count = population.len();
        let mut notes = Vec::new();

        if population_count < MIN_POPULATION {
            notes.push(format!(
                "{population_count} built capability(s) — below the {MIN_POPULATION} this will \
                 speak about. A ratio over so few would describe the sample, not the design, so \
                 no list is reported."
            ));
            return Ok(ConsumptionReport {
                observations: Vec::new(),
                population: population_count,
                consumption_modelled: modelled,
                signals_read,
                min_modelled_ratio: MIN_MODELLED_RATIO,
                not_observed_about,
                notes,
            });
        }

        let ratio = modelled as f64 / population_count as f64;
        if ratio < MIN_MODELLED_RATIO {
            let pct = (ratio * 100.0).round() as usize;
            notes.push(format!(
                "This design records a consumer for {modelled} of {population_count} built \
                 capabilities ({pct}%), below the {:.0}% at which absence carries a signal. \
                 Recording consumption is not this design's habit, so departing from it means \
                 nothing and the {} capability(s) with no recorded consumer are NOT listed — \
                 naming them would report the modelling style rather than what was built. The \
                 finding here is the ratio itself.",
                MIN_MODELLED_RATIO * 100.0,
                unconsumed.len(),
            ));
            return Ok(ConsumptionReport {
                observations: Vec::new(),
                population: population_count,
                consumption_modelled: modelled,
                signals_read,
                min_modelled_ratio: MIN_MODELLED_RATIO,
                not_observed_about,
                notes,
            });
        }

        unconsumed.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        Ok(ConsumptionReport {
            observations: unconsumed,
            population: population_count,
            consumption_modelled: modelled,
            signals_read,
            min_modelled_ratio: MIN_MODELLED_RATIO,
            not_observed_about,
            notes,
        })
    }

    /// Does the design record anything downstream of this capability?
    ///
    /// A consumer that has itself been discontinued does not count — a
    /// withdrawn caller is not a caller, and counting it would let one
    /// discontinuation quietly keep another capability looking consumed.
    fn has_a_recorded_consumer(&self, node_id: &str) -> Result<bool, DynoError> {
        for e in self.incoming(node_id, Some(edge::DEPENDS_ON))? {
            if !self.is_discontinued(&e.from_id)? {
                return Ok(true);
            }
        }
        for e in self.incoming(node_id, Some(edge::INTERACTS_WITH))? {
            if !self.is_discontinued(&e.from_id)? {
                return Ok(true);
            }
        }
        for e in self.outgoing(node_id, Some(edge::PART_OF_FLOW))? {
            if !self.is_discontinued(&e.to_id)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
