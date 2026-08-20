//! Axis-Y decomposition — the matryoshka spine and its defects
//! (docs/three-axes.md §"Axis Y", chain_reflow's matryoshka insight,
//! gap-surfacing.md GS-11).
//!
//! Components nest by `Component.level` — `component ▸ subsystem ▸ system ▸
//! system_of_systems ▸ enterprise` — with `CONTAINS` between Components
//! expressing the spine and `DEPENDS_ON` the peer coupling. The rule of thumb
//! (from the schema itself): **never link across more than one level directly.**
//! The high-value detector is a *missing intermediate level* — the
//! carburetor-to-body problem: a part wired straight to a system with no
//! subsystem between them.
//!
//! Detectors (deterministic, pure level arithmetic):
//! - `missing_intermediate_level` — a `CONTAINS`/`DEPENDS_ON` between components
//!   skips ≥2 levels.
//! - `level_mismatch` — a `CONTAINS` whose parent is not strictly above its
//!   child (inverted or same-level containment).
//! - `orphan_level` — a subsystem-or-higher component with neither a
//!   higher-level parent nor a lower-level child — a floating mid-level node.
//!   A Project counts as a parent: it is the root of the containment spine,
//!   above every level, and `contains` puts top-level parts directly under it.
//!
//! These feed DETECT (surfaced as gaps) and, per heal-process.md HEAL-14, are
//! what HEAL would repair by proposing the *missing intermediate* Component.

use std::collections::{HashMap, HashSet};

use dynograph_core::{DynoError, Value};

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// A decomposition level — mirrors `structure.yaml`'s `Component.level` enum,
/// ordered low → high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Component,
    Subsystem,
    System,
    SystemOfSystems,
    Enterprise,
}

impl Level {
    /// The exact schema enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Component => "component",
            Level::Subsystem => "subsystem",
            Level::System => "system",
            Level::SystemOfSystems => "system_of_systems",
            Level::Enterprise => "enterprise",
        }
    }

    /// Parse a stored level string; unknown → `component` (the schema default).
    pub fn from_key(s: &str) -> Level {
        match s {
            "subsystem" => Level::Subsystem,
            "system" => Level::System,
            "system_of_systems" => Level::SystemOfSystems,
            "enterprise" => Level::Enterprise,
            _ => Level::Component,
        }
    }

    /// Ordinal rank (component = 0 … enterprise = 4).
    pub fn rank(self) -> i32 {
        match self {
            Level::Component => 0,
            Level::Subsystem => 1,
            Level::System => 2,
            Level::SystemOfSystems => 3,
            Level::Enterprise => 4,
        }
    }
}

/// What kind of decomposition defect (gap-surfacing.md GS-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HierarchyIssueKind {
    /// A link skips ≥2 levels (the carburetor-to-body problem).
    MissingIntermediateLevel,
    /// A `CONTAINS` whose parent is not strictly above its child.
    LevelMismatch,
    /// A subsystem-or-higher component with no parent above and no child below.
    ///
    /// Containment by the Project counts as a parent. Without that, the shape
    /// the tools lead you to — a Project holding a few subsystems — reported one
    /// orphan per subsystem, which is how reflow2's own design produced two.
    OrphanLevel,
    /// A Component contained by MORE THAN ONE parent — a box in two boxes.
    ///
    /// The spine is a tree. Two parents make "which box is this in?" a question
    /// with two answers, and every walk that assumes one silently picks whichever
    /// it reached first. Found unflagged in reflow2's own design (`cmp:skills`,
    /// under both `proj:reflow2` and `sys:agent-surface`).
    MultipleParents,
    /// A Component that sits at the ROOT of the spine while declaring a level
    /// something else in the design claims to be above.
    ///
    /// This is the "two answers that disagree" defect, measured 2026-08-18: ask
    /// for the top tier by declared `level` and you get the subsystems; ask by
    /// spine position — components with no `CONTAINS` parent — and you get leaves
    /// that were never wired to a parent. Both queries are reasonable, they
    /// disagree, and the structural one is confidently wrong. Reported against
    /// the node so the disagreement is fixed where it is, rather than left for
    /// each caller to trip over.
    LevelSpineDisagreement,
}

