//! CORPUS — a folder of documents becomes ONE design ([BL-186]).
//!
//! Driven through the real agent handshake rather than a mock backend, because
//! the handshake is the only way a session can reach ingest and the batching
//! across documents is the thing under test.

use reflow2_core::IngestStatus;
use reflow2_core::agent::AgentAnswer;
use reflow2_core::corpus::{CorpusDocument, CorpusOptions, CorpusStep, DocumentStatus};
use reflow2_core::graph::DesignGraph;
use reflow2_core::nodes::node;

/// Two specifications from different teams that name the SAME component the
/// same way — the case `req:corpus-ingest` is really about.
const SPEC_A: &str = "PAYMENTS SPEC. The Auth Service checks every card token before capture.";
const SPEC_B: &str = "REPORTING SPEC. Nightly rollups read from the Auth Service audit log.";
/// A third that names it slightly differently — the ambiguous band.
const SPEC_C: &str = "ONBOARDING SPEC. New sellers are verified by the Authentication Service.";

fn doc(id: &str, title: &str, text: &str) -> CorpusDocument {
    CorpusDocument {
        fragment_id: id.to_string(),
        title: title.to_string(),
        text: text.to_string(),
        source: Some(format!("{title}#L1-L40")),
    }
}

/// Answer a prompt by its pass tag, and — where the pass names entities — by
/// which document's text the prompt carries. This is exactly what an agent does.
fn answer(prompt: &str) -> String {
    let component = if prompt.contains("Authentication Service") {
        r#"{"components":[{"id":"cmp:authentication-service","name":"Authentication Service","purpose":"verifies sellers"}]}"#
    } else {
        r#"{"components":[{"id":"cmp:auth-service","name":"Auth Service","purpose":"checks tokens"}]}"#
    };

    if prompt.contains("[pass:project_intent]") {
        r#"{"project":{"id":"proj:pay","name":"Payments","mode":"flexible"}}"#.to_string()
    } else if prompt.contains("[pass:discovery]") {
        r#"{"components":true,"interfaces":false,"actors":false,"decisions":false,"artifacts":false,"verifications":false,"flows":false,"resources":false}"#.to_string()
    } else if prompt.contains("[pass:components]") {
        component.to_string()
    } else {
        r#"{}"#.to_string()
    }
}

/// Drive a corpus run to completion. Returns the rounds it took and the report.
fn run(
    g: &mut DesignGraph,
    docs: &[CorpusDocument],
    opts: &CorpusOptions,
) -> (usize, Box<reflow2_core::corpus::CorpusReport>) {
    let mut answers: Vec<AgentAnswer> = Vec::new();
    for round in 1..=12 {
        match g.ingest_corpus_step(docs, opts, answers.clone()).unwrap() {
            CorpusStep::NeedsLlm { prompts, .. } => {
                assert!(!prompts.is_empty(), "NeedsLlm must name what it needs");
                for p in prompts {
                    answers.push(AgentAnswer {
                        id: p.id,
                        text: answer(&p.prompt),
                    });
                }
            }
            CorpusStep::Done { report } => return (round, report),
        }
    }
    panic!("corpus handshake did not converge");
}

#[test]
fn a_corpus_becomes_one_design_rather_than_one_per_document() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let docs = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-b", "reporting.md", SPEC_B),
    ];
    let (_, report) = run(&mut g, &docs, &CorpusOptions::default());

    assert_eq!(report.documents_ingested, 2, "{report:?}");
    assert_eq!(report.documents_failed, 0, "{report:?}");

    // THE POINT: two documents naming the same component produced ONE node, not
    // two. Without cross-document resolution this would be 2.
    assert_eq!(
        g.count_nodes(node::COMPONENT).unwrap(),
        1,
        "the same component named in two specs must converge on one node"
    );

    // And each document still has its own provenance Fragment, so the claim is
    // traceable back to the file it came from.
    assert_eq!(g.count_nodes(node::FRAGMENT).unwrap(), 2);
    assert!(g.get_node(node::FRAGMENT, "frag:spec-a").unwrap().is_some());
    assert!(g.get_node(node::FRAGMENT, "frag:spec-b").unwrap().is_some());
}

#[test]
fn the_whole_run_pins_to_one_epoch_not_one_per_document() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let docs = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-b", "reporting.md", SPEC_B),
        doc("frag:spec-c", "onboarding.md", SPEC_C),
    ];
    let opts = CorpusOptions {
        epoch_id: "epoch:the-corpus".to_string(),
        ..CorpusOptions::default()
    };
    let (_, report) = run(&mut g, &docs, &opts);

    assert_eq!(report.epoch_id, "epoch:the-corpus");
    // Left to itself `ingest` opens `epoch:{fragment_id}` per document. Three
    // documents must not read as three unrelated events.
    assert_eq!(
        g.count_nodes(node::DESIGN_EPOCH).unwrap(),
        1,
        "a corpus run is ONE event on the time axis"
    );
    assert!(
        g.get_node(node::DESIGN_EPOCH, "epoch:the-corpus")
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_re_run_skips_what_landed_before_rather_than_failing_it() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let first = vec![doc("frag:spec-a", "payments.md", SPEC_A)];
    let (_, r1) = run(&mut g, &first, &CorpusOptions::default());
    assert_eq!(r1.documents_ingested, 1);

    // The folder grew a file and the whole thing is pointed at reflow2 again —
    // which is what actually happens with a corpus.
    let second = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-b", "reporting.md", SPEC_B),
    ];
    let (_, r2) = run(&mut g, &second, &CorpusOptions::default());

    assert_eq!(r2.documents_skipped, 1, "the already-ingested one: {r2:?}");
    assert_eq!(r2.documents_ingested, 1, "only the new one: {r2:?}");
    assert_eq!(
        r2.documents_failed, 0,
        "already done is NOT a failure — collapsing them is how a half-run \
         reports the same thing as a whole one"
    );
    assert_eq!(
        r2.status,
        IngestStatus::Ok,
        "a clean resume is a clean run, not a degraded one"
    );

    // Resume is DERIVED, not bookmarked: no second Fragment for spec-a.
    assert_eq!(g.count_nodes(node::FRAGMENT).unwrap(), 2);

    let skipped = r2
        .outcomes
        .iter()
        .find(|o| o.fragment_id == "frag:spec-a")
        .expect("every document reports an outcome");
    assert_eq!(skipped.status, DocumentStatus::Skipped);
}

