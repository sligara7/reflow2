//! The usage ledger: what the server was asked, how it answered, never what
//! it was asked ABOUT.
//!
//! `req:feedback-on-reflow2-is-computed-from-a-record-the-server-kept`
//! (Anthony, 2026-09-12): feedback on reflow2 from any project is computed
//! from a record the server itself kept, never from the agent's recollection.
//! Every refusal and every call already passes through one place —
//! [`crate::service::ReflowService`]'s `call_tool` — and until this module
//! nothing was kept, so "which tools did this project's sessions use, and
//! where did reflow2 fail them" could only be answered by an agent remembering,
//! or by reading one harness's transcripts off one machine
//! (`tools/surface_usage.py`, which cannot see a Cursor, Codex or Grok session).
//!
//! # What a line carries, and the line it must not cross
//!
//! The payload shape is `req:telemetry-carries-usage-never-design-content`,
//! unchanged: **the verb, never the object.** Tool name, outcome class, refusal
//! class, duration, timestamp, the harness that connected, the seat — yes.
//! Arguments, node ids, statements, search queries, error text — never, because
//! an error message quotes the design ("`req:the-ledger-can-say-…` already
//! exists") and a search query names the user's domain in their own words. A
//! refusal is therefore recorded as its CLASS, derived here from a fixed table
//! of the server's own phrasings ([`classify`]), and the message is dropped on
//! the floor. The one argument that is recorded is `get_skill`'s `name`, which
//! is reflow2's vocabulary and not the user's.
//!
//! # Where it lives, and the unit
//!
//! `<graph-path>.usage.jsonl`, beside the store — the sidecar convention
//! `.meta.json`, `.server.json` and `.client.json` already follow, and for the
//! same reason: it belongs to the project, not to a session, and it must never
//! be inside the store a `--in-memory` design does not have. **The unit is the
//! PROJECT, not the session.** In `--shared` mode one daemon serves many
//! clients, and a per-session tally would need the connection carried through
//! the proxy or would silently be per-daemon-lifetime. A report instead covers
//! everything since the previous report's marker (or since a date the caller
//! names), which is also what was actually asked for: feedback *"not limited
//! by what the agent remembers from the current session (and past sessions)"*.
//!
//! # Best effort, like the handshake
//!
//! A ledger that could fail a tool call would be a diagnostic costing the thing
//! it diagnoses. Appends swallow their own IO errors, exactly as
//! [`crate::handshake::Handshake::write`] does; an in-memory service (no
//! `graph_path`) writes nothing at all.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `<graph-path>.usage.jsonl` — beside the store, like `.client.json`.
pub fn usage_path(graph_path: &str) -> PathBuf {
    let p = Path::new(graph_path);
    match p.file_name().and_then(|n| n.to_str()) {
        Some(n) => p.with_file_name(format!("{n}.usage.jsonl")),
        None => PathBuf::from(format!("{graph_path}.usage.jsonl")),
    }
}

/// How a call ended. Three values on purpose: a report that distinguished
/// twenty kinds of failure would be a list nobody reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Answered.
    Ok,
    /// The server declined, by design — a guard, a missing argument, a
    /// reference that resolves to nothing. Carries a [`RefusalClass`].
    Refused,
    /// Something went wrong that no guard chose: a store error, a panic
    /// caught at the boundary, an internal failure.
    Error,
}

/// WHICH guard declined, derived from the server's own phrasings and never
/// from the user's content. `Other` is the honest remainder, and a large
/// `Other` share in a report is itself a finding: a refusal class the table
/// does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalClass {
    /// The search-first guard: something close already exists.
    NearMatch,
    /// A required parameter was not given.
    MissingArgument,
    /// A parameter the served schema does not know — usually a stale client.
    UnknownArgument,
    /// An id that names nothing in the design.
    UnresolvedReference,
    /// An explicit `REFUSED` by a tool's own rule (lineage, stale binary,
    /// nobody's name on a settlement, …).
    Refused,
    /// A decline the table has no class for.
    Other,
}

