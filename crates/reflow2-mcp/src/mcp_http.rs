//! The MCP-over-HTTP client primitives: post a JSON-RPC line, read the reply,
//! and ask whether reflow2 is answering at a URL.
//!
//! ⭐ EXTRACTED 2026-08-20 TO BREAK A MODULE CYCLE. `proxy` and `shared` were
//! mutually dependent and had been since shared mode was built: the proxy
//! respawns a dead daemon through `shared::ensure_server_async`, and `shared`
//! checks a daemon is alive through what was then `proxy::probe_server_async`.
//! Neither direction was wrong — they are both real — but together they meant
//! neither module could be read, tested or moved without the other.
//!
//! The split is along the honest line: THIS module knows how to SPEAK to a
//! server, `shared` knows how to MANAGE one, and `proxy` knows how to FORWARD a
//! session to one. Both of the others depend on this and it depends on neither.
//!
//! FOUND BY RUNNING ADOPT OVER REFLOW2'S OWN SOURCE, which is the only reason
//! anyone looked: a module cycle inside one crate is legal Rust and compiles
//! silently forever, so nothing in the build was ever going to mention it.

use anyhow::{Context, bail};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use std::time::Duration;

/// A parsed `http://host:port/path` or `https://host[:port]/path` — enough of
/// a URL for loopback, and since 2026-09-26 for a remote reflow2 (`--remote`).
pub(crate) struct Endpoint {
    /// `host[:port]` exactly as written: what the `Host` header carries.
    pub(crate) authority: String,
    /// The host alone, for TLS server-name checking.
    pub(crate) host: String,
    /// Where to connect: the authority with the scheme's default port filled in.
    pub(crate) connect: String,
    pub(crate) path: String,
    pub(crate) tls: bool,
}

pub(crate) fn parse_endpoint(url: &str) -> anyhow::Result<Endpoint> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        bail!("a reflow2 server URL must start with http:// or https:// (got {url})");
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        bail!("the reflow2 server URL has no host");
    }
    // A bracketed IPv6 literal keeps its colons inside the brackets.
    let (host, has_port) = match authority.rfind(':') {
        Some(i) if !authority[i..].contains(']') => (&authority[..i], true),
        _ => (authority, false),
    };
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let connect = if has_port {
        authority.to_string()
    } else {
        format!("{authority}:{}", if tls { 443 } else { 80 })
    };
    Ok(Endpoint {
        authority: authority.to_string(),
        host,
        connect,
        path: path.to_string(),
        tls,
    })
}

/// Is this host this machine? A key is only ever sent over plain `http://` to
/// one of these; anywhere else it would cross a network in the clear.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|a| a.is_loopback())
}

/// The TLS client configuration, built once: the OPERATING SYSTEM's trust
/// store, so a company server with an internally issued certificate is trusted
/// exactly as the machine trusts it. Certificates that fail to parse are
/// skipped rather than fatal (a system store routinely holds a few), but an
/// EMPTY store is an error, because every connection would then fail for a
/// reason nobody could see.
fn tls_config() -> anyhow::Result<std::sync::Arc<rustls::ClientConfig>> {
    static CONFIG: std::sync::OnceLock<std::sync::Arc<rustls::ClientConfig>> =
        std::sync::OnceLock::new();
    if let Some(c) = CONFIG.get() {
        return Ok(c.clone());
    }
    let mut roots = rustls::RootCertStore::empty();
    let found = rustls_native_certs::load_native_certs();
    let (added, _ignored) = roots.add_parsable_certificates(found.certs);
    if added == 0 {
        bail!(
            "no trusted certificates were found on this machine, so no https:// server can be \
             checked. Install the system's CA certificates (ca-certificates on Linux)."
        );
    }
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("the TLS library refused its own default protocol versions")?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(CONFIG.get_or_init(|| std::sync::Arc::new(config)).clone())
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
pub(crate) async fn post(
    url: &str,
    session: Option<&str>,
    body: String,
    limit: Duration,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    tokio::time::timeout(limit, post_inner(url, session, body, None))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "the shared reflow2 server at {url} accepted the request but did not answer within \
                 {}s. It may be wedged; `reflow2-mcp --graph-path <graph> --stop-shared` clears it.",
                limit.as_secs()
            )
        })?
}

