//! A near-match at a PRESCRIBED LAYER of the capture loop is reported, not refused.
//!
//! # The count that reopened this
//!
//! `chg:a-cross-type-near-match-is-named-as-a-layer` (2026-08-29) shipped the
//! conservative half of this repair — reword the refusal when the types differ
//! — and REJECTED the behavioural half for exactly one stated reason:
//!
//! > Nobody has counted how often a cross-type hit IS a real duplicate, so
//! > suppressing the check would act on an unmeasured assumption.
//!
//! `fact:twelve-near-match-refusals-across-two-designs-and-not-one-was-a-duplicate`
//! is that count, taken 2026-08-31 from two field reports written by sessions
//! that could not see each other, on designs with nothing in common (a
//! shipping-container aquaponics build and a children's music game):
//!
//! * **12 refusals in the field. 12 judged distinct. 0 real duplicates.**
//! * **3 more incurred by the triage session itself**, writing the very
//!   decision that acts on the count — and those three are the only ones not
//!   self-reported by an agent grading its own work.
//!
//! Both reports independently named the same pairings. Anthony chose the
//! behavioural half on 2026-08-31.
//!
//! # Why the false-positive rate is not the whole argument
//!
//! It would be, if the guard were still doing its job. It is not. bhome, at its
//! eighth refusal:
//!
//! > I have stopped treating each as noteworthy and started passing
//! > `distinct_from` pre-emptively, which is itself the concerning outcome: a
//! > guard that is routinely pre-empted has stopped being a check.
//!
//! ⭐ SO THE SILENT MISS THAT OPTION ① WAS REJECTED TO AVOID HAD ALREADY
//! ARRIVED BY ANOTHER ROUTE — an agent that pre-declares `distinct_from` before
//! every write is not being checked at all, and unlike a suppression it leaves
//! no record of the trade.
//!
//! # What is suppressed, and what is emphatically not
//!
//! 🛑 THE REFUSAL IS SUPPRESSED. THE REPORT IS NOT. A prescribed-layer match
//! still comes back in `search_first.near_matches` exactly as before, so the
//! agent still sees what it matched and a later reader can still find it. This
//! is the difference between narrowing a guard and blinding it, and the first
//! test below pins it — without that assertion this change would be
//! indistinguishable from deleting the check.
//!
//! The suppressed set is the pairs with EVIDENCE behind them, not every pair
//! that could be argued for. Anything else still refuses, which is what the
//! last two tests exist to hold.

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

/// One idea in one vocabulary, used verbatim on both sides on purpose: the
/// layered capture is precisely the case where the words genuinely DO match.
const IDEA: &str = "A lost reading heals itself because the outdoor unit sends cumulative totals \
                    rather than deltas, so a dropped packet costs nothing.";

fn requirement(id: &str, name: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(name.into()),
        statement: Some(IDEA.into()),
        distinct_from: None,
    }
}

fn capability(id: &str, name: &str) -> CapabilityReq {
    CapabilityReq {
        id: id.into(),
        name: Some(name.into()),
        description: Some(IDEA.into()),
        status: None,
        distinct_from: None,
    }
}

fn decision(id: &str, name: &str, kind: Option<&str>) -> DecisionReq {
    DecisionReq {
        id: id.into(),
        name: Some(name.into()),
        decision: Some(IDEA.into()),
        rationale: None,
        distinct_from: None,
        kind: kind.map(Into::into),
        related_to: None,
        no_relation_note: Some("no honest relation; this is a test fixture".into()),
    }
}

/// ⭐ THE MOST-MEASURED PAIRING: 4 of bhome's 8 refusals and 1 of musicjug's 4.
/// A Capability and the Requirement it satisfies are the golden thread, and the
/// served instructions ask for both.
#[tokio::test]
async fn a_capability_and_the_requirement_it_satisfies_is_not_refused() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(
        "req:a-lost-reading-heals-itself",
        "A lost reading heals itself",
    )))
    .await
    .expect("the requirement lands");

    let out = j!(s.add_capability(Parameters(capability(
        "cap:cumulative-totals",
        "Send cumulative totals",
    ))));

    // 🛑 AND THE MATCH IS STILL REPORTED. Suppressing the refusal must not
    // suppress the evidence — without this assertion, narrowing the guard and
    // deleting it would look identical from outside.
    let reported = serde_json::to_string(&out).expect("serialisable");
    assert!(
        reported.contains("req:a-lost-reading-heals-itself"),
        "a prescribed-layer match must still be REPORTED in search_first even though it no \
         longer refuses: {reported}"
    );
}

