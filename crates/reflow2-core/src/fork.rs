//! Return to a decision point — the other half of step 4, `cap:fork-alternatives`.
//!
//! Anthony, 2026-09-22: *"decision points … that you can go back to the decision
//! point or 'fork' and say, we need to go in a different direction."* His
//! rulings of 2026-09-23 finish it
//! (`dec:step-4-fork-point-finds-its-address-in-git-and-reopen-is-one-call`).
//!
//! Two operations, one read and one write.
//!
//! [`DesignGraph::fork_point`] answers *"where would I be going back to?"* for a
//! settled decision: the epoch it is pinned to and that epoch's export hash —
//! the coordinate `dec:fork-point-address` defines — what it governs, what has
//! changed under it since, the bad news now standing behind it
//! ([`crate::doubt`]), the roads registered beside it, and whether it has
//! already been re-opened. It writes nothing. The core does no file IO, so a
//! decision with no epoch comes back with `epoch: None`; the tool layer then
//! finds the earliest committed export containing it in git, as evidence.
//!
//! [`DesignGraph::reopen_decision`] applies `dec:reopen-supersedes` in one call:
//! a NEW `proposed` Decision that OBSOLETES the original, which stays `accepted`
//! and untouched, with the road originally taken registered as the first
//! alternative when its address is known. It is the owner's act; nothing here
//! calls it on its own.

use std::collections::{BTreeMap, BTreeSet};

use crate::alternatives::AlternativeRef;
use crate::doubt::{BadNews, DoubtEvidence};
use crate::foundation::core::{DynoError, Value};
use crate::foundation::store::StoredNode;
use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// The epoch a decision is pinned to.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForkEpoch {
    pub epoch_id: String,
    pub name: Option<String>,
    pub sequence: Option<i64>,
    /// The export hash cut at this epoch — `None` for epochs before the hash
    /// chain existed, which say so rather than claim one.
    pub checksum: Option<String>,
    pub status: Option<String>,
}

/// A recorded change, at a later epoch, to something the decision governs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangeSince {
    pub change_event_id: String,
    pub name: String,
    pub epoch_id: String,
    /// The governed nodes (or the decision itself) this change touched.
    pub touches: Vec<String>,
}

/// Everything needed to decide whether — and from where — to go back.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForkPoint {
    pub decision_id: String,
    pub name: String,
    pub status: String,
    /// `None` when the decision is pinned to no epoch — 92% of accepted
    /// decisions on reflow2's own design when this was built.
    pub epoch: Option<ForkEpoch>,
    /// Nodes GOVERNED_BY the decision: what a different road would reshape.
    pub governed: Vec<String>,
    /// Changes recorded at LATER epochs to what it governs. `None`, not empty,
    /// when there is no epoch to order against — "nothing changed" and
    /// "cannot tell" must not share a reply.
    pub changed_since: Option<Vec<ChangeSince>>,
    /// Bad news behind this decision, as [`crate::doubt`] traces it.
    pub doubt: Vec<DoubtEvidence>,
    pub evidence: BTreeMap<String, BadNews>,
    /// Checks under it a real run found passing — computed, never stored.
    pub held_up: usize,
    /// Roads registered beside it (`register_alternative`).
    pub alternatives: Vec<AlternativeRef>,
    /// Decisions that already OBSOLETE this one — it has been re-opened.
    pub reopened_by: Vec<String>,
    pub note: String,
}

/// What [`DesignGraph::reopen_decision`] wrote.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reopened {
    pub decision_id: String,
    pub reopens: String,
    /// The road originally taken, registered as the first alternative — absent
    /// when no address for it was given.
    pub road_taken: Option<AlternativeRef>,
    pub note: String,
}

fn prop<'a>(n: &'a StoredNode, k: &str) -> Option<&'a str> {
    n.properties
        .get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

fn sequence_of(n: &StoredNode) -> Option<i64> {
    n.properties.get("sequence").and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_f64().map(|f| f as i64))
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

