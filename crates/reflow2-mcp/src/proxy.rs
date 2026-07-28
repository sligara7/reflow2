//! The session half of shared mode: speak stdio to the client, HTTP to the
//! shared server.
//!
//! Why a proxy at all, when MCP clients can dial HTTP directly? Because the
//! thing being protected is `req:never-silently-absent`. A client configured
//! with a bare URL and nothing listening gets connection-refused, and a session
//! that cannot reach its design brain is indistinguishable from one where
//! reflow2 was never configured — the exact outage the degraded surface exists
//! to end, moved one layer out where reflow2 cannot answer for itself. Keeping a
//! process on stdio means **something is always there to explain what happened**,
//! whatever state the server is in. It also keeps the config identical for every
//! MCP client, including the ones that speak only stdio.
//!
//! What makes this proxy simple enough to be trustworthy: **the reflow2 server
//! never initiates a message.** There are no server-side notifications, sampling
//! requests or progress pushes — verified against the service, not assumed — so
//! every byte on the wire belongs to a request/response pair the client started.
//! A transparent forwarder is therefore complete, not a subset that works until
//! someone adds a push. If that ever changes, this file is where it breaks, and
//! it breaks loudly (a message with no request to answer).

use anyhow::{Context, bail};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

/// A parsed `http://host:port/path` — enough of a URL for loopback.
struct Endpoint {
    authority: String,
    path: String,
}

fn parse_endpoint(url: &str) -> anyhow::Result<Endpoint> {
    let rest = url
        .strip_prefix("http://")
        .context("the shared server URL must start with http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        bail!("the shared server URL has no host");
    }
    Ok(Endpoint {
        authority: authority.to_string(),
        path: path.to_string(),
    })
}

/// How long a probe waits before calling a server unreachable.
///
/// Short: the question is only "is somebody answering here", and a wedged server
/// must not be able to hold up the election. This bound is what makes
/// `READY_TIMEOUT` real — a deadline consulted between iterations of a loop
/// cannot fire while an iteration never returns.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a forwarded tool call may take before the session is told it failed.
///
/// Generous on purpose: real work happens behind these calls (a detector sweep
/// over a large design, a full export), and a bound that fires on legitimate
/// work would be worse than no bound. What it rules out is the *unbounded* case.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(300);

/// One request, with a deadline.
///
/// **Every network step here is bounded, and that is a fix rather than a
/// flourish.** The first version had no timeout anywhere: `connect`,
/// `send_request` and `collect` could each block forever, so a server that
/// accepted the connection and then never answered (wedged mid-index-build,
/// stuck in RocksDB, SIGSTOPed) hung the session instead of degrading — and a
/// hang is strictly worse than the outage it replaces, because
/// `reflow2_unavailable` at least says something. Found by review before it
/// could happen to anyone (`w-74c2989e`, 2026-07-27), and it is the same class
/// this project already ruled binding elsewhere: never propagate the hang class
/// into the surfaces built to detect hangs.
async fn post(
    url: &str,
    session: Option<&str>,
    body: String,
    limit: Duration,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    tokio::time::timeout(limit, post_inner(url, session, body))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "the shared reflow2 server at {url} accepted the request but did not answer within \
                 {}s. It may be wedged; `reflow2-mcp --graph-path <graph> --stop-shared` clears it.",
                limit.as_secs()
            )
        })?
}

/// One request, one connection. Deliberate: this is loopback traffic at
/// human-interaction rates, so a connection pool would be complexity bought
/// against a cost nobody can measure — and a pooled connection that has gone
/// stale is a failure mode that shows up as an unexplained tool error much later.
async fn post_inner(
    url: &str,
    session: Option<&str>,
    body: String,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    let ep = parse_endpoint(url)?;
    let stream = tokio::net::TcpStream::connect(&ep.authority)
        .await
        .with_context(|| {
            format!(
                "could not reach the shared reflow2 server at {}",
                ep.authority
            )
        })?;
    let io = hyper_util::rt::TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("HTTP handshake with the shared reflow2 server failed")?;
    tokio::spawn(async move {
        // The connection task ends when the response is done; a debug line is
        // right here because a closed keep-alive is normal, not an incident.
        if let Err(e) = conn.await {
            tracing::debug!("connection to the shared server ended: {e}");
        }
    });

    let mut req = Request::builder()
        .method("POST")
        .uri(&ep.path)
        .header("host", &ep.authority)
        .header("content-type", "application/json")
        // Both, because the transport chooses: a single JSON body or an SSE
        // frame carrying the same message. Advertising only one would make the
        // server's choice a failure.
        .header("accept", "application/json, text/event-stream");
    if let Some(s) = session {
        req = req.header("mcp-session-id", s);
    }
    let req = req
        .body(Full::new(Bytes::from(body)))
        .context("could not build the request to the shared server")?;

    let res = sender
        .send_request(req)
        .await
        .context("the shared reflow2 server did not answer")?;
    let session_id = res
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let status = res.status();
    let collected = res
        .into_body()
        .collect()
        .await
        .context("could not read the shared server's reply")?
        .to_bytes();
    let text = String::from_utf8_lossy(&collected).to_string();
    if !status.is_success() {
        bail!("the shared reflow2 server answered {status}: {text}");
    }
    Ok((extract_messages(&text), session_id))
}

