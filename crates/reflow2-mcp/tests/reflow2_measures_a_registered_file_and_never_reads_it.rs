//! reflow2 MEASURES a registered file and never reads it for meaning
//! (`req:reflow2-measures-registered-files-and-never-reads-them-for-meaning`,
//! Anthony 2026-09-18).
//!
//! Until this landed every checksum on an Artifact was a value an agent pasted,
//! and the graph could not tell a pasted hash from a made-up one. These tests
//! pin the four properties that make measuring admissible: a file under the
//! root is hashed and the basis says so; a pasted value is recorded as
//! asserted and compared; anything outside the root is refused unread; and a
//! server that holds no tree says "not on this machine" rather than reporting
//! files absent or drift zero.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};
use std::path::Path;

fn tree() -> tempfile::TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(d.path().join("src")).unwrap();
    std::fs::write(
        d.path().join("src/gauge.rs"),
        b"pub fn gauge() -> u8 { 1 }\n",
    )
    .unwrap();
    d
}

fn sha256_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(std::fs::read(path).unwrap());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

async fn served(root: Option<&Path>) -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    let s = match root {
        Some(r) => s.with_tree_root(r),
        None => s,
    };
    let cap: CapabilityReq =
        serde_json::from_value(json!({"id": "cap:read-the-gauge", "name": "Read the gauge", "description": "Read the gauge once a second"}))
            .unwrap();
    s.add_capability(Parameters(cap)).await.expect("capability");
    s
}

async fn link(s: &ReflowService, body: Value) -> Result<Value, String> {
    let req: LinkArtifactReq = serde_json::from_value(body).expect("request shape");
    s.link_artifact(Parameters(req))
        .await
        .map(|r| r.structured_content.expect("structured"))
        .map_err(|e| e.message.to_string())
}

async fn reconcile(s: &ReflowService, body: Value) -> Result<Value, String> {
    let req: ReconcileArtifactsReq = serde_json::from_value(body).expect("request shape");
    s.reconcile_artifacts(Parameters(req))
        .await
        .map(|r| r.structured_content.expect("structured"))
        .map_err(|e| e.message.to_string())
}

async fn loop_status(s: &ReflowService) -> Value {
    let req: LoopScopeReq = serde_json::from_value(json!({})).expect("request shape");
    s.loop_status(Parameters(req))
        .await
        .expect("loop_status")
        .structured_content
        .expect("structured")
}

async fn artifact(s: &ReflowService, id: &str) -> Value {
    let req: GetNodeReq =
        serde_json::from_value(json!({"node_type": "Artifact", "node_id": id})).unwrap();
    s.get_node(Parameters(req))
        .await
        .expect("get_node")
        .structured_content
        .expect("structured")["node"]["properties"]
        .clone()
}

