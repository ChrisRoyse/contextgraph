---
namespace: handoff
from_agent: agent-e14-1-tokenizer-probe
to_agent: agent-e14-2-enum-plumbing
timestamp: 2026-04-22T15:40:00Z
status: ready
tags: e14, handoff, tokenizer, enum
---

# Handoff: Task 1 → Task 2 (agent-e14-2-enum-plumbing)

## TL;DR

Tokenizer side of E14 is **verified and unblocked**. Proceed to
`TokenizerFamily::XlmRobertaSentencePiece` plumbing (Task 2) and
`ModelId::BgeM3Dense = 14` (Task 3) without concern that the tokenizer
itself will later fail.

## What was proven

1. `tokenizers` v0.22 (the already-pinned dep at
   `crates/context-graph-embeddings/Cargo.toml:69`) loads bge-m3's
   XLM-RoBERTa SentencePiece `tokenizer.json` via
   `Tokenizer::from_file(path)` — the exact same call pattern all other
   embedder loaders use.
2. Real Shakespeare strings tokenize cleanly to 12-15 tokens each.
3. Special tokens are the standard XLM-RoBERTa set:
   `<s>`=0, `<pad>`=1, `</s>`=2, `<unk>`=3.
4. `vocab_size = 250002` — confirms XLM-RoBERTa-Large.
5. encode/decode round-trip preserves content (SentencePiece normalizes
   only whitespace, no content loss).

## For Agent 2 (enum plumbing, Tasks 2-4)

### Task 2 — `TokenizerFamily::XlmRobertaSentencePiece`

**Where:** `crates/context-graph-embeddings/src/types/model_id/tokenizer.rs`

**What you need to know:** this is a pure naming/enum variant. No
tokenizer-specific state. The existing `TokenizerFamily` enum is a light
classifier used by routing/logging, not by the tokenizer crate itself.
All the actual tokenization goes through `Tokenizer::from_file` regardless
of family.

### Task 3 — `ModelId::BgeM3Dense = 14`

**Where:** `crates/context-graph-embeddings/src/types/model_id/core.rs`

**Fields you'll need (verified values):**

| Field | Value | Source |
|---|---|---|
| `dimension()` | 1024 | bge-m3 dense head output (spec 03_pilot_bge_m3_dense.md §"What we're adding") |
| `projected_dimension()` | 1024 (same as dimension) | No Matryoshka projection for E14 in Phase 1 |
| `is_custom()` | false | It's pretrained |
| `max_tokens()` | 8192 | bge-m3 context length (spec 03) |
| `tokenizer_family()` | `TokenizerFamily::XlmRobertaSentencePiece` | from Task 2 |
| `as_str()` | `"E14_BGE_M3_DENSE"` (follow existing `E1_SEMANTIC`, `E2_DECAY`, ... pattern) | spec 03 §MCP surface: `"E14"` / `"E14_BGE_M3_DENSE"` / `"bge_m3_dense"` all accepted |
| `latency_budget_ms()` | suggest 50ms (bge-m3 on RTX 5090 with 8k context is slower than e5-large-v2 which is 15ms; pick ~3x) | spec 03 target: ≥5 docs/sec ⇒ ≤200 ms/doc worst case, but single-text latency budget should be tight |

### Task 4 — `NUM_EMBEDDERS: usize = 14`

**Where:** `crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs`

Coordinator's memory.md (see top of this repo's `CLAUDE.md`) documents that
there are currently 13 embedders. Flipping this to 14 will cause compile
errors in every fixed-size array that depends on it. Per the pilot spec,
that's intentional — the compiler will enumerate all touchpoints.

Files to watch (from prior audit evidence in memory.md):
- `tests/search_test.rs:34` has `assert_eq!(NUM_EMBEDDERS, 13)` — flip to 14
- `tests/benchmark_test.rs` same pattern

### Hard constraints you MUST respect

- **No mock fallbacks.** If any consumer breaks, fix the consumer. Do not
  add "if cfg!(test)" branches.
- **No silent defaults.** If a new match arm needs a value, use the values
  in the table above; don't use `Default::default()` or `0` as a placeholder.
- **NUM_EMBEDDERS lesson from memory.md:** "ModelId has 14 variants,
  NUM_EMBEDDERS=13 — Kepler is 14th ModelId but shares E11 weight slot".
  After your Task 3, the enum has 15 variants (14 active + Kepler sharing
  E11) and NUM_EMBEDDERS=14. Existing Kepler handling should continue to
  work unchanged because its discriminant is already ≤ 13.

## Pattern Agent 3 (model-impl) should follow for loading

From `crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs:54-69`:

```rust
let tokenizer_path = self.model_path.join("tokenizer.json");
let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
    EmbeddingError::ModelLoadError {
        model: ModelId::BgeM3Dense,  // new
        reason: format!("Failed to load tokenizer.json from {}: {}",
                        tokenizer_path.display(), e),
    }
})?;
```

Then configure truncation (bge-m3 tokenizer.json ships with
`"truncation": null`, so the loader must set this explicitly):

```rust
use tokenizers::{TruncationParams, TruncationDirection, TruncationStrategy};
tokenizer
    .with_truncation(Some(TruncationParams {
        max_length: 8192,
        direction: TruncationDirection::Right,
        strategy: TruncationStrategy::LongestFirst,
        stride: 0,
    }))
    .map_err(|e| EmbeddingError::ModelLoadError { ... })?;
```

## Model location for Agent 3

- **Tokenizer:** `~/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/5617a9f61b028005a4858fdac845db406aefb181/tokenizer.json`
- **Config:** `~/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/5617a9f61b028005a4858fdac845db406aefb181/config.json`
- **Safetensors weights:** `~/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/9a0624b896d81da7492a910ffa53731274b6cf3d/model.safetensors`
  (separate snapshot!)
- **pytorch_model.bin** also available in snapshot #1 if `.safetensors` has issues.

See `discoveries/agent-e14-1-tokenizer-probe--bge-m3-already-cached.md`
for the full file inventory.

**The safetensors and tokenizer live in DIFFERENT snapshots** — Agent 3's
loader needs to handle that by either:
(a) pointing `model_path` at snapshot 1 and copy-linking in the .safetensors, or
(b) using two paths (ModelConfig with separate `tokenizer_dir` and `weights_path`), or
(c) calling `huggingface-cli download BAAI/bge-m3 model.safetensors` to co-locate.

Recommendation: option (c) during setup script; loader itself just takes a
single `model_path` pointing at snapshot 1. Follow up in Task 10 (loader).

## Quirks / gotchas encountered

None. The tokenizer loads cleanly with no warnings. The only SentencePiece
quirk is standard: word-initial subwords have a `▁` prefix (U+2581). This
is handled transparently by `encode`/`decode`.

## Memory files written

- `memory/discoveries/agent-e14-1-tokenizer-probe--bge-m3-already-cached.md`
- `memory/decisions/agent-e14-1-tokenizer-probe--no-hf-hub-crate.md`
- `memory/journal/agent-e14-1-tokenizer-probe--success-tokenizer-works.md`
- `memory/handoff/agent-e14-1-to-e14-2--tokenizer-verified.md` (this file)
- `memory/reports/agent-e14-1-tokenizer-probe--final-report.md` (coordinator-facing)

## Files added to repo

- `crates/context-graph-embeddings/tests/bge_m3_tokenizer_probe.rs` (267 lines)

## You are clear to proceed. Good luck, Agent 2.
