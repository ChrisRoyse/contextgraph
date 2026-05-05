# Test Suite

**Source files covered:**
- `crates/*/tests/**/*.rs` — integration / FSV / E2E
- `crates/*/src/**/tests/**/*.rs` — unit tests embedded under `mod tests`
- `crates/context-graph-test-utils/src/{lib.rs,fingerprints.rs,stores.rs}` — shared fixtures
- `crates/context-graph-mcp/src/tools/definitions/mod.rs` — tool-count assertions
- `crates/context-graph-cli/src/bin/reality-loop.rs` + `gemmafrontend/manual_fsv.py` — reality loop FSV harness
- `tests/chaos/mod.rs` — root-level chaos test redirect (legacy stub)
- `Makefile` — test targets
- `scripts/v4_2/*_fsv.py` — Python FSV harnesses

Cross-references: [10_cli_reference.md](10_cli_reference.md), [15_verification_report.md](15_verification_report.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md).

## 1. Workspace Test Metrics (counted at refresh)

| Metric | Value | Source |
| --- | --- | --- |
| Workspace crates | 11 | `Cargo.toml` `[workspace.members]` |
| Default-members crates | 10 (benchmark excluded) | `Cargo.toml` `default-members` |
| All `.rs` files in repo | 1,346 | `find . -name '*.rs' -not -path '*/target/*'` |
| `.rs` files under `crates/*/src` | 1,204 | sources |
| `.rs` files under `crates/*/tests` | 246 | integration / FSV / E2E (all integration trees) |
| `.rs` files containing a `#[test]`/`#[tokio::test]` | 687 | grep |
| `#[test]` / `#[tokio::test]` attributes (workspace) | 5,437 | grep |
| FSV-tagged test files | 46 | filename grep `*_fsv.rs` |
| Python FSV scripts (`scripts/v4_2/`) | 32 | filesystem |
| Python scripts (`scripts/`, total) | 126 | filesystem |
| Python harness scripts (`gemmafrontend/`) | 6 (`server.py`, `swe_lite_refinement_loop.py`, `swe_lite_official_eval.py`, `reality_prompt_compiler.py`, `model_benchmark.py`, `model_prompt_matrix.py`) + `manual_fsv.py` | filesystem |

## 2. Test Files per Crate

Source: `find crates -name '*.rs' -path '*/tests/*'` (counts include both top-level `tests/` and `src/**/tests/` modules).

| Crate | Integration / FSV / unit-tests files | Notes |
| --- | --- | --- |
| `context-graph-embeddings` | ~120 | Per-model unit/integration suites + roundtrip + warm provider tests |
| `context-graph-cli` | 41 (top-level `tests/`) | BlackBoxFix Phase 1–24 FSVs, DynamicJEPA Phase 3–12 FSVs, E14 Phase B FSV, reward compiler FSV, cross-phase API surface FSV, E2E session/error/recovery, integration timeout/exit code, hook lifecycle |
| `context-graph-storage` | 34 (top-level) + storage-internal `src/teleological/{search/single,...}/tests/*.rs` | Crash recovery, RocksDB CF init, full-14-embedder FSV, learner state/embedder FSV, contrastive/typed-edge/training/teleological/llm validation integration, real-data full integration suite |
| `context-graph-core` | ~31 (mix of top-level + src-internal tests) | Fingerprints, weight profiles, retrieval RRF edge cases, file watcher, source metadata, synthesizer edge cases, learning |
| `context-graph-mcp` | 11 (top-level + handler `src/handlers/tests/*.rs`) | Graph linking integration + per-tool handler tests |
| `context-graph-causal-agent` | 2 | Causal agent integration |
| `context-graph-graph-agent` | 1 | Scanner integration |
| `context-graph-dynamicjepa` | 1 | CUDA smoke (cuda_smoke.rs) |
| `context-graph-cuda` | 1 | Native cubin smoke |
| `context-graph-benchmark` | (excluded) | Benchmarks live under `benches/` + `src/bin/`, not in default `cargo test` |
| `context-graph-test-utils` | 0 | Library-only crate (re-exports `create_real_fingerprint`, `generate_real_*`, store factories) |

The exact 246 figure for `.rs` files under `crates/*/tests` includes nested module helpers (`helpers.rs`, `mod.rs`, `dimension_test/*`, `e2e/*`, `integration/*`, `full_integration_real_data/*`).

