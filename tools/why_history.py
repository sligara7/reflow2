#!/usr/bin/env python3
"""Read a repository's git history as the AGENDA for the `why` skill: what
changed, when and by whom — never why.

    python3 why_history.py                                  # the whole history, summarised
    python3 why_history.py --export docs/design/x.json      # ... and what is already explained
    python3 why_history.py page --paths src/export          # one feature's changes, as a numbered page
    python3 why_history.py page --feature cap:report-export --export docs/design/x.json
                                                            # ... its folders and resume point read from the design
    python3 why_history.py ... --json                       # the same, for an agent to read

The `why` skill interviews the person who designed a system about WHY it is the
way it is. The history is used only as the index of what changed; every reason
comes from a person. That split is the whole design
(dec:idea-a-change-justification-skill-walks-the-commit-history-and-asks-why),
and it is why this is a script rather than model work: grouping thousands of
commits, dropping the noise and counting churn is mechanical, deterministic and
nearly free here, and it would be slow and expensive done by reading.

WHAT IT DOES, in order:

1. Reads the MAIN LINE (`git log --first-parent`), so a merged branch arrives
   as ONE change and a squash-merged pull request is already one commit.
2. Groups what is left into changes a person would recognise: a merge or a
   `(#123)` pull-request commit is a change on its own; consecutive direct
   commits by the same author within a day, touching the same area, are one.
3. Drops the NOISE BY RULE, and says which rule dropped how many: bot authors,
   lockfile-only dependency bumps, CI-only edits, formatting passes, small typo
   fixes. A rule is a guess about someone else's repository, so every drop is
   counted and `--include-noise` shows them.
4. Flags reverts (a revert always had a reason) and, as a GUESS FROM FILE PATHS
   that it labels as one, which changes look user-visible.
5. With `--export`, reads the committed design export and marks as explained
   every change whose commit a ChangeEvent's `commits` already names — so
   coverage is COMPUTED from the design, never kept in a second file that could
   disagree with it.

The `page` subcommand lists one feature's changes oldest first, numbered, so
the person can answer a whole page in one message. `--feature` names the
feature's Capability: its folders are the `location`s of the Artifacts that
REALIZE it, and its cursor — the last change already reviewed — is the `after`
in the `value` of its `why_cursor` finding. `--paths` and `--after` say the
same things by hand and win when given. Changes behind the cursor that no
ChangeEvent names were reviewed and needed no record, and are counted as such.

Stdlib only. Exit codes: 0 it ran · 2 it could not run (not a git repository,
unreadable export, bad arguments).
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
from collections import Counter

REC = "\x1e"
FLD = "\x1f"

# A change is noise only when EVERY file it touched matches, so a lockfile that
# rides along with a real change never hides the change.
LOCKFILES = {
    "package-lock.json", "npm-shrinkwrap.json", "yarn.lock", "pnpm-lock.yaml",
    "Cargo.lock", "poetry.lock", "Pipfile.lock", "uv.lock", "pdm.lock", "go.sum",
    "Gemfile.lock", "composer.lock", "mix.lock", "pubspec.lock", "Podfile.lock",
    "packages.lock.json", "gradle.lockfile", "flake.lock", "bun.lockb",
}
CI_PREFIXES = (".github/workflows/", ".github/actions/", ".circleci/", ".buildkite/", ".gitlab/ci/")
CI_FILES = {
    ".gitlab-ci.yml", ".travis.yml", "Jenkinsfile", "azure-pipelines.yml", "appveyor.yml",
    "bitbucket-pipelines.yml", ".drone.yml", ".github/dependabot.yml", "renovate.json",
    ".pre-commit-config.yaml",
}
BOT = re.compile(r"\[bot\]|dependabot|renovate|github-actions|greenkeeper|snyk-bot|pre-commit-ci", re.I)
FORMATTING = re.compile(
    r"^(style|fmt|format|lint)(\(.*?\))?[:!\s]"
    r"|\b(reformat(ted|ting)?|prettier|black|isort|rustfmt|cargo fmt|gofmt|clang-format|autoformat|"
    r"whitespace|trailing spaces|fix(ed)? (lint|linting|formatting))\b",
    re.I,
)
TYPO = re.compile(r"\b(typos?|spelling|misspell(ed|ing)?)\b", re.I)
TYPO_MAX_LINES = 20
PR_NUMBER = re.compile(r"\(#(\d+)\)\s*$|^Merge pull request #(\d+)|!(\d+)\b")
REVERTS = re.compile(r"This reverts commit ([0-9a-f]{7,40})", re.I)

# The user-visible guess. Path SEGMENTS and extensions that usually mean a
# screen, a route or an output a user sees. It is a heuristic about somebody
# else's layout and is always reported as one.
UI_SEGMENTS = {
    "ui", "views", "view", "templates", "template", "components", "pages", "page", "screens",
    "screen", "routes", "frontend", "web", "static", "public", "assets", "layouts", "widgets",
    "dialogs", "menus", "reports", "exports", "cli", "commands", "i18n", "locales",
}
UI_EXTENSIONS = {
    ".html", ".htm", ".vue", ".svelte", ".jsx", ".tsx", ".css", ".scss", ".less", ".xaml",
    ".ui", ".qml", ".storyboard", ".xib", ".erb", ".jinja", ".j2", ".hbs", ".ejs",
}

DAY = dt.timedelta(hours=24)


def die(msg: str) -> None:
    print(f"why_history: {msg}", file=sys.stderr)
    sys.exit(2)


def git(args: list[str], cwd: str) -> str:
    try:
        out = subprocess.run(["git", *args], capture_output=True, text=True, cwd=cwd)
    except OSError as e:
        die(f"could not run git: {e}")
    if out.returncode != 0:
        die(f"git {' '.join(args[:3])} … failed: {out.stderr.strip()}")
    return out.stdout


# ---------------------------------------------------------------------------
# Reading the main line
# ---------------------------------------------------------------------------


def read_main_line(repo: str, rev: str, since: str | None, paths: list[str]) -> list[dict]:
    """Every first-parent commit, oldest first, with the files it touched.

    A merge is diffed against its FIRST parent, so its files are what the
    merged branch brought in — the change as the main line experienced it.

    FILE NAMES ONLY, NEVER LINE COUNTS, and the difference is not small:
    counting lines makes git diff every file at every commit, and on a
    repository that commits one large generated file it was measured at 67 s
    against 0.14 s for the same 1,067 commits. The one rule that wants a size
    (a small typo fix) asks for it per candidate commit instead.
    """
    fmt = FLD.join(["%H", "%P", "%an", "%ae", "%aI", "%s", "%b"])
    args = ["log", "--first-parent", "--diff-merges=first-parent", "--name-only",
            "--reverse", f"--format={REC}{fmt}{FLD}", rev]
    if since:
        args.append(f"--since={since}")
    if paths:
        args += ["--", *paths]
    raw = git(args, repo)
    commits = []
    for chunk in raw.split(REC)[1:]:
        head, _, names_ = chunk.rpartition(FLD)
        sha, parents, name, email, date, subject, body = (head.split(FLD) + [""] * 7)[:7]
        commits.append({
            "sha": sha,
            "parents": parents.split(),
            "author": name,
            "email": email,
            "date": date,
            "subject": subject.strip(),
            "body": body.strip(),
            "files": [f for f in names_.strip().splitlines() if f.strip()],
        })
    return commits


def lines_changed(repo: str, shas: list[str]) -> int:
    """Lines added plus deleted — asked only of commits a size rule needs."""
    total = 0
    for sha in shas:
        out = git(["show", "--numstat", "--format=", "--diff-merges=first-parent", sha], repo)
        for row in out.splitlines():
            cols = row.split("\t")
            if len(cols) == 3:
                total += sum(int(x) for x in cols[:2] if x.isdigit())
    return total


def branch_commits(repo: str, merge: dict) -> list[str]:
    """The commits a merge brought in, so a ChangeEvent naming any one of them
    is recognised as explaining the merge."""
    if len(merge["parents"]) < 2:
        return []
    out = git(["rev-list", f"{merge['parents'][0]}..{merge['parents'][1]}"], repo)
    return out.split()


# ---------------------------------------------------------------------------
# Grouping, noise, reverts
# ---------------------------------------------------------------------------


def area(path: str) -> str:
    parts = path.split("/")
    return "/".join(parts[:2]) if len(parts) > 2 else (parts[0] if len(parts) > 1 else ".")


def pr_number(c: dict) -> str | None:
    m = PR_NUMBER.search(c["subject"])
    if not m:
        return None
    return "#" + next(g for g in m.groups() if g)


def when(c: dict) -> dt.datetime:
    """A commit's author date. git prints UTC as a trailing `Z` in some
    versions, and `fromisoformat` accepts that only from Python 3.11 — the
    kit supports older, and CI's runner is 3.10."""
    iso = c["date"]
    if iso.endswith("Z"):
        iso = iso[:-1] + "+00:00"
    return dt.datetime.fromisoformat(iso)


