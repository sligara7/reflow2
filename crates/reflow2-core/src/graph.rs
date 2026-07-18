//! [`DesignGraph`] — the reflow2 handle over a schema-configured graph store.
//!
//! Thin, deterministic, LLM-free (docs/interaction-surfaces.md, "deterministic
//! ops"). It wraps a dynograph-foundation [`StorageEngine`] already configured
//! with the full reflow2 [`Schema`], scopes every call to one logical graph id,
//! and exposes both generic schema-validated CRUD and typed convenience
//! constructors for the golden-thread node/edge types.
//!
//! Every write goes through the engine's `validate_node` / `validate_edge`, so a
//! bad node type, a missing required property, or an edge with the wrong
//! endpoints fails loud here (rule 4 in AGENTS.md: no silent fallbacks).

use dynograph_core::{DynoError, Schema, Value};
use dynograph_storage::{StorageEngine, StoredEdge, StoredNode};

use crate::nodes::{Props, edge, node};

/// Default logical graph id inside the storage instance. One design lives in
/// one graph; the id is just a stable name to scope keys.
pub const DEFAULT_GRAPH_ID: &str = "reflow2";

/// A design graph: a [`StorageEngine`] scoped to a single graph id.
pub struct DesignGraph {
    engine: StorageEngine,
    graph_id: String,
}

impl DesignGraph {
    /// Open an in-memory design graph configured with the full reflow2 schema.
    ///
    /// The in-memory backend needs no cargo feature and no disk — ideal for
    /// tests and dev iteration. Fails only if the embedded schema fails to
    /// merge/validate (a build-time-embedded bug, surfaced at open).
    pub fn open_in_memory() -> Result<Self, DynoError> {
        let schema = crate::schema::load_schema()?;
        Ok(Self {
            engine: StorageEngine::new_in_memory(schema),
            graph_id: DEFAULT_GRAPH_ID.to_string(),
        })
    }

    /// Open an on-disk design graph backed by RocksDB at `path`, configured
    /// with the full reflow2 schema. This is the persistent surface backend:
    /// the design survives across agent sessions (surface-plan.md, step 1),
    /// where the in-memory backend is dev/test only.
    ///
    /// Delegates to the foundation's [`StorageEngine::new_rocksdb`], which is
    /// present in the API regardless of the `rocksdb` feature: with the feature
    /// off it returns a fail-loud error (no silent fallback to memory — AGENTS.md
    /// rule 4), and the C++ `librocksdb-sys` compile stays opt-in. Also fails if
    /// the embedded schema fails to merge or the store cannot be opened.
    pub fn open_rocksdb(path: &str) -> Result<Self, DynoError> {
        let schema = crate::schema::load_schema()?;
        Ok(Self {
            engine: StorageEngine::new_rocksdb(schema, path)?,
            graph_id: DEFAULT_GRAPH_ID.to_string(),
        })
    }

    /// Use a non-default logical graph id (e.g. to host several designs in one
    /// storage instance). Chainable off a constructor.
    #[must_use]
    pub fn with_graph_id(mut self, id: impl Into<String>) -> Self {
        self.graph_id = id.into();
        self
    }

    /// The graph id every operation is scoped to.
    pub fn graph_id(&self) -> &str {
        &self.graph_id
    }

    /// The merged schema backing this graph.
    pub fn schema(&self) -> &Schema {
        self.engine.schema()
    }

    // ---- Generic, schema-validated CRUD -----------------------------------

