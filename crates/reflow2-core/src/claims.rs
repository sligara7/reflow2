//! Who has what in hand — advisory claims over regions of the design (BL-44).
//!
//! The scoping primitive `dec:multi-writer-architecture` commits to. Fifteen
//! people can work one design; the three-way merge already resolves genuine
//! overlaps correctly, and claims exist so they rarely have to.
//!
//! # A claim is not a lock, and cannot be
//!
//! The decision keeps the design as a file in each person's checkout, moved by
//! git, with no shared server. There is nowhere for a lock to live. So a claim:
//!
//! - **never blocks anyone.** Nothing in this module refuses a write, and
//!   nothing anywhere else consults a claim before allowing one. A second writer
//!   who ignores a claim gets a correct merge, exactly as if the claim did not
//!   exist.
//! - **is only as fresh as the last pull.** One person's claim is invisible to
//!   another until the export travels. Claims reduce collisions; they do not
//!   prevent them.
//!
//! Both limits are stated here rather than buried, because a claims layer that
//! *reads* like a locking mechanism is worse than none: it invites people to
//! rely on a guarantee it cannot make (`dec:report-dont-judge`).
//!
//! # The region is computed, not drawn
//!
//! A claim stores a **seed and a depth**, never a node list. The region is
//! derived from them by the same traversal `propagate_from` uses, so it tracks
//! the design as the design changes instead of freezing a membership list that
//! silently goes wrong the moment someone adds an edge.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};
use crate::propagate::PropagateOptions;

/// One region someone has in hand.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Claim {
    /// The Contributor holding it.
    pub contributor_id: String,
    /// The node the region is computed from.
    pub seed_id: String,
    /// How far from the seed the region reaches, in hops.
    pub depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The session that made it (`req:claims-have-owners`). Absent on claims
    /// made before seats were recorded — reported as unknown, never assumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat: Option<String>,
    /// Whether that session is still running. **Computed at read time** from
    /// the operating system, never stored: nothing writes "I am alive", so
    /// nothing can be stale about it.
    pub liveness: crate::identity::Liveness,
}

/// Two claims whose computed regions intersect — the thing worth telling
/// someone about *before* they start.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaimOverlap {
    pub a: Claim,
    pub b: Claim,
    /// The design nodes both regions cover, sorted.
    pub shared: Vec<String>,
}

/// Every claim on the design, and where they collide.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaimReport {
    /// All claims, sorted by contributor then seed.
    pub claims: Vec<Claim>,
    /// Pairs of claims whose regions intersect, worst (largest overlap) first.
    /// **Ghost claims are not in here** — see `stale`.
    pub overlaps: Vec<ClaimOverlap>,
    /// Claims whose session has exited. Still listed in `claims`, because the
    /// note on them is often exactly what a colleague wants to read — but kept
    /// OUT of `overlaps`, because a collision with nobody is not a collision,
    /// and reporting it as one is how an advisory report starts lying.
    pub stale: Vec<Claim>,
    /// Said in the payload, not only in the docs: whoever reads this over the
    /// wire needs to know an overlap is a warning, not a refusal.
    pub advisory: &'static str,
}

const ADVISORY: &str = "Claims are advisory: they never block a write, and they are only as fresh \
                        as the last pull. An overlap means two people may collide — the merge will \
                        still resolve it correctly if they do. Liveness is COMPUTED from the \
                        claiming session, not stored: `gone` means that session has exited and the \
                        claim is a ghost (excluded from overlaps), `unknown` means it was made on \
                        another machine or before seats were recorded — unknown is never read as \
                        free, because taking work somebody is actively doing is the expensive \
                        mistake.";

fn int_prop(props: &BTreeMap<String, Value>, key: &str, fallback: usize) -> usize {
    props
        .get(key)
        .and_then(|v| match v {
            Value::Int(i) => usize::try_from(*i).ok(),
            _ => None,
        })
        .unwrap_or(fallback)
}

fn str_prop(props: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
}

