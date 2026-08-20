//! What the graph can actually say about the quality axes (BL-184).
//!
//! Anthony, 2026-08-03, correcting a claim of mine that the maturity bands were
//! *"judgement rather than computation"*:
//!
//! > *"I'm not sure a blanket yes or no can be applied here. I feel like there
//! > are some areas where the graph may inform or give a computed signal that
//! > indicates one of the 'ilities' are not sufficiently being met by the
//! > design."*
//!
//! He was right, and the code is blunter than the argument. `DimensionAssessment.score`
//! is **only ever asserted** — a human or an LLM writes a float. The one
//! computation over the `dimension` enum is [`crate::dimensions::DimensionDrift`],
//! which fits a least-squares slope to those assertions. **So reflow2 computes
//! the trend of an opinion and never the ility.**
//!
//! Meanwhile it computes modularity, articulation points, dependency cycles,
//! misplaced capabilities, decomposition mismatches, build granularity and the
//! trajectory bands — **and connects none of it to the axis it informs.** Nine
//! distinctions that only an LLM writes into is `dec:edge-orthogonality`
//! already strained inside the schema.
//!
//! # This module connects; it does not compute
//!
//! Every signal here comes from a computation that already existed, named in
//! [`ility_source`]. That is deliberate: the finding was never *"reflow2 cannot
//! measure these"*, it was *"reflow2 measures them and says nothing about what
//! they are evidence of"*.
//!
//! # It never derives a score, and the precedent is Anthony's own
//!
//! `dec:readiness-is-an-observation-the-threshold-is-the-judgement` refused to
//! put TRL into `DimensionAssessment.score` because *"a 1-9 ladder can only
//! enter it lossily — TRL 7 as 0.78 asserts a precision the ladder does not
//! have"*. Collapsing three cycles and a routing hub into `maintainability:
//! 0.62` is the identical move. So this emits **named findings against an
//! axis**, never a number, and never writes to the graph.
//!
//! # Adverse is inherited, never re-judged
//!
//! A finding counts against an axis **only when the computation that produced
//! it already treats it as a defect**. `detect_defects` findings are defects —
//! that is the module's name and they carry severities. A misplaced capability
//! and a hierarchy mismatch are likewise called out as wrong by their own
//! modules.
//!
//! Everything else is **context, not a charge**: modularity is a ratio, the
//! trajectory bands are a *position* that `crate::maturity` explicitly refuses
//! to grade, a granularity observation is one `crate::granularity` refuses to
//! call a defect, and a surprising connection may be a hidden coupling *or* a
//! creative link the design leans on. Relabelling any of those as adverse here
//! would overrule a judgement another module deliberately declined to make —
//! and would smuggle back the thresholds those modules were built without.
//!
//! # The answer is not blanket, which was his actual point
//!
//! Four of the nine axes cannot be informed by a design graph at all. Nothing
//! structural measures latency, threat, load, or instrumentation, and computing
//! them would be fiction. Those are reported as **not informed, with the
//! reason** — an honest silence rather than an absent entry.

use std::collections::BTreeMap;

use dynograph_core::{DynoError, Value};
use serde::Serialize;

use crate::graph::DesignGraph;
use crate::heal::{HealCategory, HealSeverity};
use crate::nodes::node;

