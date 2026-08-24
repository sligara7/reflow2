//! Budgets — the classic SE quantity rollup (BL-11).
//!
//! Three independent tools in the original reflow reached for the same
//! missing thing: PROPAGATE walks *impact* but never accumulates a *quantity*
//! along the paths it walks. The general form is the systems-engineering
//! budget — latency, mass, power, cost — a limit stated once and spent by
//! parts, where the interesting questions are *does the total fit* and *does
//! the worst path fit*.
//!
//! The model uses vocabulary that was already waiting: a **`Constraint`** with
//! `category: budget` carries the `quantity` (a unit-bearing name), the
//! `limit` and the `direction`; each **`CONSTRAINS`** edge to a participating
//! node carries that node's `contribution` and its `basis`
//! (estimated/evidence/measured — the same rigor ladder as coupling weights).
//! `Constraint` itself had no write side until this landed — named in
//! `nodes.rs`, fully specified in the schema, unreachable: the fourteenth
//! recurring-lesson instance.
//!
//! The discipline is the graph-analysis one: **an unstated contribution is
//! never treated as zero.** A rollup over partial statements is a weaker
//! claim than one over complete statements, and the report says which it is —
//! the verdict is `incomplete`, not a number that quietly excludes the
//! unstated spenders.

use std::collections::BTreeMap;

use crate::foundation::core::{DynoError, Value};
use crate::foundation::store::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

impl DesignGraph {
    /// P0 · Intent — a limit the design must respect. For a numeric budget,
    /// set `quantity` (unit-bearing name, e.g. `mass_kg`), `limit` and
    /// `direction` (`maximum`, the default, or `minimum`).
    ///
    /// `category: "kpp"` marks inviolable intent, and then `limit` is the
    /// THRESHOLD (the minimum acceptable) while `objective` is what success
    /// looks like. `objective` is optional and never defaulted: many KPPs state
    /// only a threshold, and a number the graph supplied on its own is exactly
    /// what intent vocabulary must not do. It is meaningless on an ordinary
    /// Constraint, which has no notion of "better than required" — this is the
    /// typed write path for the KPP the user confirmed (`cap:kpp-proposal`),
    /// which before it existed could only be recorded through `create_node`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_constraint(
        &mut self,
        id: &str,
        name: &str,
        statement: &str,
        category: Option<&str>,
        quantity: Option<&str>,
        limit: Option<f64>,
        objective: Option<f64>,
        direction: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::CONSTRAINT,
            id,
            Props::new()
                .set("name", name)
                .set("statement", statement)
                .set_opt("category", category)
                .set_opt("quantity", quantity)
                .set_opt("limit", limit)
                .set_opt("objective", objective)
                .set_opt("direction", direction),
        )
    }

    /// `Constraint CONSTRAINS target` — the target spends `contribution` of
    /// the budget (in the Constraint's quantity unit). `from_type` is fixed:
    /// budgets hang off Constraints; `target_type` is free because anything
    /// can spend (a Component's mass, an Interface's latency, a Resource's
    /// cost). `basis` says how the number was obtained, and `note` says WHY
    /// this Constraint binds this target.
    ///
    /// ⭐ `note` WAS DECLARED ON THE EDGE AND UNREACHABLE FROM HERE until
    /// 2026-08-23 — the same trap [`governed_by`](Self::governed_by) documents
    /// at length, hit by dev_storyflow on both constructors in one session.
    /// `describe_schema` listed the field, this path rejected it with a
    /// shorter allowlist and no hint that a longer one existed, and the
    /// fallback was raw `create_edge` with the validation given up.
    #[allow(clippy::too_many_arguments)]
    pub fn constrains(
        &mut self,
        constraint_id: &str,
        target_type: &str,
        target_id: &str,
        contribution: Option<f64>,
        basis: Option<&str>,
        measured_at: Option<&str>,
        note: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        // Reject a non-finite contribution at the write seam (BL-58). A NaN
        // poisons the total (every comparison against it is false) and panics
        // the worst-path `max_by`; an infinity makes the verdict meaningless.
        // Fail loud here rather than storing a number arithmetic cannot use.
        if let Some(c) = contribution
            && !c.is_finite()
        {
            return Err(DynoError::Validation {
                node_type: node::CONSTRAINT.to_string(),
                property: "contribution".to_string(),
                message: format!("must be a finite number, got {c}"),
            });
        }
        self.create_edge(
            edge::CONSTRAINS,
            node::CONSTRAINT,
            constraint_id,
            target_type,
            target_id,
            Props::new()
                .set_opt("contribution", contribution)
                .set_opt("basis", basis)
                .set_opt("measured_at", measured_at)
                .set_opt("note", note),
        )
    }
}

