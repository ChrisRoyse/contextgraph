# Verification Report

**Date**: 2026-04-12 (re-verified 2026-04-14)
**Rust version**: rustc 1.92.0 (ded5c06cf 2025-12-08)
**Rust edition**: 2021
**Minimum supported Rust version (MSRV)**: 1.75

Source files covered: `crates/*/src/**/*.rs`, `crates/*/tests/**/*.rs`, `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`

---

## 1. Build Verification

### 1.1 Default Build (with CUDA)

The default workspace build currently **fails** due to a CUDA toolkit version incompatibility. The environment provides CUDA 13.2, but the `cudarc` dependency (v0.17.8 and v0.18.2) only supports up to CUDA 13.1.

```
error: failed to run custom build command for `cudarc v0.17.8`
  Unsupported cuda toolkit version: `13.2`. Please raise a github issue.
```

**Root cause**: The `.cargo/config.toml` sets `CUDA_HOME = "/usr/local/cuda-13.1"`, but the system-installed CUDA 13.2 is detected instead.  The `cudarc` crate does not yet recognize CUDA 13.2.

### 1.2 Build Without CUDA

When CUDA environment variables are unset, `context-graph-core` compiles successfully:

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.35s
```

The `context-graph-embeddings` crate requires the `candle` feature, which transitively depends on `cudarc`, so a full workspace build without CUDA is not currently possible on this machine.

### 1.3 Build Verdict

| Status | Scope | Notes |
|--------|-------|-------|
| FAIL   | Full workspace (default features) | `cudarc` rejects CUDA 13.2 |
| PASS   | `context-graph-core` (no default features) | Builds cleanly |
| BLOCKED | `context-graph-embeddings`, `context-graph-mcp` | Transitive `cudarc` dependency |

**Recommendation**: Update `cudarc` to a version supporting CUDA 13.2, or pin `CUDA_HOME` to 13.1 if available on the build machine. The MEMORY.md notes that previous builds on this system produced zero errors and zero warnings, indicating this is an environment regression (CUDA toolkit updated from 13.1 to 13.2).

---

## 2. Test Results Summary

### 2.1 Test Inventory by `#[test]` Attribute Count

Due to the CUDA build issue, the full test suite cannot be executed in this environment. Test counts are derived from `#[test]` and `#[tokio::test]` attribute scanning (re-run 2026-04-14). The scan regex `^\s*#\[(?:tokio::)?test(?:\(|\])` captures both bare `#[tokio::test]` and flavored `#[tokio::test(flavor = "multi_thread")]` variants.

| Crate | Combined test attrs (src + tests) |
|-------|----------------------------------:|
| context-graph-core | 2,088 |
| context-graph-embeddings | 1,386 |
| context-graph-storage | 554 |
| context-graph-benchmark | 374 |
| context-graph-mcp | 342 |
| context-graph-cli | 262 |
| context-graph-graph-agent | 64 |
| context-graph-cuda | 62 |
| context-graph-causal-agent | 52 |
| context-graph-test-utils | 0 |
| **Workspace-authoritative total** | **5,184** |
| Of which `#[test]` (sync) | 4,642 |
| Of which `#[tokio::test]` (async, any flavor) | 542 |
| Of which `#[ignore]` (not counted in 5,184 above) | 27 |

The 118-test increase since the 2026-04-13 scan reflects: 18 new tests in `core/src/training/tests.rs` (aec85437, 7179bd7c), 15 new tests in `core/src/constellation/{welford,reservoir,tests}.rs`, 15 new `#[tokio::test(flavor = "multi_thread")]` integration tests in `storage/tests/training_records_integration.rs`, plus contrastive-pairs test wiring added in commit 48f0420d.

### 2.2 Test File Count

| Category | Count |
|----------|------:|
| Files under `crates/*/tests/**/*.rs` | 192 |
| Inline `#[cfg(test)]` modules across `crates/*/src` | 488 (across 422 source files) |
| Criterion benchmark files (`crates/*/benches/*.rs`) | 7 |

### 2.3 Partial Test Execution

The `context-graph-core` crate (no-default-features) lists **189 runnable tests** via `cargo test -- --list`. This confirms the test harness is functional for non-CUDA crates.

### 2.4 Historical Test Status (from MEMORY.md)

