# Upgrading from an early v0.1.0 install to v0.2.0

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and reading order.

If you set reflow2 up before 2026-07-18, this is the whole procedure. It takes about fifteen
minutes, most of it waiting for a compile.

**Your design graph is safe.** The schema lives in the binary; the graph directory holds only your
nodes and edges. Nothing here rewrites your design, and step 3 takes a backup before it touches
anything.

---

## The three steps, in this order

Order matters. Doing them out of order leaves your project with **current instructions driving an
old server** — the tools behave differently from what the instructions describe, and nothing says
why.

```bash
# 1. Get the new source
cd /path/to/reflow2
git pull --rebase

# 2. Rebuild the server. Ten minutes or so the first time (it compiles RocksDB),
#    then cached. Do this BEFORE step 3.
cargo build -p reflow2-mcp --release

# 3. Update your project. Backs your design up first, then refreshes everything.
python3 tools/reflow2_init.py /path/to/your-project
```

That is it. Re-running step 3 is safe any time — it never touches your design graph, your own
files, or an MCP config you have customised.

### Check it worked

```bash
python3 tools/reflow2_init.py /path/to/your-project --check
```

You want to see `upstream: current`, `kit is up to date`, and **no** `binary: STALE` warning. If
the binary is stale, you did step 3 before step 2 — just re-run them in order.

---

## What you will notice

**New files in your project.** v0.2.0 installs skills into `.claude/skills/` as well as
`.grok/skills/`, and writes `opencode.json` and `.vscode/mcp.json` alongside `.mcp.json`.

That is a fix, not clutter: every agent looks in a different place, and the old kit installed only
`.grok/`. If you ever opened the project in Claude Code or VS Code, the skills your `AGENTS.md`
referred to **could not be loaded at all**. Now they can.

**A backup appears** at `.reflow2/backups/design-<timestamp>.json` — a complete, readable export
of your design, taken before the update. Keep it or delete it; they are small, and they diff
cleanly if you want to commit them.

**A stamp appears** at `.reflow2/graph.meta.json`, recording which reflow2 wrote your graph. From
now on, opening a graph with a mismatched build says so instead of behaving oddly in silence.

**One first-run message** you can ignore:

```
reflow2: this graph carried no version stamp; recording reflow2 0.2.0 (27 node types, 53 edge types) from now on
```

That is your existing graph being stamped for the first time. It appears once.

---

## What is actually new, in one line each

| | |
|---|---|
| `describe_schema` | ask what node and edge types exist, and what may connect two of them, instead of guessing |
| `open_questions` | questions you were asked in an earlier session, in the wording you saw — the agent should follow up rather than re-ask |
| `set_requirement_status` | mark a requirement `proposed` / `accepted` / `deferred` / `dropped` / `met` instead of writing "ASSUMED" into its text |
| `contain_component` + `level` | model an assembly: a system contains subsystems contains parts |
| `export_graph` / `import_graph` | your whole design as one portable file — backup, move machines, migrate across an upgrade |
| **far fewer gaps** | three detectors were flooding the list; on reflow2's own design it went from 25 gaps to 1 |
| **report-friction** | tell your agent reflow2 got in the way, and it writes a report you can send |

Full detail is in [CHANGELOG.md](../CHANGELOG.md).

---

## Then what

Read [v0.2.0 — what we don't know yet](v0.2.0-what-we-dont-know.md). It is four questions, and
your use is the only way to answer them.

**This version is deliberately frozen for a stretch.** The point is prolonged real use rather than
more changes, so if something fights you, that is the thing worth writing down — not a reason to
wait for a fix.

---

## If something goes wrong

**The server will not start, and says a graph "knows more of the schema".** Your graph was
written by a *newer* reflow2 than the one you are running — you rebuilt from an older checkout.
Re-run steps 1 and 2.

**A tool behaves differently from what `AGENTS.md` says.** Almost always step 3 ran before step 2.
Check with `--check`; look for `binary: STALE`.

**Anything else.** Ask your agent to run the **report-friction** skill. The repository is private,
so filing an issue will likely fail — that is expected, and the skill falls back to writing
`reflow2-friction-<date>.md` in your project. Send that on.
