//! P5 · Operation — the write side of the operate domain (WS-2).
//!
//! `operate.yaml` has been fully defined since the schema was written:
//! [`Release`](crate::nodes::node::RELEASE),
//! [`Environment`](crate::nodes::node::ENVIRONMENT),
//! [`Resource`](crate::nodes::node::RESOURCE), joined by `DEPLOYED_TO` and
//! `REQUIRES_RESOURCE`. The *read* side was wired too — `propagate.rs`
//! classifies both edges, `report.rs` lists all three types, and `detect.rs`
//! counts them to decide whether to raise the `no_deploy_operate` gap.
//!
//! What was missing was any way to **create** them. DETECT would tell a user
//! "you have a design and a build but nothing about deploying or operating it"
//! and offer no typed way to answer. This module closes that: it is the
//! `operate` counterpart to [`crate::artifact`]'s P3 write side.
//!
//! Deploying is where the design meets the world, so the two edges carry the
//! qualifiers that matter downstream: `DEPLOYED_TO.status` distinguishes a
//! planned rollout from an active one from a rollback, and
//! `REQUIRES_RESOURCE.criticality` distinguishes a nice-to-have from a hard
//! dependency. Both are optional and omitted rather than defaulted when absent.

use dynograph_core::DynoError;
use dynograph_storage::{StoredEdge, StoredNode};

use crate::graph::DesignGraph;
use crate::nodes::{Props, edge, node};

impl DesignGraph {
    /// P5 · Operation — a packaged, operable version of some Components or
    /// Artifacts. `name` is required; `version`, `unit_type` (default
    /// `container`) and `status` (default `planned`) are optional.
    pub fn add_release(
        &mut self,
        id: &str,
        name: &str,
        version: Option<&str>,
        unit_type: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::RELEASE,
            id,
            Props::new()
                .set("name", name)
                .set_opt("version", version)
                .set_opt("unit_type", unit_type),
        )
    }

    /// P5 · Operation — where a Release runs. `name` is required; `env_type`
    /// (default `production`) and `location` are optional.
    ///
    /// More than a deploy target: an Environment is what an `EnvironmentRule`
    /// would be imposed by, so swapping it swaps the constraint space. That
    /// compliance layer is still deferred (GS-9 / WS-4).
    pub fn add_environment(
        &mut self,
        id: &str,
        name: &str,
        env_type: Option<&str>,
        location: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::ENVIRONMENT,
            id,
            Props::new()
                .set("name", name)
                .set_opt("env_type", env_type)
                .set_opt("location", location),
        )
    }

    /// P5 · Operation — a real-world thing the built system needs: a database,
    /// a queue, a secret, a GPU, power, bandwidth. `name` is required.
    pub fn add_resource(
        &mut self,
        id: &str,
        name: &str,
        provider: Option<&str>,
    ) -> Result<StoredNode, DynoError> {
        self.create_node(
            node::RESOURCE,
            id,
            Props::new().set("name", name).set_opt("provider", provider),
        )
    }

    /// `Release DEPLOYED_TO Environment`. `status` ∈ `planned` / `active` /
    /// `rolled_back` — the declared-versus-actual axis a deployment reconcile
    /// compares against.
    pub fn deploy_to(
        &mut self,
        release_id: &str,
        environment_id: &str,
        status: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::DEPLOYED_TO,
            node::RELEASE,
            release_id,
            node::ENVIRONMENT,
            environment_id,
            Props::new().set_opt("status", status),
        )
    }

    /// `Component|Release REQUIRES_RESOURCE Resource`. `from_type` is required
    /// because the schema allows any source (`from: "*"`). `criticality` ∈
    /// `optional` / `recommended` / `required`.
    pub fn require_resource(
        &mut self,
        from_type: &str,
        from_id: &str,
        resource_id: &str,
        criticality: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        self.create_edge(
            edge::REQUIRES_RESOURCE,
            from_type,
            from_id,
            node::RESOURCE,
            resource_id,
            Props::new().set_opt("criticality", criticality),
        )
    }
}

/// What a release actually shipped, and how that compares to the design —
/// the **as-released view** (BL-34).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReleaseReport {
    pub release_id: String,
    pub release_name: String,
    /// `(artifact_id, as_checksum)` — the checksum frozen at cut time when one
    /// was recorded, so later baseline moves do not rewrite what shipped.
    pub artifacts: Vec<(String, Option<String>)>,
    pub components: Vec<String>,
    /// Capabilities the shipped artifacts realize (both P3 shapes, per BL-38).
    pub capabilities_covered: Vec<String>,
    /// Capabilities that are built (have realizing artifacts) but none of
    /// whose artifacts are in this release — designed and built, not shipped.
    pub built_capabilities_not_covered: Vec<String>,
    /// Environments this release is deployed to, with the deployment status.
    pub deployed_to: Vec<(String, Option<String>)>,
}

