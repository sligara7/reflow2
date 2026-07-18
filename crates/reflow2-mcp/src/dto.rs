//! Serializable mirrors of the foundation's storage types.
//!
//! `StoredNode` / `StoredEdge` live in `dynograph-storage` and don't derive
//! `Serialize`; the orphan rule stops us adding it from here. These thin DTOs
//! carry the same fields (all already serde-ready, `Value` included) so CRUD
//! tools can return nodes/edges as JSON.

use reflow2_core::{StoredEdge, StoredNode, Value};
use serde::Serialize;
use std::collections::HashMap;

/// JSON mirror of a [`StoredNode`].
#[derive(Debug, Clone, Serialize)]
pub struct NodeDto {
    pub graph_id: String,
    pub node_type: String,
    pub node_id: String,
    pub properties: HashMap<String, Value>,
}

impl From<StoredNode> for NodeDto {
    fn from(n: StoredNode) -> Self {
        Self {
            graph_id: n.graph_id,
            node_type: n.node_type,
            node_id: n.node_id,
            properties: n.properties,
        }
    }
}

/// JSON mirror of a [`StoredEdge`].
#[derive(Debug, Clone, Serialize)]
pub struct EdgeDto {
    pub graph_id: String,
    pub edge_type: String,
    pub from_id: String,
    pub to_id: String,
    pub properties: HashMap<String, Value>,
}

impl From<StoredEdge> for EdgeDto {
    fn from(e: StoredEdge) -> Self {
        Self {
            graph_id: e.graph_id,
            edge_type: e.edge_type,
            from_id: e.from_id,
            to_id: e.to_id,
            properties: e.properties,
        }
    }
}
