//! The schema's `default:` declarations become readable — and are checked.
//!
//! Measured 2026-08-10 while starting BL-198: **the schema declared 81 defaults
//! and nothing read a single one of them**
//! (`fact:the-schema-declares-81-defaults-and-nothing-reads-them`).
//! `dynograph_core::PropertySpec` has no `default` field, so the key parsed to
//! nobody and was silently dropped; `schema.rs` never mentioned the word; and
//! `describe_schema` surfaced `required` and `values` but never a default. They
//! were documentation wearing the costume of behaviour.
//!
//! The values that DO appear in the data came from the typed constructors
//! injecting them unconditionally — which is why 3,207 stored values across 59%
//! of reflow2's own nodes equal a declared default, and why an absent property
//! today means nothing at all.
//!
//! ## What these cases are for
//!
//! Making the declaration readable is the PRECONDITION for BL-198, not the fix.
//! The fix belongs at the write surface: stripping `value == default` at export
//! cannot work, because the store cannot tell an explicitly-chosen value from an
//! injected one, so absence would come to mean "nobody said, **or** somebody said
//! the default" — which looks like the distinction without being it.
//!
//! The last case is the one that earns its keep beyond this: **a default outside
//! its own enum is a latent bug nothing could previously see**, because neither
//! the parser that ignored defaults nor the validator that checks values ever
//! compared the two.

use reflow2_core::schema::{declared_defaults, schema_default};

#[test]
fn the_declared_defaults_are_actually_parsed() {
    let d = declared_defaults();
    assert!(
        d.len() >= 70,
        "the schema declares ~81 defaults; parsing {} means the line shape moved \
         and this parser stopped seeing them — silently, which is the failure it \
         exists to end",
        d.len()
    );
}

#[test]
fn a_known_default_resolves_by_type_and_property() {
    // One per shape: a plain enum, an enum whose type also exists elsewhere with
    // a DIFFERENT value list, and a property added late (BL-188).
    // Defaults that REMAIN — meaningful initial states, not injections.
    assert_eq!(
        schema_default("Requirement", "status").as_deref(),
        Some("proposed")
    );
    assert_eq!(
        schema_default("Artifact", "granularity").as_deref(),
        Some("atomic")
    );
    // And the seven removed for BL-198 step 1 now resolve to nothing, which is
    // the change itself: dynograph injects only what still declares a default.
    assert_eq!(schema_default("Capability", "tier"), None);
    assert_eq!(schema_default("Requirement", "kind"), None);
}

#[test]
fn a_property_with_no_declared_default_says_so_rather_than_guessing() {
    // GATED_ON.min_level is REQUIRED and deliberately undefaulted — defaulting it
    // would pick a risk posture for the user
    // (`dec:readiness-is-an-observation-the-threshold-is-the-judgement`).
    assert_eq!(schema_default("Decision", "decision"), None);
    assert_eq!(schema_default("NoSuchType", "tier"), None);
}

