//! P4 · Verification — the write side of the verify domain (WS-1).
//!
//! [`detect.rs`](crate::detect) raises two gaps that ask the user for a
//! `Verification`: `build_without_verification` ("you've built things but
//! nothing checks them") and `unverified_capability` ("this realized capability
//! has no check"). Until now neither could be answered with a typed call —
//! `Verification` was counted and reported but never constructible, so the gap
//! could be raised and not closed.
//!
//! A `Verification` is deliberately broad: a unit test, a design review, a
//! simulation, a physical inspection, a measurement, a live demonstration, an
//! observation of the fielded system. `method` and `level` carry that
//! distinction rather than the type name, so a hardware inspection and a
//! `cargo test` run are the same kind of node with different properties — which
//! is what lets the same coverage gap work across domains.
//!
//! `method` follows DoD/INCOSE practice, whose four canonical methods are test,
//! analysis, inspection and **demonstration**. Demonstration and observation
//! were added on 2026-07-26 (user's taxonomy): until then "we showed it
//! working" had to be miscoded as `test`, which is how a great deal of
//! acceptance is actually closed, and "we watched it run in the field" — the
//! as-fielded method, distinct from inspecting an artifact or running a
//! contrived example — had no value at all.

use crate::foundation::core::DynoError;
use crate::foundation::store::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};
use std::collections::{BTreeMap, BTreeSet};

/// How a capability's claim to work is checked — three-valued on purpose
/// (BL-73, from the first extensive field trial). A brownfield adopt with a
/// real per-service test suite read as "0/20 capabilities verified": the
/// suites were registered against *components*, and nothing on the read side
/// knew what that meant for the capabilities allocated to them. "Verified at
/// component granularity" is neither "verified" nor "unverified" — collapsing
/// it into either understates a tested system or overstates a wholesale claim
/// (`dec:component-verified-computed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityVerification {
    /// A passing `Verification` checks this capability itself.
    Verified,
    /// No passing check of its own, but a component it is allocated to
    /// carries one — the capability rides its component's suite. Derived,
    /// never written: the graph records exactly what was checked (the
    /// component), and this state is what that fact means one hop away.
    ComponentVerified,
    /// No passing check anywhere in sight.
    Unchecked,
}

impl DesignGraph {
    /// Whether a node has at least one incoming `VERIFIES` from a passing
    /// `Verification`. "Verified means a check that passes, not one that
    /// exists" (`dec:passing-is-verified`).
    pub(crate) fn has_passing_verification(&self, node_id: &str) -> Result<bool, DynoError> {
        for e in self.incoming(node_id, Some(edge::VERIFIES))? {
            let passing = self
                .get_node(node::VERIFICATION, &e.from_id)?
                .and_then(|v| {
                    v.properties
                        .get("status")
                        .and_then(crate::foundation::core::Value::as_str)
                        .map(|s| s == "passing")
                })
                .unwrap_or(false);
            if passing {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Compute a capability's [`CapabilityVerification`] state. See the enum
    /// for why this is three-valued.
    /// Whether a capability has passing evidence ANYWHERE — its own check or
    /// its component's suite.
    ///
    /// A predicate rather than a comparison against the enum, because the
    /// caller that wanted this was `temporal::item_is_delivered`, and naming
    /// `CapabilityVerification::Unchecked` there was the third hop of a
    /// module cycle: artifact → temporal → verify → artifact. Which VARIANT a
    /// capability is in is this module's business; whether it has evidence is
    /// the question everyone else is actually asking, and answering it here
    /// costs nothing and breaks the loop.
    ///
    /// Found 2026-08-20 by an adopt pass over reflow2's own source, which is
    /// the only reason anyone looked: a module cycle inside one crate is legal
    /// Rust and compiles silently forever.
    pub fn capability_has_evidence(&self, capability_id: &str) -> Result<bool, DynoError> {
        Ok(!matches!(
            self.capability_verification(capability_id)?,
            CapabilityVerification::Unchecked
        ))
    }

    pub fn capability_verification(
        &self,
        capability_id: &str,
    ) -> Result<CapabilityVerification, DynoError> {
        if self.has_passing_verification(capability_id)? {
            return Ok(CapabilityVerification::Verified);
        }
        for e in self.outgoing(capability_id, Some(edge::ALLOCATED_TO))? {
            if self.has_passing_verification(&e.to_id)? {
                return Ok(CapabilityVerification::ComponentVerified);
            }
        }
        Ok(CapabilityVerification::Unchecked)
    }
    /// P4 · Verification — a check that something meets its intent. `name` is
    /// required; `method` (default `test`), `level` (default `unit`),
    /// `location` and `status` (default `planned`) are optional.
    ///
    /// `status` is what makes a Verification more than an inventory entry: a
    /// `failing` check on a realized Capability is a live signal, not a record.
    /// `description` says what the check IS, at length — and it is a parameter
    /// here since 2026-08-17 because it was DECLARED, fulltext, the embedding
    /// field, and **used once in 164 nodes**. Not ignored: unreachable. This
    /// constructor took no parameter for it, so the only route was raw
    /// `create_node`, and essentially nobody took it — everyone wrote into
    /// `name` instead, because `name` was the only string on offer. That is the
    /// measured cause of a corpus whose median Verification name is 76 words
    /// and whose longest is 654 (`req:a-finding-has-somewhere-to-put-its-evidence`).
    ///
    /// What a run FOUND goes to `findings` on
    /// [`set_verification_status`](Self::set_verification_status), not here: a
    /// finding belongs to a run, and restating the definition on every re-run
    /// is the thing that separation exists to avoid.
    pub fn add_verification(
        &mut self,
        id: &str,
        name: &str,
        method: Option<&str>,
        level: Option<&str>,
        description: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.upsert_node(
            node::VERIFICATION,
            id,
            Props::new()
                .set("name", name)
                .set_opt("method", method)
                .set_opt("level", level)
                .set_opt("description", description),
        )
    }

    /// Set a `Verification`'s outcome, preserving its other properties.
    /// `status` ∈ `planned` / `passing` / `failing` / `skipped` / `blocked`.
    ///
    /// Kept separate from creation because the outcome changes far more often
    /// than the check itself, and a re-run should not have to restate what the
    /// check *is*.
    ///
    /// **Omitting `last_run_at` LEAVES IT ALONE.** It used to erase it, which is
    /// the bug dev_storyflow filed on 2026-08-08 and re-filed after retesting on
    /// 0.26.1 — reproduced on a throwaway node, one variable, the key REMOVED
    /// rather than nulled, so a wiped check was byte-identical to one that had
    /// never run.
    ///
    /// # Why the direction was the bad one
    ///
    /// It fired from the most ordinary act there is — marking a check `failing`
    /// after a regression — and erased the evidence it had ever run, failing
    /// toward `never_run`, which is precisely the field a later session greps
    /// for unproven work. And it was invisible at the call site: the response is
    /// success-shaped and dominated by a long `name`.
    ///
    /// # And it broke a convention this codebase states elsewhere as a principle
    ///
    /// `set_interface_spec` promises *"omitting one LEAVES IT ALONE"*,
    /// `set_decision_status` *"Every other property is preserved"*,
    /// `set_epoch_status` *"Everything else about the epoch is preserved."*
    /// This function's own first line has always said *"preserving its other
    /// properties"* — and then null-wrote one of them. One tool departing from a
    /// convention the others state is worse than no convention, because a caller
    /// who has read any sibling reasonably assumes it.
    ///
    /// Clearing a run time is therefore not expressible by omission, and should
    /// not be: erasing evidence is a deliberate act and deserves an explicit
    /// one.
    /// # `findings` — what this run FOUND
    ///
    /// It belongs here rather than on the constructor, and that placement is the
    /// design. A finding is produced by a RUN: it changes every time an outcome
    /// changes, while the definition of the check does not. Recording it beside
    /// the status means the evidence is written at the moment it exists, by the
    /// caller who has it in hand.
    ///
    /// Omitting it LEAVES IT ALONE, exactly as `last_run_at` does — so marking a
    /// check `passing` again without restating the evidence keeps the last
    /// evidence rather than erasing it. Erasing is a deliberate act and deserves
    /// an explicit one, which is the same reasoning that governs `last_run_at`
    /// one field over, and the same bug if it were got wrong.
    ///
    /// ⚠️ NOT VALIDATED AND NOT PARSED. reflow2 records what an author says a
    /// run found and never judges it
    /// (`dec:non-goal-reflow2-does-not-judge-whether-a-check-is-meaningful`).
    /// A `passing` status beside findings that describe a failure is a
    /// contradiction only a reader can catch — and it is exactly the case
    /// dev_storyflow reported on 2026-08-07, where a check recorded "EXIT 0,
    /// verdict STALE" and stayed `passing` forever.
    pub fn set_verification_status(
        &mut self,
        verification_id: &str,
        status: &str,
        last_run_at: Option<&str>,
        findings: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        let Some(existing) = self.get_node(node::VERIFICATION, verification_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::VERIFICATION.to_string(),
                node_id: verification_id.to_string(),
            });
        };
        let mut props = Props::new()
            .set("status", status)
            .set_opt("last_run_at", last_run_at)
            .set_opt("findings", findings);
        for (k, v) in &existing.properties {
            // `status` is always replaced. `last_run_at` and `findings` are
            // replaced ONLY when the caller supplied one — otherwise the stored
            // value is carried over, which is what "preserving its other
            // properties" means.
            let replaced = k == "status"
                || (k == "last_run_at" && last_run_at.is_some())
                || (k == "findings" && findings.is_some());
            if !replaced {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::VERIFICATION, verification_id, props)
    }

    /// Set a Verification's `kind` — `verification` (built right, against the
    /// spec) or `validation` (the right thing, against the intent). A separate
    /// setter, like `set_verification_status`: a check is created, then marked.
    /// The axis the `unvalidated_capability` gap reads (`dec:edge-orthogonality`).
    pub fn set_verification_kind(
        &mut self,
        verification_id: &str,
        kind: &str,
    ) -> Result<StoredNode, DynoError> {
        const KINDS: [&str; 2] = ["verification", "validation"];
        if !KINDS.contains(&kind) {
            return Err(DynoError::Validation {
                node_type: node::VERIFICATION.to_string(),
                property: "kind".to_string(),
                message: format!(
                    "'{kind}' is not a Verification kind (one of {})",
                    KINDS.join(", ")
                ),
            });
        }
        let Some(existing) = self.get_node(node::VERIFICATION, verification_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::VERIFICATION.to_string(),
                node_id: verification_id.to_string(),
            });
        };
        let mut props = Props::new().set("kind", kind);
        for (k, v) in &existing.properties {
            if k != "kind" {
                props = props.set(k, v.clone());
            }
        }
        self.create_node(node::VERIFICATION, verification_id, props)
    }

