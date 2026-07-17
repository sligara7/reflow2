//! INGEST — freeform design material → schema-validated graph content
//! (docs/extraction-plan.md). The **CHANGE** step's content path: how a brief,
//! spec, review note, or an agent's own reasoning becomes typed nodes and edges.
//!
//! The pipeline mirrors storyflow's battle-tested shape:
//!
//! ```text
//! input → EXTRACT (multi-pass, phase-gated) → INTEGRATE (typed, provenance-stamped)
//! ```
//!
//! Every LLM-reasoning pass goes through the pluggable [`LlmBackend`] seam, so
//! INGEST runs against the [`MockLlmBackend`](crate::llm::MockLlmBackend) with no
//! provider. The storyflow disciplines that carry over and are enforced here:
//!
//! - **One shared call helper** ([`run_pass`]) — model call + JSON parse + error
//!   enveloping live in one place (discipline 1).
//! - **Never cascade-fail** — a pass that errors fills only its own slot with an
//!   empty default and a recorded [`PassError`]; siblings survive (discipline 2/4).
//! - **The discovery gate** — a classifier answers "what content is present?" as
//!   orthogonal booleans, so phase-2 passes don't hunt for structure that isn't
//!   there (discipline 5/6).
//! - **Roster threading** — phase-2/3 passes that emit edges get the phase-1
//!   rosters (id + name) so they reference real ids, not invented ones
//!   (discipline 11).
//! - **No silent drops** — an edge whose endpoint wasn't created (a phantom ref)
//!   or fails schema validation is recorded in [`IngestReport::dropped_edges`]
//!   with a reason; a node that fails validation is recorded in `warnings`;
//!   `status` goes `Partial`. Nothing is silently swallowed.
//! - **Provenance** — everything created is linked from one `Fragment` via
//!   `YIELDED`, stamped with how it entered the graph.
//!
//! Deferred (noted so they're not mistaken for done): graph-informed
//! **fuzzy/vector resolution** and *matched-evolved* snapshotting (need an
//! embedding generator — a stub even in storyflow, and tied to the deferred
//! interaction-surface decision), the **SME** augmentation pass, real
//! parallelism (passes run sequentially here), per-fragment metrics, and the
//! remaining passes (flows, actors, interfaces, decisions, artifacts, resources,
//! dependencies, inference, dimensions, changes). This increment implements the
//! spine: project/requirements/constraints/capabilities → components → satisfies.

use std::collections::HashMap;

use dynograph_core::DynoError;
use serde::Deserialize;

use crate::graph::DesignGraph;
use crate::llm::{LlmBackend, LlmRequest, complete_json};
use crate::nodes::{Props, edge, node};

// ---- Extraction output shapes (strict JSON per pass) -----------------------

#[derive(Debug, Default, Deserialize)]
struct ProjectIntent {
    project: Option<ExtractedProject>,
}