/// Turn a failed call's text into a class, WITHOUT keeping the text.
///
/// The phrasings are the server's own — `crate::service` and the tool slices
/// produce them — so this table is a dependency on wording, and it says so.
/// A phrasing that moves lands in `Other`, which the report shows, which is
/// how the drift is noticed. Deliberately not a regex over the user's content:
/// the classification reads the FIRST clause the server wrote, never a quoted
/// id or statement, and the whole message is dropped once classed.
pub fn classify(message: &str) -> (Outcome, Option<RefusalClass>) {
    let m = message;
    let class = if m.contains("missing field") {
        RefusalClass::MissingArgument
    } else if m.contains("unknown field") {
        RefusalClass::UnknownArgument
    } else if m.contains("already says something close")
        || m.contains("DIFFERENT KIND OF RECORD")
        || m.contains("was NOT created")
    {
        RefusalClass::NearMatch
    } else if m.contains("names no ")
        || m.contains("does not exist")
        || m.contains("no such ")
        || m.contains("MUST RESOLVE")
        || m.contains("must resolve")
    {
        RefusalClass::UnresolvedReference
    } else if m.contains("REFUSED")
        || m.contains("Nothing was written")
        || m.contains("nothing was written")
        || m.contains("will not record")
    {
        RefusalClass::Refused
    } else {
        RefusalClass::Other
    };
    (Outcome::Refused, Some(class))
}

/// One line of the ledger. Two kinds share the file: a `call`, and a `report`
/// marker that [`window`] uses as "since last time". Field names are short
/// because a busy project writes thousands of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLine {
    /// Unix seconds. The file's own order is the tiebreak.
    pub at: u64,
    /// `call` or `report`.
    pub kind: String,
    /// The tool, for a call; absent on a marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalClass>,
    /// Milliseconds the call took, wall clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    /// The harness, from the handshake: `claude-code`, `cursor`, … and its
    /// version. `unknown` when the transport carried none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    /// The seat that made the call — reflow2's own handle, not a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seat: Option<String>,
    /// For `get_skill` only: which skill. reflow2's vocabulary, so it may be
    /// kept; no other argument is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
}

impl UsageLine {
    /// The line a report leaves behind so the next one starts after it.
    pub fn report_marker() -> Self {
        UsageLine {
            at: now_unix(),
            kind: "report".into(),
            tool: None,
            outcome: None,
            refusal: None,
            ms: None,
            client: None,
            client_version: None,
            seat: None,
            skill: None,
        }
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Append one line, best effort. Never returns an error: see the module doc.
pub fn append(graph_path: &str, line: &UsageLine) {
    let Ok(json) = serde_json::to_string(line) else {
        return;
    };
    let path = usage_path(graph_path);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{json}");
    }
}

/// Every line the file holds, in file order. Malformed lines are skipped
/// rather than failing the read — a ledger with one torn line (a process
/// killed mid-append) must still report the other thousand.
pub fn read_all(graph_path: &str) -> Vec<UsageLine> {
    let Ok(raw) = std::fs::read_to_string(usage_path(graph_path)) else {
        return Vec::new();
    };
    raw.lines()
        .filter_map(|l| serde_json::from_str::<UsageLine>(l).ok())
        .collect()
}

/// The lines a report covers: everything after the LAST `report` marker, or
/// everything at or after `since_unix` when a caller names a date. A named
/// date wins over the marker, because a person asking "since the first of the
/// month" must not be answered "since yesterday's report".
pub fn window(lines: &[UsageLine], since_unix: Option<u64>) -> (Vec<&UsageLine>, WindowStart) {
    if let Some(since) = since_unix {
        return (
            lines
                .iter()
                .filter(|l| l.kind == "call" && l.at >= since)
                .collect(),
            WindowStart::NamedDate(since),
        );
    }
    let last_marker = lines.iter().rposition(|l| l.kind == "report");
    match last_marker {
        Some(i) => (
            lines[i + 1..].iter().filter(|l| l.kind == "call").collect(),
            WindowStart::LastReport(lines[i].at),
        ),
        None => (
            lines.iter().filter(|l| l.kind == "call").collect(),
            WindowStart::BeginningOfLedger,
        ),
    }
}

/// Where a report's window began, so the report can say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowStart {
    LastReport(u64),
    NamedDate(u64),
    BeginningOfLedger,
}

impl WindowStart {
    pub fn describe(self) -> String {
        match self {
            WindowStart::LastReport(at) => format!("since the previous report (unix {at})"),
            WindowStart::NamedDate(at) => format!("since the date named (unix {at})"),
            WindowStart::BeginningOfLedger => {
                "since the ledger began (no previous report)".to_string()
            }
        }
    }
}

