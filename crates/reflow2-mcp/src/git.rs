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

/// The working tree `path` sits in, as [`reflow2_core::TakenAt`]: branch (None
/// on a detached HEAD), HEAD's commit, and whether anything OTHER than the
/// export file itself is uncommitted. None outside a repository, with no git
/// on PATH, or in a repository with no commits yet — absent means nobody said,
/// never "clean on main".
pub fn taken_at(path: &Path) -> Option<reflow2_core::TakenAt> {
    let dir: PathBuf = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let toplevel = PathBuf::from(git(&dir, &["rev-parse", "--show-toplevel"])?);
    let commit = git(&dir, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    let branch =
        git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| !b.is_empty() && b != "HEAD");
    // The export itself is always about to change — that is what an export
    // does — so it is excluded from "dirty"; everything else counts.
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let root = std::fs::canonicalize(&toplevel).unwrap_or(toplevel);
    let own = abs
        .strip_prefix(&root)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"));
    let dirty = git(&dir, &["status", "--porcelain", "--untracked-files=normal"])
        .map(|out| {
            out.lines().any(|line| {
                if line.len() < 4 {
                    return false;
                }
                let file = line[3..].trim().trim_matches('"');
                let file = file.rsplit(" -> ").next().unwrap_or(file);
                own.as_deref() != Some(file)
            })
        })
        .unwrap_or(false);
    Some(reflow2_core::TakenAt {
        branch,
        commit,
        dirty,
    })
}

/// The earliest committed design export that contains a node — the fork point's
/// address for a decision the design never pinned to an epoch.
///
/// ⭐ EVIDENCE, NOT ORIGIN (`dec:step-4-fork-point-finds-its-address-in-git-and-reopen-is-one-call`):
/// this is the first commit on HEAD's first-parent line whose export holds the
/// id, which is when the RECORD first carried it — never claimed as when it was
/// decided, the same rule `dec:temporal-backfill-from-releases` set.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EarliestEvidence {
    pub commit: String,
    /// The commit date, `YYYY-MM-DD`.
    pub date: String,
    /// The export's own `content_hash` there — `None` for exports older than
    /// the hash chain, which carry none.
    pub content_hash: Option<String>,
    /// `git:<commit>:<path>` — what `reopen_decision` takes as the road taken.
    pub location: String,
    /// How many committed exports were read to find it.
    pub reads: usize,
}

/// Find [`EarliestEvidence`] for `node_id` in the history of `path`.
///
/// ⚠️ BISECTED, AND WHY: a pickaxe search (`git log -S`) over this repository's
/// 750 committed exports measured 36 s on 2026-09-23; one `git show` of an
/// export measured 0.3 s. A node, once recorded, stays in the export, so
/// presence is monotonic along the first-parent line and ~10 reads find the
/// boundary. If a node was deleted and re-added the answer is still A commit
/// that introduced it, which the caller reports as evidence rather than origin.
/// Every failure returns `Err` with the reason in words.
pub fn earliest_export_containing(path: &Path, node_id: &str) -> Result<EarliestEvidence, String> {
    let dir: PathBuf = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let toplevel = git(&dir, &["rev-parse", "--show-toplevel"])
        .ok_or_else(|| "the export is not inside a git work tree".to_string())?;
    let root = std::fs::canonicalize(&toplevel).unwrap_or_else(|_| PathBuf::from(&toplevel));
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let rel = abs
        .strip_prefix(&root)
        .map_err(|_| "the export is outside the repository".to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let list = git(
        &root,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            "HEAD",
            "--",
            &rel,
        ],
    )
    .ok_or_else(|| format!("git could not list the history of {rel}"))?;
    let commits: Vec<&str> = list.lines().filter(|l| !l.is_empty()).collect();
    if commits.is_empty() {
        return Err(format!("{rel} has no committed history"));
    }
    let needle = format!("\"{node_id}\"");
    let mut reads = 0usize;
    let mut holds = |c: &str| -> Option<Vec<u8>> {
        reads += 1;
        let blob = git_bytes(&root, &["show", &format!("{c}:{rel}")])?;
        let found = blob.windows(needle.len()).any(|w| w == needle.as_bytes());
        found.then_some(blob)
    };
    let last = commits.len() - 1;
    let Some(mut found_blob) = holds(commits[last]) else {
        return Err(format!(
            "no committed export of {rel} contains {node_id} — it has never been committed"
        ));
    };
    let (mut lo, mut hi) = (0usize, last);
    while lo < hi {
        let mid = (lo + hi) / 2;
        match holds(commits[mid]) {
            Some(blob) => {
                hi = mid;
                found_blob = blob;
            }
            None => lo = mid + 1,
        }
    }
    // `found_blob` is always the export at commits[hi]: it is set on every
    // containing read, and `hi` moves only on one.
    let commit = commits[hi].to_string();
    let date = git(&root, &["show", "-s", "--format=%cs", &commit]).unwrap_or_default();
    // The hash is the export's first key; read it without parsing 20 MB.
    let head = String::from_utf8_lossy(&found_blob[..found_blob.len().min(400)]).to_string();
    let content_hash = head
        .split("\"content_hash\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .map(str::to_string);
    Ok(EarliestEvidence {
        location: format!("git:{commit}:{rel}"),
        commit,
        date,
        content_hash,
        reads,
    })
}
