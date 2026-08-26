//! Which nodes might this one be related to — offered, never drawn.
//!
//! The missing leg of half-idea linking. `relate.rs` records the judgement and
//! `detect.rs` counts what carries none (`unreviewed_ideas`), but nothing ever
//! answered the question a person working that backlog actually has: **which of
//! these 110 belong together?**
//!
//! # Why this is a separate module, and read-only
//!
//! `relate.rs` states in its own docs that it deliberately does not suggest,
//! because *"a machine that proposed edges here would be manufacturing exactly
//! the false neighbours the brainstorm skill forbids"*. That boundary is kept:
//! this module **never writes**. It ranks and explains; drawing the edge stays
//! `review_relations`, which stays a human judgement. The same doc assigns
//! offering candidates to *"the dedup guard's job and the skill's"* — this is
//! the computation that finally lets the skill do it.
//!
//! # Why not BM25
//!
//! `search_design` is the obvious tool and it is gated behind the `fulltext`
//! feature, failing loud without it. A suggester built on it would be absent
//! from every index-less build and untestable by the core gate — which is the
//! gate that runs in seconds and where 1,452 of the tests live. So the signals
//! here need no index and no database:
//!
//! - **Shared neighbours.** Two nodes that both relate to a third are related
//!   in the graph's own terms, whatever their words. This is the signal a text
//!   search cannot see.
//! - **Distinctive shared terms.** Weighted by rarity ACROSS THE POOL, so
//!   "reflow2", "design" and "decision" — true of everything — count for
//!   nothing, and "allocation", "rocksdb", "hydrate" count for a lot.
//!
//! # What it refuses to do
//!
//! It does not rank a node against one it is ALREADY related to (those are
//! returned separately, so a caller can see they were excluded rather than
//! missed), and an empty answer says WHY it is empty rather than reading as
//! "nothing is related".

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;

/// One node this one might relate to, with the walk that produced it.
#[derive(Debug, Clone, Serialize)]
pub struct NeighbourCandidate {
    pub node_id: String,
    pub node_type: String,
    /// The candidate's `name`, so a caller can put it to a person without a
    /// second read.
    pub name: String,
    /// Rough, comparable WITHIN one answer and meaningless across answers —
    /// the same honesty `what_next` states about its own score.
    pub score: f64,
    /// Why this surfaced, in words. A candidate whose reason cannot be stated
    /// is not offered.
    pub because: Vec<String>,
}

/// What might relate to one node — and what was deliberately left out.
#[derive(Debug, Clone, Serialize)]
pub struct NeighbourhoodReport {
    pub node_id: String,
    pub candidates: Vec<NeighbourCandidate>,
    /// Ids this node ALREADY relates to, excluded from the ranking. Returned
    /// rather than dropped: a caller must be able to tell "not offered because
    /// already linked" from "not offered because nothing matched".
    pub already_related: Vec<String>,
    /// How many nodes were ranked. Zero is a different fact from "nothing
    /// scored", and both are different from "this node has no text".
    pub pool_examined: usize,
    /// Present exactly when `candidates` is empty, saying which of the several
    /// possible reasons it is. An empty list on its own would read as
    /// "considered and found nothing", which is only one of them.
    pub empty_because: Option<String>,
}

/// Words too common in any design to carry a signal, plus ordinary English
/// stopwords. Deliberately short: the rarity weighting below does most of this
/// work, and a long hand-tuned list is a place for someone's taste to hide.
const STOPWORDS: &[&str] = &[
    "the", "and", "that", "this", "with", "for", "from", "not", "but", "are", "was", "were", "has",
    "have", "had", "which", "what", "when", "where", "would", "could", "should", "than", "then",
    "there", "their", "them", "they", "its", "it's", "into", "onto", "any", "all", "one", "two",
    "because", "does", "did", "doing", "been", "being", "more", "most", "some", "such", "only",
    "own", "same", "how", "why", "who", "whom", "each", "other", "another", "about", "design",
    "reflow2", "decision", "node", "nodes", "graph", "user", "session", "recorded",
];

