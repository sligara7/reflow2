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
    pub fn coverage_report(
        &self,
        observed: &[ObservedPath],
        exclusions: &[String],
        swept_at: Option<&str>,
    ) -> Result<CoverageReport, DynoError> {
        let claims_list: Vec<String> = self
            .scan_nodes(node::ARTIFACT)?
            .iter()
            .filter_map(|a| {
                a.properties
                    .get("location")
                    .and_then(Value::as_str)
                    .map(normalise)
            })
            .filter(|l| !l.is_empty())
            .collect();
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
            match claims_list.iter().find(|c| claims(c, &path)) {
                Some(c) => {
                    matched_claims.insert(c.clone());
                    claimed += 1;
                    claimed_mass += obs.mass;
                }
                None => {
                    unclaimed_mass += obs.mass;
                    unclaimed.push((path, obs.mass));
                }
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
        })
    }
}
