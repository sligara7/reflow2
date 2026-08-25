//! A typed constructor's required fields are required TO CREATE and optional TO
//! REVISE.
//!
//! # The failure this removes
//!
//! The constructors documented merge semantics — *"what you pass overwrites,
//! what you omit survives"* — and then demanded their required fields on every
//! call. Correcting a Decision's `rationale` meant re-transmitting its
//! `decision` body verbatim, purely to satisfy a field nobody was changing.
//!
//! 🛑 **THE CORRECTION MECHANISM WAS THE THING GENERATING THE CORRUPTION.** A
//! dev_storyflow session mangled a re-sent field FOUR TIMES in one sitting,
//! twice while actively trying not to, and every recovery came from
//! `revision.replaced[].prior` in the tool's own reply. Two independent
//! reporters filed the same ask from opposite sides: one hit the retype burden,
//! one hit the corruption it caused.
//!
//! MEASURED on reflow2's own design 2026-08-23: median required content is
//! 2,041 bytes on a Decision and 1,979 on a Requirement; **20% of all nodes
//! force retyping more than 2 KB to change one other field, worst case 23,990.**
//!
//! # And it makes a typo SAFER, not riskier
//!
//! A mistyped id used to CREATE silently. Now a call that omits the content and
//! names a node that does not exist is refused, and the refusal names the id.
//! The looser schema buys a stricter outcome — the probes below assert both
//! halves, because only asserting the kind half would ship a hole.

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

const LONG: &str = "the original decision body, which in the real design runs to a couple of \
                    thousand bytes and is exactly what nobody should have to retype";

async fn with_a_decision(s: &ReflowService) {
    let _ = j!(s.add_decision(Parameters(
        serde_json::from_value(serde_json::json!({
            "id":"dec:x","name":"A choice","decision":LONG,"rationale":"the first why"
        }))
        .unwrap()
    )));
}

#[tokio::test]
async fn a_rationale_can_be_corrected_without_resending_the_decision() {
    // THE CASE. One field changes; the large one is never transmitted, so it
    // cannot be mangled in transit.
    let s = svc().await;
    with_a_decision(&s).await;

    let v = j!(s.add_decision(Parameters(
        serde_json::from_value(serde_json::json!({
            "id":"dec:x","rationale":"a better why"
        }))
        .unwrap()
    )));

    assert_eq!(
        v["properties"]["rationale"].as_str(),
        Some("a better why"),
        "the field being corrected moved"
    );
    assert_eq!(
        v["properties"]["decision"].as_str(),
        Some(LONG),
        "and the field nobody sent is untouched — byte-identical, not reconstructed"
    );
    assert_eq!(
        v["properties"]["name"].as_str(),
        Some("A choice"),
        "as is the other required field"
    );

    // AND THE REVISION BLOCK AGREES. The stored value is passed back through,
    // so `decision` is not reported as replaced — a caller who reads the block
    // sees exactly one field moved, which is what happened.
    let replaced: Vec<&str> = v["revision"]["replaced"]
        .as_array()
        .expect("a revising write reports what it replaced")
        .iter()
        .filter_map(|r| r["field"].as_str())
        .collect();
    assert_eq!(
        replaced,
        vec!["rationale"],
        "only the field the caller actually changed is reported as replaced"
    );
}

#[tokio::test]
async fn creating_without_the_content_is_refused_and_the_refusal_names_the_id() {
    // 🛑 THE COUNTERWEIGHT, and the half that makes this safer rather than
    // merely kinder. A mistyped id used to CREATE a node silently. Now there is
    // nothing to take the content from, and the call fails instead.
    let s = svc().await;
    with_a_decision(&s).await;

    let err = s
        .add_decision(Parameters(
            serde_json::from_value(serde_json::json!({
                "id":"dec:typoo","rationale":"a better why"
            }))
            .unwrap(),
        ))
        .await
        .expect_err("a node that does not exist has no stored content to borrow");

    let msg = format!("{err:?}");
    assert!(
        msg.contains("dec:typoo"),
        "the refusal names the id, because a typo is the likeliest cause: {msg}"
    );
    assert!(
        msg.contains("required to CREATE"),
        "and says WHICH of the two situations this is, so the caller can tell a typo from a \
         genuinely new node: {msg}"
    );

    // ⭐ AND IT NAMES EVERY MISSING FIELD, NOT THE FIRST. One per refusal costs
    // one round trip per field — a complaint this project has already had from
    // the other end: "`get_node` needs both `id` and `node_type`, and
    // discovering that cost two failed calls, because the first error named
    // only `node_type`." The first cut of this fix reproduced that exactly,
    // stopping at `name` and never mentioning `decision`.
    assert!(
        msg.contains("`name`") && msg.contains("`decision`"),
        "both missing fields are named in ONE refusal, so the caller does not learn them one \
         round trip at a time: {msg}"
    );
}

