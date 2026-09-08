//! Every type that declares a description can be given one by the tool that
//! makes it.
//!
//! # The class, not the instance
//!
//! Five properties surfaced on 2026-09-07 when the reachability gate's
//! cross-type leak was closed, four of them the same field name on four
//! different types. Running the root-cause skill on them found one hole rather
//! than five: of the eleven types declaring `description`, SIX had a
//! constructor that did not take it — Artifact, Environment, Interface,
//! Project, Release, Resource — and for every one of those six, `description`
//! is the ONLY prose field the type declares. None of them took `statement`,
//! `purpose` or `summary` either. So on six of eleven types the surface let a
//! user NAME a thing and never say WHAT IT IS, except through the generic
//! escape hatch.
//!
//! The history says why. `CapabilityReq` carried `description` from its
//! introduction on 2026-07-18 and `ContributorReq` from 2026-07-22, but
//! `VerificationReq` gained one only on 2026-08-17 — the founding case this
//! project's own reachability instrument records in its docstring, where
//! `Verification.description` was the type's embedding field, used once in 164
//! nodes, because `add_verification` had no parameter for it. The hole was
//! found, reported, and fixed on that one type. Nobody asked which siblings had
//! it, and until today nothing could answer.
//!
//! `fact:six-constructors-cannot-write-any-prose-because-the-class-was-fixed-one-report-at-a-time-and-never-swept`.
//!
//! # Why this test is written as a sweep
//!
//! Because the cause is that nobody swept. A test naming today's six types
//! would repeat the habit one generation along: the next type added with a
//! `description` and no parameter would pass it. This walks the SCHEMA — every
//! declared node type, every declared `description` — and asserts the surface
//! can write it. A new type gets covered by existing here, which is the only
//! shape of test that pins a never-swept class.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value as JsonValue, json};

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// Types whose `description` is deliberately NOT reachable from a constructor,
/// each with the reason. Kept tiny on purpose: an exemption list is where a
/// swept class goes back to being unswept, so anything added here needs a
/// sentence a reader can disagree with.
const NO_CONSTRUCTOR: &[(&str, &str)] = &[(
    "DimensionAssessment",
    "machine-written: an assessment is produced by the dimension pass, not typed in by a user, \
     so it has no constructor by design.",
)];

/// The one type whose constructor is not named `add_<snake(type)>`: a
/// DesignEpoch is made by `add_epoch` (and `plan_epoch`). Stated as data rather
/// than special-cased in the loop, so the sweep stays readable.
const CONSTRUCTOR_NAMES: &[(&str, &str)] = &[("DesignEpoch", "add_epoch")];

