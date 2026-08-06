//! The content store (`cap:content-store`, `ver:content-store`).
//!
//! The check was written BEFORE the code, so these are the cases the design
//! said would constitute proof rather than the cases the implementation made
//! convenient. The load-bearing ones are the negatives: a store that
//! round-trips is easy, and what makes content addressing worth having is that
//! it REFUSES bytes which no longer match their address.

use reflow2_core::{ContentStore, content_hash};

/// A store of its own per test — same idiom as the persistence and provenance
/// suites, which use the process id rather than pulling in a tempfile
/// dependency for something this small.
fn store(name: &str) -> ContentStore {
    let dir = std::env::temp_dir().join(format!("reflow2-content-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    ContentStore::new(dir)
}

#[test]
fn the_same_bytes_hash_the_same_and_are_stored_once() {
    let s = store("dedup");
    let a = s.put(b"a whiteboard photo, pretend").unwrap();
    let b = s.put(b"a whiteboard photo, pretend").unwrap();
    assert_eq!(a, b, "content addressing means the address IS the content");

    // Stored once, not twice: a re-put is a no-op rather than an overwrite,
    // which is what makes put safe to retry.
    let files: Vec<_> = walk(s.root());
    assert_eq!(files.len(), 1, "one blob on disk, got {files:?}");
}

#[test]
fn get_returns_exactly_what_put_took() {
    let s = store("roundtrip");
    // Bytes that are not valid UTF-8, because the point of this store is the
    // things the graph cannot hold as text.
    let bytes: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47, 0x00, 0xff, 0xfe, 0x01];
    let hash = s.put(&bytes).unwrap();
    assert_eq!(s.get(&hash).unwrap(), bytes);
    assert!(s.exists(&hash).unwrap());
}

/// THE ONE THAT MATTERS. A content hash nobody checks is just a filename, and
/// the claim that a blob fetched from anywhere can be trusted once it verifies
/// rests entirely on this refusal.
#[test]
fn content_that_no_longer_matches_its_hash_is_refused_not_returned() {
    let s = store("corrupt");
    let hash = s.put(b"the original bytes").unwrap();

    // Corrupt it in place, exactly as a bad disk or a botched merge would.
    let path = walk(s.root()).into_iter().next().expect("one blob");
    std::fs::write(&path, b"something else entirely").unwrap();

    let err = s
        .get(&hash)
        .expect_err("bytes that do not match their address must not be handed back");
    let said = format!("{err:?}");
    assert!(
        said.contains("does not match") && said.contains(&hash),
        "the refusal must name the hash it expected, got: {said}"
    );
}

#[test]
fn a_missing_blob_names_the_hash_and_where_it_looked() {
    let s = store("missing");
    let hash = content_hash(b"never stored");
    let err = s.get(&hash).expect_err("nothing was ever put");
    let said = format!("{err:?}");
    assert!(
        said.contains(&hash),
        "someone holding the design but not the bytes needs to know WHICH bytes: {said}"
    );
    assert!(!s.exists(&hash).unwrap());
}

/// A hash is interpolated into a path, so anything that is not one must be
/// refused before it reaches the filesystem — not sanitised quietly.
#[test]
fn a_hash_shaped_like_a_path_is_refused_by_name() {
    let s = store("traversal");
    for bad in [
        "../../etc/passwd",
        "sha256:../../etc/passwd",
        "sha256:short",
        "deadbeef",
        // Upper-case hex is not what content_hash emits; accepting it would
        // give one blob two addresses.
        "sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
    ] {
        let err = s
            .get(bad)
            .err()
            .unwrap_or_else(|| panic!("'{bad}' must be refused, but get() returned bytes"));
        assert!(
            format!("{err:?}").contains("not a content hash"),
            "'{bad}' must be refused as a malformed hash, not as a missing file — the difference \
             is whether it reached the filesystem at all. Got: {err:?}"
        );
        assert!(
            s.exists(bad).is_err(),
            "'{bad}' must be refused by exists() too, not answered with false"
        );
    }
}

#[test]
fn an_interrupted_write_leaves_no_partial_blob() {
    let s = store("atomic");
    s.put(b"complete content").unwrap();

    // Nothing temporary survives a successful write...
    let leftovers: Vec<_> = walk(s.root())
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(".tmp-"))
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );

    // ...and a temp file that a dead process DID leave is not reachable as a
    // blob, because its name is not a hash. This is the property that makes a
    // half-written file harmless rather than a corrupt blob.
    let shard = walk(s.root())
        .into_iter()
        .next()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    std::fs::write(shard.join(".tmp-9999-ab"), b"half written").unwrap();
    let hash = content_hash(b"half written");
    assert!(
        !s.exists(&hash).unwrap(),
        "a stranded temp file must not be addressable as content"
    );
}

