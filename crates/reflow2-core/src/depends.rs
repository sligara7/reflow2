//! DEPENDS — declare which version of another design you depend on, and check
//! that claim against the build (`req:design-dependencies-declared`).
//!
//! ## Why this is not optional bookkeeping
//!
//! A seam analysis compares your design against a dependency's published
//! surface. Both sides move. Without a recorded pin there is nothing to take a
//! surface *as of*, so the comparison silently answers a question nobody asked:
//! "are you compatible with whatever the provider's `main` happens to be right
//! now?" — when you are not on `main` and will not be until you bump.
//!
//! Proven, not supposed: reflow2 pins dynograph-foundation at `v0.11.0` while
//! storyflow pins `v0.9.4`, two minors apart, and the provider could not produce
//! an as-of-tag surface at all. An offer taken from `main` described **neither**
//! consumer's actual contract.
//!
//! ## Two different facts, and conflating them is the bug
//!
//! - **What you MEAN to depend on** — the declaration. Durable, reviewed,
//!   committed, and the thing a provider can acknowledge.
//! - **What your build ACTUALLY resolves** — the observation. Read fresh from
//!   the build files every time, because that is what ships.
//!
//! Storing only the first gives you a document that drifts from reality. Storing
//! only the second gives you a fact with no intent behind it, so nothing can
//! ever be *wrong*. Keeping both, and comparing them, is what makes
//! "am I relying on something I never declared?" answerable — the state the
//! cross-repo trial named as the dangerous one, because it breaks with nobody at
//! fault.
//!
//! ## Why core does not parse Cargo.toml
//!
//! The caller supplies the observation, exactly as [`reconcile_artifacts`] takes
//! `observed` and `coverage_report` takes paths. Two reasons, and the second is
//! the load-bearing one:
//!
//! 1. One parser per ecosystem in Rust is a maintenance burden with no analytic
//!    gain — an agent reads a manifest perfectly well.
//! 2. **The consumers are not all Rust.** storyflow pins foundation crates in a
//!    `Cargo.toml`, a container image in `docker-compose.yml`, and versions in a
//!    `versions.env` — three build files, one dependency. A core that understood
//!    only Cargo would model a third of that seam and report the rest as absent.

use std::collections::{BTreeMap, BTreeSet};

use crate::foundation::core::{DynoError, Value};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

/// A dependency on another design, as this design DECLARES it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DependencyDeclaration {
    /// Stable id for the dependency, e.g. `dep:dynograph-foundation`.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Where it comes from — a git URL, a registry, a path. Free text, because
    /// "where a dependency comes from" has no closed vocabulary.
    pub source: String,
    /// The version this design MEANS to depend on: a tag, a commit, a release.
    /// The whole point of the file.
    pub version: String,
    /// The parts actually taken — crate names, service names, whatever the
    /// dependency's unit of consumption is.
    pub components: Vec<String>,
    /// Build-level switches this design forwards to the dependency BY NAME.
    /// Recorded because they are contract whether or not the provider thinks so:
    /// a renamed feature is a downstream build break that no public-API diff or
    /// surface export would mention.
    pub features: Vec<String>,
    /// Which build file the pin actually lives in, so the claim can be rechecked
    /// at its source rather than trusted.
    pub declared_in: Option<String>,
    /// The `graph_id` of the dependency's OWN reflow2 design, when it has one.
    ///
    /// This is the link between two facts that already sit side by side in the
    /// same file and never touched: "my build pins v0.12.0 of this thing" and
    /// "that thing is also a design I can compose with". With it, the
    /// composition target is derivable from a committed, version-pinned manifest
    /// instead of being configured per machine — and it inherits the DIRECTION
    /// the dependency edge already carries, which a flat list of graph ids
    /// cannot express.
    ///
    /// ⚠️ OPTIONAL, AND ITS ABSENCE MEANS "NOBODY HAS SAID" — never "there is no
    /// design". Most dependencies will never have one: serde, tokio, rocksdb.
    /// A dependency without a graph_id is the ordinary case and must not read as
    /// a defect, which is the same rule `reconcile_dependencies` already applies
    /// to the manifest as a whole.
    pub graph_id: Option<String>,

    /// Path to the dependency design's COMMITTED EXPORT, when this design means
    /// to watch it.
    ///
    /// ⭐ WHY A SECOND FIELD AND NOT JUST `graph_id`. The id says WHICH design;
    /// this says WHERE ITS RECORD IS. An id alone is not resolvable — reflow2
    /// does no file navigation (`describe_designs` makes the caller find the
    /// candidate paths for exactly this reason), so a watch that had only an id
    /// would have to go looking, which is the rule this deliberately does not
    /// break. Naming the path in the committed manifest keeps the pointer where
    /// a person can review it in a diff.
    ///
    /// Absent means NOBODY HAS SAID, never "there is nothing to watch": a
    /// declaration naming a design and no export is reported as `not_watched`
    /// rather than passing quietly.
    #[serde(default)]
    pub design_export: Option<String>,
    /// The upstream export's content hash AS THE DECLARER LAST SAW IT.
    ///
    /// 🛑 THIS IS A BASELINE, NOT A CACHE, and nothing may refresh it on a read.
    /// A check that updated its own baseline would report `moved` exactly once
    /// and then be permanently quiet — the failure mode that makes a signal
    /// worse than no signal. Re-declaring is the acknowledgement
    /// (`dec:ask-not-repair`: name the remedy, never take it).
    #[serde(default)]
    pub design_export_hash: Option<String>,
    /// When that baseline was taken. Caller-supplied: the core takes no clock,
    /// so an undated baseline is REPORTED as undated and never assumed fresh.
    #[serde(default)]
    pub design_export_seen_at: Option<String>,
    /// Free-text note — why this pin, what was verified, what is owed.
    pub note: Option<String>,
}

