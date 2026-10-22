# Suggested Folders from what's inside the files

Status: ready-for-agent

## Problem Statement

After the duplicate step, the app rescans my Scan Root and takes me to the Suggested Folders step, and that step is still a placeholder. My Downloads folder is full of loose PDFs, slides, documents and installers, and I have to sort them by hand. Their names often don't say what they're about ("T1T2-2706.pdf", "P.Process cho HS.pdf"), so sorting by kind, name and date puts unrelated files together and splits related ones apart. The content-grouping prototype showed that reading inside the files gives folders I'd actually agree with. The app doesn't do it yet.

Changing settings is also awkward: the only way is to edit a JSON file in an outside editor.

## Solution

The Suggested Folders step reads the loose files in the Scan Root on this computer and suggests folders for them:
- A small local model writes a short card for each file: the course, project or organization it belongs to, what kind of document it is, and one sentence about it.
- A second model compares the cards. Files with the same topic or very similar cards become one Suggested Folder, named after their most common topic, in the files' own language.
- A Suggested Folder that matches an existing subfolder goes into that subfolder instead of creating a new one.
- Images, installers and archives are never opened. Each kind is gathered into one folder of its own.

The Suggested Folders form a Plan like any other: I can rename, redirect, trash or skip each folder, take single files out, and approve or cancel. Running it is its own Session, undoable from History.

The two models (the **Grouping Models**) aren't in the installer. The first time the app opens, it asks whether to download them (about 1.8 GB) and says plainly that nothing leaves the computer. If I say no, I can install them later. Until I do, the Suggested Folders step offers the download and says that sorting by kind, name and date is coming.

A new Settings screen lets me change the Trash Destination, the Never-Touch Paths, how strict the grouping is, and the Grouping Models, each explained in plain words. The JSON file is still there for anyone who wants it.

## User Stories

### First open and the Grouping Models

1. As a user opening the app for the first time, I want to be asked whether to download the Grouping Models, so that nothing big downloads without my consent.
2. As a user, I want the offer to state the download size, so that I can judge whether I have the space and bandwidth.
3. As a user, I want the offer to say my files never leave my computer, so that I'm comfortable letting the app read them.
4. As a user, I want "Not now" to be respected and not asked again at every launch, so that the app doesn't nag me.
5. As a user, I want the download to run in the background, so that I can scan and remove duplicates while it finishes.
6. As a user, I want to see the download's progress, so that I know how long it will take.
7. As a user, I want to pause and resume the download, so that I can free my connection for something else.
8. As a user, I want to cancel the download, so that I can change my mind completely.
9. As a user, I want an interrupted download to resume where it stopped, so that a dropped connection doesn't cost me the whole 1.8 GB again.
10. As a user, I want a damaged or incomplete download never to count as installed, so that grouping doesn't fail in strange ways.
11. As a user without enough free disk space, I want to be told before the download starts, so that it doesn't fail halfway.
12. As a user who said "Not now", I want to install the models from the Suggested Folders step, so that I can change my mind when I need them.
13. As a user, I want to install the models from Settings as well, so that I can find the option when I go looking for it.
14. As a user, I want to remove the models from Settings, so that I can get 1.8 GB back.
15. As a user who reaches the Suggested Folders step while the models are still downloading, I want to see that download's progress there, so that I know to wait.

### Analysing the loose files

16. As a user, I want only the items directly in the Scan Root grouped, so that folders I've already organized aren't pulled apart.
17. As a user, I want Protected Paths, hidden files, OS Metadata Files, files with no extension and Packages left out, as everywhere else in the app.
18. As a user, I want the step to show progress ("Reading 12 of 85 files…" and the current file), so that I know it's working.
19. As a user, I want to cancel the analysis and go back with nothing changed, so that I can stop if it takes too long.
20. As a user who cancelled, I want the files already read not to be read again next time, so that the work isn't wasted.
21. As a user running the step a second time on the same folder, I want only new or changed files read, so that it's fast.
22. As a user who renamed or moved a file, I want it not to be read again, so that reorganizing doesn't slow the next run.
23. As a user with a slow or broken PDF, I want the app to give up on it after a short time and use its name instead, so that one bad file can't stall everything.
24. As a user, I want my files opened read-only during analysis, so that nothing changes until I approve a Plan.