    /// `Verification VERIFIES target` — the check and the thing it checks.
    ///
    /// `target_type` is required because the target is not inferable from the
    /// id. It is NOT because "the schema allows any target": that was true
    /// until 2026-08-08, when `to: "*"` became an enumeration precisely so
    /// `unverified_enforced_rule` could be asked, and this sentence has been
    /// stale ever since. The schema is the authority on which types are legal —
    /// see `schema/verify.yaml`, where the list carries the reasoning for every
    /// type on it and for `Project` being deliberately off it.
    ///
    /// PROPAGATE reads this edge as Upstream from the Verification, so a failing
    /// check reaches the Capability it covers and the Requirement behind it.
    pub fn verifies(
        &mut self,
        verification_id: &str,
        target_type: &str,
        target_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::VERIFIES,
            node::VERIFICATION,
            verification_id,
            target_type,
            target_id,
            Props::new(),
        )
    }

    /// Record what a check held FIXED and what it VARIED for one claim
    /// (BL-126). Scope lives on the `VERIFIES` edge, not on the `Verification`,
    /// because it is a fact about the relationship: the same suite can cover one
    /// capability across the whole input space and touch another at a single
    /// point (`dec:evidence-scope-on-the-verifies-edge`).
    ///
    /// A separate setter, like [`set_verification_status`](Self::set_verification_status)
    /// and `set_interface_spec` — one way to do it, and a re-run should not have
    /// to restate what the check *is*. Other edge properties (`coverage`) are
    /// preserved, and the edge must already exist: silently creating one would
    /// let a typo invent a verification relationship that nobody asserted.
    ///
    /// Passing an empty list for either side CLEARS that side, which is how a
    /// scope recorded in error is withdrawn. Names are stored comma-separated,
    /// so a name containing a comma is refused rather than silently split into
    /// two parameters that were never checked.
    pub fn set_evidence_scope(
        &mut self,
        verification_id: &str,
        target_type: &str,
        target_id: &str,
        pinned: &[String],
        swept: &[String],
    ) -> Result<StoredEdge, DynoError> {
        for (side, names) in [("pinned", pinned), ("swept", swept)] {
            for n in names {
                if n.contains(',') {
                    return Err(DynoError::Validation {
                        node_type: edge::VERIFIES.to_string(),
                        property: side.to_string(),
                        message: format!(
                            "parameter name '{n}' contains a comma; names are stored \
                             comma-separated, so this would be read back as two parameters \
                             that were never checked"
                        ),
                    });
                }
            }
        }

        let existing = self
            .outgoing(verification_id, Some(edge::VERIFIES))?
            .into_iter()
            .find(|e| e.to_id == target_id)
            .ok_or_else(|| DynoError::Validation {
                node_type: edge::VERIFIES.to_string(),
                property: "target".to_string(),
                message: format!(
                    "'{verification_id}' does not verify '{target_id}' — record the check \
                     against its target with `verifies` first, so a mistyped id cannot \
                     invent a verification relationship nobody asserted"
                ),
            })?;

        let mut props = Props::new()
            .set("pinned", pinned.join(","))
            .set("swept", swept.join(","));
        for (k, v) in &existing.properties {
            if k != "pinned" && k != "swept" {
                props = props.set(k, v.clone());
            }
        }
        self.create_edge(
            edge::VERIFIES,
            node::VERIFICATION,
            verification_id,
            target_type,
            target_id,
            props,
        )
    }

    /// Record that a value was FITTED to a piece of evidence (BL-136) — the
    /// relation that stops that same evidence counting as its validation.
    ///
    /// `evidence_type` is `Artifact` (a published anchor, a dataset, a
    /// measurement record) or `Verification` (the check whose output the value
    /// was fitted to). Both endpoints must already exist, as for every edge.
    ///
    /// This is deliberately a recorded relation and not a computed one. The
    /// project BL-136 came from built four independent internal diagnostics and
    /// none of them could have found its circular fit; only the outside source
    /// could. No check inside a design can establish its own independence, so
    /// the fact has to be written down by whoever made the fit.
    pub fn calibrated_against(
        &mut self,
        from_type: &str,
        from_id: &str,
        evidence_type: &str,
        evidence_id: &str,
        note: Option<&str>,
        calibrated_at: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CALIBRATED_AGAINST,
            from_type,
            from_id,
            evidence_type,
            evidence_id,
            Props::new()
                .set_opt("note", note)
                .set_opt("calibrated_at", calibrated_at),
        )
    }

    /// `record INVALIDATES finding` — the work that answered a finding says so,
    /// so the finding stops proposing work already done.
    ///
    /// Draw it from whatever recorded the work (a Constraint carrying a repair,
    /// a ChangeEvent, a Decision) to whatever recorded the finding (a
    /// Verification whose last run found it, a TemporalFact that measured it).
    ///
    /// ⭐ IT CLAIMS THE RESULT IS STALE AND NOTHING MORE. A repair does not make
    /// a check pass; only a re-run can say what is true now. That is why the
    /// edge is not called `RESOLVES`, and why nothing here touches the target's
    /// `status` — `set_verification_status` remains the only thing that moves a
    /// verdict, and it moves it on evidence.
    ///
    /// `at` is the date the invalidating work landed. Pass it whenever you have
    /// it: [`Self::invalidated_findings`] compares it against the target's own
    /// `last_run_at` to tell a re-run OWED from one already TAKEN, and with no
    /// date it reports the claim as undated rather than assuming it is fresh.
    pub fn invalidates(
        &mut self,
        from_type: &str,
        from_id: &str,
        finding_type: &str,
        finding_id: &str,
        note: Option<&str>,
        at: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::INVALIDATES,
            from_type,
            from_id,
            finding_type,
            finding_id,
            Props::new().set_opt("note", note).set_opt("at", at),
        )
    }

    /// Every finding some record claims to have invalidated, with whether a
    /// re-run is owed.
    ///
    /// THE READER THAT MAKES THE EDGE WORTH DRAWING. A marker nothing consults
    /// is a comment — the failure this project has now found in `enforced`, in
    /// `SUPERSEDES`, and in `OBSOLETES` — so this ships with the edge rather
    /// than after it.
    ///
    /// It REPORTS and never judges: a finding here stays visible and counted,
    /// exactly as a `parks` ruling leaves a parked node counted rather than
    /// hidden. Silencing it would be the truncation the marker exists to end.
    pub fn invalidated_findings(&self) -> Result<Vec<InvalidatedFinding>, DynoError> {
        let mut ids = Vec::new();
        for finding_type in [node::VERIFICATION, node::TEMPORAL_FACT] {
            for n in self.scan_nodes(finding_type)? {
                ids.push((finding_type, n));
            }
        }
        self.invalidated_among(ids)
    }

    /// The same answer for a NAMED set of findings — and the one every rollup
    /// should call.
    ///
    /// 🛑 WHY THIS EXISTS, AND IT IS A REGRESSION THIS CODE CAUSED. The first
    /// version of [`Self::invalidated_findings`] asked `incoming()` about every
    /// Verification and every TemporalFact. `scan_incoming_edges` walks the
    /// whole edge set, so that is one full-graph scan PER NODE: measured
    /// 2026-08-24 on reflow2's own graph, 483 findings over 13,188 edges cost
    /// **40.5 seconds** — to return 1.2 KB saying nothing was claimed. It
    /// shipped inside `loop_status`, which `cap:loop-status` calls ONE CHEAP
    /// CALL and which every session is told to run, so the orientation read
    /// went from ~10s to ~40s and the cause was the reader, not the graph.
    ///
    /// ⭐ THE FIX IS NOT A FASTER SCAN, IT IS ASKING A SMALLER QUESTION. A
    /// rollup only annotates rows it is already showing — the `attention` list,
    /// which is the checks that are NOT passing, and is 1 on this graph against
    /// 203 Verifications. Passing those ids turns 483 scans into `len(rows)`.
    /// The exhaustive form stays for the standalone tool, where a caller asked
    /// for it deliberately and can afford it.
    /// Claims against a NAMED set of Verifications — what a rollup should ask.
    ///
    /// Costs one adjacency scan per id rather than one per node in the graph.
    /// Give it the rows you are about to show and nothing else.
    pub fn invalidated_verifications(
        &self,
        verification_ids: &[&str],
    ) -> Result<Vec<InvalidatedFinding>, DynoError> {
        let mut found = Vec::new();
        for id in verification_ids {
            if let Some(n) = self.get_node(node::VERIFICATION, id)? {
                found.push((node::VERIFICATION, n));
            }
        }
        self.invalidated_among(found)
    }

    fn invalidated_among(
        &self,
        findings: Vec<(&str, crate::StoredNode)>,
    ) -> Result<Vec<InvalidatedFinding>, DynoError> {
        let mut out = Vec::new();
        {
            for (finding_type, n) in findings {
                let claims = self.incoming(&n.node_id, Some(edge::INVALIDATES))?;
                if claims.is_empty() {
                    continue;
                }
                let last_run_at = n
                    .properties
                    .get("last_run_at")
                    .and_then(crate::foundation::core::Value::as_str)
                    .map(str::to_string);
                let mut by = Vec::new();
                let mut newest: Option<String> = None;
                let mut undated = 0usize;
                for e in &claims {
                    let at = e
                        .properties
                        .get("at")
                        .and_then(crate::foundation::core::Value::as_str)
                        .map(str::to_string);
                    match &at {
                        Some(a) => {
                            if newest.as_deref().is_none_or(|n| a.as_str() > n) {
                                newest = Some(a.clone());
                            }
                        }
                        None => undated += 1,
                    }
                    by.push(InvalidationClaim {
                        claimed_by: e.from_id.clone(),
                        at,
                        note: e
                            .properties
                            .get("note")
                            .and_then(crate::foundation::core::Value::as_str)
                            .map(str::to_string),
                    });
                }
                // UNDATED IS REPORTED, NEVER GUESSED. With no date on either
                // side the honest answer is that nobody can say whether the run
                // already reflects the repair — not that it does.
                let rerun_owed = match (&newest, &last_run_at) {
                    (Some(a), Some(r)) => Some(a.as_str() > r.as_str()),
                    _ => None,
                };
                by.sort_by(|a, b| a.claimed_by.cmp(&b.claimed_by));
                out.push(InvalidatedFinding {
                    finding_id: n.node_id.clone(),
                    finding_type: finding_type.to_string(),
                    status: n
                        .properties
                        .get("status")
                        .and_then(crate::foundation::core::Value::as_str)
                        .map(str::to_string),
                    last_run_at,
                    rerun_owed,
                    undated_claims: undated,
                    claimed_by: by,
                });
            }
        }
        out.sort_by(|a, b| a.finding_id.cmp(&b.finding_id));
        Ok(out)
    }

    /// **The absence half: open observations this work TOUCHED that nobody has
    /// claimed.** The complement of [`Self::invalidated_findings`], which
    /// reports the claims that exist; this reports the ones that are missing.
    ///
    /// ⭐⭐ WHY THIS EXISTS, and it is a measurement rather than a theory.
    /// `INVALIDATES` shipped with its reader on 2026-08-23 so the marker would
    /// not become a comment nobody consults. Measured 2026-08-24, a day later,
    /// with the tool served the whole time: **zero edges had ever been drawn.**
    /// The edge was reachable and unused, because a design's vocabulary only
    /// reaches real work with three legs — a typed tool, an INSTRUCTION that
    /// names it, and a COMPUTATION THAT NOTICES ITS ABSENCE. Two were missing.
    /// This is the third.
    ///
    /// 🛑 IT ASKS A SESSION-SIZED QUESTION, AND THAT BOUND IS THE DESIGN.
    /// Design-wide, this graph carries 270 open observations, and a detector
    /// firing on all of them is wallpaper — the failure a consumer abandoned
    /// reflow2 over. Scoped to the ChangeEvents one session actually recorded,
    /// the answer is small: measured over all 639 events on this graph,
    /// **71% touch no open observation at all, and the median when one is
    /// touched is 1.**
    ///
    /// ⚠️ THE TAIL IS REAL AND IS NOT HIDDEN: mean 4.3, p90 13, max 40. It is
    /// driven by HUB SUBJECTS — `proj:reflow2` alone carries 25 open
    /// observations, `cmp:detect` and `cmp:service` 13 each — so a change that
    /// touches one of those gets a long list however well the question is
    /// scoped. 3% of events return more than ten. A caller rendering this to a
    /// human should sample and count the rest rather than print all of it.
    ///
    /// 🛑 TEMPORAL FACTS ONLY, AND NOT VERIFICATIONS — this is what keeps it
    /// from reversing `dec:verification-freshness-not-a-gap` (accepted
    /// 2026-07-26, and read before this was written). That decision rules that
    /// a stale-looking CHECK is a STANDING PROPERTY which would fire on every
    /// legitimate refactor, so it belongs on the confirmation ledger and never
    /// in a nagging list. A TemporalFact is the other thing: a DATED
    /// OBSERVATION, asserted once, true at a moment. Nothing re-derives it, and
    /// it goes on proposing work already done until somebody says otherwise.
    ///
    /// AND IT IS DELIBERATELY NOT A GAP SOURCE, for the same decision's reason.
    /// It answers when asked and appears in no list that must reach zero.
    ///
    /// COST: bounded by what you pass, never by the graph. One adjacency scan
    /// per event, per subject and per candidate — the "ask a smaller question"
    /// rule that [`Self::invalidated_verifications`] exists to enforce, applied
    /// here from the start rather than after a 40-second regression.
    ///
    /// ⭐ IT READS THE `subject_id` PROPERTY AS WELL AS THE SUBJECT EDGES, and
    /// that is not a detail — it is most of the coverage. Measured on this
    /// graph's 270 open observations: **151 are reachable by a subject EDGE
    /// (56%), while 261 are reachable once `subject_id` is read too (97%).**
    /// The property is REQUIRED on the type and the edges are optional, so an
    /// edges-only reader answers barely half the question while looking exactly
    /// like one that answers all of it. The first draft of this function did
    /// precisely that.
    ///
    /// WHAT IT STILL CANNOT SEE: the 9 observations whose `subject_id` names a
    /// node that does not exist. They are unreachable by any traversal and are
    /// not counted as covered. A quiet answer means nothing was touched *that
    /// is anchored*, which `subjects_examined` lets a caller tell apart from
    /// "checked, all clear".
    pub fn unclaimed_findings_near(
        &self,
        change_event_ids: &[&str],
    ) -> Result<UnclaimedFindings, DynoError> {
        let mut unknown_events = Vec::new();
        let mut subjects: BTreeSet<String> = BTreeSet::new();
        for id in change_event_ids {
            if self.get_node(node::CHANGE_EVENT, id)?.is_none() {
                // NAMED, NEVER SKIPPED. A typo'd event id would otherwise
                // produce an empty shortlist, which reads exactly like "your
                // work retired nothing" — the most reassuring answer available
                // and the one least likely to be questioned.
                unknown_events.push((*id).to_string());
                continue;
            }
            for e in self.outgoing(id, Some(edge::CHANGED))? {
                subjects.insert(e.to_id);
            }
        }

        // ONE PASS OVER THE OBSERVATIONS, indexed by the subject they name.
        // `subject_id` is REQUIRED on a TemporalFact while the subject edges
        // are optional, so this index is where most of the coverage comes from
        // — see the note above. One scan of a single node type, never the
        // per-node adjacency walk that cost `invalidated_findings` 40 seconds.
        let mut by_subject: BTreeMap<String, Vec<StoredNode>> = BTreeMap::new();
        let mut open_facts: BTreeMap<String, StoredNode> = BTreeMap::new();
        if !subjects.is_empty() {
            for n in self.scan_nodes(node::TEMPORAL_FACT)? {
                // Already closed: somebody dated its end. Nothing to ask.
                if n.properties
                    .get("valid_to")
                    .and_then(crate::foundation::core::Value::as_str)
                    .is_some_and(|v| !v.is_empty())
                {
                    continue;
                }
                if let Some(sid) = n
                    .properties
                    .get("subject_id")
                    .and_then(crate::foundation::core::Value::as_str)
                {
                    by_subject
                        .entry(sid.to_string())
                        .or_default()
                        .push(n.clone());
                }
                open_facts.insert(n.node_id.clone(), n);
            }
        }

        let mut candidates: BTreeMap<String, UnclaimedFinding> = BTreeMap::new();
        for subject in &subjects {
            let mut reached: BTreeSet<String> = BTreeSet::new();
            for n in by_subject.get(subject).into_iter().flatten() {
                reached.insert(n.node_id.clone());
            }
            for e in self.outgoing(subject, Some(edge::HAS_TEMPORAL_FACT))? {
                reached.insert(e.to_id);
            }
            for e in self.incoming(subject, Some(edge::ABOUT_ENTITY))? {
                reached.insert(e.from_id);
            }
            for fact_id in reached {
                // Only OPEN observations are in this map, so a miss here is a
                // fact that is closed or was never a TemporalFact at all.
                let Some(n) = open_facts.get(&fact_id) else {
                    continue;
                };
                // Already claimed: some record says it answered this. The
                // question has been put and answered, and asking again would
                // be the nag this is shaped to avoid.
                if !self.incoming(&fact_id, Some(edge::INVALIDATES))?.is_empty() {
                    continue;
                }
                let entry = candidates
                    .entry(fact_id.clone())
                    .or_insert_with(|| UnclaimedFinding {
                        finding_id: fact_id.clone(),
                        name: n
                            .properties
                            .get("name")
                            .and_then(crate::foundation::core::Value::as_str)
                            .map(str::to_string),
                        valid_from: n
                            .properties
                            .get("valid_from")
                            .and_then(crate::foundation::core::Value::as_str)
                            .map(str::to_string),
                        reached_via: Vec::new(),
                    });
                entry.reached_via.push(subject.clone());
            }
        }
        for c in candidates.values_mut() {
            c.reached_via.sort();
            c.reached_via.dedup();
        }
        Ok(UnclaimedFindings {
            candidates: candidates.into_values().collect(),
            subjects_examined: subjects.len(),
            unknown_events,
        })
    }
}

