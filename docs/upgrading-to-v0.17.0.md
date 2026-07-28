# Upgrading to v0.17.0 — nothing is locked out, and one thing needs a rebuild

> Part of the **Reflow 2.0** design docs — see **[overview.md](overview.md)** for the full map and
> reading order.

**Short version: your design opens fine, no version stamp moved, and nobody is locked out.** The one
cost is a slow first build, because the dynograph-foundation pin moved.

Unlike [v0.16.0](docs/upgrading-to-v0.16.0.md), this release adds **no node or edge types**. The
stamp counts types, not property values, so a reflow2 older than this can still open a design
written by it. Mixed versions keep working.

## The one thing that costs you time

The foundation pin moved **v0.11.0 → v0.12.0**, which forces a full `librocksdb-sys` C++ rebuild
(~10 minutes) on every machine that pulls. That is why this project's own rules forbid bumping the
pin as housekeeping — it is never free.

It was bumped for a reason worth the ten minutes, and the reason came out of a real cross-repo
exercise rather than a version-check:

- **A type leak across a published boundary is closed.** `StorageEngine::search_fulltext` used to
  return `dynograph_text::TextHit`, so reflow2 structurally depended on a type belonging to a crate
  it never names — reached only through an optional feature. A change to that type would have broken
  this build with no document on either side predicting it. v0.12.0 returns
  `dynograph_storage::FulltextHit` instead, owned by the boundary that returns it.
- **The `rocksdb` and `fulltext` Cargo feature names are now a committed contract upstream.**
  reflow2 forwards to them *by name*; previously nothing on the other side promised they would keep
  those names, so this build rested on an internal without knowing it.

**Is your existing graph safe?** Yes, and it was checked rather than assumed: v0.12.0 changes no
storage format. `keys.rs` and `backend.rs` are untouched between the tags — the only storage edits
are re-exports and the new type. A graph written by the previous foundation reads identically.

## What you gained

**A published surface can carry a promise, not just a boundary** (`set_requirement_designation`).
Designate a `Requirement` as `published` and it travels with `export_surface`. Found because a real
provider could not tell a real consumer the one thing it most needed — that a missing store fails
loud rather than silently falling back to memory. Behavioural commitments live in requirements, and
every requirement was withheld as internal, so the "published surface" carried structure and no
promises. Opt-in per requirement; undesignated intent still stays home and is still counted. A
surface with no promises now **says so**, because "none stated" must never read as "none exist".

**Declare which version of another design you depend on** (`declare_dependency`,
`reconcile_dependencies`, `reflow2.toml`). Two facts, deliberately kept apart: what you *mean* to
depend on, and what your build *actually* resolves. Comparing them answers *"am I relying on
something I never declared?"* — including build switches forwarded by name, which are contract
whether or not the provider treats them as one. The generated `reflow2.toml` records which reflow2
wrote it. Core does not parse `Cargo.toml`: you supply the observation, because one dependency can
be pinned across a Cargo manifest, a compose file and an env file at once.

**Two designs can be analysed together without either being written to**
(`compose_and_analyse`), and **an interface can publish its whole contract** (`set_interface_spec`)
— paradigm, payload format and schema, endpoint, operations, auth, transport security, error model.
Two designs cannot be checked for incompatibility unless the seam is described in comparable terms.

**The install reaches the file your agent actually reads.** Installing into a project with *no*
instruction file used to write `AGENTS.md`, report success, and stay invisible to a harness that
reads `CLAUDE.md` first. The rule is now *reach what reads* rather than *protect what exists* — and
the **eight slash commands ship with the kit**, the one narrow exception to serving everything
(`dec:commands-are-the-exception`). A command names a skill and carries no version-coupled content,
so a stale one is still correct; the single way it could rot — naming a skill that no longer exists
— is now a lint failure.

**Corroboration reads as corroboration.** A `CONTRADICTS` edge marked `alignment: supporting` is no
longer reported as a structural defect. An `opposing` edge, and one that says nothing, still are —
so no design written before this release changes meaning.

## What you have to do

Update, wait for the C++ rebuild once, and carry on. No tool lost a parameter, no node changed
meaning, and your design opens as it did.
