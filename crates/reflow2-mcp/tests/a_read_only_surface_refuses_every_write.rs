//! A read-only surface refuses every write, and refuses them at the ONE place a
//! write cannot avoid.
//!
//! `req:the-hosted-surface-is-read-only-so-it-can-ship-before-authentication-exists`,
//! accepted by Anthony 2026-08-26. The argument the requirement makes, in short:
//! the transport is BUILT (`cap:shared-sessions`, verified over real HTTP) and
//! cross-machine reach is BUILT (`cap:remote-sessions`, verified) — what has
//! never existed is any answer to *who is calling*. A read-only surface splits
//! that exposure in two and answers one half outright:
//!
//!   INTEGRITY       can a reacher CORRUPT the design?  NO — there is no write
//!                   to attribute, so the caller-supplied `contributor_id` that
//!                   would otherwise be an unverified claim over a network is
//!                   never accepted at all.
//!   CONFIDENTIALITY can a reacher SEE the design?      YES. Not eliminated —
//!                   RELOCATED to the network, which is what the tailnet is for.
//!
//! ⭐ WHY THE ENFORCEMENT POINT IS `write_lock` AND NOT A LIST OF TOOL NAMES.
//! A write cannot happen without taking the write guard, so refusing to hand one
//! out refuses every write that exists today AND every write anybody adds later,
//! including one whose author never heard of read-only mode. The alternative — a
//! list of mutating tool names, or a check keyed on `readOnlyHint` — is a rule
//! maintained by hand with nothing checking it, which is the exact defect class
//! this project spent 2026-08-26 fixing three times over.
//!
//! The compiler proved the sweep complete: making `write_lock` fallible turned
//! all 98 call sites into a compile error until each was updated, and it caught
//! a second assembly point (`share()`) where read-only had to be INHERITED —
//! a read-only server that handed each new client session a writable service
//! would have been read-only in name only.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

fn req(id: &str) -> CreateNodeReq {
    serde_json::from_value(serde_json::json!({
        "node_type": "Decision",
        "id": id,
        "props": { "name": "n", "decision": "d" },
    }))
    .expect("request")
}

#[tokio::test]
async fn a_read_only_service_refuses_a_write() {
    let s = ReflowService::in_memory()
        .expect("service")
        .into_read_only();

    let out = s.create_node(Parameters(req("dec:blocked"))).await;

    assert!(
        out.is_err(),
        "a read-only surface must REFUSE a write, not perform it — this is the \
         whole reason the surface can ship before authentication exists"
    );
}

#[tokio::test]
async fn a_read_only_service_still_serves_reads() {
    // The load-bearing counterweight. A read-only surface that refused reads too
    // would be no surface at all, and the requirement's entire argument is that
    // reading is the thing being offered.
    let s = ReflowService::in_memory()
        .expect("service")
        .into_read_only();

    let out = s.loop_status(Parameters(Default::default())).await;

    assert!(
        out.is_ok(),
        "a read-only surface must still answer reads: {:?}",
        out.err()
    );
}

#[tokio::test]
async fn a_normal_service_still_writes() {
    // The other counterweight, and the one that catches the fix going too far:
    // if read-only were on by default, or if `write_lock` refused unconditionally,
    // every local session would break and this is what would say so.
    let s = ReflowService::in_memory().expect("service");

    assert!(!s.is_read_only(), "a service must be writable unless asked");
    let out = s.create_node(Parameters(req("dec:allowed"))).await;
    assert!(
        out.is_ok(),
        "an ordinary service must still write: {:?}",
        out.err()
    );
}

#[tokio::test]
async fn a_session_on_a_read_only_server_is_read_only_too() {
    // ⭐ THE CASE THE COMPILER FOUND. `share()` mints a service per CLIENT
    // SESSION (`req:sessions-share-a-graph`), so this is the path every
    // connection to a shared server takes. If read-only were reset here rather
    // than inherited, the mode would hold for the process and evaporate for
    // every actual client — which is the only way anybody would ever reach it.
    let server = ReflowService::in_memory()
        .expect("service")
        .into_read_only();
    let session = server.share();

    assert!(
        session.is_read_only(),
        "a session must inherit the server's mode"
    );
    assert!(
        session
            .create_node(Parameters(req("dec:via-session")))
            .await
            .is_err(),
        "a client session on a read-only server must refuse writes too"
    );
}
