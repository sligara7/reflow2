//! Two designs under one root can carry the SAME `graph_id`, and the registry
//! silently serves one of them.
//!
//! # Found on Anthony's own machine, 2026-08-12, by looking rather than by testing
//!
//! Ten reflow2 projects live under `~/project`. Reading their identity sidecars —
//! without opening a single store — turned up two that answer to the same name:
//!
//! ```text
//! ~/project/reflow2/.reflow2/graph.id.json
//!   { "graph_id": "reflow2", "label": "reflow2",       "minted_by": "0.13.0" }
//! ~/project/dev_storyflow/.reflow2/graph.id.json
//!   { "graph_id": "reflow2", "label": "dev_storyflow", "minted_by": "0.14.0" }
//! ```
//!
//! Both are real, both are populated, both were `adopted` rather than minted, and
//! the collision has been sitting there since 0.14.0.
//!
//! # Why this is the registry's problem and not a tidiness problem
//!
//! `rule:a-design-is-named-by-an-id-not-a-path` makes `graph_id` THE name — "the
//! id is primary and the path is a storage detail". `Registry::attach` maps id to
//! path and refuses anything it does not hold, and `ver:a-session-cannot-name-another-design`
//! passes on that basis. **Every one of those guarantees assumes ids are unique,
//! and nothing anywhere enforces or checks that.**
//!
//! `Registry::discover` accumulates into a `BTreeMap<String, String>` keyed by
//! id. A second design with the same id does not collide, does not warn, and does
//! not fail — it OVERWRITES, and since the directory list is sorted, which one
//! survives is decided by alphabetical order of a path the id exists to hide.
//!
//! ⇒ So `attach("reflow2")` returns a Binding to whichever store sorted last, and
//! the other design is unreachable by its own name. That is the accident this
//! whole thread began with — writing to the wrong design — arriving through the
//! mechanism built to prevent it.
//!
//! # What this file does NOT decide
//!
//! Whether the answer is to refuse the ambiguous id, to report the collision and
//! serve neither, or to make ids unique at mint time. All three are open and are
//! Anthony's. This pins only the FACT, so that whatever is chosen has to face it.

use reflow2_mcp::registry::Registry;

struct Root {
    dir: std::path::PathBuf,
}

impl Root {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "reflow2-collision-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("root");
        Self { dir }
    }

    /// A real design under `name`, carrying whatever `graph_id` is asked for —
    /// which is exactly what two `adopted` projects did on the real machine.
    fn design_with_id(&self, dir_name: &str, graph_id: &str, label: &str) {
        let store = self.dir.join(dir_name).join(".reflow2").join("graph");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(
            self.dir
                .join(dir_name)
                .join(".reflow2")
                .join("graph.id.json"),
            format!(
                "{{\n  \"graph_id\": \"{graph_id}\",\n  \"label\": \"{label}\",\n  \
                 \"origin\": \"adopted\",\n  \"minted_by\": \"0.14.0\"\n}}\n"
            ),
        )
        .unwrap();
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// 🛑 THE DEFECT. Two designs, one id. The registry reports ONE entry and never
// says the other exists — a silent drop, which rule 6 forbids everywhere else.
#[test]
#[ignore = "demonstrates an unfixed defect; the fix is undecided — see dec:idea-what-to-do-about-two-designs-answering-to-one-id"]
fn two_designs_sharing_an_id_are_not_both_reachable() {
    let root = Root::new("dup");
    root.design_with_id("alpha", "reflow2", "reflow2");
    root.design_with_id("zulu", "reflow2", "dev_storyflow");

    let r = Registry::discover(root.dir.to_str().unwrap());

    assert_eq!(
        r.graph_ids().len(),
        2,
        "two designs exist under this root; the registry reports {} — the second was \
         silently overwritten in a map keyed by an id nothing keeps unique",
        r.graph_ids().len()
    );
}

// 🛑 AND THE CONSEQUENCE THAT MATTERS: attaching by the shared name resolves to
// whichever path sorted last, so the design a session reaches is decided by
// alphabetical order of the very thing the id exists to hide.
#[test]
#[ignore = "demonstrates an unfixed defect; the fix is undecided — see dec:idea-what-to-do-about-two-designs-answering-to-one-id"]
fn attaching_by_a_shared_id_does_not_silently_pick_one() {
    let root = Root::new("attach-dup");
    root.design_with_id("alpha", "reflow2", "reflow2");
    root.design_with_id("zulu", "reflow2", "dev_storyflow");

    let r = Registry::discover(root.dir.to_str().unwrap());

    match r.attach("reflow2") {
        Ok(bound) => panic!(
            "an ambiguous id must not resolve silently — it bound to {} and the other \
             design is unreachable by its own name",
            bound.graph_path()
        ),
        Err(_) => { /* a refusal that names the ambiguity is the acceptable shape */ }
    }
}
