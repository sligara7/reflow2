//! `coherence` tools — one slice of the MCP surface.
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
    AgentAnswer, AgentBackend, AskedQuestion, ChangeType, DEFAULT_REGION_DEPTH,
    DEFAULT_REPLY_BUDGET_CHARS, DEFAULT_SCOPE_DEPTH, DesignGraph, Dimension, DriftDisposition,
    DynoError, EpochType, GapCandidate, GenesisOptions, HealOptions, HealProposal, HealStrategy,
    IngestOptions, LinkArtifactOptions, LoopStatus, ObservedArtifact, ObservedPath,
    PromptCollector, PropagateOptions, ReadinessForecast, ReadinessGate, ReadinessKind,
    ReadinessObservation, ReconcileOptions, StoredNode, Value,
};

use crate::dto::{EdgeDto, NodeDto};
use crate::service::*;

#[tool_router(router = coherence_router, vis = "pub")]
impl ReflowService {
    // ---- DETECT / analyze (deterministic, read-only) ----

    #[tool(
        description = "Find gaps in the design to ask the human about (DETECT). Pass `scope` (a \
                       node id) to answer for ONE PART of the design instead of all of it — the \
                       question a team that owns a subsystem asks day to day. The region is that \
                       seed's containment closure plus the propagation radius around it (`depth`, \
                       default 2). NOT the same computation as claim_region, which takes the \
                       radius alone and defaults differently. A scoped answer always reports \
                       what it left out: `total` across the whole design against `in_scope`, plus \
                       `out_of_scope` and `region_size`. IT ALSO REPORTS WHETHER IT NARROWED AT \
                       ALL: `share_of_anchored` is how much of everything the design has to say \
                       is in this answer, and a `narrowing_note` appears in words when that is \
                       over half — at the old default of 3, all 56 Components of reflow2's own \
                       design returned 50-60 of its 83 gaps and nothing said so. Project-level rollups still appear when they touch \
                       your part, counted as `project_level` and carrying `scope: project` \
                       themselves — filtering is not the tool deciding what you may worry \
                       about. THE REPLY IS BOUNDED SO A CLIENT CANNOT REFUSE IT: unscoped on \
                       this design it was 79,566 characters and harnesses refused the call \
                       outright, so the session saw a wall of harness text and reflow2 never got \
                       to suggest scoping. `budget` says which tier this reply landed in and \
                       exactly what it withheld; `count` and `by_source` cover every gap either \
                       way, so a shorter answer is never a quieter one. Raise `budget_chars` if \
                       your client has the room.",
        annotations(read_only_hint = true)
    )]
    pub async fn detect_gaps(
        &self,
        Parameters(req): Parameters<GapScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let budget = req.budget_chars.unwrap_or(DEFAULT_REPLY_BUDGET_CHARS);
        match req.scope.as_deref() {
            None => ok_json(g.detect_gaps_within(budget).map_err(dyno_err)?),
            Some(seed) => ok_json(
                g.detect_gaps_in_scope_within(
                    seed,
                    req.depth.unwrap_or(DEFAULT_SCOPE_DEPTH),
                    budget,
                )
                .map_err(dyno_err)?,
            ),
        }
    }

    #[tool(
        description = "Which boundaries between parts are covered by a contract \u{2014} ANSWERED AT \
                       THE ALTITUDE YOU ASK AT. Pass `altitude` (a Component `level`: `subsystem`, \
                       `system`, \u{2026}) and every coupling and every contract is lifted to the \
                       nearest container at that level before they are compared, so the question \
                       becomes \"is this coupling covered by a contract declared at or BELOW it?\" \
                       rather than \"do these two exact modules share one?\". Omit it for the raw \
                       module-level answer. WHY IT MATTERS: a design that declares its contracts at \
                       the subsystem boundary and its dependencies between modules reads as having \
                       NO contracts at all \u{2014} measured on reflow2 itself, 64 of 72 couplings \
                       undeclared at module level and NOTHING undeclared once lifted to subsystem. \
                       `covered_by` names the LEAF pair where each contract actually lives, because \
                       \"yes, there is an interface\" without saying where it is declared sends the \
                       reader hunting. \u{1F6D1} READ `scope_note`: a zero here means every coupling \
                       VISIBLE AT THIS ALTITUDE is covered, and says nothing about the finer ones \
                       underneath. NOTHING IS WRITTEN BACK \u{2014} this is derived on every call, \
                       because storing a rolled-up edge would make the graph assert a contract \
                       nobody declared.",
        annotations(read_only_hint = true)
    )]
    pub async fn seam_coverage(
        &self,
        Parameters(req): Parameters<SeamCoverageReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.seam_coverage(req.altitude.as_deref()).map_err(dyno_err)?)
    }

    #[tool(
        description = "What parts this design has, so a session that holds NOTHING can pick where \
                       to stand. The one orientation read that asks for no seed, no scope and no \
                       topic — call it at check-in, before you have a lane. Each row is a part the \
                       design itself names (its Project and Components, never a cluster reflow2 \
                       inferred) with its size, how many gaps and defects are open inside it, and \
                       who already claims it; the `seed_id` is then what you pass as `scope` to \
                       detect_gaps, detect_defects or claim_region. IT IS NOT A PARTITION AND SAYS \
                       SO: `coverage` reports how many nodes lie in NO region (on a mature design \
                       most of the graph is bookkeeping no Component contains) and how many lie in \
                       MORE THAN ONE, because rows that overlap heavily are not the distinct areas \
                       they look like. Rows are ordered by how much is open there now, which is \
                       stated in `order` and is NOT a ranking of importance. A design that names \
                       no parts yet gets an empty list WITH a note saying that is why.",
        annotations(read_only_hint = true)
    )]
    pub async fn design_regions(
        &self,
        Parameters(req): Parameters<RegionsReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(
            g.design_regions(req.depth.unwrap_or(DEFAULT_REGION_DEPTH))
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "Which of the design VOCABULARY this design has ever used \u{2014} node \
                       types, edge types, and properties on the types that have instances. \
                       Answers what nothing else asks: the detectors check the CONSISTENCY OF \
                       WHAT EXISTS, so vocabulary a design never touched is invisible to every \
                       other computation. GROUPED BY THE SCHEMA'S OWN ELEVEN DOMAINS, never one \
                       this tool invented: unused vocabulary clusters into whole subsystems, so \
                       a mature design reads about four findings instead of thirty-one. Each \
                       domain carries `park_with`, the id of a Decision whose ACCEPTANCE \
                       declares it deliberately unused here, so a settled choice stops reading \
                       as a hole (`add_decision`, then `set_decision_status accepted`). THE \
                       FLAT LIST IS WITHHELD BY DEFAULT and returned only with \
                       `include_unused`: measured, 97 items on a day-one design and 59 on a \
                       mature one \u{2014} longest for the user least able to act on it. ASKED \
                       FOR, IT NAMES PROPERTIES TOO \u{2014} `<domain>: node property \
                       Artifact.audience`, `<domain>: edge property GOVERNED_BY.ruling` \
                       \u{2014} the only way to learn WHICH declared field a design never \
                       filled in; the figures alone reduce a set of named things to a fraction. \
                       Counted and named ONLY on types that HAVE INSTANCES, since a zero on an \
                       empty type is vacuous. A property with a schema DEFAULT can never be \
                       named: the store writes it onto every instance, so what this reports is \
                       the undefaulted optional. Under ten nodes a `note` says the figures mean \
                       the design has barely STARTED, not that vocabulary is going unused.",
        annotations(read_only_hint = true)
    )]
    pub async fn vocabulary_coverage(
        &self,
        Parameters(req): Parameters<VocabularyCoverageReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(
            g.vocabulary_coverage(req.include_unused.unwrap_or(false))
                .map_err(dyno_err)?,
        )
    }

    #[tool(
        description = "The coherence loop's outstanding debt, cheaply: what \
                       capture→detect→ask→decide steps are owed now, computed from graph state, \
                       never run history. Anchored gaps never put to the user, questions waiting \
                       or answered-but-unwritten, open decisions a named person was ASKED to \
                       settle (a `proposed` Decision carrying an AUTHORED_BY `role=approver`; \
                       one with no approver is somebody thinking out loud and stays quiet), \
                       structural defects, capabilities claiming realized/verified with no \
                       passing check, drift awaiting a disposition, and built capabilities \
                       nobody has checked against reality. `clean: true` means nothing is owed, \
                       and those decisions are LISTED in `assigned_decisions`. Pass \
                       `contributor_id` to ask WHAT NEEDS THIS PERSON. Scoped, TWO things are \
                       attributed: decisions they were asked to settle, and open gaps standing \
                       on ground they OWN (`gaps_on_owned_ground`, each naming which owned nodes \
                       it touches). Every other debt class is a fact about the DESIGN and comes \
                       back design-wide under `scope.not_attributable` rather than filtered to \
                       zero, because \"nothing is owed to you\"and \"I cannot tell whose this \
                       is\"must never be the same answer — so scoped, `clean` means nothing is \
                       owed BY THAT PERSON, not that the design is clean. An unknown \
                       contributor_id is REFUSED: a typo would otherwise give the most \
                       reassuring reply there is. `verifications` is a DIGEST: counts by status, \
                       how many never ran, and every check not currently passing. `graph_report` \
                       carries every check with its last run.",
        annotations(read_only_hint = true)
    )]
    pub async fn loop_status(
        &self,
        Parameters(req): Parameters<LoopScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let status = g
            .loop_status_for(req.contributor_id.as_deref())
            .map_err(dyno_err)?;
        let mut payload = serde_json::to_value(&status).map_err(ser_err)?;
        // The debt rollup is seven integers and a to-do list; the per-check roll
        // beside it grew with the design until the whole reply was 74 KB and no
        // longer fit in a harness turn — `cap:loop-status` says ONE CHEAP CALL,
        // and the build stopped meeting it somewhere around the hundredth check.
        // Digest here, never at the core: `graph_report` still serves the full
        // list, and both still come from `verification_recency`, so the two
        // surfaces cannot drift apart (the invariant report.rs states).
        // Findings some record CLAIMS to have answered. Read here rather than
        // in the core digest so both surfaces get it from one place, exactly as
        // the recency roll is shared.
        // 🛑 SCOPED TO THE ROWS THE DIGEST WILL SHOW, and that is a fix rather
        // than an optimisation. The exhaustive form asks `incoming()` about
        // every Verification and TemporalFact, and each of those walks the whole
        // edge set — measured 2026-08-24 at 39s on this graph, inside the call
        // `cap:loop-status` promises is CHEAP. The digest only annotates checks
        // that are NOT passing, so those are the only ids worth asking about.
        let attention_ids: Vec<&str> = status
            .verifications
            .iter()
            .filter(|v| v.status != "passing" && v.status != "superseded")
            .map(|v| v.verification_id.as_str())
            .collect();
        let invalidated = g
            .invalidated_verifications(&attention_ids)
            .map_err(dyno_err)?;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "verifications".into(),
                verification_digest(&status.verifications, &invalidated).map_err(ser_err)?,
            );
        }
        // ⚠️ DELIBERATELY NOT IN THE SERVED DESCRIPTION, and this is not an
        // oversight: `skill_lint` caps a description at 1500 chars and this one
        // was already at 1494. Documenting the field there would have cost 380
        // chars of EVERY session's context to announce something that announces
        // itself — the payload weight ophyd-service filed as friction #8. When
        // it matters it is in `next`, which is the part a reader acts on.
        //
        // IS THE SERVER ANSWERING THIS THE CODE ON DISK? `served_by.stale` has
        // answered that since 2026-08-08 and rode on `graph_report` alone —
        // which is not the call anything points a session at. The session-start
        // hook says "loop_status is the one cheap call", the stop hook nudges
        // here, and this is where a session looks. So the currency of the
        // ANSWERER belongs beside the currency of the DESIGN.
        //
        // FOURTH MEASURED INSTANCE, and the first three are in
        // tests/the_server_says_when_it_is_stale.rs: 2026-08-08 cost five
        // merged PRs that never ran live, and 2026-08-19 repeated it exactly —
        // five PRs, a deliberate session restart, and a surface that did not
        // move, because `--shared` re-attaches to the same daemon. The bit
        // existed both times. Nobody was looking at the one tool carrying it.
        //
        // CHEAP WHEN CURRENT, LOUD WHEN NOT. `stale_note` is ~1 KB of remedy
        // and is dropped when the answer is `false`, so the ordinary call pays
        // three fields for it. When the answer is anything else the note stays
        // AND `next` gains an entry, because `next` is the list an agent
        // actually acts on and a field beside it is not the same as being in
        // it — which is the whole lesson being applied here rather than
        // restated.
        {
            let served = crate::service::served_by();
            let stale = served.get("stale").and_then(serde_json::Value::as_bool);
            let mut block = served.clone();
            if stale == Some(false) {
                if let Some(o) = block.as_object_mut() {
                    o.remove("stale_note");
                }
            } else if let Some(arr) = payload.get_mut("next").and_then(|v| v.as_array_mut()) {
                arr.insert(
                    0,
                    json!(match stale {
                        Some(true) => crate::service::STALE_NEXT,
                        _ => crate::service::UNKNOWN_NEXT,
                    }),
                );
            }
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("served_by".into(), block);
            }
        }
        // Whether the loop's own safety net exists (req:nudge-path-proven).
        // Machine-readable here, and in the handshake for the sessions that
        // never call this — which are precisely the ones a nudge is for.
        let nudge = crate::nudge::status(self.graph_path.as_deref());
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "nudge".into(),
                serde_json::to_value(&nudge).map_err(ser_err)?,
            );
            if let Some(advisory) = nudge.advisory() {
                obj.insert("nudge_advisory".into(), json!(advisory));
            }
        }
        // "What did I introduce?" — asked explicitly, because answering it means
        // reading the committed export and the ordinary orientation call should
        // not pay for that (dec: Anthony 2026-08-16, both halves).
        //
        // THE BASELINE IS THE RECORD, NOT A CLOCK. Only 2 of this design's
        // 2,367 nodes carry `created_at`, so a time-based session boundary was
        // never computable; what the durable record does not yet hold is, and
        // is the more useful question anyway.
        if req.since_export
            && let Some(graph_path) = self.graph_path.as_deref()
        {
            let mut baseline: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            let state = reflow2_core::provenance::read_sync_state(graph_path);
            for path in state.last_synced.keys() {
                let Ok(raw) = std::fs::read_to_string(path) else {
                    continue;
                };
                let Ok(doc) = serde_json::from_str::<reflow2_core::GraphExport>(&raw) else {
                    continue;
                };
                for n in &doc.nodes {
                    baseline.insert(n.node_id.clone());
                }
            }
            match g.debt_since(&baseline) {
                Ok(session) => {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert(
                            "since_export".into(),
                            serde_json::to_value(&session).map_err(ser_err)?,
                        );
                    }
                }
                // A scoping failure must not take the whole orientation call
                // down: the design-wide answer above is still true and still
                // what a session needs most.
                Err(e) => {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert(
                            "since_export_unavailable".into(),
                            json!(format!("could not scope to the committed record: {e}")),
                        );
                    }
                }
            }
        }

        // Has the SHARED RECORD moved since this seat last looked? The signal
        // existed and was read only by `export_graph`, so the design knew at
        // the first moment of a session and spoke at the last
        // (dec:idea-the-graph-notices-the-record-moved-without-being-asked,
        // option A). Reported here because this is the orientation call the
        // session-start hook already points sessions at.
        //
        // GATED, and the gate is the design: silent whenever the file has not
        // moved, which is the whole of ordinary solo work. The export is built
        // only when something HAS moved, so the common path costs one file read
        // and no comparison.
        // Has the design this one DEPENDS ON moved? The other direction from
        // `record_moved` below, and the second check
        // `req:design-dependencies-declared` names in its own statement.
        //
        // GATED ON THE MANIFEST, and that is what keeps this cheap. A design
        // that declares no upstream export to watch — which is every design
        // until somebody deliberately points at one, this one included — pays a
        // single node scan and reads no files at all. Only a design that has
        // asked to watch something pays for the reading, which is the same
        // bargain `sync_debt` strikes one paragraph down.
        //
        // ONLY THE ACTIONABLE ONES REACH `next`. `unchanged` is the ordinary
        // quiet case and `never_seen` says nobody has looked, which is a
        // statement about this design's own record rather than about the
        // upstream; both stay in `upstream_status` where a reader who asked can
        // see them, and neither becomes a line in the list a session acts on.
        if let Ok(targets) = g.upstream_targets()
            && !targets.is_empty()
        {
            let (observed, _) = crate::upstream::observe_upstreams(&targets);
            if let Ok(report) = g.reconcile_upstream(&observed) {
                let acting: Vec<&reflow2_core::UpstreamFinding> = report
                    .findings
                    .iter()
                    .filter(|f| f.is_actionable())
                    .collect();
                if !acting.is_empty() {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert(
                            "upstream_moved".into(),
                            json!(
                                acting
                                    .iter()
                                    .map(|f| f.detail.clone())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            ),
                        );
                    }
                    if let Some(arr) = payload.get_mut("next").and_then(|v| v.as_array_mut()) {
                        for f in &acting {
                            arr.push(json!(f.detail.clone()));
                        }
                    }
                }
            }
        }

        if let Some(graph_path) = self.graph_path.as_deref() {
            let live_nodes = g.count_all_nodes().unwrap_or(0);
            let debts =
                crate::sync_debt::sync_debt(graph_path, live_nodes, &|| g.export_graph().ok());
            if let Some(obj) = payload.as_object_mut() {
                let behind: Vec<_> = debts.iter().filter(|d| d.is_actionable()).collect();
                if !behind.is_empty() {
                    obj.insert(
                        "record_moved".into(),
                        json!(
                            behind
                                .iter()
                                .map(|d| d.message())
                                .collect::<Vec<_>>()
                                .join(" ")
                        ),
                    );
                }
                // AN UNEXPORTED GRAPH IS OWED WORK, so it belongs in `next`
                // rather than only in a row a reader may skim.
                //
                // The sentence already existed — `SyncDebt::message` puts it on
                // the in-step line, deliberately, "because that is the line a
                // session reads before standing down". It was then filtered out
                // of everything served: `record_moved` above takes only
                // `is_actionable()` debts, and `in_step` is not actionable. So
                // the reading aid was written, tested, and never surfaced.
                //
                // REPORTED TWICE. dragon Boss, 2026-08-16, then again
                // 2026-08-22 after re-exporting as a control and getting
                // `wrote: "changed"` against a green verdict — *"`state` is the
                // field a session reads, and the counts are the field it
                // skims"*. Reproduced here the same day: `export_nodes: 2897`
                // beside `live_nodes: 2899`, verdict `in_step`, read and passed
                // over by the session that then had to be told.
                //
                // 🛑 THE VERDICT IS DELIBERATELY UNCHANGED. `state` answers
                // whether the RECORD moved ahead of this seat, and
                // `ver:the-record-moved-is-surfaced` pins "ordinary unexported
                // work is NEVER reported" as a property this rests on — making
                // `in_step` go red would fire on nearly every session
                // mid-flight. What was missing was never a different verdict;
                // it was that the verdict could be read ALONE.
                let unexported: Vec<String> = debts
                    .iter()
                    .filter(|d| d.live_nodes > d.export_nodes)
                    .map(|d| d.message())
                    .collect();
                if !unexported.is_empty()
                    && let Some(arr) = obj.get_mut("next").and_then(|v| v.as_array_mut())
                {
                    for m in unexported {
                        arr.push(json!(m));
                    }
                }
                // Every known target, including the quiet ones — "checked three
                // records, all in step" and "checked nothing" must not share an
                // answer.
                if !debts.is_empty() {
                    obj.insert(
                        "sync".into(),
                        serde_json::to_value(&debts).map_err(ser_err)?,
                    );
                }
            }
        }
        ok_json(payload)
    }

    #[tool(
        description = "Which decisions to settle next — a rough guide, not an ordering, for a design with more open \
                       questions than anyone can hold at once. FOUR BANDS, and the split is the design. `marked`: \
                       open decisions carrying AUTHORED_BY `role=approver` — the user's own word, durable across \
                       sessions, self-clearing when the status moves. NO SCORE REORDERS THESE. `ranked`: the top \
                       few of what is NOT marked, since ranking somebody's own marks back at them says nothing they \
                       do not know. `unexplored`: ONE from the zero-scoring pool — a REQUIRED exploration term, \
                       because every signal here is built on connectedness and would otherwise bury a decision \
                       nothing points at, forever. `shaping`: the few ACCEPTED decisions most of the LIVE design \
                       hangs off — not a to-do list, but what a newcomer needs to read the rest. SCORES ARE COARSE \
                       ON PURPOSE. Ranked: governed nodes (+1 each), those SCHEDULED into an increment (+2), \
                       CONTRADICTS an accepted Decision (+2). Shaping: live governed count, where live drops \
                       dropped/deferred Requirements and superseded/rejected Decisions — that filter is the whole \
                       refinement, since raw in-degree's top hit here governed ten requirements of which nine were \
                       dropped. Type breadth is reported as `shapes`, never ranked. WHAT IT CANNOT DO: a Decision \
                       has no timestamp, so staleness never enters; one linked to nothing scores zero however \
                       important — `unranked_pool` and `not_shown` report how many, so a short answer never reads \
                       as the whole set. Review records excluded throughout.",
        annotations(read_only_hint = true)
    )]
    pub async fn what_next(
        &self,
        Parameters(req): Parameters<crate::service::WhatNextReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.what_next(req.limit.unwrap_or(4)).map_err(dyno_err)?)
    }

    #[tool(
        description = "Blast radius of a recorded ChangeEvent along the golden thread. Returns \
                       a summary (counts by distance, the distance-1 ring, risk crossings); \
                       pass full=true for every impacted node with its hop chain.",
        annotations(read_only_hint = true)
    )]
    pub async fn propagate_change(
        &self,
        Parameters(req): Parameters<PropagateChangeReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let g = self.graph.read().await;
        let radius = g
            .propagate_change(&req.change_event_id, opts)
            .map_err(dyno_err)?;
        if req.full.unwrap_or(false) {
            ok_json(radius)
        } else {
            ok_json(radius.summarize())
        }
    }

    #[tool(
        description = "Speculative blast radius from seed node ids (what would this touch?). \
                       Returns a summary (counts by distance, the distance-1 ring, risk \
                       crossings); pass full=true for every impacted node with its hop chain.",
        annotations(read_only_hint = true)
    )]
    pub async fn propagate_from(
        &self,
        Parameters(req): Parameters<PropagateFromReq>,
    ) -> Result<CallToolResult, McpError> {
        let opts = PropagateOptions {
            max_depth: req.max_depth.unwrap_or(5),
        };
        let seeds: Vec<&str> = req.seed_ids.iter().map(String::as_str).collect();
        let g = self.graph.read().await;
        let radius = g.propagate_from(&seeds, opts).map_err(dyno_err)?;
        if req.full.unwrap_or(false) {
            ok_json(radius)
        } else {
            ok_json(radius.summarize())
        }
    }

    #[tool(
        description = "The confirmation ledger (BL-35): for every capability with built \
                       artifacts, when was its claim last checked against reality, and what was \
                       the answer — drift events and whether each was resolved, accept claims \
                       split into design_holds vs design_updated, first baselines counted \
                       apart from both (they are not accepts), clean-reconcile confirmations \
                       with when they last happened, design edits on the record, and a state \
                       per capability: drifting (an observed divergence is unanswered), \
                       confirmed (examined, with the claim history visible), or unexamined \
                       (nobody has ever looked — NOT the same as confirmed).",
        annotations(read_only_hint = true)
    )]
    pub async fn confirmation_ledger(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.confirmation_ledger().map_err(dyno_err)?)
    }

    #[tool(
        description = "The 'what should I look at?' rollup report (SYNTHESIZE). Its `served_by` \
                       block says whether this server is STILL THE CODE IT WAS STARTED FROM — a \
                       server that predates a rebuild keeps serving the old surface, and until \
                       2026-08-09 nothing said so. READ `served_by.stale`: `true` means every \
                       computed number here came from a binary no longer on disk; `null` means \
                       the server could not tell (never read it as false). It is DERIVED, not \
                       compared: a version literal left four sessions drawing opposite \
                       conclusions from the same true value, and two builds in one working \
                       session share a version anyway. `stale_note` carries the fix, which is \
                       `--stop-shared` plus any tool call — NOT a session restart, which \
                       re-attaches to the same daemon and silently changes nothing. THE FULL \
                       VERIFICATION ROLL IS WITHHELD BY DEFAULT and returned only with \
                       `include_verifications`: measured here, every check with its last run was \
                       152,803 of the report's 166,934 characters — 91.5% of the one read a \
                       session makes to ask what to look at, spent on a list that says \
                       \"196 passing, 1 planned\". What comes back instead is the same digest \
                       `loop_status` returns: counts by status, how many never ran, and every \
                       check NOT currently passing, in full.",
        annotations(read_only_hint = true)
    )]
    pub async fn graph_report(
        &self,
        Parameters(req): Parameters<GraphReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let full = g.graph_report().map_err(dyno_err)?;
        let roll = full.verifications.clone();
        let mut report = serde_json::to_value(full)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        // THE ROLL IS 91.5% OF THIS REPORT AND IT IS NOT WHERE THE ANSWER IS.
        // 197 checks, 196 of them passing; 93% of those bytes are the `name`
        // field, because 113 of the names carry whole reports (longest: 654
        // words) written there before `add_verification` could reach
        // `description`. The digest is the one `loop_status` already returns, so
        // the name truncation and its announcement come with it rather than
        // being written twice.
        report["verifications"] = if req.include_verifications {
            serde_json::to_value(&roll).map_err(ser_err)?
        } else {
            verification_digest(&roll, &{
                // Same scoping as loop_status, and for the same measured reason.
                let ids: Vec<&str> = roll
                    .iter()
                    .filter(|v| v.status != "passing" && v.status != "superseded")
                    .map(|v| v.verification_id.as_str())
                    .collect();
                g.invalidated_verifications(&ids).map_err(dyno_err)?
            })
            .map_err(ser_err)?
        };
        report["served_by"] = served_by();
        self.ok_read(&g, report)
    }

    #[tool(
        description = "The graph report rendered as Markdown.",
        annotations(read_only_hint = true)
    )]
    pub async fn graph_report_markdown(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let report = g.graph_report().map_err(dyno_err)?;
        let mut md = report.to_markdown();
        // The rendering sibling of graph_report, and an orientation read in its
        // own right — carry the same read-side loop_hint (BL-91), as a trailing
        // blockquote since a Markdown document has no field to hang it on.
        if let Some(hint) = self.read_loop_hint(&g)? {
            md.push_str(&format!("\n\n> **loop_hint** — {hint}\n"));
        }
        Ok(ok_markdown(md))
    }

    #[tool(
        description = "Detect structural defects the machine can repair (HEAL). Pass `scope` (a \
                       node id, `depth` default 2) to ask it of one part of the design: not \
                       \"what is my team owed\" but \"is my part of the architecture sound\" — a \
                       cycle wholly inside one subsystem is that subsystem's to fix. Reports \
                       `total` against `in_scope` so a quiet corner never implies a quiet design. \
                       UNSCOPED, IT RETURNS `{swept, defects}` RATHER THAN A BARE LIST, since \
                       2026-08-17: `swept.nodes` is what it examined, `swept.rules` names the \
                       checks that ran, and `swept.note` appears only when the sweep COULD NOT \
                       have found anything — so a zero is never read as permission before \
                       `apply_heal`, which deletes nodes. \
                       `swept.design_network_nodes` is deliberately SMALLER than `swept.nodes`: \
                       the topology rules walk a narrower graph that drops provenance types, \
                       review records and CONTAINS, and that gap is reported rather than hidden. \
                       ⭐ READ `swept.coverage_note` FIRST WHEN PRESENT — one line naming what \
                       this sweep could NOT have found. `rule_populations` says what each rule \
                       walked (a rule that walked NOTHING reports clean for the same reason an \
                       empty graph does); `coupling_by_level` says how much coupling exists AT \
                       each declared level, and that is the one that bites — measured here, the \
                       cycle rule walked 182 pairs and found none while ZERO joined two \
                       subsystems, so a clean result was SILENT about the subsystems rather \
                       than clean about them.",
        annotations(read_only_hint = true)
    )]
    pub async fn detect_defects(
        &self,
        Parameters(req): Parameters<ScopeReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let mut out = match req.scope.as_deref() {
            None => serde_json::to_value(g.detect_defects().map_err(dyno_err)?),
            Some(seed) => serde_json::to_value(
                g.detect_defects_in_scope(seed, req.depth.unwrap_or(DEFAULT_SCOPE_DEPTH))
                    .map_err(dyno_err)?,
            ),
        }
        .map_err(ser_err)?;
        lift_repair_notes(&mut out);
        ok_json(out)
    }

    #[tool(
        description = "Accept a structural defect the user has judged fine, recording WHY. It \
                       moves out of detect_defects into reviewed_defects — not deleted, not \
                       hidden — the mirror of acknowledge_gap, and for the same reason: a list \
                       that can never reach zero gets skimmed, so a genuine new defect must \
                       arrive into a list someone still reads. The reason becomes a real Decision \
                       node that outlives the session. Because a defect id hashes its category \
                       with its affected set, the review EXPIRES when that shape changes — the \
                       new shape has a new id nobody has accepted.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_defect(
        &self,
        Parameters(req): Parameters<AcknowledgeDefectReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let decision_id = g
            .acknowledge_defect(&req.defect_id, &req.affected_ids, &req.reason)
            .map_err(dyno_err)?;
        ok_json(json!({ "acknowledged": req.defect_id, "decision_id": decision_id }))
    }

    #[tool(
        description = "Structural defects that were reviewed and accepted, each with the reason \
                       given. Worth re-reading when the architecture shifts: an acknowledgement \
                       is keyed to a defect's shape, so one still listed here still applies, and \
                       one whose shape has gone is reported as `retired` rather than vanishing.",
        annotations(read_only_hint = true)
    )]
    pub async fn reviewed_defects(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        self.ok_read(&g, g.reviewed_defects().map_err(dyno_err)?)
    }

    #[tool(
        description = "Withdraw a defect's acknowledgement, returning it to the open list. The \
                       Decision is superseded rather than deleted — the judgement was real and \
                       its record survives being changed. No-ops (returns withdrawn: false) when \
                       there was no acknowledgement.",
        annotations(read_only_hint = false)
    )]
    pub async fn withdraw_defect_acknowledgement(
        &self,
        Parameters(req): Parameters<DefectIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let withdrawn = g
            .withdraw_defect_acknowledgement(&req.defect_id)
            .map_err(dyno_err)?;
        ok_json(json!({ "withdrawn": withdrawn, "defect_id": req.defect_id }))
    }

    #[tool(
        description = "Propose a HEAL plan (never mutates; review then apply_heal).",
        annotations(read_only_hint = true)
    )]
    pub async fn propose_heal(
        &self,
        Parameters(req): Parameters<ProposeHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let strategy: HealStrategy = match req.strategy.as_deref() {
            None => HealStrategy::default(),
            Some(s) => parse_enum(s, "heal strategy")?,
        };
        let opts = HealOptions {
            strategy,
            max_operations: req.max_operations,
        };
        let g = self.graph.read().await;
        ok_json(g.propose_heal(opts).map_err(dyno_err)?)
    }

    #[tool(
        description = "Evaluate how capabilities are allocated across components.",
        annotations(read_only_hint = true)
    )]
    pub async fn evaluate_allocation(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.evaluate_allocation().map_err(dyno_err)?)
    }

    #[tool(
        description = "Propose a capability→component allocation via Leiden clustering.",
        annotations(read_only_hint = true)
    )]
    pub async fn propose_allocation(
        &self,
        Parameters(req): Parameters<ProposeAllocationReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.propose_allocation(req.resolution).map_err(dyno_err)?)
    }

    #[tool(
        description = "Decomposition/hierarchy issues (matryoshka level checks).",
        annotations(read_only_hint = true)
    )]
    pub async fn hierarchy_issues(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.hierarchy_issues().map_err(dyno_err)?)
    }

    #[tool(
        description = "Surprising cross-community couplings (mined from the graph).",
        annotations(read_only_hint = true)
    )]
    pub async fn surprising_connections(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.surprising_connections().map_err(dyno_err)?)
    }

    #[tool(
        description = "All declining quality dimensions across the design, worst first.",
        annotations(read_only_hint = true)
    )]
    pub async fn dimension_drifts(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.dimension_drifts().map_err(dyno_err)?)
    }

    #[tool(
        description = "Quality-dimension drift for one target node.",
        annotations(read_only_hint = true)
    )]
    pub async fn dimension_drift(
        &self,
        Parameters(req): Parameters<DimensionDriftReq>,
    ) -> Result<CallToolResult, McpError> {
        let dim: Dimension = parse_enum(&req.dimension, "dimension")?;
        let g = self.graph.read().await;
        ok_json(g.dimension_drift(&req.target_id, dim).map_err(dyno_err)?)
    }

    #[tool(
        description = "Accept a gap the user has judged fine, recording WHY. It moves out of \
                       `detect_gaps` into `reviewed_gaps` — not deleted, not hidden. Use this \
                       once the user has actually decided something, so the open list means \
                       \"still needs attention\"; a list that can never reach zero gets skimmed. \
                       The reason is stored as a real Decision node in the graph, so it outlives \
                       this session. If the gap's affected nodes later change, the review \
                       expires and the gap returns for a fresh judgement.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_gap(
        &self,
        Parameters(req): Parameters<AcknowledgeGapReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let decision_id = g
            .acknowledge_gap_by(
                &req.gap_id,
                &req.affected_ids,
                &req.reason,
                req.approver.as_deref(),
                req.acted_at.as_deref(),
            )
            .map_err(dyno_err)?;
        // THE ABSENCE IS REPORTED, NEVER ASSUMED. An acknowledgement is the
        // owner's word by definition, so one carrying no name is a real state
        // worth saying out loud rather than a quiet default — and a caller who
        // never learns it happened is exactly how 49 of these were written in
        // one pass before anyone noticed.
        let mut out = json!({ "acknowledged": req.gap_id, "decision_id": decision_id });
        match req.approver.as_deref() {
            Some(who) => {
                out["approved_by"] = json!(who);
            }
            None => {
                out["approved_by"] = JsonValue::Null;
                out["unattributed"] = json!(
                    "This acknowledgement carries NOBODY'S NAME. It mints an accepted Decision — settled intent — and `rule:design-intent-moves-only-on-the-owners-word` says that needs a name, so it will be reported by check_intent_authority. Pass `approver` (the Contributor whose judgement it is) to record it. Recorded anyway rather than refused, because a design that has modelled no Contributor must still be able to accept a gap."
                );
            }
        }
        ok_json(out)
    }

    #[tool(
        description = "Acknowledge MANY gaps in one call — the bulk form of acknowledge_gap. \
                       EACH GAP CARRIES ITS OWN REASON, which is the point: a batch of \
                       acknowledgements under one shared reason is exactly the erosion the \
                       ask-don't-repair rule exists to prevent, and would make a bulk form worse \
                       than the loop it replaces. The round trip collapses; the judgement stays \
                       per gap. ALL OF IT OR NONE OF IT — every item is attempted so you learn \
                       every failure at once, and if anything failed nothing is acknowledged.",
        annotations(read_only_hint = false)
    )]
    pub async fn acknowledge_gaps(
        &self,
        Parameters(req): Parameters<AcknowledgeGapsReq>,
    ) -> Result<CallToolResult, McpError> {
        let items: Vec<BulkGapAck> = req
            .gaps
            .into_iter()
            .map(|g| BulkGapAck {
                approver: g.approver,
                acted_at: g.acted_at,
                gap_id: g.gap_id,
                affected_ids: g.affected_ids,
                reason: g.reason,
            })
            .collect();
        let mut g = self.write_lock().await?;
        let report = g.acknowledge_gaps(&items).map_err(dyno_err)?;
        bulk_result(report, |decision_id| json!({ "decision_id": decision_id }))
    }

    #[tool(
        description = "Gaps that were reviewed and accepted, each with the reason given. Worth \
                       re-reading when the design shifts.",
        annotations(read_only_hint = true)
    )]
    pub async fn reviewed_gaps(&self) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.reviewed_gaps().map_err(dyno_err)?)
    }

    #[tool(
        description = "Withdraw a gap's acceptance: the Decision is marked superseded (kept, not \
                       deleted) and the gap returns to the open list.",
        annotations(read_only_hint = false)
    )]
    pub async fn withdraw_gap_acknowledgement(
        &self,
        Parameters(req): Parameters<GapIdReq>,
    ) -> Result<CallToolResult, McpError> {
        let mut g = self.write_lock().await?;
        let existed = g
            .withdraw_gap_acknowledgement(&req.gap_id)
            .map_err(dyno_err)?;
        // `withdrawn`, matching withdraw_question and delete_* (BL-57): every
        // "remove it if present" tool reports the same boolean shape.
        ok_json(json!({ "gap_id": req.gap_id, "withdrawn": existed }))
    }

    #[tool(
        description = "What can this design's graph actually SAY about the quality axes — the \
                       'ilities'? `DimensionAssessment.score` is only ever ASSERTED by a person \
                       or an LLM, while reflow2 separately computes modularity, articulation \
                       points, dependency cycles, misplaced capabilities, decomposition \
                       mismatches, build granularity and the trajectory bands — and connects \
                       none of it to the axis it informs. This connects them. IT NEVER DERIVES A \
                       SCORE and never writes to the graph: collapsing three cycles into \
                       `maintainability: 0.62` asserts a precision nobody has, which is exactly \
                       why TRL was kept out of that same float. ADVERSE IS INHERITED, NEVER \
                       RE-JUDGED — a finding counts against an axis only where the computation \
                       that produced it already calls it a defect; a ratio, a trajectory position \
                       and a granularity observation are reported as CONTEXT, because the modules \
                       that produce them deliberately refuse to grade. THE ANSWER IS NOT BLANKET: \
                       four of the nine axes — performance, security, scalability, observability \
                       — cannot be informed by a design graph at all and say so with the reason, \
                       rather than reading clean. The output worth reading is `worth_weighing`: \
                       targets where somebody asserted a good score on an axis a detector found \
                       something against. That is a disagreement between two records, and reflow2 \
                       rules on neither.",
        annotations(read_only_hint = true)
    )]
    pub async fn ility_report(
        &self,
        Parameters(_req): Parameters<IlityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.ility_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Where does this design sit on the trajectory from FUNCTION to STRUCTURE? \
                       Designs normally get function right first and structure right later, \
                       iteratively and organically — so a well-developed function layer with no \
                       declared seams is a NORMAL POSITION, not debt. Reports seven bands \
                       (intent, function, allocation, seams, realization, assurance, operation), \
                       each as a count over a population with the question it answers, and names \
                       the lowest-scoring one as the FRONTIER. THE FRONTIER IS RELATIVE, so there \
                       is no threshold anywhere in this reading: reflow2 states where a design \
                       IS and REFUSES to say where it SHOULD be, because a demonstrator may sit \
                       at function-first forever and be right while a fielded increment may not \
                       — the same rule that keeps it from defaulting a TRL gate. Bands scoring \
                       ABOVE the frontier are reported as normal rather than as work done out of \
                       order, because real designs run ahead of themselves. A band with nothing \
                       to measure reads as unmeasured, never as zero. No stage name is emitted: \
                       a label no computation reads would be a distinction that does not earn \
                       its keep. Pure arithmetic over edges already in the graph — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn maturity_report(
        &self,
        Parameters(_req): Parameters<MaturityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.maturity_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Does the BUILD separate what the DESIGN separates? Reports one fact and \
                       refuses a verdict: an artifact realizing N capabilities the design \
                       distinguishes is the build holding as one thing what the design holds as \
                       N. IT NEVER SAYS 'monolith', 'too big' or 'split it', carries NO severity, \
                       and rules on NEITHER side — N capabilities in one file may mean the file \
                       should be N files, or that the design over-decomposed, or that it is right \
                       for this phase (dec:report-dont-judge). THERE IS NO SIZE THRESHOLD: \
                       artifacts are compared against THIS design's own distribution, so an \
                       early-phase design where everything lives in one file has no outlier and \
                       is told nothing — a uniformly coarse design is not a broken one. Both \
                       cutoffs travel with the answer so they can be argued with, and \
                       `not_observed_about` names what it cannot see: unregistered artifacts, \
                       size of any kind, and outliers that mask each other. Pure arithmetic over \
                       REALIZES edges — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn granularity_report(
        &self,
        Parameters(_req): Parameters<GranularityReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.granularity_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Has the SHARED RECORD moved since this graph last looked? Answers the \
                       question a session should ask FIRST when other people write to the same \
                       design: somebody pulled their work into the committed export and this \
                       graph has never seen it. One entry per file this seat has synced with — \
                       `in_step` (exactly where this graph left it), `behind` (THE ACTIONABLE ONE \
                       — the record holds nodes this graph lacks; the ids are named and \
                       `import_graph` on that path takes them in), `moved_but_current` (somebody \
                       exported, you already hold it all), `missing`, `unreadable`. AN EMPTY \
                       ANSWER MEANS THIS SEAT HAS NEVER SYNCED WITH ANY FILE — never that all is \
                       well; the quiet targets are listed for exactly that reason. BEING AHEAD OF \
                       THE RECORD IS NOT REPORTED: unexported work is the normal state of a \
                       working session, and the check is gated on the file's content hash so it \
                       is silent unless somebody ELSE has been there. IT NEVER ACTS — no \
                       auto-import, because import is an upsert and an unasked one would silently \
                       overwrite live work (dec:ask-not-repair); it names the remedy and leaves \
                       the choice. `loop_status` carries the same finding as `record_moved` when \
                       there is one.",
        annotations(read_only_hint = true)
    )]
    pub async fn sync_status(
        &self,
        Parameters(_req): Parameters<SyncStatusReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        let Some(graph_path) = self.graph_path.as_deref() else {
            return ok_json(json!({
                "sync": [],
                "note": "this server holds no graph path, so it has no sync record to check",
            }));
        };
        let live_nodes = g.count_all_nodes().unwrap_or(0);
        let debts = crate::sync_debt::sync_debt(graph_path, live_nodes, &|| g.export_graph().ok());
        let mut out = json!({
            "sync": debts,
            "behind": debts.iter().filter(|d| d.is_actionable()).map(|d| d.message()).collect::<Vec<_>>(),
            "checked": debts.len(),
        });
        // Say what this roll did NOT open. A roll that quietly checks fewer
        // records than the seat knows about reads as "all clear" while the one
        // that moved sits unopened.
        if let Some(skipped) = crate::sync_debt::not_checked(graph_path)
            && let Some(obj) = out.as_object_mut()
        {
            obj.insert("not_checked".into(), json!(skipped));
        }
        ok_json(out)
    }

    #[tool(
        description = "What did this design BUILD that it records no consumer for? Reports one \
                       fact and refuses a verdict. A Capability at `realized`/`verified` with no \
                       incoming DEPENDS_ON, no PART_OF_FLOW and no Actor INTERACTS_WITH is \
                       reported as 'THIS DESIGN RECORDS NOTHING THAT CONSUMES IT' — and NEVER as \
                       'unused', 'dead' or 'delete it'. THAT WORDING IS THE FEATURE: reflow2 \
                       reads a design, never a running system, so a capability real users call \
                       daily whose consumer nobody modelled is INDISTINGUISHABLE here from one \
                       dead since it shipped, and a detector that collapsed the two would \
                       recommend deleting working code. ABSENCE IS ONLY INFORMATIVE WHEN PRESENCE \
                       IS THE HABIT: if the design records a consumer for fewer than half of what \
                       it built, the list is WITHHELD and the ratio itself is the finding, \
                       because naming everything would report the modelling style rather than \
                       what was built (measured on reflow2's own design, where the raw signal \
                       named 100 of 110). `signals_read` names the edges counted so a missing one \
                       can be argued for, and `not_observed_about` names what it cannot see. Pure \
                       arithmetic over existing edges — no file I/O.",
        annotations(read_only_hint = true)
    )]
    pub async fn consumption_report(
        &self,
        Parameters(_req): Parameters<ConsumptionReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let g = self.graph.read().await;
        ok_json(g.consumption_report().map_err(dyno_err)?)
    }

    #[tool(
        description = "Apply a reviewed HealProposal atomically (rigid mode = no-op). Pass a \
                       proposal `propose_heal` returned — every operation is checked against what \
                       HEAL proposes for the graph as it stands now, and anything else is refused \
                       before a single write, so hand-editing the proposal or reusing a stale one \
                       fails rather than merging the wrong nodes. Merging deletes a node and \
                       cannot be undone. Read `discarded` in the result: it lists what the merge \
                       could not carry onto the survivor.",
        annotations(read_only_hint = false)
    )]
    pub async fn apply_heal(
        &self,
        Parameters(req): Parameters<ApplyHealReq>,
    ) -> Result<CallToolResult, McpError> {
        let proposal: HealProposal = parse_struct_param(req.proposal, "HealProposal")?;
        let mut g = self.write_lock().await?;
        ok_json(g.apply_heal(&proposal).map_err(dyno_err)?)
    }
}

