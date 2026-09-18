//! reflow2 MEASURES a registered file and never reads it for meaning.
//!
//! `req:reflow2-measures-registered-files-and-never-reads-them-for-meaning`
//! (Anthony, 2026-09-18). The slogan "reflow2 does no file I/O" was always a
//! shorthand for "no interpretation": the server already wrote its own export
//! and sidecars, and what it never did was touch a USER'S artifact. Until this
//! module every checksum on an Artifact was a value an agent pasted — the graph
//! could not tell a pasted hash from a made-up one, and on reflow2's own design
//! an agent hashed eighteen files by hand in one increment and re-hashed one
//! after a later edit. Hashing is counting; the graph counts. Parsing,
//! extracting and navigating stay the agent's (`dec:agent-navigates-content`).
//!
//! # The bounds, and they are the whole design
//!
//! - **Root-bounded.** A location is text somebody wrote into the graph, and
//!   the moment the server dereferences it, graph text has become a directive.
//!   So a location is resolved under the project root ONLY: the joined path is
//!   canonicalised and must still start with the canonical root. A symlink
//!   that escapes the root canonicalises outside it and is refused by name.
//! - **Bytes in, a number out.** A measurement is a digest, a size and a
//!   presence. Nothing here returns content, and nothing here lists a tree.
//! - **Not on this machine is not zero.** A server that does not hold the tree
//!   (in memory, a registry of designs, a shared server elsewhere) says so
//!   ([`NotMeasured::NoTree`]) instead of reporting an absent file, because a
//!   detector with nothing to run on reads exactly like one that ran clean.
//! - **Location fragments.** `plans.pdf#pages 64-96` is a real registration
//!   shape (a sheet range of one drawing set). When the location as written is
//!   absent and carries a `#`, the part before it is measured and the reply
//!   names the path actually hashed.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What measuring a registered location produced.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Measurement {
    /// `sha256:<hex>` — the same canonical dialect the core stores.
    pub checksum: String,
    pub bytes: u64,
    /// The path actually hashed, project-relative — differs from the location
    /// only when a `#fragment` was stripped.
    pub measured_path: String,
}

/// Why a location was NOT measured. Every variant is named in the reply, so
/// "not measured" is never mistaken for "absent" or "unchanged".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum NotMeasured {
    /// This server does not hold a project tree: an in-memory design, a
    /// registry of designs served over HTTP, or a shared server on a box that
    /// does not hold the checkout.
    NoTree,
    /// The location resolves outside the project root (an absolute path
    /// elsewhere, a `..` escape, a symlink pointing out) — refused, never read.
    OutsideRoot { location: String },
    /// Nothing at that path under the root.
    Absent { path: String },
    /// Present, but a directory or a special file: a directory has no content
    /// hash, and walking it would be listing a tree, which this never does.
    NotAFile { path: String },
    /// The read failed after the path resolved (permissions, I/O).
    Unreadable { path: String, error: String },
}

impl NotMeasured {
    /// One sentence a refusal can carry.
    pub fn reason(&self) -> String {
        match self {
            NotMeasured::NoTree => "this server does not hold the project tree (an in-memory or \
                                    registry-served design, or a shared server on another \
                                    machine), so nothing can be measured here — pass the checksum \
                                    to assert it, or reconcile with `observed`"
                .to_string(),
            NotMeasured::OutsideRoot { location } => format!(
                "{location:?} resolves outside the project root and is refused unread — a \
                 registered location is measured only under the directory the design lives in"
            ),
            NotMeasured::Absent { path } => {
                format!("nothing exists at {path:?} under the project root")
            }
            NotMeasured::NotAFile { path } => format!(
                "{path:?} is not a regular file — a directory or device has no content hash, and \
                 reflow2 does not walk a tree"
            ),
            NotMeasured::Unreadable { path, error } => {
                format!("{path:?} could not be read: {error}")
            }
        }
    }
}

/// Per-server memo of measurements keyed on the file's (length, mtime), so a
/// `loop_status` that measures every registered file at every boundary pays
/// for the 114 MB drawing set once, not per call. A property of the SERVER
/// like the graph, shared across sessions; never a process-global.
/// (length, mtime, what was measured) — the key the memo is valid under.
type Memoised = (u64, Option<std::time::SystemTime>, Measurement);

#[derive(Debug, Default)]
pub struct MeasureMemo {
    inner: Mutex<HashMap<PathBuf, Memoised>>,
}

pub type SharedMemo = Arc<MeasureMemo>;

