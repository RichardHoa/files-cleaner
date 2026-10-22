# 01: Prototype: Suggested Folders from what's inside the files

**Type:** prototype

**What to build:** A throwaway, read-only command-line program that answers one question before any app code changes: **if the app reads what's inside the loose files in a real Downloads folder, does it come up with Suggested Folders a person would agree with, and clearly better ones than grouping by kind, name and date alone?** It uses only tools that work on macOS, Windows and Linux, since the app must ship on all three (`.scratch/basic-pipeline/map.md`).

The developer runs it on their own Downloads folder and judges the output. The agent builds it and checks it on a small fixture folder. It does not judge grouping quality itself.

**Blocked by:** None (can start immediately)

**Status:** resolved

## Hard constraints

- **Read-only.** It never creates, moves, renames or deletes anything inside the folders it reads. Its only writes are the report file, at a path given on the command line that must be outside those folders, and a model cache outside them. File contents are never sent over the network; downloading the embedding model once is allowed.
- **Throwaway.** It lives in `prototypes/content-grouping/` at the repo root, outside the app (`files-cleaner/`), and touches no app code. It's a Rust crate (`cargo run --release -- …`) so the parts that work can move into the core later. Third-party crates are fine, provided each one builds on macOS, Windows and Linux.
- **Loose items only.** It groups only items directly in the Scan Root (not inside its subfolders). Existing subfolders of the Scan Root are possible destinations, not inputs.
- Skips hidden files, OS Metadata Files, files with no extension, and Packages (anything ending in `.app`, `.pages`, `.sparsebundle`, …). It doesn't open archives.

## Pipeline

Run two groupers on the same files and report both.

**A. Baseline (no ML):** group by file kind (documents, images, installers, archives, audio, video, code, …), then split by shared name tokens (e.g. `invoice`, `Screenshot`, a course code) and by modified month where a kind group is large. Name each group from its kind and shared token.

**B. Content:**

1. **Read content** from each file, capped at about 2,000 characters:
   - PDF: text layer (e.g. `pdf-extract` or `lopdf`). No OCR.
   - `.docx`, `.pptx`, `.xlsx`: unzip in memory (`zip` crate) and strip the XML to text.
   - `.txt`, `.md`, `.csv`, source code: the first bytes, read as UTF-8.
   - Everything, including images and files with no readable content: the file name with separators turned into spaces.
2. **Embed** each file's text with a small local sentence-embedding model run through ONNX (e.g. `fastembed` with all-MiniLM-L6-v2, or `ort` directly). Report which model was used.
3. **Cluster** by cosine similarity with a simple hand-written agglomerative clustering and a similarity threshold set by a command-line flag. Groups smaller than a minimum size (flag, default 3) count as "ungrouped".
4. **Anchor to existing subfolders.** Embed the files in each existing subfolder of the Scan Root (read-only) and compute one centroid per folder. For each group, report the closest subfolder and its similarity, if it's above a threshold.
5. **Name** each group from the top keywords of its text (e.g. TF-IDF against the whole folder). No LLM.

## Output

A Markdown report at the given path:

- Header: Scan Root, loose-file count, how many had readable content (by method: PDF text, Office XML, plain text, name only), total time and time per stage, and the embedding model.
- One part per grouper (A, then B). One section per Suggested Folder, largest first: proposed name; matched existing subfolder (with similarity) or "new folder"; for B, 3–5 phrases that explain the group; every file in it, with the method its content came from.
- An "Ungrouped" section per grouper.

The program prints the report path and the timings when it's done.

## Done when

- [ ] `prototypes/content-grouping/README.md` gives the exact command to run, every flag, and how to read the report
- [ ] Running it on a fixture folder (created by the agent in its scratch space, not in the repo, with a few PDFs, a `.docx`, a `.pptx`, screenshots, some contentless files and one existing subfolder) produces the report, and the fixture is unchanged afterwards (paths, sizes and modified dates compared before and after)
- [ ] `cargo check` passes for the macOS, Windows and Linux targets (at least `cargo check --target` from a Mac)
- [ ] Nothing outside `prototypes/content-grouping/` changes in the repo
- [ ] **Developer:** run it on `~/Downloads` and record the verdict below: do the groups make sense, does B clearly beat A, are the names good, are the existing-subfolder matches right, and is the speed acceptable?

## What the verdict decides

- **B clearly beats A:** write `.scratch/content-grouping/spec.md` (Suggested Folder as a Reason in the Plan, the per-folder and per-file choices, analysis progress, caching by content hash) and an ADR on the grouping engine (cross-platform ONNX embedding in the core).
- **A is as good:** ship the baseline; same spec, no ML, no ADR.
- **Neither is good enough:** note what failed (content extraction, embeddings, clustering, or naming), and whether OCR for images or LLM naming would plausibly fix it, before spending more.

## Answer

**2026-10-22: B clearly beats A.** The content grouper with LLM cards (card → embedding → same-topic rule → clustering) ships; A is kept only as the fallback when the Grouping Models aren't installed (implemented later). The app version moves off the prototype's stack: llama.cpp runs both models (Qwen2.5-1.5B-Instruct Q4_K_M for cards, Qwen3-Embedding-0.6B Q8_0 for embeddings), C/C++ build dependencies are allowed, and the developer sets the default thresholds by hand. → `../spec.md`, ADR-0004.

## Comments

**Rewritten (2026-10-20).** Was a Swift prototype on Apple frameworks (Vision, NLEmbedding, Foundation Models) reading the whole tree. Changed to cross-platform Rust, loose items only, with a no-ML baseline to beat (`.scratch/basic-pipeline/map.md`).
