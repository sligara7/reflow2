---
name: detect-and-ask
description: Use before building, and after capturing new intent, to find gaps in the design and ask the user about them. Runs reflow2's detect_gaps, phrases each gap as a plain question via the gap_to_prompt handshake, and writes the answers back — so decisions are explicit, not silently guessed.
---

# Detect gaps and ask the user

reflow2 surfaces the decisions a stateless agent would make silently. Turn them into questions.

1. Call `detect_gaps`. It returns a list of `GapCandidate`s (unallocated capabilities,
   unsatisfied requirements, phase-coverage holes, surprising couplings, …). If empty, the
   design is coherent for now — proceed to build.
2. For each gap worth resolving now, run the **gap_to_prompt handshake**:
   a. Call `gap_to_prompt` with `gap` = the GapCandidate and `answers: []`. It returns
      `{ "status": "needs_llm", "prompts": [{ "id", "prompt", "expect_json" }] }`.
   b. Answer each prompt yourself, in context (you are the language model here).
   c. Call `gap_to_prompt` again with the **same** `gap` and
      `answers: [{ "id": <prompt id>, "text": <your answer> }]`. It returns
      `{ "status": "ok", "prompt": { "question", "rephrase_degraded" } }`.
   d. Ask the **user** that `question`. (If `rephrase_degraded` is true, the raw wording is
      used — still ask it.)
3. Take the user's answer and write it back into the graph: a new/updated `add_requirement`, a
   node property via `create_node`, or a link (`satisfies`/`allocate`). Never discard it.
4. Re-run `detect_gaps` to confirm the gap is closed and nothing new opened.

Do this **before** writing code. A gap answered now is a requirement traced forever; a gap
guessed now is a silent decision that breaks later.