/// Pull JSON-RPC messages out of a reply that may be plain JSON or SSE frames.
///
/// Kept as a pure function so it can be tested without a server — the SSE shape
/// (`data:` lines, interleaved with `id:`/`retry:` and blank keep-alives) is
/// exactly the kind of thing that is easy to get subtly wrong and hard to notice,
/// because a dropped frame looks like a hung tool call rather than a parse bug.
pub fn extract_messages(body: &str) -> Vec<String> {
    let looks_like_sse = body
        .lines()
        .any(|l| l.starts_with("data:") || l.starts_with("event:"));
    if !looks_like_sse {
        let t = body.trim();
        return if t.is_empty() {
            Vec::new()
        } else {
            vec![t.to_string()]
        };
    }
    body.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim)
        // The transport opens a stream with an empty `data:` line; it is a
        // keep-alive, not a message, and forwarding it would put a blank line
        // into the client's JSON-RPC channel.
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Is a reflow2 server answering here?
///
/// Completes a real MCP `initialize` rather than settling for a TCP connect. The
/// distinction is load-bearing: the shared server's port is OS-assigned, so a
/// rendezvous record that has gone stale can name a port the kernel has since
/// handed to something unrelated. Attaching to *that* would produce a session
/// whose every tool call fails in a way that reads as a reflow2 bug.
pub async fn probe_server_async(url: &str) -> anyhow::Result<bool> {
    let hello = serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "reflow2-attach-probe", "version": env!("CARGO_PKG_VERSION")}
        }
    })
    .to_string();
    let (messages, _) = post(url, None, hello, PROBE_TIMEOUT).await?;
    Ok(messages.iter().any(|m| {
        serde_json::from_str::<serde_json::Value>(m)
            .ok()
            .and_then(|v| {
                v.get("result")?
                    .get("serverInfo")?
                    .get("name")?
                    .as_str()
                    .map(|n| n.contains("reflow2"))
            })
            .unwrap_or(false)
    }))
}

/// Everything a session needs to keep talking to its design, including after the
/// server it was talking to goes away.
struct Upstream {
    graph_path: String,
    url: Mutex<String>,
    session: Mutex<Option<String>>,
    /// The client's own `initialize`, kept verbatim so a replacement server can
    /// be brought up to the same state. Replaying the client's message rather
    /// than a synthesised one matters: the handshake carries the client's
    /// protocol version and capabilities, and inventing those would make the new
    /// server answer a different client than the one actually connected.
    hello: Mutex<Option<String>>,
}

