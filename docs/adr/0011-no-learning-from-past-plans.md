# No learning from past Plan decisions

The app doesn't remember what the user decided on the Review screen (renames, destinations, merges, removals, trashes) to shape later Plans. Most of that memory already lives on disk. A file the user moved into a folder inside the Scan Root makes that folder an existing subfolder at the next scan, and existing subfolders are already read and offered as somewhere files can go. Learning would only add something for renamed Suggested Folders and for destinations outside the Scan Root. That is too little to justify another store and matching step that users can't see. This closes issue #12, which took the idea from AI File Sorter's "learning from approved reviews".

## Considered Options

- **Remember renames and outside destinations of Suggested Folders that ran**, matched to new clusters by the centroid of their files' embeddings and shown as "Like last time": rejected as not worth building, for the reason above.
- **Also learn trash decisions** ("content like this gets trashed"): rejected because it would bring Rules back (ADR 0008).
- **Learn merges and removals as signals to clustering:** rejected because they would change the Suggested Folders in ways the user can't see or explain.

## Consequences

- Anyone who reopens this should start from what the disk doesn't already remember, and should never learn trash decisions.
