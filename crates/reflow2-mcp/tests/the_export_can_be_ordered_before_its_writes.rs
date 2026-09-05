//! `export_graph` CAN be ordered before writes issued in the same parallel
//! batch — measured, not reasoned — and the downstream net that would catch it
//! is blind to the shape that actually gets reported.
//!
//! # Why this exists
//!
//! `fact:the-decision-race-is-caller-ordering-not-read-after-write` (2026-08-09)
//! and `fact:the-parallel-batch-class-recurred-because-only-its-instance-was-fixed`
//! (2026-08-30) both close with the same admission: *"whether `export_graph` can
//! in fact be ordered before a write in the same batch is reasoned from the
//! shared lock, not measured."* `export_graph`'s tool description warns about it
//! anyway. This is the measurement that was owed, so the warning stands on an
//! observation rather than on an argument.
//!
//! # What it found, 2026-08-31, in-memory backend, 4 worker threads
//!
//! ```text
//!   ARM A  node-creating write   91/200 early (45%)   caught downstream
//!   ARM B  property-only write   10/200 early ( 5%)   caught by NOTHING (0 of 10)
//! ```
//!
//! ⭐ THE METHOD CHANGES THE ANSWER BY 40x, so anyone re-running this must keep
//! the shape. Driving the pair with `tokio::join!` inside ONE task measures 1%,
//! because `join!` polls in order and the write almost always wins the lock.
//! A real MCP server handles each request on ITS OWN TASK, which is what these
//! arms do with `tokio::spawn` — and that is 45%. Measuring it the obvious way
//! would have concluded the hazard was too rare to mention.
//!
//! 🛑 ARM B IS THE ONE THAT MATTERS, and it is the shape the field report
//! actually carried. `sync_debt` nudges about unexported work on
//! `live_nodes > export_nodes` — a NODE COUNT. A write that only changes a
//! PROPERTY (Alex's `set_artifact_checksum`, 2026-08-30) moves no count, so an
//! early export holding the OLD value is indistinguishable from a complete one.
//! Measured: 0 of 10 early exports would have been caught.
//!
//! # Why these are `#[ignore]`d
//!
//! They are MEASUREMENTS, not gates. Asserting a scheduler outcome is flaky by
//! construction and would eventually be deleted for flapping — the same reason
//! `a_dependent_write_in_a_parallel_batch.rs` deliberately pins the message
//! rather than the race. Re-run them deliberately:
//!
//! ```text
//! cargo test -p reflow2-mcp --test probe_export_ordering -- --ignored --nocapture
//! ```

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

fn event(id: &str) -> AddChangeEventReq {
    AddChangeEventReq {
        description: None,
        id: id.into(),
        name: Some("A change".into()),
        change_type: Some("defect_fix".into()),
        subject: Some("system".into()),
        summary: Some("Something moved.".into()),
        rationale: None,
        affected: None,
        detected_at: None,
    }
}

fn requirement(id: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some("A need".into()),
        statement: Some("The system shall do the thing.".into()),
        distinct_from: None,
    }
}

async fn export_text(s: &ReflowService) -> String {
    let doc = s
        .export_graph(Parameters(ExportGraphToReq {
            path: None,
            overwrite: None,
            accept_divergence: None,
        }))
        .await
        .expect("export ok")
        .structured_content
        .expect("structured content");
    serde_json::to_string(&doc).expect("serialize")
}

/// ARM A — the write CREATES A NODE. An early export is missing a node, so the
/// node-count check in `sync_debt` has something to see.
#[ignore = "measurement, not a gate — see the module header"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn probe_a_node_creating_write() {
    const TRIALS: usize = 200;
    let (mut early, mut late) = (0usize, 0usize);

    for i in 0..TRIALS {
        let s = ReflowService::in_memory().expect("service");
        let id = format!("chg:probe-{i}");
        let (sw, se) = (s.clone(), s.clone());
        let idw = id.clone();
        let hw = tokio::spawn(async move { sw.add_change_event(Parameters(event(&idw))).await });
        let he = tokio::spawn(async move { export_text(&se).await });
        let (wrote, text) = (hw.await.expect("join"), he.await.expect("join"));
        wrote.expect("the write must always land");
        if text.contains(&id) {
            late += 1
        } else {
            early += 1
        }
    }
    println!("ARM A (node-creating write), {TRIALS} trials: export EARLY {early}, after {late}.");
}

/// ARM B — the write changes only a PROPERTY of a node that already exists.
/// This is Alex's actual shape (`set_artifact_checksum`). An early export holds
/// the SAME NODE COUNT, so `live_nodes > export_nodes` cannot fire.
#[ignore = "measurement, not a gate — see the module header"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn probe_b_property_only_write() {
    const TRIALS: usize = 200;
    let (mut early, mut late) = (0usize, 0usize);
    let mut count_delta_visible = 0usize;

    for i in 0..TRIALS {
        let s = ReflowService::in_memory().expect("service");
        let id = format!("req:probe-{i}");
        s.add_requirement(Parameters(requirement(&id)))
            .await
            .expect("seed the node first, sequenced");

        let (sw, se) = (s.clone(), s.clone());
        let idw = id.clone();
        let hw = tokio::spawn(async move {
            sw.set_requirement_status(Parameters(RequirementStatusReq {
                requirement_id: idw,
                status: "accepted".into(),
            }))
            .await
        });
        let he = tokio::spawn(async move { export_text(&se).await });
        let (wrote, text) = (hw.await.expect("join"), he.await.expect("join"));
        wrote.expect("the write must always land");

        // The node is present either way; only its status differs.
        let got_status = text.contains("\"accepted\"");
        if got_status {
            late += 1
        } else {
            early += 1
        }

        // Would a node-count comparison have noticed this early export?
        let live = export_text(&s).await;
        let n = |t: &str| t.matches("\"node_id\"").count();
        if !got_status && n(&live) != n(&text) {
            count_delta_visible += 1;
        }
    }
    println!(
        "ARM B (property-only write), {TRIALS} trials: export EARLY {early}, after {late}. \
         Of the {early} early exports, {count_delta_visible} would have been caught by a \
         node-count comparison."
    );
}
