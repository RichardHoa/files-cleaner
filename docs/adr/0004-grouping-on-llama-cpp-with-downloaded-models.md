# Grouping on llama.cpp, with official GGUF models downloaded on first use; C/C++ build dependencies allowed

Suggested Folders come from reading inside files: a small LLM writes a card (topic, type, summary) per file, an embedding model embeds the cards, and the cards are clustered. Both models run on llama.cpp (through the `llama-cpp-2` crate), compiled into the app, with Metal on macOS and Vulkan on Windows and Linux, falling back to the CPU where there's no usable Vulkan GPU. The models are the makers' own GGUF files (Qwen2.5-1.5B-Instruct Q4_K_M, Qwen3-Embedding-0.6B Q8_0), pinned by SHA-256 and downloaded once into the app's data folder, only after the user agrees. We chose llama.cpp because it is the fastest local engine for the slow stage (the LLM) on every OS, and one runtime with one model format is less to ship than two. We chose official GGUFs so that nobody has to convert, host or maintain a model.

## Considered Options

- **Pure Rust (the prototype's stack):** candle for the LLM, ort's prebuilt ONNX Runtime for embeddings, and no C compiled at build time. It was rejected because candle's prompt processing is much slower, and two runtimes with two model formats are more to ship.
- **multilingual-e5-small** (the embedding model the prototype was judged on): only community GGUF conversions exist, and converting and hosting our own copy was rejected as upkeep. EmbeddingGemma-300M has an official GGUF but a more restrictive license.
- **Bundling the models in the installer:** rejected. It would add about 1.8 GB to every download, including for users who never want content grouping.

## Consequences

- Building the app needs CMake and a C++ compiler on every OS, plus the Vulkan SDK on Windows and Linux. The Vulkan loader ships with the Windows app, and the Linux packages depend on it, so a machine without Vulkan still runs on the CPU.
- Each OS is checked by building natively on it, not with `cargo check --target` from a Mac.
- llama.cpp and PDFium live in their own crate (`files-cleaner-grouping`), so `core` stays pure Rust and quick to build and test.
- Changing either model invalidates the stored cards and the tuned thresholds.
