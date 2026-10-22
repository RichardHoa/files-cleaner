# 03: Grouping engine and Suggested Folder planning

**What to build:** Given the loose files of a fresh scan, the app can produce Suggested Folders from what's inside the files and turn the user's choices into a Plan. There's no screen yet; this is checked with tests and a build.
- **Grouping engine:** a new crate beside `core`, ported from the content-grouping prototype onto llama.cpp:
  - It reads content as the prototype does, through PDFium bundled with the app, in a child process killed after 10 s.
  - The LLM writes a card per file (Qwen2.5-1.5B-Instruct), and the embedding model embeds the cards (Qwen3-Embedding-0.6B).
  - Cards are clustered with the same-topic rule, anchored to existing subfolders and named after their most common topic.
  - Archives, images and installers are never read: each kind forms one group.
  - Cards and embeddings are stored by content hash plus model ID.
  - It reports progress, can be cancelled, and can regroup from stored cards with new settings without running a model.
- **`core`:** gains the Suggested Folder Reason and plans each choice. Folder names are made safe on every OS, with a Clash Suffix when a name is taken.

See `../spec.md`: *Modules*, *Grouping engine*, *Core: planning Suggested Folders*. ADR-0004.

**Blocked by:** None (can start immediately). The engine takes the model file paths as input, so it doesn't wait for 02; put the two GGUFs in place by hand to try it.

**Status:** ready-for-agent

- [ ] New workspace crate; `core` gains no ML or C++ dependency
- [ ] Models behind a small interface (card for content, embedding for text); the real one runs `llama-cpp-2`, with Metal on macOS and Vulkan on Windows and Linux, falling back to the CPU when no usable Vulkan GPU or driver is found; the analysis status says which one ran
- [ ] Reading as in the prototype: PDF text from the first 3 pages through bundled PDFium in a killable child process (10 s); Office XML; plain text; otherwise the name. At most 2,000 characters
- [ ] Prototype behaviour kept: the card prompt and parsing, average-linkage clustering, the same-topic rule, anchoring to existing subfolders (5 sampled files each), fixed kinds, naming by most common topic; thresholds and smallest folder size passed in, with defaults in code (0.86 / 0.86 / 2)
- [ ] Card store keyed by content hash plus model ID; cards stored as they're made; a regroup entry point that runs no model
- [ ] Progress `{ finished, total, current }` at most every 100 ms; cancel checked between files
- [ ] `core`: a Suggested Folder Reason; create here / add to existing / create elsewhere (same-disk rule) / trash all / skip; per file, trash or take out; safe folder names (an empty name is an error, never a Trash); Clash Suffix
- [ ] Builds natively on macOS; Windows and Linux builds (with the Vulkan SDK on the build machine) checked on those OSes
- [ ] The app still starts and groups on the CPU on a machine with no Vulkan driver: the Vulkan loader is bundled on Windows, and the Linux packages depend on the system's Vulkan loader
- [ ] Test: through the engine entry point with fake models, three files on one topic plus one unrelated file give one Suggested Folder of three, named after the topic, with the fourth file left where it is
- [ ] Test: "create here" for one Suggested Folder plans one Create Folder plus a Move per file, with the Suggested Folder Reason

## Comments
