# Working on Reflow 2.0

This repo already carries agent instructions in portable form. Read, in order:

0. **`git pull --rebase`** — before reading anything else. Two people work this repo through
   git; a stale checkout means claiming work someone already holds.
1. **[../COORD.md](../COORD.md)** — the claim board. Claim what you take, and follow its rules
   for resolving merge conflicts (short version: never resolve by discarding the other side).
2. **[../AGENTS.md](../AGENTS.md)** — the primary instruction file: commands, architecture, and
   the non-negotiable rules for changing this project.
3. **[../docs/backlog.md](../docs/backlog.md)** — what is open, and why.

A change is done only when `cargo test --workspace`, `cargo clippy -p reflow2-core
--no-default-features --all-targets`, `cargo fmt --check`, `python3 tools/validate_schema.py`
(after a schema edit) and `python3 tools/smoke_mcp.py` (after a tool-surface change) are clean.

For fast iteration use `cargo test -p reflow2-core --no-default-features`. The `-p` is
load-bearing: `reflow2-mcp` enables the `rocksdb` feature on the dependency edge, which
`--no-default-features` cannot switch off, so dropping it costs a ten-minute C++ build. The
done-gate above is `--workspace` on purpose — the core-only command cannot see the MCP surface at
all.
