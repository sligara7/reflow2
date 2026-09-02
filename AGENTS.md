# AGENTS.md — orientation for AI agents working **on** Reflow 2.0

> ## Which are you doing?
>
> | You are… | Read | You are in the wrong place if… |
> |---|---|---|
> | **Building or improving reflow2 itself** — changing the Rust, the schema, the tool surface, the skills | **this file**, then [docs/sharpening.md](docs/sharpening.md) | — |
> | **Using reflow2 to design your own project** | **[getting-started/AGENTS.md](getting-started/AGENTS.md)** — and you should not be reading it here. Run `python3 tools/reflow2_init.py /path/to/your/project`, which installs it into your repo along with the skills and MCP config | you are following the build commands below. They build *reflow2*; they have nothing to do with your project, and reflow2 deliberately installs no `src/` layout or build file, because your project type is a design *output* |
>
> Everything after this line is for the first case. The consumer kit lives in
> [getting-started/](getting-started/) and is a separate document with a separate audience — it
> never mentions cargo, and this file never mentions your design.
>
> **This is the primary instruction file for this repo** — it follows the
> [agents.md](https://agents.md) convention, so every agent reads the same thing.
>
> **Before you start:** run **`git pull --rebase`**, then run **`claim_report`** and claim what you
> take with **`claim_region`** — the board moved into the graph on 2026-08-04
> (`dec:coord-board-in-graph`), so claims travel in `docs/design/reflow2.json` and a graph you
> haven't pulled is out of date. **[COORD.md](COORD.md)** keeps the handles, the conventions, and
> resolving merge conflicts without discarding anyone's work.
> **The graph** has what is open and why: the graph — `loop_status` for what the loop owes, `detect_gaps` for the open questions, `search_design` to find a past finding by its words. `docs/backlog.md` was retired 2026-08-07 (`dec:backlog-is-retired`); its open rows are nodes now.

Read this first. It tells you what this project is, how it's organized, and the rules to
follow so your changes stay coherent with the design.

> **If you are here to improve reflow2, read [docs/sharpening.md](docs/sharpening.md) before you
> pick up an item.** It is the standing method for finding this project's own gaps — where findings
> actually come from, how much to trust one, and the specific way this work goes wrong (shaping the
> model until the tool goes quiet, then reporting the tool is fine). It also names the
> instruments that measure whether the loop works, and their current baselines. **reflow2's silence
> is never evidence that reflow2 is healthy.**

**Per-crate files exist and the closest one wins**, per the convention. If you are editing inside
a crate, read its file too — the build commands genuinely differ, and getting that wrong costs
ten minutes of C++ compile:

| Editing | Also read | Why it differs |
|---|---|---|
| `crates/reflow2-core/**` | [its AGENTS.md](crates/reflow2-core/AGENTS.md) | sub-second test path; core-only invariants |
| `crates/reflow2-mcp/**` | [its AGENTS.md](crates/reflow2-mcp/AGENTS.md) | pays the RocksDB build; needs the smoke test |

## Commands

Everything runs from the repo root. The core crate is `crates/reflow2-core`.

```bash
# Build / test — for dev iteration ALWAYS scope to the core with -p AND pass
# --no-default-features: that combination uses the in-memory backend and skips
# the RocksDB C++ compile (~10 min). Runs in well under a second.
#
# -p reflow2-core is load-bearing, not tidiness. Without it the workspace also
# builds reflow2-mcp, which depends on reflow2-core with `features = ["rocksdb"]`
# — an explicitly-enabled feature on a dependency edge, which --no-default-features
# cannot switch off. Drop the -p and you get the C++ build you were avoiding.
cargo test -p reflow2-core --no-default-features

cargo test -p reflow2-core --no-default-features --test heal          # one integration-test file
cargo test -p reflow2-core --no-default-features golden_thread_round_trips  # one test by name
cargo test -p reflow2-core --no-default-features --lib                # unit + doctests only

cargo clippy -p reflow2-core --no-default-features --all-targets      # keep clippy-clean
cargo fmt                                               # and fmt-clean (cargo fmt --check in CI)

# The full workspace, including the MCP surface. Pays the RocksDB compile once,
# then it is cached. Run before pushing — the core-only gate cannot see
# reflow2-mcp, where the tool surface and its tests live.
#
# This is the heaviest command in the file. On a machine with ~8-12 GB of RAM it
# is the one that gets your terminal killed — see "Building with limited RAM"
# below, and prefer the memory-capped form there.
cargo test --workspace

# Schema validation (Python, no Rust toolchain needed). Must print "OK" after any
# schema/*.yaml edit. Needs PyYAML; use whatever python3 has it.
python3 tools/validate_schema.py

# The installer's own suite (stdlib only, hermetic — no network, no binary).
# Run after any tools/reflow2_init.py change; it is what lets the self-model
# call cap:kit verified.
python3 tools/test_init.py

# Load a design into a graph without speaking MCP — the sibling of --export.
# Upsert, so it layers onto whatever is there. Takes `-` for stdin, so an export
# on one machine pipes into an import on another. The graph is single-writer:
# stop any running MCP server first, and the error says so if you forget.
./target/debug/reflow2-mcp --graph-path .reflow2/graph --import docs/design/reflow2.json

# reflow2's own functional design, as a reflow2 graph (~215 nodes). The export at
# docs/design/reflow2.json is the durable record — .reflow2/ is gitignored, so
# the JSON is what gets reviewed and diffed. Rebuild it after a design change;
# --analyse-only re-imports the committed export and re-runs the analysis.
python3 tools/build_design_graph.py
python3 tools/build_design_graph.py --analyse-only

# Phase-coverage trial — does the design still carry weight after P2? Seeds a
# realistic graph, injects the divergences P3/P4/P5 are each supposed to catch,
# and scores whether the graph noticed. 13/13 as of 2026-07-19 (BL-30 and BL-9
# closed the last two probes) — fully green and exits 0, so it now works as a
# regression gate. It remains the standing measurement for the failure that
# sank the original reflow — the early phases going well and the later ones
# proceeding as if they hadn't.
python3 tools/phase_trial.py

# Seat-identity probe — does ONE client keep ONE seat? Per transport, per
# protocol version, AND is a claim with no seat refused exactly where the session
# cannot supply one. test_shared_sessions proves the other half (two clients get
# two DIFFERENT seats) and nothing proved the complement, which is why the whole
# suite stayed green through the rmcp v3 upgrade while seat identity was already
# broken: every test client here negotiates a pre-2026-07-28 version. Green since
# 2026-07-30 (mint_seat + the claim_region guard) and now a CI gate in the `full`
# job — req:seat-identity-survives-stateless-mcp, dec:stateless-seat-handle.
python3 tools/stateless_seat_probe.py

# Erosion trial — the sharper question. Not "did a file change?" but: after N
# rounds of test-fails/fix-code/accept, does the design still describe what
# shipped? 7/8 (was 2/7): the one remaining miss is the semantic
# description-vs-history judgement, deliberately not built (the ledger
# reports; the human judges). Non-zero exit by design until that decision
# changes.
python3 tools/erosion_trial.py

# The same cycle done right — the constructive counterpart. Proves designed ==
# released is reachable today with axis-Z discipline (original intent survives in
# a Snapshot), and that reflow2 gives the SAME verdict for the coherent graph and
# the eroded one. That gap is BL-35.
python3 tools/coherent_erosion_trial.py

# Project the design as viewpoints — functional / operational-flow / structural /
# traceability / as-released / decisions, DoDAF-informed (docs/viewpoints.md).
# Pure projections: anything a view needs that the graph cannot supply is
# CONFESSED, and every confession is a finding (BL-40). Also a probe.
python3 tools/render_views.py                        # the committed design export
python3 tools/render_views.py --graph-path <dir>     # a live graph (stop the server first)

# Process-model probe — can reflow2 hold its own operating model, a Flow whose
# feedback loops are the subject? Started as BL-37's friction log (no Flow write
# side, roles lost, cycles invisible, product-shaped nudge); all four are fixed
# and this now exits non-zero on any regression. Needs the built MCP binary.
python3 tools/model_the_loop.py

# End-to-end smoke test of the MCP *binary* (stdio JSON-RPC, real RocksDB graph).
# Covers what cargo test can't: the shipped surface, tool schemas, and the JSON an
# agent actually receives. Needs `cargo build -p reflow2-mcp` first (RocksDB, ~10 min
# cold). Stdlib-only Python; exits non-zero on any failed check.
python3 tools/smoke_mcp.py

# Install or update reflow2 in a consumer project (the design environment only —
# never a src/ layout or build file; project type is a design output, not an input).
python3 tools/reflow2_init.py /path/to/project           # set up, or update in place
python3 tools/reflow2_init.py /path/to/project --check   # what would change
```

Tests live beside the code as `crates/reflow2-core/tests/*.rs` (one file per module/concern) plus
unit tests in `src/schema.rs` and doctests in `src/lib.rs`/`src/nodes.rs`.

### Building with limited RAM

**If your terminal window vanishes mid-build — shell, agent and MCP server all dying at once —
that is not a crash, and nothing will be in `dmesg`.** It is `systemd-oomd`, which watches
*memory pressure* rather than the kernel OOM killer watching allocation failure. Your terminal,
the agent driving it and the MCP server all live in one VTE cgroup scope, so oomd takes the
whole scope and the symptom presents as "the terminal crashed". Find it with:

```bash
journalctl --user --since "-3 days" | grep -iE 'oomd|vte-spawn'
```

`cargo test --workspace` is what usually triggers it: RocksDB's C++ build plus parallel rustc
jobs is the peak, and a full workspace run has been measured at **9.3 GB** on a 4-core/11 GiB
box. Two mitigations that help but do **not** prevent a kill on their own — `[build] jobs = 2`
and `[profile.dev] debug = "line-tables-only"` — belong in your own `~/.cargo/config.toml`,
**not in this repo**: they are properties of one machine, and committing them would slow builds
and degrade debug info for every other developer and for CI.

**The reliable fix is to give cargo its own cgroup, so a runaway build dies instead of your
session:**

```bash
systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=2G \
    cargo test --workspace -- --test-threads=2
```

**Disabling `systemd-oomd` outright is a legitimate option, and is what this repo's maintainer
runs.** It does not remove OOM handling — the kernel OOM killer is built in and unremovable, and
that is the point: oomd kills by *cgroup* and takes your whole session, whereas the kernel kills
the single worst process, which under a build is `rustc` and not your terminal. The cost is that
the kernel only acts at true exhaustion rather than early on pressure, so a runaway build can
thrash the machine first — which is exactly what the capped scope above prevents. Use both, or
neither; the capped scope alone is the more targeted of the two.

Two habits worth keeping regardless. Write test output to a file and grep it, rather than
running the suite twice to get failures and then a pass count — that doubling is pure peak for
no information. And for dev iteration use the core-only command at the top of this section,
which skips the RocksDB compile entirely; the workspace run is a pre-push gate, not an
edit-loop command.

None of this affects people who merely *use* reflow2: `tools/install.sh` fetches a prebuilt,
checksum-verified binary and never compiles. It matters for contributors, and for anyone on a
platform with no prebuilt asset, whom the installer deliberately refuses and redirects to a
source build.

**The MCP server this repo runs on its own design graph is launched via
[`tools/reflow2-mcp-launch.sh`](tools/reflow2-mcp-launch.sh)** (wired in `.mcp.json`), not the raw
`target/debug/reflow2-mcp`. The wrapper content-hashes the sources and rebuilds `reflow2-mcp` iff
they changed before exec'ing it, so every new session and every `/mcp` reconnect serves a fresh
binary automatically. It hashes content, not mtimes, on purpose: `cargo build` keys on mtimes, and
`git pull --rebase`/checkout can leave sources *older* than the last-built binary — which silently
served stale code for whole sessions until this was added. All wrapper output goes to stderr;
stdout is the JSON-RPC channel and must stay clean.

> ⚠️ **`/mcp` RECONNECT DOES NOT PICK UP YOUR REBUILD, AND THIS PARAGRAPH SAID IT DID UNTIL
> 2026-08-17.** The old wording — *"you must `/mcp` reconnect reflow2 (or restart the session) for
> the live tools to pick it up"* — is wrong wherever a **shared server** is running, which is the
> normal case here. `--shared` re-attaches to the long-lived `--serve-shared` daemon, and the
> daemon keeps executing the image it started from. Reconnecting spawns a fresh *client* against a
> stale *server*, so it looks like it worked and changes nothing.
>
> **MEASURED THE DAY THIS WAS CORRECTED.** `heal.rs` was widened, the binary rebuilt at 22:13, and
> a `/mcp` reconnect at 22:16 spawned a new `--shared` client — while the `--serve-shared` daemon
> from 18:18 went on serving. `detect_defects` answered `total: 20` from code no longer on disk,
> and said nothing about it. Anthony reconnected *specifically* to make the change live.
>
> **THE ACTUAL REFRESH** is to stop the daemon, then make any tool call (it respawns from whatever
> is at the path):
>
> ```bash
> ./target/debug/reflow2-mcp --graph-path ./.reflow2/graph --stop-shared
> ```
>
> `crates/reflow2-mcp/src/service.rs`'s own `STALE_NOTE` has said this correctly since 2026-08-11
> — *"A SESSION RESTART ALONE, WITHOUT `--stop-shared`, CHANGES NOTHING"* — and this file
> contradicted it. **A running server is the authority on its own currency, not this document:**
> `graph_report`'s `served_by.stale` is the check (`true` = every computed number came from a
> replaced binary; `null` means it could not tell, which is never `false`). Note that
> `loop_status` and `detect_defects` do NOT carry that block, so the debt and defect numbers a
> session actually reads can be stale with nothing saying so — open under
> `req:a-report-says-what-it-swept-and-whether-its-checks-ran`.

