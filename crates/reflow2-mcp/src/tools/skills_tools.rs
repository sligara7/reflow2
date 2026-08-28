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
use rmcp::model::{CallToolResult, ContentBlock};
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
pub struct ListSkillsReq {}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInstructionsReq {
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

#[tool_router(router = skills_router, vis = "pub")]
impl ReflowService {
    /// The catalogue, with full trigger conditions.
    #[tool(
        description = "List the reflow2 skills this server carries — name and the full description \
                       an agent matches on to decide whether a skill applies. Skills are served by \
                       the server rather than installed into the project, so this list always \
                       matches the running reflow2. Read one with get_skill BEFORE doing the work \
                       it covers.",
        annotations(read_only_hint = true)
    )]
    pub async fn list_skills(
        &self,
        Parameters(_): Parameters<ListSkillsReq>,
    ) -> Result<CallToolResult, McpError> {
        let items: Vec<_> = SKILLS
            .iter()
            .map(|s| json!({"name": s.name, "description": s.description}))
            .collect();
        let mut payload = json!({
            "count": items.len(),
            "skills": items,
            "note": "Served from the reflow2 binary (dec:skills-served), so they cannot drift from \
                     the version you are running. Your harness does NOT auto-load these — call \
                     get_skill to read one in full."
        });
        // The reminder rides the response the agent is already reading.
        if let (Some(lens), Some(obj)) = (self.lens_for_response().await, payload.as_object_mut()) {
            obj.insert("lens".into(), serde_json::Value::String(lens));
        }
        structured(payload)
    }

    /// One skill, in full.
    #[tool(
        description = "Read one reflow2 skill in full, by name (see list_skills). Returns the \
                       whole SKILL.md — follow it as written. Call this BEFORE the work the skill \
                       covers, not after: these describe how to do the step, not how to report it.",
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
            "description": skill.description,
            "body": skill.body,
        });
        // The reminder rides the response the agent reads immediately BEFORE
        // doing the work — the one moment it is certain to be looked at.
        if let (Some(lens), Some(obj)) = (self.lens_for_response().await, payload.as_object_mut()) {
            obj.insert("lens".into(), serde_json::Value::String(lens));
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
                       handshake and the whole tool inventory.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_instructions(
        &self,
        Parameters(req): Parameters<GetInstructionsReq>,
    ) -> Result<CallToolResult, McpError> {
        let sections = crate::skills::instruction_sections();
        let manifest: Vec<serde_json::Value> = sections
            .iter()
            .map(|s| json!({"section": s.slug, "title": s.title, "bytes": s.body.len()}))
            .collect();

        let (body, returned_section) = match req.section.as_deref() {
            None => (INSTRUCTIONS.to_string(), None),
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
                       is working in.",
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
}

/// Same shape every other tool returns: structured content plus readable text.
fn structured(payload: serde_json::Value) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    let mut result = CallToolResult::structured(payload);
    result.content = vec![ContentBlock::text(text)];
    Ok(result)
}
