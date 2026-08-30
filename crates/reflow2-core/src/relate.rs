//! Relating a node to what is already in the design — and recording the
//! judgement when nothing is honestly related.
//!
//! `dec:idea-what-notices-an-idea-that-connects-to-nothing`, accepted
//! 2026-08-21. The measurement behind it: 145 brainstormed ideas joined by 12
//! edges, 111 of them reaching no other idea within two hops, while the
//! relation vocabulary that would have joined them had existed the whole time
//! and was used 81 times elsewhere in the same graph. **The missing leg was
//! never the tool. It was that nothing asked, and nothing noticed.**
//!
//! # Why both outcomes go through one door
//!
//! An act of relating has two honest endings: an edge, or the finding that
//! there is nothing to draw. Only the first leaves a trace today, which makes
//! the second indistinguishable from never having looked — and any detector
//! built over "has no relation" would therefore keep reporting people who did
//! the work. So [`DesignGraph::review_relations`] accepts either ending and
//! refuses to accept neither. It is the shape the dedup guard already uses:
//! that guard does not infer that you considered the near-duplicate, it makes
//! you say so, and then the saying is a fact rather than a guess.
//!
//! # Why a note and not a flag
//!
//! `no_relation_note` carries what was searched and what was nearest, because
//! that is the part a later reader can overturn. A bare boolean would record
//! that a review happened and nothing about whether it was any good — and the
//! reader who disagrees would have nothing to disagree *with*.
//!
//! # What this deliberately does not do
//!
//! It does not suggest relations. Offering candidates is the dedup guard's job
//! and the skill's; choosing among them is a judgement, and a machine that
//! proposed edges here would be manufacturing exactly the false neighbours the
//! brainstorm skill forbids. A fabricated relation is worse than a missing one,
//! because anything that searches by neighbourhood repeats it forever.

use crate::foundation::core::DynoError;
use serde::Serialize;

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// The relations a review may draw — the inference layer's "why" vocabulary,
/// which is wildcard-ended and so joins any two nodes.
///
/// Listed here rather than accepting any edge type on purpose. `CONTAINS` and
/// `SATISFIES` are structure, not commentary; letting a review draw one would
/// put load-bearing design edges behind a tool whose whole premise is that the
/// author is still thinking.
pub const REVIEW_RELATIONS: &[&str] = &[
    edge::CONTRADICTS,
    edge::EVOLVES_INTO,
    edge::DEPENDS_ON,
    edge::CAUSES,
    edge::TRIGGERS,
    edge::BLOCKS,
    edge::DUPLICATES,
    edge::ANTICIPATES,
    edge::OBSOLETES,
    edge::RISKS,
    edge::MITIGATES,
    edge::MASKS,
    edge::VIOLATES,
];

/// One relation a review draws, with the reason it was drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationLink {
    /// One of [`REVIEW_RELATIONS`].
    pub relation: String,
    pub other_type: String,
    pub other_id: String,
    /// WHY this relation is true, in a sentence. Required: a relation with no
    /// evidence is an assertion the next reader can neither check nor overturn.
    pub evidence: String,
    /// Draw the edge FROM the other node instead of from the node under review.
    ///
    /// Every one of these relations reads as a sentence — *from RELATION to* —
    /// and backwards the same edge asserts something false. "The older idea
    /// EVOLVES_INTO this one" and "this one EVOLVES_INTO the older" are not the
    /// same claim, and nothing downstream can tell that one of them is wrong.
    pub incoming: bool,
}

/// What a review did.
/// How much of the idea population has been through the linking judgement —
/// and, deliberately, how much of that this can and cannot tell you.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LinkingReport {
    /// Decisions declaring `kind: exploratory` — the population the discipline
    /// applies to, and the denominator of every figure below.
    pub ideas: usize,
    /// Of those, how many carry at least one review relation. NOT how many were
    /// reviewed — see `not_observed_about`.
    pub linked: usize,
    /// Of those, how many carry `no_relation_note`: somebody looked and
    /// recorded that nothing was honestly related. **A full answer, not a
    /// weaker one**, and the half that separates a judged idea from an
    /// unopened one.
    pub noted: usize,
    /// Neither. For these, "nobody looked" and "looked and found nothing"
    /// cannot be told apart.
    pub silent: usize,
    /// Which ones, so the number is never a bare ratio.
    pub silent_ids: Vec<String>,
    /// Decisions declaring `kind: exploratory`, `choice`, or nothing at all.
    /// The third is a real state, not a gap: absent means nobody said.
    pub exploratory: usize,
    pub choice: usize,
    pub kind_unstated: usize,
    /// What this report cannot see. Carried WITH the numbers rather than left
    /// to the reader, because a caveat that lives only in the tool that
    /// computes it gets stripped by whatever quotes the figure
    /// (`cap:a-derived-number-carries-what-it-was-computed-over`).
    pub not_observed_about: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReviewOutcome {
    pub node_type: String,
    pub node_id: String,
    /// Relations drawn by this call, as `"CONTRADICTS -> dec:x"`.
    pub drawn: Vec<String>,
    /// Relations this call asked for that were already present. Reported rather
    /// than silently counted as new, so re-running a review does not read as
    /// having found something.
    pub already_present: Vec<String>,
    /// The note stored, when the review drew nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// How the node stands after the review.
    pub state: ReviewState,
}

