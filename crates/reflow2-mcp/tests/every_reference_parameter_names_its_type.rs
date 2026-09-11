//! Every REQUIRED parameter that points at another node says which TYPE it
//! points at — in its name, in its description, or by naming the sibling
//! parameter that supplies the type.
//!
//! # The class, not the instance
//!
//! `violates_rule`, `complies_with` and `set_violation_status` each take a
//! required `rule_id` declared as a bare `{"type": "string"}` with NO
//! description. All three resolve it to an **EnvironmentRule**. This design
//! holds **27 DesignRules and 0 EnvironmentRules**, so the 27 rules the project
//! governs itself by cannot be recorded as violated or complied with by
//! anything — and nothing on the served surface says so. The generic escape
//! hatch is closed too: `create_edge` refuses the pair outright with *"No edge
//! type names both Component and DesignRule"*.
//!
//! That was found by a sweep of all 180 tools on 2026-09-11
//! (`fact:the-27-design-rules-are-unreachable-by-the-violation-vocabulary-because-it-resolves-to-environment-rule`),
//! and it is not one tool's oversight. **This test failed on 66 required
//! reference parameters across 49 tools** the first time it was run — a hand
//! estimate beforehand said 55, and the difference is exactly the judgement a
//! standing check stops being re-made. `withdraw_question` taking `gap_id`
//! rather than `question_id` is the second confirmed casualty: the sweeping
//! agent guessed `question_id` and was refused.
//!
//! ⭐ AND IT CAUGHT ITS AUTHOR MID-FIX. The first pass at the descriptions gave
//! `set_violation_status.element_id` the line *"its type is `element_type`"* —
//! copied from its siblings `violates_rule` and `complies_with`, which do carry
//! that parameter. `ViolationStatusReq` does not. The check failed because the
//! named sibling did not exist, which is the whole argument for deriving the
//! obligation rather than trusting a careful author.
//!
//! # ⭐ WHY THE MEMBERSHIP IS DERIVED AND NOT HAND-LISTED
//!
//! The sibling guard `every_declared_enum_is_wired_or_exempt` keeps a
//! hand-written `WIRED` table, and its own header explains why: a name sweep
//! there would wire `add_component`'s `level` to a fixed enum and publish a lie.
//! **Here the opposite is true.** Whether a parameter names its type is
//! decidable from the schema: strip `_id`/`_ids` and ask whether exactly one
//! declared node type matches. So the class computes its own membership, and a
//! 181st tool joins it the moment it is served.
//!
//! That is what makes `rule_id` fall out mechanically rather than by opinion:
//! **two** declared types end in `rule` (`DesignRule`, `EnvironmentRule`), so
//! the name cannot resolve, and the check demands the description say which.
//! `capability_id` resolves uniquely and is asked for nothing.
//!
//! # What this does NOT claim
//!
//! It checks that a type is STATED, never that the stated type is the one the
//! handler actually resolves. A description naming the wrong type passes here
//! and fails in front of a user. Presence is close to correctness for this
//! obligation — the type is a fact, not a judgement, so it cannot be satisfied
//! with plausible prose the way "honest limits" can — but it is not identical
//! to it.
//!
//! ⚠️ AND THE MATCH IS A SUBSTRING MATCH, SO IT CAN PASS BY ACCIDENT.
//! `withdraw_question.gap_id` satisfies this test because its description
//! contains the word "question" and `Question` is a declared node type — the
//! prose happens to be genuinely informative, so the pass is right in
//! substance and lucky in mechanism. A stricter check would demand a
//! structured annotation rather than prose; that is a bigger change than the
//! one this class needed, and it is recorded here rather than pretended away.
//!
//! Optional parameters are out of scope. The cost lands on a caller who must
//! supply a value to make the call at all, and widening this to every optional
//! reference would add noise without adding a refusal anybody meets.

