# Working one design from two places — the git workflow

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

*For two (or a few) people building one project with reflow2, on different machines, at the same
time. No server to run, nothing to host, nothing to pay for.*

---

## What you are setting up

You each keep a **complete, independent copy** of the project — code, history, and the design.
You both work whenever you like, without asking each other. Git reconciles the work when it comes
back together, and reflow2 merges the *design* itself, node by node, so two people editing
different parts of one design produce no conflict at all.

There is no shared drive, no lock, no "are you in that file?" Neither of you can be blocked by the
other, and either of you can work with no network at all.

## The idea in one page

The instinct from shared drives is that collaboration means **taking turns** — I have the file,
you wait. Git does the opposite: everyone edits at once, and reconciliation happens afterwards, by
content.

When your work meets your partner's, git compares **three** versions of each file:

| | |
|---|---|
| **base** | the last version you both started from |
| **ours** | your version |
| **theirs** | theirs |

For each region of the file: if only one side changed it, that change is taken, silently. If both
sides changed the **same region** differently, that is a conflict and a person decides.

The consequence worth internalising: **you do not conflict because you touched the same file, only
because you touched the same lines.** Two people working in one project all day typically produce
zero conflicts.

The one thing git will refuse: if your partner published while you were working, your `push` is
rejected — *non-fast-forward*. Git will not let you publish a history that does not contain
theirs. You pull, your work replays on top, you push again. **That refusal is the safety net**;
everything else is convenience.

---

## One-time setup — each person, once per machine

### 1. Install reflow2

```bash
curl -fsSL https://raw.githubusercontent.com/sligara7/reflow2/main/tools/install.sh | sh
```

Installs the `reflow2-mcp` binary to `~/.local/bin` and the kit to
`~/.local/share/reflow2/kit`, checksum-verified. (From-source build: see
`getting-started/SETUP.md`.)

### 2. Get the project

Whoever starts it does this once:

```bash
python3 ~/.local/share/reflow2/kit/tools/reflow2_init.py <project> --binary ~/.local/bin/reflow2-mcp
cd <project> && git add -A && git commit -m "reflow2 setup" && git push
```

The other person just clones it. Then run the init command in the clone too — it writes the MCP
config with *your* binary path, which is machine-specific and not shared.

### 3. Configure the design merge driver — do not skip this

This is the step that makes two people editing one design painless, and git deliberately does not
let a repository configure it for you (a repo that could run programs on clone would be a security
hole). **Each of you, once per clone:**

```bash
git config merge.reflow2.name 'reflow2 design export merge'
git config merge.reflow2.driver "$HOME/.local/bin/reflow2-mcp --merge-driver %O %A %B"
```

Use the real path to your binary. Check it took:

```bash
git config --get merge.reflow2.driver
```

Without it, nothing breaks — git falls back to an ordinary text merge on a large JSON file, which
is safe but means resolving the design by hand. With it, reflow2 merges the design itself.

### 4. Agree on two things, not more

- **Branch or not.** Two people can both commit straight to `main`; it works. If you prefer,
  `git checkout -b feat/<short-name>` per piece of work.
- **Where you announce work.** `COORD.md` in the repo, or reflow2's own claims. Either is fine —
  see *Claiming* below.

---

## What lives where — and what merges how

The single most useful thing to understand: **your design graph is not the shared thing.**

- `.reflow2/graph` is your own local database. It is **gitignored** and machine-local, like a
  build directory. It never travels.
- `docs/design/reflow2.json` is the **exported design** — the shared, human-readable, merge-able
  record. That is what travels in git.

So the rhythm is: work in your graph → export → commit → push. And the reverse on the way in.

| File | What happens when you both change it |
|---|---|
| `docs/design/reflow2.json` | **reflow2 merges it** per node and per property against the common ancestor. You add a requirement, they add a decision — merges silently. Only both of you editing *the same property* stops for a human. |
| `COORD.md`, `CHANGELOG.md` | Both sides' lines are kept automatically. A duplicate line is visible and trivial to tidy; a lost claim is not. |
| `docs/backlog.md`, coverage matrix | **Deliberately manual.** A clash here usually means you both changed the same item's status, which genuinely needs a person. |
| Source and tests | Ordinary git. A real conflict here means you were both editing the same module — a coordination miss, not a git problem. |
| `Cargo.lock` and similar | Do not hand-merge. Take either side, rebuild, commit the result. |

---

## The daily loop

```bash
# 1. Start from what they've done.
git pull --rebase

# 2. Say what you're taking (see Claiming), commit and push that FIRST.

# 3. Work. Commit in small steps.
git add -A && git commit -m "..."

# 4. Export the design if you changed it, then publish.
#    (In your agent: export_graph path=docs/design/reflow2.json overwrite=true)
git pull --rebase && git push
```