/// Open observations a piece of work touched and nobody has claimed — the
/// answer [`VerifyOps::unclaimed_findings_near`] returns.
///
/// It carries its own bounds because a short list and a blind one look
/// identical: `subjects_examined` says how much ground was actually walked, and
/// `unknown_events` names any event id that matched nothing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnclaimedFindings {
    /// The shortlist, one row per open observation nobody has claimed.
    pub candidates: Vec<UnclaimedFinding>,
    /// How many changed subjects were walked to produce it. **Zero here means
    /// the work touched nothing anchored — which is a different fact from
    /// "your work retired nothing" and must not be read as it.**
    pub subjects_examined: usize,
    /// Event ids that name no ChangeEvent. Reported rather than skipped: a typo
    /// would otherwise return an empty shortlist, which reads exactly like a
    /// clean answer.
    pub unknown_events: Vec<String>,
}

/// One open observation that a session's work touched.
///
/// IT IS A CANDIDATE AND NEVER A VERDICT. Nothing here infers that the
/// observation is false — only that the thing it describes has since moved, and
/// that nobody has said either way. The judgement is the author's, and
/// `invalidates` is how they record it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnclaimedFinding {
    pub finding_id: String,
    pub name: Option<String>,
    /// When the observation was taken. The reader needs it to judge whether the
    /// work plausibly postdates it; nothing here compares the two, because a
    /// ChangeEvent's own date is present on only a third of them so far.
    pub valid_from: Option<String>,
    /// Which changed subject(s) reached it — the reason it is on the list, so
    /// the author can see why they are being asked rather than just what.
    pub reached_via: Vec<String>,
}

