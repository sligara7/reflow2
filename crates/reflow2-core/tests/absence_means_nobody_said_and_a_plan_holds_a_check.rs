//! Two schema rulings, Anthony 2026-09-06.
//!
//! 1. `Verification.method` / `.level` carry no schema default any more, so a
//!    check recorded without them STORES nothing for them — absence means
//!    nobody said (the Artifact precedent of 2026-08-12; the injector writes
//!    every declared default, so the fix is to declare none).
//!    fact:defect-a-verifications-level-and-method-can-be-set-only-at-birth-...
//! 2. `SCHEDULED_FOR.from` enumerates Verification and Decision, so a plan can
//!    hold what an increment owes to CHECK and to DECIDE, not only to build.
//!    fact:defect-a-planned-verification-cannot-be-scheduled-...
//!
//! Written first and observed failing: (1) the stored node carried method=test,
//! level=unit; (2) both schedule_for calls were refused by endpoint validation.
use reflow2_core::DesignGraph;
use reflow2_core::foundation::core::Value;

fn props(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Value::from(*v)))
        .collect()
}

#[test]
fn a_verification_recorded_without_method_or_level_stores_neither() {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("prj:t", "T").unwrap();
    let stored = g
        .add_verification(
            "ver:inspection",
            "A code inspection",
            None,
            None,
            Some("read the source"),
        )
        .expect("verification");
    assert!(
        !stored.properties.contains_key("method"),
        "no method was given, so none is stored — it read `test` before: {:?}",
        stored.properties.get("method")
    );
    assert!(
        !stored.properties.contains_key("level"),
        "no level was given, so none is stored — it read `unit` before: {:?}",
        stored.properties.get("level")
    );
    // Saying it still works, and is kept exactly.
    let said = g
        .add_verification(
            "ver:sys",
            "System check",
            Some("inspection"),
            Some("system"),
            None,
        )
        .unwrap();
    assert_eq!(
        said.properties.get("method").and_then(|v| v.as_str()),
        Some("inspection")
    );
    assert_eq!(
        said.properties.get("level").and_then(|v| v.as_str()),
        Some("system")
    );
}

#[test]
fn a_planned_check_and_a_pending_ruling_can_be_scheduled_into_an_epoch() {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_project("prj:t", "T").unwrap();
    g.upsert_node(
        "DesignEpoch",
        "epoch:next",
        props(&[
            ("name", "Next"),
            ("epoch_type", "revision"),
            ("status", "planned"),
        ]),
    )
    .unwrap();
    g.upsert_node(
        "Verification",
        "ver:owed",
        props(&[
            ("name", "Owed check"),
            ("description", "runs next increment"),
        ]),
    )
    .unwrap();
    g.upsert_node(
        "Decision",
        "dec:owed",
        props(&[
            ("name", "Owed ruling"),
            ("decision", "TBD"),
            ("rationale", "settle next increment"),
        ]),
    )
    .unwrap();
    g.create_edge(
        "SCHEDULED_FOR",
        "Verification",
        "ver:owed",
        "DesignEpoch",
        "epoch:next",
        std::collections::HashMap::new(),
    )
    .expect("a planned check has a home in the plan");
    g.create_edge(
        "SCHEDULED_FOR",
        "Decision",
        "dec:owed",
        "DesignEpoch",
        "epoch:next",
        std::collections::HashMap::new(),
    )
    .expect("a pending ruling has a home in the plan");
    // The list is still a list: an Artifact is not something an increment owes.
    g.upsert_node(
        "Artifact",
        "art:x",
        props(&[("name", "x"), ("location", "x.rs")]),
    )
    .unwrap();
    assert!(
        g.create_edge(
            "SCHEDULED_FOR",
            "Artifact",
            "art:x",
            "DesignEpoch",
            "epoch:next",
            std::collections::HashMap::new()
        )
        .is_err()
    );
}