Steps 1 and 4 are the whole discipline. **Push every hour or two, not every few days.**

## Claiming work

Claims are **advisory**. Nothing blocks you, ever — that is deliberate, and it is what a fifteen-
session fleet running this way in production found to be right.

Two rules make it work:

1. **Announce every claim by pushing it**, every time, not only when you expect a clash. A claim
   your partner cannot see is not a claim.
2. **If two claims cross, the earlier one in git history wins** — and both of you can verify that
   independently, without anyone arbitrating.

Either write it in `COORD.md` or ask your agent to use reflow2's `claim_region`, which computes
the affected region from the design rather than from a hand-typed list of files.

## Several sessions sharing one design, live

Everything above reconciles work **afterwards**, through git. This section is the other mode: several
sessions on **one design at the same moment**, with no export, no merge and no pull between them.
There are three cases and they are not equally hard.

### Different projects — nothing to do

Each project has its own graph directory, so each session runs its own server and they never meet.
This has always worked.

### The same project — set it up in four steps

The same four steps whether the other sessions are on this machine or another one. Only **step 3**
differs, and it is the only place a second machine costs you anything extra.

#### Step 1 — pick the machine that will hold the design

One machine runs the server; every session anywhere talks to it. Pick the one that stays awake — a
desktop rather than a laptop that travels. Everything below happens on **that** machine unless it
says otherwise.

#### Step 2 — nothing, if your config was written by a current reflow2

**Several sessions on one machine share a design by default.** `reflow2_init.py` writes `--shared`
into every MCP config it generates, and a `--shared` session finds the server holding its graph,
starts one if there is none, and talks to it. No server to start, no port to choose, no config to
edit. Skip to step 3, or stop reading if everyone is on this machine.

```json
{
  "mcpServers": {
    "reflow2": {
      "command": "/home/you/.local/bin/reflow2-mcp",
      "args": ["--graph-path", ".reflow2/graph", "--shared"]
    }
  }
}
```

If yours has no `--shared`, it predates this: re-run `reflow2_init.py <project>` (or add the flag by
hand) and reconnect. **A config without it makes every session after the first fail** — each one
launches its own reflow2, the store admits one writer, and the rest get the degraded server. That
was the shipped default for a long time and it is worth naming, because the failure looks like a
property of reflow2 rather than of a config line: a fleet of six sessions spent five days taking
turns, and building a protocol for taking turns, around a limitation their binary had already lost.

**Who owns the server: nobody.** It runs in its own process group, so the session that happened to
start it can be closed, crash, or be Ctrl-C'd without disturbing the others. When no session has used
it for a while it exits and releases the store's write lock (`--idle-timeout`, default 120 minutes),
because holding that lock blocks `--import`, a live `--diff`, and anything else that opens the graph
directly. Sessions recover from that on their own — the next tool call starts a replacement and is
retried, so you will not notice.

To stop it deliberately — before a backup, say, or to run a CLI command against the store:

```bash
reflow2-mcp --graph-path ./.reflow2/graph --stop-shared
```

**Running the server yourself is still supported and is what step 3 needs**, because a server that
other *machines* dial has to bind an address you choose:

```bash
reflow2-mcp --graph-path ./.reflow2/graph --http 127.0.0.1:8787
```

Sessions can then be pointed straight at it, no `--shared` involved:

```json
{ "mcpServers": { "reflow2": { "type": "http", "url": "http://127.0.0.1:8787/" } } }
```

The trade is worth knowing: a URL in the config is one process fewer, but if that server is not
running the session gets connection-refused, which an agent cannot tell apart from *"reflow2 was
never configured for this project"*. `--shared` keeps a process on stdio precisely so there is always
something able to say what went wrong.

Either way, sessions share one design: a requirement one captures is visible to the others
immediately, and each session is its own **seat**, so claims say who actually made them.

> If the server exits complaining that the graph is held, a session somewhere still has the old
> config. Find it and restart it — the message names the path it could not open.

#### Step 3 — connect the other machine

Only for sessions on a **different** machine; skip this if they are all on this one. Pick whichever
row describes you. All three are private networks, because there is no authentication.

| Your situation | What to do |
|---|---|
| The other machine can SSH to this one | **An SSH tunnel — the simplest, and nothing to install.** |
| Both on the same home or office network | A direct connection over the LAN. |
| Different buildings, or different cities | A tailnet (Tailscale) or a VPN. |