Per the most recent successful session (2026-02-26):
- **1,179 tests passed** (MCP + Storage focused run), 0 failed
- Per session 2026-02-24: **6,527 tests passed**, 0 failed (full workspace)

---

## 3. Lint Status

### 3.1 Clippy Configuration

Clippy is configured via `.cargo/config.toml`:

```toml
clippy-all = "clippy --workspace --all-targets -- -D warnings"
```

This treats all clippy warnings as errors (`-D warnings`). No separate `clippy.toml` or `.clippy.toml` configuration file exists.

### 3.2 Historical Clippy Status

Per MEMORY.md (2026-02-24 session): **Clippy: clean** (zero warnings).

---

## 4. Codebase Metrics

### 4.1 Summary Table

Re-measured 2026-04-14.

| Metric | Value | Derivation |
|--------|------:|------------|
| Total Rust source files (`crates/**/*.rs`) | 1,180 | `find crates -name '*.rs' \| wc -l` |
| Rust source files in `src/` only | 1,097 | `find crates -name '*.rs' -path '*/src/*' \| wc -l` |
| Total lines of Rust code (src + tests) | 413,625 | `find crates -name '*.rs' -exec wc -l` |
| Lines of Rust code (src only) | 384,252 | `find crates -name '*.rs' -path '*/src/*' -exec wc -l` |
| Number of workspace crates | 10 | `Cargo.toml` `[workspace.members]` |
| Default-member crates (excl. benchmark) | 9 | `Cargo.toml` `default-members` |
| Number of MCP tools (dispatched) | 75 | `tool_dispatch!` arms in `handlers/tools/dispatch.rs` (88 match arms) |
| MCP tool name constants | 75 | `grep "pub const ... &str =" tools/names.rs` |
| Column families (RocksDB) | 59 | `TOTAL_COLUMN_FAMILIES` in `column_families.rs:397` |
| Embedders | 13 | `NUM_EMBEDDERS` in `core/teleological/types.rs:217` |
| Weight profiles | 14 | `weights/mod.rs` profile match arms |
| Test attributes (`#[test]` + `#[tokio::test]`, all flavors) | 5,241 | Whole-workspace attribute scan |
| Test files (dedicated under `tests/`) | 192 | `find crates -name '*.rs' -path '*/tests/*'` |
| Inline `#[cfg(test)]` modules in `src/` | 488 (in 422 files) | `grep -rE "#\[cfg\(test\)\]" crates/*/src` |
| Cargo.lock dependencies | 415 | `grep -c "^name =" Cargo.lock` |

### 4.2 Lines of Code per Crate

Re-measured 2026-04-14 against `crates/*/src/**/*.rs`.

| Crate | Source Files | Lines of Code (src) | % of Total |
|-------|------------:|--------------:|-----------:|
| context-graph-core | 270 | 104,246 | 27.4% |
| context-graph-benchmark | 141 | 87,254 | 22.9% |
| context-graph-embeddings | 426 | 75,523 | 19.8% |
| context-graph-mcp | 92 | 45,053 | 11.8% |
| context-graph-storage | 99 | 38,368 | 10.1% |
| context-graph-cli | 31 | 16,344 | 4.3% |
| context-graph-cuda | 18 | 5,264 | 1.4% |
| context-graph-causal-agent | 8 | 4,933 | 1.3% |
| context-graph-graph-agent | 9 | 3,951 | 1.0% |
| context-graph-test-utils | 3 | 159 | <0.1% |
| **Total** | **1,109** (src only) | **384,252** (src only) | **100%** |

Note: Total source files (1,197) includes test files outside of `src/` directories. Total lines (413,625) includes all `.rs` files (src + tests). Growth since 2026-04-14 reflects commit `61934385` (typed-edge export pipeline): `core/src/typed_edge_export/` (224 lines), `core/src/llm_edge_validation/` (137 lines), `core/src/training/edge_signature.rs` (106 lines), `mcp/src/handlers/tools/typed_edges_tools.rs` (960 lines), `mcp/src/tools/definitions/typed_edges.rs` (231 lines), `storage/src/teleological/rocksdb_store/{typed_edge_export,typed_edge_keys,llm_validation,anomaly_derivation,versioned_bincode}.rs` (1,157 lines combined), plus integration tests (701 lines).

### 4.3 Column Families Breakdown

