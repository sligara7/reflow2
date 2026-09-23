//! Which settled decisions does bad news now stand behind? — `cap:revise-trigger`.
//!
//! Step 4 of the ordering Anthony approved on 2026-09-22, on his rulings in
//! `dec:step-4-is-built-on-real-runs-with-no-threshold-and-unscored-good-news`.
//! His words: *"Negative feedback should cause 'doubt' over a design decision
//! in the graph and cause the system to determine root-cause (if appropriate)
//! or at least see if other choices would potentially do better."*
//!
//! # What it does
//!
//! Takes every piece of BAD NEWS the design holds, walks each one back along
//! the golden thread to the nodes it lands on, and names the ACCEPTED
//! Decisions those nodes are GOVERNED_BY — "this road may be worth
//! re-opening", with the evidence and the path that led there. Four kinds of
//! bad news — three of the four `cap:revise-trigger` names (a failing check,
//! drift, a field finding), with a failing check split by whether a RUN said so
//! or a person did, and structural defects left out for the reason below:
//!
//! - `failed_run` — a real run fed back through the reconcile reported the
//!   check failed (its stamp), the entry point of the founding chain;
//! - `recorded_failing` — a check somebody RECORDED as failing with no run
//!   stamp behind it: bad news on the owner's word, kept apart from a run;
//! - `drift` — an unresolved DriftEvent: the built thing and the design
//!   disagree and nobody has said which is right;
//! - `defect_finding` — an open TemporalFact of type `defect`: somebody met a
//!   defect and nothing has closed it.
//!
//! # What it does NOT do, each on his word or the design's
//!
//! - **No threshold.** Every decision with any bad news behind it is listed;
//!   the owner judges.
//! - **No ranking.** Decisions come back in id order. How much evidence stands
//!   behind one is shown, never used to sort — a ranking here is the score
//!   `dec:alternatives-unranked-forkable` refused.
//! - **Nothing re-opened.** Re-opening mints a superseding Decision
//!   (`dec:reopen-supersedes`), and that is the owner's act.
//! - **Good news is counted, never stored.** `held_up` is how many checks under
//!   the decision a real run has compared and found passing — computed on every
//!   read, never written back as a confidence.
//! - **Readiness is not bad news here, yet** — a thing can be correctly designed
//!   and simply immature.
//! - **Structural defects are not traced.** They are a finding about the
//!   design's SHAPE, reported by `detect_defects`, and on a mature design they
//!   number in the hundreds; tracing them would bury the runtime evidence this
//!   exists to surface. `cap:revise-trigger` lists them as a trigger, and that
//!   inclusion waits until this has run on real history.

use std::collections::{BTreeMap, BTreeSet};

use crate::foundation::core::{DynoError, Value};
use crate::foundation::store::StoredNode;
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// How one piece of bad news reaches the decision it is listed under. What
/// the news SAYS lives once in [`DoubtReport::evidence`], keyed by id.
///
/// ⚠️ SAID ONCE, NOT PER DECISION, AND MEASURED: the first real run, on
/// reflow2's own design (2026-09-22), paired 231 open defect findings with
/// 126 decisions 1,167 times, and repeating each finding's text under every
/// decision it reached made a 2.1 MB reply.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoubtEvidence {
    /// The node carrying the news — its kind and words are in the catalogue.
    pub evidence_id: String,
    /// The walk from the evidence to the governed node, evidence first.
    pub via: Vec<String>,
}

/// One piece of bad news, stated once.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BadNews {
    /// `failed_run` / `recorded_failing` / `drift` / `defect_finding`.
    pub kind: &'static str,
    /// Its name or summary.
    pub says: String,
}

/// A settled decision with bad news behind it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DecisionInDoubt {
    pub decision_id: String,
    pub name: String,
    pub evidence: Vec<DoubtEvidence>,
    /// Checks under this decision a real run has compared and found PASSING.
    /// Computed on read; never stored and never used to order anything.
    pub held_up: usize,
}