/// What a build actually resolves, supplied by the caller at check time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservedDependency {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
    /// Where this was read from.
    #[serde(default)]
    pub observed_in: Option<String>,
}

/// One disagreement between what was declared and what the build resolves.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DependencyFinding {
    /// `undeclared` | `unobserved` | `version_mismatch` | `undeclared_component`
    /// | `undeclared_feature`
    pub kind: &'static str,
    pub dependency: String,
    pub detail: String,
}

/// The result of checking declarations against a build.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DependencyReport {
    pub declared: Vec<DependencyDeclaration>,
    pub findings: Vec<DependencyFinding>,
    /// Declarations an accepted Decision has WITHDRAWN, skipped by the
    /// `unobserved` check rather than reported as stale. Named rather than
    /// dropped: a dependency that ended is design history and stays readable,
    /// but it must not keep failing a gate for not being in a build it was
    /// deliberately removed from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired_declarations: Vec<String>,
    /// Said plainly whichever way it comes out — "nothing declared" and
    /// "nothing to declare" must never look alike.
    pub note: String,
}

impl DesignGraph {
    /// Declare a dependency on another design (`req:design-dependencies-declared`).
    pub fn declare_external_dependency(
        &mut self,
        decl: &DependencyDeclaration,
    ) -> Result<(), DynoError> {
        if decl.version.trim().is_empty() {
            return Err(DynoError::Validation {
                node_type: node::RESOURCE.into(),
                property: "version".into(),
                message: "a dependency declaration without a version is not a declaration: the \
                          version is the whole point, because it is what a published surface can \
                          be taken AS OF"
                    .into(),
            });
        }
        let mut props = Props::new()
            .set("name", decl.name.as_str())
            .set("resource_type", "design-dependency")
            .set("provider", decl.source.as_str())
            .set("version", decl.version.as_str())
            .set("components", decl.components.join(","))
            .set("features", decl.features.join(","));
        if let Some(d) = &decl.declared_in {
            props = props.set("declared_in", d.as_str());
        }
        // Only when stated. An empty string would be a claim that the dependency
        // has a design whose id happens to be blank, which is not the same as
        // nobody having said.
        if let Some(g) = decl.graph_id.as_deref().filter(|g| !g.trim().is_empty()) {
            props = props.set("dependency_graph_id", g);
        }
        // Only when stated, for the same reason `graph_id` is: an empty string
        // would claim a watch target whose path is blank, which is not the same
        // fact as nobody having named one.
        // ⚠️ THE BASELINE AND ITS DATE EXIST ONLY RELATIVE TO A PATH, so they are
        // stored only when there is one. A hash with nothing to compare it
        // against, or a date saying when a target nobody named was last seen,
        // is a record of a check that never happened — and it reads to a person
        // scanning the manifest exactly like one that did. Found by
        // `an_unwatched_dependency_emits_no_watch_fields_at_all`, which failed
        // on a leftover `design_export_seen_at`.
        if let Some(p) = decl
            .design_export
            .as_deref()
            .filter(|p| !p.trim().is_empty())
        {
            props = props.set("design_export", p);
            if let Some(h) = decl
                .design_export_hash
                .as_deref()
                .filter(|h| !h.trim().is_empty())
            {
                props = props.set("design_export_hash", h);
            }
            if let Some(a) = decl
                .design_export_seen_at
                .as_deref()
                .filter(|a| !a.trim().is_empty())
            {
                props = props.set("design_export_seen_at", a);
            }
        }
        if let Some(n) = &decl.note {
            props = props.set("description", n.as_str());
        }
        self.create_node(node::RESOURCE, &decl.id, props)?;
        for p in self.scan_nodes(node::PROJECT)? {
            self.create_edge(
                edge::REQUIRES_RESOURCE,
                node::PROJECT,
                &p.node_id,
                node::RESOURCE,
                &decl.id,
                std::collections::HashMap::from([(
                    "criticality".to_string(),
                    Value::from("required"),
                )]),
            )?;
        }
        Ok(())
    }

