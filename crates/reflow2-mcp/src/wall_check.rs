//! The wall check, SERVED — the import-boundary walk a consumer can reach.
//!
//! `tools/wall_check.py` was built on 2026-08-20 from a hand-rolled script on
//! reflow2's own design and generalised the next day to "any project, with no
//! configuration". The code was generalised; the reach was not. It stayed a
//! script in this checkout: `find_tools` indexes MCP tools only, no skill named
//! it, the kit does not ship it — and on 2026-09-14 an agent on a consumer
//! project wrote the identical analysis by hand and filed it as "unknown"
//! (`fact:root-cause-the-wall-check-was-generalised-in-code-and-never-in-reach-
//! and-the-checkout-cannot-feel-it`). The checkout could not feel the absence
//! because it has the file.
//!
//! So the script is compiled INTO the binary, the way the skills are, and run
//! from here against the live design and the working tree. Nothing about the
//! analysis changed: it still reads the file set off Artifact locations and
//! REALIZES, still counts what it cannot read, still reports and never writes
//! (`dec:reflow2-serves-source-reading-analyses-that-report-and-never-write`).
//!
//! It is a Python instrument and stays one — porting 500 lines of language
//! readers to Rust to save a `python3` on PATH would be work spent on the
//! wrong thing; every consumer already runs the loop-nudge hook in python3.
//! A missing interpreter is REFUSED with the sentence that says what to do,
//! never a silent empty report.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The instrument, byte for byte the file under `tools/`. `include_str!`
/// tracks the file, so editing the script rebuilds the server.
pub const SCRIPT: &str = include_str!("../../../tools/wall_check.py");

/// Where the walk runs from: the project root the design describes.
///
/// Artifact locations are project-relative paths, so the root is the
/// directory the graph store sits under — `<root>/.reflow2/graph` — which is
/// also how the registry lays designs out. An ephemeral design has no store
/// and falls back to the process's working directory; the report names the
/// root it used either way, so a wrong one is visible rather than silent.
pub fn project_root(graph_path: Option<&str>, explicit: Option<&str>) -> PathBuf {
    if let Some(r) = explicit {
        return PathBuf::from(r);
    }
    if let Some(gp) = graph_path {
        let p = Path::new(gp);
        // `<root>/.reflow2/graph` → `<root>`; anything else → the store's parent.
        let root = match (p.parent(), p.parent().and_then(Path::parent)) {
            (Some(dot), Some(root))
                if dot.file_name().and_then(|s| s.to_str()) == Some(".reflow2") =>
            {
                root.to_path_buf()
            }
            (Some(parent), _) => parent.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let root = if root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            root
        };
        return std::fs::canonicalize(&root).unwrap_or(root);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Run the instrument over `export_json` (a full design export) with `root` as
/// the working directory, using `python` as the interpreter.
///
/// Returns the report text, or the refusal to show the caller. A refusal is
/// never an empty report: "nothing to check" is a sentence the script itself
/// prints when it has nothing to run on, and it is kept as the honest answer.
pub fn run(export_json: &str, root: &Path, python: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!(
        "reflow2-wall-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create a scratch dir: {e}"))?;
    let script = dir.join("wall_check.py");
    let export = dir.join("design.json");
    let result = (|| {
        std::fs::write(&script, SCRIPT).map_err(|e| format!("could not write the script: {e}"))?;
        std::fs::write(&export, export_json)
            .map_err(|e| format!("could not write the design export: {e}"))?;
        let out = Command::new(python)
            .arg(&script)
            .arg("--export")
            .arg(&export)
            .current_dir(root)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "`{python}` is not on this server's PATH, so the wall check was NOT run. It is \
                         a Python instrument (the same one as tools/wall_check.py in the reflow2 \
                         repo). Install python3 where reflow2-mcp runs, or run the script yourself \
                         against an export_graph file from this design."
                    )
                } else {
                    format!("could not start `{python}`: {e}")
                }
            })?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "the wall check exited with {} — its stderr, so the cause is visible rather than \
                 an empty report:\n{}",
                out.status,
                stderr.trim()
            ));
        }
        Ok(stdout)
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Bound a text report to `budget` characters, saying so. A document has no
/// field to hang a `budget` object on, so the bound is a trailing line.
pub fn bound_text(report: String, budget: usize) -> String {
    if report.chars().count() <= budget {
        return report;
    }
    let kept: String = report.chars().take(budget.saturating_sub(160)).collect();
    format!(
        "{kept}\n\n… [bounded: {} of {} chars shown; pass a larger `budget_chars` for the rest]\n",
        kept.chars().count(),
        report.chars().count()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_interpreter_is_refused_with_what_to_do_not_an_empty_report() {
        let err = run(
            "{\"nodes\":[],\"edges\":[]}",
            Path::new("."),
            "reflow2-no-such-python",
        )
        .unwrap_err();
        assert!(err.contains("not on this server's PATH"), "{err}");
        assert!(err.contains("tools/wall_check.py"), "{err}");
    }

    #[test]
    fn the_root_is_the_directory_the_store_sits_under() {
        let root = project_root(Some("/tmp/proj/.reflow2/graph"), None);
        assert_eq!(
            root.file_name().and_then(|s| s.to_str()),
            Some("proj"),
            "{root:?}"
        );
        let explicit = project_root(Some("x"), Some("/elsewhere"));
        assert_eq!(explicit.to_str(), Some("/elsewhere"));
    }

    #[test]
    fn bounding_says_so_and_never_grows() {
        let long = "x".repeat(5_000);
        let b = bound_text(long.clone(), 1_000);
        assert!(b.chars().count() <= 1_000, "{}", b.chars().count());
        assert!(b.contains("[bounded: "));
        assert_eq!(bound_text("short".into(), 1_000), "short");
    }
}
