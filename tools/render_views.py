#!/usr/bin/env python3
"""Render design viewpoints as pure projections of the graph — and confess.

The doctrine (2026-07-19, from the project's author, a UAF/DoDAF-trained systems
engineer): the graph stores all design details; the agent's only job is to
*render* them as viewpoints. **If rendering requires extrapolating or filling in
missing details, that is a sign something is missing or wrong inside reflow2** —
not a prompt to improvise. This is the SYNTHESIZE process held to a
no-extrapolation standard, and it makes every renderer a probe.

So this script has one hard rule: it may only emit what the graph states.
Anything a viewpoint *needs* but the graph cannot supply goes on the
CONFESSIONS list, printed loudly and rendered into the page — because each entry
is either a modelling gap or a reflow2 gap, and hiding it is exactly the
extrapolation the doctrine forbids.

The catalogue lives in docs/viewpoints.md. Views rendered here (DoDAF-flavoured,
adapted to reflow2's vocabulary):
  - Functional (≈ OV-5-ish): Requirement ← SATISFIES ← Capability, with status.
  - Operational flow (≈ OV-5b/OV-6-ish): Flow steps in order, TRIGGERS
    transitions with their roles, cycles reported never judged (BL-37).
  - Structural (≈ SV-1-ish): Component containment + PROVIDES/CONSUMES Interfaces.
  - Traceability (≈ SV-5-ish): Capability → Component → Artifact → Verification.
  - As-released (≈ SV-8-ish): what each Release shipped, checksums frozen at
    cut, the as-released diff (BL-34).
  - Decisions (no clean DoDAF box — the record of *why*, which DoDAF leaves to
    AV-1 prose): Decision + GOVERNED_BY, rationale and standing.

Run:  python3 tools/render_views.py [export.json] [-o out.html]
      python3 tools/render_views.py --graph-path .reflow2/graph   # live graph,
        via `reflow2-mcp --export`; the graph is single-writer, so stop any
        running MCP session first (the error says so if you forget).
"""

from __future__ import annotations

import argparse
import html
import json
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

CONFESSIONS: list[str] = []


def confess(needed: str, because: str) -> None:
    CONFESSIONS.append(f"{needed} — {because}")


def esc(s: str) -> str:
    """Mermaid-safe label text."""
    return s.replace('"', "'").replace("[", "(").replace("]", ")")


def mid(node_id: str) -> str:
    """Mermaid-safe node identifier."""
    return node_id.replace(":", "_").replace("-", "_").replace(".", "_")


class Graph:
    def __init__(self, doc: dict):
        self.nodes = {n["node_id"]: n for n in doc["nodes"]}
        self.edges = doc["edges"]

    def of_type(self, t: str) -> list[dict]:
        return [n for n in self.nodes.values() if n["node_type"] == t]

    def edges_of(self, et: str) -> list[dict]:
        return [e for e in self.edges if e["edge_type"] == et]

    def prop(self, node_id: str, key: str) -> str | None:
        v = self.nodes.get(node_id, {}).get("properties", {}).get(key)
        return v if isinstance(v, str) else None

    def name(self, node_id: str) -> str:
        n = self.prop(node_id, "name")
        if n is None:
            confess(f"a display name for `{node_id}`",
                    "the node carries no `name` property; the id is shown instead")
            return node_id
        return n


def view_functional(g: Graph) -> str:
    """Requirements and the capabilities that satisfy them, with status."""
    out = ["flowchart LR"]
    sat = g.edges_of("SATISFIES")
    satisfied_reqs = {e["to_id"] for e in sat}
    for r in sorted(g.of_type("Requirement"), key=lambda n: n["node_id"]):
        rid = r["node_id"]
        prio = g.prop(rid, "priority") or ""
        out.append(f'  {mid(rid)}["{esc(g.name(rid))}{f" ({prio})" if prio else ""}"]')
        if rid not in satisfied_reqs:
            confess(f"what satisfies `{rid}`", "no SATISFIES edge reaches it (a true gap, shown dangling)")
    for c in sorted(g.of_type("Capability"), key=lambda n: n["node_id"]):
        cid = c["node_id"]
        status = g.prop(cid, "status")
        if status is None:
            confess(f"the build status of `{cid}`", "no `status` property")
            status = "?"
        out.append(f'  {mid(cid)}("{esc(g.name(cid))}<br/><i>{status}</i>")')
    for e in sorted(sat, key=lambda e: (e["from_id"], e["to_id"])):
        if e["from_id"] in g.nodes and e["to_id"] in g.nodes:
            out.append(f"  {mid(e['from_id'])} -->|satisfies| {mid(e['to_id'])}")
    return "\n".join(out)


