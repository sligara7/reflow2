//! Granularity — does the build separate what the design separates?
//!
//! **This module reports a fact and refuses a verdict.** It never says
//! "monolith", never says "too big", never says "split it", and carries no
//! severity. It says one thing: *the design distinguishes N capabilities here,
//! and the build distinguishes none of them* — and leaves what to do about it
//! to the agent and the human (`dec:report-dont-judge`,
//! `dec:three-party-checks`).
//!
//! # Why this is a computation and not an opinion
//!
//! "Avoid monoliths" is a design principle, it is subjective, and reflow2
//! cannot read code — so it is not what is measured here. What is measured is a
//! **disagreement between two records the design already holds**:
//!
//! - the design asserts these are N distinct Capabilities — someone wrote them
//!   as N separate nodes;
//! - the build asserts they are one Artifact.
//!
//! That is the same shape as the rest of the family. [`crate::compare`] finds
//! design-vs-design disagreement, [`crate::drift`] finds design-vs-disk
//! disagreement, and this finds design-granularity vs build-granularity
//! disagreement. In every case reflow2 reports that two records disagree and
//! **never rules on which is right**: N capabilities in one file may mean the
//! file should be N files, or that the design over-decomposed and should be
//! fewer capabilities, or that it is exactly right for this phase.
//!
//! # Why there is no size threshold
//!
//! There is no design-derived answer to *how big is too big*, so this asks a
//! different question. It compares an artifact against **this design's own
//! distribution**, not against an absolute bar — which is what makes it safe
//! under `dec:maturity-restructuring-delta`: an early-phase design where every
//! capability lives in one file has no outlier and gets no finding, because
//! there is nothing to be out of line *with*. It only speaks once a design has
//! decomposed elsewhere and left one place behind. That is a position on the
//! trajectory, not a score — and refusing to be a score is the same discipline
//! `cap:coverage` keeps.
//!
//! The precedent is next door: [`crate::surprises`] excludes `PROVIDES` and
//! `CONSUMES` precisely because every properly-modelled contract would read as
//! a "sole bridge" — *the design discipline penalising itself*. The cutoff here
//! is stated in the report for the same reason, rather than hidden in the code.
//!
//! # What it cannot see, and says so
//!
//! It reads `REALIZES` edges, so it sees only artifacts somebody **registered**
//! — an unregistered monolith is invisible to it, which is [BL-165]'s family.
//! And it measures the *design's* granularity, not the file's: an artifact
//! holding six thousand lines that realizes exactly one capability is entirely
//! silent here, and needs a mass observation this module deliberately does not
//! take.
//!
//! Pure arithmetic over edges already in the graph — no file I/O, no LLM, and
//! deterministic: the same design always yields the byte-identical report.

use std::collections::{BTreeMap, BTreeSet};

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::StoredNode;
use crate::graph::DesignGraph;
use crate::graph_read::GraphRead;
use crate::nodes::{edge, node};

/// How far above this design's own mean an artifact must sit before it is
/// worth mentioning, in standard deviations.
///
/// **A distributional cutoff, not an absolute one** — the first concrete
/// instance of `dec:idea-distributional-thresholds`. It is a constant so it can
/// be stated in every report rather than lurking in a comparison, because a
/// threshold nobody can see is one nobody can argue with.
pub const UNUSUAL_AT: f64 = 2.0;

/// How many distinctions the design must be making before a mismatch is worth
/// a person's attention.
///
/// **A judgement, stated rather than hidden, and the reason it is needed is
/// statistical.** Capability-per-artifact counts pile up at one, so the
/// standard deviation collapses and [`UNUSUAL_AT`] alone would fire on an
/// artifact realizing *two* capabilities in a design where the rest realize
/// one. "The design separates two things and the build separates neither" is
/// true and not worth saying.
///
/// Note what this is **not**: it is a floor on how many distinctions the design
/// itself draws, never on the artifact's size. Nothing here has an opinion
/// about lines of code.
pub const MIN_DISTINCTIONS: usize = 3;

/// The smallest population this will speak about at all.
///
/// Below it the mean and spread mean nothing, and a "finding" would be an
/// artefact of having three artifacts rather than a fact about the design.
/// Reported as a note, never as silence.
pub const MIN_POPULATION: usize = 5;

/// One artifact whose build granularity is out of line with the design's,
/// measured against this design's own distribution.
///
/// Deliberately carries **no** severity, no category and no suggested fix. The
/// fields are the observation and the arithmetic behind it, so a reader can
/// disagree with the reading without re-deriving it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GranularityObservation {
    pub artifact_id: String,
    /// The artifact's `name`, when it carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where the artifact lives, when the design records it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// How many Capabilities this one artifact realizes.
    pub realizes_capabilities: usize,
    /// The ids, sorted — so the reader can see *what* the build is not
    /// separating without a second call.
    pub capability_ids: Vec<String>,
    /// How many artifacts in this design realize at least this many. `1` means
    /// it stands alone.
    pub at_or_above: usize,
    /// Distributional position: standard deviations above this design's mean.
    /// A position, **not** a severity — nothing downstream gates on it.
    pub unusual: f64,
    /// Plain-language statement of what was observed, in the house style of
    /// [`crate::surprises`]'s explained findings.
    pub reasons: Vec<String>,
}