    /// Create (or replace) a node of `node_type` with `id` and `props`.
    /// Validates against the schema; unknown type or missing required property
    /// is an error, not a silent skip.
    pub fn create_node(
        &mut self,
        node_type: &str,
        id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredNode, DynoError> {
        self.engine
            .create_node(&self.graph_id, node_type, id, props.into())
    }

    /// Fetch a node by type and id. `Ok(None)` when it does not exist.
    pub fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError> {
        self.engine.get_node(&self.graph_id, node_type, id)
    }

    /// Count nodes of a type.
    pub fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError> {
        self.engine.count_nodes(&self.graph_id, node_type)
    }

    /// Create an edge of `edge_type` between typed endpoints. Endpoint types
    /// are validated against the edge's declared `from`/`to`.
    pub fn create_edge(
        &mut self,
        edge_type: &str,
        from_type: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
        props: impl Into<std::collections::HashMap<String, Value>>,
    ) -> Result<StoredEdge, DynoError> {
        self.engine.create_edge(
            &self.graph_id,
            edge_type,
            from_type,
            from_id,
            to_type,
            to_id,
            props.into(),
        )
    }

    /// Outgoing edges from `from_id`, optionally filtered to one edge type.
    /// This is the primitive the golden-thread walk (PROPAGATE) builds on.
    pub fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        self.engine
            .scan_outgoing_edges(&self.graph_id, from_id, edge_type)
    }

    /// Incoming edges to `to_id`, optionally filtered to one edge type. The
    /// reverse-direction companion to [`outgoing`](Self::outgoing) — PROPAGATE
    /// needs both, because impact flows along an edge in whichever direction the
    /// edge's semantics carry it (e.g. a Requirement's realizers are reached via
    /// *incoming* SATISFIES).
    pub fn incoming(
        &self,
        to_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        self.engine
            .scan_incoming_edges(&self.graph_id, to_id, edge_type)
    }

    /// All nodes of a type. Used by PROPAGATE to build an id→type index (edge
    /// adjacency stores only endpoint ids, not their types).
    pub fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError> {
        self.engine.scan_nodes(&self.graph_id, node_type)
    }

    /// Delete a node and every edge attached to it. Returns whether it existed.
    pub fn delete_node(&mut self, node_type: &str, id: &str) -> Result<bool, DynoError> {
        self.engine.delete_node(&self.graph_id, node_type, id)
    }

    /// Delete a single edge. Returns whether it existed.
    pub fn delete_edge(
        &mut self,
        edge_type: &str,
        from_id: &str,
        to_id: &str,
    ) -> Result<bool, DynoError> {
        self.engine
            .delete_edge(&self.graph_id, edge_type, from_id, to_id)
    }

    // ---- Atomic batches (used by HEAL's apply step) -----------------------

    /// Begin buffering writes; nothing hits the store until [`commit_batch`].
    ///
    /// [`commit_batch`]: Self::commit_batch
    pub(crate) fn begin_batch(&mut self) {
        self.engine.begin_batch();
    }

    /// Flush all buffered writes atomically.
    pub(crate) fn commit_batch(&mut self) -> Result<usize, DynoError> {
        self.engine.commit_batch()
    }

    /// Drop all buffered writes without applying them.
    pub(crate) fn discard_batch(&mut self) {
        self.engine.discard_batch();
    }

    // ---- Typed golden-thread constructors ---------------------------------
    //
    // Convenience over `create_node` for the four spine node types, supplying
    // only their required properties. Richer properties can still go through
    // `create_node` with a full `Props`.

    /// P0 · Intent — the top-level thing being designed. `name` is required.
    pub fn add_project(&mut self, id: &str, name: &str) -> Result<StoredNode, DynoError> {
        self.create_node(node::PROJECT, id, Props::new().set("name", name))
    }

