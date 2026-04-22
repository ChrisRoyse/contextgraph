# Full-Scale Shakespeare LoRA — Optimal Training Plan

## The Core Insight

Context Graph doesn't just store Shakespeare's text — it **measures** it across 13 independent dimensions and derives higher-order signals from those measurements. Every signal has meaning. The LoRA training must use ALL of them, each in the way that maximally transfers that meaning into the model's weights.

## What Each Signal Means (and How to Use It)

### Layer 1: Raw Text (3,043 memories)
**What it is**: Shakespeare's actual words — plays, sonnets, poems, chunked at ~200 words.
**What it teaches**: The vocabulary, syntax, rhythm, and imagery of Shakespeare.
**How to use**: SFT examples where the target output IS this text.
**Training format**: `{modern prompt} → {Shakespeare text}`

### Layer 2: 13 Embeddings per Memory (39,559 vectors)
**What each embedder measures**:
- **E1 (semantic 1024D)**: What the text MEANS — the core topic
- **E2 (temporal recent 512D)**: WHEN it was written/stored — recency signal
- **E3 (temporal periodic 512D)**: Time-of-day/season patterns — cyclical rhythm
- **E4 (temporal positional 512D)**: WHERE in a sequence — position in a work
- **E5 (causal 768D × 2)**: CAUSE-EFFECT relationships — "this leads to that"
- **E6 (sparse 30K)**: EXACT KEYWORDS — which specific words appear
- **E7 (code 1536D)**: CODE PATTERNS — structure and logic
- **E8 (graph 1024D × 2)**: STRUCTURAL CONNECTIONS — what links to what
- **E9 (HDC 1024D)**: TYPO-ROBUST — character-level n-gram patterns
- **E10 (paraphrase 768D × 2)**: SAME MEANING DIFFERENT WORDS — paraphrase detection
- **E11 (entity 768D)**: ENTITIES — people, places, concepts mentioned
- **E12 (ColBERT 128D/token)**: TOKEN-LEVEL — precise word-by-word matching
- **E13 (SPLADE 30K)**: EXPANDED KEYWORDS — learned term expansion

**How to use**: These embeddings define the CONSTELLATION — the geometric shape of Shakespeare's style. They inform:
1. **Constellation-conditioned SFT**: Tag each training example with its closest constellation (work label). The model learns "Sonnets sound like THIS, Macbeth sounds like THAT."
2. **Embedder-weighted DPO**: Pairs where specific embedders disagree teach the model to handle specific style dimensions.

### Layer 3: 78 Cross-Correlations per Memory (237,354 values)
**What it means**: How the 13 dimensions INTERACT for this specific text. High E1×E5 correlation = the semantic meaning is also causally structured. High E7×E1 but low E10×E1 = code-like structure with unique (non-paraphraseable) meaning.
**How to use**: Cross-correlations define the TEXTURE of Shakespeare's writing. They can be used as:
1. **Style conditioning tags** in SFT: "This text has high causal structure" vs "This text is pure lyrical"
2. **Quality scoring for DPO**: Pairs where cross-correlation patterns differ teach the model to reproduce the RIGHT texture

### Layer 4: 44 Constellations (per-embedder centroids)
**What it means**: The geometric CENTER of each Shakespeare work in 13-space. Sonnets have a specific centroid; Macbeth has a different one.
**How to use**: 
1. **Work-specific SFT**: Each training example tagged with its closest constellation
2. **Style routing at inference**: "Write like Sonnets" vs "Write like Macbeth"
3. **Guard validation**: Score generated output against constellation

### Layer 5: 13,465 Contrastive Pairs (with anomaly classifications)
**What it means**: Pairs of Shakespeare texts that are similar on SOME dimensions but different on OTHERS. These are the MOST VALUABLE training signal because they teach the model WHAT MAKES TWO SHAKESPEARE PASSAGES DIFFERENT.
- `semantic_but_not_causal`: Same topic, different causal structure
- `keyword_but_not_paraphrase`: Same words, different meaning
- `code_shape_but_different_intent`: Same structure, different purpose
- `entity_shared_but_different_structure`: Same entities, different relationships
- `hdc_robust_but_semantic_different`: Character-similar but meaning-different
**How to use**: DPO training — chosen = the text that better matches the prompt's intent, rejected = the text that matches on surface but fails on depth.