    /// Every declared dependency, in id order.
    pub fn declared_dependencies(&self) -> Result<Vec<DependencyDeclaration>, DynoError> {
        let mut out = Vec::new();
        for n in self.scan_nodes(node::RESOURCE)? {
            let get = |k: &str| {
                n.properties
                    .get(k)
                    .and_then(Value::as_str)
                    .map(str::to_string)
            };
            if get("resource_type").as_deref() != Some("design-dependency") {
                continue;
            }
            let split = |k: &str| -> Vec<String> {
                get(k)
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            out.push(DependencyDeclaration {
                id: n.node_id.clone(),
                name: get("name").unwrap_or_default(),
                source: get("provider").unwrap_or_default(),
                version: get("version").unwrap_or_default(),
                components: split("components"),
                features: split("features"),
                declared_in: get("declared_in"),
                graph_id: get("dependency_graph_id"),
                design_export: get("design_export"),
                design_export_hash: get("design_export_hash"),
                design_export_seen_at: get("design_export_seen_at"),
                note: get("description"),
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Check the declarations against what a build actually resolves.
    ///
    /// Catches the two opposite failures the cross-repo trial named:
    /// **relying on something never declared**, and **declaring something the
    /// build no longer takes**.
    pub fn reconcile_dependencies(
        &self,
        observed: &[ObservedDependency],
    ) -> Result<DependencyReport, DynoError> {
        let declared = self.declared_dependencies()?;
        let by_name: BTreeMap<&str, &DependencyDeclaration> =
            declared.iter().map(|d| (d.name.as_str(), d)).collect();
        let observed_names: BTreeSet<&str> = observed.iter().map(|o| o.name.as_str()).collect();
        let mut findings = Vec::new();

        for o in observed {
            let Some(d) = by_name.get(o.name.as_str()) else {
                findings.push(DependencyFinding {
                    kind: "undeclared",
                    dependency: o.name.clone(),
                    detail: format!(
                        "the build depends on '{}' at {} and nothing declares it — this is the \
                         reliance nobody agreed to, and it breaks with nobody at fault",
                        o.name, o.version
                    ),
                });
                continue;
            };
            if d.version != o.version {
                findings.push(DependencyFinding {
                    kind: "version_mismatch",
                    dependency: o.name.clone(),
                    detail: format!(
                        "declared {} but the build resolves {} — a seam checked against the \
                         declared version would be answering about a version you do not ship",
                        d.version, o.version
                    ),
                });
            }
            let known: BTreeSet<&str> = d.components.iter().map(String::as_str).collect();
            for c in &o.components {
                if !known.contains(c.as_str()) {
                    findings.push(DependencyFinding {
                        kind: "undeclared_component",
                        dependency: o.name.clone(),
                        detail: format!(
                            "the build takes '{c}' and the declaration does not list it"
                        ),
                    });
                }
            }
            let known_f: BTreeSet<&str> = d.features.iter().map(String::as_str).collect();
            for f in &o.features {
                if !known_f.contains(f.as_str()) {
                    findings.push(DependencyFinding {
                        kind: "undeclared_feature",
                        dependency: o.name.clone(),
                        detail: format!(
                            "the build forwards feature '{f}' by name and the declaration does \
                             not list it — a renamed feature is a build break no API diff mentions"
                        ),
                    });
                }
            }
        }
        let mut retired = Vec::new();
        for d in &declared {
            if observed_names.contains(d.name.as_str()) {
                continue;
            }
            // A RETIRED DECLARATION IS NOT A STALE ONE, and reporting it as
            // `unobserved` forever is how a correct retirement becomes a
            // permanently red gate. `is_discontinued` is the design's existing
            // answer to "has an accepted Decision withdrawn this?" — the same
            // test `get_node` reports and the defect detectors already use.
            //
            // 🛑 THIS WAS FOUND BY DOGFOODING AND IT IS A CLASS, NOT A ONE-OFF.
            // reflow2 absorbed dynograph-foundation on 2026-08-24, retired
            // `dep:dynograph-foundation` correctly — deprecation ChangeEvent,
            // snapshot, OBSOLETES from the accepted Decision — and the gate went
            // on failing, because this reader never asked. The design already
            // records that `is_discontinued` is honoured at only a handful of
            // sites; this was another of them.
            //
            // IT IS STILL REPORTED, not silenced: `retired_declarations` says
            // which ones were skipped and why, because a declaration vanishing
            // from a report with no trace is the silent-success failure this
            // project spends most of its guards on.
            if self.is_discontinued(&d.id)? {
                retired.push(d.name.clone());
                continue;
            }
            findings.push(DependencyFinding {
                kind: "unobserved",
                dependency: d.name.clone(),
                detail: format!(
                    "'{}' is declared at {} and the build does not take it — either the \
                     declaration is stale or the observation is incomplete; both are worth \
                     knowing and neither is assumed",
                    d.name, d.version
                ),
            });
        }

        let note = if declared.is_empty() {
            "NOTHING IS DECLARED. Read that as \"nobody has said\", never as \"this design depends \
             on nothing\" — an empty declaration set is indistinguishable from one never written, \
             which is why it is stated rather than left to inference."
                .to_string()
        } else if findings.is_empty() {
            format!(
                "{} dependency(ies) declared, and the build agrees with every one.",
                declared.len()
            )
        } else {
            format!(
                "{} dependency(ies) declared, {} disagreement(s) with the build.",
                declared.len(),
                findings.len()
            )
        };
        Ok(DependencyReport {
            declared,
            findings,
            retired_declarations: retired,
            note,
        })
    }

    /// The declarations as a `reflow2.toml` document.
    ///
    /// Carries **which reflow2 wrote it** (Anthony's ask, and the same reasoning
    /// as the export's version stamp): a file whose producer is unknown cannot
    /// be read safely by a tool that has since changed what the fields mean.
    pub fn dependency_manifest(&self) -> Result<String, DynoError> {
        let declared = self.declared_dependencies()?;
        let mut s = String::new();
        s.push_str("# reflow2 dependency declarations — which version of another design this\n");
        s.push_str("# design depends on. GENERATED: re-derive it from the build files rather\n");
        s.push_str("# than editing by hand, because a hand-kept pin drifts from the build and\n");
        s.push_str("# the build is what ships.\n\n");
        s.push_str("[reflow2]\n");
        s.push_str(&format!("version = \"{}\"\n", env!("CARGO_PKG_VERSION")));
        s.push_str(&format!("graph_id = \"{}\"\n", self.graph_id()));
        if declared.is_empty() {
            s.push_str(
                "\n# NOTHING DECLARED. This is \"nobody has said\", not \"depends on nothing\".\n",
            );
            return Ok(s);
        }
        for d in &declared {
            s.push_str(&format!("\n[dependencies.{}]\n", d.name));
            s.push_str(&format!("source = \"{}\"\n", d.source));
            s.push_str(&format!("version = \"{}\"\n", d.version));
            if !d.components.is_empty() {
                s.push_str(&format!("components = {:?}\n", d.components));
            }
            if !d.features.is_empty() {
                s.push_str(&format!("features = {:?}\n", d.features));
            }
            if let Some(x) = &d.declared_in {
                s.push_str(&format!("declared_in = \"{x}\"\n"));
            }
            // Emitted only when stated. A reader must be able to tell "this
            // dependency has no reflow2 design" from "nobody recorded whether it
            // does", and an always-present empty field would collapse the two.
            if let Some(g) = &d.graph_id {
                s.push_str(&format!("graph_id = \"{g}\"\n"));
            }
            // The watch pointer and the baseline taken against it. Emitted
            // together and only when stated: a path with no hash is a target
            // nobody has looked at yet, and the manifest must be able to say so
            // rather than implying a check that never ran.
            if let Some(x) = &d.design_export {
                s.push_str(&format!("design_export = \"{x}\"\n"));
                // Only inside the path block: see the note on the write side.
                // A baseline with no target is a check that never happened
                // wearing the clothes of one that did.
                if let Some(h) = &d.design_export_hash {
                    s.push_str(&format!("design_export_hash = \"{h}\"\n"));
                }
                if let Some(a) = &d.design_export_seen_at {
                    s.push_str(&format!("design_export_seen_at = \"{a}\"\n"));
                }
            }
            if let Some(n) = &d.note {
                s.push_str(&format!("note = \"{}\"\n", n.replace('"', "'")));
            }
        }
        Ok(s)
    }
}

/// What a caller found at one declared upstream export path.
///
/// ⚠️ THE CALLER SUPPLIES THIS, exactly as `reconcile_dependencies` takes
/// `observed` and `reconcile_artifacts` takes hashes. `reflow2-core` does no
/// file I/O, deliberately and repeatedly, and reading another design's record
/// off disk is not the exception that changes that — it is the same split, one
/// boundary along.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservedUpstream {
    /// The declaration's own id, so a finding names the thing that was declared.
    pub id: String,
    /// Whether the caller could read a reflow2 export at the declared path:
    /// `read` | `missing` | `unreadable`.
    pub state: String,
    /// The export's COMPUTED content hash. Computed from content, never the
    /// hash the document states about itself — a record edited by anything
    /// other than `export_graph` keeps its old stamp, and trusting it is the
    /// defect `sync_debt` already had to fix once.
    #[serde(default)]
    pub content_hash: Option<String>,
    /// The `graph_id` the export at that path actually carries.
    #[serde(default)]
    pub graph_id: Option<String>,
    /// How many nodes it holds, for a reader who wants a sense of scale.
    #[serde(default)]
    pub nodes: Option<usize>,
}

/// One thing to say about a declared dependency's upstream design.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpstreamFinding {
    /// `moved` | `unchanged` | `never_seen` | `missing` | `unreadable`
    /// | `graph_id_mismatch` | `not_watched` | `not_observed`
    pub kind: &'static str,
    /// The declaration's id.
    pub dependency: String,
    /// Its human name.
    pub name: String,
    /// The path being watched, where one was declared.
    pub design_export: Option<String>,
    /// What a reader should do about it, in a sentence.
    pub detail: String,
}

impl UpstreamFinding {
    /// Whether this finding asks the reader to DO something.
    ///
    /// `unchanged` and `never_seen` do not: the first is the quiet ordinary
    /// case, and the second says nobody has looked yet, which is a statement
    /// about the record rather than about the upstream.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self.kind,
            "moved" | "missing" | "unreadable" | "graph_id_mismatch"
        )
    }
}

/// What the declared dependencies say about the designs upstream of them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpstreamReport {
    pub findings: Vec<UpstreamFinding>,
    /// How many declarations name another reflow2 design at all.
    pub designs_declared: usize,
    /// How many of those carry an export path to watch.
    pub watched: usize,
    /// Said plainly whichever way it comes out — "nothing watched" and
    /// "nothing has moved" must never look alike.
    pub note: String,
}