## Working on this repo

**Order of operations.** `git pull --rebase`, then `claim_report` to see what is held, then
`mint_seat` once and `claim_region` to take your item — *before* the work, because a claim nobody
can see is not a claim. Release it with `release_claim` when you finish; if you forget, liveness
is computed from your session, so the claim reads `gone` rather than sitting there looking taken.
Then ask the graph what is open and why: the graph — `loop_status` for what the loop owes, `detect_gaps` for the open questions, `search_design` to find a past finding by its words. [COORD.md](COORD.md) covers
resolving merge conflicts on the shared records without discarding anyone's work; read that before
you hit one.

**Branches.** `feat/<short-name>` off `main`, one per claimed item where practical.

**One-time setup per clone — the design-export merge driver.** `.gitattributes` points
`docs/design/reflow2.json` at reflow2's own three-way merge, but git deliberately does not let a
repository configure an executable, so each clone defines it once:

```bash
git config merge.reflow2.name 'reflow2 design export merge'
git config merge.reflow2.driver 'target/debug/reflow2-mcp --merge-driver %O %A %B'
```

Then two people editing different parts of the design merge with no conflict, and a real
both-sides conflict stops with its ids and the `--merge-apply` command that finishes it. Without
the config git falls back to a text merge of a 600KB JSON file — safe, but you resolve it by hand.
**Never resolve a design conflict with `--ours`/`--theirs`**: for code that drops a hunk, for a
design it drops a node someone wrote and nothing will tell you it is gone.

