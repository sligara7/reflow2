# Using reflow2 in VS Code when MCP is blocked — the `--call` door

A guide, kept current, for running reflow2 from VS Code's Copilot agent where an organisation's Copilot policy
blocks third-party MCP servers. **The running field log behind it is kept locally** in `docs/feedback/` (git-ignored,
`dec:field-reports-are-untracked-because-the-repository-is-public`); generic lessons from it are folded in here.

## Where reflow2 stands

**This route is supported on purpose (2026-10-01).** One reflow2 serves both routes: native MCP for an agent that
can call an MCP server, and the `--call` door for one that can only run a terminal command. MCP stays the main
route. Every tool, check and refusal is written once and reached both ways, so there is no separate build or
branch for the door.

What is planned for the door, in order:

1. a one-shot call never creates a design in a folder that names one on a server (limitation 16);
2. a writing call keeps the committed export current (limitation 4, idea 3);
3. `reflow2 read` / `reflow2 write`, so reads can be auto-approved (limitation 2, idea 4);
4. a full tool description on the CLI (limitations 5 and 6, idea 5);
5. `reflow2 init` installs this route for VS Code, `reflow2 update` refreshes it, and CI drives it the way an agent
   does (limitations 1, 10 and 11, ideas 1, 7, 8 and 11);
6. a batch of calls under one approval and one export (idea 6).

Not planned yet: joining a running shared server (idea 2), and ideas 12–15.

## The problem

With third-party MCP servers blocked, VS Code cannot register reflow2 through `.vscode/mcp.json` (the path
`reflow2 init` installs), so reflow2's tools never appear in the agent's tool list.

## What works today, with no change to reflow2

The binary's one-shot door, `--call`, runs any served tool once through the same server path a session uses (an
in-process client over an in-memory pipe), so refusals and replies read the same:

```bash
RUST_LOG=error reflow2-mcp --graph-path .reflow2/graph --call <tool> --args '<one JSON object>'
```

Exit 0 is the reply on stdout; 1 is a refusal on stderr; 2 is a reply the tool marked as an error. `--args -` reads
the object from stdin. `RUST_LOG=error` hides the per-call INFO line and the WARN line a refusal adds.

Teach the agent to use it with a **user-scope VS Code instructions file**
(`~/.config/Code/User/prompts/<name>.instructions.md`, `applyTo: '**'`) that says:

- act only when the workspace has `.reflow2/` or `REFLOW2.md`, and **not** where a `.reflow2.toml` names a design
  on a server (limitation 16);
- every "call `X`" in `REFLOW2.md`, a skill or a tool reply means `--call X`; never reimplement a tool or hand-edit
  `.reflow2/` or an export;
- discover tools with `--call find_tools --args '{"query":"…"}'`, skills with `--call list_skills` and
  `--call get_skill --args '{"name":"…"}'`;
- start with `--call loop_status`, then the `where-am-i` skill;
- handle the single writer and the missing automatic export (below);
- a Requirement or Decision status change still needs the person's explicit word.

### Measured

| Check | Result |
|---|---|
| `--call graph_report` on a scratch graph | about **0.2 s** per call, including opening the store |
| `--call find_tools` | ranked tools with **parameter names** and summaries |
| a write with a missing argument | exit 2; the message names the missing argument |
| `--call export_graph` to an existing path | refused until `"overwrite": true` |
| a read on a graph held by another session's `--serve-shared` server | answered from a **best-effort snapshot**, loud stderr warning |
| a write on that held graph | **refused**, nothing written; `--stop-shared` releases it |
| `--call upstream_status` from a hub | watches the hub's pinned designs, as under MCP |

### VS Code agent hooks (VS Code 1.138)

VS Code runs agent hooks: `chat.useHooks` is on by default (an organisation policy can turn it off with preview
features), reading `.github/hooks/*.json` (workspace) and `~/.copilot/hooks/*.json` (personal).
`chat.useClaudeHooks` (off by default) would also read `.claude/settings*.json`. A hook gets
`{tool_name, tool_input, tool_use_id}` on stdin, and a `PreToolUse` reply of
`hookSpecificOutput.permissionDecision: "deny"` blocks the call — verified live. This matters for reflow2 because
hooks can supply what the `--call` door lacks (ideas 1 and 12 below).

## Limitations

1. **Not a tool in Copilot's tool list.** Reachable only because an instructions file says so.
2. **Approval on every call.** One `chat.tools.terminal.autoApprove` regex cannot tell a read from a write.
3. **One writer, and `--call` cannot join a shared server.** A design held by another session is read-only from
   VS Code; `call_one_tool` opens the store directly.
