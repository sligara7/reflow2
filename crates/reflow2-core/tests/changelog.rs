//! The changelog view (`cap:changelog-view`, `ver:changelog-view`).
//!
//! The check was written BEFORE the code, so these are the cases the design
//! said would constitute proof. Each one is a way the capability could report
//! success while being wrong:
//!
//! 1. buckets **mapped** from graph vocabulary rather than guessed;
//! 2. the `[Unreleased]` window is everything after the last **deployed**
//!    release, not the last release node;
//! 3. an empty delta renders as an **empty draft**, not an invented entry;
//! 4. the output is a **draft** — no entry claims what a consumer should do,
//!    because the graph cannot know it.
//!
//! The load-bearing ones are 2 and 4. A window bounded by the newest release
//! node looks identical to a correct one until somebody plans a release ahead
//! of time — which this project now does routinely — and an entry that quietly
//! asserts consumer impact is the commit-log dump Keep a Changelog names an
//! antipattern.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{
    ChangeAction, ChangeRecord, ChangeType, ChangelogBucket, DesignGraph, EpochType, changelog_rule,
};

/// Two releases, each pinned to an epoch, with `before` deployed. The shape
/// every test here starts from.
fn two_releases() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_epoch("epoch:one", "v1 cut", EpochType::ReleaseCut, 10)
        .unwrap();
    g.add_epoch("epoch:two", "v2 cut", EpochType::ReleaseCut, 20)
        .unwrap();
    g.precedes("epoch:one", "epoch:two").unwrap();

    g.create_node(
        node::RELEASE,
        "rel:one",
        Props::new()
            .set("name", "v1.0.0")
            .set("version", "1.0.0")
            .set("status", "deployed"),
    )
    .unwrap();
    g.create_node(
        node::RELEASE,
        "rel:two",
        Props::new()
            .set("name", "v2.0.0")
            .set("version", "2.0.0")
            .set("status", "planned"),
    )
    .unwrap();
    g.create_edge(
        edge::AT_EPOCH,
        node::RELEASE,
        "rel:one",
        node::DESIGN_EPOCH,
        "epoch:one",
        Props::new(),
    )
    .unwrap();
    g.create_edge(
        edge::AT_EPOCH,
        node::RELEASE,
        "rel:two",
        node::DESIGN_EPOCH,
        "epoch:two",
        Props::new(),
    )
    .unwrap();
    g
}

fn a_requirement(g: &mut DesignGraph, id: &str, name: &str) {
    g.add_requirement(id, name, "a statement").unwrap();
}

// -------------------------------------------------------------------------
// 1. Buckets are MAPPED, and every entry says which rule mapped it.
// -------------------------------------------------------------------------

#[test]
fn every_bucket_is_reached_by_a_named_rule_and_never_by_a_guess() {
    let mut g = two_releases();
    a_requirement(&mut g, "req:new", "A new one");
    a_requirement(&mut g, "req:edited", "An edited one");
    a_requirement(&mut g, "req:gone", "A removed one");
    a_requirement(&mut g, "req:retired", "A deprecated one");

    for (ev, target, ct, action) in [
        (
            "chg:a",
            "req:new",
            ChangeType::NewFeature,
            ChangeAction::Added,
        ),
        (
            "chg:b",
            "req:edited",
            ChangeType::ScopeChange,
            ChangeAction::Modified,
        ),
        (
            "chg:c",
            "req:gone",
            ChangeType::Refactor,
            ChangeAction::Removed,
        ),
        (
            "chg:d",
            "req:retired",
            ChangeType::Deprecation,
            ChangeAction::Removed,
        ),
    ] {
        g.record_change(ChangeRecord {
            epoch_id: "epoch:two",
            change_event_id: ev,
            name: "a change",
            change_type: ct,
            target_type: node::REQUIREMENT,
            target_id: target,
            action,
        })
        .unwrap();
    }

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();

    let find = |id: &str| {
        draft
            .entries
            .iter()
            .find(|e| e.subject_id == id)
            .unwrap_or_else(|| panic!("no entry for {id} in {:?}", draft.entries))
            .clone()
    };

    assert_eq!(find("req:new").bucket, ChangelogBucket::Added);
    assert_eq!(find("req:new").rule, changelog_rule::ACTION_ADDED);
    assert_eq!(find("req:edited").bucket, ChangelogBucket::Changed);
    assert_eq!(find("req:edited").rule, changelog_rule::ACTION_MODIFIED);
    assert_eq!(find("req:gone").bucket, ChangelogBucket::Removed);
    assert_eq!(find("req:gone").rule, changelog_rule::ACTION_REMOVED);

    // Deprecation is NOT Removed: retirement with the intent recorded is a
    // different claim from a thing that simply left, and Keep a Changelog
    // keeps them apart because consumers act on them differently.
    assert_eq!(find("req:retired").bucket, ChangelogBucket::Deprecated);
    assert_eq!(
        find("req:retired").rule,
        changelog_rule::ACTION_REMOVED_DEPRECATION
    );

    // Every entry carries a rule from the declared set — nothing arrives in a
    // bucket without naming what put it there.
    for e in &draft.entries {
        assert!(
            changelog_rule::ALL.contains(&e.rule.as_str()),
            "entry {e:?} carries a rule outside the declared set"
        );
        assert!(!e.evidence.is_empty(), "a mapping with no evidence: {e:?}");
    }
}

