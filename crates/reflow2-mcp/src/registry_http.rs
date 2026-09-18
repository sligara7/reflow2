//! `/g/<graph_id>/` — the transport half of selecting a design per session.
//!
//! ⭐ WHAT THIS FINISHES. `registry.rs` has been the RESOLUTION half since
//! 2026-08-12: it maps a `graph_id` to a path under one root, refuses an unknown
//! id by name, and refuses a PATH as an unknown id so a filesystem route is not
//! a second way in. Nothing called it outside its own tests, so
//! `cap:select-graph-by-id` sat `in_progress` and two flo2 items were blocked on
//! it — measuring per-store cost needs one process holding two designs, and
//! proving isolation against the published image needs two designs in it.
//!
//! ⭐ THE CARRIER WAS ALREADY CHOSEN, and not by this module.
//! `cap:select-graph-by-id` names an HTTP path prefix as preferred "because the
//! selection is then visible in logs and routable by ordinary proxies", with an
//! MCP initialize parameter second and an attaching tool call worst — that last
//! "makes every session stateful in a way a reconnect silently loses, and it
//! leaves a window in which a session is connected to nothing". This implements
//! the first.
//!
//! ⭐ AND THE SECURITY PROPERTY WAS DECIDED BEFORE THE SURFACE EXISTED, which is
//! what `dec:the-registry-root-is-the-tenant-boundary` is for: **the registry
//! routes WITHIN a root and never across one.** There is no cross-root operation
//! to design, no filtered listing (that would need an identity system reflow2 has
//! twice refused in writing), and an operator serving several tenants gives each
//! its own root. Isolation is a property of WHAT IS THERE, not of a check on who
//! is asking.
//!
//! 🛑 ISOLATION HOLDS BY CONSTRUCTION HERE, NOT BY A CHECK, and that is
//! deliberate — flo2's own tracker warns that "a handler that reaches the wrong
//! graph corrupts a design rather than erroring, so this wants property tests,
//! not examples":
//!
//! - The only way to name a design is the `graph_id` segment, and it is handed
//!   straight to `Registry::attach`, which resolves it against the root. A path
//!   is refused as an unknown id, because `attach` treats its argument as an id
//!   and nothing else.
//! - A segment containing `/`, `\`, or `..` never reaches `attach` at all: it
//!   cannot survive being one path segment, and the traversal check below is
//!   belt to that brace.
//! - Each design gets its OWN `StreamableHttpService` with its OWN session
//!   manager, so a session id minted under `/g/A/` is unknown under `/g/B/`.
//!   Sessions cannot be carried across designs because they are not in the same
//!   table.
//!
//! ## What this increment does NOT do, named rather than left to be discovered
//!
//! `dec:one-process-many-stores` lists five things that must change for hosted
//! multi-graph. This does two of them — the registry lifecycle and the selection
//! carrier — and deliberately not the rest:
//!
//! - **No idle eviction.** An open design stays open until the process ends.
//!   `--registry-max-open` caps how many may be open at once and REFUSES past it
//!   with a clear error, which is the half that keeps a busy server from
//!   thrashing; reclaiming idle ones is a separate change with its own policy.
//! - **No per-graph rendezvous.** A multi-graph server does not publish itself
//!   into each held graph's sidecar, so a local `--shared` session opening one of
//!   those directories will not find it and will start its own daemon. That is
//!   the single-graph path working as it always has, not a regression, but it
//!   means the two modes do not yet cooperate.
//! - **`--import` / `--export` still open the store directly** and so still
//!   require the server to release it.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::sync::{Mutex, OnceCell};

use crate::registry::Registry;
use crate::service::ReflowService;

/// What `StreamableHttpService` answers with. Spelled out because rmcp keeps its
/// own `BoxResponse` alias `pub(crate)`; the shape is part of its public
/// `tower::Service` impl either way.
type BoxResponse = Response<BoxBody<Bytes, std::convert::Infallible>>;

type GraphService = StreamableHttpService<ReflowService, LocalSessionManager>;

/// One design's service, or the reason it could not be opened.
///
/// A FAILED OPEN IS CACHED AS THE FAILURE, not retried on every request: a
/// design whose store is held by another process would otherwise pay the full
/// open timeout on every call, and the caller gets the same answer either way.
type Opened = Result<Arc<GraphService>, String>;