**SSH tunnel.** Leave the server exactly as step 2 started it — no extra flag, nothing exposed. On
the *other* machine:

```bash
ssh -N -L 8787:127.0.0.1:8787 you@the-server-machine
```

Then give that machine the **same config as step 2** (`http://127.0.0.1:8787/`) and restart its
session. It works because the tunnel makes the far end dial its own `localhost`, which reflow2
already trusts. Encrypted, and no port is open to anyone else.

**LAN or tailnet.** Here the other machine dials this one by name or address, so the server has to
be told that name. Restart it as:

```bash
reflow2-mcp --graph-path ./.reflow2/graph \
            --http 0.0.0.0:8787 \
            --http-allow-host 192.168.1.38          # or my-desktop.tail1234.ts.net
```

and point the other machine's config at the matching URL — `http://192.168.1.38:8787/` or
`http://my-desktop.tail1234.ts.net:8787/`. Use *exactly* the host the other machine dials: if the
URL says an IP address, the flag takes that IP address, not the hostname.

#### Step 4 — check it worked

From a session on the other machine, ask your agent to capture something trivial in the design; from
a session on this one, ask what the design says. If the second sees what the first wrote, with no
export, no merge and no pull in between, you are done.

If a remote session fails instead, the two likely answers are both visible: a **403** means the
host it dialled was not named in step 3 (and the server said so when it started — check that
terminal), and a connection refused or timeout means the network, not reflow2.

### Why the second machine needs a host named

The transport answers only requests whose `Host` header is on an allowlist — `localhost`,
`127.0.0.1` and `::1` by default. That is DNS-rebinding protection: without it, a web page you visit
could have your browser post to your own machine and rewrite your design. Since reflow2 has **no
authentication**, that allowlist is the only thing in the way — so being reachable from elsewhere is
a deliberate act, not a side effect of binding a public address. It is also why the SSH tunnel needs
no flag: nothing about the server changed, only who can reach `localhost`.

`--http-allow-host` is repeatable, takes `host` or `host:port`, and **extends** the default rather
than replacing it, so naming a remote machine never locks out the local sessions already using the
server. And if you bind a reachable address and forget the flag, the server says so at startup and
names it, rather than leaving every remote session with a bare `403` and no explanation.

The reason any of this needs a server rather than one process per session is that reflow2's store is
single-writer **per process**. Six processes cannot each open the directory; one process holding it,
with six sessions attached, still has exactly one writer. The constraint is satisfied rather than
worked around.

> **There is no authentication.** Anything that can reach that port can write the design. Keep it on
> a private network — an SSH tunnel, a tailnet, a VPN — never a public interface. `0.0.0.0` binds
> every network the machine is on, so if one of them is untrusted, bind the specific private address
> instead.

### When one machine has to hold the design

Live sharing has a centre: the machine running the server. If it sleeps, the others lose the
design until it is back, and **neither of you can work offline**. The git route above has neither
property, which is why it stays the default for two people working independently — and why they
compose: share live while you are both at it, commit and push the export when you are done.

**Without a server**, a session that finds the graph already held gets a server serving exactly one
tool (`reflow2_unavailable`) telling it why — and the fallback is a directory each, which a git
worktree gives you naturally:
>
> ```bash
> git worktree add ../proj-seat2 -b feat/seat2
> cp .mcp.json ../proj-seat2/     # the graph path is relative, so it opens ITS OWN store
> ```
>
> Each worktree then has its own graph, and they reconcile through the committed export like any two
> people.

The MCP configs are **gitignored**, because they carry an absolute path to *your* binary — useless
on anyone else's machine. Each person (and each worktree) runs the installer once and gets their own.
If you upgraded from an older reflow2 and a config is already committed, the installer says so and
prints the fix (`git rm --cached .mcp.json`) — ignoring a tracked file changes nothing until you
untrack it.

## When something does conflict

Git tells you which files, and stops. Nothing is lost — an unresolved conflict sitting in your
working tree is a completely recoverable state.

- **Source files:** open them, resolve the marked regions, `git add`, continue the rebase.
- **The design export:** if reflow2's driver could not settle it, it stops with the specific node
  ids and prints the command that finishes the job. Never resolve the design with `--ours` or
  `--theirs`: those discard whole nodes the other person wrote.
- **If you are not sure — stop and ask each other.** A bad merge on `main` is somebody's afternoon
  deleted; an unmerged file is not.

## The one that would have bitten you — and what reflow2 does about it

Git protects you from publishing over work you have not seen: that is the non-fast-forward
refusal. The same hazard exists one level down, in the design, and it is **worse** there, for one
reason:

