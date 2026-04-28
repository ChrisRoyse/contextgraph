# Phase 3 — Jina-embeddings-v3 (E17)

**Model:** `jinaai/jina-embeddings-v3`
**Architecture:** XLM-RoBERTa encoder + task-specific LoRA adapters
**Output:** 1024D dense, Matryoshka-truncatable to 32D minimum
**Context:** 8192 tokens
**Parameters:** 572M base + 60M LoRA adapters

## Why this phase introduces a new pattern

Jina-v3 is the first embedder in the codebase that uses **LoRA adapters on
top of a frozen encoder**. At load time, we need to:

1. Load the base XLM-RoBERTa encoder
2. Load one of 5 task-specific LoRA adapters:
   - `retrieval.query` — for query-side retrieval
   - `retrieval.passage` — for document-side retrieval
   - `separation` — for clustering
   - `classification` — for text classification ← **we use this for style**
   - `text-matching` — for semantic similarity
3. Merge the LoRA weights into the base linearly (or apply dynamically per forward pass)

For our Shakespeare style use case, the `classification` LoRA is the
strongest signal (per Python-side Phase 1 smoke test: +0.1064 discrimination
gap vs +0.0615 for BGE-M3 dense).

## Design decisions

### Decision 1: merge LoRA into base at load time, not dynamically

Per-forward LoRA application costs 10-15% latency. Merging at load time
makes inference identical-cost to standard encoder forward. Cost: can't
switch tasks at runtime without reloading weights.

**For the pilot/classification task:** merge at load.

**For future:** if we want multiple Jina tasks, we'd either:
- Keep 5 merged instances in VRAM (wasteful but simple)
- Load-on-demand with a task cache

**Phase 3 scope: ONLY the classification task.** Other tasks become future
work.

### Decision 2: E17 slot gets only classification Jina

Future could introduce E17a/E17b/E17c... for other tasks if needed. For
now, E17 = Jina-v3 classification head.

## Directory structure

```
crates/context-graph-embeddings/src/models/pretrained/jina_v3_classification/
├── mod.rs                      (~50)
├── types.rs                    (~100)
├── constants.rs                (~30)
├── model.rs                    (~200)
├── loader.rs                   (~300)   Load base + merge LoRA
├── lora_merge.rs               (~200)   LoRA merge logic (NEW PATTERN)
├── tokenizer.rs                (~100)   XLM-RoBERTa SentencePiece (reuse from E14)
├── attention.rs                (reuse from E14 or duplicate)
├── embeddings.rs               (reuse from E14)
├── encoder.rs                  (reuse from E14)
├── ffn.rs                      (reuse from E14)
├── layer_norm.rs               (reuse from E14)
├── pooling.rs                  (~50)    Mean-pool (Jina uses mean, not CLS)
├── matryoshka.rs               (~80)    Truncate 1024D → N dim (NEW)
├── gpu_forward.rs              (~200)
├── trait_impl.rs               (~50)
└── tests/                      (~400)
```

**Total: ~1,700-2,000 lines new (+ reuse E14 encoder code).**

Large chunk of code is LoRA merge logic since it's new.

## LoRA merge detail

LoRA update: `W_new = W_base + B @ A` where A is (r × d_in), B is (d_out × r).

For Jina-v3: r=32 (typical). Merge at load:

```rust
fn merge_lora_into_linear(
    base: &LinearWeight,      // [d_out, d_in] (e.g., 1024 × 1024)
    lora_a: &Tensor,          // [r, d_in]
    lora_b: &Tensor,          // [d_out, r]
    scaling: f32,             // usually alpha / r
) -> EmbeddingResult<LinearWeight> {
    let delta = lora_b.matmul(lora_a)?;      // [d_out, d_in]
    let scaled_delta = delta.scale(scaling)?;
    let merged = base.tensor.add(&scaled_delta)?;
    Ok(LinearWeight::new(merged))
}
```

Apply to every LoRA-adapted linear layer (query, key, value, output, MLP).
HF's PEFT records which modules are adapted in `adapter_config.json`.

## Matryoshka truncation

Jina-v3 supports Matryoshka Representation Learning: the 1024D embedding is
meaningful at any prefix (32D, 64D, 128D, 256D, 512D, 1024D). Our existing
E1 has a similar feature via CF_E1_MATRYOSHKA_128.

Add `ModelId::JinaV3Classification` matryoshka variants: the full 1024D
goes in CF_E17_JINA_V3, a 128D truncated version goes in
CF_E17_JINA_V3_MATRYOSHKA_128 (mirroring E1).

## Storage schema

- `CF_E17_JINA_V3` — 4 + 1024×4 = 4100 bytes per record
- `CF_E17_JINA_V3_MATRYOSHKA_128` — 4 + 128×4 = 516 bytes per record

## HNSW

- `EmbedderIndex::E17JinaV3` — 1024D full
- `EmbedderIndex::E17JinaV3Matryoshka128` — 128D fast-recall stage

## Tests

- Loader merges LoRA correctly: verify output differs from base encoder for
  the same input (ensures the LoRA is being applied)
- Classification-task LoRA specifically: Shakespeare vs modern gap ≥ 0.10
  (matches Python smoke value)
- Matryoshka truncation: cos(full_1024, truncated_128) ≥ 0.97
- Shakespeare domain test: sample 10 Sonnets, all score ≥ 0.85 against
  Sonnets constellation centroid

## FSV — additional check for Phase 3

7th check: **LoRA-merge correctness**. Generate an embedding with LoRA
merged; generate a baseline embedding without merging LoRA. Verify:
- Outputs differ by at least cos < 0.95 (LoRA was actually applied)
- Outputs aren't bit-identical (would indicate silent no-op)

## Definition of Done

Standard 7-layer integration + the 6-check FSV + LoRA correctness check.
