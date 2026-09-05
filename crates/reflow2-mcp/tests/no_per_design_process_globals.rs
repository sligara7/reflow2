//! Per-design state never lives in a process-global.
//!
//! `ver:no-per-design-process-globals`, the detector owed by
//! `rule:per-design-state-is-never-a-process-global` from the moment Anthony set
//! it `enforced: true` on 2026-08-09. An enforced rule that nothing can check is
//! a comment with a stop sign painted on it — `dec:does-enforced-default-to-gate-blocking`
//! removed the old `enforced` default precisely so a rule could not quietly bill
//! itself for a detector nobody wrote. This is that bill, paid.
//!
//! ## Why a source scan and not a runtime assertion
//!
//! The failure this guards against is **erosion, not a decision**. Nobody will
//! ever set out to make reflow2 single-design; the door closes one convenient
//! `static` at a time, each individually reasonable, and the cost only appears
//! at the migration when `cap:graph-registry` tries to hold two designs in one
//! process and finds the second one overwriting the first's path. There is no
//! runtime moment at which that is observable, because with one design open the
//! wrong code is indistinguishable from the right code. The declaration is the
//! only place the mistake is visible, so the declaration is where we look.
//!
//! ## What this proves, and what it does NOT
//!
//! It FAILS when a process-global appears that the allowlist below does not
//! name. The cost of adding a global is therefore stating why it is not
//! per-design — which is the whole intervention. That is a real fault it
//! detects, not a token attached to turn a finding green.
//!
//! It says **nothing** about whether a struct field is correctly scoped. The
//! per-design metadata riding on `ReflowService` today (`graph_path`,
//! `write_gen`, `read_hint`) is not a global, and moving it per-graph is
//! `cap:graph-registry`'s planned work rather than a violation of this rule. A
//! check that claimed to cover that too would be the green-washing the
//! governance skill warns about: *"a graph node green-washes exactly like a
//! document."* Stating the bound here is what keeps the `passing` status honest.
//!
//! The sibling clause — a design is named by an id, never by a path — is
//! `rule:a-design-is-named-by-an-id-not-a-path` and is deliberately ADVISORY.
//! It has no compliant surface today, so it gets no test rather than a vacuous
//! one.

use std::path::{Path, PathBuf};

/// The process-globals that legitimately exist, each with the reason it is a
/// fact about the PROCESS rather than about a design.
///
/// Adding a line here is not a formality — it is the rule being applied. The
/// question to answer before you do is the one the rule states: *would this need
/// a different value for a second design open in the same process?* If yes, it
/// belongs on the design handle and this list is the wrong fix.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "reflow2-mcp/src/shared.rs",
        "STARTUP_FINGERPRINT",
        "This process's own executable, fingerprinted (size:mtime) at start, so \
         `served_by.stale` has an answer where there is no /proc/self/exe (macOS). \
         A second design open in this process runs on the SAME executable, so the \
         value could not differ per design — it is a fact about the process. \
         fact:defect-currency-is-read-from-proc-self-exe-so-every-non-linux-run-\
         answers-unknown-on-every-call.",
    ),
    (
        "reflow2-core/src/identity.rs",
        "SEAT",
        "This process's own seat id, memoised inside `seat_id()`. A seat names a \
         SESSION, not a design — `dec:identity-out-of-band`, minted with no \
         coordination. A second design open in this process does not change who \
         this process is. Note the per-client path already exists next to it \
         (`SeatLease`, `req:seat-per-client`), which is what a shared daemon \
         hands out.",
    ),
    (
        "reflow2-core/src/identity.rs",
        "ATTACHED_SEATS",
        "The attached-seat registry, so liveness is COMPUTED rather than stored. \
         Named in the rule's own statement as legitimately process-wide: it \
         answers 'is that session still here', which is a question about this \
         process's clients and not about any design's contents.",
    ),
    (
        "reflow2-core/src/identity.rs",
        "SERVES_MANY_SESSIONS",
        "The `--serve-shared` mode flag. Also named in the rule's statement. It \
         records how THIS PROCESS was launched — a shared daemon serving one \
         client is indistinguishable from stdio by any other means, which is why \
         the mode is declared rather than guessed. Cannot differ per design.",
    ),
    (
        "reflow2-core/src/schema.rs",
        "PARSED_SCHEMA",
        "The eleven schema domains, parsed once. This is a fact about the \
         BINARY, not about a design: the YAML is `include_str!`'d at compile \
         time, so its bytes are identical for every design this process will \
         ever open, and parsing them twice cannot produce two different \
         answers. Applying the rule's own question — would a second design open \
         in this process need a different value? — the answer is no, and it \
         cannot become yes without the schema being loaded from disk, which \
         would be a different design decision arriving through a different \
         door. Note what is NOT shared: `load_schema()` hands out a CLONE, \
         because `StorageEngine` takes the schema by value and two graphs must \
         not share one. The cache is the parse; the schema each design gets is \
         its own.",
    ),
];

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ dir")
        .to_path_buf()
}

/// Every `.rs` file under each crate's `src/`.
///
/// `src` only, deliberately: a `static` in a test is test scaffolding and cannot
/// reach a running server. `build.rs` is excluded for the same reason — it emits
/// immutable embedded text into `OUT_DIR` at compile time and holds no runtime
/// state.
fn source_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = std::fs::read_dir(crates_dir())
        .expect("read crates/")
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("src"))
        .filter(|p| p.is_dir())
        .collect();

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    assert!(
        out.len() > 10,
        "found only {} source files — the walk is broken, and a broken walk \
         passes this test vacuously",
        out.len()
    );
    out
}

