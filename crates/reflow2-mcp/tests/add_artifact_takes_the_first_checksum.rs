//! `add_artifact` can set the FIRST checksum, the way every other door already
//! could.
//!
//! The Artifact schema declares `checksum`; `link_artifact` takes one and its
//! skill says ALWAYS supply it, because it is the baseline that makes a later
//! edit detectable; `set_artifact_checksum` exists to move one later. Until
//! 2026-09-15 the plain constructor alone could not set it, and the workaround
//! was `create_node` with a props bag
//! (`fact:defect-two-more-constructor-shape-frictions-met-while-working-add-artifact-cannot-set-a-checksum-and-delete-edge-refuses-the-types-create-edge-requires`).
//!
//! The class is the one the reachability instrument cannot see: a declared
//! property that no typed constructor offers, hidden because most artifacts
//! carry one through the OTHER door and so the property reads as heavily used.
//!
//! A revise that would MOVE an existing baseline is refused, because that is a
//! drift disposition and belongs to `set_artifact_checksum`, which records why.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

fn fresh() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

async fn add(s: &ReflowService, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let req: AddArtifactReq = serde_json::from_value(body).expect("request shape");
    s.add_artifact(Parameters(req))
        .await
        .map(|r| r.structured_content.expect("structured content"))
        .map_err(|e| e.message.to_string())
}

fn checksum_of(out: &serde_json::Value) -> Option<String> {
    for key in ["artifact", "node"] {
        if let Some(n) = out.get(key) {
            return n["properties"]["checksum"].as_str().map(str::to_string);
        }
    }
    out["properties"]["checksum"].as_str().map(str::to_string)
}

#[tokio::test]
async fn the_constructor_stores_the_baseline_it_was_given() {
    let s = fresh();
    let out = add(
        &s,
        json!({"id": "art:gauge", "name": "Gauge.cs", "location": "src/Gauge.cs",
               "artifact_type": "code", "checksum": "sha256:abc123"}),
    )
    .await
    .expect("add_artifact with a checksum");
    assert_eq!(
        checksum_of(&out).as_deref(),
        Some("sha256:abc123"),
        "the checksum must reach the STORED property, not merely be accepted: {out}"
    );
}

#[tokio::test]
async fn omitting_it_stores_nothing() {
    let s = fresh();
    let out = add(
        &s,
        json!({"id": "art:gauge", "name": "Gauge.cs", "location": "src/Gauge.cs",
               "artifact_type": "code"}),
    )
    .await
    .expect("add_artifact without a checksum");
    assert!(
        checksum_of(&out).is_none(),
        "absence means nobody said — no default may be invented: {out}"
    );
}

#[tokio::test]
async fn a_revise_that_would_move_the_baseline_is_refused() {
    let s = fresh();
    add(
        &s,
        json!({"id": "art:gauge", "name": "Gauge.cs", "checksum": "sha256:abc123"}),
    )
    .await
    .expect("first baseline");
    let err = add(&s, json!({"id": "art:gauge", "checksum": "sha256:def456"}))
        .await
        .expect_err("moving a baseline through the constructor must be refused");
    assert!(
        err.contains("set_artifact_checksum"),
        "the refusal names the door that records WHY a baseline moved: {err}"
    );
}