/// One record's claim that a finding is stale.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InvalidationClaim {
    /// The record that says so — a Constraint, a ChangeEvent, a Decision.
    pub claimed_by: String,
    /// When the invalidating work landed, where the caller said. `None` means
    /// nobody dated it, which is reported rather than treated as recent.
    pub at: Option<String>,
    /// Why it invalidates the finding — what a later reader needs to judge it.
    pub note: Option<String>,
}

/// A finding at least one record claims to have answered.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InvalidatedFinding {
    pub finding_id: String,
    pub finding_type: String,
    /// The finding's own recorded status, UNCHANGED by the claim. A `failing`
    /// check stays `failing` here: this says the verdict is stale, never that
    /// it has turned.
    pub status: Option<String>,
    pub last_run_at: Option<String>,
    /// `Some(true)` = invalidating work lands AFTER the last run, so a re-run is
    /// owed. `Some(false)` = the run already post-dates the work. **`None` =
    /// one side carries no date and nobody can say** — never read as `false`.
    pub rerun_owed: Option<bool>,
    /// How many claims carry no date, so the reader can see what the verdict
    /// above rests on.
    pub undated_claims: usize,
    pub claimed_by: Vec<InvalidationClaim>,
}

// ---- The P4 reconcile (BL-30's M half) -------------------------------------
//
// The last of the three feedback loops: `reconcile_artifacts` asks *does the
// code match the design?* (P3), `reconcile_deployment` asks *does what runs
// match what is declared?* (P5), and this asks *does the recorded outcome
// match what the test run actually reported?* — the exact hole the erosion
// trial fell through, where a status written once was believed forever.
// Adoption's dynamic-analysis step lands here too: run the found system's
// tests, feed the outcomes in, and the graph says where its beliefs diverge.

