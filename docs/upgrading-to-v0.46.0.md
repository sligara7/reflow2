# Upgrading to v0.46.0

**One tool was renamed. Nothing else in this release can break you.**

## The rename

| Before | After |
| --- | --- |
| `declare_dependency` | **`external_dependency`** |

The concept is unchanged: it pins **which version of ANOTHER design** you build
against — a git tag or commit, the parts taken, the build switches forwarded by
name. `reconcile_dependencies` still compares that declaration against what the
build actually resolves.

**What breaks:** anything that names the old word — a script, a saved skill, an
agent instruction, a runbook. The call fails; nothing is silently ignored.

**What to do:** rename the call. There is no compatibility shim, deliberately —
see below.

If you use reflow2 through an agent and have never typed either name, **you have
nothing to do.** The agent reads the tool surface at connect time and will find
the new name on its own.

## Why the name moved

`Component DEPENDS_ON Component` — one part needs another — is what cycle
detection, single-point-of-failure analysis and the undeclared-seam gap are all
computed from. It was the **one structural edge with no typed helper**, so
recording a coupling meant a raw `create_edge` with both endpoint types spelled
out.

A field session went looking for it, matched `declare_dependency` **by name**,
found it was something else entirely, and fell back to the raw edge write. Since
`find_tools` ranks by name, that misdirection was systematic rather than
unlucky: the obvious word was pointing at the wrong thing for everyone who
reached for it.

So the obvious word now means the obvious thing:

```
depends_on(from_id: "cmp:coach", to_id: "cmp:stage")
```

and the cross-design pin took a name that says what it actually does.

## Why there is no deprecation shim

Keeping `declare_dependency` alive as an alias would have left the collision in
place — an agent searching by name would still hit it first, which is the whole
defect. `rule:fix-it-properly-while-it-is-still-cheap` treats "it would break
consumers" as a reason to do it **now**, before 1.0, rather than to defer; the
deprecation discipline begins at the stability commitment, and reflow2 has not
made one.

## ⚠️ Why this note exists when the rules said it was not owed

`flow:release-cut` says an upgrade note is owed **if and only if the schema stamp
moved**. It did not — measured against v0.45.0, identically 29 node types, 64
edge types, schema version 1.

But the stamp carries node and edge **type names**. It knows nothing about the
**tool surface**, which is where this release's only breaking change lives. A
consumer loses a tool from under them, and the rule that decides whether to warn
them cannot see it.

The note is written anyway. The gap in the rule is recorded rather than worked
around: the operational test a consumer can observe is not only the schema
stamp, and a future cut that renames or removes a tool owes a note whatever the
stamp says.

## Everything else in this release is additive

`depends_on`, automatic preservation of a replaced value, `stale_edge_evidence`,
`shortcut` on `list_skills`, and a sharpened parked-idea predicate. None removes
or renames anything; none requires action.

Two of them are honest about being **deferred in effect**: the parked-idea
change is inert until designs carry `Decision.kind`, and `depends_on` gives you
the call but not the modelling. Both say so in their own records rather than
reading as wins.
