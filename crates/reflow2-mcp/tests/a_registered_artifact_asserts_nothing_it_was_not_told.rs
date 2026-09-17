//! `add_artifact` writes only the status it is told; `link_artifact` writes
//! `realized` on first registration because it read the file off disk; and a
//! re-link never downgrades a stored status.
//!
//! Root cause (xrt-demo F1, 2026-09-16): Artifact.status defaulted to
//! `realized`, the store materialised it on write, and add_artifact had no
//! parameter for it — so six deliverables that did not exist were recorded as
//! produced, "an affirmative claim that work is done", and nothing warned. The
//! same type's `audience`, `granularity` and `volatility` had already been
//! undefaulted for exactly this reason; `status` was left out
//! (`fact:root-cause-add-artifact-materialises-realized-…`).
//!
//! Written before the fix and observed failing: the first test read
//! `status: "realized"` against the unfixed schema.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn artifact(id: &str, status: Option<&str>) -> AddArtifactReq {
    AddArtifactReq {
        id: id.into(),
        name: Some("beamline.json".into()),
        artifact_type: Some("spec".into()),
        location: Some("beamline.json".into()),
        description: None,
        status: status.map(Into::into),
        checksum: None,
    }
}

#[tokio::test]
async fn an_artifact_registered_with_no_status_carries_none() {
    let s = svc().await;
    let out = j!(s.add_artifact(Parameters(artifact("art:beamline-json", None))));
    assert!(
        out["properties"].get("status").is_none(),
        "absent means nobody said — never `realized` on a file nobody has produced: {out:?}"
    );
}

#[tokio::test]
async fn the_status_it_is_told_is_the_status_it_stores() {
    let s = svc().await;
    let out = j!(s.add_artifact(Parameters(artifact("art:verify-py", Some("planned")))));
    assert_eq!(out["properties"]["status"], "planned", "{out:?}");
}

#[tokio::test]
async fn a_file_read_off_disk_is_realized_on_first_registration_and_a_relink_keeps_verified() {
    let s = svc().await;
    s.add_capability(Parameters(CapabilityReq {
        id: "cap:trace".into(),
        name: Some("Trace the beamline".into()),
        description: Some("Runs the ray trace.".into()),
        status: None,
        distinct_from: None,
        tier: None,
        is_entry_point: None,
        is_exit_point: None,
        satisfies: None,
        allocated_to: None,
    }))
    .await
    .expect("capability");
    let link = |checksum: &str| LinkArtifactReq {
        artifact_id: "art:trace-script".into(),
        name: Some("verify.py".into()),
        location: Some("verify.py".into()),
        artifact_type: Some("code".into()),
        description: None,
        target_type: Some("Capability".into()),
        target_id: "cap:trace".into(),
        completeness: None,
        conformance: None,
        provenance: None,
        fragment_id: None,
        content_ref: None,
        note_kind: None,
        checksum: Some(checksum.into()),
    };
    j!(s.link_artifact(Parameters(link("sha256:aaaa"))));
    let node = j!(s.get_node(Parameters(GetNodeReq {
        id: "art:trace-script".into(),
        node_type: None,
    })));
    assert_eq!(
        node["node"]["properties"]["status"], "realized",
        "a checksum read off disk is evidence the file exists: {node:?}"
    );

    // Somebody verified it. A re-link must not quietly take that back.
    s.add_artifact(Parameters(artifact("art:trace-script", Some("verified"))))
        .await
        .expect("revise status");
    j!(s.link_artifact(Parameters(link("sha256:aaaa"))));
    let node = j!(s.get_node(Parameters(GetNodeReq {
        id: "art:trace-script".into(),
        node_type: None,
    })));
    assert_eq!(
        node["node"]["properties"]["status"], "verified",
        "a re-link keeps the stored status: {node:?}"
    );
}