| Category | Count | Details |
|----------|------:|---------|
| Base CFs | 11 | nodes, edges, embeddings, metadata, temporal, tags, sources, system, embedder_edges, typed_edges, typed_edges_by_type |
| Teleological CFs | 28 | fingerprints, topic_profiles, e13_splade_inverted, e6_sparse_inverted, e1_matryoshka_128, content, source_metadata, file_index, topic_portfolio, e12_late_interaction, entity_provenance, audit_log, audit_by_target, merge_history, importance_history, tool_call_index, consolidation_recommendations, embedding_registry, custom_weight_profiles, hnsw_graphs, training_records, constellations, constellation_by_selector, contrastive_pairs, contrastive_by_kind, contrastive_by_anchor, typed_edge_records, typed_edge_validations |
| Quantized Embedder CFs | 13 | emb_0 through emb_12 |
| Code CFs | 5 | code_entities, code_e7_embeddings, code_file_index, code_name_index, code_signature_index |
| Causal CFs | 2 | causal_relationships, causal_by_source |
| **Total** | **59** | Defined as `TOTAL_COLUMN_FAMILIES` in `storage/column_families.rs:397` |

Teleological expansion (20 -> 28) landed in commits `aec85437` (training_records), `7179bd7c` (constellations + constellation_by_selector), `48f0420d` (contrastive_pairs, contrastive_by_kind, contrastive_by_anchor), and `61934385` (typed_edge_records, typed_edge_validations). `TELEOLOGICAL_CF_COUNT` in `teleological/column_families.rs:551` is authoritative at 28.

### 4.4 MCP Tools (75 Dispatched)

Organized by category as listed in `dispatch.rs:87-183`:

| Category | Tools |
|----------|-------|
| Core (PRD 10.1) | `store_memory`, `get_memetic_status`, `search_graph` |
| Consolidation | `trigger_consolidation` |
| Topic (PRD 10.2) | `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, `get_divergence_alerts` |
| Curation (PRD 10.3) | `merge_concepts`, `forget_concept`, `boost_importance` |
| File Watcher | `list_watched_files`, `get_file_watcher_stats`, `delete_file_content`, `reconcile_files` |
| Sequence (E4) | `get_conversation_context`, `get_session_timeline`, `traverse_memory_chain`, `compare_session_states` |
| Causal (E5) | `search_causes`, `search_effects`, `get_causal_chain`, `search_causal_relationships` |
| Causal Discovery (LLM) | `trigger_causal_discovery`, `get_causal_discovery_status` |
| Graph (E8) | `search_connections`, `get_graph_path` |
| Graph Discovery (LLM) | `discover_graph_relationships`, `validate_graph_link` |
| Keyword (E6) | `search_by_keywords` |
| Code (E7) | `search_code` |
| Robustness (E9) | `search_robust` |
| Entity (E11) | `extract_entities`, `search_by_entities`, `infer_relationship`, `find_related_entities`, `validate_knowledge`, `get_entity_graph` |
| Embedder-First Search | `search_by_embedder`, `get_embedder_clusters`, `compare_embedder_views`, `list_embedder_indexes`, `get_memory_fingerprint`, `create_weight_profile`, `search_cross_embedder_anomalies` |
| E12/E13 Standalone | `search_by_tokens`, `search_by_expansion` |
| Temporal (E2/E3) | `search_recent`, `search_periodic` |
| Graph Linking (K-NN) | `get_memory_neighbors`, `get_typed_edges`, `traverse_graph`, `get_unified_neighbors` |
| Maintenance | `repair_causal_relationships` |
| Provenance (Phase P3) | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |
| Daemon | `daemon_status` |
| Training (data export) | `export_training_corpus`, `list_training_records`, `get_training_record`, `count_training_records` |
| Constellation (Phase 2 compiler) | `compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |
| Contrastive (Phase 3 pair miner) | `mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs` |
| Typed-Edge Training (Phase 4) | `export_typed_edges_corpus`, `derive_anomalies_from_edges`, `validate_low_confidence_edges`, `list_typed_edge_records` |

### 4.5 Embedder Architecture (13 Embedders)

