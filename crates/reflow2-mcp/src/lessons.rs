//! A lesson is served at the step it concerns.
//!
//! `req:a-lesson-is-served-at-the-step-it-concerns` (Anthony, 2026-09-12).
//! The measured premise: a lesson written in prose does not change the next
//! command. The same shell trap was recorded three times in three places and
//! repeated anyway (`fact:the-pipe-trap-recurred-twice-on-2026-09-12-a-month-
//! after-it-was-written-down`); what stopped each recurrence was something
//! arriving AT THE STEP — a gate, a guard, a refusal that carried the lesson.
//!
//! So a design's own lessons — `DesignRule`s and dated `TemporalFact`s — may
//! name the step they concern in a `steps` list: a served skill name or a
//! served tool name. Two deliveries read it:
//!
//! - **`get_skill`** carries, beside the skill's body, every lesson this design
//!   holds for that skill — in the reply the agent reads immediately before
//!   doing the work, the same slot the reader lens already rides.
//! - **The tool list** this server serves appends, to each tool's description,
//!   the lessons this design holds for that tool — read in the moment before
//!   the call. An empty design serves the surface unchanged, which is what the
//!   toolsnap goldens (taken on an empty design) continue to pin.
//!
//! # The bar a step name has to clear
//!
//! A `steps` entry is validated at write against what THIS server serves. A
//! typo would otherwise file a lesson where nothing will ever deliver it — the
//! exact quiet failure this exists to end — and the refusal names the nearest
//! served names so the caller can fix it in one motion.
//!
//! # What this deliberately is not
//!
//! Not a node type (the notepad already exists: rules and facts hung on what
//! they concern). Not a lessons page (measured insufficient). Not enrichment
//! of every tool REPLY — refusals already carry reflow2's own lessons, and a
//! project's lessons on a refusal is a later, measured step. And it reaches
//! only steps that go through reflow2: a shell command or a CI job has no step
//! here to hang on.

use rmcp::ErrorData as McpError;
use rmcp::model::Tool;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

/// One lesson as it is delivered: enough to act on, with the id to read more.
#[derive(Debug, Clone, Serialize)]
pub struct Lesson {
    pub id: String,
    pub node_type: String,
    pub name: String,
    /// The statement, cut to a paragraph: delivery is a nudge, `get_node` is
    /// the whole thing.
    pub statement: String,
    /// For a fact, the node it is hung on; a rule stands on its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hung_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
}

/// How much of a statement rides along. A lesson is one paragraph at the
/// step; the node holds the rest.
const STATEMENT_CHARS: usize = 600;

/// Every step name this server can deliver a lesson at: skill names and tool
/// names, as served.
pub fn served_steps(tools: &[Tool]) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = crate::skills::SKILLS
        .iter()
        .map(|k| k.name.to_string())
        .collect();
    s.extend(tools.iter().map(|t| t.name.to_string()));
    s
}

/// Refuse a step that names nothing served, naming the nearest served names.
/// Returns the steps trimmed and de-duplicated, in the order given.
pub fn validate_steps(
    steps: &[String],
    served: &BTreeSet<String>,
) -> Result<Vec<String>, McpError> {
    let mut out: Vec<String> = Vec::new();
    for raw in steps {
        let step = raw.trim();
        if step.is_empty() {
            continue;
        }
        if !served.contains(step) {
            let nearest = nearest(step, served);
            return Err(McpError::invalid_params(
                format!(
                    "`steps` names {step:?}, which this server serves as neither a skill nor a \
                     tool — so nothing would ever deliver the lesson there. Nearest served \
                     names: {}. A step is a skill name (`list_skills`) or a tool name, exactly \
                     as served.",
                    if nearest.is_empty() {
                        "none close".to_string()
                    } else {
                        nearest.join(", ")
                    }
                ),
                None,
            ));
        }
        if !out.iter().any(|s| s == step) {
            out.push(step.to_string());
        }
    }
    Ok(out)
}

