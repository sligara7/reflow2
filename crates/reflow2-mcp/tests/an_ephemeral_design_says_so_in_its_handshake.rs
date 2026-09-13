//! An ephemeral design tells every session that it will not survive.
//!
//! ⭐ WHY THE HANDSHAKE AND NOT ONLY STDERR. The operator who typed
//! `--ephemeral` saw the banner. An AGENT that connects over HTTP never sees
//! stderr at all, and the handshake instructions are the one channel that
//! reaches every session unasked. A session that does not know its design is
//! ephemeral is a session that will lose work and could not have known.
//!
//! ⚠️ THE CONDITION IS `graph_path: None`, WHICH IS THE SAME FACT rather than a
//! second flag that could drift from it: no directory backs this design is
//! exactly what ephemeral means. `share()` copies `graph_path`, so the warning
//! reaches per-session services too — which is the case that actually matters,
//! since every HTTP session gets one of those rather than the original.

use reflow2_mcp::service::ReflowService;
use rmcp::ServerHandler;

const MARKER: &str = "THIS DESIGN IS EPHEMERAL";

#[test]
fn an_in_memory_design_warns_in_its_handshake() {
    let svc = ReflowService::in_memory().expect("an in-memory design opens");
    let info = svc.get_info();
    let instructions = info
        .instructions
        .expect("the handshake carries instructions");
    assert!(
        instructions.contains(MARKER),
        "an ephemeral design must say so in the handshake; it said: {}",
        &instructions[..instructions.len().min(200)]
    );
    assert!(
        instructions.starts_with('🛑'),
        "and it must come FIRST — an agent that reads only the opening of a long \
         instruction block still has to see it"
    );
}

#[test]
fn the_warning_survives_being_shared_into_a_session() {
    // THE CASE THAT ACTUALLY MATTERS. Every HTTP session is served by `share()`,
    // not by the original service, so a warning that did not propagate would be
    // seen by nobody who could act on it.
    let svc = ReflowService::in_memory().expect("an in-memory design opens");
    let session = svc.share();
    let instructions = session
        .get_info()
        .instructions
        .expect("the handshake carries instructions");
    assert!(
        instructions.contains(MARKER),
        "the per-session service must carry the ephemeral warning too"
    );
}

#[test]
fn a_design_on_disk_is_not_labelled_ephemeral() {
    // The other half: the warning must not fire on an ordinary design, or it
    // becomes noise that readers learn to skip — and then it is not there when
    // it is true.
    let dir = std::env::temp_dir().join(format!(
        "reflow2-not-ephemeral-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = dir.join("graph");
    let svc = ReflowService::new_reporting(store.to_str().unwrap())
        .expect("a design on disk opens")
        .0;
    let instructions = svc
        .get_info()
        .instructions
        .expect("the handshake carries instructions");
    assert!(
        !instructions.contains(MARKER),
        "a persistent design must NOT be labelled ephemeral"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