### Layer 6: 8-Dim Edge-Type Distribution per Memory
**What it means**: Each memory's ROLE in the knowledge graph — hub, leaf, causal-root, entity-connector.
**How to use**: Curriculum learning — train on hubs first (most connected, most representative), then specialize to leaves.

### Layer 7: Adversarial Robustness (generated)
**What it means**: The model must stay in character when attacked.
**How to use**: SFT examples for injection resistance, language robustness, character persistence.

## The Full Training Recipe

### Phase 1: Data Extraction (requires daemon + Ollama)

**Step 1.1**: Pull ALL 3,043 memory contents from `CF_CONTENT`
**Step 1.2**: For each memory, get its closest constellation (work label) from `score_against_constellation`
**Step 1.3**: For each memory, generate 4 modern-topic prompt variants using Ollama
**Step 1.4**: Tag each SFT example with:
  - `work_label` (closest constellation)
  - `style_tag` (from STYLE_PROMPTS lookup)
  - `cross_correlation_pattern` (high-causal, high-lyrical, high-structural, etc.)
**Step 1.5**: Generate adversarial examples (language, code, break)

Result: ~12,000+ constellation-conditioned SFT examples

### Phase 2: DPO Preparation (no daemon needed)

**Step 2.1**: The 12,567 contrastive pairs are ALREADY BUILT
**Step 2.2**: Each pair already has `anomaly_kind` + `similarity_profile[13]`
**Step 2.3**: Format as DPO: prompt + chosen (anchor) + rejected (negative)

Result: 12,567 DPO pairs ready

### Phase 3: Training (GPU only, no daemon)

**Step 3.1**: SFT Phase — rank 256, alpha 512
  - Dataset: ~12,000 SFT examples
  - Epochs: 3-5
  - Key innovation: constellation-conditioned system prompts
  
**Step 3.2**: DPO Phase — rank 256
  - Dataset: 12,567 pairs  
  - Epochs: 1
  - Key innovation: anomaly-kind-aware preference learning
  
**Total training signals used**: ~24,567 examples leveraging:
  - All 3,043 memories (×4 prompt variants = 12,172)
  - All 13,465 contrastive pairs (→ 12,567 DPO after filtering)
  - All 44 constellations (as conditioning tags)
  - 64+ adversarial examples
  - Cross-correlation patterns as style tags

### Phase 4: Testing

15-prompt FSV suite covering all happy paths and edge cases.

## Why This Will Fix the Remaining Failures

**EC-01 (code injection)**: 20 code-injection adversarial examples × 5 epochs = 100 gradient updates teaching "when asked for code → refuse in Shakespeare"

**EC-03 (Spanish/non-English)**: 24 language adversarial examples × 5 epochs = 120 gradient updates teaching "when spoken to in any language → respond in Shakespearean English"

**EC-05 (break character)**: 20 character-break adversarial examples × 5 epochs = 100 gradient updates teaching "when asked to break character → refuse in character"

**HP-08 (memory)**: Fixed by memory management architecture (first exchange always preserved + middle summarized + recent verbatim)

## Estimated Timeline

- Phase 1 (data extraction): ~2 hours (daemon + Ollama generating prompts for 3,043 memories)
- Phase 2 (DPO prep): Already done
- Phase 3.1 (SFT training): ~90 min on RTX 5090
- Phase 3.2 (DPO training): Skip if SFT alone passes all tests; add if needed
- Phase 4 (testing): ~30 min
- **Total: ~4 hours**

## What We're NOT Using (and Why)

- **Raw embedding vectors**: These are the measurement instruments, not training targets. The MODEL learns to produce text that, when re-measured, matches the Shakespeare manifold. We don't feed raw vectors into the SFT/DPO loss.
- **K-NN neighbor lists**: These define graph structure but don't translate into (prompt, response) pairs directly.
- **Typed edges individually**: Already represented via the contrastive pairs and edge distributions.
- **Tucker cores**: Research feature, not production-critical.

The signals we ARE using (memories, constellations, contrastive pairs, cross-correlation patterns, adversarial examples) cover the full meaning space. The ones we're not using are redundant representations of the same meaning.
