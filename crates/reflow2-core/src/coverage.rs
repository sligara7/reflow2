//! COVERAGE — what the design has never been told about (BL-95).
//!
//! Every one of reflow2's gap sources reasons about nodes **already in the
//! graph**: an unsatisfied requirement, an unrealized capability, an unverified
//! one. Not one takes an unmodelled file as its subject. So a graph covering
//! 30% of a system reports the same *"0 open gaps"* as one covering 100% — and
//! the unmodelled fraction is largest exactly where the system is largest, which
//! is where a design brain is worth most. reflow2 will nag forever about a
//! capability it knows is unverified and say nothing at all about ten
//! subsystems it has never heard of.
//!
//! That is not hypothetical. `merge.rs` and `alternatives.rs` — 1,886 lines,
//! shipped in v0.10.0 — sat unmodelled inside reflow2's own repository for two
//! days and nothing fired; they were found by a person looking.
//!
//! ## The trap this deliberately avoids
//!
//! The measure must **not** be a file-count ratio. That would punish exactly the
//! modelling the `adopt` skill mandates — *one Artifact per meaningful unit, not
//! per file; a vendored or generated mass is one opaque Component; granularity
//! tracks distinct contracts, not lines.* A design that correctly models a
//! 900-file vendored tree as a single Component would score 0.1% and be told it
//! had failed.
//!
//! So coverage is measured over **claimed regions, not files**: a registered
//! artifact whose `location` is a directory claims everything beneath it, and
//! one opaque Component legitimately covers the mass under it. What is reported
//! is the *unclaimed* regions, **rolled up to the shallowest wholly-unclaimed
//! directory** and ranked by mass, so the biggest silences sort first and a
//! thousand unmodelled files arrive as one finding about their parent rather
//! than a thousand alarms.
//!
//! ## Contract
//!
//! **reflow2 performs no file I/O**, exactly as `reconcile_artifacts` does not:
//! the caller sweeps the tree and supplies what it saw. This keeps the core
//! free of a filesystem and keeps the sweep's scope something a person chose.
//!
//! It **reports and never scores or blocks** (`dec:report-dont-judge`) — there
//! is no coverage percentage to game and no threshold to fail. Exclusions are
//! **named as excluded**, never silently dropped (rule 6), because "we ignored
//! the vendored tree" and "the vendored tree is covered" must never look alike.
//!
//! ## Deliberately not built yet
//!
//! The sweep is **not persisted**, so `detect_gaps` cannot raise coverage from
//! graph state the way `unresolved_drift` is raised from a recorded
//! `DriftEvent`. That needs a node to record a sweep in — a schema change — and
//! a decision about how stale a recorded sweep may be before its claim expires
//! (the `cap:freshness` precedent). Recorded here rather than half-built:
//! until then, coverage is something a person asks for, and `adopt` should end
//! by asking.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::{DynoError, Value};

use crate::graph::DesignGraph;
use crate::graph_read::GraphRead;
use crate::nodes::node;

/// One thing the caller saw on disk. `mass` is whatever the caller counts —
/// bytes, lines, entries — used only for ranking, never compared across sweeps.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObservedPath {
    /// Path as the design would record it, relative to the project root.
    pub path: String,
    /// Size in the caller's own unit. `0` is fine; ranking then falls back to
    /// how many paths a region holds.
    #[serde(default)]
    pub mass: u64,
}

/// A path the caller deliberately left out of the question, and why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExcludedPath {
    pub path: String,
    /// The exclusion prefix that matched it — so a reader can see the rule, not
    /// just the outcome.
    pub excluded_by: String,
}

/// A directory the design has never claimed any part of.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnclaimedRegion {
    /// The shallowest directory none of whose observed contents are claimed.
    pub path: String,
    /// Observed paths beneath it, all unclaimed.
    pub paths: usize,
    /// Their summed mass — the ranking key, so the biggest silence sorts first.
    pub mass: u64,
    /// A few examples, so the region is recognisable without re-reading the tree.
    pub examples: Vec<String>,
}

