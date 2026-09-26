//! A local client works against a remote reflow2 server by URL.
//!
//! req:a-local-client-works-against-a-remote-reflow2-server-by-url, step 1
//! (2026-09-26). `reflow2-mcp --remote <url>` speaks stdio to the agent exactly
//! as a local session does and forwards to a reflow2 somewhere else — flo2's
//! api.flo2.io, or an organization's own server — carrying the person's key.
//!
//! What must hold, and what each test pins:
//! · the key reaches the server as `Authorization: Bearer`, read from the
//!   environment variable or file NAMED on the command line, never the key itself;
//! · without a key, no Authorization header is sent at all;
//! · the key appears in nothing the agent or a log reader sees;
//! · the session id the server hands out is quoted on every later request;
//! · a reply the server pretty-printed reaches the agent as ONE line;
//! · a refused key (401) is a readable JSON-RPC error, not a hung call;
//! · a key is never sent in the clear to another machine.
//!
//! The server here is a stand-in that records what it was sent, because what
//! is under test is the client's side of the wire. The whole chain — client,
//! flo2's gateway and a real reflow2 — was driven by hand on 2026-09-26 and a
//! write through it came back credited to the key's owner.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const KEY: &str = "r2k_test_7f3a9c-never-print-me";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_reflow2-mcp")
}

/// One recorded request: its lower-cased headers and its body.
#[derive(Clone, Debug)]
struct Seen {
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// How the stand-in answers.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// A working server: sessions numbered s-1, s-2, … per handshake.
    Works,
    /// Every request refused, 401.
    RefusesTheKey,
    /// The first tools/list gets the transport's 404 for a forgotten session.
    ForgetsTheSessionOnce,
    /// Every tools/list fails with a 500.
    FailsEveryList,
}