/// Path relative to `crates/`, with forward slashes, for stable comparison.
fn relative(path: &Path) -> String {
    path.strip_prefix(crates_dir())
        .expect("under crates/")
        .to_string_lossy()
        .replace('\\', "/")
}

/// The name a global is declared under, if this line declares one.
///
/// Anchored on the `static` keyword at the start of an item, which is what every
/// process-global declaration has in common — including one declared inside a
/// function body, which is still process-wide. `&'static` lifetimes and
/// `const` items never match: the first never begins an item, and the second is
/// immutable and carries no state.
fn declared_global(line: &str) -> Option<String> {
    let trimmed = line.trim_start();

    for macro_form in ["thread_local!", "lazy_static!"] {
        if trimmed.starts_with(macro_form) {
            return Some(macro_form.to_string());
        }
    }

    let rest = trimmed
        .strip_prefix("pub(crate) ")
        .or_else(|| trimmed.strip_prefix("pub "))
        .unwrap_or(trimmed);
    let rest = rest.strip_prefix("static ")?;
    let rest = rest.strip_prefix("mut ").unwrap_or(rest);

    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

#[test]
fn no_process_global_holds_per_design_state() {
    let mut found: Vec<(String, String, usize)> = Vec::new();

    for file in source_files() {
        let rel = relative(&file);
        let text = std::fs::read_to_string(&file).expect("read source file");
        for (i, line) in text.lines().enumerate() {
            if let Some(name) = declared_global(line) {
                found.push((rel.clone(), name, i + 1));
            }
        }
    }

    let unexpected: Vec<_> = found
        .iter()
        .filter(|(file, name, _)| !ALLOWED.iter().any(|(af, an, _)| af == file && an == name))
        .collect();

    assert!(
        unexpected.is_empty(),
        "rule:per-design-state-is-never-a-process-global is ENFORCED, and these \
         process-globals are not accounted for:\n{}\n\nAsk the rule's own \
         question about each one: WOULD THIS NEED A DIFFERENT VALUE FOR A SECOND \
         DESIGN OPEN IN THE SAME PROCESS?\n\n  - If yes, it is per-design state \
         and belongs on the design handle, not in a `static`. Putting it here is \
         the erosion cap:graph-registry then has to undo.\n  - If no, it is a \
         fact about the process (how it was launched, who is attached to it) and \
         belongs in ALLOWED in {}, with the reason written out.\n\nThe reason is \
         the point. A name added to the list without one leaves the next reader \
         unable to tell a considered exception from a convenient one.",
        unexpected
            .iter()
            .map(|(f, n, l)| format!("  {f}:{l}  {n}"))
            .collect::<Vec<_>>()
            .join("\n"),
        file!(),
    );
}

/// The allowlist must not outlive what it describes.
///
/// A stale exemption is worse than a missing one: it silently pre-authorises a
/// name, so re-introducing a global that was removed for good reason sails
/// through. This is the same braces-to-belt move `sessions_cannot_cross_designs`
/// makes — the guard's own assumptions get a guard.
#[test]
fn every_allowlist_entry_still_describes_a_real_global() {
    let mut found: Vec<(String, String)> = Vec::new();
    for file in source_files() {
        let rel = relative(&file);
        let text = std::fs::read_to_string(&file).expect("read source file");
        for line in text.lines() {
            if let Some(name) = declared_global(line) {
                found.push((rel.clone(), name));
            }
        }
    }

    for (file, name, reason) in ALLOWED {
        assert!(
            found.iter().any(|(f, n)| f == file && n == name),
            "ALLOWED names {name} in {file}, but no such process-global exists \
             any more. Delete the entry rather than leaving it: a stale \
             exemption pre-authorises the name for whoever adds it back.\n\
             Recorded reason was: {reason}"
        );
    }
}

/// Positive control: the detector must actually fail on a violation.
///
/// Without this, every assertion above is also satisfied by a scan that finds
/// nothing — a broken walk, a regex that never matches, an allowlist compared
/// the wrong way round. That failure mode is invisible in a green run, and this
/// project has shipped it before.
#[test]
fn the_detector_fires_on_an_unaccounted_global() {
    assert_eq!(
        declared_global("static GRAPH_PATH: OnceLock<String> = OnceLock::new();").as_deref(),
        Some("GRAPH_PATH"),
        "a plain per-design static must be detected"
    );
    assert_eq!(
        declared_global("    static SEAT: std::sync::OnceLock<String> = OnceLock::new();")
            .as_deref(),
        Some("SEAT"),
        "a static inside a function body is still process-wide"
    );
    assert_eq!(
        declared_global("pub(crate) static mut CACHE: u8 = 0;").as_deref(),
        Some("CACHE"),
        "visibility and `mut` must not hide a global"
    );
    assert_eq!(
        declared_global("thread_local! { static X: u8 = 0; }").as_deref(),
        Some("thread_local!"),
        "macro-declared globals must be detected"
    );

    // ...and must NOT fire on things that are not process state.
    assert_eq!(
        declared_global("pub const CURRENT_NOTE: &str = \"current\";"),
        None,
        "a const is immutable and holds no state"
    );
    assert_eq!(
        declared_global("    fn registry() -> &'static Mutex<SeatRegistry> {"),
        None,
        "a &'static lifetime is not a global"
    );

    // And the allowlist comparison itself: a name allowed in one file must not
    // be silently allowed in another.
    let elsewhere = ("reflow2-mcp/src/service.rs".to_string(), "SEAT".to_string());
    assert!(
        !ALLOWED
            .iter()
            .any(|(af, an, _)| *af == elsewhere.0 && *an == elsewhere.1),
        "the allowlist must be keyed on file AND name, or an exemption leaks \
         across modules"
    );
}
