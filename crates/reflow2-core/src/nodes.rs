//! Vocabulary constants and a small property builder.
//!
//! Node/edge *type names* are the schema's own strings (rule 3 in AGENTS.md:
//! terminology matches the schema). Naming them as constants here keeps the
//! typed helpers in [`crate::graph`] from sprinkling stringly-typed literals,
//! and gives one place to catch a rename against the schema.

/// Node type names, matching `schema/*.yaml`.
pub mod node {
    // P0 · Intent (core.yaml)
    pub const PROJECT: &str = "Project";
    pub const REQUIREMENT: &str = "Requirement";
    pub const CONSTRAINT: &str = "Constraint";
    pub const DESIGN_RULE: &str = "DesignRule";
    /// A question already put to the user about a gap (and whether answered).
    pub const QUESTION: &str = "Question";
    /// Who authors/decides the design itself — a person or automated agent.
    /// Distinct from [`ACTOR`], who the designed system serves.
    pub const CONTRIBUTOR: &str = "Contributor";
    // P1 · Function (functional.yaml)
    pub const CAPABILITY: &str = "Capability";
    pub const FLOW: &str = "Flow";
    pub const ACTOR: &str = "Actor";
    // P2 · Structure (structure.yaml)
    pub const COMPONENT: &str = "Component";
    pub const INTERFACE: &str = "Interface";
    pub const DECISION: &str = "Decision";
    // P3 · Realization (build.yaml)
    pub const ARTIFACT: &str = "Artifact";
    pub const FRAGMENT: &str = "Fragment";
    // P4 · Verification (verify.yaml)
    pub const VERIFICATION: &str = "Verification";
    pub const DRIFT_EVENT: &str = "DriftEvent";
    // P5 · Operation (operate.yaml)
    pub const RELEASE: &str = "Release";
    pub const ENVIRONMENT: &str = "Environment";
    pub const RESOURCE: &str = "Resource";
    // Cross-cutting · depth axis (dimensions.yaml)
    pub const DIMENSION_ASSESSMENT: &str = "DimensionAssessment";
    pub const DIMENSION_OBSERVATION: &str = "DimensionObservation";
    // Axis Z · change over time (temporal.yaml)
    pub const DESIGN_EPOCH: &str = "DesignEpoch";
    pub const TEMPORAL_FACT: &str = "TemporalFact";
    pub const SNAPSHOT: &str = "Snapshot";
    pub const CHANGE_EVENT: &str = "ChangeEvent";
}

/// Edge type names, matching `schema/*.yaml`.
pub mod edge {
    /// `Project → *` — the decomposition (axis-Y) containment spine.
    /// `Question → *` — the design nodes a question was raised about.
    pub const ASKS_ABOUT: &str = "ASKS_ABOUT";
    pub const CONTAINS: &str = "CONTAINS";
    /// `Requirement → Requirement` — child DECOMPOSES parent: a 1:1 split that
    /// adds no new information. Directed child→parent so a leaf finds its
    /// ancestry without the parent enumerating children, and because delivery
    /// rolls UP it: satisfying every child satisfies the parent.
    pub const DECOMPOSES: &str = "DECOMPOSES";
    /// `Contributor → *` — someone has a region of the design in hand.
    /// Advisory, never a lock, and deliberately NOT a traceability edge (absent
    /// from `structural_rule`, like `AUTHORED_BY`): who is working on something
    /// is coordination, not design structure, and propagating along it would
    /// drag people into blast radii.
    pub const CLAIMS: &str = "CLAIMS";
    /// `* → *` — traceability: a Capability SATISFIES a Requirement.
    pub const SATISFIES: &str = "SATISFIES";
    /// `* → Contributor` — the structured "who" behind a node's authorship.
    /// Deliberately NOT a traceability edge (absent from `structural_rule`), so
    /// authorship never propagates a blast radius.
    pub const AUTHORED_BY: &str = "AUTHORED_BY";
    /// `Constraint/DesignRule → *` — a limit binds a target; for a budget
    /// Constraint the edge carries the target's `contribution` (BL-11).
    pub const CONSTRAINS: &str = "CONSTRAINS";
    /// `Capability → Component` — the WHAT→WHERE allocation binding.
    pub const ALLOCATED_TO: &str = "ALLOCATED_TO";
    /// `* → Decision/DesignRule` — the node is shaped by a recorded decision.
    pub const GOVERNED_BY: &str = "GOVERNED_BY";
    /// `Component → Interface` — the component that exposes a contract.
    pub const PROVIDES: &str = "PROVIDES";
    /// `* → Interface` — a Component/Actor that depends on a contract. Paired
    /// with [`PROVIDES`]: an Interface consumed but never provided is a break
    /// between two parts of the design, which is what [`crate::detect`] looks for.
    pub const CONSUMES: &str = "CONSUMES";
    /// `Artifact → *` — an Artifact realizes a Capability/Component/Interface.
    pub const REALIZES: &str = "REALIZES";
    /// `Artifact → Interface/Capability/Component` — an Artifact defines the contract.
    pub const SPECIFIES: &str = "SPECIFIES";
    /// `Artifact → *` — an Artifact documents (explains) a node.
    pub const DOCUMENTS: &str = "DOCUMENTS";
    /// `Verification → Artifact` — a Verification emitted this Artifact (evidence).
    pub const PRODUCES: &str = "PRODUCES";
    /// `Fragment → *` — a note/review/pseudocode fragment annotates a node.
    pub const ANNOTATES: &str = "ANNOTATES";
    /// `Verification → *` — a Verification checks a Capability/Artifact/Component.
    pub const PERFORMED_IN: &str = "PERFORMED_IN";
    pub const VERIFIES: &str = "VERIFIES";
    /// `* → *` — a node depends on another (a lateral structural coupling).
    pub const DEPENDS_ON: &str = "DEPENDS_ON";
    /// `Capability → Flow` — a capability is a step of an ordered process
    /// (`step_order` carries its position).
    pub const PART_OF_FLOW: &str = "PART_OF_FLOW";
    /// `Fragment → *` — the fragment that produced/updated a node (provenance).
    pub const YIELDED: &str = "YIELDED";