/// Send each repair explanation ONCE instead of once per finding.
///
/// `repair_is_a_judgement` is `Option<&'static str>` — a fixed literal per
/// detector branch, saying why a category of defect has no mechanical repair.
/// It is written per ROW, so a design with two dozen orphan nodes receives the
/// same 797-character paragraph two dozen times.
///
/// MEASURED on reflow2's own design, 2026-08-23: `detect_defects` was 46,399
/// characters, of which 45,186 were the findings and **52.3% of those were this
/// one field — 50 rows carrying 3 DISTINCT values.** Lifting them saves 19,843
/// characters and loses NOTHING: no list is withheld, no prose is truncated, no
/// judgement is made about what a reader needs. It is the same paragraph, sent
/// once.
///
/// THE RULE A READER FOLLOWS IS TOTAL, which is why the map is not simply a
/// substitute: a row keeps its own text whenever that text is not the one the
/// map holds for its category, so
/// `row.repair_is_a_judgement ?? repair_is_a_judgement[row.category]` is always
/// correct. Today the mapping is one text per category and every row lifts; if a
/// future detector gives one category two different explanations, the odd rows
/// keep theirs inline rather than being silently given the wrong one.
/// EXPOSED FOR TEST. The two list shapes are the whole subtlety here and a
/// fixture cannot reliably produce the scoped one — a region holding several
/// note-bearing findings of one category occurs on a real design and is
/// awkward to synthesise, because the categories that carry a note are the ones
/// whose findings are disconnected by definition. So the shape handling is
/// pinned directly rather than through a graph that may or may not contain it.
pub fn lift_repair_notes(out: &mut JsonValue) {
    const FIELD: &str = "repair_is_a_judgement";
    // `defects` unscoped, `items` scoped — `Scoped<T>` names its list `items`,
    // and looking only for `defects` made this a silent no-op on every scoped
    // call. Found by driving the built binary, not by a test, which is why there
    // is now a test.
    let list_key = if out.get("defects").is_some_and(JsonValue::is_array) {
        "defects"
    } else {
        "items"
    };
    let Some(defects) = out.get_mut(list_key).and_then(|d| d.as_array_mut()) else {
        return;
    };

    // First pass: the text each category will carry, and only where a category
    // is unanimous. A category with two explanations gets none, and its rows all
    // keep theirs.
    let mut by_category: HashMap<String, Option<String>> = HashMap::new();
    let mut seen_count: HashMap<String, usize> = HashMap::new();
    for row in defects.iter() {
        let (Some(cat), Some(text)) = (
            row.get("category").and_then(|c| c.as_str()),
            row.get(FIELD).and_then(|t| t.as_str()),
        ) else {
            continue;
        };
        *seen_count.entry(cat.to_string()).or_insert(0) += 1;
        match by_category.entry(cat.to_string()).or_insert(None) {
            slot @ None => *slot = Some(text.to_string()),
            Some(seen) if seen != text => {
                by_category.insert(cat.to_string(), None);
            }
            Some(_) => {}
        }
    }
    // ONLY WHERE IT ACTUALLY SAVES. One row carrying a paragraph costs less
    // inline than it does as a map entry plus the sentence explaining the map,
    // and a mechanism that fires where it does not pay is how a reply gets
    // bigger while claiming to get smaller.
    let shared: HashMap<String, String> = by_category
        .into_iter()
        .filter(|(cat, _)| seen_count.get(cat).copied().unwrap_or(0) > 1)
        .filter_map(|(cat, text)| text.map(|t| (cat, t)))
        .collect();
    if shared.is_empty() {
        return;
    }

    // Second pass: drop the field from every row the map now speaks for.
    for row in defects.iter_mut() {
        let Some(obj) = row.as_object_mut() else {
            continue;
        };
        let lift = matches!(
            (
                obj.get("category").and_then(|c| c.as_str()),
                obj.get(FIELD).and_then(|t| t.as_str()),
            ),
            (Some(cat), Some(text)) if shared.get(cat).map(String::as_str) == Some(text)
        );
        if lift {
            obj.remove(FIELD);
        }
    }

    if let Some(obj) = out.as_object_mut() {
        obj.insert(FIELD.into(), json!(shared));
        obj.insert(
            "repair_note_is_per_category".into(),
            json!(
                "`repair_is_a_judgement` above is keyed by `category` and is sent ONCE rather \
                 than repeated on every finding — on this design that field was over half the \
                 reply, 3 distinct paragraphs across 50 rows. Read it as \
                 `row.repair_is_a_judgement ?? repair_is_a_judgement[row.category]`: a row keeps \
                 its own text whenever that text differs from the one its category holds, so the \
                 fallback is never wrong. NOTHING IS WITHHELD HERE — no list shortened, no prose \
                 truncated; the same words are simply not sent fifty times."
            ),
        );
    }
}

