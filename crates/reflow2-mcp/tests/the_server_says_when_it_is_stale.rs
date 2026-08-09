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
