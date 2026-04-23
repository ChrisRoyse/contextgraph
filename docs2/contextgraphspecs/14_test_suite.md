# Test Suite

Source files covered:
- `crates/*/src/**/*.rs` — inline `#[cfg(test)]` modules
- `crates/*/tests/**/*.rs` — integration tests
- `crates/context-graph-test-utils/src/{lib,fingerprints,stores}.rs` — shared fixtures
- `tests/chaos/mod.rs` — chaos-test redirect stub
- `tests/fixtures/golden/` — binary golden fixtures for E2/E3/E4/E5/E6/E7/E8/E9/E10/E11 embedders
- `scripts/mcp_full_verify.py` — end-to-end MCP JSON-RPC verification
- `Makefile` — build/test targets
- root `Cargo.toml` + `crates/*/Cargo.toml` — feature flags, `[dev-dependencies]`, `[[test]]` declarations

Cross-references:
[02_source_code_map.md](02_source_code_map.md),
[05_embedding_subsystem.md](05_embedding_subsystem.md),
[08_mcp_tools_reference.md](08_mcp_tools_reference.md),
[09_storage_layer.md](09_storage_layer.md),
[11_cuda_acceleration.md](11_cuda_acceleration.md),
[15_verification_report.md](15_verification_report.md).

---

## 1. Test Categories

| Category | Location | Scope | Run command |
|---|---|---|---|
| Unit (inline) | `crates/*/src/**/*.rs` under `#[cfg(test)]` | Pure-logic tests colocated with implementation | `cargo test --workspace --lib` |
| Integration | `crates/*/tests/*.rs` | Cross-module; uses public API + RocksDB `TempDir` + real fingerprints | `cargo test --workspace --tests` |
| E2E (CLI) | `crates/context-graph-cli/tests/e2e/` | Full CLI session, hooks, error recovery | `make test-e2e` / `cargo test -p context-graph-cli --test e2e` |
| Integration (CLI) | `crates/context-graph-cli/tests/integration/` | Timeout, exit codes, hook lifecycle | `cargo test -p context-graph-cli --test integration` |
| MCP integration | `crates/context-graph-mcp/tests/` | Graph-linking MCP handler | `make test-mcp` |
| Benches | `crates/context-graph-storage/benches/`, `crates/context-graph-benchmark/` | Criterion perf benches (MaxSim, causal, hybrid, E11) | `cargo bench -p context-graph-storage` |
| FSV (end-to-end) | `scripts/mcp_full_verify.py` | 13-phase live MCP server verification | `python3 scripts/mcp_full_verify.py` |

---

## 2. Test Attribute Inventory (Per Crate)

Method: `grep -rhE "^\s*#\[(tokio::)?test\]" <crate>` counts every `#[test]` and `#[tokio::test]` attribute across src + tests.

| Crate | Files with tests | Test attrs | Integration test files |
|---|---:|---:|---:|
| context-graph-benchmark | 85 | 374 | 0 |
| context-graph-causal-agent | 8 | 52 | 2 |
| context-graph-cli | 26 | 265 | 10 |
| context-graph-core | 171 | 2,101 | 5 |
| context-graph-cuda | 9 | 62 | 0 |
| context-graph-embeddings | 195 | 1,388 | 32 |
| context-graph-graph-agent | 8 | 64 | 1 |
| context-graph-mcp | 58 | 347 | 1 |
| context-graph-storage | 64 | 546 | 17 |
| context-graph-test-utils | 0 | 0 | 0 |
| **Total** | **624** | **5,199** | **68** |

Re-run verification:
```bash
for c in crates/*; do echo "=== $c ==="; grep -rhE "^\s*#\[(tokio::)?test\]" "$c" 2>/dev/null | wc -l; done
```

---

## 3. Integration Test Inventory

Integration files live under `crates/*/tests/`. Each file is a separate test binary.

### context-graph-causal-agent (`crates/context-graph-causal-agent/tests/`)