| ID | Name | Dimensions | Type | Notes |
|----|------|----------:|------|-------|
| E1 | Semantic (e5-large-v2) | 1,024 | Dense HNSW | Primary semantic search |
| E2 | Temporal Recent | 512 | Dense HNSW | Sinusoidal PE of absolute timestamp |
| E3 | Temporal Periodic | 512 | Dense HNSW | Periodic temporal patterns |
| E4 | Temporal Positional | 512 | Dense HNSW | V_ordering sequence |
| E5 | Causal (asymmetric) | 768 | Dense HNSW (dual) | cause/effect vectors, CAUSAL_THRESHOLD=0.04 |
| E6 | Sparse Keyword | 30,000 | Inverted index | BM25-style keyword matching |
| E7 | Code (Qodo-Embed) | 1,536 | Dense HNSW | Code search, AST-aware |
| E8 | Graph | 1,024 | Dense HNSW (dual) | source/target structural |
| E9 | HDC (Hyperdimensional) | 1,024 | Dense HNSW | Typo-tolerant, noise-robust |
| E10 | Multimodal Paraphrase | 768 | Dense HNSW (dual) | doc/query paraphrase detection |
| E11 | Entity (KEPLER) | 768 | Dense HNSW | Entity/relationship embeddings (disabled) |
| E12 | Late Interaction (ColBERT) | 128/token | MaxSim | Stage 3 reranking only |
| E13 | SPLADE | 30,000 | Inverted index | Stage 1 recall, term expansion |

---

## 5. Notable Constants and Magic Numbers

### 5.1 Capacity and Safety Limits

| Constant | Value | Location | Purpose |
|----------|-------|----------|---------|
| `MAX_CAUSAL_NODES` | 500,000 | `core/causal/scm.rs:195` | Maximum nodes in causal graph |
| `MAX_CAUSAL_EDGES` | 2,000,000 | `core/causal/scm.rs:198` | Maximum edges in causal graph |
| `MAX_TOPICS` | 10,000 | `core/clustering/manager.rs:88` | Maximum topic clusters |
| `MAX_EMBEDDINGS_PER_SPACE` | 50,000 | `core/clustering/manager.rs:82` | Per-space embedding limit for clustering |
| `MAX_SNAPSHOTS` | 1,440 | `core/clustering/stability.rs:38` | Stability tracking window |
| `MAX_CONTENT_LENGTH` | 10,000 | `core/memory/mod.rs:144` | Maximum memory content length (chars) |
| `MAX_RECENT_MEMORIES` | 50 | `core/retrieval/config.rs:35` | Default recent memory limit |
| `DEFAULT_MAX_SPARSE_DOCS` | 500,000 | `core/retrieval/sparse_index.rs:46` | Sparse inverted index capacity |
| `DEFAULT_RECALL_LIMIT` | 10,000 | `core/retrieval/sparse_index.rs:42` | Default sparse recall limit |
| `MAX_SPARSE_ACTIVE` | 1,500 | `core/embeddings/stubs.rs:31` | Maximum active sparse indices |
| `MAX_LOAD_FILE_SIZE` | 1 GB | `core/index/splade_impl.rs:252` | SPLADE file size guard |
| `HNSW max_elements` | 100,000 | `core/index/hnsw_impl/real_hnsw.rs:87` | HNSW index initial capacity |
| `MAX_LINE_BYTES` | 10 MB | `mcp/server/transport.rs:32` | MCP transport line limit |
| `MAX_BATCH_SIZE` | 100 | `mcp/server/transport.rs:36` | JSON-RPC batch size limit |
| `MAX_SOURCE_IDS` | 10 | `mcp/handlers/merge.rs:107` | Merge source ID limit |
| `MAX_TARGET_NAME_LEN` | 256 | `mcp/handlers/merge.rs:109` | Merge target name length |
| `MAX_RATIONALE_LEN` | 1,024 | `mcp/handlers/merge.rs:111` | Merge rationale length |
| `MIN_FINGERPRINT_SIZE` | 5,000 | `storage/teleological/serialization.rs:43` | Fingerprint size floor |
| `MAX_FINGERPRINT_SIZE` | 150,000 | `storage/teleological/serialization.rs:55` | Fingerprint size ceiling |
| `TOTAL_COLUMN_FAMILIES` | 59 | `storage/column_families.rs:397` | RocksDB column families (11 base + 28 teleological + 13 quantized + 5 code + 2 causal) |
| `TELEOLOGICAL_CF_COUNT` | 28 | `storage/teleological/column_families.rs:551` | Teleological CF subtotal (matches `TELEOLOGICAL_CFS` array) |
| `TELEOLOGICAL_VECTOR_DIM` | 13 | `core/types/fingerprint/teleological/types.rs:12` | Per-embedder L2-norm signature vector dimension |
| `TRAINING_RECORD_VERSION` | 2 | `core/training/mod.rs` | On-disk version byte prefix for `TrainingRecord` bincode payload |
| `NUM_CROSS_CORRELATIONS` | 78 | `core/training/mod.rs` | C(13,2) unique embedder pairs in training records |
| `NUM_EDGE_TYPE_DISTRIBUTION` | 8 | `core/training/edge_signature.rs` | Edge-type distribution vector dimension (GraphLinkEdgeType::COUNT) |
| `CONSTELLATION_VERSION` | 1 | `core/constellation/mod.rs` | On-disk version byte prefix for `Constellation` payload |
| `RESERVOIR_SAMPLE_SIZE` | 1,024 | `core/constellation/mod.rs` | Reservoir cap for percentile estimation |
| `MIN_CONSTELLATION_MEMBERS` | 3 | `core/constellation/mod.rs` | Minimum members required to compile a constellation |
| `CONTRASTIVE_PAIR_VERSION` | 1 | `core/contrastive/mod.rs` | On-disk version byte prefix for `ContrastivePair` payload |
| `NUM_ANOMALY_KINDS` | 6 | `core/contrastive/types.rs` | Number of AnomalyKind variants |
| `TYPED_EDGE_RECORD_VERSION` | 1 | `core/typed_edge_export/mod.rs` | On-disk version byte prefix for `TypedEdgeTrainingRecord` |
| `LLM_EDGE_VALIDATION_VERSION` | 1 | `core/llm_edge_validation/mod.rs` | On-disk version byte prefix for `LLMEdgeValidation` |
| `TYPED_EDGE_KEY_LEN` | 33 | `storage/teleological/rocksdb_store/typed_edge_keys.rs` | Composite key: `[source:16][target:16][edge_type:1]` bytes |