/// A stand-in MCP server on loopback, answering as `mode` says. Every reply
/// is indented, as flo2's gateway indents them.
fn stand_in(mode: Mode) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/g/abc/mcp", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
                }
            }
            let len: usize = headers
                .iter()
                .find(|(k, _)| k == "content-length")
                .and_then(|(_, v)| v.parse().ok())
                .unwrap_or(0);
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).unwrap();
            let body = String::from_utf8(body).unwrap();
            let msg: serde_json::Value = serde_json::from_str(&body).unwrap();
            record.lock().unwrap().push(Seen { headers, body });

            let is_list = msg["method"] == "tools/list";
            let lists_so_far = record
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.body.contains("tools/list"))
                .count();
            let (code, extra, reply) = match (mode, msg.get("id")) {
                (Mode::RefusesTheKey, _) => (
                    401,
                    "www-authenticate: Bearer\r\n".to_string(),
                    r#"{"error":"invalid_token"}"#.to_string(),
                ),
                (Mode::ForgetsTheSessionOnce, _) if is_list && lists_so_far == 1 => (
                    404,
                    String::new(),
                    "Not Found: Session not found".to_string(),
                ),
                (Mode::FailsEveryList, _) if is_list => (500, String::new(), String::new()),
                (_, None) => (202, String::new(), String::new()),
                (_, Some(id)) => {
                    let (result, extra) = if msg["method"] == "initialize" {
                        let hellos = record
                            .lock()
                            .unwrap()
                            .iter()
                            .filter(|s| s.body.contains("\"initialize\""))
                            .count();
                        (
                            serde_json::json!({"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                                               "serverInfo": {"name": "stand-in", "version": "0"}}),
                            format!("mcp-session-id: s-{hellos}\r\n"),
                        )
                    } else {
                        (serde_json::json!({"tools": []}), String::new())
                    };
                    let reply = serde_json::to_string_pretty(
                        &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
                    )
                    .unwrap();
                    (200, extra, reply)
                }
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    (url, seen)
}

const HELLO: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
const READY: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const LIST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

/// Run the client over one scripted session; return (stdout lines, stderr).
fn session(args: &[&str], env: &[(&str, &str)]) -> (Vec<String>, String, bool) {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .env_remove("REFLOW2_TEST_KEY")
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("the binary runs");
    {
        let mut stdin = child.stdin.take().unwrap();
        // One at a time is what an agent does: the handshake answers before
        // anything else is sent.
        writeln!(stdin, "{HELLO}").unwrap();
        writeln!(stdin, "{READY}").unwrap();
        writeln!(stdin, "{LIST}").unwrap();
    }
    let out = child
        .wait_with_output()
        .expect("the client exits when stdin ends");
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    (
        lines,
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

fn each_line_is_one_message(lines: &[String]) {
    for l in lines {
        serde_json::from_str::<serde_json::Value>(l)
            .unwrap_or_else(|e| panic!("stdout line is not one JSON message ({e}): {l:?}"));
    }
}

#[test]
fn the_key_named_by_an_environment_variable_is_sent_and_shown_nowhere() {
    let (url, seen) = stand_in(Mode::Works);
    let (lines, stderr, ok) = session(
        &["--remote", &url, "--api-key-env", "REFLOW2_TEST_KEY"],
        &[("REFLOW2_TEST_KEY", KEY)],
    );
    assert!(ok, "the client failed:\n{stderr}");
    assert_eq!(
        lines.len(),
        2,
        "one reply per request, one line each: {lines:?}"
    );
    each_line_is_one_message(&lines);
    assert!(lines[1].contains("\"tools\""));

    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        3,
        "initialize, initialized, tools/list: {seen:?}"
    );
    for s in &seen {
        assert_eq!(
            s.header("authorization"),
            Some(format!("Bearer {KEY}").as_str()),
            "{}",
            s.body
        );
    }
    assert_eq!(
        seen[0].header("mcp-session-id"),
        None,
        "the handshake opens the session"
    );
    assert_eq!(seen[1].header("mcp-session-id"), Some("s-1"));
    assert_eq!(seen[2].header("mcp-session-id"), Some("s-1"));

    assert!(
        !stderr.contains(KEY),
        "the key leaked into the log:\n{stderr}"
    );
    assert!(
        !lines.iter().any(|l| l.contains(KEY)),
        "the key leaked to the agent"
    );
}

#[test]
fn the_key_named_by_a_file_is_sent_trimmed() {
    let (url, seen) = stand_in(Mode::Works);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("key");
    std::fs::write(&file, format!("{KEY}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let (lines, stderr, ok) = session(
        &["--remote", &url, "--api-key-file", file.to_str().unwrap()],
        &[],
    );
    assert!(ok, "the client failed:\n{stderr}");
    each_line_is_one_message(&lines);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen[2].header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    assert!(
        !stderr.contains(KEY),
        "the key leaked into the log:\n{stderr}"
    );
}

#[test]
fn without_a_key_no_authorization_header_is_sent() {
    let (url, seen) = stand_in(Mode::Works);
    let (lines, stderr, ok) = session(&["--remote", &url], &[]);
    assert!(ok, "the client failed:\n{stderr}");
    assert_eq!(lines.len(), 2);
    for s in seen.lock().unwrap().iter() {
        assert_eq!(s.header("authorization"), None);
    }
}

#[test]
fn a_refused_key_is_a_readable_error_not_a_hung_call() {
    let (url, _) = stand_in(Mode::RefusesTheKey);
    let (lines, stderr, ok) = session(
        &["--remote", &url, "--api-key-env", "REFLOW2_TEST_KEY"],
        &[("REFLOW2_TEST_KEY", KEY)],
    );
    assert!(ok, "a refusal is an answer, not a crash:\n{stderr}");
    assert_eq!(lines.len(), 2, "both requests are answered: {lines:?}");
    each_line_is_one_message(&lines);
    let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(first["id"], 1);
    let msg = first["error"]["message"]
        .as_str()
        .expect("a JSON-RPC error");
    assert!(msg.contains("401"), "{msg}");
    assert!(msg.contains("refused the credential"), "{msg}");
    assert!(
        !lines.iter().any(|l| l.contains(KEY)),
        "the key leaked to the agent"
    );
    assert!(!stderr.contains(KEY), "the key leaked into the log");
}

#[test]
fn a_key_is_never_sent_in_the_clear_to_another_machine() {
    let (_, stderr, ok) = session(
        &[
            "--remote",
            "http://reflow2.example.org/g/abc/mcp",
            "--api-key-env",
            "REFLOW2_TEST_KEY",
        ],
        &[("REFLOW2_TEST_KEY", KEY)],
    );
    assert!(
        !ok,
        "a plain-http key to a non-loopback host must be refused"
    );
    assert!(
        stderr.contains("https"),
        "the refusal says what to use instead:\n{stderr}"
    );
    assert!(!stderr.contains(KEY));
}

#[test]
fn a_named_variable_that_is_unset_is_refused_up_front() {
    let (_, stderr, ok) = session(
        &[
            "--remote",
            "https://api.flo2.io/mcp",
            "--api-key-env",
            "REFLOW2_TEST_KEY",
        ],
        &[],
    );
    assert!(!ok);
    assert!(
        stderr.contains("REFLOW2_TEST_KEY"),
        "the refusal names the variable:\n{stderr}"
    );
}

#[test]
fn the_key_itself_is_not_accepted_on_the_command_line() {
    let out = Command::new(bin())
        .args(["--remote", "https://api.flo2.io/mcp", "--api-key", KEY])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "a key typed on the command line lands in shell history"
    );
}

#[test]
fn a_server_that_forgot_the_session_is_rejoined_once() {
    let (url, seen) = stand_in(Mode::ForgetsTheSessionOnce);
    let (lines, stderr, ok) = session(&["--remote", &url], &[]);
    assert!(ok, "the client failed:\n{stderr}");
    assert_eq!(
        lines.len(),
        2,
        "the list is answered after re-joining: {lines:?}"
    );
    assert!(lines[1].contains("\"tools\""), "{}", lines[1]);
    let seen = seen.lock().unwrap().clone();
    let lists: Vec<_> = seen
        .iter()
        .filter(|s| s.body.contains("tools/list"))
        .collect();
    assert_eq!(lists.len(), 2, "one refused, one retried");
    assert_eq!(lists[0].header("mcp-session-id"), Some("s-1"));
    assert_eq!(
        lists[1].header("mcp-session-id"),
        Some("s-2"),
        "retried on the NEW session"
    );
}

#[test]
fn any_other_failure_is_not_resent() {
    // A change that failed for any other reason may have landed; sending it
    // again could apply it twice. Only a forgotten session is retried.
    let (url, seen) = stand_in(Mode::FailsEveryList);
    let (lines, stderr, ok) = session(&["--remote", &url], &[]);
    assert!(ok, "the client failed:\n{stderr}");
    let answer: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    assert!(
        answer["error"]["message"].as_str().unwrap().contains("500"),
        "{answer}"
    );
    let lists = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.body.contains("tools/list"))
        .count();
    assert_eq!(lists, 1, "a failed request is sent exactly once");
}
