//! `unobserved_locations` must mean "the sweep never reported this", and nothing
//! else — music_graph F10.
//!
//! WHAT WAS REPORTED: a design registering `archive/` as a whole AND
//! `archive/reco.py` individually, sweeping both, got the individual files back
//! in `unobserved_locations` — "despite being right there in `observed`".
//!
//! WHY IT MATTERS MORE THAN THE WRONG NUMBER: the field exists to answer *did
//! you forget to sweep something*. An entry that means "a parent also claims
//! this" is a false alarm on correct modelling, and a reader who meets one
//! stops trusting the whole field — which is the only thing it was for. The
//! reporter said exactly that: "harmless here, but it makes the field
//! untrustworthy as a 'you forgot to sweep this' signal."

use reflow2_core::{DesignGraph, ObservedPath};

fn obs(path: &str, mass: u64) -> ObservedPath {
    ObservedPath {
        path: path.to_string(),
        mass,
    }
}

/// A design that claims a directory AND two files inside it — music_graph's shape.
fn graph_claiming_a_tree_and_its_files() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_artifact("art:archive", "archive", Some("code"), Some("archive"))
        .expect("dir artifact");
    g.add_artifact("art:reco", "reco.py", Some("code"), Some("archive/reco.py"))
        .expect("file artifact");
    g.add_artifact(
        "art:reco-clean",
        "reco_clean.py",
        Some("code"),
        Some("archive/reco_clean.py"),
    )
    .expect("file artifact");
    g
}

#[test]
fn a_file_that_was_swept_is_not_reported_unobserved_because_its_parent_also_claims_it() {
    // THE DEFECT, exactly as reported.
    let g = graph_claiming_a_tree_and_its_files();
    let swept = [
        obs("archive/reco.py", 100),
        obs("archive/reco_clean.py", 200),
    ];
    let r = g.coverage_report(&swept, &[], None).expect("coverage");
    assert!(
        r.unobserved_locations.is_empty(),
        "every claim was swept, yet these were called unobserved: {:?}",
        r.unobserved_locations
    );
}

#[test]
fn a_claim_the_sweep_really_did_miss_is_still_reported() {
    // THE COUNTERWEIGHT THAT KEEPS THE FIELD WORTH HAVING. Fixing the false
    // alarm by never reporting anything would be worse than the bug: the field
    // would become silence, and silence reads as "nothing was missed".
    let g = graph_claiming_a_tree_and_its_files();
    let swept = [obs("archive/reco.py", 100)];
    let r = g.coverage_report(&swept, &[], None).expect("coverage");
    assert_eq!(
        r.unobserved_locations,
        vec!["archive/reco_clean.py".to_string()],
        "the one genuinely unswept claim must still be named"
    );
}

#[test]
fn a_path_claimed_twice_is_still_counted_once() {
    // The count must NOT double when two claims match one observed path.
    // Marking every matching claim is about `unobserved_locations`; it must
    // leave `claimed` and the mass alone, or the fix trades one wrong number
    // for another.
    let g = graph_claiming_a_tree_and_its_files();
    let swept = [
        obs("archive/reco.py", 100),
        obs("archive/reco_clean.py", 200),
    ];
    let r = g.coverage_report(&swept, &[], None).expect("coverage");
    assert_eq!(r.observed, 2);
    assert_eq!(r.claimed, 2, "two paths, each claimed once");
    assert_eq!(r.unclaimed, 0);
    assert_eq!(r.claimed_mass, 300);
}

#[test]
fn a_directory_claim_alone_is_matched_by_a_file_beneath_it() {
    // The behaviour that already worked and must keep working: one opaque
    // directory artifact legitimately covers its subtree.
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_artifact("art:vendor", "vendor", Some("code"), Some("vendor"))
        .expect("artifact");
    let swept = [obs("vendor/a.py", 1), obs("vendor/deep/b.py", 1)];
    let r = g.coverage_report(&swept, &[], None).expect("coverage");
    assert!(r.unobserved_locations.is_empty());
    assert_eq!(r.claimed, 2);
}

#[test]
fn a_sibling_that_merely_shares_a_prefix_does_not_count_as_swept() {
    // `archive/reco.py` must not mark `archive/reco_clean.py` matched, and
    // `src/foo` must not be claimed by `src/foobar` — the boundary check.
    // Without it, "fixed" would mean "reports nothing", which the counterweight
    // above only partly catches.
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_artifact("art:foo", "foo", Some("code"), Some("src/foo"))
        .expect("artifact");
    g.add_artifact("art:foobar", "foobar", Some("code"), Some("src/foobar"))
        .expect("artifact");
    let swept = [obs("src/foo/mod.py", 1)];
    let r = g.coverage_report(&swept, &[], None).expect("coverage");
    assert_eq!(
        r.unobserved_locations,
        vec!["src/foobar".to_string()],
        "a shared prefix is not containment"
    );
}

#[test]
fn an_excluded_path_does_not_mark_its_claim_swept() {
    // A path the caller EXCLUDED was never really looked at, so a claim on it
    // has not been observed either. Counting it as swept would let an exclusion
    // rule quietly launder an unswept claim into a clean report.
    let mut g = DesignGraph::open_in_memory().expect("graph");
    g.add_artifact("art:build", "build", Some("code"), Some("build"))
        .expect("artifact");
    let swept = [obs("build/out.o", 5)];
    let r = g
        .coverage_report(&swept, &["build".to_string()], None)
        .expect("coverage");
    assert_eq!(r.excluded.len(), 1);
    assert_eq!(
        r.unobserved_locations,
        vec!["build".to_string()],
        "an excluded path is not an observation"
    );
}
