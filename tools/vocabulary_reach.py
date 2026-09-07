#!/usr/bin/env python3
"""Which of the design vocabulary can nothing actually write?

`req:declared-vocabulary-is-reachable-from-the-surface`. The failure this
measures happened FOUR TIMES IN TWO DAYS and was found by hand every time:

  Verification.description   declared, fulltext, the embedding field — used ONCE
                             in 164 nodes, because `add_verification` had no
                             parameter for it and `name` was the only string on
                             offer. Read as "nobody wants descriptions"; it was
                             unreachable, not unwanted.
  SUPERSEDES                 declared as an edge type, ZERO edges, while nine
                             nodes named their successor in prose instead.
  create_node's CAS          shipped demanding a precondition value it did not
                             return. Every core test passed.
  GOVERNED_BY.ruling         would have been reachable only through raw
                             create_edge; caught only because the first had
                             happened hours earlier.

⭐ WHY THIS READS THE CORPUS AND NOT THE SOURCE

The obvious instrument compares declared property names against served tool
parameter names. That was tried first and it OVERCOUNTS BADLY: it reported 70
of 215 properties (32%) unreachable, including `TemporalFact.valid_from`, which
is perfectly writable. A name match cannot see a tool that sets a property
under a different parameter name, or one written indirectly by `record_change`,
`reconcile_artifacts` or `link_artifact`.

So the primary signal here is USAGE IN A REAL DESIGN: how many nodes of a type
actually carry each declared property. A property carried by ZERO nodes across
a mature graph is evidence, not inference — and it is exactly how three of the
four above were spotted. The name match is still computed and reported, but as
a SECOND column and explicitly as a hint.

⚠️ WHAT THIS CANNOT CONCLUDE, and the requirement says so in as many words:

  (a) An unreachable field must NOT be read as one to delete. Three of the four
      above were WANTED and got a parameter.
  (b) Raw `create_node` / `create_edge` must NOT count as reachability. They
      accept an arbitrary property bag, which is precisely why they hid all
      four — a check computed against the generic escape hatch answers clean
      forever. They are excluded from the tool-parameter column below.
  (c) Zero usage is not proof of unreachability. A property can be writable and
      simply never have been needed. The output separates "zero AND no tool
      parameter" (the candidates) from "zero but a tool accepts it" (silent,
      and probably just unused).

🛑 THE TWO QUESTIONS ARE ASKED SEPARATELY, AND FUSING THEM COST A USER'S SESSION

Until 2026-09-07 the candidate loop began `if used: continue` — a property that
anything had ever written was dropped before the reachability question was ever
asked. That is one filter answering two questions, and each half then hid what
the other would have found:

  Requirement.priority       declares `default: medium`, and dynograph injects a
                             declared default AT WRITE. So all 207 requirements
                             carry it, the property reads as fully adopted, and
                             the string "priority" appears in NOT ONE of 170
                             served tools. Usage manufactured by a default is
                             not evidence of reach.
  DesignEpoch.description    reads as adopted on 8 of 202 epochs, every one
                             written through generic `create_node`. It is that
                             type's EMBEDDING FIELD — the field search finds an
                             epoch BY — and `add_epoch` cannot set it.

Both were reported by the dev_storyflow agent, as friction, in one session. This
instrument reported 14 candidates that day and neither was among them, because
each had been filtered out before it was examined. REACHABILITY is a property of
the SURFACE and needs no usage data at all; ADOPTION is a property of the GRAPH.
They are computed independently below, and a property no typed tool accepts is
reported whether or not anything wrote it — with the reason it looks adopted
stated beside it, because a default and an escape-hatch write have different
fixes. `fact:defect-a-declared-property-can-be-unreachable-and-invisible-to-the-reach-instrument-when-a-default-populates-it`.

⭐ AND IT IS WIRED INTO CI NOW, WHICH IS THE HALF THAT MAKES ANY OF IT MATTER

The same investigation found this script invoked by no workflow step, no gate
script and no build target: a correct instrument nobody runs catches nothing,
whatever its filter does. `--check` compares against a committed baseline and
exits non-zero when a property becomes unreachable that the baseline does not
carry. IT IS A REGRESSION GATE, NOT A CLEANLINESS GATE, and deliberately so —
the same shape as the intent-authority gate's grandfathering, and the same
shape as the ruling that the 183 pre-rule fixes are grandfathered rather than
backfilled. A gate that goes red on day one over a long-standing state is a
gate somebody switches off.

Reads the committed export and the committed toolsnaps; opens no store and
takes no lock. stdlib + PyYAML only.
"""

from __future__ import annotations

import collections
import json
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
EXPORT = REPO / "docs/design/reflow2.json"
SCHEMA_DIR = REPO / "schema"
TOOLSNAPS = REPO / "tools/toolsnaps"

