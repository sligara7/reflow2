#!/usr/bin/env python3
"""Keep docs/skills-and-tools.md's TOOL ROWS and HEADER COUNTS in step with the
served surface — from the committed toolsnaps, which ARE the surface.

# Why this exists

The page says of itself: *"Generated from the running server, not from memory
… If this file and the server disagree, the server is right."* It was generated
by hand, once, on 2026-08-21, and by 2026-09-12 it said 21 skills and 155 tools
against 26 and 180 served — a month stale, a third of the surface missing,
while 24 descriptions it quoted had since been rewritten. A document that
claims to be generated and is not will drift exactly this way.

So the mechanical part is now mechanical. This rewrites every
`| `tool` | **read**/**write** | one line |` row from `tools/toolsnaps/*.json`
(kind from `readOnlyHint`, the line from the description's first sentence),
places a tool the page does not yet list under the family of the slice that
serves it, and fixes the header counts. The PROSE around the tables — who
calls what, the hook, the honest limits — is a person's and is left alone.

    python3 tools/render_skills_and_tools.py --write   # regenerate the rows
    python3 tools/render_skills_and_tools.py --check   # CI: exit 1 on drift

# What it does not do

It does not judge a description; it quotes one. Quality is the blind
prediction test's job. And it does not know which family a tool BELONGS to,
only which slice serves it — those coincide today because the slices were cut
by family (BL-181).
"""

import glob
import json
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
DOC = REPO / "docs" / "skills-and-tools.md"
SNAPS = REPO / "tools" / "toolsnaps"
SLICES = REPO / "crates" / "reflow2-mcp" / "src" / "tools"
SKILLS = REPO / "getting-started" / "skills"

# slice file -> the section heading that family lives under in the page
FAMILY = {
    "capture": "### Capture — put intent and structure into the design",
    "coherence": "### Coherence — what the design says about itself",
    "query": "### Query — read the design back",
    "assure": "### Assurance — checks, evidence and confirmation",
    "built": "### Build — what exists on disk, and whether it still matches",
    "temporal_tools": "### Time — epochs, change, and what a claim was true of",
    "operate_tools": "### Operate — releases, environments, resources, readiness",
    "exchange": "### Exchange — comparing, merging and linking whole designs",
    "ask": "### Ask — turning findings into questions a person answers",
    "claims_tools": "### Coordination — who holds which region",
    "ingest_tools": "### Ingest — reading an existing corpus in",
    "skills_tools": "### Skills — reaching the served procedures",
}


def first_sentence(text: str) -> str:
    text = re.sub(r"\s+", " ", text or "").strip()
    m = re.match(r"(.+?[.!?])(\s|$)", text)
    return (m.group(1) if m else text).replace("|", "\\|")


def served() -> dict[str, dict]:
    out = {}
    for f in sorted(glob.glob(str(SNAPS / "*.json"))):
        d = json.load(open(f))
        ro = (d.get("annotations") or {}).get("readOnlyHint")
        out[d["name"]] = {"kind": "read" if ro else "write", "line": first_sentence(d.get("description"))}
    return out


def families() -> dict[str, str]:
    """tool -> slice, from which tools/*.rs declares `pub async fn <tool>(`."""
    out = {}
    for f in SLICES.glob("*.rs"):
        for m in re.finditer(r"pub async fn (\w+)\(", f.read_text()):
            out[m.group(1)] = f.stem
    return out


def render(text: str) -> str:
    tools = served()
    fam = families()
    row = re.compile(r"^\| `([a-z_]+)` \| \*\*(read|write)\*\* \| (.*) \|$")
    lines = text.splitlines()
    seen = set()
    out = []
    for l in lines:
        m = row.match(l)
        if m and m.group(1) in tools:
            t = tools[m.group(1)]
            out.append(f"| `{m.group(1)}` | **{t['kind']}** | {t['line']} |")
            seen.add(m.group(1))
        elif m:
            continue  # a tool the server no longer serves: drop the row
        else:
            out.append(l)
    # place the unlisted tools under their family's table
    missing = sorted(set(tools) - seen)
    for name in missing:
        heading = FAMILY.get(fam.get(name, ""), None)
        if heading is None or heading not in out:
            continue
        i = out.index(heading)
        # advance to the end of that section's table
        j = i + 1
        while j < len(out) and not out[j].startswith("| `"):
            j += 1
        while j < len(out) and out[j].startswith("| "):
            j += 1
        t = tools[name]
        out.insert(j, f"| `{name}` | **{t['kind']}** | {t['line']} |")
        seen.add(name)
    n_skills = len([p for p in SKILLS.iterdir() if (p / "SKILL.md").exists()])
    n_tools = len(tools)
    text = "\n".join(out) + "\n"
    text = re.sub(r"^# What reflow2 offers: \d+ skills and \d+ tools", f"# What reflow2 offers: {n_skills} skills and {n_tools} tools", text, flags=re.M)
    text = re.sub(r"^## The \d+ skills", f"## The {n_skills} skills", text, flags=re.M)
    text = re.sub(r"^## The \d+ tools", f"## The {n_tools} tools", text, flags=re.M)
    unplaced = sorted(set(tools) - seen)
    return text, unplaced


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "--check"
    current = DOC.read_text()
    new, unplaced = render(current)
    if unplaced:
        print(f"FAIL: {len(unplaced)} served tool(s) could not be placed under a family heading: {unplaced}")
        return 1
    if mode == "--write":
        DOC.write_text(new)
        print(f"wrote {DOC.relative_to(REPO)}")
        return 0
    if new != current:
        print("FAIL: docs/skills-and-tools.md does not match the served surface — run "
              "`python3 tools/render_skills_and_tools.py --write` and commit the result.")
        return 1
    print("OK: docs/skills-and-tools.md matches the served surface (rows, kinds, counts).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