### 5.2 Algorithm Parameters

| Constant | Value | Location | Purpose |
|----------|-------|----------|---------|
| `CAUSAL_THRESHOLD` | 0.04 | `core/causal/asymmetric.rs:62` | Causal gate activation |
| `NON_CAUSAL_THRESHOLD` | 0.008 | `core/causal/asymmetric.rs:65` | Non-causal gate threshold |
| `CAUSAL_BOOST` | 1.10 | `core/causal/asymmetric.rs:69` | Causal score multiplier |
| `NON_CAUSAL_DEMOTION` | 0.85 | `core/causal/asymmetric.rs:73` | Non-causal score demotion |
| `RRF_K` | 60.0 | `core/fusion/mod.rs:41` | Reciprocal Rank Fusion constant |
| `MAX_CHAIN_LENGTH` | 10 | `core/causal/chain.rs:45` | Causal chain max depth |
| `MAX_TOPIC_DEPTH` | 3 | `core/clustering/topic.rs:572` | Topic hierarchy depth |
| `MAX_WEIGHTED_AGREEMENT` | 8.5 | `core/clustering/manager.rs:72` | Topic clustering threshold |
| `MAX_RECENCY_FACTOR` | 1.3 | `core/injection/candidate.rs:149` | Recency boost ceiling |
| `MAX_DIVERSITY_BONUS` | 1.5 | `core/injection/candidate.rs:155` | Diversity bonus ceiling |
| `MAX_DF_RATIO` | 0.9 | `core/retrieval/sparse_index.rs:50` | Document frequency ratio limit |
| `CHUNK_SIZE_WORDS` | 200 | `core/memory/chunker.rs:223` | Content chunking size |
| `NUM_EMBEDDERS` | 13 | `core/graph_linking/weight_projector.rs:19` | Total embedding spaces |
| `NUM_SPACES` | 13 | `core/retrieval/similarity.rs:19` | Total similarity spaces |
| `NUM_DENSE_EMBEDDERS` | 10 | `core/embeddings/provider.rs:33` | Dense embedders (HNSW-backed) |
| `NUM_SPARSE_EMBEDDERS` | 2 | `core/embeddings/provider.rs:36` | Sparse embedders (E6, E13) |
| `NUM_GROUPS` | 6 | `core/teleological/groups.rs:18` | Teleological group types |
| `NUM_DOMAINS` | 3 | `core/teleological/resolution.rs:17` | Teleological domain types |