/// What the design covers of what the caller actually looked at.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageReport {
    /// Paths considered, after exclusions.
    pub observed: usize,
    /// …of which some node claims them.
    pub claimed: usize,
    /// …and the rest do not.
    pub unclaimed: usize,
    pub claimed_mass: u64,
    pub unclaimed_mass: u64,
    /// Unclaimed regions, biggest first.
    pub unclaimed_regions: Vec<UnclaimedRegion>,
    /// Every path left out, each naming the rule that left it out.
    pub excluded: Vec<ExcludedPath>,
    /// Registered artifact locations that the sweep did NOT report. Either the
    /// sweep was narrower than the design, or the file is gone — the second is
    /// `reconcile_artifacts`' question, and this says which artifacts to ask it
    /// about rather than guessing.
    pub unobserved_locations: Vec<String>,
    /// When the caller says the sweep was taken. reflow2 takes no clock; an
    /// undated sweep is reported as undated rather than assumed current.
    pub swept_at: Option<String>,
    /// Artifacts declaring `granularity: pending_expansion` — PLACEHOLDERS that
    /// stand in for items which should each become their own node, and which
    /// nobody has got to yet (BL-188).
    ///
    /// These qualify every number above them. A directory artifact claims its
    /// whole subtree, so `claimed` counts files that are individually
    /// unreferenceable and coverage reads green over them — measured in the
    /// field as *"every live doc is registered"* across 359 invisible files.
    /// Naming the placeholders is what makes the sentence *"53 artifacts, of
    /// which 3 stand in for the rest"* producible from the graph at all.
    ///
    /// Deliberately carries no count of what each stands for: reflow2 does no
    /// file I/O, so a stored number would be a caller-supplied figure nothing
    /// can recompute. The caller holding the sweep already knows it.
    pub pending_expansion: Vec<String>,
    /// Artifacts declaring `granularity: opaque` — a subtree claimed as a unit
    /// ON PURPOSE (a settled archive, a vendored tree).
    ///
    /// Reported APART from `pending_expansion` because the two are opposite
    /// states that used to read identically: one is a decision, the other is
    /// unfinished work. A report that conflated them would tell a team its
    /// archive was a backlog item, or its backlog was settled.
    pub opaque_claims: Vec<String>,
}

/// Normalise a path for prefix comparison: forward slashes, no `./`, no
/// trailing slash. Two spellings of one path must not read as two places.
fn normalise(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.strip_prefix("./").unwrap_or(&p);
    p.trim_end_matches('/').to_string()
}

/// True when `claim` is `path` itself or a directory containing it. The
/// directory case is what lets one opaque Component legitimately claim the mass
/// beneath it; a bare string prefix would also match `src/foo` against
/// `src/foobar`, which is why the boundary is checked.
fn claims(claim: &str, path: &str) -> bool {
    path == claim || path.starts_with(&format!("{claim}/"))
}

/// Every ancestor directory of a path, shallowest first: `a/b/c.rs` → `a`, `a/b`.
fn ancestors(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut acc = String::new();
    let parts: Vec<&str> = path.split('/').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(part);
        out.push(acc.clone());
    }
    out
}

impl DesignGraph {
    /// Measure what the design covers of a swept tree (BL-95).
    ///
    /// See the module docs for the contract. The short version: the caller
    /// sweeps, reflow2 compares against registered artifact locations, and the
    /// answer is unclaimed *regions* ranked by mass — never a score.
    /// Measure what the design covers of a swept tree (BL-95).
    ///
    /// A delegation to [`coverage_report`], kept so no caller changed when the
    /// reading moved behind `ifc:graph-read`.
    pub fn coverage_report(
        &self,
        observed: &[ObservedPath],
        exclusions: &[String],
        swept_at: Option<&str>,
    ) -> Result<CoverageReport, DynoError> {
        coverage_report(self, observed, exclusions, swept_at)
    }
}

