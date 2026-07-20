//! Functional flows — the write and read side of `Flow` (BL-37).
//!
//! `Flow` had been fully specified in `functional.yaml` since the schema was
//! written — `flow_type: process/control_flow/decision_flow`, `entry_point`,
//! `exit_point`, with `PART_OF_FLOW (Capability → Flow)` carrying `step_order`
//! — and nothing could create one: no constructor in core, no MCP tool. The
//! eleventh instance of the recurring lesson, found by modelling reflow2's own
//! coherence loop in reflow2 (`tools/model_the_loop.py`): **the one type meant
//! for "an ordered process linking Capabilities end to end" was unreachable**,
//! for the exact model that is its use case.
//!
//! The read side embodies a decision (2026-07-19): **a process's cycles are
//! reported, never judged.** In a product a cycle is a defect —
//! `circular_dependency` walks `DEPENDS_ON` and contracts, and stays that way.
//! In a process the loops are the design: *test fails → fix → test* is a cycle
//! on purpose, and the feedback edges are the most important thing the model
//! says. So [`FlowReport`] computes the cycles among a flow's members over
//! `TRIGGERS` and states them as facts, the same way the confirmation ledger
//! reports claim histories without ruling on them (`dec:report-dont-judge`).
//! Nothing here feeds HEAL.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::{DynoError, Value};
use dynograph_graph::{GraphBuilder, find_cycle, strongly_connected_components};
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

impl DesignGraph {
    /// P1 · Function — an ordered process linking Capabilities end to end.
    /// `name` is required; `flow_type` (default `process`), `entry_point` and
    /// `exit_point` name where it begins and ends (a Capability name or id).
    pub fn add_flow(
        &mut self,
        id: &str,
        name: &str,
        description: Option<&str>,
        flow_type: Option<&str>,
        entry_point: Option<&str>,
        exit_point: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::FLOW,
            id,
            Props::new()
                .set("name", name)
                .set_opt("description", description)
                .set_opt("flow_type", flow_type)
                .set_opt("entry_point", entry_point)
                .set_opt("exit_point", exit_point),
        )
    }

    /// `Capability PART_OF_FLOW Flow` — a capability is a step of the process.
    /// `step_order` is its position; steps without one sort after those with,
    /// and the report says so rather than inventing an order.
    pub fn part_of_flow(
        &mut self,
        capability_id: &str,
        flow_id: &str,
        step_order: Option<i64>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::PART_OF_FLOW,
            node::CAPABILITY,
            capability_id,
            node::FLOW,
            flow_id,
            Props::new().set_opt("step_order", step_order),
        )
    }
}

/// One step of a flow, in reported order.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowStep {
    pub capability_id: String,
    pub name: String,
    /// `PART_OF_FLOW.step_order` — `None` means nobody stated a position.
    pub step_order: Option<i64>,
}

/// A `TRIGGERS` edge between two members of the flow.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowTransition {
    pub from_id: String,
    pub to_id: String,
    /// `TRIGGERS.role` — what this transition *means* ("feeds", "forces
    /// resync", …). `None` means the model never said, which the report's
    /// confessions call out rather than leaving to be misread as "forward".
    pub role: Option<String>,
}

/// One loop in a process: every step caught in it, and one walk through it.
///
/// The two are reported separately because they are different claims, and
/// conflating them misleads. A strongly-connected cluster says *these steps
/// can all reach each other*; a representative path is one closed walk, and a
/// cluster of N steps often has a shorter cycle inside it.
///
/// The storyflow trial made the cost concrete (F7): the six-process model's
/// cluster held four steps, and the returned path omitted `p-prompt` — the
/// hand-off to the human, and the entire reason the process is a loop rather
/// than a line. The behaviour was correct and the report was still wrong,
/// which is its own lesson: a true statement presented as the whole truth is
/// a way of lying quietly.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowCycle {
    /// Every step in the cluster — all mutually reachable, sorted.
    pub members: Vec<String>,
    /// One closed walk through the cluster, rotated to its smallest id so the
    /// output is stable. **May omit members**; `members` is the full answer.
    pub path: Vec<String>,
}

