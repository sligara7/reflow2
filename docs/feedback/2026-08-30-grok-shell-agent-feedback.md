# reflow2 feedback (agent session notes)

Redacted of the host project’s design content. For the reflow2 maintainer.

A dated report from the previous day lives at `.reflow2/friction-2026-08-29.md` (server 0.42.0). This file is the 2026-08-30 session on **0.43.0**: still-reproduced items, then new ones.

## Environment

From `.reflow2/graph.client.json` (handshake, not kit-version.json):

```
reflow2 0.43.0, from `.reflow2/graph.client.json` (`reflow2_version`)
grok-shell-reflow2 1.0.13, MCP 2025-11-25
macOS (darwin arm64)
```

Shared server `.reflow2/graph.server.json` `version`: 0.43.0.  
`protocol_has_structured_content`: true.

kit-version.json still says 0.11.0 — unused here, as the skill warns.

---

## Still broken on 0.43.0 (also in friction-2026-08-29.md)

### 1. Tool results live only in `structuredContent`; this client forwards `content`

Same as yesterday’s #1. Every non-prose, non-refusal tool still returns only:

> This reply's payload is in `structuredContent`. reflow2 stopped duplicating it here because sending every reply twice was the difference between an answer a client could read and one it refused outright. …

Hit this session on: `search_design`, `open_questions`, `loop_status`, `add_requirement`, `add_capability`, `add_change_event`, `add_epoch`, `set_requirement_status`, `set_epoch_status`, `contains`, `satisfies`, `allocate`, `authored_by`, `link_artifact`, `set_artifact_checksum`, `detect_gaps`, `export_graph` (even when writing a path — `wrote` / `unchanged` never visible), `pin_at_epoch` (success path).

Workaround unchanged: `export_graph` to a scratch path, then parse JSON on disk.

**Why it matters more after 0.43.0:** the standing instruction is “read the result back after a write that matters.” On this client that instruction is unimplementable except via a full-graph dump. `loop_status` — the cheap “what does the loop owe?” call — is itself unreadable.

Unsure whether this is a reflow2 defect or a grok-shell-reflow2 1.0.13 conformance gap. The server already detects it. The session still ran on the workaround.

### 2. Near-duplicate guard matches across node types

Same as yesterday’s #2. This session: `add_capability` refused as close to a **Requirement** and a **ChangeEvent** that already held the same idea (the must-be-true and the “we shipped it” event). `distinct_from: [req, chg]` then succeeded.

The capture order after a small feature is Requirement → Capability → ChangeEvent. Those three *should* say the same thing in different types. Cross-type matching makes `distinct_from` the default path, not the exception.

---

## New this session (0.43.0)

### 3. Parallel writes + `pin_at_epoch` / `set_artifact_checksum` → “Node not found”

#### What I was doing

One assistant turn issued several graph writes together: create a ChangeEvent, then pin it to an Epoch, then later `set_artifact_checksum` with `design_updated` and that ChangeEvent’s id.

#### What I expected

Either the tools in one turn run in an order that respects “create, then refer,” or a refusal that says “X does not exist yet — create it first” *in `content`*.

#### What happened

`pin_at_epoch` (ChangeEvent → DesignEpoch) failed in `content`:

> Node not found: ChangeEvent `chg:X`

`add_change_event` was in the **same parallel batch**. The pin raced the create. The ChangeEvent did exist on a later retry.

`set_artifact_checksum` was then called with `design_change_event_id` of that same ChangeEvent. The call’s result was only the structuredContent notice. The next `export_graph` still had the **old** artifact checksum. A second `set_artifact_checksum` after the ChangeEvent was known to exist, then another export, showed the new hash.

So: first checksum call almost certainly failed (ChangeEvent missing, or export raced), and the failure was invisible.

#### Minimal shape that reproduces it

