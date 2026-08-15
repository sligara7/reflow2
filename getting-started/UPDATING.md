# Updating reflow2 without losing your design

Short answer: **updating reflow2 does not touch your design.** The binary and the design are
separate things in every deployment shape, and this page says exactly where your design lives so
you can be sure of that rather than hopeful about it.

It also says plainly what reflow2 does **not** do for you — backups — and what it gives you to
build them with.

---

## Where your design actually lives

| | What it is | Survives an update? |
|---|---|---|
| `.reflow2/graph/` | a RocksDB store — the working copy | yes, it is never inside the binary |
| `.reflow2/graph.id.json` | identity: **which design this is** | yes — and see the warning below |
| `.reflow2/graph.meta.json` | the version stamp of the reflow2 that wrote it | yes |
| `docs/design/<name>.json` | **the committed export — the durable record** | yes, it is in your repo |
| `reflow2-content/` | the content-addressed blob store | yes |

**`.reflow2/` is gitignored, and that is deliberate.** The store is a *cache*. The **export** is
the record: a complete, deterministic, content-hashed document that lives in your repo and moves
with it. If the store is ever lost, `--import` rebuilds it from the export and you lose nothing
that was committed.

---

## Two things can be out of date, and they update separately

This is the single most common confusion, so it is stated before the procedures:

| What | How it updates | What it is |
|---|---|---|
| **reflow2 itself** — the binary and the kit on this machine | re-run the installer (below) | needs the network |
| **a project already set up** — its `AGENTS.md`, slash commands, MCP config, hooks | **`reflow2 update`** | purely local, never downloads |

**Updating reflow2 does not update a project you set up earlier.** The per-project files were
copied when you ran `reflow2 init`, and they stay at that generation until you say otherwise. A
project installed at 0.16.0 under a 0.30.0 binary gets *current instructions driving an old kit*,
and nothing announces it.

```bash
cd my-project
reflow2 update --check     # what would change, writes nothing
reflow2 update             # bring this project forward
```

It reuses the harness you chose the first time, keeps files you edited, and **never touches your
design graph**. It refuses a project that was never set up rather than quietly performing a first
install — absence of a kit is not staleness, and the message names `reflow2 init` instead.

`reflow2 update` also *reports* when the binary itself is behind. It will not update it for you:
that needs the network and is the procedure below.

## Updating a locally-installed reflow2

Replace the binary. That is the whole procedure.

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
```

Then **restart your agent session** — an MCP server is a running process, and a reconnect does not
replace one that is already running. Only a full restart picks up a new binary.

**What happens the first time the new binary opens your store:** it reads the version stamp beside
it and tells you what it found.

- *"this graph was written by reflow2 0.23.0 … you are running 0.24.0. **Additive only — everything
  in it still reads.**"* — schema growth only ever adds, so an older store is safe.
- *"this graph carried no version stamp; recording reflow2 0.24.0 from now on"* — it predates the
  check. Nothing is wrong.

If a release needs a migration step, it ships an `upgrading-to-v0.X.0.md` alongside it. Those are
the exception, not the rule.

> ⚠️ **Downgrading is NOT currently checked.** Opening a store with an *older* reflow2 than the one
> that wrote it has no verdict and no warning today. If you need to roll back, restore the store
> from a backup taken before the upgrade, or re-`--import` a committed export from that era.

---

## Updating a containerised reflow2

**Replacing the image does not touch your design, because none of it is in the image.** Stop the
old container, start the new one against the same volume:

```bash
docker pull ghcr.io/<owner>/reflow2-mcp:0.25.0
docker stop reflow2 && docker rm reflow2
docker run -d --name reflow2 -p 8080:8080 -v /srv/reflow2-data:/data \
  ghcr.io/<owner>/reflow2-mcp:0.25.0
```

This is tested rather than asserted: stopping a container and starting a new one against the same
volume leaves `graph_id` byte-identical, with no re-mint warning — the new container adopts the
existing design instead of opening an empty one beside it.

### ⚠️ The one mistake that looks like data loss

**Mount the directory that CONTAINS the store, never the store itself.**

```
/data/graphs/myproject/graph            ← the store
/data/graphs/myproject/graph.id.json    ← identity  ┐  these are SIBLINGS of the store,
/data/graphs/myproject/graph.meta.json  ← version   ┘  not inside it
```

Mounting `.../graph` leaves the sidecars behind. A store opened without the identity it was
created with **finds nothing and presents as an empty design, reporting no error** — your data is
still on disk, and reflow2 will cheerfully show you a blank project beside it. Mount the parent.

**Use a real block device or local volume, not NFS.** RocksDB's exclusive lock is a filesystem
lock, and network filesystems honour those unreliably. A lock that silently fails to exclude is
how two processes end up writing one store.

---

## Backups are yours, not reflow2's

**reflow2 does not back your design up, and will not.** That is a deliberate boundary, not a gap.

Backup is a property of *where your data lives* — your volume, your storage account, your
retention window, your compliance rules. reflow2 knows none of that, and a design tool that
invented an answer would be wrong for most people who installed it.

**What reflow2 gives you to build one with:**

- **`export_graph` is a complete snapshot.** One deterministic, content-hashed document containing
  every node and edge. Two exports of an unchanged design are byte-identical, so it diffs cleanly
  and a corrupted copy is detectable.
- **`--import` is the restore.** It rebuilds a working store from an export, and preserves
  `graph_id`, so the restored design *is* the same design rather than a copy that shares a name.
- **For the repo-file model, git is already your off-host backup.** The export is committed, so
  every clone and every remote is a copy, versioned and timestamped, for free.

**A hosted deployment is the case that needs real work**, because a server has no repo. A
reasonable shape, borrowed from production practice:

1. Snapshot before every deploy — `export_graph` to a file on the volume.
2. Push that file off-host, and **verify it landed** rather than assuming it did.
3. After the deploy, compare node counts by type against the pre-deploy snapshot; treat any
   *decrease* as an alarm and print the restore command.
4. Keep receipts, so an audit can correlate a deploy with what the design looked like either side.

Steps 1 and 2 are the ones that matter. Step 3 is cheap and catches the failure that silence would
otherwise hide.

---

## Quick checks

```bash
# Which design is this, and which reflow2 wrote it?
cat .reflow2/graph.id.json .reflow2/graph.meta.json

# Does the committed record still match the build? (also checks your dependency pins)
python3 tools/reflow2_check.py --export docs/design/<name>.json

# Rebuild a working store from the committed record
reflow2-mcp --graph-path .reflow2/graph --import docs/design/<name>.json
```

If `reflow2_check` passes, the design in your repo describes the build you have. That is the
property worth protecting across an update, and it is the one this page exists to keep true.