def group(repo: str, commits: list[dict]) -> list[dict]:
    changes: list[dict] = []
    run: list[dict] = []

    def close_run() -> None:
        if run:
            changes.append(make_change(repo, list(run), kind="direct"))
            run.clear()

    for c in commits:
        is_merge = len(c["parents"]) > 1
        pr = pr_number(c)
        if is_merge or pr:
            close_run()
            ch = make_change(repo, [c], kind="merge" if is_merge else "pull_request")
            if is_merge:
                ch["commits"] += [s for s in branch_commits(repo, c) if s not in ch["commits"]]
                title = c["body"].splitlines()[0].strip() if c["body"] else ""
                if title and c["subject"].lower().startswith("merge"):
                    ch["subject"] = title
            ch["pr"] = pr
            changes.append(ch)
            continue
        if run:
            last = run[-1]
            same_author = last["email"] == c["email"]
            close_in_time = when(c) - when(last) <= DAY
            shared_area = bool({area(f) for f in last["files"]} & {area(f) for f in c["files"]})
            is_revert = bool(c["subject"].startswith("Revert ") or REVERTS.search(c["body"]))
            if not (same_author and close_in_time and shared_area) or is_revert:
                close_run()
        run.append(c)
    close_run()
    for i, ch in enumerate(changes, 1):
        ch["n"] = i
    return changes