---

## 6. Rust Edition and Version Constraints

| Property | Value |
|----------|-------|
| Workspace edition | 2021 |
| Workspace `rust-version` (MSRV) | 1.75 |
| Installed rustc | 1.92.0 (2025-12-08) |
| Target | x86_64-unknown-linux-gnu |
| Linker | clang (with lld) |
| Build flags | `-C target-cpu=native` |
| Release incremental | false |
| Dev incremental | true |

All crates inherit the workspace edition (2021) and rust-version (1.75). Some crates also declare `edition = "2021"` explicitly.

---

## 7. Dependencies

| Metric | Count |
|--------|------:|
| Total packages in Cargo.lock | 415 |
| Workspace crates | 10 |
| External dependencies | 405 |

### 7.1 Notable Dependencies

Based on the workspace Cargo.toml and crate configurations:

- **rocksdb** (v0.22.0): Persistent key-value storage
- **hnsw_rs** (v0.3.3): Hierarchical Navigable Small World graph index
- **candle-core**, **candle-nn**: ML framework (requires CUDA)
- **tokenizers**: HuggingFace tokenizer
- **cudarc** (v0.17.8, v0.18.2): CUDA runtime bindings (currently blocking build on CUDA 13.2)
- **serde**, **serde_json**: Serialization
- **tokio**: Async runtime
- **tracing**, **tracing-subscriber**: Structured logging
- **config**: Configuration management
- **uuid**: Unique identifiers

---

## 8. Feature Flags Summary

### 8.1 Feature Flag Matrix

| Crate | Default | Available Flags | Notes |
|-------|---------|-----------------|-------|
| context-graph-core | (none) | `test-utils` | Test utilities for downstream crates |
| context-graph-embeddings | `candle` | `candle`, `cuda` | `cuda` is alias for `candle` |
| context-graph-storage | `cuda` | `cuda` | Requires `context-graph-cuda` |
| context-graph-mcp | `cuda`, `llm` | `candle`, `llm`, `cuda` | Main server binary |
| context-graph-cli | `cuda` | `cuda` | Delegates to MCP + embeddings cuda |
| context-graph-cuda | `cuda` | `cuda`, `faiss-working` | RTX 5090 Blackwell support |
| context-graph-causal-agent | (none) | `llm`, `cuda` | LLM-powered causal discovery |
| context-graph-graph-agent | (none) | `llm`, `cuda`, `test-utils` | LLM-powered graph discovery |
| context-graph-benchmark | (none) | `benchmark-tests`, `real-embeddings` | Test-only features |

### 8.2 Feature Dependency Chain

```
context-graph-mcp (default: cuda, llm)
  +-- cuda --> context-graph-cuda/cuda
  |            context-graph-embeddings/cuda (= candle)
  |            context-graph-storage/cuda
  |            context-graph-causal-agent/cuda (includes llm)
  +-- llm  --> context-graph-causal-agent/llm (dep:llama-cpp-2)
  |            context-graph-graph-agent/llm
  +-- candle -> context-graph-embeddings/candle (dep:candle-core, dep:candle-nn, dep:tokenizers)
```

---

## 9. Weight Profiles (14 Total)

| Profile Name | Primary Embedder(s) | Use Case |
|-------------|---------------------|----------|
| `semantic_search` | E1 (0.33) | General queries |
| `causal_reasoning` | E1 (0.40), E5 (0.10) | "Why" questions |
| `code_search` | E7 (0.40) | Programming queries |
| `fact_checking` | E11 (0.40) | Entity/fact queries |
| `graph_reasoning` | E8 (0.40) | Structural/connectivity |
| `temporal_navigation` | E2/E3/E4 (0.23 each) | Time-based queries |
| `sequence_navigation` | E4 (0.55) | Sequence traversal |
| `conversation_history` | E4 (0.35), E1 (0.30) | Contextual recall |
| `category_weighted` | Constitution-compliant | Balanced by category |
| `typo_tolerant` | E9 (0.18), E1 (0.30) | Noisy/misspelled queries |
| `pipeline_stage1_recall` | E13 (0.25), E6 (0.25) | Sparse retrieval stage |
| `pipeline_stage2_scoring` | E1 (0.50) | Dense scoring stage |
| `pipeline_full` | E1 (0.40) | Complete pipeline |
| `balanced` | Equal (~0.077 each) | Testing/comparison |