def _sccs(nodes: list[str], arcs: list[tuple[str, str]]) -> list[list[str]]:
    """Strongly-connected components (iterative Tarjan), deterministic order.

    Mirrors `flow_report`'s contract: one entry per cluster of ≥2, plus
    degenerate self-arcs; each rotated to its smallest member.
    """
    idx: dict[str, int] = {}
    low: dict[str, int] = {}
    on: set[str] = set()
    stack: list[str] = []
    out: list[list[str]] = []
    succ: dict[str, list[str]] = {n: [] for n in nodes}
    for a, b in arcs:
        if a == b:
            out.append([a])
        else:
            succ[a].append(b)
    counter = [0]

    def strong(v: str) -> None:
        work = [(v, 0)]
        while work:
            node, pi = work.pop()
            if pi == 0:
                idx[node] = low[node] = counter[0]
                counter[0] += 1
                stack.append(node)
                on.add(node)
            recurse = False
            for i in range(pi, len(succ[node])):
                w = succ[node][i]
                if w not in idx:
                    work.append((node, i + 1))
                    work.append((w, 0))
                    recurse = True
                    break
                if w in on:
                    low[node] = min(low[node], idx[w])
            if recurse:
                continue
            if low[node] == idx[node]:
                comp = []
                while True:
                    w = stack.pop()
                    on.discard(w)
                    comp.append(w)
                    if w == node:
                        break
                if len(comp) >= 2:
                    comp.sort()
                    out.append(comp)
            if work:
                parent = work[-1][0]
                low[parent] = min(low[parent], low[node])

    for n in sorted(nodes):
        if n not in idx:
            strong(n)
    return sorted(out)


def view_flows(g: Graph) -> str | None:
    """Flow steps in stated order, roled transitions, cycles as facts (BL-37)."""
    flows = sorted(g.of_type("Flow"), key=lambda n: n["node_id"])
    if not flows:
        confess("an operational-flow viewpoint (≈ OV-5b/OV-6: activities in order)",
                "the graph holds no Flow nodes. The vocabulary can express one "
                "(add_flow / part_of_flow / TRIGGERS.role, BL-37), so this is a "
                "modelling gap: either the design has no ordered process, or "
                "nobody has modelled it")
        return None

    membership: dict[str, list[dict]] = {}
    for e in g.edges_of("PART_OF_FLOW"):
        membership.setdefault(e["to_id"], []).append(e)

    blocks: list[str] = []
    for f in flows:
        fid = f["node_id"]
        members = membership.get(fid, [])
        if not members:
            confess(f"the steps of flow `{fid}`",
                    "no PART_OF_FLOW edge reaches it — a process with no stated steps")
            continue

        def order_of(e: dict):
            v = e["properties"].get("step_order")
            return (0, v, e["from_id"]) if isinstance(v, int) else (1, 0, e["from_id"])

        unordered = sum(1 for e in members if not isinstance(e["properties"].get("step_order"), int))
        if unordered and len(members) > 1:
            confess(f"the position of {unordered} step(s) in `{fid}`",
                    "no `step_order` on their PART_OF_FLOW edge; listed after the "
                    "ordered ones in id order, because the graph never said where they go")
        member_ids = {e["from_id"] for e in members}
        for e in members:
            if e["from_id"] not in g.nodes:
                confess(f"the capability behind step `{e['from_id']}` of `{fid}`",
                        "a PART_OF_FLOW edge names it and no Capability node exists")

        out = ["flowchart LR"]
        for e in sorted(members, key=order_of):
            so = e["properties"].get("step_order")
            label = esc(g.name(e["from_id"]))
            out.append(f'  {mid(e["from_id"])}["{f"{so}. " if isinstance(so, int) else ""}{label}"]')

        arcs: list[tuple[str, str]] = []
        unroled = 0
        for e in sorted(g.edges_of("TRIGGERS"), key=lambda e: (e["from_id"], e["to_id"])):
            if e["from_id"] in member_ids and e["to_id"] in member_ids:
                role = e["properties"].get("role")
                if not isinstance(role, str):
                    unroled += 1
                    out.append(f"  {mid(e['from_id'])} --> {mid(e['to_id'])}")
                else:
                    out.append(f"  {mid(e['from_id'])} -->|{esc(role)}| {mid(e['to_id'])}")
                arcs.append((e["from_id"], e["to_id"]))
        if unroled:
            confess(f"what {unroled} transition(s) in `{fid}` mean",
                    "no `role` on the TRIGGERS edge — forward and feedback are "
                    "indistinguishable there, which for a process is the load-bearing fact")

        for which in ("entry_point", "exit_point"):
            v = g.prop(fid, which)
            if v is not None and v not in member_ids and not any(
                    g.prop(m, "name") == v for m in member_ids):
                confess(f"the {which} `{v}` of `{fid}`",
                        "it matches no member of the flow")

        # An SCC states mutual reachability, not a walk order — rendering it
        # with arrows would assert a path the graph never stated. Braces only.
        cycles = _sccs(sorted(member_ids), arcs)
        cyc_line = ("<p class=\"vp\">cycles (reported, never judged — each a cluster of "
                    "mutually reachable steps): "
                    + "; ".join("{ " + ", ".join(esc(g.name(n)) for n in c) + " }" for c in cycles)
                    + "</p>") if cycles else ""
        ftype = g.prop(fid, "flow_type")
        blocks.append(
            f'<h3>{html.escape(g.name(fid))}'
            + (f' <small>({html.escape(ftype)})</small>' if ftype else "")
            + f'</h3><div class="card"><pre class="mermaid">{chr(10).join(out)}</pre></div>{cyc_line}')
    return "\n".join(blocks) if blocks else None