#[test]
fn a_store_over_a_root_that_does_not_exist_reads_as_empty_not_an_error() {
    // Constructing a store is not a reason to create a directory: a read-only
    // consumer who has the design but not the blobs should get an honest miss.
    let s = ContentStore::new(
        std::env::temp_dir().join(format!("reflow2-content-absent-{}", std::process::id())),
    );
    let hash = content_hash(b"anything");
    assert!(!s.exists(&hash).unwrap());
    assert!(
        !s.root().exists(),
        "opening a store must not create its root"
    );
}

/// Every file under `root`, blobs and strays alike.
fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

// ---- The manifest (cap:content-manifest, ver:content-manifest) --------------

use reflow2_core::DesignGraph;
use reflow2_core::nodes::{Props, edge, node};

/// A design that references one stored diagram, plus the store holding it.
fn design_with_content(name: &str) -> (DesignGraph, ContentStore) {
    let s = store(name);
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.add_decision("dec:x", "A choice", "we chose the left one", None)
        .unwrap();
    let hash = s.put(b"graph TD; A-->B").unwrap();
    g.create_node(
        node::FRAGMENT,
        "frag:sketch",
        Props::new()
            .set("title", "layout-sketch.mermaid")
            .set("content_ref", hash.as_str()),
    )
    .unwrap();
    g.create_edge(
        edge::ANNOTATES,
        node::FRAGMENT,
        "frag:sketch",
        node::DECISION,
        "dec:x",
        Props::new(),
    )
    .unwrap();
    (g, s)
}

#[test]
fn the_manifest_names_the_content_and_what_it_explains() {
    let (g, s) = design_with_content("manifest");
    let m = g.content_manifest(Some(&s)).unwrap();

    assert_eq!(m.entries.len(), 1);
    let e = &m.entries[0];
    assert_eq!(
        e.name, "layout-sketch.mermaid",
        "the readable name is the Fragment's title"
    );
    assert!(e.present, "the bytes are in the store");
    assert!(
        e.referenced_by.contains(&"dec:x".to_string()),
        "the manifest must say what the picture is FOR, not only which fragment holds it: {:?}",
        e.referenced_by
    );
    assert!(m.missing.is_empty() && m.orphaned.is_empty());
}

/// THE CASE THE MANIFEST EXISTS FOR: someone was handed the design and not the
/// bytes. Their diagrams must be nameable, not silently absent.
#[test]
fn content_the_design_references_but_this_checkout_lacks_is_reported_by_name() {
    let (g, _s) = design_with_content("missing-bytes");
    // A different store — the design travelled, the blobs did not.
    let empty = store("missing-bytes-elsewhere");
    let m = g.content_manifest(Some(&empty)).unwrap();

    assert_eq!(m.missing.len(), 1, "the absent blob must be named");
    assert!(!m.entries[0].present);
    assert!(
        m.render().contains("**NO**"),
        "the rendered form must not hide it"
    );
}

