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
//! Scope: link-only. As-built drift detection (filesystem reconcile, `DriftEvent`)
//! is deferred to SP-6b.

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

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
        self.add_artifact(
            &opts.artifact_id,
            &opts.name,
            opts.artifact_type.as_deref(),
            opts.location.as_deref(),
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
