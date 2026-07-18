# Changelog

Notable changes to Reflow 2.0. Format follows [Keep a Changelog](https://keepachangelog.com);
versions follow [semver](https://semver.org) — pre-1.0, so the minor number moves when the
graph model or the tool surface changes shape.

Two companion records, deliberately kept separate:

- **[docs/requirements-coverage.md](docs/requirements-coverage.md)** — *are we meeting the docs?*
  Every requirement → module → test, with an honest Met/Partial/Deferred status.
- **[docs/backlog.md](docs/backlog.md)** — *what should we do next?* Open work with its evidence
  and rough size.

This file is the third view: *what changed, and when*.

## [Unreleased]

### Added

- **The assembly hierarchy is reachable** (BL-2). `contain_component` nests one Component inside
  another, and `add_component` takes an optional `level`. Both were needed: `hierarchy_issues`
  had shipped as a read tool with no writer to feed it, returning `[]` for want of input rather
  than because a design was healthy. Exposing the containment alone would have been worse than
  nothing — every component defaults to `component`, so each nesting would have reported a false
  `level_mismatch`.

- **`set_requirement_status`** (BL-3) — `proposed` / `accepted` / `deferred` / `dropped` / `met`.
  The field was in the schema and read by DETECT, but nothing could write it, so a blind trial
  put the word "ASSUMED" in the statement text instead.

### Changed

- **A cross-community coupling is a signal, not a gap** (BL-6b). It no longer appears in
  `detect_gaps`; `graph_report` lists it under "Surprising couplings" as it already did, and
  `surprising_connections` returns it whole. Nothing was lost — it stopped demanding an answer.

  It fired on correct architecture. An `Interface` joins two clusters by construction, so
  modelling every contract as AGENTS.md instructs made the detector penalise each one: ten of
  thirteen gaps in one blind trial, and the other's verdict was *"that coupling **is** the
  product"*. Two earlier rounds of tightening had not fixed it. It was also never in the gap
  taxonomy — `docs/gap-surfacing.md` lists `orphan_node`, `dead_end`, `disconnected_cluster` and
  `single_point_of_failure` — so this restores the spec rather than departing from it.

- **`reviewed_gaps` reports acknowledgements that outlived their detector.** A trial had already
  acknowledged a coupling, and retiring the gap would have made that judgement vanish from the
  reviewed list while the `Decision` sat unreferenced in the graph. Such reviews are now listed
  with `retired` set and no candidate, because a list that shrinks for reasons the user cannot
  see is the dishonesty the open/reviewed split exists to prevent. `ReviewedGap` gains `gap_id`
  and `retired`; `gap` is now optional.

- **Artifact verification gaps read as being about files** (BL-6). `unverified_capability`
  reported Capabilities *and* Artifacts, titling the latter "Nothing verifies reading.py" —
  semantically right, legibly wrong, and independently noted by both blind trials. Artifacts now
  report under `unverified_artifact` with wording of their own. Detection is unchanged: proving a
  capability works still does not prove *this file* is what delivers it.

  The `unverified_capability` key is deliberately untouched. Gap ids hash the source string and
  acknowledgements are stored under the resulting id, so renaming it would have silently expired
  every capability acknowledgement with nothing to tell the user why. A test now pins both keys.

- **HEAL respects a dropped requirement.** DETECT skipped `dropped`/`met` requirements; HEAL's
  orphan scan did not. Marking one dropped therefore silenced half the system and left the other
  half nagging about the same node. Found while making `status` writable — the field was
  unreachable, so the inconsistency had never been reachable either.

- **`describe_schema`** — the design vocabulary is now discoverable instead of guessable. Ask
  with no arguments for every node and edge type, with `node_type` for one type's properties and
  the edges it can carry, or with `from` + `to` for the question an agent actually has: *what may
  connect a Release to a Component?* A blind trial brute-forced fourteen edge types against
  `create_edge` to answer that, then settled on `DEPENDS_ON` "because it was the one that
  validated".

  Matches distinguish an endpoint that **names** a type from one that accepts it through the `*`
  wildcard, and say so in words. Without that distinction the tool would have handed back
  `DEPENDS_ON` and reproduced the original mistake with better ergonomics — validating is not the
  same as meaning what you intended.

- **Rejected writes name the alternatives.** The trial's sharper complaint was that
  `Unknown edge type: PACKAGES` "tells me I'm wrong without telling me what's right" — and a
  discovery tool only helps an agent that already knows to call it. A failed `create_edge` now
  lists the edge types that accept those endpoints, each with its schema hint; a failed
  `create_node` lists the type's properties, or the known node types when the type itself is
  unknown. Still fails loud: the rejection is better, not softer.

- **`tools/reflow2_init.py`** — set up or update reflow2 in a project with one command. Installs
  the design environment only: agent instructions, skills, an MCP config with the binary path
  already resolved, and the graph directory. Creates no `src/`, build file or language choice —
  what kind of project it is comes out of the design, not a scaffold. Re-running updates in
  place, reports what changed, and never touches the design graph, your files, or a customised
  `.mcp.json`.

- **`AGENTS.md` is now the primary instruction file**, per the [agents.md](https://agents.md)
  convention; `CLAUDE.md` is a pointer. The build commands previously lived only in `CLAUDE.md`,
  which non-Claude agents never read.
- `COORD.md` claim board, `.gitattributes` union merge for the shared records, and pull-first in
  every entry point.

### Fixed

- **Every tool returns an object.** MCP defines `structuredContent` as an object, so seven
  list-returning tools — including `detect_gaps` — were malformed and rejected outright by
  spec-compliant clients. Lists now arrive as `{"count": n, "items": [...]}`. Found by a Grok
  trial; three home-grown test layers missed it because each was a client we wrote.

- **The kit's skills reach every agent, not just one** (BL-22). Skills were installed to
  `.grok/skills/` alone — the narrowest-reach of the four harnesses — so a project bootstrapped
  by `reflow2_init.py` and opened in Claude Code had an AGENTS.md naming seven skills the agent
  could not load. They now install to `.claude/skills/` (read by Claude Code, OpenCode **and**
  Copilot/VS Code) as well as `.grok/skills/`.

  This also explains a finding from the Grok trial that had looked like a subtle registration
  problem: opencode searches `.opencode/`, `.claude/` and `.agents/`, and the kit had written
  `.grok/`. The directory was never on the search path.

- **MCP config for every agent, merged rather than overwritten.** `reflow2_init.py` now writes
  `.mcp.json`, `opencode.json` and `.vscode/mcp.json` from one generator, since only Grok reads
  another tool's format. All three are merged into: `opencode.json` is that tool's *entire*
  config, and any project may already run other MCP servers — both must survive.

  Merging fixes a silent failure in the process. The installer previously bailed out whenever
  `.mcp.json` existed without a `reflow2` entry, so **any project already using one MCP server
  never got reflow2 installed at all** — while the run still reported success.

## [0.1.0] — 2026-07-18

The first release the design loop runs end to end on: a real project was designed and built
through it by an agent that had never seen the source, and by a second user on macOS via grok
build.

### Added

- **Interface layer** — `Interface` nodes with `PROVIDES`/`CONSUMES`, typed constructors, LLM
  extraction, MCP tools, and detection of contracts with a missing side
  (`unprovided_interface` / `unconsumed_interface`). Closes the failure the original Reflow never
  solved: a change made on one side of a service boundary leaving the other side stale.
  Pairing is keyed on node identity, so a shared name cannot mask a break.
- **Circular-dependency detection** — over a *directed* dependency view (`DEPENDS_ON` plus
  contracts collapsed through their `Interface`), reported per strongly-connected cluster rather
  than per elementary cycle. Critical, and propose-only: which edge to invert is a design
  decision.
- **As-built drift** (SP-6b) — an `Artifact.checksum` baseline and `reconcile_artifacts`, which
  compares caller-supplied observations and reports `missing_artifact` / `checksum_change` /
  `undocumented_addition` / `no_baseline`. Because `REALIZES` reads as Upstream, drift walks
  *back up* the golden thread to the Capability and Requirement behind the code. The core
  performs no I/O by design.
- **Write side for the types DETECT asks about** (WS-1..3) — `Verification` (+ `VERIFIES`,
  status), `Release`/`Environment`/`Resource` (+ `DEPLOYED_TO`, `REQUIRES_RESOURCE`), and
  `Decision` (+ `GOVERNED_BY`). Previously the system raised gaps demanding exactly these types
  and offered no typed way to answer them.
- **Gap review** — `acknowledge_gap` moves a judged gap into `reviewed_gaps` with the reason,
  stored as a real `Decision` so it outlives the session; `withdraw_gap_acknowledgement` puts it
  back. Reviews expire on their own when the situation changes, because a gap's id hashes its
  affected nodes.
- **`tools/reflow2_cli.py`** — one-shot command-line access to a graph, for shells, scripts and
  agents without an MCP connection.
- **`tools/smoke_mcp.py`** — end-to-end test of the shipped binary over stdio: the whole loop,
  plus persistence and cross-process determinism.
- **`docs/reflow-audit.md`** — every workflow and tool of the original Reflow, with an
  adopt / obsoleted / do-not-port verdict.
- **`where-am-i` skill** — read the graph back to the user in their own words. Added because a
  real user could not tell what the system had concluded.
- **`check-health` skill** — the HEAL step had MCP tools and no skill to invoke them, so eight
  defect categories were unreachable in practice.

### Fixed

- **Gap detection was not reproducible across processes.** `build_network` iterated a `HashSet`,
  whose hasher Rust seeds per process, so node insertion order — and with it Leiden's tie-breaks
  and every gap derived from community structure — differed between runs. Five runs on one
  unchanged graph gave 11, 12, 13 and 11 gaps. This silently undermined gap review: an accepted
  gap could return under a different id.
- **`unexpected_coupling` fired on every correctly-modelled contract.** An `Interface` joins its
  provider to its consumers and little else, so Leiden gave it a community of its own and each
  `PROVIDES` edge read as a "sole bridge" — the modelling discipline penalising itself.
  Contracts are now collapsed to the components they couple.
- **Community fragments were treated as parts of the design.** Both endpoints of a bridge must
  now sit in a community of ≥3 — the same non-trivial test `single_point_of_failure` already
  used.
- **`Fragment` and `DriftEvent` sat inside the topology** they were never part of, shifting
  communities and, for `DriftEvent`, eligible to be reported as a coupling in its own right.
- **`link_artifact` guidance was misleading** — it told the agent to confirm the
  `unrealized_capability` gap had closed, when the first `link_artifact` *switches that detector
  on* for every other capability, so the total rises. Correct behaviour, wrong instruction.

### Changed

- `detect_gaps` now returns **open** gaps only; reviewed ones move to `reviewed_gaps`. The open
  list is meant to mean *still needs attention* — a list that can never reach zero gets skimmed.
- The MCP surface grew from 34 to 52 tools.
- `getting-started/SETUP.md` gained a kickoff line and a stop/resume section, and states the
  one-agent-at-a-time constraint with the exact error text.

### Known limits

Recorded honestly rather than omitted; see [docs/backlog.md](docs/backlog.md) for the full list.

- **No schema discovery.** An agent needing an edge type has to guess; the blind trial
  brute-forced fourteen before settling on one *because it validated*.
- **`ingest` is not reachable over MCP** (SP-3b), so the multi-pass extraction pipeline — and
  with it provenance, fuzzy dedup and time-aware resolution — does not run in agent-native use.
- **`gap_to_prompt` output is not persisted**, so a question asked in one session is re-derived
  and re-asked in the next.
- **Component hierarchy cannot be built from the surface** — `contain_component` exists in core
  and is not an MCP tool.
- **`single_point_of_failure` responds to graph shape more than to risk** — the blind trial saw
  15 defects fall to 0 after adding two bookkeeping edges.
- Multi-project graph selection, concurrent multi-agent access, `EnvironmentRule`/`QualityGate`,
  and generative HEAL content all remain deliberate deferrals.

## [0.0.1] — before 2026-07-18

Initial core: the schema (26 node types / 52 edge types), `DesignGraph`, the coherence loop
(CHANGE / PROPAGATE / DETECT / HEAL), the temporal axis, INGEST, GENESIS, artifact linking, the
graph-analysis modules, and the `reflow2-mcp` server. See
[docs/requirements-coverage.md](docs/requirements-coverage.md) for what that covered.
