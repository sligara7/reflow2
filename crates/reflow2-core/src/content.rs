//! The content store — bytes the graph points at but cannot hold.
//!
//! `req:content-store-for-what-the-graph-points-at` and
//! `req:content-reaches-every-seat`, governed by `dec:where-content-lives`,
//! `dec:content-store-implementation` and `dec:agent-navigates-content`.
//!
//! A design is text, and some of what a design *means* is not: a session
//! transcript, a mermaid diagram, an HTML mock of a layout, a photograph of a
//! whiteboard, the source documents a corpus was extracted from. Today those
//! are paths that rot — `Artifact` can name a file but nothing carries its
//! bytes, so a hand-drawn architecture sketch survives exactly as long as
//! somebody's laptop. The graph records a **content hash**; the bytes live
//! here; and the two travel together because both are committed to the repo.
//!
//! ## Three functions for the store, and a fourth for the manifest
//!
//! `put`, `get`, `exists`. Content addressing is what shrinks the problem:
//! blobs are **immutable**, so there is no update-in-place to make conditional,
//! no partial write to resume, no listing or prefix semantics to model, and no
//! rename. `dec:content-store-implementation` weighed `object_store` (Apache
//! Arrow) against this and took the hand-rolled route on a measurement — 36
//! crates new to this workspace, about 24 of them URL/ICU machinery a local
//! store never calls, to buy a feature set immutable content addressing does
//! not have. It stays the documented upgrade path for the day a cloud backend
//! is real, and these three functions are deliberately the shape it slots
//! behind.
//!
//! ## Synchronous, and in the core on purpose
//!
//! `reflow2-core` has no tokio, no async and no futures, and that is a property
//! worth keeping rather than an accident. `object_store` is async and its `fs`
//! backend pulls a runtime, which is the only reason an earlier draft put the
//! store in `reflow2-mcp` with just the hash in the core. `std::fs` removes the
//! split: the store is deterministic, testable without a runtime, and lives
//! beside everything else that reasons about the design.
//!
//! ## What this module does NOT do
//!
//! It does not read *into* content. Finding the relevant passage of a
//! thousand-page document is the agent's job and an agent already does it well
//! — `dec:agent-navigates-content`. The graph stores an opaque locator beside
//! the hash (a line range, a page, a timestamp) which reflow2 never parses.
//! Building a retrieval layer here would be reinventing something the system
//! already has, one component over.

use std::path::{Path, PathBuf};

use dynograph_core::{DynoError, Value};

use crate::graph::DesignGraph;

/// How many leading hex characters of the hash become the shard directory.
///
/// Two, giving 256 buckets — the layout git itself uses for loose objects, and
/// for the same reason: a single flat directory with tens of thousands of
/// entries is slow to list and unpleasant on several filesystems. It is not a
/// correctness property, and nothing reads it back out: `get` recomputes the
/// path from the hash, so changing this later would strand existing blobs and
/// must be treated as a migration rather than a tweak.
const SHARD_LEN: usize = 2;

/// The `sha256:` prefix every hash in this design carries — the export's
/// content hash and `set_artifact_checksum` both use it, and a bare hex string
/// somewhere else would be the same value in a second dialect.
const HASH_PREFIX: &str = "sha256:";

/// Hex characters in a sha-256 digest.
const HASH_HEX_LEN: usize = 64;

/// A content-addressed store rooted at a directory.
///
/// The root is supplied by the caller and never guessed. `reflow2-core` is
/// neutral to the interaction surface, and a store that picked its own location
/// would be deciding where a consumer's repo keeps its bytes.
#[derive(Debug, Clone)]
pub struct ContentStore {
    root: PathBuf,
}

/// The content hash of `bytes`, in this design's one dialect.
///
/// The FULL 64-hex digest, deliberately, where `Artifact.checksum` registers a
/// truncated 16. A truncated hash is fine as a drift tripwire, where the
/// question is "did this file change" and a collision means a missed
/// notification. Here the hash is the ADDRESS: a collision would return the
/// wrong bytes under the right name, silently, which is the one failure
/// content addressing exists to make impossible.
pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(HASH_PREFIX.len() + HASH_HEX_LEN);
    hex.push_str(HASH_PREFIX);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Reject anything that is not a hash this store could have produced, BEFORE
