//! Domain-neutral graph-theory algorithms over a dense in-memory graph.
//!
//! The caller builds a [`Graph`] via [`GraphBuilder`] — interning string node
//! ids, supplying finite `f64` edge weights and a directed/undirected flag —
//! and runs algorithms over it; results come back keyed by dense node index for
//! the caller to map back to ids. Design graphs are small (10^2–10^3 nodes), so
//! every algorithm here is **exact**: no approximate or streaming variants.
//!
//! Malformed input (a non-finite edge weight, a negative strength, a
//! non-positive cost, non-convergence) is reported via [`GraphError`] rather
//! than silently coerced.
//!
//! # Provenance
//!
//! ABSORBED FROM `dynograph-foundation`, recorded here because a later reader
//! has no other way to find it:
//!
//! ```text
//! repo     https://github.com/sligara7/dynograph-foundation
//! tag      v0.12.0
//! commit   f8d813f83efebf0a521faf87d9f9ecd0f9090ee6   (2026-07-17)
//! files    crates/dynograph-graph/src/{error,graph,components,scc,cycles,cuts,
//!          communities,paths,betweenness}.rs
//! taken    verbatim, with their tests
//! licence  MIT, Copyright (c) 2026 Anthony Sligar
//! ```
//!
//! Increment 3 of `dec:absorb-the-foundation-subset-and-end-the-dependency`.
//! The header is a REQUIREMENT of that decision rather than a courtesy: the
//! recorded objection to absorbing anything is that vendoring converts a
//! visible dependency into an invisible one, and the version pin's written
//! history has no successor once the code is in-tree. This block is that
//! successor.
//!
//! # What was deliberately NOT taken — 42% of the crate
//!
//! 1,613 lines in nine files that reflow2 never calls: `centrality`,
//! `closeness`, `clustering`, `eigenvector`, `link_prediction`, `maxflow`,
//! `pagerank`, `shortest_path`, `toposort`. They remain in the upstream
//! repository if ever wanted.
//!
//! 🛑 **AND THIS LIST WAS WRONG ONCE, IN THE SAME HOUR IT WAS WRITTEN.** The
//! first pass put `betweenness` and `paths` here, because the grep that found
//! reflow2's import sites matched only single-line `use dynograph_graph::X;`
//! and missed the MULTILINE `use dynograph_graph::{ ... }` in `structure.rs` —
//! which imports `betweenness_centrality`. The same blind spot had already been
//! caught and named in the dependency scan minutes earlier, and it recurred
//! because it was fixed in one grep and not the other. **A closure is only as
//! wide as the syntaxes its pattern can see, and fixing that in one place does
//! not fix it in the other.**
//!
//! ⭐ **`cuts` DOES NOT NEED `maxflow`**, and that is the one result here worth
//! checking rather than assuming. Articulation points and bridges come from a
//! DFS lowlink walk, not from a min-cut, so `max_flow_min_cut` (248 lines)
//! stays behind. A closure taken on the intuition that "cuts need flow" would
//! have dragged it in.
//!
//! # Why this module is private
//!
//! `pub(crate)`, not `pub`, for the same reason as [`crate::stats`] and
//! [`crate::fuzzy`]: `ifc:core-api` already records 277 public functions growing
//! by default and calls that surface unenumerable. Absorbing code is not a
//! reason to widen a surface already recorded as too wide.

// ⚖️ ABSORBED CODE IS KEPT VERBATIM, SO ITS UNUSED API IS ALLOWED RATHER THAN
// TRIMMED — a deliberate call, not an oversight, and the alternative was real.
//
// Eight items here are dead from reflow2's point of view: the `EmptyGraph` and
// `NotConverged` error variants, `SelfLoopPolicy::Keep`, `ParallelEdgePolicy::
// {Max, KeepFirst}`, the `self_loop_policy` / `parallel_edge_policy` accessors,
// `Graph::neighbor_sets`, `Path::dist`, and `paths::distances`. Clippy is right
// that nothing calls them.
//
// THEY STAY, for three reasons:
//   1. PROVENANCE IS THE POINT. `dec:absorb-the-foundation-subset-and-end-the-
//      dependency` requires each increment to name the tag and files it took,
//      so a later reader can diff against upstream. Files edited on the way in
//      are no longer diffable, and the header above would be making a claim
//      that is no longer quite true.
//   2. THEY ARE ONE API, NOT SPARE PARTS. The dead items are the algorithms'
//      own vocabulary — which self-loop policy, which parallel-edge policy,
//      which error a caller must handle. Deleting the unselected half of an
//      enum leaves a type that reads as if the choice never existed.
//   3. WE HAVE OWNED THIS CODE FOR MINUTES. Trimming the internals of a Leiden
//      implementation on the day it arrives is exactly when the least is known
//      about which parts matter.
//
// ⭐ "ONLY TAKE WHAT WE NEED" WAS ALREADY APPLIED, AT THE FILE LEVEL: 1,613 of
// 3,832 lines were left behind. This is about the last few items INSIDE files
// that had to come. If they are still dead in six months, that is the moment to
// trim — with a diff against upstream still possible today.
#![allow(dead_code)]

mod betweenness;
mod communities;
mod components;
mod cuts;
mod cycles;
mod error;
mod graph;
mod paths;
mod scc;

pub(crate) use betweenness::betweenness_centrality;
pub(crate) use communities::leiden;
pub(crate) use components::connected_components;
pub(crate) use cuts::cut_structure;
pub(crate) use cycles::find_cycle;
pub(crate) use graph::{Graph, GraphBuilder};
pub(crate) use scc::strongly_connected_components;
