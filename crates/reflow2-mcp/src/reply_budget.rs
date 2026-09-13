//! ONE place a reply is bounded, so a tenth tool cannot quietly miss it.
//!
//! ⭐ WHY THIS MODULE EXISTS AT ALL. Measured 2026-09-13 against reflow2's own
//! 4,403-node design (`fact:the-unbounded-reply-sweep-2026-09-13`): of 181
//! served tools, THREE accepted a reply budget — `detect_gaps`, `reviewed_gaps`
//! and `topic_report` — and each of the three had rolled its own trimming by
//! hand, after somebody met that tool's overflow personally. Nine other read
//! tools returned more than the 30,000-character default with no bound at all:
//!
//! ```text
//!     126,377  confirmation_ledger     4.2x
//!     118,246  describe_schema         3.9x
//!      89,238  invalidated_findings    3.0x
//!      76,155  evidence_report         2.5x
//!      61,669  get_instructions        2.1x
//!      57,449  detect_defects          1.9x
//!      34,319  export_surface          1.1x
//!      30,489  list_skills             1.0x
//!       9-16MB compare_designs         (on two real exports)
//! ```
//!
//! Three fixed one at a time against 178 not fixed is the instance-not-class
//! pattern `fact:the-parallel-batch-class-recurred-because-only-its-instance-was-fixed`
//! already records on this project. A fourth bespoke trimmer would have been a
//! fourth instance, so the trimming lives here instead.
//!
//! 🛑 THE FAILURE THIS STOPS IS NOT A SLOW CALL. It is the CLIENT refusing the
//! reply, at which point the session sees a wall of harness text and reflow2
//! never gets to suggest narrowing. A reader who receives nothing cannot be
//! told to narrow their question.
//!
//! ⭐ THE RULE THE TRIM FOLLOWS: **a shorter answer is never a quieter one.**
//! Numbers are never touched — every count, total and tally survives at full
//! precision. What is withheld is PROSE, and only then list length, and the
//! reply always says which happened and how much it dropped. That ordering is
//! deliberate: on every tool measured above, the bytes are in long `statement`,
//! `rationale`, `reason`, `note` and `description` fields, while the part a
//! reader acts on is the counts and the ids.
//!
//! ⚠️ WHAT THIS DOES NOT COVER. `export_graph` is excluded on purpose and must
//! stay excluded: its job IS the whole document, and a truncated export is not
//! a smaller answer but a corrupt one. It needs streaming or a file path, which
//! is a different change. See the same finding.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

/// The default ceiling, re-exported so a caller never hard-codes `30_000`
/// the way `reviewed_gaps` did before this module existed.
pub use reflow2_core::detect::DEFAULT_REPLY_BUDGET_CHARS;

/// The parameter block for a tool whose ONLY argument is its reply budget.
///
/// Tools that already take arguments grow a `budget_chars` field of their own
/// instead; this exists for the ones that took none at all — which was most of
/// the overflowing set, and the reason they had nowhere to put a bound.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BudgetReq {
    /// How many characters of JSON this reply may spend before prose is
    /// withheld to make it fit (default 30,000).
    ///
    /// RAISE IT ONLY IF YOU KNOW THIS CLIENT HAS THE ROOM. The default sits
    /// below the smallest tool-output cap in use, because the failure it
    /// exists to stop is not a slow call — it is the CLIENT refusing the reply,
    /// at which point the session sees a wall of harness text and reflow2 never
    /// gets to suggest narrowing.
    ///
    /// Every trimmed reply says what it withheld, at `budget`; counts and ids
    /// are never budgeted away, so a shorter answer is never a quieter one.
    #[serde(default)]
    pub budget_chars: Option<usize>,
}

impl BudgetReq {
    /// The budget this request asks for, or the default.
    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget_chars.unwrap_or(DEFAULT_REPLY_BUDGET_CHARS)
    }
}

/// String lengths tried in order before list length is touched at all.
///
/// Prose is cut progressively rather than in one step so a reply that is
/// slightly over budget keeps nearly all of its text, and only a reply that is
/// wildly over loses most of it. The last rung matches the 240 characters
/// `reviewed_gaps` chose by hand, so behaviour there is unchanged.
const PROSE_RUNGS: [usize; 6] = [4000, 2000, 800, 240, 120, 0];