impl Upstream {
    /// Send one message, and if the server has vanished, get a new one and try
    /// once more.
    ///
    /// This is what makes a shared server safe to let expire. Without it, an
    /// idle-exited daemon would strand every attached session with tool calls
    /// that fail forever, and the only remedy would be restarting the sessions —
    /// so the daemon would have to be immortal, and an immortal daemon holds the
    /// store's write lock against every CLI use of the graph. One retry buys the
    /// server a lifetime.
    async fn send(&self, body: String) -> anyhow::Result<Vec<String>> {
        let url = self.url.lock().await.clone();
        let sid = self.session.lock().await.clone();
        match post(&url, sid.as_deref(), body.clone(), CALL_TIMEOUT).await {
            Ok((messages, _)) => Ok(messages),
            Err(first) => {
                tracing::warn!(
                    "the shared reflow2 server stopped answering ({first:#}); starting a \
                     replacement and retrying once"
                );
                let fresh = crate::shared::ensure_server_async(&self.graph_path, None)
                    .await
                    .context("could not restart a shared reflow2 server for this design")?;
                *self.url.lock().await = fresh.clone();
                // A new server has never seen the old session id, so re-do the
                // handshake before replaying the request. Skipping this would
                // send a stale id and get a fresh, unrelated seat back.
                *self.session.lock().await = None;
                if let Some(hello) = self.hello.lock().await.clone()
                    && let Ok((_, sid)) = post(&fresh, None, hello, PROBE_TIMEOUT).await
                {
                    *self.session.lock().await = sid;
                }
                let sid = self.session.lock().await.clone();
                post(&fresh, sid.as_deref(), body, CALL_TIMEOUT)
                    .await
                    .map(|(m, _)| m)
                    .context("the replacement shared reflow2 server did not answer either")
            }
        }
    }
}

/// A JSON-RPC error for a request that could not be forwarded.
///
/// The alternative is to log and say nothing, which leaves the client waiting on
/// an id that will never be answered — a hung tool call, the least diagnosable
/// outcome there is, and precisely the silent failure this project refuses. The
/// agent gets a real error it can read and act on.
fn forwarding_error(request: &str, why: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(request).ok()?;
    let id = v.get("id")?.clone();
    Some(
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": format!(
                    "reflow2 could not reach the shared design server: {why}. The design is not \
                     lost — this session cannot currently talk to it. Check the server log beside \
                     the graph directory."
                )
            }
        })
        .to_string(),
    )
}

/// Forward this session's stdio JSON-RPC to the shared server until stdin ends.
///
/// Requests are forwarded concurrently — one task each — because a session that
/// serialised them would make a slow tool call block every other, which is
/// precisely the queueing that sharing a graph is supposed to remove. Responses
/// are correlated by JSON-RPC `id`, so returning them out of order is correct
/// rather than merely tolerated. stdout is behind a mutex: interleaving two
/// replies mid-line would corrupt the channel.
pub async fn run(url: &str, graph_path: &str) -> anyhow::Result<()> {
    let up = Arc::new(Upstream {
        graph_path: graph_path.to_string(),
        url: Mutex::new(url.to_string()),
        session: Mutex::new(None),
        hello: Mutex::new(None),
    });
    let out = Arc::new(Mutex::new(tokio::io::stdout()));
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut tasks = tokio::task::JoinSet::new();

    while let Some(line) = lines.next_line().await.context("reading stdin failed")? {
        if line.trim().is_empty() {
            continue;
        }

        // The handshake is handled INLINE, before any concurrency starts. Its
        // reply carries the session id every later request must quote, so
        // letting a second request race ahead of it would have the server open a
        // NEW session — silently giving this seat two identities, and
        // `claim_report` two owners for one agent.
        if line.contains("\"initialize\"") {
            *up.hello.lock().await = Some(line.clone());
            let url = up.url.lock().await.clone();
            // **The handshake gets the same safety net as every other request,
            // and it needs it MOST.** This was a bare `post(...).await?` whose
            // `?` propagated out of `run` and ended the process: if the shared
            // server died in the window between the election returning and the
            // client's first message, the session got a dead stdio server and no
            // explanation — the exact outcome `req:never-silently-absent` exists
            // to prevent, on the one message where the client has nothing yet.
            // Found by review (`w-74c2989e`, 2026-07-27): the first request was
            // the only one with no retry and no readable error.
            let (messages, sid) = match post(&url, None, line.clone(), PROBE_TIMEOUT).await {
                Ok(ok) => ok,
                Err(first) => {
                    tracing::warn!("the shared server did not answer the handshake ({first:#})");
                    match crate::shared::ensure_server_async(&up.graph_path, None).await {
                        Ok(fresh) => {
                            *up.url.lock().await = fresh.clone();
                            post(&fresh, None, line.clone(), PROBE_TIMEOUT)
                                .await
                                .unwrap_or_else(|_| (Vec::new(), None))
                        }
                        Err(e) => {
                            // Answer the handshake with a readable error rather
                            // than dying: a client holding an unanswered
                            // `initialize` cannot tell a broken server from an
                            // unconfigured one.
                            let mut w = out.lock().await;
                            if let Some(m) = forwarding_error(&line, &format!("{e:#}")) {
                                let _ = w.write_all(format!("{m}\n").as_bytes()).await;
                            }
                            let _ = w.flush().await;
                            continue;
                        }
                    }
                }
            };
            *up.session.lock().await = sid;
            let mut w = out.lock().await;
            for m in messages {
                w.write_all(format!("{m}\n").as_bytes()).await?;
            }
            w.flush().await?;
            continue;
        }

        let up = Arc::clone(&up);
        let out = Arc::clone(&out);
        tasks.spawn(async move {
            let replies = match up.send(line.clone()).await {
                Ok(messages) => messages,
                Err(e) => forwarding_error(&line, &format!("{e:#}"))
                    .map(|m| vec![m])
                    // A notification (no id) has nobody waiting on a reply, so
                    // there is nothing to answer; the log is the right place.
                    .unwrap_or_else(|| {
                        tracing::error!("forwarding to the shared server failed: {e:#}");
                        Vec::new()
                    }),
            };
            let mut w = out.lock().await;
            for m in replies {
                let _ = w.write_all(format!("{m}\n").as_bytes()).await;
            }
            let _ = w.flush().await;
        });
    }
    // stdin closed: the client is gone. Let in-flight work finish rather than
    // dropping replies on the floor.
    while tasks.join_next().await.is_some() {}
    Ok(())
}

