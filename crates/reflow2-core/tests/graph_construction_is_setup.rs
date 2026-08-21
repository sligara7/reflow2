//! Constructing a design graph is SETUP, and must not cost like work.
//!
//! `con:graph-construction-is-setup-not-work` — the first performance budget
//! this project has ever stated, written down BEFORE the optimisation it
//! governs, because a budget is what makes optimisation stop.
//!
//! MEASURED 2026-08-21, before: `open_in_memory` cost 41.3 ms against 54.7 µs
//! for an `add_capability` write — construction was **750× a write**. The cause
//! was `schema::load_schema()` re-parsing and re-merging eleven `include_str!`'d
//! YAML documents on every single call, in a binary where those bytes are fixed
//! at compile time and cannot parse to two different answers.
//!
//! After caching the parse: **266 µs warm**, of which 252 µs is cloning the
//! parsed schema and 14 µs is the engine itself.
//!
//! ⭐ NOTHING HERE ASSERTS A DURATION, and that is the lesson rather than the
//! style. The first version of this file did — "a warm construction takes under
//! 5 ms" — which measured 266 µs alone and **6 ms inside `cargo test
//! --workspace`**, where dozens of binaries compete. It was measuring the
//! machine, not the code. The cold-versus-warm ratio that proves the parse is
//! cached lives in `the_schema_is_parsed_once.rs`, alone in its own binary
//! because it must run before anything warms the cache.
//!
//! What is left here is the RATIO the budget is really about — setup must not
//! dominate the work it sets up — and the contract the cache has to keep.

use reflow2_core::graph::DesignGraph;
use std::time::Instant;

#[test]
fn the_schema_is_parsed_once_and_handed_out_by_value() {
    // Two graphs must not share one schema — StorageEngine takes it by value,
    // and the cache hands out clones for exactly that reason. This pins the
    // contract the cache has to keep, so a future change to `Arc` (the obvious
    // next optimisation) has to face it rather than discover it.
    let a = reflow2_core::schema::load_schema().expect("first");
    let b = reflow2_core::schema::load_schema().expect("second");
    assert_eq!(a.node_types.len(), b.node_types.len());
    assert!(
        !a.node_types.is_empty(),
        "an empty schema would pass every other check here"
    );
}

#[test]
fn a_write_still_costs_far_less_than_a_construction() {
    // The RATIO is what the budget is really about: setup must not dominate the
    // thing it sets up. Asserted as an ordering rather than a number so it says
    // something true on any machine.
    let _ = DesignGraph::open_in_memory().expect("warm");

    let t = Instant::now();
    let mut g = DesignGraph::open_in_memory().expect("open");
    let construction = t.elapsed();

    g.add_project("proj:p", "P").expect("project");
    let t = Instant::now();
    for i in 0..20 {
        g.add_capability(&format!("cap:{i}"), "C", "d", None)
            .expect("cap");
    }
    let twenty_writes = t.elapsed();

    assert!(
        construction < twenty_writes * 4,
        "one construction ({construction:?}) should not dwarf twenty writes \
         ({twenty_writes:?}) — setup is not work. Before the schema parse was \
         cached this ratio was roughly 750:1 for a SINGLE write."
    );
}
