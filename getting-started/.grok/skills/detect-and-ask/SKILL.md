---
name: detect-and-ask
description: Use before building, and after capturing new intent, to find gaps in the design and ask the user about them. Runs reflow2's detect_gaps, phrases each gap as a plain question via the gap_to_prompt handshake, and writes the answers back — so decisions are explicit, not silently guessed.
---

# Detect gaps and ask the user

reflow2 surfaces the decisions a stateless agent would make silently. Turn them into questions.

1. Call `detect_gaps`. It returns a list of `GapCandidate`s ranked by severity — unsatisfied
   requirements, unallocated capabilities, phase-coverage holes, **contracts with a missing
   side** (`unprovided_interface` — something depends on it but nothing supplies it),
   decomposition problems (`missing_intermediate_level`), surprising couplings, quality drift.
   If empty, the design is coherent for now — proceed to build.
2. For each gap worth resolving now, run the **gap_to_prompt handshake**:
   a. Call `gap_to_prompt` with `gap` = the GapCandidate and `answers: []`. It returns
      `{ "status": "needs_llm", "prompts": [{ "id", "prompt", "expect_json" }] }`.
   b. Answer each prompt yourself, in context (you are the language model here).
   c. Call `gap_to_prompt` again with the **same** `gap` and
      `answers: [{ "id": <prompt id>, "text": <your answer> }]`. It returns
      `{ "status": "ok", "prompt": { "question", "rephrase_degraded" } }`.
   d. Ask the **user** that `question`. (If `rephrase_degraded` is true, the raw wording is
      used — still ask it.)
3. Take the user's answer and write it back into the graph. There is a typed tool for every gap
   the detector raises — use it rather than generic `create_node`:

   | Gap | Record the answer with |
   |---|---|
   | `unsatisfied_requirement` / `unallocated_capability` | `add_capability` + `satisfies`, `allocate` |
   | `unprovided_interface` | `add_interface` + `provides` / `consumes` |
   | `unrealized_capability` | `link_artifact` (see **link-artifacts**) |
   | `build_without_verification`, `unverified_capability` | `add_verification` + `verifies` |
   | `no_deploy_operate` | `add_release`, `add_environment`, `deploy_to`, `add_resource`, `require_resource` |
   | a choice between real alternatives | `add_decision` + `governed_by` — record *why*, not just what |

   Never discard the answer. If none of these fit, `create_node`/`create_edge` take any schema
   type, but prefer the typed tool: it supplies the required properties for you.
4. Re-run `detect_gaps` to confirm the gap is closed and nothing new opened.

Do this **before** writing code. A gap answered now is a requirement traced forever; a gap
guessed now is a silent decision that breaks later.