#[derive(Debug, Deserialize)]
struct ExtractedProject {
    id: String,
    name: String,
    #[serde(default)]
    objective: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    domain: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RequirementsOut {
    #[serde(default)]
    requirements: Vec<ExtractedRequirement>,
}

#[derive(Debug, Deserialize)]
struct ExtractedRequirement {
    id: String,
    name: String,
    statement: String,
    #[serde(default)]
    priority: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ConstraintsOut {
    #[serde(default)]
    constraints: Vec<ExtractedConstraint>,
}

#[derive(Debug, Deserialize)]
struct ExtractedConstraint {
    id: String,
    name: String,
    statement: String,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CapabilitiesOut {
    #[serde(default)]
    capabilities: Vec<ExtractedCapability>,
}

#[derive(Debug, Deserialize)]
struct ExtractedCapability {
    id: String,
    name: String,
    description: String,
}

/// The discovery gate — orthogonal booleans over what design content is present.
/// Anchor-required: `true` only when a concrete instance is named (see the doc).
///
/// Only `components` gates a pass in this increment. The other fields are the
/// classifier's full contract and gate phase-2 passes **not yet built** (flows,
/// interfaces, actors, decisions, artifacts, resources). They are kept — rather
/// than narrowing the classifier — so the deferral is visible; it is recorded as
/// Deferred in `docs/requirements-coverage.md`. `#[allow(dead_code)]` marks the
/// gap explicitly instead of silently.
#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct Discovery {
    #[serde(default)]
    components: bool,
    #[serde(default)]
    interfaces: bool,
    #[serde(default)]
    actors: bool,
    #[serde(default)]
    decisions: bool,
    #[serde(default)]
    artifacts: bool,
    #[serde(default)]
    verifications: bool,
    #[serde(default)]
    flows: bool,
    #[serde(default)]
    resources: bool,
}

#[derive(Debug, Default, Deserialize)]
struct ComponentsOut {
    #[serde(default)]
    components: Vec<ExtractedComponent>,
}

#[derive(Debug, Deserialize)]
struct ExtractedComponent {
    id: String,
    name: String,
    purpose: String,
    /// Capability ids (from the phase-1 roster) allocated to this component.
    #[serde(default)]
    allocated_capability_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SatisfiesOut {
    #[serde(default)]
    satisfies: Vec<SatisfiesEdge>,
}

#[derive(Debug, Deserialize)]
struct SatisfiesEdge {
    capability_id: String,
    requirement_id: String,
}

// ---- Report shapes ---------------------------------------------------------

/// A pass that failed to produce usable output (enveloped, not fatal).
#[derive(Debug, Clone)]
pub struct PassError {
    /// The pass name (e.g. `"requirements"`).
    pub pass: &'static str,
    /// The error (backend or parse).
    pub error: String,
}

/// An edge that could not be created, with the reason (no silent drops).
#[derive(Debug, Clone)]
pub struct DroppedEdge {
    /// Edge type.
    pub edge_type: String,
    /// Source id.
    pub from_id: String,
    /// Target id.
    pub to_id: String,
    /// Why it was dropped.
    pub reason: String,
}

/// Whether the ingest ran fully clean or degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestStatus {
    /// All passes and integrations succeeded.
    Ok,
    /// At least one pass errored, node failed validation, or edge was dropped.
    Partial,
}

/// The outcome of an ingest.
#[derive(Debug, Clone)]
pub struct IngestReport {
    /// The provenance Fragment created for this input.
    pub fragment_id: String,
    /// Nodes created (or updated) this run.
    pub nodes_created: usize,
    /// Edges created this run.
    pub edges_created: usize,
    /// Pass-level failures (enveloped; siblings survived).
    pub pass_errors: Vec<PassError>,
    /// Node-level problems (e.g. a bad enum), recorded not fatal.
    pub warnings: Vec<String>,
    /// Edges dropped rather than emitted as phantoms.
    pub dropped_edges: Vec<DroppedEdge>,
    /// Overall status.
    pub status: IngestStatus,
}

/// Options for an ingest run.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// Id for the provenance Fragment.
    pub fragment_id: String,
    /// Title for the provenance Fragment.
    pub fragment_title: String,
    /// How this content entered the graph (`authored`/`planned`/`imported`/…).
    pub provenance: String,
    /// The active `DesignEpoch` this ingest happens in, if any (wired via
    /// `OCCURS_DURING`).
    pub epoch_id: Option<String>,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            fragment_id: "frag:ingest".to_string(),
            fragment_title: "Ingested design input".to_string(),
            provenance: "authored".to_string(),
            epoch_id: None,
        }
    }
}

/// Run one extraction pass through the shared LLM seam. On any failure it
/// records a [`PassError`] and returns `T::default()` (empty) — one bad pass
/// never cancels the others (discipline 2).
fn run_pass<T: serde::de::DeserializeOwned + Default>(
    backend: &dyn LlmBackend,
    pass: &'static str,
    prompt: String,
    errors: &mut Vec<PassError>,
) -> T {
    let request = LlmRequest::new(prompt).with_system(
        "You extract structured design entities from freeform input and return ONLY \
         strict JSON in the shape the instruction specifies. Lists are always arrays.",
    );
    match complete_json::<T>(backend, &request) {
        Ok(value) => value,
        Err(e) => {
            errors.push(PassError {
                pass,
                error: e.to_string(),
            });
            T::default()
        }
    }
}