/// One budget participant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BudgetContributor {
    pub node_id: String,
    /// `None` means the edge states no number — reported, never zeroed.
    pub contribution: Option<f64>,
    pub basis: Option<String>,
    /// When the contribution was observed, where the caller said.
    ///
    /// Only meaningful for a `basis` that decays. An estimate does not go
    /// stale; a MEASUREMENT is a statement about a system at a moment, and
    /// nothing could previously say how old one was.
    pub measured_at: Option<String>,
}

/// The verdict a rollup can honestly reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetVerdict {
    /// Every contribution stated, total on the right side of the limit.
    Within,
    /// Every contribution stated, total on the wrong side.
    Exceeded,
    /// One or more contributions unstated — no numeric verdict is honest.
    Incomplete,
    /// The Constraint carries no `limit`, so there is nothing to gate on.
    Ungated,
}

/// The budget rollup: total and worst path against the stated limit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BudgetReport {
    pub constraint_id: String,
    pub constraint_name: String,
    /// The unit-bearing quantity name, if stated.
    pub quantity: Option<String>,
    pub limit: Option<f64>,
    /// `maximum` (default) or `minimum`.
    pub direction: String,
    /// Every CONSTRAINS target, sorted by id.
    pub contributors: Vec<BudgetContributor>,
    /// Sum of the *stated* contributions.
    pub total: f64,
    /// Contributors whose edge states no contribution — the reason a verdict
    /// can be `incomplete`.
    pub unstated: Vec<String>,
    /// How many contributions rest on each basis — a total over `estimated`
    /// numbers is a weaker claim than one over `measured`, and the caller
    /// must see that.
    pub basis_coverage: BTreeMap<String, usize>,
    /// Contributors claiming `basis: measured` that do not say WHEN.
    ///
    /// ⚠️ REPORTED, NEVER GUESSED, and it is the same discipline `unstated`
    /// above applies to a missing number: `measured` is the strongest evidence
    /// claim this schema offers and the only one that decays, so an undated
    /// one is a claim whose age nobody can check. music_graph F22 measured the
    /// cost — a contribution of 6.9 fps against a 40 fps minimum whose true
    /// value was 55.06, right when taken and wrong for days.
    ///
    /// `ver:evidence-quality` already holds this line for Verification
    /// evidence, where an undated side is UNKNOWN and counted rather than
    /// silently passed. This is that rule reaching the one number a KPP
    /// verdict rests on.
    ///
    /// It does NOT change the verdict: an undated measurement is still a
    /// stated number, and refusing to total it would break every design that
    /// has not started dating them.
    pub undated_measurements: Vec<String>,
    pub verdict: BudgetVerdict,
    /// The heaviest chain of contributors along `DEPENDS_ON` (contracts
    /// collapsed: consumer depends on provider), when any such edges join
    /// them — the path-cumulative half, e.g. end-to-end latency. Empty when
    /// the contributors form no path, and the report says so in `path_note`.
    pub worst_path: Vec<String>,
    /// Sum of stated contributions along `worst_path`.
    pub worst_path_total: f64,
    /// Why the path half says what it says — always stated, never implied.
    pub path_note: String,
}

