//! Where an export's lineage is anchored: the record as COMMITTED, not the
//! file on disk.
//!
//! # Why this module exists
//!
//! `dec:export-hash-chain` builds the chain at the file-write seam — a new
//! export links to the hash of the export file it replaces. That is right when
//! exports are rare and deliberate. It breaks the moment they are frequent,
//! because the chain then records every intermediate save as a hop, and
//! `dec:export-once-per-pr` exists to stop exactly that: a pull request must
//! land ONE hop, or a squash-merge produces a document whose `prev_content_hash`
//! names a version that is on no branch anybody can see.
//!
//! **The measurement that forced it** (`fact:auto-export-really-does-sever-the-
//! lineage-and-a-separate-path-dissolves-it`, 2026-09-11) reproduced the
//! collision and falsified the hope that "no commit, no push" avoids it —
//! *"because the chain is built from the FILE, not from git."*
//!
//! So the anchor moves. Chained from the merge-base with the default branch,
//! any number of commits on a branch each chain from the same committed
//! ancestor, squash-and-merge lands exactly one hop per PR, and the "and it
//! must be LAST" clause of that rule becomes automatic rather than remembered.
//! `dec:idea-does-the-graph-write-itself-through-to-the-repo-on-every-change`,
//! road (a), accepted by Anthony 2026-09-11.
//!
//! # Why merge-base and not the tip of the default branch
//!
//! A long-lived branch's `origin/main` moves underneath it. Anchoring at the
//! TIP would silently re-point the chain at a commit the branch never saw, and
//! the export would claim descent from work it does not contain. The merge-base
//! is the commit both actually share, which is the only honest answer.
//!
//! # This module assumes nothing about git
//!
//! Every failure — no repository, no commits, the file untracked, no `git` on
//! PATH — returns `None` WITH A REASON, and the caller falls back to chaining
//! from the file on disk, which is the behaviour that has always existed. Git
//! is an improvement to the anchor, never a requirement for exporting.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The export as committed, and how it was found — the second half is for the
/// receipt, because a caller cannot otherwise tell an anchor at main from the
/// on-disk fallback, and those two produce different chains.
#[derive(Debug, Clone)]
pub struct CommittedPredecessor {
    /// The parsed export document as of the merge-base.
    pub doc: reflow2_core::GraphExport,
    /// Human-readable provenance, e.g. `origin/main@0b10966`.
    pub source: String,
}

/// Why no committed predecessor was found. Never an error — the caller
/// degrades to on-disk chaining and reports this so the difference is visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoAnchor {
    /// `git` is not on PATH, or failed to run at all.
    NoGit,
    /// The export path is not inside a git work tree.
    NotARepo,
    /// A repository with no default branch this could resolve.
    NoDefaultBranch,
    /// A repository with no commit reachable from both HEAD and the default
    /// branch — a fresh repo, or an unrelated history.
    NoMergeBase,
    /// The file exists on disk but not in the committed tree at that point.
    NotCommitted,
    /// The committed blob is not a reflow2 export document.
    NotAnExport,
}

impl NoAnchor {
    /// The sentence that goes in the receipt. States the fact, never blames.
    pub fn reason(&self) -> &'static str {
        match self {
            NoAnchor::NoGit => "no git on PATH — lineage chained from the file on disk",
            NoAnchor::NotARepo => {
                "this export is not inside a git repository — lineage chained from the file on disk"
            }
            NoAnchor::NoDefaultBranch => {
                "no default branch could be resolved (looked for origin/HEAD, origin/main, \
                 origin/master, main, master) — lineage chained from the file on disk"
            }
            NoAnchor::NoMergeBase => {
                "no commit is shared with the default branch yet — lineage chained from the file \
                 on disk"
            }
            NoAnchor::NotCommitted => {
                "this export is not committed on the default branch yet — lineage chained from \
                 the file on disk"
            }
            NoAnchor::NotAnExport => {
                "the committed version of this path is not a reflow2 export — lineage chained \
                 from the file on disk"
            }
        }
    }
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Raw bytes of a git command's stdout — `git show` of a 15 MB export is not
/// something to round-trip through `trim()`.
fn git_bytes(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

/// The default branch to anchor against, most authoritative first.
///
/// `origin/HEAD` is the repository's own answer and is preferred; the rest are
/// conventions, tried in the order that is least likely to surprise. Returning
/// the first that resolves is deliberate — guessing is bounded and reported,
/// where asking the user for a branch name at export time would not be.
fn default_branch(dir: &Path) -> Option<String> {
    if let Some(head) = git(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .filter(|h| !h.is_empty())
    {
        return Some(head);
    }
    for candidate in ["origin/main", "origin/master", "main", "master"] {
        if git(dir, &["rev-parse", "--verify", "--quiet", candidate]).is_some() {
            return Some(candidate.to_string());
        }
    }
    None
}

/// The export as committed on the default branch, for `path`.
///
/// `Ok` carries the document and where it came from; `Err` carries the reason
/// there is none, which the caller reports rather than treats as a failure.
pub fn committed_predecessor(path: &Path) -> Result<CommittedPredecessor, NoAnchor> {
    let dir: PathBuf = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Does git run at all? Distinguished from "not a repo" because the two
    // want different things done about them.
    if Command::new("git").arg("--version").output().is_err() {
        return Err(NoAnchor::NoGit);
    }
    let toplevel = git(&dir, &["rev-parse", "--show-toplevel"]).ok_or(NoAnchor::NotARepo)?;
    let toplevel = PathBuf::from(toplevel);

    // The path as git names it: relative to the work tree root, with forward
    // slashes. Canonicalize both sides so a symlinked or `..`-laden path still
    // lands on the tracked name.
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let root = std::fs::canonicalize(&toplevel).unwrap_or(toplevel);
    let rel = abs.strip_prefix(&root).map_err(|_| NoAnchor::NotARepo)?;
    let rel = rel.to_string_lossy().replace('\\', "/");

    let branch = default_branch(&root).ok_or(NoAnchor::NoDefaultBranch)?;
    let base = git(&root, &["merge-base", "HEAD", &branch]).ok_or(NoAnchor::NoMergeBase)?;
    if base.is_empty() {
        return Err(NoAnchor::NoMergeBase);
    }

    let spec = format!("{base}:{rel}");
    let blob = git_bytes(&root, &["show", &spec]).ok_or(NoAnchor::NotCommitted)?;
    let doc: reflow2_core::GraphExport =
        serde_json::from_slice(&blob).map_err(|_| NoAnchor::NotAnExport)?;

    Ok(CommittedPredecessor {
        doc,
        source: format!("{branch}@{}", &base[..base.len().min(7)]),
    })
}
