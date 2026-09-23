//! Has a real result ever come back into this design?
//!
//! Step 3 of the ordering Anthony approved on 2026-09-22, and the answer to his
//! question on `dec:idea-can-reflow2-say-whether-a-design-has-ever-closed-its-own-loop`:
//! *"if a user designs a house and never comes back and says that it failed an
//! inspection, that is on the user and it is their choice — the design should
//! show that it never completed its loop."*
//!
//! # What it reads
//!
//! The ENTRY POINT of the founding chain — testing discovers a failure → root
//! cause → the change reaches the contracts → every requirement still holds.
//! Every later link is fed by the first, and
//! `fact:every-link-of-the-founding-chain-exists-as-a-tool-and-the-chain-has-never-been-walked-on-reflow2-itself`
//! measured that the first had never fired here: 311 checks reading `passing`
//! and none ever recorded failing. Which is indistinguishable, from the status
//! field, from a design whose every check has been run and passed.
//!
//! The evidence that a run came back, all of it already in the graph:
//! - the stamp `reconcile_verification` leaves on each check it compared
//!   (`last_reconciled_outcome`, added with this reading);
//! - a `status_mismatch` DriftEvent, which only a compared run can mint and
//!   which predates the stamp.
//!
//! # What it does NOT do
//!
//! It never judges the checks (`dec:non-goal-reflow2-does-not-judge-whether-a-check-is-meaningful`)
//! and never says the user SHOULD run them. It states a fact about the design —
//! that nothing has come back — and leaves the disposition with the owner. The
//! cause and contract links of the chain have their own readings
//! (`repair_report`, `seam_coverage`) and are not recomputed here, because this
//! rides on `loop_status`, which is promised cheap.

use crate::foundation::core::{DynoError, Value};
use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// Where a design stands on feeding real results back. Computed, never stored.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopClosureState {
    /// No check claims an outcome, so there is nothing a run could have
    /// confirmed or refuted yet. Not debt: a design still being captured.
    NothingToClose,
    /// Checks claim outcomes and no real run has ever been compared to any of
    /// them. The house nobody came back from inspecting.
    NeverClosed,
    /// Real runs have come back, and none has ever reported a failure.
    ClosedWithoutAFailure,
    /// At least one real run reported a failure — the founding chain's entry
    /// point has fired at least once.
    AFailureCameBack,
}

/// The loop-closure reading. See the module docs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LoopClosure {
    pub state: LoopClosureState,
    /// Live checks — every Verification except `superseded` ones.
    pub checks: usize,
    /// Of those, how many claim a run outcome (`passing` or `failing`).
    pub claiming_an_outcome: usize,
    /// Checks a real run has ever been compared against.
    pub compared_to_a_run: usize,
    /// Checks for which a real run fed back through the reconcile has reported
    /// a failure. A check merely TYPED as `failing` is not counted.
    pub failures_fed_back: usize,
    /// The most recent dated comparison, if any comparison was dated.
    pub last_compared_at: Option<String>,
    /// One sentence a person can read, stating the fact and nothing more.
    pub summary: String,
}

impl DesignGraph {
    /// Compute the [`LoopClosure`] reading.
    pub fn loop_closure(&self) -> Result<LoopClosure, DynoError> {
        let str_prop = |n: &crate::foundation::store::StoredNode, k: &str| {
            n.properties
                .get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };

        // A status_mismatch event can only be minted by a compared run, so the
        // check it names was compared even if it carries no stamp.
        let mut mismatched: std::collections::BTreeSet<String> = Default::default();
        for ev in self.scan_nodes(node::DRIFT_EVENT)? {
            if str_prop(&ev, "drift_type").as_deref() != Some("status_mismatch") {
                continue;
            }
            for e in self.outgoing(&ev.node_id, Some(edge::DEPENDS_ON))? {
                mismatched.insert(e.to_id);
            }
        }

        let (mut checks, mut claiming, mut compared, mut failed) = (0, 0, 0, 0);
        let mut last: Option<String> = None;
        for v in self.scan_nodes(node::VERIFICATION)? {
            let status = str_prop(&v, "status").unwrap_or_else(|| "planned".into());
            if status == "superseded" {
                continue;
            }
            checks += 1;
            if status == "passing" || status == "failing" {
                claiming += 1;
            }
            let outcome = str_prop(&v, "last_reconciled_outcome");
            if outcome.is_some() || mismatched.contains(&v.node_id) {
                compared += 1;
            }
            // RUN evidence only. A check somebody TYPED as `failing` is a
            // claim, not a result that came back, and counting it would let
            // the entry point read as fired on a design that never ran a thing.
            if outcome.as_deref() == Some("failed") || str_prop(&v, "last_failed_run_at").is_some()
            {
                failed += 1;
            }
            if let Some(at) = str_prop(&v, "last_reconciled_at")
                && last.as_ref().is_none_or(|l| at > *l)
            {
                last = Some(at);
            }
        }

        let state = if failed > 0 {
            LoopClosureState::AFailureCameBack
        } else if compared > 0 {
            LoopClosureState::ClosedWithoutAFailure
        } else if claiming > 0 {
            LoopClosureState::NeverClosed
        } else {
            LoopClosureState::NothingToClose
        };

        let when = last
            .as_deref()
            .map(|d| format!(", most recently on {d}"))
            .unwrap_or_default();
        let summary = match state {
            LoopClosureState::NothingToClose => {
                "No check claims a result yet, so there is nothing a real run could have \
                 confirmed or refuted."
                    .to_string()
            }
            LoopClosureState::NeverClosed => format!(
                "{claiming} check(s) claim a result and no real run has ever been fed back \
                 against any of them — every pass and fail this design holds is what \
                 somebody recorded, never what a run was compared to."
            ),
            LoopClosureState::ClosedWithoutAFailure => format!(
                "Real runs have been compared against {compared} of {checks} check(s){when}, \
                 and none has ever reported a failure."
            ),
            LoopClosureState::AFailureCameBack => format!(
                "Real runs have been compared against {compared} of {checks} check(s){when}; \
                 {failed} check(s) have had a failure come back."
            ),
        };

        Ok(LoopClosure {
            state,
            checks,
            claiming_an_outcome: claiming,
            compared_to_a_run: compared,
            failures_fed_back: failed,
            last_compared_at: last,
            summary,
        })
    }
}