/// The JSON the report emits for the window's start.
fn window_start_json(w: WindowStart) -> serde_json::Value {
    match w {
        WindowStart::LastReport(at) => serde_json::json!({"from": "last_report", "at_unix": at}),
        WindowStart::NamedDate(at) => serde_json::json!({"from": "named_date", "at_unix": at}),
        WindowStart::BeginningOfLedger => serde_json::json!({"from": "beginning_of_ledger"}),
    }
}

/// The computed spine of a `/feedback` report — everything a person can read
/// without an agent remembering anything.
#[derive(Debug, Serialize)]
pub struct Tally {
    pub window: serde_json::Value,
    pub calls: usize,
    pub first_call_unix: Option<u64>,
    pub last_call_unix: Option<u64>,
    /// tool → count, every tool called at least once in the window.
    pub by_tool: BTreeMap<String, usize>,
    /// Served tools NEVER called in the window. What `dec:bl-155` measured
    /// from transcripts, now measured from the ledger, for every harness.
    pub never_called: Vec<String>,
    /// refusal class → count.
    pub refusals_by_class: BTreeMap<String, usize>,
    /// tool → refusal class → count, so a reader sees WHICH tool refused HOW.
    pub refusals_by_tool: BTreeMap<String, BTreeMap<String, usize>>,
    pub errors_by_tool: BTreeMap<String, usize>,
    /// skill → count of `get_skill` fetches.
    pub skills_fetched: BTreeMap<String, usize>,
    /// `name version` → count, every harness that wrote to the ledger.
    pub clients: BTreeMap<String, usize>,
    pub seats: usize,
}

