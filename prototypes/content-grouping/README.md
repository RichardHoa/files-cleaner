# Content-grouping prototype

Throwaway, read-only command-line program for `.scratch/content-grouping/issues/01-prototype.md`. It answers one question: **if we read what's inside the loose files of a real Downloads folder, are the Suggested Folders we get ones a person would agree with, and clearly better than grouping by kind, name and date alone?**

It runs two groupers on the same files and writes one Markdown report:

- **A. Baseline (no ML):** by file kind; a big kind is split by a word shared in file names (`invoice`, `Screenshot`, `CS101`), then by month modified.
- **B. Content:** reads up to 2,000 characters from each file (text of a PDF's first 3 pages through PDFium, `.docx`/`.pptx`/`.xlsx` XML, plain text and code, otherwise just the file name). It embeds the name and the content separately with a local multilingual embedding model (bge-m3 through ONNX Runtime) and averages the two vectors. It then clusters by cosine similarity, adding a bonus when two names share a word, matches each group to the most similar existing subfolder, and names it from its TF-IDF keywords.

It is not part of the app and touches no app code.

## Run it

From this folder:

```sh
cargo run --release -- ~/Downloads
```

The first run downloads two things with the system `curl` into your user cache folder (`~/Library/Caches/files-cleaner-content-grouping/` on macOS, `%LOCALAPPDATA%\files-cleaner-content-grouping\` on Windows, `~/.cache/files-cleaner-content-grouping/` on Linux):

- the model, [Xenova/bge-m3](https://huggingface.co/Xenova/bge-m3) (about 2.3 GB; an interrupted download resumes on the next run);
- PDFium from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) release `chromium/7881` (about 6 MB, unpacked with the system `tar`), for macOS, Windows and Linux on x64 or arm64.

The build downloads ONNX Runtime once. After that nothing goes over the network, and file contents never do. To use a PDFium you installed yourself, pass its library file with `--pdfium`. It must be a build compatible with chromium/7881.

The model takes about 2.3 GB of memory while it runs, and embedding is slower than with a small model: expect a few files per second on a laptop CPU.

The report is written to `reports/grouping-report.md` in this folder; each run replaces it. `reports/` is gitignored, since the report lists your own file names. When it's done it prints the report path and how long each stage took.

## What it never does

- It never creates, moves, renames or deletes any of the files it reads. Every one is opened read-only. Its only writes are the report and the download cache (the model and PDFium). Any folder can be the Scan Root, including one that holds those two (this repo, your home folder): the report is written after the analysis, and it prints a note when either write lands inside the Scan Root.
- It groups only files directly in the Scan Root. Existing subfolders are possible destinations: up to `--folder-sample` files are read from each one (recursively) to describe it.
- It leaves out hidden files, OS Metadata Files (`.DS_Store`, `Thumbs.db`, `desktop.ini`, …), files with no extension, Packages (`.app`, `.pages`, `.sparsebundle`, … folders), symlinks and the Holding Folder. It doesn't open archives, and has no OCR.

## Flags

| Flag | Default | What it does |
|---|---|---|
| `<SCAN_ROOT>` | (required) | The folder to analyse: any folder. |
| `--report <PATH>` | `reports/grouping-report.md` here | Where to write the Markdown report. Its folder must exist; it can't be a symlink. |
| `--threshold <0–1>` | `0.5` | Content grouper: two groups merge while their average similarity (cosine plus name bonus) is at least this. Lower means fewer, bigger, looser groups. |
| `--min-size <N>` | `2` | Both groupers: a group with fewer files counts as Ungrouped. |
| `--anchor-threshold <0–1>` | `0.5` | Content grouper: a group is matched to the most similar existing subfolder only if their similarity is at least this; otherwise it's a "new folder". |
| `--name-weight <0–1>` | `0.5` | Content grouper: share of a file's vector that comes from its name; the rest comes from its content. A file read by name only gets just its name's vector. |
| `--name-bonus <N>` | `0.1` | Content grouper: added to two files' similarity when their names share a word of 3+ letters that isn't a stopword or a number (`IELTS`, `CS101`, `invoice`). `0` turns it off. |
| `--folder-sample <N>` | `40` | Content grouper: most files read from each existing subfolder for its description. An empty subfolder is described by its name. |
| `--max-chars <N>` | `2000` | Most characters of content read per file. The model sees about the first 256 tokens (roughly 1,000 characters) in any case. |
| `--split-at <N>` | `8` | Baseline: a kind with at least this many files is split by shared name word, then by month. |
| `--pdf-timeout <SECONDS>` | `10` | Give up on a PDF's text after this long (the file then counts as name only). Text comes from the first 3 pages only, read in a child process of the program that is killed at the timeout, so a slow or crashing PDF can't hold memory afterwards. PDFium reads only the cross-reference table and the objects of those pages, so a PDF's size doesn't matter. |
| `--pdfium <FILE>` | (cache) | Use this PDFium library file (`libpdfium.dylib`, `pdfium.dll`, `libpdfium.so`) instead of the downloaded one. |
| `--model-dir <DIR>` | (cache) | Use another model: a folder with `model.onnx` (plus `model.onnx_data` if the export has one) and `tokenizer.json` from any sentence-transformers ONNX export whose output is `last_hidden_state`. |
| `--pooling <cls\|mean>` | `cls` for the default model, `mean` with `--model-dir` | How token vectors become one vector. Use what the model was trained with: `cls` for bge models, `mean` for most sentence-transformers models and e5. |
| `--prefix <TEXT>` | (none) | Put before every text the model embeds, e.g. `"query: "` for e5 models. |

To try several thresholds, run it again with different values and a `--report` per run (e.g. `--report reports/threshold-0.4.md`), so the reports sit side by side. The **Ungrouped: nearest files** section of one run shows where the threshold should go.

## Reading the report

- **Header:** the Scan Root; how many loose files there were; how many had readable content, by method (PDF text, Office XML, plain text, name only); what was left out; the existing subfolders and how many files described each; the model; the settings; the total time and a table of time per stage.
- **A. Baseline**, then **B. Content**: one section per Suggested Folder, largest first, with
  - its suggested name;
  - **Goes to:** the existing subfolder it matches (A by name, B by similarity, shown 0–1) or "new folder";
  - **Why** (B only): its top keywords and word pairs, each with how many of the group's files contain it;
  - every file in it, with how its content was read.
- An **Ungrouped** section per grouper.
- **Ungrouped: nearest files** (B only): for each file the content grouper left out, the start of the text it read (to spot garbled PDF text) and its 3 most similar files with their similarity. When these sit just under `--threshold`, lower it. When they are low even for files that clearly belong together, the model or the extracted text is the problem, not the threshold.

Things to judge: do the groups make sense; does B clearly beat A; are the names good; are the existing-subfolder matches right; is the speed acceptable? Record the verdict under **Answer** in the issue.

Similarity numbers are cosine similarities of normalized embeddings, plus the name bonus. Each model spreads them differently. bge-m3 scores even unrelated texts around 0.3 to 0.45, related documents around 0.55 to 0.75, and near-identical ones (screenshots with only a time in the name, invoices from one sender) above 0.85. The default thresholds assume bge-m3 and haven't been tuned on real folders yet. With `--model-dir`, set `--threshold` and `--anchor-threshold` for that model.

## Development

- `cargo test`: tests for which items count as loose files, and for the clustering and the name bonus.
- Every dependency builds on macOS, Windows and Linux without a C compiler for the target. That's why the crate uses `ort` and `tokenizers` directly instead of fastembed, which forces the C regex library oniguruma and an HTTPS stack built on `ring`. PDFium is a prebuilt library loaded at run time (pdfium-render without its `static` feature). The `pdfium_7881` feature in `Cargo.toml` and `extract::PDFIUM_RELEASE` must name the same release. Check with:

  ```sh
  rustup target add x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
  cargo check --target aarch64-apple-darwin
  cargo check --target x86_64-pc-windows-msvc
  cargo check --target x86_64-unknown-linux-gnu
  ```
