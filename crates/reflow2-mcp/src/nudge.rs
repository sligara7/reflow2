//! Does this project actually have a session-end nudge, or does it only think
//! it does?
//!
//! `req:nudge-path-proven` — *a mechanism relied on to interrupt you is
//! worthless until you have observed it interrupt you.* Taken from the
//! StoryFlow fleet, which banned two plausible monitor implementations by name
//! after measuring **zero wakes** from them, and added a heartbeat so a missed
//! wake self-heals rather than hanging forever.
//!
//! reflow2's coherence loop leans on three triggers, and they are not equal:
//!
//! - `loop_hint` on write results, and on orientation reads — **served**, so
//!   every consumer has them, and they only fire if the agent calls something.
//! - The **Stop hook** — the backstop for a session that is finishing. It is
//!   the only one that fires when the agent has stopped calling anything, which
//!   makes it the one that matters most and the one nothing verified.
//!
//! And the finding that motivated this module: **`reflow2_init.py` installs no
//! hooks at all.** The nudge exists in reflow2's own repository and nowhere
//! else unless a consumer wires it by hand. So the honest thing is not to
//! assume it — it is to look, and to say so when it is missing.
//!
//! What this can and cannot know, stated plainly: it can see whether a Stop
//! hook is *registered* and whether the script it names *exists*. It cannot see
//! whether the harness actually ran it — that is what
//! `tools/test_nudge_path.py` does, by running the registered command exactly
//! as registered.

use std::path::{Path, PathBuf};

/// What we found when we looked for the session-end nudge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeStatus {
    /// A Stop hook is registered and the script it names is on disk.
    Installed,
    /// No Stop hook mentions the nudge — nothing will interrupt a session that
    /// finishes with the loop in debt.
    Absent,
    /// A Stop hook is registered but points at a script that is not there. The
    /// worst of the three, because the settings file *looks* right: this is a
    /// safety net that will fail silently at the moment it is needed.
    Broken { command: String },
    /// The project directory could not be determined, so nothing was checked.
    /// Never reported as "absent" — claiming a net is missing when we simply
    /// did not look is the same class of lie in the other direction.
    Unknown,
}

impl NudgeStatus {
    /// The sentence for the agent, or `None` when there is nothing to say.
    pub fn advisory(&self) -> Option<String> {
        match self {
            NudgeStatus::Installed | NudgeStatus::Unknown => None,
            NudgeStatus::Absent => Some(
                "NO SESSION-END NUDGE IS INSTALLED in this project, so nothing will remind you \
                 when the coherence loop is owed something. Call `loop_status` before you finish \
                 any session in which you changed the design, and after a batch of captures — it \
                 is one cheap call that says what is owed."
                    .to_string(),
            ),
            NudgeStatus::Broken { command } => Some(format!(
                "THE SESSION-END NUDGE IS REGISTERED BUT BROKEN: the hook runs `{command}`, and \
                 that script is not there. It will fail silently exactly when it is needed, which \
                 is worse than having none — fix the path or remove the hook, and until then call \
                 `loop_status` yourself before finishing."
            )),
        }
    }
}

/// Look for the nudge, from the graph's location.
///
/// The project is the grandparent of `<project>/.reflow2/graph`; anything else
/// falls back to the working directory, which is where every configured harness
/// launches the server (the same assumption the relative graph path rests on).
pub fn status(graph_path: Option<&str>) -> NudgeStatus {
    let Some(project) = project_dir(graph_path) else {
        return NudgeStatus::Unknown;
    };
    // Both files a harness reads, local first: a project that sets it locally
    // has it, whatever the shared file says.
    let candidates = [
        project.join(".claude/settings.local.json"),
        project.join(".claude/settings.json"),
    ];
    for settings in candidates {
        let Ok(text) = std::fs::read_to_string(&settings) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(command) = stop_hook_command(&json) {
            return match script_path(&command, &project) {
                Some(script) if script.exists() => NudgeStatus::Installed,
                Some(_) => NudgeStatus::Broken { command },
                // A command we cannot parse a path out of is somebody's own
                // wrapper; assume they know what they are doing rather than
                // calling their setup broken.
                None => NudgeStatus::Installed,
            };
        }
    }
    NudgeStatus::Absent
}

/// The `Stop` hook command that mentions the nudge, if any.
fn stop_hook_command(settings: &serde_json::Value) -> Option<String> {
    let stop = settings.get("hooks")?.get("Stop")?.as_array()?;
    for group in stop {
        let Some(hooks) = group.get("hooks").and_then(|h| h.as_array()) else {
            continue;
        };
        for hook in hooks {
            let Some(command) = hook.get("command").and_then(|c| c.as_str()) else {
                continue;
            };
            if command.contains("loop_nudge") {
                return Some(command.to_string());
            }
        }
    }
    None
}

