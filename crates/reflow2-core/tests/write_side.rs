//! Write-side coverage (WS-1..3) — can a user record what DETECT asks for?
//!
//! The audit found reflow2's read side running ahead of its write side:
//! `Verification`, `Release`, `Environment`, `Resource` and `Decision` were
//! counted by DETECT, listed by the report and classified by PROPAGATE, but had
//! no typed constructor. So the system raised gaps demanding exactly those types
//! and offered no way to answer.
//!
//! These tests assert the round trip that matters: the gap fires, the user
//! records the thing it asked for, and the gap closes.

use reflow2_core::LinkArtifactOptions;
use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::{edge, node};
use reflow2_core::propagate::{ImpactDirection, PropagateOptions};

/// A design built as far as P3: intent, capability, component, and a real file.
fn built_thread() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_project("proj:1", "Scoreboard").expect("project");
    g.add_requirement("req:live", "Live scores", "scores update live")
        .expect("req");
    g.add_capability("cap:score", "Scoring", "tracks the score", None)
        .expect("cap");
    g.add_component("cmp:engine", "Score engine", "computes scores", None)
        .expect("cmp");
    g.satisfies("cap:score", "req:live").expect("satisfies");
    g.allocate("cap:score", "cmp:engine").expect("allocate");
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:score".into(),
        name: "Score.cs".into(),
        location: Some("src/Score.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:score".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:aaa".into()),
    })
    .expect("link");
    g
}

fn gap_sources(g: &DesignGraph) -> Vec<GapSource> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .map(|c| c.gap_source)
        .collect()
}

// ---- WS-1 · Verification ---------------------------------------------------

#[test]
fn recording_a_verification_closes_the_gap_that_asked_for_one() {
    let mut g = built_thread();
    assert!(
        gap_sources(&g).contains(&GapSource::BuildWithoutVerification),
        "a built design with no checks must raise the gap first"
    );

    g.add_verification("ver:score", "Score unit tests", Some("test"), Some("unit"))
        .expect("verification");
    g.verifies("ver:score", node::CAPABILITY, "cap:score")
        .expect("verifies");
    // Coverage below counts a check that PASSES, not one that exists (BL-30).
    g.set_verification_status("ver:score", "passing", None)
        .expect("status");

    let after = g.detect_gaps().expect("detect");
    let sources: Vec<GapSource> = after.iter().map(|c| c.gap_source).collect();
    assert!(
        !sources.contains(&GapSource::BuildWithoutVerification),
        "recording a verification must close the phase-level gap, got {sources:?}"
    );

    // Verifying the capability closes its gap. The artifact's own coverage is
    // no longer *asked* about — one VERIFIES edge per source file is bookkeeping
    // nobody writes, and it was 22 of 25 gaps on reflow2's own design (BL-23).
    let affected = |src: GapSource| -> Vec<&str> {
        after
            .iter()
            .filter(|c| c.gap_source == src)
            .flat_map(|c| c.affected_ids.iter().map(String::as_str))
            .collect()
    };
    assert!(
        affected(GapSource::UnverifiedCapability).is_empty(),
        "the verified capability must no longer be flagged, got {:?}",
        affected(GapSource::UnverifiedCapability)
    );
    assert!(
        affected(GapSource::UnverifiedArtifact).is_empty(),
        "per-file coverage is a signal, not a gap, got {:?}",
        affected(GapSource::UnverifiedArtifact)
    );

    // But it is still counted, and visible in the report.
    let cov = g.verification_coverage().expect("coverage");
    assert_eq!((cov.capabilities, cov.capabilities_verified), (1, 1));
    assert_eq!(
        (cov.artifacts, cov.artifacts_verified),
        (1, 0),
        "the artifact carries no check of its own, and the number says so"
    );
    assert!(
        g.graph_report()
            .unwrap()
            .to_markdown()
            .contains("Verification coverage"),
        "the report must show where per-file coverage stands"
    );
}