use reflow2_mcp::service::ReflowService;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Required reference parameters that legitimately cannot name one type, with
/// the reason. An entry here is a claim that the ambiguity is the DESIGN.
///
/// Keyed `(tool, parameter)`. Kept deliberately short: every entry is a place a
/// caller must work out the type from somewhere else.
const EXEMPT: &[(&str, &str, &str)] = &[
    // ─── NOT NODE REFERENCES AT ALL. These end in `_id` and point at a
    // detector's finding key, not at anything in the graph. Naming a node type
    // would be a lie; the descriptions say so out loud instead.
    (
        "acknowledge_gap",
        "gap_id",
        "Not a node id — the gap key `detect_gaps` reports. The acknowledgement is \
         keyed on the gap's SHAPE so it expires when the shape changes.",
    ),
    (
        "withdraw_gap_acknowledgement",
        "gap_id",
        "Not a node id — the gap key from `detect_gaps` / `reviewed_gaps`.",
    ),
    (
        "acknowledge_defect",
        "defect_id",
        "Not a node id — the `heal:…` key `detect_defects` reports.",
    ),
    (
        "withdraw_defect_acknowledgement",
        "defect_id",
        "Not a node id — the `heal:…` key from `detect_defects` / `reviewed_defects`.",
    ),
    // ─── GENUINELY ANY NODE TYPE, and that is the design rather than an
    // oversight. Each walks or addresses the graph without caring what it finds.
    (
        "claim_region",
        "seed_id",
        "Any node type: a region is walked outward from whatever seed is given, and \
         restricting the seed would restrict what a person may claim.",
    ),
    (
        "release_claim",
        "seed_id",
        "Any node type — it must match the seed of the claim being released.",
    ),
    (
        "propagate_from",
        "seed_ids",
        "Any node type: a blast radius starts wherever the caller says it starts.",
    ),
    (
        "delete_edge",
        "from_id",
        "Any node type — an edge may join any pair the schema allows, and the \
         endpoints are resolved from the ids.",
    ),
    (
        "delete_edge",
        "to_id",
        "Any node type — see `delete_edge.from_id`.",
    ),
    (
        "set_violation_status",
        "element_id",
        "Any node type, and deliberately carries no `element_type`: the \
         `VIOLATES_RULE` edge already exists and is what is being triaged, so the \
         type is whatever that edge's source is.",
    ),
];

