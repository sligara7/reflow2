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
//! - **Time-aware resolution** — each extracted node resolves to
//!   *matched-unchanged* (no-op), *matched-evolved* (snapshot the prior state +
//!   record a `ChangeEvent`, THEN apply — never a silent overwrite), or
//!   *genuinely-new*. Re-ingesting an updated brief records the change.
//! - **Cross-id fuzzy dedup** — a new id whose name closely matches an existing
//!   same-type node (`token_sort_ratio` ≥ [`FUZZY_MATCH_THRESHOLD`], no
//!   embeddings) resolves to that node instead of duplicating; the merge is
//!   recorded in `fuzzy_merges` and edges redirect through an alias map.
//!
//! Deferred (noted so they're not mistaken for done): the **vector tiebreaker**
//! for the ambiguous middle band of `fuzzy_then_vector` — matching entities that
//! mean the same but read differently needs embeddings, kept behind an optional
//! pluggable seam (see the interaction-surface decision). Also deferred: the
//! **SME** augmentation pass,
//! real parallelism (passes run sequentially here), per-pass timeout/retry,
//! metrics, and the remaining passes (flows, actors, interfaces, decisions,
//! artifacts, resources, dependencies, inference, dimensions, changes). This
//! increment implements the spine:
//! project/requirements/constraints/capabilities → components → satisfies.

use std::collections::HashMap;

use dynograph_core::{DynoError, Value};
use dynograph_resolution::token_sort_ratio;
use dynograph_storage::StoredNode;
use serde::Deserialize;

use crate::graph::DesignGraph;
use crate::llm::{LlmBackend, LlmRequest, complete_json};
use crate::nodes::{Props, edge, node};
use crate::temporal::{ChangeAction, ChangeRecord, ChangeType, EpochType};

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

/// A cross-id fuzzy dedup: an extracted node whose id was new but whose name
/// matched an existing same-type node closely enough to be treated as the same
/// entity. Surfaced (never silent) so a wrong merge is auditable.
#[derive(Debug, Clone)]
pub struct FuzzyMerge {
    /// The id the extraction produced.
    pub extracted_id: String,
    /// The existing canonical node it resolved to.
    pub canonical_id: String,
    /// The node type.
    pub node_type: &'static str,
    /// The fuzzy score (0–100) that cleared the threshold.
    pub score: u32,
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
    /// Genuinely-new nodes created this run (includes the provenance Fragment).
    pub nodes_created: usize,
    /// Matched-evolved nodes: an existing node whose content changed — snapshotted
    /// and re-recorded with a `ChangeEvent`, never silently overwritten.
    pub nodes_evolved: usize,
    /// Matched-unchanged nodes: already present, identical content → left as-is.
    pub nodes_unchanged: usize,
    /// Cross-id fuzzy dedups: a new id resolved to an existing node by name
    /// similarity instead of creating a duplicate. Auditable, never silent.
    pub fuzzy_merges: Vec<FuzzyMerge>,
    /// Edges created this run.
    pub edges_created: usize,
    /// The `DesignEpoch` matched-evolved snapshots were pinned to (`Some` only
    /// when at least one node evolved).
    pub epoch_used: Option<String>,
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
    /// `OCCURS_DURING`). Matched-evolved snapshots are pinned here; if unset and
    /// a node evolves, ingest opens `epoch:{fragment_id}` and reports it.
    pub epoch_id: Option<String>,
    /// The change type recorded on the `ChangeEvent` for every matched-evolved
    /// node this run (why you re-ingested). Per-node auto-classification is the
    /// deferred `changes` pass (EX-Z2); until then the caller declares it.
    pub change_type: ChangeType,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            fragment_id: "frag:ingest".to_string(),
            fragment_title: "Ingested design input".to_string(),
            provenance: "authored".to_string(),
            epoch_id: None,
            change_type: ChangeType::ScopeChange,
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

        // ---- INTEGRATE (resolve → typed, provenance-stamped, time-aware) ----
        let effective_epoch = options
            .epoch_id
            .clone()
            .unwrap_or_else(|| format!("epoch:{}", options.fragment_id));
        let mut st = Integration::new(&options.fragment_id, effective_epoch, options.change_type);

