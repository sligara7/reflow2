//! The server answers "am I still the code I was started from?" itself.
//!
//! `req:the-server-is-the-authority-on-its-own-currency`, which had NOTHING
//! satisfying it until now and cost three sessions across two projects.
//!
//! ## Why a derived bit and not a version comparison
//!
//! `served_by` already carried `reflow2_version`, and that is exactly what
//! failed. dev_storyflow measured the cost on 2026-08-07: four sessions read
//! `0.22.1` out of that block on two different days and drew OPPOSITE
//! conclusions from the same true value — one reported a PASS on a broken
//! invariant, because a stand-down post had told it to demand that literal and
//! the literal was satisfied while the invariant was not. A version string also
//! cannot answer the question at all when two builds share a version, which is
//! every `cargo build` in a working session.
//!
//! ## The third instance is why this got built
//!
//! 2026-08-08: Anthony restarted the session specifically to pick up a new
//! binary, and the served surface did not move — `--shared` is a CLIENT that
//! re-attaches to the detached daemon. Five merged PRs had been built, gated
//! and shipped without one of them ever running live. It was caught only
//! because the session happened to test a schema fact it remembered changing.
//!
//! ## What is NOT claimed
//!
//! A `false` here means the executable file is unchanged. It does not mean the
//! graph is current, the checkout matches, or the answer is right. It answers
//! one question — "has my own binary been replaced under me?" — which is the
//! question nothing could answer before.

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

/// The block must STATE the answer rather than leave a literal to be compared.
/// That inversion is the whole point: the old block was not missing data, it
/// was making the reader do the inference, and readers got it wrong in both
/// directions.
#[tokio::test]
async fn served_by_states_staleness_rather_than_leaving_it_to_be_inferred() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let report = j!(s.graph_report());
    let served = &report["served_by"];

    assert!(
        served.get("stale").is_some(),
        "served_by must carry an explicit `stale`, not just a version to compare: {served}"
    );
    assert!(
        served.get("stale_note").is_some(),
        "and a note saying what to DO, since a bare bit tells nobody how to act: {served}"
    );
}

/// A test binary has not been replaced under itself, so the honest answer is
/// `false` — and it must be `false` rather than `null`. Reporting "unknown"
/// when the check succeeded would make the field useless in the common case.
#[tokio::test]
async fn a_binary_that_has_not_been_replaced_says_so() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let served = j!(s.graph_report())["served_by"].clone();

    // Linux only: elsewhere /proc is absent and `null` is the correct answer.
    if std::path::Path::new("/proc/self/exe").exists() {
        assert_eq!(
            served["stale"],
            serde_json::json!(false),
            "this test binary is intact, so the check must return a definite false: {served}"
        );
        assert!(
            served["stale_note"]
                .as_str()
                .unwrap_or_default()
                .contains("current"),
            "the note must say so in words a person can read: {served}"
        );
    }
}

/// THE DISTINCTION THAT MAKES THE FIELD SAFE. `null` is `unknown`, and unknown
/// must never be readable as `false` — "I could not look" and "I looked and I
/// am current" license completely different behaviour, and collapsing them is
/// the same error as a claim reading `live` because nothing could observe it.
#[tokio::test]
async fn unknown_is_never_reported_as_current() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let served = j!(s.graph_report())["served_by"].clone();
    let stale = &served["stale"];

    assert!(
        stale.is_boolean() || stale.is_null(),
        "stale is three-valued — true / false / null-for-unknown: {served}"
    );
    if stale.is_null() {
        let note = served["stale_note"].as_str().unwrap_or_default();
        assert!(
            note.contains("unknown"),
            "a null must SAY it is unknown rather than reading as a quiet no: {note}"
        );
        assert!(
            !note.contains("current:"),
            "and it must not borrow the wording of a definite answer: {note}"
        );
    }
}

/// The note a STALE server returns, asserted unconditionally.
///
/// Against the public constant rather than a live payload, because the stale
/// branch never runs in a test process. The first draft guarded these
/// assertions with `if stale == true` and they silently never executed — the
/// same vacuous-branch problem this session kept finding elsewhere, reproduced
/// in the test written to prevent it. The live behaviour WAS verified by hand
/// (rebuild under a running daemon, `served_by.stale` flipped false -> true and
/// `binary_mtime_unix` went to null in the same call); this pins the wording so
/// it cannot drift afterwards.
#[test]
fn the_stale_note_names_the_remedy_that_actually_works() {
    let note = STALE_NOTE;
    assert!(
        note.contains("--stop-shared"),
        "the remedy must be in the payload, not left in a doc: {note}"
    );
    assert!(
        note.contains("re-attaches to the same daemon"),
        "it must say why a bare session restart changes nothing, or people keep trying it"
    );
    // MEASURED 2026-08-09. The first version of this note said `--stop-shared`
    // plus any call was the whole remedy and NO restart was needed. Written
    // confidently, wrong within the hour: the respawn failed with `No such file
    // or directory`, because after a rebuild the CLIENT spawns via its own
    // `(deleted)` path too (fact:defect-rebuild-strands-the-shared-server). A
    // remedy that works only when you have not rebuilt is no remedy in a repo
    // where rebuilding is why you are checking.
    assert!(
        note.contains("client") && note.contains("No such file or directory"),
        "it must warn that the respawn itself fails when the client was replaced, which is the \
         NORMAL case after a rebuild"
    );
}

