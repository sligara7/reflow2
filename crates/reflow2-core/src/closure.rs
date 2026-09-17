//! The closure report — does the design CLOSE, against a threshold the owner
//! declared?
//! (`req:a-design-closes-against-a-declared-threshold-and-the-report-names-the-first-hole`,
//! Anthony 2026-09-16: "need to ensure that designs close and are verified").
//!
//! Five legs, every one a computation that already exists, summed into one
//! read:
//!
//! 1. TRACEABILITY — every live requirement traced to a capability with a
//!    passing check ([`DesignGraph::requirement_is_delivered`], the delivery
//!    line's own predicate).
//! 2. BUDGETS — every Constraint with a `limit` inside it, with the declared
//!    `margin` ([`DesignGraph::budget_report`]).
//! 3. SEAMS — every coupling between parts specified on both sides
//!    ([`DesignGraph::seam_coverage`]).
//! 4. DECISIONS — no scheduled work governed by a decision still open
//!    (the same GOVERNED_BY / SCHEDULED_FOR read `what_next` scores).
//! 5. PROVENANCE — no quantity without a source
//!    ([`DesignGraph::quantity_provenance_sweep`], the detector's own sweep).
//!
//! THREE RULES THE SHAPE ENFORCES, each from the requirement:
//!
//! * A LEG SAYS WHAT IT SWEPT. `swept` is the population and `swept_note`
//!   says it in words ("budgets: 1 modelled"); a leg with NOTHING TO RUN ON
//!   carries `share: None` and `closes: None`, and a criterion that names it
//!   does not close — a detector reporting zero because it had nothing to
//!   check reads exactly like one that ran clean, and this report refuses
//!   that reading (the standing rule, `loop_status` applies it to an unknown
//!   contributor).
//! * THE THRESHOLD IS DECLARED, NEVER DEFAULTED. `Project.closure_legs` names
//!   which legs count and `Project.closure_threshold` the share of each that
//!   must close; 100 is a legal declaration and so is "traceability and
//!   budgets only". No declaration reads `no_closure_criterion_stated` — the
//!   legs are still computed and shown, the verdict is withheld.
//! * A REPORT, NEVER A GATE. Nothing here refuses a release or a commit; the
//!   release report may be cut while this says `does_not_close`, and says so.

use serde::Serialize;

use crate::budget::BudgetVerdict;
use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// The five legs, in the order the report walks them — which is also the
/// order `first_hole` is chosen in: the first leg in the DECLARED order that
/// does not close is the hole.
pub const CLOSURE_LEGS: &[&str] = &[
    "traceability",
    "budgets",
    "seams",
    "decisions",
    "provenance",
];

/// What the owner declared closure to mean.
#[derive(Debug, Clone, Serialize)]
pub struct ClosureCriterion {
    /// Which legs count, in the owner's order.
    pub legs: Vec<String>,
    /// The share of each counted leg that must close, 0–100.
    pub threshold: f64,
}

/// The first thing that keeps the design from closing.
#[derive(Debug, Clone, Serialize)]
pub struct ClosureHole {
    pub leg: String,
    /// The offending node or pair, where there is one; absent when the hole
    /// is the leg having nothing to run on.
    pub id: Option<String>,
    pub why: String,
}

