//! `built` tools — one slice of the MCP surface.
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
        ServerConfig,
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

/// Does this artifact already carry a drift baseline? Decides whether a
/// `design_holds` accept must say why the code moved: with a baseline the
/// code moved against it and the reason is asked; without one the core reads
/// the accept as a FIRST baseline, nothing moved, and there is nothing to ask.
/// An unknown artifact reads as having none — the core refuses it by name a
/// moment later, which is the better error.
fn artifact_has_baseline(g: &DesignGraph, artifact_id: &str) -> Result<bool, McpError> {
    Ok(g.get_node(reflow2_core::nodes::node::ARTIFACT, artifact_id)
        .map_err(dyno_err)?
        .and_then(|n| {
            n.properties
                .get("checksum")
                .and_then(Value::as_str)
                .map(|s| !s.trim().is_empty())
        })
        .unwrap_or(false))
}

#[tool_router(router = built_router, vis = "pub")]
impl ReflowService {
    #[tool(
        description = "Declare which version of ANOTHER DESIGN this one depends on — the pin a \
                       seam analysis is taken AS OF. Records the source, the version (a tag or \
                       commit), the parts taken, and the build switches forwarded BY NAME, \
                       because a renamed feature is a downstream build break that no API diff or \
                       surface export would mention. This is what you MEAN to depend on; \
                       reconcile_dependencies compares it against what the build actually \
                       resolves. A declaration without a version is refused: the version is the \
                       whole point. Pass `graph_id` when the dependency is ITSELF a reflow2 \
                       design, which makes it composable from this committed file rather than \
                       from a per-machine config — omit it otherwise, because absent means \
                       'nobody has said', never 'there is no design'.",
        annotations(read_only_hint = false)
    )]
    pub async fn external_dependency(
        &self,
        Parameters(req): Parameters<ExternalDependencyReq>,
    ) -> Result<CallToolResult, McpError> {
        let decl = reflow2_core::DependencyDeclaration {
            id: req.id,
            name: req.name,
            source: req.source,
            version: req.version,
            components: req.components,
            features: req.features,
            declared_in: req.declared_in,
            graph_id: req.graph_id,
            // THE BASELINE IS TAKEN HERE, AT THE MOMENT OF DECLARING, and that
            // placement is the design. A hash recorded on a READ would make the
            // watch report `moved` exactly once and then go permanently quiet,
            // because the check would keep refreshing what it compares against.
            // Declaring is a deliberate act by somebody who has looked, so it is
            // the only honest place to say "this is what it looked like".
            //
            // A pointer at nothing readable is RECORDED ANYWAY, not refused: the
            // upstream may not have exported yet, which is the normal state the
            // hxm_program report measured (zero of seven siblings had an export).
            // It comes back as `missing` on the next read, which is a finding
            // rather than a wall.
            design_export_hash: req
                .design_export
                .as_deref()
                .and_then(crate::upstream::baseline_hash),
            design_export: req.design_export,
            design_export_seen_at: req.design_export_seen_at,
            note: req.note,
        };
        let mut g = self.write_lock().await?;
        g.declare_external_dependency(&decl).map_err(dyno_err)?;
        ok_json(g.dependency_manifest().map_err(dyno_err)?)
    }

    #[tool(
        description = "Has the design this one DEPENDS ON moved since the declaration was made? The second \
                       check req:design-dependencies-declared names, and the half never built — \
                       reconcile_dependencies answers the other one, against the BUILD. Walks the declared \
                       dependencies naming another reflow2 design AND a path to its committed export, reads \
                       each WITHOUT IMPORTING IT, and compares against what was recorded at declaration time. \
                       Importing is the obvious route and the wrong one: an import into a store already holding \
                       a design keeps the HOST'S name and absorbs the incoming nodes, so watching that way \
                       swallows the thing watched. REPORTS, and silence is reported rather than assumed: \
                       `moved`, `unchanged`, `never_seen` (declared, nobody has looked yet), `missing`, \
                       `unreadable`, `graph_id_mismatch` (that export belongs to a different design), \
                       `not_watched` (names a design, gives nothing to watch) and `not_observed` (the bounded \
                       pass skipped it). IT NEVER UPDATES THE BASELINE — a check that refreshed what it \
                       compares against would report a move once and then go quiet forever; read what changed, \
                       then re-declare. An empty answer means nothing is declared to watch, NEVER that nothing \
                       moved. Ask for this to learn whether a design or library we depend on has changed, moved \
                       or been updated since we last checked or pinned it.",
        annotations(read_only_hint = true)
    )]
    pub async fn upstream_status(
        &self,
        Parameters(_req): Parameters<UpstreamStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let targets = g.upstream_targets().map_err(dyno_err)?;
        let (observed, not_read) = crate::upstream::observe_upstreams(&targets);
        let report = g.reconcile_upstream(&observed).map_err(dyno_err)?;
        let mut payload = serde_json::to_value(&report).map_err(ser_err)?;
        if let (Some(obj), Some(skipped)) = (payload.as_object_mut(), not_read) {
            obj.insert(
                "not_read".into(),
                serde_json::to_value(&skipped).map_err(ser_err)?,
            );
        }
        ok_json(payload)
    }

    #[tool(
        description = "Check the declared dependencies against what the build ACTUALLY resolves, \
                       and return the reflow2.toml manifest. Catches the two opposite failures: \
                       the build taking something nothing declares (the reliance nobody agreed \
                       to, which breaks with nobody at fault) and a declaration the build no \
                       longer takes (a stale promise). Pass `observed` read fresh from the build \
                       files — Cargo.toml, docker-compose.yml, versions.env, whatever holds the \
                       pins. Declaring nothing reads as 'nobody has said', never as 'depends on \
                       nothing'.",
        annotations(read_only_hint = true)
    )]
    pub async fn reconcile_dependencies(
        &self,
        Parameters(req): Parameters<ReconcileDependenciesReq>,
    ) -> Result<CallToolResult, McpError> {
        let observed: Vec<reflow2_core::ObservedDependency> = req
            .observed
            .into_iter()
            .map(|o| reflow2_core::ObservedDependency {
                name: o.name,
                version: o.version,
                components: o.components,
                features: o.features,
                observed_in: o.observed_in,
            })
            .collect();
        let g = self.graph.read().await;
        let report = g.reconcile_dependencies(&observed).map_err(dyno_err)?;
        let manifest = g.dependency_manifest().map_err(dyno_err)?;
        ok_json(serde_json::json!({ "report": report, "manifest": manifest }))
    }

    #[tool(
        description = "DOES THE DECOMPOSITION I DECLARED MATCH THE COUPLING THE CODE ACTUALLY HAS? Walks the \
                       imports of every file the design registers (Artifact `location` + REALIZES → \
                       Component) and holds them against the walls the design declares — DEPENDS_ON, \
                       PROVIDES/CONSUMES, CONTAINS at every level — reporting cycles at each level, coupling \
                       lifted through containment, and both directions of disagreement: a coupling the source \
                       has that nobody declared, and a contract the design declares that no import backs. NO \
                       CONFIGURATION: the file set is the design's own registered artifacts, so you get this \
                       the moment link-artifacts has run. IT NEVER QUIETLY GUESSES — files in a language it \
                       cannot read, files the design names and disk lacks, imports resolving to no registered \
                       file, and source files the design has never heard of are each COUNTED AND NAMED. IT \
                       REPORTS AND NEVER WRITES: an import is coupling, not a contract, so a declared pair the \
                       source lacks is reported as exactly that, and nothing here becomes a CONSUMES edge — \
                       that is your call, made with evidence. A Python instrument (the same file as \
                       tools/wall_check.py) run by the server against the live design; a missing python3 is \
                       refused with what to do. Served since 2026-09-14 because a consumer re-wrote it by hand \
                       25 days after it was built here. `root` points at another checkout; `budget_chars` \
                       bounds the report.",
        annotations(read_only_hint = true)
    )]
    pub async fn wall_check(
        &self,
        Parameters(req): Parameters<WallCheckReq>,
    ) -> Result<CallToolResult, McpError> {
        let export = {
            let g = self.graph.read().await;
            serde_json::to_string(&g.export_graph().map_err(dyno_err)?)
                .map_err(|e| McpError::internal_error(format!("export: {e}"), None))?
        };
        let root = crate::wall_check::project_root(self.graph_path.as_deref(), req.root.as_deref());
        let report = crate::wall_check::run(&export, &root, "python3")
            .map_err(|why| McpError::invalid_params(why, None))?;
        let budget = req
            .budget_chars
            .unwrap_or(crate::reply_budget::DEFAULT_REPLY_BUDGET_CHARS);
        let text = format!(
            "# wall check — does the decomposition you declared hold?\n\nroot: {}\n\n{}",
            root.display(),
            report
        );
        Ok(ok_markdown(crate::wall_check::bound_text(text, budget)))
    }

    #[tool(
        description = "Check the design against what was actually built. Called with NOTHING, \
                       reflow2 MEASURES every registered artifact that has a location under the \
                       project root itself — presence and sha256, never content — and reports the \
                       divergences: files that vanished, files whose content changed since they \
                       were registered, and files it could not reach, each named with why. The \
                       reply's `measurement` block says the basis (measured or asserted), the root, \
                       and what was unmeasurable. Pass `observed` only for a tree this server does \
                       not hold; a server holding no tree refuses an empty call by name rather than \
                       reporting zero. The result's `propagation_seeds` are the design nodes the \
                       changes land on — feed them to `propagate_from` to see what a code change \
                       means upstream. The reply is bounded (`budget_chars`); counts survive the cut.",
        annotations(read_only_hint = false)
    )]
    pub async fn reconcile_artifacts(
        &self,
        Parameters(req): Parameters<ReconcileArtifactsReq>,
    ) -> Result<CallToolResult, McpError> {
        let asserted: Vec<ObservedArtifact> = req
            .observed
            .into_iter()
            .map(|o| serde_json::from_value(JsonValue::Object(o)))
            .collect::<Result<_, _>>()
            .map_err(|e| McpError::invalid_params(format!("invalid observation: {e}"), None))?;
        let opts = ReconcileOptions {
            record_events: req.record_events,
            exhaustive: req.exhaustive,
            detected_at: req.detected_at,
        };
        let mut g = self.write_lock().await?;
        // MEASURED OR ASSERTED, and the reply says which. An empty `observed`
        // used to mean "nothing checked"; since 2026-09-18 it means "measure
        // it yourself", which this server can do only when it holds the tree.
        let (observed, measurement) = if asserted.is_empty() {
            if self.tree_root().is_none() {
                return Err(McpError::invalid_params(
                    format!(
                        "nothing to reconcile: {}",
                        crate::measure::NotMeasured::NoTree.reason()
                    ),
                    None,
                ));
            }
            let (obs, block) = self.measure_registered(&g)?;
            (obs, block)
        } else {
            (
                asserted,
                json!({ "basis": "asserted", "note": "observations were supplied by the caller and used as given; nothing was measured" }),
            )
        };
        let mut out =
            serde_json::to_value(g.reconcile_artifacts(&observed, &opts).map_err(dyno_err)?)
                .map_err(ser_err)?;
        if let Some(o) = out.as_object_mut() {
            o.insert("measurement".into(), measurement);
        }
        // BOUNDED like every other report: a no-argument sweep of a real
        // design answered 124,716 characters in CI the day this shipped.
        ok_json(crate::reply_budget::bound_reply_sampling(
            out,
            req.budget_chars
                .unwrap_or(crate::reply_budget::DEFAULT_REPLY_BUDGET_CHARS),
            "`unchanged`, `measurement` and every count survive trimming; pass `observed` for \
             the artifacts you care about to read their findings in full, or raise `budget_chars`.",
        ))
    }

    #[tool(
        description = "Accept an artifact's current content as the new drift baseline — a \
                       two-sided decision. `disposition` is required: `design_holds` (the change \
                       carries no design meaning; recorded as a dated claim) or `design_updated` \
                       (behaviour moved and the design moved with it; pass \
                       `design_change_event_id` from the record_change that updated it, so code \
                       and design are one change). Silent accept does not exist: it is how a \
                       design erodes into fiction over N fix cycles while reporting zero gaps. \
                       Until you accept, the same checksum_change is reported on every reconcile. \
                       An artifact with NO checksum yet has exactly one legal disposition, so whatever you pass for it is READ as `baseline_established` — a first baseline, nothing moved, the design takes no position — and the reply's change_event_id (`chg:baseline-…`) says so; no CHANGED edge is drawn. Refused inside a batch, that case used to discard the rest (2026-09-07).",
        annotations(read_only_hint = false)
    )]
    pub async fn set_artifact_checksum(
        &self,
        Parameters(req): Parameters<SetChecksumReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let disposition = parse_disposition(
            &req.disposition,
            req.change_type.as_deref(),
            req.design_change_event_id.as_deref(),
            artifact_has_baseline(&g, &req.artifact_id)?,
        )?;
        let (checksum, basis, measurement) =
            self.checksum_or_measure(&g, &req.artifact_id, req.checksum.as_deref())?;
        let (_, change_event_id) = g
            .set_artifact_checksum(
                &req.artifact_id,
                &checksum,
                disposition,
                req.note.as_deref(),
                req.at.as_deref(),
            )
            .map_err(dyno_err)?;
        let artifact = g
            .set_checksum_basis(&req.artifact_id, basis)
            .map_err(dyno_err)?;
        ok_json(serde_json::json!({
            "artifact": NodeDto::from(artifact),
            "change_event_id": change_event_id,
            "measurement": measurement,
        }))
    }

    #[tool(
        description = "Declare what an Artifact node stands for and how its content behaves — \
                       the two things only its author can say. `granularity`: `atomic` (one \
                       deliverable), `opaque` (a directory or vendored mass claimed as a unit ON \
                       PURPOSE — do not descend), or `pending_expansion` (a PLACEHOLDER for items \
                       that should each become their own node). Those last two look identical to \
                       every report today, and they are opposite states: one is a decision, the \
                       other is unfinished work — a registration check read GREEN over 359 \
                       individually unreferenceable files because nothing could tell them apart. \
                       `volatility`: `stable` (any content change is drift — the default and the \
                       safe reading), or `append_only`/`living` (a log, a bus, a changelog: a \
                       content change is EXPECTED and reports as `expected_change` rather than \
                       being recorded, so you are not owed a disposition on every reconcile \
                       forever). ABSENCE still fires at full severity whatever the volatility, \
                       because a missing file is always a real finding. `audience` says WHO THE \
                       DELIVERABLE IS FOR — `consumer` (a user of the product reaches it) or \
                       `internal` (it serves this project's own machinery: CI, a release script, \
                       a coordination board). Leaving it unset is a true answer and is NEVER \
                       inferred, in particular never from the file's path, because that would \
                       encode one project's layout. Omitted fields are left alone; every other \
                       property is preserved.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_artifact_intent(
        &self,
        Parameters(req): Parameters<ArtifactIntentReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let artifact = g
            .set_artifact_intent(
                &req.artifact_id,
                req.granularity.as_deref(),
                req.volatility.as_deref(),
                req.audience.as_deref(),
            )
            .map_err(dyno_err)?;
        ok_json(NodeDto::from(artifact))
    }

    #[tool(
        description = "Accept MANY drift baselines in one call — the bulk form of \
                       set_artifact_checksum, which was 244 consecutive calls across 22 sessions \
                       of recorded usage. EACH ITEM CARRIES ITS OWN DISPOSITION, and that is the \
                       point rather than an inconvenience: a batch under one shared disposition \
                       would be exactly the silent bulk accept that erodes a design into fiction. \
                       The round trip collapses; the judgement stays per artifact. ALL OF IT OR \
                       NONE OF IT — every item is attempted so you learn every failure at once, \
                       and if anything failed no baseline moves. \
                       Ask for this to accept several files' new content at once.",
        annotations(read_only_hint = false)
    )]
    pub async fn set_artifact_checksums(
        &self,
        Parameters(req): Parameters<SetChecksumsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let mut accepts = Vec::with_capacity(req.accepts.len());
        let mut bases: Vec<(String, &'static str)> = Vec::with_capacity(req.accepts.len());
        let mut measurements = Vec::with_capacity(req.accepts.len());
        for a in &req.accepts {
            let disposition = parse_disposition(
                &a.disposition,
                a.change_type.as_deref(),
                a.design_change_event_id.as_deref(),
                artifact_has_baseline(&g, &a.artifact_id)?,
            )
            .map_err(|e| {
                // Name the item: a batch refusal that does not say which entry
                // is silent sends the caller back to guess across fifty.
                McpError::invalid_params(format!("{}: {}", a.artifact_id, e.message), None)
            })?;
            let (checksum, basis, measurement) = self
                .checksum_or_measure(&g, &a.artifact_id, a.checksum.as_deref())
                .map_err(|e| {
                    McpError::invalid_params(format!("{}: {}", a.artifact_id, e.message), None)
                })?;
            bases.push((a.artifact_id.clone(), basis));
            measurements.push(json!({ "artifact_id": a.artifact_id, "measurement": measurement }));
            accepts.push(BulkChecksumAccept {
                artifact_id: a.artifact_id.clone(),
                checksum,
                disposition,
                note: a.note.clone(),
                at: a.at.clone(),
            });
        }
        let report = g
            .set_artifact_checksums_with(&accepts, req.check_only)
            .map_err(dyno_err)?;
        // The basis rides only on a batch that applied: a check-only pass
        // writes nothing, and a refused batch moves no baseline.
        if !req.check_only && report.applied {
            for (id, basis) in &bases {
                g.set_checksum_basis(id, basis).map_err(dyno_err)?;
            }
        }
        let mut out = bulk_result(
            report,
            |(artifact, change_event_id)| json!({ "artifact": NodeDto::from(artifact), "change_event_id": change_event_id }),
        )?;
        if let Some(sc) = out
            .structured_content
            .as_mut()
            .and_then(|v| v.as_object_mut())
        {
            sc.insert("measurements".into(), json!(measurements));
        }
        Ok(out)
    }

    // ---- Artifact linking (connect real files to the design) ----

    #[tool(
        description = "Create an Artifact node — a real deliverable (file/spec/doc) that \
                          lives outside the graph, pointed to by `location`. Lands with no \
                       status unless you pass one: absent means nobody said. \
                       CONTENT FIELDS ARE REQUIRED TO CREATE AND OPTIONAL TO REVISE: call it \
                       again with the same id and only what you are changing \u{2014} omitted \
                       fields keep their stored value, so correcting one never means re-sending \
                       a 2 KB field you did not touch. \
                       Ask for this when you want to register a file, drawing or document in the design.",
        annotations(read_only_hint = false)
    )]
    pub async fn add_artifact(
        &self,
        Parameters(req): Parameters<AddArtifactReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let mut __rf =
            crate::service::RequiredFields::new(&g, reflow2_core::nodes::node::ARTIFACT, &req.id)?;
        let name = __rf.str("name", req.name);
        let stored = g
            .add_artifact(
                &req.id,
                &name,
                req.artifact_type.as_deref(),
                req.location.as_deref(),
            )
            .map_err(dyno_err)?;
        let stored = crate::tools::capture::set_description(
            &mut g,
            reflow2_core::nodes::node::ARTIFACT,
            &req.id,
            req.description.as_deref(),
        )?
        .unwrap_or(stored);
        let stored = crate::tools::capture::set_optional_props(
            &mut g,
            reflow2_core::nodes::node::ARTIFACT,
            &req.id,
            &[("status", req.status.as_deref())],
        )?
        .unwrap_or(stored);
        let stored = match req.checksum.as_deref() {
            Some(c) => g.set_artifact_baseline(&req.id, c).map_err(dyno_err)?,
            None => stored,
        };
        ok_json(NodeDto::from(stored))
    }

    #[tool(
        description = "Link an Artifact to the Capability or Component it REALIZES — the file, drawing or binary that implements it, as opposed to one that merely describes it (`documents`). This is the as-built half of the design: a capability nothing realizes is raised as `unrealized_capability`, and `reconcile_artifacts` compares each realizing file's checksum against its recorded baseline to catch drift. `link_artifact` does this in one call with provenance and a checksum, and is the better door; use this when the Artifact node already exists. Ask for this when you want to record that a real file implements a capability or part.",
        annotations(read_only_hint = false)
    )]
    pub async fn realizes(
        &self,
        Parameters(req): Parameters<RealizesReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let target_type = crate::service::resolve_node_type(
            &g,
            req.target_type.as_deref(),
            &req.target_id,
            "target_type",
        )?;
        ok_json(EdgeDto::from(
            g.realizes(
                &req.artifact_id,
                &target_type,
                &req.target_id,
                req.completeness.as_deref(),
                req.conformance.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Link an Artifact to the node it DOCUMENTS (describes without \
                       implementing): a design doc, ADR, README, runbook, instruction file \
                       or diagram. Record a file this way when something would be WRONG if it \
                       drifted out of step with the design — not every file. Fails loud if \
                       either endpoint is missing. Distinct from REALIZES (implementation) \
                       and SPECIFIES (machine-readable contract).",
        annotations(read_only_hint = false)
    )]
    pub async fn documents(
        &self,
        Parameters(req): Parameters<DocumentsReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let target_type = crate::service::resolve_node_type(
            &g,
            req.target_type.as_deref(),
            &req.target_id,
            "target_type",
        )?;
        ok_json(EdgeDto::from(
            g.documents(
                &req.artifact_id,
                &target_type,
                &req.target_id,
                req.doc_kind.as_deref(),
            )
            .map_err(dyno_err)?,
        ))
    }

    #[tool(
        description = "Register a real file against the design WITH provenance, atomically: Artifact + a \
                       provenance Fragment (YIELDED) + a REALIZES edge to the Capability/Component it \
                       implements. Fails loud if the target is missing. Use after building a file so \
                       as-designed vs as-built stays honest. RE-LINKING IS SAFE: `name` and `description` are \
                       required only on the FIRST link, and omitting either afterwards LEAVES THE STORED ONE \
                       ALONE — so attaching a file to a second target never renames it or drops its prose. Pass \
                       them again only when you mean to change them. OMIT `checksum` for a file under the \
                       project root: the server measures it (sha256, never content) and records \
                       `checksum_basis: measured`; a supplied one is `asserted` and the reply says whether it \
                       agrees. Ask for this when you want to record that \
                       a source file or document implements a capability or part — register the file against \
                       the design.",
        annotations(read_only_hint = false)
    )]
    pub async fn link_artifact(
        &self,
        Parameters(req): Parameters<LinkArtifactReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = LinkArtifactOptions {
            artifact_id: req.artifact_id,
            name: req.name,
            location: req.location,
            description: req.description,
            artifact_type: req.artifact_type,
            target_type: self
                .resolve_type(req.target_type.as_deref(), &req.target_id, "target_type")
                .await?,
            target_id: req.target_id,
            completeness: req.completeness,
            conformance: req.conformance,
            provenance: req.provenance,
            fragment_id: req.fragment_id,
            checksum: None,
            content_ref: req.content_ref.clone(),
            note_kind: req.note_kind.clone(),
        };
        let mut g = self.write_lock().await?;
        // MEASURED WHEN IT CAN BE. The location the caller registers is the
        // one measured; an artifact re-linked without a location keeps its
        // stored one, so look that up. A location this server cannot reach is
        // registered all the same — the reply names why it was not measured,
        // and the artifact simply carries no baseline until one is supplied.
        let location = opts.location.clone().or_else(|| {
            g.get_node(reflow2_core::nodes::node::ARTIFACT, &opts.artifact_id)
                .ok()
                .flatten()
                .and_then(|n| {
                    n.properties
                        .get("location")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
        });
        let measured = location.as_deref().map(|l| self.measure(l));
        let (checksum, basis, measurement): (Option<String>, Option<&str>, JsonValue) = match (
            req.checksum.as_deref(),
            measured,
        ) {
            (Some(given), Some(Ok(m))) => {
                let agrees = reflow2_core::artifact::checksums_agree(
                    &reflow2_core::artifact::canonical_checksum(given),
                    &m.checksum,
                );
                (
                    Some(given.to_string()),
                    Some("asserted"),
                    json!({ "basis": "asserted", "measured_checksum": m.checksum, "bytes": m.bytes,
                                "measured_path": m.measured_path, "agrees": agrees }),
                )
            }
            (Some(given), Some(Err(why))) => (
                Some(given.to_string()),
                Some("asserted"),
                json!({ "basis": "asserted", "not_measured": why, "note": why.reason() }),
            ),
            (Some(given), None) => (
                Some(given.to_string()),
                Some("asserted"),
                json!({ "basis": "asserted", "note": "no location to measure" }),
            ),
            (None, Some(Ok(m))) => (
                Some(m.checksum.clone()),
                Some("measured"),
                json!({ "basis": "measured", "checksum": m.checksum, "bytes": m.bytes, "measured_path": m.measured_path }),
            ),
            (None, Some(Err(why))) => (
                None,
                None,
                json!({ "basis": "none", "not_measured": why, "note": format!("no baseline recorded — {}", why.reason()) }),
            ),
            (None, None) => (
                None,
                None,
                json!({ "basis": "none", "note": "no location and no checksum: nothing to measure, no baseline recorded" }),
            ),
        };
        let opts = LinkArtifactOptions { checksum, ..opts };
        let artifact_id = opts.artifact_id.clone();
        let link = g.link_artifact(opts).map_err(dyno_err)?;
        if let Some(b) = basis {
            g.set_checksum_basis(&artifact_id, b).map_err(dyno_err)?;
        }
        let mut out = serde_json::to_value(link).map_err(ser_err)?;
        if let Some(o) = out.as_object_mut() {
            o.insert("measurement".into(), measurement);
        }
        with_loop_hint(
            out,
            "loop: as-built moved — reconcile_artifacts confirms the design still describes \
             what's on disk; loop_status says what else is owed",
        )
    }

    /// The checksum an accept records, and how it came to be: the caller's
    /// (asserted) or the server's (measured, when the caller passed none).
    /// An omitted checksum the server cannot measure is REFUSED by name —
    /// the one thing an accept must never do is move a baseline to a guess.
    fn checksum_or_measure(
        &self,
        g: &DesignGraph,
        artifact_id: &str,
        given: Option<&str>,
    ) -> Result<(String, &'static str, JsonValue), McpError> {
        if let Some(c) = given {
            return Ok((c.to_string(), "asserted", json!({ "basis": "asserted" })));
        }
        let location = g
            .get_node(reflow2_core::nodes::node::ARTIFACT, artifact_id)
            .map_err(dyno_err)?
            .and_then(|n| {
                n.properties
                    .get("location")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!(
                        "`{artifact_id}` has no location, so there is nothing to measure — pass \
                         `checksum` to assert one"
                    ),
                    None,
                )
            })?;
        match self.measure(&location) {
            Ok(m) => Ok((
                m.checksum.clone(),
                "measured",
                json!({ "basis": "measured", "checksum": m.checksum, "bytes": m.bytes, "measured_path": m.measured_path }),
            )),
            Err(why) => Err(McpError::invalid_params(
                format!(
                    "`{artifact_id}` was not measured: {} — pass `checksum` to assert one",
                    why.reason()
                ),
                None,
            )),
        }
    }

    /// Measure every registered artifact that has a location, as the
    /// observations a reconcile takes — presence and digest, nothing read for
    /// meaning — plus the block the reply carries saying what was reached.
    /// Locations this server cannot measure are NOT reported absent: they are
    /// left out of the observations and named, with why, in the block.
    pub(crate) fn measure_registered(
        &self,
        g: &DesignGraph,
    ) -> Result<(Vec<ObservedArtifact>, JsonValue), McpError> {
        let mut observed = Vec::new();
        let mut unmeasurable = Vec::new();
        let mut absent = 0usize;
        let mut measured = 0usize;
        let mut without_location = 0usize;
        for a in g
            .scan_nodes(reflow2_core::nodes::node::ARTIFACT)
            .map_err(dyno_err)?
        {
            let Some(location) = a.properties.get("location").and_then(|v| v.as_str()) else {
                without_location += 1;
                continue;
            };
            match self.measure(location) {
                Ok(m) => {
                    measured += 1;
                    observed.push(ObservedArtifact {
                        artifact_id: a.node_id.clone(),
                        present: true,
                        checksum: Some(m.checksum),
                        realizes: None,
                    });
                }
                Err(crate::measure::NotMeasured::Absent { .. }) => {
                    absent += 1;
                    observed.push(ObservedArtifact {
                        artifact_id: a.node_id.clone(),
                        present: false,
                        checksum: None,
                        realizes: None,
                    });
                }
                Err(why) => unmeasurable.push(
                    json!({ "artifact_id": a.node_id, "location": location, "not_measured": why }),
                ),
            }
        }
        let block = json!({
            "basis": "measured",
            "root": self.tree_root().map(|r| r.display().to_string()),
            "measured": measured,
            "absent": absent,
            "without_location": without_location,
            "unmeasurable": unmeasurable,
        });
        Ok((observed, block))
    }
}