/// Build a pass prompt with the (unchanging) input FIRST for prefix-cache
/// sharing (discipline 7), then the pass instruction, then any roster context.
fn pass_prompt(input: &str, instruction: &str, roster: Option<&str>) -> String {
    let mut p = format!("INPUT:\n{input}\n\n{instruction}");
    if let Some(r) = roster {
        p.push_str("\n\nKNOWN ENTITIES (reference these ids exactly):\n");
        p.push_str(r);
    }
    p
}

/// A compact `id — name` roster for threading into edge passes.
fn roster<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    items
        .into_iter()
        .map(|(id, name)| format!("- {id} — {name}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl DesignGraph {
    /// EXTRACT freeform `input` into the graph and INTEGRATE it, stamped with
    /// provenance. Runs against any [`LlmBackend`]. See the module docs for the
    /// disciplines enforced and what's deferred.
    pub fn ingest(
        &mut self,
        input: &str,
        options: &IngestOptions,
        backend: &dyn LlmBackend,
    ) -> Result<IngestReport, DynoError> {
        let mut errors = Vec::new();

        // ---- EXTRACT · Phase 1 (always run, read input only) ----
        let project = run_pass::<ProjectIntent>(
            backend,
            "project_intent",
            pass_prompt(
                input,
                r#"[pass:project_intent] Return JSON {"project":{"id":"proj:<slug>","name":"...","objective":"...","mode":"flexible|rigid","domain":"..."}}."#,
                None,
            ),
            &mut errors,
        )
        .project;
        let requirements = run_pass::<RequirementsOut>(
            backend,
            "requirements",
            pass_prompt(
                input,
                r#"[pass:requirements] Return JSON {"requirements":[{"id":"req:<slug>","name":"...","statement":"...","priority":"low|medium|high|critical"}]}."#,
                None,
            ),
            &mut errors,
        )
        .requirements;
        let constraints = run_pass::<ConstraintsOut>(
            backend,
            "constraints",
            pass_prompt(
                input,
                r#"[pass:constraints] Return JSON {"constraints":[{"id":"con:<slug>","name":"...","statement":"...","category":"technical|business|operational|physical|regulatory|budget|schedule"}]}."#,
                None,
            ),
            &mut errors,
        )
        .constraints;
        let capabilities = run_pass::<CapabilitiesOut>(
            backend,
            "capabilities",
            pass_prompt(
                input,
                r#"[pass:capabilities] Return JSON {"capabilities":[{"id":"cap:<slug>","name":"...","description":"..."}]}."#,
                None,
            ),
            &mut errors,
        )
        .capabilities;
        let discovery = run_pass::<Discovery>(
            backend,
            "discovery",
            pass_prompt(
                input,
                r#"[pass:discovery] Classify what design content is present. Return JSON with booleans {"components":bool,"interfaces":bool,"actors":bool,"decisions":bool,"artifacts":bool,"verifications":bool,"flows":bool,"resources":bool}. Return true ONLY when a concrete named instance is described acting as a unit — not when merely alluded to."#,
                None,
            ),
            &mut errors,
        );

        // ---- EXTRACT · Phase 2 (gated by discovery, roster-threaded) ----
        let cap_roster = roster(
            capabilities
                .iter()
                .map(|c| (c.id.as_str(), c.name.as_str())),
        );
        let components = if discovery.components {
            run_pass::<ComponentsOut>(
                backend,
                "components",
                pass_prompt(
                    input,
                    r#"[pass:components] Return JSON {"components":[{"id":"cmp:<slug>","name":"...","purpose":"...","allocated_capability_ids":["cap:..."]}]}. Allocate capabilities only from the known ids."#,
                    Some(&cap_roster),
                ),
                &mut errors,
            )
            .components
        } else {
            Vec::new()
        };

        // ---- EXTRACT · Phase 3 (edge passes over rosters) ----
        let req_roster = roster(
            requirements
                .iter()
                .map(|r| (r.id.as_str(), r.name.as_str())),
        );
        let satisfies = if !capabilities.is_empty() && !requirements.is_empty() {
            run_pass::<SatisfiesOut>(
                backend,
                "satisfies",
                pass_prompt(
                    input,
                    r#"[pass:satisfies] Which capability satisfies which requirement? Return JSON {"satisfies":[{"capability_id":"cap:...","requirement_id":"req:..."}]} using only known ids."#,
                    Some(&format!("Capabilities:\n{cap_roster}\n\nRequirements:\n{req_roster}")),
                ),
                &mut errors,
            )
            .satisfies
        } else {
            Vec::new()
        };

        // ---- INTEGRATE (typed, provenance-stamped) ----
        let mut warnings = Vec::new();
        let mut dropped_edges = Vec::new();
        let mut created_ids: HashMap<String, &'static str> = HashMap::new();
        let mut nodes_created = 0usize;
        let mut edges_created = 0usize;

        // Provenance fragment first.
        let mut frag_props = Props::new()
            .set("title", options.fragment_title.as_str())
            .set("fragment_type", "design")
            .set("provenance", options.provenance.as_str());
        // provenance must be a valid enum value; fall back loudly if not.
        if !PROVENANCE_VALUES.contains(&options.provenance.as_str()) {
            warnings.push(format!(
                "provenance '{}' not a schema value; using 'authored'",
                options.provenance
            ));
            frag_props = frag_props.set("provenance", "authored");
        }
        self.create_node(node::FRAGMENT, &options.fragment_id, frag_props)?;
        nodes_created += 1;
        if let Some(epoch) = &options.epoch_id {
            // Best-effort provenance-in-time; a bad epoch id is a warning, not fatal.
            if let Err(e) = self.create_edge(
                edge::OCCURS_DURING,
                node::FRAGMENT,
                &options.fragment_id,
                node::DESIGN_EPOCH,
                epoch,
                Props::new(),
            ) {
                warnings.push(format!("OCCURS_DURING epoch '{epoch}': {e}"));
            }
        }

        // Helper closure can't borrow self mutably twice, so integrate inline.
        // Nodes: project, requirements, constraints, capabilities, components.
        if let Some(p) = &project {
            let mut props = Props::new().set("name", p.name.as_str());
            props = props.set_opt("objective", p.objective.as_deref());
            props = props.set_opt("domain", p.domain.as_deref());
            if let Some(m) = p.mode.as_deref()
                && (m == "flexible" || m == "rigid")
            {
                props = props.set("mode", m);
            }
            self.integrate_node(
                node::PROJECT,
                &p.id,
                props,
                &options.fragment_id,
                &mut created_ids,
                &mut nodes_created,
                &mut edges_created,
                &mut warnings,
            );
        }
        for r in &requirements {
            let mut props = Props::new()
                .set("name", r.name.as_str())
                .set("statement", r.statement.as_str());
            if let Some(pr) = r.priority.as_deref()
                && PRIORITY_VALUES.contains(&pr)
            {
                props = props.set("priority", pr);
            }
            self.integrate_node(
                node::REQUIREMENT,
                &r.id,
                props,
                &options.fragment_id,
                &mut created_ids,
                &mut nodes_created,
                &mut edges_created,
                &mut warnings,
            );
        }
        for c in &constraints {
            let mut props = Props::new()
                .set("name", c.name.as_str())
                .set("statement", c.statement.as_str());
            if let Some(cat) = c.category.as_deref()
                && CONSTRAINT_CATEGORIES.contains(&cat)
            {
                props = props.set("category", cat);
            }
            self.integrate_node(
                node::CONSTRAINT,
                &c.id,
                props,
                &options.fragment_id,
                &mut created_ids,
                &mut nodes_created,
                &mut edges_created,
                &mut warnings,
            );
        }
        for c in &capabilities {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("description", c.description.as_str());
            self.integrate_node(
                node::CAPABILITY,
                &c.id,
                props,
                &options.fragment_id,
                &mut created_ids,
                &mut nodes_created,
                &mut edges_created,
                &mut warnings,
            );
        }
        for c in &components {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("purpose", c.purpose.as_str());
            self.integrate_node(
                node::COMPONENT,
                &c.id,
                props,
                &options.fragment_id,
                &mut created_ids,
                &mut nodes_created,
                &mut edges_created,
                &mut warnings,
            );
        }

        // Edges: ALLOCATED_TO (capability -> component), SATISFIES (capability -> requirement).
        for c in &components {
            for cap_id in &c.allocated_capability_ids {
                self.integrate_edge(
                    edge::ALLOCATED_TO,
                    node::CAPABILITY,
                    cap_id,
                    node::COMPONENT,
                    &c.id,
                    &created_ids,
                    &mut edges_created,
                    &mut dropped_edges,
                );
            }
        }
        for s in &satisfies {
            self.integrate_edge(
                edge::SATISFIES,
                node::CAPABILITY,
                &s.capability_id,
                node::REQUIREMENT,
                &s.requirement_id,
                &created_ids,
                &mut edges_created,
                &mut dropped_edges,
            );
        }

        let status = if errors.is_empty() && warnings.is_empty() && dropped_edges.is_empty() {
            IngestStatus::Ok
        } else {
            IngestStatus::Partial
        };
        Ok(IngestReport {
            fragment_id: options.fragment_id.clone(),
            nodes_created,
            edges_created,
            pass_errors: errors,
            warnings,
            dropped_edges,
            status,
        })
    }

    /// Create/merge one node, link it from the provenance fragment via YIELDED,
    /// and contain it under the project spine. A node that fails schema
    /// validation is recorded as a warning, not fatal (no cascade).
    #[allow(clippy::too_many_arguments)]
    fn integrate_node(
        &mut self,
        node_type: &'static str,
        id: &str,
        props: Props,
        fragment_id: &str,
        created_ids: &mut HashMap<String, &'static str>,
        nodes_created: &mut usize,
        edges_created: &mut usize,
        warnings: &mut Vec<String>,
    ) {
        match self.create_node(node_type, id, props) {
            Ok(_) => {
                created_ids.insert(id.to_string(), node_type);
                *nodes_created += 1;
                // Provenance: Fragment YIELDED node {created}.
                if self
                    .create_edge(
                        edge::YIELDED,
                        node::FRAGMENT,
                        fragment_id,
                        node_type,
                        id,
                        Props::new().set("action", "created"),
                    )
                    .is_ok()
                {
                    *edges_created += 1;
                }
            }
            Err(e) => warnings.push(format!("skipped {node_type} '{id}': {e}")),
        }
    }

    /// Create one edge, but only between endpoints that were actually created
    /// this run — a reference to an unknown id is dropped with a reason rather
    /// than written as a phantom edge (no silent drops).
    #[allow(clippy::too_many_arguments)]
    fn integrate_edge(
        &mut self,
        edge_type: &str,
        from_type: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
        created_ids: &HashMap<String, &'static str>,
        edges_created: &mut usize,
        dropped_edges: &mut Vec<DroppedEdge>,
    ) {
        let drop = |reason: String, dropped: &mut Vec<DroppedEdge>| {
            dropped.push(DroppedEdge {
                edge_type: edge_type.to_string(),
                from_id: from_id.to_string(),
                to_id: to_id.to_string(),
                reason,
            });
        };
        if created_ids.get(from_id) != Some(&from_type) {
            drop(
                format!("source '{from_id}' not a created {from_type}"),
                dropped_edges,
            );
            return;
        }
        if created_ids.get(to_id) != Some(&to_type) {
            drop(
                format!("target '{to_id}' not a created {to_type}"),
                dropped_edges,
            );
            return;
        }
        match self.create_edge(edge_type, from_type, from_id, to_type, to_id, Props::new()) {
            Ok(_) => *edges_created += 1,
            Err(e) => drop(format!("schema rejected: {e}"), dropped_edges),
        }
    }
}

// Schema enum value sets (single source of truth is schema/*.yaml; kept here for
// loud-skip validation of LLM output. A drift here fails a node into `warnings`
// rather than silently dropping it).
const PROVENANCE_VALUES: &[&str] = &[
    "authored",
    "planned",
    "inferred",
    "healed",
    "reconciled",
    "imported",
];
const PRIORITY_VALUES: &[&str] = &["low", "medium", "high", "critical"];
const CONSTRAINT_CATEGORIES: &[&str] = &[
    "technical",
    "business",
    "operational",
    "physical",
    "regulatory",
    "budget",
    "schedule",
];