/// The closest served names to a typo: shared prefix first, then containment,
/// then a cheap edit distance. At most five.
fn nearest(step: &str, served: &BTreeSet<String>) -> Vec<String> {
    let lower = step.to_ascii_lowercase();
    let mut scored: Vec<(usize, &String)> = served
        .iter()
        .map(|name| {
            let n = name.to_ascii_lowercase();
            let score = if n.contains(&lower) || lower.contains(&n) {
                0
            } else {
                let prefix = n
                    .chars()
                    .zip(lower.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                if prefix >= 3 {
                    1
                } else {
                    2 + edit_distance(&n, &lower)
                }
            };
            (score, name)
        })
        .filter(|(score, _)| *score < 6)
        .collect();
    scored.sort();
    scored.into_iter().take(5).map(|(_, n)| n.clone()).collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Every lesson in the design, keyed by the step it names. One scan of the
/// two lesson types, so a 181-tool listing costs one read rather than 181.
pub fn lessons_by_step(g: &reflow2_core::graph::DesignGraph) -> BTreeMap<String, Vec<Lesson>> {
    use reflow2_core::foundation::core::Value;
    let mut by_step: BTreeMap<String, Vec<Lesson>> = BTreeMap::new();
    for node_type in [
        reflow2_core::nodes::node::TEMPORAL_FACT,
        reflow2_core::nodes::node::DESIGN_RULE,
    ] {
        let Ok(nodes) = g.scan_nodes(node_type) else {
            continue;
        };
        for n in nodes {
            let Some(Value::List(steps)) = n.properties.get("steps") else {
                continue;
            };
            // A finding with a `valid_to` has stopped being true — a closed
            // defect, a measurement since superseded — and serving it at the
            // step would teach a lesson the design has already retracted.
            // Found the day this shipped: the plan_epoch defect was fixed and
            // closed, and its fact went on being delivered at `plan_epoch`.
            if n.properties
                .get("valid_to")
                .and_then(Value::as_str)
                .is_some_and(|v| !v.trim().is_empty())
            {
                continue;
            }
            let text = |k: &str| {
                n.properties
                    .get(k)
                    .and_then(Value::as_str)
                    .map(str::to_string)
            };
            let statement = text("statement").unwrap_or_default();
            let lesson = Lesson {
                id: n.node_id.clone(),
                node_type: node_type.to_string(),
                name: text("name").unwrap_or_else(|| n.node_id.clone()),
                statement: cut(&statement, STATEMENT_CHARS),
                hung_on: text("subject_id"),
                valid_from: text("valid_from"),
            };
            for s in steps.iter().filter_map(Value::as_str) {
                by_step
                    .entry(s.to_string())
                    .or_default()
                    .push(lesson.clone());
            }
        }
    }
    // Newest first, so the lesson learned last week outranks the one from June.
    for lessons in by_step.values_mut() {
        lessons.sort_by(|a, b| {
            b.valid_from
                .cmp(&a.valid_from)
                .then_with(|| a.id.cmp(&b.id))
        });
    }
    by_step
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}… (get_node for the rest)")
}

/// The block appended to a tool's description when the design holds lessons
/// for it. Written for the agent about to make the call.
pub fn description_block(tool: &str, lessons: &[Lesson]) -> String {
    let mut out = format!(
        "\n\n⭐ LESSONS THIS DESIGN HOLDS FOR `{tool}` ({}) — recorded by earlier sessions on this \
         project, served here because a lesson filed elsewhere was measured not to change the \
         next call:",
        lessons.len()
    );
    for l in lessons {
        out.push_str(&format!("\n  · [{}] {} — {}", l.id, l.name, l.statement));
    }
    out
}

/// Append each tool's lessons to its description. Tools with none are
/// returned untouched, so an empty design serves the surface unchanged.
pub fn enrich_tools(tools: Vec<Tool>, by_step: &BTreeMap<String, Vec<Lesson>>) -> Vec<Tool> {
    if by_step.is_empty() {
        return tools;
    }
    tools
        .into_iter()
        .map(|mut t| {
            if let Some(lessons) = by_step.get(t.name.as_ref()) {
                let base = t.description.as_deref().unwrap_or_default();
                let block = description_block(t.name.as_ref(), lessons);
                t.description = Some(Cow::Owned(format!("{base}{block}")));
            }
            t
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served() -> BTreeSet<String> {
        ["where-am-i", "export_graph", "plan_epoch", "get_skill"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn a_served_name_passes_and_is_deduplicated() {
        let out = validate_steps(
            &[
                "export_graph".into(),
                " export_graph ".into(),
                "where-am-i".into(),
            ],
            &served(),
        )
        .unwrap();
        assert_eq!(
            out,
            vec!["export_graph".to_string(), "where-am-i".to_string()]
        );
    }

    /// A typo is refused, and the refusal names what would have worked.
    #[test]
    fn an_unserved_name_is_refused_with_the_nearest() {
        let err = validate_steps(&["export-graph".into()], &served()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("export-graph"), "{msg}");
        assert!(
            msg.contains("export_graph"),
            "the nearest served name is offered: {msg}"
        );
    }

    /// Only the tools the design holds lessons for are touched.
    #[test]
    fn enrichment_touches_only_the_named_tools() {
        let mk = |n: &str| {
            Tool::new(
                n.to_string(),
                "base".to_string(),
                rmcp::model::JsonObject::new(),
            )
        };
        let tools = vec![mk("export_graph"), mk("loop_status")];
        let mut by_step = BTreeMap::new();
        by_step.insert(
            "export_graph".to_string(),
            vec![Lesson {
                id: "fact:x".into(),
                node_type: "TemporalFact".into(),
                name: "Format first".into(),
                statement: "run cargo fmt before the export".into(),
                hung_on: None,
                valid_from: Some("2026-09-12".into()),
            }],
        );
        let out = enrich_tools(tools, &by_step);
        let d = |i: usize| {
            out[i]
                .description
                .as_deref()
                .unwrap_or_default()
                .to_string()
        };
        assert!(d(0).contains("LESSONS THIS DESIGN HOLDS") && d(0).contains("fact:x"));
        assert_eq!(d(1), "base");
        let untouched = enrich_tools(vec![mk("export_graph")], &BTreeMap::new());
        assert_eq!(untouched[0].description.as_deref(), Some("base"));
    }
}
