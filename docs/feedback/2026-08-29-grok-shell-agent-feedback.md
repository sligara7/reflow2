# reflow2 feedback (agent session notes)

## 2026-08-29 — tool results live only in `structuredContent`; this client forwards `content`

### What I was doing

A long design-and-build session: search, get node, add/revise Requirement/Decision/ChangeEvent/Capability, link artifacts, propagate, detect_gaps, loop_status, export.

### What I expected

To read each tool result in the same channel as refusals — enough to know whether a write landed, what `search_design` returned, and what `loop_status` owed.

### What happened

Almost every reflow2 tool reply was only this `content` (refusals and served prose excepted):

> This reply's payload is in `structuredContent`. reflow2 stopped duplicating it here because sending every reply twice was the difference between an answer a client could read and one it refused outright. IF THIS SENTENCE IS ALL YOU CAN SEE, your harness forwards only `content` and you cannot change that from where you are standing — so do this instead: the PROSE tools return their whole document in this field (`graph_report_markdown`, `get_instructions`, `get_skill`, `list_skills`), and REFUSALS arrive here in full, so a rejected call still tells you why; for anything else, call `export_graph` with a `path` and read the file you just wrote.

`.reflow2/graph.client.json` already records the mismatch:

- `protocol_has_structured_content`: true (negotiated MCP 2025-11-25 *contains* the field)
- `note`: that does **not** mean this client *reads* it; a client can negotiate a modern revision and still forward only `content`

Refusals (e.g. near-duplicate `add_requirement` / `add_decision`) and the four prose tools did arrive in `content`. Success of `add_*`, `get_node`, `search_design`, `propagate_change`, `detect_gaps`, `loop_status`, `link_artifact` did not.

Workaround used throughout: `export_graph` to a scratch path (not the committed export), then parse the JSON on disk. That is a full-graph dump (~890 nodes / ~1.7k edges / ~880 KB in this project) to answer “did this one write stick?”

### Minimal shape that reproduces it

1. Any client that negotiates MCP 2025-06-18+ but only shows the tool `content` field to the model.
2. Call `get_node` or `add_requirement` (or any non-prose, non-refusal tool).
3. The model sees only the structuredContent notice, not the payload.

No design content required. `graph.client.json` on this seat already names the client and the flag.

### Why it matters

- “Search before you add” cannot be followed from the search result; you export the whole graph instead.
- You cannot tell a successful write from a silent no-op without a second export.
- The standing export convention is “once between commits, onto the committed file.” Reading via export wants many dumps. Scratch paths avoid breaking the committed lineage, but every verification is a full export.
- Cost: tens of extra round-trips and full-graph parses per session. The notice is accurate; the loop still cannot see the graph.

### User suggestion

The user asked that this session’s reflow2 friction be written down for the reflow2 developer. Not prescribing a fix.

### Environment

```
reflow2 0.42.0, from `.reflow2/graph.client.json` (`reflow2_version`)
grok-shell-reflow2 1.0.13, MCP 2025-11-25
macOS (darwin arm64)
```

`protocol_has_structured_content`: true. Shared server `.reflow2/graph.server.json` `version`: 0.42.0.

---

## 2026-08-29 — near-duplicate guard matches across node types

### What I was doing

After a ChangeEvent was already recorded for a code change, capture the same idea as a Requirement (what must stay true). Later, capture a Decision that restated a Capability already in the graph (how vs what).

### What I expected

Near-duplicate refusal against the **same node type** (another Requirement, another Decision), or a hint that the existing node is a different type.

### What happened

1. `add_requirement` refused: the design already says something close — listed a **ChangeEvent** id. Fix was `distinct_from` naming that ChangeEvent, then the Requirement was created.
2. `add_decision` refused: close to a **Capability**. Same `distinct_from` dance.

The refusal text is clear and actionable. The surprise is cross-type matching: recording the durable must-be-true after the change event, or the choice after the capability, looks like a duplicate of a different kind of node.

### Minimal shape that reproduces it

1. `add_change_event` whose `summary` describes a user-visible behaviour.
2. `add_requirement` with a `statement` in the same words.
3. Refusal names the ChangeEvent, not another Requirement.

Invented nodes are enough; no user design needed.

### Why it matters

The capture loop after a build is: ChangeEvent (what we shipped) then Requirement/Decision (what must remain true / why we chose this). Those are supposed to be different records of the same idea. Cross-type near-duplicate adds an extra refused round-trip every time that loop is done honestly. `distinct_from` works; it is just the default path now.

### Environment

Same as the previous item: reflow2 0.42.0, grok-shell-reflow2 1.0.13, MCP 2025-11-25, macOS.
