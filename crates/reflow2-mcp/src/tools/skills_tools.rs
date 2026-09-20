//! The served-skill tools: read one skill, list them all, read the working
//! instructions, and name the design.
//!
//! ⭐ SPLIT FROM `skills.rs` 2026-08-20 TO BREAK A MODULE CYCLE. `skills.rs` held
//! both the skill DATA (compiled in by build.rs) and the TOOLS over it, and the
//! tools are implemented on `ReflowService` — so `skills` imported `service`
//! while `service` called `skills::catalogue()` for its own instructions.
//! Mutual, and invisible: a module cycle inside one crate is legal Rust.
//!
//! The data half stays in `skills.rs` and depends on nothing; this half depends
//! on the service, exactly like every other module under `tools/`. That was
//! always the intended shape — `Self::skills_router()` was already summed in
//! `ReflowService::new` alongside the rest — and only the file placement
//! disagreed with it.
//!
//! FOUND BY RUNNING ADOPT OVER REFLOW2'S OWN SOURCE.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData as McpError, tool, tool_router};
use serde_json::json;

use crate::service::ReflowService;
use crate::skills::{INSTRUCTIONS, SKILLS, alias_hint, find};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetSkillReq {
    /// The skill's name, as `list_skills` reports it.
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListSkillsReq {
    /// How many characters of JSON this reply may spend before prose is
    /// withheld to make it fit (default 30,000). See `reply_budget`: counts and
    /// ids are never budgeted away, so a shorter answer is never a quieter one.
    #[serde(default)]
    pub budget_chars: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInstructionsReq {
    /// How many characters of JSON this reply may spend before prose is
    /// withheld to make it fit (default 30,000). See `reply_budget`: counts and
    /// ids are never budgeted away, so a shorter answer is never a quieter one.
    #[serde(default)]
    pub budget_chars: Option<usize>,
    /// One section slug from the `sections` manifest, e.g. `the-loop`.
    ///
    /// **THIS EXISTS BECAUSE THE WHOLE DOCUMENT DOES NOT ALWAYS ARRIVE.** It is
    /// ~27 KB, and a client-side result cap silently keeps the front of it: a
    /// real consumer received the first ~19.5 KB twice, nine days apart, losing
    /// the gap-to-question handshake and the entire tool inventory both times,
    /// with nothing in the reply disagreeing with what it held.
    ///
    /// Omit it for the whole document plus the manifest. Pass it to fetch one
    /// part at a time, which is the path that works on a capped client. An
    /// unknown slug is REFUSED and lists the legal ones rather than returning
    /// an empty document, because a section that came back blank and a section
    /// that does not exist must not be the same answer.
    #[serde(default)]
    pub section: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignIdentityReq {
    /// A new human-facing label for this design. The id never changes — every
    /// stored key and every export ever written names it. Omit to just read.
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsageReportReq {
    /// Start of the window as a plain date, `YYYY-MM-DD`. Omit it for
    /// "since the previous report" — the marker the last `usage_report` left
    /// in the ledger — which falls back to the whole ledger when there has
    /// never been one. A named date wins over the marker.
    #[serde(default)]
    pub since: Option<String>,
    /// Look without leaving a marker. Off by default: an ordinary report
    /// closes its window so the next one starts after it, and a report that
    /// did not would count the same calls twice.
    #[serde(default)]
    pub peek: bool,
}

#[tool_router(router = skills_router, vis = "pub")]
impl ReflowService {
    /// The catalogue, with full trigger conditions.
    #[tool(
        description = "List the reflow2 skills this server carries — name and the full description \
                       an agent matches on to decide whether a skill applies. Skills are served by \
                       the server rather than installed into the project, so this list always \
                       matches the running reflow2. Read one with get_skill BEFORE doing the work \
                       it covers. \
                       Ask for this when you want to see which playbooks, procedures or step-by-step guides are available for working on this design.",
        annotations(read_only_hint = true)
    )]
    pub async fn list_skills(
        &self,
        Parameters(req): Parameters<ListSkillsReq>,
    ) -> Result<CallToolResult, McpError> {
        let items: Vec<_> = SKILLS
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "shortcut": crate::skills::shortcut_for(s.name),
                    "summary": s.summary,
                    "audience": s.audience,
                    "description": s.description,
                })
            })
            .collect();
        // COUNTED, not written: the note used to say "eight of these differ"
        // while the live list differed in nine, then ten (flo2 F9, 2026-09-18).
        let differing = SKILLS
            .iter()
            .filter(|s| crate::skills::shortcut_for(s.name) != format!("/{}", s.name))
            .count();
        let mut payload = json!({
            "count": items.len(),
            "skills": items,
            "note": format!(
                "Served from the reflow2 binary (dec:skills-served), so they cannot drift from \
                 the version you are running. Your harness does NOT auto-load these — call \
                 get_skill to read one in full. `shortcut` is what a PERSON types; {differing} of \
                 these differ from the skill name, so never derive it by matching names. \
                 `summary` is the line a person reads and `audience` says who it is for \
                 (anyone / operator / agent); `description` is the trigger an agent matches on."
            )
        });
        // The reminder rides the response the agent is already reading.
        if let (Some(lens), Some(obj)) = (self.lens_for_response().await, payload.as_object_mut()) {
            obj.insert("lens".into(), serde_json::Value::String(lens));
        }
        structured(crate::reply_budget::bound_reply(
            payload,
            req.budget_chars
                .unwrap_or(crate::reply_budget::DEFAULT_REPLY_BUDGET_CHARS),
            "Every skill is still listed with its name and shortcut; read one in full with get_skill.",
        ))
    }

    /// One skill, in full.
    #[tool(
        description = "Find the skill that fits a job you can describe but not name — the \
                       skills counterpart of find_tools. Say the job in your own words (\"we \
                       already have a working codebase, get it under control\", \"if I change \
                       this what else moves\"); the served skills are ranked by their names, \
                       one-line summaries and trigger descriptions, and every match carries the \
                       skill's name (what get_skill takes), the shortcut a PERSON types, its \
                       summary and who it is for. With thirty skills nobody remembers which one \
                       fits, including their author; this is the catalogue you ask instead of \
                       reading them all. Ask for this when you want to know which skill fits the \
                       job you are about to do.",
        annotations(read_only_hint = true)
    )]
    pub async fn find_skills(
        &self,
        Parameters(req): Parameters<crate::service::FindSkillsReq>,
    ) -> Result<CallToolResult, McpError> {
        let query = req.query.to_lowercase();
        let terms: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|t| !t.is_empty())
            .collect();
        // The same scorer find_tools uses, over the same shape: a name, a text
        // (summary then description, so the person's line counts), no
        // parameters. One scorer, two catalogues, so they cannot drift apart.
        let corpus: Vec<(String, String)> = SKILLS
            .iter()
            .map(|s| {
                (
                    s.name.to_lowercase(),
                    format!("{} {}", s.summary, s.description).to_lowercase(),
                )
            })
            .collect();
        let weighted = crate::service::term_weights(&terms, &corpus);
        let mut scored: Vec<(f64, serde_json::Value)> = SKILLS
            .iter()
            .filter_map(|s| {
                let text = format!("{} {}", s.summary, s.description);
                let score = crate::service::score_tool(s.name, &text, &[], &weighted);
                (score > 0.0).then(|| {
                    (
                        score,
                        json!({
                            "name": s.name,
                            "shortcut": crate::skills::shortcut_for(s.name),
                            "summary": s.summary,
                            "audience": s.audience,
                            "score": score,
                        }),
                    )
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1["name"].as_str().cmp(&b.1["name"].as_str()))
        });
        let matched = scored.len();
        let limit = req.limit.unwrap_or(5).max(1);
        let items: Vec<serde_json::Value> =
            scored.into_iter().take(limit).map(|(_, v)| v).collect();
        structured(json!({
            "count": items.len(),
            "items": items,
            "matched": matched,
            "omitted": matched.saturating_sub(items.len()),
            "searched": SKILLS.len(),
            "query": req.query,
            "note": "`name` is what get_skill takes; `shortcut` is what a person types and can differ from the name.",
        }))
    }

    #[tool(
        description = "Read one reflow2 skill in full, by name (see list_skills). Returns the \
                       whole SKILL.md — follow it as written. Call this BEFORE the work the skill \
                       covers, not after: these describe how to do the step, not how to report it. \
                       Ask for this when you want the full playbook, procedure or step-by-step guide for a kind of task — how to do it properly, not just what exists.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_skill(
        &self,
        Parameters(req): Parameters<GetSkillReq>,
    ) -> Result<CallToolResult, McpError> {
        let Some(skill) = find(&req.name) else {
            // Rule 4: say what would have worked. A caller who typed a slash
            // command's name gets the mapping FIRST — the list of twenty is
            // what they already could not find themselves in.
            let known: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
            let lead = match alias_hint(&req.name) {
                Some(hint) => format!("no skill named '{}'. {hint} ", req.name),
                None => format!("no skill named '{}'. ", req.name),
            };
            return Err(McpError::invalid_params(
                format!(
                    "{lead}This server carries {}: {}",
                    known.len(),
                    known.join(", ")
                ),
                None,
            ));
        };
        let mut payload = json!({
            "name": skill.name,
            "summary": skill.summary,
            "audience": skill.audience,
            "description": skill.description,
            "body": skill.body,
        });
        // The reminder rides the response the agent reads immediately BEFORE
        // doing the work — the one moment it is certain to be looked at.
        if let (Some(lens), Some(obj)) = (self.lens_for_response().await, payload.as_object_mut()) {
            obj.insert("lens".into(), serde_json::Value::String(lens));
        }
        // And so do the design's own lessons for this step, for the same reason
        // (req:a-lesson-is-served-at-the-step-it-concerns). Best effort: a
        // skill is served whether or not the graph could be read.
        let lessons = self.lessons_for_step(skill.name).await;
        if let (false, Some(obj)) = (lessons.is_empty(), payload.as_object_mut()) {
            obj.insert(
                "lessons".into(),
                json!({
                    "note": format!(
                        "{} lesson(s) THIS DESIGN holds for the `{}` step, recorded by \
                         earlier sessions on this project. Read them before the work; a \
                         lesson filed elsewhere was measured not to change the next call.",
                        lessons.len(),
                        skill.name
                    ),
                    "items": lessons,
                }),
            );
        }
        structured(payload)
    }
    /// The working instructions, served rather than installed.
    #[tool(
        description = "How to work THIS project with reflow2: the loop, the standing rules, and \
                       what to do first on an existing design. Served by the server rather than \
                       stored in the project, so it always matches the reflow2 you are talking to. \
                       Read it before the first design action of a session — the file in the repo \
                       is only a pointer here. IT IS ~27 KB AND SOME CLIENTS CAP A TOOL RESULT, \
                       so every reply states `total_bytes` and a `sections` manifest: if what you \
                       hold is shorter than `returned_bytes`, your client truncated it and you \
                       can fetch the rest a section at a time with `section`. A capped read used \
                       to be silent, and what it removed was the tail — the gap→question \
                       handshake and the whole tool inventory. \
                       Ask for this first: it says how I am supposed to work with the design here.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_instructions(
        &self,
        Parameters(req): Parameters<GetInstructionsReq>,
    ) -> Result<CallToolResult, McpError> {
        let sections = crate::skills::instruction_sections();
        let pointer = crate::skills::pointer_section();
        let mut manifest: Vec<serde_json::Value> = sections
            .iter()
            .map(|s| json!({"section": s.slug, "title": s.title, "bytes": s.body.len()}))
            .collect();
        // Listed beside the document's own sections, never inside them: the
        // pointer is what a project HOLDS, not part of what it is told.
        manifest.push(json!({
            "section": pointer.slug, "title": pointer.title, "bytes": pointer.body.len(),
            "note": "not part of this document — the file to write into a project that has no instruction file (genesis / adopt step 0)"
        }));

        let (body, returned_section) = match req.section.as_deref() {
            None => (INSTRUCTIONS.to_string(), None),
            Some(want) if want == pointer.slug => {
                (pointer.body.clone(), Some(pointer.slug.clone()))
            }
            Some(want) => {
                let Some(hit) = sections.iter().find(|s| s.slug == want) else {
                    let legal: Vec<&str> = sections.iter().map(|s| s.slug.as_str()).collect();
                    return Err(McpError::invalid_params(
                        format!(
                            "get_instructions: no section {want:?}. The sections are: {}. \
                             Call with no `section` for the whole document and this manifest.",
                            legal.join(", ")
                        ),
                        None,
                    ));
                };
                (hit.body.clone(), Some(hit.slug.clone()))
            }
        };

        // ⚠️ THIS TOOL IS NOT CHARACTER-TRIMMED, AND THAT IS DELIBERATE.
        // Its payload is a DOCUMENT: a truncated instruction set is not a
        // shorter answer but a corrupt one, the same reason export_graph is
        // excluded from `reply_budget`. Worse, the note below tells the reader
        // that `instructions` shorter than `returned_bytes` means THEIR CLIENT
        // capped it — so trimming here would make this tool accuse the client
        // of reflow2's own edit, and that signal was added because a real
        // consumer lost the back half of the document twice, nine days apart.
        //
        // The honest bound is the one the tool already has: refuse to send a
        // whole document that cannot fit, and hand back the manifest so the
        // caller fetches it a section at a time.
        let budget = req
            .budget_chars
            .unwrap_or(crate::reply_budget::DEFAULT_REPLY_BUDGET_CHARS);
        if returned_section.is_none() && body.len() > budget {
            return structured(json!({
                "instructions": null,
                "section": null,
                "sections": manifest,
                "total_bytes": INSTRUCTIONS.len(),
                "returned_bytes": 0,
                "budget": {
                    "applied": true,
                    "budget_chars": budget,
                    "full_chars": body.len(),
                    "detail": "whole_document_withheld",
                    "note": format!(
                        "WITHHELD WHOLE, NOT TRIMMED. The full instructions are {} characters \
                         against a budget of {budget}, and this document is not something a \
                         character limit can shorten honestly — half an instruction set reads \
                         exactly like a complete one. Nothing is lost: every section is listed \
                         in `sections`, and get_instructions {{\"section\": \"<slug>\"}} returns \
                         each in full. Raise `budget_chars` if this client really has the room.",
                        body.len()
                    ),
                },
            }));
        }

        structured(json!({
            "instructions": body,
            "section": returned_section,
            "sections": manifest,
            "total_bytes": INSTRUCTIONS.len(),
            "returned_bytes": body.len(),
            "note": "Served from the reflow2 binary (req:thin-install), so upgrading reflow2 \
                     changes these instructions without changing anything in your repository. \
                     The skills they refer to come from list_skills / get_skill. \
                     ⚠️ IF `instructions` IS SHORTER THAN `returned_bytes`, YOUR CLIENT CAPPED \
                     IT — this reply states its own length so a short read is detectable \
                     instead of silent. Fetch the parts you are missing with \
                     get_instructions {\"section\": \"<slug from sections>\"}.",
        }))
    }
    /// What design lives at each of these paths — without opening any of them.
    #[tool(
        description = "Say what design lives at each given path, WITHOUT opening or writing \
                       anything — the sibling of design_identity, which answers only for the \
                       design THIS session is bound to. YOU find the candidate paths (`find . \
                       -maxdepth 3 -name .reflow2`, and the same upward); reflow2 does no file \
                       navigation, and this answers what each one IS. Use it before starting a \
                       design anywhere, before pointing a project at a graph, and whenever \
                       'which design am I in?' has more than one plausible answer. Returns each \
                       design's stable id, label, minted-or-adopted origin and schema stamp. It \
                       reads only the sidecar files beside each store, so no lock is taken, \
                       nothing is written, and a design another session holds right now describes \
                       fine. Node counts are deliberately absent: counting means opening, and \
                       opening MINTS an identity where there is none.",
        annotations(read_only_hint = true)
    )]
    pub async fn describe_designs(
        &self,
        Parameters(req): Parameters<crate::latent::DescribeDesignsReq>,
    ) -> Result<CallToolResult, McpError> {
        if req.paths.is_empty() {
            return Err(McpError::invalid_params(
                "describe_designs needs at least one path. Walk the tree first — \
                 `find . -maxdepth 3 -name .reflow2` — and pass what you found; an empty sweep \
                 reported as 'nothing here' is the answer that starts an unwanted design."
                    .to_string(),
                None,
            ));
        }
        structured(crate::latent::describe_designs_payload(&req.paths))
    }

    /// Which design is this, and what is it called?
    #[tool(
        description = "Which design this graph holds: its durable id and its human label. The id is \
                       assigned once, with no coordination, and never changes — it namespaces every \
                       stored key and appears in every export, so two designs can tell each other \
                       apart when they compose (mirror_surface). Pass `label` to RENAME the design; \
                       the id is untouched. Read this when a session needs to say WHICH design it \
                       is working in. \
                       Ask for this when you want to know which design or project's model this session is connected to right now.",
        annotations(read_only_hint = false)
    )]
    pub async fn design_identity(
        &self,
        Parameters(req): Parameters<DesignIdentityReq>,
    ) -> Result<CallToolResult, McpError> {
        let Some(graph_path) = self.graph_path.as_deref() else {
            // An in-memory graph has no sidecar to remember in, and saying so is
            // better than inventing an identity that dies with the process.
            return structured(json!({
                "graph_id": self.graph.read().await.graph_id().to_string(),
                "label": null,
                "note": "This is an in-memory graph — it has no durable identity, because there is \
                         no store beside which to remember one.",
            }));
        };
        if let Some(label) = req.label {
            let identity = reflow2_core::identity::set_label(graph_path, &label)
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
            return structured(serde_json::to_value(&identity).unwrap_or(json!({})));
        }
        let identity = reflow2_core::identity::resolve(
            graph_path,
            reflow2_core::DEFAULT_GRAPH_ID,
            // Already established by the open that got us here; the probe is
            // only for a graph meeting reflow2 for the first time.
            || false,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        structured(serde_json::to_value(&identity).unwrap_or(json!({})))
    }

    /// The computed spine of a `/feedback` report, from the usage ledger.
    #[tool(
        description = "What this project's sessions actually asked reflow2 to do, and where reflow2 declined — \
                       computed from the usage ledger the server keeps beside the design \
                       (`<graph>.usage.jsonl`), never from an agent's memory. Calls by tool, the served tools \
                       NEVER called in the window, refusals by class and by tool (near_match, missing_argument, \
                       unknown_argument, unresolved_reference, refused, other), errors, skills fetched, the \
                       harnesses that connected, and the metadata the server knows without asking: reflow2's \
                       version, OS and architecture, the negotiated protocol revision, the design's node count. \
                       THE LEDGER HOLDS THE VERB, NEVER THE OBJECT — no argument, no node id, no message is \
                       ever recorded (req:telemetry-carries-usage-never-design-content), so this report can \
                       leave the machine without carrying the design. The window is SINCE THE PREVIOUS REPORT \
                       by default, and this call leaves the marker that closes it — the unit is the project \
                       across every session and harness, not one session. Pass `since` (YYYY-MM-DD) to name \
                       the start instead; `peek` to look without closing the window. An in-memory design has no \
                       ledger and says so. What it cannot see: anything outside reflow2's own tool calls — a \
                       shell error, a git failure — and WHICH MODEL the agent is, which no harness sends. \
                       Ask for this when you want a tally of which reflow2 tools were used and which calls failed, for feedback on reflow2 itself. \
                       Ask for this to give me the numbers on how this project has been using reflow2, to send a maintainer.",
        annotations(read_only_hint = false)
    )]
    pub async fn usage_report(
        &self,
        Parameters(req): Parameters<UsageReportReq>,
    ) -> Result<CallToolResult, McpError> {
        let Some(graph_path) = self.graph_path.as_deref() else {
            return structured(json!({
                "ledger": null,
                "note": "This is an in-memory design — there is no store to keep a usage ledger beside, \
                         so nothing was recorded and there is nothing to report.",
            }));
        };
        let since_unix = match req.since.as_deref() {
            None => None,
            Some(s) => match reflow2_core::dates::parse_day(s) {
                Some(days) => Some(u64::try_from(days.max(0)).unwrap_or(0) * 86_400),
                None => {
                    return Err(McpError::invalid_params(
                        format!(
                            "`since` must be a plain date, YYYY-MM-DD; got {s:?}. Omit it for \
                             \"since the previous report\"."
                        ),
                        None,
                    ));
                }
            },
        };
        let lines = crate::usage::read_all(graph_path);
        let (window, start) = crate::usage::window(&lines, since_unix);
        let served: Vec<String> = self
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        let tally = crate::usage::tally(&window, start, &served);
        let node_count = self.graph.read().await.count_all_nodes().unwrap_or(0);
        let handshake = crate::handshake::Handshake::read(graph_path);
        if !req.peek {
            crate::usage::append(graph_path, &crate::usage::UsageLine::report_marker());
        }
        structured(json!({
            "ledger": crate::usage::usage_path(graph_path).display().to_string(),
            "window_described": start.describe(),
            "tally": tally,
            "environment": {
                "reflow2_version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "harness_last_connected": handshake
                    .as_ref()
                    .map(|h| format!("{} {}", h.client_name, h.client_version)),
                "protocol_negotiated": handshake.as_ref().map(|h| h.negotiated.clone()),
                "design_nodes": node_count,
                "model": "NOT KNOWN TO THE SERVER — no harness sends the model over MCP; the agent \
                          composing the report states it, labelled as self-reported.",
            },
            "marker_left": !req.peek,
            "not_seen": "Anything outside reflow2's own tool calls: shell, git, CI. report-friction \
                         remains the form for those.",
        }))
    }
}

