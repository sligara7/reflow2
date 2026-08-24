//! A dated claim reads back with its age — `cap:claims-carry-their-age` part 3,
//! and the reader that `dec:how-should-a-temporal-fact-carry-its-date-when-there-is-no-epoch`
//! requires before the property is allowed to exist at all.
//!
//! THE POINT OF THE WHOLE FILE: 112 of 205 TemporalFacts carried a `valid_from`
//! the schema did not declare and NOTHING read. Declaring it without a reader
//! would have changed the schema and left the corpus exactly as inert as it was
//! — so these cases are the evidence that the property now does something.
//!
//! `today` is a parameter in every case here on purpose. A test that read the
//! clock would pass today and fail in a fortnight, which is the one failure
//! mode a module about staleness must not have.

use reflow2_core::Value;
use reflow2_core::dates::{ClaimAge, claim_age, days_between, parse_day};
use std::collections::HashMap;

fn props(pairs: &[(&str, &str)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
        .collect()
}

// ---------------------------------------------------------------- the arithmetic

#[test]
fn the_day_number_is_right_at_the_anchors_that_catch_off_by_one() {
    // The epoch itself, and the day either side of it — the three values a
    // sign error or an inclusive/exclusive slip gets wrong first.
    assert_eq!(parse_day("1970-01-01"), Some(0));
    assert_eq!(parse_day("1970-01-02"), Some(1));
    assert_eq!(parse_day("1969-12-31"), Some(-1));
}

#[test]
fn leap_years_follow_the_gregorian_rule_including_the_century_exceptions() {
    // 2000 was a leap year, 1900 was NOT, and 2024 was. A naive `% 4` gets
    // 1900 wrong; a naive `% 100` gets 2000 wrong. Both are caught here.
    assert_eq!(days_between("2000-02-28", "2000-03-01"), Some(2)); // leap
    assert_eq!(days_between("1900-02-28", "1900-03-01"), Some(1)); // not leap
    assert_eq!(days_between("2024-02-28", "2024-03-01"), Some(2)); // leap
    assert_eq!(days_between("2023-02-28", "2023-03-01"), Some(1)); // not leap
}

#[test]
fn a_span_across_a_year_boundary_is_counted_in_whole_days() {
    assert_eq!(days_between("2025-12-31", "2026-01-01"), Some(1));
    assert_eq!(days_between("2026-01-01", "2026-12-31"), Some(364));
    assert_eq!(days_between("2026-08-16", "2026-08-16"), Some(0));
}

#[test]
fn a_date_with_a_time_on_it_still_parses_because_the_graph_holds_both_spellings() {
    // Refusing these would report a date the design DID state as unreadable.
    assert_eq!(parse_day("2026-08-16T09:30:00Z"), parse_day("2026-08-16"));
    assert_eq!(parse_day("2026-08-16 09:30"), parse_day("2026-08-16"));
}

#[test]
fn anything_that_is_not_a_date_declines_rather_than_guessing() {
    for bad in [
        "",
        "2026",
        "2026-08",
        "16-08-2026",  // the other convention — must NOT be silently accepted
        "2026/08/16",  // right order, wrong separators
        "2026-13-01",  // month out of range
        "2026-08-32",  // day out of range
        "2026-08-16X", // trailing junk that is not a time separator
        "not-a-date",
        "last Tuesday",
    ] {
        assert_eq!(parse_day(bad), None, "should not have parsed: {bad:?}");
    }
}

// ---------------------------------------------------------------- what a hit says

#[test]
fn a_dated_fact_reports_when_it_was_true_and_how_old_that_is() {
    let age = claim_age(&props(&[("valid_from", "2026-07-05")]), "2026-08-16");
    assert_eq!(age.as_of.as_deref(), Some("2026-07-05"));
    assert_eq!(age.age_days, Some(42));
    assert!(!age.expired);
}

#[test]
fn an_undated_node_says_nothing_at_all() {
    // The ordinary hit — a Requirement, a Component — must be byte-identical
    // to what it was before ages existed, or every caller pays for a feature
    // about facts.
    let age = claim_age(&props(&[("name", "some component")]), "2026-08-16");
    assert_eq!(age, ClaimAge::default());
    assert!(age.is_silent());
    assert_eq!(serde_json::to_string(&age).unwrap(), "{}");
}

#[test]
fn a_date_nobody_can_read_is_still_reported_but_carries_no_age() {
    // THE TWO FIELDS EXIST SEPARATELY FOR EXACTLY THIS. The graph said
    // something; we repeat it. Nobody can subtract it; we do not pretend to.
    // Collapsing these would make "dated, unreadable" identical to "undated".
    let age = claim_age(&props(&[("valid_from", "sometime in July")]), "2026-08-16");
    assert_eq!(age.as_of.as_deref(), Some("sometime in July"));
    assert_eq!(age.age_days, None);
    assert!(!age.is_silent());
}

#[test]
fn an_empty_date_string_is_treated_as_no_date_rather_than_as_a_claim() {
    let age = claim_age(&props(&[("valid_from", "")]), "2026-08-16");
    assert!(age.is_silent());
}

#[test]
fn a_claim_dated_in_the_future_reports_a_negative_age_rather_than_zero() {
    // Clamping would render a forecast — or a typo — as "current", which is
    // the precise misreading this whole capability exists to stop.
    let age = claim_age(&props(&[("valid_from", "2026-12-01")]), "2026-08-16");
    assert_eq!(age.age_days, Some(-107));
}

// ---------------------------------------------------------------- expiry

#[test]
fn a_claim_whose_end_date_has_passed_is_marked_expired() {
    let age = claim_age(
        &props(&[("valid_from", "2026-01-01"), ("valid_to", "2026-06-30")]),
        "2026-08-16",
    );
    assert!(age.expired);
    assert_eq!(age.as_of.as_deref(), Some("2026-01-01"));
}

#[test]
fn a_claim_whose_end_date_has_not_yet_arrived_is_not_expired() {
    let age = claim_age(
        &props(&[("valid_from", "2026-01-01"), ("valid_to", "2026-12-31")]),
        "2026-08-16",
    );
    assert!(!age.expired);
}

#[test]
fn a_claim_expiring_today_is_not_yet_expired() {
    // The boundary. `valid_to` is the last day the claim holds, so it lapses
    // the day AFTER — off by one here would retire a live fact a day early.
    let today = "2026-08-16";
    assert!(!claim_age(&props(&[("valid_to", today)]), today).expired);
    assert!(claim_age(&props(&[("valid_to", "2026-08-15")]), today).expired);
}

#[test]
fn an_unreadable_end_date_never_retires_a_live_claim() {
    // A COUNTERWEIGHT, and the one that matters most: guessing would silently
    // mark real facts dead. An unreadable date is not evidence of anything.
    let age = claim_age(
        &props(&[("valid_from", "2026-01-01"), ("valid_to", "whenever")]),
        "2026-08-16",
    );
    assert!(!age.expired);
    assert_eq!(age.age_days, Some(227));
}

// ---------------------------------------------------------------- the serialised shape

#[test]
fn only_what_is_known_is_serialised() {
    let dated = claim_age(&props(&[("valid_from", "2026-08-01")]), "2026-08-16");
    let json = serde_json::to_string(&dated).unwrap();
    assert!(json.contains("\"as_of\":\"2026-08-01\""), "{json}");
    assert!(json.contains("\"age_days\":15"), "{json}");
    // `expired` is false, and false must not appear — an ordinary dated fact
    // should not read as though expiry had been considered and ruled out.
    assert!(!json.contains("expired"), "{json}");
}