/// A string this short is NEVER trimmed, at any rung — it is a name, an id, an
/// enum value or a status, not prose.
///
/// ⭐ THIS IS WHAT MAKES THE 0 RUNG SAFE, and the 0 rung is what lets a reply
/// keep a COMPLETE list while dropping the prose hanging off it. Without the
/// floor, trimming to 0 would blank every type name in `describe_schema` and
/// leave a vocabulary of `…` — complete in shape and empty of content, which is
/// the quiet-answer failure in its purest form. Measured: reflow2's longest id
/// is well under this, and a `sha256:` hash is 71.
const NEVER_TRIM_BELOW: usize = 80;

/// Serialized size of a value, in characters — the same measure every budget
/// in this codebase is stated in.
#[must_use]
pub fn chars_of(v: &Value) -> usize {
    v.to_string().len()
}

/// Truncate every string longer than `limit`, recursively, leaving numbers,
/// booleans, nulls and object KEYS untouched.
///
/// Keys are preserved because they are structure rather than prose: a reader
/// who gets `statement` back as `stat…` cannot tell what was withheld from
/// what was never there. Short strings are left alone, which is what keeps ids
/// intact — every id convention in this design (`req:`, `cap:`, `sha256:…`) is
/// far below the shortest rung.
fn truncate_prose(v: &Value, limit: usize, cut: &mut usize) -> Value {
    match v {
        Value::String(s) => {
            let n = s.chars().count();
            if n > limit && n > NEVER_TRIM_BELOW {
                *cut += 1;
                let kept: String = s.chars().take(limit).collect();
                Value::String(format!("{kept}…"))
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| truncate_prose(x, limit, cut)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), truncate_prose(x, limit, cut)))
                .collect(),
        ),
        _ => v.clone(),
    }
}

/// Find the longest array in the value and report its path and length.
///
/// Used only when prose alone could not make the reply fit, so the reply can
/// say WHICH list it shortened rather than reporting a bare "truncated".
fn longest_array(v: &Value, path: &str, best: &mut (String, usize)) {
    match v {
        Value::Array(a) => {
            if a.len() > best.1 {
                *best = (path.to_string(), a.len());
            }
            for (i, x) in a.iter().enumerate() {
                longest_array(x, &format!("{path}[{i}]"), best);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                longest_array(x, &p, best);
            }
        }
        _ => {}
    }
}

/// Cap the array at `path` to `keep` entries.
fn cap_array_at(v: &Value, path: &str, keep: usize) -> Value {
    fn walk(v: &Value, parts: &[&str], keep: usize) -> Value {
        if parts.is_empty() {
            if let Value::Array(a) = v {
                return Value::Array(a.iter().take(keep).cloned().collect());
            }
            return v.clone();
        }
        match v {
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| {
                        if k == parts[0] {
                            (k.clone(), walk(x, &parts[1..], keep))
                        } else {
                            (k.clone(), x.clone())
                        }
                    })
                    .collect(),
            ),
            _ => v.clone(),
        }
    }
    let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
    walk(v, &parts, keep)
}

/// Bound `full` to `budget` characters, withholding prose first and list
/// length only if prose was not enough.
///
/// Returns the value unchanged when it already fits — including the absence of
/// any `budget` key, so a reply that never needed trimming is byte-identical to
/// what the tool returned before this module existed.
///
/// `hint` is the one tool-specific sentence: where to read the withheld detail
/// (`"read one in full with get_node on its id"`, `"narrow with scope"`). It is
/// required rather than optional because a reply that says what it dropped and
/// not how to get it back has told the reader only the bad half.
/// Whether shortening a LIST is an honest answer for this tool.
///
/// 🛑 THIS IS OPT-IN AND THE DEFAULT IS NO, because the first version of this
/// module capped lists for everybody and a committed test caught it within the
/// hour: `describe_schema` went from 65 edge types to ONE, still shaped like a
/// complete vocabulary. That is the exact failure this module's own rule
/// forbids — a shorter answer that is also a quieter one — and no note attached
/// to a gutted list makes it safe, because the caller's next move is to believe
/// the design has one edge type.
///
/// Say `Sampling::Allowed` only where a partial list still answers the question
/// asked and a `total` beside it makes the sample legible. Say `Forbidden`
/// wherever completeness IS the answer: a vocabulary, a catalogue an agent
/// picks from, a list of everything that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    /// A partial list plus its true count still answers the question.
    Allowed,
    /// Completeness is the answer. Over budget, overflow LOUDLY instead.
    Forbidden,
}

