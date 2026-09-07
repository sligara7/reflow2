//! What the design holds about ONE subject, read-only — the `/topic` view.
//!
//! `dec:idea-a-topic-view-shows-what-the-design-holds-about-one-subject-read-only`
//! (ruled 2026-09-06): not a brainstorm, not a link-artifacts effort, just
//! "show me something about X". `search_design` returns a ranked id list, one
//! dimension; `get_node` reads one node; `where-am-i` reads the WHOLE design.
//! None answers "what do we know about X" as a digest. This computes it
//! SERVER-SIDE, deterministically, in one call, because the SE doctrine on
//! record says a view is a pure projection of the graph and a renderer's
//! fill-ins are defects — so the projection lives here, not in an agent's prose.
//!
//! ⭐ THE NOT-FOUND LINE IS MANDATORY AND LOAD-BEARING. Search misses on an
//! agent's phrasing were measured the week this was designed; a digest built on
//! a miss is a confident wrong answer about what the design holds, worse than a
//! raw hit list. Every report says what it searched, what matched, which
//! populated node types matched NOTHING, and whether the hit list was cut at
//! its limit — so silence is never read as absence.
//!
//! Bounded like `detect_gaps`: `budget_chars` trims detail before it trims
//! hits, and the reply always says which tier it landed in.

use std::collections::BTreeMap;