    /// P0 · Intent — a stated need. `name` and `statement` are required.
    pub fn add_requirement(
        &mut self,
        id: &str,
        name: &str,
        statement: &str,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::REQUIREMENT,
            id,
            Props::new().set("name", name).set("statement", statement),
        )
    }

    /// P1 · Function — something the design can do. `name` and `description`
    /// are required.
    pub fn add_capability(
        &mut self,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::CAPABILITY,
            id,
            Props::new()
                .set("name", name)
                .set("description", description),
        )
    }

    /// P2 · Structure — a buildable part. `name` and `purpose` are required;
    /// `kind`/`level` take their schema defaults (`module`/`component`).
    pub fn add_component(
        &mut self,
        id: &str,
        name: &str,
        purpose: &str,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::COMPONENT,
            id,
            Props::new().set("name", name).set("purpose", purpose),
        )
    }

    /// P2 · Structure — a contract between parts. `name` is required; `medium`
    /// takes its schema default (`REST`).
    ///
    /// An Interface is the seam PROPAGATE crosses to reach the *other* side of
    /// a change: one Component [`provides`](Self::provides) it, others
    /// [`consume`](Self::consumes) it.
    pub fn add_interface(&mut self, id: &str, name: &str) -> Result<StoredNode, DynoError> {
        self.create_node(node::INTERFACE, id, Props::new().set("name", name))
    }

    /// P2 · Structure — a recorded decision with its rationale (an ADR, in
    /// software terms). `name` and `decision` are required; `rationale` is
    /// optional but is the part worth having — HEAL raises a `contradiction`
    /// when two nodes disagree with no Decision resolving them, and a Decision
    /// without a reason does not actually resolve anything.
    pub fn add_decision(
        &mut self,
        id: &str,
        name: &str,
        decision: &str,
        rationale: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::DECISION,
            id,
            Props::new()
                .set("name", name)
                .set("decision", decision)
                .set_opt("rationale", rationale),
        )
    }

    // ---- Typed golden-thread edges ----------------------------------------

    /// `Project CONTAINS child` — the containment spine (axis Y).
    pub fn contains(
        &mut self,
        project_id: &str,
        child_type: &str,
        child_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONTAINS,
            node::PROJECT,
            project_id,
            child_type,
            child_id,
            Props::new(),
        )
    }

    /// `parent Component CONTAINS child Component` — the component decomposition
    /// spine (axis Y / matryoshka). Parent should be exactly one `Component.level`
    /// above the child; see [`crate::hierarchy`].
    pub fn contain_component(
        &mut self,
        parent_id: &str,
        child_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONTAINS,
            node::COMPONENT,
            parent_id,
            node::COMPONENT,
            child_id,
            Props::new(),
        )
    }

    /// `Capability SATISFIES Requirement` — the traceability link that binds
    /// WHAT back to intent (the golden thread).
    pub fn satisfies(
        &mut self,
        capability_id: &str,
        requirement_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::SATISFIES,
            node::CAPABILITY,
            capability_id,
            node::REQUIREMENT,
            requirement_id,
            Props::new(),
        )
    }

    /// `Capability ALLOCATED_TO Component` — the WHAT→WHERE binding.
    pub fn allocate(
        &mut self,
        capability_id: &str,
        component_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::ALLOCATED_TO,
            node::CAPABILITY,
            capability_id,
            node::COMPONENT,
            component_id,
            Props::new(),
        )
    }

    /// `node GOVERNED_BY Decision/DesignRule` — the node is shaped by a
    /// recorded decision. `from_type` and `to_type` are required: the schema
    /// allows any endpoints (`from: "*"`, `to: "*"`).
    pub fn governed_by(
        &mut self,
        from_type: &str,
        from_id: &str,
        to_type: &str,
        to_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::GOVERNED_BY,
            from_type,
            from_id,
            to_type,
            to_id,
            Props::new(),
        )
    }

    /// `Component PROVIDES Interface` — the side of a contract that implements it.
    pub fn provides(
        &mut self,
        component_id: &str,
        interface_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::PROVIDES,
            node::COMPONENT,
            component_id,
            node::INTERFACE,
            interface_id,
            Props::new(),
        )
    }

    /// `Component CONSUMES Interface` — the side of a contract that depends on it.
    ///
    /// This is the edge that makes "changed one side, forgot the other"
    /// findable: from the provider, PROPAGATE reaches every consumer laterally
    /// through the Interface.
    pub fn consumes(
        &mut self,
        consumer_id: &str,
        interface_id: &str,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::CONSUMES,
            node::COMPONENT,
            consumer_id,
            node::INTERFACE,
            interface_id,
            Props::new(),
        )
    }
}