/// Pull the script path out of a hook command, resolving the forms a hook
/// legitimately uses: `$CLAUDE_PROJECT_DIR`, and `~` / `$HOME`.
///
/// **`None` means "we cannot say where this points" — never "it is missing".**
/// That distinction is the whole point of this function: `status` reads `None`
/// as `Installed`, so anything we fail to resolve is left alone rather than
/// called broken. Reporting a working safety net as broken is the expensive
/// direction, because it trains the reader to ignore the one advisory whose job
/// is to be trusted when nobody is watching.
///
/// # Why `~` needs handling at all
///
/// A shell expands `~` when the hook actually runs, so the hook WORKS; but a
/// literal `~/...` string is not an absolute path, so it used to fall through to
/// `project.join("~/.local/...")` — a path that can never exist — and every
/// `loop_status` reported the nudge BROKEN. Reported from flo2 on 2026-08-09,
/// where it cost a real commit documenting a problem that did not exist.
///
/// The installer never produces this form (`reflow2_install.py` writes a quoted
/// absolute path), which is why it stayed invisible here; `getting-started/AGENTS.md`
/// documents it for hand-wiring, which is the path flo2 took.
fn script_path(command: &str, project: &Path) -> Option<PathBuf> {
    script_path_with(command, project, home_dir().as_deref())
}

/// The home directory, or `None` where the environment does not say.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// `script_path` with the home directory supplied, so it is testable without
/// mutating process-wide environment that other tests share.
fn script_path_with(command: &str, project: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let raw = command
        .split_whitespace()
        .find(|t| t.contains("loop_nudge"))?;
    let cleaned = raw.trim_matches(|c| c == '"' || c == '\'');
    let resolved = cleaned
        .replace("${CLAUDE_PROJECT_DIR}/", "")
        .replace("$CLAUDE_PROJECT_DIR/", "");

    let resolved = match home_relative(&resolved) {
        // `~/x`, `$HOME/x`, `${HOME}/x` — resolvable only if we know home.
        Some(rest) => home?.join(rest).to_string_lossy().into_owned(),
        // A bare `~user/x` names somebody else's home and no environment
        // variable answers it. Decline rather than guess.
        None if resolved.starts_with('~') => return None,
        None => resolved,
    };

    // Any variable we did not expand means the path is unknown to us — a
    // wrapper, a pipeline, someone's own indirection. Not our business.
    if resolved.contains('$') {
        return None;
    }

    let path = Path::new(&resolved);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.join(path)
    })
}

/// The tail after a leading home marker, if the path is home-relative.
fn home_relative(path: &str) -> Option<&str> {
    ["~/", "$HOME/", "${HOME}/"]
        .iter()
        .find_map(|prefix| path.strip_prefix(prefix))
}

