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

use crate::foundation::core::DynoError;
use crate::foundation::store::{StoredEdge, StoredNode};

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

/// How many store reads a module made, by operation.
///
/// A count, not a duration, and that is the point. Per-read cost was measured
/// FLAT — `get_node` cost 58.0µs over a 2853-node design and 55.1µs over a
/// 199-node one — so the store indexes correctly and what a module actually
/// controls is how many times it asks, not how long each ask takes. A count is
/// also the one measurement a parallel test suite cannot distort: a duration
/// assertion measures machine contention, and the usual response is to raise
/// the threshold until it stops complaining, which retires the gate without
/// anybody deciding to.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReadCounts {
    pub get_node: usize,
    pub scan_nodes: usize,
    pub count_nodes: usize,
    pub outgoing: usize,
    pub incoming: usize,
    /// Nodes actually handed back by `scan_nodes` — a scan is one CALL but not
    /// one unit of work, and a budget that counted only calls would rate
    /// "scan everything twice" as cheap.
    pub nodes_scanned: usize,
}

impl ReadCounts {
    /// Every call that reached the store.
    #[must_use]
    pub fn total(&self) -> usize {
        self.get_node + self.scan_nodes + self.count_nodes + self.outgoing + self.incoming
    }
}

/// Wraps any [`GraphRead`] and counts what passes through it.
///
/// THE SECOND REAL IMPLEMENTATION OF THIS CONTRACT, after `DesignGraph` itself,
/// and the one that makes the contract pay for itself immediately: the optimize
/// skill names "it cannot supply your measurement" as its own honest limit, and
/// before this there was no way to ask what a module costs the store without
/// editing the module. Now any module behind the contract can be measured from
/// the outside, by construction, without touching it.
///
/// It is a decorator, so it composes: wrap a real graph, a test fake, or
/// another decorator, and the module under measurement cannot tell.
///
/// ```no_run
/// # use reflow2_core::graph_read::{CountingRead, GraphRead};
/// # fn demo(g: &dyn GraphRead) -> Result<(), reflow2_core::DynoError> {
/// let counted = CountingRead::new(g);
/// let _ = reflow2_core::granularity::granularity_report(&counted)?;
/// assert!(counted.counts().total() > 0);
/// # Ok(())
/// # }
/// ```
pub struct CountingRead<'a> {
    inner: &'a dyn GraphRead,
    counts: std::cell::RefCell<ReadCounts>,
}

impl<'a> CountingRead<'a> {
    #[must_use]
    pub fn new(inner: &'a dyn GraphRead) -> Self {
        Self {
            inner,
            counts: std::cell::RefCell::new(ReadCounts::default()),
        }
    }

    /// What has passed through so far. Cheap to call, and does not reset.
    #[must_use]
    pub fn counts(&self) -> ReadCounts {
        *self.counts.borrow()
    }
}

impl GraphRead for CountingRead<'_> {
    fn get_node(&self, node_type: &str, id: &str) -> Result<Option<StoredNode>, DynoError> {
        self.counts.borrow_mut().get_node += 1;
        self.inner.get_node(node_type, id)
    }

    fn scan_nodes(&self, node_type: &str) -> Result<Vec<StoredNode>, DynoError> {
        self.counts.borrow_mut().scan_nodes += 1;
        let out = self.inner.scan_nodes(node_type)?;
        self.counts.borrow_mut().nodes_scanned += out.len();
        Ok(out)
    }

    fn count_nodes(&self, node_type: &str) -> Result<usize, DynoError> {
        self.counts.borrow_mut().count_nodes += 1;
        self.inner.count_nodes(node_type)
    }

    fn outgoing(
        &self,
        from_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<StoredEdge>, DynoError> {
        self.counts.borrow_mut().outgoing += 1;
        self.inner.outgoing(from_id, edge_type)
    }

    fn incoming(&self, to_id: &str, edge_type: Option<&str>) -> Result<Vec<StoredEdge>, DynoError> {
        self.counts.borrow_mut().incoming += 1;
        self.inner.incoming(to_id, edge_type)
    }
}