#[test]
fn a_change_no_rule_covers_is_reported_unfiled_rather_than_dropped() {
    let mut g = two_releases();
    a_requirement(&mut g, "req:odd", "An odd one");
    // A CHANGED edge with no `action` at all — the shape a future writer could
    // introduce. It must not vanish, and must not be guessed into a bucket.
    g.add_change_event("chg:odd", "no action recorded", ChangeType::Resync)
        .unwrap();
    g.pin_at_epoch(node::CHANGE_EVENT, "chg:odd", "epoch:two")
        .unwrap();
    g.create_edge(
        edge::CHANGED,
        node::CHANGE_EVENT,
        "chg:odd",
        node::REQUIREMENT,
        "req:odd",
        Props::new(),
    )
    .unwrap();

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();

    assert!(
        draft.entries.iter().all(|e| e.subject_id != "req:odd"),
        "an unmapped change must not be filed into a bucket"
    );
    assert_eq!(draft.unmapped.len(), 1, "and must not be dropped either");
    assert_eq!(draft.unmapped[0].subject_id, "req:odd");
    assert!(draft.needs_a_human.iter().any(|s| s.contains("no bucket")));
}

// -------------------------------------------------------------------------
// 2. [Unreleased] is bounded by the last DEPLOYED release.
// -------------------------------------------------------------------------

#[test]
fn unreleased_is_bounded_by_the_last_deployed_release_not_the_newest_one() {
    let mut g = two_releases();
    // rel:two exists, sits LATER on the axis, and is only `planned`. A window
    // bounded by "the newest release node" would start at epoch:two and miss
    // everything below — which is exactly the work [Unreleased] is for.
    a_requirement(&mut g, "req:after", "Landed after v1 shipped");
    g.record_change(ChangeRecord {
        epoch_id: "epoch:two",
        change_event_id: "chg:after",
        name: "after the deployed cut",
        change_type: ChangeType::NewFeature,
        target_type: node::REQUIREMENT,
        target_id: "req:after",
        action: ChangeAction::Added,
    })
    .unwrap();

    let draft = g.changelog_view(None, None).unwrap();

    assert_eq!(draft.heading, "[Unreleased]");
    assert_eq!(
        draft.from.as_deref(),
        Some("rel:one"),
        "the base must be the DEPLOYED release, not the newest node"
    );
    assert_eq!(draft.from_sequence, Some(10));
    assert!(
        draft.entries.iter().any(|e| e.subject_id == "req:after"),
        "work after the deployed cut belongs in [Unreleased], got {:?}",
        draft.entries
    );
}

