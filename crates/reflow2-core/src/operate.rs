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