/// `CamelCase` -> `snake_case`, so a declared node type can be compared with a
/// parameter stem.
fn snake(ty: &str) -> String {
    let mut out = String::new();
    for (i, c) in ty.char_indices() {
        if c.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn tools() -> Vec<rmcp::model::Tool> {
    let mut all = ReflowService::capture_router().list_all();
    for r in [
        ReflowService::assure_router(),
        ReflowService::exchange_router(),
        ReflowService::temporal_tools_router(),
        ReflowService::ask_router(),
        ReflowService::built_router(),
        ReflowService::coherence_router(),
        ReflowService::ingest_tools_router(),
        ReflowService::operate_tools_router(),
        ReflowService::query_router(),
        ReflowService::claims_tools_router(),
        ReflowService::skills_router(),
    ] {
        all.extend(r.list_all());
    }
    all
}

/// The declared node types, as `snake_case`.
fn declared_types() -> Vec<String> {
    let schema = reflow2_core::schema::load_schema().expect("schema loads");
    schema.node_types.keys().map(|t| snake(t)).collect()
}

/// Does this parameter's NAME resolve to exactly one declared node type?
///
/// `capability_id` -> `capability` -> `Capability`, uniquely: yes.
/// `epoch_id` -> `epoch` -> only `design_epoch` ends with it, uniquely: yes.
/// `rule_id` -> `rule` -> `design_rule` AND `environment_rule`: NO.
fn name_resolves(param: &str, types: &[String]) -> bool {
    let Some(stem) = param
        .strip_suffix("_ids")
        .or_else(|| param.strip_suffix("_id"))
    else {
        return false;
    };
    if stem.is_empty() {
        return false;
    }
    let hits = types
        .iter()
        .filter(|t| *t == stem || t.ends_with(&format!("_{stem}")))
        .count();
    hits == 1
}

/// Every required reference parameter whose name does not resolve, paired with
/// the sibling `*_type` parameters its tool offers.
fn candidates(tools: &[rmcp::model::Tool]) -> BTreeMap<(String, String), (String, Vec<String>)> {
    let types = declared_types();
    let mut out = BTreeMap::new();
    for t in tools {
        let schema: Value = serde_json::to_value(&t.input_schema).expect("schema");
        let props = schema["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let required: Vec<String> = schema["required"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let siblings: Vec<String> = props
            .keys()
            .filter(|k| k.ends_with("_type"))
            .cloned()
            .collect();
        for p in required {
            // The node the tool itself addresses: its type is the tool's own,
            // and `add_capability {id}` needs no help saying so.
            if p == "id" {
                continue;
            }
            if !(p.ends_with("_id") || p.ends_with("_ids")) {
                continue;
            }
            if name_resolves(&p, &types) {
                continue;
            }
            let desc = props[&p]["description"].as_str().unwrap_or("").to_string();
            out.insert((t.name.to_string(), p), (desc, siblings.clone()));
        }
    }
    out
}

/// THE CLASS CONTRACT. A required reference whose NAME does not name its type
/// must say the type in its description, or name the sibling parameter that
/// carries it.
#[test]
fn every_required_reference_says_what_type_it_points_at() {
    let tools = tools();
    let types = declared_types();
    let exempt: BTreeSet<(&str, &str)> = EXEMPT.iter().map(|(t, p, _)| (*t, *p)).collect();

    let mut silent: Vec<String> = Vec::new();
    for ((tool, param), (desc, siblings)) in candidates(&tools) {
        if exempt.contains(&(tool.as_str(), param.as_str())) {
            continue;
        }
        let lower = desc.to_lowercase();
        // Names a declared node type outright...
        let names_type = types.iter().any(|t| {
            // compare against both `design_rule` and `DesignRule` spellings
            lower.contains(t) || lower.contains(&t.replace('_', ""))
        });
        // ...or points at the sibling that supplies it.
        let names_sibling = siblings.iter().any(|s| desc.contains(s.as_str()));
        if !names_type && !names_sibling {
            silent.push(format!(
                "{tool}.{param}{}",
                if desc.is_empty() {
                    " (no description at all)"
                } else {
                    ""
                }
            ));
        }
    }

    assert!(
        silent.is_empty(),
        "{} required reference parameter(s) do not say what node type they point at:\n  {}\n\n\
         Each one takes an id and the caller cannot tell from the served surface what kind of \
         node to pass. Give the parameter a description naming the node type(s) it resolves to, \
         or naming the sibling `*_type` parameter that supplies it — or add it to EXEMPT with \
         the reason the ambiguity is deliberate.\n\n\
         THE CLASS THIS GUARDS: `rule_id` on violates_rule/complies_with/set_violation_status \
         resolves to EnvironmentRule while this design holds 27 DesignRules and 0 \
         EnvironmentRules, so those 27 rules cannot be recorded as violated by anything, and \
         nothing on the surface said so.",
        silent.len(),
        silent.join("\n  ")
    );
}

/// The other direction: an EXEMPT entry naming a parameter that is no longer a
/// candidate is a stale excuse, and would quietly shrink what this test sees.
#[test]
fn every_exemption_still_names_a_real_candidate() {
    let tools = tools();
    let live = candidates(&tools);
    let stale: Vec<String> = EXEMPT
        .iter()
        .filter(|(t, p, _)| !live.contains_key(&(t.to_string(), p.to_string())))
        .map(|(t, p, _)| format!("{t}.{p}"))
        .collect();
    assert!(
        stale.is_empty(),
        "EXEMPT names {} parameter(s) that are no longer required references whose name fails to \
         resolve — the tool changed, or the parameter was renamed, or a node type was added that \
         now makes the name unambiguous. Remove the stale entry: {}",
        stale.len(),
        stale.join(", ")
    );
}