/// The computations a signal can come from, named so a reader can go and check
/// one. Constants rather than inline strings so a test can assert the full set,
/// the same discipline `changelog_rule` and `preserve_rule` hold.
pub mod ility_source {
    /// `detect_defects` — a dependency loop.
    pub const CIRCULAR_DEPENDENCY: &str = "detect_defects.circular_dependency";
    /// `detect_defects` — an articulation point that really separates.
    pub const SINGLE_POINT_OF_FAILURE: &str = "detect_defects.single_point_of_failure";
    /// `detect_defects` — a cluster joined to nothing.
    pub const DISCONNECTED_COMMUNITY: &str = "detect_defects.unthreaded_cluster";
    /// `detect_defects` — two nodes that conflict with no resolving Decision.
    pub const CONTRADICTION: &str = "detect_defects.contradiction";
    /// `evaluate_allocation` — cohesion vs coupling across boundaries.
    pub const MODULARITY: &str = "evaluate_allocation.modularity";
    /// `evaluate_allocation` — capabilities coupled harder outside their
    /// component than inside it.
    pub const MISPLACED_CAPABILITY: &str = "evaluate_allocation.misplaced";
    /// `evaluate_allocation` — routing hubs the architecture cannot lose.
    pub const GOD_COMPONENT: &str = "evaluate_allocation.god_components";
    /// `hierarchy_issues` — a decomposition level skipped or mismatched.
    pub const HIERARCHY_ISSUE: &str = "hierarchy_issues";
    /// `surprising_connections` — coupling bridging distant communities.
    pub const SURPRISING_CONNECTION: &str = "surprising_connections";
    /// `granularity_report` — the build not separating what the design does.
    pub const GRANULARITY: &str = "granularity_report.observations";
    /// `maturity_report` — how much coupling runs through a declared contract.
    pub const SEAMS_BAND: &str = "maturity_report.band.seams";
    /// `maturity_report` — capabilities carrying a passing check.
    pub const ASSURANCE_BAND: &str = "maturity_report.band.assurance";
    /// `maturity_report` — the design's position on the trajectory.
    pub const TRAJECTORY_FRONTIER: &str = "maturity_report.frontier";

    /// Every source, for the exhaustiveness test.
    pub const ALL: &[&str] = &[
        CIRCULAR_DEPENDENCY,
        SINGLE_POINT_OF_FAILURE,
        DISCONNECTED_COMMUNITY,
        CONTRADICTION,
        MODULARITY,
        MISPLACED_CAPABILITY,
        GOD_COMPONENT,
        HIERARCHY_ISSUE,
        SURPRISING_CONNECTION,
        GRANULARITY,
        SEAMS_BAND,
        ASSURANCE_BAND,
        TRAJECTORY_FRONTIER,
    ];
}

/// One thing an existing computation found, attached to the axis it informs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IlityEvidence {
    /// The computation this came from — see [`ility_source`].
    pub source: &'static str,
    /// What it said, in words.
    pub finding: String,
    /// Whether it counts **against** the axis. True only when the producing
    /// computation already treats it as a defect; a ratio or a position is
    /// context, and saying otherwise would overrule a module that deliberately
    /// declined to judge.
    pub adverse: bool,
    /// The nodes involved, sorted. Empty for a design-wide reading.
    pub subjects: Vec<String>,
}

/// An existing `DimensionAssessment` — someone's stated score for this axis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssertedScore {
    pub target_id: String,
    pub score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessed_at: Option<String>,
}

/// What the graph can say about one quality axis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IlitySignal {
    /// The `dimension` enum value, exactly as the schema spells it.
    pub dimension: &'static str,
    /// Whether a design graph can inform this axis at all.
    pub informed: bool,
    /// Why not, when it cannot. Present precisely when `informed` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_informed_because: Option<&'static str>,
    /// Everything the existing computations found for this axis.
    pub evidence: Vec<IlityEvidence>,
    /// How many pieces of evidence count against it.
    pub adverse_findings: usize,
    /// Scores somebody asserted on this axis.
    pub asserted: Vec<AssertedScore>,
    /// Targets where an asserted score points one way and the computed evidence
    /// the other — **the output worth reading**. See
    /// [`IlityReport::direction_midpoint`] for what "one way" means.
    pub worth_weighing: Vec<String>,
}

/// What the graph can say about every axis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IlityReport {
    /// One per `dimension` enum value, in schema order.
    pub signals: Vec<IlitySignal>,
    /// The only non-arbitrary point on the asserted scale, and the reason it is
    /// not a threshold anyone chose: the schema documents `score` as *"0 =
    /// worst, 1 = best"*, so `0.5` is where an assertion stops saying "more
    /// good than bad". Nothing else here compares magnitudes.
    pub direction_midpoint: f64,
    /// What this reading is silent about — on every report.
    pub not_observed_about: Vec<String>,
    pub notes: Vec<String>,
}

