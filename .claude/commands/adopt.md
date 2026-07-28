---
description: Bring a system that already exists under reflow2 design control
argument-hint: <optional — what to start from, or which part to cover first>
---
Use the **adopt** skill to bring this existing system under design control.

If the reflow2 design tools are not served in this session and `reflow2_start_design` is, this directory has not opted into a design yet — call that tool first and follow the one step it returns. Do not report reflow2 as missing or broken; it is installed, and this is what a directory with no design looks like.

- Work backwards from what is actually there — code, docs, tests, whatever exists. Recover intent; don't design what you wish it were.
- Mark inferred intent as inferred. A requirement you read out of code is not one I stated, and I need to see which is which.
- Tell me plainly what you could NOT determine from the artifacts alone. Those are the questions only I can answer, and they are the point of the pass.
- Say how much of the system this pass actually covered. A pass over a third of the repo that reports "no gaps" is worse than no pass at all.

If this is a new project with no code yet, say so and use the **genesis** skill instead — that is the other starting point, and it works forwards from a brief.

Where to start:
$ARGUMENTS