use crate::dates::ClaimAge;
use crate::detect::{ReplyBudget, ReplyDetail};
use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// Hits per topic when the caller does not say. Twenty: enough to cover a
/// subject that has a requirement, a few capabilities, their checks and
/// changes, without reading the whole design back.
pub const DEFAULT_TOPIC_LIMIT: usize = 20;
/// Connections listed per hit before the rest are counted in `connections_more`.
const CONNECTIONS_CAP: usize = 8;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TopicReport {
    pub query: String,
    /// Hits found (before any budget trimming); `groups` lists them by type.
    pub count: usize,
    /// The search limit used; `count == limit` means there may be more.
    pub limit: usize,
    /// Hits per node type, in the order the groups appear.
    pub by_type: BTreeMap<String, usize>,
    /// MANDATORY. What was searched, what matched, which populated types
    /// matched nothing, and whether the list was cut at its limit.
    pub not_found: String,
    /// Search-index rows whose node is gone (the index has drifted).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<String>,
    pub budget: ReplyBudget,
    pub groups: Vec<TopicGroup>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TopicGroup {
    pub node_type: String,
    pub count: usize,
    pub hits: Vec<TopicHit>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TopicHit {
    pub node_id: String,
    pub node_type: String,
    pub name: String,
    pub score: f32,
    /// The node's own `status` (or `enforced` on a rule, `fact_type` on a
    /// fact), when it carries one. Absent means the node carries none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// True when an accepted Decision has withdrawn this node — the stored
    /// status still says what was BUILT, so this is the only thing that says
    /// the thing is gone.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub discontinued: bool,
    #[serde(flatten)]
    pub age: ClaimAge,
    /// Edges by type and direction, most numerous first, capped; withheld in
    /// the titles-only tier.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<Connection>,
    /// How many (edge_type, direction) rows the cap left out.
    #[serde(skip_serializing_if = "is_zero")]
    pub connections_more: usize,
    /// The latest DATED change that touched this node, or measurement about
    /// it — whichever is newer. Absent means nothing dated is on the record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<Latest>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Connection {
    pub edge_type: String,
    /// `out` (this node is the from end) or `in`.
    pub direction: String,
    pub count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Latest {
    /// `change` (a ChangeEvent that CHANGED this node) or `measurement` (a
    /// TemporalFact whose subject is this node).
    pub kind: String,
    pub id: String,
    pub at: String,
    pub name: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn prop<'a>(props: &'a std::collections::HashMap<String, Value>, k: &str) -> Option<&'a str> {
    props.get(k).and_then(Value::as_str)
}

impl DesignGraph {
    /// See the module doc. `limit` is the search limit (default
    /// [`DEFAULT_TOPIC_LIMIT`]); `budget_chars` bounds the reply.
    pub fn topic_report(
        &self,
        query: &str,
        limit: Option<usize>,
        budget_chars: usize,
    ) -> Result<TopicReport, DynoError> {
        let limit = limit.unwrap_or(DEFAULT_TOPIC_LIMIT).max(1);
        let found = self.search_design(query, None, limit)?;
        let index = self.node_type_index()?;
        let today = crate::dates::today_utc();

        // Which types this design is populated with, so "matched nothing" can
        // name the types that were there to be matched.
        let mut populated: BTreeMap<String, usize> = BTreeMap::new();
        for ty in index.values() {
            if matches!(
                ty.as_str(),
                node::SNAPSHOT | node::DESIGN_EPOCH | node::QUESTION | node::FRAGMENT
            ) {
                continue;
            }
            *populated.entry(ty.clone()).or_insert(0) += 1;
        }

        // Facts by subject, read once: a fact is the measurement side of
        // `latest`, and scanning all facts per hit would be hits × facts.
        let mut facts_by_subject: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
        for f in self.scan_live_nodes(node::TEMPORAL_FACT)? {
            if let (Some(subject), Some(at)) = (
                prop(&f.properties, "subject_id"),
                prop(&f.properties, "valid_from"),
            ) {
                facts_by_subject
                    .entry(subject.to_string())
                    .or_default()
                    .push((
                        f.node_id.clone(),
                        at.to_string(),
                        prop(&f.properties, "name").unwrap_or("").to_string(),
                    ));
            }
        }

        let mut hits: Vec<TopicHit> = Vec::with_capacity(found.hits.len());
        for h in &found.hits {
            let Some(n) = self.get_node(&h.node_type, &h.node_id)? else {
                continue;
            };
            let status = prop(&n.properties, "status")
                .map(str::to_string)
                .or_else(|| {
                    n.properties
                        .get("enforced")
                        .and_then(Value::as_bool)
                        .map(|b| {
                            if b {
                                "enforced".into()
                            } else {
                                "advisory".into()
                            }
                        })
                })
                .or_else(|| prop(&n.properties, "fact_type").map(str::to_string));

            // Connections: every design edge, counted by (type, direction).
            let mut conn: BTreeMap<(String, String), usize> = BTreeMap::new();
            for e in self.outgoing(&h.node_id, None)? {
                if e.edge_type == edge::HAS_SNAPSHOT {
                    continue;
                }
                *conn.entry((e.edge_type.clone(), "out".into())).or_insert(0) += 1;
            }
            for e in self.incoming(&h.node_id, None)? {
                if e.edge_type == edge::HAS_SNAPSHOT {
                    continue;
                }
                *conn.entry((e.edge_type.clone(), "in".into())).or_insert(0) += 1;
            }
            let mut conn: Vec<Connection> = conn
                .into_iter()
                .map(|((edge_type, direction), count)| Connection {
                    edge_type,
                    direction,
                    count,
                })
                .collect();
            conn.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then(a.edge_type.cmp(&b.edge_type))
                    .then(a.direction.cmp(&b.direction))
            });
            let connections_more = conn.len().saturating_sub(CONNECTIONS_CAP);
            conn.truncate(CONNECTIONS_CAP);

            // Latest dated change or measurement. Undated ones cannot be
            // ordered and are not offered — the same rule as
            // `defect_overtaken_by_change`.
            let mut latest: Option<Latest> = None;
            let mut consider = |cand: Latest| {
                let newer = match &latest {
                    Some(cur) => {
                        crate::dates::parse_day(&cand.at) > crate::dates::parse_day(&cur.at)
                    }
                    None => true,
                };
                if newer {
                    latest = Some(cand);
                }
            };
            for e in self.incoming(&h.node_id, Some(edge::CHANGED))? {
                if let Some(ev) = self.get_node(node::CHANGE_EVENT, &e.from_id)?
                    && let Some(at) = prop(&ev.properties, "detected_at")
                    && crate::dates::parse_day(at).is_some()
                {
                    consider(Latest {
                        kind: "change".into(),
                        id: ev.node_id.clone(),
                        at: at.to_string(),
                        name: prop(&ev.properties, "name").unwrap_or("").to_string(),
                    });
                }
            }
            if let Some(facts) = facts_by_subject.get(&h.node_id) {
                for (id, at, name) in facts {
                    if crate::dates::parse_day(at).is_some() {
                        consider(Latest {
                            kind: "measurement".into(),
                            id: id.clone(),
                            at: at.clone(),
                            name: name.clone(),
                        });
                    }
                }
            }

            hits.push(TopicHit {
                node_id: h.node_id.clone(),
                node_type: h.node_type.clone(),
                name: h.name.clone(),
                score: h.score,
                status,
                discontinued: self.is_discontinued(&h.node_id)?,
                // Through the graph-aware helper, not the property-only one:
                // a topic view that showed a refuted finding as current would
                // repeat the failure on the second read surface.
                age: self.claim_age_of(&h.node_id, &n.properties, &today)?,
                connections: conn,
                connections_more,
                latest,
            });
        }

        let count = hits.len();
        let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
        for h in &hits {
            *by_type.entry(h.node_type.clone()).or_insert(0) += 1;
        }

        // Groups ordered by their best hit; hits within a group by score.
        let mut groups: Vec<TopicGroup> = Vec::new();
        for h in hits {
            match groups.iter_mut().find(|g| g.node_type == h.node_type) {
                Some(g) => g.hits.push(h),
                None => groups.push(TopicGroup {
                    node_type: h.node_type.clone(),
                    count: 0,
                    hits: vec![h],
                }),
            }
        }
        for g in &mut groups {
            g.hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            g.count = g.hits.len();
        }
        groups.sort_by(|a, b| {
            let ba = a.hits.first().map(|h| h.score).unwrap_or(0.0);
            let bb = b.hits.first().map(|h| h.score).unwrap_or(0.0);
            bb.partial_cmp(&ba).unwrap_or(std::cmp::Ordering::Equal)
        });

        let not_found = not_found_line(query, count, limit, &populated, &by_type, &found.stale);

        let (budget, groups) = fit(groups, count, budget_chars);
        Ok(TopicReport {
            query: query.to_string(),
            count,
            limit,
            by_type,
            not_found,
            stale: found.stale,
            budget,
            groups,
        })
    }
}

