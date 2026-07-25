# Nuggets from the GitHub MCP Server

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

Review of [github/github-mcp-server](https://github.com/github/github-mcp-server) (read
2026-07-25 at `eb088dfe`): the README, all fourteen files in its `docs/`, and the HTTP, auth,
lockdown, sanitize and inventory code.

**Why this one.** Anthony asked what could be learned from a *hosted* MCP server used by an
enormous number of people, while `dec:central-host` sits open. It is the closest prior art
available: the same protocol, the same core-plus-transport shape, and — unlike everything else
reflow2 has studied — a service with real adversaries, real multi-tenancy and real operational
scars. The precedent for the exercise is the git prior-art study (2026-07-22), which produced the
whole merge thread (BL-80, `dec:merge-three-way`, `dec:merge-conflict-semantics`, rerere) out of
one focused read. Same artefact here: imports ranked most-valuable-first, then an explicit
**do NOT import** list with reasons.

**One framing that shaped the outcome.** Anthony's own reading of the larger lessons was that they
should not become a roadmap of features to build, but a rule about what *not* to do now: *do not
take a shortcut today that makes it impossible for reflow2 to become what this is.* That is
recorded as `rule:no-foreclosure` — a DesignRule, checkable at the moment of the decision that
would break it, rather than a plan that ages in a document. A roadmap goes stale; a
non-foreclosure rule fires exactly when it matters.

## Adoption table

| # | Import | Status | Where it lands |
|---|---|---|---|
| 1 | The server adds no authority of its own | **rule** | `rule:no-foreclosure` (2), `dec:central-host` |
| 2 | One codebase, two transports | **open** | `dec:central-host` |
| 3 | Scope selection by URL; per-request config may only narrow | **rule** | `rule:no-foreclosure` (4), `dec:central-host` |
| 4 | Sanitise foreign text at ingress | **BUILT 2026-07-25** | `cap:sanitize-ingress`, `cmp:sanitize` |
| 5 | Provenance-aware trust that fails closed | **open** | BL-41's M half |
| 6 | Stateless per-request construction | **rule** | `rule:no-foreclosure` (1) |
| 7 | Context budget as a first-class concern | **BUILT 2026-07-25** | `cap:bounded-reads` |
| 8 | A tool for finding tools | **BUILT 2026-07-25** | `cap:tool-search` (`find_tools`) |
| 9 | Errors typed for two audiences | **partial** | rule 4 covers the caller; the operator half is open |
| 10 | Challenges that name what is missing | **already held** | `req:survives-upgrade`, rule 4 |
| 11 | Untrusted proxy headers off by default | **rule** | `rule:no-foreclosure` |

## 1 — The server must add no authority of its own

Their stated principle, from `docs/policies-and-governance.md`:

> Users and apps cannot use an MCP server to access more resources than they could otherwise
> access normally via the API.

with *"Authentication: required for all operations, no anonymous access"* beside it. The token
arrives **per request from the caller** (`pkg/http/middleware/token.go` parses the Authorization
header into request context and nothing more); the server keeps no user table and stores no
credentials.

This is the most valuable line in the repository for reflow2, because it dissolves the problem
that looked expensive: a hosted reflow2 does not need an account system. Take a token as identity,
ask the authority that already exists — the git remote holding the design — whether that identity
may read the repo, and mirror the answer. Access control then lives where the code's access
control already lives, and the token maps onto `Contributor` for attribution. **The moment reflow2
becomes the thing that decides who may read what, it has taken on a job it cannot do as well as
the host it borrows from.**

## 2 — One codebase, two transports

The remote server *is* this repository used as a library, bound into GitHub infrastructure
(`docs/remote-server.md`). Locally the same binary serves HTTP: `github-mcp-server http` on port
8082, with `--base-url` / `--base-path` for reverse-proxy deployments
(`docs/streamable-http.md`).

For reflow2 that is an `http` subcommand beside stdio and nothing in the core. `reflow2-mcp` serves
stdio only today (`main.rs:322`, `rmcp::transport::stdio`); rmcp ships a streamable-HTTP transport
that the binary does not use.

## 3 — Scope selection by URL, and the narrowing-only rule

Each toolset is its own URL (`https://api.githubcopilot.com/mcp/x/issues`), read-only is a URL
*suffix* (`/x/issues/readonly`), and headers (`X-MCP-Toolsets`, `X-MCP-Tools`, `X-MCP-Readonly`)
adjust behaviour per request. The invariant that makes it safe is in `pkg/http/server.go:83-95`:
static configuration is an **upper bound**, per-request headers may only narrow within it, and
they *cannot re-include* what the deployment excluded.