/// Tokens worth comparing: lowercase alphanumeric runs of four or more
/// characters that are not stopwords.
fn terms(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(str::to_lowercase)
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

impl DesignGraph {
    /// The text this node offers for comparison: its name, plus whichever long
    /// prose field its type carries.
    fn comparable_text(&self, props: &HashMap<String, Value>) -> String {
        let mut s = String::new();
        for key in ["name", "decision", "statement", "description", "rationale"] {
            if let Some(v) = props.get(key).and_then(Value::as_str) {
                s.push_str(v);
                s.push(' ');
            }
        }
        s
    }

    /// Every node this one already has an inference-layer relation to, either
    /// direction — the set the ranking must not re-propose.
    fn already_related_to(&self, node_id: &str) -> Result<BTreeSet<String>, DynoError> {
        let mut out = BTreeSet::new();
        for r in crate::relate::REVIEW_RELATIONS {
            for e in self.outgoing(node_id, Some(r))? {
                out.insert(e.to_id);
            }
            for e in self.incoming(node_id, Some(r))? {
                out.insert(e.from_id);
            }
        }
        Ok(out)
    }

    /// Everything this node touches by ANY edge — used for the shared-neighbour
    /// signal, which is why it is deliberately wider than `already_related_to`.
    fn neighbours_of(&self, node_id: &str) -> Result<BTreeSet<String>, DynoError> {
        let mut out = BTreeSet::new();
        for e in self.outgoing(node_id, None)? {
            out.insert(e.to_id);
        }
        for e in self.incoming(node_id, None)? {
            out.insert(e.from_id);
        }
        Ok(out)
    }

    /// Which nodes of `pool_type` this one might relate to, ranked, with the
    /// reason for each. **Reads only** — see the module docs for why drawing
    /// the edge stays a human act.
    pub fn relation_candidates(
        &self,
        node_type: &str,
        node_id: &str,
        pool_type: Option<&str>,
        limit: usize,
    ) -> Result<NeighbourhoodReport, DynoError> {
        let Some(subject) = self.get_node(node_type, node_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node_type.into(),
                node_id: node_id.into(),
            });
        };

        let already = self.already_related_to(node_id)?;
        let subject_terms = terms(&self.comparable_text(&subject.properties));
        let subject_neighbours = self.neighbours_of(node_id)?;

        let pool = self.scan_nodes(pool_type.unwrap_or(node_type))?;

        // Document frequency over the POOL, so "rare" means rare here rather
        // than rare in English. A term true of every idea says nothing about
        // which two ideas belong together.
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut pool_terms: Vec<(String, String, String, BTreeSet<String>)> = Vec::new();
        for n in &pool {
            if n.node_id == node_id {
                continue;
            }
            let t = terms(&self.comparable_text(&n.properties));
            for term in &t {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
            let name = n
                .properties
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&n.node_id)
                .to_string();
            pool_terms.push((n.node_id.clone(), n.node_type.clone(), name, t));
        }

        let pool_examined = pool_terms.len();
        let mut scored: Vec<(bool, NeighbourCandidate)> = Vec::new();

        for (cand_id, cand_type, cand_name, cand_terms) in &pool_terms {
            if already.contains(cand_id) {
                continue;
            }
            let mut because = Vec::new();
            let mut score = 0.0_f64;

            // ── structural: a third node both of them touch ──
            let cand_neighbours = self.neighbours_of(cand_id)?;
            let shared: Vec<&String> = subject_neighbours.intersection(&cand_neighbours).collect();
            let has_shared_neighbour = !shared.is_empty();
            if has_shared_neighbour {
                score += shared.len() as f64;
                let sample: Vec<&str> = shared.iter().take(3).map(|s| s.as_str()).collect();
                because.push(format!(
                    "both relate to {}{}",
                    sample.join(", "),
                    if shared.len() > 3 {
                        format!(" and {} more", shared.len() - 3)
                    } else {
                        String::new()
                    }
                ));
            }

            // ── textual: shared terms, weighted by how rare they are here ──
            let mut hits: Vec<(&String, f64)> = subject_terms
                .intersection(cand_terms)
                .filter_map(|t| {
                    let d = *df.get(t).unwrap_or(&0);
                    if d == 0 {
                        return None;
                    }
                    // A term in more than a third of the pool is background —
                    // but ONLY once the pool is big enough for a proportion to
                    // mean anything. Applied unconditionally it rejected every
                    // term on a two-node pool, where any shared word is 50% by
                    // arithmetic and not by being common. Found by the test.
                    if pool_examined >= 6 && d * 3 > pool_examined {
                        return None;
                    }
                    // Ordinary smoothed IDF. Smoothed so a pool of one can
                    // still say something, and shaped so a term true of the
                    // whole pool tends to nothing on its own.
                    Some((t, ((pool_examined + 1) as f64 / d as f64).ln()))
                })
                .collect();
            if !hits.is_empty() {
                hits.sort_by(|a, b| b.1.total_cmp(&a.1));
                score += hits.iter().map(|(_, w)| w).sum::<f64>();
                let sample: Vec<&str> = hits.iter().take(4).map(|(t, _)| t.as_str()).collect();
                because.push(format!(
                    "{} distinctive term(s) in common: {}",
                    hits.len(),
                    sample.join(", ")
                ));
            }

            // A candidate whose reason cannot be stated is not offered.
            if because.is_empty() {
                continue;
            }
            scored.push((
                has_shared_neighbour,
                NeighbourCandidate {
                    node_id: cand_id.clone(),
                    node_type: cand_type.clone(),
                    name: cand_name.clone(),
                    score,
                    because,
                },
            ));
        }

        // ⭐ AN ASSERTED CONNECTION OUTRANKS A VOCABULARY COINCIDENCE, AND IT
        // DOES SO CATEGORICALLY RATHER THAN BY A MAGIC MULTIPLIER. A shared
        // neighbour is something a person WROTE INTO the graph; shared words
        // are incidental and can be produced by two authors reaching for the
        // same metaphor. This project already ranks asserted above inferred
        // elsewhere — HEAL merges only on a human-drawn DUPLICATES edge, never
        // on the duplicate heuristic — and a weight chosen to make one example
        // come out right would be taste presented as arithmetic. So: every
        // candidate sharing a neighbour ranks above every candidate that does
        // not, and `score` orders WITHIN each group.
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.score.total_cmp(&a.1.score))
                .then(a.1.node_id.cmp(&b.1.node_id))
        });
        let mut scored: Vec<NeighbourCandidate> = scored.into_iter().map(|(_, c)| c).collect();
        scored.truncate(limit.max(1));

        // AN EMPTY LIST MUST SAY WHICH EMPTY IT IS. "Nothing scored" and "this
        // node has no text to compare" and "there was nothing to compare it
        // against" are three different facts, and only one of them means the
        // node is genuinely unrelated to everything.
        let empty_because = if !scored.is_empty() {
            None
        } else if pool_examined == 0 {
            Some(format!(
                "nothing to compare against: the pool holds no other {} node(s)",
                pool_type.unwrap_or(node_type)
            ))
        } else if subject_terms.is_empty() {
            Some(
                "this node carries no comparable text (no name, decision, statement, description \
                 or rationale), so the textual signal had nothing to work with"
                    .into(),
            )
        } else if !already.is_empty() {
            Some(format!(
                "no unrelated candidate scored; note this node is ALREADY related to {} node(s), \
                 which were excluded from the ranking",
                already.len()
            ))
        } else {
            Some(format!(
                "{pool_examined} node(s) were ranked and none shared a neighbour or a distinctive \
                 term. That is evidence this node may be genuinely new — which is a real answer, \
                 and `review_relations` takes it as a note"
            ))
        };

        Ok(NeighbourhoodReport {
            node_id: node_id.to_string(),
            candidates: scored,
            already_related: already.into_iter().collect(),
            pool_examined,
            empty_because,
        })
    }
}
