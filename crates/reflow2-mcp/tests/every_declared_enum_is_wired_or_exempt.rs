//! Every enum the schema declares either publishes its values on the served
//! surface, or is named here with a reason why it must not.
//!
//! # The class, not the instance
//!
//! hxm_program F-19 reported one field: `Requirement.concern` taught its eleven
//! values by REFUSING a wrong one, while its neighbour `priority` on the same
//! request struct published them. That was fixed on its own. This is the class
//! behind it — wiring is one macro line plus one attribute per field, done by
//! hand, with nothing measuring whether it was done.
//!
//! ⭐ THE COUNT WAS GOT WRONG THREE TIMES BEFORE THIS TEST EXISTED, which is the
//! argument for the test rather than a footnote to it. A name-matching sweep of
//! `service.rs` said ~40 (it matched any same-NAMED field on any struct — two
//! different `concern` fields counted off one hit). A per-struct sweep with an
//! incomplete type map said ~3. The checked answer is 31, and the difference
//! between those numbers is exactly the judgement a human had to make each time.
//! A standing check is what stops that judgement being re-made from scratch.
//!
//! # ⚠️ WHY A NAME SWEEP IS WRONG AND THIS LIST IS HAND-MADE
//!
//! Five request fields share a name with a declared enum and MUST NOT be wired
//! to it. They are in `EXEMPT` below with their reasons. The sharpest is
//! `add_component`'s `level`: the decomposition ladder is DELIBERATELY OPEN and
//! per-project (`Project.decomposition_levels`), settled in
//! `dec:the-decomposition-ladder-is-open-not-a-fixed-enum`. Pinning it to a
//! fixed enum would publish a lie AND contradict an accepted decision. A sweep
//! that wired by name would have done exactly that.
//!
//! Those five are deliberately NOT in `EXEMPT`: that list is keyed on DECLARED
//! enums, and four of them are not declared enums at all — they only collide
//! with one on another type. The staleness half of the second test catches an
//! entry naming something the schema does not declare, which is exactly how
//! this was found.
//!
//! # What this guards
//!
//! 1. Every wired field publishes the SCHEMA'S OWN values — not a hand-copied
//!    list that can drift from what the server actually enforces.
//! 2. Every declared enum is accounted for: wired, or exempt with a reason. A
//!    NEW enum property added to the schema fails this test until somebody
//!    decides which it is. That is the half that makes it a class guard rather
//!    than a snapshot.
//!
//! # ⭐ THE EXEMPT LIST IS ALSO A FINDING, AND WORTH READING AS ONE
//!
//! 31 fields were wired here. 33 more are exempt, and only five of those are
//! exempt because they mean something else. THE OTHER 28 ARE EXEMPT BECAUSE NO
//! TYPED CALL CAN WRITE THEM AT ALL — 17 edge properties reachable only through
//! `create_edge`'s free-JSON `props` map, and 11 node properties reachable only
//! through `create_node`'s. They are declared, validated, and unreachable from
//! the path every skill points at, which is the class
//! `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` records.
//! Publishing their values would not help, because nothing can pass them;
//! giving them parameters is a larger question than this test settles. The list
//! below is where that backlog is now visible instead of implied.

use reflow2_mcp::service::ReflowService;
use serde_json::Value;
use std::collections::BTreeSet;

