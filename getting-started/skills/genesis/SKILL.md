---
name: genesis
description: Use at the very start of a project, or whenever the reflow2 design graph is empty, to bootstrap it from the user's opening brief. Scaffolds the Project, seeds the brief into Requirements and Capabilities, captures deployment/platform context, and runs the first gap-detection round. Run this before any other reflow2 work on a new project.
---

# GENESIS — bootstrap the design from a brief

Turn "here's my idea" into a seeded design graph the coherence loop can work with. Do this once,
at the start.

1. **Scaffold.** Call the `genesis` tool with `project_id`, `name`, and (if known) `domain`,
   `objective`, and `mode` (`flexible` = design evolves with the build; `rigid` = design is the
   source of truth). It creates the Project + a genesis Epoch and returns a `next_steps`
   checklist. If it reports `already_initialized: true`, the graph is already set up — skip to
   step 4 (detect_gaps).

2. **Seed the brief into P0/P1 — and stop there.** Extract the user's brief in context:
   - `add_requirement` for each thing that must be true (P0).
   - `add_capability` for each thing the system does (P1); link it with `satisfies` to the
     requirement(s) it fulfills.
   - `contains` each new node under the Project.
   - **Do NOT create Components (P2) yet.** Leaving structure unspecified is deliberate: the
     first DETECT round will surface `concept_without_design`, which is the right next question
     ("how should this be structured?"). Answer it *with the user*, not by guessing.

3. **Capture deployment/consumer context as Requirements.** This is easy to forget and expensive
   to discover late. Explicitly ask the user (or record what you already know) as
   `add_requirement` nodes: **target platform(s)** (e.g. macOS, Windows), **the driving agent**
   (e.g. grok build), **how it's invoked/run**, and **where it persists**. These are real
   requirements — they ripple into everything.

4. **Hand off to DETECT.** Run `detect_gaps`. For each gap, use the `gap_to_prompt` handshake to
   ask the user a plain question, and write their answers back into the graph. Now the normal
   loop (see AGENTS.md) takes over.

Genesis is guarded: calling the `genesis` tool again won't clobber an existing design. But seed
carefully the first time — this is the foundation the whole design grows from.