        // Provenance fragment first (a new fragment per ingest).
        let mut frag_props = Props::new()
            .set("title", options.fragment_title.as_str())
            .set("fragment_type", "design")
            .set("provenance", options.provenance.as_str());
        if !PROVENANCE_VALUES.contains(&options.provenance.as_str()) {
            st.warnings.push(format!(
                "provenance '{}' not a schema value; using 'authored'",
                options.provenance
            ));
            frag_props = frag_props.set("provenance", "authored");
        }
        match self.create_node(node::FRAGMENT, &options.fragment_id, frag_props) {
            Ok(_) => st.nodes_created += 1,
            Err(e) => st
                .warnings
                .push(format!("fragment '{}': {e}", options.fragment_id)),
        }
        // Honor a caller-named epoch up front so provenance-in-time is valid.
        if options.epoch_id.is_some() {
            self.ensure_epoch(&mut st);
            self.link_fragment_epoch(&mut st);
        }

        // Nodes (resolved: unchanged / evolved / new).
        if let Some(p) = &project {
            let mut props = Props::new().set("name", p.name.as_str());
            props = props.set_opt("objective", p.objective.as_deref());
            props = props.set_opt("domain", p.domain.as_deref());
            if let Some(m) = p.mode.as_deref()
                && (m == "flexible" || m == "rigid")
            {
                props = props.set("mode", m);
            }
            self.integrate_node(&mut st, node::PROJECT, &p.id, props);
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
            self.integrate_node(&mut st, node::REQUIREMENT, &r.id, props);
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
            self.integrate_node(&mut st, node::CONSTRAINT, &c.id, props);
        }
        for c in &capabilities {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("description", c.description.as_str());
            self.integrate_node(&mut st, node::CAPABILITY, &c.id, props);
        }
        for c in &components {
            let props = Props::new()
                .set("name", c.name.as_str())
                .set("purpose", c.purpose.as_str());
            self.integrate_node(&mut st, node::COMPONENT, &c.id, props);
        }

        // Edges: ALLOCATED_TO (capability -> component), SATISFIES (capability -> requirement).
        for c in &components {
            for cap_id in &c.allocated_capability_ids {
                self.integrate_edge(
                    &mut st,
                    edge::ALLOCATED_TO,
                    node::CAPABILITY,
                    cap_id,
                    node::COMPONENT,
                    &c.id,
                );
            }
        }
        for s in &satisfies {
            self.integrate_edge(
                &mut st,
                edge::SATISFIES,
                node::CAPABILITY,
                &s.capability_id,
                node::REQUIREMENT,
                &s.requirement_id,
            );
        }

