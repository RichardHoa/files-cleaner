# Content-grouping prototype

Throwaway, read-only command-line program for `.scratch/content-grouping/issues/01-prototype.md`. It answers one question: **if we read what's inside the loose files of a real Downloads folder, are the Suggested Folders we get ones a person would agree with, and clearly better than grouping by kind, name and date alone?**

It runs two groupers on the same files and writes one Markdown report:

- **A. Baseline (no ML):** by file kind; a big kind is split by a word shared in file names (`invoice`, `Screenshot`, `CS101`), then by month modified.
- **B. Content:** reads up to 2,000 characters from each file (PDF text layer, `.docx`/`.pptx`/`.xlsx` XML, plain text and code, otherwise just the file name), embeds it with a local sentence-embedding model (all-MiniLM-L6-v2 through ONNX Runtime), clusters by cosine similarity, matches each group to the most similar existing subfolder, and names it from its TF-IDF keywords.

It is not part of the app and touches no app code.

## Run it

From this folder:

```sh
cargo run --release -- ~/Downloads
```

The first run downloads the model (about 90 MB) with the system `curl` into your user cache folder (`~/Library/Caches/files-cleaner-content-grouping/` on macOS, `%LOCALAPPDATA%\files-cleaner-content-grouping\` on Windows, `~/.cache/files-cleaner-content-grouping/` on Linux), and the build downloads ONNX Runtime once. After that nothing goes over the network, and file contents never do.

The report is written to `reports/grouping-report.md` in this folder; each run replaces it. `reports/` is gitignored, since the report lists your own file names. When it's done it prints the report path and how long each stage took.

## What it never does

- It never creates, moves, renames or deletes any of the files it reads. Every one is opened read-only. Its only writes are the report and the model cache. Any folder can be the Scan Root, including one that holds those two (this repo, your home folder): the report is written after the analysis, and it prints a note when either write lands inside the Scan Root.
- It groups only files directly in the Scan Root. Existing subfolders are possible destinations: up to `--folder-sample` files are read from each one (recursively) to describe it.
- It leaves out hidden files, OS Metadata Files (`.DS_Store`, `Thumbs.db`, `desktop.ini`, …), files with no extension, Packages (`.app`, `.pages`, `.sparsebundle`, … folders), symlinks and the Holding Folder. It doesn't open archives, and has no OCR.

## Flags

| Flag | Default | What it does |
|---|---|---|
| `<SCAN_ROOT>` | (required) | The folder to analyse: any folder. |
| `--report <PATH>` | `reports/grouping-report.md` here | Where to write the Markdown report. Its folder must exist; it can't be a symlink. |
| `--threshold <0–1>` | `0.5` | Content grouper: two groups merge while their average cosine similarity is at least this. Lower means fewer, bigger, looser groups. |
| `--min-size <N>` | `3` | Both groupers: a group with fewer files counts as Ungrouped. |
| `--anchor-threshold <0–1>` | `0.45` | Content grouper: a group is matched to the most similar existing subfolder only if their similarity is at least this; otherwise it's a "new folder". |
| `--folder-sample <N>` | `40` | Content grouper: most files read from each existing subfolder for its description. An empty subfolder is described by its name. |
| `--max-chars <N>` | `2000` | Most characters of content read per file. The model sees about the first 256 tokens (roughly 1,000 characters) in any case. |
| `--split-at <N>` | `8` | Baseline: a kind with at least this many files is split by shared name word, then by month. |
| `--pdf-timeout <SECONDS>` | `10` | Give up on a PDF's text after this long (the file then counts as name only). |
| `--max-pdf-mb <MB>` | `100` | Don't read text from bigger PDFs. |
| `--model-dir <DIR>` | (cache) | Use another model: a folder with `model.onnx` and `tokenizer.json` from any sentence-transformers ONNX export whose output is `last_hidden_state`, for example a multilingual one for non-English files. |

To try several thresholds, run it again with different values and a `--report` per run (e.g. `--report reports/threshold-0.3.md`), so the reports sit side by side. Reading and embedding take seconds, so re-runs are cheap.

## Reading the report

- **Header:** the Scan Root; how many loose files there were; how many had readable content, by method (PDF text, Office XML, plain text, name only); what was left out; the existing subfolders and how many files described each; the model; the settings; the total time and a table of time per stage.
- **A. Baseline**, then **B. Content**: one section per Suggested Folder, largest first, with
  - its suggested name;
  - **Goes to:** the existing subfolder it matches (A by name, B by similarity, shown 0–1) or "new folder";
  - **Why** (B only): its top keywords and word pairs, each with how many of the group's files contain it;
  - every file in it, with how its content was read.
- An **Ungrouped** section per grouper.

Things to judge: do the groups make sense; does B clearly beat A; are the names good; are the existing-subfolder matches right; is the speed acceptable? Record the verdict under **Answer** in the issue.

Similarity numbers are cosine similarities of normalized MiniLM embeddings. Near-identical texts (screenshots with only a time in the name, invoices from one sender) score above 0.8. Clearly related documents often score 0.3 to 0.6. Unrelated files score below 0.2. A file read by name only has a short text, so it can land close to anything else that shares a word or number in its name.

## Development

- `cargo test`: tests for which items count as loose files.
- Every dependency builds on macOS, Windows and Linux without a C compiler for the target. That's why the crate uses `ort` and `tokenizers` directly instead of fastembed, which forces the C regex library oniguruma and an HTTPS stack built on `ring`. Check with:

  ```sh
  rustup target add x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
  cargo check --target aarch64-apple-darwin
  cargo check --target x86_64-pc-windows-msvc
  cargo check --target x86_64-unknown-linux-gnu
  ```