/// What a flow says, read back as facts — steps, transitions, cycles.
///
/// Cycles are the point, not a problem: a process model's loops are its
/// design, so they are listed here and deliberately **not** raised by
/// `detect_defects` (whose `circular_dependency` stays scoped to `DEPENDS_ON`
/// and contracts, where a cycle really is a defect).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowReport {
    pub flow_id: String,
    pub flow_name: String,
    pub flow_type: Option<String>,
    pub entry_point: Option<String>,
    pub exit_point: Option<String>,
    /// Members ordered by `step_order` (unstated positions sort last), then id.
    pub steps: Vec<FlowStep>,
    /// `TRIGGERS` edges where **both** endpoints are members, with their role.
    /// Edges leaving the flow are out of its scope, not dropped: this report
    /// is about the process, and its boundary is the membership.
    pub transitions: Vec<FlowTransition>,
    /// The process's loops, one entry per strongly-connected cluster of
    /// members. Reported as fact; never a defect.
    pub cycles: Vec<FlowCycle>,
    /// What the projection could not honestly render — an unmatched
    /// entry/exit point, steps with no stated order, transitions with no
    /// stated role. Per the projection doctrine, a confession is a gap in the
    /// model, not a fill-in by the renderer.
    pub confessions: Vec<String>,
}

impl DesignGraph {
    /// Read one flow back: its steps in stated order, the transitions among
    /// them with their roles, and the cycles — reported, never judged.
    pub fn flow_report(&self, flow_id: &str) -> Result<FlowReport, DynoError> {
        let Some(flow) = self.get_node(node::FLOW, flow_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::FLOW.to_string(),
                node_id: flow_id.to_string(),
            });
        };
        let prop = |key: &str| {
            flow.properties
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        };

        let mut confessions = Vec::new();

        // Members, via incoming PART_OF_FLOW.
        let mut steps = Vec::new();
        let mut member_names: BTreeMap<String, String> = BTreeMap::new();
        for e in self.incoming(flow_id, Some(edge::PART_OF_FLOW))? {
            let cap = self.get_node(node::CAPABILITY, &e.from_id)?;
            if cap.is_none() {
                // The storage layer accepts an edge to a node that does not
                // exist; rendering the id as if it were a step would be a
                // silent fill-in. Caught live: the smoke test attached a
                // member that was never created and the report said nothing.
                confessions.push(format!(
                    "member '{}' has a PART_OF_FLOW edge but no Capability node — the step \
                     is listed by id only, because there is nothing to read.",
                    e.from_id
                ));
            }
            let name = cap
                .as_ref()
                .and_then(|n| n.properties.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(&e.from_id)
                .to_string();
            let step_order = e.properties.get("step_order").and_then(Value::as_i64);
            member_names.insert(e.from_id.clone(), name.clone());
            steps.push(FlowStep {
                capability_id: e.from_id,
                name,
                step_order,
            });
        }
        steps.sort_by(|a, b| {
            match (a.step_order, b.step_order) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then(a.capability_id.cmp(&b.capability_id))
        });
        let unordered = steps.iter().filter(|s| s.step_order.is_none()).count();
        if unordered > 0 && steps.len() > 1 {
            confessions.push(format!(
                "{unordered} of {} step(s) carry no step_order — they are listed after the \
                 ordered ones, in id order, because the graph never said where they go.",
                steps.len()
            ));
        }

        // Entry/exit points name a member, or the report says they don't.
        let members: BTreeSet<&str> = member_names.keys().map(String::as_str).collect();
        for (which, value) in [
            ("entry_point", prop("entry_point")),
            ("exit_point", prop("exit_point")),
        ] {
            if let Some(v) = &value {
                let matches =
                    members.contains(v.as_str()) || member_names.values().any(|name| name == v);
                if !matches {
                    confessions.push(format!(
                        "{which} '{v}' matches no member of this flow — either the boundary \
                         capability was never attached with PART_OF_FLOW, or the point names \
                         something that does not exist."
                    ));
                }
            }
        }

        // Transitions: TRIGGERS with both endpoints inside the membership.
        let mut transitions = Vec::new();
        let mut unroled = 0usize;
        for id in member_names.keys() {
            for e in self.outgoing(id, Some(edge::TRIGGERS))? {
                if !members.contains(e.to_id.as_str()) {
                    continue;
                }
                let role = e
                    .properties
                    .get("role")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if role.is_none() {
                    unroled += 1;
                }
                transitions.push(FlowTransition {
                    from_id: e.from_id,
                    to_id: e.to_id,
                    role,
                });
            }
        }
        transitions.sort_by(|a, b| a.from_id.cmp(&b.from_id).then(a.to_id.cmp(&b.to_id)));
        if unroled > 0 {
            confessions.push(format!(
                "{unroled} of {} transition(s) carry no role — forward and feedback edges \
                 are indistinguishable there, which for a process is the load-bearing fact.",
                transitions.len()
            ));
        }

        // Cycles among members — one representative per strongly-connected
        // cluster, same shape as structure.rs's circular_dependencies, but as
        // a fact of the process rather than a defect of the product.
        let cycles = cycles_among(&transitions);

        Ok(FlowReport {
            flow_id: flow_id.to_string(),
            flow_name: prop("name").unwrap_or_else(|| flow_id.to_string()),
            flow_type: prop("flow_type"),
            entry_point: prop("entry_point"),
            exit_point: prop("exit_point"),
            steps,
            transitions,
            cycles,
            confessions,
        })
    }
}