> **A stale export is not a conflicting export. It is a complete one.**

Your live graph is a long-lived copy of the committed design. If your partner's work reached the
file and you export from a graph that never caught up, you write a document that is internally
perfect and simply older. The merge driver finds no conflict — there is none to find — and their
requirements are gone, with nothing in the diff that looks like an error.

**reflow2 now refuses that write** (`req:stale-seat-knows`). Before replacing an export it asks
one question: *would this drop anything the file already holds?*

- The file is where you left it → written, silently. (The ordinary case, every time.)
- The file moved but nothing in it would be lost → written, and the movement is reported.
- The write would delete nodes or edges the file holds → **refused**, naming what would have gone
  and what to do instead.

So the only time it stops you is the only time you were about to lose something. When it does:

```text
REFUSED: writing this design over docs/design/reflow2.json would DELETE 2 node(s)
and 1 edge(s) that the file holds and your graph does not …
  1. git pull --rebase
  2. import_graph from that path (or compare_designs against it first)
  3. Export again; it will be a superset and go through.
```

Doing exactly that clears it. If you genuinely mean to discard their work, `accept_divergence=true`
says so out loud.

The habit that avoids the refusal altogether is still worth having: **pull before you export, and
export immediately before you commit.**

## The rules that keep this working

1. **Pull before you start. Push when you stop.** Hours, not days.
2. **Small commits.** They merge cleanly, revert cleanly, and read well later.
3. **Short-lived branches.** A branch alive for two weeks is a merge you will spend a day on.
4. **Announce before you start**, not after you finish.
5. **Never rewrite published history.** Rebasing your own unpushed work is fine; rebasing what
   your partner already pulled is how you both lose a morning.

Every failure mode here is a version of one thing: **divergence you allowed to grow.**

## Troubleshooting

| What you see | What it means | What to do |
|---|---|---|
| `push` rejected, *non-fast-forward* | They published while you worked. Working as intended. | `git pull --rebase` then push again |
| Conflict in `reflow2.json` on every pull | The merge driver is not configured on this clone | Re-run the two `git config` lines above |
| Your agent shows no reflow2 tools, or one called `reflow2_unavailable` | Usually a config without `--shared`: every session after the first is launching its own reflow2 and losing the store lock. Or the graph was written by a different reflow2 | Call that tool — it names the reason. Then check the config carries `--shared` (step 2) |
| The design in the repo does not match what your agent says | Your graph and the committed export have diverged | `compare_designs` against `docs/design/reflow2.json` |
| Your agent's skills seem out of date | They are served by the server now, so they cannot be | Update reflow2 itself; nothing in the project needs changing |
| `REFUSED: … would DELETE n node(s)` on export | Their work reached the file and your graph never caught up — reflow2 stopping real data loss | `git pull --rebase`, `import_graph` from the file, export again |
| The `--http` server will not start: the graph is held | A session is still launching its own reflow2 instead of connecting to the server | Switch every session's MCP config to the `http` URL and restart them (step 2 above) |
| Every session stalls ~30s and *then* shows `reflow2_unavailable` | Something is holding the store that is not a shared server — an older reflow2 in stdio mode, or a stray process. Sessions wait out the election, then degrade rather than hang | `--stop-shared` if it is a shared server; otherwise stop that process. Flip configs to `--shared` **after** it is gone, or the first impression of a working setup is a 30-second stall |
| A session opens the WRONG design after you copied a project | The copy carried `<graph>.server.json` with it, naming the original's server | Nothing — reflow2 refuses it and elects a server for the copy. If you see this in a log, it is the guard working |
| A remote session gets `403 Forbidden` | It dialled a host the server was not told to answer for | Add `--http-allow-host <that exact host>` and restart the server — its startup line says this too |
| A remote session cannot connect at all | The network, not reflow2 — the port is not reachable from there | Check the tunnel, LAN route or tailnet first; `curl http://<host>:8787/` from that machine |

---

## Why not a shared server on a droplet?

It is a fair question and the answer is not "never" — it is that the two halves have opposite
answers.

**Hosting the design** on one server so you both see it live genuinely works, and is on the
roadmap (`dec:central-host`): one process, many clients, still exactly one writer.

**Sharing one working copy of the code** inverts the property you are reaching for. With a single
copy there is no common ancestor, so there is no three-way merge — two people editing one file
become last-write-wins with no conflict marker, no review, and nothing to recover from. It also
shares the things git assumes are yours alone: one branch, one staging area, one build directory.
And it makes that machine a single point of failure for both of you, where today either of you can
work on a plane.

A shared live copy does not remove conflicts. It removes conflict *detection*.
