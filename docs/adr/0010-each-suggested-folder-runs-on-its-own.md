# Each Suggested Folder's moves and trashes run on their own, from the review screen

Extending ADR 0006, every Suggested Folder's panel gets its own "Move N files" and "Trash N files" buttons, and the files left where they are get their own "Trash N files"; each runs at once as its own Session. The header's Run carries out whatever hasn't run yet, together with the ticked Emptied Folders, which never run on their own because whether a folder ends up empty depends on everything else. Once a folder's moves have run, that folder is finished: it shows what was done and links to its Session, and it can no longer be renamed, merged into or dropped onto, though its files marked Trash can still be run. We chose this so users can carry out the parts of a Plan they're sure of and keep working on the rest, the same way they already handle the duplicates.

## Considered Options

- **Tick boxes on the Suggested Folders and one Run for what's ticked:** rejected. History would stay tidier, but it adds controls to a screen that's already busy, and it works differently from the Duplicates pane's button that runs immediately.
- **A finished folder stays open for more moves (its choice becoming "Add to"):** rejected. The panel would have to show a mix of files that already moved and files that are only planned.

## Consequences

- One review can leave many Sessions in History: the duplicates, each folder's moves, each folder's trashes, the trashes among the files left where they are, and the rest.
- Once anything has run, the review's Cancel becomes "Done", as leaving no longer undoes anything.
- The Plan is built with everything already run, only so the Emptied Folders are offered correctly; those Operations aren't run twice.
