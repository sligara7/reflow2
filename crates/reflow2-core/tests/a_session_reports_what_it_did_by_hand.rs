//! A session can record the work it did BY HAND that reflow2 already serves.
//!
//! `req:a-session-says-what-it-did-by-hand-that-reflow2-already-serves`, accepted
//! by Anthony 2026-08-26 and answered "build it now" the same day.
//!
//! ⭐ WHY THE NEGATIVE SPACE IS WORTH MORE THAN AN ABSENT CALL. Every measurement
//! reflow2 has of its own adoption looks at what a session DID with it. All of
//! them are blind to the same thing, and `dec:bl-155` states the consequence
//! outright: it measured 40 of 132 tools never called and **cannot tell unused
//! from unreachable**. Hand-rolled work discriminates between them, because it
//! carries INTENT — a session that wrote a script to do X proves somebody wanted
//! X, at a moment, badly enough to build it. A zero in a usage table never shows
//! that.
//!
//! It has already produced two of this project's central findings unprompted:
//! the 2026-08-26 allocation-vs-artifact comparison (no reflow2 tool does it —
//! `reconcile_artifacts` compares design against DISK, `compare_designs` compares
//! design against DESIGN, and neither compares two layers of ONE design), and the
//! 2026-08-20 "does my declared decomposition match the real coupling?" run.
//!
//! 🛑 TWO STANDING OBJECTIONS, NEITHER RESOLVED BY BUILDING THIS, both recorded
//! rather than smoothed away:
//!
//! 1. **It depends on the agent having noticed the tool existed** — the same
//!    blind spot it is trying to see. The requirement says so in its own words.
//! 2. **`dec:how-should-reflow2-log-its-own-usage` argues a tool is the wrong
//!    shape** — "a tool must be CALLED, and the population most worth measuring
//!    is precisely the one least likely to call it." That is correct, and for
//!    THIS signal it is also unavoidable: the server observes calls, and work
//!    done by hand happens where the server cannot see. Asking is the only route
//!    that exists, which is why the requirement chose the cheapest form.
//!
//! ⚠️ AND THE REPORT STAYS LOCAL. `req:telemetry-carries-usage-never-design-content`
//! governs what LEAVES a machine — "log the verb, never the object", and "a free
//! text field anywhere in the payload defeats this". A by-hand report is free
//! text naming the user's domain in their own words, so it belongs in the user's
//! own graph and **must never be lifted into a telemetry payload**. That is
//! pinned below, because the next contributor wiring telemetry will be looking
//! for useful fields and this is one.

use reflow2_core::DesignGraph;

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g
}

#[test]
fn a_report_lands_as_a_dated_observation() {
    let mut g = graph();

    let id = g
        .report_manual_work(
            "compared the allocation layer against the artifact layer with a hand-written script",
            "tool_missing",
            None,
            Some("2026-08-26"),
        )
        .unwrap();

    let n = g.get_node("TemporalFact", &id).unwrap().expect("recorded");
    assert_eq!(
        n.properties.get("fact_type").and_then(|v| v.as_str()),
        Some("manual_work"),
        "a report must be findable as one, not buried among every other observation"
    );
    assert_eq!(
        n.properties.get("valid_from").and_then(|v| v.as_str()),
        Some("2026-08-26"),
        "a report is a DATED observation — an undated one cannot be aged out"
    );
}

#[test]
fn the_same_work_reported_twice_is_one_record() {
    // Sessions repeat. If two reports of the same hand-rolled work made two
    // nodes, the signal would inflate with re-tellings and the count would stop
    // meaning "how many distinct things did people build by hand".
    let mut g = graph();
    let a = g
        .report_manual_work(
            "wrote a script to diff two layers",
            "tool_missing",
            None,
            None,
        )
        .unwrap();
    let b = g
        .report_manual_work(
            "wrote a script to diff two layers",
            "tool_missing",
            None,
            None,
        )
        .unwrap();

    assert_eq!(a, b, "the same work reported twice must be one record");
}

#[test]
fn a_diagnosis_it_does_not_know_is_refused() {
    // ⭐ THE SIGNAL'S WHOLE VALUE IS THE DIAGNOSIS, so a typo must not become a
    // silent new category. `dec:bl-155`'s finding is that reflow2 cannot tell
    // "unused" from "unreachable"; this field is what separates them, and a
    // free-text diagnosis would let the distinction rot one report at a time.
    let mut g = graph();

    let out = g.report_manual_work("did a thing", "tool_was_grumpy", None, None);

    assert!(
        out.is_err(),
        "an unknown diagnosis must be REFUSED and name what would have worked, \
         not stored as a new category nobody chose"
    );
}

// 📌 `a_named_tool_that_does_not_exist_is_refused` LIVES IN THE MCP SUITE, NOT
// HERE, and the reason is a real layering fact rather than convenience. That
// check asks "does reflow2 serve a tool by this name?", and the CORE does not
// know what the surface serves. Answering it here would mean a second copy of
// the tool list maintained by hand — the defect class this project spent
// 2026-08-26 fixing three times. At the tool, `tool_router.has_route()` answers
// it from the router itself, so nothing is maintained at all.
// See crates/reflow2-mcp/tests/a_by_hand_report_names_a_real_tool.rs.

#[test]
fn a_report_is_reportable() {
    // ⭐ THE LEG THAT KILLS VOCABULARY WHEN IT IS MISSING. The 2026-08-26 surface
    // audit found five types carrying all three legs and still unused, and
    // `DECOMPOSES` reached ZERO edges while shipping a detector for them. A
    // record nothing reads back is a record nobody writes twice.
    let mut g = graph();
    g.report_manual_work(
        "built a thing by hand",
        "tool_missing",
        None,
        Some("2026-08-26"),
    )
    .unwrap();
    g.report_manual_work(
        "built another",
        "tool_not_found",
        Some("search_design"),
        None,
    )
    .unwrap();

    let rows = g.manual_work_report().unwrap();
    assert_eq!(rows.len(), 2, "every report must come back");
    assert!(
        rows.iter().any(|r| r.diagnosis == "tool_not_found"
            && r.reflow2_tool.as_deref() == Some("search_design")),
        "the diagnosis and the named tool are the signal — they must survive the round trip"
    );
}
