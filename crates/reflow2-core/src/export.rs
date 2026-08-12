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

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::{DynoError, Value};
use dynograph_storage::{StoredEdge, StoredNode};
use serde::{Deserialize, Serialize};

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};
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
    /// [`crate::provenance`] asks of a graph directory. Optional on the way in,
    /// the sibling rule to `content_hash`: a hand-authored or third-party
    /// document legitimately has no stamp, and its absence is a first-class,
    /// reported state (see [`ImportReport::provenance_note`]), never a reason to
    /// refuse the document (BL-87). Every export reflow2 writes carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<GraphStamp>,
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
    /// Which design wrote it. **Optional on the way in (BL-138)**, and the
    /// reason is that `import_graph` never reads it: an import loads into the
    /// receiving graph, whose id the server already knows, so demanding it from
    /// a hand author asks the caller to restate the receiver's own identity —
    /// and then ignores the answer. The `adopt` skill's central instruction is
    /// *"build one export document and `import_graph` it once"*, and following
    /// it literally failed on `missing field 'graph_id'`.
    ///
    /// **`mirror_surface` DOES read it and still refuses a document without
    /// one**, because the two operations genuinely differ: mirroring records
    /// where a surface came from (`mirror_of`) and guards against mirroring a
    /// design into itself, neither of which is answerable from an unidentified
    /// document. Distinguishing them is the point — the same shape as [BL-119],
    /// where one requirement was right for a round-tripped export and wrong for
    /// a hand-authored one.
    ///
    /// Empty means unidentified; see [`GraphExport::is_unidentified`]. Every
    /// export reflow2 writes carries a real one.
    #[serde(default)]
    pub graph_id: String,
    /// Defaulted so a document may legitimately carry no nodes — and so that a
    /// hand-authored `{"nodes": [...]}` with no edge list is accepted rather
    /// than refused for omitting an empty array.
    #[serde(default)]
    pub nodes: Vec<ExportedNode>,
    /// See [`GraphExport::nodes`].
    #[serde(default)]
    pub edges: Vec<ExportedEdge>,
}

impl GraphExport {
    /// Whether the document arrived with no stamp — a hand-authored or
    /// third-party document rather than one reflow2 exported (BL-87).
    pub fn is_unstamped(&self) -> bool {
        self.stamp.is_none()
    }

    /// Whether the document names no source design (BL-138). Harmless for
    /// `import_graph`, which never reads it; disqualifying for
    /// `mirror_surface`, which cannot record where a surface came from without
    /// it. Reported, never guessed at.
    pub fn is_unidentified(&self) -> bool {
        self.graph_id.is_empty()
    }