/// The three states a detector has to tell apart. Only the third is a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Carries at least one relation to another node.
    Linked,
    /// Reviewed, and deliberately not linked — the judgement is on the record.
    ReviewedUnlinked,
}

impl DesignGraph {
    /// Record the relations of one node: draw the honest ones, or say in
    /// writing that there were none.
    ///
    /// # Errors
    ///
    /// Refuses, rather than accepting a half-record:
    ///
    /// - **neither links nor a note** — the whole point is that "I looked and
    ///   found nothing" is an answer, and an answer has to be given;
    /// - an unknown relation, naming the ones that exist;
    /// - a link with no evidence;
    /// - a link to a node that does not exist, or to itself;
    /// - a node that does not exist.
    pub fn review_relations(
        &mut self,
        node_type: &str,
        node_id: &str,
        links: &[RelationLink],
        note: Option<&str>,
    ) -> Result<ReviewOutcome, DynoError> {
        let Some(existing) = self.get_node(node_type, node_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node_type.into(),
                node_id: node_id.into(),
            });
        };

        let note = note.map(str::trim).filter(|n| !n.is_empty());
        if links.is_empty() && note.is_none() {
            return Err(DynoError::Validation {
                node_type: node_type.into(),
                property: "no_relation_note".into(),
                message: "a review needs an outcome: either the relations you drew, or a note \
                          saying what you searched and why nothing was honest. Recording neither \
                          leaves this node indistinguishable from one nobody has opened, which \
                          is the state this call exists to rule out."
                    .into(),
            });
        }

        // Validate EVERY link before writing any of them. A review that drew
        // two edges and then refused the third would leave the design holding
        // half a judgement, and the caller with no way to know which half.
        for l in links {
            if !REVIEW_RELATIONS.contains(&l.relation.as_str()) {
                return Err(DynoError::Validation {
                    node_type: node_type.into(),
                    property: "relation".into(),
                    message: format!(
                        "'{}' is not a relation a review can draw (one of {}). Structural edges \
                         like CONTAINS and SATISFIES are design, not commentary, and are written \
                         with their own tools.",
                        l.relation,
                        REVIEW_RELATIONS.join(", ")
                    ),
                });
            }
            if l.evidence.trim().is_empty() {
                return Err(DynoError::Validation {
                    node_type: node_type.into(),
                    property: "evidence".into(),
                    message: format!(
                        "the {} to '{}' needs its reason in one sentence — a relation with no \
                         evidence is an assertion the next reader can neither check nor overturn.",
                        l.relation, l.other_id
                    ),
                });
            }
            if l.other_id == node_id {
                return Err(DynoError::Validation {
                    node_type: node_type.into(),
                    property: "other_id".into(),
                    message: format!("'{node_id}' cannot be related to itself."),
                });
            }
            if self.get_node(&l.other_type, &l.other_id)?.is_none() {
                return Err(DynoError::NodeNotFound {
                    node_type: l.other_type.clone(),
                    node_id: l.other_id.clone(),
                });
            }
        }

        let mut drawn = Vec::new();
        let mut already_present = Vec::new();
        for l in links {
            let (from_t, from_i, to_t, to_i): (&str, &str, &str, &str) = if l.incoming {
                (&l.other_type, &l.other_id, node_type, node_id)
            } else {
                (node_type, node_id, &l.other_type, &l.other_id)
            };
            let label = if l.incoming {
                format!("{} <- {}", l.relation, l.other_id)
            } else {
                format!("{} -> {}", l.relation, l.other_id)
            };
            let present = self
                .outgoing(from_i, Some(&l.relation))?
                .into_iter()
                .any(|e| e.to_id == to_i);
            if present {
                already_present.push(label);
                continue;
            }
            self.create_edge(
                &l.relation,
                from_t,
                from_i,
                to_t,
                to_i,
                Props::new().set("evidence", l.evidence.trim()),
            )?;
            drawn.push(label);
        }

        // The note is stored only when the review drew nothing. Where an edge
        // exists the edge IS the record, and a note beside it would be a second
        // account of the same act for a reader to reconcile.
        let store_note = drawn.is_empty() && already_present.is_empty();
        if store_note && let Some(n) = note {
            let mut props = Props::new().set("no_relation_note", n);
            for (k, v) in &existing.properties {
                if k != "no_relation_note" {
                    props = props.set(k, v.clone());
                }
            }
            self.create_node(node_type, node_id, props)?;
        }

        Ok(ReviewOutcome {
            node_type: node_type.to_string(),
            node_id: node_id.to_string(),
            state: if store_note {
                ReviewState::ReviewedUnlinked
            } else {
                ReviewState::Linked
            },
            note: if store_note {
                note.map(str::to_string)
            } else {
                None
            },
            drawn,
            already_present,
        })
    }

    /// Does this node carry any relation from the review vocabulary, in either
    /// direction?
    ///
    /// Direction-blind on purpose: an idea somebody else's idea CONTRADICTS is
    /// just as connected as one that contradicts something. Asking only about
    /// outgoing edges would report the second idea in every pair as an orphan.
    pub(crate) fn has_review_relation(&self, node_id: &str) -> Result<bool, DynoError> {
        for r in REVIEW_RELATIONS {
            if !self.outgoing(node_id, Some(r))?.is_empty()
                || !self.incoming(node_id, Some(r))?.is_empty()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Proposed Decisions that carry no relation and no note — the ideas
    /// **nobody has opened**, as distinct from the ones somebody judged and
    /// found genuinely new.
    ///
    /// Excludes decision POINTS (two or more registered alternatives): those
    /// are a fork being weighed rather than a thought nobody has connected, and
    /// `undecided_decision_point` already asks about them. Excludes parked
    /// nodes, which are the recorded answer "this correctly attaches to
    /// nothing".
    /// **Is the linking discipline being followed?** — the question the design
    /// could not answer about itself.
    ///
    /// `unreviewed_ideas` counts the ideas nobody has opened, and that count is
    /// consumed by a detector the owner has ACCEPTED (an unlinked idea is not a
    /// defect — `decision:ack:baa3d7b03a08dedb`, 2026-08-23). So the number was
    /// computed, wired to something deliberately silent, and reachable from
    /// nowhere else. Measured 2026-08-30, that silence hid a real shape: 140 of
    /// 207 ideas carried a relation and `no_relation_note` had been used TWICE,
    /// so for the 67 unlinked ideas "nobody looked" and "looked and found
    /// nothing" were indistinguishable — the exact distinction the note exists
    /// to preserve.
    ///
    /// THIS REPORTS, IT NEVER PRESSES. It raises no gap and changes no
    /// detector: the owner's judgement that an unlinked idea is not a defect
    /// stands untouched. What it removes is the inability to SEE.
    ///
    /// ⚠️ AND IT SAYS WHAT IT CANNOT KNOW, in `not_observed_about`, because the
    /// honest limit is severe: an edge can be drawn by a dozen tools, and
    /// nothing records WHICH drew it. So `linked` counts ideas that have a
    /// relation, never ideas somebody reviewed — the two are not the same, and
    /// the report must not let a reader mistake one for the other.
    pub fn linking_report(&self) -> Result<LinkingReport, DynoError> {
        let mut r = LinkingReport::default();
        for dec in self.scan_nodes(node::DECISION)? {
            let kind = dec
                .properties
                .get("kind")
                .and_then(crate::foundation::core::Value::as_str)
                .map(str::to_string);
            let noted = dec.properties.contains_key("no_relation_note");
            let linked = self.has_review_relation(&dec.node_id)?;
            match kind.as_deref() {
                Some("exploratory") => r.exploratory += 1,
                Some("choice") => r.choice += 1,
                _ => r.kind_unstated += 1,
            }
            // Only ideas carry the discipline. A Decision nobody has classified
            // is counted apart rather than assumed to be one — the same reason
            // the property has no default.
            if kind.as_deref() != Some("exploratory") {
                continue;
            }
            r.ideas += 1;
            if linked {
                r.linked += 1;
            }
            if noted {
                r.noted += 1;
            }
            if !linked && !noted {
                r.silent += 1;
                r.silent_ids.push(dec.node_id.clone());
            }
        }
        r.silent_ids.sort();
        r.not_observed_about = vec![
            "WHETHER THE DISCIPLINE WAS FOLLOWED. An edge can be drawn by any of a dozen tools              and nothing records which drew it, so `linked` counts ideas that HAVE a relation,              never ideas somebody reviewed. A relation drawn in passing by `governed_by` or              `satisfies` is indistinguishable here from one somebody judged."
                .into(),
            "THE IDEAS CARRYING NO `kind`. They are counted in `kind_unstated` and excluded from              every other figure, because their only evidence of being ideas is an id prefix that              was deliberately retired — reading it here would launder it back in."
                .into(),
            "WHETHER A SILENT IDEA IS A PROBLEM. It is not reported as a defect and raises no              gap: an unlinked idea legitimately governs nothing yet, which is the owner's              standing judgement."
                .into(),
        ];
        Ok(r)
    }

    pub fn unreviewed_ideas(&self) -> Result<Vec<String>, DynoError> {
        let mut out = Vec::new();
        for dec in self.scan_nodes(node::DECISION)? {
            if dec
                .properties
                .get("status")
                .and_then(crate::foundation::core::Value::as_str)
                != Some("proposed")
            {
                continue;
            }
            if dec.properties.contains_key("no_relation_note") {
                continue;
            }
            if self.alternatives_for(&dec.node_id)?.len() >= 2 {
                continue;
            }
            if self.is_parked(&dec.node_id)? {
                continue;
            }
            if self.has_review_relation(&dec.node_id)? {
                continue;
            }
            out.push(dec.node_id.clone());
        }
        out.sort();
        Ok(out)
    }
}