#[test]
fn a_failing_check_reaches_the_requirement_behind_it() {
    let mut g = built_thread();
    g.add_verification("ver:score", "Score unit tests", Some("test"), Some("unit"))
        .expect("verification");
    g.verifies("ver:score", node::CAPABILITY, "cap:score")
        .expect("verifies");
    g.set_verification_status("ver:score", "failing", Some("2026-07-18T10:00:00Z"))
        .expect("status");

    // A failing check is a live signal: propagate from it to see what it means.
    let radius = g
        .propagate_from(&["ver:score"], PropagateOptions::default())
        .expect("propagate");
    let reached: Vec<&str> = radius.impacted.iter().map(|n| n.node_id.as_str()).collect();
    assert!(
        reached.contains(&"cap:score") && reached.contains(&"req:live"),
        "a failing check must reach the capability and the requirement, got {reached:?}"
    );
    let cap = radius
        .impacted
        .iter()
        .find(|n| n.node_id == "cap:score")
        .expect("capability impacted");
    assert_eq!(cap.direction, ImpactDirection::Upstream);
}

#[test]
fn setting_status_preserves_the_rest_of_the_check() {
    let mut g = built_thread();
    g.add_verification(
        "ver:score",
        "Score unit tests",
        Some("review"),
        Some("system"),
    )
    .expect("verification");
    let updated = g
        .set_verification_status("ver:score", "passing", None)
        .expect("status");

    assert_eq!(updated.properties["status"].as_str(), Some("passing"));
    assert_eq!(
        updated.properties["method"].as_str(),
        Some("review"),
        "updating the outcome must not erase what the check is"
    );
    assert_eq!(updated.properties["level"].as_str(), Some("system"));
}

#[test]
fn status_on_a_missing_verification_fails_loud() {
    let mut g = built_thread();
    assert!(
        g.set_verification_status("ver:ghost", "passing", None)
            .is_err(),
        "no silent create-on-update"
    );
}

// ---- WS-2 · Release / Environment / Resource -------------------------------

#[test]
fn recording_a_deployment_closes_the_deploy_operate_gap() {
    let mut g = built_thread();
    assert!(
        gap_sources(&g).contains(&GapSource::NoDeployOperate),
        "a design with no operate layer must raise the gap first"
    );

    g.add_release("rel:v1", "Scoreboard v1", Some("1.0.0"), Some("container"))
        .expect("release");
    g.add_environment(
        "env:prod",
        "Production",
        Some("production"),
        Some("us-west"),
    )
    .expect("environment");
    g.deploy_to("rel:v1", "env:prod", Some("active"))
        .expect("deploy");

    assert!(
        !gap_sources(&g).contains(&GapSource::NoDeployOperate),
        "recording a deployment must close the gap"
    );
}

#[test]
fn a_release_carries_its_deployment_status() {
    let mut g = built_thread();
    g.add_release("rel:v1", "Scoreboard v1", Some("1.0.0"), None)
        .expect("release");
    g.add_environment("env:prod", "Production", Some("production"), None)
        .expect("environment");
    g.deploy_to("rel:v1", "env:prod", Some("rolled_back"))
        .expect("deploy");

    let edges = g
        .outgoing("rel:v1", Some(edge::DEPLOYED_TO))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].to_id, "env:prod");
    assert_eq!(
        edges[0].properties["status"].as_str(),
        Some("rolled_back"),
        "the declared-vs-actual axis must survive the round trip"
    );
}

#[test]
fn a_resource_dependency_is_recorded_with_its_criticality() {
    let mut g = built_thread();
    g.add_resource("res:db", "Scores database", Some("postgres"))
        .expect("resource");
    g.require_resource(node::COMPONENT, "cmp:engine", "res:db", Some("required"))
        .expect("requires");

    let edges = g
        .outgoing("cmp:engine", Some(edge::REQUIRES_RESOURCE))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(
        edges[0].properties["criticality"].as_str(),
        Some("required")
    );
}

#[test]
fn a_changed_resource_reaches_what_depends_on_it() {
    let mut g = built_thread();
    g.add_resource("res:db", "Scores database", Some("postgres"))
        .expect("resource");
    g.require_resource(node::COMPONENT, "cmp:engine", "res:db", Some("required"))
        .expect("requires");

    // Swapping the database should surface the component that needs it.
    let radius = g
        .propagate_from(&["res:db"], PropagateOptions::default())
        .expect("propagate");
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cmp:engine"),
        "a resource change must reach its dependents"
    );
}

#[test]
fn an_unknown_enum_value_fails_loud_rather_than_defaulting() {
    let mut g = built_thread();
    assert!(
        g.add_environment("env:x", "Nowhere", Some("moon_base"), None)
            .is_err(),
        "an invalid env_type must fail, not silently fall back to the default"
    );
}

