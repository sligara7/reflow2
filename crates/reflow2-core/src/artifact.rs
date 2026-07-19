//! Artifact linking — connect real deliverables back to the design (SP-6).
//!
//! The loop's closing move: the agent builds a real file (a Unity C# script, an
//! OpenAPI spec, a doc) and registers it as an [`Artifact`](crate::nodes::node::ARTIFACT)
//! that **`REALIZES`** the Capability/Component it implements, with provenance. That
//! keeps as-designed vs as-built honest and makes DETECT's `unrealized_capability`
//! gap productive (a Capability with no incoming `REALIZES` is a build gap).
//!
//! The read side already exists (DETECT/HEAL/PROPAGATE/report all expect
//! `Artifact → Capability` REALIZES); this module is the missing **write side**.
//!
//! ## Provenance
//!
//! An `Artifact` carries no `provenance` property — provenance lives on a
//! [`Fragment`](crate::nodes::node::FRAGMENT). So [`DesignGraph::link_artifact`]
//! records provenance the same way INGEST does: it creates a provenance Fragment
//! that `YIELDED` (action `created`) the Artifact. Bare [`add_artifact`] /
//! [`realizes`](DesignGraph::realizes) skip the Fragment when provenance isn't needed.
//!
//! Scope: the write side of the link, plus the drift **baseline** — an optional
//! `checksum` recorded at link time. Comparing that baseline against observed
//! reality is [`crate::drift`] (SP-6b).

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};
use crate::temporal::{ChangeAction, ChangeType};

/// Which way an accepted drift went — the answer to the **second question**.
///
/// The code moved; that much is observed. Accepting the new baseline is a
/// decision about what the movement *meant*, and the erosion trial showed why
/// it cannot be silent: five legitimate fix cycles, each accepted with no
/// question asked, left a design describing a system that no longer existed —
/// while reporting zero gaps. The third option, "accept the file, leave the
/// design alone, say nothing", is the one that erodes, so it does not exist:
/// every accept states which of these it is, and the claim goes on axis Z.
#[derive(Debug, Clone)]
pub enum DriftDisposition<'a> {
    /// The change carries no design meaning — a refactor, a cosmetic fix, a
    /// bug fix restoring intended behaviour. This is itself a recorded claim:
    /// a `ChangeEvent` is written saying the design was judged to still hold
    /// against this checksum. The claim can be wrong, but it cannot be silent,
    /// and it is dated — which is exactly what a later freshness check reads.
    DesignHolds {
        /// Why the code moved (usually [`ChangeType::TestFailureFix`] or
        /// [`ChangeType::Refactor`]).
        change_type: ChangeType,
    },
    /// The behaviour moved and the design moved with it. References the
    /// `ChangeEvent` recorded (via
    /// [`record_change`](DesignGraph::record_change)) when the design was
    /// updated — the same event is linked to the artifact, so the code accept
    /// and the design edit are one change on axis Z, not two coincidences.
    DesignUpdated {
        /// The existing `ChangeEvent` from the design-side update. Verified to
        /// exist; a dangling reference is refused rather than recorded.
        change_event_id: &'a str,
    },
}

/// Inputs for [`DesignGraph::link_artifact`] — register a real file against the
/// design with provenance. Serializable: it crosses the MCP boundary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinkArtifactOptions {
    /// Stable Artifact id (e.g. `art:ball-physics`).
    pub artifact_id: String,
    /// Artifact name.
    pub name: String,
    /// Where the artifact lives (path / URI / content-hash). Points outside the graph.
    #[serde(default)]
    pub location: Option<String>,
    /// `code` (default) / `spec` / `document` / `diagram` / `model` / …
    #[serde(default)]
    pub artifact_type: Option<String>,
    /// Node type the artifact realizes (e.g. `Capability`, `Component`).
    pub target_type: String,
    /// Node id the artifact realizes.
    pub target_id: String,
    /// REALIZES completeness: `stub` / `partial` / `complete`.
    #[serde(default)]
    pub completeness: Option<String>,
    /// Provenance stamped on the Fragment (default `authored`).
    #[serde(default)]
    pub provenance: Option<String>,
    /// Provenance Fragment id (default `frag:<artifact_id>`).
    #[serde(default)]
    pub fragment_id: Option<String>,
    /// Content hash of the file as registered — the baseline
    /// [`reconcile_artifacts`](DesignGraph::reconcile_artifacts) compares
    /// against later. Opaque to reflow2; the caller picks the algorithm. Without
    /// it the artifact can still be checked for existence, but a content change
    /// is reported as `no_baseline` rather than passing silently.
    #[serde(default)]
    pub checksum: Option<String>,
}