### Suggested Folders

25. As a user, I want each Suggested Folder named after what its files are about, so that the name makes sense to me.
26. As a Vietnamese-speaking user, I want names in the language of my files, so that a Vietnamese course folder has a Vietnamese name.
27. As a user, I want to edit a Suggested Folder's name, so that I can fix a poor one.
28. As a user, I want an empty name to be rejected rather than treated as "trash", so that clearing a name can never lose files.
29. As a user on any OS, I want names to be made valid for my file system, so that a topic with a colon or slash doesn't break the move.
30. As a user, I want a new folder whose name is already taken to get a Clash Suffix, so that nothing is merged by accident.
31. As a user, I want a group that matches an existing subfolder to go into it, shown as "Add to **Invoices**", so that I don't get a second folder for the same thing.
32. As a user, I want to switch a matched group to a new folder, so that I can override a wrong match.
33. As a user, I want images, installers and archives each gathered into one folder of their own without being opened, so that they're out of the way quickly.
34. As a user, I want those kind folders to go into an existing subfolder with a matching name, so that my existing "Images" folder is reused.
35. As a user, I want to create a Suggested Folder in the Scan Root by default, so that the common case needs no extra choice.
36. As a user, I want to create a Suggested Folder somewhere else I pick, so that I can file things straight into a folder outside the Scan Root.
37. As a user, I want to trash all the files of a Suggested Folder, so that I can clear out a group of things I don't need.
38. As a user, I want to skip a Suggested Folder, so that its files stay where they are.
39. As a user, I want to trash a single file inside a Suggested Folder, so that I can drop one unwanted file without trashing the group.
40. As a user, I want to take a single file out of a Suggested Folder so it stays where it is, so that one wrong file doesn't spoil the group.
41. As a user, I want to see a file's one-sentence summary when I hover over it, so that I can check why it's in the group without opening it.
42. As a user, I want the list itself to show only file names, so that the screen stays quiet.
43. As a user, I want files that didn't group listed under "Left where they are", so that I can see nothing was missed.
44. As a user, I want a group of fewer files than the smallest folder size not to become a Suggested Folder, so that I don't get a folder for every single file.
45. As a user, I want a "Looser ↔ Stricter" slider on the step that regroups instantly, so that I can find the grouping that suits this folder.
46. As a user, I want the slider's position remembered, so that next time starts from what I chose.
47. As a user, I want the Suggested Folders Plan to run as its own Session, undoable from History, so that I can reverse it without touching the duplicate Session.
48. As a user, I want moves to another disk refused as elsewhere in the app, so that nothing is copied and deleted underneath.

### Settings

49. As a user, I want a Settings screen reachable from Home, so that I can change how the app behaves without editing JSON.
50. As a user, I want to choose the Trash Destination there, with the System Trash shown as not yet supported, so that I know what's available.
51. As a user, I want to add a Never-Touch Path with a folder picker, so that I don't have to type paths.
52. As a user, I want to remove a Never-Touch Path, so that I can let the app clean a folder I'd protected.
53. As a user, I want to set how closely files must match to share a folder, with a slider labelled "Looser: fewer, bigger folders ↔ Stricter: more, smaller folders", so that I understand the trade-off.
54. As a user, I want to set how closely a group must match an existing folder to go into it, so that I can control how often my existing folders are reused.
55. As a power user, I want to type the exact threshold next to each slider, kept in step with it, so that I can tune precisely.
56. As a user, I want a one-line explanation under each grouping setting, so that I know what it changes.
57. As a user, I want "Reset to default" next to each grouping setting, so that I can undo my experiments.
58. As a user, I want to set the smallest folder size between 2 and 10 files, so that I can avoid tiny folders.
59. As a user, I want the Grouping Models' status, size and Download / Pause / Cancel / Remove in Settings, so that I manage them in one place.
60. As a user, I want changes saved as soon as I make them, so that there's no separate Save to forget.
61. As a user who enters an invalid value, I want an inline error and nothing saved, so that a typo can't break the app.
62. As an advanced user, I want "Open config file" still available, so that I can edit the JSON directly.
63. As an advanced user who edits the JSON by hand, I want an out-of-range grouping value rejected with the line and column, as for any other config error, so that I can find and fix it.
64. As a user, I want my frozen Rules kept untouched when Settings saves, so that nothing I wrote by hand disappears.

