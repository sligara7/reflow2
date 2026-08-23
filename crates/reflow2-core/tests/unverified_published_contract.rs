//! What a design owes for the boundaries it PUBLISHES — three findings.
//!
//! The file is named for the first of them and now covers all three; they are
//! tested together because they walk the same population and their INDEPENDENCE
//! is itself something to assert.
//!
//!   `unverified_published_contract`   nothing shows this promise holds
//!   `no_published_boundary`           you publish nothing — deliberate, or unclassified?
//!   `incomplete_published_contract`   the promise is not described in comparable terms
//!
//! A published contract with no passing check — and the posture question a
//! design gets when it has published nothing at all.
//!
//! The sibling of `unverified_enforced_rule`, one layer over. Both name an
//! obligation nobody can observe compliance with: a rule that claims the power
//! to fail a build, and a boundary that claims a consumer may depend on it.
//!
//! ⭐ IT COULD NOT HAVE EXISTED BEFORE 2026-08-21. `VERIFIES` could not reach an
//! `Interface` at all until then, so every contract in every design read as
//! unverified and this detector would have fired on all of them — correctly and
//! uselessly. The edge landed, and on reflow2's own design exactly ONE check has
//! been drawn since: vocabulary with no detector, seen from the outside.
//!
//! 🛑 THE HARD PART IS NOT THE FINDING, IT IS THE ZERO. `Interface.designation`
//! DEFAULTS to `internal`, so "nobody classified this" and "deliberately
//! internal" are stored identically — the schema says so itself. That makes a
//! zero from this detector genuinely ambiguous, which is why publishing nothing
//! raises its own question instead of reading as clean.

use reflow2_core::detect::GapSource;
use reflow2_core::graph::DesignGraph;

fn finding(g: &DesignGraph, src: GapSource) -> Option<reflow2_core::detect::GapCandidate> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .find(|c| c.gap_source == src)
}

/// Every id this source is billing across ALL its findings.
///
/// 🛑 EXISTS BECAUSE THE FIRST VERSION OF `both_is_billed_and_required_is_not`
/// WAS INERT. It read one finding's `affected_ids` and asserted `required` was
/// absent from it — but this detector emits ONE FINDING PER CONTRACT, so a
/// mutation that billed `required` simply added a SECOND finding and the first
/// one still looked right. The mutation survived. Asking what the detector
/// billed in total is the only question that can catch it.
fn billed(g: &DesignGraph, src: GapSource) -> Vec<String> {
    g.detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|c| c.gap_source == src)
        .flat_map(|c| c.affected_ids)
        .collect()
}

/// A design with two boundaries, one of them published.
fn seeded() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:api", "The public API").expect("ifc");
    g.add_interface("ifc:guts", "Internal plumbing")
        .expect("ifc");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");
    g
}

fn check(g: &mut DesignGraph, id: &str, target: &str, status: &str) {
    g.add_verification(id, id, Some("test"), Some("integration"), None)
        .expect("verification");
    g.verifies(id, "Interface", target).expect("verifies");
    g.set_verification_status(id, status, None, None)
        .expect("status");
}

#[test]
fn a_published_contract_with_no_check_is_named() {
    let g = seeded();
    let gap =
        finding(&g, GapSource::UnverifiedPublishedContract).expect("one published, unchecked");
    let billed = billed(&g, GapSource::UnverifiedPublishedContract);

    assert!(
        gap.affected_ids.contains(&"ifc:api".to_string()),
        "the finding names the boundary it is about: {:?}",
        gap.affected_ids
    );
    assert!(
        !billed.contains(&"ifc:guts".to_string()),
        "an INTERNAL boundary is plumbing the owner may change freely and is never asked about — \
         checked across every finding, not just this one: {billed:?}"
    );
    assert!(
        gap.evidence.contains("2026-08-21"),
        "the evidence says VERIFIES could not reach an Interface before that date, so an older \
         design having none is not neglect: {}",
        gap.evidence
    );
}

#[test]
fn a_passing_check_settles_it_and_a_planned_one_does_not() {
    // `dec:passing-is-verified`. Attaching a check that has never passed must
    // not silence the question, or the detector becomes the green-washing it
    // exists to catch.
    let mut g = seeded();

    check(&mut g, "ver:planned", "ifc:api", "planned");
    let gap =
        finding(&g, GapSource::UnverifiedPublishedContract).expect("a planned check shows nothing");
    assert!(
        gap.evidence.contains("none of them passing"),
        "and the evidence distinguishes 'no check' from 'a check that has not passed': {}",
        gap.evidence
    );

    g.set_verification_status("ver:planned", "passing", None, None)
        .expect("status");
    assert!(
        finding(&g, GapSource::UnverifiedPublishedContract).is_none(),
        "a PASSING check is what settles it"
    );
}

