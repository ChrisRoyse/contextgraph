# Test Suite

Source files covered: all `crates/*/src/` inline `#[cfg(test)]` modules, all `crates/*/tests/` integration test files, `crates/context-graph-test-utils/`, `crates/context-graph-benchmark/benches/`, `crates/context-graph-storage/benches/`, `crates/context-graph-cli/benches/`, `Makefile`, `.cargo/config.toml`, workspace `Cargo.toml`.

Cross-references: [05_embedding_subsystem.md](05_embedding_subsystem.md), [09_storage_layer.md](09_storage_layer.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [02_source_code_map.md](02_source_code_map.md), [15_verification_report.md](15_verification_report.md).

---

## 1. Test Suite Overview

Re-verified 2026-04-15 by attribute scanning against `crates/*/src` and `crates/*/tests`. Counts grew since the 2026-04-14 measurement because of commit `61934385` (typed-edge export pipeline — ~30 new tests across `core/src/typed_edge_export/tests.rs`, `core/src/llm_edge_validation/tests.rs`, `storage/tests/{typed_edge_export,llm_validation,anomaly_derivation}_integration.rs`, `mcp/src/handlers/tools/typed_edges_tools.rs`).

| Metric | Value |
|--------|-------|
| Total `#[test]` attributes (src + tests) | 4,642 |
| Total `#[tokio::test]` async tests (any flavor) | 542 |
| Grand total test attrs | 5,184 |
| Of which marked `#[ignore]` (opt-in via `--ignored`) | 27 |
| Total assertions (`assert!`/`assert_eq!`/`assert_ne!`) | ~13,700 (drift ±50) |
| FSV verification markers (`[VERIFIED]`/`[PASS]`/`[FSV]`) | ~1,500 |
| Source files with inline `#[cfg(test)]` modules | 488 modules across 422 files |
| Dedicated test files in `crates/*/tests/` directories | 192 |
| Total lines in `tests/` directory files | ~36,000 |
| Criterion benchmark files | 7 |
| Total lines in `benches/` files | 3,469 |
| Workspace crates | 10 members (9 in `default-members`; `context-graph-benchmark` excluded per `Cargo.toml`) |

The 27 `#[ignore]` markers cover: 10 Qodo-Embed 1.5B integration tests (`embeddings/tests/qodo_embed_integration.rs`), 1 causal-agent LLM load test (`causal-agent/src/service/mod.rs::test_discovery_cycle`), 1 HNSW 10K-vector perf test (`storage/src/teleological/indexes/hnsw_impl/tests.rs::test_performance_scaling`), 8 GPU-dependent CUDA tests, and others. Run with `cargo test -- --ignored` for the full set.

---

## 2. Test Count per Crate

Scanned 2026-04-14 via `grep -rP "^\s*#\[(?:tokio::)?test(?:\(|\])"` across each crate, capturing both `#[test]` and flavored `#[tokio::test(flavor = "multi_thread")]` forms.

| Crate | Combined test attrs (src + tests) | Category |
|-------|----------------------------------:|----------|
| context-graph-core | 2,088 | Core domain logic (+ training/constellation/contrastive) |
| context-graph-embeddings | 1,386 | Embedding models & GPU |
| context-graph-storage | 554 | RocksDB persistence (+ training/constellation/contrastive integration tests) |
| context-graph-benchmark | 374 | Benchmark metrics |
| context-graph-mcp | 342 | MCP protocol server |
| context-graph-cli | 262 | CLI & hooks |
| context-graph-graph-agent | 64 | Graph discovery agent |
| context-graph-cuda | 62 | CUDA/GPU bindings |
| context-graph-causal-agent | 52 | Causal discovery agent |
| context-graph-test-utils | 0 | Shared test utilities (no tests of its own) |
| **Total (`#[test]` + `#[tokio::test]`)** | **5,184** | |

The 118-test increase since 2026-04-13 breaks down as: 18 training-module tests (`core/src/training/tests.rs`), 15 constellation tests (`core/src/constellation/{tests,reservoir,welford}.rs`), 15 async integration tests in `storage/tests/training_records_integration.rs`, plus contrastive-pairs integration tests added in commit `48f0420d`.

---

## 3. Test Categories

### 3.1 Unit Tests (`#[cfg(test)]` inline modules)

Inline test modules co-located with source code. 488 `#[cfg(test)]` blocks live across 422 source files (re-measured 2026-04-14). These test individual functions, types, and algorithms in isolation.

New module test wiring since 2026-04-13:
- `core/src/training/tests.rs` (18 tests) — cross-correlation ordering, group-alignment identities, topic-profile fallback, temporal-labels boundaries, bincode roundtrip
- `core/src/constellation/tests.rs` + `reservoir.rs` + `welford.rs` (15 tests total) — reservoir uniform-sample invariants, Welford online mean/variance correctness, constellation compiler
- `core/src/contrastive/` (tests inline) — pair mining, anchor selection, anomaly-kind routing

Key inline test locations:

| Source File | Tests | Lines | Subject |
|-------------|------:|------:|---------|
| `core/src/causal/asymmetric.rs` | 56 | 2,077 | Causal asymmetric scoring |
| `core/src/clustering/manager.rs` | 53 | 3,403 | Cluster lifecycle |
| `core/src/memory/chunker.rs` | 42 | 1,550 | Text chunking |
| `embeddings/src/types/embedding/tests.rs` | 39 | 465 | Embedding type operations |
| `core/src/config/tests/mcp_config_tests.rs` | 38 | 655 | MCP configuration |
| `cli/src/commands/hooks/types.rs` | 35 | 2,073 | Hook type definitions |
| `core/src/injection/formatter.rs` | 32 | 825 | Injection formatting |
| `core/src/injection/priority.rs` | 31 | 822 | Priority scoring |
| `mcp/src/handlers/tools/topic_dtos.rs` | 32 | 1,028 | Topic DTOs |
| `mcp/src/handlers/tools/curation_dtos.rs` | 30 | 725 | Curation DTOs |
| `mcp/src/handlers/tools/causal_dtos.rs` | 29 | 1,220 | Causal DTOs |
| `core/src/graph/asymmetric.rs` | 29 | 1,205 | Graph asymmetric scoring |
| `core/src/injection/budget.rs` | 27 | 831 | Token budget management |
| `core/src/types/fingerprint/sparse.rs` | 28 | 713 | Sparse vector types |
| `storage/src/teleological/search/pipeline/tests.rs` | 27 | 666 | Search pipeline |
| `storage/src/column_families.rs` | 26 | 751 | Column family definitions |
| `core/src/quantization/fingerprint.rs` | 25 | 1,422 | Quantization |
| `mcp/src/handlers/tools/graph_link_dtos.rs` | 25 | 1,387 | Graph link DTOs |
| `core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` | 24 | 551 | Semantic fingerprint |
| `storage/src/teleological/indexes/metrics.rs` | 24 | 591 | Index metrics |

### 3.2 Integration Tests (`crates/*/tests/`)

Dedicated integration test files that exercise cross-module functionality with real RocksDB storage.

**context-graph-core** (5 files):

| File | Tests | Lines | Description |
|------|------:|------:|-------------|
| `tests/file_watcher_integration.rs` | 0 (async) | 458 | File watcher + memory capture pipeline |
| `tests/injection_manual_test.rs` | 9 | 515 | Context injection with real fingerprints |
| `tests/rrf_edge_cases.rs` | 5 | 182 | RRF fusion edge cases |
| `tests/source_metadata_integration.rs` | 0 (async) | 372 | Source metadata roundtrip |
| `tests/synthesizer_edge_case_test.rs` | 13 | 435 | Cluster synthesizer edge cases |

**context-graph-embeddings** (14 test binaries):

| File / Test Binary | Tests | Lines | Description |
|--------------------|------:|------:|-------------|
| `tests/attention_strategy_tests.rs` | 15 | 326 | GPU attention strategies |
| `tests/benchmark_test.rs` | 23 | 682 | Embedding benchmark validation |
| `tests/dimension_test/` (8 files) | 62 | 1,023 | Dimension consistency across all 13 embedders |
| `tests/e9_typo_tolerance_test.rs` | 3 | 96 | HDC typo tolerance |
| `tests/e9_vector_differentiation_test.rs` | 0 (async) | 86 | E9 vector uniqueness |
| `tests/embed_dual_with_hint_integration.rs` | 8 | 391 | Dual-vector causal hint |
| `tests/global_provider_fsv.rs` | 10 | 335 | Global warm provider FSV |
| `tests/gpu_memory_slots_test.rs` | 13 | 522 | GPU VRAM slot allocation |
| `tests/no_fake_data_test.rs` | 25 | 643 | AP-007: No fake data compliance |
| `tests/qodo_embed_integration.rs` | 0 (async) | 669 | E7 Qodo-Embed code model |
| `tests/search_test.rs` | 26 | 782 | Multi-space search integration |
| `tests/storage_roundtrip_test/` (8 files) | 36 | 976 | Quantized storage roundtrip |
| `tests/fixtures/` (2 files) | 0 | 153 | Physical verification fixtures |

**context-graph-storage** (10 test binaries as of 2026-04-14):

| File / Test Binary | Tests | Lines | Description |
|--------------------|------:|------:|-------------|
| `tests/crash_recovery_tests.rs` | 0 (async) | 708 | RocksDB crash recovery |
| `tests/e12_e13_source_of_truth_verification.rs` | 0 (async) | 495 | ColBERT/SPLADE source-of-truth |
| `tests/e9_storage_roundtrip_test.rs` | 1 | 160 | E9 HDC storage roundtrip |
| `tests/full_integration_real_data/` (6 files) | 2 (async) | 951 | Full pipeline with real data |
| `tests/teleological_integration.rs` | 8 | 505 | Teleological store integration |
| `tests/training_records_integration.rs` | 15 (async) | ~560 | Training-corpus export + roundtrip (added `aec85437`/`7179bd7c`) |
| `tests/constellation_integration.rs` | (new) | (new) | Constellation compile/score/persistence (added `7179bd7c`) |
| `tests/contrastive_integration.rs` | (new) | (new) | Contrastive-pairs mining + persistence (added `48f0420d`) |
| `tests/typed_edge_export_integration.rs` | 7 (async) | 219 | F1 typed-edge export: store/get/list/count/clear, version mismatch, decode rejection (added `61934385`) |
| `tests/llm_validation_integration.rs` | 7 (async) | 231 | F4 LLM validation: Valid/Invalid/Reclassify round-trip, version mismatch, overwrite (added `61934385`) |
| `tests/anomaly_derivation_integration.rs` | 1 (async) | 251 | F2 anomaly derivation end-to-end: 6 synthetic edges, 5 expected anomaly pairs, per_kind_counts (added `61934385`) |

**context-graph-mcp** (1 integration file + 8 handler test files):

| File | Tests | Lines | Description |
|------|------:|------:|-------------|
| `tests/graph_linking_integration.rs` | 7 | 402 | Graph linking integration |
| `src/handlers/tests/mcp_protocol_e2e_test.rs` | 1 (async) | 984 | Full MCP protocol E2E |
| `src/handlers/tests/tcp_transport_integration.rs` | 10 | 211 | TCP transport |
| `src/handlers/tests/tools_call.rs` | 0 (async) | 187 | Tool call handling |
| `src/handlers/tests/tools_list.rs` | 0 (async) | 53 | Tool listing |
| `src/handlers/tests/initialize.rs` | 0 (async) | 108 | MCP initialization |
| `src/handlers/tests/error_codes.rs` | 0 (async) | 94 | Error code compliance |
| `src/handlers/tests/topic_tools.rs` | 0 (async) | 558 | Topic tool handling |
| `src/handlers/tests/search_periodic_test.rs` | 0 (async) | 479 | Periodic search |

**context-graph-causal-agent** (2 files):

| File | Tests | Lines | Description |
|------|------:|------:|-------------|
| `tests/analyze_single_text_integration.rs` | 1 | 224 | Single text causal analysis |
| `tests/background_loop_integration.rs` | 1 | 746 | Background loop lifecycle |

**context-graph-graph-agent** (1 file):

| File | Tests | Lines | Description |
|------|------:|------:|-------------|
| `tests/scanner_integration.rs` | 10 | 478 | Graph scanner integration |

**context-graph-cli** (6 files):

| File | Tests | Lines | Description |
|------|------:|------:|-------------|
| `tests/e2e/full_session_test.rs` | 0 (async) | 826 | Full session E2E |
| `tests/e2e/error_recovery_test.rs` | 0 (async) | 634 | Error recovery E2E |
| `tests/e2e/helpers.rs` | 5 | 549 | E2E test helpers |
| `tests/integration/exit_code_test.rs` | 7 | 496 | Exit code compliance (AP-26) |
| `tests/integration/hook_lifecycle_test.rs` | 0 (async) | 505 | Hook lifecycle |
| `tests/integration/timeout_test.rs` | 0 (async) | 405 | Timeout enforcement |

### 3.3 E2E Tests

E2E tests exercise the full system including CLI binary execution, shell scripts, and real database state.

Location: `crates/context-graph-cli/tests/e2e/`

- `full_session_test.rs` -- Complete session lifecycle: start, tool use, memory capture, persist, restore, end
- `error_recovery_test.rs` -- Error recovery across hook failures, database errors, invalid input
- `helpers.rs` -- E2E utilities: `execute_hook_script()`, `verify_*` helpers, exit code constants from AP-26

Run command: `make test-e2e` or `cargo test -p context-graph-cli --test e2e`

Key pattern: Tests execute real shell scripts via `Command::new("bash").arg(script_path)` with JSON piped to stdin, as opposed to integration tests which invoke the CLI binary directly.

### 3.4 Benchmark Tests

**Criterion Benchmarks** (7 files, 3,469 lines):

| File | Lines | Description |
|------|------:|-------------|
| `benchmark/benches/comparative_suite.rs` | - | Multi-space vs single-embedder comparison |
| `benchmark/benches/scaling_analysis.rs` | - | Scaling behavior analysis |
| `cli/benches/cli_benchmarks.rs` | - | CLI hook performance |
| `storage/benches/causal_relationships_bench.rs` | - | Causal relationship CRUD performance |
| `storage/benches/e11_quality_bench.rs` | - | E11 entity quality metrics |
| `storage/benches/hybrid_effectiveness_bench.rs` | - | Hybrid search effectiveness |
| `storage/benches/maxsim_bench.rs` | - | MaxSim late interaction performance |

Run command: `cargo bench -p context-graph-storage --bench causal_relationships_bench` (example for one bench).

**Benchmark Metric Tests** (19 files, 374 inline `#[test]` functions):

Located in `crates/context-graph-benchmark/src/metrics/`:

| File | Tests | Lines | Metric Domain |
|------|------:|------:|---------------|
| `causal.rs` | 10 | 1,331 | Causal retrieval accuracy |
| `e4_hybrid_session.rs` | 6 | 1,020 | Temporal session benchmarks |
| `temporal_realdata.rs` | 5 | 983 | Real temporal data |
| `temporal.rs` | 6 | 918 | Temporal embedder metrics |
| `e11_entity.rs` | 8 | 859 | Entity embedding quality |
| `e1_semantic.rs` | 5 | 734 | E1 semantic accuracy |
| `sparse.rs` | 4 | 721 | Sparse retrieval (E6/E13) |
| `multimodal.rs` | 4 | 630 | Multimodal (E10) metrics |
| `graph_structure.rs` | 5 | 617 | Graph linking metrics |
| `embedder_contribution.rs` | 4 | 595 | Per-embedder ablation |
| `resource_usage.rs` | 5 | 542 | Memory/VRAM usage |
| `e7_code.rs` | 7 | 521 | Code embedding quality |
| `e7_iou.rs` | 9 | 489 | Code IoU metrics |
| `graph_linking.rs` | 4 | 457 | Graph link accuracy |
| `validation.rs` | 6 | 439 | Result validation |
| `clustering.rs` | 5 | 418 | Cluster quality |
| `retrieval.rs` | 4 | 341 | Retrieval precision/recall |
| `performance.rs` | 4 | 309 | Latency/throughput |
| `divergence.rs` | 4 | 243 | Distribution divergence |

### 3.5 FSV Tests (Full State Verification)

FSV tests follow a specific methodology: define source of truth, execute operations, verify state transitions, log evidence with `[VERIFIED]`/`[PASS]`/`[FSV]` markers.

Key FSV test files:

| File | Tests | Description |
|------|------:|-------------|
| `embeddings/tests/global_provider_fsv.rs` | 10 | Global warm provider singleton behavior |
| `embeddings/tests/no_fake_data_test.rs` | 25 | AP-007 no-stub-data compliance |
| `storage/tests/e12_e13_source_of_truth_verification.rs` | 0 (async) | ColBERT/SPLADE source-of-truth |
| `core/examples/fsv_line_numbers.rs` | - | Line number FSV (run as example) |
| `core/examples/fsv_source_metadata.rs` | - | Source metadata FSV (run as example) |
| `core/examples/divergence_fsv_test.rs` | - | Divergence detection FSV |

FSV examples are run via `cargo run --example fsv_line_numbers -p context-graph-core`.

---

## 4. How to Run Tests

### 4.1 Primary Commands

```bash
# Run all workspace tests (default: 4 threads)
make test
# Equivalent to:
cargo test --workspace

# Run all tests with cargo alias (explicit 4 threads)
cargo test-all

# Run tests for a specific crate
cargo test -p context-graph-core
cargo test -p context-graph-mcp
cargo test -p context-graph-storage

# Run MCP tests only
make test-mcp

# Run E2E hook tests only
make test-e2e
# Equivalent to:
cargo test -p context-graph-cli --test e2e
```

### 4.2 Feature-Gated Tests

```bash
# Tests requiring test-utils feature
cargo test -p context-graph-core --features test-utils

# Tests requiring CUDA (default feature)
cargo test -p context-graph-mcp            # cuda + llm features enabled by default
cargo test -p context-graph-storage        # cuda feature enabled by default

# LLM agent tests
cargo test-llm    # Runs causal agent example with cuda + release
```

### 4.3 Benchmark Execution

```bash
# Criterion benchmarks
cargo bench -p context-graph-storage --bench causal_relationships_bench
cargo bench -p context-graph-storage --bench maxsim_bench
cargo bench -p context-graph-benchmark --bench comparative_suite
cargo bench -p context-graph-benchmark --bench scaling_analysis

# CLI performance benchmarks
cargo bench -p context-graph-cli --bench cli_benchmarks
```

### 4.4 Configuration

From `.cargo/config.toml`:
- `RUST_TEST_THREADS = "4"` -- limits parallel test execution to 4 threads
- `rustflags = ["-C", "target-cpu=native"]` -- native CPU optimizations
- Linker: `clang` with `lld` on x86_64-linux-gnu

### 4.5 Quick Checks

```bash
# Type check without running tests
make check
# Equivalent to:
cargo check --workspace --all-targets

# Lint
make clippy
# Equivalent to:
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 5. Test Utilities (`context-graph-test-utils` crate)

**Location:** `crates/context-graph-test-utils/` (159 lines across 3 files)

**Purpose:** Shared test helpers for the workspace. ALL generated data is REAL -- no mocks.

### 5.1 Fingerprint Generators (`src/fingerprints.rs`)

| Function | Description |
|----------|-------------|
| `generate_real_unit_vector(dim)` | Random unit vector (L2 norm = 1.0) of specified dimension |
| `generate_real_sparse_vector(nnz)` | Sparse vector with `nnz` non-zero entries, indices from 0..30522 (BERT vocab) |
| `generate_real_semantic_fingerprint()` | Complete `SemanticFingerprint` with correct dimensions for all 13 embedders |
| `generate_real_content_hash()` | Random 32-byte SHA-256 content hash |
| `create_real_fingerprint()` | `TeleologicalFingerprint` with new random UUID |
| `create_real_fingerprint_with_id(id)` | `TeleologicalFingerprint` with specific UUID |
| `generate_real_teleological_fingerprint(id)` | Alias for `create_real_fingerprint_with_id()` |
| `hex_string(bytes)` | Format bytes as hex (first 64 bytes) |

The semantic fingerprint generator produces vectors with correct dimensions for each embedder:
- E1: 1024D, E2-E4: 512D, E5: 768D (dual cause+effect), E6: sparse 30K
- E7: 1536D, E8: 1024D (dual source+target), E9: 1024D (HDC)
- E10: 768D (dual paraphrase+context), E11: 768D, E12: 128D/token (16 tokens), E13: sparse 30K

Legacy unified fields (`e5_causal`, `e8_graph`) are intentionally left empty to force tests through the correct dual-vector code paths.

### 5.2 Store Factories (`src/stores.rs`)

| Function | Description |
|----------|-------------|
| `create_test_store(temp_dir)` | Create `RocksDbTeleologicalStore` from a `TempDir` reference |
| `create_initialized_store(path)` | Create store from arbitrary path |

Both open a real RocksDB instance with all column families initialized.

### 5.3 Dependencies

```toml
[dependencies]
context-graph-core = { path = "../context-graph-core" }
context-graph-storage = { path = "../context-graph-storage" }
rand = "0.8"
tempfile = "3"
uuid = "1.6"
```

### 5.4 Usage

Add to crate's `[dev-dependencies]`:
```toml
context-graph-test-utils = { path = "../context-graph-test-utils" }
```

Currently used by: `context-graph-graph-agent`.

---

## 6. Test Naming Conventions

The overwhelming majority of test functions use the `test_` prefix (97% of sampled tests). The naming pattern is:

```
test_<subject>_<behavior>
```

Examples from the codebase:
- `test_cosine_identical_vectors`
- `test_delete_typed_edge`
- `test_cf_names_are_snake_case`
- `test_edge_case_empty_database`
- `test_fake_pointer_constant_value`

FSV tests use the `fsv_` prefix:
- `fsv_status_message_before_init`
- `fsv_is_warm_initialized_safe`

Edge case tests often include `edge_case` in the name:
- `edge_case_minimum_cache_size`
- `edge_case_empty_memory_id_list`
- `test_edge_cases_with_synthetic_data`

### Async Test Annotation

- Synchronous: `#[test]`
- Async: `#[tokio::test]` (508 instances across 89 files)
- Serial execution: `#[serial]` from the `serial_test` crate (used in 2 files for tests requiring exclusive resource access)

---

## 7. Mock/Stub Patterns

The project follows a strong **"no fake data"** philosophy (Constitution AP-007). The testing strategy avoids traditional mocking in favor of real implementations.

### 7.1 InMemoryTeleologicalStore (Primary Test Stub)

**Location:** `crates/context-graph-core/src/stubs/teleological_store_stub/`

- Full implementation of the `TeleologicalMemoryStore` trait
- Uses `DashMap` for concurrent access
- O(n) search (full table scan, no HNSW indexing)
- Real algorithms, not mock return values
- Used for unit tests where RocksDB is unnecessary
- Approximately 46KB per fingerprint in memory
- Not suitable for datasets larger than ~1000 fingerprints

### 7.2 Other Stubs

| Stub | Location | Purpose |
|------|----------|---------|
| `InMemoryTeleologicalStore` | `core/src/stubs/teleological_store_stub/` | TeleologicalMemoryStore trait impl for unit tests |
| `StubMultiArrayProvider` | `core/src/stubs/multi_array_stub.rs` | MultiArrayEmbeddingProvider returning errors (not zeros) |
| `StubEmbeddingProvider` | `core/src/embeddings/stubs.rs` | Embedding provider stub |
| `StubLayerStatusProvider` | `core/src/monitoring.rs` | Layer status for MCP test setup |
| `stub.rs` | `cuda/src/stub.rs` | CUDA stub when GPU unavailable |
| `stubs.rs` | `graph-agent/src/stubs.rs` | Graph discovery service stub |
| `MockProvider` | `core/src/traits/multi_array_embedding.rs` (inline test) | Minimal mock for testing trait dimensions |

### 7.3 Anti-Pattern: No Silent Fallbacks

Per AP-007, stubs that return data must return **errors**, not zeros or fake values:
- `StubMultiArrayProvider` returns `Err("not implemented")`, not zero vectors
- MCP handler tests use `ProductionMultiArrayProvider` with real GPU models
- The `no_fake_data_test.rs` file (25 tests) specifically validates that fake pointer patterns, sin wave patterns, all-zeros output, and VRAM mismatches are all detected and rejected

### 7.4 Real Data Testing Pattern

MCP handler tests (`crates/context-graph-mcp/src/handlers/tests/mod.rs`) use:
1. Real RocksDB (via `TempDir`)
2. Real GPU embeddings (via `ProductionMultiArrayProvider`)
3. Warm-loaded model cache (models loaded once into GPU VRAM, shared across all tests via `OnceCell`)
4. `_tempdir` binding pattern to keep database alive for test duration

```rust
async fn create_test_handlers() -> (Handlers, TempDir) {
    // Real RocksDB + Real GPU embeddings
}

#[tokio::test]
async fn test_example() {
    let (handlers, _tempdir) = create_test_handlers().await;
    // _tempdir keeps the database alive until end of test
}
```

---

## 8. Coverage Metrics

No dedicated coverage tool configuration (tarpaulin, llvm-cov, codecov) was found in the repository. Coverage is tracked indirectly through:

1. **FSV markers** -- 1,479 `[VERIFIED]`/`[PASS]`/`[FSV]` markers in test source provide per-assertion evidence
2. **Assertion density** -- 13,480 assertions across 5,066 combined test attributes (average ~2.7 assertions per test)
3. **No-fake-data enforcement** -- AP-007 compliance tests verify that no stub data leaks into production paths
4. **Benchmark metrics** -- 374 inline tests in the benchmark crate validate correctness of each metric calculation

The project's MEMORY.md tracks test counts per session:
- 2026-02-26 session: 1,179 tests passed (MCP + Storage focused run), 0 failed
- 2026-02-24 session: 6,527 tests passed (full workspace), 0 failed

No line or branch coverage percentages are reported.

---

## 9. Notable Test Patterns

### 9.1 Full State Verification (FSV)

FSV is the project's primary verification methodology, used for critical correctness validation:

1. **Define Source of Truth** -- identify the canonical state (e.g., RocksDB column families, HNSW index)
2. **Execute & Inspect** -- perform operations and capture state transitions
3. **Edge Cases** -- test boundary conditions explicitly
4. **Evidence of Success** -- log detailed verification with `[VERIFIED]`, `[PASS]`, `[FSV]` markers

Example from `global_provider_fsv.rs`:
```rust
#[test]
fn fsv_status_message_before_init() {
    let status = warm_status_message();
    assert!(valid_states.iter().any(|s| status.starts_with(s)));
    println!("\n[FSV-1 VERIFIED] Status message returns expected values");
}
```

### 9.2 Edge Case Testing

Dedicated edge case test files and functions target boundary conditions:

- **Empty inputs**: zero-length vectors, empty strings, empty databases
- **NaN/Infinity**: NaN-safe sorting, infinity handling in cosine similarity
- **Overflow**: large batch sizes (e.g., `edge_case_large_batch`), max token counts
- **Unicode**: multi-byte UTF-8 in code embeddings (e.g., `test_edge_case_unicode_code`)
- **Concurrency**: parallel access patterns, warm provider thread safety
- **Zero vectors**: zero-norm vector rejection, zero cache size behavior

Files with dedicated edge case coverage:
- `crates/context-graph-embeddings/tests/dimension_test/edge_cases.rs` (8 tests)
- `crates/context-graph-embeddings/src/quantization/router/tests/edge_case_tests.rs` (5 tests)
- `crates/context-graph-core/tests/rrf_edge_cases.rs` (5 tests)
- `crates/context-graph-core/tests/synthesizer_edge_case_test.rs` (13 tests)
- `crates/context-graph-storage/src/teleological/search/matrix/tests_boundary.rs` (8 tests)
- `crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` (207 lines)

### 9.3 Warm Model Cache Pattern

MCP and embedding tests that require GPU inference share a single warm-loaded model cache:

```rust
static WARM_MODEL_CACHE: OnceCell<Arc<dyn MultiArrayEmbeddingProvider>> = OnceCell::const_new();
```

This prevents CUDA OOM when tests run in parallel -- all 13 embedding models (~20GB VRAM) are loaded once and shared. Tests fail fast if CUDA is unavailable (no fallback stubs).

### 9.4 TempDir Lifecycle Pattern

All tests using RocksDB follow a strict `TempDir` ownership pattern:

```rust
let temp_dir = TempDir::new().unwrap();
let store = create_test_store(&temp_dir);
// temp_dir MUST outlive store -- dropping it deletes the database
```

In async tests, the `_tempdir` binding convention keeps the directory alive:
```rust
let (handlers, _tempdir) = create_test_handlers().await;
```

### 9.5 Constitution Compliance Testing

Tests explicitly reference constitution rules:
- **AP-007**: No stub data in production (`no_fake_data_test.rs`)
- **AP-026**: Exit codes 0-6 (`exit_code_test.rs`, `helpers.rs`)
- **AP-050**: Native hooks only (E2E test helpers)
- **SRC-3**: Cosine normalization `[-1,1] -> [0,1]` via `(raw+1)/2`

### 9.6 Deterministic Data Generation

Test helpers use seeded random generation for reproducibility:
- `create_test_fingerprint_with_seed(seed: u64)` in storage tests
- `create_test_embeddings_with_deterministic_data(seed: u8)` in embedding tests
- `DatasetGenerator::with_config(GeneratorConfig { seed: 42, .. })` in benchmarks

### 9.7 Serial Test Isolation

Two test files use `#[serial]` from the `serial_test` crate to prevent resource contention:
- `crates/context-graph-causal-agent/tests/analyze_single_text_integration.rs` -- LLM model requires exclusive GPU access
- `crates/context-graph-cli/tests/e2e/error_recovery_test.rs` -- E2E tests that modify shared state

### 9.8 Test Feature Gates

Several integration test binaries require feature flags:
```toml
[[test]]
name = "file_watcher_integration"
required-features = ["test-utils"]
```

The `test-utils` feature on `context-graph-core` exposes stubs for use in downstream crate tests.

---

## 10. Test File Inventory

### 10.1 External Test Files (`crates/*/tests/`)

| # | File | Lines | `#[test]` | `#[tokio::test]` |
|---|------|------:|----------:|------------------:|
| 1 | `causal-agent/tests/analyze_single_text_integration.rs` | 224 | 1 | 0 |
| 2 | `causal-agent/tests/background_loop_integration.rs` | 746 | 1 | 0 |
| 3 | `cli/tests/e2e/error_recovery_test.rs` | 634 | 0 | async |
| 4 | `cli/tests/e2e/full_session_test.rs` | 826 | 0 | async |
| 5 | `cli/tests/e2e/helpers.rs` | 549 | 5 | 0 |
| 6 | `cli/tests/e2e/mod.rs` | 39 | 0 | 0 |
| 7 | `cli/tests/integration/exit_code_test.rs` | 496 | 7 | 0 |
| 8 | `cli/tests/integration/helpers.rs` | 534 | 0 | 0 |
| 9 | `cli/tests/integration/hook_lifecycle_test.rs` | 505 | 0 | async |
| 10 | `cli/tests/integration/mod.rs` | 31 | 0 | 0 |
| 11 | `cli/tests/integration/timeout_test.rs` | 405 | 0 | async |
| 12 | `core/tests/file_watcher_integration.rs` | 458 | 0 | async |
| 13 | `core/tests/injection_manual_test.rs` | 515 | 9 | 0 |
| 14 | `core/tests/rrf_edge_cases.rs` | 182 | 5 | 0 |
| 15 | `core/tests/source_metadata_integration.rs` | 372 | 0 | async |
| 16 | `core/tests/synthesizer_edge_case_test.rs` | 435 | 13 | 0 |
| 17 | `embeddings/tests/attention_strategy_tests.rs` | 326 | 15 | 0 |
| 18 | `embeddings/tests/benchmark_test.rs` | 682 | 23 | 0 |
| 19 | `embeddings/tests/dimension_test/aggregate_dimensions.rs` | 107 | 6 | 0 |
| 20 | `embeddings/tests/dimension_test/comprehensive.rs` | 68 | 1 | 0 |
| 21 | `embeddings/tests/dimension_test/constants.rs` | 73 | 0 | 0 |
| 22 | `embeddings/tests/dimension_test/edge_cases.rs` | 148 | 8 | 0 |
| 23 | `embeddings/tests/dimension_test/main.rs` | 37 | 0 | 0 |
| 24 | `embeddings/tests/dimension_test/model_metadata.rs` | 96 | 3 | 0 |
| 25 | `embeddings/tests/dimension_test/native_dimensions.rs` | 202 | 15 | 0 |
| 26 | `embeddings/tests/dimension_test/offsets.rs` | 93 | 5 | 0 |
| 27 | `embeddings/tests/dimension_test/projected_dimensions.rs` | 232 | 16 | 0 |
| 28 | `embeddings/tests/dimension_test/quantization_methods.rs` | 169 | 8 | 0 |
| 29 | `embeddings/tests/e9_typo_tolerance_test.rs` | 96 | 3 | 0 |
| 30 | `embeddings/tests/e9_vector_differentiation_test.rs` | 86 | 0 | async |
| 31 | `embeddings/tests/embed_dual_with_hint_integration.rs` | 391 | 8 | 0 |
| 32 | `embeddings/tests/fixtures/create_fixture.rs` | 52 | 0 | 0 |
| 33 | `embeddings/tests/fixtures/physical_verification_test.rs` | 101 | 0 | 0 |
| 34 | `embeddings/tests/global_provider_fsv.rs` | 335 | 10 | 0 |
| 35 | `embeddings/tests/gpu_memory_slots_test.rs` | 522 | 13 | 0 |
| 36 | `embeddings/tests/no_fake_data_test.rs` | 643 | 25 | 0 |
| 37 | `embeddings/tests/qodo_embed_integration.rs` | 669 | 0 | async |
| 38 | `embeddings/tests/search_test.rs` | 782 | 26 | 0 |
| 39 | `embeddings/tests/storage_roundtrip_test/comprehensive_validation.rs` | 77 | 1 | 0 |
| 40 | `embeddings/tests/storage_roundtrip_test/edge_cases.rs` | 107 | 5 | 0 |
| 41 | `embeddings/tests/storage_roundtrip_test/fingerprint_creation.rs` | 161 | 7 | 0 |
| 42 | `embeddings/tests/storage_roundtrip_test/helpers.rs` | 121 | 0 | 0 |
| 43 | `embeddings/tests/storage_roundtrip_test/index_entry.rs` | 164 | 9 | 0 |
| 44 | `embeddings/tests/storage_roundtrip_test/main.rs` | 30 | 0 | 0 |
| 45 | `embeddings/tests/storage_roundtrip_test/query_result.rs` | 104 | 4 | 0 |
| 46 | `embeddings/tests/storage_roundtrip_test/rrf_formula.rs` | 143 | 6 | 0 |
| 47 | `embeddings/tests/storage_roundtrip_test/serialization_roundtrip.rs` | 170 | 4 | 0 |
| 48 | `graph-agent/tests/scanner_integration.rs` | 478 | 10 | 0 |
| 49 | `mcp/tests/graph_linking_integration.rs` | 402 | 7 | 0 |
| 50 | `mcp/src/handlers/tests/error_codes.rs` | 94 | 0 | async |
| 51 | `mcp/src/handlers/tests/initialize.rs` | 108 | 0 | async |
| 52 | `mcp/src/handlers/tests/mcp_protocol_e2e_test.rs` | 984 | 1 | async |
| 53 | `mcp/src/handlers/tests/mod.rs` | 391 | 0 | 0 |
| 54 | `mcp/src/handlers/tests/search_periodic_test.rs` | 479 | 0 | async |
| 55 | `mcp/src/handlers/tests/tcp_transport_integration.rs` | 211 | 10 | 0 |
| 56 | `mcp/src/handlers/tests/tools_call.rs` | 187 | 0 | async |
| 57 | `mcp/src/handlers/tests/tools_list.rs` | 53 | 0 | async |
| 58 | `mcp/src/handlers/tests/topic_tools.rs` | 558 | 0 | async |
| 59 | `storage/tests/crash_recovery_tests.rs` | 708 | 0 | async |
| 60 | `storage/tests/e12_e13_source_of_truth_verification.rs` | 495 | 0 | async |
| 61 | `storage/tests/e9_storage_roundtrip_test.rs` | 160 | 1 | 0 |
| 62 | `storage/tests/full_integration_real_data/helpers.rs` | 7 | 0 | 0 |
| 63 | `storage/tests/full_integration_real_data/main.rs` | 66 | 1 | 0 |
| 64 | `storage/tests/full_integration_real_data/operations_tests.rs` | 178 | 0 | async |
| 65 | `storage/tests/full_integration_real_data/performance_tests.rs` | 213 | 0 | async |
| 66 | `storage/tests/full_integration_real_data/pipeline_tests.rs` | 389 | 0 | async |
| 67 | `storage/tests/full_integration_real_data/roundtrip_tests.rs` | 213 | 1 | async |
| 68 | `storage/tests/full_integration_real_data/search_tests.rs` | 98 | 0 | async |
| 69 | `storage/tests/teleological_integration.rs` | 505 | 8 | 0 |

### 10.2 Inline Test Module Files (src-level `tests/` subdirectories)

These are test files organized as `tests/` subdirectories within `src/` modules, compiled as part of the crate's `#[cfg(test)]` code.

| # | File | Lines | Tests |
|---|------|------:|------:|
| 1 | `core/src/config/tests/default_tests.rs` | 109 | 8 |
| 2 | `core/src/config/tests/mcp_config_tests.rs` | 655 | 38 |
| 3 | `core/src/config/tests/serialization_tests.rs` | 164 | 5 |
| 4 | `core/src/config/tests/validation_tests.rs` | 465 | 13 |
| 5 | `core/src/index/hnsw_impl/tests/multi_space_tests.rs` | 252 | 0 (async) |
| 6 | `core/src/index/hnsw_impl/tests/persistence_tests.rs` | 215 | 3 |
| 7 | `core/src/index/hnsw_impl/tests/real_hnsw_tests.rs` | 153 | 6 |
| 8 | `core/src/similarity/dense/tests/integration_tests.rs` | 137 | 9 |
| 9 | `core/src/similarity/dense/tests/primitives_tests.rs` | 256 | 19 |
| 10 | `core/src/similarity/dense/tests/simd_tests.rs` | 177 | 10 |
| 11 | `core/src/teleological/matrix_search/tests/advanced.rs` | 110 | 4 |
| 12 | `core/src/teleological/matrix_search/tests/basic.rs` | 126 | 5 |
| 13 | `core/src/teleological/synergy_matrix/tests/analysis.rs` | 45 | 3 |
| 14 | `core/src/teleological/synergy_matrix/tests/basic.rs` | 62 | 3 |
| 15 | `core/src/teleological/synergy_matrix/tests/presets.rs` | 94 | 2 |
| 16 | `core/src/teleological/synergy_matrix/tests/validation.rs` | 68 | 2 |
| 17 | `core/src/types/fingerprint/semantic/tests/core_tests.rs` | 200 | 10 |
| 18 | `core/src/types/fingerprint/semantic/tests/storage_tests.rs` | 110 | 5 |
| 19 | `core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` | 551 | 24 |
| 20 | `core/src/types/fingerprint/semantic/tests/validation_tests.rs` | 470 | 22 |
| 21 | `embeddings/src/models/custom/hdc/tests/construction.rs` | 11 | 1 |
| 22 | `embeddings/src/models/custom/hdc/tests/encoding.rs` | 10 | 1 |
| 23 | `embeddings/src/models/custom/hdc/tests/hypervector.rs` | 10 | 1 |
| 24 | `embeddings/src/models/custom/hdc/tests/operations.rs` | 13 | 1 |
| 25 | `embeddings/src/models/custom/hdc/tests/embedding.rs` | 14 | 0 |
| 26 | `embeddings/src/models/custom/hdc/tests/edge_cases.rs` | 2 | 0 |
| 27 | `embeddings/src/models/custom/temporal_periodic/tests/construction.rs` | 15 | 1 |
| 28 | `embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` | 207 | 0 |
| 29 | `embeddings/src/models/custom/temporal_periodic/tests/embedding.rs` | 15 | 0 |
| 30 | `embeddings/src/models/custom/temporal_periodic/tests/periodicity.rs` | 22 | 0 |
| 31 | `embeddings/src/models/custom/temporal_periodic/tests/thread_safety.rs` | 9 | 1 |
| 32 | `embeddings/src/models/custom/temporal_periodic/tests/timestamp.rs` | 68 | 3 |
| 33 | `embeddings/src/models/custom/temporal_positional/tests/construction.rs` | 19 | 1 |
| 34 | `embeddings/src/models/custom/temporal_positional/tests/edge_cases.rs` | 2 | 0 |
| 35 | `embeddings/src/models/custom/temporal_positional/tests/embedding.rs` | 17 | 0 |
| 36 | `embeddings/src/models/custom/temporal_positional/tests/hybrid.rs` | 21 | 0 |
| 37 | `embeddings/src/models/custom/temporal_positional/tests/uniqueness.rs` | 23 | 0 |
| 38 | `embeddings/src/models/custom/temporal_positional/tests/verification.rs` | 2 | 0 |
| 39 | `embeddings/src/models/factory/tests/factory_tests.rs` | 20 | 2 |
| 40 | `embeddings/src/models/factory/tests/memory_tests.rs` | 28 | 2 |
| 41 | `embeddings/src/models/factory/tests/model_creation_tests.rs` | 59 | 2 |
| 42 | `embeddings/src/models/memory_tracker/tests/advanced.rs` | 35 | 2 |
| 43 | `embeddings/src/models/memory_tracker/tests/basic.rs` | 53 | 3 |
| 44 | `embeddings/src/models/pretrained/semantic/tests/basic.rs` | 20 | 0 |
| 45 | `embeddings/src/models/pretrained/semantic/tests/edge_cases.rs` | 35 | 0 |
| 46 | `embeddings/src/models/pretrained/semantic/tests/embedding.rs` | 28 | 0 |
| 47 | `embeddings/src/models/pretrained/semantic/tests/helpers.rs` | 30 | 0 |
| 48 | `embeddings/src/models/pretrained/semantic/tests/verification.rs` | 2 | 0 |
| 49 | `embeddings/src/quantization/pq8/tests/dimension_tests.rs` | 62 | 3 |
| 50 | `embeddings/src/quantization/pq8/tests/encoder_tests.rs` | 250 | 14 |
| 51 | `embeddings/src/quantization/pq8/tests/persistence_tests.rs` | 130 | 4 |
| 52 | `embeddings/src/quantization/pq8/tests/training_tests.rs` | 236 | 7 |
| 53 | `embeddings/src/quantization/router/tests/binary_tests.rs` | 51 | 2 |
| 54 | `embeddings/src/quantization/router/tests/edge_case_tests.rs` | 125 | 5 |
| 55 | `embeddings/src/quantization/router/tests/error_tests.rs` | 113 | 5 |
| 56 | `embeddings/src/quantization/router/tests/float8_tests.rs` | 83 | 3 |
| 57 | `embeddings/src/quantization/router/tests/pq8_tests.rs` | 90 | 3 |
| 58 | `embeddings/src/quantization/router/tests/routing_tests.rs` | 169 | 9 |
| 59 | `embeddings/src/traits/embedding_model/tests/core_tests.rs` | 288 | 19 |
| 60 | `embeddings/src/traits/embedding_model/tests/integration_tests.rs` | 183 | 0 (async) |
| 61 | `embeddings/src/traits/embedding_model/tests/test_model.rs` | 72 | 0 |
| 62 | `embeddings/src/types/input/tests/audio_tests.rs` | 90 | 6 |
| 63 | `embeddings/src/types/input/tests/code_tests.rs` | 53 | 4 |
| 64 | `embeddings/src/types/input/tests/hash_and_size_tests.rs` | 146 | 10 |
| 65 | `embeddings/src/types/input/tests/image_tests.rs` | 106 | 11 |
| 66 | `embeddings/src/types/input/tests/input_type_tests.rs` | 154 | 13 |
| 67 | `embeddings/src/types/input/tests/serde_tests.rs` | 55 | 2 |
| 68 | `embeddings/src/types/input/tests/text_tests.rs` | 64 | 5 |
| 69 | `embeddings/src/types/input/tests/type_tests.rs` | 99 | 8 |
| 70 | `embeddings/src/warm/integration/tests/advanced.rs` | 124 | 6 |
| 71 | `embeddings/src/warm/integration/tests/basic.rs` | 183 | 11 |
| 72 | `embeddings/src/warm/loader/tests/loader_tests.rs` | 434 | 15 |
| 73 | `embeddings/src/warm/loader/tests/summary_tests.rs` | 91 | 5 |
| 74 | `embeddings/src/warm/tests/config_tests.rs` | 116 | 9 |
| 75 | `embeddings/src/warm/tests/diagnostics_tests.rs` | 144 | 6 |
| 76 | `embeddings/src/warm/tests/error_tests.rs` | 281 | 6 |
| 77 | `embeddings/src/warm/tests/handle_tests.rs` | 62 | 7 |
| 78 | `embeddings/src/warm/tests/health_tests.rs` | 110 | 5 |
| 79 | `embeddings/src/warm/tests/integration_tests.rs` | 126 | 2 |
| 80 | `embeddings/src/warm/tests/loader_tests.rs` | 119 | 3 |
| 81 | `embeddings/src/warm/tests/memory_pool_tests.rs` | 163 | 12 |
| 82 | `embeddings/src/warm/tests/registry_tests.rs` | 247 | 13 |
| 83 | `embeddings/src/warm/tests/state_tests.rs` | 97 | 7 |
| 84 | `embeddings/src/warm/tests/validation_tests.rs` | 289 | 21 |
| 85 | `embeddings/src/warm/validation/tests/config_tests.rs` | 60 | 5 |
| 86 | `embeddings/src/warm/validation/tests/model_tests.rs` | 119 | 6 |
| 87 | `embeddings/src/warm/validation/tests/result_tests.rs` | 34 | 2 |
| 88 | `embeddings/src/warm/validation/tests/validator_tests.rs` | 263 | 20 |
| 89 | `storage/src/teleological/tests/column_family.rs` | 174 | 14 |
| 90 | `storage/src/teleological/tests/content.rs` | 386 | 0 (async) |
| 91 | `storage/src/teleological/tests/helpers.rs` | 41 | 0 |
| 92 | `storage/src/teleological/tests/key_format.rs` | 128 | 9 |
| 93 | `storage/src/teleological/tests/panic.rs` | 90 | 8 |
| 94 | `storage/src/teleological/tests/serialization.rs` | 290 | 11 |
| 95 | `storage/src/teleological/tests/stale_lock.rs` | 157 | 0 (async) |
| 96 | `storage/src/teleological/search/single/tests/integration.rs` | 132 | 1 |
| 97 | `storage/src/teleological/search/single/tests/search.rs` | 360 | 12 |
| 98 | `storage/src/teleological/search/single/tests/validation.rs` | 190 | 8 |

### 10.3 Criterion Benchmark Files (`benches/`)

| # | File | Lines |
|---|------|------:|
| 1 | `benchmark/benches/comparative_suite.rs` | - |
| 2 | `benchmark/benches/scaling_analysis.rs` | - |
| 3 | `cli/benches/cli_benchmarks.rs` | - |
| 4 | `storage/benches/causal_relationships_bench.rs` | - |
| 5 | `storage/benches/e11_quality_bench.rs` | - |
| 6 | `storage/benches/hybrid_effectiveness_bench.rs` | - |
| 7 | `storage/benches/maxsim_bench.rs` | - |

Total benchmark lines: 3,469.

### 10.4 FSV Example Files

| # | File | Description |
|---|------|-------------|
| 1 | `core/examples/fsv_line_numbers.rs` | Line number verification |
| 2 | `core/examples/fsv_source_metadata.rs` | Source metadata verification |
| 3 | `core/examples/divergence_fsv_test.rs` | Divergence detection verification |

---

## 11. Dev-Dependencies by Crate

| Crate | Key Dev-Dependencies |
|-------|---------------------|
| context-graph-core | `tempfile`, `tokio[test-util]`, `rand_chacha`, `criterion` |
| context-graph-embeddings | `tokio-test`, `tempfile`, `serial_test`, `context-graph-core[test-utils]` |
| context-graph-storage | `tempfile`, `context-graph-test-utils`, `tokio-test` |
| context-graph-mcp | `tokio-test` |
| context-graph-cli | `tokio-test`, `tempfile`, `futures`, `chrono`, `serial_test`, `context-graph-core[test-utils]`, `context-graph-storage` |
| context-graph-causal-agent | `tokio-test`, `tempfile`, `tracing-subscriber`, `serial_test` |
| context-graph-graph-agent | `tokio-test`, `tempfile`, `tracing-subscriber`, `context-graph-test-utils` |
| context-graph-cuda | `tokio-test`, `context-graph-core[test-utils]` |
| context-graph-benchmark | `tokio-test` |