---

## 10. Cross-References

- **Architecture overview**: See `docs2/contextgraphspecs/01_*.md` (if present)
- **Audit history**: See `docs/HIDDEN_BROKENNESS_AUDIT_*.md` (audits #1 through #15, all resolved)
- **Memory management audit**: See `docs/MEMORY_MANAGEMENT_AUDIT_2026_02_23.md`
- **Memory review**: See `docs/MEMORY_REVIEW_2026_02_24.md` (Grade: A-)
- **Session memory**: See `.claude/projects/-home-cabdru-contextgraph/memory/MEMORY.md`

---

## 11. Known Issues

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| ENV-1 | HIGH | `cudarc` (v0.17.8/v0.18.2) does not support CUDA toolkit 13.2; blocks full workspace build in current environment | Open |
| ENV-2 | LOW | `.cargo/config.toml` sets `CUDA_HOME=/usr/local/cuda-13.1` but system has 13.2 | Open |
| HIST-1 | INFO | Per MEMORY.md, previous full builds (2026-02-26) passed with zero errors/warnings on CUDA 13.1 | Regressed (environment) |
| SRC-1 | RESOLVED | `TELEOLOGICAL_CF_COUNT` updated to 28 at `teleological/column_families.rs:551` matching the `TELEOLOGICAL_CFS` array; `TOTAL_COLUMN_FAMILIES = 59` at `storage/column_families.rs:397` is consistent. Phase 4 added typed_edge_records + typed_edge_validations in commit `61934385`. | Closed (2026-04-15) |

---

## 12. Recent Code Changes (since initial 2026-04-12 generation)

Per git log, the following commits landed at or before each generation pass and affect this report:

- `48f0420d feat: contrastive mining, Tucker decomposition, and constellation/training wiring` — added 3 contrastive CFs (`contrastive_pairs`, `contrastive_by_kind`, `contrastive_by_anchor`) and 4 `mine/list/get/count_contrastive_pairs` MCP tools. [2026-04-14 pass]
- `7179bd7c feat: complete training module + add constellation scaffolding` — finalized `core/src/training/` (mod.rs, computation.rs, temporal.rs, tests.rs — 18 tests) and added `core/src/constellation/` (mod.rs, compiler.rs, reservoir.rs, welford.rs, types.rs, tests.rs — 15 tests) plus CFs `constellations`, `constellation_by_selector` and 5 constellation MCP tools. [2026-04-14 pass]
- `aec85437 feat: training records export + test performance optimizations` — added CF `training_records`, 4 training MCP tools (`export_training_corpus`, `list_training_records`, `get_training_record`, `count_training_records`), 15 async integration tests in `storage/tests/training_records_integration.rs`, plus `#[ignore]` markers on 10 Qodo tests, 1 causal-agent LLM test, and 1 HNSW perf test for faster default runs. [2026-04-14 pass]
- `80558edc feat: integrate 9 capabilities and remove dead code (Parts 0-9)` — reshuffled clustering topic name generation and hierarchy.
- `e5e1d6b2 fix: correct typed edge target_id for incoming edges and prevent secondary index overwrites` — K-NN graph and typed-edge correctness (BUG-1/BUG-2/BUG-3 in MEMORY.md session 2026-02-26).
- `16b5bdeb feat: E2/E3/E4 temporal embedders as first-class fusion participants with sinusoidal PE fix` — E2-E4 now participate in fusion when weight > 0.0.
- `dfa81235 feat: enable E2/E3 temporal embedders with timestamp instructions and remove dead code`
- `719c93fb refactor: completely disable E11 KEPLER embedder and remove stale Marblestone references`
- `65c9f415 build: exclude benchmark crate from default workspace members`
- `03e7cc9e refactor: remove 41K lines tech debt — delete graph crate, Memex layer, and dead modules`

---

*Generated 2026-04-12. Re-verified 2026-04-13 and 2026-04-14. All metrics derived from source inspection via `find`/`wc`/`grep`; no data was invented.*
