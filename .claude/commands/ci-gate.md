---
description: Make the build fail when the design drifts
---
Use the **ci-gate** skill to set up the design gate on this project.

- Point it at the committed design export, never at the local store — the gate has to read what everyone else can see.
- Tell me what will turn the build red, and what deliberately will not.
- Explain how to make a red build green HONESTLY. If the only way past the gate is to acknowledge something, say so plainly rather than showing me how to quiet it.