/// The sentence that keeps a miss from reading as absence.
fn not_found_line(
    query: &str,
    count: usize,
    limit: usize,
    populated: &BTreeMap<String, usize>,
    by_type: &BTreeMap<String, usize>,
    stale: &[String],
) -> String {
    let total_nodes: usize = populated.values().sum();
    let unmatched: Vec<String> = populated
        .iter()
        .filter(|(ty, _)| !by_type.contains_key(*ty))
        .map(|(ty, n)| format!("{ty} ({n})"))
        .collect();
    let mut s = if count == 0 {
        format!(
            "NOTHING MATCHED “{query}” across {total_nodes} node(s) in {} type(s). The design \
             may hold nothing about this subject, or it may hold it in other words — search is \
             keyword-based, so try the domain's own terms before concluding it is absent.",
            populated.len()
        )
    } else {
        format!(
            "Matched {count} node(s) across {} of the design's {} populated type(s), out of \
             {total_nodes} node(s) searched.",
            by_type.len(),
            populated.len()
        )
    };
    // A type absent from a list that was CUT at its limit is not a type that
    // matched nothing — it may simply rank below the cut. Found by reading
    // reflow2's own design through this tool on the day it was written: 436
    // TemporalFacts reported as "NOT matched" for a query many of them
    // plainly match, because the list held twenty hits.
    if count > 0 && !unmatched.is_empty() {
        if count >= limit {
            s.push_str(&format!(
                " Present but not among the top {limit}: {}.",
                unmatched.join(", ")
            ));
        } else {
            s.push_str(&format!(
                " NOT matched, though present: {}.",
                unmatched.join(", ")
            ));
        }
    }
    if count >= limit {
        s.push_str(&format!(
            " The list was cut at its limit of {limit}: there may be more — raise `limit` or \
             narrow the query."
        ));
    }
    if !stale.is_empty() {
        s.push_str(&format!(
            " {} index row(s) named a node that no longer exists; the search index has drifted.",
            stale.len()
        ));
    }
    s
}

fn json_len<T: serde::Serialize + ?Sized>(v: &T) -> usize {
    serde_json::to_string(v)
        .map(|s| s.len())
        .unwrap_or(usize::MAX)
}

/// Fit the groups to the budget: full detail, then titles only (connections
/// and latest withheld), then the best-scored prefix of hits per group.
fn fit(
    mut groups: Vec<TopicGroup>,
    count: usize,
    budget_chars: usize,
) -> (ReplyBudget, Vec<TopicGroup>) {
    let chars = json_len(&groups);
    if chars <= budget_chars {
        return (
            ReplyBudget {
                detail: ReplyDetail::Full,
                chars,
                budget_chars,
                listed: count,
                of: count,
                note: None,
            },
            groups,
        );
    }
    for g in &mut groups {
        for h in &mut g.hits {
            h.connections.clear();
            h.connections_more = 0;
            h.latest = None;
        }
    }
    let mut chars = json_len(&groups);
    let mut listed = count;
    // Drop the lowest-scored hit of the largest group until it fits, or one
    // hit per group remains. `count` and `by_type` above are never trimmed.
    while chars > budget_chars {
        let Some(g) = groups
            .iter_mut()
            .filter(|g| g.hits.len() > 1)
            .max_by_key(|g| g.hits.len())
        else {
            break;
        };
        g.hits.pop();
        listed -= 1;
        chars = json_len(&groups);
    }
    let note = if listed < count {
        format!(
            "Titles only, and {} of {count} hit(s) withheld to fit {budget_chars} characters — \
             `count` and `by_type` still cover every hit. Raise `budget_chars`, or narrow the \
             query.",
            count - listed
        )
    } else {
        format!(
            "Titles only: connections and the latest dated change were withheld to fit \
             {budget_chars} characters. Raise `budget_chars` to read them, or `get_node` one hit."
        )
    };
    (
        ReplyBudget {
            detail: ReplyDetail::TitlesOnly,
            chars,
            budget_chars,
            listed,
            of: count,
            note: Some(note),
        },
        groups,
    )
}
