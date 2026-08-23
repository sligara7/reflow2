//! What `claim_report` says about a seat the `mint_seat` TOOL handed out.
//!
//! This file exists because there was no test here at all. The 2026-08-08
//! liveness fix (#100) is pinned in `reflow2-core/tests/claims.rs`, and that
//! test calls `register_seat` BY HAND — supplying the one step the served path
//! omits. So a real fix read as proven while the tool surface it shipped behind
//! was untouched, and dev_storyflow measured the gap the same day (w-aa0607ff)
//! on the deployment the original defect came from.
//!
//! Its own binary because `declare_serving_many_sessions` is a process-global
//! one-way latch; see `reflow2-core/tests/seat_liveness_shared.rs`.

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

async fn shared_service() -> ReflowService {
    // Before the service is built, exactly as `--serve-shared` does it: building
    // one mints the service's own lease, and the mode decides how that lease's
    // siblings are answered.
    reflow2_core::identity::declare_serving_many_sessions();
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:seat".into(),
        name: Some("Seat".into()),
    })));
    j!(s.add_contributor(Parameters(ContributorReq {
        id: "who:ann".into(),
        name: Some("Ann".into()),
        kind: None,
        handle: None,
        description: None,
    })));
    s
}

fn claim_with(seat: &str) -> ClaimReq {
    ClaimReq {
        contributor_id: "who:ann".into(),
        seed_id: "proj:seat".into(),
        depth: Some(1),
        note: Some("cross-validating what another session mined".into()),
        at: None,
        seat: Some(seat.into()),
    }
}

/// End to end over the served surface: mint a seat the way an agent is told to,
/// claim with it, and read the board. The claim must not report `live`.
#[tokio::test]
async fn a_claim_carrying_a_tool_minted_seat_does_not_read_live_on_a_shared_server() {
    let s = shared_service().await;

    let seat = j!(s.mint_seat())["seat"]
        .as_str()
        .expect("mint_seat returns a seat")
        .to_string();
    j!(s.claim_region_inner(claim_with(&seat), true));

    let report = j!(s.claim_report());
    let claims = report["claims"].as_array().expect("claims array");
    assert_eq!(claims.len(), 1, "the claim was made, so the board shows it");

    let liveness = claims[0]["liveness"]
        .as_str()
        .expect("liveness is reported");
    assert_eq!(
        liveness, "unknown",
        "THE DEFECT: this said `live`, and would have said it for every seat this server ever \
         minted, forever — `gone` was unreachable, so the field could never produce its \
         negative value while presenting its positive one as information"
    );
}

/// `unknown` must not be quietly upgraded into a ghost. A ghost is excluded from
/// overlaps; an unknown is not, because taking work somebody is actively doing
/// is the expensive mistake.
#[tokio::test]
async fn an_unobservable_seat_is_not_reported_as_a_ghost() {
    let s = shared_service().await;

    let seat = j!(s.mint_seat())["seat"]
        .as_str()
        .expect("mint_seat returns a seat")
        .to_string();
    j!(s.claim_region_inner(claim_with(&seat), true));

    let report = j!(s.claim_report());
    assert!(
        report["stale"].as_array().expect("stale array").is_empty(),
        "a seat we cannot observe is not a seat we know has departed"
    );
}

/// The advisory the tool returns has now been wrong in both directions, so it is
/// computed from what the process can observe. Pin that it says so.
#[tokio::test]
async fn mint_seat_says_it_cannot_observe_this_session() {
    let s = shared_service().await;

    let minted = j!(s.mint_seat());
    let said = minted["liveness_of_this_seat"]
        .as_str()
        .expect("the advisory is present")
        .to_string();

    assert!(
        said.contains("UNOBSERVABLE"),
        "a shared server must not promise liveness it cannot compute, got: {said}"
    );
    assert!(
        said.contains("release_claim"),
        "and it must name what DOES clear the claim, since nothing else will, got: {said}"
    );
}
