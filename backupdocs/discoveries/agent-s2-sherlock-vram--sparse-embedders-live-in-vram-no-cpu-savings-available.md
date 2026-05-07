---
namespace: discoveries
agent_id: agent-s2-sherlock-vram
timestamp: 2026-04-17T12:15:00Z
status: active
severity: medium
tags: gpu, vram, embedders, e6, e13, sparse, splade, rtx5090
---

# E6 (Sparse) and E13 (SPLADE) ARE loaded in VRAM — disabling them saves ~1.4 GB on-disk equivalent

## What Happened

Investigation of `crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs`:

- `SparseModel::with_model_id()` calls `init_gpu()` and has `AP-007: NO CPU fallback` policy
- `forward::gpu_forward_sparse` does BERT attention on GPU then produces sparse vectors
- Both E6 (naver/splade-cocondenser-ensembledistil) and E13 (splade-v3) run BERT forward on GPU

They produce SPARSE output vectors (used by CPU InvertedIndex for retrieval), but the **encoder (BERT backbone)** that generates those sparse vectors lives fully in GPU memory.

## On-Disk Weights (safetensors)
| Model | Path | Size | Approx bf16 VRAM |
|---|---|---|---|
| E6 Sparse (cocondenser) | `models/sparse/` | 687 MB | ~0.7 GB |
| E13 SPLADE-v3 | `models/splade-v3/` | 688 MB | ~0.7 GB |
| **Combined** | — | **1,375 MB** | **~1.4 GB if disabled** |

## Why It Matters

There is NO command-line flag to disable specific embedders — daemon loads all 13 or none (via `--no-warm`, but that only defers loading, doesn't prevent it).

Saving 1.4 GB by disabling E6 + E13 would turn the 32.2 GB Phase-G conflict (20.7 + 11.5 over 31.8 GB budget) into a safe 30.8 GB budget — but **this requires a source-code change**, not a flag.

## Other embedder VRAM contributions (on-disk size)
| Embedder | Path | On-disk | Approx VRAM (bf16) |
|---|---|---|---|
| E1 Semantic (e5-large-v2) | `models/semantic/` | 1.3 GB | ~1.3 GB |
| E5 Causal | `models/causal/` | 574 MB | ~0.6 GB |
| E7 Code (1536-D) | `models/code-1536/` | 5.8 GB | **~5.8 GB (largest)** |
| E8 Graph | `models/graph/` | 88 MB | ~0.1 GB |
| E10 Multimodal | `models/multimodal/` | 1.6 GB | ~1.6 GB |
| E11 Entity/Kepler | `models/entity/` + `models/kepler/` | 88+481 MB | ~0.6 GB |
| E12 ColBERT (late-interaction) | `models/late-interaction/` | 419 MB | ~0.4 GB |
| E6 Sparse | `models/sparse/` | 687 MB | ~0.7 GB |
| E13 SPLADE-v3 | `models/splade-v3/` | 688 MB | ~0.7 GB |
| Contextual | `models/contextual/` | 419 MB | ~0.4 GB |
| **Sum of embedder weights** | — | **~12.1 GB** | — |
| Measured daemon RSS (nvidia-smi) | — | **17.34 GB** | includes HNSW indexes, KV caches, workspaces |
| Overhead inferred | — | ~5.2 GB | HNSW in-memory indexes, usearch buffers, RocksDB page cache |

E7 Code is the single largest embedder at ~5.8 GB — disabling E7 alone would bring the daemon below 12 GB and eliminate Phase G's conflict completely. But disabling E7 breaks code-aware scoring which Shakespeare plays DO use for structural matches.

## Recommendation

Do NOT attempt to disable individual embedders for this mission. The work is non-trivial (requires source patching + testing), and the Phase-D stagger (daemon DOWN → Gemma up → Gemma down → daemon UP) + Phase-G Gemma-Q4 approach fits within the budget without it.

## Related
- Source: `crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` lines 82-203
- On-disk sizes: verified via `du -sh /home/cabdru/contextgraph/models/*/` on 2026-04-17
- VRAM budget decision: `memory/decisions/agent-s2-sherlock-vram--phase-budget-manifest.md`
