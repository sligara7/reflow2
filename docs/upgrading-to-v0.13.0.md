# Upgrading to v0.13.0 — your design gets a name, and the loop gets its net

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: nothing breaks, nothing in your repository changes, and there is one new file
beside your graph.** Update the binary, re-run the installer once, carry on.

## What changes for you

### 1. Your design now has an identity of its own

Every reflow2 graph used to answer to one hardcoded name, which meant no design could tell another
design from itself — so linking two designs (`mirror_surface`) could never work outside of tests.

From v0.13.0 a design is given an id on first open, kept in **`<graph-path>.id.json`** beside the
store (machine-local, like the graph itself; it is covered by the gitignore the installer writes).
`design_identity` reads it, or renames the human-facing label — the id itself never moves, because
every stored key and every export names it.

**Existing graphs keep the name their data is already under.** A store that holds a design under the
old shared id adopts it, permanently. Nothing to migrate, no re-export, and every committed export
stays valid — `graph_id` is inside the export's content hash, so this mattered.

> If the identity file is unreadable, reflow2 **refuses to open** rather than guessing. Defaulting
> would open a *different* design at the same path and report nothing wrong.

### 2. Claims say who made them, and stop lying when that session ends

`CLAIMS` gains a `seat` property (additive — old claims simply have none). `claim_report` now
computes liveness by asking the operating system whether the claiming session still exists:

| | |
|---|---|
| `live` | somebody is working this |
| `gone` | that session exited — listed in `stale`, note kept, **excluded from overlaps** |
| `unknown` | another machine, or a claim from before seats existed — **still counts** as a possible collision |

A collision with nobody is not a collision. `unknown` is never read as free: taking work somebody is
actively doing is the expensive mistake.

### 3. The loop's session-end nudge is installed for you

Until now `reflow2_init.py` wired no hooks, so every consumer project ran with **no session-end
backstop** — the one trigger that fires when an agent has stopped calling tools. Re-running the
installer registers it in **`.claude/settings.local.json`** (machine-local, gitignored, because the
command carries an absolute path to your kit; a shared file would hand a collaborator a hook
pointing at a script they do not have).

Your own hooks and settings survive — it merges. A nudge you have repointed is left alone and
reported; `--force-hooks` if you want it repointed.

And whatever you do, reflow2 now **tells you what it found**: `loop_status` carries a `nudge` field
of `installed` / `absent` / `broken` / `unknown`, and when it is missing or broken the handshake
instructions say so. `broken` is the one to care about — a registered hook whose script is not
there, which fails silently at exactly the moment it is needed.

## What to run

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
python3 ~/.local/share/reflow2/kit/tools/reflow2_init.py <your-project> --check   # look first
python3 ~/.local/share/reflow2/kit/tools/reflow2_init.py <your-project>
```

The only change in your repository will be `.gitignore` gaining a line, if it does not already have
one. Everything else the installer writes is machine-local and ignored.

## New tools

| Tool | Purpose |
|---|---|
| `design_identity` | What this design is called; pass `label` to rename it (the id never changes) |

`claim_region` gains an optional `seat`; `loop_status` gains `nudge`. No other tool changed shape —
108 toolsnaps, the rest unmoved.

## If you are on v0.11.0 or earlier

Read [upgrading-to-v0.12.0.md](upgrading-to-v0.12.0.md) first: that release moved the skills and the
working instructions into the server, and it is the one that *removes* files from your project.