/// The doubt report. See the module docs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoubtReport {
    /// In decision-id order — deliberately not by how much evidence.
    pub decisions: Vec<DecisionInDoubt>,
    /// Every piece of bad news that reached a decision, stated once.
    pub evidence: BTreeMap<String, BadNews>,
    /// How many pieces of each kind of bad news were found at all.
    pub evidence_found: BTreeMap<&'static str, usize>,
    /// Bad news that reached no accepted decision. Not quieter for it: it is
    /// news about a part of the design nobody has recorded a choice for.
    pub evidence_reaching_no_decision: Vec<String>,
    pub note: String,
}

fn prop<'a>(n: &'a StoredNode, k: &str) -> Option<&'a str> {
    n.properties
        .get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// How far up the thread a piece of news is walked. Two hops reaches
/// check → capability → requirement, and artifact → capability → requirement,
/// which is where the choices that shaped a thing are recorded.
const HOPS: usize = 2;

/// The edges a piece of bad news climbs, from the thing that broke toward the
/// intent it serves — the reverse of how the design was built down.
const UPWARD: &[&str] = &[
    edge::VERIFIES,
    edge::REALIZES,
    edge::SATISFIES,
    edge::ALLOCATED_TO,
    edge::DEPENDS_ON,
];

impl DesignGraph {
    /// Compute the [`DoubtReport`].
    pub fn decisions_in_doubt(&self) -> Result<DoubtReport, DynoError> {
        let mut found: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut news: Vec<(&'static str, String, String, String)> = Vec::new(); // kind, id, says, anchor

        for v in self.scan_nodes(node::VERIFICATION)? {
            let says = prop(&v, "name").unwrap_or(&v.node_id).to_string();
            if prop(&v, "last_reconciled_outcome") == Some("failed") {
                news.push(("failed_run", v.node_id.clone(), says, v.node_id.clone()));
            } else if prop(&v, "status") == Some("failing") {
                news.push((
                    "recorded_failing",
                    v.node_id.clone(),
                    says,
                    v.node_id.clone(),
                ));
            }
        }
        for d in self.scan_nodes(node::DRIFT_EVENT)? {
            if d.properties.get("resolved").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let says = prop(&d, "summary")
                .or_else(|| prop(&d, "name"))
                .unwrap_or(&d.node_id)
                .to_string();
            for e in self.outgoing(&d.node_id, Some(edge::DEPENDS_ON))? {
                news.push(("drift", d.node_id.clone(), says.clone(), e.to_id));
            }
        }
        for f in self.scan_nodes(node::TEMPORAL_FACT)? {
            if prop(&f, "fact_type") != Some("defect") || prop(&f, "valid_to").is_some() {
                continue;
            }
            if !self
                .incoming(&f.node_id, Some(edge::INVALIDATES))?
                .is_empty()
            {
                continue;
            }
            let Some(subject) = prop(&f, "subject_id") else {
                continue;
            };
            let says = prop(&f, "name")
                .or_else(|| prop(&f, "statement"))
                .unwrap_or(&f.node_id)
                .to_string();
            news.push((
                "defect_finding",
                f.node_id.clone(),
                says,
                subject.to_string(),
            ));
        }

        let mut by_decision: BTreeMap<String, Vec<DoubtEvidence>> = BTreeMap::new();
        let mut catalogue: BTreeMap<String, BadNews> = BTreeMap::new();
        // One piece of news can have several anchors (a drift on two files):
        // it reached a decision if ANY anchor did.
        let mut unreached: BTreeSet<String> = BTreeSet::new();
        let mut reached: BTreeSet<String> = BTreeSet::new();
        let mut counted: BTreeSet<(&'static str, String)> = BTreeSet::new();
        for (kind, id, says, anchor) in news {
            if counted.insert((kind, id.clone())) {
                *found.entry(kind).or_default() += 1;
            }
            let mut reached_any = false;
            for (governed, path) in self.walk_up(&anchor)? {
                for g in self.outgoing(&governed, Some(edge::GOVERNED_BY))? {
                    let Some(dec) = self.get_node(node::DECISION, &g.to_id)? else {
                        continue;
                    };
                    if prop(&dec, "status") != Some("accepted") {
                        continue;
                    }
                    reached_any = true;
                    let mut via = vec![id.clone()];
                    if anchor != id {
                        via.push(anchor.clone());
                    }
                    via.extend(path.iter().skip(1).cloned());
                    let list = by_decision.entry(dec.node_id.clone()).or_default();
                    if !list.iter().any(|e| e.evidence_id == id) {
                        list.push(DoubtEvidence {
                            evidence_id: id.clone(),
                            via,
                        });
                    }
                    catalogue.entry(id.clone()).or_insert_with(|| BadNews {
                        kind,
                        says: says.clone(),
                    });
                }
            }
            if reached_any {
                reached.insert(id);
            } else {
                unreached.insert(id);
            }
        }

        let mut decisions = Vec::new();
        for (decision_id, evidence) in by_decision {
            let dec = self
                .get_node(node::DECISION, &decision_id)?
                .expect("read a moment ago");
            decisions.push(DecisionInDoubt {
                name: prop(&dec, "name").unwrap_or(&decision_id).to_string(),
                held_up: self.held_up_under(&decision_id)?,
                decision_id,
                evidence,
            });
        }

        let note = if found.is_empty() {
            "No bad news found: no check has a failed run or a recorded failure, no drift is \
             unresolved, and no defect finding is open. That is what the design HOLDS — if no \
             run has ever been fed back, it is also what an untested design looks like; \
             loop_status's loop_closure says which."
                .to_string()
        } else {
            "Every settled decision with any bad news behind it, in id order — not ranked, and \
             no threshold (Anthony, 2026-09-22). Nothing here is re-opened: re-opening mints a \
             superseding Decision, and that is the owner's act. `held_up` counts checks under \
             the decision a real run found passing, computed now and never stored."
                .to_string()
        };

        Ok(DoubtReport {
            decisions,
            evidence: catalogue,
            evidence_found: found,
            evidence_reaching_no_decision: unreached.difference(&reached).cloned().collect(),
            note,
        })
    }

    /// Every node reachable from `start` along [`UPWARD`] edges within
    /// [`HOPS`], each with the path that reached it (start first).
    fn walk_up(&self, start: &str) -> Result<Vec<(String, Vec<String>)>, DynoError> {
        let mut out = vec![(start.to_string(), vec![start.to_string()])];
        let mut seen: BTreeSet<String> = BTreeSet::from([start.to_string()]);
        let mut frontier = vec![(start.to_string(), vec![start.to_string()])];
        for _ in 0..HOPS {
            let mut next = Vec::new();
            for (at, path) in &frontier {
                for t in UPWARD {
                    for e in self.outgoing(at, Some(t))? {
                        if seen.insert(e.to_id.clone()) {
                            let mut p = path.clone();
                            p.push(e.to_id.clone());
                            out.push((e.to_id.clone(), p.clone()));
                            next.push((e.to_id, p));
                        }
                    }
                }
            }
            frontier = next;
        }
        Ok(out)
    }

    /// Checks under a decision that a real run compared and found passing:
    /// checks VERIFYING any node GOVERNED_BY it, or governed checks themselves.
    fn held_up_under(&self, decision_id: &str) -> Result<usize, DynoError> {
        let mut checks: BTreeSet<String> = BTreeSet::new();
        for g in self.incoming(decision_id, Some(edge::GOVERNED_BY))? {
            if self.get_node(node::VERIFICATION, &g.from_id)?.is_some() {
                checks.insert(g.from_id.clone());
            }
            for v in self.incoming(&g.from_id, Some(edge::VERIFIES))? {
                checks.insert(v.from_id);
            }
        }
        let mut n = 0;
        for c in checks {
            if let Some(v) = self.get_node(node::VERIFICATION, &c)?
                && prop(&v, "last_reconciled_outcome") == Some("passed")
            {
                n += 1;
            }
        }
        Ok(n)
    }
}
