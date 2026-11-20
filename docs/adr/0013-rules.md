---
status: accepted
---

# Rules, written in plain words, shape the Plan

This supersedes ADR 0008. The app has Rules: instructions the user writes once in plain words, such as "sort by date: today, yesterday, older" or "put my files in folders by project", and that apply to every scan after that. A Rule never touches the disk itself. It only decides which Suggested Folders the Plan proposes and which Loose Files go in each, so every change still appears in the Plan for the user to approve, untick or cancel, and still runs as a journaled Session that can be undone.

The Grouping Models are small (a 1.5B describing model), so a Rule is never trusted to the model's reading of it each time:

- **The model only drafts a Rule.** It turns the user's words into a structured Rule (what files it matches, and how it groups them), with its reply held to a grammar so it always parses. The user sees the draft in plain terms, corrects it and saves it. From then on the saved Rule runs as plain code, so a misreading costs one correction, not a wrong sort at every scan. Common phrasings (by date, by kind, by name) are recognised without the model.
- **Fields are matched by code.** Dates, kinds, extensions, names and sizes are checked by code, never by the model.
- **Grouping by meaning uses embeddings first.** For a Rule such as "by project", each folder (named by the user, or proposed from the files' cards) is compared with each file's card embedding. The describing model only breaks close ties by picking from the list, the same task as placing leftovers today. A file that matches no folder clearly stays where it is.

## Considered Options

- **Keep ADR 0008 and only add a one-off instruction for each scan:** rejected because users want their way of sorting applied every time without retyping it.
- **Have the model sort each file directly from the user's words at every scan:** rejected because a 1.5B model misreads anything but simple instructions, and its mistakes would differ from scan to scan.
- **Have the model write code that sorts the files:** rejected because that code couldn't be checked or trusted.

## Consequences

- ARCHITECTURE.md's invariant stays: nothing acts on files except an approved Plan. Rules feed the Plan and don't bypass it.
- Without the Grouping Models, Rules still work, but they have to be built from the structured fields instead of from plain words, and grouping by meaning isn't available.
- Rules are edited only on their own Rules screen, never by hand in `config.json`. They're saved in the config under a new key. The old frozen `rules` value is still read and ignored, since it uses the old format.
- A Rule replaces automatic grouping for the files it matches: the Rule alone decides their folder, and automatic grouping only sorts the files no Rule matched.
- Rules only sort Loose Files into Suggested Folders. They don't trash files, and reaching into subfolders would be a separate decision.
- ADR 0011 still holds: the app doesn't learn Rules from what the user did on the Review screen.