impl DesignGraph {
    /// Where going back to `decision_id` would start from. Reads only.
    pub fn fork_point(&self, decision_id: &str) -> Result<ForkPoint, DynoError> {
        let Some(dec) = self.get_node(node::DECISION, decision_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DECISION.into(),
                node_id: decision_id.into(),
            });
        };

        let epoch = match self.outgoing(decision_id, Some(edge::AT_EPOCH))?.first() {
            Some(e) => self
                .get_node(node::DESIGN_EPOCH, &e.to_id)?
                .map(|ep| ForkEpoch {
                    epoch_id: ep.node_id.clone(),
                    name: prop(&ep, "name").map(str::to_string),
                    sequence: sequence_of(&ep),
                    checksum: prop(&ep, "checksum").map(str::to_string),
                    status: prop(&ep, "status").map(str::to_string),
                }),
            None => None,
        };

        let mut governed: Vec<String> = self
            .incoming(decision_id, Some(edge::GOVERNED_BY))?
            .into_iter()
            .map(|e| e.from_id)
            .collect();
        governed.sort();
        governed.dedup();

        let changed_since = match epoch.as_ref().and_then(|e| e.sequence) {
            None => None,
            Some(anchor) => {
                let watched: BTreeSet<&str> = governed
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(decision_id))
                    .collect();
                let mut later: BTreeMap<String, i64> = BTreeMap::new();
                for ep in self.scan_nodes(node::DESIGN_EPOCH)? {
                    if let Some(s) = sequence_of(&ep)
                        && s > anchor
                    {
                        later.insert(ep.node_id.clone(), s);
                    }
                }
                let mut out = Vec::new();
                for ch in self.scan_nodes(node::CHANGE_EVENT)? {
                    let Some(at) = self
                        .outgoing(&ch.node_id, Some(edge::AT_EPOCH))?
                        .into_iter()
                        .map(|e| e.to_id)
                        .find(|id| later.contains_key(id))
                    else {
                        continue;
                    };
                    let touches: Vec<String> = self
                        .outgoing(&ch.node_id, Some(edge::CHANGED))?
                        .into_iter()
                        .map(|e| e.to_id)
                        .filter(|id| watched.contains(id.as_str()))
                        .collect();
                    if !touches.is_empty() {
                        out.push(ChangeSince {
                            change_event_id: ch.node_id.clone(),
                            name: prop(&ch, "name").unwrap_or(&ch.node_id).to_string(),
                            epoch_id: at,
                            touches,
                        });
                    }
                }
                out.sort_by(|a, b| {
                    later[&a.epoch_id]
                        .cmp(&later[&b.epoch_id])
                        .then_with(|| a.change_event_id.cmp(&b.change_event_id))
                });
                Some(out)
            }
        };

        let doubt_report = self.decisions_in_doubt()?;
        let (doubt, held_up) = doubt_report
            .decisions
            .iter()
            .find(|d| d.decision_id == decision_id)
            .map(|d| (d.evidence.clone(), d.held_up))
            .unwrap_or_default();
        let evidence: BTreeMap<String, BadNews> = doubt
            .iter()
            .filter_map(|e| {
                doubt_report
                    .evidence
                    .get(&e.evidence_id)
                    .map(|b| (e.evidence_id.clone(), b.clone()))
            })
            .collect();

        let mut reopened_by: Vec<String> = Vec::new();
        for e in self.incoming(decision_id, Some(edge::OBSOLETES))? {
            if self.get_node(node::DECISION, &e.from_id)?.is_some() {
                reopened_by.push(e.from_id);
            }
        }
        reopened_by.sort();

        let status = prop(&dec, "status").unwrap_or("proposed").to_string();
        let note = match (&epoch, status.as_str()) {
            (_, s) if s != "accepted" => format!(
                "This decision is `{s}`, not settled — there is no road to go back from. \
                 An open choice is worked with register_alternative and collapse_decision."
            ),
            (None, _) => "No epoch is recorded for this decision, so the design version it \
                 was made in is not in the design. The tool layer looks for the earliest \
                 committed export containing it in git; that is EARLIEST EVIDENCE, not when \
                 it was decided. `changed_since` is null: without an epoch there is nothing \
                 to order changes against."
                .to_string(),
            (Some(e), _) if e.checksum.is_none() => format!(
                "Pinned to {} — an epoch that predates the export hash chain, so it is \
                 addressed by its release tag alone and claims no checksum.",
                e.epoch_id
            ),
            (Some(e), _) => format!(
                "Pinned to {}; its export hash is the fork point's address. Re-opening is \
                 reopen_choice — the owner's act, never automatic.",
                e.epoch_id
            ),
        };

        Ok(ForkPoint {
            decision_id: decision_id.to_string(),
            name: prop(&dec, "name").unwrap_or(decision_id).to_string(),
            status,
            epoch,
            governed,
            changed_since,
            doubt,
            evidence,
            held_up,
            alternatives: self.alternatives_for(decision_id)?,
            reopened_by,
            note,
        })
    }

    /// Re-open a settled decision, as `dec:reopen-supersedes` defines it.
    ///
    /// Writes a new `proposed` Decision `new_id` that OBSOLETES `decision_id`.
    /// The original stays `accepted` and is not touched: the record keeps both
    /// that the question was settled and that it was re-asked. When
    /// `road_taken_location` is given — the fork point's address, e.g. a git
    /// commit's export — the road originally taken is registered as the new
    /// decision's first alternative, a sibling now rather than the incumbent.
    ///
    /// REFUSES: an unknown or non-accepted original (only a settled road can be
    /// re-opened); an existing `new_id`; and an original a still-open decision
    /// already re-opens, naming it, so the same question is not asked twice.
    pub fn reopen_decision(
        &mut self,
        decision_id: &str,
        new_id: &str,
        name: &str,
        reason: &str,
        road_taken_location: Option<&str>,
    ) -> Result<Reopened, DynoError> {
        let refuse = |message: String| DynoError::Validation {
            node_type: node::DECISION.into(),
            property: "status".into(),
            message,
        };
        let Some(orig) = self.get_node(node::DECISION, decision_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::DECISION.into(),
                node_id: decision_id.into(),
            });
        };
        if prop(&orig, "status") != Some("accepted") {
            return Err(refuse(format!(
                "'{decision_id}' is not accepted — only a settled road can be re-opened. An \
                 open choice is worked with register_alternative and collapse_decision."
            )));
        }
        if self.get_node(node::DECISION, new_id)?.is_some() {
            return Err(refuse(format!(
                "'{new_id}' already exists — a re-opening is a NEW decision"
            )));
        }
        for e in self.incoming(decision_id, Some(edge::OBSOLETES))? {
            if let Some(d) = self.get_node(node::DECISION, &e.from_id)?
                && prop(&d, "status") == Some("proposed")
            {
                return Err(refuse(format!(
                    "'{decision_id}' is already re-opened by '{}', which is still open — work \
                     that one rather than asking the same question twice",
                    d.node_id
                )));
            }
        }

        let orig_name = prop(&orig, "name").unwrap_or(decision_id).to_string();
        let text = format!(
            "RE-OPENS {decision_id} (\"{orig_name}\"), which stays accepted and untouched \
             until this closes (dec:reopen-supersedes).\n\nWHY: {reason}"
        );
        self.begin_batch();
        let wrote = (|| -> Result<(), DynoError> {
            self.create_node(
                node::DECISION,
                new_id,
                Props::new()
                    .set("name", name)
                    .set("decision", text.as_str())
                    .set("kind", "choice")
                    .set("status", "proposed"),
            )?;
            self.create_edge(
                edge::OBSOLETES,
                node::DECISION,
                new_id,
                node::DECISION,
                decision_id,
                Props::new(),
            )?;
            Ok(())
        })();
        if let Err(e) = wrote {
            self.discard_batch();
            return Err(e);
        }
        self.commit_batch()?;

        let road_taken = match road_taken_location {
            Some(loc) => {
                let art = format!("art:road-taken-{}", decision_id.trim_start_matches("dec:"));
                Some(self.register_alternative(
                    new_id,
                    &art,
                    &format!("The road taken: {orig_name}"),
                    loc,
                )?)
            }
            None => None,
        };
        let note = if road_taken.is_some() {
            "Re-opened. The original stays accepted; the road it took is the first alternative. \
             Register the other roads with register_alternative, then settle with \
             collapse_decision — when the new decision is accepted, the original reads as \
             discontinued."
                .to_string()
        } else {
            "Re-opened, with no address for the road originally taken, so it is NOT registered \
             as an alternative — pass road_taken_location (fork_point gives it) to register it. \
             The original stays accepted until the new decision is settled."
                .to_string()
        };
        Ok(Reopened {
            decision_id: new_id.to_string(),
            reopens: decision_id.to_string(),
            road_taken,
            note,
        })
    }
}