/// What [`DesignGraph::link_artifact`] created.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArtifactLink {
    /// The Artifact node id.
    pub artifact_id: String,
    /// The realized target node id.
    pub target_id: String,
    /// The provenance Fragment id that `YIELDED` the Artifact.
    pub fragment_id: String,
    /// The REALIZES completeness recorded (as stored).
    pub completeness: String,
    /// The provenance recorded on the Fragment (as stored).
    pub provenance: String,
}

impl DesignGraph {
    /// Create an `Artifact` node — a deliverable that lives outside the graph.
    /// `name` is required; `artifact_type` (default `code`) and `location` are
    /// optional (omitted rather than blank when absent).
    pub fn add_artifact(
        &mut self,
        id: &str,
        name: &str,
        artifact_type: Option<&str>,
        location: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::ARTIFACT,
            id,
            Props::new()
                .set("name", name)
                .set_opt("artifact_type", artifact_type)
                .set_opt("location", location),
        )
    }

    /// Accept a drifted artifact's new content hash as the baseline — a
    /// **two-sided decision**, never a silent update (BL-33).
    ///
    /// Updating the baseline answers "which state do we compare against next
    /// time?". The [`DriftDisposition`] answers the question that used to go
    /// unasked: *the code moved — did the design move too, or did it not need
    /// to?* Both answers leave a `ChangeEvent` `CHANGED`-linked to the
    /// artifact, so the accept is on axis Z either way:
    ///
    /// - [`DriftDisposition::DesignHolds`] writes a new event recording the
    ///   claim that this change carried no design meaning, at a deterministic
    ///   id (`chg:accept-…` hashed from artifact + checksum) so re-accepting
    ///   the same state is idempotent.
    /// - [`DriftDisposition::DesignUpdated`] links the **existing** event from
    ///   the design-side [`record_change`](DesignGraph::record_change) to the
    ///   artifact — one change, both sides. A `change_event_id` that does not
    ///   exist is refused loudly: a phantom reference would let the claim
    ///   "the design was updated" stand with nothing behind it.
    ///
    /// Returns the updated artifact and the id of the change event the accept
    /// now hangs off.
    pub fn set_artifact_checksum(
        &mut self,
        artifact_id: &str,
        checksum: &str,
        disposition: DriftDisposition<'_>,
        note: Option<&str>,
        at: Option<&str>,
    ) -> Result<(StoredNode, String), DynoError> {
        let Some(existing) = self.get_node(node::ARTIFACT, artifact_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::ARTIFACT.to_string(),
                node_id: artifact_id.to_string(),
            });
        };

        let event_id = match disposition {
            DriftDisposition::DesignHolds { change_type } => {
                let event_id = format!(
                    "chg:accept-{:016x}",
                    crate::detect::fnv1a(&format!("{artifact_id}|{checksum}"))
                );
                if self.get_node(node::CHANGE_EVENT, &event_id)?.is_none() {
                    self.add_change_event(
                        &event_id,
                        note.unwrap_or(
                            "Accepted a new baseline: the change carries no design meaning",
                        ),
                        change_type,
                    )?;
                    if let Some(at) = at {
                        // The claim is worth more dated. Read-modify-write so the
                        // event keeps its name and type.
                        let ev = self
                            .get_node(node::CHANGE_EVENT, &event_id)?
                            .expect("just created");
                        let mut props = Props::new().set("detected_at", at);
                        for (k, v) in &ev.properties {
                            if k != "detected_at" {
                                props = props.set(k, v.clone());
                            }
                        }
                        self.create_node(node::CHANGE_EVENT, &event_id, props)?;
                    }
                    self.accept_changed_edge(&event_id, artifact_id)?;
                }
                event_id
            }
            DriftDisposition::DesignUpdated { change_event_id } => {
                if self
                    .get_node(node::CHANGE_EVENT, change_event_id)?
                    .is_none()
                {
                    return Err(DynoError::NodeNotFound {
                        node_type: node::CHANGE_EVENT.to_string(),
                        node_id: change_event_id.to_string(),
                    });
                }
                self.accept_changed_edge(change_event_id, artifact_id)?;
                change_event_id.to_string()
            }
        };

        // The accept answers the open drift on this artifact, so the schema's
        // own lifecycle flag says so: `DriftEvent.resolved` was declared with
        // `default: false` and, until BL-35, nothing ever wrote it — recorded
        // divergences stayed "open" forever no matter what happened next.
        for e in self.incoming(artifact_id, Some(edge::DEPENDS_ON))? {
            let Some(ev) = self.get_node(node::DRIFT_EVENT, &e.from_id)? else {
                continue; // DEPENDS_ON from something that isn't a DriftEvent
            };
            let already = ev
                .properties
                .get("resolved")
                .and_then(dynograph_core::Value::as_bool)
                .unwrap_or(false);
            if already {
                continue;
            }
            let mut props = Props::new().set("resolved", true);
            for (k, v) in &ev.properties {
                if k != "resolved" {
                    props = props.set(k, v.clone());
                }
            }
            self.create_node(node::DRIFT_EVENT, &ev.node_id, props)?;
        }

        let mut props = Props::new().set("checksum", checksum);
        for (k, v) in &existing.properties {
            if k != "checksum" {
                props = props.set(k, v.clone());
            }
        }
        let artifact = self.create_node(node::ARTIFACT, artifact_id, props)?;
        Ok((artifact, event_id))
    }

    /// The `CHANGED` edge an accept writes: marked `accepted_baseline: true`,
    /// which is how the confirmation ledger tells a disposition claim from
    /// ordinary change history on the same artifact.
    fn accept_changed_edge(&mut self, event_id: &str, artifact_id: &str) -> Result<(), DynoError> {
        self.create_edge(
            edge::CHANGED,
            node::CHANGE_EVENT,
            event_id,
            node::ARTIFACT,
            artifact_id,
            Props::new()
                .set("action", ChangeAction::Modified.as_str())
                .set("accepted_baseline", true),
        )?;
        Ok(())
    }

    /// Link an `Artifact` to the entity it implements via `REALIZES`. `target_type`
    /// is required because `REALIZES` accepts any target type (`to: "*"`).
    pub fn realizes(
        &mut self,
        artifact_id: &str,
        target_type: &str,
        target_id: &str,
        completeness: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::REALIZES,
            node::ARTIFACT,
            artifact_id,
            target_type,
            target_id,
            Props::new().set_opt("completeness", completeness),
        )
    }

    /// Register a real file against the design **with provenance**, atomically:
    /// create the Artifact + a provenance Fragment that `YIELDED` it + the
    /// `REALIZES` edge to its target.
    ///
    /// Fails loud if the target node does not exist — no dangling REALIZES edge.
    pub fn link_artifact(&mut self, opts: LinkArtifactOptions) -> Result<ArtifactLink, DynoError> {
        // The target must exist — never author an edge into thin air.
        if self.get_node(&opts.target_type, &opts.target_id)?.is_none() {
            return Err(DynoError::NodeNotFound {
                node_type: opts.target_type.clone(),
                node_id: opts.target_id.clone(),
            });
        }

        let provenance = opts.provenance.as_deref().unwrap_or("authored");
        let completeness = opts.completeness.as_deref().unwrap_or("complete");
        let fragment_id = opts
            .fragment_id
            .clone()
            .unwrap_or_else(|| format!("frag:{}", opts.artifact_id));

        // All four writes land together or not at all — a failed one (e.g. a bad
        // enum value) leaves no half-linked Artifact behind.
        self.begin_batch();
        match self.write_artifact_link(&opts, &fragment_id, provenance, completeness) {
            Ok(()) => {
                self.commit_batch()?;
                Ok(ArtifactLink {
                    artifact_id: opts.artifact_id,
                    target_id: opts.target_id,
                    fragment_id,
                    completeness: completeness.to_string(),
                    provenance: provenance.to_string(),
                })
            }
            Err(e) => {
                self.discard_batch();
                Err(e)
            }
        }
    }

    /// The mutation half of [`link_artifact`](Self::link_artifact): Artifact +
    /// provenance Fragment + `YIELDED` + `REALIZES`. Run inside a batch so it's atomic.
    fn write_artifact_link(
        &mut self,
        opts: &LinkArtifactOptions,
        fragment_id: &str,
        provenance: &str,
        completeness: &str,
    ) -> Result<(), DynoError> {
        // Provenance Fragment (invalid provenance fails loud via schema validation).
        self.create_node(
            node::FRAGMENT,
            fragment_id,
            Props::new()
                .set("title", format!("Registered {}", opts.name))
                .set("fragment_type", "implementation")
                .set("provenance", provenance),
        )?;
        // The Artifact itself.
        self.create_node(
            node::ARTIFACT,
            &opts.artifact_id,
            Props::new()
                .set("name", opts.name.as_str())
                .set_opt("artifact_type", opts.artifact_type.as_deref())
                .set_opt("location", opts.location.as_deref())
                .set_opt("checksum", opts.checksum.as_deref()),
        )?;
        // Fragment YIELDED the Artifact (the provenance anchor).
        self.create_edge(
            edge::YIELDED,
            node::FRAGMENT,
            fragment_id,
            node::ARTIFACT,
            &opts.artifact_id,
            Props::new().set("action", "created"),
        )?;
        // Artifact REALIZES its target.
        self.realizes(
            &opts.artifact_id,
            &opts.target_type,
            &opts.target_id,
            Some(completeness),
        )?;
        Ok(())
    }
}
