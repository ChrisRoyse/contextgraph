# Extended Embedder Integration — Planning Docs

Full plan for wiring 4 new SOTA embedders into the native Rust daemon at
full parity with the existing E1-E13 embedders.

## Target embedders

| Slot | Model | Dim | Arch family | Complexity |
|------|-------|-----|-------------|------------|
| E14 | BAAI/bge-m3 (dense output) | 1024 | XLM-RoBERTa encoder | Medium (closest to E1) |
| E15 | BAAI/bge-m3 (sparse output) | ~30K | XLM-RoBERTa + MLM head | Medium (closest to E13) |
| E16 | BAAI/bge-m3 (ColBERT multi-vector) | 128/token | XLM-RoBERTa + linear proj | Medium (closest to E12) |
| E17 | jinaai/jina-embeddings-v3 (classification task) | 1024 | XLM-RoBERTa + task LoRA | **New pattern** — task-LoRA adapters |
| E18 | Qwen/Qwen3-Embedding-0.6B | 1024 | **Qwen2.5 decoder-only** | **New pattern** — first decoder-only embedder |
| E19 | BAAI/bge-reranker-v2-m3 | 1 logit | XLM-RoBERTa + seq classifier | Medium (cross-encoder pair scoring) |

Note: BGE-M3 has 3 heads sharing one backbone. Treat as one shared encoder
(saving VRAM + weights) with 3 projection heads → 3 ModelId variants.

## Document index

| Doc | Purpose |
|-----|---------|
| [`00_strategy.md`](00_strategy.md) | Top-level strategy, phasing, success criteria |
| [`01_reference_patterns.md`](01_reference_patterns.md) | How existing embedders are wired (with file/line citations) |
| [`02_touchpoints.md`](02_touchpoints.md) | All 938 ModelId references categorized — what needs to change |
| [`03_pilot_bge_m3_dense.md`](03_pilot_bge_m3_dense.md) | Pilot spec: first embedder end-to-end (E14) |
| [`04_bge_m3_sparse_colbert.md`](04_bge_m3_sparse_colbert.md) | Extend pilot to sparse + ColBERT heads (E15, E16) |
| [`05_jina_v3.md`](05_jina_v3.md) | Jina-v3 with task-LoRA (E17) |
| [`06_qwen3_embedding.md`](06_qwen3_embedding.md) | Qwen3 decoder-embedder (E18) |
| [`07_bge_reranker.md`](07_bge_reranker.md) | BGE-reranker cross-encoder (E19) |
| [`08_storage_layer.md`](08_storage_layer.md) | CF column families + HNSW index integration |
| [`09_mcp_surface.md`](09_mcp_surface.md) | MCP tool updates — `search_by_embedder`, `score_against_constellation`, new tools |
| [`10_fsv_protocol.md`](10_fsv_protocol.md) | Full State Verification after each phase |
| [`11_reingestion.md`](11_reingestion.md) | How to re-embed existing 2,770+ records through new models |
| [`12_task_breakdown_pilot.md`](12_task_breakdown_pilot.md) | Atomic tasks for E14 pilot (BGE-M3 dense) |

## Reading order

1. Start with `00_strategy.md` for the big picture
2. Read `01_reference_patterns.md` + `02_touchpoints.md` before any code changes
3. For the pilot implementation, read `03_pilot_bge_m3_dense.md` and `12_task_breakdown_pilot.md`
4. After pilot is green, apply the pattern to other embedders in order: E15, E16, E17, E18, E19

## Core principles (non-negotiable)

1. **Native candle only.** No ONNX, no Python sidecar, no pyo3.
2. **Full parity with existing embedders.** Same trait, same loader pattern,
   same CF column-family shape, same HNSW indexing, same MCP exposure.
3. **Fail fast.** Any model-load failure, any dimension mismatch, any
   weight-file checksum failure → panic at startup or return descriptive
   error. No silent fallbacks.
4. **FSV gate between phases.** No advancing to the next embedder until the
   previous one passes the 6-check FSV (disk, schema, determinism,
   discrimination, edge cases, physical evidence).
5. **Backwards compatibility during transition.** Tests using `NUM_EMBEDDERS`
   = 13 must continue to pass OR be explicitly migrated in the same PR.
6. **Re-ingestion is explicit.** Never silently produce zeroed vectors for
   old records — either reject reads until re-ingestion completes or mark
   missing-extended flag in SourceMetadata.
7. **Document as we go.** Every new CF gets entry in `column_families.rs`
   docs block; every new ModelId gets rustdoc; every new MCP tool gets
   schema + description.