    /// The reflow2 version that wrote this document, or `"unstamped"` when it
    /// carries no stamp — so callers reporting provenance (compare, merge) read
    /// one value and never unwrap an absent stamp.
    pub fn reflow2_version(&self) -> &str {
        self.stamp
            .as_ref()
            .map(|s| s.reflow2_version.as_str())
            .unwrap_or("unstamped")
    }

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
    /// Set when the document arrived with no `stamp` — a hand-authored or
    /// third-party document rather than one reflow2 exported. The import
    /// proceeds (the stamp is provenance metadata, not a gate — the sibling
    /// rule to an absent `content_hash`), but never silently: an unstamped
    /// document cannot be checked for an upgrade-direction mismatch, so the
    /// human should know what they loaded (BL-87). `None` for a stamped
    /// document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_note: Option<String>,
    /// Set when this import ADOPTED the document's design identity — a restore
    /// into an empty store, which takes the design's own name rather than
    /// renaming it to the receiver's (BL-169).
    ///
    /// Reported rather than merely done, because the alternative is exactly the
    /// failure that filed the row: a design silently renamed, committed and
    /// pushed through a fully green pipeline, with `graph_id` sitting inside the
    /// export's content hash. `None` when nothing was adopted — an in-memory
    /// graph, a store that already holds a design, or a document whose name the
    /// store already carries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adopted_identity: Option<String>,
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
            stamp: Some(GraphStamp::current(self.schema())),
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
        // WHOSE DESIGN IS THIS? Answered before a single write, because the id
        // namespaces every stored key and the import writes under whatever name
        // the graph currently carries (BL-169).
        //
        // Restoring a design into an EMPTY store takes the document's name.
        // Anything else renames it: `graph_id` is inside the export's content
        // hash, so a round trip that renamed would not come back byte-identical,
        // and on 2026-08-02 exactly that shipped — an export replayed through a
        // temp graph came back as `05a6fbe860bf7a23` where the design has been
        // `reflow2` since its first commit, and it was committed and pushed with
        // both CI jobs and the coherence gate green.
        //
        // This rule already existed and lived in the WRONG PLACE: `main.rs`
        // applied it on the CLI `--import` path only, so the command and the
        // tool disagreed about what a restore means. It belongs in the operation
        // — one predicate, every caller, including any future one.
        //
        // `adopt_on_import` is the shared predicate and it is conservative in
        // the direction that matters: a store already holding a design keeps its
        // own name, because layering an export onto a live design is an upsert,
        // not a restore, and taking the incoming name there would rename a
        // design to whatever was last imported into it.
        let mut adopted_identity = None;
        if let Some(path) = self.store_path.clone() {
            let holds = self.holds_a_design();
            if let Some(adopted) = crate::identity::adopt_on_import(&path, &doc.graph_id, holds)? {
                self.graph_id = adopted.graph_id.clone();
                adopted_identity = Some(adopted.graph_id);
            }
        }

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
            // EVERY FAULT IN ONE RESPONSE, NOT ONE PER ROUND TRIP (BL-118).
            // This used to be `create_node(..)?`, so validation stopped at the
            // first violation: an external adopt pass over a hand-authored
            // 9,000-line document took FOUR consecutive imports to learn four
            // faults — a missing stamp field, then three different enum
            // violations — each attempt a full edit-retry cycle for one error.
            //
            // THE ATOMICITY IS UNTOUCHED AND MUST STAY THAT WAY: the same
            // session named "nothing half-loaded across four failures" as one
            // of the things that worked notably well. This is exactly
            // `dec:bulk-is-all-or-nothing-with-per-item-findings` — every item
            // attempted so you learn every failure at once, and if any failed
            // nothing is written — which BL-153 settled for the bulk tools and
            // whose own row noted that `import_graph` files the opposite
            // defect. The shape is borrowed rather than reinvented.
            //
            // It still returns Err rather than an Ok-with-failures report, and
            // that is deliberate: a caller that treated a rejected import as
            // success would be the silent-failure this crate's first principle
            // forbids. The error carries the whole list.
            let mut faults: Vec<String> = Vec::new();
            for (index, n) in doc.nodes.iter().enumerate() {
                let props: std::collections::HashMap<String, Value> =
                    n.properties.clone().into_iter().collect();
                if let Err(e) = self.create_node(&n.node_type, &n.node_id, props) {
                    faults.push(format!("nodes[{index}] {}: {e}", n.node_id));
                }
            }
            let mut edges_written = 0;
            let mut skipped_edges = Vec::new();
            for (index, e) in doc.edges.iter().enumerate() {
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
                        match self.create_edge(&e.edge_type, ft, &e.from_id, tt, &e.to_id, props) {
                            Ok(_) => edges_written += 1,
                            Err(err) => faults.push(format!(
                                "edges[{index}] {} {} -> {}: {err}",
                                e.edge_type, e.from_id, e.to_id
                            )),
                        }
                    }
                    _ => skipped_edges.push(format!(
                        "{} {} -> {} (endpoint not in the document or the graph)",
                        e.edge_type, e.from_id, e.to_id
                    )),
                }
            }
            if !faults.is_empty() {
                let n = faults.len();
                return Err(DynoError::Validation {
                    node_type: "GraphExport".into(),
                    property: "nodes/edges".into(),
                    message: format!(
                        "{n} item(s) in this document are invalid, and NOTHING was written — \
                         the import is all-or-nothing. Every one is listed so you can fix them \
                         in one edit rather than learning them one import at a time:\n  - {}",
                        faults.join("\n  - ")
                    ),
                });
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
                provenance_note: doc.is_unstamped().then(|| {
                    "the document had no stamp — imported as unstamped (a hand-authored or \
                     third-party document, not one reflow2 exported); provenance and the \
                     upgrade-direction check cannot be run on it"
                        .to_string()
                }),
                adopted_identity: adopted_identity.clone(),
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

/// What a published-surface export contains, and what it deliberately does not.
///
/// A node this surface kept whose container it withheld.
///
/// Reported so a recipient can tell a genuinely top-level part from one this
/// filtering orphaned — the two are indistinguishable in the document itself.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeveredContainment {
    /// A node this surface KEPT.
    pub node_id: String,
    /// A container it has in the full design, which this surface WITHHELD — so
    /// in the document the recipient holds, the node has no parent.
    pub withheld_parent: String,
}

