//! `exchange` tools — one slice of the MCP surface.
//!
//! Split out of `service.rs` under BL-181, which had grown to 6,356 lines and
//! 139 tools in one file: the design distinguished the systems these tools
//! serve and the build did not separate them at all. That mismatch is what
//! `granularity_report` reported, and this is the answer to it.
//!
//! **Function is unchanged by construction.** Every item here moved verbatim;
//! nothing was rewritten. `rmcp` composes routers, so this module declares its
//! own and `ReflowService::new` sums them — the surface a client sees is
//! byte-identical, which `tools/toolsnap.py` is what proves rather than claims.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use tokio::sync::RwLock;

use reflow2_core::bulk::{
    AskedRecord as BulkAskedRecord, ChecksumAccept as BulkChecksumAccept, EdgeSpec as BulkEdgeSpec,
    GapAck as BulkGapAck, NodeSpec as BulkNodeSpec,
};
use reflow2_core::temporal::ChangeRecord;
use reflow2_core::{
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, DEFAULT_SCOPE_DEPTH, DesignGraph,
    Dimension, DriftDisposition, DynoError, EpochType, GapCandidate, GenesisOptions, HealOptions,
    HealProposal, HealStrategy, IngestOptions, LinkArtifactOptions, LoopStatus, ObservedArtifact,
    ObservedPath, PromptCollector, PropagateOptions, ReadinessForecast, ReadinessGate,
    ReadinessKind, ReadinessObservation, ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

#[tool_router(router = exchange_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Compute the seam between this design and another by COMPLEMENTARY ROLE, \
                       instead of hand-asserting which boundaries correspond. Each boundary \
                       declares a role on `Interface.designation` and pairing matches \
                       COMPLEMENTS — `published`/`both` against `required`/`both` — never like \
                       with like, the way a base pairs with its complement and not a copy of \
                       itself. Two boundaries pair when their NAMES match fuzzily AND they agree \
                       on medium, transport_security and auth. FIVE OUTCOMES, all useful: paired \
                       (the seam, computed); CONFLICTS, where the names match but the axes refuse \
                       — reported with EVERY refusing axis, never dropped as a non-match, because \
                       \"you publish this, I need this, and we cannot connect as either is built\" \
                       is the finding worth having; unmet needs (we require it, nobody publishes \
                       it — the loudest signal); dead surface (they publish it, nobody here needs \
                       it); and duplicate providers (two publishers of one need is a conflict, \
                       not a match). Uncertain name matches are CANDIDATES to ask about, never \
                       actions. Boundaries carrying no role are counted and NAMED, because \
                       `internal` is the DEFAULT and cannot tell \"deliberately internal\" from \
                       \"never classified\" — otherwise a design that did no labelling reports a \
                       clean seam. Feed `paired` to seam_report to learn whether the full \
                       contracts agree (req:complementary-pairing).",
        annotations(read_only_hint = true)
    )]
    pub async fn pair_designs(
        &self,
        Parameters(req): Parameters<PairDesignsReq>,
    ) -> Result<CallToolResult, McpError> {
        let other: reflow2_core::GraphExport =
            serde_json::from_value(JsonValue::Object(req.design)).map_err(|e| {
                McpError::invalid_params(format!("not an export document: {e}"), None)
            })?;
        let g = self.graph.read().await;
        ok_json(g.pair_designs(&other).map_err(dyno_err)?)
    }

    #[tool(
        description = "Compare paired boundaries across a seam and say where two designs \
                       DISAGREE — the check the ordinary detectors cannot do, because they \
                       reason about structure and a contract mismatch is a comparison of \
                       PROPERTIES ACROSS A PAIR. Compares medium, paradigm, payload format, \
                       auth, transport security, operations, error model and payload schema. \
                       THREE RULES WORTH KNOWING: `unspecified` on either side reports as \
                       UNSTATED, never as agreement, so 0 incompatibilities can never be read \
                       as compatible; free-text axes report as DIFFERS for a person to read, \
                       never as incompatible, because a machine cannot tell a real mismatch \
                       from different wording; and the report always names what it did NOT \
                       examine — the types that CROSS these boundaries are part of the contract \
                       and are invisible to it.",
        annotations(read_only_hint = true)
    )]
    pub async fn seam_report(
        &self,
        Parameters(req): Parameters<SeamReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let other: reflow2_core::GraphExport =
            serde_json::from_value(JsonValue::Object(req.design)).map_err(|e| {
                McpError::invalid_params(format!("not an export document: {e}"), None)
            })?;
        let pairs: Vec<(String, String)> =
            req.pairs.into_iter().map(|p| (p.ours, p.theirs)).collect();
        let g = self.graph.read().await;
        ok_json(g.seam_report(&other, &pairs).map_err(dyno_err)?)
    }

    #[tool(
        description = "The whole design as one portable document — every node and edge, sorted so \
                       two exports of an unchanged graph are byte-identical. Use it to back the \
                       design up, move it between machines, or migrate it across a reflow2 upgrade \
                       (export with the old build, import with the new). It carries a stamp saying \
                       which reflow2 wrote it. Pass `path` to write the document to a file instead \
                       of returning it — on a large design the payload overflows what a session \
                       can read, and a backup wants to be a file anyway. CONVENTION: export ONCE \
                       between commits, straight onto the committed file — the lineage link is \
                       built from whatever file is already at that path, so exporting elsewhere \
                       and copying it in, or exporting twice, both break the chain silently.",
        annotations(read_only_hint = true)
    )]
    pub async fn export_graph(
        &self,
        Parameters(req): Parameters<ExportGraphToReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let mut export = g.export_graph().map_err(dyno_err)?;
        let Some(path) = req.path else {
            return ok_json(export);
        };
        // Refuse to clobber an existing file unless the caller opts in. Graph
        // text is untrusted (the server's own instructions say so), so a stray
        // or injected `path` pointing at a real file must not silently destroy
        // it (BL-57). A new path writes freely.
        let target = std::path::Path::new(&path);
        if target.exists() && !req.overwrite.unwrap_or(false) {
            return Err(McpError::invalid_params(
                format!(
                    "{path} already exists — refusing to overwrite it. Pass overwrite=true \
                     to replace it, or choose a path that does not exist."
                ),
                None,
            ));
        }
        // The file-write seam is where lineage lives (dec:export-hash-chain):
        // replacing an export file links the new document to the old one's
        // content hash — advancing only when content actually changed, so an
        // unchanged design still writes byte-identical files. A file that is
        // not a reflow2 export records no chain, and says so in the receipt.
        let mut chain_note = None;
        let mut sync_note = None;
        // WHAT THIS WRITE ACTUALLY DID, because the receipt could not say.
        //
        // `content_hash` and `prev_content_hash` DO NOT ANSWER IT. Measured on
        // 0.31.0 across a five-export chain: an export that changed the file and
        // one that changed nothing return **byte-identical receipts** — same
        // content hash, same prev hash — because `chain_after` gives an
        // unchanged export the predecessor's own `prev`. In both cases
        // `content_hash != prev_content_hash`, so that difference discriminates
        // nothing.
        //
        // It matters because of who hits it. On a `--shared` server a peer's
        // export publishes YOUR in-flight work (measured: 28 nodes once, 17 the
        // next), and your own export afterwards is then a no-op — which read to
        // the seat that hit it as a FAILED SAVE. Reported five times by three
        // seats before this existed. `sync_status` answers the other direction
        // and says out loud that it declines this one.
        //
        // Same principle as the `revision` block on the constructors and the
        // who-edge refusals: two different facts must not share one reply.
        let mut wrote = "created";
        if target.exists() {
            match std::fs::read_to_string(target)
                .ok()
                .and_then(|raw| serde_json::from_str::<reflow2_core::GraphExport>(&raw).ok())
            {
                Some(predecessor) => {
                    // req:stale-seat-knows. Before the lineage link, the
                    // question git answers with a non-fast-forward refusal:
                    // would writing this drop design the file already holds?
                    // Only the lossy case stops — see reflow2_core::sync.
                    let last = self
                        .graph_path
                        .as_deref()
                        .and_then(|g| reflow2_core::provenance::last_synced(g, &path));
                    let verdict = reflow2_core::sync::assess_overwrite(
                        Some(&predecessor),
                        &export,
                        last.as_deref(),
                    );
                    if verdict.is_loss() && !req.accept_divergence.unwrap_or(false) {
                        return Err(McpError::invalid_params(
                            verdict.message(&path).unwrap_or_default(),
                            None,
                        ));
                    }
                    sync_note = verdict.message(&path);
                    wrote = if predecessor.effective_content_hash()
                        == export.effective_content_hash()
                    {
                        "unchanged"
                    } else {
                        "changed"
                    };
                    export.chain_after(&predecessor);
                }
                None => {
                    wrote = "changed";
                    chain_note = Some(
                        "the file being replaced was not a reflow2 export — no lineage recorded",
                    );
                }
            }
        }
        // Through `serde_json::Value` so keys serialize sorted (its object is a
        // BTreeMap) — the same convention as the committed design export, so a
        // file this writes diffs cleanly against one written before it.
        let v = serde_json::to_value(&export).map_err(ser_err)?;
        let text = format!("{}\n", serde_json::to_string_pretty(&v).map_err(ser_err)?);
        std::fs::write(target, &text).map_err(|e| {
            // A path the caller supplied that cannot be written is the caller's
            // mistake, not a server fault.
            McpError::invalid_params(format!("cannot write export to {path}: {e}"), None)
        })?;
        // This seat is now in step with what it just wrote — so the next
        // export takes the one-hash fast path instead of comparing documents,
        // and a file that moves after this is detectable (req:stale-seat-knows).
        if let (Some(graph_path), Some(hash)) = (self.graph_path.as_deref(), &export.content_hash) {
            reflow2_core::provenance::record_sync(graph_path, &path, hash);
        }
        // Report where it actually landed: a relative path resolves against the
        // server's cwd, which the calling agent cannot see.
        let resolved = std::fs::canonicalize(target)
            .map(|p| p.display().to_string())
            .unwrap_or(path);
        let mut receipt = json!({
            "path": resolved,
            "bytes": text.len(),
            "nodes": export.nodes.len(),
            "edges": export.edges.len(),
            "content_hash": export.content_hash,
            "prev_content_hash": export.prev_content_hash,
            "wrote": wrote,
            "stamp": serde_json::to_value(&export.stamp).map_err(ser_err)?,
        });
        if wrote == "unchanged" {
            // Only on the case that misleads. A note present on every successful
            // export is the noise `search_first` and the `revision` block both
            // refuse to become.
            receipt["wrote_note"] = json!(
                "NOTHING WAS WRITTEN — the file already held exactly this design, byte for byte. \
                 That is not a failure and not a refusal, and it is worth reading twice on a \
                 shared server: it means somebody else's export already carried your work, so \
                 there was nothing left for yours to add. `content_hash` and `prev_content_hash` \
                 look identical to a successful write and cannot tell you this."
            );
        }
        if let Some(note) = chain_note {
            receipt["chain_note"] = json!(note);
        }
        if let Some(note) = sync_note {
            receipt["sync_note"] = json!(note);
        }
        ok_json(receipt)
    }

    #[tool(
        description = "Export ONLY the published surface — the contracts others are entitled to \
                       rely on, and nothing internal. Every Interface designated `published`, the \
                       artifacts that specify or realize it (the machine-readable ICD), the \
                       components on each side, and the project. Requirements, capabilities, \
                       decisions, verifications and history stay home, and the result COUNTS what \
                       it withheld — a recipient cannot tell a small design from a filtered one, \
                       so the note says which they are holding. Use it to hand a boundary to \
                       another team or a vendor without handing over the design. Deliberately not \
                       part of the export hash chain: this is a derived view, not a record of the \
                       design, and it is not a backup. A design with no designated boundary gets \
                       an EMPTY SURFACE warning rather than a quietly empty file.",
        annotations(read_only_hint = true)
    )]
    pub async fn export_surface(
        &self,
        Parameters(req): Parameters<ExportSurfaceReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let surface = g.export_surface().map_err(dyno_err)?;
        match req.path.as_deref() {
            None => ok_json(surface),
            Some(path) => {
                let rendered = serde_json::to_string_pretty(&surface.document).map_err(ser_err)?;
                if !req.overwrite.unwrap_or(false) && std::path::Path::new(path).exists() {
                    return Err(McpError::invalid_params(
                        format!(
                            "{path} already exists — pass overwrite: true to replace it. A \
                             published surface is meant to be shared, so clobbering one silently \
                             could replace what a consumer is building against."
                        ),
                        None,
                    ));
                }
                std::fs::write(path, format!("{rendered}\n")).map_err(|e| {
                    McpError::internal_error(format!("failed to write {path}: {e}"), None)
                })?;
                ok_json(json!({
                    "path": path,
                    "published": surface.published,
                    "nodes": surface.document.nodes.len(),
                    "edges": surface.document.edges.len(),
                    "withheld_nodes": surface.withheld_nodes,
                    "withheld_edges": surface.withheld_edges,
                    "content_hash": surface.document.content_hash,
                    "note": surface.note,
                }))
            }
        }
    }

    #[tool(
        description = "Mirror ANOTHER design's published surface into this graph as foreign nodes \
                       carrying the coordinate that says whose they are — which design, at what \
                       content hash, when. The composition step of dec:nested-graphs option (c): \
                       designs are separate graphs at ownership boundaries and link by mirroring, \
                       because an edge cannot cross a store. Afterwards your own components \
                       provides/consumes the mirrored Interface with ORDINARY local edges, so the \
                       golden thread, propagate and every detector work unchanged, and foreignness \
                       is a property of the node rather than of the link. COLLISIONS ARE REFUSED, \
                       never merged: an id that already exists here is left untouched and \
                       reported, because upsert would otherwise overwrite your design with \
                       somebody else's node, and two designs using one id for different things is \
                       a naming conversation between owners.",
        annotations(read_only_hint = false)
    )]
    pub async fn mirror_surface(
        &self,
        Parameters(req): Parameters<MirrorSurfaceReq>,
    ) -> Result<CallToolResult, McpError> {
        let doc: reflow2_core::GraphExport =
            serde_json::from_value(JsonValue::Object(req.document)).map_err(|e| {
                McpError::invalid_params(format!("not a reflow2 surface document: {e}"), None)
            })?;
        let mut g = self.write_lock().await?;
        ok_json(
            g.mirror_surface(&doc, req.at.as_deref())
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "The designs this one is composed with, and the version each was pinned to: \
                       project id, source graph, surface content hash, and when the mirror was \
                       taken. A mirror is a dated claim about a VERSION of another design, never a \
                       live truth, so this is the list to re-check when a partner publishes again.",
        annotations(read_only_hint = true)
    )]
    pub async fn mirrors(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        self.ok_read(&g, g.mirrors().map_err(dyno_err)?)
    }

    #[tool(
        description = "Load an exported design into this graph. THE DOCUMENT SHAPE, which an export of an \
                       empty graph cannot teach you: \
                       {\"nodes\":[{\"node_type\":\"Requirement\",\"node_id\":\"req:x\",\
                       \"properties\":{...}}],\"edges\":[{\"edge_type\":\"SATISFIES\",\
                       \"from_id\":\"cap:x\",\"to_id\":\"req:x\",\"properties\":{}}]}. \
                       That is the whole required envelope — `graph_id`, `stamp`, `content_hash` \
                       and `prev_content_hash` are all OPTIONAL on the way in, and `edges` may be \
                       omitted entirely. Endpoint types are not stored on an edge; they are \
                       recovered from the nodes in the same document or from this graph. Use \
                       describe_schema for the properties each node_type takes. \
                       EACH NODE MUST BE COMPLETE: validation applies to the whole node, so a \
                       partial node is refused rather than merged into the one already there — \
                       unlike create_node, where a partial props object edits. Re-importing a \
                       corrected node means sending all of its properties, not just the changed \
                       one. \
                       Upsert, not replace: ids already present are overwritten and anything not \
                       in the document is left alone, so clear the graph first if you want a \
                       clean restore. Atomic — a document that fails validation leaves the graph \
                       untouched rather than half-loaded — and EVERY invalid item is reported in \
                       one response with its position. Reports any edge whose endpoints were missing \
                       rather than dropping it. \
                       IDENTITY: an EMPTY store adopts the document's `graph_id` (reported as \
                       `adopted_identity`) instead of renaming the design; a store already \
                       holding one keeps its name.",
        annotations(read_only_hint = false)
    )]
    pub async fn import_graph(
        &self,
        Parameters(req): Parameters<ImportGraphReq>,
    ) -> Result<CallToolResult, McpError> {
        let doc: reflow2_core::GraphExport = match (req.document, &req.path) {
            (Some(document), None) => parse_struct_param(document, "reflow2 export")?,
            (None, Some(path)) => read_export_document(path)?,
            (Some(_), Some(_)) => {
                return Err(McpError::invalid_params(
                    "pass document OR path, not both — with two sources there is no way to say                      which one was imported."
                        .to_string(),
                    None,
                ));
            }
            (None, None) => {
                return Err(McpError::invalid_params(
                    "nothing to import: pass document (an export payload) or path (a file)."
                        .to_string(),
                    None,
                ));
            }
        };
        let mut g = self.write_lock().await?;
        let report = g.import_graph(&doc).map_err(dyno_err)?;
        // Absorbing a file puts this seat in step with it, which is exactly
        // what the stale-seat refusal tells people to do — so record it, or the
        // remedy would not clear the condition it names (req:stale-seat-knows).
        if let (Some(graph_path), Some(path), Some(hash)) =
            (self.graph_path.as_deref(), &req.path, &doc.content_hash)
        {
            reflow2_core::provenance::record_sync(graph_path, path, hash);
        }
        ok_json(report)
    }

    #[tool(
        description = "Compare two as-designed records — the design-vs-design sibling of the \
                       reconcile family, which only ever compares design against reality. \
                       Findings are directional relative to the named base: `added` / `removed` \
                       / `changed` (property-level), banded into design content vs the \
                       supporting layer (change events, questions, provenance). Pass base_path \
                       alone to compare the live graph against a committed export ('has this \
                       session diverged from the record?'); pass other_path too to compare two \
                       export files (branches, machines, alternatives). Reports divergence, \
                       never judges which side is right.",
        annotations(read_only_hint = true)
    )]
    pub async fn compare_designs(
        &self,
        Parameters(req): Parameters<CompareDesignsReq>,
    ) -> Result<CallToolResult, McpError> {
        let base = read_export_document(&req.base_path)?;
        match &req.other_path {
            Some(other_path) => {
                let other = read_export_document(other_path)?;
                ok_json(reflow2_core::compare_designs(
                    &base,
                    &other,
                    &req.base_path,
                    other_path,
                ))
            }
            None => {
                let g = self.graph.read().await;
                ok_json(
                    g.compare_with_base(&base, &req.base_path)
                        .map_err(dyno_err)?,
                )
            }
        }
    }

    #[tool(
        description = "Decide whether a restructuring PRESERVED FUNCTION — compare_designs' \
                       verdict-bearing sibling. A maturity restructuring holds the function set \
                       invariant and moves everything else (allocation, packaging, which \
                       functions live in which component, which seams are declared), and it is \
                       safe exactly when function is provably unchanged. That is computable, so \
                       this CERTIFIES rather than asserts: every divergence is classified \
                       function / structure / supporting and the verdict is `preserved`, \
                       `not_preserved` or `indeterminate`. NOTHING IS WAVED THROUGH — a node \
                       type, an edge endpoint or a property edit the rules cannot place lands in \
                       `unclassified` and forces `indeterminate`, because a classifier that has \
                       not been taught part of the vocabulary must not certify a design it never \
                       examined. A reworded capability is undecidable by construction (a rename \
                       and a scope change are the same bytes) and comes back with both values \
                       for a human. `not_certified_about` is on every certificate INCLUDING a \
                       clean one: this reads two design records and has read no code, so it \
                       never claims the implementation preserved behaviour.",
        annotations(read_only_hint = true)
    )]
    pub async fn certify_preservation(
        &self,
        Parameters(req): Parameters<CertifyPreservationReq>,
    ) -> Result<CallToolResult, McpError> {
        let base = read_export_document(&req.base_path)?;
        match &req.other_path {
            Some(other_path) => {
                let other = read_export_document(other_path)?;
                let diff = reflow2_core::compare_designs(&base, &other, &req.base_path, other_path);
                ok_json(reflow2_core::certify_preservation(&diff, &base, &other))
            }
            None => {
                let g = self.graph.read().await;
                ok_json(
                    g.certify_preservation_against(&base, &req.base_path)
                        .map_err(dyno_err)?,
                )
            }
        }
    }

    #[tool(
        description = "Propose a three-way merge of two divergent designs against their common \
                       ancestor — compare's write-side sibling (BL-80). Runs git's trivial-merge \
                       case table per node and per property over typed values: only one side \
                       changed → take it; both changed the same way → take it; both changed \
                       differently → a conflict, surfaced as a Question for the human, never \
                       guessed. A node one side deleted and the other changed is retained and \
                       asked (deletion must be re-justified); edges get the identical rule. Pass \
                       base_path (the ancestor — e.g. git merge-base + the committed export at \
                       that commit), ours_path (merge into) and theirs_path (merge in). This is a \
                       PROPOSAL: it writes nothing. Applying the resolved merge is a separate, \
                       explicit step.",
        annotations(read_only_hint = true)
    )]
    pub async fn merge_designs(
        &self,
        Parameters(req): Parameters<MergeDesignsReq>,
    ) -> Result<CallToolResult, McpError> {
        let base = read_export_document(&req.base_path)?;
        let ours = read_export_document(&req.ours_path)?;
        let theirs = read_export_document(&req.theirs_path)?;
        ok_json(reflow2_core::merge_designs(
            &base,
            &ours,
            &theirs,
            &req.base_path,
            &req.ours_path,
            &req.theirs_path,
        ))
    }

    #[tool(
        description = "Apply a resolved three-way merge into the live design — the write side of \
                       merge_designs (BL-80). `ours` is the live graph at --graph-path; this \
                       merges `theirs` into it against the common ancestor `base`, making the live \
                       design equal the merged result, atomically. Pass `resolutions` — one \
                       decision per conflict (its `merge:…` id → base/ours/theirs, from a prior \
                       merge_designs run). It REFUSES and writes nothing if any conflict is \
                       undecided, or a decision names no conflict. This is the explicit commit the \
                       proposal is designed around: run merge_designs first, decide the conflicts, \
                       then apply.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn apply_merge(
        &self,
        Parameters(req): Parameters<ApplyMergeReq>,
    ) -> Result<CallToolResult, McpError> {
        let base = read_export_document(&req.base_path)?;
        let theirs = read_export_document(&req.theirs_path)?;
        let mut resolutions = std::collections::BTreeMap::new();
        for (id, choice) in &req.resolutions {
            let parsed = reflow2_core::Resolution::parse(choice).ok_or_else(|| {
                dyno_err(reflow2_core::DynoError::Validation {
                    node_type: "merge".into(),
                    property: "resolutions".into(),
                    message: format!(
                        "conflict '{id}' has resolution '{choice}', which is not one of \
                         base/ours/theirs"
                    ),
                })
            })?;
            resolutions.insert(id.clone(), parsed);
        }
        let mut g = self.write_lock().await?;
        ok_json(
            g.apply_merge(&base, &theirs, &resolutions, req.use_recorded)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Recall recorded conflict resolutions (rerere) by their content keys — the \
                       advisory half of merge (BL-80 #5). Pass the `resolution_key`s (`rr:…`) that \
                       merge_designs put on its conflicts; returns, for each one previously \
                       resolved, the recorded decision (base/ours/theirs). Because the key is the \
                       conflict's *content* (values, not location), one recorded decision is \
                       recalled for every node with the identical conflict — resolve the shape \
                       once, then apply_merge with use_recorded, or feed these suggestions back as \
                       explicit resolutions. A suggestion, never an auto-decision.",
        annotations(read_only_hint = true)
    )]
    pub async fn recall_resolutions(
        &self,
        Parameters(req): Parameters<RecallResolutionsReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(
            g.recall_resolutions(&req.resolution_keys)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Compare parallel design alternatives on the same measures — an analysis of \
                       alternatives (BL-70). Pass the paths to two or more alternative export \
                       documents (branch-by-file); the first is the baseline. Returns each branch's \
                       measures side by side — design nodes, open gaps, structural defects, \
                       allocation modularity, capabilities verified — plus every non-baseline \
                       branch's structural divergence from the baseline (added/removed/changed). \
                       Makes alternatives comparable on measures, not advocacy; it opens its own \
                       throwaway graphs, so it never touches and is never blocked by the live one. \
                       Collapse the winner with merge_designs/apply_merge and retire the losers.",
        annotations(read_only_hint = true)
    )]
    pub async fn analyze_alternatives(
        &self,
        Parameters(req): Parameters<AnalyzeAlternativesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut alternatives = Vec::with_capacity(req.paths.len());
        for p in &req.paths {
            alternatives.push((p.clone(), read_export_document(p)?));
        }
        ok_json(reflow2_core::analyze_alternatives(&alternatives).map_err(dyno_err)?)
    }

    #[tool(
        description = "Set a Decision's lifecycle status — proposed / accepted / superseded / \
                       rejected (BL-70). Setting it to `proposed` opens it as a decision point: an \
                       undecided fork you can register alternatives under. Every other property is \
                       preserved.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn set_decision_status(
        &self,
        Parameters(req): Parameters<SetDecisionStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        ok_json(NodeDto::from(
            g.set_decision_status(&req.decision_id, &req.status)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "WHICH NODES MIGHT THIS ONE RELATE TO \u{2014} offered, never drawn. The missing \
                       half of half-idea linking: `unreviewed_ideas` counts the ideas connected to \
                       nothing, `review_relations` records the judgement, and until now NOTHING \
                       answered the question a person working that backlog actually has \u{2014} which of \
                       these belong together? \u{2b50} EVERY CANDIDATE CARRIES THE WALK THAT PRODUCED IT \
                       in `because`: a shared neighbour (two nodes that both relate to a third are \
                       related in the graph\u{2019}s own terms, whatever their words) or distinctive shared \
                       terms (weighted by rarity ACROSS THE POOL, so words true of everything count \
                       for nothing). A candidate whose reason cannot be stated is not offered. \
                       \u{1f6d1} IT NEVER WRITES AND NEVER PROPOSES AN EDGE. Ranking is the machine\u{2019}s half; \
                       drawing is yours, through `review_relations`, and a false neighbour is worse \
                       than a missing one because anything searching by neighbourhood repeats it \
                       forever. Nodes ALREADY related are excluded from the ranking and returned in \
                       `already_related`, so \u{2018}not offered because already linked\u{2019} is distinguishable \
                       from \u{2018}not offered because nothing matched\u{2019}. \u{26a0} AN EMPTY ANSWER SAYS WHICH \
                       EMPTY IT IS \u{2014} `empty_because` separates \u{2018}nothing to compare against\u{2019}, \u{2018}this \
                       node carries no comparable text\u{2019} and \u{2018}ranked N and none matched\u{2019}, because only \
                       the last means the node may be genuinely new. Needs no search index and no \
                       database, so it works in an index-less build.",
        annotations(read_only_hint = true)
    )]
    pub async fn relation_candidates(
        &self,
        Parameters(req): Parameters<RelationCandidatesReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(
            g.relation_candidates(
                &req.node_type,
                &req.node_id,
                req.pool_type.as_deref(),
                req.limit.unwrap_or(5),
            )
            .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Declare that this Decision states the quality attribute the design is \
                       BUILT FOR — the answer to \"what is this system for?\". A TARGET, not a \
                       score: DimensionAssessment records what a system IS on an axis, this \
                       records what it is aiming at. ⭐ IT MATTERS BECAUSE THE ATTRIBUTE DECIDES \
                       WHICH GROUPING IS RIGHT, and the four disagree — performance wants least \
                       chatter across boundaries, reliability wants no articulation point and may \
                       deliberately DUPLICATE a function across parts, maintainability wants \
                       what-changes-together-to-live-together, security wants boundaries \
                       following trust rather than coupling. Allocating without this answer \
                       silently picks performance. ASK IT EARLY: asking is cheap and shapes \
                       everything downstream, while the cost of asking late is reworking \
                       services already built — which is why genesis asks it and allocation, \
                       correctly, still waits for the last responsible moment. 🛑 THERE IS NO \
                       `none`: absence means nobody was asked, a different fact from a design \
                       that weighed it and chose, and `quality_target_unstated` reads exactly \
                       that difference. Every other property is preserved; link what the choice \
                       shapes with governed_by.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn set_quality_target(
        &self,
        Parameters(req): Parameters<SetQualityTargetReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        ok_json(NodeDto::from(
            g.set_quality_target(&req.decision_id, &req.quality_target)
                .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Register an alternative under a proposed decision point (BL-70): a \
                       lightweight Artifact pointer that names where the alternative's design \
                       export lives (branch-by-file), GOVERNED_BY the Decision and CONTRADICTS its \
                       siblings. Refuses unless the Decision is `proposed` — you fork an open \
                       choice, not a settled one. Compare the registered alternatives with \
                       analyze_alternatives, then collapse_decision to choose.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn register_alternative(
        &self,
        Parameters(req): Parameters<RegisterAlternativeReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        ok_json(
            g.register_alternative(&req.decision_id, &req.artifact_id, &req.name, &req.location)
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "List the alternatives registered under a decision point (BL-70) — the \
                       Artifact pointers GOVERNED_BY the Decision, with their export locations. \
                       Feed the locations to analyze_alternatives to compare them.",
        annotations(read_only_hint = true)
    )]
    pub async fn alternatives_for(
        &self,
        Parameters(req): Parameters<AlternativesForReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.alternatives_for(&req.decision_id).map_err(dyno_err)?)
    }

    #[tool(
        description = "Collapse a decision point (BL-70): choose the winning alternative. The \
                       Decision moves to `accepted`, the losing alternatives are superseded \
                       (OBSOLETES — retired on the record, not deleted), and the outcome is \
                       written into the Decision's own `alternatives` field with the rationale. \
                       This records the choice; merge the winner's design content into the \
                       baseline separately with apply_merge.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    pub async fn collapse_decision(
        &self,
        Parameters(req): Parameters<CollapseDecisionReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        ok_json(
            g.collapse_decision(&req.decision_id, &req.winner_id, req.note.as_deref())
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Analyse THIS design together with another one — a dependency, a partner \
                       system — and report what only shows up when both are present. Rather than \
                       comparing them, it imports theirs alongside yours and runs reflow2's \
                       ORDINARY checks over the whole, so seam problems arrive as the gaps they \
                       already are: a contract with no provider once both sides are visible, a \
                       requirement nothing satisfies across the join, a duplicate that is one \
                       thing named twice. Findings are attributed OURS / THEIRS / SEAM, and the \
                       seam ones are what neither design could have found alone. NOTHING IS \
                       WRITTEN: the combined graph is built in memory and thrown away, so your \
                       design is unchanged and your exports never start carrying theirs. Ids are \
                       namespaced, because two designs routinely name different things the same \
                       and a plain import would silently overwrite yours.",
        annotations(read_only_hint = true)
    )]
    pub async fn compose_and_analyse(
        &self,
        Parameters(req): Parameters<ComposeReq>,
    ) -> Result<CallToolResult, McpError> {
        let doc = serde_json::from_value(JsonValue::Object(req.design))
            .map_err(|e| McpError::invalid_params(format!("invalid design document: {e}"), None))?;
        let g = self.graph.read().await;
        ok_json(
            g.compose_and_analyse(&doc, &req.namespace)
                .map_err(dyno_err)?,
        )
    }
}
