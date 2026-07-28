# Upgrading to v0.18.0 — nothing is locked out, and nothing needs a rebuild

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: your design opens fine, no version stamp moved, no pin moved, and nobody is
locked out.** This release adds **no node or edge types** — the stamp holds at 28/56 — and the
dynograph-foundation pin stays at v0.12.0, so there is not even a slow first build this time.
Mixed versions keep working.

## The reason this release exists

**reflow2 now installs once per machine, and starting a project stops being a thing you do**
(`req:no-setup-per-project`, `dec:install-once-per-machine`). The installer registers the MCP
server at *user* scope with `--only-if-present`, ships ten slash commands to
`~/.claude/commands/`, wires the coherence-loop hook machine-wide, and puts a `reflow2` command
on `PATH`. After that, `cd anywhere && claude` and `/genesis` (or `/adopt`) is the whole story.

The safety mechanism is the **latent surface** (`--only-if-present`, `cap:latent-surface`): in a
directory where no design has been started, reflow2 serves exactly one tool
(`reflow2_start_design`) and **creates nothing** — no RocksDB store appears just because a
session opened there. This matters for v0.17.0 users specifically: v0.17.0's `install.sh` on
`main` already described this behaviour, but the v0.17.0 *assets* predate it, so the installer's
final registration step failed with a missing `reflow2_install.py` and the binary refused
`--only-if-present`. Re-running the installer against this release completes cleanly. If you
worked around the gap by hand (a wrapper script, a manual `claude mcp add`), remove the
workaround and re-run the installer — the real flag replaces it.

## What else you gained

- **`/genesis` and `/adopt` are discoverable doors.** The eight commands that shipped before
  were all mid-loop ones; a brand-new project had no way in.
- **A seam can now disagree out loud** (`seam_report`, from the v0.17.0-era work, finished
  here): its `design` parameter now *declares* it takes an object (BL-28), so an agent sees the
  contract in the schema rather than in a rejection.
- **`Interface.medium` no longer defaults to `REST`** — an unstated medium reads as
  `unspecified`, and two silences can no longer "agree" on a protocol neither chose. Defaults
  apply on create, never retroactively: interfaces written before this keep what they have, so
  an existing design may still claim a medium nobody chose — worth one look if seam analysis
  matters to you.

## Is your existing graph safe?

Yes, and it was checked rather than assumed: no schema type was added, no property became
required, and the storage format is untouched. A graph written by v0.17.0 (or v0.16.0) reads
identically under this binary, and a design written by this binary still opens under v0.17.0.
