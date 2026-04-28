# Test Suite

**Source files covered:**
- `crates/*/tests/**/*.rs`
- `crates/*/src/**/*.rs`
- `Cargo.toml`

## 1. Test Metrics
| Metric | Value |
| --- | --- |
| Integration test files | 76 |
| All Rust files under crates | 1234 |
| Test attributes in all Rust files | 5351 |
| Ignored test attributes | 32 |

## 2. Integration Test Files
| File | Test attributes | Ignored |
| --- | --- | --- |
| crates/context-graph-causal-agent/tests/analyze_single_text_integration.rs | 5 | 0 |
| crates/context-graph-causal-agent/tests/background_loop_integration.rs | 12 | 3 |
| crates/context-graph-cli/tests/e14_optimization_fsv.rs | 10 | 0 |
| crates/context-graph-cli/tests/e14_phase_b_fsv.rs | 9 | 0 |
| crates/context-graph-cli/tests/e14_phase_b_full_state_verification.rs | 12 | 0 |
| crates/context-graph-cli/tests/e2e/error_recovery_test.rs | 7 | 0 |
| crates/context-graph-cli/tests/e2e/full_session_test.rs | 8 | 0 |
| crates/context-graph-cli/tests/e2e/helpers.rs | 5 | 0 |
| crates/context-graph-cli/tests/e2e/mod.rs | 0 | 0 |
| crates/context-graph-cli/tests/export_training_integration.rs | 5 | 0 |
| crates/context-graph-cli/tests/integration/exit_code_test.rs | 7 | 0 |
| crates/context-graph-cli/tests/integration/helpers.rs | 0 | 0 |
| crates/context-graph-cli/tests/integration/hook_lifecycle_test.rs | 4 | 0 |
| crates/context-graph-cli/tests/integration/mod.rs | 0 | 0 |
| crates/context-graph-cli/tests/integration/timeout_test.rs | 4 | 0 |
| crates/context-graph-core/tests/file_watcher_integration.rs | 4 | 0 |
| crates/context-graph-core/tests/injection_manual_test.rs | 9 | 0 |
| crates/context-graph-core/tests/rrf_edge_cases.rs | 5 | 0 |
| crates/context-graph-core/tests/source_metadata_integration.rs | 4 | 0 |
| crates/context-graph-core/tests/synthesizer_edge_case_test.rs | 13 | 0 |
| crates/context-graph-embeddings/tests/attention_strategy_tests.rs | 15 | 0 |
| crates/context-graph-embeddings/tests/benchmark_test.rs | 23 | 0 |
| crates/context-graph-embeddings/tests/bge_m3_tokenizer_probe.rs | 2 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/aggregate_dimensions.rs | 6 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/comprehensive.rs | 1 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/constants.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/edge_cases.rs | 8 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/main.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/model_metadata.rs | 3 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/native_dimensions.rs | 15 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/offsets.rs | 5 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/projected_dimensions.rs | 16 | 0 |
| crates/context-graph-embeddings/tests/dimension_test/quantization_methods.rs | 8 | 0 |
| crates/context-graph-embeddings/tests/e9_typo_tolerance_test.rs | 3 | 0 |
| crates/context-graph-embeddings/tests/e9_vector_differentiation_test.rs | 2 | 2 |
| crates/context-graph-embeddings/tests/embed_dual_with_hint_integration.rs | 8 | 0 |
| crates/context-graph-embeddings/tests/fixtures/create_fixture.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/fixtures/physical_verification_test.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/global_provider_fsv.rs | 10 | 0 |
| crates/context-graph-embeddings/tests/gpu_memory_slots_test.rs | 13 | 0 |
| crates/context-graph-embeddings/tests/no_fake_data_test.rs | 25 | 0 |
| crates/context-graph-embeddings/tests/qodo_embed_integration.rs | 10 | 10 |
| crates/context-graph-embeddings/tests/search_test.rs | 26 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/comprehensive_validation.rs | 1 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/edge_cases.rs | 5 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/fingerprint_creation.rs | 7 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/helpers.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/index_entry.rs | 9 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/main.rs | 0 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/query_result.rs | 4 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/rrf_formula.rs | 6 | 0 |
| crates/context-graph-embeddings/tests/storage_roundtrip_test/serialization_roundtrip.rs | 4 | 0 |
| crates/context-graph-graph-agent/tests/scanner_integration.rs | 10 | 0 |
| crates/context-graph-mcp/tests/graph_linking_integration.rs | 7 | 0 |
| crates/context-graph-storage/tests/anomaly_derivation_integration.rs | 1 | 0 |
| crates/context-graph-storage/tests/constellation_integration.rs | 12 | 0 |
| crates/context-graph-storage/tests/contrastive_integration.rs | 11 | 0 |
| crates/context-graph-storage/tests/crash_recovery_tests.rs | 8 | 0 |
| crates/context-graph-storage/tests/e12_e13_source_of_truth_verification.rs | 6 | 0 |
| crates/context-graph-storage/tests/e14_fsv.rs | 14 | 0 |
| crates/context-graph-storage/tests/e9_storage_roundtrip_test.rs | 1 | 0 |
| crates/context-graph-storage/tests/full_14_embedder_fsv.rs | 4 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/helpers.rs | 0 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/main.rs | 1 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/operations_tests.rs | 2 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/performance_tests.rs | 2 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/pipeline_tests.rs | 3 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/roundtrip_tests.rs | 2 | 0 |
| crates/context-graph-storage/tests/full_integration_real_data/search_tests.rs | 1 | 0 |
| crates/context-graph-storage/tests/learning_events_fsv.rs | 4 | 0 |
| crates/context-graph-storage/tests/llm_validation_integration.rs | 7 | 0 |
| crates/context-graph-storage/tests/teleological_integration.rs | 8 | 0 |
| crates/context-graph-storage/tests/training_records_integration.rs | 15 | 0 |
| crates/context-graph-storage/tests/typed_edge_export_integration.rs | 7 | 0 |
| crates/context-graph-storage/tests/utl_learner_embedders_fsv.rs | 4 | 0 |
| crates/context-graph-storage/tests/utl_learner_state_fsv.rs | 4 | 0 |

## 3. Commands
| Command | Purpose |
| --- | --- |
| cargo test --workspace | Run default workspace tests |
| cargo test -p context-graph-mcp tools::definitions::tests::test_total_tool_count_and_no_duplicates | Verify MCP tool count with default features |
| cargo test -p context-graph-mcp --no-default-features tools::definitions::tests::test_total_tool_count_and_no_duplicates | Verify MCP count without defaults |
| cargo test -p context-graph-storage | Run storage tests |
| cargo test -p context-graph-core | Run core tests |

## 4. Coverage
Coverage percentage is not determined from source.