/// The report is the load-bearing half. A surface document is a *partial* design
/// by construction, so shipping one without saying what was held back would be
/// the silent drop rule 6 forbids — and worse than usual here, because the
/// recipient cannot tell a small design from a heavily-filtered one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SurfaceExport {
    /// The document: published Interfaces, what specifies them, and the parts on
    /// each side. Importable like any export.
    pub document: GraphExport,
    /// The published boundaries this surface is *about*, sorted.
    pub published: Vec<String>,
    /// Design nodes kept back as internal.
    pub withheld_nodes: usize,
    /// Edges dropped because at least one endpoint was withheld.
    pub withheld_edges: usize,
    /// Nodes this surface KEPT whose container it WITHHELD, each naming the
    /// container that went — so a recipient can tell a genuinely top-level part
    /// from one this filtering orphaned.
    ///
    /// **Reported, never repaired.** Carrying the ancestry would disclose the
    /// internals a surface exists to withhold; re-parenting the orphan to the
    /// Project would assert a `CONTAINS` nobody drew, which is the fabrication
    /// `req:a-repair-suggestion-never-proposes-fabrication` forbids; dropping the
    /// child would delete the provider of a published contract. So the document
    /// is unchanged and the severance is stated.
    ///
    /// Found 2026-08-12 on the first real cross-design trial: four `subsystem`
    /// components travelled without the `system` that contains them, and
    /// `hierarchy_issues` in the receiving graph went 0 → 4 `orphan_level` —
    /// findings that were FALSE about the source design and CORRECT about the
    /// document it was handed. The detector was innocent; the document lied by
    /// omission.
    pub severed_containment: Vec<SeveredContainment>,
    /// Said in words, because a count alone does not tell the recipient what
    /// kind of document they are holding.
    pub note: String,
}