/// One observed check outcome from a real run. `outcome` is what the runner
/// reported: `passed` / `failed` / `skipped`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObservedVerification {
    pub verification_id: String,
    pub outcome: String,
}

/// One divergence between a recorded status and an observed outcome.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationFinding {
    pub verification_id: String,
    /// What the design believed (`Verification.status`).
    pub declared: String,
    /// What the run reported.
    pub observed: String,
    pub message: String,
    /// What this check verifies — where the divergence lands in the design.
    pub verifies: Vec<String>,
    pub event_id: Option<String>,
}

/// The outcome of a P4 reconcile pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationDriftReport {
    /// Divergences, believed-proven-actually-broken first.
    pub findings: Vec<VerificationFinding>,
    /// Observations matching the recorded status exactly.
    pub agreements: usize,
    /// Observed ids the design has never heard of.
    pub unknown_ids: Vec<String>,
    /// Observations refused by name (an outcome that is not
    /// passed/failed/skipped) — the rest of the batch still processes.
    pub rejected: Vec<String>,
    /// Recorded `passing`/`failing` claims the observation did not cover.
    /// Only under `exhaustive` — a partial run is not evidence of absence.
    pub unobserved: Vec<String>,
    /// Prior events this pass resolved: the divergence is no longer observed.
    pub resolved_events: Vec<String>,
    /// `DriftEvent`s recorded this run (empty unless recording).
    pub recorded_events: Vec<String>,
    /// Seeds for `propagate_from` — the diverging checks' VERIFIES targets.
    pub propagation_seeds: Vec<String>,
}

