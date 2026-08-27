//! Does the design know whose vocabulary to speak in?
//!
//! `dec:idea-a-user-carries-a-persona-that-shapes-every-reply` records the
//! standing defect: reflow2's own words — gap, loop, detector, node id — reach
//! the person, and TWO users independently invented the same workaround of
//! asking the agent to drop them. The v0.36.0 increment answered it in served
//! prose, which has a failure mode this project measured twice in one week: it
//! is read once and drifts, and nothing observes whether it held.
//!
//! `reader_lens` is the computed half. These tests pin the two properties that
//! make it worth attaching to a response rather than writing down again:
//! **it reports ABSENCE**, and **it never claims to know who is at the keyboard.**

use reflow2_core::DesignGraph;

fn graph() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:p", "P").unwrap();
    g
}

/// ⭐ THE CASE THE WHOLE THING EXISTS FOR. A design nobody has described
/// themselves in cannot say whose words to use — and today that silence is
/// invisible, because every detector reasons over what EXISTS and none asks
/// whether something was never used at all.
#[test]
fn a_design_with_no_recorded_background_says_so_rather_than_going_quiet() {
    let mut g = graph();
    g.add_contributor("who:ann", "Ann", Some("person"), None, None)
        .unwrap();

    let lens = g.reader_lens().unwrap();

    assert!(
        lens.is_silent(),
        "a design where nobody carries a background must report itself silent, \
         not merely return an empty list nobody reads"
    );
    assert_eq!(lens.with_background, Vec::<String>::new());
    assert_eq!(
        lens.without_background,
        vec!["who:ann".to_string()],
        "and it must name WHO could be asked — a bare count is not actionable"
    );
}

/// An empty design is silent too, and for the same reason. Stated separately
/// because "no contributors" and "contributors with nothing recorded" are
/// different facts that must not collapse into one reassuring answer.
#[test]
fn an_empty_design_is_silent_and_names_nobody() {
    let g = graph();
    let lens = g.reader_lens().unwrap();
    assert!(lens.is_silent());
    assert!(lens.without_background.is_empty());
}

/// The ordinary case: some people described, some not. BOTH lists matter —
/// the second is what makes the first honest, because a design that names two
/// described readers while hiding three undescribed ones invites the agent to
/// assume it knows the room.
#[test]
fn it_separates_who_is_described_from_who_is_merely_present() {
    let mut g = graph();
    g.add_contributor("who:ann", "Ann", Some("person"), None, Some("Vet. Cattle."))
        .unwrap();
    g.add_contributor("who:bob", "Bob", Some("person"), None, None)
        .unwrap();

    let lens = g.reader_lens().unwrap();

    assert!(!lens.is_silent());
    assert_eq!(lens.with_background, vec!["who:ann".to_string()]);
    assert_eq!(lens.without_background, vec!["who:bob".to_string()]);
}

/// ⭐ AN AUTOMATED AGENT IS NOT A READER. Its `description` is provenance about
/// a tool, and counting it would let a design claim it knows its audience when
/// it only knows its authors — the exact false-confidence this is meant to end.
#[test]
fn an_automated_agent_is_never_counted_as_someone_to_speak_to() {
    let mut g = graph();
    g.add_contributor(
        "who:claude",
        "Claude Code",
        Some("automated_agent"),
        None,
        Some("Writes the design under a person's direction."),
    )
    .unwrap();

    let lens = g.reader_lens().unwrap();

    assert!(
        lens.is_silent(),
        "a described BOT must leave the design silent about its human reader"
    );
    assert!(lens.with_background.is_empty());
    assert!(
        lens.without_background.is_empty(),
        "and it is not askable either — it must not appear in the ask list"
    );
}

/// `kind` is optional and predates much of the graph, so silence about it must
/// read as "probably a person". Treating unset as not-a-person would hide
/// exactly the contributors most likely to be human.
#[test]
fn a_contributor_with_no_kind_is_treated_as_a_person() {
    let mut g = graph();
    g.add_contributor(
        "who:old",
        "Old Record",
        None,
        None,
        Some("Beamline scientist."),
    )
    .unwrap();

    let lens = g.reader_lens().unwrap();

    assert!(!lens.is_silent());
    assert_eq!(lens.with_background, vec!["who:old".to_string()]);
}

/// Whitespace is not a background. A description of `"  "` satisfies "has a
/// description" and tells a reader nothing, which is the shape of answer this
/// whole line of work exists to stop accepting — the same reason the served ask
/// stopped accepting "Bob".
#[test]
fn a_blank_description_does_not_count_as_a_background() {
    let mut g = graph();
    g.add_contributor("who:ann", "Ann", Some("person"), None, Some("   "))
        .unwrap();

    let lens = g.reader_lens().unwrap();

    assert!(lens.is_silent(), "whitespace must not read as a background");
    assert_eq!(lens.without_background, vec!["who:ann".to_string()]);
}
