# Working on Reflow 2.0

This repo already carries agent instructions in portable form. Read, in order:

0. **`git pull --rebase`** — before reading anything else. Two people work this repo through
   git; a stale checkout means claiming work someone already holds.
1. **[../COORD.md](../COORD.md)** — the claim board. Claim what you take, and follow its rules
   for resolving merge conflicts (short version: never resolve by discarding the other side).
2. **[../AGENTS.md](../AGENTS.md)** — what Reflow 2.0 is, the mental model, and the
   non-negotiable rules for changing it.
3. **[../CLAUDE.md](../CLAUDE.md)** — commands and code-level architecture.
4. **[../docs/backlog.md](../docs/backlog.md)** — what is open, and why.

A change is done only when `cargo test --no-default-features`, `cargo clippy
--no-default-features --all-targets` and `cargo fmt --check` are clean.