        // If we lazily opened an epoch for evolutions, tie the fragment to it too.
        if options.epoch_id.is_none() && st.nodes_evolved > 0 {
            self.link_fragment_epoch(&mut st);
        }
        let epoch_used = if st.nodes_evolved > 0 || options.epoch_id.is_some() {
            Some(st.epoch_id.clone())
        } else {
            None
        };
        let status = if errors.is_empty() && st.warnings.is_empty() && st.dropped_edges.is_empty() {
            IngestStatus::Ok
        } else {
            IngestStatus::Partial
        };
        Ok(IngestReport {
            fragment_id: options.fragment_id.clone(),
            nodes_created: st.nodes_created,
            nodes_evolved: st.nodes_evolved,
            nodes_unchanged: st.nodes_unchanged,
            fuzzy_merges: st.fuzzy_merges,
            edges_created: st.edges_created,
            epoch_used,
            pass_errors: errors,
            warnings: st.warnings,
            dropped_edges: st.dropped_edges,
            status,
        })
    }

    /// Ensure the effective `DesignEpoch` node exists — created lazily the first
    /// time a matched-evolved node needs somewhere to pin its snapshot.
    fn ensure_epoch(&mut self, st: &mut Integration) {
        if st.epoch_ready {
            return;
        }
        st.epoch_ready = true;
        if matches!(self.get_node(node::DESIGN_EPOCH, &st.epoch_id), Ok(None))
            && let Err(e) = self.add_epoch(&st.epoch_id, "ingest epoch", EpochType::Revision, 0)
        {
            st.warnings
                .push(format!("open epoch '{}': {e}", st.epoch_id));
        }
    }

    /// `Fragment OCCURS_DURING epoch` — provenance-in-time.
    fn link_fragment_epoch(&mut self, st: &mut Integration) {
        if self
            .create_edge(
                edge::OCCURS_DURING,
                node::FRAGMENT,
                st.fragment_id,
                node::DESIGN_EPOCH,
                &st.epoch_id,
                Props::new(),
            )
            .is_ok()
        {
            st.edges_created += 1;
        }
    }

    /// Resolve one extracted node against the graph and integrate it:
    /// **genuinely-new** → create; **matched-unchanged** → leave as-is (no write,
    /// no snapshot); **matched-evolved** → snapshot the prior state + record a
    /// `ChangeEvent` (via [`record_change`](DesignGraph::record_change)) THEN
    /// apply the edit — never a silent overwrite (extraction-plan.md, "a
    /// matched-evolved result that lands with no Snapshot is an integrity
    /// breach"). Every resolved node is registered so later edges can reference
    /// it, and linked from the provenance fragment.
    fn integrate_node(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        props: Props,
    ) {
        let new_map = props.build();
        match self.get_node(node_type, id) {
            Err(e) => st.warnings.push(format!("resolve {node_type} '{id}': {e}")),
            // Direct id hit → resolve against that node.
            Ok(Some(_)) => self.integrate_existing(st, node_type, id, new_map),
            // Id miss → try cross-id fuzzy dedup before creating a duplicate.
            Ok(None) => match self.fuzzy_match(node_type, &new_map, id) {
                Err(e) => {
                    st.warnings
                        .push(format!("fuzzy-match {node_type} '{id}': {e}"));
                    self.integrate_new(st, node_type, id, new_map);
                }
                Ok(Some((canonical, score))) => {
                    st.aliases.insert(id.to_string(), canonical.clone());
                    st.fuzzy_merges.push(FuzzyMerge {
                        extracted_id: id.to_string(),
                        canonical_id: canonical.clone(),
                        node_type,
                        score,
                    });
                    self.integrate_existing(st, node_type, &canonical, new_map);
                }
                Ok(None) => self.integrate_new(st, node_type, id, new_map),
            },
        }
    }

    /// Create a genuinely-new node + its provenance link.
    fn integrate_new(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        new_map: HashMap<String, Value>,
    ) {
        match self.create_node(node_type, id, new_map) {
            Ok(_) => {
                st.created_ids.insert(id.to_string(), node_type);
                st.nodes_created += 1;
                self.yield_edge(st, node_type, id, "created");
            }
            Err(e) => st.warnings.push(format!("skipped {node_type} '{id}': {e}")),
        }
    }

    /// Resolve an extracted node against an existing one (`id` is a real node —
    /// a direct id hit or a fuzzy-matched canonical): matched-unchanged →
    /// no-op; matched-evolved → snapshot + `ChangeEvent` THEN apply.
    fn integrate_existing(
        &mut self,
        st: &mut Integration,
        node_type: &'static str,
        id: &str,
        new_map: HashMap<String, Value>,
    ) {
        let existing = match self.get_node(node_type, id) {
            Ok(Some(n)) => n,
            Ok(None) => return, // vanished between resolve and integrate — nothing to do
            Err(e) => {
                st.warnings.push(format!("resolve {node_type} '{id}': {e}"));
                return;
            }
        };
        if node_unchanged(&existing, &new_map) {
            st.created_ids.insert(id.to_string(), node_type);
            st.nodes_unchanged += 1;
            return;
        }
        // matched-evolved: remember the past, then apply the edit.
        self.ensure_epoch(st);
        let ce_id = format!("chg:{}:{id}", st.fragment_id);
        let name = format!("Re-ingest updated {node_type} {id}");
        let rec = ChangeRecord {
            epoch_id: &st.epoch_id,
            change_event_id: &ce_id,
            name: &name,
            change_type: st.change_type,
            target_type: node_type,
            target_id: id,
            action: ChangeAction::Modified,
        };
        match self.record_change(rec) {
            Ok(_) => {
                if let Err(e) = self.create_node(node_type, id, new_map) {
                    st.warnings
                        .push(format!("apply evolved {node_type} '{id}': {e}"));
                }
                st.created_ids.insert(id.to_string(), node_type);
                st.nodes_evolved += 1;
                self.yield_edge(st, node_type, id, "updated");
            }
            Err(e) => st
                .warnings
                .push(format!("snapshot evolved {node_type} '{id}': {e}")),
        }
    }

    /// Cross-id dedup: find an existing same-type node whose `name` matches the
    /// extracted node's name at/above [`FUZZY_MATCH_THRESHOLD`] (token-order- and
    /// case-insensitive, no embeddings). Returns the best canonical id + score.
    /// Conservative on purpose — the ambiguous middle band is where the deferred
    /// LLM adjudication / vector tiebreaker (EX-R2) belongs.
    fn fuzzy_match(
        &self,
        node_type: &'static str,
        new_map: &HashMap<String, Value>,
        extracted_id: &str,
    ) -> Result<Option<(String, u32)>, DynoError> {
        let Some(new_name) = new_map.get("name").and_then(Value::as_str) else {
            return Ok(None);
        };
        let mut best: Option<(String, u32)> = None;
        for n in self.scan_nodes(node_type)? {
            if n.node_id == extracted_id {
                continue;
            }
            if let Some(existing_name) = n.properties.get("name").and_then(Value::as_str) {
                let score = token_sort_ratio(new_name, existing_name);
                if score >= FUZZY_MATCH_THRESHOLD && best.as_ref().is_none_or(|(_, b)| score > *b) {
                    best = Some((n.node_id.clone(), score));
                }
            }
        }
        Ok(best)
    }

    /// `Fragment YIELDED node {action}` — provenance link.
    fn yield_edge(&mut self, st: &mut Integration, node_type: &str, id: &str, action: &str) {
        if self
            .create_edge(
                edge::YIELDED,
                node::FRAGMENT,
                st.fragment_id,
                node_type,
                id,
                Props::new().set("action", action),
            )
            .is_ok()
        {
            st.edges_created += 1;
        }
    }

    /// Create one edge, but only between endpoints resolved this run — a
    /// reference to an unknown id is dropped with a reason, never a phantom edge.
    fn integrate_edge(
        &mut self,
        st: &mut Integration,
        edge_type: &str,
        from_type: &'static str,
        from_id: &str,
        to_type: &'static str,
        to_id: &str,
    ) {
        // Redirect endpoints through any fuzzy-merge aliases, so an edge that
        // referenced a merged-away id lands on the canonical node.
        let from = st
            .aliases
            .get(from_id)
            .cloned()
            .unwrap_or_else(|| from_id.to_string());
        let to = st
            .aliases
            .get(to_id)
            .cloned()
            .unwrap_or_else(|| to_id.to_string());

        let mut drop = |reason: String| {
            st.dropped_edges.push(DroppedEdge {
                edge_type: edge_type.to_string(),
                from_id: from.clone(),
                to_id: to.clone(),
                reason,
            });
        };
        if st.created_ids.get(from.as_str()) != Some(&from_type) {
            drop(format!("source '{from}' not a resolved {from_type}"));
            return;
        }
        if st.created_ids.get(to.as_str()) != Some(&to_type) {
            drop(format!("target '{to}' not a resolved {to_type}"));
            return;
        }
        match self.create_edge(edge_type, from_type, &from, to_type, &to, Props::new()) {
            Ok(_) => st.edges_created += 1,
            Err(e) => drop(format!("schema rejected: {e}")),
        }
    }
}