/// Bound a reply, trimming prose only — never list length.
///
/// The right default: on every tool measured, the bytes are in long prose
/// fields while the part a reader acts on is the counts, the ids and the
/// completeness of the list.
#[must_use]
pub fn bound_reply(full: Value, budget: usize, hint: &str) -> Value {
    bound_reply_with(full, budget, hint, Sampling::Forbidden)
}

/// Bound a reply, and permit shortening its longest list if prose alone could
/// not make it fit. See [`Sampling`] before reaching for this.
#[must_use]
pub fn bound_reply_sampling(full: Value, budget: usize, hint: &str) -> Value {
    bound_reply_with(full, budget, hint, Sampling::Allowed)
}

#[must_use]
fn bound_reply_with(full: Value, budget: usize, hint: &str, sampling: Sampling) -> Value {
    let full_chars = chars_of(&full);
    if full_chars <= budget {
        return full;
    }

    // ① Prose first, progressively. Numbers and ids are never candidates.
    let mut body = full.clone();
    let mut rung_used = None;
    let mut strings_cut = 0usize;
    for rung in PROSE_RUNGS {
        let mut cut = 0usize;
        let candidate = truncate_prose(&full, rung, &mut cut);
        body = candidate;
        rung_used = Some(rung);
        strings_cut = cut;
        if chars_of(&body) <= budget {
            break;
        }
    }

    // ② Only if prose was not enough, and only where a sample is honest for
    //    this tool: shorten the longest list, and say which.
    let mut list_note = String::new();
    if chars_of(&body) > budget && sampling == Sampling::Forbidden {
        list_note = format!(
            " THE LIST ITSELF IS NOT shortened AND THE REPLY IS STILL {} CHARACTERS: for this \
             tool completeness IS the answer, so returning part of the list would be a quieter \
             answer wearing a complete one's shape. Narrow the question instead, or raise \
             `budget_chars`.",
            chars_of(&body)
        );
    } else if chars_of(&body) > budget {
        let mut best = (String::new(), 0usize);
        longest_array(&body, "", &mut best);
        let (path, len) = best;
        if len > 1 {
            // Halve until it fits or we are down to a token sample.
            let mut keep = len;
            while keep > 1 && chars_of(&body) > budget {
                keep = (keep / 2).max(1);
                body = cap_array_at(&body, &path, keep);
            }
            list_note = format!(
                " THEN LIST LENGTH: `{path}` is shown as {keep} of {len} entries, because \
                 trimming prose alone still did not fit."
            );
        }
    }

    let now = chars_of(&body);
    let rung = rung_used.unwrap_or(*PROSE_RUNGS.last().unwrap());
    let note = format!(
        "WITHHELD TO FIT. The whole answer is {full_chars} characters against a budget of \
         {budget}, which is the size at which a client refuses the reply outright — and a \
         reader who receives nothing cannot be told to narrow their question. What was \
         withheld is PROSE: {strings_cut} text field(s) cut to {rung} characters. Every \
         number, count and id is untouched and at full precision, so a shorter answer here \
         is never a quieter one.{list_note} Raise `budget_chars` if this client has the room. \
         {hint}"
    );

    let mut out = body;
    if let Value::Object(ref mut o) = out {
        o.insert(
            "budget".to_string(),
            json!({
                "applied": true,
                "budget_chars": budget,
                "full_chars": full_chars,
                "reply_chars": now,
                "prose_limit": rung,
                "fields_trimmed": strings_cut,
                "note": note,
            }),
        );
        out
    } else {
        // A bare array or scalar has nowhere to carry the block, so it is
        // wrapped rather than silently returned without its own explanation.
        json!({
            "items": out,
            "budget": {
                "applied": true,
                "budget_chars": budget,
                "full_chars": full_chars,
                "reply_chars": now,
                "prose_limit": rung,
                "fields_trimmed": strings_cut,
                "note": note,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long(n: usize) -> String {
        "x".repeat(n)
    }

    #[test]
    fn a_reply_that_fits_is_returned_byte_identical() {
        let v = json!({"count": 3, "items": ["a", "b"]});
        let out = bound_reply(v.clone(), 30_000, "hint");
        assert_eq!(out, v, "a reply under budget must not be rewritten at all");
        assert!(
            out.get("budget").is_none(),
            "no budget block when nothing was withheld"
        );
    }

    #[test]
    fn prose_is_cut_but_numbers_never_are() {
        let v = json!({"count": 12345, "total": 999, "statement": long(50_000)});
        let out = bound_reply(v, 1_000, "hint");
        assert_eq!(out["count"], 12345, "counts must survive trimming exactly");
        assert_eq!(out["total"], 999);
        assert!(chars_of(&out) <= 2_000, "must be near the budget");
        assert_eq!(out["budget"]["applied"], true);
    }

    #[test]
    fn ids_are_short_enough_to_survive_every_rung() {
        let v = json!({
            "node_id": "req:one-open-design-costs-a-deliberate-amount-of-memory",
            "checksum": "sha256:acccfe9a30a4c45a273299fdf8cc35c8f722ff76dea00e8adf003824f3422e2e",
            "statement": long(80_000),
        });
        let out = bound_reply(v, 500, "hint");
        assert_eq!(
            out["node_id"], "req:one-open-design-costs-a-deliberate-amount-of-memory",
            "an id must never be truncated — it is structure, not prose"
        );
        assert!(out["checksum"].as_str().unwrap().starts_with("sha256:"));
        assert!(!out["checksum"].as_str().unwrap().ends_with('…'));
    }

    #[test]
    fn a_list_is_never_shortened_unless_the_tool_opted_in() {
        // THE REGRESSION THIS PINS. The first version of this module capped
        // lists for every caller, and `describe_schema` came back with ONE of
        // its 65 edge types — still shaped exactly like a whole vocabulary.
        // A committed test caught it; this one keeps it caught.
        let items: Vec<Value> = (0..4000).map(|i| json!({"id": format!("n:{i}")})).collect();
        let v = json!({"count": 4000, "items": items});
        let out = bound_reply(v, 2_000, "narrow the question");
        assert_eq!(
            out["items"].as_array().unwrap().len(),
            4000,
            "completeness is the default: a list must survive whole unless the tool opted in"
        );
        assert!(
            out["budget"]["note"]
                .as_str()
                .unwrap()
                .contains("NOT shortened"),
            "and the reply must say it chose to overflow rather than go quiet"
        );
    }

    #[test]
    fn list_length_is_touched_only_after_prose_and_says_so() {
        // Many rows of SHORT strings: prose trimming cannot help, so the list
        // itself has to give. This is the path that must announce itself.
        let items: Vec<Value> = (0..4000).map(|i| json!({"id": format!("n:{i}")})).collect();
        let v = json!({"count": 4000, "items": items});
        let out = bound_reply_sampling(v, 2_000, "narrow with scope");
        assert_eq!(out["count"], 4000, "the COUNT still reports every row");
        assert!(
            out["budget"]["note"]
                .as_str()
                .unwrap()
                .contains("THEN LIST LENGTH"),
            "a shortened list must be named, not silent"
        );
        assert!(out["items"].as_array().unwrap().len() < 4000);
    }

    #[test]
    fn the_hint_always_reaches_the_reader() {
        let v = json!({"statement": long(50_000)});
        let out = bound_reply(v, 500, "read one in full with get_node on its id");
        assert!(
            out["budget"]["note"]
                .as_str()
                .unwrap()
                .contains("read one in full with get_node on its id"),
            "saying what was dropped without saying how to get it back is half an answer"
        );
    }

    #[test]
    fn a_name_survives_the_zero_rung_that_drops_every_hint() {
        // The vocabulary case: describe_schema must come back COMPLETE — every
        // type and edge name present — even when its prose is dropped whole.
        // Without the floor, the 0 rung would blank the names too and return a
        // vocabulary of ellipses that still counted 65 edge types.
        let types: Vec<Value> = (0..200)
            .map(|i| json!({"edge_type": format!("EDGE_TYPE_{i}"), "hint": long(3_000)}))
            .collect();
        let out = bound_reply(
            json!({"edge_types": types}),
            5_000,
            "narrow with from and to",
        );
        let got = out["edge_types"].as_array().unwrap();
        assert_eq!(got.len(), 200, "every edge type must still be listed");
        assert_eq!(
            got[7]["edge_type"], "EDGE_TYPE_7",
            "a NAME is not prose and must survive every rung"
        );
        assert!(
            got[7]["hint"].as_str().unwrap().chars().count() < 100,
            "its hint, which is prose, should be gone"
        );
    }

    #[test]
    fn a_bare_array_is_wrapped_so_it_can_carry_its_own_explanation() {
        let v = json!([{"statement": long(50_000)}]);
        let out = bound_reply(v, 500, "hint");
        assert!(out.get("items").is_some());
        assert_eq!(out["budget"]["applied"], true);
    }
}
