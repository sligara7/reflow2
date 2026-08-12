//! A session names the design it wants by `graph_id`, and cannot name another.
//!
//! # ⭐ WRITTEN BEFORE `registry.rs` EXISTED
//!
//! `cap:select-graph-by-id` is the second half of `req:a-session-chooses-its-design`
//! (accepted). The first half — discovering what designs exist without opening
//! them — shipped as `describe_designs` / `cap:describe-design-at-path`. This is
//! the half that lets a session be POINTED at one.
//!
//! # The two clauses this file exists to cover
//!
//! `crates/reflow2-mcp/tests/sessions_cannot_cross_designs.rs` carried a
//! TRIPWIRE that failed the moment `registry.rs` appeared, because the risk was
//! never the missing code — it was the missing test:
//!
//! > *"the registry lands, the green tests above are taken as the precondition
//! > met, and the two clauses that motivated the condition are never written."*
//!
//! Those clauses, from `ver:a-session-cannot-name-another-design`, are:
//!
//! 1. **a `graph_id` a session was not attached to is REFUSED rather than served**
//! 2. **a path is not an alternative route in**
//!
//! They are the conditions `dec:one-process-many-stores` was accepted on, and
//! this file is where they become checkable.
//!
//! # Why selection is by id
//!
//! `rule:a-design-is-named-by-an-id-not-a-path` (Anthony, 2026-08-09): *"the id
//! is primary and the path is a storage detail. A surface that treats a
//! filesystem path as the canonical identity forecloses object storage."* The
//! rule is ADVISORY only because nothing could yet tell compliance from the
//! status quo — it names its own trigger, *"cap:select-graph-by-id becoming
//! realized"*, which is this.
//!
//! # What this deliberately does NOT settle
//!
//! WHO MAY SEE WHICH DESIGNS. The registry lists what the OPERATOR put in its
//! root, which is right for one owner's own neighbourhood and is NOT a
//! multi-tenant policy. `dec:idea-a-session-holds-several-graphs` records the
//! distinction — *"multi-tenant isolation is not the same as composition"* — and
//! it is unanswered. Nothing here forecloses it: a per-tenant registry, or a
//! filtered listing, both remain open.

use reflow2_mcp::registry::{AttachError, Registry};

/// A scratch root holding several designs, built the way a host would: each
/// design in its own directory, identity written by opening the store once.
struct Root {
    dir: std::path::PathBuf,
}