def make_change(repo: str, cs: list[dict], kind: str) -> dict:
    files = sorted({f for c in cs for f in c["files"]})
    authors = list(dict.fromkeys(c["author"] for c in cs))
    emails = list(dict.fromkeys(c["email"] for c in cs))
    subject = cs[0]["subject"] if len(cs) == 1 else f"{cs[0]['subject']} (+{len(cs) - 1} more)"
    bodies = "\n".join(c["body"] for c in cs)
    reverted = REVERTS.search(bodies)
    ch = {
        "kind": kind,
        "commits": [c["sha"] for c in cs],
        "date": cs[-1]["date"][:10],
        "first_date": cs[0]["date"][:10],
        "authors": authors,
        "subject": subject,
        "messages": [c["subject"] for c in cs],
        "files": files,
        "areas": sorted({area(f) for f in files}),
        "revert": bool(cs[0]["subject"].startswith("Revert ") or reverted),
        "reverts": reverted.group(1) if reverted else None,
        "pr": None,
    }
    ch["noise"] = noise_rule(repo, ch, cs, emails)
    ch["user_visible_guess"] = any(looks_user_visible(f) for f in files)
    return ch


def noise_rule(repo: str, ch: dict, cs: list[dict], emails: list[str]) -> str | None:
    """Which rule, if any, says nobody needs to be asked about this change.
    A revert is never noise: it always had a reason."""
    if ch["revert"]:
        return None
    if all(BOT.search(a) for a in ch["authors"]) or all(BOT.search(e) for e in emails):
        return "bot"
    files = ch["files"]
    if files and all(os.path.basename(f) in LOCKFILES for f in files):
        return "dependencies"
    if files and all(f in CI_FILES or f.startswith(CI_PREFIXES) for f in files):
        return "ci"
    if all(FORMATTING.search(m) for m in ch["messages"]):
        return "formatting"
    if (all(TYPO.search(m) for m in ch["messages"])
            and lines_changed(repo, [c["sha"] for c in cs]) <= TYPO_MAX_LINES):
        return "typo"
    return None