// ---- WS-3 · Decision -------------------------------------------------------

#[test]
fn a_decision_can_be_recorded_and_linked_to_what_it_governs() {
    let mut g = built_thread();
    g.add_decision(
        "dec:store",
        "Use Postgres",
        "Scores are stored in Postgres, not in-memory.",
        Some("Scores must survive a restart; in-memory loses them."),
    )
    .expect("decision");
    g.governed_by(node::COMPONENT, "cmp:engine", node::DECISION, "dec:store")
        .expect("governed_by");

    let edges = g
        .outgoing("cmp:engine", Some(edge::GOVERNED_BY))
        .expect("outgoing");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].to_id, "dec:store");

    let stored = g
        .get_node(node::DECISION, "dec:store")
        .expect("get")
        .expect("present");
    assert!(
        stored.properties["rationale"].as_str().is_some(),
        "the reason is the point of recording the decision"
    );
}

#[test]
fn a_decision_reaches_what_it_governs_when_it_changes() {
    let mut g = built_thread();
    g.add_decision(
        "dec:store",
        "Use Postgres",
        "Postgres, not in-memory.",
        None,
    )
    .expect("decision");
    g.governed_by(node::COMPONENT, "cmp:engine", node::DECISION, "dec:store")
        .expect("governed_by");

    let radius = g
        .propagate_from(&["dec:store"], PropagateOptions::default())
        .expect("propagate");
    assert!(
        radius.impacted.iter().any(|n| n.node_id == "cmp:engine"),
        "revisiting a decision must surface what it shaped"
    );
}

#[test]
fn a_decision_without_its_content_fails_loud() {
    let mut g = built_thread();
    // `decision` is a required property; an empty design record is not useful.
    assert!(
        g.create_node(
            node::DECISION,
            "dec:empty",
            reflow2_core::nodes::Props::new().set("name", "Nameless")
        )
        .is_err(),
        "a Decision with no decision must fail loud"
    );
}

/// BL-6 kept `unverified_capability`'s key deliberately: gap ids hash the
/// source string, and an acknowledgement is stored as a Decision under the
/// resulting id. Renaming it would silently expire every capability
/// acknowledgement a user had made, with nothing to tell them why. This pins
/// the key so a future tidy-up cannot do that by accident.
#[test]
fn the_capability_gap_key_is_frozen_because_acknowledgements_hang_off_it() {
    assert_eq!(
        GapSource::UnverifiedCapability.as_str(),
        "unverified_capability"
    );
    assert_eq!(
        GapSource::UnverifiedArtifact.as_str(),
        "unverified_artifact"
    );
}

/// BL-3. The trial wrote "ASSUMED" into a statement because nothing could set
/// status. Setting it must move the field *and* keep the requirement's own
/// wording intact — a status change must never cost the statement.
#[test]
fn setting_a_requirement_status_preserves_its_statement() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "Offline", "Must work offline.")
        .unwrap();

    let updated = g.set_requirement_status("req:r", "deferred").unwrap();
    assert_eq!(
        updated.properties.get("status").and_then(|v| v.as_str()),
        Some("deferred")
    );
    assert_eq!(
        updated.properties.get("statement").and_then(|v| v.as_str()),
        Some("Must work offline."),
        "the statement must survive a status change"
    );
    assert!(
        g.set_requirement_status("req:nope", "met").is_err(),
        "unknown id fails loud"
    );
}