#[test]
fn a_release_with_no_epoch_is_announced_rather_than_silently_widening_the_window() {
    let mut g = two_releases();
    // The defect found live on 2026-07-31: v0.19.0 was cut with its epoch
    // created but never linked, and nothing said so.
    g.create_node(
        node::RELEASE,
        "rel:orphan",
        Props::new()
            .set("name", "v3.0.0")
            .set("version", "3.0.0")
            .set("status", "deployed"),
    )
    .unwrap();

    let draft = g
        .changelog_view(Some("rel:orphan"), Some("rel:two"))
        .unwrap();

    assert!(
        draft.notes.iter().any(|n| n.contains("no epoch")),
        "a release with no AT_EPOCH must be called out, got {:?}",
        draft.notes
    );
    assert_eq!(draft.from_sequence, None);
}

// -------------------------------------------------------------------------
// 3. An empty delta is an empty draft, not an invented entry.
// -------------------------------------------------------------------------

#[test]
fn an_empty_window_renders_an_empty_draft_and_invents_nothing() {
    let g = two_releases();
    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();

    assert!(
        draft.entries.is_empty(),
        "nothing changed, so nothing is drafted — got {:?}",
        draft.entries
    );
    assert!(draft.unmapped.is_empty());
    assert!(
        draft.manifest.appeared.is_empty() && draft.manifest.left.is_empty(),
        "neither release ships anything yet"
    );
    // Still a draft, still headed, still honest about what a person owes.
    assert!(draft.is_draft);
    assert_eq!(draft.heading, "[2.0.0]");
    assert!(!draft.needs_a_human.is_empty());
}

#[test]
fn the_manifest_delta_files_what_appeared_and_what_left() {
    let mut g = two_releases();
    g.add_artifact("art:kept", "kept.rs", None, None).unwrap();
    g.add_artifact("art:new", "new.rs", None, None).unwrap();
    g.add_artifact("art:gone", "gone.rs", None, None).unwrap();

    g.release_includes("rel:one", node::ARTIFACT, "art:kept", None)
        .unwrap();
    g.release_includes("rel:one", node::ARTIFACT, "art:gone", None)
        .unwrap();
    g.release_includes("rel:two", node::ARTIFACT, "art:kept", None)
        .unwrap();
    g.release_includes("rel:two", node::ARTIFACT, "art:new", None)
        .unwrap();

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();

    assert_eq!(draft.manifest.appeared, vec!["art:new".to_string()]);
    assert_eq!(draft.manifest.left, vec!["art:gone".to_string()]);

    let added: Vec<_> = draft
        .entries
        .iter()
        .filter(|e| e.bucket == ChangelogBucket::Added)
        .map(|e| e.subject_id.as_str())
        .collect();
    assert_eq!(added, vec!["art:new"]);
    let removed: Vec<_> = draft
        .entries
        .iter()
        .filter(|e| e.bucket == ChangelogBucket::Removed)
        .map(|e| e.subject_id.as_str())
        .collect();
    assert_eq!(removed, vec!["art:gone"]);
    // art:kept is in both manifests and therefore in neither list — a
    // changelog reports the DELTA, not the inventory.
    assert!(draft.entries.iter().all(|e| e.subject_id != "art:kept"));
}

// -------------------------------------------------------------------------
// 4. It is a DRAFT: no entry claims what a consumer should do.
// -------------------------------------------------------------------------

#[test]
fn no_entry_asserts_consumer_impact_and_the_obligation_is_named_instead() {
    let mut g = two_releases();
    a_requirement(&mut g, "req:x", "Something");
    g.record_change(ChangeRecord {
        epoch_id: "epoch:two",
        change_event_id: "chg:x",
        name: "a change",
        change_type: ChangeType::NewFeature,
        target_type: node::REQUIREMENT,
        target_id: "req:x",
        action: ChangeAction::Added,
    })
    .unwrap();

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();

    assert!(draft.is_draft, "is_draft is not negotiable");
    assert!(
        !draft.entries.is_empty(),
        "this test is worthless without an entry to inspect"
    );

    // An entry carries WHAT moved and WHY it is in this bucket. It must not
    // carry an instruction — that is the half the graph cannot know, and
    // asserting it would be the fabrication `dec:report-dont-judge` forbids.
    for e in &draft.entries {
        let text = format!("{} {} {}", e.subject_name, e.rule, e.evidence).to_lowercase();
        for imperative in [
            "you should",
            "upgrade to",
            "you must",
            "re-run",
            "no action required",
        ] {
            assert!(
                !text.contains(imperative),
                "entry {e:?} asserts consumer impact via {imperative:?}"
            );
        }
    }

    // And the obligation is NAMED rather than left absent, so a reader cannot
    // mistake a draft for a finished changelog.
    assert!(
        draft
            .needs_a_human
            .iter()
            .any(|s| s.to_lowercase().contains("consumer")),
        "the consumer-impact obligation must be stated: {:?}",
        draft.needs_a_human
    );
}

