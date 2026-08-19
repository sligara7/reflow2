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

/// What a user-facing slash command is actually served as.
///
/// The commands in `getting-started/commands/` are named for what a person
/// would type — `/rules`, `/where` — while the skills are named for what they
/// do. `get_skill('rules')` therefore failed with a list of twenty names, none
/// of them `rules`, and an agent told to read a skill first could not reach it
/// by the name it had been advertised under (reported 2026-08-19).
///
/// ⚠️ TWO OF THESE RESOLVE TO NO SKILL AT ALL, and saying so is the point.
/// `/decisions` and `/debt` are commands that call a TOOL directly —
/// `scan_nodes(Decision)` and `loop_status` — so answering them with a
/// plausible-looking skill name would send the caller somewhere that cannot
/// help. A refusal that names what would have worked has to be able to say
/// "nothing here, use this tool instead" (`req:a-refusal-names-what-would-
/// have-worked`).
const COMMAND_ALIASES: &[(&str, Result<&str, &str>)] = &[
    ("rules", Ok("governance-proposal")),
    ("req", Ok("capture-intent")),
    ("gaps", Ok("detect-and-ask")),
    ("where", Ok("where-am-i")),
    ("kpp", Ok("kpp-proposal")),
    ("health", Ok("check-health")),
    ("decisions", Err("no skill — it calls `scan_nodes` for the `Decision` type directly")),
    ("debt", Err("no skill — it calls the `loop_status` tool directly")),
];

/// The sentence that turns a refusal into an instruction, when the name asked
/// for is a slash command rather than a skill.
pub fn alias_hint(name: &str) -> Option<String> {
    COMMAND_ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(alias, target)| match target {
            Ok(skill) => format!("'{alias}' is the slash command; it is served as '{skill}'."),
            Err(why) => format!("'{alias}' is a slash command with {why}."),
        })
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

#[cfg(test)]
mod alias_tests {
    use super::{COMMAND_ALIASES, alias_hint, find};

    /// Every alias that claims to name a skill must name one that EXISTS.
    ///
    /// This is the whole failure being fixed, inverted: the commands advertised
    /// eight names the server did not carry, and nothing noticed. A refusal that
    /// points at a second name that is also wrong is worse than the original.
    #[test]
    fn every_alias_that_names_a_skill_names_a_real_one() {
        for (alias, target) in COMMAND_ALIASES {
            if let Ok(skill) = target {
                assert!(
                    find(skill).is_some(),
                    "alias '{alias}' maps to '{skill}', which this server does not carry"
                );
            }
        }
    }

    /// An alias must not shadow a real skill name — if it did, `find` would
    /// answer first and the hint would never be reached.
    #[test]
    fn no_alias_is_itself_a_skill_name() {
        for (alias, _) in COMMAND_ALIASES {
            assert!(
                find(alias).is_none(),
                "'{alias}' is served as a skill, so it should not be in the alias table"
            );
        }
    }

    /// The two commands that resolve to no skill must say so, and must NOT be
    /// answered with a skill name. Sending someone to a skill that cannot help
    /// is the failure mode this table exists to avoid.
    #[test]
    fn a_command_with_no_skill_names_the_tool_instead() {
        let debt = alias_hint("debt").expect("debt is a known command");
        assert!(debt.contains("loop_status"), "{debt}");
        assert!(!debt.contains("served as"), "{debt}");

        let decisions = alias_hint("decisions").expect("decisions is a known command");
        assert!(decisions.contains("scan_nodes"), "{decisions}");
    }

    /// A name nobody advertised gets no invented mapping.
    #[test]
    fn an_unknown_name_gets_no_hint() {
        assert!(alias_hint("definitely-not-a-command").is_none());
    }
}
