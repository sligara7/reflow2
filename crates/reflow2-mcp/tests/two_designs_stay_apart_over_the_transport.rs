//! Two designs served by ONE process, addressed as `/g/<graph_id>/`, stay apart.
//!
//! ⭐ WHY THIS IS AN END-TO-END TEST AND NOT A UNIT ONE. `sessions_cannot_cross_designs`
//! and `a_session_names_its_design` already prove the mechanism against the
//! crate: `Registry::attach` refuses an unknown id, refuses a path as an unknown
//! id, and refuses a real design outside the root identically. What none of them
//! could prove until 2026-09-13 is that the TRANSPORT routes to the design it
//! named — because there was no transport. `Registry::discover` had one caller
//! class, and it was tests.
//!
//! 🛑 THE PROPERTY THIS GUARDS FAILS SILENTLY. flo2's own tracker states it:
//! "a handler that reaches the wrong graph corrupts a design rather than
//! erroring, so this wants property tests, not examples." A leak here does not
//! raise; it writes one customer's design into another's.
//!
//! WHAT IS ASSERTED, in the order the damage would happen:
//!   · a request naming no design is refused and SAYS what would have worked
//!   · each design answers on its own prefix
//!   · a node written into A is readable in A
//!   · ...and is ABSENT from B — the breach that corrupts rather than errors
//!   · A's session id is not valid under B, because the session tables are
//!     per-design rather than shared
//!   · an unknown id is refused by name
//!   · a PATH is refused exactly as an unknown id is

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A registry root nothing else in the suite can collide with. Same idiom as
/// `sessions_cannot_cross_designs`: two tests sharing a path would take the
/// same RocksDB lock and look like a hang rather than a collision.
fn tmp_root() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-registry-http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    // The integration-test binary sits beside the built one.
    let mut p = std::env::current_exe().expect("test binary has a path");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("reflow2-mcp")
}

/// Mint a design by running the binary over stdio once, and return its graph_id.
fn mint(dir: &Path) -> String {
    let store = dir.join(".reflow2").join("graph");
    let mut child = Command::new(bin())
        .arg("--graph-path")
        .arg(&store)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn reflow2-mcp to mint a design");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"mint","version":"1"}}}}}}"#
        )
        .expect("write initialize");
    }
    // CLOSE STDIN so the child sees EOF and exits. Without this it waits for
    // more JSON-RPC that never comes and the test pays the full timeout per
    // design — which is what made the first run take two minutes.
    drop(child.stdin.take());
    let _ = child.wait_timeout_kill(Duration::from_secs(60));
    let id_file = dir.join(".reflow2").join("graph.id.json");
    let raw = std::fs::read_to_string(&id_file)
        .unwrap_or_else(|e| panic!("no identity sidecar at {}: {e}", id_file.display()));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("identity sidecar is json");
    v["graph_id"]
        .as_str()
        .expect("identity sidecar names a graph_id")
        .to_string()
}

trait WaitKill {
    fn wait_timeout_kill(&mut self, d: Duration) -> Option<std::process::ExitStatus>;
}
impl WaitKill for Child {
    fn wait_timeout_kill(&mut self, d: Duration) -> Option<std::process::ExitStatus> {
        let start = Instant::now();
        loop {
            match self.try_wait() {
                Ok(Some(s)) => return Some(s),
                Ok(None) if start.elapsed() < d => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    let _ = self.kill();
                    return None;
                }
            }
        }
    }
}

/// A registry server on an OS-assigned port, and the port it landed on.
struct Server {
    child: Child,
    port: u16,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_registry(root: &Path) -> Server {
    let mut child = Command::new(bin())
        .arg("--registry-root")
        .arg(root)
        .arg("--http")
        .arg("127.0.0.1:0")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the registry server");

    // Read the bound port off the server's OWN banner rather than guessing or
    // sleeping: the port is OS-assigned, and a fixed one would collide with a
    // parallel run of this suite.
    //
    // ⚠️ AND KEEP DRAINING IT AFTERWARDS, on a thread. Reading until the port
    // appears and then dropping the reader closes the pipe under a server that
    // is still writing to it — the next stderr write gets EPIPE and the server
    // dies, which presents as a connection that establishes and never answers.
    // That is exactly how the first version of this test failed.
    let stderr = child.stderr.take().expect("stderr piped");
    let (tx, rx) = std::sync::mpsc::channel::<u16>();
    std::thread::spawn(move || {
        let mut sent = false;
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if !sent && let Some(i) = line.find("http://127.0.0.1:") {
                let tail = &line[i + "http://127.0.0.1:".len()..];
                let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(p) = digits.parse::<u16>() {
                    let _ = tx.send(p);
                    sent = true;
                }
            }
            // keep consuming so the pipe never fills and never closes early
        }
    });
    let port = rx
        .recv_timeout(Duration::from_secs(90))
        .expect("the server prints the address it bound within 90s");
    Server { child, port }
}