/// Unknown must not borrow the wording of a definite answer.
#[test]
fn the_unknown_note_says_unknown() {
    assert!(UNKNOWN_NOTE.contains("unknown"));
    assert!(
        UNKNOWN_NOTE.contains("not `false`"),
        "it must say outright that unknown is not a quiet no"
    );
    assert_ne!(UNKNOWN_NOTE, CURRENT_NOTE);
}

// ═══════════════════════════════════════════════════════════════════════════
// FOURTH INSTANCE, 2026-08-19 — and the first three are in this file's header.
//
// The bit above was built after instance three and it did not prevent instance
// four, because it rode on `graph_report` alone. That is not a call anything
// points a session at: the session-start hook says "loop_status is the one
// cheap call", the stop hook nudges there, and a whole day of work ran against
// a daemon four hours older than the code without one signal reaching the
// reader. Five merged PRs, a deliberate restart, no movement — the same shape
// as 2026-08-08, with the remedy already shipped and unreachable.
//
// So the currency of the ANSWERER now rides beside the currency of the DESIGN.

/// The orientation call carries the answer, not just the report nobody calls.
#[tokio::test]
async fn loop_status_says_whether_its_own_answers_are_current() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let status = j!(s.loop_status(Parameters(Default::default())));
    let served = &status["served_by"];

    assert!(
        !served.is_null(),
        "loop_status is the call the project points sessions at; it must say who \
         answered it: {status}"
    );
    assert!(
        served.get("stale").is_some(),
        "an explicit bit, not a version to compare — the inference is what readers \
         got wrong in both directions: {served}"
    );
    assert!(served.get("reflow2_version").is_some(), "{served}");
}

/// CHEAP WHEN CURRENT. The remedy note is ~1 KB and the ordinary call must not
/// pay for it, or `cap:loop-status`'s "one cheap call" erodes the way the
/// per-check roll already did once.
#[tokio::test]
async fn a_current_server_costs_three_fields_and_does_not_touch_next() {
    let s = ReflowService::in_memory().expect("in-memory service");
    let status = j!(s.loop_status(Parameters(Default::default())));
    let served = &status["served_by"];

    // In a test process the exe is present and unchanged, so this is the
    // current branch — the one that must stay quiet.
    if served["stale"] == serde_json::Value::Bool(false) {
        assert!(
            served.get("stale_note").is_none(),
            "a current server carries the FACT and drops the essay: {served}"
        );
        let next = status["next"].as_array().expect("next is a list");
        for item in next {
            let s = item.as_str().unwrap_or_default();
            assert!(
                !s.contains("NOT THE BINARY ON DISK") && !s.contains("CANNOT TELL"),
                "a current server must not add currency debt to next: {s}"
            );
        }
    }
}

/// The wording an agent would ACT on, asserted without arranging a replaced
/// binary — the same reason `STALE_NOTE` is public. A branch that can only be
/// checked by hand is the vacuous test this file already refuses.
#[test]
fn the_next_entries_name_the_remedy_and_the_trap() {
    // The remedy, exactly.
    assert!(STALE_NEXT.contains("--stop-shared"), "{STALE_NEXT}");
    // AND the trap that produced all four instances: restarting is not enough.
    assert!(
        STALE_NEXT.contains("SESSION RESTART ALONE DOES NOT"),
        "the entry must kill the assumption that a restart fixes it: {STALE_NEXT}"
    );
    assert!(STALE_NEXT.contains("re-attaches"), "{STALE_NEXT}");
    // Writes are safe — said out loud, so nobody panics about the graph.
    assert!(STALE_NEXT.contains("WRITES are unaffected"), "{STALE_NEXT}");

    // Unknown is not false, and must not be phrased as if it were.
    assert!(UNKNOWN_NEXT.contains("CANNOT TELL"), "{UNKNOWN_NEXT}");
    assert!(
        UNKNOWN_NEXT.contains("not `false`"),
        "unknown must not read as a clean bill: {UNKNOWN_NEXT}"
    );
}