> **`--no-fail-fast` is not optional and not a nicety.** `cargo test` ABORTS EVERY REMAINING TEST
> BINARY the moment one fails, and this workspace has ~155 of them behind a ~13-minute run. So a
> change with three independent failures costs three full cycles — you fix one, wait, discover the
> next, wait again — and on CI that is three push-and-wait rounds for information one run already
> had. **Measured 2026-08-25 while adding one detector**: successive runs reached 31, then 47, then
> 114 suites, surfacing exactly one new failure each time; all four were in `reflow2-core` and one
> `--no-fail-fast` run would have shown all of them at once. The cost is that a RED run now takes
> full wall-clock instead of stopping early, which is the right trade: a red run is already a
> failure, and what you want from it is every fact it has, not the first one.

**A change is done when all of these are clean** — the everyday subset, with the flags CI
actually uses. **Both `-D warnings`, because that is what turns a local warning into a red build:**

```bash
cargo test --workspace --no-fail-fast                    # both crates
cargo test -p reflow2-core --no-default-features --no-fail-fast   # the in-memory backend, alone
cargo clippy -p reflow2-core --no-default-features --all-targets -- -D warnings
cargo clippy -p reflow2-mcp --all-targets -- -D warnings
cargo fmt --check
python3 tools/validate_schema.py                         # after any schema/*.yaml edit
python3 tools/smoke_mcp.py                               # after any tool-surface change
python3 tools/toolsnap.py                                # tool schemas vs committed goldens; --update to bless
python3 tools/skill_lint.py                              # after any skill or tool-surface edit
python3 tools/test_wall_check.py                         # the wall-check instrument's own net
python3 tools/reflow2_check.py --export docs/design/reflow2.json   # design vs build, and the export chain
python3 tools/check_intent_authority.py docs/design/reflow2.json    # settled intent carries the owner's name
python3 tools/check_command_surface.py                   # the skill/command copies still agree
```

> **This list is a SUBSET and `ci.yml` is the authority.** The full job also runs the instruments
> and the Python suites — `phase_trial`, `coherent_erosion_trial`, `model_the_loop`,
> `stateless_seat_probe`, `test_init`, `test_shared_sessions`, `test_merge_driver`,
> `test_degraded_server`, `test_nudge_path`, `test_loop_nudge`, `test_render_views`,
> `test_stale_seat`, `test_reflow2_check`, `check_doc_versions`, `test_check_doc_versions`,
> `test_skill_lint`, `self_host_uses_documents`, `test_check_intent_authority` — so **green here is not green
> there**, and *"believe CI"* below is not a figure of speech. Run the ones your change touches;
> [docs/sharpening.md](docs/sharpening.md) says which instrument covers what.
>
> **KEEPING THESE TWO LISTS IN AGREEMENT IS NO LONGER MANUAL (BL-159).** `skill_lint.py` now fails
> the build if any `cargo`/`python3` gate in `ci.yml` appears in neither this block nor the
> sentence above, if a listed command is spelled differently from the way CI runs it (**flags
> included** — that is the exact defect that filed BL-159: the `-p reflow2-mcp` clippy line was
> missing entirely and the `-p reflow2-core` one lacked `-D warnings`, so following this block
> exactly still produced a red build), or if either list names a gate CI has stopped running.
> **So edit these two lists together with `ci.yml`, or the lint will say so.** Adding a gate to
> `ci.yml` means adding it here — to the block if a developer should run it every time, to the
> sentence above if not. Both answers are honest; silence is the one that is not.