    /// `DesignEpoch → DesignEpoch` — one epoch comes before another (ordering).
    pub const PRECEDES: &str = "PRECEDES";
    /// `DesignEpoch → DesignEpoch` — one epoch nests inside a larger one.
    pub const CONTAINS_EPOCH: &str = "CONTAINS_EPOCH";
    /// `Release → Environment` — a packaged version runs in an environment.
    pub const DEPLOYED_TO: &str = "DEPLOYED_TO";
    pub const INCLUDES: &str = "INCLUDES";
    /// `* → Resource` — a Component or Release consumes a real-world resource.
    pub const REQUIRES_RESOURCE: &str = "REQUIRES_RESOURCE";

    // Axis Z · change over time (temporal.yaml)
    /// `* → DesignEpoch` — a Snapshot or ChangeEvent is pinned to its epoch.
    pub const AT_EPOCH: &str = "AT_EPOCH";
    /// `Requirement|Capability → DesignEpoch|Release` — the satisfaction
    /// schedule: this item is DUE there. Distinct from [`AT_EPOCH`], which
    /// means *belongs to*.
    pub const SCHEDULED_FOR: &str = "SCHEDULED_FOR";
    /// `ChangeEvent → *` — the node a ChangeEvent added/modified/removed.
    pub const CHANGED: &str = "CHANGED";
    /// `* → Snapshot` — an entity has a captured state snapshot.
    pub const HAS_SNAPSHOT: &str = "HAS_SNAPSHOT";
    /// `* → DesignEpoch` — a Fragment/ChangeEvent/Verification happened during an epoch.
    pub const OCCURS_DURING: &str = "OCCURS_DURING";
    /// `* → TemporalFact` — an entity carries a time-bounded fact.
    pub const HAS_TEMPORAL_FACT: &str = "HAS_TEMPORAL_FACT";
    /// `TemporalFact → *` — the entity a temporal fact concerns.
    pub const ABOUT_ENTITY: &str = "ABOUT_ENTITY";
    /// `TemporalFact → DesignEpoch` — the fact becomes true at this epoch.
    pub const VALID_FROM: &str = "VALID_FROM";
    /// `TemporalFact → DesignEpoch` — the fact stops being true at this epoch.
    pub const VALID_TO: &str = "VALID_TO";

    // Inference "why" edges (inference.yaml) referenced by HEAL/PROPAGATE.
    /// `* → *` — two nodes are contradictory (a tension to resolve).
    pub const CONTRADICTS: &str = "CONTRADICTS";
    /// `* → *` — source supersedes target (target retired on the record — a
    /// decision-point's losing alternative, superseded by the winner: BL-70).
    pub const OBSOLETES: &str = "OBSOLETES";
    /// `* → *` — two nodes cover the same ground (candidates to merge).
    pub const DUPLICATES: &str = "DUPLICATES";
    /// `* → *` — a planned/anticipated need (may lack follow-through).
    pub const ANTICIPATES: &str = "ANTICIPATES";
    /// `* → *` — source initiates target; in a process model the `role`
    /// property says what the trigger *means* (feeds vs forces a resync).
    pub const TRIGGERS: &str = "TRIGGERS";