| File | Covers |
|---|---|
| `analyze_single_text_integration.rs` | `CausalDiscoveryLLM.analyze_single_text()` LLM path |
| `background_loop_integration.rs` | Background causal-discovery loop lifecycle (LLM-gated) |

### context-graph-cli (`crates/context-graph-cli/tests/`)

| File / Subdir | Covers |
|---|---|
| `export_training_integration.rs` | `context-graph-cli export training-corpus` |
| `e2e/full_session_test.rs` | Full CLI session lifecycle |
| `e2e/error_recovery_test.rs` | CLI error paths and graceful recovery |
| `e2e/helpers.rs` | E2E test harness |
| `integration/timeout_test.rs` | Per-command timeout behaviour |
| `integration/exit_code_test.rs` | Exit-code contract |
| `integration/hook_lifecycle_test.rs` | Hook install/trigger/remove |
| `integration/helpers.rs` | Integration harness |

### context-graph-core (`crates/context-graph-core/tests/`)

| File | Covers |
|---|---|
| `file_watcher_integration.rs` | FS watcher + stale-embedding cleanup (needs `test-utils`) |
| `source_metadata_integration.rs` | `SourceMetadata` end-to-end (needs `test-utils`) |
| `synthesizer_edge_case_test.rs` | `TopicSynthesizer` edge cases |
| `rrf_edge_cases.rs` | Reciprocal-Rank-Fusion edge cases |
| `injection_manual_test.rs` | `InjectionCandidate` / `InjectionCategory` edges |

### context-graph-embeddings (`crates/context-graph-embeddings/tests/`)

| File / Module | Covers |
|---|---|
| `attention_strategy_tests.rs` | Attention strategy implementations |
| `benchmark_test.rs` | Performance benches (timing bands) |
| `bge_m3_tokenizer_probe.rs` | BGE-M3 tokenizer probing |
| `embed_dual_with_hint_integration.rs` | E5 `embed_dual_with_hint()` asymmetric path |
| `e9_typo_tolerance_test.rs` | E9 HDC typo-robust retrieval |
| `e9_vector_differentiation_test.rs` | E9 vectors distinct from E1 (gated on model files) |
| `global_provider_fsv.rs` | Global embedder provider full-state verification |
| `gpu_memory_slots_test.rs` | GPU slot management |
| `no_fake_data_test.rs` | AP-007 compliance: reject synthetic vectors |
| `qodo_embed_integration.rs` | Qodo-Embed 1.5B E7 real inference (`--ignored`) |
| `search_test.rs` | Multi-space search integration |
| `dimension_test/*.rs` (10 files) | Quantization, offsets, model metadata, projections, native dims |
| `storage_roundtrip_test/*.rs` (9 files) | Fingerprint↔storage roundtrip + RRF formula |
| `fixtures/{create_fixture,physical_verification_test}.rs` | Golden-fixture maintenance |

### context-graph-graph-agent (`crates/context-graph-graph-agent/tests/`)

| File | Covers |
|---|---|
| `scanner_integration.rs` | `MemoryScanner` end-to-end |

### context-graph-mcp (`crates/context-graph-mcp/tests/`)

| File | Covers |
|---|---|
| `graph_linking_integration.rs` | Graph-linking MCP handler + RocksDB |

### context-graph-storage (`crates/context-graph-storage/tests/`)

| File / Module | Covers |
|---|---|
| `anomaly_derivation_integration.rs` | F2 anomaly derivation from typed edges |
| `constellation_integration.rs` | `CF_CONSTELLATIONS` + selector index |
| `contrastive_integration.rs` | `CF_CONTRASTIVE_PAIRS` + kind index |
| `crash_recovery_tests.rs` | RocksDB crash-recovery |
| `e12_e13_source_of_truth_verification.rs` | E12/E13 canonical storage |
| `e9_storage_roundtrip_test.rs` | E9 vectors survive roundtrip distinct from E1 |
| `llm_validation_integration.rs` | `CF_TYPED_EDGE_VALIDATIONS` (F4) |
| `teleological_integration.rs` | Teleological store end-to-end |
| `training_records_integration.rs` | `CF_TRAINING_RECORDS` persistence |
| `typed_edge_export_integration.rs` | Typed-edge export path |
| `full_integration_real_data/*.rs` (7 files) | Pipeline, operations, roundtrip, search, perf |