/// `<project>/.reflow2/graph` → `<project>`; otherwise the working directory.
fn project_dir(graph_path: Option<&str>) -> Option<PathBuf> {
    if let Some(graph_path) = graph_path {
        let p = std::fs::canonicalize(graph_path).unwrap_or_else(|_| PathBuf::from(graph_path));
        let under_reflow2 = p.parent().and_then(Path::file_name) == Some(".reflow2".as_ref());
        if let Some(project) = p.parent().and_then(Path::parent).filter(|_| under_reflow2) {
            return Some(project.to_path_buf());
        }
    }
    std::env::current_dir().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with(settings: &str) -> tempdir::TempProject {
        tempdir::TempProject::new(settings)
    }

    #[test]
    fn a_registered_hook_whose_script_exists_is_installed() {
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 \"$CLAUDE_PROJECT_DIR/tools/loop_nudge.py\""}]}]}}"#,
        );
        std::fs::create_dir_all(p.dir.join("tools")).unwrap();
        std::fs::write(
            p.dir.join("tools/loop_nudge.py"),
            "#!/usr/bin/env python3\n",
        )
        .unwrap();
        assert_eq!(status(Some(&p.graph())), NudgeStatus::Installed);
    }

    #[test]
    fn a_registered_hook_pointing_at_nothing_is_broken_not_installed() {
        // The dangerous middle case: the settings file looks right, and the net
        // fails silently at the moment it is needed.
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 \"$CLAUDE_PROJECT_DIR/tools/loop_nudge.py\""}]}]}}"#,
        );
        let NudgeStatus::Broken { command } = status(Some(&p.graph())) else {
            panic!("a hook pointing at a missing script must not read as installed");
        };
        assert!(command.contains("loop_nudge"));
    }

    #[test]
    fn no_hook_is_absent_and_says_what_to_do_instead() {
        let p = project_with(r#"{"hooks":{}}"#);
        let status = status(Some(&p.graph()));
        assert_eq!(status, NudgeStatus::Absent);
        let advisory = status.advisory().unwrap();
        assert!(advisory.contains("loop_status"), "{advisory}");
    }

    #[test]
    fn a_settings_file_that_is_not_json_does_not_read_as_installed() {
        let p = project_with("{ not json");
        assert_eq!(status(Some(&p.graph())), NudgeStatus::Absent);
    }

    // ---------------------------------------------------------------- `~`
    //
    // Reported from flo2, 2026-08-09: a hook wired by hand from
    // getting-started/AGENTS.md uses `~/.local/share/reflow2/kit/tools/loop_nudge.py`,
    // the script is there, the hook demonstrably runs — and every `loop_status`
    // said BROKEN, because a literal `~` is not an absolute path and the check
    // joined it onto the project directory.

    /// The home directory is supplied rather than read from the environment,
    /// so these never race with other tests over a process-wide `HOME`.
    fn resolve(command: &str, home: &Path) -> Option<PathBuf> {
        script_path_with(command, Path::new("/nonexistent-project"), Some(home))
    }

    #[test]
    fn a_tilde_path_resolves_against_home_rather_than_the_project() {
        let got = resolve(r#"python3 ~/kit/tools/loop_nudge.py"#, Path::new("/home/x"));
        assert_eq!(got, Some(PathBuf::from("/home/x/kit/tools/loop_nudge.py")));
    }

    #[test]
    fn home_variable_forms_resolve_the_same_way() {
        for command in [
            r#"python3 "$HOME/kit/tools/loop_nudge.py""#,
            r#"python3 "${HOME}/kit/tools/loop_nudge.py""#,
        ] {
            assert_eq!(
                resolve(command, Path::new("/home/x")),
                Some(PathBuf::from("/home/x/kit/tools/loop_nudge.py")),
                "{command}"
            );
        }
    }

    /// The positive control. Expanding `~` must not turn the check into one that
    /// approves everything — a home-relative path to a script that is genuinely
    /// absent is still the dangerous middle case this whole module exists for.
    #[test]
    fn a_tilde_path_pointing_at_nothing_is_still_broken() {
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 ~/.reflow2-no-such-dir-8f3a/loop_nudge.py"}]}]}}"#,
        );
        let NudgeStatus::Broken { command } = status(Some(&p.graph())) else {
            panic!("expanding ~ must not make a missing script read as installed");
        };
        assert!(command.contains("loop_nudge"));
    }

    #[test]
    fn a_tilde_path_whose_script_exists_is_installed() {
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 ~/kit/loop_nudge.py"}]}]}}"#,
        );
        // Point "home" at the temp project itself, so no real home is touched.
        std::fs::create_dir_all(p.dir.join("kit")).unwrap();
        std::fs::write(p.dir.join("kit/loop_nudge.py"), "#!/usr/bin/env python3\n").unwrap();
        assert_eq!(
            resolve("python3 ~/kit/loop_nudge.py", &p.dir),
            Some(p.dir.join("kit/loop_nudge.py"))
        );
        assert!(
            resolve("python3 ~/kit/loop_nudge.py", &p.dir)
                .unwrap()
                .exists()
        );
    }

    /// What we cannot resolve, we do not judge — `None` becomes `Installed`.
    #[test]
    fn a_path_we_cannot_resolve_is_never_called_broken() {
        for command in [
            // Somebody else's home: no variable answers this.
            "python3 ~someone/kit/loop_nudge.py",
            // A variable we do not know.
            "python3 $MY_KIT/tools/loop_nudge.py",
            "python3 ${XDG_DATA_HOME}/reflow2/loop_nudge.py",
        ] {
            assert_eq!(
                resolve(command, Path::new("/home/x")),
                None,
                "{command} — an unresolvable path must yield None, which status() reads as Installed"
            );
        }
    }

    /// And with no home in the environment at all, a `~` path is unknowable
    /// rather than missing.
    #[test]
    fn without_a_home_a_tilde_path_is_unknowable_not_missing() {
        let got = script_path_with(
            "python3 ~/kit/loop_nudge.py",
            Path::new("/nonexistent-project"),
            None,
        );
        assert_eq!(got, None);
    }

    /// The form the installer actually writes must keep working — a quoted
    /// absolute path. This is why the bug stayed invisible in this repo.
    #[test]
    fn the_installers_own_quoted_absolute_form_still_resolves() {
        let got = resolve(
            r#"python3 "/home/x/.local/share/reflow2/kit/tools/loop_nudge.py""#,
            Path::new("/home/x"),
        );
        assert_eq!(
            got,
            Some(PathBuf::from(
                "/home/x/.local/share/reflow2/kit/tools/loop_nudge.py"
            ))
        );
    }

    mod tempdir {
        pub struct TempProject {
            pub dir: std::path::PathBuf,
        }
        impl TempProject {
            pub fn new(settings: &str) -> Self {
                let dir = std::env::temp_dir().join(format!(
                    "reflow2-nudge-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                std::fs::create_dir_all(dir.join(".claude")).unwrap();
                std::fs::create_dir_all(dir.join(".reflow2")).unwrap();
                std::fs::write(dir.join(".claude/settings.json"), settings).unwrap();
                Self { dir }
            }
            pub fn graph(&self) -> String {
                self.dir.join(".reflow2/graph").display().to_string()
            }
        }
        impl Drop for TempProject {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.dir);
            }
        }
    }
}