/// it reaches the filesystem.
///
/// This is a path-traversal guard as much as a validation: a hash is
/// interpolated straight into a path, so `../../etc/passwd` arriving as a
/// "hash" must never get that far. Requiring the exact prefix and exactly 64
/// lowercase hex characters leaves no room for a separator, and the check is
/// cheap enough to run on every call rather than trusting callers
/// (`req:no-silent-fallback` — refuse by name, do not sanitise quietly).
fn parse_hash(hash: &str) -> Result<&str, DynoError> {
    let hex = hash.strip_prefix(HASH_PREFIX).ok_or_else(|| DynoError::Validation {
        node_type: "ContentStore".into(),
        property: "hash".into(),
        message: format!(
            "'{hash}' is not a content hash: it must start with `{HASH_PREFIX}`. Hashes come from \
             content_hash() or from a pointer already in the graph; a bare path or filename is not \
             one."
        ),
    })?;
    if hex.len() != HASH_HEX_LEN
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(DynoError::Validation {
            node_type: "ContentStore".into(),
            property: "hash".into(),
            message: format!(
                "'{hash}' is not a content hash: expected exactly {HASH_HEX_LEN} lowercase hex \
                 characters after `{HASH_PREFIX}`, found {}. This is refused before it reaches the \
                 filesystem, because a hash is interpolated into a path.",
                hex.len()
            ),
        });
    }
    Ok(hex)
}

impl ContentStore {
    /// Open a store rooted at `root`. The directory is created on first write,
    /// not here — opening a store is not a reason to make a directory, and a
    /// read-only consumer should be able to construct one over a root that
    /// does not exist yet and get an honest miss rather than a side effect.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// The directory this store is rooted at.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where a given hash lives: `<root>/<first two hex>/<remaining hex>`.
    fn path_for(&self, hex: &str) -> PathBuf {
        let (shard, rest) = hex.split_at(SHARD_LEN);
        self.root.join(shard).join(rest)
    }