## Implementation Decisions

### Modules

- **Grouping engine:** a new crate in the workspace, next to `core`.
  - It holds everything that needs llama.cpp or PDFium. `core` gains no ML or C++ dependency and stays pure Rust and quick to build and test (ADR-0004).
  - The Tauri app depends on both crates and passes the grouping result to `core`.
- **Core** gains:
  - the Suggested Folder Reason;
  - planning for Suggested Folder choices;
  - folder name sanitizing;
  - the grouping config section;
  - config saving.
- **Tauri commands** are added for:
  - Grouping Models status and download control;
  - analysis with progress and cancel;
  - regrouping;
  - building the Suggested Folders Plan;
  - loading and saving settings.
- **UI** (plain ES modules, no framework, no build step: ADR-0003):
  - a first-open offer;
  - the real Suggested Folders step, replacing the placeholder;
  - a new Settings screen.

### Grouping engine

- **Interface:** one main entry point. It takes the loose files from the fresh scan the step already has (it doesn't walk the disk again), the existing subfolders of the Scan Root, the grouping settings, and a **models** interface. It returns:
  - the Suggested Folders, each with:
    - a name;
    - its files;
    - the matched existing subfolder, or none;
    - whether it's a fixed-kind folder;
  - the ungrouped files;
  - each file's summary, for the tooltip.
- **Models interface:** the models sit behind a small interface (card for a file's content, embedding for a text), so the engine can be driven with fakes. The real implementation runs llama.cpp.
- **Regrouping:** a second entry point regroups from stored cards and embeddings with new settings, and runs no model.
- **Behaviour carried over from the prototype:**
  - **Fixed kinds:** archives, images and installers (the prototype's extension lists) are never read, and each kind forms one group.
  - **Reading:** at most 2,000 characters per file:
    - PDF text from the first 3 pages through PDFium, in a child process killed after 10 seconds;
    - `.docx`, `.pptx` and `.xlsx` XML stripped to text;
    - plain text and code read as UTF-8;
    - otherwise the file name, with separators as spaces.
  - **Card:** the LLM reads the name and the first 1,200 characters and replies with topic, type and summary, using the prototype's prompt and reply parsing. At most 120 new tokens.
  - **Embedding:** the card text (topic, type, summary) is embedded and L2-normalized.
  - **Clustering:** average-linkage agglomerative clustering on cosine similarity. Two files with the same topic key (lowercased, whitespace collapsed) count as similarity 1. A file with no topic never counts as the same topic.
  - **Anchoring:** each existing subfolder is described by up to 5 files sampled from inside it, recursively. A group goes to the most similar subfolder at or above the existing-folder threshold. Fixed-kind folders match a subfolder by name.
  - **Naming:** a group is named after its most common topic.
- **Runtime** (ADR-0004):
  - llama.cpp through the `llama-cpp-2` crate, with Metal on macOS and Vulkan on Windows and Linux.
  - When no usable Vulkan GPU or driver is found, or Vulkan fails to start, analysis falls back to the CPU. It must never crash or refuse to start. The Vulkan loader is bundled with the Windows app, and the Linux packages depend on the system's Vulkan loader, so the app always launches.
  - Building for Windows or Linux needs the Vulkan SDK on the build machine.
  - Models are loaded when analysis starts and dropped when it ends, freeing about 2 GB.
  - All pending files are carded first, then embedded.
- **PDFium:** the prebuilt library from the release the prototype pinned (bblanchon `chromium/7881`) is bundled with the app per OS and loaded at run time. Nothing is downloaded at run time. The child process is the app itself, started in a "read one PDF" mode.
- **Card store:**
  - Cards and embeddings are stored in the app's data folder, keyed by content hash plus model ID. The content hash is the BLAKE3 the scan computes; files the scan didn't hash are hashed here.
  - A renamed or moved file hits the store. Changing either model's ID makes old entries unused.
  - Cards are stored as they're made, so a cancelled analysis keeps them.
- **Progress:** `{ finished, total, current }`, reported at most every 100 ms, the same shape and rate as Run progress. A cancel flag is checked between files.

### Grouping Models

- **The two models:** the makers' official GGUF files:
  - **LLM:** Qwen2.5-1.5B-Instruct, Q4_K_M, about 1.1 GB.
  - **Embedding:** Qwen3-Embedding-0.6B, Q8_0, about 0.64 GB.
- **Pinning:** each has a URL at a fixed repository revision (never a moving branch), its size and its SHA-256. llama.cpp reads the tokenizer from the GGUF itself.
- **Downloading:**
  - Downloads go into a models folder in the app's data folder, through a partial file resumed with HTTP Range requests.
  - A model counts as installed only after its SHA-256 matches, when it is moved into place.
  - Before starting, free space is checked for the total plus about 10%.
  - Pause keeps the partial file; cancel deletes it.
- **Status:** one of not installed, downloading (with progress), paused or installed. Commands: start, pause, resume, cancel, remove.
- **First-open offer:** it appears once, before Home, and its answer is remembered in app data. After "Not now", it reappears only inside the Suggested Folders step and Settings, never as a prompt at launch.
- **Without the models:** the Suggested Folders step shows the install offer (or the running download) and a note that sorting by kind, name and date is coming. The step branches on "models installed" so that the fallback grouper can slot in later without restructuring.
- **Wording:** "grouping models" everywhere. Never "AI", "smart" or "LLM" in the UI.

### Core: planning Suggested Folders

- **Reason:** a new Reason variant, Suggested Folder, carries the folder's name.
- **Choices per Suggested Folder:**
  - **create here:** Create Folder in the Scan Root, then a Move per file;
  - **add to existing:** a Move per file into the matched subfolder, with no Create Folder;
  - **create elsewhere:** Create Folder at the picked place, then Moves, still subject to the same-disk rule;
  - **trash all:** a Trash per file;
  - **skip:** nothing.
- **Per file:** trash (a Trash Operation) or take out (no Operation).
- **Folder names** are made safe on every OS before planning:
  - remove `/ \ : * ? " < > |` and control characters;
  - trim whitespace and trailing dots;
  - alter Windows reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`) so they're no longer reserved;
  - cap at 100 characters.

  A name that's empty afterwards is a validation error, never a Trash. A new folder whose name is taken gets a Clash Suffix.
- The Suggested Folders Plan is its own Session, recorded in the Journal like any other.

### Core: config

- **The `grouping` section:** a new optional section with three optional keys:
  - `folderThreshold`, 0–1: how closely files must match to share a folder;
  - `existingFolderThreshold`, 0–1: how closely a group must match an existing subfolder to go into it;
  - `smallestFolder`, 2–10: the minimum number of files in a Suggested Folder.

  Defaults live in code. They start at the prototype's 0.86, 0.86 and 2, and the developer tunes them by hand for the new embedding model. An out-of-range value fails with the existing line-and-column config error. Nothing is clamped silently.
- **Saving (new):** the validated config is written back as JSON, keeping every key the UI doesn't edit (including the frozen Rules) unchanged. The write is atomic: a temporary file, then a rename.

### UI

- **First-open offer:** a quiet sheet: "Group files by what's inside them?", with one line saying the app can download grouping models (1.8 GB) to read files on this computer and suggest folders, and that nothing leaves the computer. It names the right OS. Buttons: **Download** and **Not now**.
- **Suggested Folders step:**
  - **While analysing:** progress ("Reading 12 of 85 files…" and the current file) with Cancel, which goes back with nothing changed.
  - **The Plan:** follows the Plan review layout, with one card per Suggested Folder. Each card has:
    - its editable name;
    - "New folder in {Scan Root}" or "Add to **{subfolder}**";
    - the choice: create here / create elsewhere… / trash all / skip;
    - its files, one line each, each with trash or take out and the summary as a hover tooltip.
  - **Last:** "Left where they are", with no tick boxes.
  - **The slider:** a "Looser ↔ Stricter" slider above the cards regroups through the regroup command and saves `folderThreshold`. Per-folder edits are kept for groups that still exist.
- **Settings screen**, reached from Home, has four sections:
  - **Trash Destination:** the Holding Folder; the System Trash is shown as not yet supported.
  - **Never-Touch Paths:** a list with Add (folder dialog) and Remove.
  - **Grouping:**
    - each threshold is a slider plus a number field (0–1, two decimals), kept in step, with a one-line explanation and "Reset to default";
    - the smallest folder size is a 2–10 stepper.
  - **Grouping Models:** status, size, and Download / Pause / Cancel / Remove.

  A small "Open config file" link stays. Changes save as they're made. An invalid value shows an inline error and isn't saved. The slider on the step and the one in Settings map to the threshold the same way.
- **Visual direction:** the same as the Plan review spec. Quiet and native, like Finder or System Settings, with the existing tokens only. No tinted tiles, glow or pastel panels, and no "AI" wording.

## Testing Decisions

- **For now: minimal tests, best case only.** One test at each of three places, and nothing more. Fuller and edge-case testing is deferred.
- **What makes a good test here:** it calls the module's public entry point with plain inputs and checks what comes out (the Suggested Folders, the Plan's Operations, the loaded config). It never reaches into private helpers, and it never needs the real models.
- **The three places:**
  1. **The grouping engine's entry point,** driven by fake models (fixed cards and vectors). Three files on one topic and one unrelated file give one Suggested Folder of three, named after the topic, with the unrelated file left where it is. This is the one new seam the feature needs: the models interface. Prior art: the prototype's clustering tests, which this replaces with a higher-level test.
  2. **Core's planner.** "Create here" for one Suggested Folder gives one Create Folder plus a Move per file, each with the Suggested Folder Reason. Prior art: the existing Plan tests for the duplicate step.
  3. **Config load and save.** Saving a changed `folderThreshold` and loading it again gives the same value. Prior art: the existing config tests.
- **Not tested for now:**
  - the model download;
  - the real llama.cpp and PDFium code (checked by running the app);
  - the UI screens and the live slider.
- **Builds:** each OS is checked by building natively on it, through CI or a VM, since the grouping crate compiles C++ (ADR-0004).

## Out of Scope

- The fallback grouper (kind, then a shared name word, then month) for users without the Grouping Models. It gets its own spec later.
- CUDA and ROCm. Vulkan already covers NVIDIA, AMD and Intel GPUs.
- Choosing a model size by hardware, or offering bigger or smaller models.
- OCR for images and screenshots, and opening archives.
- Grouping files inside subfolders.
- Reviving Rules, and the System Trash (ADR-0001).
- A developer report command. The developer tunes thresholds through Settings or the config.
- Removing the content-grouping prototype, which is the developer's call.

## Further Notes

- **The verdict:** B, the content grouper with LLM cards, beat the no-ML baseline. It's recorded in the prototype issue. The engine decision is ADR-0004, and the Grouping Models term is in the glossary.
- **Model change:** the embedding model changes from the prototype's multilingual-e5-small to Qwen3-Embedding-0.6B, because only official GGUF files are used and none exists for e5. The similarity scale differs, so the default thresholds must be re-tuned before release.
- **Size and memory:** the total download is about 1.8 GB, and memory use during analysis is about 2 GB.
- **Implementation order:**
  1. the grouping engine, and installing the Grouping Models, in parallel;
  2. the Suggested Folders step and Plan;
  3. the Settings screen;
  4. the live slider.
