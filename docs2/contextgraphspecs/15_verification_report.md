# Verification Report

Source files covered: derived via `find` / `wc` / `grep` on the source tree (`crates/*/src/**/*.rs`, `crates/*/tests/**/*.rs`, `scripts/**/*.py`, workspace and per-crate `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`).

Cross-references:
[01_system_overview.md](01_system_overview.md),
[02_source_code_map.md](02_source_code_map.md),
[04_database_schema.md](04_database_schema.md),
[05_embedding_subsystem.md](05_embedding_subsystem.md),
[08_mcp_tools_reference.md](08_mcp_tools_reference.md),
[09_storage_layer.md](09_storage_layer.md),
[11_cuda_acceleration.md](11_cuda_acceleration.md),
[14_test_suite.md](14_test_suite.md),
[16_training_subsystem.md](16_training_subsystem.md),
[17_constellation_subsystem.md](17_constellation_subsystem.md),
[18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

---

## 1. Codebase Metrics

| Metric | Value | Command |
|---|---:|---|
| Workspace crates | 10 | `ls crates/` |
| Rust source files (`crates/*/src/**/*.rs`) | 1,109 | `find crates/*/src -name "*.rs" \| wc -l` |
| Rust lines of code (sum) | 384,252 | `find crates/*/src -name "*.rs" -exec wc -l {} + \| tail -1` |
| Integration test files (`crates/*/tests/**/*.rs`) | 68 | `find crates/*/tests -name "*.rs" \| wc -l` |
| Python scripts (`scripts/**/*.py`) | 67 | `find scripts -name "*.py" \| wc -l` |
| Test attribute count (`#[test]` + `#[tokio::test]`, src + tests) | 5,199 | `grep -rhE "^\s*#\[(tokio::)?test\]" crates/*/src crates/*/tests \| wc -l` |
| `#[ignore]` markers | 27 | `grep -rhE '#\[ignore' crates/*/src crates/*/tests \| wc -l` |
| Column families (RocksDB) | 59 | `grep "TOTAL_COLUMN_FAMILIES: usize = " crates/context-graph-storage/src/column_families.rs` |
| MCP tool name constants | 75 | `grep -cE "^pub const [A-Z_]+: &str" crates/context-graph-mcp/src/tools/names.rs` |
| Weight profiles (built-in) | 14 | `crates/context-graph-core/src/weights/mod.rs::WEIGHT_PROFILES` |
| Embedders | 13 (E1–E13) | `crates/context-graph-core/src/types/.../NUM_EMBEDDERS` |

---

## 2. Schema Version Bytes

Each persisted record type carries an explicit `u8` version at offset 0. Bumping a version requires a migration + CF-compatibility check.

| Constant | Value | Defined in | Applies to |
|---|---:|---|---|
| `TRAINING_RECORD_VERSION` | 2 | `crates/context-graph-core/src/training/mod.rs:67` | `CF_TRAINING_RECORDS` payload |
| `TYPED_EDGE_RECORD_VERSION` | 1 | `crates/context-graph-core/src/typed_edge_export/mod.rs:32` | Typed-edge export records |
| `LLM_EDGE_VALIDATION_VERSION` | 1 | `crates/context-graph-core/src/llm_edge_validation/mod.rs:25` | `CF_TYPED_EDGE_VALIDATIONS` |
| `CONSTELLATION_VERSION` | 1 | `crates/context-graph-core/src/constellation/mod.rs:52` | `CF_CONSTELLATIONS` |
| `CONTRASTIVE_PAIR_VERSION` | 1 | `crates/context-graph-core/src/contrastive/mod.rs:49` | `CF_CONTRASTIVE_PAIRS` |
| `TELEOLOGICAL_VERSION` | 1 | `crates/context-graph-storage/src/teleological/serialization.rs:34` | Teleological fingerprint payload |
| `GRAPH_EDGE_VERSION` | 1 | `crates/context-graph-storage/src/graph_edges/serialization.rs:16` | Graph edge payload |
| `STORAGE_VERSION` | 1 | `crates/context-graph-embeddings/src/storage/types/constants.rs:31` | Embedding storage header |
| `CODEBOOK_VERSION` | 1 | `crates/context-graph-embeddings/src/quantization/pq8/types.rs:20` | PQ-8 quantization codebook |

---

## 3. Feature Flag Matrix

| Crate | Default features | Optional features | Purpose |
|---|---|---|---|
| context-graph-core | (none) | `test-utils` | Enables stub constructors / `zeroed()` helpers for downstream tests. Never enable in production builds. |
| context-graph-cuda | `cuda` | `faiss-working` | `cuda`: Blackwell/RTX 5090 required path. `faiss-working`: links `libfaiss_c.so` (requires CUDA 13.1+ FAISS rebuild). |
| context-graph-embeddings | `candle` | `cuda` (alias of `candle`) | `candle` is mandatory — `lib.rs` has `compile_error!` if absent. Pulls `candle-core`/`candle-nn` 0.9.2-alpha with CUDA. |
| context-graph-storage | `cuda` | — | Enables `context-graph-cuda`/`context-graph-embeddings` CUDA paths. |
| context-graph-causal-agent | (none) | `llm`, `cuda` | `llm`: pulls `llama-cpp-2` (~500MB binary). `cuda` = `llm` + `llama-cpp-2/cuda`. |
| context-graph-graph-agent | (none) | `llm`, `cuda`, `test-utils` | `llm` propagates to causal-agent. `cuda` propagates to causal-agent CUDA. |
| context-graph-mcp | `cuda`, `llm` | `candle` | `cuda` = all CUDA paths. `llm` = causal + graph discovery LLM tools. No CPU fallback by design. |
| context-graph-cli | `cuda` | — | Propagates CUDA to `context-graph-mcp` + `context-graph-embeddings`. |
| context-graph-benchmark | (none) | `benchmark-tests`, `real-embeddings` | `real-embeddings`: pulls full embeddings + mcp + graph-agent + causal-agent. |
| context-graph-test-utils | (none) | — | No features; always-on shared helpers. |

---

## 4. Build Prerequisites

| Prerequisite | Value / Constraint | Source |
|---|---|---|
| Rust toolchain channel | `stable` with `rustfmt`, `clippy`, `rust-src` | `rust-toolchain.toml` |
| MSRV (minimum supported Rust version) | 1.75 | `Cargo.toml` `rust-version` |
| Rust edition | 2021 | `Cargo.toml` |
| Target triple | `x86_64-unknown-linux-gnu` | `rust-toolchain.toml` |
| Supported CUDA toolkit | Up to **13.1** (per `cudarc` 0.17.8 / 0.18.2) | `Cargo.lock` — `cudarc 0.17.8` (via candle 0.9.2-alpha), `cudarc 0.18.2` (via newer stack) |
| `CUDA_HOME` (expected) | `/usr/local/cuda-13.1` | `.cargo/config.toml` |
| Candle version | 0.9.2-alpha (with `cuda` feature) | `Cargo.toml` workspace deps |
| llama-cpp-2 version | 0.1 (with optional `cuda` sub-feature) | `Cargo.toml` workspace deps |
| Target GPU arch | Blackwell / sm_120 (RTX 5090) | `context-graph-cuda/Cargo.toml` comments, `context-graph-embeddings/src/models/registry/config.rs` (32 GB budget) |
| GPU VRAM budget (default) | 32 GB (`memory_budget_bytes = 32_000_000_000`) | `crates/context-graph-embeddings/src/models/registry/config.rs:54` |

### 4.1 LLM model files (required for `llm` feature)

| Role | File | Location |
|---|---|---|
| Hermes-2-Pro-Mistral-7B (quantized) | `Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf` | `models/hermes-2-pro/` |
| Causal-analysis grammar | `causal_analysis.gbnf` | `models/hermes-2-pro/` |
| Graph-relationship grammar | `graph_relationship.gbnf` | `models/hermes-2-pro/` |
| Validation grammar | `validation.gbnf` | `models/hermes-2-pro/` |

### 4.2 HuggingFace model weights (required for embedders)

Downloaded via `scripts/download_models.py` (requires `huggingface_hub` + `torch`).

| Embedder | Repo ID | Local dir |
|---|---|---|
| E1 Semantic | `intfloat/e5-large-v2` | `models/semantic/` |
| E5 Causal | `allenai/longformer-base-4096` | `models/causal/` |
| E7 Code | `Qodo/Qodo-Embed-1-1.5B` | `models/code-1536/` |
| E8 Graph | `intfloat/e5-large-v2` (shared) / `sentence-transformers/paraphrase-MiniLM-L6-v2` | `models/graph/` |
| E9 HDC | local (built from `models/hdc/`) | `models/hdc/` |
| E10 Multimodal | `openai/clip-vit-large-patch14` | `models/multimodal/` |
| E10 Contextual (paraphrase) | `intfloat/e5-base-v2` | `models/contextual/` |
| E11 Entity (KEPLER) | local KEPLER weights | `models/kepler/` |
| E11 Entity (legacy) | `sentence-transformers/all-MiniLM-L6-v2` | `models/entity/` |
| E12 Late-interaction | `colbert-ir/colbertv2.0` | `models/late-interaction/` |
| E13 SPLADE-v3 | local SPLADE-v3 weights | `models/splade-v3/` |
| E6 Sparse | `naver/splade-cocondenser-ensembledistil` | `models/sparse/` |

---

## 5. Lint Configuration

Workspace-wide CI lint: `make clippy` → `cargo clippy --workspace --all-targets -- -D warnings` (all warnings are hard errors).

No `clippy.toml` / `.clippy.toml` — lint levels are set per-crate via inner attributes on `lib.rs`:

| Crate | Deny | Allow |
|---|---|---|
| context-graph-mcp | `deprecated` | `clippy::field_reassign_with_default`, `clippy::type_complexity`, `clippy::result_large_err` |
| context-graph-core | `deprecated` | `clippy::field_reassign_with_default`, `clippy::module_inception`, `clippy::needless_range_loop` |
| context-graph-cuda | `deprecated` | (none) |
| context-graph-storage | `deprecated` | `clippy::module_inception`, `clippy::needless_range_loop`, `clippy::type_complexity` |
| context-graph-embeddings | `deprecated` | `clippy::module_inception` (and crate-level `compile_error!` without `candle`) |

---

## 6. Notable Constants

| Constant | Value | Source |
|---|---:|---|
| `TOTAL_COLUMN_FAMILIES` | 59 | `crates/context-graph-storage/src/column_families.rs:397` |
| `NUM_EMBEDDERS` | 13 | `crates/context-graph-core/src/types/...` (referenced by `WEIGHT_PROFILES`) |
| `RRF_K` (core fusion) | 60.0 (f32) | `crates/context-graph-core/src/fusion/mod.rs:41` |
| `RRF_K` (RocksDB fusion) | 60 (i32) | `crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs:49` |
| `CAUSAL_THRESHOLD` | 0.04 | `crates/context-graph-core/src/causal/asymmetric.rs:62` |
| `NON_CAUSAL_THRESHOLD` | 0.008 | `crates/context-graph-core/src/causal/asymmetric.rs:65` |
| `MAX_HNSW_VECTORS_PER_INDEX` | 2,000,000 | `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs:18` |
| `MAX_CAUSAL_NODES` | 500,000 | `crates/context-graph-core/src/causal/scm.rs:195` |
| `MAX_CAUSAL_EDGES` | 2,000,000 | `crates/context-graph-core/src/causal/scm.rs:198` |
| `MAX_TOPICS` | 10,000 | `crates/context-graph-core/src/clustering/manager.rs:88` |
| `MAX_SNAPSHOTS` (stability) | 1,440 | `crates/context-graph-core/src/clustering/stability.rs:38` |
| `MAX_CUSTOM_PROFILES` | 1,000 | `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs:33` |
| `MAX_BATCH_SIZE` (JSON-RPC) | 100 | `crates/context-graph-mcp/src/server/transport.rs:36` |
| `MAX_LINE_BYTES` (stdio) | 10 MiB | `crates/context-graph-mcp/src/server/transport.rs:32` |
| `MAX_CONTENT_SIZE` (memory node) | 1,048,576 B (1 MiB) | `crates/context-graph-core/src/types/memory_node/mod.rs:74` |
| `MAX_CONTENT_LENGTH` (memory insert) | 10,000 | `crates/context-graph-core/src/memory/mod.rs:144` |
| `MAX_SPARSE_ACTIVE` | 1,526 (= ⌊30522 × 0.05⌋) | `crates/context-graph-core/src/types/fingerprint/sparse.rs:29` |
| `MAX_RECENT_MEMORIES` | 50 | `crates/context-graph-core/src/retrieval/config.rs:35` |
| `DEFAULT_MAX_SPARSE_DOCS` | 500,000 | `crates/context-graph-core/src/retrieval/sparse_index.rs:46` |
| `MAX_CHAIN_LENGTH` (causal chain) | 10 | `crates/context-graph-core/src/causal/chain.rs:45` |
| `MAX_TOPIC_DEPTH` | 3 | `crates/context-graph-core/src/clustering/topic.rs:572` |
| `DEFAULT_MAX_MEMBERS` (constellation) | 50,000 | `crates/context-graph-core/src/constellation/compiler.rs:46` |
| `DEFAULT_MAX_PAIRS` (contrastive) | 10,000 | `crates/context-graph-core/src/contrastive/types.rs:24` |
| `DEFAULT_EXPORT_MAX_EDGES` | 10,000 | `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs:43` |
| `DEFAULT_VALIDATE_MAX_EDGES` | 100 | `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs:46` |
| `MAX_QUANTIZED_SIZE_BYTES` | 25,000 | `crates/context-graph-embeddings/src/storage/types/constants.rs:21` |
| `MAX_ALLOCATION_HISTORY` (CUDA alloc) | 100 | `crates/context-graph-embeddings/src/warm/cuda_alloc/constants.rs:32` |
| `CODE_MAX_TOKENS` (Qodo) | 32,768 | `crates/context-graph-embeddings/src/models/pretrained/code/constants.rs:13` |
| `SEMANTIC_MAX_TOKENS` (E1/E5/E10) | 512 | `crates/context-graph-embeddings/src/models/pretrained/{semantic,causal,contextual,late_interaction}/constants.rs` |
| `MAX_TIME_DELTA_SECS` (E2 temporal) | 31,536,000 (1 yr) | `crates/context-graph-embeddings/src/models/custom/temporal_recent/constants.rs:22` |

---

## 7. Weight Profiles

Defined as `WEIGHT_PROFILES: &[(&str, [f32; NUM_EMBEDDERS])]` in `crates/context-graph-core/src/weights/mod.rs`. 14 entries, each a 13-element weight vector over E1–E13.

| # | Name | Primary embedder(s) | Purpose |
|---:|---|---|---|
| 1 | `semantic_search` | E1 (0.33), E7 (0.20), E5 (0.15), E10 (0.15) | Default general-purpose query routing |
| 2 | `causal_reasoning` | E1 (0.40), E7 (0.15), E5 (0.10), E8 (0.10), E10 (0.10), E11 (0.10) | "Why" / cause-effect queries; E5 demoted to binary gate signal |
| 3 | `code_search` | E7 (0.40), E1 (0.20) | Programming queries, function signatures |
| 4 | `fact_checking` | E11 primary, E6 for keywords | Entity / fact verification |
| 5 | `graph_reasoning` | E8 (graph) + E1 | Relational graph walks |
| 6 | `temporal_navigation` | E2/E3/E4 temporal embedders | Time-anchored retrieval |
| 7 | `sequence_navigation` | E4 positional | Conversation-turn ordering |
| 8 | `conversation_history` | E4 + E1 | Prior-turn recall |
| 9 | `category_weighted` | Balanced across categories | Heuristic rerank input |
| 10 | `typo_tolerant` | E9 HDC primary | Robust noisy-input retrieval |
| 11 | `pipeline_stage1_recall` | E13 SPLADE (inverted-index recall only) | Stage 1 of 3-stage pipeline |
| 12 | `pipeline_stage2_scoring` | Dense ensemble | Stage 2 dense scoring |
| 13 | `pipeline_full` | Full E13 → E1 → E12 pipeline | 3-stage cascade |
| 14 | `balanced` | Equal-ish across all semantic embedders | Fallback baseline |

Invariant: E12 (Late-Interaction) and E13 (SPLADE) carry weight 0.0 in every *semantic-scoring* profile — they participate only as Stage-1 recall (E13 inverted index) and Stage-3 re-ranking (E12 MaxSim) per AP-73 / AP-74.

---

## 8. MCP Tool Surface

75 tools registered as `pub const <NAME>: &str = "<name>"` in `crates/context-graph-mcp/src/tools/names.rs`. Tools cluster into: memory CRUD, 4 search strategies, per-embedder search, weight profiles, causal (E5 + LLM), graph (E8 + LLM), entity (E11), topics, sessions, provenance, file-watcher, typed-edge export/validate, constellation, contrastive pairs, training records. Full surface in [08_mcp_tools_reference.md](08_mcp_tools_reference.md).

---

## 9. Persistence Surface

- **59** RocksDB column families (`TOTAL_COLUMN_FAMILIES`): 11 base + 20 teleological + 13 quantized + 5 code + 2 causal + 8 newer (constellation, contrastive, training, typed-edge validations, selectors).
- **15** HNSW indexes: E1, E1Matryoshka128, E2, E3, E4, E5 + E5CausalCause + E5CausalEffect, E7, E8 (source + target), E9, E10MultimodalParaphrase + E10MultimodalContext, E11.
- Asymmetric embedders (E5 / E8 / E10) store **dual** vectors.

Detailed CF map: [04_database_schema.md](04_database_schema.md). Storage layer: [09_storage_layer.md](09_storage_layer.md).

---

## 10. Re-verify These Numbers

```bash
# Source counts
find crates/*/src -name "*.rs" | wc -l
find crates/*/src -name "*.rs" -exec wc -l {} + | tail -1
find crates/*/tests -name "*.rs" | wc -l
find scripts -name "*.py" | wc -l

# Test attrs per crate
for c in crates/*; do
  echo "=== $c ==="
  grep -rhE "^\s*#\[(tokio::)?test\]" "$c" 2>/dev/null | wc -l
done

# Ignored tests
grep -rhE '#\[ignore' crates/*/src crates/*/tests | wc -l

# Config constants
grep "TOTAL_COLUMN_FAMILIES: usize = " crates/context-graph-storage/src/column_families.rs
grep -cE "^pub const [A-Z_]+: &str" crates/context-graph-mcp/src/tools/names.rs

# Schema versions
grep -rnE "^pub const [A-Z_]+VERSION[A-Z_]*: u8 = " crates/ | grep -v target

# cudarc version
grep -A 2 '^name = "cudarc"' Cargo.lock

# Rust toolchain
cat rust-toolchain.toml
grep "rust-version" Cargo.toml
```
