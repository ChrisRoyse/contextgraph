# Phase 5 — BGE-reranker-v2-m3 (E19)

**Model:** `BAAI/bge-reranker-v2-m3`
**Architecture:** XLM-RoBERTa-Large encoder + sequence classification head
**Output:** 1 scalar logit per (query, passage) pair (NOT a vector)
**Parameters:** 568M

## Why this phase is different

The reranker is a **cross-encoder**, not a bi-encoder. It doesn't produce
document embeddings. It produces a relevance **score** for a `(query, passage)`
pair by encoding them jointly.

Implication: the reranker does NOT fit the `EmbeddingModel` trait. It
needs a different trait and different MCP tool surface.

## Proposed design

### New trait: `RerankerModel`

```rust
pub trait RerankerModel: Send + Sync {
    fn rerank_batch(&self, pairs: &[(String, String)])
        -> EmbeddingResult<Vec<f32>>;
}
```

Separate from `EmbeddingModel` to keep the type system honest — cross-encoders
are a different thing.

### Expose via NEW MCP tool: `rerank_with_bge_v2_m3`

```
Tool: rerank_with_bge_v2_m3
Inputs:
    query: string           — the retrieval query
    memory_ids: list[UUID]  — candidates to rerank
    top_k: int (optional)   — return only top K after reranking
Outputs:
    ranked: list of { memory_id, logit_score, rank }
```

### Do NOT get an HNSW index

Reranker has no dense vector to index. At runtime, get first-stage results
from HNSW (E1/E14 etc.), then call reranker on the top-K.

### Do NOT get a persistent CF

The reranker's output depends on the query; cached scores would be
query-specific. No meaningful "store this" semantics. Pure runtime scorer.

## Directory structure

```
crates/context-graph-embeddings/src/models/pretrained/bge_reranker_v2_m3/
├── mod.rs                      (~50)
├── types.rs                    (~80)
├── constants.rs                (~30)
├── model.rs                    (~200)
├── loader.rs                   (~250)
├── tokenizer.rs                (reuse XLM-RoBERTa from E14/E15/E16)
├── attention.rs                (reuse)
├── embeddings.rs               (reuse)
├── encoder.rs                  (reuse)
├── ffn.rs                      (reuse)
├── layer_norm.rs               (reuse)
├── pair_encoding.rs            (~100)    Format query + [SEP] + passage
├── classifier_head.rs          (~80)     Linear + sigmoid → logit
├── gpu_forward.rs              (~200)
├── trait_impl.rs               (~60)
└── tests/                      (~300)
```

**Total: ~1,400 lines new + reuse XLM-RoBERTa from Phases 1-2.**

## ModelId enum

Decision: reranker DOES get a ModelId variant for consistency, but it's
flagged in `is_embedder()`:

```rust
impl ModelId {
    pub fn is_embedder(&self) -> bool {
        !matches!(self, Self::BgeRerankerV2M3)
    }
}
```

`NUM_EMBEDDERS` excludes it. `all()` includes it. `production()` includes
it but consumers can filter.

## MCP tool spec

```json
{
  "name": "rerank_with_bge_v2_m3",
  "description": "Re-rank a list of candidate memories by relevance to a query using the BGE-reranker-v2-m3 cross-encoder. Higher score = more relevant. Use AFTER first-stage HNSW retrieval to get precision uplift.",
  "inputs": {
    "query": { "type": "string", "required": true },
    "memory_ids": { "type": "array", "items": { "type": "string" }, "required": true },
    "top_k": { "type": "integer", "default": null }
  },
  "outputs": {
    "ranked": {
      "type": "array",
      "items": {
        "memory_id": "string",
        "logit_score": "number",
        "rank": "integer"
      }
    },
    "elapsed_ms": "number"
  }
}
```

## Integration with existing search tools

Option A: Optional `rerank=true` flag on existing `search_by_embedder`,
which internally calls the reranker on top-K.

Option B: Keep separate; user chains manually.

**Pick A** for ergonomics. Add `rerank_with_bge_v2_m3: bool` param to
`search_by_embedder`, default false.

## Tests

- Rerank produces higher score for exact-match passage vs unrelated
- Shakespeare query ranks Sonnet 18 > modern weather prose (logit gap ≥ 5)
  — matches Python-side value (+7.47)
- Batch scoring is deterministic (same pairs → same logits)
- Python parity: logit diff ≤ 0.1 vs HF reference

## FSV

Modified 4-check FSV (disk + schema checks skipped since no persistent data):

1. **Load:** model loads and registers via Factory
2. **Determinism:** same inputs → same logits across 5 runs
3. **Discrimination:** Shakespeare > modern by logit gap ≥ 5 on probe
4. **Edges:** empty query → error, mismatched batch dim → error, 0-length
   memory_ids list → empty output (not error)

## Definition of Done

- [ ] `RerankerModel` trait + implementation
- [ ] `ModelId::BgeRerankerV2M3` variant + `is_embedder()` returns false
- [ ] MCP tool `rerank_with_bge_v2_m3` registered
- [ ] `search_by_embedder(rerank=true)` flag works
- [ ] 4-check FSV passes
- [ ] Python parity
- [ ] Tests pass
- [ ] No HNSW / CF added (intentional)

## Estimate

~10-15 engineer hours. Mostly new trait/tool surface + small cross-encoder
head on top of reused encoder.
