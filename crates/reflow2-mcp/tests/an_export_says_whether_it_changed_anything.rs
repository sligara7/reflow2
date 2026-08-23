//! `export_graph` says whether it actually wrote anything.
//!
//! # Why this exists
//!
//! The receipt could not answer it. Measured on 0.31.0 across a five-export
//! chain, an export that CHANGED the file and one that changed NOTHING returned
//! byte-identical receipts:
//!
//! ```text
//!   2. changed     content=f617f1ee  prev=9552429a
//!   3. UNCHANGED   content=f617f1ee  prev=9552429a
//! ```
//!
//! because `chain_after` gives an unchanged export the predecessor's own `prev`.
//! In both cases `content_hash != prev_content_hash`, so that difference — the
//! one a caller would reach for — discriminates nothing.
//!
//! It matters because of who meets it. On a `--shared` server a peer's export
//! publishes your in-flight work (28 nodes once, 17 the next), so your own
//! export afterwards is a no-op, and the seat that hit it read that as a **failed
//! save**. Reported five times by three seats.
//!
//! ⚠️ A SHORT PROBE HIDES THIS. A two-export test shows `prev_content_hash: null`
//! on the no-op and looks like a discriminator — it is not, it is the first
//! export's null being inherited once. `a_no_op_on_a_young_chain_still_says_unchanged`
//! is that case, kept so the illusion cannot come back.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

/// House pattern (design_identity.rs, latent_mode.rs): a pid-scoped directory
/// under the system temp dir, so tests do not collide and no dev-dependency is
/// added for three paths.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-export-wrote-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

async fn svc() -> ReflowService {
    ReflowService::in_memory().expect("in-memory service")
}

fn req(id: &str, name: &str, statement: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(name.into()),
        statement: Some(statement.into()),
        distinct_from: None,
    }
}

fn export(path: &std::path::Path) -> ExportGraphToReq {
    ExportGraphToReq {
        path: Some(path.display().to_string()),
        overwrite: Some(true),
        accept_divergence: None,
    }
}

/// A first export creates the file, and says so.
#[tokio::test]
async fn a_first_export_reports_created() {
    let f = scratch("created").join("design.json");
    let s = svc().await;
    let out = j!(s.export_graph(Parameters(export(&f))));
    assert_eq!(out["wrote"], "created", "{out:?}");
    assert!(out.get("wrote_note").is_none(), "no note on the clear case");
}

/// THE DEFECT CASE, on a chain long enough to be representative.
#[tokio::test]
async fn a_no_op_export_says_unchanged_where_the_hashes_cannot() {
    let f = scratch("noop").join("design.json");
    let s = svc().await;

    j!(s.export_graph(Parameters(export(&f))));
    j!(s.add_requirement(Parameters(req(
        "req:running-total",
        "A dropped packet costs nothing",
        "Every reading carries the running total, so a gap in the series closes without a retransmit."
    ))));
    let changed = j!(s.export_graph(Parameters(export(&f))));
    let unchanged = j!(s.export_graph(Parameters(export(&f))));

    assert_eq!(changed["wrote"], "changed", "{changed:?}");
    assert_eq!(unchanged["wrote"], "unchanged", "{unchanged:?}");

    // The point of the whole change: without `wrote`, these two are the same.
    assert_eq!(
        changed["content_hash"], unchanged["content_hash"],
        "the hashes are identical — which is exactly why they cannot discriminate"
    );
    assert_eq!(
        changed["prev_content_hash"], unchanged["prev_content_hash"],
        "so is the lineage link"
    );
    assert!(
        unchanged["wrote_note"]
            .as_str()
            .is_some_and(|n| n.contains("NOTHING WAS WRITTEN")),
        "the misleading case gets the note: {unchanged:?}"
    );
    assert!(
        changed.get("wrote_note").is_none(),
        "and a real write does not, or the note becomes noise"
    );
}

/// The trap that produced a wrong verification: on a TWO-export chain the no-op
/// inherits the first export's null `prev`, which looks like a signal. It is not
/// one, and `wrote` must be right here too.
#[tokio::test]
async fn a_no_op_on_a_young_chain_still_says_unchanged() {
    let f = scratch("young").join("design.json");
    let s = svc().await;

    j!(s.export_graph(Parameters(export(&f))));
    let second = j!(s.export_graph(Parameters(export(&f))));

    assert_eq!(second["wrote"], "unchanged", "{second:?}");
    assert!(
        second["prev_content_hash"].is_null(),
        "this null is the inherited one that misled a verification — kept as a fixture"
    );
}