impl HierarchyIssueKind {
    /// Stable snake_case key.
    pub fn as_str(self) -> &'static str {
        match self {
            HierarchyIssueKind::MissingIntermediateLevel => "missing_intermediate_level",
            HierarchyIssueKind::LevelMismatch => "level_mismatch",
            HierarchyIssueKind::OrphanLevel => "orphan_level",
            HierarchyIssueKind::MultipleParents => "multiple_parents",
            HierarchyIssueKind::LevelSpineDisagreement => "level_spine_disagreement",
        }
    }
}

/// A detected decomposition defect.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HierarchyIssue {
    /// The kind of defect.
    pub kind: HierarchyIssueKind,
    /// The component(s) involved (1 for orphan_level, 2 for the edge defects).
    pub components: Vec<String>,
    /// Which edge produced an edge-based defect (`"contains"` / `"depends_on"`),
    /// or `None` for a node-based one. A `CONTAINS` and a `DEPENDS_ON`
    /// missing-intermediate between the SAME pair are the same kind over the
    /// same components, so without this they hashed to one gap id and a single
    /// acknowledgement suppressed both (BL-58). It discriminates the gap id.
    pub relation: Option<&'static str>,
    /// Human-readable description with the levels involved.
    pub message: String,
}

impl DesignGraph {
    /// Build the id → level map for every Component (level defaults to
    /// `component`, applied by the schema on create).
    fn component_levels(&self) -> Result<HashMap<String, Level>, DynoError> {
        let mut levels = HashMap::new();
        for c in self.scan_nodes(node::COMPONENT)? {
            let lvl = c
                .properties
                .get("level")
                .and_then(Value::as_str)
                .map(Level::from_key)
                .unwrap_or(Level::Component);
            levels.insert(c.node_id, lvl);
        }
        Ok(levels)
    }

