---
name: link-artifacts
description: Use right after you create or substantially change a real source file (Unity C#, a spec, a doc), to register it in the reflow2 graph as an Artifact that REALIZES the capability it implements. Keeps as-designed vs as-built honest and closes the unrealized_capability gap.
---

# Link real files back to the design

You write the code; reflow2 tracks *which real file realizes which capability*. Register each
deliverable so the graph stays an honest as-built map — not just a plan.

1. **After building a file**, call `link_artifact` with:
   - `artifact_id` (stable, e.g. `art:ball-physics`) and `name` (e.g. `Ball.cs`),
   - `location` — the real path/URI (`src/Ball.cs`),
   - `artifact_type` — usually `code` (also `spec`, `document`, `diagram`, `model`),
   - `target_type` + `target_id` — the Capability (or Component) the file implements,
   - `completeness` — `stub` / `partial` / `complete` (default `complete`).

   This atomically creates the Artifact, a provenance Fragment (so it's clear the file was
   authored, not just planned), and the `REALIZES` edge. It fails loud if the target capability
   doesn't exist — create/So find it first.

2. **For partial work**, set `completeness: "stub"` or `"partial"` so the graph reflects reality;
   update it later when the file is done.

3. **Confirm the loop closed:** run `detect_gaps`. A capability with no realizing artifact shows
   as `unrealized_capability`; after `link_artifact` it should be gone for that capability. If it
   isn't, you linked the wrong target.

Bare `add_artifact` + `realizes` exist for cases where you don't need provenance recorded, but
prefer `link_artifact` — provenance is cheap and makes the as-built view trustworthy.