impl DesignGraph {
    /// Roll a budget up: the total of stated contributions, the worst
    /// dependency path among contributors, and an honest verdict.
    pub fn budget_report(&self, constraint_id: &str) -> Result<BudgetReport, DynoError> {
        let Some(c) = self.get_node(node::CONSTRAINT, constraint_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::CONSTRAINT.to_string(),
                node_id: constraint_id.to_string(),
            });
        };
        let sprop = |key: &str| {
            c.properties
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let limit = c.properties.get("limit").and_then(Value::as_f64);
        let direction = sprop("direction").unwrap_or_else(|| "maximum".to_string());

        let mut contributors = Vec::new();
        for e in self.outgoing(constraint_id, Some(edge::CONSTRAINS))? {
            contributors.push(BudgetContributor {
                node_id: e.to_id,
                contribution: e.properties.get("contribution").and_then(Value::as_f64),
                basis: e
                    .properties
                    .get("basis")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                measured_at: e
                    .properties
                    .get("measured_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        contributors.sort_by(|a, b| a.node_id.cmp(&b.node_id));

        let total: f64 = contributors.iter().filter_map(|c| c.contribution).sum();
        let unstated: Vec<String> = contributors
            .iter()
            .filter(|c| c.contribution.is_none())
            .map(|c| c.node_id.clone())
            .collect();
        // Only `measured` is asked for a date. An `estimated` figure does not
        // decay, so demanding one would raise a finding on correct work — the
        // signal-trained-to-be-ignored failure this project keeps naming.
        let undated_measurements: Vec<String> = contributors
            .iter()
            .filter(|c| c.contribution.is_some())
            .filter(|c| c.basis.as_deref() == Some("measured") && c.measured_at.is_none())
            .map(|c| c.node_id.clone())
            .collect();
        let mut basis_coverage: BTreeMap<String, usize> = BTreeMap::new();
        for c in contributors.iter().filter(|c| c.contribution.is_some()) {
            *basis_coverage
                .entry(c.basis.clone().unwrap_or_else(|| "estimated".to_string()))
                .or_insert(0) += 1;
        }

        // A provable verdict beats the epistemic caveat (BL-58). Unstated
        // spenders can only ADD to the total, so for a `maximum` a stated total
        // already over the limit is definitely Exceeded no matter what the
        // unknowns are; for a `minimum` a stated total already at/over the
        // limit is definitely Within. Only when the stated side leaves the
        // outcome genuinely open do the unstated contributions make it
        // Incomplete.
        let verdict = match limit {
            None => BudgetVerdict::Ungated,
            Some(l) if direction == "minimum" => {
                if total >= l {
                    BudgetVerdict::Within
                } else if !unstated.is_empty() {
                    BudgetVerdict::Incomplete
                } else {
                    BudgetVerdict::Exceeded // all stated and still short of the minimum
                }
            }
            Some(l) => {
                if total > l {
                    BudgetVerdict::Exceeded
                } else if !unstated.is_empty() {
                    BudgetVerdict::Incomplete
                } else {
                    BudgetVerdict::Within // all stated and within the maximum
                }
            }
        };

        let (worst_path, worst_path_total, path_note) =
            self.worst_path(&contributors, &direction)?;

        Ok(BudgetReport {
            constraint_id: constraint_id.to_string(),
            constraint_name: sprop("name").unwrap_or_else(|| constraint_id.to_string()),
            quantity: sprop("quantity"),
            limit,
            direction,
            contributors,
            total,
            unstated,
            basis_coverage,
            undated_measurements,
            verdict,
            worst_path,
            worst_path_total,
            path_note,
        })
    }

    /// The heaviest source→sink chain among contributors, along `DEPENDS_ON`
    /// (with contracts collapsed to consumer→provider), by stated
    /// contribution sum. DAG longest-path; a cycle among contributors makes a
    /// longest path undefined, which is reported rather than approximated.
    fn worst_path(
        &self,
        contributors: &[BudgetContributor],
        direction: &str,
    ) -> Result<(Vec<String>, f64, String), DynoError> {
        if direction == "minimum" {
            return Ok((
                Vec::new(),
                0.0,
                "path analysis applies to maximum budgets; a minimum budget gates the total only"
                    .into(),
            ));
        }
        let ids: Vec<&str> = contributors.iter().map(|c| c.node_id.as_str()).collect();
        let member = |id: &str| ids.contains(&id);
        let value: BTreeMap<&str, f64> = contributors
            .iter()
            .filter_map(|c| c.contribution.map(|v| (c.node_id.as_str(), v)))
            .collect();

        // Arcs among contributors: direct DEPENDS_ON, plus contract collapse.
        let mut arcs: Vec<(String, String)> = Vec::new();
        for id in &ids {
            for e in self.outgoing(id, Some(edge::DEPENDS_ON))? {
                if member(&e.to_id) && e.from_id != e.to_id {
                    arcs.push((e.from_id, e.to_id));
                }
            }
            for c in self.outgoing(id, Some(edge::CONSUMES))? {
                for p in self.incoming(&c.to_id, Some(edge::PROVIDES))? {
                    if member(&p.from_id) && p.from_id != *id {
                        arcs.push(((*id).to_string(), p.from_id));
                    }
                }
            }
        }
        arcs.sort();
        arcs.dedup();
        if arcs.is_empty() {
            return Ok((
                Vec::new(),
                0.0,
                "contributors form no dependency path; the total is the only rollup".into(),
            ));
        }

        // Kahn's toposort; leftovers mean a cycle.
        let mut indeg: BTreeMap<&str, usize> = ids.iter().map(|i| (*i, 0)).collect();
        for (_, b) in &arcs {
            *indeg.get_mut(b.as_str()).unwrap() += 1;
        }
        let mut queue: Vec<&str> = {
            let mut q: Vec<&str> = indeg
                .iter()
                .filter(|(_, d)| **d == 0)
                .map(|(i, _)| *i)
                .collect();
            q.sort();
            q
        };
        let mut order: Vec<&str> = Vec::new();
        while let Some(n) = queue.pop() {
            order.push(n);
            for (a, b) in &arcs {
                if a == n {
                    let d = indeg.get_mut(b.as_str()).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push(b.as_str());
                        queue.sort();
                    }
                }
            }
        }
        if order.len() < ids.len() {
            let stuck: Vec<&str> = ids
                .iter()
                .filter(|i| !order.contains(*i))
                .copied()
                .collect();
            return Ok((
                Vec::new(),
                0.0,
                format!(
                    "contributors { } depend on each other in a cycle — a longest path is undefined there, so no path claim is made",
                    stuck.join(", ")
                ),
            ));
        }

        // Longest path by contribution sum. An unstated contribution breaks
        // any path through it — reported, not zeroed.
        let mut best: BTreeMap<&str, (f64, Vec<String>)> = BTreeMap::new();
        for n in &order {
            let Some(v) = value.get(n) else { continue };
            let mut incoming_best: Option<(f64, Vec<String>)> = None;
            for (a, b) in &arcs {
                if b == n
                    && let Some((s, p)) = best.get(a.as_str())
                    && incoming_best.as_ref().is_none_or(|(bs, _)| s > bs)
                {
                    incoming_best = Some((*s, p.clone()));
                }
            }
            let (base, mut path) = incoming_best.unwrap_or((0.0, Vec::new()));
            path.push((*n).to_string());
            best.insert(n, (base + v, path));
        }
        let skipped = ids.iter().filter(|i| !value.contains_key(*i)).count();
        let note = if skipped > 0 {
            format!(
                "{skipped} contributor(s) state no contribution and cannot lie on a scored path — the worst path is over the stated ones only"
            )
        } else {
            "worst dependency path by stated contribution".to_string()
        };
        let (total, path) = best
            .into_values()
            .max_by(|(a, pa), (b, pb)| a.total_cmp(b).then(pb.cmp(pa)))
            .unwrap_or((0.0, Vec::new()));
        Ok((path, total, note))
    }
}