/// Options for a P4 reconcile pass (the same shape as its two siblings).
#[derive(Debug, Clone, Default)]
pub struct VerifyReconcileOptions {
    pub record_events: bool,
    pub exhaustive: bool,
    pub detected_at: Option<String>,
}

/// declared status ↔ observed outcome agreement.
fn agrees(declared: &str, observed: &str) -> bool {
    matches!(
        (declared, observed),
        ("passing", "passed") | ("failing", "failed") | ("skipped", "skipped")
    )
}

impl DesignGraph {
    /// Compare what a real run reported against what the design records.
    ///
    /// Never edits the design — the answer to a divergence is
    /// [`set_verification_status`](Self::set_verification_status) with what
    /// the run actually said (or fixing the thing under test), confirmed by
    /// the next reconcile, which resolves the event on agreement. Recording
    /// is optional; resolution of this pass's own prior events is not, since
    /// a divergence no longer observed is answered by definition.
    pub fn reconcile_verification(
        &mut self,
        observed: &[ObservedVerification],
        options: &VerifyReconcileOptions,
    ) -> Result<VerificationDriftReport, DynoError> {
        let mut findings: Vec<VerificationFinding> = Vec::new();
        let mut agreements = 0usize;
        let mut unknown_ids = Vec::new();
        let mut rejected = Vec::new();
        let mut covered: Vec<String> = Vec::new();

        for obs in observed {
            if !matches!(obs.outcome.as_str(), "passed" | "failed" | "skipped") {
                rejected.push(format!(
                    "{}: outcome '{}' is not one of passed/failed/skipped",
                    obs.verification_id, obs.outcome
                ));
                continue;
            }
            let Some(ver) = self.get_node(node::VERIFICATION, &obs.verification_id)? else {
                unknown_ids.push(obs.verification_id.clone());
                continue;
            };
            covered.push(obs.verification_id.clone());
            let declared = ver
                .properties
                .get("status")
                .and_then(crate::foundation::core::Value::as_str)
                .unwrap_or("planned")
                .to_string();
            if agrees(&declared, &obs.outcome) {
                agreements += 1;
                continue;
            }
            let verifies: Vec<String> = self
                .outgoing(&obs.verification_id, Some(edge::VERIFIES))?
                .into_iter()
                .map(|e| e.to_id)
                .collect();
            findings.push(VerificationFinding {
                verification_id: obs.verification_id.clone(),
                declared: declared.clone(),
                observed: obs.outcome.clone(),
                message: format!(
                    "'{}' is recorded as '{declared}' and the run reported '{}'",
                    obs.verification_id, obs.outcome
                ),
                verifies,
                event_id: None,
            });
        }

        // Believed-proven-actually-broken is the reflow1 failure in miniature
        // and sorts first; then by id for determinism.
        findings.sort_by_key(|f| {
            (
                u8::from(!(f.declared == "passing" && f.observed == "failed")),
                f.verification_id.clone(),
            )
        });

        let mut unobserved = Vec::new();
        if options.exhaustive {
            for ver in self.scan_nodes(node::VERIFICATION)? {
                if covered.contains(&ver.node_id) {
                    continue;
                }
                let status = ver
                    .properties
                    .get("status")
                    .and_then(crate::foundation::core::Value::as_str)
                    .unwrap_or("planned");
                // Only run-outcome claims can be contradicted by a run that
                // did not include them; planned/skipped/blocked are not
                // claims about a run.
                if status == "passing" || status == "failing" {
                    unobserved.push(ver.node_id.clone());
                }
            }
            unobserved.sort();
        }

        // Resolve prior events for checks this pass observed, where the
        // divergence is no longer among the current findings.
        let current: std::collections::BTreeSet<String> = findings
            .iter()
            .map(|f| verification_event_id(&f.verification_id, &f.declared, &f.observed))
            .collect();
        let mut resolved_events = Vec::new();
        for ev in self.scan_nodes(node::DRIFT_EVENT)? {
            let is_status = ev
                .properties
                .get("drift_type")
                .and_then(crate::foundation::core::Value::as_str)
                == Some("status_mismatch");
            let resolved = ev
                .properties
                .get("resolved")
                .and_then(crate::foundation::core::Value::as_bool)
                .unwrap_or(false);
            if !is_status || resolved || current.contains(&ev.node_id) {
                continue;
            }
            let about_covered = self
                .outgoing(&ev.node_id, Some(edge::DEPENDS_ON))?
                .iter()
                .any(|e| covered.contains(&e.to_id));
            if about_covered {
                let mut props = Props::new().set("resolved", true);
                for (k, v) in &ev.properties {
                    if k != "resolved" {
                        props = props.set(k, v.clone());
                    }
                }
                self.create_node(node::DRIFT_EVENT, &ev.node_id, props)?;
                resolved_events.push(ev.node_id.clone());
            }
        }
        resolved_events.sort();

        let mut recorded_events = Vec::new();
        if options.record_events {
            for f in &mut findings {
                let id = verification_event_id(&f.verification_id, &f.declared, &f.observed);
                if self.get_node(node::DRIFT_EVENT, &id)?.is_none() {
                    let severity = if f.declared == "passing" && f.observed == "failed" {
                        "high" // believed proven, actually broken
                    } else {
                        "medium"
                    };
                    self.create_node(
                        node::DRIFT_EVENT,
                        &id,
                        Props::new()
                            .set("name", format!("{} status drift", f.verification_id))
                            .set("summary", f.message.as_str())
                            .set("drift_type", "status_mismatch")
                            .set("severity", severity)
                            .set_opt("detected_at", options.detected_at.as_deref()),
                    )?;
                    self.create_edge(
                        edge::DEPENDS_ON,
                        node::DRIFT_EVENT,
                        &id,
                        node::VERIFICATION,
                        &f.verification_id,
                        Props::new(),
                    )?;
                }
                recorded_events.push(id.clone());
                f.event_id = Some(id);
            }
        }

        let mut propagation_seeds: Vec<String> = findings
            .iter()
            .flat_map(|f| f.verifies.iter().cloned())
            .collect();
        propagation_seeds.sort();
        propagation_seeds.dedup();
        unknown_ids.sort();
        rejected.sort();

        Ok(VerificationDriftReport {
            findings,
            agreements,
            unknown_ids,
            rejected,
            unobserved,
            resolved_events,
            recorded_events,
            propagation_seeds,
        })
    }
}