**Export the design EXACTLY ONCE PER PULL REQUEST, straight onto the committed file** — not once
per commit. The distinction is not pedantry and it cost a broken chain on `56bc698`: PRs merge by
**squash**, so N commits become one on `main`, and only the *last* export's `prev_content_hash`
survives. If two commits on a branch each exported, the surviving one links to an intermediate that
`main` never saw, and the history has a hole. A branch may hold as many commits as you like; only
one of them may touch `docs/design/reflow2.json`, and it should be the last.

> ⚠️ **The old corollary here — "put the COORD claim commit first, it carries no export" — died
> with the claim board's move into the graph on 2026-08-04, and what replaced it is a real
> tension rather than a rewording.** A claim is now graph state, so it travels in
> `docs/design/reflow2.json` — and that file may only be written once per PR, last. So a claim
> made *before* the work is not visible to anyone else until the PR **merges**, which is after
> the work is done. That defeats the point of claiming first.
>
> With one writer this costs nothing and is why the move was still right. With two it is a
> regression against the file it replaced, because COORD.md could be committed and pushed on its
> own the moment work started. Recorded as `dec:a-graph-claim-cannot-be-published-before-its-pr`;
> do not treat the claims layer as proven under contention until that is answered.

The gate checks
this since BL-107 and will fail the build if you get it wrong:

```bash
git checkout docs/design/reflow2.json    # start from what is committed
# …then export_graph --path docs/design/reflow2.json --overwrite, ONCE, last of all
```

Each export records the `content_hash` of the one it replaced, which gives the design a history
independent of git. That link is built from **whatever file is already at the target path**, so
there are two ways to break it and both are silent without the gate: exporting somewhere else and
copying the file into place (there was nothing to link to), or exporting **twice** between commits
(the committed record then links to an intermediate that was never committed, leaving a hole).
Make every graph write first; restore and export once at the end.

**CI enforces these on every push** (`.github/workflows/ci.yml`): a fast core job
(core tests, clippy `-D warnings`, fmt, schema, installer suite, skill lint) and a full job
(workspace tests, the smoke test and the exit-zero instruments against the real binary). Green
locally but red in CI means your local run skipped a gate — believe CI. The skill lint checks
the skills' *contract* (served tool names, mirrors byte-identical, frontmatter, the standing
rule); their semantic quality stays evidenced by trials, per docs/sharpening.md — deliberately
no LLM evals in CI.

### Branch, then PR — nothing lands on `main` directly

**Standing practice from 2026-07-31 (Anthony), starting after PR #3.** Work goes to a branch and
reaches `main` only through a pull request that CI has passed. No direct commits to `main`, and
no merge on a red or unrun build.

**The wrinkle, because the trigger config decides the workflow.** `ci.yml` runs `on: push` for
**`main` only**, plus `on: pull_request` for anything. So *a feature branch gets no CI at all
until a PR exists* — "wait for CI, then open the PR" cannot happen in that order here. The
sequence that gets the intent is:

1. Commit to the branch and push. (No CI yet. This is expected, not a problem.)
2. **Open the PR as a draft** — that is what starts CI.
3. Run the local gates meanwhile; a red CI on a branch you gated locally is a finding about the
   gates, not just about the branch (*"believe CI"*, above).
4. Mark it ready and merge **only when both jobs are green**.

A draft PR is the trigger, not a claim that the work is finished — which is exactly why the
draft state exists. Opening one early costs nothing and is the only way to learn what CI thinks.

**When a PR merges, its branch is deleted.** Pushing more commits to that branch afterwards
recreates it *with no PR and therefore no CI*, and they will sit there looking merged when they
are not. Follow-up work after a merge starts a new branch and a new PR.

**If you changed a detector, a phase capability, or anything in the coherence loop**, run the
instruments in [docs/sharpening.md](docs/sharpening.md) before and after, and add a probe for what
you built. A number that moves is your claim; a number that moves the wrong way is a finding. They
are not pass/fail gates yet — they record baselines that are failing on purpose.

Compiling is not the finish line, and neither is a green unit test. Drive the thing you changed:
the surface a user actually touches is the MCP binary, and three home-grown test layers once
agreed with each other and were all wrong because each was a client we wrote.

**Update the records in the same change, not afterwards** — this is the rule most often skipped,
and the records are the project's memory:

| Record | Update when |
|---|---|
| [CHANGELOG.md](CHANGELOG.md) | a user would notice |
| [docs/requirements-coverage.md](docs/requirements-coverage.md) | a status moves |
| the graph (a TemporalFact `defect`, a `planned` Capability, a `proposed` Decision, a DesignRule) | an item is discovered — see `dec:backlog-is-retired` for which shape |
| [docs/trials/](docs/trials/) | a real session went wrong — verbatim, append-only |
| `claim_region` / `release_claim` (the graph) | you start, and again when you finish |

**Claims made in the records must be evidence-backed.** If a backlog entry asserts a consequence,
it should be traceable to something someone observed — not inferred while writing the entry. When
you cannot source it, say so or strike it.

## Architecture

Reflow 2.0 is a graph-backed engine that keeps a design coherent across its lifecycle. Two
crates: **`reflow2-core`** is the deterministic, LLM-free coherence engine; **`reflow2-mcp`**
is the thin agent-native surface over it — the `reflow2-mcp` stdio binary a consumer actually
runs (the surface decision was made and built; see below). The core stays neutral to the
interaction surface (MCP / CLI / hosted) and to any LLM provider — those plug in at the seam,
not the centre.

### The store and schema (the foundation)

- The graph store is **in this tree**, at `src/foundation/` — the schema and `Value`
  vocabulary (`foundation/core/`), the RocksDB-backed store (`foundation/store/`), and the
  full-text index (`foundation/text.rs`). RocksDB is opt-in behind reflow2-core's own
  `rocksdb` feature, so the core runs on the in-memory backend for dev and tests.
  Graph-theory algorithms live beside it in `src/graphalg/`.
