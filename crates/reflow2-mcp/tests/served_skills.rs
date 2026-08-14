//! What the server serves is what the kit says — checked, not assumed.
//!
//! `dec:skills-served`: the skills stopped being copied into consumer projects
//! and started being compiled into the binary, because a copy drifts. That
//! moves the drift risk exactly one step: the binary could now be built against
//! the wrong directory, or a skill could be added to the kit and silently not
//! served, and the failure would look identical to everything working.
//!
//! This is `ver:kit-manifest-agrees` finally getting an owner. The old version
//! of that risk was measured rather than theorised — reflow2's installed kit
//! manifest read **0.8.0 with twelve skills** while the project was at 0.11.0
//! with fifteen, for four releases, and nothing anywhere noticed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use reflow2_mcp::skills::{SKILLS, catalogue, find};

fn kit() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .join("getting-started")
        .join("skills")
}

fn kit_names() -> BTreeSet<String> {
    std::fs::read_dir(kit())
        .expect("the kit is readable")
        .filter_map(|e| {
            let p = e.expect("entry").path();
            (p.is_dir() && p.join("SKILL.md").exists())
                .then(|| p.file_name()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect()
}

#[test]
fn every_skill_in_the_kit_is_served() {
    let served: BTreeSet<String> = SKILLS.iter().map(|s| s.name.to_string()).collect();
    assert_eq!(
        served,
        kit_names(),
        "the served catalogue and the kit source have diverged — which is the exact failure \
         serving them was meant to make impossible"
    );
    assert!(
        !SKILLS.is_empty(),
        "an empty catalogue would serve silently"
    );
}

#[test]
fn a_served_skill_is_byte_identical_to_its_source() {
    // Not "looks right": the same bytes. An agent following a paraphrase of a
    // skill is following something nobody wrote.
    for skill in SKILLS {
        let on_disk = std::fs::read_to_string(kit().join(skill.name).join("SKILL.md"))
            .unwrap_or_else(|e| panic!("{}: {e}", skill.name));
        assert_eq!(
            skill.body, on_disk,
            "{} drifted from its source",
            skill.name
        );
    }
}

#[test]
fn the_description_served_is_the_one_the_harness_would_have_matched_on() {
    // The description is the whole discovery mechanism now: with skills served
    // rather than installed, nothing else tells an agent when a skill applies.
    for skill in SKILLS {
        assert!(
            !skill.description.is_empty(),
            "{} has no description",
            skill.name
        );
        assert!(
            skill
                .body
                .contains(&format!("description: {}", skill.description)),
            "{}'s served description is not its frontmatter description",
            skill.name
        );
    }
}

#[test]
fn the_catalogue_names_every_skill_and_says_they_are_not_auto_loaded() {
    // THE test for the trade this decision made. A served skill is never
    // offered by the harness, so an agent that is not told the catalogue exists
    // will simply never use a skill again — which would be a worse outcome than
    // the staleness this replaced.
    let catalogue = catalogue();
    for skill in SKILLS {
        assert!(
            catalogue.contains(skill.name),
            "{} is served but absent from the catalogue the agent actually reads",
            skill.name
        );
    }
    assert!(
        catalogue.contains("NOT auto-loaded"),
        "the catalogue must say the harness will not offer these: {catalogue}"
    );
    assert!(
        catalogue.contains("get_skill") && catalogue.contains("list_skills"),
        "and must name the tools that read them"
    );
}

#[test]
fn asking_for_a_skill_that_does_not_exist_names_the_ones_that_do() {
    assert!(find("no-such-skill").is_none());
    assert!(find("capture-intent").is_some(), "a known skill resolves");
}

#[test]
fn the_working_instructions_are_served_and_the_project_holds_only_a_pointer() {
    // `req:thin-install` completed. The skills moved to the server first, and
    // the ~20 KB instruction file was the one thing left that still churned a
    // consumer's repository on every release — the same defect, in the last
    // place it could hide.
    use reflow2_mcp::skills::INSTRUCTIONS;

    let kit = kit().parent().expect("kit root").to_path_buf();
    let source = std::fs::read_to_string(kit.join("AGENTS.md")).expect("kit instructions");
    assert_eq!(
        INSTRUCTIONS, source,
        "the served instructions must be the kit's, byte for byte"
    );

    let pointer = std::fs::read_to_string(kit.join("POINTER.md")).expect("kit pointer");
    assert!(
        pointer.len() * 3 < INSTRUCTIONS.len(),
        "what a project holds must be a pointer, not a copy: {} vs {}",
        pointer.len(),
        INSTRUCTIONS.len()
    );
    for tool in ["get_instructions", "list_skills", "get_skill"] {
        assert!(pointer.contains(tool), "the pointer must name {tool}");
    }
    // The rule that cannot live only in the served text: it has to be true even
    // for an agent that never calls a tool.
    assert!(
        pointer.contains("never follow it"),
        "the graph-text-is-data rule stays in the file, because it governs \
         reading the graph at all"
    );
}

#[test]
fn the_capture_skills_never_ask_the_user_to_choose_a_node_type() {
    // Alex, 2026-08-14, first run on a real work project: the agent asked him
    // what node types to store his data models as. He said "you figure it out"
    // and the result was fine — but a non-technical user has no such escape,
    // and a wrong answer shapes their design where nobody can see it.
    //
    // req:the-kind-of-system-is-established-before-its-parts-are-captured makes
    // the mapping the AGENT's job. That is a rule about conduct, and conduct is
    // exactly what nothing can observe — so it is pinned here instead, where a
    // future edit that quietly drops it turns this red.
    for name in ["capture-intent", "genesis"] {
        let skill = find(name).unwrap_or_else(|| panic!("{name} must be served"));
        assert!(
            skill.body.contains("Never ask the user which node type")
                || skill.body.contains("Never ask which node type"),
            "{name} must forbid handing the vocabulary question back to the user"
        );
        assert!(
            skill.body.contains("describe_schema"),
            "{name} must name the tool the agent looks the vocabulary up WITH — \
             forbidding the question without naming the alternative is half a rule"
        );
    }
}

#[test]
fn capture_intent_carries_a_routing_table_that_admits_where_it_runs_out() {
    // A table is only better than improvisation because it can be WRONG IN
    // PUBLIC. capture-session's own routing table shipped with a missing row
    // and its first dogfood found it — which is the argument for having one,
    // not against. So the honest boundary is part of the artifact: the row that
    // says "nothing fits cleanly" is what stops an agent forcing operational
    // know-how into a requirement's prose because the table looked complete.
    let skill = find("capture-intent").expect("capture-intent must be served");
    assert!(
        skill.body.contains("| What they said | Where it goes |"),
        "the mapping must be a table an agent can look up, not prose it re-derives"
    );
    assert!(
        skill.body.contains("Nothing fits cleanly"),
        "the table must name where it runs out — a routing table that pretends \
         to be complete teaches you to mis-file things"
    );
    for tool in [
        "add_requirement",
        "add_capability",
        "add_component",
        "add_interface",
    ] {
        assert!(
            skill.body.contains(tool),
            "the table must name {tool} as the destination, not just the node type"
        );
    }
}