def view_structural(g: Graph) -> str:
    """Components in their containment tree; interfaces with both sides."""
    out = ["flowchart TB"]
    contains = [e for e in g.edges_of("CONTAINS")
                if g.nodes.get(e["from_id"], {}).get("node_type") == "Component"
                and g.nodes.get(e["to_id"], {}).get("node_type") == "Component"]
    children = {e["to_id"] for e in contains}
    parents: dict[str, list[str]] = {}
    for e in contains:
        parents.setdefault(e["from_id"], []).append(e["to_id"])

    def emit_component(cid: str, indent: str) -> None:
        kids = sorted(parents.get(cid, []))
        if kids:
            out.append(f'{indent}subgraph {mid(cid)}["{esc(g.name(cid))}"]')
            for k in kids:
                emit_component(k, indent + "  ")
            out.append(f"{indent}end")
        else:
            out.append(f'{indent}{mid(cid)}["{esc(g.name(cid))}"]')

    for c in sorted(g.of_type("Component"), key=lambda n: n["node_id"]):
        if c["node_id"] not in children:
            emit_component(c["node_id"], "  ")
    for i in sorted(g.of_type("Interface"), key=lambda n: n["node_id"]):
        out.append(f'  {mid(i["node_id"])}{{{{"{esc(g.name(i["node_id"]))}"}}}}')
    provided = {e["to_id"] for e in g.edges_of("PROVIDES")}
    consumed = {e["to_id"] for e in g.edges_of("CONSUMES")}
    for e in sorted(g.edges_of("PROVIDES"), key=lambda e: (e["from_id"], e["to_id"])):
        out.append(f"  {mid(e['from_id'])} -->|provides| {mid(e['to_id'])}")
    for e in sorted(g.edges_of("CONSUMES"), key=lambda e: (e["from_id"], e["to_id"])):
        out.append(f"  {mid(e['to_id'])} -->|consumed by| {mid(e['from_id'])}")
    for i in g.of_type("Interface"):
        if i["node_id"] not in provided:
            confess(f"who provides `{i['node_id']}`", "no PROVIDES edge (a true gap, shown dangling)")
        if i["node_id"] not in consumed:
            confess(f"who consumes `{i['node_id']}`", "no CONSUMES edge (a true gap, shown dangling)")
    return "\n".join(out)


def view_traceability(g: Graph) -> str:
    """Capability → allocated Component → realizing Artifact → Verification, as a table."""
    alloc: dict[str, list[str]] = {}
    for e in g.edges_of("ALLOCATED_TO"):
        alloc.setdefault(e["from_id"], []).append(e["to_id"])
    realizes: dict[str, list[str]] = {}
    for e in g.edges_of("REALIZES"):
        realizes.setdefault(e["to_id"], []).append(e["from_id"])
    verifies: dict[str, list[str]] = {}
    for e in g.edges_of("VERIFIES"):
        verifies.setdefault(e["to_id"], []).append(e["from_id"])

    rows = []
    for c in sorted(g.of_type("Capability"), key=lambda n: n["node_id"]):
        cid = c["node_id"]
        cmps = sorted(alloc.get(cid, []))
        arts = realizes.get(cid, []) or [a for c2 in cmps for a in realizes.get(c2, [])]
        vers = verifies.get(cid, [])
        ver_cells = []
        for v in vers:
            st = g.prop(v, "status")
            if st is None:
                confess(f"the outcome of `{v}`", "Verification has no `status` property")
                st = "?"
            ver_cells.append(f"{esc(g.name(v))} <i>({st})</i>")
        rows.append(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>".format(
                esc(g.name(cid)),
                ", ".join(esc(g.name(c2)) for c2 in cmps) or "<em>unallocated</em>",
                ", ".join(esc(g.name(a)) for a in sorted(arts)) or "<em>none</em>",
                ", ".join(sorted(ver_cells)) or "<em>none</em>",
            )
        )
    return ("<table><thead><tr><th>Capability</th><th>Allocated to</th>"
            "<th>Realized by</th><th>Verified by</th></tr></thead><tbody>"
            + "\n".join(rows) + "</tbody></table>")


