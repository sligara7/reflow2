//! An enum request field publishes its legal values in the tool schema, sourced
//! from the compiled-in design schema — so the first call can be right.
//!
//! fact:defect-the-schema-discovery-tax-enum-fields-are-strings-so-the-published-
//! schema-cannot-name-their-values. Observed failing first against the committed
//! pre-change toolsnaps: `tools/toolsnap.py`'s enum_invariants reported every
//! hand-listed field as publishing no enum (38 of them); `set_interface_spec.auth`
//! published `{"type": ["string","null"]}` and nothing else.
use reflow2_mcp::service::ReflowService;
use serde_json::Value;

fn prop(tools: &[rmcp::model::Tool], tool: &str, field: &str) -> Value {
    let t = tools
        .iter()
        .find(|t| t.name == tool)
        .unwrap_or_else(|| panic!("tool {tool}"));
    let schema = serde_json::to_value(&t.input_schema).expect("schema");
    schema["properties"][field].clone()
}

fn schema_values(ty: &str, prop: &str) -> Vec<String> {
    reflow2_core::schema::enum_values(ty, prop).expect("enum property")
}

fn enum_strings(v: &Value) -> Vec<String> {
    v["enum"]
        .as_array()
        .expect("enum published")
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

#[test]
fn set_interface_spec_auth_publishes_the_mechanisms_the_schema_enforces() {
    let tools = ReflowService::capture_router().list_all();
    let auth = prop(&tools, "set_interface_spec", "auth");
    assert_eq!(
        enum_strings(&auth),
        schema_values("Interface", "auth"),
        "{auth}"
    );
    assert!(
        auth["enum"].as_array().unwrap().iter().any(|x| x.is_null()),
        "an Option field is nullable: {auth}"
    );
    assert!(
        auth["description"]
            .as_str()
            .unwrap_or("")
            .contains("AUTHENTICATION"),
        "the doc comment survives schema_with: {auth}"
    );
}

#[test]
fn add_verification_method_and_level_publish_their_values() {
    let tools = ReflowService::assure_router().list_all();
    assert_eq!(
        enum_strings(&prop(&tools, "add_verification", "method")),
        schema_values("Verification", "method")
    );
    assert_eq!(
        enum_strings(&prop(&tools, "add_verification", "level")),
        schema_values("Verification", "level")
    );
}

#[test]
fn a_required_enum_field_is_not_nullable_and_a_code_only_set_reuses_the_handlers_const() {
    let tools = ReflowService::exchange_router().list_all();
    let status = prop(&tools, "set_decision_status", "status");
    assert_eq!(enum_strings(&status), schema_values("Decision", "status"));
    assert!(
        !status["enum"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x.is_null()),
        "a required field is not nullable: {status}"
    );
    let capture = ReflowService::capture_router().list_all();
    let t = capture
        .iter()
        .find(|t| t.name == "review_relations")
        .expect("tool");
    let schema = serde_json::to_value(&t.input_schema).expect("schema");
    // `relation` lives inside each link item, which schemars hoists into $defs;
    // it publishes the handler's own REVIEW_RELATIONS list.
    let item_rel = schema["$defs"]["RelationLinkReq"]["properties"]["relation"].clone();
    let want: Vec<String> = reflow2_core::relate::REVIEW_RELATIONS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(enum_strings(&item_rel), want, "{item_rel}");
}
