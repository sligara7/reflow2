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

use crate::mcp_http::{CALL_TIMEOUT, PROBE_TIMEOUT, post};
use anyhow::{Context, bail};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

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