/// One leg's reading.
#[derive(Debug, Clone, Serialize)]
pub struct ClosureLeg {
    pub leg: String,
    /// Whether the declared criterion counts this leg. Every leg is computed
    /// and shown; only counted legs move the verdict.
    pub counted: bool,
    /// The population the leg swept.
    pub swept: usize,
    /// How many of them close.
    pub closed: usize,
    /// `closed / swept` as a percentage; `None` when nothing was swept.
    pub share: Option<f64>,
    /// What was swept, in words — so "0 open" beside "0 modelled" cannot
    /// read as clean.
    pub swept_note: String,
    /// The first offender in deterministic (sorted) order.
    pub worst: Option<ClosureHole>,
    /// Whether this leg meets the threshold; `None` when nothing was swept or
    /// no criterion is declared.
    pub closes: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureVerdict {
    Closes,
    DoesNotClose,
    NoClosureCriterionStated,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClosureReport {
    pub project_id: Option<String>,
    pub criterion: Option<ClosureCriterion>,
    pub legs: Vec<ClosureLeg>,
    pub verdict: ClosureVerdict,
    /// The first counted leg, in declared order, that does not close — with
    /// its worst offender, or the fact that it had nothing to run on.
    pub first_hole: Option<ClosureHole>,
    pub note: String,
}

/// The verdict alone, for reports that carry closure beside their own answer
/// (the release report).
#[derive(Debug, Clone, Serialize)]
pub struct ClosureSummary {
    pub verdict: ClosureVerdict,
    pub first_hole: Option<ClosureHole>,
    pub note: String,
}

/// The provenance sweep, shared with the detector so both read one
/// definition.
#[derive(Debug, Clone, Default)]
pub struct QuantityProvenanceSweep {
    /// Constraints carrying a numeric `limit`.
    pub limits: usize,
    /// Every stated number: limits plus numbered contributions.
    pub quantities: usize,
    /// (affected id, what) for each number with no source, in sweep order.
    pub unsourced: Vec<(String, String)>,
    /// (check, constraint) for each Verification on a limit that no Artifact
    /// IMPLEMENTS, once per check.
    pub checks_without_form: Vec<(String, String)>,
}

fn is_review(id: &str) -> bool {
    id.starts_with("decision:ack:")
}

fn clean(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl DesignGraph {
    /// Declare what closure means for this project: which legs count and the
    /// share of each that must close. Validated here rather than by the
    /// schema because `closure_legs` is a list and the schema has no list
    /// enum; an unknown leg name fails loud with the five that exist.
    pub fn set_closure_criterion(
        &mut self,
        project_id: &str,
        legs: &[&str],
        threshold: f64,
    ) -> Result<crate::foundation::store::StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::PROJECT, project_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::PROJECT.to_string(),
                node_id: project_id.to_string(),
            });
        };
        if legs.is_empty() {
            return Err(DynoError::Validation {
                node_type: node::PROJECT.into(),
                property: "closure_legs".into(),
                message: format!(
                    "a closure criterion names at least one leg (of {}); an empty list would \
                     make every design close",
                    CLOSURE_LEGS.join(", ")
                ),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for leg in legs {
            if !CLOSURE_LEGS.contains(leg) {
                return Err(DynoError::Validation {
                    node_type: node::PROJECT.into(),
                    property: "closure_legs".into(),
                    message: format!(
                        "'{leg}' is not a closure leg (one of {})",
                        CLOSURE_LEGS.join(", ")
                    ),
                });
            }
            if !seen.insert(*leg) {
                return Err(DynoError::Validation {
                    node_type: node::PROJECT.into(),
                    property: "closure_legs".into(),
                    message: format!("'{leg}' is named twice"),
                });
            }
        }
        if !(0.0..=100.0).contains(&threshold) || threshold.is_nan() {
            return Err(DynoError::Validation {
                node_type: node::PROJECT.into(),
                property: "closure_threshold".into(),
                message: format!(
                    "{threshold} is not a share: the threshold is the percentage of each counted \
                     leg that must close, 0 to 100"
                ),
            });
        }
        let list: Vec<Value> = legs.iter().map(|l| Value::from(*l)).collect();
        let mut props = Props::new()
            .set("closure_legs", Value::List(list))
            .set("closure_threshold", threshold);
        for (k, v) in &existing.properties {
            if k != "closure_legs" && k != "closure_threshold" {
                props = props.set(k, v.clone());
            }
        }
        self.upsert_node(node::PROJECT, project_id, props)
    }

    /// The closure verdict alone, for a report that carries it beside its own.
    pub fn closure_summary(&self) -> Result<ClosureSummary, DynoError> {
        let r = self.closure_report()?;
        Ok(ClosureSummary {
            verdict: r.verdict,
            first_hole: r.first_hole,
            note: r.note,
        })
    }

    /// The one sweep behind `quantity_without_source`,
    /// `quantity_check_without_executable_form` and the provenance leg.
    pub fn quantity_provenance_sweep(&self) -> Result<QuantityProvenanceSweep, DynoError> {
        let mut s = QuantityProvenanceSweep::default();
        let mut checks_seen = std::collections::BTreeSet::new();
        for c in self.scan_live_nodes(node::CONSTRAINT)? {
            if c.properties.get("limit").and_then(Value::as_f64).is_none() {
                continue;
            }
            s.limits += 1;
            s.quantities += 1;
            if clean(c.properties.get("limit_source")).is_none() {
                s.unsourced
                    .push((c.node_id.clone(), format!("the limit of '{}'", c.node_id)));
            }
            for e in self.outgoing(&c.node_id, Some(edge::CONSTRAINS))? {
                let has_number = e
                    .properties
                    .get("contribution")
                    .and_then(Value::as_f64)
                    .is_some();
                if !has_number {
                    continue;
                }
                s.quantities += 1;
                if clean(e.properties.get("source")).is_none() {
                    s.unsourced.push((
                        e.to_id.clone(),
                        format!("the contribution of '{}' to '{}'", e.to_id, c.node_id),
                    ));
                }
            }
            for v in self.incoming(&c.node_id, Some(edge::VERIFIES))? {
                if !checks_seen.insert(v.from_id.clone()) {
                    continue;
                }
                if self
                    .incoming(&v.from_id, Some(edge::IMPLEMENTS))?
                    .is_empty()
                {
                    s.checks_without_form
                        .push((v.from_id.clone(), c.node_id.clone()));
                }
            }
        }
        Ok(s)
    }

    /// Does the design close? See the module docs for the five legs and the
    /// three rules.
    pub fn closure_report(&self) -> Result<ClosureReport, DynoError> {
        // The criterion lives on the Project. One project per graph is the
        // norm; with several, the first (sorted) that declares one is read
        // and named, so the choice is visible rather than silent.
        let mut projects = self.scan_live_nodes(node::PROJECT)?;
        projects.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        let mut project_id = projects.first().map(|p| p.node_id.clone());
        let mut criterion: Option<ClosureCriterion> = None;
        for p in &projects {
            let legs: Vec<String> = match p.properties.get("closure_legs") {
                Some(Value::List(items)) => items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            let threshold = p
                .properties
                .get("closure_threshold")
                .and_then(Value::as_f64);
            if let (false, Some(t)) = (legs.is_empty(), threshold) {
                project_id = Some(p.node_id.clone());
                criterion = Some(ClosureCriterion { legs, threshold: t });
                break;
            }
        }

        let counted = |leg: &str| -> bool {
            criterion
                .as_ref()
                .map(|c| c.legs.iter().any(|l| l == leg))
                .unwrap_or(false)
        };
        let threshold = criterion.as_ref().map(|c| c.threshold);

        let mut legs: Vec<ClosureLeg> = vec![
            self.leg_traceability()?,
            self.leg_budgets()?,
            self.leg_seams()?,
            self.leg_decisions()?,
            self.leg_provenance()?,
        ];

        for leg in &mut legs {
            leg.counted = counted(&leg.leg);
            leg.closes = match (threshold, leg.share) {
                (Some(t), Some(share)) if leg.counted => Some(share + 1e-9 >= t),
                _ => None,
            };
        }

        let (verdict, first_hole, note) = match &criterion {
            None => (
                ClosureVerdict::NoClosureCriterionStated,
                None,
                "No closure criterion stated: the project has not declared which legs count or \
                 what share of each must close (set_closure_criterion). The legs are computed \
                 and shown; no verdict is offered in place of the owner's word, and no default \
                 stands in for it."
                    .to_string(),
            ),
            Some(c) => {
                // Walk the DECLARED order, so the first hole is the first leg
                // the owner named that fails, not the first the report
                // happened to compute.
                let mut hole: Option<ClosureHole> = None;
                for name in &c.legs {
                    let Some(leg) = legs.iter().find(|l| &l.leg == name) else {
                        continue;
                    };
                    match leg.closes {
                        Some(true) => {}
                        Some(false) => {
                            hole = Some(leg.worst.clone().unwrap_or(ClosureHole {
                                leg: leg.leg.clone(),
                                id: None,
                                why: format!(
                                    "{:.0}% of {} closes; the criterion asks {:.0}%",
                                    leg.share.unwrap_or(0.0),
                                    leg.swept_note,
                                    c.threshold
                                ),
                            }));
                            break;
                        }
                        None => {
                            hole = Some(ClosureHole {
                                leg: leg.leg.clone(),
                                id: None,
                                why: format!(
                                    "nothing to run on — {}; a leg the criterion counts cannot \
                                     read as closed with nothing swept",
                                    leg.swept_note
                                ),
                            });
                            break;
                        }
                    }
                }
                match hole {
                    None => (
                        ClosureVerdict::Closes,
                        None,
                        format!(
                            "Closes: every counted leg ({}) meets {:.0}%. This is the design's own \
                             chain — intent, budgets, seams, decisions and sources as recorded — \
                             and says nothing about whether the built thing does what its users \
                             need.",
                            c.legs.join(", "),
                            c.threshold
                        ),
                    ),
                    Some(h) => (
                        ClosureVerdict::DoesNotClose,
                        Some(h.clone()),
                        format!(
                            "Does not close: the first hole is on {} — {}. A report, not a gate: \
                             a release may still be cut, and the release report will say the \
                             design did not close.",
                            h.leg, h.why
                        ),
                    ),
                }
            }
        };

        Ok(ClosureReport {
            project_id,
            criterion,
            legs,
            verdict,
            first_hole,
            note,
        })
    }

    fn leg_traceability(&self) -> Result<ClosureLeg, DynoError> {
        let mut reqs = self.scan_live_nodes(node::REQUIREMENT)?;
        reqs.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        let mut swept = 0usize;
        let mut closed = 0usize;
        let mut worst: Option<ClosureHole> = None;
        for r in &reqs {
            let status = r
                .properties
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("proposed");
            if status == "dropped" {
                continue;
            }
            swept += 1;
            let inferred =
                r.properties.get("provenance").and_then(Value::as_str) == Some("inferred");
            if !inferred && self.requirement_is_delivered(&r.node_id)? {
                closed += 1;
            } else if worst.is_none() {
                worst = Some(ClosureHole {
                    leg: "traceability".into(),
                    id: Some(r.node_id.clone()),
                    why: if inferred {
                        format!(
                            "'{}' was inferred from what implements it, so its thread proves nothing",
                            r.node_id
                        )
                    } else {
                        format!(
                            "'{}' has no capability that is built and currently checked",
                            r.node_id
                        )
                    },
                });
            }
        }
        Ok(leg(
            "traceability",
            swept,
            closed,
            format!("requirements: {swept} live"),
            worst,
        ))
    }

    fn leg_budgets(&self) -> Result<ClosureLeg, DynoError> {
        let mut cons = self.scan_live_nodes(node::CONSTRAINT)?;
        cons.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        let mut swept = 0usize;
        let mut closed = 0usize;
        let mut worst: Option<ClosureHole> = None;
        let mut with_margin = 0usize;
        for c in &cons {
            if c.properties.get("limit").and_then(Value::as_f64).is_none() {
                continue;
            }
            swept += 1;
            let r = self.budget_report(&c.node_id)?;
            let margin = c.properties.get("margin").and_then(Value::as_f64);
            if margin.is_some() {
                with_margin += 1;
            }
            let (ok, why) = match r.verdict {
                BudgetVerdict::Within => {
                    // Within the limit — and within the declared margin, when
                    // one is declared. A margin is headroom the owner wants
                    // kept, in the limit's unit.
                    match (margin, r.limit) {
                        (Some(m), Some(limit)) => {
                            let inside = if r.direction == "minimum" {
                                r.total >= limit + m
                            } else {
                                r.total <= limit - m
                            };
                            if inside {
                                (true, String::new())
                            } else {
                                (
                                    false,
                                    format!(
                                        "'{}' is within its limit ({}) but not its declared margin ({}): total {}",
                                        c.node_id, limit, m, r.total
                                    ),
                                )
                            }
                        }
                        _ => (true, String::new()),
                    }
                }
                BudgetVerdict::Exceeded => (
                    false,
                    format!(
                        "'{}' is exceeded: total {} against limit {}",
                        c.node_id,
                        r.total,
                        r.limit.unwrap_or(f64::NAN)
                    ),
                ),
                BudgetVerdict::Incomplete => (
                    false,
                    format!(
                        "'{}' cannot be summed: {} contribution(s) unstated, {} in another unit",
                        c.node_id,
                        r.unstated.len(),
                        r.unit_mismatched.len()
                    ),
                ),
                BudgetVerdict::Ungated => (true, String::new()),
            };
            if ok {
                closed += 1;
            } else if worst.is_none() {
                worst = Some(ClosureHole {
                    leg: "budgets".into(),
                    id: Some(c.node_id.clone()),
                    why,
                });
            }
        }
        let note = format!("budgets: {swept} modelled, {with_margin} with a declared margin");
        Ok(leg("budgets", swept, closed, note, worst))
    }

    fn leg_seams(&self) -> Result<ClosureLeg, DynoError> {
        let s = self.seam_coverage(None)?;
        let mut uncovered = s.uncovered.clone();
        uncovered.sort();
        let worst = uncovered.first().map(|(a, b)| ClosureHole {
            leg: "seams".into(),
            id: Some(format!("{a} <-> {b}")),
            why: format!("'{a}' and '{b}' are coupled and no contract between them is declared"),
        });
        let note = format!(
            "seams: {} coupling(s) between parts, {} contract pair(s) declared",
            s.couplings, s.declared
        );
        Ok(leg("seams", s.couplings, s.covered, note, worst))
    }

    fn leg_decisions(&self) -> Result<ClosureLeg, DynoError> {
        // The scheduled increment: every node SCHEDULED_FOR an epoch still
        // `planned`. Each is closed when no decision governing it is still
        // open. Swept = the scheduled items, so "no increment scheduled"
        // reads as nothing to run on rather than as clean.
        let mut epochs = self.scan_live_nodes(node::DESIGN_EPOCH)?;
        epochs.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        let mut items: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut planned = 0usize;
        for ep in &epochs {
            let status = ep
                .properties
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("arrived");
            if status != "planned" {
                continue;
            }
            planned += 1;
            for e in self.incoming(&ep.node_id, Some(edge::SCHEDULED_FOR))? {
                items.insert(e.from_id.clone());
            }
        }
        let mut closed = 0usize;
        let mut worst: Option<ClosureHole> = None;
        for item in &items {
            let mut open_governor: Option<String> = None;
            let mut govs: Vec<String> = self
                .outgoing(item, Some(edge::GOVERNED_BY))?
                .into_iter()
                .map(|e| e.to_id)
                .filter(|id| !is_review(id))
                .collect();
            govs.sort();
            for d in govs {
                if let Some(dec) = self.get_node(node::DECISION, &d)?
                    && dec.properties.get("status").and_then(Value::as_str) == Some("proposed")
                {
                    open_governor = Some(d);
                    break;
                }
            }
            match open_governor {
                None => closed += 1,
                Some(d) => {
                    if worst.is_none() {
                        worst = Some(ClosureHole {
                            leg: "decisions".into(),
                            id: Some(d.clone()),
                            why: format!(
                                "'{item}' is scheduled and governed by '{d}', which is still proposed"
                            ),
                        });
                    }
                }
            }
        }
        let note = format!(
            "decisions: {} item(s) scheduled into {} planned increment(s)",
            items.len(),
            planned
        );
        Ok(leg("decisions", items.len(), closed, note, worst))
    }

    fn leg_provenance(&self) -> Result<ClosureLeg, DynoError> {
        let s = self.quantity_provenance_sweep()?;
        let worst = s.unsourced.first().map(|(id, what)| ClosureHole {
            leg: "provenance".into(),
            id: Some(id.clone()),
            why: format!("{what} has no source"),
        });
        let closed = s.quantities.saturating_sub(s.unsourced.len());
        let note = format!(
            "quantities: {} stated ({} limit(s) and their numbered contributions)",
            s.quantities, s.limits
        );
        Ok(leg("provenance", s.quantities, closed, note, worst))
    }
}

fn leg(
    name: &str,
    swept: usize,
    closed: usize,
    swept_note: String,
    worst: Option<ClosureHole>,
) -> ClosureLeg {
    let share = if swept == 0 {
        None
    } else {
        Some(closed as f64 * 100.0 / swept as f64)
    };
    ClosureLeg {
        leg: name.to_string(),
        counted: false,
        swept,
        closed,
        share,
        swept_note,
        worst,
        closes: None,
    }
}