/// The nine axes, with why the graph cannot inform four of them.
const AXES: &[(&str, Option<&str>)] = &[
    ("reliability", None),
    (
        "performance",
        Some(
            "nothing structural measures latency, throughput or resource use — a design graph \
             holds what depends on what, never how fast it runs",
        ),
    ),
    ("maintainability", None),
    (
        "security",
        Some(
            "security is a property of threats and trust boundaries, not of a dependency \
             structure; it needs threat modelling, which nothing here performs",
        ),
    ),
    (
        "scalability",
        Some("behaviour under load is not derivable from structure"),
    ),
    (
        "observability",
        Some(
            "whether a running system can be seen into depends on instrumentation the design \
             graph does not record",
        ),
    ),
    ("testability", None),
    ("coupling", None),
    ("maturity", None),
];

/// Which axes a defect category counts against. A category absent from this map
/// informs no axis — `orphan_node` is a real finding about the design and says
/// nothing about any quality axis, so it appears nowhere below.
fn axes_for(category: HealCategory) -> &'static [&'static str] {
    match category {
        // A loop means neither part can be built, tested or reasoned about
        // alone — which is maintainability and testability at once.
        HealCategory::CircularDependency => &["maintainability", "testability", "coupling"],
        HealCategory::SinglePointOfFailure => &["reliability"],
        HealCategory::UnthreadedCluster => &["maintainability"],
        HealCategory::Contradiction => &["maintainability"],
        HealCategory::OrphanNode
        | HealCategory::Duplicate
        | HealCategory::UnresolvedSetup
        | HealCategory::DeadEnd => &[],
    }
}

/// The source constant for a defect category.
fn source_for(category: HealCategory) -> Option<&'static str> {
    match category {
        HealCategory::CircularDependency => Some(ility_source::CIRCULAR_DEPENDENCY),
        HealCategory::SinglePointOfFailure => Some(ility_source::SINGLE_POINT_OF_FAILURE),
        HealCategory::UnthreadedCluster => Some(ility_source::DISCONNECTED_COMMUNITY),
        HealCategory::Contradiction => Some(ility_source::CONTRADICTION),
        _ => None,
    }
}

impl DesignGraph {
    /// What the graph can actually say about each quality axis, and where that
    /// disagrees with what somebody asserted.
    ///
    /// See the module docs for what this refuses to do — derive a score,
    /// re-judge another module's findings, or pretend four of the nine axes are
    /// computable.
    pub fn ility_report(&self) -> Result<IlityReport, DynoError> {
        let mut per_axis: BTreeMap<&'static str, Vec<IlityEvidence>> = BTreeMap::new();
        let mut push = |axis: &'static str, e: IlityEvidence| {
            per_axis.entry(axis).or_default().push(e);
        };

        // ---- defects: adverse, because their own module calls them defects --
        for issue in self.open_defects()? {
            let Some(source) = source_for(issue.category) else {
                continue;
            };
            // Info-level findings are "nice to fix" and are not evidence that a
            // quality axis is unmet; counting them would make a parked decision
            // read as a maintainability problem.
            //
            // UNREACHABLE TODAY, and stated rather than left to be rediscovered:
            // all four categories `source_for` maps are Warning or Critical by
            // construction, and `orphan_node` (the `info` case that motivated
            // this) is already excluded by mapping to no axis. Kept as defence
            // for the day a category that CAN be `info` — `unresolved_setup`,
            // say — gets an axis. Found by mutation-checking: deleting this
            // guard fails nothing, which is a fact about the mapping, not a
            // licence to remove it.
            if issue.severity == HealSeverity::Info {
                continue;
            }
            let mut subjects = issue.affected_ids.clone();
            subjects.sort();
            for axis in axes_for(issue.category) {
                push(
                    axis,
                    IlityEvidence {
                        source,
                        finding: issue.message.clone(),
                        adverse: true,
                        subjects: subjects.clone(),
                    },
                );
            }
        }

