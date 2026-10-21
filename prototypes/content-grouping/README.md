# Content-grouping prototype

Throwaway, read-only command-line program for `.scratch/content-grouping/issues/01-prototype.md`. It answers one question: **if we read what's inside the loose files of a real Downloads folder, are the Suggested Folders we get ones a person would agree with, and clearly better than grouping by kind, name and date alone?**

It runs two groupers on the same files and writes one Markdown report:

- **A. Baseline (no ML):** by file kind; a big kind is split by a word shared in file names (`invoice`, `Screenshot`, `CS101`), then by month modified.
- **B. Content:**
  - **Archives, images and installers** (`.dmg`, `.pkg`, `.exe`, `.msi`, `.msix`, `.deb`, `.rpm`, `.AppImage`, `.apk`, …) are never read: each kind goes into one folder of its own, whatever the files are called.
  - **Everything else** is read: up to 2,000 characters (text of a PDF's first 3 pages through PDFium, `.docx`/`.pptx`/`.xlsx` XML, plain text and code; otherwise just the file name). A small local LLM (Qwen2.5-1.5B-Instruct, 4-bit, Vietnamese and English) reads the name and the first 1,200 characters and writes a **card**: the file's *topic* (the course, project, organization or subject it belongs to), its *type* and a one-sentence *summary*. A small multilingual embedding model (multilingual-e5-small through ONNX Runtime) embeds each card. Files with the same topic always group; otherwise they cluster by the cards' cosine similarity. Each group is matched to the most similar existing subfolder and named after its most common topic.

It is not part of the app and touches no app code.

## Run it

From this folder:

```sh
cargo run --release -- ~/Downloads
```

The first run downloads two things with the system `curl` into your user cache folder (`~/Library/Caches/files-cleaner-content-grouping/` on macOS, `%LOCALAPPDATA%\files-cleaner-content-grouping\` on Windows, `~/.cache/files-cleaner-content-grouping/` on Linux):

- the LLM, [Qwen2.5-1.5B-Instruct](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF) as 4-bit GGUF plus its tokenizer (about 1.1 GB);
- the embedding model, [Xenova/multilingual-e5-small](https://huggingface.co/Xenova/multilingual-e5-small), 8-bit ONNX (about 120 MB; an interrupted download resumes on the next run);
- PDFium from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) release `chromium/7881` (about 6 MB, unpacked with the system `tar`), for macOS, Windows and Linux on x64 or arm64.

The build downloads ONNX Runtime once. After that nothing goes over the network, and file contents never do. To use a PDFium you installed yourself, pass its library file with `--pdfium`. It must be a build compatible with chromium/7881.

The LLM is the slow stage: it writes one card per file, one file at a time, and prints `card n/N` as it goes. On a Mac it runs on the GPU through Metal (pass `--cpu` to compare); elsewhere on the CPU, where expect a few seconds per file. Existing subfolders cost `--folder-sample` cards each. Memory is about 2 GB while it runs.

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
| `--threshold <0–1>` | `0.86` | Content grouper: two groups merge while their cards' average similarity is at least this (two files with the same topic count as 1). Lower means fewer, bigger, looser groups. |
| `--min-size <N>` | `2` | Both groupers: a group with fewer files counts as Ungrouped. |
| `--anchor-threshold <0–1>` | `0.86` | Content grouper: a group is matched to the most similar existing subfolder only if their similarity is at least this; otherwise it's a "new folder". Archives, images and installers are matched to a subfolder by name instead. |
| `--folder-sample <N>` | `5` | Content grouper: most files read and carded from each existing subfolder (recursively) for its description, leaving out archives, images and installers. A subfolder with none of those is described by its name. |
| `--max-chars <N>` | `2000` | Most characters of content read per file. The LLM reads the first 1,200 of them. |
| `--split-at <N>` | `8` | Baseline: a kind with at least this many files is split by shared name word, then by month. |
| `--pdf-timeout <SECONDS>` | `10` | Give up on a PDF's text after this long (the file then counts as name only). Text comes from the first 3 pages only, read in a child process of the program that is killed at the timeout, so a slow or crashing PDF can't hold memory afterwards. PDFium reads only the cross-reference table and the objects of those pages, so a PDF's size doesn't matter. |
| `--pdfium <FILE>` | (cache) | Use this PDFium library file (`libpdfium.dylib`, `pdfium.dll`, `libpdfium.so`) instead of the downloaded one. |
| `--llm-dir <DIR>` | (cache) | Use another LLM: a folder with `model.gguf` (any Qwen2 or Qwen2.5 GGUF, e.g. the 0.5B for speed or the 3B for quality) and its `tokenizer.json`. |
| `--cpu` | off | Run the LLM on the CPU even on a Mac with Metal. |
| `--model-dir <DIR>` | (cache) | Use another embedding model: a folder with `model.onnx` (plus `model.onnx_data` if the export has one) and `tokenizer.json` from any sentence-transformers ONNX export whose output is `last_hidden_state`. |
| `--pooling <cls\|mean>` | `mean` | How token vectors become one vector. Use what the model was trained with: `cls` for bge models, `mean` for most sentence-transformers models and e5. |
| `--prefix <TEXT>` | `"query: "` for the default model, none with `--model-dir` | Put before every text the embedding model embeds. |

To try several thresholds, run it again with different values and a `--report` per run (e.g. `--report reports/threshold-0.4.md`), so the reports sit side by side. The **Ungrouped: nearest files** section of one run shows where the threshold should go.

## Reading the report

- **Header:** the Scan Root; how many loose files there were; how many had readable content, by method (PDF text, Office XML, plain text, name only); what was left out; the existing subfolders and how many files described each; the models and whether the LLM ran on the GPU; the settings; the total time and a table of time per stage.
- **A. Baseline**, then **B. Content**: one section per Suggested Folder, largest first, with
  - its suggested name;
  - **Goes to:** the existing subfolder it matches (A by name, B by similarity, shown 0–1) or "new folder";
  - **Why** (B only): its most common topics and types from the cards, each with how many of the group's files have it;
  - every file in it, with how its content was read and, in B, its card (**topic** · type · summary).
- An **Ungrouped** section per grouper.
- **Ungrouped: nearest files** (B only): for each file the content grouper left out, the start of the text it read (to spot garbled PDF text), its card, and its 3 most similar files with their similarity. When these sit just under `--threshold`, lower it. When two files clearly belong together but their cards name different topics, the LLM is the problem; when the text itself is garbled, extraction is.

Things to judge: do the groups make sense; does B clearly beat A; are the names good; are the existing-subfolder matches right; is the speed acceptable? Record the verdict under **Answer** in the issue.

Similarity numbers are cosine similarities of the normalized card embeddings, or 1 for two files with the same topic. Each model spreads them differently: e5 models score almost everything high, unrelated texts often around 0.75 to 0.8, so the useful range is narrow. The default thresholds assume multilingual-e5-small and haven't been tuned on real folders yet. With `--model-dir`, set `--threshold` and `--anchor-threshold` for that model.

## Development

- `cargo test`: tests for which items count as loose files, the fixed kinds, reading the LLM's reply, and the clustering with its same-topic rule.
- The LLM runs on candle, pinned at 0.9.2. PDFium is a prebuilt library loaded at run time (pdfium-render without its `static` feature). The `pdfium_7881` feature in `Cargo.toml` and `extract::PDFIUM_RELEASE` must name the same release. Check with:

  ```sh
  rustup target add x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
  cargo check --target aarch64-apple-darwin
  cargo check --target x86_64-pc-windows-msvc
  cargo check --target x86_64-unknown-linux-gnu
  ```
