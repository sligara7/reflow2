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
    (
        "decisions",
        Err("no skill — it calls `scan_nodes` for the `Decision` type directly"),
    ),
    (
        "debt",
        Err("no skill — it calls the `loop_status` tool directly"),
    ),
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