/// Aggregate a window. `served` is the surface as the server lists it now,
/// so `never_called` is against the tools a session COULD have reached.
pub fn tally(lines: &[&UsageLine], start: WindowStart, served: &[String]) -> Tally {
    let mut by_tool: BTreeMap<String, usize> = BTreeMap::new();
    let mut refusals_by_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut refusals_by_tool: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut errors_by_tool: BTreeMap<String, usize> = BTreeMap::new();
    let mut skills_fetched: BTreeMap<String, usize> = BTreeMap::new();
    let mut clients: BTreeMap<String, usize> = BTreeMap::new();
    let mut seats = std::collections::BTreeSet::new();
    for l in lines {
        let tool = l.tool.clone().unwrap_or_else(|| "?".into());
        *by_tool.entry(tool.clone()).or_default() += 1;
        match l.outcome {
            Some(Outcome::Refused) => {
                let class = l
                    .refusal
                    .and_then(|c| serde_json::to_value(c).ok())
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "other".into());
                *refusals_by_class.entry(class.clone()).or_default() += 1;
                *refusals_by_tool
                    .entry(tool.clone())
                    .or_default()
                    .entry(class)
                    .or_default() += 1;
            }
            Some(Outcome::Error) => *errors_by_tool.entry(tool.clone()).or_default() += 1,
            _ => {}
        }
        if let Some(s) = &l.skill {
            *skills_fetched.entry(s.clone()).or_default() += 1;
        }
        let client = format!(
            "{} {}",
            l.client.as_deref().unwrap_or("unknown"),
            l.client_version.as_deref().unwrap_or("")
        )
        .trim()
        .to_string();
        *clients.entry(client).or_default() += 1;
        if let Some(s) = &l.seat {
            seats.insert(s.clone());
        }
    }
    let mut never_called: Vec<String> = served
        .iter()
        .filter(|t| !by_tool.contains_key(*t))
        .cloned()
        .collect();
    never_called.sort();
    Tally {
        window: window_start_json(start),
        calls: lines.len(),
        first_call_unix: lines.iter().map(|l| l.at).min(),
        last_call_unix: lines.iter().map(|l| l.at).max(),
        by_tool,
        never_called,
        refusals_by_class,
        refusals_by_tool,
        errors_by_tool,
        skills_fetched,
        clients,
        seats: seats.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(at: u64, tool: &str, outcome: Outcome, refusal: Option<RefusalClass>) -> UsageLine {
        UsageLine {
            at,
            kind: "call".into(),
            tool: Some(tool.into()),
            outcome: Some(outcome),
            refusal,
            ms: Some(1),
            client: Some("test-harness".into()),
            client_version: Some("0".into()),
            seat: Some("seat:a".into()),
            skill: None,
        }
    }

    fn marker(at: u64) -> UsageLine {
        UsageLine {
            at,
            kind: "report".into(),
            tool: None,
            outcome: None,
            refusal: None,
            ms: None,
            client: None,
            client_version: None,
            seat: None,
            skill: None,
        }
    }

    /// The table names the server's phrasings and nothing of the user's.
    #[test]
    fn refusals_are_classed_from_the_servers_own_phrasing() {
        let cases = [
            ("missing field `id`", RefusalClass::MissingArgument),
            (
                "unknown field `nme`, expected one of",
                RefusalClass::UnknownArgument,
            ),
            (
                "The design already says something close to this, so `dec:x` was NOT created",
                RefusalClass::NearMatch,
            ),
            (
                "`req:x` names no Requirement",
                RefusalClass::UnresolvedReference,
            ),
            (
                "REFUSED: this server's executable has been replaced",
                RefusalClass::Refused,
            ),
            (
                "`add_decision` will not record `accepted` with nobody's name on it. Nothing was written.",
                RefusalClass::Refused,
            ),
            ("the store is on fire", RefusalClass::Other),
        ];
        for (msg, want) in cases {
            let (o, c) = classify(msg);
            assert_eq!(o, Outcome::Refused);
            assert_eq!(c, Some(want), "{msg}");
        }
    }

    /// The window is "since the last report", and a named date overrides it.
    #[test]
    fn the_window_starts_after_the_last_marker_unless_a_date_is_named() {
        let lines = vec![
            call(10, "a", Outcome::Ok, None),
            marker(20),
            call(30, "b", Outcome::Ok, None),
            marker(40),
            call(50, "c", Outcome::Ok, None),
            call(60, "d", Outcome::Ok, None),
        ];
        let (w, start) = window(&lines, None);
        assert_eq!(
            w.iter()
                .map(|l| l.tool.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["c", "d"]
        );
        assert_eq!(start, WindowStart::LastReport(40));

        let (w, start) = window(&lines, Some(30));
        assert_eq!(
            w.iter()
                .map(|l| l.tool.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["b", "c", "d"],
            "a named date reaches back past the marker"
        );
        assert_eq!(start, WindowStart::NamedDate(30));

        let (w, start) = window(&lines[..1], None);
        assert_eq!(w.len(), 1);
        assert_eq!(start, WindowStart::BeginningOfLedger);
    }

    /// `never_called` is against the served surface, refusals are counted by
    /// class AND by tool, and the skill fetched is kept by name.
    #[test]
    fn the_tally_counts_what_a_report_needs() {
        let mut fetched = call(1, "get_skill", Outcome::Ok, None);
        fetched.skill = Some("where-am-i".into());
        let lines = [
            fetched,
            call(
                2,
                "add_decision",
                Outcome::Refused,
                Some(RefusalClass::NearMatch),
            ),
            call(3, "add_decision", Outcome::Ok, None),
            call(4, "get_node", Outcome::Error, None),
        ];
        let refs: Vec<&UsageLine> = lines.iter().collect();
        let served = vec![
            "add_decision".to_string(),
            "get_node".to_string(),
            "get_skill".to_string(),
            "loop_status".to_string(),
        ];
        let t = tally(&refs, WindowStart::BeginningOfLedger, &served);
        assert_eq!(t.calls, 4);
        assert_eq!(t.by_tool["add_decision"], 2);
        assert_eq!(t.never_called, vec!["loop_status".to_string()]);
        assert_eq!(t.refusals_by_class["near_match"], 1);
        assert_eq!(t.refusals_by_tool["add_decision"]["near_match"], 1);
        assert_eq!(t.errors_by_tool["get_node"], 1);
        assert_eq!(t.skills_fetched["where-am-i"], 1);
        assert_eq!(t.clients["test-harness 0"], 4);
        assert_eq!(t.seats, 1);
    }

    /// A torn line does not lose the others, and the path sits beside the store.
    #[test]
    fn a_torn_line_is_skipped_and_the_ledger_sits_beside_the_store() {
        let d = std::env::temp_dir().join(format!("reflow2-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let graph = d.join("graph");
        let gp = graph.to_str().unwrap();
        assert_eq!(usage_path(gp), d.join("graph.usage.jsonl"));
        append(gp, &call(1, "a", Outcome::Ok, None));
        std::fs::OpenOptions::new()
            .append(true)
            .open(usage_path(gp))
            .unwrap()
            .write_all(b"{\"at\": 2, \"kind\": \"ca")
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(usage_path(gp))
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        append(gp, &call(3, "b", Outcome::Ok, None));
        let all = read_all(gp);
        assert_eq!(all.len(), 2, "{all:?}");
        std::fs::remove_dir_all(&d).ok();
    }
}