#[test]
fn a_document_that_cannot_be_taken_is_named_and_the_run_continues() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    // The middle document repeats the first's fragment_id — it would overwrite
    // the first's provenance and reopen its epoch (BL-58), so it is refused.
    let docs = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-a", "payments-copy.md", SPEC_B),
        doc("frag:spec-c", "onboarding.md", SPEC_C),
    ];
    let (_, report) = run(&mut g, &docs, &CorpusOptions::default());

    assert_eq!(report.documents_failed, 1, "{report:?}");
    assert_eq!(
        report.documents_ingested, 2,
        "one bad document must not cancel its siblings"
    );
    assert_eq!(report.status, IngestStatus::Partial);

    // NAMED, not just counted — the list this feature exists for.
    assert_eq!(report.failures.len(), 1);
    let failed = &report.failures[0];
    assert_eq!(failed.title, "payments-copy.md");
    assert!(
        failed
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("duplicate fragment_id"),
        "the failure must say WHY: {failed:?}"
    );
    // The locator the agent supplied comes back with it, so a person can go
    // straight to the document rather than searching for it.
    assert_eq!(failed.source.as_deref(), Some("payments-copy.md#L1-L40"));
}

#[test]
fn the_handshake_batches_so_a_corpus_costs_the_rounds_a_document_does() {
    let mut one = DesignGraph::open_in_memory().unwrap();
    let (rounds_for_one, _) = run(
        &mut one,
        &[doc("frag:spec-a", "payments.md", SPEC_A)],
        &CorpusOptions::default(),
    );

    let mut many = DesignGraph::open_in_memory().unwrap();
    let (rounds_for_three, report) = run(
        &mut many,
        &[
            doc("frag:spec-a", "payments.md", SPEC_A),
            doc("frag:spec-b", "reporting.md", SPEC_B),
            doc("frag:spec-c", "onboarding.md", SPEC_C),
        ],
        &CorpusOptions::default(),
    );

    assert_eq!(report.documents_ingested, 3);
    // THE COST PROPERTY: round trips are set by the pipeline's depth, not by how
    // many documents there are. Looping the single-document handshake would make
    // this 3x.
    assert_eq!(
        rounds_for_three, rounds_for_one,
        "three documents must cost the same ROUNDS as one — got {rounds_for_three} vs \
         {rounds_for_one}"
    );
}

#[test]
fn the_first_round_asks_for_every_document_at_once() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let docs = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-b", "reporting.md", SPEC_B),
        doc("frag:spec-c", "onboarding.md", SPEC_C),
    ];
    match g
        .ingest_corpus_step(&docs, &CorpusOptions::default(), Vec::new())
        .unwrap()
    {
        CorpusStep::NeedsLlm {
            documents_pending, ..
        } => {
            assert_eq!(
                documents_pending, 3,
                "round one must gather every document, not walk them one at a time"
            );
        }
        CorpusStep::Done { .. } => panic!("a fresh corpus cannot be done without answers"),
    }
}

#[test]
fn nothing_is_written_until_the_handshake_finishes() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let docs = vec![doc("frag:spec-a", "payments.md", SPEC_A)];

    // One prepare round, then abandon — the shape of a session that dies or a
    // user who changes their mind.
    let _ = g
        .ingest_corpus_step(&docs, &CorpusOptions::default(), Vec::new())
        .unwrap();

    assert_eq!(
        g.count_nodes(node::FRAGMENT).unwrap(),
        0,
        "an abandoned corpus must leave NO half-design behind"
    );
    assert_eq!(g.count_nodes(node::COMPONENT).unwrap(), 0);
    assert_eq!(g.count_nodes(node::DESIGN_EPOCH).unwrap(), 0);
}

#[test]
fn every_document_reports_an_outcome_even_when_it_did_nothing() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    let docs = vec![
        doc("frag:spec-a", "payments.md", SPEC_A),
        doc("frag:spec-b", "reporting.md", SPEC_B),
    ];
    let (_, report) = run(&mut g, &docs, &CorpusOptions::default());

    assert_eq!(report.documents_total, 2);
    assert_eq!(report.outcomes.len(), 2, "no document is silently dropped");
    for outcome in &report.outcomes {
        assert!(
            !outcome.title.is_empty(),
            "an outcome a person cannot identify is not a report"
        );
    }
    // The counts must actually add up — a report whose parts disagree with its
    // total is worse than no report.
    assert_eq!(
        report.documents_ingested + report.documents_skipped + report.documents_failed,
        report.documents_total
    );
}