/// One representative cycle per strongly-connected cluster of the transition
/// graph, plus degenerate self-triggers. Paths are rotated to start at their
/// smallest id so the output is byte-stable run to run.
fn cycles_among(transitions: &[FlowTransition]) -> Vec<FlowCycle> {
    let mut cycles = Vec::new();
    if transitions.is_empty() {
        return cycles;
    }

    let mut builder = GraphBuilder::new();
    for t in transitions {
        builder.add_node(&t.from_id);
        builder.add_node(&t.to_id);
    }
    for t in transitions {
        if t.from_id == t.to_id {
            cycles.push(FlowCycle {
                members: vec![t.from_id.clone()],
                path: vec![t.from_id.clone()],
            });
            continue;
        }
        // Endpoints were added above; a duplicate edge is fine for SCC purposes.
        let _ = builder.add_edge(&t.from_id, &t.to_id, 1.0);
    }
    let graph = builder.build(true);

    let mut meta = vec![String::new(); graph.node_count()];
    for t in transitions {
        for id in [&t.from_id, &t.to_id] {
            if let Some(idx) = graph.idx_of(id) {
                meta[idx] = id.clone();
            }
        }
    }

    for group in strongly_connected_components(&graph).groups() {
        if group.len() < 2 {
            continue;
        }
        let members: BTreeSet<usize> = group.iter().copied().collect();
        let mut sub = GraphBuilder::new();
        let mut sub_ids: Vec<&str> = group.iter().map(|&i| meta[i].as_str()).collect();
        sub_ids.sort_unstable();
        for id in &sub_ids {
            sub.add_node(id);
        }
        for &i in &group {
            for &(j, _) in graph.out_neighbors(i) {
                if members.contains(&j) {
                    let _ = sub.add_edge(&meta[i], &meta[j], 1.0);
                }
            }
        }
        let sub_graph = sub.build(true);
        if let Some(path) = find_cycle(&sub_graph) {
            let mut ids: Vec<String> = path
                .iter()
                .map(|&i| {
                    sub_ids
                        .iter()
                        .find(|id| sub_graph.idx_of(id) == Some(i))
                        .map(|id| id.to_string())
                        .unwrap_or_default()
                })
                .collect();
            if let Some(start) = ids
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.cmp(b.1))
                .map(|(i, _)| i)
            {
                ids.rotate_left(start);
            }
            cycles.push(FlowCycle {
                // The cluster's full membership — the honest answer to "what
                // is caught in this loop?". The walk below may be shorter.
                members: sub_ids.iter().map(|s| (*s).to_string()).collect(),
                path: ids,
            });
        }
    }
    cycles.sort_by(|a, b| a.members.cmp(&b.members));
    cycles
}
