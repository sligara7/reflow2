# Upgrading to v0.65.0

🛑 **Upgrade every seat before anybody records a settled decision.** This release's schema stamp
moves: `ChangeEvent.change_type` gains the value `decision_settled`. No type or edge count changes
with it.

## What an older binary does

Since v0.59.0 the stamp records every enum vocabulary the schema declares, and a binary **refuses
to open a graph that stores a value it does not know**, naming the value and both versions. So a
v0.64.0 or earlier binary meeting a `decision_settled` ChangeEvent stops with an explanation
rather than reading past it. That is the guard working — and it is still a stopped seat.

The value is written only when somebody records that an open decision was settled, so an
un-upgraded seat keeps working until that happens once. After it happens, that seat cannot open
the design at all until it is upgraded.

| | v0.64.0 | v0.65.0 |
| --- | --- | --- |
| Node types | 28 | 28 |
| Edge types | 65 | 65 |
| `ChangeEvent.change_type` | 13 values | + **`decision_settled`** |

## What you do

1. Update every binary that opens the design — every machine, every harness. The installer's
   update does it; a hand-built copy needs rebuilding.
2. Open the graph. A v0.65.0 binary opens an older graph untouched: an absent enum record is not
   a claim, and the graph is re-stamped on the first write.
3. Nothing to migrate. No export, no import.

## If you cannot upgrade a seat yet

Do not record a settled decision with `change_type: decision_settled` until every seat can read
it. Any other change type is unaffected, and the older wording (`documentation`, `scope_change`)
still describes the same event.

## Also in this release, needing nothing from you

- **`--call <tool> --args '<json>'`** — a build script, a Makefile or a CI step can run one tool
  and read the reply as JSON, without an MCP session. A read-only tool still answers while a
  server holds the graph, from a best-effort snapshot that says so; a tool that writes refuses.
- **`find_skills`** — say the job in your own words and get the skills ranked, each with the name,
  the shortcut a person types, the one-line summary and who it is for.
- **`replace_text`** — move one sentence of a node's text without re-sending the whole field.
- **One status contract across the constructors** — every constructor of a type that carries a
  status now takes `status` and its description says what omitting it lands. `add_component`,
  `add_release`, `add_project` and `add_epoch` join the five that already did. Nothing changes for
  a call that omits it.
- **A node is named by the key the last tool used** — 25 tools whose parameter names a node by a
  typed key (`decision_id`, `capability_id`, `epoch_id`, …) also accept `id` and `node_id`; the
  published schema still teaches the typed key. `props` accepts `properties`.
- **Every served skill carries a line for a person and says who it is for** — `summary` and
  `audience` on `list_skills` and `get_skill`.
- **A project's first session leaves the adapter in place** — genesis and adopt write the pointer
  file when a project has none, or run the installer when its kit is present.
- **A contribution may be negative** — a reclaim or credit against a budget always rolled up
  correctly; now the tool, its parameter and the optimize skill say so.
- **A missing-field refusal names the fields that call lacked**, with the ones already passed set
  apart.