    // Depth axis (dimensions.yaml)
    /// `DimensionAssessment → *` — links an assessment to the node it scores.
    pub const ASSESSED_ON: &str = "ASSESSED_ON";
    /// `* → DimensionObservation` — an entity carries a per-fragment reading.
    pub const HAS_OBSERVATION: &str = "HAS_OBSERVATION";
    /// `DimensionObservation → Fragment` — the fragment a reading came from.
    pub const OBSERVED_IN: &str = "OBSERVED_IN";
}

use std::collections::HashMap;

use dynograph_core::Value;

/// Ergonomic builder for a node/edge property map.
///
/// ```
/// # use reflow2_core::nodes::Props;
/// let props = Props::new().set("name", "Auth").set("priority", "high").build();
/// assert_eq!(props.len(), 2);
/// ```
#[derive(Debug, Default, Clone)]
pub struct Props(HashMap<String, Value>);

impl Props {
    /// Start an empty property map.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Set a property. Chainable. Overwrites a prior value for the same key.
    #[must_use]
    pub fn set(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.to_string(), value.into());
        self
    }

    /// Set a property only when `value` is `Some` — omit it otherwise, so an
    /// absent optional never lands as an empty string (no silent placeholder).
    #[must_use]
    pub fn set_opt(self, key: &str, value: Option<impl Into<Value>>) -> Self {
        match value {
            Some(v) => self.set(key, v),
            None => self,
        }
    }

    /// Consume into the `HashMap` the storage engine expects.
    pub fn build(self) -> HashMap<String, Value> {
        self.0
    }
}

impl From<Props> for HashMap<String, Value> {
    fn from(p: Props) -> Self {
        p.0
    }
}

/// The semantic direction of an impact hop (docs/impact-propagation.md,
/// "Direction matters").
///
/// Lives here rather than in [`crate::propagate`] because the golden-thread
/// rule table below is shared vocabulary: PROPAGATE walks it, `structure`'s
/// topology analysis filters by it, and keeping it in the zero-dependency base
/// is what lets those two modules agree on "connected in the design" without
/// depending on each other (they used to form the crate's one real module
/// cycle — invisible to rustc because both sides are `impl DesignGraph`
/// blocks, and reported by reflow2's own self-model on 2026-07-20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDirection {
    /// Realization: "what did this node's existence justify or shape?"
    Downstream,
    /// Rationale: "what intent does this node serve, that may now be unmet?"
    Upstream,
    /// Peers/contracts: "what shares a contract or depends sideways?"
    Lateral,
    /// Inference: "what did this cause / enable / risk?"
    Causal,
}

impl ImpactDirection {
    /// A short, stable label.
    pub fn as_str(self) -> &'static str {
        match self {
            ImpactDirection::Downstream => "downstream",
            ImpactDirection::Upstream => "upstream",
            ImpactDirection::Lateral => "lateral",
            ImpactDirection::Causal => "causal",
        }
    }
}

/// How a structural edge propagates impact when walked **forward** (along an
/// outgoing edge) vs **backward** (along an incoming edge). `None` on a side
/// means impact does not propagate that way, so the traversal never crosses it
/// (and can therefore always explain why a node is in the blast radius).
pub(crate) struct EdgeRule {
    pub(crate) forward: Option<ImpactDirection>,
    pub(crate) backward: Option<ImpactDirection>,
}

