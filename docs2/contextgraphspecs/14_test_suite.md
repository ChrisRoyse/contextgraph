# Test Suite

**Source files covered:**
- `crates/*/tests/**/*.rs` — top-level integration / FSV / E2E targets
- `crates/*/src/**/tests/**/*.rs` and `crates/*/src/**/tests.rs` — unit tests embedded under `mod tests`
- `crates/context-graph-test-utils/src/{lib.rs,fingerprints.rs,stores.rs}` — shared real-data fixtures (no mocks)
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — tool-count assertions
- `crates/context-graph-storage/src/dynamicjepa/tests.rs` — DJ CF and serialization assertions (`DJ_CF_COUNT == 28`)
- `crates/context-graph-cli/src/bin/reality-loop.rs` — Rust SWE-bench reality-loop binary
- `tests/chaos/mod.rs` — root-level chaos test redirect (legacy stub; see §11)
- `Makefile` — `test`, `test-rust`, `test-mcp`, `test-e2e`, `test-python-smoke` targets
- `scripts/v4_2/*_fsv.py` — Python FSV harnesses

Cross-references: [10_cli_reference.md](10_cli_reference.md), [15_verification_report.md](15_verification_report.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md), [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md), [04_database_schema.md](04_database_schema.md).

---

## 1. Workspace Test Metrics

All counts produced by `find` / `grep` on the working tree (2026-05-08).

| Metric | Value | Source command |
| --- | --- | --- |
| Workspace crates | 11 | `Cargo.toml` `[workspace.members]` |
| Default-members crates | 10 (benchmark excluded) | `Cargo.toml` `default-members` |
| `.rs` files in repo (excl. `target/`) | 1,361 | `find . -name '*.rs' -not -path '*/target/*'` |
| `.rs` files under `crates/*/src` | 1,219 | `find crates -name '*.rs' -path '*/src/*'` |
| `.rs` files under `crates/*/tests` | 246 | `find crates -name '*.rs' -path '*/tests/*'` |
| Total Rust LOC (workspace) | 581,933 | `find crates -name '*.rs' \| xargs wc -l` |
| Source LOC (`crates/*/src`) | 523,153 | `find crates -name '*.rs' -path '*/src/*' \| xargs wc -l` |
| Integration-tests LOC (`crates/*/tests`) | 68,783 | `find crates -name '*.rs' -path '*/tests/*' \| xargs wc -l` |
| Files containing `#[test]` / `#[tokio::test]` | 699 | `grep -rln '#\[test\]\|#\[tokio::test\]' crates` |
| `#[test]` / `#[tokio::test]` attributes (workspace) | 5,524 | `grep -rn '#\[test\]\|#\[tokio::test\]' crates \| wc -l` |
| FSV-tagged test files (`*_fsv.rs`) | 46 | `find crates -name '*_fsv.rs'` |
| Python FSV scripts (`scripts/v4_2/`) | 9 | `find scripts/v4_2 -name '*_fsv.py'` |
| Python scripts in `scripts/v4_2/` | 32 | `find scripts/v4_2 -maxdepth 1 -name '*.py'` |
| Python scripts (whole `scripts/`) | 126 | `find scripts -name '*.py'` |

Note: the previously-documented `gemmafrontend/` Python harness directory was removed in this branch (cutover commits) — the only Python FSV layer that remains is `scripts/v4_2/`.

---

## 2. Test Files per Crate

`tests-dir` counts files anywhere under any `tests/` directory inside the crate (top-level + `src/**/tests/`). `test-attrs` is the count of `#[test]` + `#[tokio::test]` attributes inside the crate.

| # | Crate | `.rs` total | `tests/` files | `#[test]` attrs | Default-members |
| --- | --- | ---: | ---: | ---: | --- |
| 1 | `context-graph-mcp` | 119 | 11 | 391 | yes |
| 2 | `context-graph-core` | 312 | 31 | 2,129 | yes |
| 3 | `context-graph-cuda` | 20 | 1 | 65 | yes |
| 4 | `context-graph-embeddings` | 473 | 113 | 1,405 | yes |
| 5 | `context-graph-storage` | 158 | 34 | 591 | yes |
| 6 | `context-graph-dynamicjepa` | 8 | 1 | 7 | yes |
| 7 | `context-graph-cli` | 95 | 52 | 446 | yes |
| 8 | `context-graph-causal-agent` | 14 | 2 | 52 | yes |
| 9 | `context-graph-graph-agent` | 13 | 1 | 64 | yes |
| 10 | `context-graph-test-utils` | 3 | 0 | 0 | yes |
| 11 | `context-graph-benchmark` | 142 | 0 | 374 | **no** |

