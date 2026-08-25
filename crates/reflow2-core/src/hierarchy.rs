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

use crate::foundation::core::{DynoError, Value};

use crate::graph::DesignGraph;
use crate::nodes::{edge, node};

/// THIS DESIGN'S DECOMPOSITION LADDER — the rungs, ordered bottom-first.
///
/// # Why this is not an enum any more
///
/// It was: `component ▸ subsystem ▸ system ▸ system_of_systems ▸ enterprise`,
/// with `rank()` a match arm. That made `component` a HARD FLOOR — there was no
/// value to give a part of a component, so `cmp:byte-store CONTAINS
/// cmp:memory-backend` could not be expressed at all and came back as a
/// `level_mismatch`. `req:recursive-black-box-decomposition` (accepted, 2026-08-07)
/// asks for nesting "as deep as the design needs, from code projects to biology
/// projects", so the closed enum made an accepted requirement UNSATISFIABLE rather
/// than merely unfinished.
///
/// ⭐ AND THE FIX IS NOT ONE MORE RUNG. The canonical SE ladder adds `assembly`
/// between subsystem and component, which would have fixed the case above. It was
/// declined because **"atomic" shifts by domain**: the atomic unit is a
/// microprocessor chip for aerospace and a single code module for software. A
/// ladder of fixed names asserts every domain bottoms out at the same conceptual
/// depth (`dec:the-decomposition-ladder-is-open-not-a-fixed-enum`).
///
/// # The one home for the ordering
///
/// 🛑 THE LADDER USED TO BE DECLARED TWICE — `schema/structure.yaml`'s enum and
/// this file's `Level` — with nothing checking they agreed, and `from_key`
/// mapping anything unrecognised to `Component`. A rung added to the YAML and
/// forgotten here would have silently ranked 0, which is a silent fallback in a
/// project holding `req:no-silent-fallback` at critical priority. Ordering is now
/// DATA with exactly one home: `Project.decomposition_levels`, or [`DEFAULT_RUNGS`]
/// when a design has not said. [`Ladder::rank`] returns `None` for a name that is
/// not on the ladder — never `Some(0)` — and the callers report it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Ladder {
    rungs: Vec<String>,
}

/// The ladder a design gets when it has not declared one. These are exactly the
/// five values the old enum carried, in the old order, so every design that
/// predates the open ladder behaves as it always did and no data migrates.
pub const DEFAULT_RUNGS: [&str; 5] = [
    "component",
    "subsystem",
    "system",
    "system_of_systems",
    "enterprise",
];

impl Default for Ladder {
    fn default() -> Self {
        Ladder {
            rungs: DEFAULT_RUNGS.iter().map(|s| (*s).to_string()).collect(),
        }
    }
}

impl Ladder {
    /// Build from an explicit ordered list, bottom-first. Empty falls back to the
    /// default rather than producing a ladder on which nothing can be ranked —
    /// a design that declares no rungs has not chosen a different ladder, it has
    /// said nothing.
    pub fn from_rungs(rungs: Vec<String>) -> Ladder {
        if rungs.is_empty() {
            Ladder::default()
        } else {
            Ladder { rungs }
        }
    }

    /// The rungs, bottom-first.
    pub fn rungs(&self) -> &[String] {
        &self.rungs
    }

    /// Position on the ladder, 0 = finest.
    ///
    /// `None` means THIS NAME IS NOT ON THIS LADDER, which is a different fact
    /// from "it is the bottom rung" and must never be collapsed into it — that
    /// collapse is exactly what the old `from_key` did.
    pub fn rank(&self, level: &str) -> Option<usize> {
        self.rungs.iter().position(|r| r == level)
    }

    /// The finest rung. Used where the old code said `Level::Component`.
    pub fn bottom(&self) -> &str {
        &self.rungs[0]
    }
}