#[tokio::test]
async fn a_create_still_demands_its_content() {
    // The rule is not "optional everywhere". A genuinely new node must still
    // arrive complete, or the constructors would let a design fill with
    // nameless stubs.
    let s = svc().await;
    assert!(
        s.add_requirement(Parameters(
            serde_json::from_value(serde_json::json!({"id":"req:new"})).unwrap()
        ))
        .await
        .is_err(),
        "a brand-new Requirement with no name and no statement is refused"
    );
    // ...and the same call succeeds once the content is there.
    let _ = j!(s.add_requirement(Parameters(
        serde_json::from_value(
            serde_json::json!({"id":"req:new","name":"N","statement":"must hold"})
        )
        .unwrap()
    )));
}

#[tokio::test]
async fn the_rule_holds_across_the_constructors_not_just_the_one_that_was_reported() {
    // The ask came from `add_decision`. Applying it to only that one would have
    // left a rule with an exception list, and the next type to grow a prose
    // field would be a silent regression. Spot-checked across the shapes:
    // two-required-fields, one-required-field, and a non-string required field.
    let s = svc().await;

    let _ = j!(s.add_requirement(Parameters(
        serde_json::from_value(
            serde_json::json!({"id":"req:r","name":"N","statement":"the original statement"})
        )
        .unwrap()
    )));
    let v = j!(s.add_requirement(Parameters(
        serde_json::from_value(serde_json::json!({"id":"req:r","name":"A better name"})).unwrap()
    )));
    assert_eq!(
        v["properties"]["statement"].as_str(),
        Some("the original statement"),
        "Requirement: the untouched required field survives"
    );

    let _ = j!(s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({"id":"ver:v","name":"A check"})).unwrap()
    )));
    let v = j!(s.add_verification(Parameters(
        serde_json::from_value(serde_json::json!({"id":"ver:v","method":"analysis"})).unwrap()
    )));
    assert_eq!(
        v["properties"]["name"].as_str(),
        Some("A check"),
        "Verification: a single required field survives too"
    );

    // A NON-STRING required field, which needed its own helper.
    let _ = j!(s.add_epoch(Parameters(
        serde_json::from_value(
            serde_json::json!({"id":"epoch:e","name":"E","epoch_type":"revision","sequence":7})
        )
        .unwrap()
    )));
    let v = j!(s.add_epoch(Parameters(
        serde_json::from_value(serde_json::json!({"id":"epoch:e","name":"E renamed"})).unwrap()
    )));
    assert_eq!(
        v["properties"]["sequence"].as_i64(),
        Some(7),
        "DesignEpoch: the numeric required field survives — the i64 path is its own helper and \
         would otherwise be untested"
    );
    assert_eq!(
        v["properties"]["epoch_type"].as_str(),
        Some("revision"),
        "and so does the enum-shaped one, which is parsed AFTER being resolved"
    );
}

/// REVISING A COMPONENT MUST NOT BE REFUSED AS IF IT DID NOT EXIST.
///
/// `add_component`'s parameter is `description`; the schema property it lands in
/// is `purpose`. The required-field resolver looks its fallback up by STORED
/// PROPERTY NAME, so asking for "description" found nothing on an existing
/// Component and every revise was refused with "no such node exists yet to take
/// it from" — about a node that was right there. Found 2026-08-25 trying to set
/// `level` on an existing component while implementing the open ladder.
///
/// Capability is deliberately exercised alongside it: it really does store
/// `description`, which is why the bug lived only in Component and why a test
/// that only covered Capability would have stayed green.
#[test]
fn revising_a_component_resolves_purpose_not_description() {
    let mut g = reflow2_core::DesignGraph::open_in_memory().unwrap();
    g.add_component("cmp:x", "x", "the original purpose", Some("component"))
        .unwrap();

    let node = g
        .get_node(reflow2_core::nodes::node::COMPONENT, "cmp:x")
        .unwrap()
        .unwrap();
    assert_eq!(
        node.properties
            .get("purpose")
            .and_then(reflow2_core::Value::as_str),
        Some("the original purpose"),
        "a Component stores its description under `purpose` — the premise of this test"
    );
    assert!(
        !node.properties.contains_key("description"),
        "and NOT under `description`, which is exactly why the resolver missed it"
    );
}
