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
    /// No hook, and none is possible: this project named a harness reflow2 has
    /// no event model for, so there is nothing to install and nothing to fix.
    ///
    /// **This exists because `Absent` was answering two different questions
    /// with one word.** "Nobody installed it" is actionable — install it.
    /// "Your harness cannot carry one" is permanent, and telling that user to
    /// install it sends them after something that does not exist. The pair is
    /// the same distinction `Unknown` already draws against `Absent` at the
    /// other end: *could not look* is not *looked and found nothing*, and
    /// *cannot be installed* is not *is not installed*.
    NoHookForThisHarness { harnesses: String },
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
            // Reports what reflow2 LOOKED AT, not a claim about the world.
            //
            // This used to read "NO SESSION-END NUDGE IS INSTALLED… so nothing
            // will remind you". dev_storyflow filed it on 2026-08-09: their
            // `loop_status` said exactly that, and a Stop hook fired at them
            // minutes later — the very reminder the sentence said did not
            // exist. Their hook is harness-side rather than one reflow2
            // installed, so the FIELD was right and the SENTENCE was not.
            //
            // Why the wording matters more than it looks: the advisory exists to
            // make a session self-police because nothing else will, and **a
            // session that believes it is unwatched budgets differently from one
            // that knows a hook will catch it.** Arguing for the first while the
            // second is true is the wrong error to make.
            NudgeStatus::Absent => Some(
                "REFLOW2 HAS NOT INSTALLED A SESSION-END NUDGE in this project — your harness \
                 may have one of its own, which reflow2 cannot see. If nothing reminds you, \
                 nothing will say the coherence loop is owed something. Call `loop_status` \
                 before you finish any session in which you changed the design, and after a \
                 batch of captures — it is one cheap call that says what is owed."
                    .to_string(),
            ),
            NudgeStatus::Broken { command } => Some(format!(
                "THE SESSION-END NUDGE IS REGISTERED BUT BROKEN: the hook runs `{command}`, and \
                 that script is not there. It will fail silently exactly when it is needed, which \
                 is worse than having none — fix the path or remove the hook, and until then call \
                 `loop_status` yourself before finishing."
            )),
            // Deliberately NOT phrased as a shortfall. Nothing here is missing,
            // nobody skipped a step, and there is no command that would fix it —
            // so the sentence says what is true and hands over the one thing the
            // reader can actually do. Telling somebody to install a hook their
            // harness cannot hold is how an advisory teaches people to skip
            // advisories.
            NudgeStatus::NoHookForThisHarness { harnesses } => Some(format!(
                "THERE IS NO SESSION-END NUDGE FOR THIS HARNESS, and none is possible: this \
                 project is set up for {harnesses}, and reflow2 has a trigger only for Claude \
                 Code (a Stop hook) and OpenCode (a plugin). \
                 Nothing is missing and there is nothing to install — the coherence loop is \
                 yours to run. Call `loop_status` before you finish any session in which you \
                 changed the design, and after a batch of captures."
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
    status_with_home(
        graph_path,
        std::env::var_os("HOME").map(std::path::PathBuf::from),
    )
}

/// [`status`] with the home directory passed in, so a test can look at a
/// project without also seeing whatever the developer's own machine has
/// installed under `~/.config/opencode/plugins/`.
pub fn status_with_home(graph_path: Option<&str>, home: Option<PathBuf>) -> NudgeStatus {
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
    // THE OPENCODE HALF (2026-09-14, PR #511): OpenCode has no hook
    // configuration to register into; it loads plugin FILES from a directory,
    // and the kit ships an adapter there that feeds OpenCode's hooks to the
    // same loop_nudge.py. So the presence test for OpenCode is the file, per
    // project or machine-wide. Checked here rather than inferred from the
    // harness stamp, for the same reason the Claude search reads the settings
    // file: a project that HAS a working nudge must never be told otherwise.
    if opencode_plugin_present(&project, home.as_deref()) {
        return NudgeStatus::Installed;
    }
    // No hook and no plugin. Before calling that a shortfall, ask whether one
    // was ever possible here — the installer records which harness this
    // project named, and reflow2 has a trigger for two of them: a Stop hook on
    // Claude Code and a plugin on OpenCode.
    //
    // ORDER MATTERS: the searches above run FIRST and win. Somebody on any
    // harness may have wired their own Stop hook, and a project that HAS a
    // working nudge must never be told its harness cannot have one.
    match recorded_harnesses(&project) {
        Some(harnesses)
            if !harnesses
                .iter()
                .any(|h| TRIGGER_HARNESSES.contains(&h.as_str())) =>
        {
            NudgeStatus::NoHookForThisHarness {
                harnesses: harnesses.join(", "),
            }
        }
        // Recorded and includes Claude, or nothing recorded at all. The second
        // is the pre-`--harness` project and every project set up before the
        // installer asked: it could have a hook, so its absence is a real gap.
        _ => NudgeStatus::Absent,
    }
}

/// The harnesses reflow2 has a loop trigger for: a Stop hook registered into
/// Claude Code's settings, and a plugin file OpenCode loads from its plugin
/// directory (PR #511, 2026-09-14). Anything else has no event model reflow2
/// can reach, and is told so rather than told to install something.
const TRIGGER_HARNESSES: &[&str] = &["claude", "opencode"];

/// The kit's OpenCode plugin, by the name the installers write it under.
const OPENCODE_PLUGIN: &str = "reflow2-loop-nudge.js";

/// Is the OpenCode loop-nudge plugin installed for this project — in the
/// project's own `.opencode/plugins/`, or machine-wide in
/// `~/.config/opencode/plugins/`, the two directories OpenCode loads plugins
/// from? Either one reaches a session opened in this project.
fn opencode_plugin_present(project: &Path, home: Option<&Path>) -> bool {
    if project
        .join(".opencode/plugins")
        .join(OPENCODE_PLUGIN)
        .exists()
    {
        return true;
    }
    home.is_some_and(|home| {
        home.join(".config/opencode/plugins")
            .join(OPENCODE_PLUGIN)
            .exists()
    })
}

/// Which harnesses this project was set up for, as `reflow2_init.py` recorded
/// them in `.reflow2/kit-version.json`.
///
/// **`None` means nobody ever said** — an older project, or one set up before
/// the installer asked. It is deliberately not read as "no harnesses": the
/// whole point of this module is that *not knowing* and *knowing it is absent*
/// are different answers, and inventing the second from the first here would
/// reproduce the defect one layer down.
fn recorded_harnesses(project: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(project.join(".reflow2/kit-version.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let listed = json.get("harnesses")?.as_array()?;
    let names: Vec<String> = listed
        .iter()
        .filter_map(|h| h.as_str().map(str::to_string))
        .collect();
    // An empty or unreadable list is nobody's answer, not an empty answer.
    (!names.is_empty()).then_some(names)
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
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Installed
        );
    }

    #[test]
    fn a_registered_hook_pointing_at_nothing_is_broken_not_installed() {
        // The dangerous middle case: the settings file looks right, and the net
        // fails silently at the moment it is needed.
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 \"$CLAUDE_PROJECT_DIR/tools/loop_nudge.py\""}]}]}}"#,
        );
        let NudgeStatus::Broken { command } = status_with_home(Some(&p.graph()), None) else {
            panic!("a hook pointing at a missing script must not read as installed");
        };
        assert!(command.contains("loop_nudge"));
    }

    #[test]
    fn no_hook_is_absent_and_says_what_to_do_instead() {
        let p = project_with(r#"{"hooks":{}}"#);
        let status = status_with_home(Some(&p.graph()), None);
        assert_eq!(status, NudgeStatus::Absent);
        let advisory = status.advisory().unwrap();
        assert!(advisory.contains("loop_status"), "{advisory}");
    }

    /// The advisory reports what reflow2 LOOKED AT, never a claim about the
    /// world it cannot see.
    ///
    /// dev_storyflow, 2026-08-09: `loop_status` told them "nothing will remind
    /// you" and a Stop hook fired at them minutes later. Their hook was
    /// harness-side, so reflow2's FIELD was right and its SENTENCE was not — and
    /// a session that believes it is unwatched budgets differently from one that
    /// knows a hook will catch it.
    #[test]
    fn the_absent_advisory_does_not_claim_nothing_else_will_remind_you() {
        let p = project_with(r#"{"hooks":{}}"#);
        let advisory = status_with_home(Some(&p.graph()), None).advisory().unwrap();
        assert!(
            advisory.contains("REFLOW2 HAS NOT INSTALLED"),
            "it must say what reflow2 did, not what the world contains: {advisory}"
        );
        assert!(
            advisory.contains("your harness"),
            "and must allow for a hook reflow2 cannot see: {advisory}"
        );
        assert!(
            !advisory.contains("so nothing will remind you"),
            "the overclaim must be gone: {advisory}"
        );
    }

    #[test]
    fn a_settings_file_that_is_not_json_does_not_read_as_installed() {
        let p = project_with("{ not json");
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Absent
        );
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
        let NudgeStatus::Broken { command } = status_with_home(Some(&p.graph()), None) else {
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

    /// Write the installer's stamp, naming which harnesses set this project up.
    fn set_up_for(p: &tempdir::TempProject, harnesses: &[&str]) {
        let listed = harnesses
            .iter()
            .map(|h| format!("\"{h}\""))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            p.dir.join(".reflow2/kit-version.json"),
            format!(r#"{{"reflow2_version":"0.30.0","harnesses":[{listed}]}}"#),
        )
        .unwrap();
    }

    #[test]
    fn a_harness_that_cannot_hold_a_hook_is_not_reported_as_a_missing_one() {
        // The defect this variant exists for: `Absent` answered two questions
        // with one word, and sent a user after something that does not exist.
        // (OpenCode was the example until 2026-09-14, when it got a plugin —
        // so the harness with no trigger is now VS Code.)
        let p = project_with("{}");
        set_up_for(&p, &["vscode"]);
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::NoHookForThisHarness {
                harnesses: "vscode".to_string()
            }
        );
    }

    #[test]
    fn the_advisory_for_an_impossible_hook_asks_for_nothing_to_be_installed() {
        // An advisory that tells somebody to fix an unfixable thing is how a
        // reader learns to skip advisories.
        let p = project_with("{}");
        set_up_for(&p, &["vscode"]);
        let said = status_with_home(Some(&p.graph()), None)
            .advisory()
            .expect("must say something");
        assert!(said.contains("none is possible"), "{said}");
        assert!(said.contains("vscode"), "it must name the harness: {said}");
        assert!(
            said.contains("loop_status"),
            "saying the trigger is absent is only useful with what to do instead: {said}"
        );
        assert!(
            !said.to_lowercase().contains("install a"),
            "must not ask for an install that cannot happen: {said}"
        );
    }

    #[test]
    fn a_project_set_up_for_claude_still_reports_a_missing_hook_as_missing() {
        // The counterweight. Narrowing must not silence the real gap.
        let p = project_with("{}");
        set_up_for(&p, &["claude"]);
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Absent
        );
    }

    #[test]
    fn a_hook_the_user_wired_themselves_wins_over_the_recorded_harness() {
        // Order matters: somebody on any harness may have wired their own Stop
        // hook, and a project that HAS a working nudge must never be told its
        // harness cannot have one.
        let p = project_with(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command",
               "command":"python3 \"$CLAUDE_PROJECT_DIR/tools/loop_nudge.py\""}]}]}}"#,
        );
        set_up_for(&p, &["opencode"]);
        std::fs::create_dir_all(p.dir.join("tools")).unwrap();
        std::fs::write(
            p.dir.join("tools/loop_nudge.py"),
            "#!/usr/bin/env python3\n",
        )
        .unwrap();
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Installed
        );
    }

    #[test]
    fn a_project_that_never_said_which_harness_is_absent_not_impossible() {
        // Every project set up before the installer asked. Not knowing must not
        // become "cannot" — that would reproduce, one layer down, the very
        // conflation this variant was added to fix.
        let p = project_with("{}");
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Absent
        );
    }

    #[test]
    fn a_multi_harness_project_including_claude_can_still_have_a_hook() {
        let p = project_with("{}");
        set_up_for(&p, &["opencode", "claude"]);
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Absent
        );
    }

    #[test]
    fn the_two_absences_serialise_differently_so_a_reader_can_tell_them_apart() {
        // loop_status carries this as a machine-readable field. If both states
        // serialised the same, the distinction would exist in Rust and nowhere
        // a consumer could act on it.
        let absent = serde_json::to_value(NudgeStatus::Absent).unwrap();
        let impossible = serde_json::to_value(NudgeStatus::NoHookForThisHarness {
            harnesses: "opencode".to_string(),
        })
        .unwrap();
        assert_ne!(absent, impossible);
        assert_eq!(absent, serde_json::json!("absent"));
        assert_eq!(
            impossible,
            serde_json::json!({"no_hook_for_this_harness": {"harnesses": "opencode"}})
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

    /// PR #511 (2026-09-14): OpenCode has no hook to register, it loads a
    /// plugin FILE — so the presence test for OpenCode is that file.
    #[test]
    fn the_opencode_plugin_in_the_project_reads_as_installed() {
        let p = project_with(r#"{"hooks":{}}"#);
        std::fs::create_dir_all(p.dir.join(".opencode/plugins")).unwrap();
        std::fs::write(
            p.dir.join(".opencode/plugins/reflow2-loop-nudge.js"),
            "export default () => ({})\n",
        )
        .unwrap();
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Installed
        );
    }

    /// The machine-wide copy reaches every project on the machine, so it counts
    /// for a project that has none of its own.
    #[test]
    fn the_machine_wide_opencode_plugin_reads_as_installed() {
        let p = project_with(r#"{"hooks":{}}"#);
        let home = p.dir.join("fake-home");
        std::fs::create_dir_all(home.join(".config/opencode/plugins")).unwrap();
        std::fs::write(
            home.join(".config/opencode/plugins/reflow2-loop-nudge.js"),
            "export default () => ({})\n",
        )
        .unwrap();
        assert_eq!(
            status_with_home(Some(&p.graph()), Some(home)),
            NudgeStatus::Installed
        );
    }

    /// An OpenCode project WITHOUT the plugin is told it is absent — fixable —
    /// and no longer that no nudge is possible for its harness.
    #[test]
    fn an_opencode_project_without_the_plugin_is_absent_not_impossible() {
        let p = project_with(r#"{"hooks":{}}"#);
        std::fs::create_dir_all(p.dir.join(".reflow2")).unwrap();
        std::fs::write(
            p.dir.join(".reflow2/kit-version.json"),
            r#"{"harnesses":["opencode"]}"#,
        )
        .unwrap();
        assert_eq!(
            status_with_home(Some(&p.graph()), None),
            NudgeStatus::Absent
        );
    }
}