/// Minimum `token_sort_ratio` (0–100) for a cross-id fuzzy dedup. High on
/// purpose: below this, resolution creates a new node rather than risk a wrong
/// merge — the uncertain band is the deferred LLM/vector tiebreaker's job.
const FUZZY_MATCH_THRESHOLD: u32 = 90;

/// Mutable accumulators for one integration pass — bundled so the integration
/// methods keep small, stable signatures (per the modular-code principle).
struct Integration<'a> {
    fragment_id: &'a str,
    epoch_id: String,
    change_type: ChangeType,
    epoch_ready: bool,
    created_ids: HashMap<String, &'static str>,
    /// extracted id → canonical id, for edges that referenced a fuzzy-merged id.
    aliases: HashMap<String, String>,
    nodes_created: usize,
    nodes_evolved: usize,
    nodes_unchanged: usize,
    fuzzy_merges: Vec<FuzzyMerge>,
    edges_created: usize,
    warnings: Vec<String>,
    dropped_edges: Vec<DroppedEdge>,
}

impl<'a> Integration<'a> {
    fn new(fragment_id: &'a str, epoch_id: String, change_type: ChangeType) -> Self {
        Self {
            fragment_id,
            epoch_id,
            change_type,
            epoch_ready: false,
            created_ids: HashMap::new(),
            aliases: HashMap::new(),
            nodes_created: 0,
            nodes_evolved: 0,
            nodes_unchanged: 0,
            fuzzy_merges: Vec::new(),
            edges_created: 0,
            warnings: Vec::new(),
            dropped_edges: Vec::new(),
        }
    }
}

/// Whether `existing` already holds every property the extraction produced
/// (compared only over the extracted keys, so schema defaults don't read as a
/// change). Equal ⇒ matched-unchanged; differing ⇒ matched-evolved.
fn node_unchanged(existing: &StoredNode, new_map: &HashMap<String, Value>) -> bool {
    new_map
        .iter()
        .all(|(k, v)| existing.properties.get(k) == Some(v))
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
