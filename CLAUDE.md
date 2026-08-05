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
3. **[docs/backlog.md](docs/backlog.md)** — what is open and why.

> **reflow2 is installed here.** The design graph is this project's memory — read [REFLOW2.md](REFLOW2.md) and consult it before writing or changing code.