#[tokio::test]
async fn a_file_under_the_root_is_measured_and_the_basis_says_so() {
    let d = tree();
    let s = served(Some(d.path())).await;
    let out = link(
        &s,
        json!({"artifact_id": "art:gauge", "name": "gauge.rs", "location": "src/gauge.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .expect("link without a checksum");
    let expected = sha256_of(&d.path().join("src/gauge.rs"));
    assert_eq!(out["measurement"]["basis"], "measured", "{out}");
    assert_eq!(out["measurement"]["checksum"], expected, "{out}");
    assert!(
        out["measurement"].get("content").is_none(),
        "nothing returns content: {out}"
    );
    let props = artifact(&s, "art:gauge").await;
    assert_eq!(
        props["checksum"], expected,
        "the measured hash is the stored baseline: {props}"
    );
    assert_eq!(props["checksum_basis"], "measured", "{props}");
}

#[tokio::test]
async fn a_pasted_checksum_is_recorded_as_asserted_and_compared_with_what_was_measured() {
    let d = tree();
    let s = served(Some(d.path())).await;
    let out = link(
        &s,
        json!({"artifact_id": "art:gauge", "name": "gauge.rs", "location": "src/gauge.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge",
               "checksum": "sha256:0000000000000000000000000000000000000000000000000000000000000000"}),
    )
    .await
    .expect("link with a pasted checksum");
    assert_eq!(out["measurement"]["basis"], "asserted", "{out}");
    assert_eq!(
        out["measurement"]["agrees"], false,
        "a pasted value that disagrees with the file is said so, not silently stored: {out}"
    );
    let props = artifact(&s, "art:gauge").await;
    assert_eq!(props["checksum_basis"], "asserted", "{props}");
}

#[tokio::test]
async fn a_location_outside_the_root_is_refused_unread_and_the_artifact_is_still_registered() {
    let d = tree();
    let s = served(Some(d.path())).await;
    let out = link(
        &s,
        json!({"artifact_id": "art:passwd", "name": "passwd", "location": "../../../../etc/passwd",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .expect("registration itself is not refused");
    assert_eq!(out["measurement"]["basis"], "none", "{out}");
    assert_eq!(
        out["measurement"]["not_measured"]["why"], "outside_root",
        "{out}"
    );
    let props = artifact(&s, "art:passwd").await;
    assert!(
        props.get("checksum").is_none(),
        "no baseline was guessed: {props}"
    );
}

#[tokio::test]
async fn reconcile_with_nothing_measures_the_registered_set_and_finds_the_edit() {
    let d = tree();
    let s = served(Some(d.path())).await;
    link(
        &s,
        json!({"artifact_id": "art:gauge", "name": "gauge.rs", "location": "src/gauge.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .unwrap();
    // Nothing moved: the sweep is measured and clean.
    let clean = reconcile(&s, json!({}))
        .await
        .expect("reconcile with nothing");
    assert_eq!(clean["measurement"]["basis"], "measured", "{clean}");
    assert_eq!(clean["measurement"]["measured"], 1, "{clean}");
    assert_eq!(clean["unchanged"], 1, "{clean}");
    assert_eq!(
        clean["findings"].as_array().map(Vec::len),
        Some(0),
        "{clean}"
    );
    // Then the file is edited behind the design's back.
    std::fs::write(
        d.path().join("src/gauge.rs"),
        b"pub fn gauge() -> u8 { 2 }\n",
    )
    .unwrap();
    let drifted = reconcile(&s, json!({}))
        .await
        .expect("reconcile after the edit");
    let kinds: Vec<&str> = drifted["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["kind"].as_str())
        .collect();
    assert_eq!(kinds, vec!["checksum_change"], "{drifted}");
    // And the loop says so at the boundary, without being asked to reconcile.
    let ls = loop_status(&s).await;
    assert_eq!(ls["artifacts"]["measured"], true, "{ls}");
    assert_eq!(ls["artifacts"]["changed"], 1, "{ls}");
    let next = ls["next"].to_string();
    assert!(
        next.contains("ON DISK NOW") && next.contains("art:gauge"),
        "{next}"
    );
    // Then the file goes missing.
    std::fs::remove_file(d.path().join("src/gauge.rs")).unwrap();
    let ls = loop_status(&s).await;
    assert_eq!(ls["artifacts"]["missing"], 1, "{ls}");
}

#[tokio::test]
async fn a_server_holding_no_tree_says_so_instead_of_reporting_zero() {
    let s = served(None).await;
    let out = link(
        &s,
        json!({"artifact_id": "art:gauge", "name": "gauge.rs", "location": "src/gauge.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .expect("registration works without a tree");
    assert_eq!(
        out["measurement"]["not_measured"]["why"], "no_tree",
        "{out}"
    );
    let err = reconcile(&s, json!({}))
        .await
        .expect_err("an empty sweep is refused, not zero");
    assert!(err.contains("does not hold the project tree"), "{err}");
    let ls = loop_status(&s).await;
    assert_eq!(ls["artifacts"]["measured"], false, "{ls}");
    assert!(
        ls["artifacts"]["note"]
            .as_str()
            .unwrap_or("")
            .contains("does not hold"),
        "{ls}"
    );
    // Supplied observations still work, and are recorded as asserted.
    let out = reconcile(
        &s,
        json!({"observed": [{"artifact_id": "art:gauge", "present": true, "checksum": "sha256:abc"}]}),
    )
    .await
    .expect("asserted observations");
    assert_eq!(out["measurement"]["basis"], "asserted", "{out}");
}

#[tokio::test]
async fn an_accept_without_a_checksum_measures_and_an_unreachable_one_is_refused_by_name() {
    let d = tree();
    let s = served(Some(d.path())).await;
    link(
        &s,
        json!({"artifact_id": "art:gauge", "name": "gauge.rs", "location": "src/gauge.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .unwrap();
    std::fs::write(
        d.path().join("src/gauge.rs"),
        b"pub fn gauge() -> u8 { 3 }\n",
    )
    .unwrap();
    let req: SetChecksumReq = serde_json::from_value(json!({
        "artifact_id": "art:gauge", "disposition": "design_holds", "change_type": "refactor"
    }))
    .unwrap();
    let out = s
        .set_artifact_checksum(Parameters(req))
        .await
        .expect("accept measures the new content")
        .structured_content
        .unwrap();
    assert_eq!(out["measurement"]["basis"], "measured", "{out}");
    assert_eq!(
        out["artifact"]["properties"]["checksum"],
        sha256_of(&d.path().join("src/gauge.rs")),
        "{out}"
    );
    // An artifact whose location this server cannot reach is refused, naming why.
    link(
        &s,
        json!({"artifact_id": "art:elsewhere", "name": "elsewhere", "location": "/nowhere/else.rs",
               "target_type": "Capability", "target_id": "cap:read-the-gauge"}),
    )
    .await
    .unwrap();
    let req: SetChecksumReq = serde_json::from_value(json!({
        "artifact_id": "art:elsewhere", "disposition": "baseline_established"
    }))
    .unwrap();
    let err = s
        .set_artifact_checksum(Parameters(req))
        .await
        .expect_err("cannot move a baseline to a guess");
    assert!(
        err.message.contains("pass `checksum` to assert one"),
        "{}",
        err.message
    );
}
