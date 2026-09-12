//! An enum value an older reflow2 cannot read is refused, not shown as absent.
//!
//! # The hole this closes, and why it is the severe one
//!
//! The stamp named TYPES and nothing else, so the guard was blind to every
//! change that is not a type appearing or disappearing. The worst of those is an
//! ENUM VALUE: `Decision.status` gaining `deferred` moves no count and no name,
//! so an older binary opened the graph with **no warning at all**, compared
//! `status == "proposed"`, and the decision did not error — it VANISHED from
//! `loop_status` and `what_next`.
//!
//! That is the exact harm the guard's own refusal message names — *"opening it
//! could silently show you less of your design than it holds"* — arriving
//! through the door the guard did not watch.
//!
//! # The live case, and why these tests no longer name it
//!
//! `req:an-idea-that-stopped-is-not-counted-as-debt-somebody-owes` added
//! `deferred` to `Decision.status` on 2026-09-12, the increment after this one.
//! Shipping it first would have demonstrated this bug on reflow2's own graph,
//! which is why the two were sequenced this way on Anthony's word.
//!
//! ⚠️ UNTIL THAT DAY THE TESTS BELOW USED `deferred` AS THEIR UNKNOWN VALUE, and
//! the day it landed two of them FAILED — correctly: the value was no longer
//! unknown, so there was nothing to refuse. That is the guard doing its job,
//! not the tests breaking. They now use a value no reflow2 will ever declare,
//! so they describe the general situation an older binary meets; the LAST test
//! keeps `deferred` as the pin on the real case.
//!
//! # What these pin
//!
//! 1. **A declared-but-unused value OPENS.** The rule bought with two lockouts:
//!    refuse on a population, never on a declaration.
//! 2. **A value the graph actually HOLDS refuses, and names the value.**
//! 3. **A stamp with no enum record opens** — an absent record is not a claim
//!    that nothing changed, and it must not be read as one.
//! 4. **An unreadable population refuses**, rather than collapsing to "none".

use reflow2_core::foundation::core::Schema;
use reflow2_core::provenance::{GraphStamp, check_and_stamp, stamp_path};
use reflow2_core::schema::load_schema;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("reflow2-enum-guard-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn holds_no_retired_type(_t: &[String]) -> Result<Vec<String>, reflow2_core::DynoError> {
    Ok(Vec::new())
}

/// A stamp from a reflow2 whose `Decision.status` has one more value than this
/// binary's — the shape any future value has, and the shape `deferred` had
/// until 2026-09-12.
fn stamp_with_extra_value(schema: &Schema, field: &str, extra: &str) -> GraphStamp {
    let mut s = GraphStamp::current(schema);
    let mut values = s
        .enum_values
        .clone()
        .unwrap_or_default()
        .get(field)
        .cloned()
        .unwrap_or_default();
    assert!(
        !values.is_empty(),
        "{field} must be a declared enum for this test to mean anything"
    );
    values.push(extra.to_string());
    values.sort();
    let mut map = s.enum_values.clone().unwrap_or_default();
    map.insert(field.to_string(), values);
    s.enum_values = Some(map);
    // A NEWER reflow2 wrote it, which is the case that actually happens.
    s.reflow2_version = "99.0.0".into();
    s
}

fn write_stamp(graph: &std::path::Path, s: &GraphStamp) {
    std::fs::create_dir_all(graph).unwrap();
    std::fs::write(
        stamp_path(graph.to_str().unwrap()),
        serde_json::to_string(s).unwrap(),
    )
    .unwrap();
}

