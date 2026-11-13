# The user may swap the model that describes files, never the embedding model

This partly reverses ADR 0004. Of the two Grouping Models, the user may replace the one that describes each file (Qwen2.5-1.5B-Instruct by default) with any GGUF file they link from Hugging Face. The embedding model (Qwen3-Embedding-0.6B) stays fixed. The grouping levels on the Settings screen are tuned against the embedding model's similarity scale, so swapping it would quietly change what every level means. A different describing model changes the descriptions, not the scale those levels sit on.

A custom model can't be pinned by SHA-256 the way the defaults are. Instead it is checked by loading it and describing one sample file. If that fails (it isn't a chat model, or its output can't be read), it is refused with the reason and the default stays in use. Its hash is computed and shown, never checked. Only a link to one `.gguf` file is accepted (`/blob/` or `/resolve/`), not a repo link, since a repo holds many quantizations. The default's file stays on disk so switching back is instant, and at most one custom model is kept.

## Considered Options

- **Keep both models fixed (ADR 0004 as it was):** rejected because users want to try a bigger or different-language describing model.
- **Let both models be swapped:** rejected because the grouping levels would stop meaning what they say.
- **Accept a repo link and pick the file automatically:** rejected because a guessed quantization would surprise people.

## Consequences

- Saved descriptions are already stored per model pair, so switching models never mixes old and new descriptions. Each switch means files are read again.
- The option sits behind "Advanced" in Settings and never appears in the first-run introduction.
