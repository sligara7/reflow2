//! The schema serves the rules that DISTINGUISH the types, not only the types.
//!
//! FLO2 F24, with a measured consequence on their live site on 2026-09-19:
//! "the water can never drop below 68 degrees" was recorded as a REQUIREMENT.
//! That is a numeric prohibition, which reflow2's own routing table sends to a
//! Constraint — and this design's own capture-intent skill says conflating the
//! two leaves prohibitions reporting unsatisfied forever.
//!
//! WHY THE SCHEMA AND NOT THE SKILL. `describe_schema` answers with node types,
//! their properties, their enums and which edges may join two types. The rules
//! that actually decide an extraction — the cue phrases in a user's own words,
//! the neighbouring type each is confused with, and a counter-example — existed
//! only as an eleven-row markdown table inside capture-intent's SKILL.md,
//! addressed to an agent reading a skill. A consumer generating an extraction
//! prompt FROM THE SCHEMA, which is what `dec:contract-from-toolsnaps` and the
//! black-box rule tell it to do, got the type names and none of what separates
//! them.
//!
//! ⭐ AND IT IS LOAD-BEARING NOW RATHER THAN A NICETY. flo2's own client is
//! retired, so a connector's whole product experience is tool names, tool
//! descriptions and tool results — there is no system prompt left on the
//! consumer's side for these rules to live in
//! (`fact:connector-use-is-expected-to-be-reflow2s-primary-use-for-most-people`).
//!
//! OBSERVED FAILING before the schema carried the rules: every assertion below
//! failed, because `discrimination` was not a field any type had.

use reflow2_core::DesignGraph;

/// The types the capture-intent routing table actually routes to. A type here
/// with no discrimination block is a row of that table a consumer cannot see.
const ROUTED: [&str; 8] = [
    "Requirement",
    "Capability",
    "Component",
    "Interface",
    "Flow",
    "Constraint",
    "Artifact",
    "DesignRule",
];

fn graph() -> DesignGraph {
    DesignGraph::open_in_memory().expect("in-memory graph")
}

#[test]
fn every_routed_type_serves_its_discrimination_rules() {
    let g = graph();
    for t in ROUTED {
        let detail = g
            .describe_node_type(t)
            .unwrap_or_else(|e| panic!("{t}: {e}"));
        let v = serde_json::to_value(&detail).expect("serializable");
        let d = &v["discrimination"];
        assert!(
            !d.is_null(),
            "{t} serves no discrimination rules. The capture-intent routing table routes to it, \
             so a consumer building an extraction prompt from the schema gets the type name and \
             nothing that separates it from its neighbour — which is how a numeric prohibition \
             was filed as a requirement on a live site (flo2 F24)"
        );
        assert!(
            d["cues"].as_array().is_some_and(|c| !c.is_empty()),
            "{t} declares no cue phrases — the words a person actually says that route here"
        );
        assert!(
            d["confused_with"].as_array().is_some_and(|c| !c.is_empty()),
            "{t} names no type it is confused with. The near-miss is the half that decides a \
             hard case; without it the rules are a glossary rather than a discriminator"
        );
    }
}

/// The case that cost a real misclassification, asserted by its own content
/// rather than by the presence of a field: a numeric prohibition must route to
/// Constraint, and Constraint must say it is confused with Requirement.
#[test]
fn a_numeric_prohibition_is_discriminated_from_a_requirement() {
    let g = graph();

    let constraint = serde_json::to_value(g.describe_node_type("Constraint").expect("Constraint"))
        .expect("serializable");
    let cues = constraint["discrimination"]["cues"]
        .as_array()
        .expect("Constraint cues")
        .iter()
        .filter_map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
        .to_lowercase();
    assert!(
        cues.contains("never") || cues.contains("not allowed"),
        "Constraint's cues must carry a PROHIBITION phrasing — the row reading \"we must never…, \
         it is not allowed to…\" is the one flo2's sentence should have matched. Got: {cues}"
    );

    let confused: Vec<String> = constraint["discrimination"]["confused_with"]
        .as_array()
        .expect("Constraint confused_with")
        .iter()
        .filter_map(|c| c["type"].as_str().map(str::to_string))
        .collect();
    assert!(
        confused.iter().any(|t| t == "Requirement"),
        "Constraint must name Requirement as the type it is confused with: that is the exact \
         confusion that filed \"the water can never drop below 68 degrees\" as a requirement. \
         Got: {confused:?}"
    );

    // And the reverse direction, because a discriminator that only points one
    // way leaves the other type's reader with no warning at all.
    let requirement =
        serde_json::to_value(g.describe_node_type("Requirement").expect("Requirement"))
            .expect("serializable");
    let back: Vec<String> = requirement["discrimination"]["confused_with"]
        .as_array()
        .expect("Requirement confused_with")
        .iter()
        .filter_map(|c| c["type"].as_str().map(str::to_string))
        .collect();
    assert!(
        back.iter().any(|t| t == "Constraint"),
        "Requirement must name Constraint too — the confusion is symmetric and a reader arriving \
         from either side needs the warning. Got: {back:?}"
    );
}