/// `(tool, field, type, property)` — the field publishes that type's enum.
const WIRED: &[(&str, &str, &str, &str)] = &[
    ("add_capability", "tier", "Capability", "tier"),
    (
        "add_change_event",
        "change_type",
        "ChangeEvent",
        "change_type",
    ),
    ("add_change_event", "subject", "ChangeEvent", "subject"),
    ("add_component", "tier", "Component", "tier"),
    ("add_constraint", "concern", "Constraint", "concern"),
    ("add_constraint", "direction", "Constraint", "direction"),
    ("add_constraint", "priority", "Constraint", "priority"),
    ("add_decision", "kind", "Decision", "kind"),
    ("add_flow", "tier", "Flow", "tier"),
    ("add_readiness", "kind", "ReadinessAssessment", "kind"),
    (
        "dimension_drift",
        "dimension",
        "DimensionAssessment",
        "dimension",
    ),
    ("forecast_readiness", "kind", "ReadinessAssessment", "kind"),
    ("gate_on", "kind", "GATED_ON", "kind"),
    ("genesis", "mode", "Project", "mode"),
    ("governed_by", "ruling", "GOVERNED_BY", "ruling"),
    ("ingest_corpus_step", "provenance", "Fragment", "provenance"),
    ("link_artifact", "provenance", "Fragment", "provenance"),
    ("record_change", "change_type", "ChangeEvent", "change_type"),
    ("record_change", "subject", "ChangeEvent", "subject"),
    ("record_finding", "basis", "TemporalFact", "basis"),
    ("schedule_for", "modality", "SCHEDULED_FOR", "modality"),
    ("set_artifact_intent", "audience", "Artifact", "audience"),
    (
        "set_artifact_intent",
        "granularity",
        "Artifact",
        "granularity",
    ),
    (
        "set_artifact_intent",
        "volatility",
        "Artifact",
        "volatility",
    ),
    (
        "set_capability_delivery",
        "delivery",
        "Capability",
        "delivery",
    ),
    ("set_epoch_status", "status", "DesignEpoch", "status"),
    (
        "set_interface_designation",
        "designation",
        "Interface",
        "designation",
    ),
    ("set_project_mode", "mode", "Project", "mode"),
    (
        "set_requirement_designation",
        "designation",
        "Requirement",
        "designation",
    ),
    ("set_verification_kind", "kind", "Verification", "kind"),
];

/// The same, for a field nested inside a list parameter's item schema:
/// `(tool, $defs entry, field, type, property)`.
const WIRED_NESTED: &[(&str, &str, &str, &str, &str)] = &[(
    "set_artifact_checksums",
    "ChecksumAcceptReq",
    "change_type",
    "ChangeEvent",
    "change_type",
)];

