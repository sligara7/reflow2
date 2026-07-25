//! The skills, served from the binary instead of copied into your project.
//!
//! `dec:skills-served` (accepted 2026-07-25) and `req:thin-install`. The
//! consumer's repo gets a pointer paragraph and an MCP config; everything else
//! that used to be installed — fifteen skills across two directory trees —
//! now lives here, compiled in by `build.rs` and served on demand.
//!
//! **What this trades, stated once so it is never a surprise.** A skill in
//! `.claude/skills/` is auto-matched by the harness from its frontmatter
//! description: the agent never asks for it, the harness offers it. A served
//! skill has no such magic — the agent has to know it exists. That is why the
//! catalogue goes into the server *instructions* (see `catalogue`), which every
//! client puts in the agent's context at handshake: it is progressive
//! disclosure rebuilt by hand, and it is the price of never being stale.
//!
//! The staleness it buys off was real and measured: reflow2's own installed kit
//! manifest read 0.8.0 with twelve skills while the project was at 0.11.0 with
//! fifteen, and nothing anywhere noticed.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData as McpError, tool, tool_router};
use serde_json::json;

use crate::service::ReflowService;

/// One skill, compiled in from `getting-started/skills/<name>/SKILL.md`.
pub struct EmbeddedSkill {
    /// Directory name and frontmatter `name` — verified equal at build time.
    pub name: &'static str,
    /// The frontmatter `description`: *when to reach for this*, which is what
    /// an agent matches on.
    pub description: &'static str,
    /// The whole SKILL.md, frontmatter included.
    pub body: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/skills_generated.rs"));

/// The skill named, if it exists.
pub fn find(name: &str) -> Option<&'static EmbeddedSkill> {
    SKILLS.iter().find(|s| s.name == name)
}

/// The catalogue that goes into the server instructions.
///
/// One line per skill, carrying the **trigger** rather than a summary: the
/// first sentence of the description is written as "Use when…" in every skill,
/// which is precisely the sentence a harness would have matched on. The full
/// descriptions are one `list_skills` call away, and the line says so.
pub fn catalogue() -> String {
    let mut out = String::from(
        "SKILLS ARE SERVED, NOT INSTALLED. This project's reflow2 skills live in the server, so \
         they always match the running version and nothing in your repo goes stale. They are NOT \
         auto-loaded by your harness — call `get_skill` to read one in full before doing the work \
         it covers, and `list_skills` for the complete trigger conditions. Available:",
    );
    for skill in SKILLS {
        out.push_str("\n- ");
        out.push_str(skill.name);
        out.push_str(": ");
        out.push_str(trigger(skill.description));
    }
    out
}

/// The first sentence of a description — its trigger condition.
///
/// Truncating a description is a real risk (an agent could skip a skill it
/// needed on a partial reading), which is why every caller is pointed at
/// `list_skills` for the rest rather than left to assume this is all there is.
fn trigger(description: &str) -> &str {
    match description.find(". ") {
        Some(i) => &description[..=i],
        None => description,
    }
}

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
pub struct GetInstructionsReq {}

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
        let payload = json!({
            "count": items.len(),
            "skills": items,
            "note": "Served from the reflow2 binary (dec:skills-served), so they cannot drift from \
                     the version you are running. Your harness does NOT auto-load these — call \
                     get_skill to read one in full."
        });
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
            // Rule 4: say what would have worked.
            let known: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
            return Err(McpError::invalid_params(
                format!(
                    "no skill named '{}'. This server carries {}: {}",
                    req.name,
                    known.len(),
                    known.join(", ")
                ),
                None,
            ));
        };
        structured(json!({
            "name": skill.name,
            "description": skill.description,
            "body": skill.body,
        }))
    }
    /// The working instructions, served rather than installed.
    #[tool(
        description = "How to work THIS project with reflow2: the loop, the standing rules, and \
                       what to do first on an existing design. Served by the server rather than \
                       stored in the project, so it always matches the reflow2 you are talking to. \
                       Read it before the first design action of a session — the file in the repo \
                       is only a pointer here.",
        annotations(read_only_hint = true)
    )]
    pub async fn get_instructions(
        &self,
        Parameters(_): Parameters<GetInstructionsReq>,
    ) -> Result<CallToolResult, McpError> {
        structured(json!({
            "instructions": INSTRUCTIONS,
            "note": "Served from the reflow2 binary (req:thin-install), so upgrading reflow2 \
                     changes these instructions without changing anything in your repository. \
                     The skills they refer to come from list_skills / get_skill.",
        }))
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
