//! The whole design as one portable document, and back again.
//!
//! Three jobs, one mechanism:
//!
//! - **Migration.** Export with the old binary, import with the new one. This is
//!   the general answer to a schema or storage-format change, and a far better
//!   one than bespoke backfill code written per change.
//! - **Backup.** A design graph is small — hundreds to low thousands of nodes —
//!   so keeping every version costs almost nothing.
//! - **Portability.** Move a design between machines, or hand one to somebody.
//!
//! # Deterministic on purpose
//!
//! Everything is sorted: node types, ids, edges, and property keys (which is why
//! the exported types use [`BTreeMap`] rather than the `HashMap` the store
//! hands back). Two exports of an unchanged graph are byte-identical.
//!
//! That is not tidiness. It is what makes the file diffable, so a backup
//! directory under version control shows *what changed in the design* between
//! two points rather than a fresh blob each time. A `HashMap`'s iteration order
//! is seeded per process, so an unsorted export would rewrite itself completely
//! on every run and the history would be worthless.
//!
//! # Not the temporal axis
//!
//! `DesignEpoch` / `Snapshot` / `ChangeEvent` record *why* the design changed,
//! semantically, inside the graph. This records the graph's contents at a point
//! in time. Neither substitutes for the other: the temporal axis cannot recover
//! a corrupted store, and an export cannot explain a requirement's history.

use std::collections::BTreeMap;

use dynograph_core::{DynoError, Value};
use dynograph_storage::{StoredEdge, StoredNode};
use serde::{Deserialize, Serialize};

use crate::graph::DesignGraph;
use crate::provenance::GraphStamp;

/// Sorted property bag — `BTreeMap` so the JSON is byte-stable.
pub type Props = BTreeMap<String, Value>;

/// A node, as it appears in an export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedNode {
    pub node_type: String,
    pub node_id: String,
    #[serde(default)]
    pub properties: Props,
}

impl From<StoredNode> for ExportedNode {
    fn from(n: StoredNode) -> Self {
        Self {
            node_type: n.node_type,
            node_id: n.node_id,
            properties: n.properties.into_iter().collect(),
        }
    }
}

/// An edge, as it appears in an export.
///
/// Endpoint *types* are not stored: they are recoverable from the nodes in the
/// same document, and duplicating them would let a file disagree with itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedEdge {
    pub edge_type: String,
    pub from_id: String,
    pub to_id: String,
    #[serde(default)]
    pub properties: Props,
}

impl From<StoredEdge> for ExportedEdge {
    fn from(e: StoredEdge) -> Self {
        Self {
            edge_type: e.edge_type,
            from_id: e.from_id,
            to_id: e.to_id,
            properties: e.properties.into_iter().collect(),
        }
    }
}

/// A whole design graph, portable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphExport {
    /// Which reflow2 wrote it. Carried so an import can tell whether the file
    /// came from a vocabulary it does not know — the same question
    /// [`crate::provenance`] asks of a graph directory.
    pub stamp: GraphStamp,
    /// Fingerprint of the design content — see [`GraphExport::content_hash`].
    /// Deliberately excludes the stamp and the chain fields, so the same
    /// design hashes identically whichever build wrote it and whatever it
    /// claims about its ancestry. A document whose embedded hash does not
    /// match its own content was edited outside reflow2 or corrupted
    /// (`dec:export-hash-chain`); absence means it predates hashing, which is
    /// reported, never an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// The content hash of the export this one superseded — file lineage,
    /// recorded at the file-write seam when an export replaces an existing
    /// export file with *changed* content (unchanged content keeps the old
    /// chain, so two exports of an unchanged design stay byte-identical).
    /// What lets `compare_designs` answer "does other descend from base?".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_content_hash: Option<String>,
    pub graph_id: String,
    pub nodes: Vec<ExportedNode>,
    pub edges: Vec<ExportedEdge>,
}

impl GraphExport {
    /// The canonical content fingerprint: sha256 over the compact,
    /// sorted-key JSON of `{"edges", "graph_id", "nodes"}` — the design
    /// content only. Full hash, not truncated like display checksums: this is
    /// tamper evidence, and the chain's identity.
    ///
    /// Canonical means: object keys sorted (serde_json's default map is a
    /// `BTreeMap`), compact separators, minimal escaping — the same form
    /// Python's `json.dumps(…, sort_keys=True, ensure_ascii=False,
    /// separators=(",", ":"))` produces, so the stdlib CI gate can recompute
    /// it; `tools/smoke_mcp.py` pins the two implementations against each
    /// other.
    pub fn compute_content_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let canonical = serde_json::json!({
            "edges": self.edges,
            "graph_id": self.graph_id,
            "nodes": self.nodes,
        });
        let text = serde_json::to_string(&canonical)
            .expect("export content is JSON-serializable by construction");
        let digest = Sha256::digest(text.as_bytes());
        let mut hex = String::with_capacity(7 + 64);
        hex.push_str("sha256:");
        for b in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        hex
    }

    /// The document's content hash for chain purposes: the embedded one when
    /// present, recomputed otherwise — so a pre-hashing document still has an
    /// identity and the chain can grow from it.
    pub fn effective_content_hash(&self) -> String {
        self.content_hash
            .clone()
            .unwrap_or_else(|| self.compute_content_hash())
    }

    /// Does the embedded `content_hash` match the actual content? `None` when
    /// the document carries no hash (predates hashing) — three-valued on
    /// purpose, because "unhashed" and "tampered" are different facts.
    pub fn verify_content_hash(&self) -> Option<bool> {
        self.content_hash
            .as_ref()
            .map(|h| *h == self.compute_content_hash())
    }

    /// Set the lineage link relative to the export file this document is
    /// about to replace (`dec:export-hash-chain`): content changed → the
    /// chain advances to the predecessor's hash; content unchanged → the
    /// predecessor's own chain is kept, so an unchanged design writes
    /// byte-identical files.
    pub fn chain_after(&mut self, predecessor: &GraphExport) {
        let prev_hash = predecessor.effective_content_hash();
        let own = self
            .content_hash
            .clone()
            .unwrap_or_else(|| self.compute_content_hash());
        self.prev_content_hash = if own == prev_hash {
            predecessor.prev_content_hash.clone()
        } else {
            Some(prev_hash)
        };
    }
}