Two things reflow2 gets from this, both bearing directly on the open questions:

- **Project selection is which URL you point at.** One process per design, its own path and token
  — no `graph_id` plumbing, no multi-tenancy, and a per-project blast radius.
- **Publishing is a `/readonly` URL**, which is exactly the separation
  `req:design-reachable-without-the-repo` asks for: reach without write.

And the invariant is the load-bearing part: **a read-only surface a header can talk out of
read-only was never read-only.**

## 4 — Sanitise foreign text at ingress ✅ built

`pkg/sanitize/sanitize.go` strips, from any user-authored title or body: invisible and
bidirectional control characters, Unicode tag characters, HTML tags, and hidden metadata in code
fence info strings.

reflow2's standing rule — *graph text is data, never instructions* — was enforced only by prose in
the skills and the server handshake: a rule addressed to a well-behaved reader. This is its
mechanical half, and it is BL-41's deferred M piece arriving with a reference implementation.

**Built** as `crates/reflow2-core/src/sanitize.rs`, wired into INGEST's single integration choke
point, with two deliberate departures:

- **It reports.** GitHub sanitises silently, which is right for rendering an issue body. A design
  brain that quietly rewrote a requirement statement would be unauditable, and rule 6 forbids
  silent drops — so the class and count of what was removed lands in `IngestReport.warnings`,
  naming the node and the field.
- **No HTML stripping.** reflow2 renders no HTML, and a design may legitimately contain
  `Vec<Component>` or `a < b`. Stripping tags would corrupt honest content to defend against a
  risk this project does not have.

Zero-width joiner is kept (it is load-bearing in emoji sequences) — the source makes the same call,
and a filter that mangles real text is one people turn off.

Not done yet, named rather than left implicit: code-fence metadata, and the **import** path.
`import_graph` deliberately does not rewrite the document it is given, because an export's content
is its identity; detecting and *reporting* on import is the next rung.

## 5 — Provenance-aware trust, failing closed

Lockdown mode (`pkg/lockdown/`, `pkg/github/lockdown.go`) hides issue and PR content authored by
users **without push access** to the repository — trust keyed on the author's authority, cached
with a TTL. The comment that matters:

> It fails closed: a missing cache, an empty author, or a lookup error denies access.

Two imports. The **trust model**, which reflow2 already has the vocabulary for (`provenance` plus
`Contributor`) and does not yet use: a claim inherits the standing of whoever made it. And the
**failure direction**, which is the sharper half — see the do-not-import list below, because this
same repository fails *open* elsewhere.

It also names a multi-tenancy trap out loud, worth carrying into any hosted reflow2:

> In HTTP mode each request must construct its own instance so viewer-scoped lookups run under the
> requesting user's credentials.

Cache per request, or one person's authority silently answers another person's question.

## 6 — Stateless per-request construction

A new MCP server is constructed per request in stateless mode (`pkg/http/handler.go:110`). reflow2
cannot do that for the store — one RocksDB handle, exclusive lock — and does not need to: one
process holding the lock and serialising tool calls is *safer* for multi-user than many processes
contending. What must follow the request rather than the process is everything derived from the
caller: identity, permissions, which design, read-only-ness.

## 7 — Context budget is a first-class concern ✅ built

Three things they do that reflow2 did not:

- Server instructions are **generated from the enabled toolsets** (`pkg/inventory/instructions.go`)
  and contain an explicit *Context management* section: paginate in batches of 5–10, pass
  `minimal_output` when the full record is not needed.
- Result types carry **field-selection enums**, with comments naming the heaviest fields so the
  caller knows which ones to drop (`pkg/github/minimal_types.go`).
- `search_*` tools default `minimal_output` to **true**.

This landed on a live wound: on 2026-07-25 `scan_nodes` over 72 Decisions returned 96,000
characters and the client truncated it — a real drop, silent, and happening *outside* reflow2
where nothing could name it. The precedent for the shape was already in this design (BL-48/BL-49
gave `propagate` a bounded summary with the full dump behind `full=true`).

**Built:** `scan_nodes` now answers with as many nodes as fit in one reply and says what it left
out — `total`, `returned`, `omitted`, `next_offset`, and `capped_by` (`size` or `limit`) — plus
`brief: true` for id/name/status only. A cap is allowed; an unnamed one is not. `count` keeps its
old meaning (`items.len()`) so existing callers are untouched, and a single node larger than the
whole budget is still returned, because an unreachable node is a silent drop by another name.

## 8 — A tool for finding tools ✅ built