    /// Store `bytes` and return the hash they are addressed by.
    ///
    /// **Idempotent by construction.** The same bytes hash the same, so storing
    /// them twice is one file — and a re-put is not an overwrite but a no-op,
    /// which is what makes this safe to retry. Nothing is ever replaced: an
    /// "edit" produces a different hash and therefore a different file, which
    /// is also why git sees an ADDITION rather than an opaque binary
    /// modification (`dec:content-manifest`).
    ///
    /// **Atomic.** The bytes go to a temporary file in the same shard
    /// directory and are then renamed into place, so a process that dies
    /// mid-write leaves either nothing or a complete blob — never a truncated
    /// file sitting under a hash that promises its content. Same-directory
    /// rename keeps it on one filesystem, where rename is atomic.
    pub fn put(&self, bytes: &[u8]) -> Result<String, DynoError> {
        let hash = content_hash(bytes);
        let hex = parse_hash(&hash)?;
        let target = self.path_for(hex);

        // Already present means already correct: the name IS the digest of the
        // content, so there is nothing a second write could improve.
        if target.exists() {
            return Ok(hash);
        }

        let dir = target.parent().expect("path_for always yields a parent");
        std::fs::create_dir_all(dir).map_err(|e| {
            DynoError::Storage(format!(
                "cannot create the content-store directory {}: {e}",
                dir.display()
            ))
        })?;

        // Unique per process AND per call: two threads storing different bytes
        // into the same shard must not collide on the temp name.
        let temp = dir.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            &hex[..SHARD_LEN.min(hex.len())]
        ));
        std::fs::write(&temp, bytes).map_err(|e| {
            DynoError::Storage(format!("cannot write content to {}: {e}", temp.display()))
        })?;
        std::fs::rename(&temp, &target).map_err(|e| {
            // Leave no litter if the rename is what failed.
            let _ = std::fs::remove_file(&temp);
            DynoError::Storage(format!("cannot place content at {}: {e}", target.display()))
        })?;
        Ok(hash)
    }

    /// Whether the bytes for `hash` are present. Does NOT verify them — that
    /// is `get`'s job, and conflating the two would make a cheap presence check
    /// secretly expensive on a large blob.
    pub fn exists(&self, hash: &str) -> Result<bool, DynoError> {
        Ok(self.path_for(parse_hash(hash)?).exists())
    }

    /// Every hash present in the store, sorted.
    ///
    /// A FOURTH function, where `dec:content-store-implementation` said three —
    /// worth naming rather than slipping in. That decision argued content
    /// addressing needs no LISTING, and it was right about the kind
    /// `object_store` provides: paths, prefixes, delimiters, pagination. This is
    /// a different thing — enumerating a flat set of addresses — and orphan
    /// detection is impossible without it. A manifest that can only report
    /// content the graph ALREADY names could never find the bytes nobody
    /// references, which is precisely how a store silently grows
    /// (`ver:content-manifest`).
    ///
    /// Anything whose name is not a hash is skipped: a stranded `.tmp-` file, a
    /// committed `MANIFEST.md`, an editor backup. The store owns this directory
    /// but must not claim every file in it is content.
    pub fn list(&self) -> Result<Vec<String>, DynoError> {
        let mut out = Vec::new();
        let Ok(shards) = std::fs::read_dir(&self.root) else {
            return Ok(out); // no store yet is an empty store, not an error
        };
        for shard in shards.flatten() {
            if !shard.path().is_dir() {
                continue;
            }
            let Some(prefix) = shard.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(blobs) = std::fs::read_dir(shard.path()) else {
                continue;
            };
            for blob in blobs.flatten() {
                let Some(rest) = blob.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                let candidate = format!("{HASH_PREFIX}{prefix}{rest}");
                if parse_hash(&candidate).is_ok() {
                    out.push(candidate);
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Read the bytes for `hash`, **verifying them against it**.
    ///
    /// The verification is the point, not a belt-and-braces extra. A content
    /// hash that is never checked is only a filename, and the property this
    /// whole design leans on — that a blob fetched from anywhere can be trusted
    /// once it verifies (`dec:where-content-lives`) — is exactly this check. So
    /// content that no longer matches its address is REFUSED rather than
    /// returned: handing back bytes under a hash they do not have would be the
    /// silent corruption `req:no-silent-fallback` forbids, and the caller has
    /// no way to notice.
    ///
    /// A missing blob names the hash and where it was looked for, because the
    /// person hitting this is usually someone who has the design and not the
    /// bytes (`req:content-reaches-every-seat`), and "not found" alone does not
    /// tell them that.
    pub fn get(&self, hash: &str) -> Result<Vec<u8>, DynoError> {
        let hex = parse_hash(hash)?;
        let path = self.path_for(hex);
        let bytes = std::fs::read(&path).map_err(|e| {
            DynoError::Storage(format!(
                "content {hash} is not in this store (looked in {}): {e}. The design references it, \
                 so either the blobs did not travel with the design or it was never stored.",
                path.display()
            ))
        })?;
        let actual = content_hash(&bytes);
        if actual != hash {
            return Err(DynoError::Storage(format!(
                "content at {} does not match the hash it is stored under: expected {hash}, the \
                 bytes hash to {actual}. Refused rather than returned — a content hash that is not \
                 checked is only a filename.",
                path.display()
            )));
        }
        Ok(bytes)
    }
}

// ---- The manifest: what the design points at, in names a person can read ----

/// One piece of content the design references.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    /// The content hash, as the graph records it.
    pub hash: String,
    /// A readable name — the referencing Fragment's `title`. The graph already
    /// requires one, so the manifest needs no second place to keep filenames
    /// and cannot disagree with the design about what something is called.
    pub name: String,
    /// The locator the agent wrote after the hash, if any — a line range, a
    /// page, a timestamp. Carried VERBATIM and never parsed
    /// (`dec:agent-navigates-content`).
    pub locator: Option<String>,
    /// What in the design points at this content: the Fragment holding the
    /// reference, and whatever it annotates or yielded.
    pub referenced_by: Vec<String>,
    /// Whether the bytes are actually in the store.
    pub present: bool,
}

/// What the design depends on, and whether this checkout has it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContentManifest {
    /// Where the store was looked for.
    pub store_root: String,
    /// Every referenced piece of content, sorted by hash.
    pub entries: Vec<ManifestEntry>,
    /// Referenced by the design and NOT present — the case someone holding
    /// only the export hits, and the reason this exists at all.
    pub missing: Vec<String>,
    /// Present in the store and referenced by NOTHING. Reported because
    /// unreferenced bytes are how a store silently grows, and because
    /// `dec:where-content-lives` left repo growth deliberately unbounded.
    pub orphaned: Vec<String>,
}

/// Split a `content_ref` into its hash and whatever the agent appended.
///
/// The convention is `sha256:<64 hex>` optionally followed by `#<locator>`.
/// Anything that does not begin with a well-formed hash is not a content
/// reference at all — a `content_ref` may still hold a plain path from before
/// the store existed, and treating that as content would invent a missing blob.
fn split_content_ref(reference: &str) -> Option<(String, Option<String>)> {
    let (hash, locator) = match reference.split_once('#') {
        Some((h, l)) => (h, Some(l.to_string())),
        None => (reference, None),
    };
    parse_hash(hash).ok()?;
    Some((hash.to_string(), locator))
}

impl DesignGraph {
    /// What content this design points at, whether the bytes are here, and what
    /// is here that nothing points at (`cap:content-manifest`).
    ///
    /// Derived entirely from the graph plus a directory listing — nothing is
    /// stored twice. That is deliberate: a manifest kept as its own record
    /// would be a second source of truth about what the design references, and
    /// would drift from the graph the first time someone edited one and not the
    /// other. Rendering it to a committed file (see [`ContentManifest::render`])
    /// is a projection, the same relationship `dec:views-are-projections`
    /// already sets for every other view.
    pub fn content_manifest(&self, store: &ContentStore) -> Result<ContentManifest, DynoError> {
        use crate::nodes::{edge, node};

        let mut entries: Vec<ManifestEntry> = Vec::new();
        let mut referenced: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for fragment in self.scan_nodes(node::FRAGMENT)? {
            let Some(reference) = fragment
                .properties
                .get("content_ref")
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some((hash, locator)) = split_content_ref(reference) else {
                continue; // a path, not a content hash — not this manifest's business
            };
            referenced.insert(hash.clone());

            // The Fragment holds the reference; what it annotates or yielded is
            // why anyone cares. Both go in, so "what is this picture for?" is
            // answerable from the manifest alone.
            let mut referenced_by = vec![fragment.node_id.clone()];
            for e in self.outgoing(&fragment.node_id, Some(edge::ANNOTATES))? {
                referenced_by.push(e.to_id);
            }
            for e in self.outgoing(&fragment.node_id, Some(edge::YIELDED))? {
                referenced_by.push(e.to_id);
            }
            referenced_by.sort();
            referenced_by.dedup();

            entries.push(ManifestEntry {
                present: store.exists(&hash)?,
                hash,
                name: fragment
                    .properties
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("(untitled)")
                    .to_string(),
                locator,
                referenced_by,
            });
        }
        entries.sort_by(|a, b| (&a.hash, &a.name).cmp(&(&b.hash, &b.name)));

        let missing: Vec<String> = entries
            .iter()
            .filter(|e| !e.present)
            .map(|e| e.hash.clone())
            .collect();
        let orphaned: Vec<String> = store
            .list()?
            .into_iter()
            .filter(|h| !referenced.contains(h))
            .collect();

        Ok(ContentManifest {
            store_root: store.root().display().to_string(),
            entries,
            missing,
            orphaned,
        })
    }
}

impl ContentManifest {
    /// The committed, human-readable form.
    ///
    /// Markdown rather than JSON because its whole second job is being legible
    /// in a diff: a blob lands as `blobs/ab/cdef…`, and what a reviewer wants to
    /// see in `git log -p` is a line saying which picture that is and what it
    /// explains (`dec:content-manifest`).
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# Content manifest\n\n");
        out.push_str(
            "What this design points at but does not itself contain. GENERATED — derived from the \
             graph and the store, never edited by hand: it is a projection, and a manifest kept as \
             its own record would drift from the design the first time someone updated one and not \
             the other.\n\n",
        );
        if self.entries.is_empty() {
            out.push_str("_Nothing in this design references stored content yet._\n");
        }
        for e in &self.entries {
            out.push_str(&format!(
                "- **{}** — `{}`{}\n  - referenced by: {}\n  - bytes present: {}\n",
                e.name,
                e.hash,
                e.locator
                    .as_ref()
                    .map(|l| format!(" (at `{l}`)"))
                    .unwrap_or_default(),
                e.referenced_by.join(", "),
                if e.present { "yes" } else { "**NO**" },
            ));
        }
        if !self.missing.is_empty() {
            out.push_str(
                "\n## Missing\n\nReferenced by this design and not in the store. If you were sent \
                 the design on its own, this is what did not come with it.\n\n",
            );
            for h in &self.missing {
                out.push_str(&format!("- `{h}`\n"));
            }
        }
        if !self.orphaned.is_empty() {
            out.push_str(
                "\n## Orphaned\n\nIn the store and referenced by nothing. Not an error — content \
                 can be stored before it is wired up — but this is how a store grows without \
                 anyone deciding to.\n\n",
            );
            for h in &self.orphaned {
                out.push_str(&format!("- `{h}`\n"));
            }
        }
        out
    }
}
