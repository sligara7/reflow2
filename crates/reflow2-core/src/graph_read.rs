//! `ifc:graph-read` — the five operations a module needs to READ a design
//! without being able to change it.
//!
//! # Why this exists
//!
//! Before 2026-08-21 this crate had exactly one trait — [`LlmBackend`] — and
//! 274 public functions hanging off a single `DesignGraph` struct across 43
//! files, all sharing its private state through `pub(crate)`. So `detect.rs`
//! did not DEPEND ON `graph.rs`; it WAS `graph.rs`, in another file. Nothing
//! could be swapped for an alternative, held still and optimised, or tested
//! without standing up a whole store, because there was no boundary to stand
//! outside of.
//!
//! [`LlmBackend`]: crate::llm::LlmBackend
//!
//! # Why these five, and not some other set
//!
//! They were counted, not chosen. Across reflow2-core the calls that reach the
//! store are: `get_node` 113, `scan_nodes` 78, `outgoing` 59, `incoming` 57,
//! `count_nodes` 17 — 324 of them, against 83 writes. And 17 of the 40
//! store-touching modules never write at all. The boundary was already there;
//! this only names it.
//!
//! # What is deliberately NOT here
//!
//! **Every write.** A module holding this contract *cannot change the design*,
//! and that is the property that makes it safe to swap and cheap to test. A
//! module needing writes is not a read-only black box and should say so by
//! taking `&mut DesignGraph`.
//!
//! **Derived views** like `design_network`. They are BUILT from these five, so
//! they consume this contract rather than belong to it. Putting a derived view
//! in the trait would force every alternative implementation to reimplement
//! reflow2's own analysis before it could answer a single call.
//!
//! **`engine()` and `graph_id()`.** `search.rs` reaches through them to the
//! storage engine directly, which is why search is NOT a consumer of this
//! boundary despite calling only three methods. That exclusion is information:
//! it says search needs work before it can be swapped, and writing this
//! contract is what surfaced it.
//!
//! # The contract the types cannot carry
//!
//! ABSENCE IS NOT AN ERROR. A node that is not there is `Ok(None)`; a type with
//! no nodes, or a node with no matching edges, is `Ok(vec![])`. An `Err` means
//! the store could not answer, never that the answer was empty.
//!
//! AN UNKNOWN `node_type` IS AN ERROR, and must NOT be answered `Ok(None)` or
//! `Ok(vec![])`. "No such type" and "no such node" are different facts and must
//! not share a reply — collapsing them answers "nothing there" for every typo,
//! forever, in the most reassuring possible way. An implementation that did so
//! would satisfy every signature here and break every caller's meaning, which
//! is why it is written down rather than left to the compiler.
//!
//! # The constraint this contract inherits
//!
//! [`StoredNode`] and [`StoredEdge`] belong to dynograph-foundation, recorded
//! in the design as `ifc:req-dyno-storage-api` with designation `required` —
//! reflow2 consumes it and does not own it. So an alternative implementation is
//! free to store bytes however it likes and is NOT free to invent its own node
//! and edge types: it must produce these. That limit on substitutability was
//! inherited, not chosen.

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

/// Read-only access to one design's nodes and edges.
///
/// See the [module docs](self) for the obligations an implementation takes on
/// that the signatures cannot express — chiefly that absence is not an error
/// and that an unknown `node_type` is.
pub trait GraphRead {
    /// One node by type and id. `Ok(None)` when no such node exists; `Err`
    /// when the type is unknown or the store could not answer.
    fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError>;

    /// Every node of one type. `Ok(vec![])` when there are none.
    fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError>;

    /// How many nodes of one type — separate from [`scan_nodes`](Self::scan_nodes)
    /// because a count need not materialise the nodes, and 17 call sites want
    /// only the number.
    fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError>;

    /// Edges leaving `from_id`, optionally filtered to one edge type.
    fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError>;

    /// Edges arriving at `to_id`, optionally filtered to one edge type.
    fn incoming(&self, to_id: &str, edge_type: Option<&str>) -> Result<Vec<StoredEdge>, DynoError>;
}

/// The real implementation, delegating to `DesignGraph`'s inherent methods.
///
/// It adds nothing and hides nothing: the inherent methods stay, so every
/// existing caller is untouched, and this impl is what lets a module ask for
/// `&dyn GraphRead` instead of the whole 274-function struct.
impl GraphRead for crate::graph::DesignGraph {
    fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError> {
        crate::graph::DesignGraph::get_node(self, node_type, id)
    }

    fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError> {
        crate::graph::DesignGraph::scan_nodes(self, node_type)
    }

    fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError> {
        crate::graph::DesignGraph::count_nodes(self, node_type)
    }

    fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        crate::graph::DesignGraph::outgoing(self, from_id, edge_type)
    }

    fn incoming(&self, to_id: &str, edge_type: Option<&str>) -> Result<Vec<StoredEdge>, DynoError> {
        crate::graph::DesignGraph::incoming(self, to_id, edge_type)
    }
}
