//! Vocabulary constants and a small property builder.
//!
//! Node/edge *type names* are the schema's own strings (rule 3 in AGENTS.md:
//! terminology matches the schema). Naming them as constants here keeps the
//! typed helpers in [`crate::graph`] from sprinkling stringly-typed literals,
//! and gives one place to catch a rename against the schema.

/// Node type names, matching `schema/*.yaml`.
pub mod node {
    // P0 · Intent (core.yaml)
    pub const PROJECT: &str = "Project";
    pub const REQUIREMENT: &str = "Requirement";
    pub const CONSTRAINT: &str = "Constraint";
    pub const DESIGN_RULE: &str = "DesignRule";
    /// A question already put to the user about a gap (and whether answered).
    pub const QUESTION: &str = "Question";
    // P1 · Function (functional.yaml)
    pub const CAPABILITY: &str = "Capability";
    pub const FLOW: &str = "Flow";
    pub const ACTOR: &str = "Actor";
    // P2 · Structure (structure.yaml)
    pub const COMPONENT: &str = "Component";
    pub const INTERFACE: &str = "Interface";
    pub const DECISION: &str = "Decision";
    // P3 · Realization (build.yaml)
    pub const ARTIFACT: &str = "Artifact";
    pub const FRAGMENT: &str = "Fragment";
    // P4 · Verification (verify.yaml)
    pub const VERIFICATION: &str = "Verification";
    pub const DRIFT_EVENT: &str = "DriftEvent";
    // P5 · Operation (operate.yaml)
    pub const RELEASE: &str = "Release";
    pub const ENVIRONMENT: &str = "Environment";
    pub const RESOURCE: &str = "Resource";
    // Cross-cutting · depth axis (dimensions.yaml)
    pub const DIMENSION_ASSESSMENT: &str = "DimensionAssessment";
    pub const DIMENSION_OBSERVATION: &str = "DimensionObservation";
    // Axis Z · change over time (temporal.yaml)
    pub const DESIGN_EPOCH: &str = "DesignEpoch";
    pub const TEMPORAL_FACT: &str = "TemporalFact";
    pub const SNAPSHOT: &str = "Snapshot";
    pub const CHANGE_EVENT: &str = "ChangeEvent";
}

/// Edge type names, matching `schema/*.yaml`.
pub mod edge {
    /// `Project → *` — the decomposition (axis-Y) containment spine.
    /// `Question → *` — the design nodes a question was raised about.
    pub const ASKS_ABOUT: &str = "ASKS_ABOUT";
    pub const CONTAINS: &str = "CONTAINS";
    /// `* → *` — traceability: a Capability SATISFIES a Requirement.
    pub const SATISFIES: &str = "SATISFIES";
    /// `Capability → Component` — the WHAT→WHERE allocation binding.
    pub const ALLOCATED_TO: &str = "ALLOCATED_TO";
    /// `* → Decision/DesignRule` — the node is shaped by a recorded decision.
    pub const GOVERNED_BY: &str = "GOVERNED_BY";
    /// `Component → Interface` — the component that exposes a contract.
    pub const PROVIDES: &str = "PROVIDES";
    /// `* → Interface` — a Component/Actor that depends on a contract. Paired
    /// with [`PROVIDES`]: an Interface consumed but never provided is a break
    /// between two parts of the design, which is what [`crate::detect`] looks for.
    pub const CONSUMES: &str = "CONSUMES";
    /// `Artifact → *` — an Artifact realizes a Capability/Component/Interface.
    pub const REALIZES: &str = "REALIZES";
    /// `Artifact → Interface/Capability/Component` — an Artifact defines the contract.
    pub const SPECIFIES: &str = "SPECIFIES";
    /// `Artifact → *` — an Artifact documents (explains) a node.
    pub const DOCUMENTS: &str = "DOCUMENTS";
    /// `Verification → Artifact` — a Verification emitted this Artifact (evidence).
    pub const PRODUCES: &str = "PRODUCES";
    /// `Fragment → *` — a note/review/pseudocode fragment annotates a node.
    pub const ANNOTATES: &str = "ANNOTATES";
    /// `Verification → *` — a Verification checks a Capability/Artifact/Component.
    pub const VERIFIES: &str = "VERIFIES";
    /// `* → *` — a node depends on another (a lateral structural coupling).
    pub const DEPENDS_ON: &str = "DEPENDS_ON";
    /// `Fragment → *` — the fragment that produced/updated a node (provenance).
    pub const YIELDED: &str = "YIELDED";

