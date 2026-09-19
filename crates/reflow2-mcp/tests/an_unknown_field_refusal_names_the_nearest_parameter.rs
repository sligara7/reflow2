//! An unknown-field refusal names the nearest served parameter FIRST and
//! offers the stale-client possibility second. flo2, 2026-09-18: eight
//! rejected calls were filed upstream as schema drift because the refusal led
//! with "your client's tool list may predate the server", when five of the six
//! names had never existed in any release — the agent had guessed.

use reflow2_mcp::service::stale_client_hint;

fn serde_message(unknown: &str, legal: &[&str]) -> String {
    let list = legal
        .iter()
        .map(|l| format!("`{l}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("unknown field `{unknown}`, expected one of {list}")
}

#[test]
fn node_id_reaches_id_and_parent_id_reaches_project_id() {
    let m = stale_client_hint(&serde_message(
        "node_id",
        &["id", "name", "decision", "rationale"],
    ));
    assert!(m.contains("Nearest served parameter: `id`."), "{m}");
    let m = stale_client_hint(&serde_message(
        "parent_id",
        &["from_id", "node_id", "project_id", "to_id"],
    ));
    assert!(m.contains("Nearest served parameter: `project_id`."), "{m}");
    let m = stale_client_hint(&serde_message(
        "authored_at",
        &[
            "from_type",
            "node_type",
            "from_id",
            "acted_at",
            "contributor_id",
        ],
    ));
    assert!(m.contains("Nearest served parameter: `acted_at`."), "{m}");
}

#[test]
fn the_same_concept_under_a_sibling_tools_name_is_found_by_its_shared_token_not_by_letters() {
    // flo2 F10, 2026-09-19: four of seven rejections were one concept named
    // differently by an adjacent tool. `id` is closer to `status` by letters.
    let m = stale_client_hint(&serde_message(
        "id",
        &["decision_id", "status", "approver", "acted_at"],
    ));
    assert!(
        m.contains("Nearest served parameter: `decision_id`."),
        "{m}"
    );
    let m = stale_client_hint(&serde_message(
        "node_id",
        &[
            "target_id",
            "target_type",
            "epoch_id",
            "change_type",
            "action",
        ],
    ));
    assert!(m.contains("Nearest served parameter: `target_id`."), "{m}");
    let m = stale_client_hint(&serde_message(
        "properties",
        &[
            "edge_type",
            "from_type",
            "from_id",
            "to_type",
            "to_id",
            "props",
        ],
    ));
    assert!(m.contains("Nearest served parameter: `props`."), "{m}");
}

#[test]
fn a_field_that_resembles_nothing_served_gets_no_nearest_name() {
    // `acted_at` on add_capability: no sibling field is the same concept, and
    // offering `id` as "nearest" would be a wrong answer dressed as help.
    let m = stale_client_hint(&serde_message(
        "acted_at",
        &["id", "name", "description", "status", "satisfies", "tier"],
    ));
    assert!(!m.contains("Nearest served parameter"), "{m}");
    let m = stale_client_hint(&serde_message(
        "kind",
        &["id", "name", "statement", "status", "priority", "concern"],
    ));
    assert!(!m.contains("Nearest served parameter"), "{m}");
}

#[test]
fn the_stale_client_line_comes_after_the_nearest_name_and_is_a_possibility() {
    let m = stale_client_hint(&serde_message("node_id", &["id", "name"]));
    let nearest = m.find("Nearest served parameter").expect("nearest first");
    let stale = m.find("may predate the server").expect("stale second");
    assert!(nearest < stale, "{m}");
    assert!(m.contains("If nothing listed is what you meant"), "{m}");
}

#[test]
fn a_message_without_a_legal_list_still_reads_sensibly() {
    let m = stale_client_hint("unknown field `x`");
    assert!(!m.contains("Nearest served parameter"), "{m}");
    assert!(m.contains("may predate the server"), "{m}");
}