        // ---- allocation: one adverse finding, two pieces of context --------
        let alloc = self.evaluate_allocation()?;
        push(
            "coupling",
            IlityEvidence {
                source: ility_source::MODULARITY,
                finding: match alloc.modularity {
                    Some(v) => format!(
                        "modularity {:.2} across {} component(s), measured over {} of them — the \
                         share of coupling weight that stays inside a component. A ratio, \
                         reported as context: no bar is set here.",
                        v,
                        alloc.components.len(),
                        alloc.components_with_coupling
                    ),
                    None => format!(
                        "modularity NOT MEASURABLE: {} of {} component(s) carry any coupling, so \
                         there is no partition to score. Reported as an absence of evidence \
                         rather than as a value — this axis is uninformed here.",
                        alloc.components_with_coupling,
                        alloc.components.len()
                    ),
                },
                adverse: false,
                subjects: Vec::new(),
            },
        );
        for m in &alloc.misplaced {
            push(
                "coupling",
                IlityEvidence {
                    source: ility_source::MISPLACED_CAPABILITY,
                    finding: format!(
                        "{} is allocated to {} but couples more strongly to {}",
                        m.capability_id, m.current_component, m.suggested_component
                    ),
                    adverse: true,
                    subjects: vec![m.capability_id.clone()],
                },
            );
        }
        if !alloc.god_components.is_empty() {
            let mut subjects = alloc.god_components.clone();
            subjects.sort();
            push(
                "reliability",
                IlityEvidence {
                    source: ility_source::GOD_COMPONENT,
                    finding: format!(
                        "{} routing hub(s) whose removal would split the architecture",
                        subjects.len()
                    ),
                    adverse: true,
                    subjects,
                },
            );
        }

        // ---- decomposition: its own module calls these issues --------------
        for h in self.hierarchy_issues()? {
            let mut subjects = h.components.clone();
            subjects.sort();
            push(
                "maintainability",
                IlityEvidence {
                    source: ility_source::HIERARCHY_ISSUE,
                    finding: h.message.clone(),
                    adverse: true,
                    subjects,
                },
            );
        }

        // ---- context: modules that deliberately refuse to judge ------------
        let surprises = self.surprising_connections()?;
        if !surprises.is_empty() {
            push(
                "coupling",
                IlityEvidence {
                    source: ility_source::SURPRISING_CONNECTION,
                    finding: format!(
                        "{} coupling(s) bridge otherwise-distant parts of the design. Context, \
                         not a charge: a bridge may be hidden coupling OR a creative link the \
                         design leans on, and `surprises` does not decide which.",
                        surprises.len()
                    ),
                    adverse: false,
                    subjects: Vec::new(),
                },
            );
        }

        let gran = self.granularity_report()?;
        for o in &gran.observations {
            push(
                "maintainability",
                IlityEvidence {
                    source: ility_source::GRANULARITY,
                    finding: format!(
                        "{} realizes {} capabilities the design distinguishes (median {}). \
                         Context: `granularity` refuses to call this a defect, and so does this.",
                        o.artifact_id,
                        o.realizes_capabilities,
                        gran.median_capabilities_per_artifact
                    ),
                    adverse: false,
                    subjects: vec![o.artifact_id.clone()],
                },
            );
        }

        let maturity = self.maturity_report()?;
        for b in &maturity.bands {
            let axis = match b.name {
                "seams" => "coupling",
                "assurance" => "testability",
                _ => continue,
            };
            let source = if b.name == "seams" {
                ility_source::SEAMS_BAND
            } else {
                ility_source::ASSURANCE_BAND
            };
            let value = match b.ratio {
                Some(r) => format!("{:.1}% ({}/{})", r * 100.0, b.present, b.population),
                None => "not measurable".to_string(),
            };
            push(
                axis,
                IlityEvidence {
                    source,
                    finding: format!(
                        "trajectory band `{}` reads {value}. Context: a POSITION, which \
                         `maturity` explicitly refuses to grade — being early is not a fault.",
                        b.name
                    ),
                    adverse: false,
                    subjects: Vec::new(),
                },
            );
        }
        if let Some(frontier) = maturity.frontier {
            push(
                "maturity",
                IlityEvidence {
                    source: ility_source::TRAJECTORY_FRONTIER,
                    finding: format!(
                        "the design's frontier is `{frontier}` — the lowest-scoring band. A \
                         position on the trajectory, not a score against it."
                    ),
                    adverse: false,
                    subjects: Vec::new(),
                },
            );
        }

