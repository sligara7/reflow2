//! A discontinued node SAYS SO to whoever reads it — not only to the detectors.
//!
//! # The defect, and it cost a wrong recommendation to the owner's face
//!
//! `dec:idea-discontinued-is-a-first-class-state` (accepted 2026-08-11) settled
//! the retirement mechanism: **a Decision `OBSOLETES` what it withdrew**, and
//! four readers consult it — the three capability detectors fall silent, and
//! delivery arithmetic stops counting a discontinued capability as a satisfier.
//! That decision's own headline was *"this is the first READER either retirement
//! edge has ever had"*, against `dec:one-retire-edge`'s finding that *"retiring
//! something marks it and changes nothing"*.
//!
//! **All four readers are COMPUTATIONS. None of them is a READ.**
//!
//! Measured 2026-08-12 on reflow2's own design. `cap:content-store` was
//! discontinued on 2026-08-09 (`dec:the-content-store-is-discontinued`,
//! accepted, `OBSOLETES` drawn correctly) and its code deleted. A session then
//! ran `scan_nodes` for `Capability` and got back:
//!
//! ```text
//! {"name":"Store bytes beside the design and point at them by content hash",
//!  "node_id":"cap:content-store","node_type":"Capability","status":"realized"}
//! ```
//!
//! `realized`, for code that does not exist. Nothing in the reply carried the
//! retirement. The session read that, believed the store was live and missing
//! only a surface, and **recommended to Anthony that he build a surface for a
//! feature he had personally deleted three days earlier.**
//!
//! ⇒ The graph was right, the detectors were right, and the reader was told
//! nothing. A fact the server HAS and declines to give is the same class as
//! `get_node`'s wrong-type `null` next door: *"no such TYPE" and "no such node"
//! are different facts and must not share one reply.* Here, `realized` and
//! `realized-but-withdrawn` were sharing one reply.
//!
//! # Why a derived field and not a status value
//!
//! The status property is NOT touched. `dec:idea-does-a-capability-need-a-cancelled-state`
//! is still open and still Anthony's, and writing a terminal status here would
//! settle it by implementation. `discontinued` is DERIVED from the edge on every
//! read, exactly like the detectors derive it — so the two can never disagree,
//! and the open question stays open.
//!
//! # Present and false, never absent
//!
//! The field is emitted on every node, `false` included. "This node is not
//! discontinued" and "this build does not report discontinuation" must not be
//! the same answer — the rule `severed_containment` and `not_observed_about`
//! already follow, and the one this defect is an instance of.

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

/// A capability that shipped and was then withdrawn by an ACCEPTED decision —
/// the `cap:content-store` shape, reduced.
async fn withdrawn() -> ReflowService {
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_project(Parameters(IdName {
        id: "proj:x".into(),
        name: Some("X".into()),
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:store".into(),
        name: Some("Store bytes beside the design".into()),
        description: Some("Built, shipped, and later withdrawn.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:live".into(),
        name: Some("Something still in use".into()),
        description: Some("Never withdrawn.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    j!(s.add_decision(Parameters(DecisionReq {
        id: "dec:discontinue".into(),
        name: Some("The store is discontinued".into()),
        decision: Some("Built, shipped, correct, and used zero times.".into()),
        rationale: None,
        distinct_from: None,
    })));
    j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:discontinue".into(),
        status: "accepted".into(),
    })));
    j!(s.create_edge(Parameters(CreateEdgeReq {
        edge_type: "OBSOLETES".into(),
        from_type: "Decision".into(),
        from_id: "dec:discontinue".into(),
        to_type: "Capability".into(),
        to_id: "cap:store".into(),
        props: None,
    })));
    s
}

async fn scanned(s: &ReflowService, brief: bool) -> serde_json::Value {
    j!(s.scan_nodes(Parameters(ScanReq {
        level: None,
        node_type: "Capability".into(),
        brief: Some(brief),
        limit: None,
        offset: None,
    })))
}

fn find<'a>(items: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    items
        .as_array()
        .expect("items array")
        .iter()
        .find(|n| n["node_id"] == id)
        .unwrap_or_else(|| panic!("{id} not in the scan"))
}

// 🛑 THE DEFECT CASE, in the tool that produced the wrong recommendation.
#[tokio::test]
async fn a_brief_scan_says_a_withdrawn_capability_is_discontinued() {
    let s = withdrawn().await;
    let env = scanned(&s, true).await;
    let node = find(&env["items"], "cap:store");

    assert_eq!(
        node["discontinued"], true,
        "a brief scan is what a session reads first, and it must carry the retirement: {node}"
    );
}

// The full scan carries it too — a reader who paid for full properties must not
// get LESS of the derived truth than one who asked for the cheap shape.
#[tokio::test]
async fn a_full_scan_says_so_as_well() {
    let s = withdrawn().await;
    let env = scanned(&s, false).await;
    assert_eq!(find(&env["items"], "cap:store")["discontinued"], true);
}

// And the single-node read, which is what a session uses to check one thing.
#[tokio::test]
async fn get_node_says_so() {
    let s = withdrawn().await;
    let got = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Capability".into(),
        id: "cap:store".into(),
    })));
    assert_eq!(got["node"]["discontinued"], true, "got {got}");
}

