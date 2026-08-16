//! "Which of my rules have no detector?" — a question the graph could not be asked.
//!
//! Proposed by dev_storyflow's api-boss on 2026-08-08, built on Anthony's own
//! governance-as-design framing, and MEASURED on reflow2's own design the same
//! day: five `DesignRule`s, FOUR carrying `enforced: true` — which the schema
//! defines as "violations are gate-blocking" — and **not one of the five** with
//! an incoming `VERIFIES`. Four rules that can fail a build, zero detectors, and
//! no report that named it.
//!
//! ## The mechanism was the schema, not the detector
//!
//! `schema/verify.yaml` declared `VERIFIES` as `from: Verification, to: "*"`.
//! The wildcard ACCEPTED a DesignRule target — nothing was ever broken — but it
//! did not MODEL one, so `describe_schema` ranked the pair as merely tolerated
//! and no detector treated a rule as a verifiable thing. The cost of the
//! wildcard was not a wrong edge. It was an unaskable question.
//!
//! Their evidence is stronger than ours and is the reason this is not a tidiness
//! fix: in ONE session that fleet found five rules with no check — a routing
//! rule naming a file that names no worker entry point, a verification step that
//! false-negatives on a shared tree, a comms rule whose quoted example is a
//! forgeable handshake, a claim protocol with no detector for its own bypass,
//! and a detector built for that bypass that was blind to it. Their words:
//! *"Every one is a RULE WITHOUT A CHECK, and no report could have listed them,
//! because the graph cannot currently be asked."*
//!
//! ## What is deliberately NOT claimed here
//!
//! Their own counter-argument, which must survive into the build: **"a graph
//! node green-washes exactly like a document"** — proven twice in their graph,
//! where nine directory artifacts swallowed 373 files while the check read
//! green. Attaching a passing Verification silences this gap, and a passing
//! check that tests nothing is still a lie the graph cannot see. This detector
//! makes the question ASKABLE. It does not certify the answer.

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, node};

/// A minimal design with one rule, so the assertions are about the rule under
/// test rather than fixture noise.
fn base_with_rule(id: &str, enforced: Option<bool>) -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    let mut props = Props::new()
        .set("name", "Every change lands through a pull request")
        .set("statement", "Nothing is pushed straight to main.");
    if let Some(e) = enforced {
        props = props.set("enforced", e);
    }
    g.create_node(node::DESIGN_RULE, id, props).unwrap();
    g
}

fn rule_gap_ids(g: &DesignGraph) -> Vec<String> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|c| c.gap_source == reflow2_core::detect::GapSource::UnverifiedEnforcedRule)
        .flat_map(|c| c.affected_ids)
        .collect()
}

/// THE DEFECT, reproduced: a gate-blocking rule nothing can detect a violation
/// of, and before this the graph had no way to say so.
#[test]
fn an_enforced_rule_with_no_check_is_reported() {
    let g = base_with_rule("rule:prs-only", Some(true));
    assert!(
        rule_gap_ids(&g).contains(&"rule:prs-only".to_string()),
        "a rule whose violations are gate-blocking, with nothing able to detect one, must be \
         asked about — this is the question that was structurally unaskable"
    );
}

fn unstated_gap_ids(g: &DesignGraph) -> Vec<String> {
    g.detect_gaps()
        .unwrap()
        .into_iter()
        .filter(|c| c.gap_source == reflow2_core::detect::GapSource::UnstatedRuleEnforcement)
        .flat_map(|c| c.affected_ids)
        .collect()
}

/// THE READING THAT CHANGED, and it read the other way round for exactly one
/// day. `enforced` used to default to true, so a rule nobody had thought about
/// was BILLED FOR A DETECTOR — which is how all four of reflow2's own enforced
/// rules got that way, none of them chosen. The default is gone
/// (dec:does-enforced-default-to-gate-blocking): absence now means nobody has
/// said, and is not read as either answer.
#[test]
fn a_rule_that_never_mentioned_enforcement_is_not_billed_for_a_detector() {
    let g = base_with_rule("rule:unstated", None);
    assert!(
        !rule_gap_ids(&g).contains(&"rule:unstated".to_string()),
        "an unstated rule must not owe a check nobody agreed to; only an explicit `true` is billed"
    );
}

/// But it is NOT silently let off either — that would just move the unchosen
/// claim to the other side. It is ASKED, at a lower severity, because deciding
/// what a rule is costs a word while proving one costs a detector.
#[test]
fn an_unstated_rule_is_asked_which_it_is() {
    let g = base_with_rule("rule:unstated", None);
    assert!(
        unstated_gap_ids(&g).contains(&"rule:unstated".to_string()),
        "absent must mean `nobody has said` and be asked, not quietly read as advisory"
    );
}

