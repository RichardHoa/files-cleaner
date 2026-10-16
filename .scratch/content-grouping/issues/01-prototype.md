# 01: Prototype: group files by what's inside them

**Type:** prototype

**What to build:** A throwaway, read-only command-line script that answers one question before any app code changes: **if the app reads what's inside the files in a real Downloads folder, does it come up with groups a person would agree with?** Static Rules (extension and name glob) were judged too mechanical to be worth much. The direction is to group files by their content and propose where each group goes, preferably into folders the user already has. This prototype measures whether that's good enough to build.

The developer runs it on their own Downloads folder and judges the output. The agent builds it and checks it on a small fixture folder. It does not judge grouping quality itself.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

## Hard constraints

- **Read-only.** It never creates, moves, renames or deletes anything inside the folders it reads. Its only write is the report file, at a path given on the command line that must be outside those folders. Nothing is sent over the network (spec: "No cloud uploading of file contents").
- **Throwaway.** It lives in `prototypes/content-grouping/` at the repo root, outside the app (`files-cleaner/`), and touches no app code. Swift is the suggested language, since it reaches Vision, PDFKit, NaturalLanguage and Foundation Models directly. Run it with `swift run` or a single `swift` script. No third-party packages.
- Skips hidden files, OS Metadata Files, and Packages (anything ending in `.app`, `.pages`, `.sparsebundle`, …, or that macOS reports as a package). It doesn't open archives.

## Pipeline

1. **Read content** from each file (recursively under the Scan Root), capped at about 2,000 characters per file:
   - PDF: text layer via PDFKit. If there isn't one, OCR the first page.
   - `.docx`, `.pptx`, `.xlsx`: unzip in memory, or with `/usr/bin/unzip -p` writing to stdout, and strip the XML to text.
   - `.txt`, `.md`, `.csv`, source code: the first bytes, read as UTF-8.
   - Images (png, jpg, heic, …): OCR with Vision (`VNRecognizeTextRequest`).
   - Everything, including files with no readable content: the file name with separators turned into spaces, plus its parent folder's name.
2. **Embed** each file's text with the NaturalLanguage framework (`NLEmbedding.sentenceEmbedding`, or `NLContextualEmbedding` averaged over tokens; report which one was used).
3. **Cluster** by cosine similarity with a simple hand-written agglomerative clustering and a similarity threshold set by a command-line flag. Groups smaller than a minimum size (flag, default 3) count as "ungrouped".
4. **Anchor to existing folders.** Optional `--existing <dir>` (repeatable, e.g. `~/Documents`): embed the files in each subfolder there, read-only, and compute one centroid per folder. For each group, report the closest existing folder and its similarity, if it's above a threshold.
5. **Name** each group. Use Apple's on-device Foundation Models (`LanguageModelSession`) once per group, given the group's file names and short snippets, asking for a 1–4 word folder name. If the model is unavailable, fall back to the top keywords of the group's text, and say in the report which was used.

## Output

A Markdown report at the given path:

- Header: Scan Root, file count, how many had readable content (by method: text layer, OCR, Office XML, name only), total time and time per stage, and which embedding and naming method was used.
- One section per group, largest first: proposed name; matched existing folder (with similarity) or "new folder"; 3–5 phrases that explain the group; every file in it, with its relative path and the method its content came from.
- An "Ungrouped" section listing the rest.

The script prints the report path and the timings when it's done.

## Done when

- [ ] `prototypes/content-grouping/README.md` gives the exact command to run, every flag, and how to read the report
- [ ] Running it on a fixture folder (created by the agent in its scratch space, not in the repo, with a few PDFs, a `.docx`, a `.pptx`, screenshots with text, and some contentless files) produces the report, and the fixture is unchanged afterwards (paths, sizes and modified dates compared before and after)
- [ ] It works when Foundation Models is unavailable (keyword names), and the report says so
- [ ] Nothing outside `prototypes/content-grouping/` changes in the repo
- [ ] **Developer:** run it on `~/Downloads` (with `--existing ~/Documents`) and record the verdict below: do the groups make sense, are the names good, are the existing-folder matches right, and is the speed acceptable?

## What the verdict decides

- **Groups make sense:** write `.scratch/content-grouping/spec.md` (content groups as a new Reason in the Plan, analysis progress, caching by content hash) and an ADR on on-device models: Apple frameworks behind a swappable interface vs. cross-platform ONNX/llama.cpp. That ADR unblocks ticket `milestone-1/06` (Windows/Linux port).
- **They don't:** note what failed (content extraction, embeddings, clustering, or naming) and whether a stronger local model would plausibly fix it, before spending more.

## Answer

_(developer's verdict goes here)_