- It came from **[dynograph-foundation](https://github.com/sligara7/dynograph-foundation)**
  and was **absorbed from it at `v0.12.0`** — reflow2 no longer links anything from that
  repository (`dec:absorb-the-foundation-subset-and-end-the-dependency`, 2026-08-24).
  **Every absorbed module carries a provenance header naming the tag, files and commits it
  took**, because the recorded objection to absorbing anything is that vendoring turns a
  visible dependency into an invisible one — the pin carried a written reason for every bump
  and in-tree code has no successor to that record. Those headers are that successor, and
  `tools/check_doc_versions.py` reads the tag out of them rather than out of prose.
- The **schema is the vocabulary** (29 node types, 60 edge types across 11 `schema/*.yaml`
  domains): the node/edge names are load-bearing. `src/schema.rs` embeds all ten YAML files
  via `include_str!` and merges them with `Schema::from_multiple_yamls` — the same files
  `tools/validate_schema.py` checks, so there is one source of truth. Terminology in code
  must match the schema; `src/nodes.rs` holds the `node::`/`edge::` name constants.

### The design graph handle

`src/graph.rs` — `DesignGraph` wraps a `foundation::store::StorageEngine` scoped to one
logical graph id. It is the single handle everything else hangs off: generic
schema-validated CRUD (`create_node`/`get_node`/`create_edge`/`outgoing`/`incoming`/
`scan_nodes`/`delete_*`), typed golden-thread constructors (`add_project`,
`add_requirement`, `add_capability`, `add_component`, `satisfies`, `allocate`, `contains`),
and `pub(crate)` batch controls for atomic apply. Each coherence-loop step is a set of
methods on `DesignGraph` implemented in its own module (Rust lets `impl DesignGraph` span
files):

| Loop step | Module | Entry points |
|---|---|---|
| **CHANGE** (axis Z — never overwrite the past) | `src/temporal.rs` | `add_epoch`, `snapshot_node`, `add_change_event`, `record_change` |
| **PROPAGATE** (blast radius along the golden thread) | `src/propagate.rs` | `propagate_change` (reactive), `propagate_from` (speculative) |
| **DETECT** (find gaps to ask the human) | `src/detect.rs` | `detect_gaps` → `GapCandidate`s; `GapCandidate::to_prompt` (PROMPT half, via `LlmBackend`) |
| **HEAL** (fix structure the machine can) | `src/heal.rs` (+ `src/structure.rs`) | `detect_defects`, `propose_heal`, `apply_heal` |
| **LLM seam** | `src/llm.rs` | `LlmBackend` trait, `MockLlmBackend`, `complete_json` |

`src/structure.rs` builds a `graphalg` view (the "design network" — design nodes
joined by *traceability* edges) for HEAL's topology detectors.

### Load-bearing invariants (do not regress these)

- **No silent fallbacks / no silent drops** (AGENTS.md rule 4). This is enforced concretely:
  CRUD fails loud on unknown types / missing required props; PROPAGATE bounds depth but
  *reports* `truncated_beyond_depth`; HEAL moves un-appliable ops to `skipped_operations`
  with a reason; DETECT surfaces unknown seeds; the LLM PROMPT step degrades to raw wording
  with `rephrase_degraded = true`. New code must keep this bar; tests assert it explicitly.
- **HEAL is propose-then-apply.** `propose_heal` never mutates; `apply_heal` mutates
  atomically, is mode-aware (`rigid` project mode = propose-only), gates generated content
  behind `requires_human_review`, and does post-repair verification. Keep detection,
  proposal, and mutation separate.
- **PROPAGATE / structure exclude `CONTAINS`.** Decomposition is not traceability; including
  it makes the Project a hub that short-circuits distances. Impact and topology traverse the
  shared traceability set (`nodes::is_traceability_edge`). `INCLUDES` (Release → Artifact/
  Component) *is* in that set as of the v0.5.0 as-released work — a changed artifact reaches
  the releases that ship it, and a Release+Environment pair is no longer a disconnected island.
- **A release records everything it actually ships — including the documentation.** When cutting a
  release, `release_includes` must list every component that goes out, not a hand-maintained
  roll-call: anything built since the last cut raises `unreleased_component` until its release
  records it, and that gap is the reminder working rather than noise. `cmp:docs` is called out
  because documentation is part of the deliverable and it is the one people forget (Anthony,
  2026-07-24: "it'd be like if I designed a fighter jet using reflow2, but then didn't deliver any
  documentation on it to the user"). The nine releases up to v0.10.1 do **not** record documentation
  and are deliberately left that way — they genuinely shipped a README and a setup guide, but the
  design did not model documentation as a deliverable until 2026-07-25, and back-filling would
  reconstruct history rather than record it (`dec:intent-preserved`). Starts from the next release,
  on the user's call.
- **A release must be pinned to its epoch, and the design now says so itself.** `AT_EPOCH` is
  what puts a Release on the time axis; without it `changelog_point` resolves the release to a
  position with no sequence, so any window computed from it loses its lower bound and silently
  widens to the beginning of the design. This was a memory-only step and it failed twice —
  `rel:v0190` was cut without the edge four hours before `changelog_view` needed it, and
  `v0.17.0` still lacks one while `v0.18.0`'s commit message boasted of not repeating the
  fault. `release_without_epoch` now raises it per release (BL-122). **Do not "fix" a release
  by renaming things to match**: a matching name plus an existing epoch node is exactly what
  made the missing edge invisible, and only the edge counts.
- **A release must record what it shipped, and the cut ENDS by reading that record back.** An
  `INCLUDES` edge per artifact and component is what makes the as-released view exist; the
  schema has always said so (*"a Release with none is a version number, not a manifest"*) and
  nothing enforced it until 2026-08-21. `release_without_manifest` now raises it per release
  when a Release is pinned to an epoch, deployed, and includes nothing, **at severity 0.85 —
  which STOPS THE BUILD** (Anthony's call, 2026-08-21). It sits with the findings that say the
  design asserts something untrue rather than with the questions that wait for a human, because
  a note was demonstrably not enough: the v0.38.0 cut passed `reflow2_check` green with 96 notes
  scrolling past it. A genuinely contentless release — a re-tag, a docs-only republish — is
  still a real state, and stays acknowledgeable with a reason on the record. **The rule never
  consults `Release.status`** — `rel:v0380` was tagged, published, deployed and asset-verified
  while its status still read `planned`, so a status-based exemption would have excused the one
  case it exists for. The same clause was retrofitted to `release_without_epoch`, which had
  exempted any `planned` release since BL-122 and now exempts one only while it is undeployed.
  **The trap this closes:** `release_includes_all` defaults to a DRY RUN and answers
  `{"added": 304, "applied": false}` — pass `apply: true`, and finish the cut with
  `release_report`, which is the authoritative object. v0.38.0 was published with an empty
  manifest because `isError` was false, `reflow2_check` was green, and nobody read the object.
- **The whole cut is `flow:release-cut` in the graph — read it with `flow_report`, do not
  reconstruct it.** Eight ordered steps with their transitions, including the two backward
  "forces resync" edges that say what sends a cut back a step. It satisfies
  `req:a-project-records-how-it-ships`, and it exists because five separate cuts each shipped a
  different step wrong and recorded it only afterwards. **Order is load-bearing in three
  places**: accept the `reflow2.toml` version drift BEFORE freezing the manifest, rebuild the
  binary after the version bump and before the export, and export LAST because it costs the
  session its MCP access.
- **Structural topology detectors are selective.** A design's golden thread is tree-shaped,
  where every internal node is a naive articulation point — so `single_point_of_failure`
  only fires when a node separates ≥2 real subsystems (see `structure.rs`).
- **Do not bump the `rocksdb` pin as housekeeping.** The foundation pin this rule used to
  name is gone (absorbed, 2026-08-24), but its reason outlived it and now attaches to the
  storage dependency itself: moving `rocksdb` forces a full `librocksdb-sys` C++ rebuild
  (~10 min) on **every** machine that pulls — yours, your collaborators', and every consumer
  project. Bump it only when a reflow2 change actually needs something the new version
  provides, and say which capability in the commit message. "Latest is probably better" is not
  a reason; a routine reflow2 update should cost a consumer nothing but a text refresh.
  ⚠️ `rocksdb` sits at 0.24, the historically-unmaintained wrapper, **deliberately** — see
  `dec:absorb-rocksdb-024-unchanged-then-switch-separately`. The move to the maintained
  `rust-rocksdb` gets its own PR so the migration has one variable.
- **A storage-format change is a data-migration question, not just a code change — and
  absorbing the store made it EASIER to make one by accident.** Nothing is stamped on the graph
  directory — not a schema version, not a foundation tag — and validation runs on write, never
  on read. So a change to `foundation/store/keys.rs` or value serialization could misread an
  existing store with nothing to detect it, and an additive schema change leaves mixed-vintage
  nodes rather than backfilling (`foundation/store/engine/tests.rs` pins that behaviour:
  defaults apply on create, not retroactively). This used to be gated by the friction of
  bumping someone else's pin; now it is an ordinary edit in this repo. Before touching either,
  ask what happens to a graph written by the previous version. See **BL-19**.
- **Deterministic ids.** Gap/heal issue ids are a stable FNV-1a hash of
  `source + sorted affected ids` (not `std` `DefaultHasher`) so they're reproducible for
  dedup/caching.
- **`LlmBackend` is sync and object-safe.** The core holds `&dyn LlmBackend` and never names
  a provider. Typed JSON parsing is the free function `complete_json`, kept off the trait to
  preserve object safety. Build and test new LLM-reasoning ops against `MockLlmBackend`; do
  not add an async runtime or a provider dependency to the core.

### What's deliberately not here yet

One decision remains deferred — **real LLM provider backends** (unneeded on the agent-native
route: the ambient coding agent *is* the LLM). Still unbuilt: **SME augmentation**,
**generative HEAL content** (proposals stay review-gated stubs), and the optional **embedding
seam** (semantic dedup/retrieval). **SP-3b landed 2026-07-27** — `ingest_step` drives the
extraction pipeline with the calling agent as the model, so INGEST is finally reachable from a
session rather than only from a test. See the coverage
matrix for the exact deferral list. Everything else in the loop — the MCP surface, GENESIS,
INGEST's core, the consumer kit, search, the reconcile family — is built and shipping as of
v0.46.0.

---

## What Reflow 2.0 is

A graph-backed system that partners with an LLM agent to **design and build anything** —
software, hardware, a document, a full acquisition program. It captures the **entire
lifecycle of a design (concept → operations) in one knowledge graph**, tied together by
the systems-engineering *golden thread* (traceability from every artifact back to the
intent it serves).

The payoff: when **anything changes in any phase**, the ripple effects are automatically
found, surfaced to the user as plain questions, and healed back to coherence — so concept
through operations always stays in agreement. **The user never needs to know systems
engineering; the graph does.**

This is a clean-room rebuild ([github.com/sligara7/reflow2](https://github.com/sligara7/reflow2))
of ideas from the author's earlier projects (all under
[github.com/sligara7](https://github.com/sligara7)): `reflow`, `storyflow`,
`chain_reflow`, and the graph engine `dynograph-foundation`.

### What it is for, and what it is not — **reflow2 for *why*, tests for *whether*.**

The sharpest statement of this came from outside, and it is worth keeping in the owner's words.
A project designed end to end through reflow2 (2026-07-31) ran sixteen sessions of physics that
kept overturning itself, and ended with a six-deep chain of superseded Decisions — five
abandoned positions, each carrying why it failed — which they judged the payoff: *"no code
review, git history or comment thread would give a reader that. This is what the graph is for."*

And the counterpart, stated just as plainly: *"the one thing the graph could not do at any point
was tell me a number was wrong. That needed sources and executable checks."* Their division held
across the whole arc — **reflow2 is where reasoning persists; tests are what stop you being
wrong. They are complementary, and it would be a mistake for either to try to be the other.**

Carry it as a design constraint, not a slogan. It is the reason reflow2 reasons from graph state
and never from run history (`dec:loop-status-state-not-history`), and the reason a detector
should say what it *cannot* see rather than grow toward becoming a test runner.

## The one mental model to hold: the coherence loop

```
CHANGE → PROPAGATE → DETECT → SURFACE → RESOLVE/HEAL → COHERENCE
```

- **CHANGE** — any edit becomes a `ChangeEvent` at a `DesignEpoch` (the old state is snapshotted, never overwritten).
- **PROPAGATE** — walk the traceability edges to compute the blast radius.
- **DETECT** — re-diagnose the touched region for new gaps/contradictions.
- **SURFACE** — turn those into constructive, plain-language questions for the user.
- **RESOLVE/HEAL** — the user answers (re-ingested) or HEAL proposes structural fixes.

Three complementary lenses on the graph: **phases** (P0–P5 lifecycle), **three axes**
(X = network, Y = decomposition, Z = change-over-time), and this **loop** (behavior).

## Current state (important)

**Shipping at v0.46.0.** The deterministic core, the agent-native MCP surface, and the consumer
kit are all built, released as prebuilt binaries, and cold-start-verified. As of v0.12.0 the kit
is *served* rather than installed: a project holds a pointer file and the MCP config, and both the
skills and the working instructions come from the binary (`req:thin-install`). The interaction
surface — once an open question — was **decided (agent-native MCP, 2026-07-18)** and built;
`docs/requirements-coverage.md` is the living status matrix. Do not re-litigate the surface
decision or assume the surface doesn't exist — it is the binary a consumer runs, and
`tools/smoke_mcp.py` drives it end to end.

- `crates/reflow2-core/` — the deterministic, LLM-free coherence engine. Each coherence-loop
  step and each analysis is its own module (30 files); the load-bearing ones: `schema` (merges
  the 11 domains), `graph` (`DesignGraph`: schema-validated CRUD + typed golden-thread
  constructors + `upsert_node`), `nodes` (the `node::`/`edge::` name constants + the
  traceability-edge table), `temporal` (**CHANGE** — epochs, snapshots, `record_change`),
  `propagate` (**PROPAGATE** — direction-classified bounded BFS → an explained `BlastRadius`,
  summary-by-default with `full` for the dump), `detect` (**DETECT** — ranked `GapCandidate`s),
  `heal` + `structure` (**HEAL** — propose-never-mutate → atomic `apply_heal`; topology defects
  over the design network), `llm` (the sync object-safe `LlmBackend` seam + `MockLlmBackend`),
  `ingest` (**INGEST** — freeform text → graph via `LlmBackend`), `genesis` (**GENESIS** —
  bootstrap from a brief), `allocate`/`hierarchy`/`dimensions`/`budget`/`surprises`
  (graph-analysis), `confirm`/`drift`/`fielded`/`verify` (the reconcile + confirmation-ledger
  family), `artifact`/`operate`/`flow`/`provenance`/`report`/`search`/`vocabulary`/`export`
  (as-built linking, releases/environments, process flows, provenance, the rollup report,
  BM25 search, schema discovery, portable export/import). Fast dev/test build:
  `cargo test -p reflow2-core --no-default-features`.
- `crates/reflow2-mcp/` — the agent-native MCP stdio server (`service.rs`, ~78 tools; `dto.rs`;
  `main.rs`). Thin: every tool locks the graph, calls one core op, returns. It carries the
  `rocksdb` feature on its dependency edge, so it pays the C++ build; see its own AGENTS.md.

Still unbuilt (see "What's deliberately not here yet" above and the coverage matrix): external
LLM provider backends (deferred — unneeded agent-native), SME, generative HEAL content, and the
embedding seam. The `ingest` MCP handshake (SP-3b) shipped in v0.16.0.

- `schema/*.yaml` — 11 composable schema domains (29 node types, 60 edge types), in the
  format defined by `src/foundation/core/schema.rs`. This is the foundation everything builds on.
- `docs/*.md` — the vision, design, and process specifications; `docs/overview.md` maps them.
- `getting-started/` — the consumer kit installed into a project being designed (never a build
  file). `tools/reflow2_init.py` installs it; `install.sh` fetches the released binaries.
- `tools/validate_schema.py` — validates the schema against `foundation/core`'s rules.

## Where to look

**Always start with [docs/overview.md](docs/overview.md)** — it maps every document and the
reading order (Vision → Design → Process → Heritage). Then:

| You want to… | Read |
|---|---|
| understand the "why" | [docs/vision.md](docs/vision.md) |
| understand the graph structure | [docs/three-axes.md](docs/three-axes.md), `schema/` |
| know how content gets into the graph | [docs/extraction-plan.md](docs/extraction-plan.md), [docs/sme-augmentation.md](docs/sme-augmentation.md), [docs/artifact-linking.md](docs/artifact-linking.md) |
| know how change is handled | [docs/impact-propagation.md](docs/impact-propagation.md), [docs/gap-surfacing.md](docs/gap-surfacing.md), [docs/heal-process.md](docs/heal-process.md) |
| understand the operating environment/ruleset | [docs/operating-environment.md](docs/operating-environment.md) |
| know how a human drives it (and the LLM-sourcing tradeoff) | [docs/interaction-surfaces.md](docs/interaction-surfaces.md) |
| confirm the build meets the docs (traceability) | [docs/requirements-coverage.md](docs/requirements-coverage.md) |
| use the graph to *drive* design decisions (allocation, weights, analysis crates) | [docs/graph-analysis.md](docs/graph-analysis.md) |
| make reflow2 drivable by a coding agent (the next build phase) | [docs/surface-plan.md](docs/surface-plan.md) |
| see where ideas came from | [docs/reflow-v3-nuggets.md](docs/reflow-v3-nuggets.md), [docs/chain-reflow-nuggets.md](docs/chain-reflow-nuggets.md) |

## Rules for changing this project

1. **Schema-first.** The node/edge vocabulary is load-bearing. After any `schema/*.yaml`
   edit, run `python3 tools/validate_schema.py` (needs PyYAML — on this machine use
   `~/miniconda3/bin/python`). It must print "OK".
2. **Keep docs cohesive.** Every doc carries a breadcrumb to `overview.md`; new docs must
   too, and must be added to the overview's document map and reading order.
3. **Terminology matches the schema.** Use the real node/edge names (e.g. `Capability`,
   `Component`, `Artifact`, `Verification`, `Environment`, `EnvironmentRule`,
   `ChangeEvent`). Do not reintroduce retired names from old Reflow (e.g. `PhaseEvent`,
   `ContractedFunction`, `APIEndpoint`).
4. **Honor the disciplines** the process docs call "non-negotiable" — most importantly
   **no silent fallbacks / no silent drops**: surface failures and skipped items loudly;
   never let data loss or an unstated assumption pass as success.
5. **References are the author's own** under `github.com/sligara7`. The only third-party
   pieces are dependencies (RocksDB/Tantivy/serde, declared directly since the foundation
   was absorbed; LLM providers like OpenRouter) — never conceptual content.
6. **Don't touch the sibling source repos** (`../../storyflow`, etc.) — mine them for
   ideas, but all new work lands here.
7. **A self-host finding owes a served fix.** reflow2 designing itself is a test harness, not
   the point. A lesson recorded only in reflow2's own graph reaches nobody who uses reflow2:
   graph nodes are project-local, and a consumer has their own graph and their own rules. So
   before a finding is filed, ask **which served surface carries it** — a skill, a tool's
   behaviour or output, the served instructions, or the kit. A finding that lands only in our
   own record is unfinished work, not a completed capture. Measured 2026-08-14: three gate
   lessons were captured as DesignRules here while `ci-gate` — served to every project — was
   silent on all three *and* documented two of its four failure modes, and `parallel-work` told
   users to export before every commit, which is exactly what the lineage check fails on.
   The mirror error is real too: don't push our internal discipline into a served skill, where
   it becomes a rule for somebody else's project that nobody there chose.

### What carries across sessions, and what does not

Taken from the StoryFlow fleet's boss charter, which draws the line explicitly and puts the
highest-consequence permission on the far side of it: *"prior-session GOs — merge authority,
deploy windows — do not carry into your session. Get fresh ones."* Absent a written list, both
errors are available: asking permission for the routine, and assuming it for the irreversible.
The line is **reversible → standing; outward-facing → per-session.**

**Standing** — do these without asking again:

- Accept checksum drift on a registered artifact you just edited, with a `note` saying which it
  was (standing policy, 2026-07-23). The accept is recorded either way, so it is reviewable.
- Record intent in the graph, run the detectors, commit and push to `main` with the gate green.
- Acknowledge a gap or a defect **only** with the user's reason on the record — the
  acknowledgement is a Decision, and `dec:certainty-derived` says a status change carries the
  *user's* word, not yours.

**Per-session** — ask every time, however recently it was granted:

- **Cutting a release.** Tagging and publishing is this project's deploy: it reaches other
  people's machines and cannot be recalled.
- **Filing an issue or anything else in a repository the user does not control** (the
  `report-friction` skill already enforces this).
- **Moving a Requirement off `proposed`, or a Decision to `accepted`.** That write *is* the
  user's signature; making it on their behalf forges it.

## Engineering principles (adapted from storyflow's `PROTOCOL.md ⭐`)

These are the author's hard-won code-quality principles, carried over and adapted to
reflow2 (a single Rust core, no fleet). They **override speed — timing bends to correctness**.

1. **Right long-term fix — no patches/stopgaps.** Find the *root cause* first (reproduce,
   trace, prove the mechanism), then fix it at the root, not the symptom. If you can't name
   the root cause, say so and keep digging. If the correct fix needs something that isn't
   there yet, **stop and report the gap** — a reported gap is honest; a papered-over one
   re-breaks later.
2. **No silent fallbacks / no silent drops — an integrity line, not a style preference.**
   Never swallow an error into a "looks fine" state (no catch-returns-default, no atomic op
   that drops the bad part, no empty-on-failure). A swallowed failure makes broken code
   report success — it *lies* to the user. Fail loud, or don't write it. This is rule 4
   above; it is the project's first principle. (Enforced concretely across the core — see
   the "load-bearing invariants" under Architecture above.)
3. **Record every deferral — no silent stubs.** When you defer work, write it down as
   Deferred in [docs/requirements-coverage.md](docs/requirements-coverage.md) **in the same
   change**, and annotate the code site (an unused field, a stubbed branch) with a pointer
   back. A deferral nobody wrote down is a silent stub — the same integrity breach as a
   silent drop. "Partial and recorded" is fine; "looks done but quietly isn't" is not.
4. **Verify your own claims before stating them.** Run the real check yourself (foreground
   `cargo test --no-default-features`, clippy, fmt) and confirm any symbol/field/API you
   reference actually exists. "Tests pass" means you watched them pass; a green that only
   passed because an error was swallowed is a false report.
5. **Real-path tests.** A test must exercise the path callers actually use, end to end — not
   an unchanged inner helper. "Done" = the real behavior is observable and tested, not "it
   compiles."
6. **No silent caps/truncation.** If you bound coverage (top-N, a subset of passes, a depth
   cap), say so loudly in the code and the report — silent truncation reads as "covered
   everything" when it didn't. (This is why PROPAGATE reports `truncated_beyond_depth` and
   INGEST records `dropped_edges`.)
7. **Modular, composable code — no monoliths.** Keep files focused and single-responsibility;
   split along natural seams (each coherence-loop step is its own module). Prefer small
   composable pieces and dependency injection (e.g. `&dyn LlmBackend` passed in) over
   sprawling files or deep inheritance.

## Provenance of the ideas (so you can trace any decision)

- **storyflow** → the extraction pipeline, the six universal processes, the operating-
  environment ruleset (its "cosmology"), SME/supplementary analysis, the note layer, the three axes.
- **chain_reflow** → matryoshka/missing-intermediate detection, correlation-vs-causation
  rigor, creative linking, system-of-systems.
- **reflow (v3)** → the phase spine, as-designed/as-built/as-fielded fidelity views,
  framework packs, root-cause change classification.
- **dynograph-foundation** → the schema-driven graph store (RocksDB + BM25 + fuzzy
  matching). The subset reflow2 actually called was absorbed into this tree at `v0.12.0`;
  what reflow2 never called (HNSW/vector resolution, pagerank, the entity resolver) stayed
  behind.

> **reflow2 is installed here.** The design graph is this project's memory — read [REFLOW2.md](REFLOW2.md) and consult it before writing or changing code.