## 3. Test Categories

| Category | Pattern | Notes |
| --- | --- | --- |
| Unit | `#[test]` inside `mod tests {}` blocks in `src/` | Embedded in source; majority of the 5,437 `#[test]` attrs |
| Integration | `crates/*/tests/*.rs` (top-level) | Each file becomes its own integration target |
| FSV (Full-State Verification) | `*_fsv.rs`, `*_full_state_verification.rs` | Physical RocksDB readback + state diff + audit trail |
| E2E | `crates/context-graph-cli/tests/e2e/{full_session_test.rs, error_recovery_test.rs, helpers.rs, mod.rs}` | Driven via `[[test]] name = "e2e" path = "tests/e2e/mod.rs"` |
| CLI integration | `crates/context-graph-cli/tests/integration/{hook_lifecycle_test.rs, exit_code_test.rs, timeout_test.rs, helpers.rs, mod.rs}` | Driven via `[[test]] name = "integration"` |
| Roundtrip | `e9_storage_roundtrip_test.rs`, embeddings `*roundtrip*` | Encode → decode parity |
| Real-data | `crates/context-graph-storage/tests/full_integration_real_data/{main.rs, helpers.rs, operations_tests.rs, performance_tests.rs, pipeline_tests.rs, roundtrip_tests.rs, search_tests.rs}` | Real embeddings on real corpus (uses test utils) |
| Crash recovery | `crates/context-graph-storage/tests/crash_recovery_tests.rs` | Open store after dirty shutdown |
| Tool-count assertions | `crates/context-graph-mcp/src/tools/definitions/mod.rs:131-198` | `test_total_tool_count_and_no_duplicates`, `test_submodule_counts` |
| Reality-loop FSV (Python) | `gemmafrontend/manual_fsv.py`, `gemmafrontend/model_benchmark.py`, `gemmafrontend/model_prompt_matrix.py` | Reality loop / SWE-bench / verified-outcome harnesses (see [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md)) |

## 4. Run Commands

| Command | Purpose |
| --- | --- |
| `cargo build --release --workspace` | Build everything (warnings → error per `make clippy`) |
| `cargo test --release` | Default workspace test run (release profile) |
| `cargo test --workspace` | Workspace test run (debug profile, used by `make test-rust`) |
| `cargo test -p context-graph-mcp` | MCP server tests |
| `cargo test -p context-graph-storage` | Storage tests (CF init, recovery, FSV) |
| `cargo test -p context-graph-core` | Core tests (fingerprints, retrieval, learning) |
| `cargo test -p context-graph-embeddings` | All 14 embedders' unit + roundtrip tests |
| `cargo test -p context-graph-cli` | CLI integration + E2E + BlackBoxFix Phase FSVs |
| `cargo test -p context-graph-cli --test e2e` | E2E only (full session, error recovery) |
| `cargo test -p context-graph-cli --test integration` | Hook lifecycle, exit codes, timeout |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 145 tools (llm) |
| `cargo test -p context-graph-mcp --no-default-features tools::definitions::tests::test_total_tool_count_and_no_duplicates` | Asserts 141 tools (no-llm) |
| `cargo test -p context-graph-mcp tools::definitions::tests::test_submodule_counts` | Asserts per-submodule counts |
| `cargo run --release --bin reality-loop -- --task-manifest <path>.json --official-eval-each-attempt` | End-to-end reality loop (see [22_reality_loop_subsystem.md](22_reality_loop_subsystem.md)) |
| `python3 gemmafrontend/manual_fsv.py --run-root /tmp/blackboxfix-agentops-demo-manual-03` | Manual FSV against the gemmafrontend stack |
| `python3 gemmafrontend/swe_lite_refinement_loop.py --official-eval-each-attempt --max-attempts 3` | Python ContextGraph-guided SWE-bench Lite refinement loop |
| `make test` | Rust + Python smoke |
| `make test-rust` | `cargo test --workspace` |
| `make test-e2e` | `cargo test -p context-graph-cli --test e2e` |
| `make test-mcp` | `cargo test -p context-graph-mcp` |
| `make test-python` / `make test-python-smoke` | Pytest in `clipcannon/` |

## 5. Notable FSV Targets (Rust)

