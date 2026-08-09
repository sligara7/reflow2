---
description: Record a rule the project follows — and decide whether breaking it should stop the build
argument-hint: <the rule, in your own words — "we always branch before pushing", "never edit generated files">
---
Use the **governance-proposal** skill on what I've written below.

- Ask me the consequence question before recording anything: if somebody broke this, should the build have stopped them, or is it advice? Do not answer it for me.
- Write my answer explicitly either way. If I don't give you one, leave `enforced` off rather than guessing — absent means "nobody has said" and reflow2 will ask me; it does not silently mean gate-blocking any more.
- If I say it should stop the build, tell me plainly that it now owes a detector, and do not attach a check just to close the finding.
- Bind it to what it actually governs if there is anything to bind; if there isn't, leave it unbound and tell me that's what you did.
- When done, tell me in plain language what the rule now says, whether it can fail a build, and what would have to exist to catch a violation — no raw ids.

The rule:
$ARGUMENTS