    /// Detect axis-Y decomposition defects. See the module docs.
    pub fn hierarchy_issues(&self) -> Result<Vec<HierarchyIssue>, DynoError> {
        let levels = self.component_levels()?;
        let mut issues = Vec::new();

        // Edge-based defects: CONTAINS (parent→child) and DEPENDS_ON (peer).
        for (id, &lvl) in &levels {
            // CONTAINS: parent should be exactly one level above the child.
            for e in self.outgoing(id, Some(edge::CONTAINS))? {
                let Some(&child) = levels.get(&e.to_id) else {
                    continue; // only component→component containment is the spine
                };
                let diff = lvl.rank() - child.rank();
                if diff >= 2 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::MissingIntermediateLevel,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("contains"),
                        message: format!(
                            "'{}' ({}) directly contains '{}' ({}) — {} intermediate level(s) skipped",
                            e.from_id, lvl.as_str(), e.to_id, child.as_str(), diff - 1
                        ),
                    });
                } else if diff <= 0 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::LevelMismatch,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("contains"),
                        message: format!(
                            "'{}' ({}) contains '{}' ({}) but a parent must be above its child",
                            e.from_id,
                            lvl.as_str(),
                            e.to_id,
                            child.as_str()
                        ),
                    });
                }
            }
            // DEPENDS_ON: peers ≥2 levels apart mean a missing intermediate.
            for e in self.outgoing(id, Some(edge::DEPENDS_ON))? {
                let Some(&other) = levels.get(&e.to_id) else {
                    continue;
                };
                if (lvl.rank() - other.rank()).abs() >= 2 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::MissingIntermediateLevel,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("depends_on"),
                        message: format!(
                            "'{}' ({}) depends directly on '{}' ({}) across ≥2 levels — a missing intermediate",
                            e.from_id, lvl.as_str(), e.to_id, other.as_str()
                        ),
                    });
                }
            }
        }

        // orphan_level: a subsystem-or-higher component with no higher-level
        // parent and no lower-level child on the CONTAINS spine.
        //
        // The Project anchors the spine. It carries no `Component.level` — it
        // sits above all of them — so a subsystem it CONTAINS has a parent even
        // though `levels` knows nothing about it. Reading only the Component
        // side made every top-level part look floating: `contains` is exactly
        // how a Project takes ownership of one.
        let projects: HashSet<String> = self
            .scan_nodes(node::PROJECT)?
            .into_iter()
            .map(|n| n.node_id)
            .collect();

        for (id, &lvl) in &levels {
            if lvl.rank() < Level::Subsystem.rank() {
                continue; // a bare component with no parent/child is normal
            }
            let has_higher_parent = self.incoming(id, Some(edge::CONTAINS))?.iter().any(|e| {
                projects.contains(&e.from_id)
                    || levels
                        .get(&e.from_id)
                        .is_some_and(|p| p.rank() > lvl.rank())
            });
            let has_lower_child = self
                .outgoing(id, Some(edge::CONTAINS))?
                .iter()
                .any(|e| levels.get(&e.to_id).is_some_and(|c| c.rank() < lvl.rank()));
            if !has_higher_parent && !has_lower_child {
                issues.push(HierarchyIssue {
                    kind: HierarchyIssueKind::OrphanLevel,
                    components: vec![id.clone()],
                    relation: None,
                    message: format!(
                        "'{}' ({}) is not contained by anything above it and contains \
                         nothing below it",
                        id,
                        lvl.as_str()
                    ),
                });
            }
        }

        // multiple_parents: the spine is a tree, so two parents is a defect
        // regardless of level arithmetic. Project parents COUNT here, unlike in
        // the level checks: `proj:reflow2 CONTAINS cmp:skills` and
        // `sys:agent-surface CONTAINS cmp:skills` are two boxes, and which one
        // "the" box is cannot be answered.
        for id in levels.keys() {
            let parents: Vec<String> = self
                .incoming(id, Some(edge::CONTAINS))?
                .iter()
                .filter(|e| projects.contains(&e.from_id) || levels.contains_key(&e.from_id))
                .map(|e| e.from_id.clone())
                .collect();
            if parents.len() > 1 {
                let mut named = parents.clone();
                named.sort();
                issues.push(HierarchyIssue {
                    kind: HierarchyIssueKind::MultipleParents,
                    components: vec![id.clone()],
                    relation: Some("contains"),
                    message: format!(
                        "'{}' is contained by {} parents ({}) — the spine is a tree, so \
                         \"which box is this in?\" has no single answer and every walk that \
                         assumes one picks whichever it reached first",
                        id,
                        named.len(),
                        named.join(", ")
                    ),
                });
            }
        }

        // level_spine_disagreement: a parentless Component that declares a level
        // something else claims to be above. Asking for the top tier by `level`
        // and by spine position then return different sets.
        //
        // SELF-LIMITING BY CONSTRUCTION: the comparison is against the highest
        // level actually PRESENT, so a flat design where every part is
        // `component` reports nothing. Nothing here prescribes a ladder depth —
        // that would be the over-modelling this project refuses.
        if let Some(&top) = levels.values().max_by_key(|l| l.rank()) {
            for (id, &lvl) in &levels {
                if lvl.rank() >= top.rank() {
                    continue; // legitimately a root
                }
                let has_parent = self
                    .incoming(id, Some(edge::CONTAINS))?
                    .iter()
                    .any(|e| projects.contains(&e.from_id) || levels.contains_key(&e.from_id));
                if !has_parent {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::LevelSpineDisagreement,
                        components: vec![id.clone()],
                        relation: None,
                        message: format!(
                            "'{}' declares level '{}' but sits at the ROOT of the spine — \
                             nothing contains it, while '{}' exists above it. Asking for the \
                             top tier by declared level and by spine position give different \
                             answers, and this node is why",
                            id,
                            lvl.as_str(),
                            top.as_str()
                        ),
                    });
                }
            }
        }

        issues.sort_by(|a, b| {
            a.kind
                .as_str()
                .cmp(b.kind.as_str())
                .then(a.components.cmp(&b.components))
        });
        Ok(issues)
    }
}