/// A dropped requirement must go quiet in *both* halves. DETECT already
/// skipped it; HEAL did not, so marking something dropped used to silence one
/// and leave the other nagging about the same node — which reads as broken.
#[test]
fn a_dropped_requirement_is_ignored_by_detect_and_heal_alike() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_requirement("req:r", "Dropped thing", "We decided against this.")
        .unwrap();
    g.contains("proj:p", "Requirement", "req:r").unwrap();
    // A capability elsewhere, so the question is meaningful: on a graph with
    // no capabilities at all, "nothing satisfies this" is answered by the
    // project-level `concept_without_design` nudge, not per requirement.
    g.add_capability("cap:other", "Other", "does something else", None)
        .unwrap();

    let nags_heal = |g: &DesignGraph| {
        g.detect_defects()
            .unwrap()
            .iter()
            .any(|i| i.affected_ids.iter().any(|a| a == "req:r"))
    };
    let nags_detect = |g: &DesignGraph| {
        g.detect_gaps()
            .unwrap()
            .iter()
            .any(|c| c.affected_ids.iter().any(|a| a == "req:r"))
    };

    // BL-42 moved where this is asked. HEAL used to raise an unsatisfied
    // requirement as an `orphan_node` alongside DETECT's
    // `unsatisfied_requirement` — one finding in two lists, which four trials
    // complained about and which reached 20 of 31 defects on storyflow. The
    // requirement question now lives only in DETECT, so HEAL is silent about
    // req:r from the start rather than needing its own status check.
    assert!(
        !nags_heal(&g),
        "requirements are DETECT's question; HEAL never doubles it"
    );
    assert!(
        nags_detect(&g),
        "an unsatisfied requirement is still asked about — once"
    );
    g.set_requirement_status("req:r", "dropped").unwrap();
    assert!(!nags_detect(&g), "a dropped requirement stops being asked");
    assert!(!nags_heal(&g), "and HEAL still says nothing");
}

/// BL-23. The artifact rule was not wrong, it was loud: one VERIFIES edge per
/// source file is bookkeeping nobody writes, and on reflow2's own design it was
/// 22 of 25 gaps — on a crate whose capabilities are all tested. This pins the
/// shape at small scale: many files under one verified capability produce no
/// gaps at all, and the coverage number still tells you where you stand.
#[test]
fn many_files_under_a_verified_capability_raise_no_gaps() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_capability("cap:c", "Scoring", "tracks the score", None)
        .unwrap();
    for i in 0..12 {
        g.link_artifact(LinkArtifactOptions {
            artifact_id: format!("art:{i}"),
            name: format!("file{i}.rs"),
            location: None,
            artifact_type: Some("code".into()),
            target_type: node::CAPABILITY.into(),
            target_id: "cap:c".into(),
            completeness: None,
            provenance: None,
            fragment_id: None,
            checksum: None,
        })
        .unwrap();
    }
    g.add_verification("ver:c", "Scoring tests", Some("test"), Some("unit"))
        .unwrap();
    g.verifies("ver:c", node::CAPABILITY, "cap:c").unwrap();
    // "Verified" in coverage means the check PASSES, not that it exists (BL-30).
    g.set_verification_status("ver:c", "passing", None).unwrap();

    let gaps = g.detect_gaps().unwrap();
    assert!(
        !gaps
            .iter()
            .any(|c| c.gap_source == GapSource::UnverifiedArtifact),
        "12 files under one tested capability must not become 12 questions, got {:?}",
        gaps.iter().map(|c| &c.title).collect::<Vec<_>>()
    );

    // The information is not lost — it is counted instead of asked.
    let cov = g.verification_coverage().unwrap();
    assert_eq!((cov.artifacts, cov.artifacts_verified), (12, 0));
    assert_eq!((cov.capabilities, cov.capabilities_verified), (1, 1));
}

// ---- BL-27 · adopting a system that already exists -------------------------
//
// The graph has to be able to say two things a greenfield design never needs:
// *this capability already ships*, and *I read this back out of the code rather
// than being told it*. Both were unsayable, so an adoption pass produced a graph
// that asserted a production system was entirely unbuilt (ophyd, 15 capabilities)
// and smuggled `[EXTERNAL — …]` into statement text because provenance had
// nowhere to go.

#[test]
fn a_capability_that_already_ships_can_say_so_at_creation() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability(
        "cap:live",
        "Device locking",
        "serialises device access",
        Some("realized"),
    )
    .unwrap();

    let stored = g.get_node(node::CAPABILITY, "cap:live").unwrap().unwrap();
    assert_eq!(
        stored.properties.get("status").unwrap().as_str().unwrap(),
        "realized",
        "a capability recorded as already built must not read back as planned"
    );
}

#[test]
fn a_capability_left_unsaid_still_defaults_to_planned() {
    // The greenfield path must not have to opt out of the correct default.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:new", "Ghost replay", "replays a lap", None)
        .unwrap();

    let stored = g.get_node(node::CAPABILITY, "cap:new").unwrap().unwrap();
    assert_eq!(
        stored.properties.get("status").unwrap().as_str().unwrap(),
        "planned"
    );
}

