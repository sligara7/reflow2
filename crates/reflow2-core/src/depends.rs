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

use dynograph_core::{DynoError, Value};

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
    /// Said plainly whichever way it comes out — "nothing declared" and
    /// "nothing to declare" must never look alike.
    pub note: String,
}

impl DesignGraph {
    /// Declare a dependency on another design (`req:design-dependencies-declared`).
    pub fn declare_dependency(&mut self, decl: &DependencyDeclaration) -> Result<(), DynoError> {
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
        for d in &declared {
            if !observed_names.contains(d.name.as_str()) {
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
            if let Some(n) = &d.note {
                s.push_str(&format!("note = \"{}\"\n", n.replace('"', "'")));
            }
        }
        Ok(s)
    }
}