/// Serves many designs under one root, selected by `/g/<graph_id>/`.
#[derive(Clone)]
pub struct GraphRouter {
    root: String,
    read_only: bool,
    max_open: usize,
    config: StreamableHttpServerConfig,
    /// `graph_id` -> a cell that resolves once.
    ///
    /// ⭐ A CELL PER ID RATHER THAN A LOCK ACROSS THE MAP. A cold open builds
    /// the full-text index and takes SECONDS on a large design, so holding one
    /// lock across it would stall every OTHER design's requests behind an
    /// unrelated open. `OnceCell` also collapses concurrent first requests for
    /// the SAME id into one open, which `dec:one-process-many-stores` asks for
    /// by name: "concurrent first requests for the same graph must collapse into
    /// one open rather than racing".
    open: Arc<Mutex<HashMap<String, Arc<OnceCell<Opened>>>>>,
}

impl GraphRouter {
    pub fn new(
        root: String,
        read_only: bool,
        max_open: usize,
        config: StreamableHttpServerConfig,
    ) -> Self {
        Self {
            root,
            read_only,
            max_open,
            config,
            open: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The designs under this root, rediscovered on each call.
    ///
    /// NOT CACHED, on purpose: a design created after the server started should
    /// be reachable without a restart, and `discover` is a directory read.
    pub fn graph_ids(&self) -> Vec<String> {
        Registry::discover(&self.root).graph_ids()
    }

    /// Split `/g/<id>/rest` into the id and what the inner service should see.
    ///
    /// Returns `None` for any path that is not under `/g/`, so the router can
    /// say what would have worked instead of serving a design nobody named.
    fn split(path: &str) -> Option<(String, String)> {
        let rest = path.strip_prefix("/g/")?;
        let (id, tail) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if id.is_empty() {
            return None;
        }
        Some((
            id.to_string(),
            if tail.is_empty() {
                "/".to_string()
            } else {
                tail.to_string()
            },
        ))
    }

    async fn service_for(&self, graph_id: &str) -> Opened {
        // ⚠️ BELT TO THE BRACE. A `graph_id` arrives as ONE path segment, so it
        // cannot contain `/` and `..` alone names no design under the root. This
        // refuses the shapes anyway rather than relying on that argument holding
        // forever — the cost of being wrong here is a handler reaching another
        // design, which corrupts rather than errors.
        if graph_id.contains('/') || graph_id.contains('\\') || graph_id == ".." || graph_id == "."
        {
            return Err(format!(
                "'{graph_id}' is not a design id. Designs are named by graph_id, never by path — \
                 the registry maps the id to a path under its root, and a path is refused exactly \
                 as an unknown id is."
            ));
        }

        let cell = {
            let mut map = self.open.lock().await;
            if !map.contains_key(graph_id) && map.len() >= self.max_open {
                // A CLEAR ERROR AT THE CAP, never silent thrashing
                // (dec:one-process-many-stores). Naming the cap and the flag is
                // the difference between "reflow2 is broken" and "raise this".
                return Err(format!(
                    "this server already holds {} design(s), which is its limit — '{graph_id}' \
                     was not opened. Each open design costs a store, its file handles and its \
                     full-text index, so the cap is a real resource bound rather than a policy. \
                     Raise it with --registry-max-open, or run a second server.",
                    self.max_open
                ));
            }
            Arc::clone(map.entry(graph_id.to_string()).or_default())
        };

        let root = self.root.clone();
        let read_only = self.read_only;
        let config = self.config.clone();
        let id = graph_id.to_string();
        cell.get_or_init(|| async move {
            // Resolve id -> path against the root. `attach` is the ONLY way in.
            let binding = Registry::discover(&root)
                .attach(&id)
                .map_err(|e| e.to_string())?;
            let path = binding.graph_path().to_string();

            // ⚠️ OPENING IS BLOCKING AND SLOW — RocksDB plus a full-text index
            // rebuild, seconds on a large design. On the async runtime that
            // would stall every other connection this process is serving, which
            // is the whole point of holding many designs in one process.
            let opened = tokio::task::spawn_blocking(move || {
                ReflowService::new_reporting(&path).map(|(svc, _prov)| svc)
            })
            .await
            .map_err(|e| format!("the open task failed: {e}"))?
            .map_err(|e| format!("{e}"))?;

            // A registry holds DESIGNS, not checkouts: the directory a store
            // sits under is not the tree its artifacts describe, so a
            // measurement here would find every file absent and call it
            // missing. Say "not on this machine" instead (`crate::measure`).
            let opened = opened.without_tree();
            let svc = if read_only {
                opened.into_read_only()
            } else {
                opened
            };
            Ok(Arc::new(StreamableHttpService::new(
                move || Ok(svc.share()),
                LocalSessionManager::default().into(),
                config,
            )))
        })
        .await
        .clone()
    }
}

fn text(status: StatusCode, body: String) -> BoxResponse {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from(body)).boxed())
        .expect("a static text response is always well-formed")
}

