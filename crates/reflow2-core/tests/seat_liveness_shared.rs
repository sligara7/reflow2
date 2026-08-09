//! Liveness of a seat a SHARED server handed out but never leased.
//!
//! Its own test binary on purpose. `declare_serving_many_sessions` is a one-way
//! latch on a process-global, because a server does not stop being shared
//! halfway through its life — so a test that sets it would change the answer for
//! every other test sharing the binary, and `tests/claims.rs` deliberately
//! asserts the stdio reading of the very same call. Two processes, two modes, no
//! ordering to get right.
//!
//! WHAT THIS PINS, and why it is not the same thing #100 pinned. The 2026-08-08
//! registry made a LEASED seat answer about its session: attached is `live`,
//! dropped is `gone`. That works. But the `mint_seat` tool hands out a seat it
//! never leases, and that seat fell through to the pid probe — which under
//! `--shared` asks whether the DAEMON is alive and answers `live` for every seat
//! it ever minted, forever. Reported by dev_storyflow w-aa0607ff 2026-08-08,
//! reproduced here: the fix had landed on the transport that did not need it.

use reflow2_core::identity::{
    Liveness, declare_serving_many_sessions, mint_seat, register_seat, release_seat, seat_liveness,
};

/// The defect, stated as the assertion that used to fail.
///
/// A handed-out seat carries this process's pid. On a shared server that pid
/// says only that we answered the call, so `live` is not an answer — and it is
/// the one answer a colleague reads as "somebody is in here".
#[test]
fn a_seat_a_shared_server_handed_out_is_unknown_not_live() {
    declare_serving_many_sessions();

    let handed_out = mint_seat();
    assert_eq!(
        seat_liveness(&handed_out),
        Liveness::Unknown,
        "THE DEFECT: this read `live`, because the pid in the seat is the serving process's \
         and the serving process is what answered. `gone` was unreachable for it, so the \
         field could never produce its negative value"
    );
}

/// The flag must not swallow the case the registry genuinely answers, or the
/// fix for the leased seat would be undone by the fix for the handed-out one.
#[test]
fn a_leased_seat_still_answers_about_its_session_on_a_shared_server() {
    declare_serving_many_sessions();

    let leased = mint_seat();
    register_seat(&leased);
    assert_eq!(
        seat_liveness(&leased),
        Liveness::Live,
        "an attached session is live even on a shared server — the registry knows this one"
    );

    release_seat(&leased);
    assert_eq!(
        seat_liveness(&leased),
        Liveness::Gone,
        "and a departed one is a ghost: `unknown` must not become the answer to everything"
    );
}

/// `unknown` earns its keep only if it is distinguishable from `gone`, because
/// the two license opposite behaviour: a ghost is excluded from overlaps, an
/// unknown is not. Taking work somebody is doing is the expensive mistake.
#[test]
fn unknown_is_not_gone() {
    declare_serving_many_sessions();

    let handed_out = mint_seat();
    assert_ne!(
        seat_liveness(&handed_out),
        Liveness::Gone,
        "a seat we cannot observe must not be reported as a ghost — that would invite a \
         colleague to take a region somebody is actively holding"
    );
}