impl Root {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "reflow2-registry-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("root");
        Self { dir }
    }

    /// Create a real design under the root and return its graph_id.
    fn design(&self, name: &str) -> String {
        let path = self.dir.join(name).join(".reflow2").join("graph");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut g = reflow2_core::DesignGraph::open_rocksdb(path.to_str().unwrap())
            .expect("open a real store");
        g.add_project(&format!("proj:{name}"), name).unwrap();
        let id = g.graph_id().to_string();
        drop(g);
        id
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// WHAT ANTHONY ASKED FOR, 2026-08-05: "can a reflow2 tool be to simply return
// all graph_ids to the agent and the agent then can choose whichever graph_id
// the user specifies to use for the project?"
#[test]
fn the_registry_lists_the_designs_under_its_root_by_id() {
    let root = Root::new("list");
    let a = root.design("alpha");
    let b = root.design("beta");

    let r = Registry::discover(root.dir.to_str().unwrap());
    let mut ids = r.graph_ids();
    ids.sort();
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(ids, want, "every design under the root is nameable");
}

// SELECTION IS BY ID. The path is the server's storage detail and never
// travels — `rule:a-design-is-named-by-an-id-not-a-path`.
#[test]
fn attaching_by_id_resolves_to_that_designs_store() {
    let root = Root::new("attach");
    let a = root.design("alpha");
    root.design("beta");

    let r = Registry::discover(root.dir.to_str().unwrap());
    let bound = r.attach(&a).expect("a known id attaches");
    assert_eq!(bound.graph_id(), a);
    assert!(
        bound.graph_path().contains("alpha"),
        "it resolved to alpha's own store, got {}",
        bound.graph_path()
    );
}

// 🛑 CLAUSE 1, and the reason the tripwire existed: an id this registry does not
// hold is REFUSED, not served, and not silently substituted with anything.
#[test]
fn an_id_the_registry_does_not_hold_is_refused() {
    let root = Root::new("unknown");
    root.design("alpha");
    let r = Registry::discover(root.dir.to_str().unwrap());

    match r.attach("some-other-design") {
        Err(AttachError::UnknownGraphId { requested }) => {
            assert_eq!(requested, "some-other-design");
        }
        other => panic!("an unknown id must be REFUSED by name, got {other:?}"),
    }
}

// 🛑 CLAUSE 1, the case that actually matters: a design that EXISTS on this
// machine but is NOT in this registry's root is refused exactly like a made-up
// one. Knowing a real graph_id must not be a way in.
#[test]
fn a_real_design_outside_the_root_is_refused_like_any_other() {
    let inside = Root::new("inside");
    inside.design("alpha");
    let outside = Root::new("outside");
    let stranger = outside.design("stranger");

    let r = Registry::discover(inside.dir.to_str().unwrap());
    assert!(
        matches!(r.attach(&stranger), Err(AttachError::UnknownGraphId { .. })),
        "a real id from another root must be refused — the registry's root is the boundary"
    );
}

// 🛑 CLAUSE 2 — "a path is not an alternative route in". The surface takes ids
// and nothing else, so a caller holding a filesystem path has no way to spend
// it. Pinned as a REFUSAL rather than left to the type system, because a future
// convenience overload is exactly how this would be lost.
#[test]
fn a_filesystem_path_is_not_an_alternative_route_in() {
    let root = Root::new("path-route");
    root.design("alpha");
    let r = Registry::discover(root.dir.to_str().unwrap());

    let path = root.dir.join("alpha").join(".reflow2").join("graph");
    let as_path = path.to_str().unwrap();
    assert!(
        matches!(r.attach(as_path), Err(AttachError::UnknownGraphId { .. })),
        "a path handed to attach() must be refused as an unknown ID, never resolved as a path"
    );
}

// A BINDING IS THE CAPABILITY. Once attached, there is no operation that takes
// another id — so "cannot name another design" holds by construction and not by
// a check somebody must remember to keep.
#[test]
fn a_binding_carries_one_design_and_offers_no_way_to_ask_for_a_second() {
    let root = Root::new("binding");
    let a = root.design("alpha");
    let b = root.design("beta");

    let r = Registry::discover(root.dir.to_str().unwrap());
    let bound = r.attach(&a).unwrap();
    assert_eq!(bound.graph_id(), a);
    assert_ne!(
        bound.graph_path(),
        r.attach(&b).unwrap().graph_path(),
        "two bindings are two stores; a binding never widens to cover both"
    );
}

// NO SILENT CAPS. An empty root reports EMPTY rather than looking like a
// registry that simply has not been asked — "nothing here" and "I did not look"
// must not be one answer (dec:loop-status-cannot-say-it-never-looked's shape).
#[test]
fn an_empty_root_says_it_holds_nothing_rather_than_looking_unasked() {
    let root = Root::new("empty");
    let r = Registry::discover(root.dir.to_str().unwrap());
    assert!(r.graph_ids().is_empty());
    assert_eq!(r.root(), root.dir.to_str().unwrap());
}

// A directory that has opted in but holds no identity must NOT be nameable —
// naming it would mean opening it, which MINTS an identity and answers its own
// question (the failure `describe_designs` was built to avoid).
#[test]
fn a_store_with_no_identity_is_not_nameable() {
    let root = Root::new("unnamed");
    let opted = root.dir.join("gamma").join(".reflow2");
    std::fs::create_dir_all(&opted).unwrap();

    let r = Registry::discover(root.dir.to_str().unwrap());
    assert!(
        r.graph_ids().is_empty(),
        "an opted-in directory with no identity is not a design a session may name"
    );
}
