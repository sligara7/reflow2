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
