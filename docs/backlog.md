# Backlog — what's open, and why

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the map.

Three records, three questions:

| Record | Answers |
|---|---|
| [requirements-coverage.md](requirements-coverage.md) | *Are we meeting the docs?* — requirement → module → test |
| [../CHANGELOG.md](../CHANGELOG.md) | *What changed, and when?* |
| **this file** | *What should we do next, and on what evidence?* |

Items point at their source rather than restating it. Sizes are rough: **S** ≈ an afternoon,
**M** ≈ a day or two, **L** ≈ a real project.

Each item has a stable id (**BL-n**). Claim one on the board in
[../COORD.md](../COORD.md) before starting, so two people don't build the same thing.

## Evidence base

Three independent sources, which is why several items appear on more than one list:

- **Blind trial, 2026-07-18** — an agent with no knowledge of reflow2's source designed and
  built a weather station through the consumer kit. Its friction log is the single richest
  source of findings we have; quotes below are its words.
- **Grok via opencode, 2026-07-18** — a second blind trial, different model *and* harness. Found
  the `structuredContent` array bug that three home-grown test layers all missed, because every
  one of them was a client we wrote. Notes: [trials/](trials/).
- **macOS / grok build, 2026-07-18** — first real external user. Reached the design loop and
  asked for things the trial agent could not (it had no continuity across sessions to miss).
- **[reflow-audit.md](reflow-audit.md)** — the original Reflow's workflows and tools, with
  adopt/obsolete verdicts.

## Next up

| ID | Item | Why | Size |
|---|---|---|---|
| **BL-1** | **Schema discovery tool** | Trial agent brute-forced 14 edge-type guesses, then used `DEPENDS_ON` *"because it was the one that validated, which is precisely the kind of silent accommodation this project says it's against."* Its bogus `Release → Component` edges then polluted coupling output. | S |
| **BL-2** | **Expose `contain_component`** | Exists in core, not on the surface, so an assembly hierarchy can't be modelled — and `hierarchy_issues` returns `[]` because it never has a hierarchy to check. | S |
| **BL-3** | **`Requirement.status` reachable** | Already in the schema, defaulting to `proposed`. The trial agent wanted to mark requirements provisional and wrote "ASSUMED" into the statement text instead. | S |
| **BL-4** | **Persist asked questions** | `gap_to_prompt` output evaporates, so the next session re-derives and re-asks. The trial agent's own framing: *"the stateless-agent problem reflow2 is supposed to solve."* Same gap the external user hit as "how do I pause and resume". | M |
| **BL-5** | **Re-examine `single_point_of_failure`** | *"All 15 defects vanished at once when I added two bookkeeping edges. Nothing about actual fragility changed."* Use that case as the test. | M |
| **BL-6b** | **Demote `unexpected_coupling` from a gap to a report-only signal** | Three independent reports now — both blind trials and the coupling work itself. Grok: *"that coupling **is** the product"* on the one real interface in a 3-component design. Community structure is not meaningful at this scale, so it should inform rather than demand attention. | S |
| **BL-6** | **Rename/split `unverified_capability`** | Fires per-Artifact despite the name, producing gaps titled "Nothing verifies reading.py". Semantically right, legibly wrong. | S |

## Bigger threads

**BL-15 · Project bootstrap and kit updates** — *from the external user, 2026-07-18.* "You should
be able to launch a project from reflow, which bootstraps everything into a new repo... And maybe
it adds a script for pulling in releases. You won't know up front what project type it is though."

Two problems, and his caveat is the design.

*Bootstrap.* Today the kit installs by three hand-run `cp`s, one of which needs the binary path
edited in, plus a hidden `.grok/` that `cp *` misses. That should be one command.

*The caveat is the product, not an obstacle.* You don't know the project type up front **because
that is a design decision the loop is supposed to make.** So bootstrap only what is type-neutral
— AGENTS.md, skills, MCP config, `.reflow2/`, `.gitignore`, a brief template — and deliberately
scaffold no `src/` layout, build file, or language choice. Those come *out* of the design, and
both blind trials produced exactly that: a structure that fitted what had been designed. A
scaffold that guessed would commit a design decision before the design existed.

*Updates are the sharper half, and currently absent.* The kit is copied, so a consumer's copy
freezes at install time — the first external user's copy is already stale by a day of skill
fixes and nothing tells him. Text (AGENTS.md + skills) is easy to refresh; the binary needs a
~10-minute RocksDB build, so it wants either published release binaries or a pinned-version
check. Bears directly on the embedded-vs-service fork: a service would make this disappear.

Size: **M** for bootstrap, **M–L** for updates depending on the release story.



| ID | Item | Why | Size |
|---|---|---|---|
| **BL-7** | **`ingest` over MCP** (SP-3b) | The multi-pass extraction pipeline is unreachable agent-native, so provenance, fuzzy dedup and time-aware resolution never run. Needs a transactional prepare pass. Closely tied to #4 and to session continuity. | L |
| **BL-8** | **Session state / multi-project** | Select a graph per project; give agents memory across sessions. Core already supports `graph_id`; nothing exposes it. See the memory note and [reflow-audit.md](reflow-audit.md). | L |
| **BL-9** | **As-fielded view** | Audit item 2, and it needs **no new schema** — `operate.yaml` is fully defined and now writable (WS-2). `reconcile_deployment` as a sibling of `reconcile_artifacts`. Guard against the library-plugin false positive the audit flags. | M |
| **BL-10** | **Root-cause classification of drift** | `drift.rs` detects divergence with no notion of *why*, so no notion of which side is wrong. Reflow's seven-category taxonomy ends in a decision rule. Needs a scalar coherence score to gate on. | M |
| **BL-11** | **Path-cumulative budget analysis** | Three independent reflow tools reached for it. PROPAGATE walks impact but never accumulates a quantity along source→sink paths — the classic SE budget rollup (latency, mass, power, cost). | M |
| **BL-12** | **Concurrent multi-agent / team access** | Deliberate future effort. RocksDB is single-writer; this is the strongest argument for the service shape. | L |
| **BL-13** | **Advanced testing tiers** | Comprehension (partly answered by the blind trial), scale (all fixtures are 3–10 nodes), messy input, longitudinal. | M |
| **BL-14** | **`tools/` sweep follow-ups** | The remaining adopt-list items in [reflow-audit.md](reflow-audit.md): typed gap resolution strategies, abstraction-gap → strategy, document round-trip, MCP resources/prompts. | M |

## Deliberate deferrals

Not gaps — decisions, recorded so they aren't rediscovered as bugs.

- **WS-4 `EnvironmentRule` / WS-5 `QualityGate`** — nothing reads or asks for either type, so a
  constructor would build the mirror image of the problem WS-1..3 fixed. Each lands with its
  detector.
- **Real LLM provider backends** — unnecessary agent-native; the ambient agent is the LLM.
- **`EmbeddingBackend`** — semantic dedup and retrieval. The audit has prior art on shape
  (local MiniLM/384-dim, normalized inner product, hash-gated rebuild) and one caution: retrieval
  thresholds are not identity thresholds.
- **Generative HEAL content** — proposals stay review-gated stubs.
- **Bayesian architecture optimization** — assessed and dropped; see the audit's do-not-port list.

## Recurring lesson

Seven times in one session a capability existed in core and was unreachable or unadvertised on
the surface: `Interface`, HEAL's skill, the `Verification`/operate write side,
`contain_component`, `graph_id`, `Requirement.status`, and `graph_report` as an answer to
"where am I". Items 1–3 above are all the same shape.

Before building something new, the higher-yield question is usually: **what does the core
already do that nothing can reach?**
