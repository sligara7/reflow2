---
description: Turn a folder of documents into one design
argument-hint: <the folder, and anything you know about what is in it>
---
Use the **ingest-corpus** skill on the folder below.

- Walk the whole folder in one batched pass. This is the scale sibling of capturing one document, not a loop over it.
- Report what you could NOT read — unreadable files, formats you skipped, anything truncated. A pass that silently drops a third of the corpus and reports success is the failure this skill exists to avoid.
- Tell me how much of it landed, and where I should look first at what it produced.

The folder:
$ARGUMENTS