| Target | Crate | Coverage |
| --- | --- | --- |
| `e14_fsv.rs` | storage | E14 BGE-M3 dense end-to-end on RocksDB |
| `e14_optimization_fsv.rs` | cli | E14 fast-path + memory budget |
| `full_14_embedder_fsv.rs` | storage | All 14 embedders end-to-end (E1–E14) |
| `learning_events_fsv.rs` | storage | Learning event readback |
| `utl_learner_state_fsv.rs` | storage | Learner-state CFs (Phase 0/1) |
| `utl_learner_embedders_fsv.rs` | storage | E15–E21 slot listing |
| `e14_phase_b_fsv.rs` | cli | E14 ingest → search → consistency |
| `e14_phase_b_full_state_verification.rs` | cli | Full state diff |
| `blackbox_phase{1..24}_*_fsv.rs` | cli | BlackBoxFix per-phase FSVs (24 phase files: phase1, phase2, phase3, phase4, phase5_export, phase6_mcp_policy, phase7_dashboard, phase8_enterprise, phase9_gitlab_ci, phase10_scanner, phase11_github_issue_evidence, phase12_scorecard, phase13_privacy_retention, phase14_release_gate, phase15_pilot_package, phase16_risk_register, phase17_http_server, phase18_first_wedge_demo, phase19_fsv_protocol, phase20_k8s_deployment, phase21_terraform_cloud, phase22_ticketing, phase23_otel_export, phase24_sdk_ingest) |
| `blackbox_cross_phase_api_surface_hardening_fsv.rs` | cli | Cross-phase API surface guard |
| `blackbox_reward_compiler_fsv.rs` | cli | Reward compiler FSV (cross-references reality-loop reward spec) |
| `dynamicjepa_phase{3,4,5,6,7,8,10,12}_fsv.rs` | cli | DynamicJEPA per-phase FSVs |
| `dynamicjepa_pairwise_mi_phase5_fsv.rs` | cli | Phase-5 pairwise MI audit |
| `dynamicjepa_cross_domain_transfer_phase6_v21_fsv.rs` | cli | Cross-domain transfer |
| `dynamicjepa_research_smoke_fsv.rs` | cli | Research smoke battery |
| `dynamicjepa_bundle_export_fsv.rs` | cli | Episode JSONL export |
| `e2e/full_session_test.rs` | cli | Full session lifecycle |
| `e2e/error_recovery_test.rs` | cli | Recovery from corrupted state |
| `crash_recovery_tests.rs` | storage | Open RocksDB after dirty shutdown |
| `contrastive_integration.rs` | storage | Phase-3 contrastive pair miner |
| `typed_edge_export_integration.rs` | storage | Phase-4 typed-edge export |
| `training_records_integration.rs` | storage | Training record schema |
| `teleological_integration.rs` | storage | 13-D teleological vectors + CFs |
| `constellation_integration.rs` | storage | Constellation compile/score/delete |
| `anomaly_derivation_integration.rs` | storage | Cross-embedder anomalies |
| `e12_e13_source_of_truth_verification.rs` | storage | E12 ColBERT + E13 SPLADE invariants |
| `e9_storage_roundtrip_test.rs` | storage | E9 HDC roundtrip |
| `llm_validation_integration.rs` | storage | Hermes-2-Pro grammar-constrained outputs |
| `graph_linking_integration.rs` | mcp | Graph linker handler |
| `rrf_edge_cases.rs` | core | RRF fusion edge cases (zero-norm, single-hit) |
| `synthesizer_edge_case_test.rs` | core | Context synthesizer edge cases |
| `source_metadata_integration.rs` | core | Provenance / source metadata |
| `file_watcher_integration.rs` | core | Markdown watcher → memory store |
| `global_provider_fsv.rs` | embeddings | Global provider end-to-end |
| `cuda_smoke.rs` | dynamicjepa | CUDA smoke for tiny JEPA stack |
| `native_cubin_smoke.rs` | cuda | Native cubin smoke |

## 6. FSV Python Scripts (`scripts/v4_2/`)