/// THE CROSS-CHECK NOTHING COULD MAKE BEFORE. The validator checks that a stored
/// value is one of `values`; the parser ignored `default` entirely. So a default
/// naming a value its own enum does not allow was invisible to both.
#[test]
fn every_declared_default_is_a_legal_value_of_its_own_enum() {
    let offenders: Vec<String> = declared_defaults()
        .into_iter()
        .filter(|d| !d.values.is_empty() && !d.values.contains(&d.value))
        .map(|d| {
            format!(
                "{}.{} = {} not in {:?}",
                d.node_type, d.property, d.value, d.values
            )
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "a declared default must be one of the property's own declared values: {offenders:#?}"
    );
}

// ---------------------------------------------------------------------------
// BL-198, step 1: an absent property honestly means "nobody said".
//
// `default:` in the schema YAML is not documentation — dynograph-core APPLIES it
// at write time (`schema.rs:507`: any missing property that declares a default
// is inserted before validation). So a caller who said nothing got the value
// STORED, and 3,207 stored values across 59% of reflow2's own nodes equalled a
// declared default with no way to tell a choice from an injection.
//
// The fix therefore needs no code and crosses no repo boundary: REMOVE the
// `default:` key where injection is wrong. It is forward-only — nodes already
// written keep what they hold — so the committed export does not churn and
// `compare_designs` sees nothing, which is why this route avoids entirely the
// phantom-diff cost BL-198 warned about.
//
// Anthony confirmed the list 2026-08-10: the descriptive enums nobody chooses.
// `status` is deliberately NOT among them — `Requirement.status = proposed` is a
// meaningful initial state that the whole certainty doctrine reads, and removing
// its default would break derivation rather than clean it.
// ---------------------------------------------------------------------------

use reflow2_core::DesignGraph;
use reflow2_core::nodes::node;

/// The seven confirmed properties are no longer injected, so their absence is a
/// fact about what nobody said rather than an artefact of the constructor.
#[test]
fn the_descriptive_enums_nobody_chooses_are_no_longer_injected() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:x", "X", "a need").unwrap();
    let r = g.get_node(node::REQUIREMENT, "req:x").unwrap().unwrap();
    for absent in ["kind", "concern", "designation"] {
        assert!(
            !r.properties.contains_key(absent),
            "Requirement.{absent} must be ABSENT when nobody chose it — its \
             `default:` was removed from the schema so dynograph stops injecting it"
        );
    }

    g.add_capability("cap:x", "Cap", "does x", None).unwrap();
    let c = g.get_node(node::CAPABILITY, "cap:x").unwrap().unwrap();
    assert!(!c.properties.contains_key("tier"));

    g.add_component("cmp:x", "C", "a part", None).unwrap();
    let cm = g.get_node(node::COMPONENT, "cmp:x").unwrap().unwrap();
    assert!(!cm.properties.contains_key("kind"));

    // AND THE ONE THAT CAME OFF THE LIST, kept as a case rather than a comment.
    // `lineage` was on the confirmed removal list and `requirement_lineage.rs`
    // refused it: `original` is the BASE CASE of a three-way classification whose
    // other two values (`decomposed`, `derived`) are set explicitly by
    // operations, so it is a meaningful initial state exactly like `status` — not
    // a descriptive enum nobody chooses. The test that caught it exists to pin
    // that the edge and the label are the same fact.
    assert_eq!(
        r.properties
            .get("lineage")
            .and_then(dynograph_core::Value::as_str),
        Some("original"),
        "lineage stays injected: `original` is a claim, not an absence"
    );

    // SECOND PASS, confirmed 2026-08-10 after auditing all 81. These four are
    // DEAD VOCABULARY rather than descriptive enums: `is_entry_point`,
    // `is_exit_point` and `Component.tier` have ZERO string-literal occurrences
    // anywhere in either crate — never set, never read, materialised on every
    // node. `Release.unit_type` is settable through add_release but never read,
    // and defaulting it asserted that every release is a container while reflow2
    // ships tarballs.
    assert!(!c.properties.contains_key("is_entry_point"));
    assert!(!c.properties.contains_key("is_exit_point"));
    assert!(!cm.properties.contains_key("tier"));

    g.add_release("rel:x", "v1", None, None).unwrap();
    let rel = g.get_node(node::RELEASE, "rel:x").unwrap().unwrap();
    assert!(
        !rel.properties.contains_key("unit_type"),
        "a release no longer claims to be a container merely because nobody said"
    );
}

/// THE COUNTERWEIGHT, and it is the one that keeps this honest. A default that
/// carries a MEANING must keep being injected: `status` is read by
/// `dec:certainty-derived`, graph_report's certainty line, detect_gaps,
/// loop_status and what_next. Removing it would break derivation, not clean it.
#[test]
fn a_meaningful_initial_state_is_still_injected() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:x", "X", "a need").unwrap();
    let r = g.get_node(node::REQUIREMENT, "req:x").unwrap().unwrap();
    assert_eq!(
        r.properties
            .get("status")
            .and_then(dynograph_core::Value::as_str),
        Some("proposed"),
        "a requirement lands at `proposed` and the certainty doctrine reads it — \
         this default is a statement, not an injection"
    );
    assert!(
        r.properties.contains_key("provenance"),
        "provenance is read by requirement_certainty; it stays"
    );
}

/// A value the caller DOES supply is stored, so absence and choice are now
/// distinguishable — which is the whole point of the change.
#[test]
fn a_value_the_caller_chooses_is_still_stored() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "Cap", "does x", None).unwrap();
    g.create_node(
        node::CAPABILITY,
        "cap:x",
        reflow2_core::nodes::Props::new()
            .set("name", "Cap")
            .set("description", "does x")
            .set("tier", "strategic"),
    )
    .unwrap();
    let c = g.get_node(node::CAPABILITY, "cap:x").unwrap().unwrap();
    assert_eq!(
        c.properties
            .get("tier")
            .and_then(dynograph_core::Value::as_str),
        Some("strategic"),
        "presence now means somebody chose it"
    );
}