/// THE RULE BOUGHT WITH TWO LOCKOUTS: a declaration is not a population.
#[test]
fn a_value_the_graph_does_not_hold_opens() {
    let d = tmpdir("declared-only");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    write_stamp(
        &g,
        &stamp_with_extra_value(&schema, "Decision.status", "set-aside-by-a-newer-reflow2"),
    );

    let v = check_and_stamp(
        g.to_str().unwrap(),
        &schema,
        holds_no_retired_type,
        |_| Ok(Vec::new()), // nothing stores it
    )
    .expect("a value nobody has used is not a reason to lock anybody out");
    // Openable, and the verdict says the graph came from a different vocabulary.
    assert!(
        v.note().is_some(),
        "the difference should still be reported"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// THE POINT: a value the graph HOLDS is refused, and the refusal names it.
#[test]
fn a_value_the_graph_holds_is_refused_and_named() {
    let d = tmpdir("populated");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    write_stamp(
        &g,
        &stamp_with_extra_value(&schema, "Decision.status", "set-aside-by-a-newer-reflow2"),
    );

    let err = check_and_stamp(
        g.to_str().unwrap(),
        &schema,
        holds_no_retired_type,
        |unknown| Ok(unknown.to_vec()), // every unknown value is stored
    )
    .expect_err("a stored value this binary cannot read must refuse");

    let msg = err.to_string();
    assert!(
        msg.contains("Decision.status") && msg.contains("set-aside-by-a-newer-reflow2"),
        "the refusal must NAME the value — a version number alone is what this \
         whole change exists to stop: {msg}"
    );
    assert!(
        msg.contains("99.0.0"),
        "and it must give the reader the one discriminator there is, the stamps' \
         own versions: {msg}"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// An absent enum record is NOT a claim that nothing changed.
#[test]
fn a_stamp_that_predates_the_enum_record_still_opens() {
    let d = tmpdir("legacy");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    let mut s = GraphStamp::current(&schema);
    s.enum_values = None; // every stamp written before this existed
    s.reflow2_version = "0.50.0".into();
    write_stamp(&g, &s);

    check_and_stamp(g.to_str().unwrap(), &schema, holds_no_retired_type, |_| {
        panic!("nothing to probe when the stamp records no enum vocabulary")
    })
    .expect("a stamp with no enum record must open, not refuse");
    std::fs::remove_dir_all(&d).ok();
}

/// A population that cannot be established refuses — never reads as empty.
#[test]
fn an_uncountable_population_refuses() {
    let d = tmpdir("unscannable");
    let g = d.join("graph");
    let schema = load_schema().unwrap();
    write_stamp(
        &g,
        &stamp_with_extra_value(&schema, "Decision.status", "set-aside-by-a-newer-reflow2"),
    );

    let err = check_and_stamp(g.to_str().unwrap(), &schema, holds_no_retired_type, |_| {
        Err(reflow2_core::DynoError::Storage("the scan failed".into()))
    })
    .expect_err("an uncountable population must not read as an empty one");
    assert!(
        err.to_string().contains("the scan failed"),
        "and the real reason must survive: {err}"
    );
    std::fs::remove_dir_all(&d).ok();
}

/// The stamp records the vocabulary it claims to. Cheap, and it is what every
/// test above rests on: a collector that quietly returned nothing would make
/// them all pass while the guard saw nothing.
#[test]
fn the_stamp_actually_records_the_declared_enum_values() {
    let schema = load_schema().unwrap();
    let s = GraphStamp::current(&schema);
    let values = s.enum_values.expect("a current stamp records enum values");
    let status = values
        .get("Decision.status")
        .expect("Decision.status is a declared enum");
    assert!(
        status.contains(&"proposed".to_string()) && status.contains(&"accepted".to_string()),
        "and it records the real values: {status:?}"
    );
    assert!(
        status.contains(&"deferred".to_string()),
        "deferred IS in the schema since 2026-09-12 (increment 433). Until that day this \
         assertion was inverted, so the refusal tests above described a real future; \
         now they describe what an older binary meets when it opens this design"
    );
    // The vocabulary is broad enough to be worth recording at all.
    assert!(
        values.len() > 20,
        "the schema declares enums across many types; got {}",
        values.len()
    );
}