1. In one parallel tool batch: `add_change_event` id=`chg:X` **and** `pin_at_epoch` node=`chg:X`.
2. Pin refuses: node not found.
3. Optionally in the same or next turn: `set_artifact_checksum` disposition=`design_updated` with `design_change_event_id=chg:X` before you have confirmed `chg:X` exists. Result unreadable; export still shows the previous checksum.

Invented nodes are enough. No host-project content required.

#### Why it matters

Harnesses emit independent tool calls in one turn. Graph writes have dependencies. There is no batch/transaction API, and success is not visible in `content`, so the agent cannot tell a no-op from a save. Cost: extra export + retry; risk of a committed export whose artifact hashes do not match disk.

A one-line note in the pin / checksum refusals already helps (“create the ChangeEvent first”). Serializing writes that name a just-created id would help more.

---

### 4. `get_instructions { section: "existing-design" }` — slug does not exist

#### What I was doing

Session start on a graph that already has a Project. The loop text says: on an existing design, orient first. I fetched `get_instructions` with `section: "existing-design"`.

#### What I expected

Either that slug, or the loop naming the real slug (`overview` / `the-loop`).

#### What happened

Refusal in `content` (good — refusals work):

> get_instructions: no section "existing-design". The sections are: overview, the-one-rule, …

#### Why it matters

Documentation gap, not a crash. I guessed a slug from the prose “existing design” rather than from the `sections` manifest. The refusal lists legal slugs, so recovery is one extra call. Label: **documentation**. If the loop keeps a special “existing design” path, giving it a section (or naming `the-loop` in that sentence) would stop the miss.

---

### 5. Skill name vs slash-command name (`where` vs `where-am-i`)

#### What I was doing

`get_skill` with `name: "where"` after the loop said to run the where-am-i skill. The slash command in the client skill list is `where`.

#### What happened

Refusal (again, good):

> no skill named 'where'. 'where' is the slash command; it is served as 'where-am-i'.

#### Why it matters

The error is exactly right. Friction is only that the client-facing name and the served skill name differ, and the loop uses both wordings. Label: **documentation**. Keep the refusal; consider accepting the slash-command alias.

---

### 6. “Read the result back” cannot be done without a full export

#### What I was doing

Follow the 0.43.0 habit: after a write that matters, fetch the node and confirm it.

#### What happened

`get_node` / `search_design` / `loop_status` all return the structuredContent notice. The only confirmation path is `export_graph` + parse. That conflicts with “export once between commits, onto the committed file.” Scratch-path dumps were used instead (`.reflow2/scratch-read.json`).

#### Why it matters

This is #1’s operational consequence, called out because 0.43.0’s “check anyway” section makes the unreadable-success problem louder, not quieter. A successful tool response is a claim the agent **cannot inspect**.

---

## What worked (keep these)

- **Refusals in `content`** are complete and actionable (unknown `get_instructions` section, unknown skill name, near-duplicate with `distinct_from` recipe, pin node-not-found).
- **Prose tools** (`get_instructions`, `get_skill`, `list_skills`, `graph_report_markdown`) return the document in `content`. Sectioned `get_instructions` avoids the ~27 KB cap.
- **`export_graph` with `path` + `overwrite`** is a reliable read/write. Lineage on the committed export is easy to break if you use that file as a scratch read — scratch path is the right workaround.
- **`distinct_from`** unblocks cross-type near-duplicates once you treat it as normal.

---

## Suggested maintainer priorities (from this seat, not prescriptions)

1. Make non-prose results visible to clients that only forward `content` (or fix grok-shell-reflow2 1.0.13 to read `structuredContent`). Everything else is downstream of this.
2. Do not treat Requirement / Capability / ChangeEvent / Decision with the same words as duplicates of each other; or name the *type* in the refusal so the agent knows it is looking at a different kind of node.
3. Dependent writes in one turn: either serialize, or fail in `content` with “created in this batch, retry pin.” Invisible checksum failure is the sharp edge.

I have not opened GitHub issues. Say if you want these filed on `sligara7/reflow2`.