def view_released(g: Graph) -> str:
    """What each Release shipped, checksums frozen at cut, deployments,
    and the as-released diff (BL-34). Both P3 shapes count as built."""
    releases = sorted(g.of_type("Release"), key=lambda n: n["node_id"])
    if not releases:
        return ('<p class="vp">Nothing to project: the graph holds no Release. '
                "(The phase detectors ask about this as <code>no_deploy_operate</code>; "
                "absence is the graph's honest statement, not a rendering gap.)</p>")

    realizes: dict[str, list[str]] = {}
    for e in g.edges_of("REALIZES"):
        realizes.setdefault(e["to_id"], []).append(e["from_id"])
    alloc: dict[str, list[str]] = {}
    for e in g.edges_of("ALLOCATED_TO"):
        alloc.setdefault(e["from_id"], []).append(e["to_id"])

    blocks = []
    for r in releases:
        rid = r["node_id"]
        version = g.prop(rid, "version")
        head = esc(g.name(rid)) + (f" <small>({esc(version)})</small>" if version else "")
        includes = [e for e in g.edges_of("INCLUDES") if e["from_id"] == rid]
        if not includes:
            confess(f"what `{rid}` shipped",
                    "the Release exists and no INCLUDES edge records its contents "
                    "(reflow2 reports the same shape as `unreleased_component`)")
            blocks.append(f"<h3>{head}</h3><p class=\"vp\"><em>contents not modelled</em></p>")
            continue

        shipped_art: set[str] = set()
        art_rows = []
        cmp_cells = []
        for e in sorted(includes, key=lambda e: e["to_id"]):
            tid = e["to_id"]
            if g.nodes.get(tid, {}).get("node_type") == "Artifact":
                shipped_art.add(tid)
                frozen = e["properties"].get("as_checksum")
                if not isinstance(frozen, str):
                    confess(f"the shipped hash of `{tid}` in `{rid}`",
                            "INCLUDES carries no `as_checksum`, so what the release "
                            "actually contained cannot be told from the artifact's "
                            "moving baseline")
                    frozen = "?"
                art_rows.append(f"<tr><td>{esc(g.name(tid))}</td>"
                                f"<td><code>{html.escape(frozen)}</code></td></tr>")
            else:
                cmp_cells.append(esc(g.name(tid)))

        covered, not_covered = [], []
        for c in sorted(g.of_type("Capability"), key=lambda n: n["node_id"]):
            cid = c["node_id"]
            building = list(realizes.get(cid, []))
            for c2 in alloc.get(cid, []):
                building.extend(realizes.get(c2, []))
            if not building:
                continue
            (covered if any(a in shipped_art for a in building) else not_covered).append(cid)

        deploys = [e for e in g.edges_of("DEPLOYED_TO") if e["from_id"] == rid]
        dep_cells = []
        for e in sorted(deploys, key=lambda e: e["to_id"]):
            st = e["properties"].get("status")
            dep_cells.append(esc(g.name(e["to_id"])) + (f" <i>({esc(st)})</i>" if isinstance(st, str) else ""))

        diff = (", ".join(esc(g.name(c)) for c in not_covered)
                if not_covered else "none — every built capability shipped")
        blocks.append(
            f"<h3>{head}</h3>"
            "<table><thead><tr><th>Shipped artifact</th><th>Checksum at cut</th></tr></thead>"
            f"<tbody>{''.join(art_rows)}</tbody></table>"
            + (f'<p class="vp">components included: {", ".join(cmp_cells)}</p>' if cmp_cells else "")
            + f'<p class="vp">capabilities covered: {len(covered)} · '
            f"<b>built but not shipped (the as-released diff):</b> {diff}</p>"
            + (f'<p class="vp">deployed to: {", ".join(dep_cells)}</p>' if dep_cells
               else '<p class="vp">deployed to: <em>nowhere the graph records</em></p>'))
    return "\n".join(blocks)


