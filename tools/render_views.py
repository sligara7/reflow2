#!/usr/bin/env python3
"""Render design viewpoints as pure projections of the graph — and confess.

The doctrine (2026-07-19, from the project's author, a UAF/DoDAF-trained systems
engineer): the graph stores all design details; the agent's only job is to
*render* them as viewpoints. **If rendering requires extrapolating or filling in
missing details, that is a sign something is missing or wrong inside reflow2** —
not a prompt to improvise. This is the SYNTHESIZE process held to a
no-extrapolation standard, and it makes every renderer a probe.

So this script has one hard rule: it may only emit what the export document
states. Anything a viewpoint *needs* but the graph cannot supply goes on the
CONFESSIONS list, printed loudly and rendered into the page — because each entry
is either a modelling gap or a reflow2 gap, and hiding it is exactly the
extrapolation the doctrine forbids.

Views (DoDAF-flavoured, adapted to reflow2's vocabulary):
  - Functional (≈ OV-5-ish): Requirement ← SATISFIES ← Capability, with status.
  - Structural (≈ SV-1-ish): Component containment + PROVIDES/CONSUMES Interfaces.
  - Traceability (≈ SV-5-ish): Capability → Component → Artifact → Verification.

Run:  python3 tools/render_views.py [export.json] [-o out.html]
"""

from __future__ import annotations

import argparse
import html
import json
import pathlib
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
  .vp {{ color:var(--faint); font-size:.85rem; margin:0 0 .9rem; }}
  section > div.card {{ background:var(--card); border:1px solid var(--rule);
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
  fill-in during rendering is a defect in the model, not a job for the agent.</p>

  <section><h2>Functional viewpoint</h2>
  <p class="vp">≈ OV-5: what must be true, and what the system does about it. Capability status from the graph.</p>
  <div class="card"><pre class="mermaid">{functional}</pre></div></section>

  <section><h2>Structural viewpoint</h2>
  <p class="vp">≈ SV-1: parts, their containment, and the contracts between them — both sides of each interface.</p>
  <div class="card"><pre class="mermaid">{structural}</pre></div></section>

  <section><h2>Traceability viewpoint</h2>
  <p class="vp">≈ SV-5: function → structure → build → proof, one row per capability.</p>
  <div class="card">{traceability}</div></section>

  <section><h2>What the graph could not tell this renderer</h2>
  <div class="confess">
  <p>{n_confessions} item(s). Each is a modelling gap, a reflow2 gap, or a true design gap — never
  something to improvise past.</p>
  <ul>{confessions}</ul></div></section>
</main>
"""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("export", nargs="?", default=str(REPO / "docs/design/reflow2.json"))
    ap.add_argument("-o", "--out", default=str(REPO / "docs/design/views.html"))
    args = ap.parse_args()

    doc = json.loads(pathlib.Path(args.export).read_text())
    g = Graph(doc)
    projects = g.of_type("Project")
    title = g.name(projects[0]["node_id"]) if projects else "(no Project node)"
    if not projects:
        confess("a title for the page", "the graph holds no Project node")

    # The viewpoint a UAF/DoDAF reader asks for first: activities in order
    # (≈ OV-5b/OV-6). Projectable only if the graph can express an ordered
    # process — which is exactly what BL-37 found it cannot.
    if not g.of_type("Flow") and not g.edges_of("PART_OF_FLOW"):
        confess("an operational-flow viewpoint (≈ OV-5b/OV-6: activities in order)",
                "the graph holds no Flow nodes and no PART_OF_FLOW edges; `Flow` is fully "
                "specified in the schema and has no write side (BL-37), so no design can "
                "currently express one")
    if not any(g.nodes.get(e["from_id"], {}).get("node_type") == "Capability"
               and g.nodes.get(e["to_id"], {}).get("node_type") == "Capability"
               for e in g.edges_of("DEPENDS_ON")):
        confess("the functional dependency ordering between capabilities",
                "no Capability -DEPENDS_ON-> Capability edges exist, so the functional view "
                "shows a set, not a flow — and evaluate_allocation is inert for the same "
                "reason (a committed-model gap, on the record per sharpening.md §2)")

    functional = view_functional(g)
    structural = view_structural(g)
    traceability = view_traceability(g)

    dedup = sorted(set(CONFESSIONS))
    page = PAGE.format(
        title=html.escape(title),
        source=html.escape(str(pathlib.Path(args.export).name)),
        n_nodes=len(doc["nodes"]), n_edges=len(doc["edges"]),
        functional=functional, structural=structural, traceability=traceability,
        n_confessions=len(dedup),
        confessions="\n".join(f"<li>{html.escape(c)}</li>" for c in dedup) or "<li>nothing — every view was fully specified</li>",
    )
    pathlib.Path(args.out).write_text(page)
    print(f"wrote {args.out}")
    print(f"\n== confessions: {len(dedup)} ==")
    for c in dedup:
        print(f"  - {c}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