#[test]
fn both_is_billed_and_required_is_not() {
    // `both` offers a contract as well as needing one, so it is a promise
    // somebody outside may rely on. `required` is a promise somebody ELSE made
    // and is not this design's to prove.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:offers-and-needs", "Two-way")
        .expect("ifc");
    g.add_interface("ifc:needs", "Someone else's surface")
        .expect("ifc");
    g.set_interface_designation("ifc:offers-and-needs", "both")
        .expect("designation");
    g.set_interface_designation("ifc:needs", "required")
        .expect("designation");

    let billed = billed(&g, GapSource::UnverifiedPublishedContract);
    assert!(
        billed.contains(&"ifc:offers-and-needs".to_string()),
        "`both` offers a contract as well as needing one, so it is a promise: {billed:?}"
    );
    assert!(
        !billed.contains(&"ifc:needs".to_string()),
        "a contract this design NEEDS from outside is somebody else's promise and not this \
         design's to prove — checked across EVERY finding, because billing it would add a \
         second one rather than change the first: {billed:?}"
    );
    assert_eq!(billed.len(), 1, "exactly one promise here: {billed:?}");
}

#[test]
fn publishing_nothing_raises_the_posture_question_instead_of_reading_clean() {
    // 🛑 THE DECISION THIS PINS, and it is the whole reason there are two
    // findings. A detector answering zero over an empty population reads
    // exactly like one that ran clean. `Artifact.audience` is undefaulted so
    // its silence is legible; `designation` DEFAULTS to internal, so zero
    // published boundaries cannot distinguish a settled choice from an
    // unclassified design. So it asks.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:a", "A").expect("ifc");
    g.add_interface("ifc:b", "B").expect("ifc");

    assert!(
        finding(&g, GapSource::UnverifiedPublishedContract).is_none(),
        "nothing is published, so there is no promise to prove"
    );
    let gap = finding(&g, GapSource::NoPublishedBoundary).expect("but the zero is not clean");
    assert!(
        gap.evidence.contains("defaults to `internal`"),
        "the evidence names WHY the count is ambiguous rather than just reporting it: {}",
        gap.evidence
    );

    // And it stands down the moment somebody has actually chosen.
    g.set_interface_designation("ifc:a", "published")
        .expect("designation");
    assert!(
        finding(&g, GapSource::NoPublishedBoundary).is_none(),
        "once a boundary is published the posture question is answered"
    );
}

#[test]
fn a_design_with_no_boundaries_is_asked_nothing() {
    // A design with no interfaces has not declined to publish one — it has not
    // got there. Same reasoning as vocabulary_coverage's barely-started note:
    // a new design must not meet a wall of red on its first read.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_requirement("req:one", "One", "must hold")
        .expect("req");

    assert!(finding(&g, GapSource::NoPublishedBoundary).is_none());
    assert!(finding(&g, GapSource::UnverifiedPublishedContract).is_none());
}

#[test]
fn the_posture_question_keeps_its_id_and_each_promise_gets_its_own() {
    // Aggregate-vs-per-node, asserted through the BEHAVIOUR it exists for
    // rather than by reading the flag: a gap id is what an acknowledgement is
    // keyed on, so a stable id is exactly what makes a standing judgement
    // survive — and an unstable one is what makes it expire.
    //
    // 🛑 HONEST BOUND ON THE FIRST HALF. Flipping `NoPublishedBoundary` off the
    // aggregate list does NOT fail this, and that is a fact about the finding
    // rather than a weak assertion: it names no nodes, so `gap_id`'s per-node
    // branch hashes an empty list and the id holds either way. What this half
    // really pins is that the finding stays node-free and its id stays put as
    // the population moves. The SECOND half, below, is where the per-node
    // keying is genuinely exercised.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:a", "A").expect("ifc");
    let first = finding(&g, GapSource::NoPublishedBoundary)
        .expect("nothing published")
        .id;

    g.add_interface("ifc:b", "B").expect("ifc");
    let second = finding(&g, GapSource::NoPublishedBoundary).expect("still nothing published");
    assert_eq!(
        first, second.id,
        "\"we publish nothing on purpose\" is a claim about POSTURE — adding a boundary must not \
         expire it, or the question is asked again on every write"
    );
    assert!(
        second.title.contains('2'),
        "the id holds still while the COUNT moves on in the title: {}",
        second.title
    );

    // The promise is the opposite: one id per contract, so acknowledging that
    // one boundary is checked elsewhere cannot silently cover the next.
    g.set_interface_designation("ifc:a", "published")
        .expect("designation");
    g.set_interface_designation("ifc:b", "published")
        .expect("designation");
    let ids: Vec<String> = g
        .detect_gaps()
        .expect("detect")
        .into_iter()
        .filter(|c| c.gap_source == GapSource::UnverifiedPublishedContract)
        .map(|c| c.id)
        .collect();
    assert_eq!(ids.len(), 2, "two published contracts, two questions");
    assert_ne!(ids[0], ids[1], "each keyed on its own boundary");
}

