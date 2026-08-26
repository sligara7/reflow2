//! The work a session did BY HAND that reflow2 already serves — the negative
//! space.
//!
//! `req:a-session-says-what-it-did-by-hand-that-reflow2-already-serves`.
//!
//! ⭐ WHY THIS IS A STRONGER SIGNAL THAN AN ABSENT CALL. Every other measurement
//! reflow2 has of its own adoption looks at what a session DID with it, and
//! `dec:bl-155` states the consequence outright: it measured 40 of 132 tools
//! never called and **cannot tell unused from unreachable**. Hand-rolled work
//! discriminates between them because it carries INTENT — a session that wrote a
//! script to do X proves somebody wanted X, at a moment, badly enough to build
//! it. A zero in a usage table can never show that.
//!
//! ⚠️ TWO STANDING OBJECTIONS, NEITHER RESOLVED BY BUILDING THIS.
//!
//! 1. It depends on the agent having NOTICED the tool existed — the same blind
//!    spot it is trying to see. The requirement says so in its own words.
//! 2. `dec:how-should-reflow2-log-its-own-usage` argues a tool is the wrong
//!    shape: *"a tool must be CALLED, and the population most worth measuring is
//!    precisely the one least likely to call it."* That is right, and here it is
//!    also unavoidable — the server observes CALLS, and work done by hand happens
//!    where the server cannot see. Asking is the only route that exists.
//!
//! 🛑 THE REPORT STAYS LOCAL AND MUST NEVER BECOME TELEMETRY.
//! `req:telemetry-carries-usage-never-design-content` governs what LEAVES a
//! machine: *"log the verb, never the object"*, and *"a free-text field anywhere
//! in the payload defeats this no matter what the policy says, because the next
//! contributor will put something useful in it."* `what` IS that field — it
//! names the user's domain in their own words. It belongs in the user's own
//! graph and must not be lifted into any telemetry payload. This paragraph
//! exists because the next contributor wiring telemetry will be looking for
//! useful fields and this is one.

use crate::DesignGraph;
use crate::foundation::core::{DynoError, Value};
use crate::nodes::{Props, fnv1a, node};

/// What the session concluded about why the work was done by hand.
///
/// ⭐ A CLOSED SET, AND THAT IS THE POINT. The signal's whole value is that it
/// separates "the tool is MISSING" from "the tool exists and was not FOUND" —
/// the distinction `dec:bl-155` says reflow2 cannot otherwise make. A free-text
/// diagnosis would let that distinction rot one report at a time, so an unknown
/// value is REFUSED and the refusal names what would have worked.
pub const DIAGNOSES: &[&str] = &[
    // No reflow2 tool does this. The signal names a MISSING tool.
    "tool_missing",
    // A tool does it and the session did not find it. The signal names a
    // DISCOVERABILITY failure, which is a different repair entirely.
    "tool_not_found",
    // A tool exists and was reached for and would not do it — a refusal, a
    // limit, an argument it would not take.
    "tool_refused",
    // The session did it by hand and cannot say which of the above it was.
    // Kept deliberately: forcing a guess would corrupt the three above, and an
    // honest "I do not know" is worth more than a confident wrong bucket.
    "unknown",
];

/// One recorded piece of hand-rolled work.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManualWork {
    pub id: String,
    /// The SHAPE of the work, in the session's own words.
    pub what: String,
    /// One of [`DIAGNOSES`].
    pub diagnosis: String,
    /// The served tool that should have done it, where the session can name one.
    pub reflow2_tool: Option<String>,
    /// The date it was recorded, when the caller supplied one.
    pub at: Option<String>,
}

impl DesignGraph {
    /// Record work a session did by hand that reflow2 already serves, or should.
    ///
    /// Returns the id of the record. The id is DERIVED FROM `what`, so the same
    /// work reported twice is one record rather than two — sessions repeat, and a
    /// count that inflated with re-tellings would stop meaning "how many distinct
    /// things did people build by hand".
    pub fn report_manual_work(
        &mut self,
        what: &str,
        diagnosis: &str,
        reflow2_tool: Option<&str>,
        at: Option<&str>,
    ) -> Result<String, DynoError> {
        if !DIAGNOSES.contains(&diagnosis) {
            return Err(DynoError::Validation {
                node_type: node::TEMPORAL_FACT.to_string(),
                property: "diagnosis".to_string(),
                message: format!(
                    "unknown diagnosis {diagnosis:?} — a report says WHY the work was \
                     hand-rolled, and the set is closed on purpose so the distinction cannot rot \
                     one report at a time. Use one of: {}",
                    DIAGNOSES.join(", ")
                ),
            });
        }

        // ⚠️ THE TOOL NAME IS NOT VALIDATED HERE, AND CANNOT BE. `tool_not_found`
        // asserts the tool EXISTS and the session missed it, so a name that
        // matches nothing means the diagnosis is really `tool_missing` — but the
        // CORE does not know what the MCP surface serves, and inventing a list
        // here would be a second copy of the tool surface maintained by hand,
        // which is the defect class this project spent 2026-08-26 fixing three
        // times. The check lives at the tool, where the router already knows its
        // own names, and is pinned there.

        let id = format!("fact:manual-work-{:016x}", fnv1a(what));
        let mut props = Props::new()
            // The design this observation is about. A TemporalFact whose
            // `subject_id` resolves to nothing is invisible to
            // `unclaimed_findings` — nine of them were found and repaired on
            // 2026-08-26 for exactly that reason — so it is read from the graph
            // rather than assembled from a convention.
            .set(
                "subject_id",
                self.scan_nodes(node::PROJECT)?
                    .first()
                    .map(|p| p.node_id.clone())
                    .unwrap_or_default(),
            )
            .set("fact_type", "manual_work")
            .set("statement", what)
            .set("basis", "measured")
            .set(
                "value",
                serde_json::json!({
                    "diagnosis": diagnosis,
                    "reflow2_tool": reflow2_tool,
                })
                .to_string(),
            );
        if let Some(a) = at {
            props = props.set("valid_from", a);
        }
        self.upsert_node(node::TEMPORAL_FACT, &id, props)?;
        Ok(id)
    }

    /// Every piece of hand-rolled work this design has recorded.
    ///
    /// ⭐ THE LEG THAT KILLS VOCABULARY WHEN IT IS MISSING. The 2026-08-26
    /// surface audit found five types carrying all three legs and still unused —
    /// `DECOMPOSES` reached ZERO edges while shipping a detector for them. A
    /// record nothing reads back is a record nobody writes twice, so the reader
    /// ships WITH the writer rather than after it.
    pub fn manual_work_report(&self) -> Result<Vec<ManualWork>, DynoError> {
        let mut out = Vec::new();
        for n in self.scan_nodes(node::TEMPORAL_FACT)? {
            if n.properties.get("fact_type").and_then(Value::as_str) != Some("manual_work") {
                continue;
            }
            // `value` holds JSON as a STRING, so it is parsed with serde_json
            // rather than read as a property: the property Value is the store's
            // own scalar type and has no object access.
            let v: serde_json::Value = n
                .properties
                .get("value")
                .and_then(Value::as_str)
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            out.push(ManualWork {
                id: n.node_id.clone(),
                what: n
                    .properties
                    .get("statement")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                diagnosis: v
                    .get("diagnosis")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                reflow2_tool: v
                    .get("reflow2_tool")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                at: n
                    .properties
                    .get("valid_from")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
}