/// The granularity reading for a whole design.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GranularityReport {
    /// Artifacts out of line with this design's own distribution, most
    /// out-of-line first. Empty is a perfectly ordinary answer.
    pub observations: Vec<GranularityObservation>,
    /// Registered artifacts that realize at least one Capability — the
    /// population every figure above is relative to.
    pub population: usize,
    pub mean_capabilities_per_artifact: f64,
    pub median_capabilities_per_artifact: f64,
    /// The cutoffs actually applied, echoed so they can be argued with.
    pub unusual_at: f64,
    /// The minimum number of design distinctions an artifact must collapse
    /// before it is mentioned. See [`MIN_DISTINCTIONS`].
    pub min_distinctions: usize,
    /// What this reading is silent about. Present on every report, including
    /// an empty one — a quiet report is evidence about what it covers and says
    /// nothing about the rest.
    pub not_observed_about: Vec<String>,
    /// Anything that shaped this particular answer: too small a population, no
    /// spread, nothing registered.
    pub notes: Vec<String>,
}

/// The sample median of an already-sorted slice.
fn median(sorted: &[usize]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    } else {
        sorted[mid] as f64
    }
}

impl DesignGraph {
    /// Read how the build's granularity compares with the design's.
    ///
    /// See the module docs for what this does and — more importantly — what it
    /// refuses to do.
    ///
    /// A one-line delegation to [`granularity_report`], which is where the
    /// work lives. Kept so that every existing caller — `report.rs`,
    /// `ility.rs`, the tests — is untouched by the module moving behind a
    /// contract: a refactor that forced its callers to change would be paying
    /// for the boundary twice.
    pub fn granularity_report(&self) -> Result<GranularityReport, DynoError> {
        granularity_report(self)
    }
}