`context-graph-benchmark` is built with `cargo build -p context-graph-benchmark` because it is excluded from `default-members` (the crate is ~87K LOC with 31 binaries + 2 Criterion benches).

---

## 3. Test Categories

| Category | Pattern | Notes |
| --- | --- | --- |
| Unit | `#[test]` inside `mod tests {}` blocks under `src/` | Embedded in source. Majority of the 5,524 attributes. |
| Integration (top-level) | `crates/*/tests/*.rs` | Each file is its own integration target compiled separately. |
| FSV (Full-State Verification) | `*_fsv.rs`, `*_full_state_verification.rs` | Physical RocksDB readback, audit-trail diff, cross-CF invariants. |
| E2E | `crates/context-graph-cli/tests/e2e/{full_session_test.rs, error_recovery_test.rs, helpers.rs, mod.rs}` | Driven via `[[test]] name = "e2e" path = "tests/e2e/mod.rs"`. |
| CLI integration | `crates/context-graph-cli/tests/integration/{hook_lifecycle_test.rs, exit_code_test.rs, timeout_test.rs, helpers.rs, mod.rs}` | Driven via `[[test]] name = "integration"`. |
| Roundtrip | `e9_storage_roundtrip_test.rs`, embeddings `*roundtrip*` | Encode → store → reload → decode parity. |
| Real-data | `crates/context-graph-storage/tests/full_integration_real_data/{main.rs, helpers.rs, operations_tests.rs, performance_tests.rs, pipeline_tests.rs, roundtrip_tests.rs, search_tests.rs}` | Real embeddings on real corpus (uses `context-graph-test-utils` factories). |
| Crash recovery | `crates/context-graph-storage/tests/crash_recovery_tests.rs` | Reopen RocksDB after dirty shutdown. |
| Tool-count assertions | `crates/context-graph-mcp/src/tools/definitions/mod.rs:144-223` | `test_total_tool_count_and_no_duplicates`, `test_submodule_counts`, `test_reality_loop_lightweight_registry_only_lists_callable_tools`. |
| DJ CF assertions | `crates/context-graph-storage/src/dynamicjepa/tests.rs` | Asserts `DJ_CF_COUNT == 28` and individual CF descriptors round-trip. |
| Reality-loop FSV | `crates/context-graph-cli/src/bin/reality-loop.rs` | Rust-owned SWE-bench refinement loop (see [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md)). |

---

## 4. Run Commands

