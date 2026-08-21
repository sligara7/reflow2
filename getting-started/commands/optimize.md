---
description: Make one thing measurably faster, against a budget written down first
---
Use the **optimize** skill on whatever I have named — a module, a command, a slow call. If I have not named one, measure first and tell me where the cost actually is before touching anything.

Follow its order, and do not skip step 4:

- **Measure before forming an opinion**, and rank by cost per unit of work rather than by total. If nothing is worth optimising, say that — it is a real answer, not a failed run.
- **Find the cause by experiment, on a copy.** Tell me which hypotheses you falsified along the way; they are as useful to me as the one that survived.
- **Write the budget down BEFORE changing any code**, as a Constraint, with the reasoning that produced the number rather than a round figure.
- **Re-measure against the budget, not against where we started** — and stop when it is met, even if the next improvement is obvious. Tell me plainly what you left undone and why.
- **Leave a guard that asserts the structure that makes it fast**, not the wall-clock time.

If a rule or an existing test refuses the change, bring it to me with the reasoning rather than weakening the guard.