/// Read how the build's granularity compares with the design's, over anything
/// that can be read as a design.
///
/// ⭐ THE FIRST MODULE IN THIS CRATE TO STAND BEHIND [`GraphRead`] RATHER THAN
/// INSIDE `DesignGraph`, and the pilot for the rest. Taking `&dyn GraphRead`
/// rather than `&DesignGraph` buys three things that were not previously
/// possible for any module here:
///
/// - **it can be swapped**: any implementation of the contract can be fed to
///   it, and it cannot tell the difference;
/// - **it can be tested with no store at all** — see this module's tests,
///   which build a design in memory as plain vectors;
/// - **it can be optimised against a budget**, because the boundary is now
///   somewhere a measurement can be taken and held still.
///
/// It also cannot corrupt anything it reads. `GraphRead` has no writes, so the
/// compiler now enforces what the module docs used to only assert.
pub fn granularity_report(g: &dyn GraphRead) -> Result<GranularityReport, DynoError> {
    // Capabilities realized per artifact. Components are deliberately not
    // counted: an artifact realizing its component is the ordinary way to
    // say "this file is that part", and counting it would make every
    // properly-registered artifact look coarser than it is.
    let mut per_artifact: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Which ids ARE capabilities, asked once rather than once per edge.
    //
    // This loop used to call `get_node(CAPABILITY, ..)` for every REALIZES edge
    // purely to type-check its target — 333 point lookups on reflow2's own
    // design, against 218 artifacts. That made the reading cost one store read
    // per EDGE when its answer is about ARTIFACTS, which is exactly what
    // `con:granularity-reads-scale-with-artifacts-not-edges` forbids. One scan
    // answers all of them, and membership is then free.
    let capability_ids: BTreeSet<String> = g
        .scan_nodes(node::CAPABILITY)?
        .into_iter()
        .map(|n| n.node_id)
        .collect();
    // The artifacts are kept, not just their ids: the observation loop below
    // needs each flagged artifact's properties, and re-reading them would be
    // paying the store twice for something already in hand.
    let mut artifacts: BTreeMap<String, StoredNode> = BTreeMap::new();
    for art in g.scan_nodes(node::ARTIFACT)? {
        let art_id = art.node_id.clone();
        let mut caps: Vec<String> = Vec::new();
        for e in g.outgoing(&art.node_id, Some(edge::REALIZES))? {
            if capability_ids.contains(&e.to_id) {
                caps.push(e.to_id);
            }
        }
        if !caps.is_empty() {
            caps.sort();
            artifacts.insert(art.node_id.clone(), art);
            per_artifact.insert(art_id, caps);
        }
    }

    let population = per_artifact.len();
    let mut counts: Vec<usize> = per_artifact.values().map(Vec::len).collect();
    counts.sort_unstable();

    let mut notes = Vec::new();
    let not_observed_about = vec![
        "Artifacts nobody registered. This reads REALIZES edges, so a file the design never \
             claimed is invisible here — run coverage_report for that question."
            .to_string(),
        "The size of anything. This measures the DESIGN's granularity against the build's; \
             an artifact of six thousand lines realizing exactly one capability is silent here."
            .to_string(),
        "Outliers that hide each other. The spread an artifact is measured against includes \
             the outliers themselves, so several equally coarse artifacts mask one another and \
             may all go unreported. Read this as a prompt to look, never as a count of how many \
             there are."
            .to_string(),
        "Which side is right. N capabilities in one artifact may mean the artifact should be \
             N files, or that the design over-decomposed and should hold fewer capabilities, or \
             that it is correct for this phase. That judgement is not reflow2's."
            .to_string(),
    ];

    if population < MIN_POPULATION {
        notes.push(format!(
            "{population} artifact(s) realize a capability — below the {MIN_POPULATION} this \
                 will speak about. A spread computed over so few would describe the sample, not \
                 the design, so nothing is reported."
        ));
        return Ok(GranularityReport {
            observations: Vec::new(),
            population,
            mean_capabilities_per_artifact: 0.0,
            median_capabilities_per_artifact: 0.0,
            unusual_at: UNUSUAL_AT,
            min_distinctions: MIN_DISTINCTIONS,
            not_observed_about,
            notes,
        });
    }

    let n = counts.len() as f64;
    let mean = counts.iter().sum::<usize>() as f64 / n;
    let variance = counts
        .iter()
        .map(|&c| {
            let d = c as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / (n - 1.0);
    let sd = variance.sqrt();
    let med = median(&counts);

    if sd == 0.0 {
        notes.push(
            "Every registered artifact realizes the same number of capabilities, so nothing \
                 is out of line with anything. A design whose build is uniformly coarse is not \
                 reported as a problem — there is no outlier, and an absolute bar is exactly what \
                 this refuses to apply."
                .to_string(),
        );
        return Ok(GranularityReport {
            observations: Vec::new(),
            population,
            mean_capabilities_per_artifact: mean,
            median_capabilities_per_artifact: med,
            unusual_at: UNUSUAL_AT,
            min_distinctions: MIN_DISTINCTIONS,
            not_observed_about,
            notes,
        });
    }

    let mut observations = Vec::new();
    for (artifact_id, capability_ids) in &per_artifact {
        let realizes = capability_ids.len();
        let unusual = (realizes as f64 - mean) / sd;
        if unusual < UNUSUAL_AT || realizes < MIN_DISTINCTIONS {
            continue;
        }
        let at_or_above = counts.iter().filter(|&&c| c >= realizes).count();
        let art = artifacts.get(artifact_id);
        let prop = |k: &str| {
            art.as_ref()
                .and_then(|a| a.properties.get(k))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let mut reasons = vec![format!(
            "The design distinguishes {realizes} capabilities here; the build holds them in \
                 one artifact, so it separates none of them."
        )];
        reasons.push(format!(
            "The median registered artifact realizes {med} capability(ies), across a \
                 population of {population}."
        ));
        if at_or_above == 1 {
            reasons.push(
                "No other artifact in this design realizes as many, so this is not the \
                     design's normal coarseness — it is one place that did not follow the rest."
                    .to_string(),
            );
        } else {
            reasons.push(format!(
                "{at_or_above} artifacts realize at least this many."
            ));
        }
        observations.push(GranularityObservation {
            artifact_id: artifact_id.clone(),
            name: prop("name"),
            location: prop("location"),
            realizes_capabilities: realizes,
            capability_ids: capability_ids.clone(),
            at_or_above,
            unusual,
            reasons,
        });
    }

    // Most out-of-line first; id breaks ties so the report is deterministic.
    observations.sort_by(|a, b| {
        b.unusual
            .partial_cmp(&a.unusual)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.artifact_id.cmp(&b.artifact_id))
    });

    if observations.is_empty() {
        notes.push(format!(
            "No artifact both sits {UNUSUAL_AT} standard deviations above this design's own \
                 mean of {mean:.2} and collapses at least {MIN_DISTINCTIONS} distinctions. That \
                 is an ordinary answer, not a clean bill of health — the cutoff is \
                 distributional, so a uniformly coarse design reports nothing."
        ));
    }

    Ok(GranularityReport {
        observations,
        population,
        mean_capabilities_per_artifact: mean,
        median_capabilities_per_artifact: med,
        unusual_at: UNUSUAL_AT,
        min_distinctions: MIN_DISTINCTIONS,
        not_observed_about,
        notes,
    })
}