/// Deterministic event id: the divergence is "check X reported OBSERVED while
/// the design believed DECLARED", so re-observing the same pair is the same
/// unresolved event and a different pair is a new one — the flapping history
/// stays visible, per axis Z.
fn verification_event_id(verification_id: &str, declared: &str, observed: &str) -> String {
    format!(
        "drift:{:016x}",
        crate::nodes::fnv1a(&format!(
            "status_mismatch|{verification_id}|{declared}|{observed}"
        ))
    )
}

/// Where a capability's evidence actually came from.
///
/// `req:design-the-simulator`, the half a computation can defend. Until
/// 2026-07-27 a check run against a simulation rig and the same check run in the
/// field were indistinguishable to reflow2 — both were simply "passing" — so a
/// capability proven only against a model read exactly like one proven against
/// reality. That is the risk the requirement exists to surface: issues are cheap
/// to fix in simulation and expensive in the field, which is only an argument
/// for simulating first if somebody can still tell the two apart afterwards.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CapabilityEvidence {
    pub capability_id: String,
    pub capability_name: String,
    /// Environments its PASSING checks were performed in, deduplicated, sorted.
    pub proven_in: Vec<String>,
    /// …of those, the ones whose `env_type` is `simulation`.
    pub simulated_environments: Vec<String>,
    /// Passing checks that name no environment at all. Reported rather than
    /// assumed real — an unstated place is unknown, not the field.
    pub unplaced_checks: usize,
    /// Every passing check was performed in a simulated environment, and at
    /// least one said where. The claim worth surfacing.
    pub simulation_only: bool,

    // ---- BL-126 · OVER WHAT the evidence ranges -----------------------------
    /// Parameters some passing check PINNED and no passing check ever swept.
    /// The state BL-126 names: "31 checks, all at one value". Sorted.
    pub pinned_everywhere: Vec<String>,
    /// Parameters some passing check actually VARIED. A parameter here is never
    /// in `pinned_everywhere`, however many other checks pinned it — something
    /// moved it, so the claim is not resting on a single point.
    pub swept: Vec<String>,
    /// Passing checks that state neither what they pinned nor what they swept.
    /// Reported rather than assumed broad: silence about coverage is not
    /// coverage, the same rule `unplaced_checks` applies to place.
    pub unscoped_checks: usize,

    // ---- BL-136 · INDEPENDENT of what --------------------------------------
    /// Passing checks that cannot count as evidence because the capability (or
    /// an artifact realizing it) was calibrated against them. `[CONSUMED — a
    /// fit, not a test]`.
    pub consumed_checks: Vec<ConsumedCheck>,
    /// Passing checks left after the consumed ones are removed.
    pub independent_checks: usize,
    /// At least one passing check survives as independent evidence. False when
    /// every check is a fit — the state where a design agrees with its own
    /// anchor and every status reads green.
    pub independently_verified: bool,
}

/// One passing check that cannot count as independent evidence, because the
/// thing it verifies was calibrated against it (BL-136).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsumedCheck {
    pub verification_id: String,
    /// The evidence node on both sides of the circle — what the target was
    /// fitted to, and what this check rests on.
    pub evidence_id: String,
    /// Why it is consumed, in the words a reader needs: either the check *is*
    /// the evidence, or the check produced it.
    pub reason: String,
}

/// The evidence picture across the design.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EvidenceReport {
    pub capabilities: Vec<CapabilityEvidence>,
    /// Capabilities whose every placed passing check was a simulation.
    pub simulation_only: usize,
    /// Capabilities with passing checks that say nowhere they were run.
    pub with_unplaced_checks: usize,
    /// Capabilities proven only at fixed values of some parameter (BL-126).
    pub narrowly_proven: usize,
    /// Capabilities where no passing check states its input scope at all.
    /// Counted so "0 narrowly proven" can never be read as "everything is
    /// broadly proven" — silence about coverage is not coverage.
    pub with_unscoped_checks: usize,
    /// Capabilities whose every passing check is consumed (BL-136).
    pub not_independently_verified: usize,
    /// Realizing links by whether anyone confirmed the artifact still does what
    /// the target REQUIRES.
    ///
    /// Belongs here rather than in `coverage_report` because it is a fact about
    /// the graph alone: `coverage_report` needs a file sweep supplied by the
    /// caller, and making somebody sweep a tree to learn that nobody has checked
    /// anything would put a cost in front of the one number that should be free.
    /// And deliberately NOT in `loop_status`: every edge starts `unchecked`, so
    /// a large figure that never moves would be exactly the
    /// signal-trained-to-be-ignored failure `req:the-loop-can-say-what-this-session-owes`
    /// was built to end. This is read on demand, not nagged.
    pub conformance: crate::artifact::ConformanceTally,
}