/// `(type, property, why it must not publish that enum)`.
///
/// Each of these shares a NAME with a declared enum and means something else.
const EXEMPT: &[(&str, &str, &str)] = &[
    (
        "SATISFIES",
        "coverage",
        "Written through create_edge's generic `props` map; there is no typed parameter to hang \
         a schema on. The edge-property family is a separate and larger question.",
    ),
    (
        "ALLOCATED_TO",
        "weight_basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "ANNOTATES",
        "note_kind",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "Artifact",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "CAUSES",
        "basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "CAUSES",
        "validation_status",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "CONSUMES",
        "weight_basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "CONTRADICTS",
        "alignment",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "Capability",
        "provenance",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Component",
        "kind",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Component",
        "provenance",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Component",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "DEPENDS_ON",
        "dependency_type",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "DEPENDS_ON",
        "weight_basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "DUPLICATES",
        "basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "DimensionObservation",
        "dimension",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "DriftEvent",
        "drift_type",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "DriftEvent",
        "severity",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Fragment",
        "fragment_type",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Fragment",
        "phase",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Fragment",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "IMPLEMENTS",
        "covers",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "Interface",
        "provenance",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "PART_OF_FLOW",
        "weight_basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "PRODUCES",
        "outcome",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "PROVIDES",
        "weight_basis",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "Project",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Question",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "RISKS",
        "severity",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "Release",
        "status",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "Requirement",
        "kind",
        "Node property no constructor accepts: the only way to write it is create_node's \
         generic `props` map. Declared and unwritable from the typed path \u{2014} the class \
         `fact:a-third-of-declared-properties-name-nothing-any-tool-accepts` names. Wiring \
         would need a parameter first, which is a bigger question than publishing a set.",
    ),
    (
        "SUPPLEMENTS",
        "grounding",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "VERIFIES",
        "coverage",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
    (
        "YIELDED",
        "action",
        "Edge property with no typed parameter anywhere on the surface: the only way to \
         write it is create_edge's generic `props` map, which takes free JSON and has no \
         schema to publish. Wiring would need a typed helper first.",
    ),
];

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

fn schema_of(tools: &[rmcp::model::Tool], tool: &str) -> Value {
    let t = tools
        .iter()
        .find(|t| t.name == tool)
        .unwrap_or_else(|| panic!("tool `{tool}` is not served by any router in this test"));
    serde_json::to_value(&t.input_schema).expect("schema")
}

fn published(v: &Value) -> Vec<String> {
    v["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("no enum published: {v}"))
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

/// Every wired field publishes the schema's own values.
#[test]
fn every_wired_field_publishes_the_schemas_own_values() {
    let tools = tools();
    for (tool, field, ty, prop) in WIRED {
        let s = schema_of(&tools, tool);
        let p = &s["properties"][field];
        assert!(
            !p.is_null(),
            "{tool} has no property `{field}` — the table is stale"
        );
        let want = reflow2_core::schema::enum_values(ty, prop)
            .unwrap_or_else(|| panic!("{ty}.{prop} is not a declared enum — the table is wrong"));
        assert_eq!(
            published(p),
            want,
            "{tool}.{field} must publish {ty}.{prop}'s values"
        );
    }
    for (tool, def, field, ty, prop) in WIRED_NESTED {
        let s = schema_of(&tools, tool);
        let p = &s["$defs"][def]["properties"][field];
        assert!(!p.is_null(), "{tool} $defs.{def} has no `{field}`");
        let want = reflow2_core::schema::enum_values(ty, prop).expect("declared enum");
        assert_eq!(
            published(p),
            want,
            "{tool} $defs.{def}.{field} must publish {ty}.{prop}'s values"
        );
    }
}

/// ⭐ THE CLASS GUARD. Every enum the schema declares is either wired above or
/// exempt with a reason — so a NEW declared enum fails this test until somebody
/// decides which it is, instead of shipping unreachable and being found by a
/// user hitting a refusal.
#[test]
fn every_declared_enum_is_accounted_for() {
    let schema = reflow2_core::schema::load_schema().expect("schema loads");
    let mut declared: BTreeSet<(String, String)> = BTreeSet::new();
    for (ty, def) in schema.node_types.iter().map(|(t, d)| (t.clone(), d)) {
        for (p, pd) in &def.properties {
            if pd.values.is_some() {
                declared.insert((ty.clone(), p.clone()));
            }
        }
    }
    for (ty, def) in schema.edge_types.iter().map(|(t, d)| (t.clone(), d)) {
        for (p, pd) in &def.properties {
            if pd.values.is_some() {
                declared.insert((ty.clone(), p.clone()));
            }
        }
    }

    // Wired anywhere on the surface, read from the source of the wiring itself
    // rather than re-listed here — a second list would be a second thing to
    // keep true.
    // Read the WHOLE file, not line by line: `cargo fmt` wraps a long
    // invocation across four lines, so a per-line parse silently misses every
    // wrapped one. That bug made this test's first run report 41 unwired enums
    // of which a dozen were wired — a guard that is wrong in the SAFE direction
    // is still wrong, and would have had somebody write exemptions for fields
    // that did not need them.
    let src = include_str!("../src/enum_schema.rs");
    let mut wired: BTreeSet<(String, String)> = BTreeSet::new();
    for chunk in src.split("schema_enum!(").skip(1) {
        let head = &chunk[..chunk.find(");").unwrap_or(chunk.len())];
        let parts: Vec<&str> = head.split('"').collect();
        if parts.len() >= 4 {
            wired.insert((parts[1].to_string(), parts[3].to_string()));
        }
    }
    let exempt: BTreeSet<(String, String)> = EXEMPT
        .iter()
        .map(|(t, p, _)| (t.to_string(), p.to_string()))
        .collect();

    let unaccounted: Vec<String> = declared
        .difference(&wired)
        .filter(|k| !exempt.contains(*k))
        .map(|(t, p)| format!("{t}.{p}"))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "{} declared enum(s) are neither wired nor exempt:\n  {}\n\nWire each one \
         (schema_enum! plus a #[schemars(schema_with = ...)] on its request field), or add it to \
         EXEMPT with the reason it must not publish that set. A field that shares a NAME with an \
         enum is not necessarily that enum — see add_component's `level`.",
        unaccounted.len(),
        unaccounted.join("\n  ")
    );

    // And the other direction: an EXEMPT entry naming something the schema no
    // longer declares is a stale excuse, which would quietly shrink the set
    // this test can see.
    let stale: Vec<String> = exempt
        .difference(&declared)
        .map(|(t, p)| format!("{t}.{p}"))
        .collect();
    assert!(
        stale.is_empty(),
        "EXEMPT names {} property(ies) the schema no longer declares as an enum: {}",
        stale.len(),
        stale.join(", ")
    );
}