impl DesignGraph {
    /// Export **only the published surface** — the contracts others are entitled
    /// to rely on, and nothing internal.
    ///
    /// The first piece of `req:design-composes` that every architecture answer
    /// needs: whatever composes, it composes through a published boundary rather
    /// than by reaching into another system's internals. Also the openness half
    /// of `req:key-interfaces` ("a design that publishes a surface should be able
    /// to export exactly that surface").
    ///
    /// **What goes in**: every Interface designated `published`, the artifacts
    /// that `SPECIFIES` or `REALIZES` it (the machine-readable contract — an
    /// OpenAPI or protobuf file is the real ICD; both edges are honoured because
    /// `link_artifact` writes REALIZES while the schema's intent for a contract
    /// artifact is SPECIFIES), the Components that provide or consume it, and the
    /// Project for provenance. Nodes are exported **as stored** — nothing is
    /// trimmed or rewritten, because a fabricated node would import as a lie.
    /// What leaks internals is the graph *beneath* a component, and that is what
    /// is excluded.
    ///
    /// **What stays home**: requirements, capabilities, decisions, verifications,
    /// history, provenance, and every internal component and contract. All
    /// counted, never silently dropped.
    ///
    /// **It does not join the file's hash chain.** `prev_content_hash` is left
    /// unset: this is a derived view, not a record of the design, and letting it
    /// advance the chain would make `compare_designs` treat a published surface
    /// as an ancestor of the full design (`dec:export-hash-chain`).
    pub fn export_surface(&self) -> Result<SurfaceExport, DynoError> {
        let published = self.published_interfaces()?;
        let mut keep: BTreeSet<String> = published.clone();

        // The parts on each side of a published contract: PROVIDES/CONSUMES run
        // from the Component to the Interface, so the sides are its incoming.
        for ifc in &published {
            for e in self.incoming(ifc, None)? {
                if e.edge_type == edge::PROVIDES || e.edge_type == edge::CONSUMES {
                    keep.insert(e.from_id.clone());
                }
            }
            // The contract artifacts. SPECIFIES is the modelled intent — "an
            // OpenAPI / protobuf / IDL file SPECIFIES an Interface (its
            // authoritative contract)" — but `link_artifact` writes REALIZES, so
            // in practice the ICD arrives on either edge. Both belong on a
            // published surface: one is the contract as written, the other as
            // built, and a recipient needs whichever exists.
            for e in self.incoming(ifc, None)? {
                if e.edge_type == edge::SPECIFIES || e.edge_type == edge::REALIZES {
                    keep.insert(e.from_id.clone());
                }
            }
        }
        // The promises this design publishes (`req:publishable-promise`). A
        // structural surface says what the boundaries ARE and cannot say what any
        // of them undertakes to DO — "fails loud rather than falling back",
        // "ordering is preserved" — because a behavioural commitment lives in a
        // Requirement and every Requirement was withheld. Found by a real trial,
        // where the promise ended up asserted in a comment in the CONSUMER's
        // build file, on the wrong side of the seam.
        //
        // Opt-in, exactly like the boundaries: only requirements the owner
        // deliberately designated `published` travel. Everything else is still
        // withheld and still counted.
        let promises = self.published_promises()?;
        keep.extend(promises.iter().cloned());

        // The Project, so a recipient knows whose surface this is.
        for p in self.scan_nodes(node::PROJECT)? {
            keep.insert(p.node_id);
        }

        let full = self.export_graph()?;
        let total_nodes = full.nodes.len();
        let total_edges = full.edges.len();
        let nodes: Vec<ExportedNode> = full
            .nodes
            .into_iter()
            .filter(|n| keep.contains(&n.node_id))
            .collect();
        let edges: Vec<ExportedEdge> = full
            .edges
            .into_iter()
            .filter(|e| keep.contains(&e.from_id) && keep.contains(&e.to_id))
            .collect();

        let withheld_nodes = total_nodes - nodes.len();
        let withheld_edges = total_edges - edges.len();

        // What the withholding did to what we KEPT. A node whose every CONTAINS
        // parent was filtered out arrives with no place in the hierarchy, and
        // the recipient cannot tell that from a genuinely top-level part.
        let mut severed_containment: Vec<SeveredContainment> = Vec::new();
        for n in &nodes {
            let mut lost: Vec<String> = Vec::new();
            let mut survived = false;
            for e in self.incoming(&n.node_id, Some(edge::CONTAINS))? {
                if keep.contains(&e.from_id) {
                    survived = true;
                    break;
                }
                lost.push(e.from_id.clone());
            }
            if !survived {
                lost.sort();
                for parent in lost {
                    severed_containment.push(SeveredContainment {
                        node_id: n.node_id.clone(),
                        withheld_parent: parent,
                    });
                }
            }
        }
        severed_containment.sort_by(|a, b| {
            (&a.node_id, &a.withheld_parent).cmp(&(&b.node_id, &b.withheld_parent))
        });
        let severance_note = if severed_containment.is_empty() {
            String::new()
        } else {
            format!(
                " ⚠️ {} kept node(s) LOST THEIR CONTAINER to this filtering and arrive with no                  parent: {}. They are not top-level in the source design — read them as                  structurally partial, and do not diagnose their hierarchy from this document.",
                severed_containment.len(),
                severed_containment
                    .iter()
                    .map(|s| format!("{} (was in {})", s.node_id, s.withheld_parent))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        // Said plainly whichever way it comes out, because "no promises" and
        // "promises you cannot see" must never look alike (`req:publishable-promise`).
        let promise_note = if promises.is_empty() {
            " NO BEHAVIOURAL PROMISES are published: this surface says what the boundaries ARE \
             and nothing about what they undertake to DO. Read that as \"none stated\", never as \
             \"none exist\" — designate a Requirement `published` to commit to one."
                .to_string()
        } else {
            format!(
                " {} behavioural promise(s) published alongside the structure — commitments a \
                 consumer is entitled to rely on, not just contracts it may call.",
                promises.len()
            )
        };
        let note = if published.is_empty() {
            // The dangerous case: an empty surface looks exactly like a design
            // with nothing to share, and someone could publish it believing they
            // had shared something. Say so unmistakably rather than refusing —
            // "prove I publish nothing" is a legitimate question.
            format!(
                "EMPTY SURFACE: no Interface is designated `published`, so this document exposes \
                 nothing. {withheld_nodes} node(s) and {withheld_edges} edge(s) were withheld as \
                 internal. If you meant to publish a boundary, designate it first \
                 (set_interface_designation) — an empty surface is indistinguishable from a design \
                 with nothing in it.{promise_note}"
            )
        } else {
            format!(
                "Published surface: {} boundary(ies), {} node(s) exposed. WITHHELD as internal: \
                 {withheld_nodes} node(s), {withheld_edges} edge(s) — undesignated requirements, \
                 capabilities, decisions, verifications and history stay home. This is a partial \
                 design by design; it is not a backup and cannot be imported as one.                 {promise_note}{severance_note}",
                published.len(),
                nodes.len()
            )
        };

        let mut document = GraphExport {
            stamp: Some(GraphStamp::current(self.schema())),
            content_hash: None,
            // Deliberately not chained — see the doc comment.
            prev_content_hash: None,
            graph_id: self.graph_id().to_string(),
            nodes,
            edges,
        };
        document.content_hash = Some(document.compute_content_hash());

        Ok(SurfaceExport {
            document,
            published: published.into_iter().collect(),
            withheld_nodes,
            withheld_edges,
            severed_containment,
            note,
        })
    }
}

/// What mirroring another design's published surface did — and, critically, what
/// it refused to do.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MirrorReport {
    /// The `graph_id` of the design mirrored.
    pub mirror_of: String,
    /// The surface document's content hash: the coordinate this mirror is pinned
    /// to, and what makes staleness computable later.
    pub mirror_content_hash: Option<String>,
    /// Nodes brought in as foreign mirrors.
    pub mirrored_nodes: usize,
    /// Edges brought in (only those wholly inside the mirrored set).
    pub mirrored_edges: usize,
    /// Ids in the surface that ALREADY EXIST here and were left untouched.
    ///
    /// The hazard this exists for: `import_graph` is an upsert, so mirroring a
    /// foreign surface whose ids collide with local ones would silently overwrite
    /// your own design with somebody else's nodes. A collision is reported and
    /// skipped, never resolved by guessing — and a non-empty list means the two
    /// designs are using the same id for different things, which is a naming
    /// conversation between two owners, not a merge.
    pub collisions: Vec<String>,
    /// True when this replaced a mirror of the SAME design rather than adding a
    /// new one — the two are different operations and only one of them may
    /// remove nodes.
    pub refreshed: bool,
    /// Ids this refresh REMOVED because the far side no longer publishes them.
    /// Empty on a first mirror, which removes nothing by construction.
    pub withdrawn: Vec<String>,
    /// Said in words, because the counts alone do not tell you whether to worry.
    pub note: String,
}

impl DesignGraph {
    /// Mirror another design's published surface into this graph as **foreign**
    /// nodes carrying the coordinate that says whose they are.
    ///
    /// The first rung of `dec:nested-graphs` option (c): designs are separate
    /// graphs at ownership boundaries and link by mirroring, because an edge
    /// cannot cross a store — the schema validates both endpoints. After
    /// mirroring, your own components `provides`/`consumes` the mirrored
    /// Interface with **ordinary local edges**, so the golden thread, propagate,
    /// and every detector work unchanged. Foreignness is a property of the node,
    /// never of the link.
    ///
    /// The mirrored Project node carries the coordinate — `mirror_of`,
    /// `mirror_content_hash`, `mirrored_at` — which is what makes a later
    /// surface with a different hash detectable as staleness rather than assumed
    /// current (`req:design-composes`, obligation 3).
    ///
    /// **Collisions are refused, not merged.** An id present here already is left
    /// exactly as it is and reported: mirroring must never overwrite your design
    /// with someone else's node, and two designs using one id for different
    /// things is a conversation between owners rather than something a tool may
    /// silently resolve (`dec:ask-not-repair`).
    pub fn mirror_surface(
        &mut self,
        doc: &GraphExport,
        at: Option<&str>,
    ) -> Result<MirrorReport, DynoError> {
        let source = doc.graph_id.clone();
        // `graph_id` became optional for import (BL-138) because import never
        // reads it. Mirroring does: it records `mirror_of` and guards against
        // mirroring a design into itself, and neither question has an answer
        // here. Refused by name rather than mirrored under an empty provenance,
        // which would record a claim nobody made.
        if doc.is_unidentified() {
            return Err(DynoError::Validation {
                node_type: node::PROJECT.into(),
                property: "mirror_of".into(),
                message: "this surface document carries no `graph_id`, so there is no way to \
                          record WHERE it came from or to check it is not this design. \
                          import_graph accepts an unidentified document because it never needs \
                          the answer; mirroring does. Add the source design's `graph_id`, or \
                          use import_graph if you meant to load it as your own."
                    .into(),
            });
        }
        if source == self.graph_id() {
            return Err(DynoError::Validation {
                node_type: node::PROJECT.into(),
                property: "mirror_of".into(),
                message: format!(
                    "this surface came from '{source}', which is this graph — mirroring a design \
                     into itself would overwrite your own nodes with a filtered copy of them. \
                     Import a surface from ANOTHER design, or use import_graph if you meant to \
                     restore a backup."
                ),
            });
        }

        // IS THIS A REFRESH? A Project already carrying this `mirror_of` means we
        // hold a copy of this design, so re-mirroring is "replace what I hold of
        // theirs, at a new pin" — NOT "add a stranger and refuse clashes". Before
        // this, the second mirror of one design collided with its own first
        // (12 of 13 ids, measured 2026-08-12) and left the pin reporting the OLD
        // hash, so the staleness register read FRESH after a failed refresh.
        let held: Vec<MirrorRef> = self
            .mirrors()?
            .into_iter()
            .filter(|m| m.mirror_of == source)
            .collect();
        let refreshed = !held.is_empty();

        // What we currently hold of theirs. Recorded on the mirrored Project at
        // mirror time, because it cannot be recovered afterwards: `provenance:
        // imported` is also set by corpus ingest, and the project's CONTAINS
        // edges do not span the mirrored set (measured: 5 of 13).
        let mut previously: BTreeSet<String> = BTreeSet::new();
        for m in &held {
            if let Some(p) = self.get_node(node::PROJECT, &m.project_id)?
                && let Some(list) = p.properties.get("mirror_nodes").and_then(Value::as_str)
            {
                previously.extend(
                    list.split(',')
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                );
            }
            previously.insert(m.project_id.clone());
        }

        // WOULD THIS WITHDRAW SOMETHING WE CONSUME? A partner dropping a contract
        // you depend on is a conversation, not a cleanup (dec:ask-not-repair), and
        // reflow2 refuses dangling edges anyway — so this refuses BEFORE writing
        // and names both the boundary and what of ours points at it.
        let arriving: BTreeSet<String> = doc.nodes.iter().map(|n| n.node_id.clone()).collect();
        let withdrawn: Vec<String> = previously.difference(&arriving).cloned().collect();
        let mut blocked: Vec<String> = Vec::new();
        for gone in &withdrawn {
            for e in self.incoming(gone, None)? {
                if !previously.contains(&e.from_id) {
                    blocked.push(format!("{} {} -> {}", e.edge_type, e.from_id, gone));
                }
            }
        }
        if !blocked.is_empty() {
            return Err(DynoError::Validation {
                node_type: node::PROJECT.into(),
                property: "mirror_of".into(),
                message: format!(
                    "REFUSED: this surface from '{source}' no longer publishes {} boundary(ies) \
                     that YOUR design still points at, and nothing was changed. Withdrawn: {}. \
                     Your edges: {}.\n\nA partner retiring a contract you consume is a \
                     conversation between the two owners, not a cleanup this tool may do for \
                     you. Re-point or remove your own edge first, then mirror again.",
                    withdrawn.len(),
                    withdrawn.join(", "),
                    blocked.join(", "),
                ),
            });
        }

        // Nothing of ours depends on what went, so replace what we hold. Removal
        // first, so a renamed node does not read as a collision with its own
        // former self.
        if refreshed {
            for gone in &previously {
                if let Some(t) = self.node_type_index()?.get(gone) {
                    let t = t.clone();
                    self.delete_node(&t, gone)?;
                }
            }
        }

        let existing = self.node_type_index()?;
        let mut collisions = Vec::new();
        let mut mirrored: BTreeSet<String> = BTreeSet::new();

        self.begin_batch();
        let result = (|| -> Result<(usize, usize), DynoError> {
            let mut nodes_written = 0;
            for n in &doc.nodes {
                if existing.contains_key(&n.node_id) {
                    collisions.push(n.node_id.clone());
                    continue;
                }
                let mut props: std::collections::HashMap<String, Value> =
                    n.properties.clone().into_iter().collect();
                // Every mirrored node says how it got here. `imported` is the
                // existing provenance value for exactly this (BL-45's "imported
                // reference nodes aren't marked foreign" — now they are).
                props.insert("provenance".into(), Value::from("imported"));
                if n.node_type == node::PROJECT {
                    props.insert("mirror_of".into(), Value::from(source.as_str()));
                    if let Some(hash) = &doc.content_hash {
                        props.insert("mirror_content_hash".into(), Value::from(hash.as_str()));
                    }
                    if let Some(at) = at {
                        props.insert("mirrored_at".into(), Value::from(at));
                    }
                }
                self.create_node(&n.node_type, &n.node_id, props)?;
                mirrored.insert(n.node_id.clone());
                nodes_written += 1;
            }
            let mut edges_written = 0;
            for e in &doc.edges {
                // Only edges wholly inside what we actually mirrored. An edge
                // touching a collided id is dropped rather than rewired, because
                // pointing their edge at OUR same-named node would fabricate a
                // relationship neither design asserted.
                if mirrored.contains(&e.from_id) && mirrored.contains(&e.to_id) {
                    let props: std::collections::HashMap<String, Value> =
                        e.properties.clone().into_iter().collect();
                    let (from_type, to_type) = (
                        doc.nodes
                            .iter()
                            .find(|n| n.node_id == e.from_id)
                            .map(|n| n.node_type.as_str())
                            .unwrap_or_default(),
                        doc.nodes
                            .iter()
                            .find(|n| n.node_id == e.to_id)
                            .map(|n| n.node_type.as_str())
                            .unwrap_or_default(),
                    );
                    self.create_edge(
                        &e.edge_type,
                        from_type,
                        &e.from_id,
                        to_type,
                        &e.to_id,
                        props,
                    )?;
                    edges_written += 1;
                }
            }
            Ok((nodes_written, edges_written))
        })();
        match result {
            Ok((mirrored_nodes, mirrored_edges)) => {
                self.commit_batch()?;
                let note = if collisions.is_empty() {
                    format!(
                        "Mirrored {mirrored_nodes} node(s) and {mirrored_edges} edge(s) from \
                         '{source}', pinned to that surface's content hash. They are marked \
                         `imported` and are not yours to edit: link to them with your own \
                         provides/consumes edges instead. A newer surface with a different hash \
                         means this mirror is stale."
                    )
                } else {
                    format!(
                        "Mirrored {mirrored_nodes} node(s) and {mirrored_edges} edge(s) from \
                         '{source}'. REFUSED {} id(s) that already exist here and were left \
                         untouched: {}. Two designs using one id for different things is a naming \
                         conversation between their owners — nothing was overwritten and nothing \
                         was guessed.",
                        collisions.len(),
                        collisions.join(", ")
                    )
                };
                // WHAT WE HOLD OF THEIRS, recorded on their mirrored Project so a
                // later refresh can replace exactly this set. It cannot be
                // recovered afterwards — `provenance: imported` is also set by
                // corpus ingest, and the project's CONTAINS edges do not span the
                // mirrored set (5 of 13, measured). A comma-joined id list follows
                // the schema's existing precedent (`pinned` / `swept` on VERIFIES).
                if let Some(proj) = doc
                    .nodes
                    .iter()
                    .find(|n| n.node_type == node::PROJECT && mirrored.contains(&n.node_id))
                {
                    let list = mirrored.iter().cloned().collect::<Vec<_>>().join(",");
                    // Re-written whole: core `create_node` validates the entire
                    // node, so a partial props object is refused here even though
                    // the MCP layer merges.
                    if let Some(stored) = self.get_node(node::PROJECT, &proj.node_id)? {
                        let mut props: std::collections::HashMap<String, Value> =
                            stored.properties.into_iter().collect();
                        props.insert("mirror_nodes".into(), Value::from(list.as_str()));
                        self.create_node(node::PROJECT, &proj.node_id, props)?;
                    }
                }

                Ok(MirrorReport {
                    mirror_of: source,
                    mirror_content_hash: doc.content_hash.clone(),
                    refreshed,
                    withdrawn: withdrawn.clone(),
                    mirrored_nodes,
                    mirrored_edges,
                    collisions,
                    note,
                })
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }

    /// The designs this graph mirrors: `(project_id, mirror_of, content_hash,
    /// mirrored_at)`, sorted — who we are composed with, and at what version.
    pub fn mirrors(&self) -> Result<Vec<MirrorRef>, DynoError> {
        let mut out = Vec::new();
        for p in self.scan_nodes(node::PROJECT)? {
            let Some(mirror_of) = p.properties.get("mirror_of").and_then(Value::as_str) else {
                continue;
            };
            out.push(MirrorRef {
                project_id: p.node_id.clone(),
                mirror_of: mirror_of.to_string(),
                mirror_content_hash: p
                    .properties
                    .get("mirror_content_hash")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                mirrored_at: p
                    .properties
                    .get("mirrored_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        out.sort_by(|a, b| a.project_id.cmp(&b.project_id));
        Ok(out)
    }
}

/// One design this graph is composed with, and the version it was pinned to.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MirrorRef {
    pub project_id: String,
    pub mirror_of: String,
    pub mirror_content_hash: Option<String>,
    pub mirrored_at: Option<String>,
}
