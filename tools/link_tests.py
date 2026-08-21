#!/usr/bin/env python3
"""Which of this project's tests does the design know about? — any project.

A design that cannot say which part of the system a test exercises cannot answer
the question a person actually asks before writing more of them: *where is the
testing thin?* Counting `Verification` nodes per subsystem answers it only if
the tests on disk and the tests in the design are the same set. On reflow2 they
were not — 127 test files existed and 47 were pointed at by an `Artifact`, so
every per-subsystem count was a statement about the MODEL and was being read as
a statement about the TESTING.

🛑 **A ZERO IN THAT TABLE IS THE DANGEROUS CELL.** `sys:documentation` showed no
verifications at all. With 80 of 127 files unmodelled, that reads identically
whether the subsystem is untested or merely undescribed — and acting on it means
writing tests for something already covered while something genuinely bare stays
quiet. This tool exists to tell those two apart.

⭐ IT NEEDS NO CONFIGURATION, for the same reason `wall_check` needs none: the
design already says where its code lives. `Artifact.location` plus `REALIZES`
gives the module-to-component mapping, and the test roots are derived from the
paths the design already claims.

# The evidence rule, and why it is narrow on purpose

A test is attributed to a component only when BOTH hold:

  1. the test file's name matches the name of a source file the design maps to
     that component (`tests/heal.rs` ↔ `src/heal.rs`), AND
  2. the test actually CALLS a function that source file defines.

Clause 2 is what turns a name coincidence into evidence. Clause 1 is what keeps
clause 2 from drowning: nearly every test in a codebase like this calls the
graph constructor and a handful of builders, so "calls a function this module
defines" alone attributes half the suite to whichever module owns the setup.

**Everything else is reported as UNATTRIBUTED, with the reason.** That is the
whole discipline. Scoring heuristics were tried first and measured on reflow2:
ranking components by how many of their uniquely-owned functions a test calls
left 27 of 80 files with no dominant owner and put `cmp:graph` at the top of
most of the rest, because opening a graph is setup and not subject. A number
produced that way looks like an answer and is a guess, and a guessed mapping is
worse than a missing one here — it would make the per-subsystem table *appear*
complete while quietly attributing tests to the wrong part.

# What this deliberately does not do

It does not write to the graph. It proposes, and a person or an agent decides —
`--emit json` gives the proposals in a form that can be applied. Attribution is
a claim about what a test is FOR, and the two-clause rule is good evidence, not
proof.

It also does not judge whether a test is any good, how much it covers, or
whether a subsystem has enough of them. It answers exactly one question: does
the design know this test exists, and if so, what does it say the test is about.

Run:  python3 tools/link_tests.py [--export docs/design/reflow2.json] [--emit json]
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict

DEFAULT_EXPORT = "docs/design/reflow2.json"

# Source extensions whose call structure this tool can read. A file it cannot
# parse is NOT a file with no calls, and is reported rather than skipped.
READABLE = {".rs", ".py"}

FN_DEF = {
    ".rs": re.compile(r"pub(?:\(crate\))? fn ([a-z_][a-z0-9_]*)"),
    ".py": re.compile(r"^def ([a-z_][a-z0-9_]*)", re.M),
}


def strip_comments(text: str, ext: str) -> str:
    """Comments and docstrings are prose, and prose is never structure.

    The trap has a price already paid: an adopt run reported a fourth module
    cycle that was a rustdoc link inside a comment. A test file's header here is
    typically a long essay naming other modules, so reading it as calls would
    attribute nearly every file to nearly every component.
    """
    marker = "//" if ext == ".rs" else "#"
    out = []
    for line in text.splitlines():
        s = line.lstrip()
        if s.startswith(marker):
            continue
        out.append(line)
    body = "\n".join(out)
    if ext == ".rs":
        body = re.sub(r"/\*.*?\*/", " ", body, flags=re.S)
    else:
        body = re.sub(r'"""(?:.|\n)*?"""', " ", body)
    # String literals are prose too — but stripped LINE BY LINE, never across the
    # whole file. A file-wide pass pairs quotes greedily, so one unbalanced quote
    # (a raw string, an escaped one, an apostrophe in a doc line the comment pass
    # missed) swallows everything to the next quote — real code included. That is
    # not a hypothetical: on reflow2 it reduced `heal.rs` from dozens of visible
    # `pub fn` definitions to ONE, and the tool then reported the heal test as
    # calling nothing heal.rs defines. A parsing bug that silently shrinks the
    # evidence looks exactly like an honest "no evidence found".
    return "\n".join(
        re.sub(r'"(?:[^"\\]|\\.)*"', '""', line) for line in body.splitlines()
    )


def load(export_path):
    if not os.path.exists(export_path):
        return None
    with open(export_path, encoding="utf-8") as fh:
        return json.load(fh)


def component_sources(doc):
    """{source path: component id} — straight from the design, never guessed."""
    nodes = {n["node_id"]: n for n in doc["nodes"]}
    loc = {
        i: (n["properties"].get("location") or "")
        for i, n in nodes.items()
        if n["node_type"] == "Artifact"
    }
    out = {}
    for e in doc["edges"]:
        if e["edge_type"] != "REALIZES":
            continue
        if nodes.get(e["to_id"], {}).get("node_type") != "Component":
            continue
        p = loc.get(e["from_id"], "")
        if p and os.path.splitext(p)[1] in READABLE and not is_test_path(p):
            out[p] = e["to_id"]
    return out


def is_test_path(path: str) -> bool:
    parts = path.replace("\\", "/").split("/")
    return "tests" in parts or os.path.basename(path).startswith("test_")


def claimed_locations(doc):
    return {
        (n["properties"].get("location") or "")
        for n in doc["nodes"]
        if n["node_type"] == "Artifact"
    }


def test_roots(sources):
    """Where to look for tests: beside every source tree the design claims.

    Derived rather than configured, so a project with `spec/`, `t/` or
    `src/test` is found on the same terms as one with `tests/`.
    """
    roots = set()
    for p in sources:
        d = os.path.dirname(p)
        while d and d not in (".", "/"):
            for name in ("tests", "test", "spec"):
                cand = os.path.join(os.path.dirname(d), name)
                if os.path.isdir(cand):
                    roots.add(cand)
            d = os.path.dirname(d)
    return sorted(roots)


def find_tests(roots):
    found = []
    for r in roots:
        for dirpath, _dirs, files in os.walk(r):
            for f in sorted(files):
                if os.path.splitext(f)[1] in READABLE:
                    found.append(os.path.join(dirpath, f).replace("\\", "/"))
    return sorted(set(found))


def attribute(tests, sources, claimed):
    """Apply the two-clause rule. Returns (proposals, unattributed)."""
    by_name = defaultdict(list)
    for path, comp in sources.items():
        by_name[os.path.basename(path)].append((path, comp))

    defined = {}
    for path in sources:
        if not os.path.exists(path):
            continue
        ext = os.path.splitext(path)[1]
        with open(path, encoding="utf-8", errors="replace") as fh:
            defined[path] = set(FN_DEF[ext].findall(strip_comments(fh.read(), ext)))

    proposals, unattributed = [], []
    for t in tests:
        known = t in claimed
        base = os.path.basename(t)
        cands = by_name.get(base, [])
        if not cands:
            unattributed.append((t, known, "no source file of this name is mapped to a component"))
            continue
        if len(cands) > 1:
            names = ", ".join(c for _, c in cands)
            unattributed.append((t, known, f"the name is ambiguous across {names}"))
            continue
        src, comp = cands[0]
        ext = os.path.splitext(t)[1]
        with open(t, encoding="utf-8", errors="replace") as fh:
            body = strip_comments(fh.read(), ext)
        called = sorted(fn for fn in defined.get(src, ()) if re.search(rf"\b{fn}\s*\(", body))
        if not called:
            unattributed.append(
                (t, known, f"name matches {src} but it calls nothing that file defines")
            )
            continue
        proposals.append(
            {
                "test": t,
                "component": comp,
                "source": src,
                "already_known": known,
                "evidence": f"calls {len(called)} function(s) defined in {src}: "
                + ", ".join(called[:4]),
            }
        )
    return proposals, unattributed


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--export", default=DEFAULT_EXPORT)
    ap.add_argument("--emit", choices=["text", "json"], default="text")
    args = ap.parse_args()

    doc = load(args.export)
    if doc is None:
        print(f"no export at {args.export} — nothing to check against.")
        return 0

    sources = component_sources(doc)
    if not sources:
        print(
            "0 source file(s) are mapped to a component in this design, so there is\n"
            "nothing to attribute a test TO. This is a statement about the design,\n"
            "not a clean result: register the sources first (link_artifact + realizes)."
        )
        return 0

    claimed = claimed_locations(doc)
    tests = find_tests(test_roots(sources))
    proposals, unattributed = attribute(tests, sources, claimed)

    if args.emit == "json":
        json.dump(
            {
                "proposals": proposals,
                "unattributed": [
                    {"test": t, "already_known": k, "reason": r} for t, k, r in unattributed
                ],
            },
            sys.stdout,
            indent=1,
        )
        print()
        return 0

    new = [p for p in proposals if not p["already_known"]]
    known_unattr = [u for u in unattributed if u[1]]
    print("=" * 74)
    print("COVERAGE — what this answer is built on")
    print("=" * 74)
    print(f"  {len(sources)} source file(s) the design maps to a component")
    print(f"  {len(tests)} test file(s) found beside them")
    print(f"  {len(tests) - len(claimed & set(tests))} of those the design has never heard of")
    print()
    print(f"{len(proposals)} test(s) attributable on the two-clause rule "
          f"({len(new)} of them not yet registered):")
    for p in new[:40]:
        print(f"    {p['test']} -> {p['component']}")
        print(f"        {p['evidence']}")
    if len(new) > 40:
        print(f"    ... and {len(new) - 40} more (use --emit json for all of them)")
    print()
    print(f"{len(unattributed)} test(s) NOT attributed — reported, never guessed:")
    reasons = defaultdict(int)
    for _t, _k, r in unattributed:
        reasons[r.split(" but ")[0].split(" across ")[0]] += 1
    for r, n in sorted(reasons.items(), key=lambda kv: -kv[1]):
        print(f"    {n:>4}  {r}")
    if known_unattr:
        print(f"    ({len(known_unattr)} of them ARE registered — the design knows the file")
        print("     exists, it just does not say which part the test is about.)")
    print()
    print("NOTHING HERE IS WRITTEN TO THE GRAPH. An attribution is a claim about what")
    print("a test is FOR; the two-clause rule is good evidence, not proof. Read the")
    print("proposals, then apply the ones that are right.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
