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
    // P1 · Function (functional.yaml)
    pub const CAPABILITY: &str = "Capability";
    pub const FLOW: &str = "Flow";
    pub const ACTOR: &str = "Actor";
    // P2 · Structure (structure.yaml)
    pub const COMPONENT: &str = "Component";
    pub const INTERFACE: &str = "Interface";
}

/// Edge type names, matching `schema/*.yaml`.
pub mod edge {
    /// `Project → *` — the decomposition (axis-Y) containment spine.
    pub const CONTAINS: &str = "CONTAINS";
    /// `* → *` — traceability: a Capability SATISFIES a Requirement.
    pub const SATISFIES: &str = "SATISFIES";
    /// `Capability → Component` — the WHAT→WHERE allocation binding.
    pub const ALLOCATED_TO: &str = "ALLOCATED_TO";
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