impl DesignGraph {
    /// Report where each capability's passing evidence was actually obtained
    /// (`req:design-the-simulator`).
    ///
    /// **Reports; never ranks.** reflow2 says "proven only in simulation", which
    /// it can defend from `env_type` alone. It deliberately does NOT assert that
    /// lab beats staging beats field: which of those counts as more real is
    /// domain-specific, and an ordering that is wrong somewhere gets worked
    /// around rather than corrected (`dec:report-dont-judge`, and BL-42's lesson
    /// that a detector punishing correct work needs a different question).
    ///
    /// A check naming no environment is counted as **unplaced**, never assumed
    /// to be real — silence is not evidence of the field.
    pub fn evidence_report(&self) -> Result<EvidenceReport, DynoError> {
        let mut capabilities = Vec::new();
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let mut proven_in: Vec<String> = Vec::new();
            let mut simulated: Vec<String> = Vec::new();
            let mut unplaced = 0usize;
            let mut any_passing = false;
            let mut pinned: Vec<String> = Vec::new();
            let mut swept: Vec<String> = Vec::new();
            let mut unscoped_checks = 0usize;
            let mut consumed_checks: Vec<ConsumedCheck> = Vec::new();
            let mut passing_checks = 0usize;

            // BL-136 · what this claim's value was fitted to. The capability's
            // own calibrations plus those of the artifacts realizing it — a
            // fitted constant lives in a file while the check names the
            // capability, so looking only at the capability would miss the
            // commonest shape.
            let mut calibration_sources: Vec<String> = self
                .outgoing(&cap.node_id, Some(edge::CALIBRATED_AGAINST))?
                .into_iter()
                .map(|e| e.to_id)
                .collect();
            for r in self.incoming(&cap.node_id, Some(edge::REALIZES))? {
                for c in self.outgoing(&r.from_id, Some(edge::CALIBRATED_AGAINST))? {
                    calibration_sources.push(c.to_id);
                }
            }
            calibration_sources.sort();
            calibration_sources.dedup();

            for e in self.incoming(&cap.node_id, Some(edge::VERIFIES))? {
                let Some(v) = self.get_node(node::VERIFICATION, &e.from_id)? else {
                    continue;
                };
                let passing = v
                    .properties
                    .get("status")
                    .and_then(crate::foundation::core::Value::as_str)
                    .is_some_and(|s| s == "passing");
                if !passing {
                    continue;
                }
                any_passing = true;
                passing_checks += 1;

                // BL-126 · what this check held fixed and what it varied, read
                // off the EDGE (dec:evidence-scope-on-the-verifies-edge): the
                // same suite can be broad about one claim and narrow about
                // another, and only the edge can say which.
                let edge_pinned = scope_list(&e.properties, "pinned");
                let edge_swept = scope_list(&e.properties, "swept");
                if edge_pinned.is_empty() && edge_swept.is_empty() {
                    unscoped_checks += 1;
                }
                pinned.extend(edge_pinned);
                swept.extend(edge_swept);

                // BL-136 · is this check's evidence the very thing the target
                // was fitted to? Two forms: the check IS the evidence, or the
                // check produced it.
                if calibration_sources.contains(&v.node_id) {
                    consumed_checks.push(ConsumedCheck {
                        verification_id: v.node_id.clone(),
                        evidence_id: v.node_id.clone(),
                        reason: format!(
                            "'{}' was calibrated against this check, so its agreement is a fit, not a test",
                            cap.node_id
                        ),
                    });
                } else if let Some(produced) = self
                    .outgoing(&v.node_id, Some(edge::PRODUCES))?
                    .into_iter()
                    .find(|p| calibration_sources.contains(&p.to_id))
                {
                    consumed_checks.push(ConsumedCheck {
                        verification_id: v.node_id.clone(),
                        evidence_id: produced.to_id.clone(),
                        reason: format!(
                            "'{}' was calibrated against '{}', which this check produced",
                            cap.node_id, produced.to_id
                        ),
                    });
                }

                let places = self.outgoing(&v.node_id, Some(edge::PERFORMED_IN))?;
                if places.is_empty() {
                    unplaced += 1;
                    continue;
                }
                for place in places {
                    let Some(env) = self.get_node(node::ENVIRONMENT, &place.to_id)? else {
                        continue;
                    };
                    if !proven_in.contains(&env.node_id) {
                        proven_in.push(env.node_id.clone());
                    }
                    let simulation = env
                        .properties
                        .get("env_type")
                        .and_then(crate::foundation::core::Value::as_str)
                        .is_some_and(|t| t == "simulation");
                    if simulation && !simulated.contains(&env.node_id) {
                        simulated.push(env.node_id.clone());
                    }
                }
            }
            if !any_passing {
                continue; // nothing proven yet — unverified_capability's question
            }
            proven_in.sort();
            simulated.sort();
            // Only claimable when something said where: all-unplaced is unknown,
            // not simulated.
            let simulation_only = !proven_in.is_empty() && proven_in.len() == simulated.len();

            // A parameter SOMETHING swept is not pinned-everywhere, however
            // many other checks pinned it — the claim no longer rests on one
            // point of that axis, which is the only question BL-126 asks.
            swept.sort();
            swept.dedup();
            let mut pinned_everywhere: Vec<String> =
                pinned.into_iter().filter(|p| !swept.contains(p)).collect();
            pinned_everywhere.sort();
            pinned_everywhere.dedup();

            consumed_checks.sort_by(|a, b| a.verification_id.cmp(&b.verification_id));
            let independent_checks = passing_checks.saturating_sub(consumed_checks.len());
            let independently_verified = independent_checks > 0;

            capabilities.push(CapabilityEvidence {
                capability_id: cap.node_id.clone(),
                capability_name: cap
                    .properties
                    .get("name")
                    .and_then(crate::foundation::core::Value::as_str)
                    .unwrap_or(&cap.node_id)
                    .to_string(),
                proven_in,
                simulated_environments: simulated,
                unplaced_checks: unplaced,
                simulation_only,
                pinned_everywhere,
                swept,
                unscoped_checks,
                consumed_checks,
                independent_checks,
                independently_verified,
            });
        }
        capabilities.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let simulation_only = capabilities.iter().filter(|c| c.simulation_only).count();
        let with_unplaced_checks = capabilities
            .iter()
            .filter(|c| c.unplaced_checks > 0)
            .count();
        let narrowly_proven = capabilities
            .iter()
            .filter(|c| !c.pinned_everywhere.is_empty())
            .count();
        let with_unscoped_checks = capabilities
            .iter()
            .filter(|c| c.unscoped_checks > 0)
            .count();
        let not_independently_verified = capabilities
            .iter()
            .filter(|c| !c.independently_verified)
            .count();
        Ok(EvidenceReport {
            capabilities,
            simulation_only,
            with_unplaced_checks,
            narrowly_proven,
            with_unscoped_checks,
            not_independently_verified,
            conformance: self.conformance_tally()?,
        })
    }
}

/// Read one of the two comma-separated scope lists off a `VERIFIES` edge.
///
/// Flat storage because the schema has no list type
/// (`dec:evidence-scope-on-the-verifies-edge`); empty entries are dropped and
/// each name is trimmed, so `"seed, , order"` is two parameters rather than
/// three. An absent property and an empty string are the same thing — unstated.
fn scope_list(
    props: &std::collections::HashMap<String, crate::foundation::core::Value>,
    key: &str,
) -> Vec<String> {
    props
        .get(key)
        .and_then(crate::foundation::core::Value::as_str)
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
