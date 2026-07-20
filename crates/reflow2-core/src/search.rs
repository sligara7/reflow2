//! SEARCH — find design nodes by what they say, not by knowing their id.
//!
//! The schema has declared `fulltext:` on `name`/`statement`/`description`
//! properties since it was written, and the foundation implements the index
//! (`dynograph-text`, BM25 over Tantivy, mirrored automatically on every node
//! write) — but until 2026-07-20 nothing in reflow2 enabled the feature or
//! served it, so the only retrieval was `get_node` (know the id) and
//! `scan_nodes` (read a whole type). That made finding-by-content the LLM's
//! job, which is the seat-swap docs/partnership.md forbids: finding and
//! counting belong to the graph.
//!
//! The index is a **derived, rebuildable sidecar** — the node store stays the
//! source of truth. A graph written by a binary built *without* the feature
//! has nodes the index never saw, which is why [`DesignGraph::reindex_search`]
//! exists and is run once at server start: stale silence is worse than the
//! cost of one bounded rebuild.

use dynograph_core::DynoError;

use crate::graph::DesignGraph;

/// One search hit, hydrated: the scored id plus the node's `name`, so a caller
/// can show a result list without a `get_node` round trip per hit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub node_id: String,
    pub node_type: String,
    /// BM25 relevance — comparable within one result list, not across queries.
    pub score: f32,
    /// The node's `name` property at hit time (empty if it has none).
    pub name: String,
}

/// A search result that owns up to what it could not do: hits the index
/// returned whose node no longer exists in the store are reported, never
/// silently dropped — a non-empty `stale` list means the index has drifted
/// and a [`DesignGraph::reindex_search`] is due.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    /// Ids the index returned but the store no longer holds (index drift).
    pub stale: Vec<String>,
    /// The limit that bounded this result — `hits.len() == limit` means there
    /// may be more; this is the no-silent-caps rule made visible.
    pub limit: usize,
}

#[cfg(feature = "fulltext")]
impl DesignGraph {
    /// BM25 keyword search over every `fulltext` property in the design,
    /// optionally scoped to one node type. Keyword search, not substring or
    /// regex: "persistence graph" finds nodes whose text carries those terms,
    /// ranked. Empty query or zero limit returns an empty result rather than
    /// everything.
    pub fn search_design(
        &self,
        query: &str,
        node_type: Option<&str>,
        limit: usize,
    ) -> Result<crate::search::SearchResult, DynoError> {
        let raw = self
            .engine()
            .search_fulltext(self.graph_id(), query, node_type, limit)?;
        let mut hits = Vec::with_capacity(raw.len());
        let mut stale = Vec::new();
        for h in raw {
            match self.get_node(&h.node_type, &h.node_id)? {
                Some(node) => hits.push(SearchHit {
                    name: node
                        .properties
                        .get("name")
                        .and_then(dynograph_core::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    node_id: h.node_id,
                    node_type: h.node_type,
                    score: h.score,
                }),
                None => stale.push(h.node_id),
            }
        }
        Ok(SearchResult { hits, stale, limit })
    }

    /// Rebuild the full-text index from the node store. Bounded by graph size
    /// and idempotent; run at server start so a graph written by an older,
    /// index-less binary becomes searchable instead of silently absent.
    /// Returns the number of nodes indexed.
    pub fn reindex_search(&self) -> Result<usize, DynoError> {
        self.engine().reindex_fulltext(self.graph_id())
    }
}

#[cfg(not(feature = "fulltext"))]
impl DesignGraph {
    /// Fails loud without the `fulltext` feature (mirroring the `rocksdb`
    /// contract): a search that silently returns nothing would read as "the
    /// design says nothing about that", which is a lie.
    pub fn search_design(
        &self,
        _query: &str,
        _node_type: Option<&str>,
        _limit: usize,
    ) -> Result<crate::search::SearchResult, DynoError> {
        Err(DynoError::Storage(
            "this reflow2 was built without the `fulltext` feature, so it cannot search the \
             design. Rebuild with:  cargo build -p reflow2-mcp  (the surface crate enables it)"
                .into(),
        ))
    }

    /// Fails loud without the `fulltext` feature; see [`Self::search_design`].
    pub fn reindex_search(&self) -> Result<usize, DynoError> {
        Err(DynoError::Storage(
            "this reflow2 was built without the `fulltext` feature, so there is no search \
             index to rebuild. Rebuild with:  cargo build -p reflow2-mcp"
                .into(),
        ))
    }
}