/// The two findings must not both fire on one rule: they are different
/// questions and a rule can only be in one of the three states.
#[test]
fn a_stated_rule_is_never_also_reported_as_unstated() {
    for stated in [true, false] {
        let g = base_with_rule("rule:stated", Some(stated));
        assert!(
            !unstated_gap_ids(&g).contains(&"rule:stated".to_string()),
            "a rule that stated `{stated}` has said which it is"
        );
    }
}

/// The counterweight that keeps the list able to reach zero. An advisory rule is
/// guidance, and demanding a detector for guidance is how `unverified_artifact`
/// became 22 of 25 gaps and had to be retired.
#[test]
fn an_advisory_rule_is_not_asked_about() {
    let g = base_with_rule("rule:style", Some(false));
    assert!(
        rule_gap_ids(&g).is_empty(),
        "an explicitly advisory rule has no obligation to be detectable; a detector that fires \
         on correct work teaches you to skim it"
    );
}

/// `dec:passing-is-verified`. Attaching a check that has never passed must not
/// buy silence, or this detector becomes the green-washing its own proposal
/// warned about.
#[test]
fn a_planned_check_does_not_silence_the_question() {
    let mut g = base_with_rule("rule:prs-only", Some(true));
    g.add_verification("ver:someday", "We will grep for it one day", None, None)
        .unwrap();
    g.verifies("ver:someday", node::DESIGN_RULE, "rule:prs-only")
        .unwrap();

    assert!(
        rule_gap_ids(&g).contains(&"rule:prs-only".to_string()),
        "a `planned` check detects nothing; only a passing one closes this"
    );
}

// ---------------------------------------------------------------------------
// The sibling that makes the family self-seeding.
//
// Everything above fires on a rule that ALREADY EXISTS. On a design where
// nobody ever wrote one down they are all silent, so governance would never
// surface at all — Anthony's question when he read the first draft, and he was
// right: a gap that only speaks about recorded things cannot ask for the thing
// itself.
// ---------------------------------------------------------------------------

fn governance_rollup_fires(g: &DesignGraph) -> bool {
    g.detect_gaps()
        .unwrap()
        .iter()
        .any(|c| c.gap_source == reflow2_core::detect::GapSource::BuildWithoutGovernance)
}

/// A design with real files and no recorded conventions. Those files already
/// follow rules — a branching rule, a review step, a house style — and nothing
/// says what they are. This is the adopt case, where the rules exist in the code
/// and only the record is missing.
#[test]
fn a_build_with_no_adopted_conventions_is_asked_about() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .unwrap();
    g.realizes("art:a", node::CAPABILITY, "cap:a", None, None)
        .unwrap();

    assert!(
        governance_rollup_fires(&g),
        "files exist, so conventions exist; the design recording none of them is the question"
    );
}

/// THE COUNTERWEIGHT THAT DECIDED THE TRIGGER. At genesis there is intent and no
/// build — no convention has been chosen yet, and asking gets a shrug. Keying
/// this on artifacts rather than components is what keeps it from firing on
/// every design on its first day.
#[test]
fn a_design_with_no_build_yet_is_not_asked_about() {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_requirement("req:r", "R", "need r").unwrap();
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_component("cmp:c", "C", "part c", None).unwrap();

    assert!(
        !governance_rollup_fires(&g),
        "a design on paper has not chosen how it will be built; asking now is asking too early"
    );
}

/// And it can reach zero. One recorded convention answers it — the question is
/// "what conventions?", not "have you written enough of them?".
#[test]
fn one_recorded_convention_answers_the_rollup() {
    let mut g = base_with_rule("rule:prs-only", Some(false));
    g.add_capability("cap:a", "Cap A", "does a", None).unwrap();
    g.add_artifact("art:a", "a.rs", Some("code"), Some("src/a.rs"))
        .unwrap();
    g.realizes("art:a", node::CAPABILITY, "cap:a", None, None)
        .unwrap();

    assert!(
        !governance_rollup_fires(&g),
        "a detector that cannot reach zero teaches you to skim it"
    );
}

/// And the close: a passing check answers it. This also pins that the edge is
/// legal at all — `VERIFIES` now ENUMERATES DesignRule rather than tolerating it
/// through a `*` wildcard, which is what made the question expressible.
#[test]
fn a_passing_check_closes_it() {
    let mut g = base_with_rule("rule:prs-only", Some(true));
    g.add_verification("ver:branch-guard", "CI refuses a push to main", None, None)
        .unwrap();
    g.verifies("ver:branch-guard", node::DESIGN_RULE, "rule:prs-only")
        .unwrap();
    g.set_verification_status("ver:branch-guard", "passing", None)
        .unwrap();

    assert!(
        rule_gap_ids(&g).is_empty(),
        "a rule with a passing detector is answered"
    );
}