def looks_user_visible(path: str) -> bool:
    segs = {s.lower() for s in path.split("/")[:-1]}
    return bool(segs & UI_SEGMENTS) or os.path.splitext(path)[1].lower() in UI_EXTENSIONS


# ---------------------------------------------------------------------------
# What the design already holds
# ---------------------------------------------------------------------------


class Design:
    """What the committed export already says that this script reads."""

    def __init__(self) -> None:
        self.named: list[str] = []          # commit ids ChangeEvents name
        self.follow_ups: list[dict] = []    # open follow-ups, for "waiting on"
        self.locations: dict[str, list[str]] = {}   # capability -> realizing artifact locations
        self.cursors: dict[str, str] = {}   # capability -> the last change reviewed
        self.present = False


def read_export(path: str | None) -> Design:
    """Read the committed export rather than ask a server, so this runs in
    CI, offline and with no reflow2 binary present."""
    d = Design()
    if not path:
        return d
    try:
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
    except (OSError, ValueError) as e:
        die(f"could not read the design export {path}: {e}")
    d.present = True
    artifacts: dict[str, str] = {}
    cursor_dates: dict[str, str] = {}
    for n in doc.get("nodes", []):
        props = n.get("properties") or {}
        kind = n.get("node_type")
        if kind == "ChangeEvent" and props.get("commits"):
            d.named += [s.lower() for s in re.split(r"[,\s]+", str(props["commits"])) if len(s) >= 7]
        elif kind == "Artifact" and isinstance(props.get("location"), str):
            artifacts[n.get("node_id")] = props["location"]
        elif kind == "TemporalFact" and not props.get("valid_to"):
            if props.get("fact_type") == "follow_up":
                d.follow_ups.append({"id": n.get("node_id"), "statement": props.get("statement", "")})
            elif props.get("fact_type") == "why_cursor":
                try:
                    after = json.loads(props.get("value") or "{}").get("after")
                except (ValueError, AttributeError):
                    after = None
                subject = props.get("subject_id")
                when_ = props.get("valid_from") or ""
                if after and subject and when_ >= cursor_dates.get(subject, ""):
                    d.cursors[subject], cursor_dates[subject] = str(after), when_
    for e in doc.get("edges", []):
        if e.get("edge_type") == "REALIZES" and e.get("from_id") in artifacts:
            d.locations.setdefault(e.get("to_id"), []).append(artifacts[e["from_id"]])
    return d


def names(ch: dict, sha_prefixes: list[str]) -> bool:
    return any(c.startswith(p) or p.startswith(c) for c in ch["commits"] for p in sha_prefixes)


def waiting_on(ch: dict, follow_ups: list[dict]) -> str | None:
    """A follow-up whose statement quotes one of this change's commits — the
    skill's convention for a question put to somebody else."""
    for f in follow_ups:
        for s in re.findall(r"\b[0-9a-f]{7,40}\b", f["statement"].lower()):
            if any(c.startswith(s) for c in ch["commits"]):
                return f["id"]
    return None


def annotate(changes: list[dict], d: Design) -> None:
    for ch in changes:
        ch["explained"] = names(ch, d.named)
        ch["waiting_on"] = None if ch["explained"] else waiting_on(ch, d.follow_ups)