/// Measure what the design covers of a swept tree, over anything readable as a
/// design.
///
/// Behind [`GraphRead`] since 2026-08-21, the second module to move there after
/// `granularity`. It was the cheapest possible next step — 335 lines making
/// exactly ONE store call — which is why it went first: the move is mechanical,
/// so anything that broke would be the contract's fault rather than the
/// module's.
///
/// See the module docs for the contract. The short version: the caller sweeps,
/// reflow2 compares against registered artifact locations, and the answer is
/// unclaimed *regions* ranked by mass — never a score.
pub fn coverage_report(
    g: &dyn GraphRead,
    observed: &[ObservedPath],
    exclusions: &[String],
    swept_at: Option<&str>,
) -> Result<CoverageReport, DynoError> {
    let artifacts = g.scan_nodes(node::ARTIFACT)?;
    let claims_list: Vec<String> = artifacts
        .iter()
        .filter_map(|a| {
            a.properties
                .get("location")
                .and_then(Value::as_str)
                .map(normalise)
        })
        .filter(|l| !l.is_empty())
        .collect();

    // What the numbers below are standing on (BL-188). Collected from the
    // nodes rather than inferred from the paths: whether a directory is a
    // settled archive or an untouched backlog is a statement its author
    // makes, and no amount of looking at the tree can recover it.
    let by_granularity = |want: &str| -> Vec<String> {
        artifacts
            .iter()
            .filter(|a| {
                a.properties
                    .get("granularity")
                    .and_then(Value::as_str)
                    .unwrap_or("atomic")
                    == want
            })
            .map(|a| a.node_id.clone())
            .collect()
    };
    let pending_expansion = by_granularity("pending_expansion");
    let opaque_claims = by_granularity("opaque");
    let exclusions: Vec<String> = exclusions.iter().map(|e| normalise(e)).collect();

    let mut excluded = Vec::new();
    let mut claimed = 0usize;
    let mut claimed_mass = 0u64;
    let mut unclaimed: Vec<(String, u64)> = Vec::new();
    let mut unclaimed_mass = 0u64;
    let mut matched_claims: BTreeSet<String> = BTreeSet::new();

    for obs in observed {
        let path = normalise(&obs.path);
        if let Some(rule) = exclusions.iter().find(|e| claims(e, &path)) {
            excluded.push(ExcludedPath {
                path,
                excluded_by: rule.clone(),
            });
            continue;
        }
        // EVERY claim that covers this path is marked seen, not just the
        // first one found (music_graph F10). A design may legitimately
        // register `archive/` as a whole AND `archive/reco.py` inside it;
        // with `find`, whichever came first absorbed the observation and
        // the other was reported in `unobserved_locations` — a file the
        // sweep had just handed us, named as never swept.
        //
        // That is worse than a wrong number. The field answers "did you
        // forget to sweep something", so a false entry is an alarm on
        // correct modelling, and a reader who meets one stops trusting the
        // only thing the field was for.
        //
        // The COUNT deliberately stays one per observed path: two claims
        // covering one file is one file, and incrementing per claim would
        // trade this bug for an inflated `claimed`.
        let mut covered = false;
        for c in claims_list.iter().filter(|c| claims(c, &path)) {
            matched_claims.insert(c.clone());
            covered = true;
        }
        if covered {
            claimed += 1;
            claimed_mass += obs.mass;
        } else {
            unclaimed_mass += obs.mass;
            unclaimed.push((path, obs.mass));
        }
    }

    // Roll unclaimed paths up to the SHALLOWEST directory none of whose
    // observed contents are claimed. Without this a vendored tree arrives as
    // 900 findings instead of one, and nobody reads the 900.
    let mut has_claimed_below: BTreeSet<String> = BTreeSet::new();
    for obs in observed {
        let path = normalise(&obs.path);
        if exclusions.iter().any(|e| claims(e, &path)) {
            continue;
        }
        if claims_list.iter().any(|c| claims(c, &path)) {
            for dir in ancestors(&path) {
                has_claimed_below.insert(dir);
            }
        }
    }

    let mut regions: BTreeMap<String, (usize, u64, Vec<String>)> = BTreeMap::new();
    for (path, mass) in &unclaimed {
        // The shallowest ancestor with nothing claimed under it; if every
        // ancestor holds something claimed, the file stands alone.
        let region = ancestors(path)
            .into_iter()
            .find(|d| !has_claimed_below.contains(d))
            .unwrap_or_else(|| path.clone());
        let entry = regions.entry(region).or_insert((0, 0, Vec::new()));
        entry.0 += 1;
        entry.1 += mass;
        if entry.2.len() < 3 {
            entry.2.push(path.clone());
        }
    }

    let mut unclaimed_regions: Vec<UnclaimedRegion> = regions
        .into_iter()
        .map(|(path, (paths, mass, examples))| UnclaimedRegion {
            path,
            paths,
            mass,
            examples,
        })
        .collect();
    // Biggest silence first; ties broken by path so the answer is stable.
    unclaimed_regions.sort_by(|a, b| {
        b.mass
            .cmp(&a.mass)
            .then(b.paths.cmp(&a.paths))
            .then(a.path.cmp(&b.path))
    });

    let mut unobserved_locations: Vec<String> = claims_list
        .iter()
        .filter(|c| !matched_claims.contains(*c))
        .cloned()
        .collect();
    unobserved_locations.sort();
    unobserved_locations.dedup();

    Ok(CoverageReport {
        observed: claimed + unclaimed.len(),
        claimed,
        unclaimed: unclaimed.len(),
        claimed_mass,
        unclaimed_mass,
        unclaimed_regions,
        excluded,
        unobserved_locations,
        swept_at: swept_at.map(str::to_string),
        pending_expansion,
        opaque_claims,
    })
}