        // ---- what somebody asserted ----------------------------------------
        let mut asserted: BTreeMap<String, Vec<AssertedScore>> = BTreeMap::new();
        for a in self.scan_nodes(node::DIMENSION_ASSESSMENT)? {
            let (Some(dim), Some(target)) = (
                a.properties.get("dimension").and_then(Value::as_str),
                a.properties.get("target_id").and_then(Value::as_str),
            ) else {
                continue;
            };
            let Some(score) = a.properties.get("score").and_then(Value::as_f64) else {
                continue;
            };
            asserted
                .entry(dim.to_string())
                .or_default()
                .push(AssertedScore {
                    target_id: target.to_string(),
                    score,
                    assessed_at: a
                        .properties
                        .get("assessed_at")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
        }

        const MIDPOINT: f64 = 0.5;
        let mut signals = Vec::with_capacity(AXES.len());
        for (dimension, why_not) in AXES {
            let mut evidence = per_axis.remove(*dimension).unwrap_or_default();
            evidence.sort_by(|a, b| (a.source, &a.finding).cmp(&(b.source, &b.finding)));
            let adverse_findings = evidence.iter().filter(|e| e.adverse).count();
            let mut scores = asserted.remove(*dimension).unwrap_or_default();
            scores.sort_by(|a, b| a.target_id.cmp(&b.target_id));

            // A disagreement in DIRECTION, never in magnitude. Only the
            // asserted-good / evidence-adverse case is reported: the reverse
            // would rest on an ABSENCE of findings, and absence of evidence is
            // not evidence of a problem.
            let worth_weighing: Vec<String> = if adverse_findings > 0 {
                scores
                    .iter()
                    .filter(|s| s.score > MIDPOINT)
                    .filter(|s| {
                        evidence
                            .iter()
                            .any(|e| e.adverse && e.subjects.contains(&s.target_id))
                    })
                    .map(|s| s.target_id.clone())
                    .collect()
            } else {
                Vec::new()
            };

            signals.push(IlitySignal {
                dimension,
                informed: why_not.is_none(),
                not_informed_because: *why_not,
                evidence,
                adverse_findings,
                asserted: scores,
                worth_weighing,
            });
        }

        let informed = signals.iter().filter(|s| s.informed).count();
        let mut notes = vec![format!(
            "{informed} of {} axes can be informed by a design graph at all; the rest report why \
             not. That split is the point — computing the other four would be fiction.",
            AXES.len()
        )];
        let flagged: usize = signals.iter().map(|s| s.worth_weighing.len()).sum();
        if flagged > 0 {
            notes.push(format!(
                "{flagged} target(s) carry an asserted score above {MIDPOINT} on an axis where a \
                 detector found something against them. That is a disagreement between two \
                 records, not a ruling on which is right."
            ));
        }

        Ok(IlityReport {
            signals,
            direction_midpoint: MIDPOINT,
            not_observed_about: vec![
                "Any score of its own. This never derives a number and never writes to the \
                 graph — collapsing findings into `maintainability: 0.62` asserts a precision \
                 nobody has, which is why TRL was kept out of that same float."
                    .to_string(),
                "Whether an asserted score is right. A disagreement says the two records point \
                 different ways; which one to believe is not reflow2's call."
                    .to_string(),
                "Four of the nine axes entirely — performance, security, scalability and \
                 observability are reported as not informed, with the reason, rather than \
                 silently omitted."
                    .to_string(),
                "Anything the underlying computations cannot see. This connects existing \
                 findings to axes; it adds no new detection, so every blind spot they have, it \
                 has."
                    .to_string(),
                "Whether a clean axis is actually healthy. No adverse finding means no detector \
                 fired — absence of evidence, which is why the reverse disagreement (asserted \
                 low, nothing found) is deliberately NOT flagged."
                    .to_string(),
            ],
            notes,
        })
    }
}