# ---------------------------------------------------------------------------
# The two readings
# ---------------------------------------------------------------------------


def summarise(repo: str, commits: list[dict], changes: list[dict], d: Design) -> dict:
    exported = d.present
    noise = Counter(ch["noise"] for ch in changes if ch["noise"])
    asked = [ch for ch in changes if not ch["noise"]]
    churn = Counter()
    for ch in asked:
        for a in ch["areas"]:
            churn[a] += 1
    return {
        "repo": repo,
        "commits": len(commits),
        "first": commits[0]["date"][:10] if commits else None,
        "last": commits[-1]["date"][:10] if commits else None,
        "changes": len(changes),
        "grouped": dict(Counter(ch["kind"] for ch in changes)),
        "noise": sum(noise.values()),
        "noise_by_rule": dict(sorted(noise.items())),
        "to_ask_about": len(asked),
        "reverts": sum(1 for ch in asked if ch["revert"]),
        "user_visible_guess": sum(1 for ch in asked if ch["user_visible_guess"]),
        "explained": sum(1 for ch in asked if ch["explained"]) if exported else None,
        "waiting": sum(1 for ch in asked if ch["waiting_on"]) if exported else None,
        "features_walked": len(d.cursors) if exported else None,
        "most_changed_areas": [{"area": a, "changes": n} for a, n in churn.most_common(20)],
    }


def page(changes: list[dict], after: str | None, size: int, include_noise: bool, exported: bool) -> dict:
    cursor_at = 0
    if after:
        hit = [ch for ch in changes if names(ch, [after.lower()])]
        if not hit:
            die(f"--after {after} names no change on these paths; is it the right cursor?")
        cursor_at = hit[-1]["n"]
    total = len(changes)
    noise = [ch for ch in changes if ch["noise"]]
    real = [ch for ch in changes if not ch["noise"]]
    explained = [ch for ch in real if ch["explained"]]
    waiting = [ch for ch in real if ch["waiting_on"]]
    reviewed = [ch for ch in real if ch["n"] <= cursor_at and not ch["explained"] and not ch["waiting_on"]]
    open_ = [ch for ch in (changes if include_noise else real)
             if ch["n"] > cursor_at and not ch["explained"] and not ch["waiting_on"]]
    shown = open_[:size]
    return {
        "coverage": {
            "changes": total,
            "noise": len(noise),
            "explained": len(explained) if exported else None,
            "waiting": len(waiting) if exported else None,
            "reviewed_no_record": len(reviewed),
            "left": len([ch for ch in open_ if not ch["noise"]]),
        },
        "page": shown,
        "more": len(open_) > len(shown),
        "next_after": shown[-1]["commits"][0][:12] if shown else None,
    }


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------


def render_summary(s: dict) -> str:
    def row(label: str, value, note: str = "") -> str:
        v = "—" if value is None else f"{value:,}"
        return f"  {label:<24}{v:>7}{('   ' + note) if note else ''}"

    g = s["grouped"]
    grouped = ", ".join(f"{g[k]:,} {label}" for k, label in
                        (("merge", "merges"), ("pull_request", "pull requests"), ("direct", "direct runs"))
                        if g.get(k))
    rules = ", ".join(f"{k} {v:,}" for k, v in s["noise_by_rule"].items())
    out = [
        f"why_history: {s['repo']} — {s['commits']:,} commits on the main line, {s['first']} to {s['last']}",
        row("changes", s["changes"], f"({grouped})" if grouped else ""),
        row("noise, not asked", s["noise"], f"({rules}; --include-noise lists them)" if rules else ""),
        row("to ask about", s["to_ask_about"]),
        row("  reverts", s["reverts"], "(each one had a reason)"),
        row("  user-visible?", s["user_visible_guess"], "(a GUESS from file paths)"),
        row("  already explained", s["explained"], "" if s["explained"] is not None else "(pass --export to count)"),
        row("  waiting on someone", s["waiting"]),
        row("features walked", s["features_walked"], "(each with a resume point in the design)"),
        "  most-changed areas:",
    ]
    out += [f"    {a['area']:<40}{a['changes']:>5} changes" for a in s["most_changed_areas"][:10]]
    return "\n".join(out)