// ---------------------------------------------------------------------------
// THE OTHER HALF OF THE SAME BOUNDARY: described in terms another design could
// compare, or not (`incomplete_published_contract`, serving
// `req:interface-spec-complete`).
//
// Separate from the finding above even though both walk the published set,
// because a design can be in either state independently: a fully specified
// contract nobody checks, and a checked contract nobody described.
// ---------------------------------------------------------------------------

/// Fill every agreement axis the schema has a field for.
fn fully_specified(g: &mut DesignGraph, id: &str) {
    g.set_interface_spec(
        id,
        Some("REST"),
        Some("synchronous"),
        Some("json"),
        Some("openapi.yaml"),
        Some("/v1/things"),
        Some("GET, POST"),
        Some("oauth2"),
        Some("tls"),
        Some("RFC 7807 problem+json"),
    )
    .expect("spec");
}

#[test]
fn a_published_contract_missing_agreement_axes_names_which() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:api", "The public API").expect("ifc");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");
    g.set_interface_spec(
        "ifc:api",
        Some("REST"),
        Some("synchronous"),
        Some("json"),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("spec");

    let gap = finding(&g, GapSource::IncompletePublishedContract).expect("six axes unset");
    assert!(
        gap.affected_ids.contains(&"ifc:api".to_string()),
        "the finding names the boundary"
    );
    for axis in [
        "payload_schema",
        "endpoint",
        "operations",
        "auth",
        "transport_security",
        "error_model",
    ] {
        assert!(
            gap.evidence.contains(axis),
            "every unset axis is NAMED, never reduced to a count — the whole point of \
             req:interface-spec-complete is that a seam be described in COMPARABLE terms, and a \
             fraction is not comparable: {axis} missing from {}",
            gap.evidence
        );
    }
    assert!(
        !gap.evidence.contains("medium,") && !gap.evidence.contains("paradigm"),
        "and an axis that IS set must not be named: {}",
        gap.evidence
    );

    // ⭐ THE QUESTION IS ASKED IN THE REQUIREMENT'S WORDS, NOT THE COLUMN'S.
    // `payload_schema` is a field name; "which fields are mandatory, and their
    // types" is the thing two systems have to agree on. The gloss is what makes
    // this answerable by somebody who never reads the schema — the standing
    // "speak the reader's domain" rule — and it is trivially strippable in a
    // refactor, so it is pinned rather than trusted.
    assert!(
        gap.description
            .contains("which fields are mandatory, and their types"),
        "each unset axis carries its plain-English gloss, not just its field name: {}",
        gap.description
    );
    assert!(
        gap.description
            .contains("the failure vocabulary a consumer parses"),
        "...for every axis, not only the first: {}",
        gap.description
    );
}

#[test]
fn a_fully_specified_contract_is_not_asked() {
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:api", "The public API").expect("ifc");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");
    fully_specified(&mut g, "ifc:api");

    assert!(
        finding(&g, GapSource::IncompletePublishedContract).is_none(),
        "every axis the schema can hold is filled"
    );
}

#[test]
fn an_internal_boundary_is_never_asked_to_describe_itself() {
    // Plumbing the owner may change freely. Only what a design has CHOSEN to
    // publish is billed — the same gate as the sibling finding.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:guts", "Internal plumbing")
        .expect("ifc");

    assert!(
        billed(&g, GapSource::IncompletePublishedContract).is_empty(),
        "an internal boundary owes nobody a description"
    );
}