impl<B> tower_service::Service<Request<B>> for GraphRouter
where
    B: http_body::Body + Send + 'static,
    B::Error: std::fmt::Display,
    B::Data: Send + 'static,
{
    type Response = BoxResponse;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();

            let Some((graph_id, tail)) = GraphRouter::split(&path) else {
                // RULE 4 — SAY WHAT WOULD HAVE WORKED. A bare 404 here is the
                // failure this repo keeps meeting: a session that cannot tell
                // "reflow2 is not here" from "you addressed it wrongly".
                let ids = this.graph_ids();
                let known = if ids.is_empty() {
                    "This server's registry root holds no designs.".to_string()
                } else {
                    format!("Designs under this root: {}.", ids.join(", "))
                };
                return Ok(text(
                    StatusCode::NOT_FOUND,
                    format!(
                        "reflow2 is serving SEVERAL designs here, so a request must name which \
                         one: POST to /g/<graph_id>/ rather than to {path}.\n\n{known}\n\nA design \
                         is named by its graph_id, never by a filesystem path.\n"
                    ),
                ));
            };

            match this.service_for(&graph_id).await {
                Ok(svc) => {
                    // Strip the prefix so the inner service sees the path it
                    // would have seen as a single-graph server. Everything else
                    // about the request — method, headers, body, the session id
                    // — passes through untouched.
                    let (mut parts, body) = req.into_parts();
                    let query = parts
                        .uri
                        .query()
                        .map(|q| format!("?{q}"))
                        .unwrap_or_default();
                    parts.uri = format!("{tail}{query}")
                        .parse()
                        .unwrap_or_else(|_| "/".parse().expect("'/' is a valid URI"));
                    let inner = Request::from_parts(parts, body);
                    let mut svc = (*svc).clone();
                    tower_service::Service::call(&mut svc, inner).await
                }
                Err(why) => Ok(text(
                    StatusCode::NOT_FOUND,
                    format!("reflow2 could not attach to design '{graph_id}': {why}\n"),
                )),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::GraphRouter;

    #[test]
    fn a_design_is_named_by_one_segment_and_the_rest_is_passed_through() {
        assert_eq!(
            GraphRouter::split("/g/reflow2/"),
            Some(("reflow2".into(), "/".into()))
        );
        assert_eq!(
            GraphRouter::split("/g/reflow2"),
            Some(("reflow2".into(), "/".into())),
            "a bare id with no trailing slash still names the design"
        );
        assert_eq!(
            GraphRouter::split("/g/flo2/message"),
            Some(("flo2".into(), "/message".into())),
            "and the inner service sees the path it would have seen alone"
        );
    }

    #[test]
    fn a_path_that_names_no_design_is_not_routed_to_one() {
        assert_eq!(GraphRouter::split("/"), None);
        assert_eq!(GraphRouter::split("/message"), None);
        assert_eq!(GraphRouter::split("/g/"), None, "an empty id names nothing");
        assert_eq!(
            GraphRouter::split("/gg/reflow2/"),
            None,
            "the prefix is exact — a near miss must not resolve"
        );
    }

    #[test]
    fn traversal_cannot_be_smuggled_through_the_id_segment() {
        // `..` as a whole segment is the only traversal that survives being one
        // segment at all, and it names no design under the root. The service
        // refuses it before `attach` ever sees it; this pins the parse half.
        assert_eq!(
            GraphRouter::split("/g/../etc/passwd"),
            Some(("..".into(), "/etc/passwd".into())),
            "it parses as an id of '..' — which service_for refuses by name, \
             rather than being silently resolved as a path"
        );
    }
}