/// Call one tool on a shared server and return its text content.
///
/// This is what lets the plain CLI reads keep working while a server holds the
/// store. `--export` used to fail outright against a held graph, which is how
/// `--export-snapshot` — a *copy* of the store, explicitly not crash-consistent
/// — came to exist. Going through the running server instead returns the real
/// design, live, with no copy and no caveat.
pub async fn call_tool(url: &str, name: &str, args: serde_json::Value) -> anyhow::Result<String> {
    let hello = serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "reflow2-cli", "version": env!("CARGO_PKG_VERSION")}
        }
    })
    .to_string();
    let (_, sid) = post(url, None, hello, PROBE_TIMEOUT).await?;
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": name, "arguments": args}
    })
    .to_string();
    let (messages, _) = post(url, sid.as_deref(), call, CALL_TIMEOUT).await?;
    for m in messages {
        let v: serde_json::Value = match serde_json::from_str(&m) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(err) = v.get("error") {
            bail!("the shared reflow2 server refused `{name}`: {err}");
        }
        if let Some(content) = v.get("result").and_then(|r| r.get("content")) {
            let text: String = content
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            return Ok(text);
        }
    }
    bail!("the shared reflow2 server returned no result for `{name}`")
}

#[cfg(test)]
mod tests {
    use super::{extract_messages, parse_endpoint};

    #[test]
    fn parses_a_loopback_url() {
        let e = parse_endpoint("http://127.0.0.1:41653/").unwrap();
        assert_eq!(e.authority, "127.0.0.1:41653");
        assert_eq!(e.path, "/");
    }

    #[test]
    fn a_url_without_a_path_still_targets_root() {
        let e = parse_endpoint("http://127.0.0.1:41653").unwrap();
        assert_eq!(e.path, "/");
    }

    #[test]
    fn plain_json_body_is_one_message() {
        let got = extract_messages(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
        assert_eq!(got.len(), 1);
        assert!(got[0].contains("\"id\":1"));
    }

    #[test]
    fn sse_frames_yield_their_data_payloads() {
        // The real shape the transport answered with when this was measured:
        // an opening keep-alive with an EMPTY data line, then the message.
        let body = "data: \nid: 0\nretry: 3000\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n";
        let got = extract_messages(body);
        assert_eq!(
            got.len(),
            1,
            "the empty keep-alive frame must not be forwarded as a message — a blank line in the \
             client's JSON-RPC channel is corruption, not a no-op"
        );
        assert!(got[0].contains("\"ok\":true"));
    }

    #[test]
    fn several_sse_frames_all_come_through() {
        let body = "data: {\"a\":1}\n\ndata: {\"b\":2}\n\n";
        assert_eq!(extract_messages(body), vec!["{\"a\":1}", "{\"b\":2}"]);
    }

    #[test]
    fn an_empty_body_is_no_messages_not_an_empty_line() {
        assert!(extract_messages("").is_empty());
        assert!(extract_messages("   \n").is_empty());
    }
}