/// Measure `location` under `root`. See the module note for the bounds.
pub fn measure(
    root: &Path,
    location: &str,
    memo: Option<&MeasureMemo>,
) -> Result<Measurement, NotMeasured> {
    let canonical_root = std::fs::canonicalize(root).map_err(|_| NotMeasured::NoTree)?;
    let (candidate, measured_path) = resolve(&canonical_root, location)?;
    let meta = std::fs::symlink_metadata(&candidate).map_err(|_| NotMeasured::Absent {
        path: measured_path.clone(),
    })?;
    // Canonicalise AFTER existence is known, so an absent path is reported as
    // absent rather than as an escape it never made; then bound it.
    let real = std::fs::canonicalize(&candidate).map_err(|e| NotMeasured::Unreadable {
        path: measured_path.clone(),
        error: e.to_string(),
    })?;
    if !real.starts_with(&canonical_root) {
        return Err(NotMeasured::OutsideRoot {
            location: location.to_string(),
        });
    }
    let real_meta = std::fs::metadata(&real).map_err(|e| NotMeasured::Unreadable {
        path: measured_path.clone(),
        error: e.to_string(),
    })?;
    if !real_meta.is_file() {
        return Err(NotMeasured::NotAFile {
            path: measured_path,
        });
    }
    let _ = meta;
    let len = real_meta.len();
    let mtime = real_meta.modified().ok();
    if let Some(m) = memo
        && let Ok(guard) = m.inner.lock()
        && let Some((l, t, hit)) = guard.get(&real)
        && *l == len
        && *t == mtime
    {
        let mut hit = hit.clone();
        hit.measured_path = measured_path;
        return Ok(hit);
    }
    let checksum = sha256_file(&real).map_err(|e| NotMeasured::Unreadable {
        path: measured_path.clone(),
        error: e.to_string(),
    })?;
    let out = Measurement {
        checksum,
        bytes: len,
        measured_path,
    };
    if let Some(m) = memo
        && let Ok(mut guard) = m.inner.lock()
    {
        guard.insert(real, (len, mtime, out.clone()));
    }
    Ok(out)
}

/// Join and bound WITHOUT touching the disk beyond existence: absolute
/// locations are allowed only when they already sit under the root; `..` is
/// resolved lexically first so an obvious escape is refused before any read.
fn resolve(root: &Path, location: &str) -> Result<(PathBuf, String), NotMeasured> {
    let as_written = Path::new(location);
    let joined = if as_written.is_absolute() {
        as_written.to_path_buf()
    } else {
        root.join(as_written)
    };
    let lexical = lexical_normalize(&joined);
    if !lexical.starts_with(root) {
        return Err(NotMeasured::OutsideRoot {
            location: location.to_string(),
        });
    }
    if lexical.exists() {
        return Ok((lexical, relative(root, location)));
    }
    // A `#fragment` names a part of a file; the file is what is measured.
    if let Some((file, _frag)) = location.rsplit_once('#') {
        let file = file.trim_end();
        if !file.is_empty() {
            let f = Path::new(file);
            let joined = if f.is_absolute() {
                f.to_path_buf()
            } else {
                root.join(f)
            };
            let lexical = lexical_normalize(&joined);
            if !lexical.starts_with(root) {
                return Err(NotMeasured::OutsideRoot {
                    location: location.to_string(),
                });
            }
            if lexical.exists() {
                return Ok((lexical, relative(root, file)));
            }
        }
    }
    Err(NotMeasured::Absent {
        path: relative(root, location),
    })
}

fn relative(root: &Path, location: &str) -> String {
    let p = Path::new(location);
    if p.is_absolute() {
        p.strip_prefix(root)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or_else(|_| location.to_string())
    } else {
        location.to_string()
    }
}

fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Streamed, so a 114 MB drawing set does not become a 114 MB allocation.
fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(7 + digest.len() * 2);
    hex.push_str("sha256:");
    for b in digest.iter() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/a.rs"), b"fn a() {}\n").unwrap();
        d
    }

    #[test]
    fn a_file_under_the_root_is_hashed_and_nothing_else_is_returned() {
        let d = tree();
        let m = measure(d.path(), "src/a.rs", None).expect("measured");
        assert!(m.checksum.starts_with("sha256:") && m.checksum.len() == 7 + 64);
        assert_eq!(m.bytes, 10);
        assert_eq!(m.measured_path, "src/a.rs");
    }

    #[test]
    fn an_escape_is_refused_by_name_and_never_read() {
        let d = tree();
        let err = measure(d.path(), "../../etc/passwd", None).unwrap_err();
        assert!(matches!(err, NotMeasured::OutsideRoot { .. }), "{err:?}");
        let err = measure(d.path(), "/etc/passwd", None).unwrap_err();
        assert!(matches!(err, NotMeasured::OutsideRoot { .. }), "{err:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_pointing_out_of_the_root_is_refused() {
        let d = tree();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), b"x").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), d.path().join("src/link"))
            .unwrap();
        let err = measure(d.path(), "src/link", None).unwrap_err();
        assert!(matches!(err, NotMeasured::OutsideRoot { .. }), "{err:?}");
    }

    #[test]
    fn absent_and_directory_are_told_apart_from_each_other_and_from_an_escape() {
        let d = tree();
        assert!(matches!(
            measure(d.path(), "src/nope.rs", None).unwrap_err(),
            NotMeasured::Absent { .. }
        ));
        assert!(matches!(
            measure(d.path(), "src", None).unwrap_err(),
            NotMeasured::NotAFile { .. }
        ));
    }

    #[test]
    fn a_fragment_names_the_file_that_is_measured() {
        let d = tree();
        let m = measure(d.path(), "src/a.rs#pages 1-2", None).expect("measured the file");
        assert_eq!(m.measured_path, "src/a.rs");
    }

    #[test]
    fn the_memo_serves_an_unchanged_file_and_notices_a_changed_one() {
        let d = tree();
        let memo = MeasureMemo::default();
        let first = measure(d.path(), "src/a.rs", Some(&memo)).unwrap();
        let again = measure(d.path(), "src/a.rs", Some(&memo)).unwrap();
        assert_eq!(first.checksum, again.checksum);
        std::fs::write(d.path().join("src/a.rs"), b"fn a() { 1 }\n").unwrap();
        let changed = measure(d.path(), "src/a.rs", Some(&memo)).unwrap();
        assert_ne!(
            first.checksum, changed.checksum,
            "a longer file must not be served from the memo"
        );
    }
}