---

## 4. Run Commands

### 4.1 Workspace-wide

```bash
# Full test run (default features chain: mcp default = ["cuda", "llm"])
cargo test --workspace

# Lib tests only (inline #[cfg(test)])
cargo test --workspace --lib

# Integration tests only (crates/*/tests)
cargo test --workspace --tests

# Makefile convenience
make test
```

### 4.2 Per-crate

```bash
cargo test -p context-graph-core
cargo test -p context-graph-storage
cargo test -p context-graph-embeddings
cargo test -p context-graph-mcp              # make test-mcp
cargo test -p context-graph-cli --test e2e   # make test-e2e
```

### 4.3 Feature selection

Default features per crate — see feature flag matrix in [15_verification_report.md](15_verification_report.md).

```bash
# Core (no default features)
cargo test -p context-graph-core

# Core with test-utils (required for file_watcher/source_metadata tests)
cargo test -p context-graph-core --features test-utils

# Embeddings: candle is default AND required (compile_error! in lib.rs without it)
cargo test -p context-graph-embeddings

# MCP without LLM (saves ~500MB llama-cpp-2 binary)
cargo test -p context-graph-mcp --no-default-features --features cuda

# Storage: cuda is default and pulls embeddings/candle
cargo test -p context-graph-storage
```

### 4.4 Ignored tests (opt-in)

```bash
cargo test --workspace -- --ignored
cargo test -p context-graph-embeddings --test qodo_embed_integration -- --ignored
```

### 4.5 Benches

```bash
cargo bench -p context-graph-storage --bench maxsim_bench
cargo bench -p context-graph-storage --bench causal_relationships_bench
cargo bench -p context-graph-storage --bench hybrid_effectiveness_bench
cargo bench -p context-graph-storage --bench e11_quality_bench
```

### 4.6 Lint

```bash
make clippy      # cargo clippy --workspace --all-targets -- -D warnings
make check       # cargo check --workspace --all-targets
```

---

## 5. Shared Fixtures (`crates/context-graph-test-utils/`)

Consumed via `context-graph-test-utils = { path = "../context-graph-test-utils" }` under `[dev-dependencies]`.

| Module | Public items | Purpose |
|---|---|---|
| `lib.rs` | Re-exports from `fingerprints` + `stores` | Facade |
| `fingerprints.rs` | `generate_real_unit_vector(dim)`, `generate_real_sparse_vector(nnz)`, `generate_real_semantic_fingerprint()`, `generate_real_content_hash()`, `create_real_fingerprint()`, `create_real_fingerprint_with_id(uuid)`, `generate_real_teleological_fingerprint(uuid)`, `hex_string(bytes)` | Produces L2-normalised unit vectors with correct dimensions per embedder (E1=1024, E2-E4=512, E5=768 dual, E6=sparse 30K, E7=1536, E8=1024 dual, E9=1024, E10=768 dual, E11=768, E12=128×16 tokens, E13=sparse 30K). Legacy `e5_causal` / `e8_graph` fields intentionally empty to exercise dual-vector paths. |
| `stores.rs` | `create_test_store(&TempDir)`, `create_initialized_store(&Path)` | Opens `RocksDbTeleologicalStore` with all 59 CFs registered |

Binary golden fixtures: `tests/fixtures/golden/E{2,3,4,5,6,7,8,9,10,11}_*/` — each directory contains `test_input.bin`, `golden_output.bin`, `checksum.txt` consumed by embedder-specific roundtrip tests.

---

## 6. Ignored Tests

Total `#[ignore]` markers: **27** (verify: `grep -rhE '#\[ignore' crates/*/src crates/*/tests | wc -l`).