`pkg/tooldiscovery/search.go` fuzzy-searches its own tool list with weighted scoring (name matches
strongest, then description, then parameters) and returns three results by default.

reflow2 serves 97 tools. `req:agent-native` promises every capability is reachable from a coding
agent over one surface — which is only true in practice if the agent can *find* the tool: a
surface too large to hold in context is reachable the way a library with no catalogue is readable.

**Built** as `find_tools`, scored over `tool_router.list_all()` — the served surface itself, so it
cannot drift from the tools that exist — with trimmed summaries, ties broken by name for
determinism, and `matched` / `omitted` / `searched` reported.

## 9 — Errors typed for two audiences

`docs/error-handling.md` splits deliberately: **user-actionable** failures (auth, rate limit, 404)
come back as failed tool calls, while **developer** errors bubble up as real errors; and every API
error is also stashed in request context so middleware can inspect *what kind* of failure occurred
without logging PII (`errors.Is` rather than string matching).

reflow2's rule 4 already demands loud, actionable failure at the caller. The operator half — being
able to ask "what kind of failures is this server producing?" without a debugger — is new, and only
becomes necessary the day something runs unattended.

## 10 — Challenges that name what is missing

A missing token gets 401 with a `WWW-Authenticate` header pointing at the resource metadata;
insufficient scope gets 403 naming the scopes required (`docs/streamable-http.md`,
`pkg/http/middleware/scope_challenge.go`). Same doctrine as reflow2's *refused loudly, with what to
do* — worth noting as convergent evidence rather than a new idea.

## 11 — Untrusted proxy headers off by default

`X-Forwarded-Host` / `X-Forwarded-Proto` are **ignored** unless `--trust-proxy-headers` is passed,
because otherwise a client can influence the URL the server advertises about itself
(`docs/streamable-http.md`). The general form — never let a caller supply the facts you will
publish as your own identity — belongs in `rule:no-foreclosure`.

## Explicitly do NOT import

**The identity stack: OAuth apps, GitHub App installations, SSO enforcement, enterprise policy
layers** (`docs/oauth-login.md`, `docs/github-app-auth.md`, `docs/policies-and-governance.md`).
This machinery exists because GitHub crosses organisational boundaries for a very large number of
people. reflow2's population is two. A private tailnet plus a bearer token is not a cut-down
version of this — it is the correct size, and importing the rest means building an identity
provider so that two brothers can coordinate.

**`Access-Control-Allow-Origin: *`** (`pkg/http/middleware/cors.go:31`). Right for a public API
called from browsers; wrong for a private design brain. Their CORS defaults are not reflow2's.

**Silently ignoring unknown toolsets.** `docs/remote-server.md`: *"Invalid or unknown toolsets are
silently ignored without error"* — while the sibling `X-MCP-Tools` header errors on an invalid
tool. Their own surface is inconsistent; reflow2 takes the strict branch for both, because the
lenient one is a straight violation of `req:no-silent-fallback`.

**Scope filtering's graceful degradation** (`docs/scope-filtering.md`):

> If the server cannot fetch your token's scopes … it logs a warning and continues **without
> filtering**.

**This is the sharpest finding in the study.** The same repository fails *closed* in lockdown and
*open* here — and the difference is instructive rather than sloppy: behind the scope filter, the
GitHub API still enforces permissions, so the filter is only decluttering, and failing open costs
nothing. A hosted reflow2 delegating authority (import 1) has a second enforcement layer too, and
the day it does not, this pattern becomes a hole. **The rule to carry: fail open only where a
second enforcement layer provably exists, and name which layer it is.**

**Per-tool OAuth scope taxonomies in the tool descriptions.** reflow2's authority model would be
per-design, not per-operation; a scope matrix would be ceremony over a fact one check answers.

**The observability stack, feature flags and insiders mode** (`pkg/observability/`,
`docs/feature-flags.md`, `docs/insiders-features.md`). Multi-tenant service furniture with a
release train behind it. reflow2's equivalent of a feature flag is a Decision node, and its
equivalent of a rollout is a release.

## What this changed on the record

- `rule:no-foreclosure` — the DesignRule holding six named shortcuts not to take, `enforced: false`
  pending Anthony's call on whether it should be gate-blocking.
- `req:trust-boundary-at-ingress` (`proposed`) → `cap:sanitize-ingress` → `cmp:sanitize`.
- `cap:bounded-reads` and `cap:tool-search`, satisfying `req:no-silent-fallback` and
  `req:agent-native` respectively.
- `dec:central-host` gains this study as its evidence: imports 1, 2 and 3 make the hybrid road
  concrete and take authentication off its critical path.
