# reflow2 feedback (agent session notes)

# Session 2026-08-27 (Grok Build TUI)

Agent: Grok (Grok Build TUI). OS: macOS.
Skills used: get_instructions, list_skills, revise-design, capture-session, report-friction, plus constructors (`add_decision`, `add_requirement`, `add_capability`, `add_change_event`, `record_change`, `export_graph`, `get_node`, `search_design`, `loop_status`, `detect_gaps`).

### reflow2 version (this session)

| Source | Version | Notes |
|---|---|---|
| Shared MCP server that served this session | **0.39.0** | `.reflow2/graph.server.json` (`version`, `pid` 37887, port 50793). `.reflow2/graph.meta.json` `reflow2_version` = `0.39.0`. Export stamp `reflow2_version` = `0.39.0`. |
| `reflow2-mcp` on PATH now | **0.39.0** | `/Users/…/.local/bin/reflow2-mcp` reports `reflow2-mcp 0.39.0`. Matches the daemon. |
| `.reflow2/kit-version.json` | 0.11.0 (`0ec473e`, 2026-07-25, `release-tarball`) | **Still stale.** Same trap as 2026-08-18. Do not cite this as the running version. |

Friction below was against the **0.39.0** shared server. PATH and daemon agreed.

Redacted: no product names, requirement text, or real node ids. Placeholders only (`cap:X`, `req:Y`, `dec:Z`).

---

## What I was doing

Finish a small UI-timing revision already in flight, then capture the decision (why alternatives lost) and export the graph. Compaction had dropped the earlier stretch; orientation had to come from the live graph.

## What worked

- **Near-match refusal on `add_decision` arrived in `content`.** Creating a Decision that restated an existing Requirement + Capability was refused with the two ids to put in `distinct_from`, and that refusal was readable. Second call with `distinct_from` succeeded. This is the intended path.
- **`export_graph` with `path` + `overwrite: true`** wrote the committed export. Could not see `{path, bytes, nodes, wrote}` in the tool reply (see friction 1), but the file on disk grew and contained the new Decision / ChangeEvent / Artifact.
- **Constructors merge on existing ids.** Revising a Requirement/Capability by calling `add_*` with the same id kept omitted fields (status, name).
- **Prose tools still return a document in `content`:** `get_instructions`, `get_skill`, `list_skills`. Those were the only reflow2 reads this session that the harness showed in full (aside from refusals).

## Friction

### 1. Structured tools return a stub in `content`; this harness never shows `structuredContent` (defect / client contract)

**Expected:** `get_node`, `search_design`, `loop_status`, `detect_gaps`, `record_change`, `export_graph`, and the other JSON tools either put the payload in `content` or the harness reads `structuredContent`.

**What happened:** Almost every structured reflow2 call returned only this string in `content`:

> This reply's payload is in `structuredContent`. reflow2 no longer duplicates it into `content`, because sending every reply twice was the difference between an answer a client could read and one it refused outright. If you are reading this string, your client is reading `content` for a tool that declares structured output: read `structuredContent` instead. Prose tools (graph_report_markdown and its siblings) are unaffected and still return their document here.

The Grok Build TUI agent sees `content` only. Session `mcp/call-*.json` dumps for these tools were not written with `structuredContent` either (the only full dumps on disk this session were `get_instructions`). So the payload was gone from both the model context and the recoverable files.

**Tools observed as stubs (not exhaustive):** `get_node`, `search_design`, `scan_nodes`, `sync_status`, `compare_designs`, `record_change`, `add_epoch`, `add_requirement`, `add_capability`, `add_change_event`, `add_decision` (success path), `detect_gaps`, `loop_status`, `export_graph`, `link_artifact`, `set_artifact_checksum`, `satisfies`, `governed_by`, `authored_by`, `set_decision_status`, `set_requirement_status`, `review_relations`, `pin_at_epoch`, `precedes`, `propagate_from`.

**What still had a usable `content` body:** `get_instructions`, `get_skill`, `list_skills`, and **refusals** (the near-match `add_decision` error).

**Workaround:** write `export_graph` to a file, then parse that JSON with Python. That is a post-hoc check, not a read. It cannot replace `search_design` before a create, or `loop_status` before finishing.

**Why it matters:** On 0.39.0 the capture → detect → ask loop is unreadable in this harness. The agent cannot confirm a write, cannot map the user's words to node ids without guessing, and cannot see whether anything is owed. Cost: many turns of log archaeology; risk of duplicate nodes; `loop_status` / `detect_gaps` after capture were cargo-cult calls.

**Shape:** Any tool that declares structured output, invoked from a client that only forwards `content`. Mature graph (~800 nodes) is not required; a single `get_node` is enough.

**Unsure:** whether this is a reflow2 contract change the TUI has not implemented, or a regression that dropped a fallback. The stub text is clearly intentional on the server side.

### 2. `get_instructions` still truncated by the harness (environment, still true on 0.39.0)

Same as 2026-08-18 item 4. First ~19.5 KB of ~26.8 KB, remainder on a JSON dump path. Recoverable, but the standing rules at the end of the file are the part that gets cut.

### 3. Kit version file still disagrees with the running server (documentation, still true)

Same as 2026-08-18 item 6. `report-friction` still says read `.reflow2/kit-version.json`. That file is 0.11.0. The session talked to 0.39.0. Accurate pin remains `graph.server.json` / graph metadata / `reflow2-mcp --version` when PATH matches the daemon.

### 4. No session-end nudge (product, still true)

Handshake still says this project has no session-end nudge. Export happened only because the user asked. Live graph had unexported work until that ask.

## Documentation gaps (not defects)

- The structured-output stub tells the *client* to read `structuredContent`. Nothing in `get_instructions` (the part that arrived) tells an agent in a `content`-only harness what to do instead — export-to-file is an inferred workaround, not a documented one.
- `add_decision` vs Requirement/Capability near-match is correct behaviour; the refusal text is good. Worth a line in capture-session / revise-design: a Decision that only restates a Requirement will be refused until `distinct_from` names those nodes.

## What I did not hit

Did not hit a silent failure (writes that reported success and did not land — verified after export). Did not hit the Verification create-then-link race from 2026-08-18. Did not use `gap_to_prompt`. `graph_report_markdown` unused this session (prose path; would likely have been the only readable orientation tool).

## Why it matters

0.39.0 stopped duplicating structured payloads into `content` so large replies would not be refused. In this harness that made the *ordinary* read path (`get_node`, `search_design`, `loop_status`) empty. Refusals still work. Prose skills still work. Everything that is “the graph answering a question” does not, unless the agent exports to disk and parses the file.

## File?

Not filed to `sligara7/reflow2` from this session. Item 1 looks new relative to the 2026-08-18 notes (those were on 0.31.0). Ask before opening an issue.