| Group | Count | Location | Reason |
|---|---:|---|---|
| GPU-gated HDBSCAN | 8 | `crates/context-graph-cuda/src/hdbscan/tests.rs` | `requires GPU` (runtime CUDA device) |
| Qodo-Embed E7 | 10 | `crates/context-graph-embeddings/tests/qodo_embed_integration.rs` | `loads 1.5B Qodo-Embed model; run with --ignored` |
| Pretrained-weight gated | 3 | `crates/context-graph-embeddings/{src/provider/diagnostic_test.rs,tests/e9_vector_differentiation_test.rs}` | `requires models directory with pretrained weights` |
| LLM-weight gated | 4 | `crates/context-graph-causal-agent/{src/service/mod.rs, tests/background_loop_integration.rs}` | `requires LLM model weights` / `loads 7B GGUF LLM from disk` |
| HNSW perf | 1 | `crates/context-graph-storage/src/teleological/indexes/hnsw_impl/tests.rs` | `builds 10K-vector HNSW index (~5-15s); perf benchmark` |
| Misc | 1 | remainder | — |

Run all ignored: `cargo test --workspace -- --ignored`.

---

## 7. Chaos / Fault Injection

`tests/chaos/mod.rs` is a stub referencing relocated chaos tests. There is no active chaos test binary currently wired into the workspace. Crash-recovery testing is covered by `crates/context-graph-storage/tests/crash_recovery_tests.rs` (RocksDB).

---

## 8. Python Verification Harness (`scripts/mcp_full_verify.py`)

End-to-end verification against a live `target/release/context-graph-mcp` binary over JSON-RPC / stdio.

| Property | Value |
|---|---|
| Protocol | JSON-RPC 2.0 over stdio (MCP 2024-11-05) |
| Server binary | `target/release/context-graph-mcp` |
| Per-call timeout | 90 s |
| Total phases | 13 (numbered 1-12 + 14; phase 13 is reserved) |
| Approx test count | ~85 |
| Python deps | stdlib only (`json`, `subprocess`, `threading`, `queue`) |

Phases:

| # | Title |
|---:|---|
| 1 | Infrastructure Health Check (embedder count=13, storage=rocksdb, causal gate, thresholds, weight profiles ≥ 14) |
| 2 | Core Memory Operations (CRUD) |
| 3 | Search Strategy Verification (e1_only, multi_space, pipeline, embedder_first) |
| 4 | Individual Embedder Verification (all 13 embedders) |
| 5 | Weight Profiles (built-in + custom) |
| 6 | Causal Subsystem (E5 + LLM) |
| 7 | Graph Subsystem (E8 + LLM) |
| 8 | Entity Subsystem (E11) |
| 9 | Topic, Session, and Temporal |
| 10 | Provenance, Audit Trail, and Curation |
| 11 | Embedder-First Advanced Tools |
| 12 | File Watcher Subsystem |
| 14 | Error Handling and Edge Cases |

Run:

```bash
# 1. Build release server (requires CUDA + LLM features — see 15_verification_report.md)
cargo build --release -p context-graph-mcp

# 2. Run verification
python3 scripts/mcp_full_verify.py
```

Sibling `scripts/download_models.py` requires `huggingface_hub` + `torch`; verify.py itself uses stdlib only.

---

## 9. Test Invariants

- Fingerprint generators produce **real** (random but correctly dimensioned and L2-normalised) vectors — never zero, never hardcoded. AP-007 compliance is enforced by `crates/context-graph-embeddings/tests/no_fake_data_test.rs`.
- `create_test_store(&TempDir)` registers all 59 column families at open time — verified by `TOTAL_COLUMN_FAMILIES` assertion in `crates/context-graph-storage/src/column_families.rs`.
- Integration tests requiring `context-graph-core` feature `test-utils` declare this via `[[test]] required-features = ["test-utils"]` in `crates/context-graph-core/Cargo.toml`.
- `#[tokio::test(flavor = "multi_thread")]` is used for tests that spawn background tasks; single-threaded runtime is default.