/// The signposted reply, which is what every other tool returns.
///
/// 🛑 THIS USED TO BE A SECOND REPLY BUILDER, and its doc-comment said "Same
/// shape every other tool returns: structured content plus readable text."
/// That was TRUE when it was written and stopped being true on 2026-08-23,
/// when `json_result` replaced the duplicated text block with a one-line
/// signpost (`fact:every-reply-was-sent-twice`). Nothing read this comment for
/// 28 days, so six tools — `get_instructions`, `list_skills`, `get_skill`,
/// `find_skills`, `usage_report`, `design_identity` — went on sending the
/// payload twice to every client, 64,120 bytes on the wire for 31,374 of
/// payload on the first call of a session.
///
/// AND THE COST WAS NOT ONLY BYTES: `content_policy::apply` rewrites a reply
/// only if it is ALREADY in signpost shape, so the per-client rule was inert on
/// exactly those six. OpenCode's empty-block fallback never fired on the tools
/// carrying the instructions and the skills; Grok was served correctly by
/// accident.
///
/// It now delegates rather than building, so there is ONE builder to change.
/// `tools/a_reply_is_sent_once.py` asks the whole served surface, because the
/// Rust test that owned this invariant asked one tool out of 191.
fn structured(payload: serde_json::Value) -> Result<CallToolResult, McpError> {
    crate::service::json_result(payload)
}