#[test]
fn setting_a_capability_status_preserves_its_description() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:x", "Scoring", "tracks the score", None)
        .unwrap();
    g.set_capability_status("cap:x", "verified").unwrap();

    let stored = g.get_node(node::CAPABILITY, "cap:x").unwrap().unwrap();
    assert_eq!(
        stored.properties.get("status").unwrap().as_str().unwrap(),
        "verified"
    );
    assert_eq!(
        stored
            .properties
            .get("description")
            .unwrap()
            .as_str()
            .unwrap(),
        "tracks the score",
        "moving a capability's standing must not drop its wording"
    );
}

#[test]
fn status_on_a_missing_capability_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    assert!(g.set_capability_status("cap:nope", "realized").is_err());
}

#[test]
fn an_unknown_capability_status_fails_loud_rather_than_defaulting() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    assert!(
        g.add_capability("cap:x", "X", "does x", Some("shipped"))
            .is_err(),
        "`shipped` is not in the enum; accepting it silently would be a silent fallback"
    );
}

#[test]
fn a_requirement_read_out_of_the_code_can_be_marked_inferred() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement(
        "req:lock",
        "Device locking",
        "only one client drives a device",
    )
    .unwrap();
    g.set_provenance(node::REQUIREMENT, "req:lock", "inferred")
        .unwrap();

    let stored = g.get_node(node::REQUIREMENT, "req:lock").unwrap().unwrap();
    assert_eq!(
        stored
            .properties
            .get("provenance")
            .unwrap()
            .as_str()
            .unwrap(),
        "inferred"
    );
    assert_eq!(
        stored
            .properties
            .get("statement")
            .unwrap()
            .as_str()
            .unwrap(),
        "only one client drives a device",
        "provenance must be queryable, not smuggled into the statement text"
    );
}

#[test]
fn provenance_defaults_to_authored() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_requirement("req:a", "Stated", "a stakeholder said so")
        .unwrap();

    let stored = g.get_node(node::REQUIREMENT, "req:a").unwrap().unwrap();
    assert_eq!(
        stored
            .properties
            .get("provenance")
            .unwrap()
            .as_str()
            .unwrap(),
        "authored"
    );
}

#[test]
fn provenance_on_a_type_that_has_no_such_property_fails_loud() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:1", "Thing").unwrap();
    let err = g
        .set_provenance(node::PROJECT, "proj:1", "inferred")
        .expect_err(
            "Project declares no provenance; silently doing nothing would be a silent drop",
        );
    assert!(
        err.to_string().contains("Requirement"),
        "the rejection must name where the property does live, got: {err}"
    );
}

#[test]
fn provenance_survives_an_export_import_round_trip() {
    // import_graph is the one bulk write path, and the backlog points an adopt
    // pass at it rather than at N setter calls — so it has to carry this.
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_capability("cap:a", "Polling", "polls devices", Some("realized"))
        .unwrap();
    g.set_provenance(node::CAPABILITY, "cap:a", "inferred")
        .unwrap();
    let doc = g.export_graph().unwrap();

    let mut fresh = DesignGraph::open_in_memory().unwrap();
    fresh.import_graph(&doc).unwrap();

    let stored = fresh.get_node(node::CAPABILITY, "cap:a").unwrap().unwrap();
    assert_eq!(
        stored
            .properties
            .get("provenance")
            .unwrap()
            .as_str()
            .unwrap(),
        "inferred"
    );
    assert_eq!(
        stored.properties.get("status").unwrap().as_str().unwrap(),
        "realized"
    );
}

// ---- BL-34 · the as-released view ------------------------------------------

#[test]
fn a_release_records_what_it_ships_and_the_report_reads_it_back() {
    let mut g = built_thread();
    g.add_release("rel:v1", "v1.0", Some("1.0.0"), Some("binary"))
        .unwrap();
    g.release_includes("rel:v1", node::ARTIFACT, "art:score", Some("sha256:aaa"))
        .unwrap();
    g.add_environment("env:prod", "Production", Some("production"), None)
        .unwrap();
    g.deploy_to("rel:v1", "env:prod", Some("active")).unwrap();

    let rep = g.release_report("rel:v1").unwrap();
    assert_eq!(
        rep.artifacts,
        [("art:score".to_string(), Some("sha256:aaa".to_string()))]
    );
    assert_eq!(rep.capabilities_covered, ["cap:score"]);
    assert!(rep.built_capabilities_not_covered.is_empty());
    assert_eq!(
        rep.deployed_to,
        [("env:prod".to_string(), Some("active".to_string()))]
    );
}

