//! Enum request fields publish their legal values in the tool schema.
//!
//! Every enum-valued field on the tool surface was typed `Option<String>`, so
//! the JSON schema a harness shows an agent said "string" and the legal set
//! lived only in the handler — learned one refusal at a time (~24 in one
//! dev_storyflow session; Alex's report; the qs report). The refusal already
//! names the legal values (req:a-refusal-names-what-would-have-worked); this is
//! the half that shows them BEFORE the first call.
//!
//! ONE SOURCE OF TRUTH: the values come from the compiled-in schema at
//! tool-listing time (`reflow2_core::schema::enum_values`), so nothing here can
//! drift from what the validator enforces. The four sets that live only in code
//! (`review_relations`' relation, `documents`' doc_kind, `report_manual_work`'s
//! diagnosis, `propose_heal`'s strategy) reuse the same const the handler
//! checks against, for the same reason.
//!
//! The field stays a `String` for serde, so deserialization and the refusal path
//! are unchanged; only what is PUBLISHED changes. `tools/toolsnap.py` carries the
//! guard: a property whose description hand-lists values must publish an enum
//! that contains every one of them.
//! fact:defect-the-schema-discovery-tax-enum-fields-are-strings-so-the-published-schema-cannot-name-their-values

use schemars::{Schema, SchemaGenerator, json_schema};
use serde_json::Value;

fn enum_of(values: impl IntoIterator<Item = String>, nullable: bool) -> Schema {
    let mut e: Vec<Value> = values.into_iter().map(Value::String).collect();
    if nullable {
        e.push(Value::Null);
        json_schema!({ "type": ["string", "null"], "enum": e })
    } else {
        json_schema!({ "type": "string", "enum": e })
    }
}

/// Values read from the schema for `type_name.property`. An unknown pair
/// publishes a bare string rather than an empty enum, so a typo here degrades to
/// today's behaviour instead of forbidding every value.
pub fn from_schema(type_name: &str, property: &str, nullable: bool) -> Schema {
    match reflow2_core::schema::enum_values(type_name, property) {
        Some(v) if !v.is_empty() => enum_of(v, nullable),
        _ => {
            if nullable {
                json_schema!({ "type": ["string", "null"] })
            } else {
                json_schema!({ "type": "string" })
            }
        }
    }
}

pub fn from_list(values: &[&str], nullable: bool) -> Schema {
    enum_of(values.iter().map(|s| s.to_string()), nullable)
}

/// One `schema_with` function per field. `opt` = the field is `Option<String>`.
macro_rules! schema_enum {
    ($name:ident, $ty:literal, $prop:literal, opt) => {
        pub fn $name(_: &mut SchemaGenerator) -> Schema {
            from_schema($ty, $prop, true)
        }
    };
    ($name:ident, $ty:literal, $prop:literal, req) => {
        pub fn $name(_: &mut SchemaGenerator) -> Schema {
            from_schema($ty, $prop, false)
        }
    };
}
macro_rules! list_enum {
    ($name:ident, $list:expr, opt) => {
        pub fn $name(_: &mut SchemaGenerator) -> Schema {
            from_list($list, true)
        }
    };
    ($name:ident, $list:expr, req) => {
        pub fn $name(_: &mut SchemaGenerator) -> Schema {
            from_list($list, false)
        }
    };
}