def render_page(p: dict, paths: list[str], me: str | None) -> str:
    c = p["coverage"]
    def num(v):
        return "?" if v is None else v
    lines = [
        f"why_history: {', '.join(paths)} — {c['changes']} changes: {num(c['explained'])} explained, "
        f"{c['reviewed_no_record']} reviewed with nothing to record, {num(c['waiting'])} waiting on someone, "
        f"{c['noise']} noise, {c['left']} left",
    ]
    for ch in p["page"]:
        who = ", ".join(a for a in ch["authors"] if not (me and me.lower() in a.lower()))
        tags = []
        if ch["revert"]:
            tags.append("revert")
        if ch["noise"]:
            tags.append(f"noise: {ch['noise']}")
        tag = f" [{', '.join(tags)}]" if tags else ""
        who_s = f"({who}) " if who else ""
        lines.append(f"  {ch['n']:>4}  {ch['date'][:7]}  {who_s}{ch['subject']}{tag}  "
                     f"· {len(ch['files'])} file(s) · {ch['commits'][0][:9]}")
    if p["more"]:
        lines.append(f"  … more after this page: --after {p['next_after']}")
    elif not p["page"]:
        lines.append("  nothing left to ask about on these paths")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("command", nargs="?", choices=["summary", "page"], default="summary")
    ap.add_argument("--repo", default=".", help="the repository to read (default: here)")
    ap.add_argument("--rev", default="HEAD", help="whose main line to read (default: HEAD)")
    ap.add_argument("--since", help="only history after this date (anything git accepts)")
    ap.add_argument("--paths", nargs="+", default=[], help="one feature's files or directories")
    ap.add_argument("--feature", help="page: the feature's Capability id — its folders and cursor come from --export")
    ap.add_argument("--export", help="the committed design export, to count what is explained")
    ap.add_argument("--after", help="page: the feature's cursor — the last change already reviewed")
    ap.add_argument("--size", type=int, default=15, help="page: changes per page (default 15)")
    ap.add_argument("--me", help="page: your name, so only OTHER people's changes show an author")
    ap.add_argument("--include-noise", action="store_true", help="show changes the noise rules dropped")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    a = ap.parse_args(argv)

    d = read_export(a.export)
    if a.feature:
        if not d.present:
            die("--feature reads the feature's folders and cursor from the design: pass --export too")
        a.paths = a.paths or d.locations.get(a.feature, [])
        a.after = a.after or d.cursors.get(a.feature)
        if not a.paths:
            die(f"{a.feature} has no registered location in {a.export}: register where it lives "
                f"(link_artifact with the folder as location), or pass --paths")
    if a.command == "page" and not a.paths:
        die("page needs --paths or --feature: the files or directories that make up the feature")
    repo = git(["rev-parse", "--show-toplevel"], a.repo).strip()
    commits = read_main_line(repo, a.rev, a.since, a.paths)
    changes = group(repo, commits)
    annotate(changes, d)

    if a.command == "summary":
        s = summarise(repo, commits, changes, d)
        if a.include_noise:
            s["noise_changes"] = [ch for ch in changes if ch["noise"]]
        print(json.dumps(s, indent=2) if a.json else render_summary(s))
    else:
        p = page(changes, a.after, a.size, a.include_noise, exported=d.present)
        p["paths"] = a.paths
        p["feature"] = a.feature
        p["after"] = a.after
        print(json.dumps(p, indent=2) if a.json else render_page(p, a.paths, a.me))
    return 0


if __name__ == "__main__":
    sys.exit(main())