/// One declared upstream a caller should go and look at.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpstreamTarget {
    pub id: String,
    pub name: String,
    pub design_export: String,
    /// The declared design's id, where one was named.
    pub graph_id: Option<String>,
    /// The baseline this design last saw, absent when nobody has looked.
    pub design_export_hash: Option<String>,
}

impl DesignGraph {
    /// The upstream exports a caller should read, from the committed manifest.
    ///
    /// This is the "who are my children" list, and it is deliberately NOT a new
    /// vocabulary: the dependency manifest already holds it, version-pinned and
    /// reviewable in a diff, and it carries the direction a flat list of ids
    /// could not express.
    pub fn upstream_targets(&self) -> Result<Vec<UpstreamTarget>, DynoError> {
        let mut out = Vec::new();
        for d in self.declared_dependencies()? {
            let Some(path) = d.design_export.filter(|p| !p.trim().is_empty()) else {
                continue;
            };
            out.push(UpstreamTarget {
                id: d.id,
                name: d.name,
                design_export: path,
                graph_id: d.graph_id,
                design_export_hash: d.design_export_hash,
            });
        }
        Ok(out)
    }

    /// Has the design this one depends on moved since the declaration was made?
    ///
    /// The second check `req:design-dependencies-declared` names in its own
    /// statement — *"declared-versus-upstream answers has what I depend on moved
    /// since"* — and the half that was never built. `reconcile_dependencies`
    /// answers the other one, against the BUILD.
    ///
    /// 🛑 IT NEVER UPDATES THE BASELINE. The recorded hash is what the declarer
    /// last looked at; refreshing it here would make this report `moved` exactly
    /// once and then go quiet forever. Re-declaring is the acknowledgement.
    ///
    /// ⚠️ SILENCE IS REPORTED, NOT ASSUMED. A dependency naming another design
    /// with no export path comes back as `not_watched`, and a watched target the
    /// caller did not look at comes back as `not_observed` — because "nothing
    /// has moved" and "nothing was checked" must never share an answer.
    pub fn reconcile_upstream(
        &self,
        observed: &[ObservedUpstream],
    ) -> Result<UpstreamReport, DynoError> {
        let declared = self.declared_dependencies()?;
        let by_id: BTreeMap<&str, &ObservedUpstream> =
            observed.iter().map(|o| (o.id.as_str(), o)).collect();

        let mut findings = Vec::new();
        let mut designs_declared = 0usize;
        let mut watched = 0usize;

        for d in &declared {
            let names_a_design = d.graph_id.as_deref().is_some_and(|g| !g.trim().is_empty());
            let path = d.design_export.as_deref().filter(|p| !p.trim().is_empty());
            if names_a_design {
                designs_declared += 1;
            }
            let Some(path) = path else {
                // A dependency with no design and no export is an ordinary code
                // dependency and says nothing here. One that NAMES a design and
                // gives nothing to watch is the silence worth reporting.
                if names_a_design {
                    findings.push(UpstreamFinding {
                        kind: "not_watched",
                        dependency: d.id.clone(),
                        name: d.name.clone(),
                        design_export: None,
                        detail: format!(
                            "'{}' names the reflow2 design '{}' but no export to watch, so nothing \
                             here can tell you when it moves. Declare `design_export` pointing at \
                             that design's committed export.",
                            d.name,
                            d.graph_id.as_deref().unwrap_or("")
                        ),
                    });
                }
                continue;
            };
            watched += 1;

            let Some(o) = by_id.get(d.id.as_str()) else {
                findings.push(UpstreamFinding {
                    kind: "not_observed",
                    dependency: d.id.clone(),
                    name: d.name.clone(),
                    design_export: Some(path.to_string()),
                    detail: format!(
                        "Nobody looked at {path} on this pass, so this says nothing about whether \
                         '{}' has moved.",
                        d.name
                    ),
                });
                continue;
            };

            match o.state.as_str() {
                "missing" => findings.push(UpstreamFinding {
                    kind: "missing",
                    dependency: d.id.clone(),
                    name: d.name.clone(),
                    design_export: Some(path.to_string()),
                    detail: format!(
                        "'{}' is declared to be watched at {path} and there is no file there. \
                         Either the upstream moved its record or the pointer is wrong.",
                        d.name
                    ),
                }),
                "unreadable" => findings.push(UpstreamFinding {
                    kind: "unreadable",
                    dependency: d.id.clone(),
                    name: d.name.clone(),
                    design_export: Some(path.to_string()),
                    detail: format!(
                        "What is at {path} is not a readable reflow2 export, so '{}' cannot be \
                         watched from there.",
                        d.name
                    ),
                }),
                _ => {
                    // A path that points at a DIFFERENT design is worth more than
                    // a moved hash: the two designs would otherwise be compared
                    // forever and always disagree. Checked before movement, and
                    // only when both sides said which design they mean.
                    if let (Some(want), Some(got)) = (d.graph_id.as_deref(), o.graph_id.as_deref())
                        && !want.trim().is_empty()
                        && want != got
                    {
                        findings.push(UpstreamFinding {
                            kind: "graph_id_mismatch",
                            dependency: d.id.clone(),
                            name: d.name.clone(),
                            design_export: Some(path.to_string()),
                            detail: format!(
                                "'{}' is declared against design '{want}' but the export at {path} \
                                 belongs to '{got}'. Watching it would compare two different \
                                 designs and always disagree.",
                                d.name
                            ),
                        });
                        continue;
                    }
                    let Some(baseline) = d
                        .design_export_hash
                        .as_deref()
                        .filter(|h| !h.trim().is_empty())
                    else {
                        findings.push(UpstreamFinding {
                            kind: "never_seen",
                            dependency: d.id.clone(),
                            name: d.name.clone(),
                            design_export: Some(path.to_string()),
                            detail: format!(
                                "{path} is readable but this design has never recorded what it \
                                 looked like, so movement cannot be computed. Re-declare '{}' to \
                                 take the baseline.",
                                d.name
                            ),
                        });
                        continue;
                    };
                    let found = o.content_hash.as_deref().unwrap_or_default();
                    if found == baseline {
                        findings.push(UpstreamFinding {
                            kind: "unchanged",
                            dependency: d.id.clone(),
                            name: d.name.clone(),
                            design_export: Some(path.to_string()),
                            detail: format!(
                                "'{}' is exactly as this design last saw it{}.",
                                d.name,
                                d.design_export_seen_at
                                    .as_deref()
                                    .map(|a| format!(", on {a}"))
                                    .unwrap_or_else(|| ", on a date nobody recorded".into())
                            ),
                        });
                    } else {
                        findings.push(UpstreamFinding {
                            kind: "moved",
                            dependency: d.id.clone(),
                            name: d.name.clone(),
                            design_export: Some(path.to_string()),
                            detail: format!(
                                "'{}' HAS MOVED since this design last looked{}. It is pinned at \
                                 version '{}'. Read what changed before assuming that pin still \
                                 describes it, then re-declare to take a new baseline — nothing \
                                 here updates it for you.",
                                d.name,
                                d.design_export_seen_at
                                    .as_deref()
                                    .map(|a| format!(" on {a}"))
                                    .unwrap_or_default(),
                                d.version
                            ),
                        });
                    }
                }
            }
        }

        let moved = findings.iter().filter(|f| f.kind == "moved").count();
        let note = if declared.is_empty() {
            "Nothing is declared, so nothing can be watched. This is \"nobody has said\", never \
             \"depends on nothing\"."
                .to_string()
        } else if watched == 0 {
            format!(
                "{designs_declared} dependency(ies) name another reflow2 design and NONE carries \
                 an export to watch, so this report is silent for want of a target rather than \
                 because nothing moved."
            )
        } else if moved == 0 {
            format!(
                "{watched} upstream design(s) watched, none moved since this design last looked. \
                 Findings that say nobody looked, or that no baseline exists, are listed rather \
                 than counted as agreement."
            )
        } else {
            format!("{moved} of {watched} watched upstream design(s) have moved.")
        };

        Ok(UpstreamReport {
            findings,
            designs_declared,
            watched,
            note,
        })
    }
}
