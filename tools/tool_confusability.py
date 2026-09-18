#!/usr/bin/env python3
"""Tool confusability — the instrument behind the rank ratchet
(dec:idea-prune-the-tool-surface-to-an-orthogonal-essential-set, 2026-09-18).

An offline replica of find_tools' scorer over tools/toolsnaps (name,
description, parameters), validated against the live binary at 185/185 top-1
agreement on 2026-09-18. Run after `python3 tools/toolsnap.py --update`:

    python3 tools/tool_confusability.py              # the report
    python3 tools/tool_confusability.py --baseline   # rewrite the ratchet fixture

The report: which tools are NOT ranked first for their own corpus query and
who beats them, the mutually confusable pairs, and the crowders (tools that
sit in the most other tools' top 5). With `.reflow2/graph.usage.jsonl` present
it also says which served tools this project has never called — one project's
ledger, weak evidence, never a verdict (BL-155).

The Rust test beside the corpus is the authority; the replica can miss a tie-broken pair, so `--baseline` never removes a pair the test found — merge by hand.

Description words count once (has_word is boolean) and a name match scores
five times a description hit, so a tool beaten on a NAME match cannot be
rescued by words; it belongs in the baseline with that reason.
"""
import json, glob, math, re, sys

# Refusing stubs for renamed tools (service.rs DEPRECATED_TOOLS): find_tools
# never offers them, so the replica leaves them out too.
DEPRECATED = {"record_change", "manual_work_report"}

def load_tools(snapdir="tools/toolsnaps", overrides=None):
    tools = {}
    for f in glob.glob(snapdir + "/*.json"):
        d = json.load(open(f))
        name = d["name"]
        if name in DEPRECATED:
            continue
        desc = d.get("description") or ""
        params = list((d.get("inputSchema", {}) or {}).get("properties", {}).keys())
        tools[name] = (desc, params)
    if overrides:
        for k, v in overrides.items():
            if k in tools:
                tools[k] = (v, tools[k][1])
    return tools

def terms_of(query):
    return [t for t in re.split(r"[^0-9a-z_]", query.lower()) if t]

def has_word(hay, term):
    return term in re.split(r"[^0-9a-zA-Z]", hay)

def weights(terms, tools):
    n = len(tools)
    out = []
    for t in terms:
        df = sum(1 for name, (desc, _) in tools.items() if t in name.lower() or has_word(desc.lower(), t))
        out.append((t, max(math.log((n + 1) / (df + 1)), 0.0)))
    return out

def score(name, desc, params, weighted):
    nl, dl = name.lower(), desc.lower()
    s = 0.0
    for t, w in weighted:
        if nl == t: s += 8.0 * w
        elif t in nl: s += 5.0 * w
        elif any(p.startswith(t) for p in nl.split("_")): s += 1.5 * w
        if has_word(dl, t): s += 2.0 * w
        if any(has_word(p.lower(), t) for p in params): s += 1.0 * w
    return s / math.log(2.0 + len(dl) / 200.0)

def rank(query, tools, limit=5):
    w = weights(terms_of(query), tools)
    scored = [(score(n, d, p, w), n) for n, (d, p) in tools.items()]
    scored = [(s, n) for s, n in scored if s > 0]
    scored.sort(key=lambda x: (-x[0], x[1]))
    return scored[:limit]

def report(baseline=False):
    corpus = json.load(open("crates/reflow2-mcp/tests/fixtures/find_tools_corpus.json"))["queries"]
    tools = load_tools()
    top = {t: [n for _, n in rank(q, tools)] for t, q in sorted(corpus.items())}
    not_first = [t for t, l in top.items() if not l or l[0] != t]
    mutual = sorted({tuple(sorted((t, u))) for t, l in top.items() for u in l if u != t and u in corpus and t in top.get(u, [])})
    crowd = {}
    for t, l in top.items():
        for u in l:
            if u != t: crowd[u] = crowd.get(u, 0) + 1
    print(f"served with a query: {len(corpus)}  |  not ranked first for their own job: {len(not_first)}  |  mutual pairs: {len(mutual)}")
    for t in not_first:
        r = rank(corpus[t], tools)
        print(f"  {t:34s} first={r[0][1]} ({r[0][0]:.1f})  own rank={next((i+1 for i,(_,n) in enumerate(r) if n==t), '>5')}")
    print("crowders:", sorted(crowd.items(), key=lambda kv: -kv[1])[:10])
    try:
        calls = {}
        for line in open(".reflow2/graph.usage.jsonl"):
            try: rec = json.loads(line)
            except Exception: continue
            t = rec.get("tool")
            if t: calls[t] = calls.get(t, 0) + 1
        never = sorted(set(tools) - set(calls))
        print(f"never called in this project's ledger ({sum(calls.values())} calls): {len(never)} — weak evidence, one project")
    except FileNotFoundError:
        pass
    if baseline:
        path = "crates/reflow2-mcp/tests/fixtures/find_tools_rank_baseline.json"
        doc = json.load(open(path)); doc["not_first"] = not_first
        kept = [tuple(p) for p in doc.get("mutual", [])]
        doc["mutual"] = [list(m) for m in sorted(set(mutual) | set(kept))]
        json.dump(doc, open(path, "w"), indent=1); print("baseline rewritten:", path)

if __name__ == "__main__":
    if "--validate" not in sys.argv:
        report(baseline="--baseline" in sys.argv)
        sys.exit(0)
    corpus = json.load(open("crates/reflow2-mcp/tests/fixtures/find_tools_corpus.json"))["queries"]
    tools = load_tools()
    live = {}
    B = "/tmp/claude-1000/-home-ajs7-project-reflow2/db95cda5-9b0b-45f1-8b79-eed71eb2f89e/scratchpad/build/"
    for line in open(B + "confus.out"):
        if line.startswith("ok  find_tools: "):
            r = json.loads(line[len("ok  find_tools: "):])
            t = next(k for k, v in corpus.items() if v == r["query"])
            live[t] = [x["tool"] for x in r["items"]]
    agree1 = agree5 = 0
    for t, q in corpus.items():
        mine = [n for _, n in rank(q, tools)]
        if mine[:1] == live[t][:1]: agree1 += 1
        if mine == live[t]: agree5 += 1
    print(f"replica vs live: top-1 agree {agree1}/{len(corpus)}, exact top-5 agree {agree5}/{len(corpus)}")
