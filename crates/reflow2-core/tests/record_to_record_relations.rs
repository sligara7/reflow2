//! Relationships BETWEEN records are modelled, not wildcard leftovers.
//!
//! `req:a-record-can-say-what-runs-it-what-complements-it-and-what-replaced-it`,
//! `cap:record-to-record-relations-are-modelled`.
//!
//! # The class, named three times before anyone looked for a pattern
//!
//! dev_storyflow filed three independent reports across four days, each about a
//! different pair, none of them connecting the three: `Artifact → Verification`
//! had no edge type at all; `SUPERSEDES` was refused between two Verifications
//! while its own NAME describes the relation exactly; and two DesignRules that
//! deliberately stand beside each other could only be joined by an edge that
//! invites the merge they must never have. The golden thread — Requirement ←
//! Capability ← Component ← Artifact — is richly modelled. Record-to-record is
//! where the vocabulary ran out.
//!
//! # Each of these has a reader, and that is the entry fee
//!
//! `dec:edge-orthogonality`: a vocabulary distinction earns its keep ONLY IF A
//! COMPUTATION READS IT. Two further candidates raised the same day —
//! `ChangeEvent → Decision` and a governs-which-repo property — were DECLINED
//! for failing exactly this test, so these three had to pass it visibly. The
//! tests below are that proof: each asserts on the computation, not on the edge
//! being writable.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

// ─────────────────────────────────────────────────────────────────────────────
// 1. IMPLEMENTS — a check can name the file that runs it
// ─────────────────────────────────────────────────────────────────────────────

/// A design with two checks: one has a script, one has nothing to run.
fn two_checks_one_scripted() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_verification(
        "ver:scripted",
        "The plot-thread stub matches the design",
        None,
        None,
        None,
    )
    .unwrap();
    g.add_verification(
        "ver:prose-only",
        "The audit conclusions still hold",
        None,
        None,
        None,
    )
    .unwrap();
    g.add_artifact(
        "art:the-script",
        "check_plot_thread_stub_matches_design.py",
        Some("code"),
        Some("tools/check_plot_thread_stub.py"),
    )
    .unwrap();
    g.create_edge(
        edge::IMPLEMENTS,
        node::ARTIFACT,
        "art:the-script",
        node::VERIFICATION,
        "ver:scripted",
        Props::new(),
    )
    .unwrap();
    g
}

#[test]
fn a_check_with_a_script_is_distinguishable_from_one_with_nothing_to_run() {
    // THE WHOLE POINT. Before this edge existed both of these were simply
    // "never run" — one number covering a scheduling problem and a check that
    // exists only as a sentence.
    let g = two_checks_one_scripted();
    let recency = g.verification_recency().unwrap();

    let scripted = recency
        .iter()
        .find(|v| v.verification_id == "ver:scripted")
        .expect("the scripted check is in the roll");
    let prose = recency
        .iter()
        .find(|v| v.verification_id == "ver:prose-only")
        .expect("the prose-only check is in the roll");

    assert!(
        scripted.has_executable_form,
        "an incoming IMPLEMENTS is what says a check has something to run"
    );
    assert!(
        !prose.has_executable_form,
        "a check nothing implements has no executable form, and that is the finding"
    );
}

