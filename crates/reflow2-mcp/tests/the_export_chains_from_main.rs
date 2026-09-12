//! An export's lineage anchors at the COMMITTED record, not at the file on disk.
//!
//! # The rule this makes automatic
//!
//! `dec:export-once-per-pr` says a pull request lands exactly one hop of the
//! export chain, and that the exporting commit must be LAST. Both halves were
//! the author's to remember, and both were forgotten — because the chain was
//! built from the FILE. Two exports on one branch produced two hops, and a
//! squash-merge then published a document whose `prev_content_hash` named a
//! version on no branch anybody could see.
//!
//! Anchored at the merge-base with the default branch, the rule holds by
//! construction: any number of exports on a branch each chain from the same
//! committed ancestor, so the squash lands one hop whatever order the commits
//! were made in. `fact:auto-export-really-does-sever-the-lineage-and-a-separate-
//! path-dissolves-it` is the measurement that forced it, and it falsified the
//! hope that "no commit, no push" avoids the collision — *"because the chain is
//! built from the FILE, not from git."*
//!
//! # What these pin
//!
//! 1. **Two exports on a branch chain from main, not from each other** — the
//!    whole point, and the thing that was false before.
//! 2. **Outside git nothing is blocked**: the on-disk chain still forms, and the
//!    receipt says why it was used.
//!
//! Both are hermetic: a pid-scoped scratch repo under the system temp dir, the
//! house pattern from `an_export_says_whether_it_changed_anything.rs`, so no
//! dev-dependency is added for three paths.

use reflow2_mcp::service::*;
use rmcp::handler::server::wrapper::Parameters;

macro_rules! j {
    ($call:expr) => {
        $call
            .await
            .expect("tool ok")
            .structured_content
            .expect("structured content present")
    };
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-chain-from-main-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Run git in `dir`, failing the test with git's own stderr rather than a bare
/// status code.
fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} could not run: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn req(id: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(id.into()),
        statement: Some(format!(
            "Requirement {id} exists so the export content moves."
        )),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
    }
}

fn export(path: &std::path::Path) -> ExportGraphToReq {
    ExportGraphToReq {
        path: Some(path.display().to_string()),
        overwrite: Some(true),
        accept_divergence: None,
    }
}

/// THE ONE THAT MATTERS. Two exports, two commits, one branch — and both
/// lineages point at the same committed ancestor, so the squash lands one hop.
#[tokio::test]
async fn two_exports_on_a_branch_both_chain_from_main() {
    let dir = scratch("branch");
    git(&dir, &["init", "--quiet", "--initial-branch=main"]);
    git(&dir, &["config", "user.email", "test@example.invalid"]);
    git(&dir, &["config", "user.name", "Test"]);
    std::fs::create_dir_all(dir.join("docs")).expect("docs dir");
    let file = dir.join("docs").join("design.json");

    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(req("req:on-main"))));

    // The state of main: one committed export.
    let first = j!(s.export_graph(Parameters(export(&file))));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "the design, as committed on main"]);
    let main_hash = first["content_hash"].as_str().expect("a hash").to_string();

    // A branch, and two separate exports on it — the shape dec:export-once-per-pr
    // could not survive while the chain came from the file.
    git(&dir, &["checkout", "-q", "-b", "feature"]);

    j!(s.add_requirement(Parameters(req("req:first-on-the-branch"))));
    let second = j!(s.export_graph(Parameters(export(&file))));
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "first commit on the branch"]);

    j!(s.add_requirement(Parameters(req("req:second-on-the-branch"))));
    let third = j!(s.export_graph(Parameters(export(&file))));

    assert_eq!(
        second["prev_content_hash"].as_str(),
        Some(main_hash.as_str()),
        "the first export on the branch must chain from main: {second:?}"
    );
    assert_eq!(
        third["prev_content_hash"].as_str(),
        Some(main_hash.as_str()),
        "THE WHOLE POINT: the second export on the branch chains from main TOO, so a \
         squash-merge lands exactly ONE hop: {third:?}"
    );
    assert_ne!(
        third["prev_content_hash"].as_str(),
        second["content_hash"].as_str(),
        "chaining from the previous export on the same branch is the defect being fixed"
    );
    for r in [&second, &third] {
        let from = r["chained_from"].as_str().unwrap_or_default();
        assert!(
            from.contains("main@"),
            "the receipt must name the committed anchor, got {from:?}"
        );
    }
}

/// Outside a repository the old behaviour stands, and the receipt says why —
/// reflow2 assumes nothing about git.
#[tokio::test]
async fn outside_a_repository_the_disk_chain_still_forms_and_says_why() {
    let file = scratch("plain").join("design.json");
    let s = ReflowService::in_memory().expect("in-memory service");

    j!(s.add_requirement(Parameters(req("req:one"))));
    let first = j!(s.export_graph(Parameters(export(&file))));
    assert_eq!(
        first["chained_from"].as_str(),
        Some("nothing"),
        "a first export outside git has no ancestor to name: {first:?}"
    );

    j!(s.add_requirement(Parameters(req("req:two"))));
    let second = j!(s.export_graph(Parameters(export(&file))));

    assert_eq!(
        second["prev_content_hash"].as_str(),
        first["content_hash"].as_str(),
        "outside git the chain still grows from the file on disk — nothing is blocked"
    );
    assert_eq!(second["chained_from"].as_str(), Some("disk"), "{second:?}");
    let note = second["chain_note"].as_str().unwrap_or_default();
    assert!(
        note.contains("git repository") || note.contains("git on PATH"),
        "the receipt must say WHY the disk anchor was used, got {note:?}"
    );
}
