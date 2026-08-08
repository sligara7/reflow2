# CLAUDE.md

**Read [AGENTS.md](AGENTS.md).** It is the primary instruction file for this repo — commands,
architecture, and the rules for changing the project — and it follows the
[agents.md](https://agents.md) convention, so every agent working here reads the same thing.
Keeping the operational content there rather than duplicating it here is deliberate: only Claude
Code reads this file, and a build rule that half the collaborators never see is worse than no
rule at all.

Order:

1. **`git pull --rebase`**, then **`claim_report`** — the claim board lives in the graph now
   (`dec:coord-board-in-graph`, 2026-08-04). Claim what you take with `claim_region`.
   **[COORD.md](COORD.md)** keeps the handles, conventions and conflict doctrine.
2. **[AGENTS.md](AGENTS.md)** — commands, architecture, invariants.
3. **The design graph itself** — what is open and why: `loop_status` for what the loop owes,
   `detect_gaps` for the open questions, `search_design` to find a past finding by its words.
   `docs/backlog.md` was retired 2026-08-07 (`dec:backlog-is-retired`); its open rows are nodes now.

> **reflow2 is installed here.** The design graph is this project's memory — read [REFLOW2.md](REFLOW2.md) and consult it before writing or changing code.