def view_decisions(g: Graph) -> str:
    """The record of why: Decision + GOVERNED_BY. What was decided, the
    rationale, its standing, and which parts of the design it governs."""
    decisions = sorted(g.of_type("Decision"), key=lambda n: n["node_id"])
    if not decisions:
        return ('<p class="vp">Nothing to project: the graph records no Decision. '
                "Choices were made — they always are — but none is on the record, "
                "so nothing here to render.</p>")

    governs: dict[str, list[str]] = {}
    for e in g.edges_of("GOVERNED_BY"):
        governs.setdefault(e["to_id"], []).append(e["from_id"])

    rows = []
    for d in decisions:
        did = d["node_id"]
        decided = g.prop(did, "decision")
        if decided is None:
            confess(f"what `{did}` decided", "no `decision` property, which the schema requires")
            decided = "?"
        rationale = g.prop(did, "rationale")
        if rationale is None:
            confess(f"why `{did}` was decided",
                    "no `rationale` — a decision without its why is a conclusion, not a record")
            rationale = "<em>not recorded</em>"
        else:
            rationale = esc(rationale)
        status = g.prop(did, "status") or "accepted"
        ruled = governs.get(did, [])
        if not ruled:
            confess(f"what `{did}` governs",
                    "no GOVERNED_BY edge reaches it — a decision attached to nothing it decides")
        rows.append(
            "<tr><td><b>{}</b><br/>{}</td><td>{}</td><td><i>{}</i></td><td>{}</td></tr>".format(
                esc(g.name(did)), esc(decided), rationale, esc(status),
                ", ".join(esc(g.name(n)) for n in sorted(ruled)) or "<em>nothing</em>",
            ))
    return ("<table><thead><tr><th>Decision</th><th>Why</th><th>Standing</th>"
            "<th>Governs</th></tr></thead><tbody>" + "\n".join(rows) + "</tbody></table>")


def view_evolution(g: Graph) -> str:
    """Axis Z as a viewpoint: the epoch chain, and what happened at each.

    Order comes only from what is stated: `PRECEDES` edges (drawn solid) or the
    `sequence` property (drawn dotted, labelled with its source). When both
    exist they are checked against each other — a disagreement is a confession,
    and a `PRECEDES` cycle is the chain contradicting itself.
    """
    epochs = sorted(g.of_type("DesignEpoch"), key=lambda n: n["node_id"])
    if not epochs:
        return ('<p class="vp">Nothing to project: the graph records no DesignEpoch. '
                "Axis Z's write side is <code>add_epoch</code> / <code>record_change</code>; "
                "a design with no epochs has no recorded history to draw.</p>")

    eids = {e["node_id"] for e in epochs}
    prec = [(e["from_id"], e["to_id"]) for e in g.edges_of("PRECEDES")
            if e["from_id"] in eids and e["to_id"] in eids]

    def seq_of(eid: str):
        v = g.nodes[eid]["properties"].get("sequence")
        return v if isinstance(v, int) else None

    # The chain contradicting itself is a model contradiction, stated as such.
    for cluster in _sccs(sorted(eids), prec):
        confess("a consistent order for the epoch chain",
                "PRECEDES forms a cycle among { " + ", ".join(cluster) + " } — "
                "the chain contradicts itself")

    # Where both orderings are stated, they must agree.
    for a, b in sorted(prec):
        sa, sb = seq_of(a), seq_of(b)
        if sa is not None and sb is not None and sa >= sb:
            confess(f"which of `{a}` and `{b}` comes first",
                    f"PRECEDES says {a} → {b} but their sequence numbers say "
                    f"{sa} ≥ {sb} — the two stated orderings disagree")

    if len(epochs) > 1:
        chained = {x for pair in prec for x in pair}
        unplaced = [e["node_id"] for e in epochs
                    if seq_of(e["node_id"]) is None and e["node_id"] not in chained]
        if unplaced:
            confess("the position of epoch(s) " + ", ".join(f"`{e}`" for e in unplaced),
                    "no PRECEDES edge and no sequence number — the timeline cannot "
                    "place them, so they are listed after the ordered ones in id order")

    # Deterministic display order: sequence first (stated), then id.
    def display_key(n: dict):
        s = seq_of(n["node_id"])
        return (0, s, n["node_id"]) if s is not None else (1, 0, n["node_id"])

    ordered = sorted(epochs, key=display_key)

    out = ["flowchart LR"]
    for e in ordered:
        eid = e["node_id"]
        etype = g.prop(eid, "epoch_type") or ""
        out.append(f'  {mid(eid)}["{esc(g.name(eid))}'
                   f'{f"<br/><i>{esc(etype)}</i>" if etype else ""}"]')
    if prec:
        for a, b in sorted(prec):
            out.append(f"  {mid(a)} -->|precedes| {mid(b)}")
    else:
        # No chain edges: the dotted arrow renders the `sequence` ordering and
        # says so — order from a stated property, drawn with its source named.
        seq_sorted = [e["node_id"] for e in ordered if seq_of(e["node_id"]) is not None]
        for a, b in zip(seq_sorted, seq_sorted[1:]):
            out.append(f"  {mid(a)} -.->|sequence| {mid(b)}")

    # What happened at each epoch: anything pinned or occurring there.
    at: dict[str, list[str]] = {}
    for et in ("AT_EPOCH", "OCCURS_DURING"):
        for e in g.edges_of(et):
            if e["to_id"] in eids and e["from_id"] in g.nodes:
                src = g.nodes[e["from_id"]]
                kind = src["node_type"]
                extra = src["properties"].get("change_type")
                label = esc(g.name(e["from_id"])) + f" <i>({kind}" + \
                    (f": {esc(extra)}" if isinstance(extra, str) else "") + ")</i>"
                at.setdefault(e["to_id"], []).append(label)

    pinned_events = {e["from_id"] for et in ("AT_EPOCH", "OCCURS_DURING")
                     for e in g.edges_of(et) if e["to_id"] in eids}
    floating = [n["node_id"] for n in g.of_type("ChangeEvent")
                if n["node_id"] not in pinned_events]
    if floating:
        confess("when " + ", ".join(f"`{c}`" for c in sorted(floating)) + " happened",
                "the ChangeEvent is pinned to no epoch (no AT_EPOCH / OCCURS_DURING) — "
                "a change floating off the timeline is the axis-Z discipline broken")

    rows = "".join(
        "<tr><td>{}<br/><i>{}</i></td><td>{}</td></tr>".format(
            esc(g.name(e["node_id"])), esc(g.prop(e["node_id"], "epoch_type") or ""),
            ", ".join(sorted(at.get(e["node_id"], []))) or "<em>nothing recorded here</em>",
        ) for e in ordered)
    single = ('<p class="vp">One epoch and nothing after it: axis Z has recorded no '
              "evolution yet. The write side is <code>record_change</code>; the chain is "
              "drawn by <code>precedes</code>.</p>") if len(epochs) == 1 else ""
    return (f'<div class="card"><pre class="mermaid">{chr(10).join(out)}</pre></div>{single}'
            "<table><thead><tr><th>Epoch</th><th>What happened there</th></tr></thead>"
            f"<tbody>{rows}</tbody></table>")