// Node properties
schema_enum!(capability_status_opt, "Capability", "status", opt);
schema_enum!(capability_status_req, "Capability", "status", req);
schema_enum!(requirement_status_req, "Requirement", "status", req);
schema_enum!(requirement_status_opt, "Requirement", "status", opt);
schema_enum!(requirement_lineage_req, "Requirement", "lineage", req);
schema_enum!(requirement_provenance_req, "Requirement", "provenance", req);
schema_enum!(requirement_provenance_opt, "Requirement", "provenance", opt);
schema_enum!(requirement_priority_opt, "Requirement", "priority", opt);
schema_enum!(actor_type_opt, "Actor", "actor_type", opt);
schema_enum!(fragment_note_kind_opt, "Fragment", "note_kind", opt);
schema_enum!(
    environment_rule_type_opt,
    "EnvironmentRule",
    "rule_type",
    opt
);
schema_enum!(violation_proposer_opt, "VIOLATES_RULE", "proposer", opt);
schema_enum!(violation_severity_opt, "VIOLATES_RULE", "severity", opt);
schema_enum!(violation_status_req, "VIOLATES_RULE", "status", req);
schema_enum!(artifact_type_opt, "Artifact", "artifact_type", opt);
schema_enum!(verification_method_opt, "Verification", "method", opt);
schema_enum!(verification_level_opt, "Verification", "level", opt);
schema_enum!(verification_status_req, "Verification", "status", req);
schema_enum!(verification_status_opt, "Verification", "status", opt);
schema_enum!(release_unit_type_opt, "Release", "unit_type", opt);
schema_enum!(environment_env_type_opt, "Environment", "env_type", opt);
schema_enum!(flow_type_opt, "Flow", "flow_type", opt);
schema_enum!(constraint_category_opt, "Constraint", "category", opt);
schema_enum!(contributor_kind_opt, "Contributor", "kind", opt);
schema_enum!(interface_medium_opt, "Interface", "medium", opt);
schema_enum!(interface_paradigm_opt, "Interface", "paradigm", opt);
schema_enum!(
    interface_payload_format_opt,
    "Interface",
    "payload_format",
    opt
);
schema_enum!(interface_auth_opt, "Interface", "auth", opt);
schema_enum!(
    interface_transport_security_opt,
    "Interface",
    "transport_security",
    opt
);
schema_enum!(
    change_event_change_type_opt,
    "ChangeEvent",
    "change_type",
    opt
);
schema_enum!(epoch_type_opt, "DesignEpoch", "epoch_type", opt);
schema_enum!(decision_status_req, "Decision", "status", req);
// The one-call landing status on the constructors (2026-09-06,
// dec:idea-should-a-constructor-accept-the-owners-word-in-one-call): optional,
// same legal values as the setter.
schema_enum!(decision_status_opt, "Decision", "status", opt);
schema_enum!(
    decision_quality_target_req,
    "Decision",
    "quality_target",
    req
);
// Edge properties
schema_enum!(realizes_completeness_opt, "REALIZES", "completeness", opt);
schema_enum!(realizes_conformance_opt, "REALIZES", "conformance", opt);
schema_enum!(constrains_basis_opt, "CONSTRAINS", "basis", opt);
schema_enum!(deployed_to_status_opt, "DEPLOYED_TO", "status", opt);
schema_enum!(
    requires_resource_criticality_opt,
    "REQUIRES_RESOURCE",
    "criticality",
    opt
);
schema_enum!(authored_by_role_opt, "AUTHORED_BY", "role", opt);
schema_enum!(changed_action_opt, "CHANGED", "action", opt);
schema_enum!(changed_action_req, "CHANGED", "action", req);
// Sets that live only in code — the same const the handler enforces.
list_enum!(
    review_relation_req,
    reflow2_core::relate::REVIEW_RELATIONS,
    req
);
list_enum!(
    manual_work_diagnosis_req,
    reflow2_core::manual_work::DIAGNOSES,
    req
);
list_enum!(
    observed_outcome_req,
    reflow2_core::verify::OBSERVED_OUTCOMES,
    req
);
pub const DOC_KINDS: &[&str] = &[
    "design_doc",
    "adr",
    "readme",
    "runbook",
    "agent_instructions",
    "dataflow",
    "sequence_diagram",
    "arch_diagram",
];
list_enum!(doc_kind_opt, DOC_KINDS, opt);
pub const HEAL_STRATEGIES: &[&str] = &["conservative", "balanced", "aggressive"];
list_enum!(heal_strategy_opt, HEAL_STRATEGIES, opt);
