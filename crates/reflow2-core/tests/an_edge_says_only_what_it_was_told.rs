//! Two edge findings from two projects on one day were one class: edge
//! properties were never swept for reach or for asserting defaults
//! (`fact:root-cause-two-field-holes-a-rejected-coverage-and-a-basis-asserted-for-a-number-that-is-not-there-…`).
//!
//! bhome: `satisfies` rejected `coverage`, declared on the edge since the
//! schema existed, so a capability that only partly met a need was recorded
//! as fully meeting it. xrt-demo F6: `constrains` materialised `basis:
//! estimated` on an edge with no contribution — the provenance of a number
//! that is not there.
//!
//! Written before the fix: the coverage half did not compile against the
//! unfixed core (no `satisfies_with_coverage`), and the basis half was
//! observed failing (`estimated` came back on an edge with no number).

use reflow2_core::DesignGraph;
use reflow2_core::foundation::core::Value;

fn g() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("prj:p", "P").unwrap();
    g.add_requirement(
        "req:see-it",
        "See the container in 3D",
        "A person can look at it.",
    )
    .unwrap();
    g.add_capability("cap:render", "Render the shell", "Draws a picture.", None)
        .unwrap();
    g
}

#[test]
fn coverage_is_stored_where_it_is_read_and_a_partial_satisfier_does_not_deliver() {
    let mut g = g();
    let e = g
        .satisfies_with_coverage("cap:render", "req:see-it", Some("partial"))
        .unwrap();
    assert_eq!(
        e.properties.get("coverage").and_then(Value::as_str),
        Some("partial")
    );
    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.requirements, 1);
    assert_eq!(
        d.partially_satisfied, 1,
        "a capability that only partly meets a need is counted as that, not as satisfaction"
    );
    assert_eq!(d.satisfied, 0, "{d:?}");
}

#[test]
fn an_unstated_coverage_still_reads_as_full_so_older_designs_do_not_move() {
    let mut g = g();
    g.satisfies("cap:render", "req:see-it").unwrap();
    let d = g.delivery_coverage().unwrap();
    assert_eq!(d.satisfied, 1);
    assert_eq!(d.partially_satisfied, 0);
}

#[test]
fn a_contribution_that_is_not_there_has_no_basis() {
    let mut g = g();
    g.add_constraint(
        "con:time",
        "Trace time",
        "Under a minute per trace.",
        Some("budget"),
        Some("trace_time"),
        Some(60.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.add_component("cmp:tracer", "tracer", "runs traces", None)
        .unwrap();
    let e = g
        .constrains(
            "con:time",
            "Component",
            "cmp:tracer",
            None,
            Some("estimated"),
            None,
            None,
        )
        .unwrap();
    assert!(
        !e.properties.contains_key("basis"),
        "no number, so nothing a basis could describe: {:?}",
        e.properties
    );
    assert!(!e.properties.contains_key("contribution"));
}

#[test]
fn a_stated_number_keeps_its_basis_and_the_rollup_still_reads_an_absent_one_as_estimated() {
    let mut g = g();
    g.add_constraint(
        "con:time",
        "Trace time",
        "Under a minute per trace.",
        Some("budget"),
        Some("trace_time"),
        Some(60.0),
        None,
        Some("maximum"),
    )
    .unwrap();
    g.add_component("cmp:a", "a", "part", None).unwrap();
    g.add_component("cmp:b", "b", "part", None).unwrap();
    g.constrains(
        "con:time",
        "Component",
        "cmp:a",
        Some(20.0),
        Some("measured"),
        Some("2026-09-16"),
        None,
    )
    .unwrap();
    g.constrains(
        "con:time",
        "Component",
        "cmp:b",
        Some(10.0),
        None,
        None,
        None,
    )
    .unwrap();
    let r = g.budget_report("con:time").unwrap();
    assert_eq!(r.basis_coverage.get("measured"), Some(&1));
    assert_eq!(
        r.basis_coverage.get("estimated"),
        Some(&1),
        "the reader supplies the safe reading; the store no longer asserts it: {:?}",
        r.basis_coverage
    );
}