| Command | Purpose |
| --- | --- |
| `cargo build --release --workspace` | Build everything in `default-members`. Benchmark crate is not built. |
| `cargo build -p context-graph-benchmark` | Explicitly build the excluded benchmark crate. |
| `cargo test --workspace` | Default test profile, all default-members crates. Used by `make test-rust`. |
| `cargo test --release` | Same suite under release profile. |
| `cargo test -p context-graph-mcp` | MCP server tests (handler integration + tool registry assertions). |
| `cargo test -p context-graph-storage` | Storage tests (CF init, recovery, FSVs, real-data integration). |
| `cargo test -p context-graph-core` | Core (fingerprints, retrieval, fusion, learning, learner, training, dynamicjepa core types). |
| `cargo test -p context-graph-embeddings` | All 14 embedders (per-model unit + integration + roundtrip). |
| `cargo test -p context-graph-cli` | CLI integration + E2E + 25 BlackBoxFix phase FSVs + 11 DynamicJEPA FSVs + E14 Phase B. |
| `cargo test -p context-graph-cli --test e2e` | E2E only. |
| `cargo test -p context-graph-cli --test integration` | Hook lifecycle / exit codes / timeout. |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 145 (`llm`) / 141 (no-llm). |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_submodule_counts` | Asserts every submodule's tool count (see [15_verification_report.md §7](15_verification_report.md#7-mcp-tool-composition)). |
| `cargo bench --package context-graph-cli` | Run `cli_benchmarks` Criterion target. |
| `cargo bench --package context-graph-storage` | Run all four storage benches (`maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench`). |
| `cargo bench --package context-graph-benchmark` | Run `comparative_suite` and `scaling_analysis`. |
| `cargo run --release --bin reality-loop -- --task-manifest <path>.json --official-eval-each-attempt` | Drive one SWE-bench attempt via the Rust reality loop ([22](22_reality_loop_subsystem.md)). |
| `make test` | `test-rust` + `test-python-smoke`. |
| `make test-rust` | `cargo test --workspace` + `scripts/trim-stale-deps.sh debug`. |
| `make test-mcp` | `cargo test -p context-graph-mcp`. |
| `make test-e2e` | `cargo test -p context-graph-cli --test e2e`. |
| `make test-python-smoke` | Pytest in `clipcannon/` against `$(PYTHON_SMOKE_TESTS)`. |
| `make test-python` | Full Pytest in `clipcannon/tests/`. |
| `make clippy` / `make lint-rust` | `cargo clippy --workspace --all-targets -- -D warnings`. |

---

## 5. Bench Inventory (Criterion)

Per-crate `[[bench]]` declarations in `crates/*/Cargo.toml`. All benches run with `harness = false` (Criterion).

| Crate | Bench file | Purpose |
| --- | --- | --- |
| `context-graph-cli` | `benches/cli_benchmarks.rs` | CLI command latency / throughput (Phase 6 P-budget validation). |
| `context-graph-storage` | `benches/maxsim_bench.rs` | E12 ColBERT MaxSim retrieval latency. |
| `context-graph-storage` | `benches/causal_relationships_bench.rs` | E5 causal-relationship traversal. |
| `context-graph-storage` | `benches/hybrid_effectiveness_bench.rs` | Multi-space + RRF hybrid recall/precision. |
| `context-graph-storage` | `benches/e11_quality_bench.rs` | E11 entity-embedding retrieval quality. |
| `context-graph-benchmark` | `benches/comparative_suite.rs` | Comparative suite across configurations. |
| `context-graph-benchmark` | `benches/scaling_analysis.rs` | Throughput / latency vs. corpus size. |

`context-graph-benchmark` also exposes 31 `[[bin]]` entries (e.g. `benchmark-report`, `realdata-bench`, `gpu-bench`, `mcp-bench`) used to assemble end-to-end SWE-bench-style measurement runs from the command line.

---

## 6. Notable FSV Targets (Rust)

All paths relative to repo root.

| Target | Crate | Coverage |
| --- | --- | --- |
| `crates/context-graph-storage/tests/e14_fsv.rs` | storage | E14 BGE-M3 dense end-to-end on RocksDB |
| `crates/context-graph-cli/tests/e14_optimization_fsv.rs` | cli | E14 fast-path + memory budget |
| `crates/context-graph-storage/tests/full_14_embedder_fsv.rs` | storage | All 14 embedders (E1–E14) end-to-end |
| `crates/context-graph-storage/tests/learning_events_fsv.rs` | storage | `LearningEvent` ingest / readback |
| `crates/context-graph-storage/tests/utl_learner_state_fsv.rs` | storage | Learner-state CFs (Phase 0/1) |
| `crates/context-graph-storage/tests/utl_learner_embedders_fsv.rs` | storage | E15–E21 slot listing |
| `crates/context-graph-cli/tests/e14_phase_b_fsv.rs` | cli | E14 ingest → search → consistency |
| `crates/context-graph-cli/tests/e14_phase_b_full_state_verification.rs` | cli | Full state diff |
| `crates/context-graph-cli/tests/blackbox_phase{1..24}_*_fsv.rs` | cli | 24 BlackBoxFix phase FSVs (`phase1` through `phase24_sdk_ingest`) |
| `crates/context-graph-cli/tests/blackbox_cross_phase_api_surface_hardening_fsv.rs` | cli | Cross-phase API surface guard |
| `crates/context-graph-cli/tests/blackbox_reward_compiler_fsv.rs` | cli | Reward-spec compiler (cross-references reality-loop reward spec) |
| `crates/context-graph-cli/tests/dynamicjepa_phase{3,4,5,6,7,8,10,12}_fsv.rs` | cli | DynamicJEPA per-phase FSVs |
| `crates/context-graph-cli/tests/dynamicjepa_pairwise_mi_phase5_fsv.rs` | cli | Phase-5 pairwise MI audit |
| `crates/context-graph-cli/tests/dynamicjepa_cross_domain_transfer_phase6_v21_fsv.rs` | cli | Cross-domain transfer |
| `crates/context-graph-cli/tests/dynamicjepa_research_smoke_fsv.rs` | cli | Research smoke battery |
| `crates/context-graph-cli/tests/dynamicjepa_bundle_export_fsv.rs` | cli | Episode JSONL export |
| `crates/context-graph-cli/tests/e2e/full_session_test.rs` | cli | Full session lifecycle |
| `crates/context-graph-cli/tests/e2e/error_recovery_test.rs` | cli | Recovery from corrupted state |
| `crates/context-graph-storage/tests/crash_recovery_tests.rs` | storage | RocksDB reopen after dirty shutdown |
| `crates/context-graph-storage/tests/contrastive_integration.rs` | storage | Phase-3 contrastive pair miner |
| `crates/context-graph-storage/tests/typed_edge_export_integration.rs` | storage | Phase-4 typed-edge export |
| `crates/context-graph-storage/tests/training_records_integration.rs` | storage | Training record schema (`TRAINING_RECORD_VERSION = 3`) |
| `crates/context-graph-storage/tests/teleological_integration.rs` | storage | 13-D teleological vectors + CFs |
| `crates/context-graph-storage/tests/constellation_integration.rs` | storage | Constellation compile/score/delete |
| `crates/context-graph-storage/tests/anomaly_derivation_integration.rs` | storage | Cross-embedder anomalies |
| `crates/context-graph-storage/tests/e12_e13_source_of_truth_verification.rs` | storage | E12 ColBERT + E13 SPLADE invariants |
| `crates/context-graph-storage/tests/e9_storage_roundtrip_test.rs` | storage | E9 HDC roundtrip |
| `crates/context-graph-storage/tests/llm_validation_integration.rs` | storage | Hermes-2-Pro grammar-constrained outputs |
| `crates/context-graph-mcp/tests/graph_linking_integration.rs` | mcp | Graph linker handler |
| `crates/context-graph-core/tests/rrf_edge_cases.rs` | core | RRF fusion edge cases |
| `crates/context-graph-core/tests/synthesizer_edge_case_test.rs` | core | Context synthesizer edge cases |
| `crates/context-graph-core/tests/source_metadata_integration.rs` | core | Provenance / source metadata |
| `crates/context-graph-core/tests/file_watcher_integration.rs` | core | Markdown watcher → memory store |
| `crates/context-graph-embeddings/tests/global_provider_fsv.rs` | embeddings | Global provider end-to-end |
| `crates/context-graph-dynamicjepa/tests/cuda_smoke.rs` | dynamicjepa | CUDA smoke for tiny JEPA stack |
| `crates/context-graph-cuda/tests/native_cubin_smoke.rs` | cuda | Native cubin smoke |

---

## 7. Embedded Storage Test Trees

Storage carries multiple in-source `tests/` directories in addition to top-level integration files:

| Path | Purpose |
| --- | --- |
| `src/teleological/tests/{column_family,content,key_format,panic,serialization,stale_lock,helpers,mod}.rs` | Teleological CF lifecycle, key format, panic safety, stale-lock recovery |
| `src/teleological/quantized/tests.rs` | PQ-8 quantization round-trip |
| `src/teleological/rocksdb_store/tests.rs` | RocksDB store unit tests |
| `src/teleological/search/{pipeline,multi}/tests.rs` | Pipeline + multi-space search unit tests |
| `src/teleological/search/single/tests/` | Single-space search unit tests (directory) |
| `src/teleological/indexes/hnsw_impl/tests.rs` | HNSW types + insert/remove |
| `src/dynamicjepa/tests.rs` | DJ CFs and serialization (`DJ_CF_COUNT == 28`) |

Core mirrors this pattern with `src/teleological/{comparator,profile,services/profile_manager}/tests.rs` and `src/dynamicjepa/tests.rs`.

---

## 8. Shared Test Fixtures

Source: `crates/context-graph-test-utils/src/`. The crate is library-only with zero of its own tests (`tests-dir = 0`); it is consumed via `[dev-dependencies]` in every other crate.

| Module | Public exports |
| --- | --- |
| `fingerprints.rs` | `create_real_fingerprint`, `create_real_fingerprint_with_id`, `generate_real_content_hash`, `generate_real_semantic_fingerprint`, `generate_real_sparse_vector`, `generate_real_teleological_fingerprint`, `generate_real_unit_vector`, `hex_string` |
| `stores.rs` | `create_test_store`, `create_initialized_store` |
| `lib.rs` | Re-exports the above at crate root |

All fixtures generate **REAL** data (no mocks). Tests using these fixtures assert real RocksDB writes, real HNSW inserts, and real cosine recall. The "no fake data" rule is also enforced by `crates/context-graph-embeddings/tests/no_fake_data_test.rs`.

---

## 9. Tool-Count Assertions

Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs:138-224`.

| Test | Asserts |
| --- | --- |
| `test_total_tool_count_and_no_duplicates` | `tools.len() == 145` (llm) / `141` (no-llm); no duplicate names; every tool has a non-empty `description` and an `input_schema` with `type`. |
| `test_submodule_counts` | Per-submodule counts (full table in [15_verification_report.md §7](15_verification_report.md#7-mcp-tool-composition)). Highlights: `dynamicjepa = 35`, `utl = 18`, `learning = 14`, `entity = 6`, `constellation = 6`, `core = 5`. |
| `test_reality_loop_lightweight_registry_only_lists_callable_tools` | The reduced `--mode reality-loop` registry equals `reality::definitions()` (36 tools), contains `reality_attempt_summary` / `dynamicjepa_reality_for_attempt` / `optimizer_record_recommendation`, and excludes `search_graph` / `get_memory_neighbors` / `get_audit_trail`. |

---

## 10. Python FSV Scripts (`scripts/v4_2/`)

| Script | Coverage |
| --- | --- |
| `bigunlock_fsv_lib.py` | BigUnlock FSV harness library (shared utilities) |
| `learner_state_bigunlock_fsv.py` | Learner-state end-to-end |
| `shakespeare_corpus_fsv.py` | Shakespeare corpus ingest + retrieve |
| `shakespeare_state_conditioned_fsv.py` | State-conditioned retrieval |
| `shakespeare_learner_training_fsv.py` | Learner training cycle |
| `shakespeare_learning_event_fsv.py` | Learning event ingest |
| `unified_realdata_fsv.py` | Unified real-data FSV (all embedders) |
| `extended_fsv.py` | Extended CAMD sidecar |
| `cuda13_wsl_fsv.py` | CUDA 13 / WSL2 Blackwell smoke |
| `conceptual_capabilities_fsv.py` | Capability-matrix readback |

Reality-loop FSV harnesses formerly lived under `gemmafrontend/`; that directory was removed in the `ccreality-cutover` branch and is no longer present in the working tree. Reality-loop coverage is now exclusively the Rust binary `crates/context-graph-cli/src/bin/reality-loop.rs` plus the BlackBoxFix phase FSV files in `crates/context-graph-cli/tests/`.

---

## 11. Test Profiles

| Profile | When | Notes |
| --- | --- | --- |
| `dev` | `cargo test`, `make test-rust` | `opt-level = 0`, `debug = 1` (line info only), `incremental = true`, `split-debuginfo = "unpacked"`. External deps `debug = false` (`[profile.dev.package."*"]`). |
| `release` | `cargo test --release` | `lto = "thin"`, `codegen-units = 1`, `strip = true`. |
| Toolchain | `rust-toolchain.toml` | `channel = "stable"`, `components = ["rustfmt", "clippy", "rust-src"]`, `targets = ["x86_64-unknown-linux-gnu"]`. Workspace `rust-version = "1.95"`, edition `2021`. |

Both `make build-rust` and `make test-rust` chain `scripts/trim-stale-deps.sh` afterwards (the Makefile header documents that an 11-crate workspace produces ~600 MB of binaries per build → 240 GB+ within weeks without trimming).

---

## 12. Coverage Notes

- Coverage percentages are not instrumented in this workspace. The numbers above are static (`find` + `grep` + Cargo declarations).
- `tests/chaos/mod.rs` is a redirect-only stub pointing at a `context-graph-graph` crate that no longer exists in `Cargo.toml` `[workspace.members]`. Effective chaos-test coverage at HEAD is zero — flagged for follow-up.
- `cargo test --workspace` does not exercise feature-gated code paths when the gate is off: `llm`-feature MCP tools (`graph_*` LLM-gated tools, `causal_discovery`) and CUDA-only embedders are skipped without the `llm` and `cuda` features respectively. `make test-rust` runs with the workspace defaults (`cuda` on for storage / embeddings / cli; `llm` on for the `mcp` crate when its default feature set includes it).
- See [15_verification_report.md §4](15_verification_report.md#4-most-recent-test-verifications) for the most recent successful test counts and audit history.
