# Upgrading from v0.2.0 to v0.3.0

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

v0.3.0 is a bigger step than v0.2.0 was, and two of its changes have teeth: the schema grew an edge
type, and one tool's contract changed on purpose. Budget twenty minutes, most of it compile time.

**Your design graph is safe — with one new one-way door.** This build opens every existing graph.
But a graph *written by* v0.3.0 is **refused by older binaries**, loudly, naming what wrote it: the
schema went from 53 to 54 edge types (`INCLUDES`, the as-released view), and the `GraphStamp`
mechanism built in v0.2.0 now does the job it was built for. Practically: once any machine on a
shared design runs v0.3.0, every machine should.

---

## The four steps, in this order

Order matters. Done out of order you get **current instructions driving an old server** — and this
release finally lets you *see* that state instead of suffering it silently (step 4).

```bash
# 1. Get the new source
cd /path/to/reflow2
git pull --rebase

# 2. Rebuild the server BEFORE refreshing any project.
cargo build -p reflow2-mcp --release

# 3. Update each consumer project. Backs the design up first, then refreshes
#    AGENTS.md (or REFLOW2.md — see below), the skills, and the MCP configs.
python3 tools/reflow2_init.py /path/to/your-project

# 4. Restart your editor / agent session, then verify you are actually on the
#    new server: ask for graph_report and read `served_by`.
#      "served_by": { "reflow2_version": "0.3.0", ... }
#    An MCP session started before the rebuild keeps serving the old surface
#    with the old behaviour — restarting is what picks the new binary up.
```

## What will bite you if you skip reading this

**`set_artifact_checksum` now requires a `disposition` — your existing calls will be refused.**
Accepting a drifted file's new baseline is a two-sided decision: `design_holds` (the change carries
no design meaning; recorded as a dated claim) or `design_updated` (the design moved with it; pass
`design_change_event_id` from the `record_change` that updated it). The silent third option was how
a design erodes into fiction over N fix cycles while reporting zero gaps, so it no longer exists.
The refreshed **link-artifacts** skill teaches the new contract; if a call is refused, the error
says what to pass.

**If your project has its own `AGENTS.md`, the kit now installs as `REFLOW2.md` beside it.**
Earlier installers overwrote the project's own file; that is fixed, and step 3 will never touch a
file it did not author. Add one line to your `AGENTS.md` pointing at `REFLOW2.md` so the agent
reads both.

**Gap lists will look different, on purpose.** Anchored gaps now outrank phase nudges; new
detectors report a capability nothing asked for (`unmotivated_capability`), two components that may
be one thing (`possible_duplicate`), a check that is failing (`failing_verification`, 0.8 — the
highest severity in the system), a recorded divergence nobody has answered (`unresolved_drift`),
and a built component no release ships (`unreleased_component`). Verification coverage now counts
checks that **pass**, not checks that exist.

## What you gain

- **The confirmation ledger** (`confirmation_ledger`, and a rollup in `graph_report`): per built
  capability, whether it is *drifting*, *confirmed* (with the claim history — who accepted what,
  and whether the design moved), or *unexamined* — nobody has ever looked, which is no longer
  indistinguishable from fine.
- **The as-released view**: `release_includes` records what a release ships (checksums frozen at
  cut time), `release_report` answers *"does what we released match what we designed?"* —
  `built_capabilities_not_covered` is the diff. `pin_at_epoch` joins a release to its cut epoch.
- **`reflow2-mcp --import`**: restore a backup or load a design built elsewhere without speaking
  MCP — the sibling `--export` never had. Takes `-` for stdin.
- **`apply_heal` verifies proposals**: an operation HEAL would not itself propose for the graph as
  it stands is refused before any write. Merges report what they could not carry (`discarded`).
- The server now introduces itself correctly (it used to report the MCP library's version), and
  `graph_report.served_by` tells you which binary is actually answering — the check in step 4.

## If something goes wrong

`reflow2_init.py` exported your design to `.reflow2/backups/design-<utc>.json` before changing
anything. Restore it with the new binary:

```bash
reflow2-mcp --graph-path .reflow2/graph --import .reflow2/backups/design-<utc>.json
```

The graph is single-writer: if that command says another process holds it, close the editor session
using it and run again — the error now says exactly this.