/// The reverse, and the reason `list` exists: bytes nobody points at are how a
/// store grows without anyone deciding to.
#[test]
fn bytes_nothing_references_are_reported_as_orphaned() {
    let (g, s) = design_with_content("orphan");
    let stray = s.put(b"a render nobody wired up").unwrap();

    let m = g.content_manifest(Some(&s)).unwrap();
    assert_eq!(m.orphaned, vec![stray], "an unreferenced blob must surface");
    assert!(m.missing.is_empty(), "the referenced one is still present");
}

/// A `content_ref` predating the store holds a PATH. Treating it as content
/// would invent a missing blob that was never stored and never referenced.
#[test]
fn a_content_ref_that_is_not_a_hash_is_not_treated_as_content() {
    let s = store("legacy-ref");
    let mut g = DesignGraph::open_in_memory().unwrap();
    g.create_node(
        node::FRAGMENT,
        "frag:old",
        Props::new()
            .set("title", "an old note")
            .set("content_ref", "docs/notes/old.md"),
    )
    .unwrap();
    let m = g.content_manifest(Some(&s)).unwrap();
    assert!(
        m.entries.is_empty() && m.missing.is_empty(),
        "a path is not a content hash and must not be reported as missing content: {m:?}"
    );
}

#[test]
fn the_locator_travels_verbatim_and_is_never_parsed() {
    let s = store("locator");
    let mut g = DesignGraph::open_in_memory().unwrap();
    let hash = s.put(b"a thousand pages, pretend").unwrap();
    g.create_node(
        node::FRAGMENT,
        "frag:spec",
        Props::new()
            .set("title", "vendor-spec.md")
            .set("content_ref", format!("{hash}#L412-L440").as_str()),
    )
    .unwrap();
    let m = g.content_manifest(Some(&s)).unwrap();
    assert_eq!(m.entries[0].locator.as_deref(), Some("L412-L440"));
    assert!(
        m.entries[0].present,
        "the locator must not confuse the lookup"
    );
}

// ---- Growth: the guard, and the reporting that matters more ----------------

/// The guard catches the catastrophic accident and says so — including that it
/// is NOT what keeps the store small
/// (`dec:content-growth-is-bounded-by-what-not-by-size`).
#[test]
fn oversized_content_is_refused_until_the_caller_says_yes_on_the_record() {
    let s = store("oversize");
    let big = vec![0u8; reflow2_core::LARGE_CONTENT_BYTES + 1];

    let err = s
        .put(&big)
        .expect_err("committed blobs are permanent; this needs a decision");
    let said = format!("{err:?}");
    assert!(
        said.contains("accept_large"),
        "the refusal must name the override: {said}"
    );
    assert!(
        said.contains("transcripts"),
        "the refusal must say the cap is not the real control, or it will be mistaken for one: {said}"
    );

    // Overridable — deliberately, on the record.
    let hash = s
        .put_allowing_large(&big, true)
        .expect("an explicit yes stores it");
    assert!(s.exists(&hash).unwrap());
}

/// Ordinary content is untouched by the guard: a transcript-sized file passes,
/// which is exactly why the cap is not the answer to growth.
#[test]
fn a_transcript_sized_file_passes_the_cap_which_is_the_whole_point() {
    let s = store("transcript-sized");
    let four_mb = vec![b'x'; 4 * 1024 * 1024];
    s.put(&four_mb).expect("4 MB is nowhere near the bar");
}

#[test]
fn the_manifest_reports_total_size_and_the_largest_entries() {
    let (g, s) = design_with_content("sizes");
    s.put(&vec![b'a'; 2048]).unwrap();
    s.put(&vec![b'b'; 512]).unwrap();

    let m = g.content_manifest(Some(&s)).unwrap();
    assert!(
        m.total_bytes >= 2048 + 512,
        "total must count every blob, got {}",
        m.total_bytes
    );
    assert_eq!(m.largest[0].1, 2048, "largest first");
    assert!(
        m.render().contains("## Size"),
        "growth must be visible in the committed form, not only in the JSON"
    );
}
