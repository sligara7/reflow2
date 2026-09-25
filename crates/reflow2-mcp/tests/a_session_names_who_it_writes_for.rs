//! A session names, once, the person it writes for — and every write it makes
//! is credited to them, on trust, for attribution only.
//!
//! `req:a-session-names-the-person-it-writes-for-and-the-server-remembers-it`
//! (accepted, Anthony 2026-09-23). Driven through the same two steps
//! `call_tool` takes around every handler — resolve who the call is for
//! (`effective_writes_for`), refuse a write for somebody the design does not
//! hold (`writes_for_precheck`), then run the handler writing for them
//! (`serving_for`) — so what is pinned here is the path a real request takes.
//!
//! What these hold:
//!   · a declared session's writes are credited to its contributor as author;
//!   · a request's own `_meta` name wins over the session's, for that request;
//!   · nothing declared means nothing credited — today's behaviour exactly;
//!   · the declaration never signs an approval;
//!   · a name the design does not hold is refused, and nothing is written;
//!   · a declaration is per SESSION: a second client starts with nobody;
//!   · the sessionless transport refuses a declaration that could not persist.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

fn req<T: serde::de::DeserializeOwned>(v: Value) -> Parameters<T> {
    Parameters(serde_json::from_value(v).expect("request shape"))
}

async fn session() -> ReflowService {
    let s = ReflowService::in_memory().expect("service");
    for (id, name) in [("who:sister", "Sister"), ("who:uncle", "Uncle")] {
        s.add_contributor(req(json!({"id": id, "name": name, "kind": "person"})))
            .await
            .expect("contributor");
    }
    s
}

/// A requirement written the way `call_tool` would write it for a request
/// whose `_meta` named `from_request` (or nobody).
async fn write_requirement(s: &ReflowService, id: &str, from_request: Option<&str>) {
    let who = s.effective_writes_for(from_request);
    assert!(
        s.writes_for_precheck("add_requirement", who.as_deref())
            .await
            .is_none()
    );
    s.serving_for(
        who,
        s.add_requirement(req(
            json!({"id": id, "name": id, "statement": "Anything about health can be said"}),
        )),
    )
    .await
    .expect("add_requirement");
}

/// (contributor, roles) for every AUTHORED_BY edge out of `id`, read back from
/// the exported design.
async fn authors_of(s: &ReflowService, id: &str) -> Vec<(String, Vec<String>)> {
    let doc = s
        .export_graph(req(json!({})))
        .await
        .expect("export")
        .structured_content
        .expect("structured");
    let edges = doc["edges"].as_array().cloned().unwrap_or_default();
    edges
        .into_iter()
        .filter(|e| e["edge_type"] == "AUTHORED_BY" && e["from_id"] == id)
        .map(|e| {
            let roles = e["properties"]["roles"]
                .as_array()
                .map(|r| {
                    r.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            (e["to_id"].as_str().unwrap_or_default().to_string(), roles)
        })
        .collect()
}

#[tokio::test]
async fn a_declared_sessions_writes_are_credited_to_its_contributor() {
    let s = session().await;
    s.writes_for_inner(
        serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
        false,
    )
    .await
    .expect("declare");
    write_requirement(&s, "req:talk-freely", None).await;
    assert_eq!(
        authors_of(&s, "req:talk-freely").await,
        vec![("who:sister".to_string(), vec!["author".to_string()])]
    );
}

#[tokio::test]
async fn a_requests_own_name_wins_for_that_request_only() {
    let s = session().await;
    s.writes_for_inner(
        serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
        false,
    )
    .await
    .expect("declare");
    write_requirement(&s, "req:uncles-input", Some("who:uncle")).await;
    write_requirement(&s, "req:sisters-input", None).await;
    assert_eq!(authors_of(&s, "req:uncles-input").await[0].0, "who:uncle");
    assert_eq!(authors_of(&s, "req:sisters-input").await[0].0, "who:sister");
}

#[tokio::test]
async fn nothing_declared_means_nothing_credited() {
    let s = session().await;
    write_requirement(&s, "req:as-today", None).await;
    assert!(authors_of(&s, "req:as-today").await.is_empty());
}

#[tokio::test]
async fn the_declaration_never_signs_an_approval() {
    let s = session().await;
    s.writes_for_inner(
        serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
        false,
    )
    .await
    .expect("declare");
    write_requirement(&s, "req:x", None).await;
    let who = s.effective_writes_for(None);
    let _ = s
        .serving_for(
            who,
            s.set_requirement_status(req(
                json!({"requirement_id": "req:x", "status": "accepted"}),
            )),
        )
        .await;
    for (_, roles) in authors_of(&s, "req:x").await {
        assert!(!roles.contains(&"approver".to_string()), "{roles:?}");
    }
}

#[tokio::test]
async fn a_name_the_design_does_not_hold_is_refused_before_anything_is_written() {
    let s = session().await;
    let refusal = s
        .writes_for_precheck("add_requirement", Some("who:nobody"))
        .await
        .expect("a write for an unknown contributor must be refused");
    assert!(refusal.contains("add_contributor"), "{refusal}");
    assert!(
        s.writes_for_precheck("search_design", Some("who:nobody"))
            .await
            .is_none(),
        "a READ writes nothing, so there is nothing to credit and nothing to refuse"
    );
    let err = s
        .writes_for_inner(
            serde_json::from_value(json!({"contributor_id": "who:nobody"})).unwrap(),
            false,
        )
        .await
        .expect_err("declaring an unknown contributor");
    assert!(err.message.contains("add_contributor"), "{}", err.message);
    assert_eq!(
        s.effective_writes_for(None),
        None,
        "a refused declaration must not stick"
    );
}

#[tokio::test]
async fn a_declaration_belongs_to_one_session() {
    let s = session().await;
    s.writes_for_inner(
        serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
        false,
    )
    .await
    .expect("declare");
    let other = s.share();
    assert_eq!(
        other.effective_writes_for(None),
        None,
        "a new client must start with nobody"
    );
    assert_eq!(s.effective_writes_for(None).as_deref(), Some("who:sister"));
}

#[tokio::test]
async fn declaring_nobody_stops_the_crediting() {
    let s = session().await;
    s.writes_for_inner(
        serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
        false,
    )
    .await
    .expect("declare");
    s.writes_for_inner(serde_json::from_value(json!({})).unwrap(), false)
        .await
        .expect("clear");
    write_requirement(&s, "req:after-clearing", None).await;
    assert!(authors_of(&s, "req:after-clearing").await.is_empty());
}

#[tokio::test]
async fn the_sessionless_transport_refuses_a_declaration_that_could_not_persist() {
    let s = session().await;
    let err = s
        .writes_for_inner(
            serde_json::from_value(json!({"contributor_id": "who:sister"})).unwrap(),
            true,
        )
        .await
        .expect_err("stateless");
    assert!(
        err.message.contains("reflow2/writes_for"),
        "{}",
        err.message
    );
    assert_eq!(s.effective_writes_for(None), None);
}
