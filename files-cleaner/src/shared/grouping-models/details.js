// What the Grouping Models are, what they're for and why they're downloaded: in full on the introduction's
// download card, and as the tooltip of the shorter offers elsewhere.

/**
 * The paragraphs saying which models are downloaded, `size` (already formatted) in all, what each is for,
 * that they run on this `computer`, and what happens without them.
 */
export function groupingModelsDetails(size, computer) {
  return [
    "To sort files by topic, the app reads inside them (text, PDFs, scanned pages) and works out what each one is about.",
    "That needs two open-source language models from Qwen: Qwen2.5-1.5B writes a short description of each file, and Qwen3-Embedding-0.6B compares the descriptions to find files that belong together.",
    `${size}, downloaded once from Hugging Face. They run on this ${computer}, so your files never leave it.`,
    "Without them, folders come from file kind, name and date. You can remove them anytime in Settings.",
  ];
}