# The generic escape hatches. Excluded deliberately — see (b) above.
GENERIC = {"create_node", "create_nodes", "create_edge", "create_edges"}

# What `--check` compares against. Every `Type.property` the surface cannot
# write TODAY. A new entry is a regression and fails the build; an entry that
# disappears (somebody gave the property a parameter) is progress and does not.
BASELINE = REPO / "tools/vocabulary_reach_baseline.json"


def declared() -> tuple[dict[str, dict], dict[str, dict]]:
    """Every declared node property and edge property, from schema/*.yaml."""
    try:
        import yaml
    except ImportError:
        print("PyYAML required", file=sys.stderr)
        raise SystemExit(2)
    nodes: dict[str, dict] = {}
    edges: dict[str, dict] = {}
    for f in sorted(SCHEMA_DIR.glob("*.yaml")):
        doc = (yaml.safe_load(f.read_text()) or {}).get("schema") or {}
        for name, spec in (doc.get("node_types") or {}).items():
            nodes.setdefault(name, {}).update((spec or {}).get("properties") or {})
        for name, spec in (doc.get("edge_types") or {}).items():
            edges.setdefault(name, {}).update((spec or {}).get("properties") or {})
    return nodes, edges


# THE CONSTRUCTOR FOR EACH TYPE, named explicitly because nothing derives it.
#
# 🛑 WHY A PER-TYPE SET AND NOT ONE FLAT ONE. A flat set of every parameter name
# on the surface reads as "some tool somewhere takes a field spelled this way",
# which is not the same question as "can I set this property on this type" — and
# the difference was MEASURED on 2026-09-07, on the very pair that prompted this
# work. `DesignEpoch.description` is declared, is that type's EMBEDDING FIELD,
# and `add_epoch` offers only id/name/epoch_type/sequence. The flat set called it
# reachable anyway, because six OTHER constructors take a parameter called
# `description`. One type's field covered another type's hole, and the property
# stayed invisible in a report built to find exactly it.
#
# A type absent from this map has no known constructor, so its properties are
# judged against the whole surface — the conservative reading, which under-reports
# rather than inventing a finding.
WRITES_TYPE = {
    "add_actor": "Actor",
    "add_artifact": "Artifact",
    "add_capability": "Capability",
    "add_change_event": "ChangeEvent",
    "add_component": "Component",
    "add_constraint": "Constraint",
    "add_contributor": "Contributor",
    "add_decision": "Decision",
    "add_design_rule": "DesignRule",
    "add_environment": "Environment",
    "add_epoch": "DesignEpoch",
    "add_flow": "Flow",
    "add_interface": "Interface",
    "add_project": "Project",
    "add_readiness": "ReadinessAssessment",
    "add_release": "Release",
    "add_requirement": "Requirement",
    "add_resource": "Resource",
    "add_verification": "Verification",
    "record_finding": "TemporalFact",
    # SETTERS AND THE OTHER WRITERS. A setter is as type-specific as a
    # constructor and leaks exactly the same way when it is left unmapped:
    # measured 2026-09-07, adding `description` to add_epoch made SIX
    # properties read as newly reachable instead of two, because `plan_epoch`
    # was not in this map and its parameters were therefore pooled as
    # cross-type. One type's field covering another's is the very miss this
    # map was added to stop, so the map has to cover every tool that writes a
    # type's own properties, not only the ones named `add_*`.
    "plan_epoch": "DesignEpoch",
    "link_artifact": "Artifact",
    "set_artifact_checksum": "Artifact",
    "set_artifact_checksums": "Artifact",
    "set_artifact_intent": "Artifact",
    "set_capability_delivery": "Capability",
    "set_capability_signature": "Capability",
    "set_capability_status": "Capability",
    "set_interface_spec": "Interface",
    "set_interface_designation": "Interface",
    "set_requirement_lineage": "Requirement",
    "set_requirement_status": "Requirement",
    "set_requirement_designation": "Requirement",
    "set_verification_status": "Verification",
    "set_verification_kind": "Verification",
    "set_decision_status": "Decision",
    "set_quality_target": "Decision",
    "set_epoch_status": "DesignEpoch",
    "set_project_mode": "Project",
    "genesis": "Project",
    "record_change": "ChangeEvent",
    "answer_question": "Question",
    "withdraw_question": "Question",
    "acknowledge_gap": "Question",
    "acknowledge_gaps": "Question",
    "withdraw_gap_acknowledgement": "Question",
    "gap_to_prompt": "Question",
    "gaps_to_prompts": "Question",
    "external_dependency": "Resource",
}