#[test]
fn the_frozen_checksum_survives_a_later_baseline_move() {
    // The artifact's own checksum is the live drift baseline and moves with
    // every accept; what a PAST release shipped must not move with it.
    let mut g = built_thread();
    g.add_release("rel:v1", "v1.0", Some("1.0.0"), None)
        .unwrap();
    g.release_includes("rel:v1", node::ARTIFACT, "art:score", Some("sha256:aaa"))
        .unwrap();
    g.set_artifact_checksum(
        "art:score",
        "sha256:bbb",
        reflow2_core::DriftDisposition::DesignHolds {
            change_type: reflow2_core::temporal::ChangeType::Refactor,
        },
        None,
        None,
    )
    .unwrap();

    let rep = g.release_report("rel:v1").unwrap();
    assert_eq!(
        rep.artifacts[0].1.as_deref(),
        Some("sha256:aaa"),
        "the manifest of a past release does not rewrite itself"
    );
}

#[test]
fn a_built_capability_the_release_leaves_out_is_the_diff() {
    // "Does what we released match what we designed?" — the previously
    // inexpressible question, now a field.
    let mut g = built_thread();
    g.add_capability("cap:extra", "Extra", "also built", None)
        .unwrap();
    g.add_component("cmp:extra", "Extra part", "part", None)
        .unwrap();
    g.allocate("cap:extra", "cmp:extra").unwrap();
    g.link_artifact(LinkArtifactOptions {
        artifact_id: "art:extra".into(),
        name: "Extra.cs".into(),
        location: Some("src/Extra.cs".into()),
        artifact_type: Some("code".into()),
        target_type: node::CAPABILITY.into(),
        target_id: "cap:extra".into(),
        completeness: None,
        provenance: None,
        fragment_id: None,
        checksum: Some("sha256:eee".into()),
    })
    .unwrap();
    g.add_release("rel:v1", "v1.0", None, None).unwrap();
    g.release_includes("rel:v1", node::ARTIFACT, "art:score", None)
        .unwrap();

    let rep = g.release_report("rel:v1").unwrap();
    assert_eq!(rep.capabilities_covered, ["cap:score"]);
    assert_eq!(
        rep.built_capabilities_not_covered,
        ["cap:extra"],
        "built, designed, not shipped — the as-released diff"
    );
}

#[test]
fn a_built_component_in_no_release_is_a_gap_once_contents_are_modelled() {
    let mut g = built_thread();
    // cmp:engine is built (art:score realizes cap:score allocated to it).
    g.add_release("rel:v1", "v1.0", None, None).unwrap();

    // Releases exist but model no contents: silence, not a per-component flood.
    assert!(
        !g.detect_gaps()
            .unwrap()
            .iter()
            .any(|x| x.gap_source == GapSource::UnreleasedComponent),
        "an unmodelled manifest is not evidence of unshipped work"
    );

    // Contents are modelled — and this component's build is not among them.
    g.add_artifact("art:other", "other.bin", Some("binary"), None)
        .unwrap();
    g.release_includes("rel:v1", node::ARTIFACT, "art:other", None)
        .unwrap();
    let gaps = g.detect_gaps().unwrap();
    let hit: Vec<&str> = gaps
        .iter()
        .filter(|x| x.gap_source == GapSource::UnreleasedComponent)
        .flat_map(|x| x.affected_ids.iter().map(String::as_str))
        .collect();
    assert_eq!(hit, ["cmp:engine"], "built but ships in nothing");

    // Including its realizing artifact clears it.
    g.release_includes("rel:v1", node::ARTIFACT, "art:score", None)
        .unwrap();
    assert!(
        !g.detect_gaps()
            .unwrap()
            .iter()
            .any(|x| x.gap_source == GapSource::UnreleasedComponent)
    );
}

#[test]
fn a_release_cannot_include_a_requirement() {
    let mut g = built_thread();
    g.add_release("rel:v1", "v1.0", None, None).unwrap();
    assert!(
        g.release_includes("rel:v1", node::REQUIREMENT, "req:live", None)
            .is_err(),
        "a release ships built things, not intent"
    );
}
