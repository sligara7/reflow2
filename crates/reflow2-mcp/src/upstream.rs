//! Read the designs this one DEPENDS ON, without taking them in.
//!
//! # The finding this exists to fix
//!
//! `req:design-dependencies-declared` (ACCEPTED 2026-07-27) names two checks in
//! its own statement: *"declared-versus-mirrored answers am I actually composing
//! against the version I said, and DECLARED-VERSUS-UPSTREAM answers has what I
//! depend on moved since. Either question is unanswerable with only one of the
//! two halves."* Only the build-facing half shipped —
//! `reconcile_dependencies` compares a declaration against `Cargo.toml` and its
//! siblings. Nothing compared a declaration against the upstream DESIGN.
//!
//! # Why the obvious route was barred
//!
//! [`crate::sync_debt`] already answers "has that file moved", over
//! `provenance::last_synced`. But that record is written by exactly two paths,
//! `export_graph` and `import_graph` — so the only ways to get a path into it
//! are to export to it or import from it. And `import_graph` into a store that
//! already holds a design KEEPS THE HOST'S NAME and upserts the incoming nodes
//! into it (`adopt_on_import`: *"a store already holding a design keeps its own
//! name, because layering an export onto a live design is an upsert, not a
//! restore"*). **So watching a design by importing it absorbs that design** —
//! which is precisely what the hub case
//! (`dec:idea-a-hub-owns-designs-it-does-not-absorb-and-that-is-a-third-relation`)
//! says must not happen.
//!
//! The missing piece was therefore much smaller than a new relation: a way to
//! WATCH a path without importing it. The comparison, the reporting and the
//! child list all already existed.
//!
//! # Why the child list is the manifest
//!
//! Something has to say WHICH designs to watch, and the obvious worry was that
//! this would drag a hub relation in through the back door. It does not: the
//! dependency manifest already holds the list — checked in, version-pinned,
//! naming each design by `graph_id`, reviewable in a diff, and carrying the
//! direction a flat list of ids could not express.
//!
//! # Why the reading lives here and not in the core
//!
//! `reflow2-core` does no file I/O, deliberately and repeatedly — the same
//! reason [`crate::sync_debt`] states for itself. The core holds the
//! declarations and the COMPARISON; this module supplies what was found on
//! disk. That split is `reconcile_dependencies` and `reconcile_artifacts` all
//! over again, one boundary along.
//!
//! ⚠️ IT DOES NOT NAVIGATE. `describe_designs` makes the CALLER find candidate
//! paths because reflow2 does no file navigation, and that rule is intact here:
//! every path read is one this design pointed at ITSELF, in its own committed
//! manifest. Reading a file you were handed is a weaker claim than going
//! looking for one, and the difference is the whole reason this is allowed.

use reflow2_core::{GraphExport, ObservedUpstream, UpstreamTarget};

/// How many upstream exports one pass will actually open.
///
/// The same bound, for the same measured reason, as
/// [`crate::sync_debt::MAX_RECORDS_CHECKED`]: every target costs a full document
/// read and parse, and one seat had accumulated 16 sync targets totalling
/// 102 MB before anybody noticed. A declared-dependency list grows more slowly
/// than a sync-target list — nobody adds one by accident — so the bound is
/// higher, but it is not absent, and what it skips is NAMED rather than
/// dropped.
pub const MAX_UPSTREAMS_READ: usize = 16;

/// What a bounded pass left unopened.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpstreamNotRead {
    pub count: usize,
    pub dependencies: Vec<String>,
    pub note: String,
}

/// Read each declared upstream export and say what is there.
///
/// Returns one [`ObservedUpstream`] per target opened, plus whatever the bound
/// left out. Pass the result to `DesignGraph::reconcile_upstream`, which holds
/// the judgement.
///
/// 🛑 IT NEVER WRITES. Not the baseline hash, not a sync record, nothing. A
/// check that refreshed its own baseline on read would report `moved` exactly
/// once and then be permanently quiet, which is worse than no check at all.
/// Taking a new baseline is `declare_dependency`'s job, and it is a deliberate
/// act by the person who read what changed (`dec:ask-not-repair`).
pub fn observe_upstreams(
    targets: &[UpstreamTarget],
) -> (Vec<ObservedUpstream>, Option<UpstreamNotRead>) {
    let (read, skipped) = targets.split_at(targets.len().min(MAX_UPSTREAMS_READ));

    let observed = read.iter().map(observe_one).collect();

    let not_read = if skipped.is_empty() {
        None
    } else {
        Some(UpstreamNotRead {
            count: skipped.len(),
            dependencies: skipped.iter().map(|t| t.name.clone()).collect(),
            note: format!(
                "{} declared upstream design(s) were NOT opened by this pass, which reads at most \
                 {MAX_UPSTREAMS_READ} because every one costs a full document read. Named here \
                 rather than dropped: a pass that quietly checks fewer than it knows about is the \
                 silent truncation this project refuses. They come back as `not_observed`, never \
                 as agreement.",
                skipped.len()
            ),
        })
    };
    (observed, not_read)
}

fn observe_one(t: &UpstreamTarget) -> ObservedUpstream {
    let bare = |state: &str| ObservedUpstream {
        id: t.id.clone(),
        state: state.to_string(),
        content_hash: None,
        graph_id: None,
        nodes: None,
    };
    let path = std::path::Path::new(&t.design_export);
    if !path.exists() {
        return bare("missing");
    }
    let Some(doc) = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<GraphExport>(&raw).ok())
    else {
        return bare("unreadable");
    };
    // ⚠️ COMPUTED, NEVER the hash the file states about itself. `sync_debt`
    // learned this the hard way: `effective_content_hash` TRUSTS the embedded
    // stamp and computes only when it is absent, so a document edited by
    // anything other than `export_graph` — a merge, a hand-fix, another tool —
    // keeps its old stamp and reads as unmoved while its content has moved.
    // The document is already parsed, so computing costs nothing extra.
    ObservedUpstream {
        id: t.id.clone(),
        state: "read".to_string(),
        content_hash: Some(doc.compute_content_hash()),
        graph_id: Some(doc.graph_id.clone()),
        nodes: Some(doc.nodes.len()),
    }
}

/// Read the current content hash of one export, for taking a BASELINE.
///
/// Used by `declare_dependency` when a declaration names an export to watch:
/// the hash recorded is what the declarer saw AT THAT MOMENT, which is the only
/// thing a later "has it moved?" can honestly be measured against. Returns
/// `None` when there is nothing readable there — a declaration whose pointer is
/// wrong must still be recordable, and it comes back as `missing` on the next
/// read rather than being refused now.
pub fn baseline_hash(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<GraphExport>(&raw).ok())
        .map(|doc| doc.compute_content_hash())
}
