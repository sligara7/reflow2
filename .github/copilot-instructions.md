# Working on Reflow 2.0

This repo already carries agent instructions in portable form. Read, in order:

1. **[../COORD.md](../COORD.md)** — the claim board. Read it before starting work and claim what
   you take; two people work this repo with different agents.
2. **[../AGENTS.md](../AGENTS.md)** — what Reflow 2.0 is, the mental model, and the
   non-negotiable rules for changing it.
3. **[../CLAUDE.md](../CLAUDE.md)** — commands and code-level architecture.
4. **[../docs/backlog.md](../docs/backlog.md)** — what is open, and why.

A change is done only when `cargo test --no-default-features`, `cargo clippy
--no-default-features --all-targets` and `cargo fmt --check` are clean.
