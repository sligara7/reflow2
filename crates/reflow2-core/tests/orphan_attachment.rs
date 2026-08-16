//! BL-176 — `orphan_node` and what counts as ATTACHING an Artifact.
//!
//! The rule used to be `REALIZES` and nothing else, so an Artifact filed
//! exactly the way the served **link-artifacts** skill prescribes — a design
//! doc, ADR, README, runbook or agent-instruction file linked with `DOCUMENTS`,
//! an OpenAPI/IDL contract linked with `SPECIFIES` — read as *"realizes
//! nothing"*. The message was true and the CATEGORY was false: an orphan is a
//! node attached to nothing, and those are attached.
//!
//! MEASURED IN THE FIELD, which is why this is a fix and not a preference: on a
//! real corpus, registering 26 architecture/ADR documents took structural
//! defects from 13 to 39 — **+26, exactly the batch size, every correctly-filed
//! document becoming a defect** — and the false-positive rate from 46% to 82%,
//! with ~730 documents still to go. The reporter stopped work rather than
//! continue, and refused the available workaround (adding a bogus `REALIZES`)
//! because it would be a lie at 756x scale.
//!
//! THE COUNTERWEIGHTS ARE THE POINT, and they are the last three tests here.
//! An Artifact attached by NOTHING must still fire — that distinction is the
//! whole value of the detector, and a fix that silenced it would have traded a
//! false positive for a false negative. In particular the rule is deliberately
//! NOT degree-zero: `INCLUDES` from a Release is bookkeeping, not design
//! attachment, so it must not silence anything.

use reflow2_core::nodes::{Props, edge, node};
use reflow2_core::{DesignGraph, HealCategory};

/// A design with one Component to attach things to.
fn base() -> DesignGraph {
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_project("proj:x", "X").unwrap();
    g.add_component("cmp:core", "Core", "the core crate", None)
        .unwrap();
    g
}

/// Every `orphan_node` finding in the graph, by the id it names.
fn orphans(g: &DesignGraph) -> Vec<String> {
    g.detect_defects()
        .unwrap()
        .into_iter()
        .filter(|i| i.category == HealCategory::OrphanNode)
        .flat_map(|i| i.affected_ids)
        .collect()
}

// ---- the defect cases: correctly-filed artifacts that used to fire ---------

/// BL-176's exact case. The **link-artifacts** skill says a design doc, ADR,
/// README, runbook or agent-instruction file *"describes the design instead:
/// register it with `add_artifact` (artifact_type `document`) and link it with
/// `documents` (+ `doc_kind`), **not** `realizes`"*. Following that instruction
/// literally used to produce a warning.
#[test]
fn a_document_attached_by_documents_is_not_an_orphan() {
    let mut g = base();
    g.add_artifact(
        "art:agents-md",
        "AGENTS.md",
        Some("document"),
        Some("AGENTS.md"),
    )
    .unwrap();
    g.documents(
        "art:agents-md",
        node::COMPONENT,
        "cmp:core",
        Some("agent_instructions"),
    )
    .unwrap();

    assert!(
        !orphans(&g).contains(&"art:agents-md".to_string()),
        "a document attached by DOCUMENTS is attached — the skill prescribes \
         exactly this and it must not read as an orphan"
    );
}

/// The field's other half: six service OpenAPI contracts, each carrying
/// `SPECIFIES -> ifc:<service>_api`, all six reported as orphans. A contract
/// *specifies* an interface; it does not *implement* anything, so there is no
/// true `REALIZES` available and the only way to clear the finding was to
/// assert one that is false.
#[test]
fn a_spec_attached_by_specifies_is_not_an_orphan() {
    let mut g = base();
    g.add_interface("ifc:auth-api", "auth API").unwrap();
    g.add_artifact(
        "art:auth-openapi",
        "auth.openapi.yaml",
        Some("spec"),
        Some("contracts/auth.openapi.yaml"),
    )
    .unwrap();
    g.create_edge(
        edge::SPECIFIES,
        node::ARTIFACT,
        "art:auth-openapi",
        node::INTERFACE,
        "ifc:auth-api",
        Props::new(),
    )
    .unwrap();

    assert!(
        !orphans(&g).contains(&"art:auth-openapi".to_string()),
        "a contract attached by SPECIFIES is attached"
    );
}

// ---- the counterweights: what must STILL fire -----------------------------

/// THE ONE THAT KEEPS THE DETECTOR WORTH HAVING. An Artifact nobody attached to
/// anything is a real orphan, and the fix must not have bought quiet by
/// dropping the finding.
#[test]
fn an_artifact_attached_by_nothing_is_still_an_orphan() {
    let mut g = base();
    g.add_artifact("art:loose", "loose.rs", Some("code"), Some("src/loose.rs"))
        .unwrap();

    assert!(
        orphans(&g).contains(&"art:loose".to_string()),
        "an artifact attached to nothing must still fire — that distinction is \
         the whole value of the rule"
    );
}

/// A document that documents nothing is ALSO still an orphan. BL-176 was
/// explicit that *"this doc describes nothing either" IS worth saying*, so the
/// fix must key on the presence of an attaching edge, never on the artifact's
/// `artifact_type` alone.
#[test]
fn a_document_that_documents_nothing_is_still_an_orphan() {
    let mut g = base();
    g.add_artifact(
        "art:stray-doc",
        "stray.md",
        Some("document"),
        Some("docs/stray.md"),
    )
    .unwrap();

    assert!(
        orphans(&g).contains(&"art:stray-doc".to_string()),
        "a document artifact with no DOCUMENTS edge describes nothing and is \
         still an orphan — the type must not be a free pass"
    );
}

/// The rule is deliberately NOT degree-zero. `INCLUDES` from a Release is
/// release bookkeeping, not design attachment: almost every artifact in a
/// mature graph carries one, so accepting any edge at all would silence the
/// detector everywhere. This is where the Artifact rule correctly differs from
/// the Decision rule in the same detector, which IS degree-zero.
#[test]
fn a_release_include_does_not_silence_the_orphan_rule() {
    let mut g = base();
    g.add_artifact("art:loose", "loose.rs", Some("code"), Some("src/loose.rs"))
        .unwrap();
    g.add_release("rel:v1", "v1.0.0", Some("1.0.0"), None)
        .unwrap();
    g.create_edge(
        edge::INCLUDES,
        node::RELEASE,
        "rel:v1",
        node::ARTIFACT,
        "art:loose",
        Props::new(),
    )
    .unwrap();

    assert!(
        orphans(&g).contains(&"art:loose".to_string()),
        "a Release INCLUDES edge is bookkeeping, not attachment — it must not \
         silence the rule, or the detector goes quiet on a mature graph"
    );
}

/// Unchanged behaviour, kept as a regression guard: the original case the
/// detector was built for still passes.
#[test]
fn a_code_artifact_that_realizes_is_not_an_orphan() {
    let mut g = base();
    g.add_capability("cap:x", "Cap X", "does x", None).unwrap();
    g.add_artifact("art:x", "x.rs", Some("code"), Some("src/x.rs"))
        .unwrap();
    g.realizes("art:x", node::CAPABILITY, "cap:x", None, None)
        .unwrap();

    assert!(
        !orphans(&g).contains(&"art:x".to_string()),
        "REALIZES must keep working exactly as before"
    );
}
