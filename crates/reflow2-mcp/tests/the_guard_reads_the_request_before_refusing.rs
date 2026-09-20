//! The near-duplicate guard reads the request before refusing: a near-match
//! the write itself names as a relation target is never a duplicate, and the
//! golden-thread pairs measured on 2026-09-16 (Capability↔Component,
//! Requirement↔Component, Capability↔DesignRule, Capability↔Constraint) are
//! reported, not refused.
//!
//! Root cause (xrt-demo F8 + bhome's 4th-of-4, 2026-09-16): seven cross-type
//! refusals in one genesis, all thread pairs, none a duplicate — three of them
//! a capability refused against the component passed as its own
//! `allocated_to`. The August repair limited the prescribed set to pairs with
//! evidence and said it was waiting for this count; 19 refusals across three
//! designs and 0 duplicates is the count
//! (`fact:root-cause-the-guard-still-refuses-thread-pairs-…`).
//!
//! Written before the fix and observed failing: the first two tests were
//! refused against the unfixed guard; the last (same-type) passed before and
//! after, which is what shows the change narrows the guard rather than
//! blinding it.

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

/// One idea in one vocabulary on both sides, on purpose: the function side
/// and the part side of a focusing mirror genuinely share their words.
const WORDS: &str = "Focus the monochromatic beam onto the sample with a toroidal mirror at \
                     3 mrad so the spot at 45 m is under 30 by 5 microns.";

fn component(id: &str, name: &str) -> ComponentReq {
    ComponentReq {
        id: id.into(),
        name: Some(name.into()),
        description: Some(WORDS.into()),
        kind: None,
        level: None,
        distinct_from: None,
        tier: None,
        status: None,
    }
}

fn capability(id: &str, name: &str, allocated_to: Option<&str>) -> CapabilityReq {
    CapabilityReq {
        id: id.into(),
        name: Some(name.into()),
        description: Some(WORDS.into()),
        status: None,
        distinct_from: None,
        tier: None,
        is_entry_point: None,
        is_exit_point: None,
        satisfies: None,
        allocated_to: allocated_to.map(Into::into),
    }
}

#[tokio::test]
async fn a_capability_is_not_refused_against_the_part_it_names_as_its_own_allocation() {
    let s = svc().await;
    j!(s.add_component(Parameters(component(
        "cmp:focusing-mirror",
        "Toroidal focusing mirror"
    ))));
    let out = j!(s.add_capability(Parameters(capability(
        "cap:focus-at-sample",
        "Focus the beam at the sample",
        Some("cmp:focusing-mirror"),
    ))));
    // Reported, never hidden: the match is still in the reply.
    let near = out["search_first"]["near_matches"].to_string();
    assert!(
        near.contains("cmp:focusing-mirror"),
        "the near-match is still reported so a later reader can find it: {out:?}"
    );
}

#[tokio::test]
async fn a_part_that_meets_a_requirement_reads_like_it_and_is_not_refused() {
    let s = svc().await;
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:spot-at-sample".into(),
        name: Some("Spot at the sample under 30 by 5 microns".into()),
        statement: Some(WORDS.into()),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
        kind: None,
    })));
    let out = j!(s.add_component(Parameters(component(
        "cmp:focusing-mirror",
        "Toroidal focusing mirror"
    ))));
    assert!(
        out["search_first"]["near_matches"]
            .to_string()
            .contains("req:spot-at-sample"),
        "reported, not refused: {out:?}"
    );
}

#[tokio::test]
async fn a_second_part_with_the_same_words_is_still_refused() {
    // The counterweight: same type, same words, nothing declared — the case
    // the guard exists for. Unchanged by this repair.
    let s = svc().await;
    j!(s.add_component(Parameters(component(
        "cmp:focusing-mirror",
        "Toroidal focusing mirror"
    ))));
    let err = s
        .add_component(Parameters(component(
            "cmp:focusing-mirror-2",
            "Toroidal focusing mirror (again)",
        )))
        .await
        .expect_err("a same-type near duplicate is refused");
    assert!(
        err.to_string().contains("distinct_from"),
        "and the refusal still hands over the escape: {err}"
    );
}