/// Summarise the verification roll for `loop_status` — the digest that keeps
/// "one cheap call" true as the check count grows.
///
/// What survives in full is what a reader would act on: every check that is
/// **not currently passing**, in the loud-first order `verification_recency`
/// already sorts them into. The passing remainder is counted, never dropped
/// silently — `total` against `omitted` says exactly what is not here, the same
/// contract `scan_nodes` states for a capped page.
///
/// `never_run` is the one count worth promoting out of the roll: a `passing`
/// with no `last_run_at` is an assertion, not a measurement, and that is
/// invisible in a status tally alone.
fn verification_digest(
    all: &[reflow2_core::report::VerificationRecency],
    invalidated: &[reflow2_core::InvalidatedFinding],
) -> Result<serde_json::Value, serde_json::Error> {
    let mut by_status: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for v in all {
        *by_status.entry(v.status.as_str()).or_insert(0) += 1;
    }
    // 🛑 `superseded` IS NOT ATTENTION. A retired check is not a quiet failure;
    // it is a check somebody deliberately replaced, and surfacing it beside the
    // failing ones would make the loud-first list quieter by diluting it. This
    // filter said `!= "passing"` and would have swept every superseded check in
    // the moment the status value shipped — the enum value and its reader had
    // to land together or the addition would have made a report WORSE.
    let attention: Vec<_> = all
        .iter()
        .filter(|v| v.status != "passing" && v.status != "superseded")
        .collect();
    let never_run = all.iter().filter(|v| v.last_run_at.is_none()).count();

    // ⭐ A REPAIR CAN NOW SAY IT ANSWERED A CHECK, AND THIS IS WHERE THAT IS
    // READ. dev_storyflow, 2026-08-23: `where-am-i` reported a `failing` check's
    // two defects to a user as the live state of the system; both had been fixed
    // hours earlier and recorded on Constraint nodes. Every node was right and
    // the COMPOSITION was wrong, because nothing joined the repair to the check.
    //
    // 🛑 THE CHECK IS NOT REMOVED FROM `attention`, and that is the whole design.
    // A repair does not make a check pass — only a re-run can say what is true
    // now — so silencing it here would replace one wrong reading with another,
    // and it would be the silent truncation `parks` was careful not to become.
    // It stays listed, and it gains a sentence saying a claim stands against it.
    let claimed: std::collections::BTreeMap<&str, &reflow2_core::InvalidatedFinding> = invalidated
        .iter()
        .filter(|f| f.finding_type == "Verification")
        .map(|f| (f.finding_id.as_str(), f))
        .collect();
    let rerun_owed = claimed
        .values()
        .filter(|f| f.rerun_owed == Some(true))
        .count();
    let undated = claimed.values().filter(|f| f.rerun_owed.is_none()).count();

    // ⭐ THE SPLIT `never_run` COULD NOT MAKE UNTIL `IMPLEMENTS` EXISTED. A check
    // nobody has run and a check with NOTHING TO RUN were one number, and they
    // are different debts: the first is scheduling, the second means the check
    // exists only as a sentence. Retired checks are excluded from both — a
    // superseded check owes nobody an executable form.
    let no_executable_form = all
        .iter()
        .filter(|v| !v.has_executable_form && v.status != "superseded")
        .count();

    // TRUNCATE THE NAME, AND SAY SO. dev_storyflow, 2026-08-08: `loop_status`
    // is documented as "one cheap call" and is cheap to CALL and expensive to
    // READ — `verifications.attention[0].name` came back as a single ~450-word
    // paragraph holding a full graded walk report. Measured on reflow2's own
    // graph the same week: median Verification name 76 words, longest 654.
    //
    // ⭐ THE ANNOUNCEMENT IS NOT POLITENESS, it is the other half of this
    // increment. Silent truncation reads as "that is the whole name", which is
    // the same defect one layer over as a vacuous zero reading as a pass — and
    // `req:a-report-says-what-it-swept-and-whether-its-checks-ran` would forbid
    // it even if `names_truncated` were the only thing here saying so.
    //
    // The cause was fixed at the same time rather than only the symptom: names
    // are long because `description` was declared, fulltext, the embedding
    // field, and UNREACHABLE from `add_verification`, so authors had nowhere
    // else to write. Truncating alone would have been the stopgap
    // `rule:fix-it-properly-while-it-is-still-cheap` forbids.
    let mut truncated_count = 0usize;
    let attention: Vec<serde_json::Value> = attention
        .iter()
        .map(|v| {
            let mut item = serde_json::to_value(v)?;
            if let Some(obj) = item.as_object_mut() {
                let shortened = obj.get("name").and_then(|n| n.as_str()).and_then(|name| {
                    let words: Vec<&str> = name.split_whitespace().collect();
                    (words.len() > NAME_WORDS_IN_ROLLUP).then(|| {
                        (
                            format!("{} …", words[..NAME_WORDS_IN_ROLLUP].join(" ")),
                            words.len(),
                        )
                    })
                });
                if let Some((short, full_words)) = shortened {
                    truncated_count += 1;
                    obj.insert("name".into(), json!(short));
                    obj.insert("name_truncated".into(), json!(true));
                    obj.insert("name_words".into(), json!(full_words));
                }
                // The verdict below is stale if somebody claimed it. Said ON the
                // row, because the row is what a reader quotes.
                if let Some(f) = obj
                    .get("verification_id")
                    .and_then(|v| v.as_str())
                    .and_then(|id| claimed.get(id))
                {
                    obj.insert("invalidated_by".into(), json!(f.claimed_by));
                    obj.insert("rerun_owed".into(), json!(f.rerun_owed));
                    obj.insert(
                        "invalidation_note".into(),
                        json!(match f.rerun_owed {
                            Some(true) =>
                                "A repair claims to have answered this. THE VERDICT \
                                           BELOW PREDATES IT — re-run before quoting it as \
                                           current.",
                            Some(false) =>
                                "A repair claims to have answered this, and the last \
                                            run POSTDATES it, so this verdict already reflects \
                                            the repair.",
                            None =>
                                "A repair claims to have answered this, but one side carries \
                                     no date, so whether the last run already reflects it cannot \
                                     be told. Re-run, or date the claim.",
                        }),
                    );
                }
            }
            Ok(item)
        })
        .collect::<Result<_, serde_json::Error>>()?;

    let mut out = json!({
        "total": all.len(),
        "by_status": by_status,
        "never_run": never_run,
        "no_executable_form": no_executable_form,
        "attention": attention,
        "omitted": all.len() - attention.len(),
        "full_list": "graph_report {\"include_verifications\": true} — every check with its \
                      status and last run. NAME THE FLAG: that report withholds the roll for the \
                      same reason this digest exists, so a bare `graph_report` no longer returns \
                      it.",
    });
    if !claimed.is_empty()
        && let Some(obj) = out.as_object_mut()
    {
        obj.insert("invalidation_claims".into(), json!(claimed.len()));
        obj.insert(
            "rerun_owed".into(),
            json!(format!(
                "{} check(s) carry a repair claiming to have answered them: {rerun_owed} where the \
                 claim POSTDATES the last run (re-run owed), {undated} where a date is missing on \
                 one side so nobody can say. They are still listed in `attention` and still \
                 counted — a claim says a verdict is STALE, never that it has turned, and only a \
                 run can say what is true now.",
                claimed.len()
            )),
        );
    }
    if truncated_count > 0
        && let Some(obj) = out.as_object_mut()
    {
        obj.insert(
            "names_truncated".into(),
            json!(format!(
                "{truncated_count} name(s) above are CUT SHORT at {NAME_WORDS_IN_ROLLUP} words \
                 and carry `name_truncated: true` with their real `name_words`. Read the whole \
                 one with get_node. A long name usually means a report was written into it \
                 because there was nowhere else — `description` (what the check IS) and \
                 `findings` (what a run FOUND) are where that belongs."
            )),
        );
    }
    Ok(out)
}

/// How much of a Verification's `name` the `loop_status` rollup shows.
///
/// Enough to identify the check in a list and not enough to hide the reply, on
/// a corpus whose median name is 76 words. It is a display bound, never a
/// storage one: nothing is lost, and `name_truncated` says when it applied.
const NAME_WORDS_IN_ROLLUP: usize = 25;
