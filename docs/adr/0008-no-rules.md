---
status: superseded by ADR 0013
---

# No Rules

The app has no Rules, meaning user-written instructions such as `.dmg` → Trash. The user acts on files only through the Plan: duplicates, Suggested Folders, and their own changes on the Review screen. Rules were built and then frozen. This decision makes their removal final, which also closes finding C1 and step 7 of the user-control audit (#3), which suggested bringing them back.

## Consequences

- Leftover rule code is to be deleted, not revived. An old config's `rules` value is read and ignored.
- Any feature that would act on files by a standing instruction, rather than through a reviewed Plan, brings Rules back and needs this ADR reopened (see ADR 0011).
