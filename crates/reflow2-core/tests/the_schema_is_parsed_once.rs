//! The schema is parsed ONCE per process, not once per graph.
//!
//! `con:graph-construction-is-setup-not-work`. This file holds exactly ONE test
//! on purpose: it compares the FIRST construction in the process against a
//! later one, so it must run in a process where nothing has warmed the cache.
//! A second test in this binary would warm it and this one would silently
//! measure nothing — passing, forever, while proving no such thing.
//!
//! ⭐ WHY A RATIO AND NOT A DURATION, learned the hard way the same hour it was
//! written. The first version asserted "a warm construction takes under 5 ms".
//! Alone it measured 266 µs and passed comfortably. Inside `cargo test
//! --workspace`, where dozens of test binaries run at once, the same call
//! measured **6 ms** and the gate failed — **it was measuring machine
//! contention, not the code**. A duration threshold in a parallel suite is a
//! flake generator, and the usual response is to raise it until it stops
//! complaining, which retires the gate without anyone deciding to.
//!
//! A ratio survives contention because both halves are slowed by the same load.
//! And it asserts the thing that actually broke and was actually fixed: **the
//! parse happening once rather than every time.** The duration was only ever
//! the symptom.
//!
//! 🛑 THE GENERAL RULE THIS CASE ARGUES FOR: an optimisation guard should
//! assert the STRUCTURE that makes the code fast, not the time it takes. Time
//! is a property of the machine as much as of the program; structure is not.

use reflow2_core::graph::DesignGraph;
use std::time::Instant;

/// Measured cold:warm is roughly 150:1 (41.3 ms against 266 µs). Five is far
/// enough below that to survive any load, and far enough above 1 to be
/// impossible if the parse were still running on every call.
const MIN_RATIO: u32 = 5;

#[test]
fn the_first_construction_pays_for_the_parse_and_the_rest_do_not() {
    let t = Instant::now();
    let _first = DesignGraph::open_in_memory().expect("first");
    let cold = t.elapsed();

    // Several, so one unlucky scheduling slice cannot decide the verdict.
    let n = 10;
    let t = Instant::now();
    for _ in 0..n {
        let _ = DesignGraph::open_in_memory().expect("warm");
    }
    let warm = t.elapsed() / n;

    assert!(
        cold > warm * MIN_RATIO,
        "the first construction ({cold:?}) was not materially more expensive than a \
         later one ({warm:?}), so the schema is being parsed on EVERY construction \
         rather than once — check that PARSED_SCHEMA in schema.rs is still a \
         LazyLock and that load_schema() reads it. Before that cache existed the \
         ratio here was about 150:1; this asserts only 5:1."
    );
}
