//! `export_graph --path` stamps the export with the working tree it was taken
//! in — branch, commit, dirty — and keeps that stamp while the design content
//! does not move.
//!
//! Why the EXPORT and not the node: the graph does not branch with git, so a
//! session on a feature branch exports a file carrying every branch's writes,
//! and nothing in the file said which tree it came from — caught once as four
//! phantom drifts that would have merged cleanly. A per-node coordinate would
//! be a hand-maintained fact nothing recomputes; the export is written by the
//! one layer that may read git, at the one moment the question is asked
//! (`dec:idea-should-a-node-carry-its-git-coordinate`, option ②, Anthony
//! 2026-09-16).
//!
//! Three things pinned: the receipt and the file carry branch + HEAD; `dirty`
//! ignores the export file itself and notices anything else; and an export
//! whose content did not change keeps its predecessor's coordinate, so the
//! bytes stay identical across commits and `wrote: unchanged` stays true.
//! Outside git there is no coordinate rather than a made-up one.

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
    let dir = std::env::temp_dir().join(format!("reflow2-taken-at-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

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

fn repo(name: &str) -> std::path::PathBuf {
    let dir = scratch(name);
    git(&dir, &["init", "--quiet", "--initial-branch=main"]);
    git(&dir, &["config", "user.email", "test@example.invalid"]);
    git(&dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), "hello\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "first"]);
    std::fs::create_dir_all(dir.join("docs")).expect("docs dir");
    dir
}

fn req(id: &str) -> RequirementReq {
    RequirementReq {
        id: id.into(),
        name: Some(id.into()),
        statement: Some(format!("Requirement {id} moves the content.")),
        distinct_from: None,
        status: None,
        approver: None,
        acted_at: None,
        priority: None,
        concern: None,
        kind: None,
    }
}

fn export(path: &std::path::Path) -> ExportGraphToReq {
    ExportGraphToReq {
        path: Some(path.display().to_string()),
        overwrite: Some(true),
        accept_divergence: None,
    }
}

fn taken_at_on_disk(path: &std::path::Path) -> serde_json::Value {
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    doc.get("taken_at")
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn the_export_names_the_branch_and_commit_it_was_taken_at() {
    let dir = repo("branch-and-commit");
    let file = dir.join("docs").join("design.json");
    let head = git(&dir, &["rev-parse", "HEAD"]);

    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(req("req:a"))));
    let receipt = j!(s.export_graph(Parameters(export(&file))));

    let t = &receipt["taken_at"];
    assert_eq!(t["branch"].as_str(), Some("main"), "{receipt:?}");
    assert_eq!(t["commit"].as_str(), Some(head.as_str()), "{receipt:?}");
    assert_eq!(
        t["dirty"].as_bool(),
        Some(false),
        "the export file itself is never 'dirty' — that is what an export does: {receipt:?}"
    );
    assert_eq!(
        taken_at_on_disk(&file),
        *t,
        "the file carries what the receipt says"
    );
}

#[tokio::test]
async fn uncommitted_work_other_than_the_export_reads_as_dirty() {
    let dir = repo("dirty");
    let file = dir.join("docs").join("design.json");
    std::fs::write(dir.join("README.md"), "changed and not committed\n").unwrap();

    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(req("req:a"))));
    let receipt = j!(s.export_graph(Parameters(export(&file))));
    assert_eq!(
        receipt["taken_at"]["dirty"].as_bool(),
        Some(true),
        "{receipt:?}"
    );
}

#[tokio::test]
async fn an_unchanged_design_keeps_the_coordinate_it_was_taken_at() {
    let dir = repo("kept");
    let file = dir.join("docs").join("design.json");

    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(req("req:a"))));
    let first = j!(s.export_graph(Parameters(export(&file))));
    let first_commit = first["taken_at"]["commit"].as_str().unwrap().to_string();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "the export"]);
    let bytes_then = std::fs::read(&file).unwrap();

    // HEAD moved; the design did not.
    let second = j!(s.export_graph(Parameters(export(&file))));
    assert_eq!(second["wrote"].as_str(), Some("unchanged"), "{second:?}");
    assert_eq!(
        second["taken_at"]["commit"].as_str(),
        Some(first_commit.as_str()),
        "an unchanged design is still the one taken at the earlier commit — bumping the \
         coordinate would rewrite a file whose content did not move: {second:?}"
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        bytes_then,
        "byte-identical across the commit, so `wrote: unchanged` is true"
    );

    // The design moves: the coordinate moves with it.
    j!(s.add_requirement(Parameters(req("req:b"))));
    let third = j!(s.export_graph(Parameters(export(&file))));
    assert_eq!(third["wrote"].as_str(), Some("changed"));
    assert_eq!(
        third["taken_at"]["commit"].as_str(),
        Some(git(&dir, &["rev-parse", "HEAD"]).as_str()),
        "{third:?}"
    );
}

#[tokio::test]
async fn outside_git_there_is_no_coordinate_rather_than_a_made_up_one() {
    let dir = scratch("no-git");
    let file = dir.join("design.json");
    let s = ReflowService::in_memory().expect("in-memory service");
    j!(s.add_requirement(Parameters(req("req:a"))));
    let receipt = j!(s.export_graph(Parameters(export(&file))));
    assert!(
        receipt["taken_at"].is_null(),
        "absent means nobody said: {receipt:?}"
    );
    assert!(taken_at_on_disk(&file).is_null());
}