/// What [`DesignGraph::move_component`] did — never a bare success, because a
/// re-parent that silently dropped a containment the caller did not know about
/// is the failure the operation exists to prevent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoveOutcome {
    /// The Component that moved.
    pub child_id: String,
    /// Its parent now.
    pub new_parent_id: String,
    /// Every parent DETACHED by the move, sorted. Empty means the component
    /// was previously unplaced — a real and different fact from being moved,
    /// which is why it is reported rather than folded into a success flag.
    pub detached: Vec<String>,
    /// The new parent already contained it, so only the other parents moved.
    pub already_there: bool,
    /// How the two levels relate, when the answer is not "parent exactly one
    /// above child". Reported and never enforced: `hierarchy_issues` is the
    /// authority, and one rule with two homes is one rule that can disagree
    /// with itself.
    pub level_note: Option<String>,
    /// Present when a containment was detached and nothing has snapshotted the
    /// child, naming the call that preserves the previous parent.
    pub history_note: Option<String>,
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
    /// A Component whose `level` is not a rung on this design's ladder. Replaces
    /// the validation the closed enum used to give: the schema cannot know a
    /// per-design ladder, so an off-ladder level is REPORTED rather than
    /// silently ranked at the bottom (`req:no-silent-fallback`).
    UnknownLevel,
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
            HierarchyIssueKind::UnknownLevel => "unknown_level",
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
    /// This design's ladder, read from the Project node. A design with several
    /// Projects (a mirror sits alongside your own) takes the first that declares
    /// one, sorted by id so the answer is deterministic; a mirror carrying its
    /// own ladder must not silently redefine yours, and that case is not yet
    /// distinguished — see the module docs.
    pub fn decomposition_ladder(&self) -> Result<Ladder, DynoError> {
        let mut projects = self.scan_nodes(node::PROJECT)?;
        projects.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        for p in projects {
            if let Some(Value::List(items)) = p.properties.get("decomposition_levels") {
                let rungs: Vec<String> = items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
                if !rungs.is_empty() {
                    return Ok(Ladder::from_rungs(rungs));
                }
            }
        }
        Ok(Ladder::default())
    }

    /// Each Component's declared level, as written. Unranked here on purpose:
    /// ranking needs the ladder, and a name that is not on it must stay
    /// distinguishable from the bottom rung.
    fn component_levels(&self) -> Result<HashMap<String, String>, DynoError> {
        let mut levels = HashMap::new();
        for c in self.scan_nodes(node::COMPONENT)? {
            let lvl = c
                .properties
                .get("level")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_RUNGS[0])
                .to_string();
            levels.insert(c.node_id, lvl);
        }
        Ok(levels)
    }

    /// Detect axis-Y decomposition defects. See the module docs.
    pub fn hierarchy_issues(&self) -> Result<Vec<HierarchyIssue>, DynoError> {
        let levels = self.component_levels()?;
        let ladder = self.decomposition_ladder()?;
        let mut issues = Vec::new();

        // A level that is not on this design's ladder cannot be ranked, so it is
        // reported and then SKIPPED by every arithmetic check below rather than
        // being ranked at the bottom. Ranking it would resurrect exactly the
        // silent fallback the open ladder exists to remove: an off-ladder name
        // would read as the finest rung and every containment above it would
        // come back as a mismatch nobody could explain.
        let mut ranked: HashMap<&str, usize> = HashMap::new();
        for (id, lvl) in &levels {
            match ladder.rank(lvl) {
                Some(r) => {
                    ranked.insert(id.as_str(), r);
                }
                None => issues.push(HierarchyIssue {
                    kind: HierarchyIssueKind::UnknownLevel,
                    components: vec![id.clone()],
                    relation: None,
                    message: format!(
                        "'{}' declares level '{}', which is not a rung on this design's \
                         ladder ({}). Add it to Project.decomposition_levels, or correct \
                         the component.",
                        id,
                        lvl,
                        ladder.rungs().join(" ▸ ")
                    ),
                }),
            }
        }

        // Edge-based defects: CONTAINS (parent→child) and DEPENDS_ON (peer).
        for (id, lvl) in &levels {
            let Some(&lvl_rank) = ranked.get(id.as_str()) else {
                continue; // off-ladder: already reported as unknown_level
            };
            // CONTAINS: parent should be exactly one level above the child.
            for e in self.outgoing(id, Some(edge::CONTAINS))? {
                let Some(&child_rank) = ranked.get(e.to_id.as_str()) else {
                    continue; // only component→component containment is the spine
                };
                let child = &levels[&e.to_id];
                let diff = lvl_rank as i64 - child_rank as i64;
                if diff >= 2 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::MissingIntermediateLevel,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("contains"),
                        message: format!(
                            "'{}' ({}) directly contains '{}' ({}) — {} intermediate level(s) skipped",
                            e.from_id, lvl, e.to_id, child, diff - 1
                        ),
                    });
                } else if diff <= 0 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::LevelMismatch,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("contains"),
                        message: format!(
                            "'{}' ({}) contains '{}' ({}) but a parent must be above its child",
                            e.from_id, lvl, e.to_id, child
                        ),
                    });
                }
            }
            // DEPENDS_ON: peers ≥2 levels apart mean a missing intermediate.
            for e in self.outgoing(id, Some(edge::DEPENDS_ON))? {
                let Some(&other_rank) = ranked.get(e.to_id.as_str()) else {
                    continue;
                };
                let other = &levels[&e.to_id];
                if (lvl_rank as i64 - other_rank as i64).abs() >= 2 {
                    issues.push(HierarchyIssue {
                        kind: HierarchyIssueKind::MissingIntermediateLevel,
                        components: vec![e.from_id.clone(), e.to_id.clone()],
                        relation: Some("depends_on"),
                        message: format!(
                            "'{}' ({}) depends directly on '{}' ({}) across ≥2 levels — a missing intermediate",
                            e.from_id, lvl, e.to_id, other
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

        for (id, lvl) in &levels {
            let Some(&lvl_rank) = ranked.get(id.as_str()) else {
                continue; // off-ladder: already reported as unknown_level
            };
            // The FINEST rung is exempt: a leaf with no parent and no child is
            // ordinary, not floating. This used to read `< Level::Subsystem`,
            // which hardcoded the second rung of a fixed ladder; on an open
            // ladder the same intent is "anything above the bottom".
            if lvl_rank == 0 {
                continue;
            }
            let has_higher_parent = self.incoming(id, Some(edge::CONTAINS))?.iter().any(|e| {
                projects.contains(&e.from_id)
                    || ranked
                        .get(e.from_id.as_str())
                        .is_some_and(|&p| p > lvl_rank)
            });
            let has_lower_child = self
                .outgoing(id, Some(edge::CONTAINS))?
                .iter()
                .any(|e| ranked.get(e.to_id.as_str()).is_some_and(|&c| c < lvl_rank));
            if !has_higher_parent && !has_lower_child {
                issues.push(HierarchyIssue {
                    kind: HierarchyIssueKind::OrphanLevel,
                    components: vec![id.clone()],
                    relation: None,
                    message: format!(
                        "'{}' ({}) is not contained by anything above it and contains \
                         nothing below it",
                        id, lvl
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
        if let Some(top_rank) = ranked.values().copied().max() {
            let top = ladder.rungs()[top_rank].clone();
            for (id, lvl) in &levels {
                let Some(&lvl_rank) = ranked.get(id.as_str()) else {
                    continue; // off-ladder: already reported as unknown_level
                };
                if lvl_rank >= top_rank {
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
                            id, lvl, top
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
    /// Move a Component to a different parent on the containment spine —
    /// **detaching every parent it already had**, and saying which.
    ///
    /// ⭐ WHY THIS EXISTS AS ITS OWN OPERATION, and it is a product gap found
    /// by using reflow2 on a real re-decomposition (2026-08-20): the only way
    /// to re-parent used to be [`DesignGraph::contain_component`], which ADDS
    /// a parent and removes nothing. Asked in a user's own words — *"move a
    /// component to a different parent, re-decompose"* — `find_tools` ranked
    /// `contain_component` top of 152, so the discoverable route is also the
    /// wrong one: it leaves the old edge in place and the spine stops being a
    /// tree. `hierarchy_issues` then reports `multiple_parents` AFTERWARDS,
    /// which is the wrong end of the act. Re-decomposition is not exotic —
    /// it is what adopting a brownfield system, or acting on a design review,
    /// or doing severability work all consist of.
    ///
    /// **It never silently detaches.** The returned `detached` names every
    /// parent removed, because "which box was this in before?" is design
    /// history and a caller who did not realise there was one deserves to be
    /// told at the moment it happens rather than by a later detector.
    ///
    /// **It does not snapshot for you, and says so.** The old containment is
    /// design history; `record_change` against the child, taken while it still
    /// says the old thing, is what preserves it (see the `revise-design`
    /// skill). This returns [`MoveOutcome::history_note`] naming that call
    /// whenever anything was detached — the remedy reachable from the message
    /// the reader actually sees, rather than documented somewhere they are not
    /// looking.
    ///
    /// It deliberately does NOT try to work out whether the move is already
    /// recorded. The first version suppressed the note when the child had any
    /// snapshot at all, which is silent for precisely the long-lived node whose
    /// history matters most: a snapshot from an earlier epoch says nothing
    /// about THIS move. A note that is occasionally redundant beats one that is
    /// occasionally missing, so it states a fact rather than an accusation.
    ///
    /// The level relation is REPORTED, not enforced: `hierarchy_issues` is the
    /// authority on decomposition defects and refusing here would give the
    /// same rule two homes that could disagree.
    pub fn move_component(
        &mut self,
        child_id: &str,
        new_parent_id: &str,
    ) -> Result<MoveOutcome, DynoError> {
        if child_id == new_parent_id {
            return Err(DynoError::Validation {
                node_type: node::COMPONENT.into(),
                property: "move_component".into(),
                message: format!("'{child_id}' cannot contain itself."),
            });
        }
        for (role, id) in [("child", child_id), ("new parent", new_parent_id)] {
            if self.get_node(node::COMPONENT, id)?.is_none() {
                return Err(DynoError::NodeNotFound {
                    node_type: format!("{} ({role})", node::COMPONENT),
                    node_id: id.to_string(),
                });
            }
        }

        // Every current parent, whatever type holds it: a Component parent is
        // the ordinary case, but a node wired straight to the Project — which
        // is what a component with no subsystem looks like — must come off too,
        // or the move creates the very `multiple_parents` defect it exists to
        // prevent. That is not hypothetical: it is exactly what happened when
        // `cmp:identity` was given a real parent for the first time.
        let mut detached = Vec::new();
        for e in self.incoming(child_id, Some(edge::CONTAINS))? {
            if e.from_id == new_parent_id {
                continue;
            }
            self.delete_edge(edge::CONTAINS, &e.from_id, child_id)?;
            detached.push(e.from_id);
        }
        detached.sort();

        let already = self
            .incoming(child_id, Some(edge::CONTAINS))?
            .iter()
            .any(|e| e.from_id == new_parent_id);
        if !already {
            self.contain_component(new_parent_id, child_id)?;
        }

        let child_level = self.component_level(child_id)?;
        let parent_level = self.component_level(new_parent_id)?;
        // Rank both ends on THIS design's ladder. Either being off-ladder is a
        // real answer and not a diff of 0: the move still happens, and the note
        // says why no level advice could be given rather than inventing one.
        let ladder = self.decomposition_ladder()?;
        let level_note = match (ladder.rank(&parent_level), ladder.rank(&child_level)) {
            (Some(p), Some(c)) => match p as i64 - c as i64 {
                1 => None,
                0 => Some(format!(
                    "'{new_parent_id}' and '{child_id}' are both at level '{child_level}', so \
                     this containment is a level_mismatch — a parent should sit exactly one \
                     rung above its child. Set the levels with add_component; hierarchy_issues \
                     is the authority."
                )),
                d if d < 0 => Some(format!(
                    "'{new_parent_id}' (level '{parent_level}') sits BELOW '{child_id}' (level \
                     '{child_level}') — the containment is inverted and hierarchy_issues will \
                     report level_mismatch."
                )),
                _ => Some(format!(
                    "'{new_parent_id}' (level '{parent_level}') is more than one rung above \
                     '{child_id}' (level '{child_level}') — hierarchy_issues will report \
                     missing_intermediate_level."
                )),
            },
            (parent_rank, child_rank) => {
                let off: Vec<&str> = [
                    (parent_rank.is_none()).then_some(parent_level.as_str()),
                    (child_rank.is_none()).then_some(child_level.as_str()),
                ]
                .into_iter()
                .flatten()
                .collect();
                Some(format!(
                    "no level advice: {} is not a rung on this design's ladder ({}). The move \
                     was made; hierarchy_issues reports this as unknown_level.",
                    off.join(" and "),
                    ladder.rungs().join(" ▸ ")
                ))
            }
        };

        // Fires on ANY detachment, not only when the child has never been
        // snapshotted — which is what this checked first, and it was wrong in
        // the dangerous direction. A snapshot taken in some earlier epoch is no
        // evidence that THIS move was recorded, so keying on "has ever been
        // snapshotted" stayed silent for exactly the long-lived node whose
        // history is most worth keeping. Worded as a fact rather than an
        // accusation, so a caller who did record it is told nothing untrue.
        let history_note = (!detached.is_empty()).then(|| {
            let names = detached
                .iter()
                .map(|p| format!("'{p}'"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Detached {names} from '{child_id}'. That containment is design history: if this \
                 move is not already on the record, record_change against '{child_id}' preserves \
                 it — taken BEFORE the edit it captures the previous parent, and taken after it \
                 captures this one."
            )
        });

        Ok(MoveOutcome {
            child_id: child_id.to_string(),
            new_parent_id: new_parent_id.to_string(),
            detached,
            already_there: already,
            level_note,
            history_note,
        })
    }

    /// A Component's declared level, defaulting the way the schema does.
    fn component_level(&self, id: &str) -> Result<String, DynoError> {
        Ok(self
            .get_node(node::COMPONENT, id)?
            .and_then(|n| {
                n.properties
                    .get("level")
                    .and_then(|v| v.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| DEFAULT_RUNGS[0].to_string()))
    }
}