#[test]
fn the_same_window_twice_produces_the_identical_draft() {
    let mut g = two_releases();
    a_requirement(&mut g, "req:a", "A");
    a_requirement(&mut g, "req:b", "B");
    for (ev, target) in [("chg:1", "req:a"), ("chg:2", "req:b")] {
        g.record_change(ChangeRecord {
            epoch_id: "epoch:two",
            change_event_id: ev,
            name: "a change",
            change_type: ChangeType::NewFeature,
            target_type: node::REQUIREMENT,
            target_id: target,
            action: ChangeAction::Added,
        })
        .unwrap();
    }

    let a = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();
    let b = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();
    assert_eq!(a, b, "a derived view must be deterministic");
}

#[test]
fn a_drift_accept_that_repaired_the_code_is_fixed_not_changed() {
    let mut g = two_releases();
    g.add_artifact("art:thing", "thing.rs", None, Some("sha256:aaaa"))
        .unwrap();

    // `set_artifact_checksum` with DesignHolds writes its own ChangeEvent and
    // marks the CHANGED edge `accepted_baseline`. That edge carries
    // action=modified, so if the accept rule were tried AFTER the modified
    // rule this would be filed as Changed and the Fixed bucket would be
    // unreachable in practice while still existing in the enum.
    let (_, ev) = g
        .set_artifact_checksum(
            "art:thing",
            "sha256:bbbb",
            reflow2_core::DriftDisposition::DesignHolds {
                change_type: ChangeType::TestFailureFix,
            },
            Some("restored intended behaviour"),
            Some("2026-07-31"),
        )
        .unwrap();
    g.pin_at_epoch(node::CHANGE_EVENT, &ev, "epoch:two")
        .unwrap();

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();
    let entry = draft
        .entries
        .iter()
        .find(|e| e.subject_id == "art:thing")
        .unwrap_or_else(|| panic!("no entry for the accepted artifact: {:?}", draft.entries));

    assert_eq!(entry.bucket, ChangelogBucket::Fixed);
    assert_eq!(entry.rule, changelog_rule::ACCEPT_TEST_FAILURE_FIX);
}

#[test]
fn an_accept_that_is_not_a_fix_stays_out_of_the_fixed_bucket() {
    let mut g = two_releases();
    g.add_artifact("art:refactored", "r.rs", None, Some("sha256:aaaa"))
        .unwrap();
    // The counterweight: a `design_holds` accept for a REFACTOR carried no
    // design meaning and certainly fixed nothing. Without this case the rule
    // above could be "any accept is Fixed" and still pass.
    let (_, ev) = g
        .set_artifact_checksum(
            "art:refactored",
            "sha256:cccc",
            reflow2_core::DriftDisposition::DesignHolds {
                change_type: ChangeType::Refactor,
            },
            None,
            None,
        )
        .unwrap();
    g.pin_at_epoch(node::CHANGE_EVENT, &ev, "epoch:two")
        .unwrap();

    let draft = g.changelog_view(Some("rel:one"), Some("rel:two")).unwrap();
    let entry = draft
        .entries
        .iter()
        .find(|e| e.subject_id == "art:refactored")
        .unwrap();
    assert_eq!(entry.bucket, ChangelogBucket::Changed);
    assert_eq!(entry.rule, changelog_rule::ACTION_MODIFIED);
}
