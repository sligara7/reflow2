//! `decision_settled`: a record-axis change type for the commonest design
//! event, added 2026-09-18 after flo2 reported none of the values named it.

use reflow2_core::ChangeType;

#[test]
fn decision_settled_round_trips_through_its_string_form() {
    assert_eq!(ChangeType::DecisionSettled.as_str(), "decision_settled");
    let parsed: ChangeType = serde_json::from_str("\"decision_settled\"").expect("parses");
    assert_eq!(parsed, ChangeType::DecisionSettled);
}