// 🛑 THE STORED STATUS IS NOT TOUCHED. This is derived, and writing a terminal
// status would settle dec:idea-does-a-capability-need-a-cancelled-state — which
// is open, marked, and Anthony's to decide.
#[tokio::test]
async fn the_stored_status_is_left_exactly_as_it_was() {
    let s = withdrawn().await;
    let got = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Capability".into(),
        id: "cap:store".into(),
    })));
    assert_eq!(
        got["node"]["properties"]["status"], "realized",
        "the property is the record of what was BUILT and must not be rewritten: {got}"
    );
}

// COUNTERWEIGHT, and the one that decides whether the field is usable: a live
// node reports FALSE, present in the reply. "Not discontinued" and "this build
// does not say" must never be one answer.
#[tokio::test]
async fn a_live_node_reports_false_rather_than_omitting_the_field() {
    let s = withdrawn().await;
    let env = scanned(&s, true).await;
    let node = find(&env["items"], "cap:live");

    assert!(
        node.get("discontinued").is_some(),
        "the field must be PRESENT on a live node, not absent: {node}"
    );
    assert_eq!(node["discontinued"], false);
}

// 🛑 ONLY AN ACCEPTED DECISION WITHDRAWS ANYTHING. A proposed decision to
// withdraw something has withdrawn nothing — rule:design-intent-moves-only-on-the-owners-word
// applied to the retirement path. An agent may draw the edge and argue for it.
#[tokio::test]
async fn a_proposed_decision_discontinues_nothing() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:maybe".into(),
        name: Some("Argued about".into()),
        description: Some("An agent proposed withdrawing this.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    j!(s.add_decision(Parameters(DecisionReq {
        id: "dec:proposed".into(),
        name: Some("Should we withdraw it?".into()),
        decision: Some("Not settled.".into()),
        rationale: None,
        distinct_from: None,
    })));
    j!(s.create_edge(Parameters(CreateEdgeReq {
        edge_type: "OBSOLETES".into(),
        from_type: "Decision".into(),
        from_id: "dec:proposed".into(),
        to_type: "Capability".into(),
        to_id: "cap:maybe".into(),
        props: None,
    })));

    let got = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Capability".into(),
        id: "cap:maybe".into(),
    })));
    assert_eq!(
        got["node"]["discontinued"], false,
        "a proposed withdrawal has withdrawn nothing: {got}"
    );
}

// COUNTERWEIGHT: OBSOLETES from something that is NOT a Decision is a different
// relationship — a superseding epoch, say — and this deliberately does not read
// it. Mirrors the guard already in the core's is_discontinued.
#[tokio::test]
async fn obsoleted_by_a_non_decision_is_not_a_discontinuation() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:old".into(),
        name: Some("Superseded by a newer capability".into()),
        description: Some("Replaced, not withdrawn.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    j!(s.add_capability(Parameters(CapabilityReq {
        id: "cap:new".into(),
        name: Some("The replacement".into()),
        description: Some("Took over.".into()),
        status: Some("realized".into()),
        distinct_from: None,
    })));
    j!(s.create_edge(Parameters(CreateEdgeReq {
        edge_type: "OBSOLETES".into(),
        from_type: "Capability".into(),
        from_id: "cap:new".into(),
        to_type: "Capability".into(),
        to_id: "cap:old".into(),
        props: None,
    })));

    let got = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Capability".into(),
        id: "cap:old".into(),
    })));
    assert_eq!(
        got["node"]["discontinued"], false,
        "replacement is not withdrawal, and only a Decision withdraws: {got}"
    );
}

// GENERAL, not capability-only. OBSOLETES is `* -> *` in the schema, and a
// Requirement withdrawn by an accepted decision is exactly as invisible to a
// reader as a capability was.
#[tokio::test]
async fn it_is_not_a_capability_only_field() {
    let s = ReflowService::in_memory().expect("service");
    j!(s.add_requirement(Parameters(RequirementReq {
        id: "req:gone".into(),
        name: Some("A need we stopped having".into()),
        statement: Some("Withdrawn by decision.".into()),
        distinct_from: None,
    })));
    j!(s.add_decision(Parameters(DecisionReq {
        id: "dec:drop".into(),
        name: Some("We no longer need it".into()),
        decision: Some("Withdrawn.".into()),
        rationale: None,
        distinct_from: None,
    })));
    j!(s.set_decision_status(Parameters(SetDecisionStatusReq {
        decision_id: "dec:drop".into(),
        status: "accepted".into(),
    })));
    j!(s.create_edge(Parameters(CreateEdgeReq {
        edge_type: "OBSOLETES".into(),
        from_type: "Decision".into(),
        from_id: "dec:drop".into(),
        to_type: "Requirement".into(),
        to_id: "req:gone".into(),
        props: None,
    })));

    let got = j!(s.get_node(Parameters(TypedIdReq {
        node_type: "Requirement".into(),
        id: "req:gone".into(),
    })));
    assert_eq!(got["node"]["discontinued"], true, "got {got}");
}
