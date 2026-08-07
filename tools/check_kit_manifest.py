#!/usr/bin/env python3
"""Does an installed kit's manifest agree with the kit source?

`ver:kit-manifest-agrees`, written 2026-08-07 and planned since 2026-07-25. The
friction it was raised from is the sharpest possible argument for it:
**reflow2's own manifest was four releases stale and nothing noticed.** The check
whose absence let that happen then sat `planned` and never ran for another
thirteen days, during which it went two releases staler.

`.reflow2/kit-version.json` claims three things, and until now nothing tested
any of them:

  reflow2_version   which kit produced this installation
  commit            which source revision that kit was built from
  installed_files   rel path -> sha256 of the file as installed

The manifest is not decoration. `place_kit_file` uses it to decide whether a file
is the kit's to refresh or the user's to leave alone — ownership is *proven* by a
hash match. So a manifest that has drifted from the kit is not a cosmetic problem:
it is the input to a clobber decision about somebody else's repository.

FOUR FINDINGS:

  stale_version   the stamp names an older kit than this source is
  file_set        the kit ships a path the manifest does not record, or the
                  manifest records a retired path that is not even on disk
  content         a recorded hash disagrees with the kit source's hash
  shadowing       a retired SKILL file is still installed and still tracked

WHAT IS NOT DRIFT, and this was got wrong on the first pass. A manifest entry for
a file the kit no longer ships looks like drift and mostly is not: `install()`
(reflow2_init.py:1031) writes `new_manifest[rel] = recorded` on purpose for a
retired file the user has edited, because dropping it would stop tracking a file
that is still there. So those entries are the installer's intended steady state,
they survive every future run, and flagging them would make this gate permanently
red on any project that ever edited a skill — a false positive by the installer's
own design. They are reported as notes.

`shadowing` is the exception, and it is a finding because the consequence is real
rather than tidiness. The installer's own comment says it: a harness auto-loads
anything in a skills directory, a served skill is never offered when a local one
exists, so **an edited local copy silently wins over every future release of that
skill**. That is `dec:skills-served` defeated in the one way it cannot detect,
and it is exactly the failure mode that makes a stale copy worse than no copy.

WHICH QUESTION THIS ASKS, because there are two and only one is checkable.
"Agrees with the kit source it CLAIMS to be" would mean fetching the kit as of the
recorded commit and diffing against that — real git archaeology, and impossible
for a released tarball with no history. So this asks the question that actually
catches staleness: **does this installation match the kit source that is here
now?**

    python3 tools/check_kit_manifest.py [PROJECT]     (default: this repo)

Exit 0 = the installation agrees with the kit. Exit 1 = it has drifted. Exit 2 =
there is nothing to check (no stamp), which is not a failure: a project that has
never been installed into cannot have a stale manifest.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Imported rather than restated. The shipped set lives in ONE place, and a second
# copy here would drift from it — which is the whole defect class this file is
# about, reproduced inside the checker.
sys.path.insert(0, str(REPO / "tools"))
import reflow2_init as kit  # noqa: E402


def file_sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def shipped_sources() -> dict[str, Path]:
    """Every path the kit installs, mapped to its source file.

    Both halves of the installer's contract: the flat `FILES` list and the
    `TREES` copied wholesale. Reading them from the installer's own module is
    deliberate — see the import note above.
    """
    out: dict[str, Path] = {}
    for src, rel in kit.FILES:
        out[rel] = src
    for src_dir, rel_dir in kit.TREES:
        if not src_dir.is_dir():
            continue
        for src in sorted(src_dir.rglob("*")):
            if src.is_file():
                out[f"{rel_dir}/{src.relative_to(src_dir).as_posix()}"] = src
    return out


def sidecar_of(rel: str) -> str | None:
    """Where kit content lands when the project already owns `rel`."""
    return kit.SIDECAR.get(rel)


def check(project: Path) -> tuple[list[str], list[str]]:
    """(findings, notes) — findings fail the check, notes are context."""
    findings: list[str] = []
    notes: list[str] = []

    stamp_path = project / kit.STAMP
    try:
        stamp = json.loads(stamp_path.read_text())
    except FileNotFoundError:
        return ([], [f"no kit stamp at {kit.STAMP} — nothing has been installed here"])
    except (OSError, json.JSONDecodeError) as e:
        return ([f"the kit stamp at {kit.STAMP} is unreadable ({e}). It records what "
                 f"the kit owns, and an unreadable one means the next install cannot "
                 f"tell its own files from yours"], [])

    # --- claim 1: version -------------------------------------------------
    source = kit.kit_version()
    claimed, actual = stamp.get("reflow2_version"), source.get("version") or source.get("reflow2_version")
    if claimed and actual and claimed != actual:
        findings.append(
            f"stale_version: installed by kit {claimed}, but this source is {actual}. "
            f"Re-run the installer to refresh it — until then the manifest below "
            f"describes a kit that is {claimed}, not the one you are running"
        )
    elif not actual:
        notes.append("could not read this source's kit version; skipped the version claim")
    if (c := stamp.get("commit")) and (a := source.get("commit")) and c != a:
        notes.append(f"stamped at commit {c}, this source is at {a}")

    # --- claim 2: file set ------------------------------------------------
    recorded: dict = stamp.get("installed_files") or {}
    if not isinstance(recorded, dict):
        return (findings + ["the manifest's installed_files is not an object"], notes)
    if not recorded:
        notes.append("pre-manifest install (no installed_files) — the file-set and "
                     "content claims cannot be checked, and one install closes that")
        return (findings, notes)

    shipped = shipped_sources()
    # A path the project owned lands at its sidecar instead, so the sidecar is a
    # legitimate manifest key for the file it stands in for.
    allowed = set(shipped)
    for rel in list(shipped):
        if side := sidecar_of(rel):
            allowed.add(side)

    for rel in sorted(set(shipped) - set(recorded)):
        # Not a finding when the sidecar was used instead — the file IS tracked,
        # under the name it was actually written to.
        if (side := sidecar_of(rel)) and side in recorded:
            continue
        findings.append(f"file_set: the kit ships {rel} and the manifest does not record it")
    for rel in sorted(set(recorded) - allowed):
        on_disk = project / rel
        if not on_disk.exists():
            # A dead entry: retired by the kit AND gone from the tree, so it
            # tracks nothing. The install's prune loop skips absent files
            # (reflow2_init.py:1025), so this one never clears itself.
            findings.append(
                f"file_set: the manifest records {rel}, which the kit no longer "
                f"ships ({kit.why_gone(rel)}) and which is not on disk — the entry "
                f"tracks nothing and an install will not clear it"
            )
        elif kit.is_skill_path(rel):
            # THE ONE WITH A CONSEQUENCE — but only when the content has drifted.
            #
            # A harness auto-loads anything under a skills directory and a served
            # skill is never offered when a local file of that name exists, so the
            # local copy always WINS. That is only a problem when it wins with
            # DIFFERENT content. reflow2's own repo keeps `.claude/skills/` and
            # `.grok/skills/` as deliberate byte-identical mirrors of
            # `getting-started/skills/`, enforced by skill_lint (skill_lint.py:45).
            # Flagging those would make this gate permanently red on the one
            # repository that maintains them correctly — the same false positive
            # the `file_set` rule above already had to be corrected for.
            #
            # So the discriminator is content, not presence: identical to the kit's
            # skill source means an in-sync mirror; anything else means a stale copy
            # silently beating every future release of that skill.
            # `.claude/skills/adopt/SKILL.md` -> `<kit>/skills/adopt/SKILL.md`:
            # drop BOTH leading components, not just the harness directory. Got
            # this wrong first and it produced `skills/skills/...`, which does not
            # exist — so every mirror read as drifted and the check "found" ten
            # problems that were not there. A path that cannot resolve must never
            # look like a difference.
            source = kit.KIT / "skills" / Path(*Path(rel).parts[2:])
            if source.is_file() and file_sha(on_disk) == file_sha(source):
                notes.append(
                    f"{rel} shadows the served skill but is byte-identical to "
                    f"{source.relative_to(REPO)} — an in-sync mirror, which is "
                    f"deliberate here and enforced by skill_lint"
                )
            else:
                findings.append(
                    f"shadowing: {rel} is installed, still tracked, and does NOT "
                    f"match the kit's skill source. A harness auto-loads it, so it "
                    f"SHADOWS the served skill of the same name and every future "
                    f"release of that skill loses to this copy, silently — the one "
                    f"way dec:skills-served can be defeated without anything "
                    f"noticing. Delete it to follow the served skill."
                )
        else:
            notes.append(
                f"{rel} is retired by the kit but still on disk and still tracked "
                f"({kit.why_gone(rel)}) — the installer's intended steady state for "
                f"an edited retired file, not drift"
            )

    # --- claim 3: per-file hashes ----------------------------------------
    for rel, want in sorted(recorded.items()):
        src = shipped.get(rel)
        if src is None:
            # Sidecar: find the shipped path it stands in for.
            src = next((s for r, s in shipped.items() if sidecar_of(r) == rel), None)
        if src is None:
            continue  # already reported as a file_set finding
        have = file_sha(src)
        if have != want:
            findings.append(
                f"content: {rel} was installed as {want[:12]} but this kit's source "
                f"hashes to {have[:12]} — the installed copy is from a different kit"
            )

    # Context, never a finding: whether the files are still on disk as recorded is
    # the INSTALL's business (place_kit_file treats a mismatch as the user's edits
    # and keeps it). This check is about the manifest against the kit, not the
    # manifest against the working tree.
    missing = [rel for rel in recorded if not (project / rel).exists()]
    if missing:
        notes.append(f"{len(missing)} recorded file(s) are not on disk: "
                     + ", ".join(sorted(missing)[:4])
                     + (" …" if len(missing) > 4 else ""))
    return (findings, notes)


def main() -> int:
    project = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else REPO
    findings, notes = check(project)
    for n in notes:
        print(f"  note  {n}")
    for f in findings:
        print(f"  FAIL  {f}")
    if not (project / kit.STAMP).exists():
        print(f"kit manifest: nothing installed at {project}")
        return 2
    if findings:
        print(f"\nkit manifest: DRIFTED — {len(findings)} finding(s), {len(notes)} note(s).")
        print("The manifest is the input to a clobber decision about this repository, "
              "so drift here is not cosmetic. Re-run the installer against this "
              "project to bring it back into agreement.")
        return 1
    print(f"kit manifest: OK — the installation agrees with the kit source "
          f"({len(notes)} note(s)).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