/// The structural golden-thread direction table (docs/impact-propagation.md).
/// Inference edges are not here — they are classified as
/// [`Causal`](ImpactDirection::Causal) at runtime from
/// `schema.inference_edge_types()`. Structural edges not listed (e.g.
/// SPECIFIES, DOCUMENTS, temporal bookkeeping) are intentionally not traversed.
pub(crate) fn structural_rule(edge_type: &str) -> Option<EdgeRule> {
    use ImpactDirection::{Downstream, Lateral, Upstream};
    let (fwd, bwd) = match edge_type {
        // Note: CONTAINS (decomposition, axis Y) is deliberately *not* here.
        // It is not a traceability edge; propagating along it would make the
        // Project a hub that short-circuits every sibling to ~2 hops. The doc's
        // impact diagram omits it too.
        //
        // Traceability: Capability SATISFIES Requirement. From the requirement
        // (incoming) you reach the realizer that may now be wrong (downstream);
        // from the capability (outgoing) you reach the intent it serves (upstream).
        "SATISFIES" => (Some(Upstream), Some(Downstream)),
        // Decomposition within intent: child DECOMPOSES parent. Same shape as
        // SATISFIES, and for the same reason — from the parent (incoming) you
        // reach the children that carry it out, from a child (outgoing) you
        // reach the intent it serves. Unlike CONTAINS this IS a traceability
        // edge: it joins intent to intent rather than making the Project a hub,
        // so requirement families cluster on their own meaning.
        "DECOMPOSES" => (Some(Upstream), Some(Downstream)),
        // A node CONSTRAINS another it shapes.
        "CONSTRAINS" => (Some(Downstream), Some(Upstream)),
        // WHAT→WHERE: Capability ALLOCATED_TO Component.
        "ALLOCATED_TO" => (Some(Downstream), Some(Upstream)),
        // Realization: Artifact REALIZES Capability/Component/Interface.
        "REALIZES" => (Some(Upstream), Some(Downstream)),
        // Verification VERIFIES its target; a moved target staled it.
        "VERIFIES" => (Some(Upstream), Some(Downstream)),
        // Governance: source GOVERNED_BY a Decision/DesignRule.
        "GOVERNED_BY" => (Some(Upstream), Some(Downstream)),
        // Contracts / dependencies — sideways.
        "PROVIDES" | "CONSUMES" | "DEPENDS_ON" | "PART_OF_FLOW" => (Some(Lateral), Some(Lateral)),
        // Operation chain.
        "DEPLOYED_TO" | "REQUIRES_RESOURCE" => (Some(Downstream), Some(Upstream)),
        // As-released packaging: Release INCLUDES Artifact/Component. Same
        // shape as REALIZES — the contents are the source of truth, the
        // release a downstream packaging of them: a changed artifact ripples
        // to the releases that ship it (the next cut differs), and from a
        // release you reach what it packaged. Absent from this table, every
        // Release+Environment pair was a disconnected island in the design
        // network — DEPLOYED_TO joined them to each other and INCLUDES joined
        // them to nothing (found modelling v0.4.0, fixed for v0.5.0).
        "INCLUDES" => (Some(Upstream), Some(Downstream)),
        // The satisfaction schedule: Capability/Requirement SCHEDULED_FOR a
        // DesignEpoch or Release. WHAT→WHEN, the same shape as ALLOCATED_TO's
        // WHAT→WHERE — the item is the source of truth and the moment is a
        // commitment downstream of it, so a changed capability ripples to the
        // increment it was promised to, and from an increment you reach what
        // it was promised.
        //
        // THIS IS THE INCLUDES BUG ABOVE, RECURRED. Absent from this table, a
        // Release wired only by its schedule is a disconnected island again —
        // found 2026-07-31 the first time a release was modelled from the
        // schedule rather than from a manifest, by `disconnected_community`
        // firing on `rel:v0190` and the decision governing it. Twice now a new
        // edge type has reached a Release without anyone asking whether the
        // impact table should know about it, and nothing checks that question
        // is asked: the edge validates, stores and queries perfectly while
        // every traversal steps over it (`dec:schedule-is-structural`).
        "SCHEDULED_FOR" => (Some(Downstream), Some(Upstream)),
        _ => return None,
    };
    Some(EdgeRule {
        forward: fwd,
        backward: bwd,
    })
}

/// Whether an edge type is a structural traceability edge (the design network's
/// coupling edges — the same set PROPAGATE walks, excluding CONTAINS). Used by
/// both [`crate::propagate`] and [`crate::structure`] so the impact walk and
/// the topology analysis agree on what "connected in the design" means.
pub(crate) fn is_traceability_edge(edge_type: &str) -> bool {
    structural_rule(edge_type).is_some()
}

/// FNV-1a 64-bit — a small, stable, dependency-free hash so the deterministic
/// ids of *derived* nodes (gaps, heal issues, merge conflicts, agent prompts,
/// drift and artifact-claim keys) are reproducible across runs and platforms
/// (`std`'s `DefaultHasher` is not guaranteed stable). Discipline 6.
///
/// It lives here, in the vocabulary/identity layer, because minting a derived
/// node's id is an identity concern shared across the coherence loop — not a
/// detection one. It began in `detect.rs` (where gap-id hashing first needed
/// it) and every other module borrowed it through `crate::detect::fnv1a`, which
/// manufactured a false dependency on the detect domain from eight modules and
/// a genuine detect↔verify *cycle* (verify's only tie to detect was this
/// helper). `nodes` is a leaf everything already sits above, so the primitive's
/// true, edge-free home is here (the cycle-break decision on the record).
pub(crate) fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