impl DesignGraph {
    /// Take a region in hand: `contributor` claims everything within `depth`
    /// hops of `seed`.
    ///
    /// Refuses an unknown contributor or seed — a claim naming something that
    /// does not exist tells a colleague nothing and would sit in the export
    /// looking authoritative. Re-claiming the same seed updates the existing
    /// claim rather than accumulating duplicates.
    ///
    /// Note what this does NOT do: it does not check whether anyone else
    /// already holds the region, and it does not refuse if they do. Overlap is
    /// reported by [`claim_report`](Self::claim_report), never prevented here —
    /// two people are allowed to work the same area, and sometimes must.
    pub fn claim_region(
        &mut self,
        contributor_id: &str,
        seed_id: &str,
        depth: usize,
        note: Option<&str>,
        at: Option<&str>,
        seat: Option<&str>,
    ) -> Result<Claim, DynoError> {
        if self.get_node(node::CONTRIBUTOR, contributor_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: node::CONTRIBUTOR.into(),
                node_id: contributor_id.into(),
            });
        }
        let index = self.node_type_index()?;
        let Some(seed_type) = index.get(seed_id).cloned() else {
            return Err(DynoError::NodeNotFound {
                node_type: "any".into(),
                node_id: seed_id.into(),
            });
        };

        let mut props = Props::new().set("depth", i64::try_from(depth).unwrap_or(i64::MAX));
        if let Some(at) = at {
            props = props.set("claimed_at", at);
        }
        if let Some(note) = note {
            props = props.set("note", note);
        }
        // Default to THIS session rather than leaving it blank: a claim with no
        // owner cannot be told from one nobody is working, and the whole point
        // is that the report stops lying about which is which. A caller may pass
        // its own seat (a fleet worker handle, say) — it is a name, not a lock.
        let seat = seat
            .map(str::to_string)
            .unwrap_or_else(crate::identity::seat_id);
        props = props.set("seat", seat.as_str());
        self.create_edge(
            edge::CLAIMS,
            node::CONTRIBUTOR,
            contributor_id,
            &seed_type,
            seed_id,
            props,
        )?;
        Ok(Claim {
            contributor_id: contributor_id.to_string(),
            seed_id: seed_id.to_string(),
            depth,
            claimed_at: at.map(str::to_string),
            note: note.map(str::to_string),
            liveness: crate::identity::seat_liveness(&seat),
            seat: Some(seat),
        })
    }

    /// Let a region go. `true` if a claim was there to release.
    pub fn release_claim(
        &mut self,
        contributor_id: &str,
        seed_id: &str,
    ) -> Result<bool, DynoError> {
        self.delete_edge(edge::CLAIMS, contributor_id, seed_id)
    }

    /// Every claim currently held, sorted.
    pub fn claims(&self) -> Result<Vec<Claim>, DynoError> {
        let mut out = Vec::new();
        for c in self.scan_nodes(node::CONTRIBUTOR)? {
            for e in self.outgoing(&c.node_id, Some(edge::CLAIMS))? {
                let props: BTreeMap<String, Value> = e.properties.into_iter().collect();
                let seat = str_prop(&props, "seat");
                out.push(Claim {
                    contributor_id: c.node_id.clone(),
                    seed_id: e.to_id,
                    depth: int_prop(&props, "depth", 2),
                    claimed_at: str_prop(&props, "claimed_at"),
                    note: str_prop(&props, "note"),
                    // Asked of the OS on every read. A claim from a session that
                    // has since exited is a ghost, and a ghost reported as held
                    // is what makes people wait for nobody.
                    liveness: seat
                        .as_deref()
                        .map(crate::identity::seat_liveness)
                        .unwrap_or(crate::identity::Liveness::Unknown),
                    seat,
                });
            }
        }
        out.sort_by(|a, b| {
            a.contributor_id
                .cmp(&b.contributor_id)
                .then(a.seed_id.cmp(&b.seed_id))
        });
        Ok(out)
    }

    /// The nodes one claim covers — computed from its seed and depth, never
    /// stored, so it follows the design rather than freezing a stale list.
    pub fn claimed_region(&self, claim: &Claim) -> Result<BTreeSet<String>, DynoError> {
        let radius = self.propagate_from(
            &[claim.seed_id.as_str()],
            PropagateOptions {
                max_depth: claim.depth,
            },
        )?;
        let mut ids: BTreeSet<String> = radius.impacted.into_iter().map(|i| i.node_id).collect();
        ids.insert(claim.seed_id.clone());
        Ok(ids)
    }

    /// Who holds what, and where two people are working the same ground.
    ///
    /// Overlaps are ranked by how much they share, because a two-node brush is
    /// worth knowing and a forty-node collision is worth talking about, and a
    /// flat list would present them identically.
    pub fn claim_report(&self) -> Result<ClaimReport, DynoError> {
        let claims = self.claims()?;
        let mut regions = Vec::with_capacity(claims.len());
        for c in &claims {
            regions.push(self.claimed_region(c)?);
        }

        let mut overlaps = Vec::new();
        for i in 0..claims.len() {
            for j in (i + 1)..claims.len() {
                // Two claims by the SAME person are not a collision — one
                // person holding two overlapping regions is just one person
                // working, and reporting it would train people to ignore this.
                if claims[i].contributor_id == claims[j].contributor_id {
                    continue;
                }
                // A claim whose session has exited is not held by anybody, so
                // an overlap with it is not a collision — that is exactly the
                // ghost this requirement exists to stop reporting as live.
                // `Unknown` still counts: it may well be somebody working on
                // another machine, and the costly error is the other direction.
                if claims[i].liveness == crate::identity::Liveness::Gone
                    || claims[j].liveness == crate::identity::Liveness::Gone
                {
                    continue;
                }
                let shared: Vec<String> = regions[i]
                    .intersection(&regions[j])
                    .map(String::from)
                    .collect();
                if !shared.is_empty() {
                    overlaps.push(ClaimOverlap {
                        a: claims[i].clone(),
                        b: claims[j].clone(),
                        shared,
                    });
                }
            }
        }
        overlaps.sort_by(|x, y| {
            y.shared
                .len()
                .cmp(&x.shared.len())
                .then(x.a.contributor_id.cmp(&y.a.contributor_id))
                .then(x.b.contributor_id.cmp(&y.b.contributor_id))
        });

        let stale: Vec<Claim> = claims
            .iter()
            .filter(|c| c.liveness == crate::identity::Liveness::Gone)
            .cloned()
            .collect();
        Ok(ClaimReport {
            claims,
            overlaps,
            stale,
            advisory: ADVISORY,
        })
    }
}