#[test]
fn the_two_published_findings_are_independent() {
    // 🛑 THE REASON THESE ARE TWO DETECTORS AND NOT ONE. A design can be in
    // either state alone, and one acknowledgement must not settle both
    // questions.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:described", "Described but unchecked")
        .expect("ifc");
    g.set_interface_designation("ifc:described", "published")
        .expect("designation");
    fully_specified(&mut g, "ifc:described");

    assert!(
        finding(&g, GapSource::IncompletePublishedContract).is_none(),
        "fully described"
    );
    assert!(
        finding(&g, GapSource::UnverifiedPublishedContract).is_some(),
        "and still has nothing showing it holds — described is not checked"
    );

    // Now the mirror image: checked, but never described.
    let mut g2 = DesignGraph::open_in_memory().expect("open");
    g2.add_interface("ifc:checked", "Checked but undescribed")
        .expect("ifc");
    g2.set_interface_designation("ifc:checked", "published")
        .expect("designation");
    check(&mut g2, "ver:c", "ifc:checked", "passing");

    assert!(
        finding(&g2, GapSource::UnverifiedPublishedContract).is_none(),
        "a passing check settles the evidence question"
    );
    assert!(
        finding(&g2, GapSource::IncompletePublishedContract).is_some(),
        "and leaves the description question entirely open — checked is not described"
    );
}

#[test]
fn the_finding_admits_the_characteristic_it_cannot_ask_about() {
    // 🛑 `req:interface-spec-complete` names SIX characteristics; the schema has
    // fields for five. There is nowhere to record performance and constraints —
    // rate limits, concurrency, timeouts — so a design can clear this finding
    // completely and still not say what it promises under load. A detector that
    // quietly checks five sixths of a requirement and reports clean is the
    // green-gate-over-what-it-does-not-cover failure, so it says so in the
    // evidence a reader actually sees.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:api", "The public API").expect("ifc");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");

    let gap = finding(&g, GapSource::IncompletePublishedContract).expect("nothing specified");
    assert!(
        gap.evidence.contains("performance and constraints"),
        "the limit rides on the finding, not only in a doc comment nobody reads: {}",
        gap.evidence
    );
}

#[test]
fn unspecified_is_unset_and_none_is_an_answer() {
    // 🛑 THE MOST IMPORTANT ASSERTION IN THIS FILE, and the detector was WRONG
    // until it was written. Five of the nine agreement axes are enums DEFAULTING
    // to `unspecified`, and the store materialises defaults on write — so every
    // Interface ever created already carries the word. A presence test counts it
    // as an answer, which would have made this detector green over exactly the
    // thing it exists to check.
    //
    // MEASURED ON REFLOW2'S OWN DESIGN before the fix: `ifc:mcp-tools-http` read
    // 9 of 9 complete while its `medium` said `unspecified`, and
    // `ifc:graph-export` reported four gaps where there are six.
    //
    // ⭐ AND THE OTHER HALF IS JUST AS LOAD-BEARING: `none` IS AN ANSWER.
    // `auth: none` on an unauthenticated local pipe is a real, deliberate
    // statement, and sweeping it up with `unspecified` would nag a design that
    // had answered honestly. Declared absence and undeclared absence are
    // different facts — the distinction `Artifact.audience` enforces by having
    // no default at all.
    let mut g = DesignGraph::open_in_memory().expect("open");
    g.add_interface("ifc:api", "The public API").expect("ifc");
    g.set_interface_designation("ifc:api", "published")
        .expect("designation");
    g.set_interface_spec(
        "ifc:api",
        Some("unspecified"), // explicitly saying nothing
        Some("synchronous"),
        Some("json"),
        Some("openapi.yaml"),
        Some("/v1/things"),
        Some("GET"),
        Some("none"), // ...as against an honest "no auth"
        Some("none"),
        Some("problem+json"),
    )
    .expect("spec");

    let gap = finding(&g, GapSource::IncompletePublishedContract)
        .expect("`unspecified` medium is an unanswered axis, not an answered one");
    assert!(
        gap.evidence.contains("medium"),
        "an axis explicitly set to `unspecified` is reported as unset: {}",
        gap.evidence
    );
    assert!(
        !gap.evidence.contains("auth") && !gap.evidence.contains("transport_security"),
        "but `none` is a real answer and must not be reported — a design that said 'no auth' \
         answered the question: {}",
        gap.evidence
    );

    // And the whole finding stands down once the last unanswered axis is named.
    g.set_interface_spec(
        "ifc:api",
        Some("REST"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("spec");
    assert!(
        finding(&g, GapSource::IncompletePublishedContract).is_none(),
        "every axis now carries a real answer"
    );
}
