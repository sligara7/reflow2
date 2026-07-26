# Upgrading to v0.15.0 — two more ways to say how you checked

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: update and carry on.** Nothing breaks, nothing in your repository changes, and
your existing design opens exactly as before. This document exists because the schema moved, and
this project's rule is that a schema change is explained before you take it — not because there is
anything for you to do.

## What changed

`Verification.method` gained two values: **`demonstration`** and **`observation`**.

| Method | What it means |
|---|---|
| `test` | Put the thing under specific inputs or stress and see how it behaves. |
| `analysis` | Prove it works with maths, models, or past data. |
| `inspection` | Examine the parts or the code, by hand or with tools, for flaws. |
| **`demonstration`** | **Show the feature working on a live example.** |
| `measurement` | Get exact numbers — size, weight, time, speed. |
| **`observation`** | **Watch the system run as fielded, without changing it.** |
| `review` | A read-through of the document or design. |
| `simulation` | Run a model of it rather than the thing itself. |

**Why these two.** Test, analysis, inspection and demonstration are the four canonical verification
methods in DoD and INCOSE practice, and reflow2 carried only three of them. So "we showed it
working" — which is how a great deal of acceptance actually gets closed — had to be recorded as a
`test`, which it is not. `observation` is the as-fielded method: watching a system run in its real
environment is neither inspecting an artifact for flaws nor running a contrived example, and
reflow2 has an as-fielded phase that previously had no way to name how something was checked there.

`review` and `simulation` are unchanged and still valid. Nothing was removed — removing a value
would strand every existing node that carries it.

## What you have to do

**Nothing.** Take the update.

Your existing Verifications keep whatever method they have. The default is still `test`. No tool
gained or lost a parameter; `add_verification` takes the same arguments it always did and simply
accepts two more values for `method`.

## Can an older reflow2 still read my graph?

**Yes — this was tested, not assumed.**

A binary built with the previous value set opens a graph containing `method: demonstration` and
reports the value faithfully. Two reasons, both structural:

- **Validation runs on write, never on read.** An older build has no opportunity to object to a
  value it does not recognise, because it never re-validates what it loads.
- **The version stamp counts node and edge *types*,** which are unchanged at 28 and 55. This is the
  check that refuses a graph outright (see [upgrading-to-v0.11.0.md](upgrading-to-v0.11.0.md) and
  BL-94); adding a property value does not touch it.

What an older build *cannot* do is **write** the new values — it will reject `demonstration` with a
validation error naming the values it knows. So a mixed-version team can read each other's designs
either way, and only the newer side can record the new methods. If that matters to you, upgrade
both sides; if it does not, nothing will go quietly wrong.

This is the same call, for the same reason, as `DriftEvent.drift_type`'s earlier growth — the
schema has carried a note to that effect since the as-fielded work landed.

## Also in this release

Everything from [CHANGELOG.md](../CHANGELOG.md)'s v0.15.0 section, of which one item is worth
knowing about even though it needs nothing from you:

**The degraded surface now follows the transport you asked for.** If the design graph is already
held by another process *and* you started reflow2 with `--http`, v0.14.0 served its one-tool
explanation over stdio and left nothing listening on the port — so every session pointed at that
URL got a refused connection, which looks exactly like reflow2 never having been configured. If you
are running the shared-server setup from [collaborating.md](collaborating.md), this is the release
you want.