def view_provenance(g: Graph) -> str:
    """Where the design came from: `provenance` per node, and the Fragments
    that yielded nodes (`YIELDED`, with the action taken)."""
    typed = ("Requirement", "Capability", "Component", "Interface")
    counts: dict[str, dict[str, int]] = {}
    unstated: list[str] = []
    inferred: list[str] = []
    for t in typed:
        for n in g.of_type(t):
            p = n["properties"].get("provenance")
            if not isinstance(p, str):
                unstated.append(n["node_id"])
                p = "unstated"
            elif p == "inferred":
                inferred.append(n["node_id"])
            counts.setdefault(t, {})
            counts[t][p] = counts[t].get(p, 0) + 1
    if unstated:
        confess("the origin of " + ", ".join(f"`{n}`" for n in sorted(unstated)),
                "no `provenance` property — authored by a stakeholder and inferred "
                "from an implementation are different kinds of truth, and these "
                "nodes state neither")

    all_provs = sorted({p for c in counts.values() for p in c})
    header = "".join(f"<th>{html.escape(p)}</th>" for p in all_provs)
    body = "".join(
        f"<tr><td>{t}</td>" + "".join(
            f"<td>{counts.get(t, {}).get(p, 0) or ''}</td>" for p in all_provs) + "</tr>"
        for t in typed if counts.get(t))
    summary = (f"<table><thead><tr><th>Type</th>{header}</tr></thead>"
               f"<tbody>{body}</tbody></table>") if counts else \
        '<p class="vp">Nothing to project: no Requirement / Capability / Component / Interface.</p>'

    inferred_line = ('<p class="vp"><b>Marked inferred</b> (read out of an implementation — '
                     "satisfied by construction, so they can never contradict anything): "
                     + ", ".join(esc(g.name(n)) for n in sorted(inferred)) + "</p>"
                     ) if inferred else \
        '<p class="vp">Nothing is marked <i>inferred</i>.</p>'

    frags = sorted(g.of_type("Fragment"), key=lambda n: n["node_id"])
    yielded: dict[str, list[tuple[str, str]]] = {}
    for e in g.edges_of("YIELDED"):
        action = e["properties"].get("action")
        yielded.setdefault(e["from_id"], []).append(
            (e["to_id"], action if isinstance(action, str) else ""))
        if e["from_id"] in g.nodes and e["to_id"] not in g.nodes:
            confess(f"what fragment `{e['from_id']}` yielded",
                    f"YIELDED names `{e['to_id']}` and no such node exists — a dangling "
                    "provenance claim")
    frag_rows = []
    for f in frags:
        fid = f["node_id"]
        got = yielded.get(fid, [])
        if not got:
            confess(f"what `{fid}` produced",
                    "a recorded source with no YIELDED edge — either it truly yielded "
                    "nothing, or the provenance link was never drawn")
        cells = ", ".join(
            (esc(g.name(t)) if t in g.nodes else f"<em>{esc(t)}?</em>")
            + (f" <i>({esc(a)})</i>" if a else "") for t, a in sorted(got))
        title = g.prop(fid, "title") or g.name(fid)
        ftype = g.prop(fid, "fragment_type") or ""
        fprov = g.prop(fid, "provenance") or ""
        frag_rows.append(f"<tr><td>{esc(title)}</td><td>{esc(ftype)}</td>"
                         f"<td>{esc(fprov)}</td><td>{cells or '<em>nothing</em>'}</td></tr>")
    frag_table = ("<h3>Sources (Fragments) and what they yielded</h3>"
                  "<table><thead><tr><th>Fragment</th><th>Kind</th><th>Provenance</th>"
                  "<th>Yielded</th></tr></thead><tbody>" + "".join(frag_rows)
                  + "</tbody></table>") if frags else \
        '<p class="vp">No Fragments: nothing records <i>which source</i> produced the nodes above.</p>'

    return summary + inferred_line + frag_table


