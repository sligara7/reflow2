<!-- Preserved verbatim. Primary evidence, not a summary — do not tidy. -->

# Grok trial, 2026-07-18 — backyard weather station

**Method.** The same brief and consumer kit as the Claude blind trial, run through **opencode**
with **`openrouter/~x-ai/grok-latest`** — a different model *and* a different harness, reaching
reflow2 over real MCP rather than the CLI wrapper. Same instruction: do the design work the way
AGENTS.md says, then write down what fought you.

**Why it mattered.** It found the `structuredContent` array bug, which broke `detect_gaps`,
`scan_nodes` and `detect_defects` for any spec-compliant MCP client. Three home-grown test layers
had missed it because every one of them was a client we wrote, and all three shared the same
wrong assumption. Fixed in `f8bddeb`.

It also independently confirmed two known findings — artifact-level verification gap spam, and
`unexpected_coupling` firing on a legitimate interface (*"that coupling **is** the product"*).

---

# NOTES — what I did and what fought me

## What I did

1. **Genesis** — `reflow2_genesis` for `proj:backyard-weather` (flexible, mixed hardware/software).
2. **Seeded brief** into 13 Requirements + 9 Capabilities, `satisfies` + `contains` (no components yet, per genesis skill).
3. **Gap pass** via `graph_report` (see tooling note below). Phrased top gaps with `gap_to_prompt`.
4. **No user available** — wrote questions + assumptions to `QUESTIONS.md`. Proceeded on best judgement:
   - Three components: outdoor node, indoor collector, phone web UI
   - WiFi + buffered uplink for v0 software
   - Local web/PWA, SQLite, LAN-only
   - Rain onset = first tip after ≥30 min dry
5. **Recorded decisions** (`dec:three-part-architecture`, `dec:wifi-buffered-link`, `dec:local-web-ui`), allocated caps, wired interfaces with provides/consumes both sides.
6. **Implemented real code under `src/`** (stdlib Python, no third-party deps):
   - `src/collector/` — SQLite store, rain detector, alert hook, HTTP API + static + SSE
   - `src/web/` — live cards, canvas history chart, rain banner, PWA manifest
   - `src/outdoor/simulate.py` — fake node with outage buffer
   - `src/outdoor/firmware_sketch.cpp` — ESP32 stub matching the uplink JSON
   - `tests/test_collector.py` — 5 tests, all passing (`python3 -m unittest tests.test_collector -v`)
7. **Linked artifacts** with sha256 checksums; marked three verifications passing; planned release → home LAN.

### Run it

```bash
python3 -m collector --db data/weather.db --port 8765   # from src/ on PYTHONPATH, or:
PYTHONPATH=src python3 -m collector --port 8765
# other terminal:
python3 src/outdoor/simulate.py --interval 5 --rain-every 20
# phone on LAN: http://<host>:8765/
```

---

## Tooling / instructions that were confusing, missing, or fought me

### 1. MCP tools that return arrays are broken in this environment

`detect_gaps`, `scan_nodes`, and `detect_defects` all fail with:

```text
Invalid input: expected record, received array
path: structuredContent
```

So the **core DETECT loop tool does not work**. I had to bounce off `graph_report` / `graph_report_markdown`, which return objects and include `top_gaps` (truncated) and `defect_count` without listing defects. That is a serious agent-loop break: AGENTS.md says “run detect_gaps” as step 2 and after every build, and that tool is unusable here.

Whatever wires reflow2 MCP → opencode is likely wrapping array results incorrectly (Zod/schema expecting `Record` for `structuredContent`). Object tools (`genesis`, `add_*`, `link_artifact`, `graph_report`, `gap_to_prompt`, `acknowledge_gap`) work.

### 2. Skills are files, not auto-injected

AGENTS.md says “see the genesis skill” etc. Skills live under `.grok/skills/*/SKILL.md`. The available_skills list in the system prompt only listed `council` and `customize-opencode` — **not** the reflow2 skills. I had to discover and `Read` them manually. That’s fine once you know, but easy to miss; either register them as first-class skills or put the full loop only in AGENTS.md.

### 3. “Ask the user” vs “no user available”

AGENTS.md / detect-and-ask require asking the user before building. The trial prompt says write `QUESTIONS.md` and carry on. Those two instructions conflict; I followed the trial prompt. Without that override, a pedantic agent would hard-stop after genesis.

### 4. Genesis says “no Components yet” then DETECT surfaces structure

Clear once you’ve read the skill. Still awkward: the agent immediately has to invent structure (or wait), and “concept_without_design” is expected theater rather than a real unknown when the brief already names outdoor / indoor / phone. Worth special-casing: if the brief already names parts, seed them as Components in genesis, or mark that gap auto-closable from brief text.

### 5. `link_artifact` is picky about required fields

First calls failed with `missing field name` / `target_type` when args were incomplete. Error is fine; docs are okay. Small nit: two-step `add_artifact` + `realizes` is less crashy for batch scripting but skill correctly prefers `link_artifact`.

### 6. Verification gap spam after first artifact

Skill warns that gap count goes *up* after first `link_artifact` — true and loud. Also every individual *artifact* becomes an `unverified_capability`-style gap (`Nothing verifies "server.py"`). That is noisy for a small personal project: verifying every file is not the same as verifying a capability. Prefer only capability/requirement verification gaps, or collapse artifact-level ones under the capability they realize.

### 7. “Unexpected coupling” on the *one* real interface

Outdoor↔indoor via `ifc:reading-uplink` was flaggfed as surprising coupling (sole bridge across communities). That coupling **is the product**. Hard to know whether to “fix” architecture or acknowledge. I acknowledged. Detectors should treat an explicitly modeled Interface with both provides and consumes as intentional, not surprising.

### 8. Capability allocation vs phone UI

Phone UI is a Component with no Capability allocated to it — display is folded into `cap:serve-live-ui` on the collector. Graph is slightly dishonest (`allocate` only went collector-side). Could use a `cap:view-on-phone` allocated to `cmp:phone-ui`. Minor.

### 9. Platform/deploy context requirements felt forced

Genesis wants platform / driving agent / invoke / persist as Requirements. Putting “built via reflow2 agent loop” as a product Requirement immediately created an `unsatisfied_requirement` gap until I acknowledged it as meta. Process context should maybe be Project properties or a separate “context” node kind, not Requirements competing for SATISFIES.

### 10. Hardware caps only have firmware stubs

Solar power management and winter hardiness are real requirements with almost no software. Graph looks “realized” via stub firmware REALIZES edges, which overstates build status even with `completeness: stub`. Gaps do not strongly distinguish “stub file exists” from “capability shipped.”

### 11. Working directory / a git-less scratchpad

Project is under a trial path, not a git repo. Harmless, but no commit legible history. `src/` was empty and ready — good.

### 12. Tweaks

- SSE handler in `server.py` is a simple polling loop (fine for home use, not for scale).
- Phone background alerts without cloud remain unsolved product-wise; banner + optional `BW_NOTIFY_URL` hook only.
- No `requirements.txt` needed (stdlib only) — could add one empty or a note in README; I skipped README unless asked.

---

## Blunt summary

Reflow2 is useful as a durable design notebook once seeded; genesis + decisions + artifact checksums are the good parts. This trial was dominated by **`detect_gaps` being broken in the MCP path**, which is the tool the whole agent loop is supposed to orbit. Graph_report is a weak substitute. Heal/`detect_defects` same breakage. Fix array-shaped tool responses first or the design brain is half-deaf.