fn schema_types_declaring_description(s: &JsonValue) -> Vec<String> {
    let mut out = Vec::new();
    let types = s
        .get("node_types")
        .and_then(|v| v.as_array())
        .expect("describe_schema returns node_types");
    for t in types {
        let name = t
            .get("node_type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let has_desc = t
            .get("properties")
            .and_then(|p| p.as_array())
            .map(|props| {
                props.iter().any(|p| {
                    p.get("name").and_then(|v| v.as_str()) == Some("description")
                        || p.get("property").and_then(|v| v.as_str()) == Some("description")
                })
            })
            .unwrap_or(false);
        if has_desc {
            out.push(name);
        }
    }
    out
}

/// THE SWEEP. Every type the schema says has a description must be creatable
/// with one, or be named in the exemption list with a reason.
#[tokio::test]
async fn every_type_that_declares_a_description_can_be_given_one() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let schema = j!(s.describe_schema(Parameters(serde_json::from_value(json!({})).unwrap())));
    let declaring = schema_types_declaring_description(&schema);
    assert!(
        declaring.len() >= 10,
        "the sweep must actually find the types; got {declaring:?}"
    );

    // The surface a caller sees: which tools accept a `description` parameter.
    let mut writable = std::collections::BTreeSet::new();
    for tool in ReflowService::capture_router()
        .list_all()
        .into_iter()
        .chain(ReflowService::operate_tools_router().list_all())
        .chain(ReflowService::temporal_tools_router().list_all())
        .chain(ReflowService::built_router().list_all())
        .chain(ReflowService::assure_router().list_all())
    {
        let has = tool
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|p| p.contains_key("description"))
            .unwrap_or(false);
        if has {
            writable.insert(tool.name.to_string());
        }
    }

    let exempt: std::collections::BTreeMap<&str, &str> = NO_CONSTRUCTOR.iter().copied().collect();
    let mut missing = Vec::new();
    for ty in &declaring {
        if exempt.contains_key(ty.as_str()) {
            continue;
        }
        // The constructor is named for the type by convention across this
        // surface, which is what makes the sweep possible at all.
        let expected = CONSTRUCTOR_NAMES
            .iter()
            .find(|(t, _)| *t == ty.as_str())
            .map(|(_, tool)| (*tool).to_string())
            .unwrap_or_else(|| format!("add_{}", to_snake(ty)));
        if !writable.contains(&expected) {
            missing.push(expected);
        }
    }
    assert!(
        missing.is_empty(),
        "these constructors cannot say what the thing they create IS — the type declares a \
         `description` and the tool does not take one, which is the class that survived every \
         instance fix until 2026-09-07: {missing:?}. Give the parameter, or add the type to \
         NO_CONSTRUCTOR with a reason."
    );

    // THE REVERSE SWEEP, AND IT IS THE HALF THAT WAS MISSING. An exemption is a
    // `continue`, so an entry that has stopped being true is skipped in silence
    // — the list can only ever drift toward exempting more than it should.
    //
    // Measured 2026-09-07: it already had. `Actor` sat here reading "there is no
    // add_actor to give a parameter to" while `add_actor` had shipped hours
    // earlier taking a `description`, and `EnvironmentRule` sat here for a type
    // that declares no `description` at all, so it could never have been swept.
    // `QualityGate` sat here for a type that was about to leave the schema
    // (dec:qualitygate-is-retired-the-phase-gate-dissolved-into-the-detectors).
    // Three of four entries were dead and nothing could say so.
    //
    // This is the same class as constructors_preserve.rs's membership being
    // drawn on `add_*` call names: a hand-maintained list with no check that its
    // entries are still earned.
    let mut stale = Vec::new();
    for (ty, _) in NO_CONSTRUCTOR {
        if !declaring.iter().any(|d| d == ty) {
            stale.push(format!(
                "{ty} (declares no `description`, so it is never swept)"
            ));
            continue;
        }
        let expected = CONSTRUCTOR_NAMES
            .iter()
            .find(|(t, _)| t == ty)
            .map(|(_, tool)| (*tool).to_string())
            .unwrap_or_else(|| format!("add_{}", to_snake(ty)));
        if writable.contains(&expected) {
            stale.push(format!("{ty} (`{expected}` now takes a description)"));
        }
    }
    assert!(
        stale.is_empty(),
        "these NO_CONSTRUCTOR entries are no longer earned and are silently shrinking the \
         sweep: {stale:?}. Delete them — an exemption nobody rechecks is how a swept class \
         goes back to being unswept."
    );
}

fn to_snake(camel: &str) -> String {
    let mut out = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// And the value has to REACH THE STORED PROPERTY. A tool that accepts a field
/// and drops it passes a signature sweep and still fails the caller, which is
/// the shape of defect this whole family belongs to.
#[tokio::test]
async fn the_description_reaches_the_stored_property() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let text = "what this thing is, said at the moment it was made";

    let out = j!(s.add_interface(Parameters(
        serde_json::from_value(json!({"id": "iface:gauge-feed", "name": "Gauge feed",
            "description": text}))
        .unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_interface");

    let out = j!(s.add_release(Parameters(
        serde_json::from_value(json!({"id": "rel:v1", "name": "v1", "description": text})).unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_release");

    let out = j!(s.add_resource(Parameters(
        serde_json::from_value(json!({"id": "res:gauge", "name": "Gauge", "description": text}))
            .unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_resource");

    let out = j!(s.add_environment(Parameters(
        serde_json::from_value(json!({"id": "env:field", "name": "Field", "description": text}))
            .unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_environment");

    let out = j!(s.add_project(Parameters(
        serde_json::from_value(json!({"id": "proj:rain", "name": "Rain", "description": text}))
            .unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_project");

    let out = j!(s.add_artifact(Parameters(
        serde_json::from_value(json!({"id": "art:gauge-cs", "name": "Gauge.cs",
            "location": "src/Gauge.cs", "artifact_type": "code", "description": text}))
        .unwrap()
    )));
    assert_eq!(props_of(&out)["description"], text, "add_artifact");
}

fn props_of(out: &JsonValue) -> &JsonValue {
    for key in [
        "interface",
        "release",
        "resource",
        "environment",
        "project",
        "artifact",
        "node",
    ] {
        if let Some(v) = out.get(key).and_then(|v| v.get("properties")) {
            return v;
        }
    }
    out.get("properties")
        .expect("the node's properties come back")
}