PAGE = """<title>{title} — projected viewpoints</title>
<style>
  :root {{ --ink:#15211F; --paper:#F6F8F7; --card:#FFFFFF; --rule:#D8E0DD;
           --accent:#1E6E5C; --warn:#9A4A32; --faint:#66756F; }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --ink:#E3EAE7; --paper:#101615; --card:#182120; --rule:#2C3936;
             --accent:#63BFA8; --warn:#D98B6F; --faint:#8AA09A; }} }}
  :root[data-theme="dark"] {{ --ink:#E3EAE7; --paper:#101615; --card:#182120;
    --rule:#2C3936; --accent:#63BFA8; --warn:#D98B6F; --faint:#8AA09A; }}
  :root[data-theme="light"] {{ --ink:#15211F; --paper:#F6F8F7; --card:#FFFFFF;
    --rule:#D8E0DD; --accent:#1E6E5C; --warn:#9A4A32; --faint:#66756F; }}
  body {{ background:var(--paper); color:var(--ink);
         font-family:system-ui,-apple-system,"Segoe UI",sans-serif; line-height:1.55; }}
  main {{ max-width:1080px; margin:0 auto; padding:3rem 1.25rem 5rem; }}
  h1 {{ font-size:1.55rem; letter-spacing:-.015em; margin:0; text-wrap:balance; }}
  .sub {{ color:var(--faint); margin:.5rem 0 0; max-width:70ch; }}
  h2 {{ font-size:1.05rem; margin:2.75rem 0 .25rem; color:var(--accent);
       text-transform:uppercase; letter-spacing:.08em; font-weight:600; }}
  h3 {{ font-size:.95rem; margin:1.2rem 0 .4rem; }}
  h3 small {{ color:var(--faint); font-weight:400; }}
  .vp {{ color:var(--faint); font-size:.85rem; margin:.35rem 0 .9rem; }}
  div.card {{ background:var(--card); border:1px solid var(--rule);
       border-radius:4px; padding:1rem; overflow-x:auto; }}
  table {{ border-collapse:collapse; width:100%; font-size:.9rem; }}
  th,td {{ text-align:left; padding:.55rem .7rem; border-bottom:1px solid var(--rule);
          vertical-align:top; }}
  th {{ font-size:.72rem; text-transform:uppercase; letter-spacing:.09em;
       color:var(--faint); }}
  em {{ color:var(--warn); font-style:normal; }}
  .confess {{ border-left:3px solid var(--warn); background:var(--card);
             padding: .9rem 1.1rem; border-radius:0 4px 4px 0; }}
  .confess li {{ margin:.3rem 0; }}
  .confess p {{ margin:.2rem 0 .7rem; }}
  code {{ font-family:ui-monospace,Menlo,Consolas,monospace; font-size:.86em; }}
</style>
<main>
  <h1>{title} — projected viewpoints</h1>
  <p class="sub">Every element below is projected from <code>{source}</code> ({n_nodes} nodes,
  {n_edges} edges). Nothing is hand-drawn: the renderer may only emit what the graph states, and
  everything it needed but could not find is confessed at the end — per the doctrine that a
  fill-in during rendering is a defect in the model, not a job for the agent. The catalogue:
  <code>docs/viewpoints.md</code>.</p>

  <section><h2>Functional viewpoint</h2>
  <p class="vp">≈ OV-5: what must be true, and what the system does about it. Capability status from the graph.</p>
  <div class="card"><pre class="mermaid">{functional}</pre></div></section>

  <section><h2>Operational flow viewpoint</h2>
  <p class="vp">≈ OV-5b/OV-6: activities in stated order, transitions labelled with what they
  mean. A process's cycles are its design — listed as facts, never defects (BL-37).</p>
  {flows}</section>

  <section><h2>Structural viewpoint</h2>
  <p class="vp">≈ SV-1: parts, their containment, and the contracts between them — both sides of each interface.</p>
  <div class="card"><pre class="mermaid">{structural}</pre></div></section>

  <section><h2>Traceability viewpoint</h2>
  <p class="vp">≈ SV-5: function → structure → build → proof, one row per capability.</p>
  <div class="card">{traceability}</div></section>

  <section><h2>As-released viewpoint</h2>
  <p class="vp">≈ SV-8: what actually shipped, hashes frozen at cut so history cannot rewrite
  itself, and the diff against what was built (BL-34).</p>
  <div class="card">{released}</div></section>

  <section><h2>Decision viewpoint</h2>
  <p class="vp">No clean DoDAF box — the record of <i>why</i>, which DoDAF leaves to AV-1 prose.
  Each decision, its rationale, and what it governs.</p>
  <div class="card">{decisions}</div></section>

  <section><h2>Evolution viewpoint</h2>
  <p class="vp">≈ SV-8 proper (axis Z): the epoch chain and what happened at each. Solid arrows
  are stated PRECEDES edges; dotted arrows render the stated <code>sequence</code> numbers and
  say so. The two orderings are checked against each other.</p>
  {evolution}</section>

  <section><h2>Provenance viewpoint</h2>
  <p class="vp">≈ AV-2-ish: where the design came from — authored vs inferred per node, and the
  recorded sources (Fragments) with what each yielded.</p>
  <div class="card">{provenance}</div></section>

  <section><h2>What the graph could not tell this renderer</h2>
  <div class="confess">
  <p>{n_confessions} item(s). Each is a modelling gap, a reflow2 gap, or a true design gap — never
  something to improvise past.</p>
  <ul>{confessions}</ul></div></section>
</main>
"""


