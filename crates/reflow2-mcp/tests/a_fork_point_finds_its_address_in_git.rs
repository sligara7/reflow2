//! A decision the design never pinned to an epoch still has a fork point: the
//! earliest committed export that contains it, found in git and reported as
//! EVIDENCE (`dec:step-4-fork-point-finds-its-address-in-git-and-reopen-is-one-call`).
//! Measured on reflow2's own repository first: 37 of 454 accepted decisions
//! carry an epoch, so without this the fork point is unaddressable for the rest.

use std::path::Path;
use std::process::Command;

use reflow2_mcp::git::earliest_export_containing;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .expect("git runs")
        .success();
    assert!(ok, "git {args:?}");
}

fn export(hash: &str, ids: &[&str]) -> String {
    let nodes: Vec<String> = ids
        .iter()
        .map(|i| format!("{{\"node_id\": \"{i}\"}}"))
        .collect();
    format!(
        "{{\n  \"content_hash\": \"{hash}\",\n  \"nodes\": [{}]\n}}\n",
        nodes.join(", ")
    )
}

/// A repo whose export gains `dec:b` at the third of six commits.
fn repo() -> (tempfile::TempDir, std::path::PathBuf, Vec<String>) {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path().to_path_buf();
    git(&root, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(root.join("docs/design")).expect("mkdir");
    let path = root.join("docs/design/reflow2.json");
    let mut commits = Vec::new();
    for i in 0..6 {
        let ids: Vec<&str> = if i >= 2 {
            vec!["dec:a", "dec:b"]
        } else {
            vec!["dec:a"]
        };
        std::fs::write(&path, export(&format!("sha256:h{i}"), &ids)).expect("write");
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", &format!("c{i}")]);
        let out = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        commits.push(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    (dir, path, commits)
}

#[test]
fn the_first_commit_that_holds_the_decision_is_found_with_its_hash() {
    let (_d, path, commits) = repo();
    let ev = earliest_export_containing(&path, "dec:b").expect("found");
    assert_eq!(ev.commit, commits[2]);
    assert_eq!(ev.content_hash.as_deref(), Some("sha256:h2"));
    assert_eq!(
        ev.location,
        format!("git:{}:docs/design/reflow2.json", commits[2])
    );
    assert!(ev.reads <= 5, "bisected, not scanned: {} reads", ev.reads);
}

#[test]
fn a_decision_present_from_the_start_resolves_to_the_first_commit() {
    let (_d, path, commits) = repo();
    let ev = earliest_export_containing(&path, "dec:a").expect("found");
    assert_eq!(ev.commit, commits[0]);
}

#[test]
fn a_decision_never_committed_says_so_rather_than_guessing() {
    let (_d, path, _) = repo();
    let err = earliest_export_containing(&path, "dec:never").expect_err("absent");
    assert!(err.contains("never been committed"), "{err}");
}

#[test]
fn an_id_that_is_a_prefix_of_another_is_not_matched_by_it() {
    let (_d, path, _) = repo();
    // `dec:` alone must not match `dec:a` — the id is matched quoted.
    assert!(earliest_export_containing(&path, "dec:").is_err());
}