/// The promotion path, and the only measured SAME-TYPE false positive: step 5
/// of the brainstorm skill turns an `exploratory` Decision into a settled one,
/// so the idea and its answer are supposed to read alike. 1 of bhome's 8 and 2
/// of musicjug's 4.
#[tokio::test]
async fn an_idea_and_the_decision_that_settles_it_is_not_refused() {
    let s = svc().await;
    s.add_decision(Parameters(decision(
        "dec:idea-cumulative-or-deltas",
        "OPEN — cumulative totals or deltas?",
        Some("exploratory"),
    )))
    .await
    .expect("the idea lands");

    s.add_decision(Parameters(decision(
        "dec:cumulative-totals",
        "DECIDED — cumulative totals",
        Some("choice"),
    )))
    .await
    .expect("the decision that settles an idea is not a duplicate of the idea");
}

/// The pairing this triage session incurred three times, writing the decision
/// that acts on the count. A measurement and the Decision it informs are
/// supposed to read alike.
#[tokio::test]
async fn a_decision_and_the_measurement_it_acts_on_is_not_refused() {
    let s = svc().await;
    s.add_project(Parameters(
        serde_json::from_value(serde_json::json!({ "id": "proj:test", "name": "Test" }))
            .expect("project request"),
    ))
    .await
    .expect("the project lands");

    s.create_node(Parameters(CreateNodeReq {
        node_type: "TemporalFact".into(),
        id: "fact:deltas-lose-a-dropped-packet".into(),
        props: Some(
            serde_json::json!({
                "statement": IDEA,
                "subject_id": "proj:test",
                "basis": "measured",
                "valid_from": "2026-08-31",
            })
            .as_object()
            .expect("object")
            .clone(),
        ),
        expected_content_hash: None,
    }))
    .await
    .expect("the measurement lands");

    s.add_decision(Parameters(decision(
        "dec:send-cumulative-totals",
        "DECIDED — send cumulative totals",
        Some("choice"),
    )))
    .await
    .expect("a decision is not a duplicate of the measurement that motivates it");
}

/// 🛑 THE COUNTERWEIGHT THAT MATTERS MOST. The guard's whole purpose is the
/// same-type duplicate — the same record written twice in different words. If
/// this ever passes, the change above stopped being a narrowing and became a
/// deletion.
#[tokio::test]
async fn a_same_type_duplicate_is_still_refused() {
    let s = svc().await;
    s.add_requirement(Parameters(requirement(
        "req:a-lost-reading-heals-itself",
        "A lost reading heals itself",
    )))
    .await
    .expect("the first requirement lands");

    let err = s
        .add_requirement(Parameters(requirement(
            "req:cumulative-not-deltas",
            "Cumulative totals, not deltas",
        )))
        .await
        .expect_err("a same-type near duplicate is still refused");

    assert!(
        err.message.to_string().contains("SHARPEN"),
        "the same-type route must still be offered: {}",
        err.message
    );
}

/// 🛑 THE SECOND COUNTERWEIGHT. Only the pairings with evidence behind them are
/// suppressed. A cross-type pair nobody has measured still refuses, and still
/// refuses with the LAYER wording rather than the duplicate wording — so the
/// 2026-08-29 repair survives underneath this one rather than being replaced
/// by it.
#[tokio::test]
async fn an_unmeasured_cross_type_pair_still_refuses_with_the_layer_wording() {
    let s = svc().await;
    s.add_component(Parameters(ComponentReq {
        id: "cmp:outdoor-unit".into(),
        name: Some("Outdoor unit".into()),
        description: Some(IDEA.into()),
        level: None,
        distinct_from: None,
    }))
    .await
    .expect("the component lands");

    let err = s
        .add_requirement(Parameters(requirement(
            "req:a-lost-reading-heals-itself",
            "A lost reading heals itself",
        )))
        .await
        .expect_err("Requirement against Component is not a measured layer pair, so it refuses");

    let msg = err.message.to_string();
    assert!(
        msg.to_lowercase().contains("different kind of record"),
        "an unmeasured cross-type match keeps the 2026-08-29 layer wording: {msg}"
    );
}