#[test]
fn the_edge_is_a_modelled_fit_not_a_tolerated_wildcard() {
    // `req:...` names this explicitly, and it is the reason a wildcard was
    // refused: a wildcard would ACCEPT the pair and never MODEL it, so
    // describe_schema would go on ranking it merely tolerated and no detector
    // would treat it as real. Assert the declared endpoints, so widening
    // IMPLEMENTS to `*` later fails here rather than silently.
    let g = DesignGraph::open_in_memory().unwrap();
    let schema = g.schema();
    let e = schema
        .edge_types
        .get(edge::IMPLEMENTS)
        .expect("IMPLEMENTS is declared");
    // A wildcard accepts EVERYTHING, so proving it rejects an unrelated type is
    // what distinguishes a modelled fit from a tolerated one.
    assert!(
        e.from.accepts(node::ARTIFACT),
        "IMPLEMENTS accepts an Artifact source"
    );
    assert!(
        !e.from.accepts(node::REQUIREMENT),
        "IMPLEMENTS must NOT be a wildcard source — a wildcard would accept a Requirement"
    );
    assert!(
        e.to.accepts(node::VERIFICATION),
        "IMPLEMENTS accepts a Verification target"
    );
    assert!(
        !e.to.accepts(node::REQUIREMENT),
        "IMPLEMENTS must NOT be a wildcard target — a wildcard would accept a Requirement"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. COMPLEMENTS — two rules that must never be merged
// ─────────────────────────────────────────────────────────────────────────────

/// Two governance rules that deliberately stand beside each other, joined by a
/// DUPLICATES edge somebody drew — the exact situation that destroys one of
/// them. `park` decides whether the COMPLEMENTS edge protecting them exists.
fn two_rules_somebody_called_duplicates(protected: bool) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    for (id, name) in [
        ("rule:claims-are-bounded", "What you may CLAIM is bounded"),
        (
            "rule:truth-is-bounded",
            "What must be TRUE whether or not anyone claims it",
        ),
    ] {
        g.create_node(
            node::DESIGN_RULE,
            id,
            Props::new().set("name", name).set("statement", name),
        )
        .unwrap();
    }
    if protected {
        g.create_edge(
            edge::COMPLEMENTS,
            node::DESIGN_RULE,
            "rule:claims-are-bounded",
            node::DESIGN_RULE,
            "rule:truth-is-bounded",
            Props::new().set(
                "evidence",
                "One binds what you may claim, the other what must be true whether or not \
                 anyone claims anything. Merging destroys the distinction.",
            ),
        )
        .unwrap();
    }
    g.create_edge(
        edge::DUPLICATES,
        node::DESIGN_RULE,
        "rule:claims-are-bounded",
        node::DESIGN_RULE,
        "rule:truth-is-bounded",
        Props::new().set("basis", "asserted"),
    )
    .unwrap();
    g
}

#[test]
fn heal_refuses_to_merge_a_pair_declared_complementary() {
    // 🛑 THE LOAD-BEARING TEST. A merge is irreversible and there is no undo.
    // Before this edge, the only thing standing between two complementary rules
    // and a deletion was a paragraph somebody wrote asking readers not to.
    let g = two_rules_somebody_called_duplicates(true);
    let proposal = g.propose_heal(Default::default()).unwrap();
    let merges: Vec<_> = proposal
        .operations
        .iter()
        .filter(|o| format!("{o:?}").contains("Merge"))
        .collect();
    assert!(
        merges.is_empty(),
        "a COMPLEMENTS pair must never be proposed for merge, got: {merges:?}"
    );
}

#[test]
fn without_the_edge_the_same_pair_is_proposed_for_merge() {
    // ⭐ PROVES THE TEST ABOVE IS NOT INERT. If HEAL never merged two
    // DesignRules for some unrelated reason, the guard would pass vacuously and
    // say nothing. Same graph, one edge removed, opposite answer.
    let g = two_rules_somebody_called_duplicates(false);
    let proposal = g.propose_heal(Default::default()).unwrap();
    let merges: Vec<_> = proposal
        .operations
        .iter()
        .filter(|o| format!("{o:?}").contains("Merge"))
        .collect();
    assert!(
        !merges.is_empty(),
        "without COMPLEMENTS the pair IS a merge candidate — otherwise the guard proves nothing"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. superseded — a retired check stops reporting live coverage
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn a_superseded_check_does_not_count_as_passing_coverage() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_verification("ver:narrow", "The old one-off probe", None, None, None)
        .unwrap();
    g.set_verification_status("ver:narrow", "passing", None, None)
        .unwrap();
    let before = g
        .verification_recency()
        .unwrap()
        .iter()
        .filter(|v| v.status == "passing")
        .count();
    assert_eq!(before, 1, "it passes before it is retired");

    g.set_verification_status("ver:narrow", "superseded", None, None)
        .unwrap();
    let after = g
        .verification_recency()
        .unwrap()
        .iter()
        .filter(|v| v.status == "passing")
        .count();
    assert_eq!(
        after, 0,
        "a superseded check must stop counting as live coverage — reporting a retired \
         check as passing is the misstatement this status value exists to end"
    );
}

#[test]
fn supersedes_joins_two_verifications() {
    // The edge whose NAME describes the relation exactly used to be refused for
    // this pair, and the honest fallback (EVOLVES_INTO, through a double
    // wildcard) could not be told apart from "these two happen to be linked".
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g.add_verification("ver:broad", "One table-driven check", None, None, None)
        .unwrap();
    g.add_verification("ver:narrow", "An old one-off probe", None, None, None)
        .unwrap();
    g.create_edge(
        edge::SUPERSEDES,
        node::VERIFICATION,
        "ver:broad",
        node::VERIFICATION,
        "ver:narrow",
        Props::new(),
    )
    .expect("SUPERSEDES must accept Verification -> Verification");
}

#[test]
fn widening_supersedes_did_not_strand_its_original_pair() {
    // ⚠️ THE TRAP THE VERIFIES COMMENT RECORDS, checked rather than assumed:
    // changing an endpoint list can make already-written edges unimportable.
    // This was a WIDENING, so Fragment -> Fragment must still be legal.
    let g = DesignGraph::open_in_memory().unwrap();
    let e = g
        .schema()
        .edge_types
        .get(edge::SUPERSEDES)
        .expect("SUPERSEDES is declared");
    assert!(
        e.from.accepts(node::FRAGMENT) && e.to.accepts(node::FRAGMENT),
        "the original Fragment pair must survive the widening"
    );
    assert!(
        e.from.accepts(node::VERIFICATION) && e.to.accepts(node::VERIFICATION),
        "and the new Verification pair must be accepted"
    );
}