| Script | Coverage |
| --- | --- |
| `bigunlock_fsv_lib.py` | BigUnlock FSV harness library |
| `learner_state_bigunlock_fsv.py` | Learner-state end-to-end |
| `shakespeare_corpus_fsv.py` | Shakespeare corpus ingest + retrieve |
| `shakespeare_state_conditioned_fsv.py` | State-conditioned retrieval |
| `shakespeare_learner_training_fsv.py` | Learner training cycle |
| `shakespeare_learning_event_fsv.py` | Learning event ingest |
| `shakespeare_phase6_policy_benchmark.py` | Phase-6 retrieval policy benchmark |
| `unified_realdata_fsv.py` | Unified real-data FSV (all embedders) |
| `extended_fsv.py` | Extended CAMD sidecar |
| `cuda13_wsl_fsv.py` | CUDA 13 / WSL2 Blackwell smoke |
| `conceptual_capabilities_fsv.py` | Capability matrix readback |

Reality-loop FSV harnesses (out-of-tree, in `gemmafrontend/`):

| Script | Coverage |
| --- | --- |
| `manual_fsv.py` | End-to-end FSV for `server.py`: starts the local server, loads a real run root, reads the SQLite ledger, verifies the persisted snapshot, runs read-only multi-tool turn, builds product category pack, checks every pack artifact hash, verifies signed attestation, proves tamper detection, exercises fail-closed edge cases. |
| `model_benchmark.py` | Multi-model verified-outcome benchmark; reads `blackbox.sqlite` directly for ledger counts. |
| `model_prompt_matrix.py` | Model × prompt grid + 4 edge cases (`run_edge_cases`, line 308) → Markdown summary. |

## 7. Shared Test Fixtures
Source: `crates/context-graph-test-utils/src/`.

| Module | Exports |
| --- | --- |
| `fingerprints.rs` | `create_real_fingerprint`, `create_real_fingerprint_with_id`, `generate_real_content_hash`, `generate_real_semantic_fingerprint`, `generate_real_sparse_vector`, `generate_real_teleological_fingerprint`, `generate_real_unit_vector`, `hex_string` |
| `stores.rs` | `create_test_store`, `create_initialized_store` |
| `lib.rs` | Re-exports the above at crate root |

All fixtures generate **REAL** data (no mocks). Tests using these fixtures assert real RocksDB writes, real HNSW inserts, and real cosine recall.

## 8. Tool-Count Assertions
Source: `crates/context-graph-mcp/src/tools/definitions/mod.rs:131-198`.

| Test | Asserts |
| --- | --- |
| `test_total_tool_count_and_no_duplicates` | `tools.len() == 145` (llm) / `141` (no-llm); no duplicate names; all have description + schema type |
| `test_submodule_counts` | Per-submodule counts: capability=1, core=5, merge=1, curation=2, topic=4, file_watcher=4, sequence=4, causal=4, keyword=1, code=1, robustness=1, entity=6, embedder=7+2 standalone, temporal=2, graph_link=4, maintenance=1, provenance=3, daemon=1, training=4, learning=14, utl=18, constellation=6, contrastive=4, typed_edges=4, dynamicjepa=35, graph=4(llm)/2(no-llm), causal_discovery=2(llm)/0(no-llm) |

## 9. Recently Verified Counts (per CLAUDE memory)

| Suite | Tests passed | Source / Date |
| --- | --- | --- |
| MCP + Storage (unified_neighbors FSV session) | 1,179 | 2026-02-26 |
| Workspace full audit (Audit #15) | 6,527 | 2026-02-26 |
| Workspace full audit (Audit #9) | 6,724 | 2026-02-22 |
| Build status across audits | zero errors, zero warnings | continuous |

These are aggregated counts at audit time and exclude `#[ignore]`-gated tests, feature-gated `llm` tests when the feature is off, and CUDA-only tests when the `cuda` feature is off.

## 10. Test Profiles

| Profile | When | Notes |
| --- | --- | --- |
| `dev` | `cargo test` | `opt-level=0`, `debug=1`, `incremental=true`, `split-debuginfo=unpacked`; external deps `debug=false` (Cargo.toml) |
| `release` | `cargo test --release`, `make test-rust` w/ release flag | `lto="thin"`, `codegen-units=1`, `strip=true` |

`make build-rust` and `make test-rust` always run `scripts/trim-stale-deps.sh` afterwards to bound `target/` size (Makefile header notes ~600 MB per build → 240 GB+ in weeks without trim).

## 11. Coverage
Coverage percentages are not instrumented in the workspace. Counts above are static (file count + grep) plus self-reported test totals from audit sessions logged in `/home/cabdru/.claude/projects/-home-cabdru-contextgraph/memory/MEMORY.md`. See [15_verification_report.md](15_verification_report.md) for the most recent build / lint / metric snapshot.