4. **No automatic export.** `--export-to` write-through only starts in a long-lived server; `--call` exits first.
5. **No up-front schemas.** `find_tools` gives parameter names only; shape is learned one refusal at a time.
   Measured: five writes in one session each needed at least one refused attempt.
6. **Lessons attached to tools are invisible.** `tools/list` appends `steps` lessons to tool descriptions; a
   `--call` agent never reads `tools/list`. Skill lessons still arrive through `get_skill`.
7. **Skills are not native in a thin-installed project**, and slash commands don't exist in VS Code chat.
8. **Each call is a fresh session.** Session-scoped state (seats, claim liveness) does not carry between calls.
   *Not yet measured.*
9. **No loop nudges.** Nothing prompts `loop_status` at a boundary in VS Code.
10. **Telemetry can't name the harness.** Every call identifies as `reflow2-mcp --call`.
11. **The setup is one person's local file**; `reflow2 init` / `update` don't install or refresh it.
12. **Shell quoting.** Prose needs the `--args -` heredoc or a file.
13. **A hub has no member list.** The served `hub` skill says a local hub's list *is* the session's MCP config;
    under `--call` there is none, so the list must live elsewhere (a project file of store paths, for now).
14. **A hub can't see a member's unexported changes.** `upstream_status` compares the committed export, not the
    live store; without write-through the export routinely lags.
15. **A design can exist only in its store.** Nothing warns when a design has never been exported anywhere.
16. **A call in a moved design's folder creates a stray, empty design.** Where a `.reflow2.toml` names a design on
    a server, `--call` ignores that pointer and `--only-if-present` and creates a fresh local store with a new id.
    The agent then works an empty design, with no error. Measured 2026-10-01. Until it is fixed (planned step 1),
    don't run the door in such a folder.

### Tool friction found along the way

- `external_dependency` with a string where a list is expected → `failed to deserialize parameters: invalid type:
  string "…", expected a sequence`: names neither the tool nor the field.
- `external_dependency` replies with a dependency-declaration rendering rather than a node receipt.
- `find_tools` for "read one node by id with its properties and edges" does not return `get_node`.
- `add_epoch` refuses a missing `sequence` on every first epoch (clear message, one extra round trip).

## Ideas, in rough order of value for effort

1. **A served CLI-door harness in `reflow2 init`** (`--harness vscode-cli`): writes the instructions file into
   `.github/instructions/`, served from the binary so `reflow2 update` refreshes it, **plus VS Code hook files**
   (idea 12). Fixes 1, 9, 11.
2. **`--call` joins a running shared server** instead of refusing writes. Fixes 3.
3. **`--call … --export-to FILE`** (or the path from project config) after a successful write. Fixes 4 and 14.
4. **A read/write verb split** (`reflow2 read` refuses any tool not `read_only_hint`), so reads can be auto-approved
   safely. Fixes 2. Cheaper than it looks: `--call` already sorts every tool into read or write from that annotation,
   to decide whether a held design may be read from a snapshot.
5. **`--describe <tool>` / `--list-tools`** with full schemas and the lessons `tools/list` would append. Fixes 5, 6.
6. **`--call-batch`**: JSONL of calls, one store open, one approval, stop at the first refusal.
7. **Skill stubs in `.github/skills/`** that route to `get_skill`, so VS Code picks skills by description. Fixes 7.
8. **`REFLOW2_HARNESS=vscode`** under `--call`. Fixes 10.
9. **A VS Code extension registering Language Model Tools** backed by `--call` or a local process. Check with the
   organisation first: if the policy means "no unvetted agent tools", this is the wrong answer.
10. **An MCP registry allowlist**, if the organisation's policy is registry-based.
11. **A CI probe for the door** (`tools/test_call_door.py`, beside `test_opencode_plugin.py`).
12. **Ship VS Code hooks**: a `Stop` hook running `--call export_graph` and a `SessionStart` hook running
    `--call loop_status`. Fixes 4 and 9 without touching the binary.
13. **A hub address book**: hub pins that carry a store location, and a tool that resolves "where is member X's
    store on this machine". Fixes 13.
14. **Warn on a never-exported design** in `loop_status` / `where-am-i`. Fixes 15.
15. **Refusal shape**: the `refusal_speaks` gate should cover bare deserialize errors (the `external_dependency`
    case above).