def load_document(args: argparse.Namespace) -> tuple[dict, str]:
    """The export document, from a file or from a live graph directory."""
    if args.graph_path:
        binary = pathlib.Path(args.bin)
        if not binary.exists():
            sys.exit(f"no binary at {binary} — build it: cargo build -p reflow2-mcp")
        proc = subprocess.run(
            [str(binary), "--graph-path", args.graph_path, "--export"],
            capture_output=True, text=True,
        )
        if proc.returncode != 0:
            # The binary's own message is the good one (lock → stop the server).
            sys.exit(proc.stderr.strip() or "export failed with no message")
        return json.loads(proc.stdout), f"live graph at {args.graph_path}"
    p = pathlib.Path(args.export)
    return json.loads(p.read_text()), p.name


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("export", nargs="?", default=str(REPO / "docs/design/reflow2.json"))
    ap.add_argument("--graph-path", default=None,
                    help="project a live graph directory instead of an export file "
                         "(runs `reflow2-mcp --export`; single-writer — stop any "
                         "running MCP session first)")
    ap.add_argument("--bin", default=str(REPO / "target/debug/reflow2-mcp"))
    ap.add_argument("-o", "--out", default=str(REPO / "docs/design/views.html"))
    args = ap.parse_args()

    doc, source = load_document(args)
    g = Graph(doc)
    projects = g.of_type("Project")
    title = g.name(projects[0]["node_id"]) if projects else "(no Project node)"
    if not projects:
        confess("a title for the page", "the graph holds no Project node")

    if not any(g.nodes.get(e["from_id"], {}).get("node_type") == "Capability"
               and g.nodes.get(e["to_id"], {}).get("node_type") == "Capability"
               for e in g.edges_of("DEPENDS_ON")) and not g.of_type("Flow"):
        # A Flow also states capability ordering, so this only fires when the
        # graph expresses no ordering of any kind.
        confess("the functional dependency ordering between capabilities",
                "no Capability -DEPENDS_ON-> Capability edges exist, so the functional view "
                "shows a set, not a flow — and evaluate_allocation is inert for the same "
                "reason (a committed-model gap, on the record per sharpening.md §2)")

    functional = view_functional(g)
    flows = view_flows(g)
    structural = view_structural(g)
    traceability = view_traceability(g)
    released = view_released(g)
    decisions = view_decisions(g)
    evolution = view_evolution(g)
    provenance = view_provenance(g)

    dedup = sorted(set(CONFESSIONS))
    page = PAGE.format(
        title=html.escape(title),
        source=html.escape(source),
        n_nodes=len(doc["nodes"]), n_edges=len(doc["edges"]),
        functional=functional,
        flows=flows or '<p class="vp">Nothing to project — see the confession below.</p>',
        structural=structural, traceability=traceability,
        released=released, decisions=decisions,
        evolution=evolution, provenance=provenance,
        n_confessions=len(dedup),
        confessions="\n".join(f"<li>{html.escape(c)}</li>" for c in dedup) or "<li>nothing — every view was fully specified</li>",
    )
    pathlib.Path(args.out).write_text(page)
    print(f"wrote {args.out}  (projected from {source})")
    print(f"\n== confessions: {len(dedup)} ==")
    for c in dedup:
        print(f"  - {c}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
