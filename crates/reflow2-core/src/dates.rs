//! DATES — the small amount of calendar arithmetic a design brain needs, and
//! not one line more.
//!
//! reflow2 has never parsed a date. Every timestamp it holds is a string it
//! stores and hands back, which was enough for as long as nothing had to know
//! how OLD a claim was. `cap:claims-carry-their-age` needs exactly that and
//! nothing else, so this module converts `YYYY-MM-DD` to a day number and
//! subtracts. No timezones, no times, no formatting, no dependency.
//!
//! **Why not `chrono`.** The core is deliberately dependency-light, and the
//! whole requirement is one subtraction. The civil-from-days conversion below
//! is a well-known closed form (Howard Hinnant's `days_from_civil`), exact for
//! every date in the proleptic Gregorian calendar, and it is fifteen lines.
//!
//! **Why `today` is a parameter everywhere in here.** A function that reads the
//! clock cannot be tested, only observed. The clock is read once, at the edge,
//! by [`today_utc`]; everything that decides anything takes the date as an
//! argument. That is the same split the rest of the core makes between the
//! store and the computation.

/// Days from the civil date 1970-01-01 to `y-m-d`, negative before it.
///
/// Exact for any proleptic Gregorian date. Adapted from Howard Hinnant's
/// `days_from_civil`, which is the canonical closed form for this.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    // Shift the year so March is month 1 — that puts the leap day at the END of
    // the year, which is what makes the whole thing branch-free.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // March = 0
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Parse a leading `YYYY-MM-DD` into a day number, or `None`.
///
/// TOLERANT OF A TRAILING TIME ON PURPOSE — `2026-08-16` and
/// `2026-08-16T09:30:00Z` both parse, because the graph holds both spellings
/// and a reader that understood only one would report a date it was given as
/// unparseable. Anything else returns `None`, which every caller renders as
/// "no age stated" rather than as an error: a malformed date is a reason to say
/// less, never a reason to fail a search.
pub fn parse_day(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    if b.len() > 10 && b[10] != b'T' && b[10] != b' ' {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> { s.get(from..to)?.parse::<i64>().ok() };
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Whole days from `from` to `to`. `None` if either fails to parse.
///
/// Signed deliberately: a fact dated in the future is a real thing to be able
/// to say (a `forecast`, or a typo), and clamping it to zero would report it as
/// current — which is the exact misreading this module exists to prevent.
pub fn days_between(from: &str, to: &str) -> Option<i64> {
    Some(parse_day(to)? - parse_day(from)?)
}

/// Today, UTC, as `YYYY-MM-DD`. **The only clock read in this module** — call
/// it at the edge and pass the result down, so everything that DECIDES stays
/// testable.
pub fn today_utc() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86_400)
        .unwrap_or(0);
    civil_from_days(days)
}

/// Inverse of [`days_from_civil`] — day number back to `YYYY-MM-DD`.
fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March = 0
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// How old a time-bounded claim is, and whether it has lapsed.
///
/// Every field is optional or false-by-default so that a node with no dates
/// produces a value that serialises to nothing at all — the ordinary hit is
/// byte-identical to what it was before ages existed.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ClaimAge {
    /// The date the claim became true, verbatim as the graph holds it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
    /// Whole days from `as_of` to today. Negative means dated in the future.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_days: Option<i64>,
    /// `valid_to` is set and has passed — the claim is over, and narrating it
    /// as current is the failure `req:a-dated-observation-does-not-read-as-standing-doctrine`
    /// names.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub expired: bool,
}

impl ClaimAge {
    /// True when nothing is worth saying — no dates, or none that parsed.
    pub fn is_silent(&self) -> bool {
        self.as_of.is_none() && self.age_days.is_none() && !self.expired
    }
}

/// Read a node's date properties into a [`ClaimAge`], relative to `today`.
///
/// PREFERS NOTHING AND INVENTS NOTHING. A fact with no `valid_from` gets no
/// `as_of`; a `valid_from` that will not parse still yields `as_of` (the graph
/// said it, so it is reported) but no `age_days` (nobody can compute it). The
/// two are separate fields precisely so "dated, but I cannot read the date" is
/// sayable — collapsing them would make a malformed date indistinguishable from
/// no date at all.
pub fn claim_age(
    props: &std::collections::HashMap<String, crate::foundation::core::Value>,
    today: &str,
) -> ClaimAge {
    let get = |k: &str| -> Option<String> {
        props
            .get(k)
            .and_then(crate::foundation::core::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let as_of = get("valid_from");
    let age_days = as_of.as_deref().and_then(|f| days_between(f, today));
    // A `valid_to` that has not parsed is NOT expired — an unreadable date is
    // not evidence that the claim lapsed, and guessing would retire live facts.
    let expired = get("valid_to")
        .and_then(|t| days_between(&t, today))
        .is_some_and(|d| d > 0);
    ClaimAge {
        as_of,
        age_days,
        expired,
    }
}