impl DesignGraph {
    /// Record that a Release ships an Artifact or Component (`INCLUDES`).
    ///
    /// `as_checksum` freezes the artifact's content hash *as shipped in this
    /// release*: the artifact node's own `checksum` is the live drift baseline
    /// and moves with every accept, so without the frozen copy a past release's
    /// manifest would quietly rewrite itself — the axis-Z sin again.
    pub fn release_includes(
        &mut self,
        release_id: &str,
        target_type: &str,
        target_id: &str,
        as_checksum: Option<&str>,
    ) -> Result<StoredEdge, DynoError> {
        if target_type != node::ARTIFACT && target_type != node::COMPONENT {
            return Err(DynoError::InvalidEdge {
                edge_type: edge::INCLUDES.to_string(),
                from_type: node::RELEASE.to_string(),
                to_type: target_type.to_string(),
            });
        }
        self.create_edge(
            edge::INCLUDES,
            node::RELEASE,
            release_id,
            target_type,
            target_id,
            Props::new().set_opt("as_checksum", as_checksum),
        )
    }

    /// The as-released view: what this release shipped, which capabilities that
    /// covers, and which built capabilities it left out. "Does what we released
    /// match what we designed?" was inexpressible before the `INCLUDES` edge
    /// existed; this is the read side that makes it a query.
    pub fn release_report(&self, release_id: &str) -> Result<ReleaseReport, DynoError> {
        let Some(rel) = self.get_node(node::RELEASE, release_id)? else {
            return Err(DynoError::NodeNotFound {
                node_type: node::RELEASE.to_string(),
                node_id: release_id.to_string(),
            });
        };

        let mut artifacts = Vec::new();
        let mut components = Vec::new();
        for e in self.outgoing(release_id, Some(edge::INCLUDES))? {
            if self.get_node(node::ARTIFACT, &e.to_id)?.is_some() {
                let frozen = e
                    .properties
                    .get("as_checksum")
                    .and_then(dynograph_core::Value::as_str)
                    .map(str::to_string);
                artifacts.push((e.to_id, frozen));
            } else {
                components.push(e.to_id);
            }
        }
        artifacts.sort();
        components.sort();

        // Which capabilities do the shipped artifacts realize? Both P3 shapes:
        // an artifact realizing the capability, or realizing a component the
        // capability is allocated to (BL-38).
        let shipped: std::collections::BTreeSet<&str> =
            artifacts.iter().map(|(a, _)| a.as_str()).collect();
        let mut covered = std::collections::BTreeSet::new();
        let mut built_not_covered = Vec::new();
        for cap in self.scan_nodes(node::CAPABILITY)? {
            let mut realizing: Vec<String> = self
                .incoming(&cap.node_id, Some(edge::REALIZES))?
                .into_iter()
                .map(|e| e.from_id)
                .collect();
            for alloc in self.outgoing(&cap.node_id, Some(edge::ALLOCATED_TO))? {
                for e in self.incoming(&alloc.to_id, Some(edge::REALIZES))? {
                    realizing.push(e.from_id);
                }
            }
            if realizing.is_empty() {
                continue; // not built — unrealized_capability's question
            }
            if realizing.iter().any(|a| shipped.contains(a.as_str())) {
                covered.insert(cap.node_id.clone());
            } else {
                built_not_covered.push(cap.node_id.clone());
            }
        }
        built_not_covered.sort();

        let mut deployed_to = Vec::new();
        for e in self.outgoing(release_id, Some(edge::DEPLOYED_TO))? {
            let status = e
                .properties
                .get("status")
                .and_then(dynograph_core::Value::as_str)
                .map(str::to_string);
            deployed_to.push((e.to_id, status));
        }
        deployed_to.sort();

        Ok(ReleaseReport {
            release_id: release_id.to_string(),
            release_name: rel
                .properties
                .get("name")
                .and_then(dynograph_core::Value::as_str)
                .unwrap_or(release_id)
                .to_string(),
            artifacts,
            components,
            capabilities_covered: covered.into_iter().collect(),
            built_capabilities_not_covered: built_not_covered,
            deployed_to,
        })
    }
}