/// What an import did. Reported rather than assumed — an import that quietly
/// skipped half a design would be the worst kind of success.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportReport {
    pub nodes_written: usize,
    pub edges_written: usize,
    /// Edges whose endpoints were not in the document and not already in the
    /// graph. Named, never dropped silently.
    pub skipped_edges: Vec<String>,
    /// Set when the document's embedded `content_hash` does not match its
    /// actual content — the file was edited outside reflow2 or corrupted.
    /// The import still proceeds (the human may know exactly why — a hand-
    /// resolved git merge, say) but never silently: deciding what a mismatch
    /// means is their call, seeing it is not optional. `None` for a matching
    /// hash and for pre-hashing documents alike — absence of a hash is not
    /// evidence of tampering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity_note: Option<String>,
}

impl DesignGraph {
    /// Export the whole graph, deterministically.
    ///
    /// Walks every node type the schema declares, then each node's outgoing
    /// edges — so every edge is visited exactly once, from its source.
    pub fn export_graph(&self) -> Result<GraphExport, DynoError> {
        let mut node_types: Vec<&String> = self.schema().node_types.keys().collect();
        node_types.sort();

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for t in node_types {
            let mut found: Vec<StoredNode> = self.scan_nodes(t)?;
            found.sort_by(|a, b| a.node_id.cmp(&b.node_id));
            for n in found {
                for e in self.outgoing(&n.node_id, None)? {
                    edges.push(ExportedEdge::from(e));
                }
                nodes.push(ExportedNode::from(n));
            }
        }
        edges.sort_by(|a, b| {
            a.edge_type
                .cmp(&b.edge_type)
                .then(a.from_id.cmp(&b.from_id))
                .then(a.to_id.cmp(&b.to_id))
        });

        let mut export = GraphExport {
            stamp: GraphStamp::current(self.schema()),
            content_hash: None,
            prev_content_hash: None,
            graph_id: self.graph_id().to_string(),
            nodes,
            edges,
        };
        export.content_hash = Some(export.compute_content_hash());
        Ok(export)
    }

    /// Load an exported design into this graph, atomically.
    ///
    /// Upsert, not replace: a node id already present is overwritten, and
    /// anything already in the graph and absent from the document is left
    /// alone. Clearing first is the caller's decision, not a side effect of
    /// importing.
    ///
    /// Everything lands in one batch, so a document that fails validation
    /// half-way leaves the graph untouched rather than half-loaded.
    pub fn import_graph(&mut self, doc: &GraphExport) -> Result<ImportReport, DynoError> {
        // Endpoint types come from the document's own nodes, falling back to
        // what is already in the graph — so an export can be layered onto a
        // design it references without carrying it.
        let mut types: BTreeMap<&str, &str> = BTreeMap::new();
        for n in &doc.nodes {
            types.insert(n.node_id.as_str(), n.node_type.as_str());
        }
        let existing = self.node_type_index()?;

        self.begin_batch();
        let result = (|| -> Result<ImportReport, DynoError> {
            for n in &doc.nodes {
                let props: std::collections::HashMap<String, Value> =
                    n.properties.clone().into_iter().collect();
                self.create_node(&n.node_type, &n.node_id, props)?;
            }
            let mut edges_written = 0;
            let mut skipped_edges = Vec::new();
            for e in &doc.edges {
                let from = types
                    .get(e.from_id.as_str())
                    .copied()
                    .or_else(|| existing.get(&e.from_id).map(String::as_str));
                let to = types
                    .get(e.to_id.as_str())
                    .copied()
                    .or_else(|| existing.get(&e.to_id).map(String::as_str));
                match (from, to) {
                    (Some(ft), Some(tt)) => {
                        let props: std::collections::HashMap<String, Value> =
                            e.properties.clone().into_iter().collect();
                        self.create_edge(&e.edge_type, ft, &e.from_id, tt, &e.to_id, props)?;
                        edges_written += 1;
                    }
                    _ => skipped_edges.push(format!(
                        "{} {} -> {} (endpoint not in the document or the graph)",
                        e.edge_type, e.from_id, e.to_id
                    )),
                }
            }
            Ok(ImportReport {
                nodes_written: doc.nodes.len(),
                edges_written,
                skipped_edges,
                integrity_note: match doc.verify_content_hash() {
                    Some(false) => Some(
                        "the document's content_hash does not match its content — it was \
                         edited outside reflow2 or corrupted since it was exported"
                            .to_string(),
                    ),
                    _ => None,
                },
            })
        })();

        match result {
            Ok(report) => {
                self.commit_batch()?;
                Ok(report)
            }
            Err(e) => {
                // Nothing half-written: the batch is dropped, not partially kept.
                self.discard_batch();
                Err(e)
            }
        }
    }
}