/// `post`, carrying a bearer credential — the remote client's door
/// (`--remote`). The credential goes in the `Authorization` header and nowhere
/// else: never into a log line, an error message or the reply.
pub(crate) async fn post_with(
    url: &str,
    session: Option<&str>,
    body: String,
    limit: Duration,
    bearer: Option<&str>,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    // Not the shared-mode hint: a remote server is someone else's to restart.
    tokio::time::timeout(limit, post_inner(url, session, body, bearer))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "the reflow2 server did not answer within {}s",
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
    bearer: Option<&str>,
) -> anyhow::Result<(Vec<String>, Option<String>)> {
    let ep = parse_endpoint(url)?;
    let stream = tokio::net::TcpStream::connect(&ep.connect)
        .await
        .with_context(|| format!("could not reach the reflow2 server at {}", ep.authority))?;
    if ep.tls {
        let name = rustls::pki_types::ServerName::try_from(ep.host.clone())
            .with_context(|| format!("{} is not a valid server name for TLS", ep.host))?;
        let tls = tokio_rustls::TlsConnector::from(tls_config()?)
            .connect(name, stream)
            .await
            .with_context(|| format!("the TLS handshake with {} failed", ep.authority))?;
        exchange(tls, &ep, session, body, bearer).await
    } else {
        exchange(stream, &ep, session, body, bearer).await
    }
}

/// The server no longer knows the session this request quoted: the MCP
/// transport's 404 for a session id it has terminated, after which the spec
/// has the client start a new session. Typed, so a caller re-joins on THIS and
/// nothing else — a timeout or a refusal is not a forgotten session, and
/// re-sending a change after a timeout can apply it twice.
#[derive(Debug)]
pub(crate) struct SessionGone(pub String);

impl std::fmt::Display for SessionGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the reflow2 server no longer knows this session (404): {}",
            self.0
        )
    }
}

impl std::error::Error for SessionGone {}

/// One HTTP/1.1 request over an already-connected stream, plain or TLS.
async fn exchange<S>(
    stream: S,
    ep: &Endpoint,
    session: Option<&str>,
    body: String,
    bearer: Option<&str>,
) -> anyhow::Result<(Vec<String>, Option<String>)>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let io = hyper_util::rt::TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .context("HTTP handshake with the reflow2 server failed")?;
    tokio::spawn(async move {
        // The connection task ends when the response is done; a debug line is
        // right here because a closed keep-alive is normal, not an incident.
        if let Err(e) = conn.await {
            tracing::debug!("connection to the reflow2 server ended: {e}");
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
    if let Some(key) = bearer {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    let req = req
        .body(Full::new(Bytes::from(body)))
        .context("could not build the request to the reflow2 server")?;

    let res = sender
        .send_request(req)
        .await
        .context("the reflow2 server did not answer")?;
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
        .context("could not read the reflow2 server's reply")?
        .to_bytes();
    let text = String::from_utf8_lossy(&collected).to_string();
    if status == hyper::StatusCode::UNAUTHORIZED {
        bail!(
            "the reflow2 server at {} refused the credential (401 Unauthorized): the key is \
             missing, wrong, expired or revoked. Check the key this client was given, or sign in again.",
            ep.authority
        );
    }
    if status == hyper::StatusCode::FORBIDDEN {
        bail!(
            "the reflow2 server at {} accepted who you are but not this request (403 Forbidden): \
             the credential lacks the permission it needs, typically to CHANGE a design. {text}",
            ep.authority
        );
    }
    if status == hyper::StatusCode::NOT_FOUND && session.is_some() {
        return Err(SessionGone(text).into());
    }
    if status == hyper::StatusCode::NOT_FOUND && bearer.is_some() {
        bail!(
            "the reflow2 server at {} has no such design for you (404): the id is wrong, or you \
             are not a member of it.",
            ep.authority
        );
    }
    if !status.is_success() {
        bail!("the reflow2 server answered {status}: {text}");
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
        if t.is_empty() {
            return Vec::new();
        }
        // A plain-JSON reply may be pretty-printed (flo2's gateway indents its
        // replies), and a newline inside it would split one message into
        // several broken lines on the client's stdio channel, which carries
        // exactly one message per line. Re-serialize compactly; a body that is
        // not JSON is passed through for the client to reject visibly.
        return vec![
            serde_json::from_str::<serde_json::Value>(t)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| t.to_string()),
        ];
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
    fn a_pretty_printed_body_becomes_one_line() {
        // flo2's gateway indents its replies; the stdio channel is one message per line.
        let got =
            extract_messages("{\n  \"jsonrpc\": \"2.0\",\n  \"id\": 1,\n  \"result\": {}\n}\n");
        assert_eq!(
            got,
            vec![r#"{"id":1,"jsonrpc":"2.0","result":{}}"#.to_string()]
        );
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