    /// `DesignEpoch → DesignEpoch` — one epoch comes before another (ordering).
    pub const PRECEDES: &str = "PRECEDES";
    /// `DesignEpoch → DesignEpoch` — one epoch nests inside a larger one.
    pub const CONTAINS_EPOCH: &str = "CONTAINS_EPOCH";
    /// `Release → Environment` — a packaged version runs in an environment.
    pub const DEPLOYED_TO: &str = "DEPLOYED_TO";
    /// `* → Resource` — a Component or Release consumes a real-world resource.
    pub const REQUIRES_RESOURCE: &str = "REQUIRES_RESOURCE";

    // Axis Z · change over time (temporal.yaml)
    /// `* → DesignEpoch` — a Snapshot or ChangeEvent is pinned to its epoch.
    pub const AT_EPOCH: &str = "AT_EPOCH";
    /// `ChangeEvent → *` — the node a ChangeEvent added/modified/removed.
    pub const CHANGED: &str = "CHANGED";
    /// `* → Snapshot` — an entity has a captured state snapshot.
    pub const HAS_SNAPSHOT: &str = "HAS_SNAPSHOT";
    /// `* → DesignEpoch` — a Fragment/ChangeEvent/Verification happened during an epoch.
    pub const OCCURS_DURING: &str = "OCCURS_DURING";
    /// `* → TemporalFact` — an entity carries a time-bounded fact.
    pub const HAS_TEMPORAL_FACT: &str = "HAS_TEMPORAL_FACT";
    /// `TemporalFact → *` — the entity a temporal fact concerns.
    pub const ABOUT_ENTITY: &str = "ABOUT_ENTITY";
    /// `TemporalFact → DesignEpoch` — the fact becomes true at this epoch.
    pub const VALID_FROM: &str = "VALID_FROM";
    /// `TemporalFact → DesignEpoch` — the fact stops being true at this epoch.
    pub const VALID_TO: &str = "VALID_TO";

    // Inference "why" edges (inference.yaml) referenced by HEAL/PROPAGATE.
    /// `* → *` — two nodes are contradictory (a tension to resolve).
    pub const CONTRADICTS: &str = "CONTRADICTS";
    /// `* → *` — two nodes cover the same ground (candidates to merge).
    pub const DUPLICATES: &str = "DUPLICATES";
    /// `* → *` — a planned/anticipated need (may lack follow-through).
    pub const ANTICIPATES: &str = "ANTICIPATES";

    // Depth axis (dimensions.yaml)
    /// `DimensionAssessment → *` — links an assessment to the node it scores.
    pub const ASSESSED_ON: &str = "ASSESSED_ON";
    /// `* → DimensionObservation` — an entity carries a per-fragment reading.
    pub const HAS_OBSERVATION: &str = "HAS_OBSERVATION";
    /// `DimensionObservation → Fragment` — the fragment a reading came from.
    pub const OBSERVED_IN: &str = "OBSERVED_IN";
}

use std::collections::HashMap;

use dynograph_core::Value;

/// Ergonomic builder for a node/edge property map.
///
/// ```
/// # use reflow2_core::nodes::Props;
/// let props = Props::new().set("name", "Auth").set("priority", "high").build();
/// assert_eq!(props.len(), 2);
/// ```
#[derive(Debug, Default, Clone)]
pub struct Props(HashMap<String, Value>);

impl Props {
    /// Start an empty property map.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Set a property. Chainable. Overwrites a prior value for the same key.
    #[must_use]
    pub fn set(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.to_string(), value.into());
        self
    }

    /// Set a property only when `value` is `Some` — omit it otherwise, so an
    /// absent optional never lands as an empty string (no silent placeholder).
    #[must_use]
    pub fn set_opt(self, key: &str, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(v) => self.set(key, v),
            None => self,
        }
    }

    /// Consume into the `HashMap` the storage engine expects.
    pub fn build(self) -> HashMap<String, Value> {
        self.0
    }
}

impl From<Props> for HashMap<String, Value> {
    fn from(p: Props) -> Self {
        p.0
    }
}