/// One HTTP POST to the server, returning (status, session id, body).
fn post(port: u16, path: &str, body: &str, session: Option<&str>) -> (u16, Option<String>, String) {
    use std::io::Read;
    use std::net::TcpStream;
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(120))).ok();
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(sid) = session {
        req.push_str(&format!("mcp-session-id: {sid}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).expect("write request");
    let mut raw = String::new();
    let _ = s.read_to_string(&mut raw);
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let sid = raw.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim().eq_ignore_ascii_case("mcp-session-id")).then(|| v.trim().to_string())
    });
    (status, sid, raw)
}

fn open_session(port: u16, prefix: &str) -> String {
    let (status, sid, body) = post(
        port,
        prefix,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#,
        None,
    );
    assert_eq!(status, 200, "initialize on {prefix} failed: {body}");
    let sid = sid.unwrap_or_else(|| panic!("no session id from {prefix}"));
    let _ = post(
        port,
        prefix,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        Some(&sid),
    );
    sid
}

fn call(port: u16, prefix: &str, sid: &str, tool: &str, args: &str) -> String {
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#
    );
    post(port, prefix, &body, Some(sid)).2
}

#[test]
fn two_designs_under_one_root_stay_apart_over_the_transport() {
    let root = tmp_root();
    let root = root.as_path();
    let a_dir = root.join("alpha");
    let b_dir = root.join("beta");
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = mint(&a_dir);
    let b = mint(&b_dir);
    assert_ne!(a, b, "two designs must not share a graph_id");

    let server = start_registry(root);
    let port = server.port;
    let pa = format!("/g/{a}/");
    let pb = format!("/g/{b}/");

    // ① A request naming no design is refused AND says what would have worked.
    //    A bare 404 is the failure this repo keeps meeting: a session cannot
    //    tell "reflow2 is not here" from "you addressed it wrongly".
    let (status, _, body) = post(
        port,
        "/",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        None,
    );
    assert_eq!(
        status, 404,
        "a request naming no design must not be served one. Raw response was:\n{body}"
    );
    assert!(
        body.contains("/g/<graph_id>/"),
        "it must say the shape: {body}"
    );
    assert!(
        body.contains(&a) && body.contains(&b),
        "and list what is actually there, so the reader can pick: {body}"
    );

    // ② Each design answers on its own prefix.
    let sa = open_session(port, &pa);
    let sb = open_session(port, &pb);

    // ③ A write into A is readable in A.
    //
    // A Requirement, deliberately: it needs nothing to already exist in the
    // store. The first version wrote a TemporalFact whose `subject_id` pointed
    // at itself, which a freshly minted design correctly refuses — and the test
    // then failed for that reason rather than for isolation, which is the kind
    // of false red that gets a real test deleted.
    let wrote = call(
        port,
        &pa,
        &sa,
        "add_requirement",
        r#"{"id":"req:only-in-alpha","name":"only in alpha","statement":"this belongs to alpha"}"#,
    );
    assert!(
        wrote.contains("req:only-in-alpha"),
        "the write into A must have landed before isolation means anything: {wrote}"
    );
    let seen_in_a = call(port, &pa, &sa, "get_node", r#"{"id":"req:only-in-alpha"}"#);
    assert!(
        seen_in_a.contains("this belongs to alpha"),
        "the design that was written to must hold it: {seen_in_a}"
    );

    // ④ THE ONE THAT CORRUPTS RATHER THAN ERRORS. B must not see it.
    let seen_in_b = call(port, &pb, &sb, "get_node", r#"{"id":"req:only-in-alpha"}"#);
    assert!(
        !seen_in_b.contains("this belongs to alpha"),
        "ISOLATION BREACH: design B returned design A's node: {seen_in_b}"
    );

    // ⑤ A's session id is not valid under B. The session tables are per-design,
    //    so a session cannot be carried across by reusing its id.
    let crossed = call(port, &pb, &sa, "get_node", r#"{"id":"req:only-in-alpha"}"#);
    assert!(
        !crossed.contains("this belongs to alpha"),
        "SESSION CROSSED DESIGNS: A's session read A's node through B's prefix: {crossed}"
    );

    // ⑥ An unknown id is refused BY NAME.
    let (status, _, body) = post(
        port,
        "/g/no-such-design/",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        None,
    );
    assert_eq!(status, 404);
    assert!(
        body.contains("no-such-design"),
        "the refusal must name what was asked for: {body}"
    );

    // ⑦ A PATH is refused exactly as an unknown id is — a filesystem route must
    //    not be a second way in. This is the crate-level property
    //    (`attach` treats its argument as an id and nothing else) holding all
    //    the way out at the transport.
    let (status, _, _) = post(
        port,
        "/g/..%2f..%2fetc/",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        None,
    );
    assert_eq!(status, 404, "a path must be refused like any unknown id");
}