def tool_parameters() -> tuple[set[str], dict[str, set[str]]]:
    """Parameters the surface accepts: the flat set, and the per-type view.

    The flat set is every name a non-generic tool takes. The per-type view is,
    for each type with a known constructor, that constructor's own parameters
    PLUS every parameter of every tool that is not somebody else's constructor —
    the setters, the relation tools and the indirect writers (`record_change`,
    `link_artifact`, `reconcile_artifacts`), which genuinely do reach properties
    the constructor never names. Keeping those in is what stops this becoming
    the naive name-match the module docstring rejects for over-reporting.
    """
    params: set[str] = set()
    per_tool: dict[str, set[str]] = {}
    for f in sorted(TOOLSNAPS.glob("*.json")):
        if f.stem in GENERIC:
            continue
        snap = json.loads(f.read_text())
        # A READ CANNOT WRITE ANYTHING, so a read-only tool is no evidence that
        # a property is reachable — and its parameters collide with property
        # names by coincidence. `search_design`, `scan_nodes`, `what_next` and
        # `find_tools` all take a `limit`; `topic_report` takes a `query`. Seven
        # read tools were pooled as writers until this line, on the strength of
        # names that happen to match.
        if ((snap.get("annotations") or {}).get("readOnlyHint")) is True:
            continue
        names = set(((snap.get("inputSchema") or {}).get("properties") or {}).keys())
        per_tool[f.stem] = names
        params |= names
    shared: set[str] = set()
    for tool, names in per_tool.items():
        if tool not in WRITES_TYPE:
            shared |= names
    by_type: dict[str, set[str]] = {}
    for tool, node_type in WRITES_TYPE.items():
        by_type.setdefault(node_type, set()).update(per_tool.get(tool, set()))
    for node_type in by_type:
        by_type[node_type] |= shared
    return params, by_type


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    check_mode = "--check" in argv
    update_baseline = "--update-baseline" in argv
    unknown = [a for a in argv if a not in {"--check", "--update-baseline"}]
    if unknown:
        print(f"unknown argument(s): {' '.join(unknown)}", file=sys.stderr)
        print("usage: vocabulary_reach.py [--check | --update-baseline]", file=sys.stderr)
        return 2
    node_props, edge_props = declared()
    params, params_by_type = tool_parameters()
    doc = json.loads(EXPORT.read_text())

    # Corpus usage.
    per_type: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    type_counts: collections.Counter = collections.Counter()
    for n in doc["nodes"]:
        t = n["node_type"]
        type_counts[t] += 1
        for k in n.get("properties", {}):
            per_type[t][k] += 1
    edge_used: collections.Counter = collections.Counter()
    edge_prop_used: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for e in doc["edges"]:
        edge_used[e["edge_type"]] += 1
        for k in e.get("properties", {}) or {}:
            edge_prop_used[e["edge_type"]][k] += 1

    unreachable, unused_but_offered, no_instances, edge_dead = [], [], [], []
    # THE BUCKET THE FUSED FILTER COULD NOT PRODUCE: no typed tool accepts the
    # name, and yet the property is populated. Each entry carries WHY it looks
    # adopted, because that decides the fix — a schema default means every node
    # gets the value whether anyone meant it or not, while an escape-hatch write
    # means somebody wanted the property enough to reach past the typed surface
    # for it. Both are unreachable; only the second has a user behind it.
    hidden: list[tuple[str, str, int, int, str]] = []
    total_props = 0
    for t, props in sorted(node_props.items()):
        for p in sorted(props):
            total_props += 1
            used = per_type[t][p]
            spec = props[p] if isinstance(props[p], dict) else {}
            # REACHABILITY IS ASKED FIRST AND ALONE. It is a fact about the
            # served surface; no amount of usage makes an unoffered name
            # writable, and no amount of disuse makes an offered one unwritable.
            # Per-type when the type has a known constructor; the flat set
            # otherwise, which under-reports rather than inventing a finding.
            offered = p in params_by_type.get(t, params)
            if used:
                if not offered and type_counts[t]:
                    why = (
                        f"declares `default: {spec['default']}` — every node gets it at write"
                        if "default" in spec
                        else "written through generic create_node"
                    )
                    hidden.append((t, p, used, type_counts[t], why))
                continue
            if type_counts[t] == 0:
                # ⭐ "0 of 0" IS A VACUOUS ZERO and this bucket exists because
                # the first run of this instrument did not have it: six node
                # types have no instances at all, and every property on them
                # landed in the candidate list looking like evidence. A design
                # that has never created an EnvironmentRule says NOTHING about
                # whether EnvironmentRule.authority is writable. Reporting them
                # together would have been this tool committing the exact
                # defect its own epoch is named after.
                no_instances.append((t, p, 0))
            elif p not in params:
                unreachable.append((t, p, type_counts[t]))
            else:
                unused_but_offered.append((t, p, type_counts[t]))
    for et in sorted(edge_props):
        if edge_used[et] == 0:
            edge_dead.append((et, "the EDGE TYPE itself", 0))
        else:
            for p in sorted(edge_props[et]):
                if edge_prop_used[et][p] == 0:
                    edge_dead.append((et, p, edge_used[et]))

    # What the SURFACE cannot write, regardless of what the graph happens to
    # hold. This is the list `--check` guards, and it is the union of the two
    # unreachable buckets: the properties nothing has written and the ones that
    # only look written.
    unreachable_now = sorted(
        [f"{t}.{p}" for t, p, _ in unreachable] + [f"{t}.{p}" for t, p, _, _, _ in hidden]
    )

    if check_mode:
        if not BASELINE.exists():
            print(
                f"no baseline at {BASELINE.relative_to(REPO)} — write one with --update-baseline",
                file=sys.stderr,
            )
            return 2
        known = set(json.loads(BASELINE.read_text()).get("unreachable", []))
        new = [x for x in unreachable_now if x not in known]
        if new:
            print("=" * 74)
            print(f"UNREACHABLE VOCABULARY IS NEW SINCE THE BASELINE: {len(new)}")
            print("=" * 74)
            for x in new:
                print(f"  {x}")
            print()
            print(
                "A property was declared that no typed tool accepts, so the only way to write\n"
                "it is the generic escape hatch — and a schema default would make it look\n"
                "adopted while nobody could set it. Give it a parameter on the constructor for\n"
                "its type, or, if it is deliberately unreachable, record why and add it to\n"
                f"{BASELINE.relative_to(REPO)} with --update-baseline in the SAME diff."
            )
            return 1
        gone = sorted(known - set(unreachable_now))
        print(f"vocabulary reach: OK — {len(unreachable_now)} unreachable, none new.")
        if gone:
            print(
                f"  {len(gone)} became reachable since the baseline "
                f"(progress; refresh with --update-baseline): {', '.join(gone[:6])}"
            )
        return 0

    if update_baseline:
        BASELINE.write_text(
            json.dumps(
                {
                    "_comment": (
                        "Every Type.property the served surface cannot write, as of the last "
                        "refresh. `vocabulary_reach.py --check` fails when a NEW one appears. "
                        "A regression gate, not a cleanliness gate: entries here are known "
                        "and grandfathered, not approved."
                    ),
                    "unreachable": unreachable_now,
                },
                indent=2,
            )
            + "\n"
        )
        print(f"wrote {len(unreachable_now)} entries to {BASELINE.relative_to(REPO)}")
        return 0

    print(f"declared node properties : {total_props} across {len(node_props)} types")
    print(f"declared edge types      : {len(edge_props)}")
    print(f"non-generic tool params  : {len(params)}  (create_node/create_edge excluded)")
    print()
    print("=" * 74)
    print(f"UNREACHABLE BUT POPULATED — no typed tool accepts the name, and yet")
    print(f"nodes carry it: {len(hidden)}")
    print("  THE BUCKET A FUSED FILTER CANNOT PRODUCE. Usage is not reach: a schema")
    print("  default is injected at write, and generic create_node takes anything.")
    print("=" * 74)
    for t, p, used, n, why in hidden:
        print(f"  {t}.{p}".ljust(52) + f"({used} of {n})")
        print(f"      {why}")
    print()
    print("=" * 74)
    print(f"CANDIDATES — the type IS used, the property NEVER is, and no typed tool")
    print(f"accepts the name: {len(unreachable)}")
    print("=" * 74)
    for t, p, n in unreachable:
        print(f"  {t}.{p}".ljust(52) + f"(0 of {n} {t} nodes)")
    print()
    print("-" * 74)
    print(f"UNUSED BUT OFFERED — zero uses, but a typed tool does accept the name: {len(unused_but_offered)}")
    print("  Probably genuinely unused rather than unreachable. Reported so the")
    print("  count above cannot be quoted as 'everything the design cannot write'.")
    print("-" * 74)
    for t, p, n in unused_but_offered:
        print(f"  {t}.{p}".ljust(52) + f"(0 of {n})")
    print()
    print("-" * 74)
    print(f"SAYS NOTHING — the node type has NO INSTANCES, so a zero on its")
    print(f"properties is vacuous rather than evidence: {len(no_instances)}")
    print("  Across " + ", ".join(sorted({t for t, _, _ in no_instances})) + ".")
    print("-" * 74)
    print()
    print("-" * 74)
    print(f"EDGE VOCABULARY WITH NO INSTANCES: {len(edge_dead)}")
    print("-" * 74)
    for et, what, n in edge_dead:
        print(f"  {et}.{what}".ljust(52) + f"({n} edges of this type exist)")
    print()
    print("READ THE MODULE DOCSTRING BEFORE ACTING ON ANY OF THIS: zero usage is")
    print("evidence, not proof, and an unreachable field is never automatically")
    print("one to delete.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
