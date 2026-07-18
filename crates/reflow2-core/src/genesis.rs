//! GENESIS — bootstrap a design graph from an opening brief (surface-plan.md, SP-5).
//!
//! GENESIS is the coherence loop's front door: it turns "here's my idea" into a
//! seeded graph so the loop has something to work with and the first DETECT round
//! is productive. It is a deliberate, **guarded, one-shot** process (like the
//! predecessor's `ir2-init`): it refuses to clobber an already-initialized graph,
//! and it plants the axis-Z temporal anchor everything else hangs off.
//!
//! **This op is the thin deterministic half.** It guarantees the invariants — the
//! Project scaffold, the genesis Epoch, and a next-steps checklist — but does *no*
//! LLM extraction. Expanding the brief into Requirements/Capabilities is done
//! agent-natively by the GENESIS skill through the existing tools (add_*, satisfies,
//! detect_gaps), so GENESIS needs no LLM backend and sidesteps the mutating-extraction
//! rollback problem (deferred to SP-3b).
//!
//! ## How much to seed
//!
//! The skill seeds **P0 (Requirements) + P1 (Capabilities + `satisfies`) only —
//! stopping before P2 (Components)**. That is deliberate: DETECT's phase-coverage
//! logic then fires `concept_without_design` as the first gap, which is exactly the
//! productive next question ("how should this be structured?"). Genesis itself only
//! plants the Project + Epoch; the report tells the agent what to seed next.

use dynograph_core::DynoError;
use dynograph_storage::StoredNode;

use crate::graph::DesignGraph;
use crate::nodes::{Props, node};
use crate::temporal::EpochType;

/// Id of the Baseline epoch GENESIS plants as the axis-Z anchor.
pub const GENESIS_EPOCH_ID: &str = "epoch:genesis";

/// Inputs for [`DesignGraph::genesis`]. Serializable: it crosses the MCP boundary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenesisOptions {
    /// Stable Project id (e.g. `proj:softball`).
    pub project_id: String,
    /// Project name.
    pub name: String,
    /// Optional domain hint (software / hardware / document / …).
    #[serde(default)]
    pub domain: Option<String>,
    /// Optional one-line "what success looks like".
    #[serde(default)]
    pub objective: Option<String>,
    /// Project mode: `flexible` (default) or `rigid`. Left unset → schema default.
    #[serde(default)]
    pub mode: Option<String>,
    /// Allow bootstrapping over an existing Project instead of a no-op.
    #[serde(default)]
    pub rescan: bool,
}

/// What GENESIS did (or found already present). Serializable for the tool result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GenesisReport {
    /// The Project id.
    pub project_id: String,
    /// The genesis Epoch id (the temporal anchor).
    pub epoch_id: String,
    /// True when a Project already existed and `rescan` was not set (no clobber).
    pub already_initialized: bool,
    /// True when this call created the scaffold.
    pub created: bool,
    /// The Project's mode (`flexible` / `rigid`) as stored.
    pub project_mode: String,
    /// The checklist the agent should follow next (the skill drives these).
    pub next_steps: Vec<String>,
}

/// The next-steps checklist GENESIS hands back — the P0/P1 seeding rule, the
/// deployment lesson, and the hand-off to DETECT.
fn genesis_next_steps() -> Vec<String> {
    vec![
        "Extract the brief into P0/P1 only: add_requirement + add_capability + satisfies \
         (link each to the Project with contains). Do NOT create Components yet — let DETECT \
         surface the structure gap."
            .to_string(),
        "Capture deployment/consumer context as Requirements: target platform(s), the driving \
         agent, how it is invoked, and where the design persists."
            .to_string(),
        "Run detect_gaps for the first round (expect concept_without_design), then use \
         gap_to_prompt to ask the user each surfaced gap."
            .to_string(),
    ]
}

impl DesignGraph {
    /// Bootstrap the graph: plant the Project scaffold and the genesis Epoch.
    ///
    /// Guarded and idempotent: if a Project already exists and `rescan` is false,
    /// this is a no-op that reports `already_initialized` (never a silent clobber).
    /// Deterministic — no LLM; the brief is expanded by the GENESIS skill via the
    /// write/DETECT tools afterward (see the module docs).
    pub fn genesis(&mut self, opts: GenesisOptions) -> Result<GenesisReport, DynoError> {
        // Guard: refuse to re-bootstrap over an existing design unless asked.
        if self.count_nodes(node::PROJECT)? > 0 && !opts.rescan {
            let mode = existing_project_mode(self)?;
            return Ok(GenesisReport {
                project_id: opts.project_id,
                epoch_id: GENESIS_EPOCH_ID.to_string(),
                already_initialized: true,
                created: false,
                project_mode: mode,
                next_steps: vec![
                    "This graph is already initialized. Skip scaffolding and run detect_gaps \
                     to see what to work on next."
                        .to_string(),
                ],
            });
        }

        // Scaffold the Project (schema applies mode=flexible, status=active when unset).
        let mut props = Props::new().set("name", opts.name.as_str());
        if let Some(domain) = opts.domain.as_deref() {
            props = props.set("domain", domain);
        }
        if let Some(objective) = opts.objective.as_deref() {
            props = props.set("objective", objective);
        }
        if let Some(mode) = opts.mode.as_deref() {
            props = props.set("mode", mode);
        }
        let project = self.create_node(node::PROJECT, &opts.project_id, props)?;

        // Plant the axis-Z anchor: a Baseline epoch. Ignore an already-present
        // epoch on rescan so genesis stays idempotent.
        if self
            .get_node(node::DESIGN_EPOCH, GENESIS_EPOCH_ID)?
            .is_none()
        {
            self.add_epoch(GENESIS_EPOCH_ID, "Genesis", EpochType::Baseline, 0)?;
        }

        Ok(GenesisReport {
            project_id: opts.project_id,
            epoch_id: GENESIS_EPOCH_ID.to_string(),
            already_initialized: false,
            created: true,
            project_mode: project_mode_of(&project),
            next_steps: genesis_next_steps(),
        })
    }
}

/// Read the `mode` property off a Project node (defaults to `flexible`).
fn project_mode_of(project: &StoredNode) -> String {
    project
        .properties
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("flexible")
        .to_string()
}

/// Find the existing Project's mode for the already-initialized report.
fn existing_project_mode(g: &DesignGraph) -> Result<String, DynoError> {
    let projects = g.scan_nodes(node::PROJECT)?;
    Ok(projects
        .first()
        .map(project_mode_of)
        .unwrap_or_else(|| "flexible".to_string()))
}
