---
status: accepted
---

# When Rules overlap, the first one wins and the user is warned

Two Rules can match the same Loose File and send it to different Suggested Folders, for example "PDFs into Documents" and "names containing invoice into Invoices" for `invoice-march.pdf`. Such a file goes where the first enabled Rule in the list that matches it says. No later Rule sees it, so a file is never in two folders and never left out because of the overlap.

Saving overlapping Rules is allowed. The Rules screen marks each Rule that some file could share with an enabled Rule above it, and the mark's tooltip names the earlier Rule that wins. Whether two Rules overlap is worked out from their fields alone, not from the files of a scan: they're kept apart only when a field rules out every file one of them matches (different kinds or extensions, size or date ranges that don't meet, names that can't both fit, or a word one leaves out that every name the other matches contains). Otherwise they're taken to overlap. So the warning can show for Rules that no real file shares, but it never misses a real overlap.

## Considered Options

- **Reject a Rule that overlaps an existing one, so every file fits at most one Rule:** rejected because names almost always can overlap ("contains invoice" and "is a PDF"), so a broad Rule would block nearly every narrower one until the user adds exceptions to it. It's too strict to be pleasant.
- **Put a file in every folder whose Rule matches it:** rejected because a file can only be moved to one place.
- **Let the user drag Rules into order, so they pick which one wins:** deferred. New Rules go to the end of the list, so older Rules win until the user moves one up or down.

## Consequences

- The order of the Rules list is their priority. New Rules go to the end, and the user moves a Rule up or down on the Rules screen to change which one wins.
- The first-match behaviour lives in the grouping crate alone, and the overlap check sits next to it, so both follow the same matching.
- Disabled Rules match nothing, so they neither cause nor get the warning, and switching one back on brings it back.
