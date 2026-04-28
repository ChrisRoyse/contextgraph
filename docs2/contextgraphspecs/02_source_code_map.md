# Source Code Map

**Source files covered:**
All `crates/*/src/**/*.rs` files, `scripts/v4_2/*.py`, workspace manifests, and generated Rust `use` edges.

## 1. Workspace Package Dependencies
| Package | Workspace path dependencies |
| --- | --- |
| context-graph-mcp | context-graph-causal-agent, context-graph-core, context-graph-cuda, context-graph-embeddings, context-graph-graph-agent, context-graph-storage |
| context-graph-causal-agent | context-graph-core, context-graph-embeddings, context-graph-storage |
| context-graph-core | none |
| context-graph-embeddings | context-graph-core, context-graph-cuda |
| context-graph-cuda | context-graph-core |
| context-graph-storage | context-graph-core, context-graph-cuda, context-graph-embeddings, context-graph-test-utils |
| context-graph-test-utils | context-graph-core, context-graph-storage |
| context-graph-graph-agent | context-graph-causal-agent, context-graph-core, context-graph-embeddings, context-graph-storage, context-graph-test-utils |
| context-graph-cli | context-graph-core, context-graph-embeddings, context-graph-mcp, context-graph-storage, context-graph-test-utils |
| context-graph-benchmark | context-graph-causal-agent, context-graph-core, context-graph-embeddings, context-graph-graph-agent, context-graph-mcp, context-graph-storage |

## 2. Full Source File Inventory
Generated from `crates/*/src/**/*.rs`; descriptions are path-derived for token efficiency.

`crates/context-graph-benchmark/src/ablation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/baseline/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/baseline/single_embedder.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/baseline/single_hnsw.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_e2e_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_embedding_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_perf_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_prompt_experiment_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_provenance_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_provenance_e2e_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_provenance_llm_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/causal_span_fix_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/disk_space_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e10_modifier_tuning.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e11_entity_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e7_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/e7_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/embedder_impact_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/embedder_stress.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/gpu_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/graph_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/graph_linking_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/graph_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/hf_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/mcp_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/model_injection_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/model_injection_rigorous_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/multimodal_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/multimodal_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/report.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/sparse_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/stub_fix_verification.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/temporal_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/unified_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/validation_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/bin/weight_profile_bench.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/comparison.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/data_loader.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/metrics.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/phases.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/provider.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/causal_bench/report.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/config.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/beir_loader.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/causal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/e11_entity.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/e1_semantic.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/generator.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/graph_linking.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/ground_truth.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/multimodal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/sparse.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/temporal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/temporal_sessions.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/datasets/topic_clusters.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/lib.rs`  # library root for context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/causal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/clustering.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/divergence.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/e11_entity.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/e1_semantic.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/e4_hybrid_session.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/e7_code.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/e7_iou.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/embedder_contribution.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/graph_linking.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/graph_structure.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/multimodal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/performance.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/resource_usage.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/retrieval.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/sparse.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/temporal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/temporal_realdata.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/metrics/validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/config.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/embedder.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/ground_truth.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/loader.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/report.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/results.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/runner.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/realdata/temporal_injector.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/reports/json.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/reports/markdown.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/reports/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/causal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/comparative.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/e11_entity.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/e1_semantic.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/embedder_impact.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/failfast.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/graph_linking.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/graph_realdata.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/multimodal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/retrieval.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/scaling.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/sparse.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/temporal.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/temporal_realdata.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/topic.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/runners/unified_realdata.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/scaling/corpus_builder.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/scaling/degradation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/scaling/memory_profiler.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/scaling/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/stress_corpus.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/tuning/e7_tuning.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/tuning/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/util.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/ablation_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/arch_compliance.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/asymmetric_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/config_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/e1_foundation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/fusion_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/ground_truth_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/mod.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/results_aggregator.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-benchmark/src/validation/temporal_validation.rs`  # Rust source in context-graph-benchmark
`crates/context-graph-causal-agent/src/activator/mod.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/error.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/lib.rs`  # library root for context-graph-causal-agent
`crates/context-graph-causal-agent/src/llm/mod.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/llm/prompt.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/scanner/mod.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/service/mod.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-causal-agent/src/types/mod.rs`  # Rust source in context-graph-causal-agent
`crates/context-graph-cli/src/commands/backfill_e14.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/divergence.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/export_training.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/args.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/error.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/memory_cache.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/mod.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/pre_compact.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/session_end.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/session_start.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/session_state.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/task_completed.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/types.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/learning.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/memory/capture.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/memory/inject.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/memory/mod.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/mod.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/session/mod.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/session/persist.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/session/restore.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/setup.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/topic.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/utl.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/warmup.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/commands/watch.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/error.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/lib.rs`  # library root for context-graph-cli
`crates/context-graph-cli/src/main.rs`  # binary entry point for context-graph-cli
`crates/context-graph-cli/src/mcp_client.rs`  # Rust source in context-graph-cli
`crates/context-graph-cli/src/mcp_helpers.rs`  # Rust source in context-graph-cli
`crates/context-graph-core/src/causal/asymmetric.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/causal/chain.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/causal/inference.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/causal/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/causal/scm.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/birch.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/cluster.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/hdbscan.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/manager.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/membership.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/persistence.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/stability.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/synthesizer.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/clustering/topic.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/code/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/constants.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/embedder_config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/sub_configs.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/tests/default_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/tests/mcp_config_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/tests/serialization_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/config/tests/validation_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/compiler.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/reservoir.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/constellation/welford.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/contrastive/mining.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/contrastive/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/contrastive/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/contrastive/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/category.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/provider.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/stubs.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/token_pruning.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/embeddings/vector.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/entity/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/conversions.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/legacy.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/sub_errors.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/error/unified.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/fusion/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph/asymmetric.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/direction.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/edge_builder.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/edge_type.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/embedder_edge.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/knn_graph.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/nn_descent.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/service.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/storage_keys.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/thresholds.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/typed_edge.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/graph_linking/weight_projector.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/persistence.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/real_hnsw.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/tests/persistence_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/tests/real_hnsw_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/hnsw_impl/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/manager.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/splade_impl.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/index/status.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/budget.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/candidate.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/formatter.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/pipeline.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/priority.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/injection/temporal_enrichment.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/learner/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/learner_training.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/learning/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/lib.rs`  # library root for context-graph-core
`crates/context-graph-core/src/llm_edge_validation/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/llm_edge_validation/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/ast_chunker.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/capture.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/chunker.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/code_capture.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/code_watcher.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/manager.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/session.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/source.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/store.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/memory/watcher.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/monitoring.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/accuracy.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/batch.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/fingerprint.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/traits.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/quantization/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/aggregation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/detector.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/distance.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/divergence.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/executor.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/in_memory_executor.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/insight_annotation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/multi_space.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/query.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/retriever.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/similarity.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/sparse_index.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/teleological_query.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/teleological_result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/retrieval/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/default_engine.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/primitives.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/simd.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/tests/integration_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/tests/primitives_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/dense/tests/simd_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/engine.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/explanation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/multi_utl.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/sparse.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/similarity/token_level.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/multi_array_stub.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/content.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/similarity.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/batch.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/strategies.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/teleological.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparator/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/comparison_error.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/embedder.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/groups.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/config.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/embedder_names.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/search/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/strategies/helpers.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/strategies/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/tests/advanced.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/matrix_search/weights.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/meaning.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/fusion_strategy.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/metrics.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/task_type.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/teleological.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/profile/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/resolution.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/correlation_extractor.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/fusion_engine.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/group_aggregator.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/multi_space_retriever.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/profile_manager/builtin.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/profile_manager/manager.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/profile_manager/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/profile_manager/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/profile_manager/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/synergy_service.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/services/tucker_decomposer.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/accessors.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/analysis.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/constants.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/constructors.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/presets.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/tests/analysis.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/tests/basic.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/tests/presets.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/tests/validation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/synergy_matrix/validation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/tucker.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/teleological/vector.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/training/computation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/training/edge_signature.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/training/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/training/temporal.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/training/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/multi_array_embedding.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/backend.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/ext.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/traits/teleological_memory_store_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/typed_edge_export/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/typed_edge_export/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/audit.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/causal_relationship.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/code_entity.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/discovery.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/file_index.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/slice.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/tests/core_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/tests/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/tests/storage_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/tests/validation_tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/semantic/validation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/sparse.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/teleological/core.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/teleological/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/teleological/test_helpers.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/teleological/tests.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/fingerprint/teleological/types.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/edge.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/modulation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/tests_constructor.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/tests_modulation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/tests_struct.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/tests_traversal.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/graph_edge/traversal.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/metadata.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/node.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/tests_metadata.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/tests_node.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/tests_serialization.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/memory_node/validation.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/types/source_metadata.rs`  # Rust source in context-graph-core
`crates/context-graph-core/src/weights/mod.rs`  # Rust source in context-graph-core
`crates/context-graph-cuda/src/context/green_contexts.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/context/mod.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/error.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/ffi/cuda_driver.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/ffi/faiss.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/ffi/knn.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/ffi/mod.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/hdbscan/clusterer.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/hdbscan/error.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/hdbscan/gpu_knn.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/hdbscan/mod.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/hdbscan/tests.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/lib.rs`  # library root for context-graph-cuda
`crates/context-graph-cuda/src/ops.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/safe/device.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/safe/mod.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/similarity.rs`  # Rust source in context-graph-cuda
`crates/context-graph-cuda/src/stub.rs`  # Rust source in context-graph-cuda
`crates/context-graph-embeddings/src/adapters/code_provider.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/adapters/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/stats.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/submit.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/processor/worker.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/queue.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/request.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/stats.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/batch/types/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/bin/train_causal.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/cache/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/config/batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/config/cache.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/config/gpu.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/config/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/config/models.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/error/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/error/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/error/tests_extended.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/error/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/global_provider.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/device/accessors.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/device/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/device/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/device/utils.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/budget.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/error.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/pool.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/pressure.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/slots.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/stats.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/memory/tracker.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/batch_loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/embedding_loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/error.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/layer_loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/tensor_utils.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/model_loader/weights.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/ops.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/tensor/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/tensor/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/gpu/tensor/ops.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/learner/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/lib.rs`  # library root for context-graph-embeddings
`crates/context-graph-embeddings/src/models/attention/dense.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/attention/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/attention/sliding_window.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/attention/tiled.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/construction.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/embedding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/hypervector.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/tests/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/hdc/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/embed_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/construction.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/embedding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/periodicity.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/thread_safety.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/timestamp.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/session_signature.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/construction.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/embedding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/hybrid.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/uniqueness.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/verification.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_positional/timestamp.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/timestamp.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/default_factory.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/tests/factory_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/tests/memory_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/memory_tracker/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/memory_tracker/tests/advanced.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/memory_tracker/tests/basic.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/memory_tracker/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/embeddings.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/ffn.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/layer_norm.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/pooling.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/trait_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/ops.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/causal/weights.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/position.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/tests_batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/tests_edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/code/weights.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/contextual/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/contextual/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/contextual/state.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/ffn.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/layer_norm.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/projections.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/state.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/graph/tests_edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/encoding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/pooling.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/trait_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/transe.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention_ops.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_projection.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_utils.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/scoring.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_extended.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_token.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/rgcn/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/rgcn/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/rgcn/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/embeddings.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/ffn.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/layer_norm.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/pooling.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/basic.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/edge_cases.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/embedding.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/verification.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/trait_impl.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/semantic/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/shared.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/attention.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/embeddings.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/mlm_head.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/error.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_batch.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/traits.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/sparse/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/loader.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/stats.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/tests_concurrent.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/tests_stats.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/models/registry/tests_unload.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/provider/causal_hint.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/provider/diagnostic_test.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/provider/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/provider/multi_array.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/pruning/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/pruning/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/pruning/token_pruner.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/binary.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/edge_case_verification.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/float8.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/encoder.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/persistence.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/tests/dimension_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/tests/encoder_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/tests/persistence_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/tests/training_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/training.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/pq8/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/encoders.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/binary_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/edge_case_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/error_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/float8_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/pq8_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/router/tests/routing_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/quantization/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/full_state_verification.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/multi_space.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/fingerprint.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/index_entry.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/query_results.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/storage/types/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/data.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/distillation.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/evaluation.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/lora.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/loss.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/multitask.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/optimizer.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/pipeline.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/training/trainer.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/tests/core_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/tests/integration_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/tests/test_model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/device.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/memory.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/quantization.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/concatenated/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/concatenated/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/concatenated/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/concatenated/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/dimensions/aggregates.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/dimensions/arrays.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/dimensions/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/dimensions/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/dimensions/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/tensor.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/embedding/validation.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/accessors.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/image_format.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/input_type.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/model_input.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/audio_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/code_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/hash_and_size_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/image_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/input_type_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/serde_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/text_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/input/tests/type_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/conversions.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/display.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/repository.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/types/model_id/tokenizer.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocation.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/gpu_info.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/cuda_alloc/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/output.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/diagnostics/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/error.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/handle.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/check.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/checker.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/status.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/inference/engine.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/inference/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/inference/validation.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/integration/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/integration/tests/advanced.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/integration/tests/basic.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/constants.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/engine.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/preflight.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/summary.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/inference.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/metadata.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/vram.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/loader/types/weights.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/model_pool.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/memory_pool/working_pool.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/core.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/operations.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/queries.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/tests_errors.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/tests_queries.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/tests_transitions.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/registry/types.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/state.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/config_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/diagnostics_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/error_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/handle_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/health_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/helpers.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/loader_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/memory_pool_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/registry_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/state_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/tests/validation_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/comparisons.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/config.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/mod.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/result.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/tests/config_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/tests/model_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/tests/result_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/tests/validator_tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/tests.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/validator.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs`  # Rust source in context-graph-embeddings
`crates/context-graph-graph-agent/src/activator/mod.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/error.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/lib.rs`  # library root for context-graph-graph-agent
`crates/context-graph-graph-agent/src/llm/mod.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/llm/prompt.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/scanner/mod.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/service/mod.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/stubs.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-graph-agent/src/types/mod.rs`  # Rust source in context-graph-graph-agent
`crates/context-graph-mcp/src/adapters/causal_hint.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/adapters/lazy_provider.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/adapters/mod.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/handlers/core/dispatch.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/core/handlers.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/core/mod.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/merge.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/mod.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/error_codes.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/initialize.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/mcp_protocol_e2e_test.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/mod.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/search_periodic_test.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/tcp_transport_integration.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/tools_call.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/tools_list.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tests/topic_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/causal_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/code_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/daemon_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/dispatch.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/entity_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/entity_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/file_watcher_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/graph_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/graph_link_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/graph_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/helpers.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/keyword_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/keyword_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/learning_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/maintenance_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/mod.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/provenance_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/provenance_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/robustness_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/robustness_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/sequence_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/status_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/temporal_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/temporal_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/topic_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/training_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/utl_tools.rs`  # MCP handler module
`crates/context-graph-mcp/src/handlers/tools/validate.rs`  # MCP handler module
`crates/context-graph-mcp/src/lib.rs`  # library root for context-graph-mcp
`crates/context-graph-mcp/src/main.rs`  # binary entry point for context-graph-mcp
`crates/context-graph-mcp/src/monitoring.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/protocol.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/server/mod.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/server/transport.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/server/watchers.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/tools/aliases.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/tools/definitions/causal.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/causal_discovery.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/code.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/constellation.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/contrastive.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/core.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/curation.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/daemon.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/embedder.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/entity.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/file_watcher.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/graph.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/graph_link.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/keyword.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/learning.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/maintenance.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/merge.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/mod.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/provenance.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/robustness.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/sequence.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/temporal.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/topic.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/training.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/definitions/utl.rs`  # MCP tool definition module
`crates/context-graph-mcp/src/tools/mod.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/tools/names.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/tools/types.rs`  # Rust source in context-graph-mcp
`crates/context-graph-mcp/src/weights.rs`  # Rust source in context-graph-mcp
`crates/context-graph-storage/src/code/error.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/code/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/code/store.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/column_families.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/error.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/graph_edges/builder.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/graph_edges/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/graph_edges/repository.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/graph_edges/serialization.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/graph_edges/types.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/indexes.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/lib.rs`  # library root for context-graph-storage
`crates/context-graph-storage/src/teleological/column_families.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/embedder_index.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/config.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/constants.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/distance.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/embedder.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/functions.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_config/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_impl/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_impl/ops.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_impl/tests.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/hnsw_impl/types.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/metrics.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/indexes/registry.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/error.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/helpers.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/rocksdb_impl.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/tests.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/quantized/trait_def.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/audit_log.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/causal_hnsw_index.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/causal_relationships.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/content.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/file_index.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/fusion.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/helpers.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/index_ops.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/inverted_index.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/learner.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/learner_training.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/learning.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/persistence.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/provenance_storage.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/search.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/source_metadata.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/store.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/tests.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/trait_impl.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_keys.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/types.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/schema.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/error.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/analysis.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/builder.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/results.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/search_matrix.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/strategy_search.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/tests_boundary.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/matrix/tests_unit.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/maxsim.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/multi/builder.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/multi/executor.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/multi/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/multi/tests.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/multi/types.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/builder.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/execution.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/stages.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/tests.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/traits.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/pipeline/types.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/result.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/config.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/search.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/tests/integration.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/tests/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/tests/search.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/single/tests/validation.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/temporal_boost.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/search/token_storage.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/serialization.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/column_family.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/content.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/helpers.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/key_format.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/mod.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/panic.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/serialization.rs`  # Rust source in context-graph-storage
`crates/context-graph-storage/src/teleological/tests/stale_lock.rs`  # Rust source in context-graph-storage
`crates/context-graph-test-utils/src/fingerprints.rs`  # Rust source in context-graph-test-utils
`crates/context-graph-test-utils/src/lib.rs`  # library root for context-graph-test-utils
`crates/context-graph-test-utils/src/stores.rs`  # Rust source in context-graph-test-utils

## 3. CAMD Python Files
`scripts/v4_2/__init__.py`  # Python script
`scripts/v4_2/analyze_correlations.py`  # Python script
`scripts/v4_2/backfill_extended.py`  # Python script
`scripts/v4_2/camd_lib.py`  # Python script
`scripts/v4_2/extended_camd.py`  # Python script
`scripts/v4_2/extended_constellation.py`  # Python script
`scripts/v4_2/extended_ensemble.py`  # Python script
`scripts/v4_2/extended_fsv.py`  # Python script
`scripts/v4_2/extended_store.py`  # Python script
`scripts/v4_2/mine_camd_from_lmdb.py`  # Python script
`scripts/v4_2/mine_camd_pairs.py`  # Python script
`scripts/v4_2/mine_camd_pairs_extended.py`  # Python script
`scripts/v4_2/smoke_dpo_v4_2.py`  # Python script
`scripts/v4_2/train_camd_dpo.py`  # Python script
`scripts/v4_2/validate_camd.py`  # Python script

## 4. Dependency Edges
`crates/context-graph-benchmark/src/ablation.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/ablation.rs` -> `super::*`
`crates/context-graph-benchmark/src/baseline/single_embedder.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/baseline/single_embedder.rs` -> `crate::util::cosine_similarity as compute_cosine_similarity`
`crates/context-graph-benchmark/src/baseline/single_embedder.rs` -> `super::*`
`crates/context-graph-benchmark/src/baseline/single_embedder.rs` -> `super::single_hnsw::SingleBruteForceIndex`
`crates/context-graph-benchmark/src/baseline/single_hnsw.rs` -> `crate::util::`
`crates/context-graph-benchmark/src/baseline/single_hnsw.rs` -> `super::*`
`crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs` -> `context_graph_benchmark::metrics::causal::`
`crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_causal_agent::llm::CausalDiscoveryLLM`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_core::monitoring::`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_graph_agent::GraphDiscoveryService`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_mcp::handlers::Handlers`
`crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs` -> `context_graph_mcp::protocol::`
`crates/context-graph-benchmark/src/bin/causal_e2e_bench.rs` -> `context_graph_core::causal::CausalDirection`
`crates/context-graph-benchmark/src/bin/causal_e2e_bench.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/bin/causal_embedding_bench.rs` -> `context_graph_benchmark::causal_bench::`
`crates/context-graph-benchmark/src/bin/causal_embedding_bench.rs` -> `context_graph_benchmark::causal_bench::provider::EmbeddingProvider`
`crates/context-graph-benchmark/src/bin/causal_perf_bench.rs` -> `context_graph_causal_agent::CausalDiscoveryLLM`
`crates/context-graph-benchmark/src/bin/causal_perf_bench.rs` -> `context_graph_core::causal::CausalDirection`
`crates/context-graph-benchmark/src/bin/causal_perf_bench.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/bin/causal_perf_bench.rs` -> `context_graph_embeddings::models::pretrained::CausalModel`
`crates/context-graph-benchmark/src/bin/causal_prompt_experiment_bench.rs` -> `context_graph_causal_agent::llm::`
`crates/context-graph-benchmark/src/bin/causal_provenance_bench.rs` -> `context_graph_core::types::causal_relationship::CausalSourceSpan`
`crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs` -> `context_graph_benchmark::realdata::embedder::`
`crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs` -> `context_graph_benchmark::realdata::loader::`
`crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/bin/causal_span_fix_bench.rs` -> `context_graph_causal_agent::llm::`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_benchmark::metrics::e7_code::`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_core::memory::CodeEmbeddingProvider`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_core::types::`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_embeddings::adapters::E7CodeEmbeddingProvider`
`crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-benchmark/src/bin/disk_space_bench.rs` -> `super::*`
`crates/context-graph-benchmark/src/bin/e11_entity_bench.rs` -> `context_graph_benchmark::metrics::e11_entity::thresholds`
`crates/context-graph-benchmark/src/bin/e11_entity_bench.rs` -> `context_graph_benchmark::runners::e11_entity::`
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs` -> `context_graph_benchmark::datasets::e1_semantic::`
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs` -> `context_graph_benchmark::datasets::e1_semantic::SemanticDomain`
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs` -> `context_graph_benchmark::realdata::`
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs` -> `context_graph_benchmark::runners::e1_semantic::`
`crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` -> `context_graph_core::traits::`
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` -> `context_graph_embeddings::models::TemporalRecentModel`
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` -> `context_graph_embeddings::traits::EmbeddingModel`
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` -> `context_graph_embeddings::types::ModelInput`
`crates/context-graph-benchmark/src/bin/e2_manual_verification.rs` -> `context_graph_storage::teleological::search::temporal_boost`
`crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs` -> `context_graph_benchmark::datasets::temporal_sessions::`
`crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs` -> `context_graph_benchmark::realdata::`
`crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs` -> `context_graph_benchmark::runners::e4_hybrid_session::`
`crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/e7_bench.rs` -> `context_graph_benchmark::metrics::e7_iou::tokenize_code`
`crates/context-graph-benchmark/src/bin/e7_realdata_bench.rs` -> `context_graph_benchmark::metrics::e7_code::`
`crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs` -> `context_graph_benchmark::datasets::graph_linking::ScaleTier`
`crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs` -> `context_graph_benchmark::metrics::causal::E5ImpactAnalysis`
`crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs` -> `context_graph_benchmark::runners::embedder_impact::`
`crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/bin/embedder_impact_realdata_bench.rs` -> `context_graph_benchmark::datasets::beir_loader::`
`crates/context-graph-benchmark/src/bin/embedder_impact_realdata_bench.rs` -> `context_graph_benchmark::metrics::retrieval::`
`crates/context-graph-benchmark/src/bin/embedder_stress.rs` -> `context_graph_benchmark::stress_corpus::`
`crates/context-graph-benchmark/src/bin/embedder_stress.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/embedder_stress.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/bin/gpu_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/graph_bench.rs` -> `context_graph_core::graph::asymmetric::`
`crates/context-graph-benchmark/src/bin/graph_linking_bench.rs` -> `context_graph_benchmark::datasets::graph_linking::`
`crates/context-graph-benchmark/src/bin/graph_linking_bench.rs` -> `context_graph_benchmark::runners::graph_linking::`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_causal_agent::activator::ActivatorConfig`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_causal_agent::scanner::ScannerConfig`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs` -> `context_graph_benchmark::realdata::loader::`
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs` -> `context_graph_storage::graph_edges::EdgeRepository`
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs` -> `super::*`
`crates/context-graph-benchmark/src/bin/graph_realdata_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/graph_realdata_bench.rs` -> `context_graph_benchmark::runners::graph_realdata::`
`crates/context-graph-benchmark/src/bin/hf_bench.rs` -> `context_graph_benchmark::realdata::embedder::`
`crates/context-graph-benchmark/src/bin/hf_bench.rs` -> `context_graph_benchmark::realdata::loader::`
`crates/context-graph-benchmark/src/bin/hf_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/mcp_bench.rs` -> `context_graph_core::monitoring::`
`crates/context-graph-benchmark/src/bin/mcp_bench.rs` -> `context_graph_core::traits::`
`crates/context-graph-benchmark/src/bin/mcp_bench.rs` -> `context_graph_mcp::handlers::Handlers`
`crates/context-graph-benchmark/src/bin/mcp_bench.rs` -> `context_graph_mcp::protocol::`
`crates/context-graph-benchmark/src/bin/mcp_bench.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-benchmark/src/bin/model_injection_bench.rs` -> `context_graph_causal_agent::llm::`
`crates/context-graph-benchmark/src/bin/model_injection_rigorous_bench.rs` -> `context_graph_causal_agent::llm::`
`crates/context-graph-benchmark/src/bin/multimodal_bench.rs` -> `context_graph_benchmark::datasets::multimodal::E10MultimodalDatasetConfig`
`crates/context-graph-benchmark/src/bin/multimodal_bench.rs` -> `context_graph_benchmark::runners::multimodal::`
`crates/context-graph-benchmark/src/bin/multimodal_realdata_bench.rs` -> `context_graph_benchmark::realdata::embedder::`
`crates/context-graph-benchmark/src/bin/multimodal_realdata_bench.rs` -> `context_graph_benchmark::realdata::embedder::ChunkInfo`
`crates/context-graph-benchmark/src/bin/multimodal_realdata_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/realdata_bench.rs` -> `context_graph_benchmark::realdata::`
`crates/context-graph-benchmark/src/bin/report.rs` -> `context_graph_benchmark::config::`
`crates/context-graph-benchmark/src/bin/report.rs` -> `context_graph_benchmark::reports::`
`crates/context-graph-benchmark/src/bin/report.rs` -> `context_graph_benchmark::runners::BenchmarkHarness`
`crates/context-graph-benchmark/src/bin/sparse_bench.rs` -> `context_graph_benchmark::datasets::sparse::`
`crates/context-graph-benchmark/src/bin/sparse_bench.rs` -> `context_graph_benchmark::realdata::RealDataEmbedder`
`crates/context-graph-benchmark/src/bin/sparse_bench.rs` -> `context_graph_benchmark::realdata::embedder::EmbeddedDataset`
`crates/context-graph-benchmark/src/bin/sparse_bench.rs` -> `context_graph_benchmark::runners::sparse::`
`crates/context-graph-benchmark/src/bin/sparse_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs` -> `context_graph_benchmark::realdata::embedder::`
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs` -> `context_graph_core::traits::`
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs` -> `context_graph_core::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-benchmark/src/bin/temporal_bench.rs` -> `context_graph_benchmark::datasets::temporal::`
`crates/context-graph-benchmark/src/bin/temporal_bench.rs` -> `context_graph_benchmark::runners::temporal::`
`crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs` -> `context_graph_benchmark::datasets::`
`crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs` -> `context_graph_benchmark::realdata::embedder::RealDataEmbedder`
`crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/bin/unified_realdata_bench.rs` -> `context_graph_benchmark::realdata::config::`
`crates/context-graph-benchmark/src/bin/unified_realdata_bench.rs` -> `context_graph_benchmark::realdata::report::`
`crates/context-graph-benchmark/src/bin/unified_realdata_bench.rs` -> `context_graph_benchmark::runners::unified_realdata::UnifiedRealdataBenchmarkRunner`
`crates/context-graph-benchmark/src/bin/validation_bench.rs` -> `context_graph_core::monitoring::`
`crates/context-graph-benchmark/src/bin/validation_bench.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-benchmark/src/bin/validation_bench.rs` -> `context_graph_mcp::handlers::Handlers`
`crates/context-graph-benchmark/src/bin/validation_bench.rs` -> `context_graph_mcp::protocol::`
`crates/context-graph-benchmark/src/bin/validation_bench.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::realdata::config::TemporalInjectionConfig`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::realdata::ground_truth::GroundTruthGenerator`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::realdata::temporal_injector::TemporalMetadataInjector`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::validation::`
`crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs` -> `context_graph_benchmark::validation::ablation_validation::AblationValidator`
`crates/context-graph-benchmark/src/bin/weight_profile_bench.rs` -> `context_graph_core::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-benchmark/src/bin/weight_profile_bench.rs` -> `context_graph_core::weights::`
`crates/context-graph-benchmark/src/causal_bench/comparison.rs` -> `crate::causal_bench::metrics::PhaseBenchmarkResult`
`crates/context-graph-benchmark/src/causal_bench/comparison.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/comparison.rs` -> `super::metrics::`
`crates/context-graph-benchmark/src/causal_bench/data_loader.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/metrics.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `crate::causal_bench::data_loader::BenchmarkPair`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `super::data_loader::`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `super::metrics::`
`crates/context-graph-benchmark/src/causal_bench/phases.rs` -> `super::provider::EmbeddingProvider`
`crates/context-graph-benchmark/src/causal_bench/provider.rs` -> `context_graph_embeddings::models::pretrained::`
`crates/context-graph-benchmark/src/causal_bench/provider.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-benchmark/src/causal_bench/provider.rs` -> `context_graph_embeddings::types::ModelInput`
`crates/context-graph-benchmark/src/causal_bench/provider.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/report.rs` -> `super::*`
`crates/context-graph-benchmark/src/causal_bench/report.rs` -> `super::metrics::`
`crates/context-graph-benchmark/src/config.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/beir_loader.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/causal.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/datasets/causal.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/e11_entity.rs` -> `context_graph_core::entity::`
`crates/context-graph-benchmark/src/datasets/e11_entity.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/e1_semantic.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `super::graph_linking::ScaleTier`
`crates/context-graph-benchmark/src/datasets/embedder_impact.rs` -> `super::topic_clusters::`
`crates/context-graph-benchmark/src/datasets/generator.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-benchmark/src/datasets/generator.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/datasets/generator.rs` -> `crate::config::TierConfig`
`crates/context-graph-benchmark/src/datasets/generator.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/generator.rs` -> `super::topic_clusters::`
`crates/context-graph-benchmark/src/datasets/graph_linking.rs` -> `context_graph_core::graph_linking::GraphLinkEdgeType`
`crates/context-graph-benchmark/src/datasets/graph_linking.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-benchmark/src/datasets/graph_linking.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/graph_linking.rs` -> `super::topic_clusters::`
`crates/context-graph-benchmark/src/datasets/ground_truth.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/mod.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/datasets/mod.rs` -> `crate::config::TierConfig`
`crates/context-graph-benchmark/src/datasets/multimodal.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/sparse.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/temporal.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/temporal_sessions.rs` -> `crate::realdata::loader::`
`crates/context-graph-benchmark/src/datasets/temporal_sessions.rs` -> `crate::realdata::loader::RealDataset`
`crates/context-graph-benchmark/src/datasets/temporal_sessions.rs` -> `super::*`
`crates/context-graph-benchmark/src/datasets/topic_clusters.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-benchmark/src/datasets/topic_clusters.rs` -> `crate::util::cosine_similarity`
`crates/context-graph-benchmark/src/datasets/topic_clusters.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/causal.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-benchmark/src/metrics/causal.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/clustering.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/divergence.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/e11_entity.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/e1_semantic.rs` -> `crate::metrics::retrieval::RetrievalMetrics`
`crates/context-graph-benchmark/src/metrics/e1_semantic.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/e4_hybrid_session.rs` -> `crate::datasets::temporal_sessions::SessionChunk`
`crates/context-graph-benchmark/src/metrics/e4_hybrid_session.rs` -> `crate::datasets::temporal_sessions::TemporalSession`
`crates/context-graph-benchmark/src/metrics/e4_hybrid_session.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/e7_code.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/e7_iou.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/embedder_contribution.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/metrics/embedder_contribution.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/graph_linking.rs` -> `crate::datasets::graph_linking::ScaleTier`
`crates/context-graph-benchmark/src/metrics/graph_linking.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/graph_structure.rs` -> `context_graph_core::graph_linking::GraphLinkEdgeType`
`crates/context-graph-benchmark/src/metrics/graph_structure.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/metrics/graph_structure.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/multimodal.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/performance.rs` -> `crate::config::targets`
`crates/context-graph-benchmark/src/metrics/performance.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/resource_usage.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/metrics/resource_usage.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/retrieval.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/sparse.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/temporal.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/temporal_realdata.rs` -> `crate::datasets::`
`crates/context-graph-benchmark/src/metrics/temporal_realdata.rs` -> `super::*`
`crates/context-graph-benchmark/src/metrics/validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/config.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/embedder.rs` -> `context_graph_core::traits::EmbeddingMetadata`
`crates/context-graph-benchmark/src/realdata/embedder.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/realdata/embedder.rs` -> `super::loader::RealDataset`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `crate::realdata::config::TemporalInjectionConfig`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `crate::realdata::temporal_injector::TemporalMetadataInjector`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `super::config::EmbedderName`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `super::loader::`
`crates/context-graph-benchmark/src/realdata/ground_truth.rs` -> `super::temporal_injector::`
`crates/context-graph-benchmark/src/realdata/loader.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/report.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/report.rs` -> `super::config::EmbedderName`
`crates/context-graph-benchmark/src/realdata/report.rs` -> `super::results::UnifiedBenchmarkResults`
`crates/context-graph-benchmark/src/realdata/report.rs` -> `super::super::config::UnifiedBenchmarkConfig`
`crates/context-graph-benchmark/src/realdata/report.rs` -> `super::super::results::*`
`crates/context-graph-benchmark/src/realdata/results.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/results.rs` -> `super::config::`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `crate::baseline::SingleEmbedderBaseline`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `crate::metrics::`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `crate::metrics::clustering::ClusterSizeStats`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/runner.rs` -> `super::embedder::EmbeddedDataset`
`crates/context-graph-benchmark/src/realdata/temporal_injector.rs` -> `super::*`
`crates/context-graph-benchmark/src/realdata/temporal_injector.rs` -> `super::config::TemporalInjectionConfig`
`crates/context-graph-benchmark/src/realdata/temporal_injector.rs` -> `super::loader::`
`crates/context-graph-benchmark/src/reports/json.rs` -> `crate::config::Tier`
`crates/context-graph-benchmark/src/reports/json.rs` -> `crate::runners::BenchmarkResults`
`crates/context-graph-benchmark/src/reports/json.rs` -> `super::BenchmarkSummary`
`crates/context-graph-benchmark/src/reports/markdown.rs` -> `crate::runners::BenchmarkResults`
`crates/context-graph-benchmark/src/reports/mod.rs` -> `crate::runners::BenchmarkResults`
`crates/context-graph-benchmark/src/runners/causal.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/runners/causal.rs` -> `crate::datasets::causal::`
`crates/context-graph-benchmark/src/runners/causal.rs` -> `crate::metrics::causal::`
`crates/context-graph-benchmark/src/runners/causal.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/comparative.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/runners/comparative.rs` -> `crate::metrics::ScalingMetrics`
`crates/context-graph-benchmark/src/runners/comparative.rs` -> `crate::scaling::DegradationComparison`
`crates/context-graph-benchmark/src/runners/comparative.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/comparative.rs` -> `super::scaling::`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_core::entity::EntityMetadata`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_core::entity::entity_jaccard_similarity`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_core::similarity::cosine_similarity`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_core::traits::`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/e11_entity.rs` -> `context_graph_embeddings::models::KeplerModel`
`crates/context-graph-benchmark/src/runners/e1_semantic.rs` -> `crate::datasets::e1_semantic::`
`crates/context-graph-benchmark/src/runners/e1_semantic.rs` -> `crate::metrics::e1_semantic::`
`crates/context-graph-benchmark/src/runners/e1_semantic.rs` -> `crate::metrics::e1_semantic::TopicSeparationMetrics`
`crates/context-graph-benchmark/src/runners/e1_semantic.rs` -> `crate::metrics::retrieval::`
`crates/context-graph-benchmark/src/runners/e1_semantic.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs` -> `crate::datasets::temporal_sessions::`
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs` -> `crate::datasets::temporal_sessions::SessionChunk`
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs` -> `crate::metrics::e4_hybrid_session::`
`crates/context-graph-benchmark/src/runners/e4_hybrid_session.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `context_graph_core::fusion::`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `crate::ablation::`
`crates/context-graph-benchmark/src/runners/embedder_impact.rs` -> `crate::datasets::embedder_impact::`
`crates/context-graph-benchmark/src/runners/failfast.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/graph_linking.rs` -> `crate::datasets::graph_linking::`
`crates/context-graph-benchmark/src/runners/graph_linking.rs` -> `crate::metrics::graph_linking::`
`crates/context-graph-benchmark/src/runners/graph_linking.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `context_graph_core::types::fingerprint::EmbeddingSlice`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `crate::realdata::config::EmbedderName`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `crate::realdata::loader::`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `crate::realdata::loader::DatasetLoader`
`crates/context-graph-benchmark/src/runners/graph_realdata.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/multimodal.rs` -> `crate::datasets::multimodal::`
`crates/context-graph-benchmark/src/runners/multimodal.rs` -> `crate::metrics::multimodal::`
`crates/context-graph-benchmark/src/runners/multimodal.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::baseline::SingleEmbedderBaseline`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::datasets::BenchmarkDataset`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::datasets::DatasetGenerator`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::metrics::performance::`
`crates/context-graph-benchmark/src/runners/retrieval.rs` -> `crate::metrics::retrieval::`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `crate::baseline::SingleEmbedderBaseline`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `crate::datasets::`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `crate::metrics::ScalingMetrics`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `crate::scaling::`
`crates/context-graph-benchmark/src/runners/scaling.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/sparse.rs` -> `crate::datasets::sparse::`
`crates/context-graph-benchmark/src/runners/sparse.rs` -> `crate::metrics::sparse::`
`crates/context-graph-benchmark/src/runners/sparse.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/temporal.rs` -> `crate::datasets::temporal::`
`crates/context-graph-benchmark/src/runners/temporal.rs` -> `crate::metrics::temporal::`
`crates/context-graph-benchmark/src/runners/temporal.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/temporal_realdata.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/temporal_realdata.rs` -> `crate::datasets::`
`crates/context-graph-benchmark/src/runners/temporal_realdata.rs` -> `crate::metrics::temporal_realdata::`
`crates/context-graph-benchmark/src/runners/temporal_realdata.rs` -> `super::*`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::baseline::SingleEmbedderBaseline`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::datasets::BenchmarkDataset`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::datasets::DatasetGenerator`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::metrics::clustering::`
`crates/context-graph-benchmark/src/runners/topic.rs` -> `crate::metrics::performance::`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `context_graph_core::types::fingerprint::EmbeddingSlice`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `crate::realdata::config::`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `crate::realdata::embedder::`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `crate::realdata::ground_truth::`
`crates/context-graph-benchmark/src/runners/unified_realdata.rs` -> `crate::realdata::loader::`
`crates/context-graph-benchmark/src/scaling/corpus_builder.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-benchmark/src/scaling/corpus_builder.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/scaling/corpus_builder.rs` -> `crate::datasets::`
`crates/context-graph-benchmark/src/scaling/corpus_builder.rs` -> `super::*`
`crates/context-graph-benchmark/src/scaling/degradation.rs` -> `crate::config::`
`crates/context-graph-benchmark/src/scaling/degradation.rs` -> `crate::metrics::`
`crates/context-graph-benchmark/src/scaling/degradation.rs` -> `crate::metrics::ScalingMetrics`
`crates/context-graph-benchmark/src/scaling/degradation.rs` -> `super::*`
`crates/context-graph-benchmark/src/scaling/memory_profiler.rs` -> `crate::config::Tier`
`crates/context-graph-benchmark/src/scaling/memory_profiler.rs` -> `super::*`
`crates/context-graph-benchmark/src/stress_corpus.rs` -> `context_graph_storage::teleological::indexes::EmbedderIndex`
`crates/context-graph-benchmark/src/stress_corpus.rs` -> `super::*`
`crates/context-graph-benchmark/src/tuning/e7_tuning.rs` -> `context_graph_benchmark::tuning::e7_tuning::`
`crates/context-graph-benchmark/src/tuning/e7_tuning.rs` -> `crate::metrics::e7_code::`
`crates/context-graph-benchmark/src/tuning/e7_tuning.rs` -> `super::*`
`crates/context-graph-benchmark/src/tuning/mod.rs` -> `context_graph_benchmark::tuning::`
`crates/context-graph-benchmark/src/util.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/ablation_validation.rs` -> `crate::realdata::config::EmbedderName`
`crates/context-graph-benchmark/src/validation/ablation_validation.rs` -> `crate::realdata::results::AblationResults`
`crates/context-graph-benchmark/src/validation/ablation_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/ablation_validation.rs` -> `super::results_aggregator::AggregatedResults`
`crates/context-graph-benchmark/src/validation/ablation_validation.rs` -> `super::super::results_aggregator::`
`crates/context-graph-benchmark/src/validation/arch_compliance.rs` -> `crate::realdata::config::`
`crates/context-graph-benchmark/src/validation/arch_compliance.rs` -> `crate::realdata::results::ConstitutionalCompliance`
`crates/context-graph-benchmark/src/validation/arch_compliance.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/asymmetric_validation.rs` -> `crate::realdata::config::EmbedderName`
`crates/context-graph-benchmark/src/validation/asymmetric_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/config_validation.rs` -> `crate::realdata::config::`
`crates/context-graph-benchmark/src/validation/config_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/e1_foundation.rs` -> `crate::realdata::config::`
`crates/context-graph-benchmark/src/validation/e1_foundation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/fusion_validation.rs` -> `crate::realdata::config::`
`crates/context-graph-benchmark/src/validation/fusion_validation.rs` -> `crate::realdata::results::FusionResults`
`crates/context-graph-benchmark/src/validation/fusion_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/ground_truth_validation.rs` -> `crate::realdata::config::EmbedderName`
`crates/context-graph-benchmark/src/validation/ground_truth_validation.rs` -> `crate::realdata::ground_truth::`
`crates/context-graph-benchmark/src/validation/ground_truth_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/mod.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/results_aggregator.rs` -> `crate::realdata::config::EmbedderName`
`crates/context-graph-benchmark/src/validation/results_aggregator.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/temporal_validation.rs` -> `crate::realdata::config::TemporalInjectionConfig`
`crates/context-graph-benchmark/src/validation/temporal_validation.rs` -> `crate::realdata::temporal_injector::InjectedTemporalMetadata`
`crates/context-graph-benchmark/src/validation/temporal_validation.rs` -> `super::*`
`crates/context-graph-benchmark/src/validation/temporal_validation.rs` -> `super::results_aggregator::TemporalMetrics as MeasuredTemporalMetrics`
`crates/context-graph-causal-agent/src/activator/mod.rs` -> `context_graph_core::causal::`
`crates/context-graph-causal-agent/src/activator/mod.rs` -> `context_graph_embeddings::models::CausalModel`
`crates/context-graph-causal-agent/src/activator/mod.rs` -> `crate::error::`
`crates/context-graph-causal-agent/src/activator/mod.rs` -> `crate::types::`
`crates/context-graph-causal-agent/src/activator/mod.rs` -> `super::*`
`crates/context-graph-causal-agent/src/lib.rs` -> `context_graph_causal_agent::types::MemoryForAnalysis`
`crates/context-graph-causal-agent/src/lib.rs` -> `context_graph_core::types::`
`crates/context-graph-causal-agent/src/llm/mod.rs` -> `crate::error::`
`crates/context-graph-causal-agent/src/llm/mod.rs` -> `crate::types::`
`crates/context-graph-causal-agent/src/llm/mod.rs` -> `super::*`
`crates/context-graph-causal-agent/src/llm/prompt.rs` -> `super::*`
`crates/context-graph-causal-agent/src/scanner/mod.rs` -> `crate::error::`
`crates/context-graph-causal-agent/src/scanner/mod.rs` -> `crate::types::`
`crates/context-graph-causal-agent/src/scanner/mod.rs` -> `super::*`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::causal::CausalGraph`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::types::CausalRelationship`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::types::DiscoveryCycleResult`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::types::ServiceStatus`
`crates/context-graph-causal-agent/src/service/mod.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-causal-agent/src/types/mod.rs` -> `context_graph_core::traits::`
`crates/context-graph-causal-agent/src/types/mod.rs` -> `super::*`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_embeddings::models::DefaultModelFactory`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_embeddings::traits::`
`crates/context-graph-cli/src/commands/backfill_e14.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-cli/src/commands/divergence.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/divergence.rs` -> `super::*`
`crates/context-graph-cli/src/commands/export_training.rs` -> `context_graph_storage::teleological::`
`crates/context-graph-cli/src/commands/hooks/args.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/error.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/memory_cache.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `crate::commands::hooks::args::OutputFormat`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `super::args::PostToolArgs`
`crates/context-graph-cli/src/commands/hooks/post_tool_use.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/pre_compact.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/hooks/pre_compact.rs` -> `super::args::PreCompactArgs`
`crates/context-graph-cli/src/commands/hooks/pre_compact.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/pre_compact.rs` -> `super::types::`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `crate::commands::hooks::args::OutputFormat`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `super::args::PreToolArgs`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `super::memory_cache::`
`crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs` -> `super::types::`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `crate::commands::hooks::args::OutputFormat`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `super::args::SessionEndArgs`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/session_end.rs` -> `super::memory_cache::clear_session_cache`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `crate::commands::hooks::args::OutputFormat`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `super::args::SessionStartArgs`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/session_start.rs` -> `super::memory_cache`
`crates/context-graph-cli/src/commands/hooks/task_completed.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/hooks/task_completed.rs` -> `super::args::TaskCompletedArgs`
`crates/context-graph-cli/src/commands/hooks/task_completed.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/hooks/task_completed.rs` -> `super::types::`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `context_graph_cli::commands::hooks::`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `context_graph_cli::commands::hooks::CoherenceState`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `context_graph_cli::commands::hooks::DriftMetrics`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `context_graph_cli::commands::hooks::HookEventType`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `context_graph_cli::commands::hooks::StabilityLevel`
`crates/context-graph-cli/src/commands/hooks/types.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `crate::commands::hooks::args::OutputFormat`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `super::*`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `super::args::PromptSubmitArgs`
`crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs` -> `super::error::`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_core::learner::sha256_json`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_core::learner_training::`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_core::learning::`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_core::training::NUM_CROSS_CORRELATIONS`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_core::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-cli/src/commands/learning.rs` -> `context_graph_storage::teleological::`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `context_graph_core::memory::`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `crate::error::CliExitCode`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `crate::mcp_client::McpClientError`
`crates/context-graph-cli/src/commands/memory/capture.rs` -> `crate::mcp_helpers::`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::commands::hooks::memory_cache::get_cached_memories`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::error::CliExitCode`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::mcp_client::McpClientError`
`crates/context-graph-cli/src/commands/memory/inject.rs` -> `crate::mcp_helpers::`
`crates/context-graph-cli/src/commands/session/persist.rs` -> `crate::commands::hooks::session_state::`
`crates/context-graph-cli/src/commands/session/persist.rs` -> `crate::commands::hooks::session_state::store_in_cache`
`crates/context-graph-cli/src/commands/session/persist.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/session/persist.rs` -> `super::*`
`crates/context-graph-cli/src/commands/session/restore.rs` -> `crate::commands::hooks::session_state::`
`crates/context-graph-cli/src/commands/session/restore.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/commands/session/restore.rs` -> `super::*`
`crates/context-graph-cli/src/commands/setup.rs` -> `super::*`
`crates/context-graph-cli/src/commands/topic.rs` -> `crate::mcp_client::McpClient`
`crates/context-graph-cli/src/commands/topic.rs` -> `super::*`
`crates/context-graph-cli/src/commands/utl.rs` -> `context_graph_core::learner::`
`crates/context-graph-cli/src/commands/utl.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-cli/src/commands/utl.rs` -> `context_graph_storage::teleological::`
`crates/context-graph-cli/src/commands/watch.rs` -> `context_graph_core::memory::`
`crates/context-graph-cli/src/commands/watch.rs` -> `context_graph_core::memory::store::MemoryStore`
`crates/context-graph-cli/src/commands/watch.rs` -> `context_graph_core::memory::watcher::GitFileWatcher`
`crates/context-graph-cli/src/commands/watch.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-cli/src/commands/watch.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-cli/src/error.rs` -> `context_graph_storage::StorageError`
`crates/context-graph-cli/src/error.rs` -> `super::*`
`crates/context-graph-cli/src/main.rs` -> `context_graph_cli::commands`
`crates/context-graph-cli/src/mcp_client.rs` -> `super::*`
`crates/context-graph-cli/src/mcp_helpers.rs` -> `crate::commands::test_utils::GLOBAL_IDENTITY_LOCK`
`crates/context-graph-cli/src/mcp_helpers.rs` -> `crate::error::CliExitCode`
`crates/context-graph-cli/src/mcp_helpers.rs` -> `crate::mcp_client::`
`crates/context-graph-cli/src/mcp_helpers.rs` -> `super::*`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `context_graph_core::causal::asymmetric::compute_e5_asymmetric_fingerprint_similarity`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `crate::retrieval::distance::cosine_similarity_raw as cosine_similarity_f32`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/causal/asymmetric.rs` -> `super::*`
`crates/context-graph-core/src/causal/chain.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-core/src/causal/chain.rs` -> `context_graph_core::causal::chain::`
`crates/context-graph-core/src/causal/chain.rs` -> `context_graph_core::causal::chain::rank_causes_by_abduction`
`crates/context-graph-core/src/causal/chain.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/causal/chain.rs` -> `crate::retrieval::distance::cosine_similarity_raw as cosine_similarity`
`crates/context-graph-core/src/causal/chain.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/causal/inference.rs` -> `super::*`
`crates/context-graph-core/src/causal/mod.rs` -> `super::*`
`crates/context-graph-core/src/causal/scm.rs` -> `super::*`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::clustering::birch::`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::clustering::birch::BIRCHEntry`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::clustering::birch::BIRCHNode`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::clustering::birch::BIRCHParams`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::clustering::birch::ClusteringFeature`
`crates/context-graph-core/src/clustering/birch.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/cluster.rs` -> `context_graph_core::clustering::Cluster`
`crates/context-graph-core/src/clustering/cluster.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/cluster.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/clustering/cluster.rs` -> `super::*`
`crates/context-graph-core/src/clustering/error.rs` -> `crate::error::StorageError`
`crates/context-graph-core/src/clustering/error.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/clustering/error.rs` -> `super::*`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `context_graph_core::clustering::`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `context_graph_core::types::SemanticFingerprint`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `crate::embeddings::config::get_dimension`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `crate::teleological::`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `crate::types::SemanticFingerprint`
`crates/context-graph-core/src/clustering/fingerprint_matrix.rs` -> `crate::types::SparseVector`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::clustering::HDBSCANClusterer`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::clustering::hdbscan::`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::clustering::hdbscan::ClusterSelectionMethod`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::clustering::hdbscan::HDBSCANParams`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::index::config::DistanceMetric`
`crates/context-graph-core/src/clustering/hdbscan.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/manager.rs` -> `context_graph_core::clustering::`
`crates/context-graph-core/src/clustering/manager.rs` -> `context_graph_core::clustering::MultiSpaceClusterManager`
`crates/context-graph-core/src/clustering/manager.rs` -> `context_graph_core::types::SemanticFingerprint`
`crates/context-graph-core/src/clustering/manager.rs` -> `crate::clustering::`
`crates/context-graph-core/src/clustering/manager.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/clustering/manager.rs` -> `crate::embeddings::config::get_dimension`
`crates/context-graph-core/src/clustering/membership.rs` -> `context_graph_core::clustering::ClusterMembership`
`crates/context-graph-core/src/clustering/membership.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/membership.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/clustering/membership.rs` -> `super::*`
`crates/context-graph-core/src/clustering/persistence.rs` -> `context_graph_core::clustering::`
`crates/context-graph-core/src/clustering/persistence.rs` -> `context_graph_core::clustering::TopicPortfolio`
`crates/context-graph-core/src/clustering/persistence.rs` -> `crate::clustering::topic::`
`crates/context-graph-core/src/clustering/persistence.rs` -> `super::*`
`crates/context-graph-core/src/clustering/persistence.rs` -> `super::stability::TopicStabilityTracker`
`crates/context-graph-core/src/clustering/persistence.rs` -> `super::topic::Topic`
`crates/context-graph-core/src/clustering/stability.rs` -> `super::*`
`crates/context-graph-core/src/clustering/stability.rs` -> `super::topic::Topic`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `context_graph_core::clustering::`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `crate::embeddings::category::`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `super::*`
`crates/context-graph-core/src/clustering/synthesizer.rs` -> `super::error::ClusterError`
`crates/context-graph-core/src/clustering/topic.rs` -> `context_graph_core::clustering::`
`crates/context-graph-core/src/clustering/topic.rs` -> `context_graph_core::clustering::TopicProfile`
`crates/context-graph-core/src/clustering/topic.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/clustering/topic.rs` -> `crate::embeddings::category::`
`crates/context-graph-core/src/clustering/topic.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/clustering/topic.rs` -> `super::*`
`crates/context-graph-core/src/code/mod.rs` -> `context_graph_core::code::`
`crates/context-graph-core/src/code/mod.rs` -> `context_graph_core::code::CodeQueryType`
`crates/context-graph-core/src/code/mod.rs` -> `context_graph_core::code::detect_code_query_type`
`crates/context-graph-core/src/code/mod.rs` -> `crate::retrieval::distance::cosine_similarity_raw as cosine_similarity`
`crates/context-graph-core/src/code/mod.rs` -> `super::*`
`crates/context-graph-core/src/config/constants.rs` -> `super::*`
`crates/context-graph-core/src/config/constants.rs` -> `super::similarity`
`crates/context-graph-core/src/config/embedder_config.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/config/embedder_config.rs` -> `super::*`
`crates/context-graph-core/src/config/mod.rs` -> `crate::error::`
`crates/context-graph-core/src/config/sub_configs.rs` -> `crate::error::CoreError`
`crates/context-graph-core/src/config/tests/default_tests.rs` -> `crate::config::`
`crates/context-graph-core/src/config/tests/mcp_config_tests.rs` -> `crate::config::McpConfig`
`crates/context-graph-core/src/config/tests/serialization_tests.rs` -> `crate::config::`
`crates/context-graph-core/src/config/tests/validation_tests.rs` -> `crate::config::`
`crates/context-graph-core/src/constellation/compiler.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/constellation/compiler.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/constellation/compiler.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/constellation/compiler.rs` -> `super::reservoir::ReservoirSample`
`crates/context-graph-core/src/constellation/compiler.rs` -> `super::types::`
`crates/context-graph-core/src/constellation/compiler.rs` -> `super::welford::`
`crates/context-graph-core/src/constellation/reservoir.rs` -> `super::*`
`crates/context-graph-core/src/constellation/tests.rs` -> `crate::constellation::`
`crates/context-graph-core/src/constellation/tests.rs` -> `crate::constellation::ConstellationAccumulator`
`crates/context-graph-core/src/constellation/tests.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/constellation/tests.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/constellation/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/constellation/types.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/constellation/welford.rs` -> `super::*`
`crates/context-graph-core/src/contrastive/mining.rs` -> `crate::contrastive::types::`
`crates/context-graph-core/src/contrastive/mining.rs` -> `crate::similarity::jaccard_similarity`
`crates/context-graph-core/src/contrastive/mining.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/contrastive/mining.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/contrastive/tests.rs` -> `crate::contrastive::`
`crates/context-graph-core/src/contrastive/tests.rs` -> `crate::contrastive::types::`
`crates/context-graph-core/src/contrastive/tests.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/contrastive/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/contrastive/types.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/embeddings/category.rs` -> `context_graph_core::embeddings::category::category_for`
`crates/context-graph-core/src/embeddings/category.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/embeddings/category.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/embeddings/category.rs` -> `super::*`
`crates/context-graph-core/src/embeddings/config.rs` -> `context_graph_core::embeddings::config::get_config`
`crates/context-graph-core/src/embeddings/config.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/embeddings/config.rs` -> `crate::embeddings::category::`
`crates/context-graph-core/src/embeddings/config.rs` -> `crate::index::config::`
`crates/context-graph-core/src/embeddings/config.rs` -> `crate::index::config::*`
`crates/context-graph-core/src/embeddings/config.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/embeddings/provider.rs` -> `context_graph_core::embeddings::provider::StubMultiArrayProvider`
`crates/context-graph-core/src/embeddings/provider.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-core/src/embeddings/provider.rs` -> `crate::error::`
`crates/context-graph-core/src/embeddings/provider.rs` -> `crate::traits::`
`crates/context-graph-core/src/embeddings/provider.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/embeddings/provider.rs` -> `super::*`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `context_graph_core::embeddings::stubs::StubSingleEmbedder`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `context_graph_core::embeddings::stubs::StubSparseEmbedder`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `context_graph_core::embeddings::stubs::StubTokenEmbedder`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `crate::embeddings::EMBEDDER_CONFIGS`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `crate::error::`
`crates/context-graph-core/src/embeddings/stubs.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/embeddings/token_pruning.rs` -> `crate::quantization::`
`crates/context-graph-core/src/embeddings/token_pruning.rs` -> `super::*`
`crates/context-graph-core/src/embeddings/vector.rs` -> `context_graph_core::embeddings::vector::BinaryVector`
`crates/context-graph-core/src/embeddings/vector.rs` -> `context_graph_core::embeddings::vector::DenseVector`
`crates/context-graph-core/src/embeddings/vector.rs` -> `super::*`
`crates/context-graph-core/src/entity/mod.rs` -> `super::*`
`crates/context-graph-core/src/error/conversions.rs` -> `crate::index::config::EmbedderIndex`
`crates/context-graph-core/src/error/conversions.rs` -> `crate::index::error::IndexError as ExistingIndexError`
`crates/context-graph-core/src/error/conversions.rs` -> `crate::teleological::embedder::Embedder`
`crates/context-graph-core/src/error/conversions.rs` -> `super::legacy::CoreError`
`crates/context-graph-core/src/error/conversions.rs` -> `super::sub_errors::`
`crates/context-graph-core/src/error/conversions.rs` -> `super::unified::ContextGraphError`
`crates/context-graph-core/src/error/legacy.rs` -> `context_graph_core::CoreError`
`crates/context-graph-core/src/error/mod.rs` -> `context_graph_core::Embedder`
`crates/context-graph-core/src/error/mod.rs` -> `context_graph_core::error::`
`crates/context-graph-core/src/error/sub_errors.rs` -> `crate::teleological::embedder::Embedder`
`crates/context-graph-core/src/error/tests.rs` -> `crate::teleological::embedder::Embedder`
`crates/context-graph-core/src/error/tests.rs` -> `super::*`
`crates/context-graph-core/src/error/unified.rs` -> `context_graph_core::Embedder`
`crates/context-graph-core/src/error/unified.rs` -> `context_graph_core::error::`
`crates/context-graph-core/src/error/unified.rs` -> `super::sub_errors::`
`crates/context-graph-core/src/fusion/mod.rs` -> `context_graph_core::fusion::`
`crates/context-graph-core/src/fusion/mod.rs` -> `crate::retrieval::AggregationStrategy`
`crates/context-graph-core/src/fusion/mod.rs` -> `super::*`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `context_graph_core::graph::asymmetric::`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `context_graph_core::graph::asymmetric::compute_e8_asymmetric_fingerprint_similarity`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `crate::retrieval::distance::cosine_similarity_raw as cosine_similarity_f32`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/graph/asymmetric.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/direction.rs` -> `context_graph_core::graph_linking::DirectedRelation`
`crates/context-graph-core/src/graph_linking/direction.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/edge_builder.rs` -> `context_graph_core::graph_linking::edge_builder::`
`crates/context-graph-core/src/graph_linking/edge_builder.rs` -> `crate::graph_linking::EmbedderEdge`
`crates/context-graph-core/src/graph_linking/edge_builder.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/edge_type.rs` -> `context_graph_core::graph_linking::GraphLinkEdgeType`
`crates/context-graph-core/src/graph_linking/edge_type.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/embedder_edge.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-core/src/graph_linking/embedder_edge.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/graph_linking/embedder_edge.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/error.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/error.rs` -> `super::GraphLinkEdgeType`
`crates/context-graph-core/src/graph_linking/knn_graph.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-core/src/graph_linking/knn_graph.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/knn_graph.rs` -> `super::EmbedderEdge`
`crates/context-graph-core/src/graph_linking/nn_descent.rs` -> `context_graph_core::graph_linking::nn_descent::`
`crates/context-graph-core/src/graph_linking/nn_descent.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/service.rs` -> `context_graph_core::graph_linking::service::`
`crates/context-graph-core/src/graph_linking/service.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/graph_linking/service.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/storage_keys.rs` -> `context_graph_core::graph_linking::EdgeStorageKey`
`crates/context-graph-core/src/graph_linking/storage_keys.rs` -> `context_graph_core::graph_linking::TypedEdgeStorageKey`
`crates/context-graph-core/src/graph_linking/storage_keys.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/thresholds.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-core/src/graph_linking/thresholds.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/thresholds.rs` -> `super::GraphLinkEdgeType`
`crates/context-graph-core/src/graph_linking/typed_edge.rs` -> `context_graph_core::graph_linking::`
`crates/context-graph-core/src/graph_linking/typed_edge.rs` -> `super::*`
`crates/context-graph-core/src/graph_linking/weight_projector.rs` -> `super::*`
`crates/context-graph-core/src/index/config.rs` -> `super::*`
`crates/context-graph-core/src/index/error.rs` -> `super::*`
`crates/context-graph-core/src/index/error.rs` -> `super::config::EmbedderIndex`
`crates/context-graph-core/src/index/hnsw_impl/mod.rs` -> `super::config::`
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -> `crate::index::config::`
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -> `crate::index::error::IndexResult`
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -> `crate::index::splade_impl::SpladeInvertedIndex`
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -> `crate::index::status::IndexStatus`
`crates/context-graph-core/src/index/hnsw_impl/multi_space.rs` -> `super::real_hnsw::RealHnswIndex`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `crate::index::config::EmbedderIndex`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `crate::index::error::`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `crate::index::manager::MultiSpaceIndexManager`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `crate::index::status::IndexStatus`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/index/hnsw_impl/multi_space_trait.rs` -> `super::multi_space::HnswMultiSpaceIndex`
`crates/context-graph-core/src/index/hnsw_impl/persistence.rs` -> `crate::index::error::`
`crates/context-graph-core/src/index/hnsw_impl/persistence.rs` -> `super::real_hnsw::RealHnswIndex`
`crates/context-graph-core/src/index/hnsw_impl/persistence.rs` -> `super::types::HnswPersistenceData`
`crates/context-graph-core/src/index/hnsw_impl/real_hnsw.rs` -> `crate::index::config::`
`crates/context-graph-core/src/index/hnsw_impl/real_hnsw.rs` -> `crate::index::error::`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::index::config::EmbedderIndex`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::index::error::IndexError`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::index::hnsw_impl::HnswMultiSpaceIndex`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::index::manager::MultiSpaceIndexManager`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::index::status::IndexHealth`
`crates/context-graph-core/src/index/hnsw_impl/tests/multi_space_tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/index/hnsw_impl/tests/persistence_tests.rs` -> `crate::index::hnsw_impl::`
`crates/context-graph-core/src/index/hnsw_impl/tests/persistence_tests.rs` -> `crate::index::manager::MultiSpaceIndexManager`
`crates/context-graph-core/src/index/hnsw_impl/tests/persistence_tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/index/hnsw_impl/tests/persistence_tests.rs` -> `super::real_hnsw_tests::random_vector`
`crates/context-graph-core/src/index/hnsw_impl/tests/real_hnsw_tests.rs` -> `crate::index::config::`
`crates/context-graph-core/src/index/hnsw_impl/tests/real_hnsw_tests.rs` -> `crate::index::error::IndexError`
`crates/context-graph-core/src/index/hnsw_impl/tests/real_hnsw_tests.rs` -> `crate::index::hnsw_impl::RealHnswIndex`
`crates/context-graph-core/src/index/hnsw_impl/types.rs` -> `crate::index::config::DistanceMetric`
`crates/context-graph-core/src/index/hnsw_impl/types.rs` -> `super::HnswConfig`
`crates/context-graph-core/src/index/manager.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/index/manager.rs` -> `super::config::EmbedderIndex`
`crates/context-graph-core/src/index/manager.rs` -> `super::error::IndexResult`
`crates/context-graph-core/src/index/manager.rs` -> `super::status::IndexStatus`
`crates/context-graph-core/src/index/mod.rs` -> `context_graph_core::index::`
`crates/context-graph-core/src/index/splade_impl.rs` -> `super::*`
`crates/context-graph-core/src/index/splade_impl.rs` -> `super::config::`
`crates/context-graph-core/src/index/splade_impl.rs` -> `super::error::`
`crates/context-graph-core/src/index/status.rs` -> `super::*`
`crates/context-graph-core/src/index/status.rs` -> `super::config::EmbedderIndex`
`crates/context-graph-core/src/injection/budget.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/injection/budget.rs` -> `super::*`
`crates/context-graph-core/src/injection/budget.rs` -> `super::candidate::`
`crates/context-graph-core/src/injection/candidate.rs` -> `crate::clustering::MAX_WEIGHTED_AGREEMENT`
`crates/context-graph-core/src/injection/candidate.rs` -> `crate::embeddings::is_temporal`
`crates/context-graph-core/src/injection/candidate.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/injection/candidate.rs` -> `super::*`
`crates/context-graph-core/src/injection/formatter.rs` -> `crate::retrieval::divergence::DivergenceAlert`
`crates/context-graph-core/src/injection/formatter.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/injection/formatter.rs` -> `super::*`
`crates/context-graph-core/src/injection/formatter.rs` -> `super::candidate::`
`crates/context-graph-core/src/injection/mod.rs` -> `crate::clustering::MAX_WEIGHTED_AGREEMENT`
`crates/context-graph-core/src/injection/pipeline.rs` -> `context_graph_core::injection::InjectionPipeline`
`crates/context-graph-core/src/injection/pipeline.rs` -> `context_graph_core::memory::MemoryStore`
`crates/context-graph-core/src/injection/pipeline.rs` -> `context_graph_core::retrieval::SimilarityRetriever`
`crates/context-graph-core/src/injection/pipeline.rs` -> `crate::memory::`
`crates/context-graph-core/src/injection/pipeline.rs` -> `crate::retrieval::`
`crates/context-graph-core/src/injection/pipeline.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/injection/priority.rs` -> `crate::injection::candidate::InjectionCategory`
`crates/context-graph-core/src/injection/priority.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/injection/priority.rs` -> `super::*`
`crates/context-graph-core/src/injection/priority.rs` -> `super::candidate::InjectionCandidate`
`crates/context-graph-core/src/injection/result.rs` -> `context_graph_core::injection::InjectionResult`
`crates/context-graph-core/src/injection/result.rs` -> `crate::retrieval::DivergenceAlert`
`crates/context-graph-core/src/injection/result.rs` -> `super::*`
`crates/context-graph-core/src/injection/result.rs` -> `super::candidate::InjectionCategory`
`crates/context-graph-core/src/injection/temporal_enrichment.rs` -> `crate::similarity::cosine_similarity`
`crates/context-graph-core/src/injection/temporal_enrichment.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/injection/temporal_enrichment.rs` -> `super::*`
`crates/context-graph-core/src/learner/mod.rs` -> `crate::error::`
`crates/context-graph-core/src/learner/mod.rs` -> `super::*`
`crates/context-graph-core/src/learner_training.rs` -> `crate::error::`
`crates/context-graph-core/src/learner_training.rs` -> `crate::learning::LearningEvent`
`crates/context-graph-core/src/learner_training.rs` -> `crate::training::NUM_CROSS_CORRELATIONS`
`crates/context-graph-core/src/learner_training.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/learning/mod.rs` -> `crate::error::`
`crates/context-graph-core/src/learning/mod.rs` -> `crate::similarity::MultiUtlParams`
`crates/context-graph-core/src/learning/mod.rs` -> `crate::training::NUM_CROSS_CORRELATIONS`
`crates/context-graph-core/src/learning/mod.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/learning/mod.rs` -> `super::*`
`crates/context-graph-core/src/lib.rs` -> `context_graph_core::traits::`
`crates/context-graph-core/src/llm_edge_validation/mod.rs` -> `crate::typed_edge_export::LLMVerdict`
`crates/context-graph-core/src/llm_edge_validation/tests.rs` -> `crate::typed_edge_export::LLMVerdict`
`crates/context-graph-core/src/llm_edge_validation/tests.rs` -> `super::*`
`crates/context-graph-core/src/memory/ast_chunker.rs` -> `super::*`
`crates/context-graph-core/src/memory/capture.rs` -> `context_graph_core::memory::capture::MultiArrayEmbeddingAdapter`
`crates/context-graph-core/src/memory/capture.rs` -> `context_graph_embeddings::get_warm_provider`
`crates/context-graph-core/src/memory/capture.rs` -> `crate::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-core/src/memory/capture.rs` -> `crate::traits::TeleologicalMemoryStore`
`crates/context-graph-core/src/memory/capture.rs` -> `crate::types::SourceMetadata`
`crates/context-graph-core/src/memory/capture.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/memory/chunker.rs` -> `context_graph_core::memory::`
`crates/context-graph-core/src/memory/chunker.rs` -> `context_graph_core::memory::TextChunker`
`crates/context-graph-core/src/memory/chunker.rs` -> `super::*`
`crates/context-graph-core/src/memory/chunker.rs` -> `super::ChunkMetadata`
`crates/context-graph-core/src/memory/code_capture.rs` -> `crate::types::`
`crates/context-graph-core/src/memory/code_capture.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/memory/code_capture.rs` -> `super::*`
`crates/context-graph-core/src/memory/code_capture.rs` -> `super::ast_chunker::`
`crates/context-graph-core/src/memory/code_capture.rs` -> `super::super::ast_chunker::CodeChunkMetadata`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `crate::types::CodeEntity`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `crate::types::CodeLanguage`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `super::*`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `super::ast_chunker::`
`crates/context-graph-core/src/memory/code_watcher.rs` -> `super::code_capture::`
`crates/context-graph-core/src/memory/manager.rs` -> `crate::memory::SessionStatus`
`crates/context-graph-core/src/memory/manager.rs` -> `super::*`
`crates/context-graph-core/src/memory/manager.rs` -> `super::Session`
`crates/context-graph-core/src/memory/mod.rs` -> `context_graph_core::memory::`
`crates/context-graph-core/src/memory/mod.rs` -> `context_graph_core::types::fingerprint::TeleologicalArray`
`crates/context-graph-core/src/memory/mod.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/memory/mod.rs` -> `crate::types::fingerprint::TeleologicalArray`
`crates/context-graph-core/src/memory/mod.rs` -> `super::*`
`crates/context-graph-core/src/memory/session.rs` -> `context_graph_core::memory::`
`crates/context-graph-core/src/memory/session.rs` -> `context_graph_core::memory::Session`
`crates/context-graph-core/src/memory/session.rs` -> `super::*`
`crates/context-graph-core/src/memory/source.rs` -> `super::*`
`crates/context-graph-core/src/memory/store.rs` -> `context_graph_core::memory::`
`crates/context-graph-core/src/memory/store.rs` -> `context_graph_core::memory::MemoryStore`
`crates/context-graph-core/src/memory/store.rs` -> `crate::memory::`
`crates/context-graph-core/src/memory/store.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/memory/store.rs` -> `super::*`
`crates/context-graph-core/src/memory/watcher.rs` -> `crate::memory::MemoryStore`
`crates/context-graph-core/src/memory/watcher.rs` -> `crate::memory::capture::TestEmbeddingProvider`
`crates/context-graph-core/src/memory/watcher.rs` -> `super::*`
`crates/context-graph-core/src/memory/watcher.rs` -> `super::capture::CaptureError`
`crates/context-graph-core/src/memory/watcher.rs` -> `super::chunker::ChunkerError`
`crates/context-graph-core/src/monitoring.rs` -> `context_graph_core::monitoring::`
`crates/context-graph-core/src/monitoring.rs` -> `super::*`
`crates/context-graph-core/src/quantization/accuracy.rs` -> `super::*`
`crates/context-graph-core/src/quantization/accuracy.rs` -> `super::types::Precision`
`crates/context-graph-core/src/quantization/batch.rs` -> `crate::embeddings::TokenPruningEmbedding`
`crates/context-graph-core/src/quantization/batch.rs` -> `super::*`
`crates/context-graph-core/src/quantization/batch.rs` -> `super::traits::Quantizable`
`crates/context-graph-core/src/quantization/batch.rs` -> `super::types::`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `context_graph_core::quantization::fingerprint::quantize_fingerprint`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `crate::embeddings::vector::DenseVector`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `crate::quantization::accuracy::`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/quantization/fingerprint.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/quantization/traits.rs` -> `super::types::`
`crates/context-graph-core/src/quantization/types.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/aggregation.rs` -> `context_graph_core::retrieval::AggregationStrategy`
`crates/context-graph-core/src/retrieval/aggregation.rs` -> `crate::config::constants::similarity`
`crates/context-graph-core/src/retrieval/aggregation.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/retrieval/aggregation.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/config.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/retrieval/config.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/config.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/detector.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/retrieval/detector.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/detector.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/retrieval/detector.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/detector.rs` -> `super::config::`
`crates/context-graph-core/src/retrieval/detector.rs` -> `super::divergence::`
`crates/context-graph-core/src/retrieval/distance.rs` -> `crate::causal::asymmetric::`
`crates/context-graph-core/src/retrieval/distance.rs` -> `crate::causal::asymmetric::CausalDirection`
`crates/context-graph-core/src/retrieval/distance.rs` -> `crate::causal::asymmetric::direction_mod`
`crates/context-graph-core/src/retrieval/distance.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/distance.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/retrieval/distance.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/divergence.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/retrieval/divergence.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/divergence.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/executor.rs` -> `crate::error::CoreResult`
`crates/context-graph-core/src/retrieval/executor.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/retrieval/executor.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `context_graph_core::retrieval::InMemoryMultiEmbeddingExecutor`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `context_graph_core::stubs::`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `crate::config::constants::pipeline`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `crate::error::`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `crate::stubs::`
`crates/context-graph-core/src/retrieval/in_memory_executor.rs` -> `crate::traits::`
`crates/context-graph-core/src/retrieval/insight_annotation.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/insight_annotation.rs` -> `super::CombinedResult`
`crates/context-graph-core/src/retrieval/mod.rs` -> `context_graph_core::retrieval::`
`crates/context-graph-core/src/retrieval/mod.rs` -> `context_graph_core::stubs::`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `crate::causal::asymmetric::CausalDirection`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/multi_space.rs` -> `super::config::SimilarityThresholds`
`crates/context-graph-core/src/retrieval/query.rs` -> `context_graph_core::retrieval::`
`crates/context-graph-core/src/retrieval/query.rs` -> `crate::config::constants::`
`crates/context-graph-core/src/retrieval/query.rs` -> `crate::error::`
`crates/context-graph-core/src/retrieval/query.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/retrieval/query.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/query.rs` -> `super::aggregation::AggregationStrategy`
`crates/context-graph-core/src/retrieval/result.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `context_graph_core::memory::MemoryStore`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `context_graph_core::retrieval::SimilarityRetriever`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `crate::memory::`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `crate::retrieval::config::MAX_RECENT_MEMORIES`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/retrieval/retriever.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/similarity.rs` -> `crate::embeddings::category::`
`crates/context-graph-core/src/retrieval/similarity.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/retrieval/similarity.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/sparse_index.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/retrieval/sparse_index.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/teleological_query.rs` -> `context_graph_core::retrieval::TeleologicalQuery`
`crates/context-graph-core/src/retrieval/teleological_query.rs` -> `crate::error::`
`crates/context-graph-core/src/retrieval/teleological_query.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/retrieval/teleological_query.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/teleological_query.rs` -> `super::PipelineStageConfig`
`crates/context-graph-core/src/retrieval/teleological_result.rs` -> `super::*`
`crates/context-graph-core/src/retrieval/teleological_result.rs` -> `super::PipelineStageTiming`
`crates/context-graph-core/src/retrieval/tests.rs` -> `crate::stubs::`
`crates/context-graph-core/src/retrieval/tests.rs` -> `crate::traits::TeleologicalMemoryStore`
`crates/context-graph-core/src/retrieval/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/retrieval/tests.rs` -> `super::*`
`crates/context-graph-core/src/similarity/config.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/config.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/similarity/config.rs` -> `super::*`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `crate::retrieval::AggregationStrategy`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `super::*`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `super::config::`
`crates/context-graph-core/src/similarity/default_engine.rs` -> `super::engine::CrossSpaceSimilarityEngine`
`crates/context-graph-core/src/similarity/dense/primitives.rs` -> `super::error::DenseSimilarityError`
`crates/context-graph-core/src/similarity/dense/simd.rs` -> `super::error::DenseSimilarityError`
`crates/context-graph-core/src/similarity/dense/simd.rs` -> `super::primitives::cosine_similarity`
`crates/context-graph-core/src/similarity/dense/tests/integration_tests.rs` -> `super::super::*`
`crates/context-graph-core/src/similarity/dense/tests/primitives_tests.rs` -> `super::super::*`
`crates/context-graph-core/src/similarity/dense/tests/simd_tests.rs` -> `super::super::*`
`crates/context-graph-core/src/similarity/engine.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/engine.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/similarity/engine.rs` -> `super::config::`
`crates/context-graph-core/src/similarity/engine.rs` -> `super::error::SimilarityError`
`crates/context-graph-core/src/similarity/engine.rs` -> `super::explanation::SimilarityExplanation`
`crates/context-graph-core/src/similarity/engine.rs` -> `super::multi_utl::MultiUtlParams`
`crates/context-graph-core/src/similarity/error.rs` -> `super::*`
`crates/context-graph-core/src/similarity/explanation.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/similarity/explanation.rs` -> `super::*`
`crates/context-graph-core/src/similarity/mod.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/multi_utl.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/multi_utl.rs` -> `context_graph_core::similarity::sigmoid`
`crates/context-graph-core/src/similarity/multi_utl.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/similarity/multi_utl.rs` -> `super::*`
`crates/context-graph-core/src/similarity/result.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/similarity/result.rs` -> `super::*`
`crates/context-graph-core/src/similarity/sparse.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/similarity/sparse.rs` -> `super::*`
`crates/context-graph-core/src/similarity/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/similarity/tests.rs` -> `super::*`
`crates/context-graph-core/src/similarity/token_level.rs` -> `context_graph_core::similarity::`
`crates/context-graph-core/src/similarity/token_level.rs` -> `context_graph_core::similarity::token_alignments`
`crates/context-graph-core/src/similarity/token_level.rs` -> `crate::similarity::dense::cosine_similarity`
`crates/context-graph-core/src/similarity/token_level.rs` -> `crate::types::fingerprint::E12_TOKEN_DIM`
`crates/context-graph-core/src/similarity/token_level.rs` -> `super::*`
`crates/context-graph-core/src/stubs/mod.rs` -> `context_graph_core::stubs::`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `context_graph_core::stubs::StubMultiArrayProvider`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `crate::error::CoreResult`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `crate::traits::`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/stubs/multi_array_stub.rs` -> `super::*`
`crates/context-graph-core/src/stubs/teleological_store_stub/content.rs` -> `crate::error::CoreResult`
`crates/context-graph-core/src/stubs/teleological_store_stub/content.rs` -> `super::InMemoryTeleologicalStore`
`crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs` -> `crate::clustering::PersistedTopicPortfolio`
`crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs` -> `crate::traits::TeleologicalStorageBackend`
`crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs` -> `crate::types::`
`crates/context-graph-core/src/stubs/teleological_store_stub/mod.rs` -> `crate::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs` -> `crate::error::`
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs` -> `crate::traits::`
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs` -> `super::InMemoryTeleologicalStore`
`crates/context-graph-core/src/stubs/teleological_store_stub/search.rs` -> `super::similarity::compute_semantic_scores`
`crates/context-graph-core/src/stubs/teleological_store_stub/similarity.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` -> `crate::error::CoreError`
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` -> `crate::traits::`
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` -> `super::InMemoryTeleologicalStore`
`crates/context-graph-core/src/stubs/teleological_store_stub/tests.rs` -> `super::similarity::cosine_similarity`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::error::`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::traits::`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::types::CausalRelationship`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::types::CausalSearchResult`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::types::FileIndexEntry`
`crates/context-graph-core/src/stubs/teleological_store_stub/trait_impl.rs` -> `crate::types::SourceMetadata`
`crates/context-graph-core/src/teleological/comparator/batch.rs` -> `context_graph_core::teleological::BatchComparator`
`crates/context-graph-core/src/teleological/comparator/batch.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/comparator/batch.rs` -> `crate::types::SemanticFingerprint`
`crates/context-graph-core/src/teleological/comparator/batch.rs` -> `super::result::ComparisonResult`
`crates/context-graph-core/src/teleological/comparator/batch.rs` -> `super::teleological::TeleologicalComparator`
`crates/context-graph-core/src/teleological/comparator/result.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/comparator/strategies.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `context_graph_core::teleological::`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `context_graph_core::types::SemanticFingerprint`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `crate::similarity::`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `crate::types::`
`crates/context-graph-core/src/teleological/comparator/teleological.rs` -> `super::result::ComparisonResult`
`crates/context-graph-core/src/teleological/comparator/tests.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/comparator/tests.rs` -> `crate::teleological::comparator::`
`crates/context-graph-core/src/teleological/comparator/tests.rs` -> `crate::types::SemanticFingerprint`
`crates/context-graph-core/src/teleological/comparator/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/teleological/comparison_error.rs` -> `super::*`
`crates/context-graph-core/src/teleological/embedder.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/teleological/embedder.rs` -> `super::*`
`crates/context-graph-core/src/teleological/groups.rs` -> `super::*`
`crates/context-graph-core/src/teleological/groups.rs` -> `super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/config.rs` -> `super::super::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/teleological/matrix_search/config.rs` -> `super::types::`
`crates/context-graph-core/src/teleological/matrix_search/config.rs` -> `super::weights::ComponentWeights`
`crates/context-graph-core/src/teleological/matrix_search/embedder_names.rs` -> `crate::teleological::embedder::Embedder`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `crate::teleological::TopicProfile`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `super::core::TeleologicalMatrixSearch`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `super::super::super::groups::`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `super::super::super::synergy_matrix::CROSS_CORRELATION_COUNT`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `super::super::super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/search/clustering.rs` -> `super::super::super::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::core::TeleologicalMatrixSearch`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::super::config::MatrixSearchConfig`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::super::super::groups::GroupType`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::super::super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::super::super::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/search/comprehensive.rs` -> `super::super::types::`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::config::MatrixSearchConfig`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::strategies::`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::super::groups::GroupType`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::super::synergy_matrix::`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::super::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/search/core.rs` -> `super::super::types::SimilarityBreakdown`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::helpers::`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::super::config::MatrixSearchConfig`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::super::super::groups::GroupType`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::super::super::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::super::super::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/strategies/computer.rs` -> `super::super::types::`
`crates/context-graph-core/src/teleological/matrix_search/strategies/helpers.rs` -> `super::super::super::groups::GroupType`
`crates/context-graph-core/src/teleological/matrix_search/strategies/helpers.rs` -> `super::super::super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/strategies/helpers.rs` -> `super::super::super::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/tests/advanced.rs` -> `crate::teleological::comparison_error::ComparisonValidationError`
`crates/context-graph-core/src/teleological/matrix_search/tests/advanced.rs` -> `crate::teleological::matrix_search::`
`crates/context-graph-core/src/teleological/matrix_search/tests/advanced.rs` -> `super::basic::`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::TopicProfile`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::groups::GroupAlignments`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::matrix_search::`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/tests/basic.rs` -> `crate::teleological::vector::TeleologicalVector`
`crates/context-graph-core/src/teleological/matrix_search/types.rs` -> `super::super::groups::GroupType`
`crates/context-graph-core/src/teleological/matrix_search/types.rs` -> `super::super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/matrix_search/weights.rs` -> `context_graph_core::teleological::ComponentWeights`
`crates/context-graph-core/src/teleological/matrix_search/weights.rs` -> `super::super::comparison_error::`
`crates/context-graph-core/src/teleological/meaning.rs` -> `super::*`
`crates/context-graph-core/src/teleological/meaning.rs` -> `super::types::`
`crates/context-graph-core/src/teleological/mod.rs` -> `context_graph_core::teleological::`
`crates/context-graph-core/src/teleological/profile/task_type.rs` -> `super::FusionStrategy`
`crates/context-graph-core/src/teleological/profile/teleological.rs` -> `crate::teleological::types::`
`crates/context-graph-core/src/teleological/profile/tests.rs` -> `crate::teleological::profile::`
`crates/context-graph-core/src/teleological/profile/tests.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/resolution.rs` -> `super::*`
`crates/context-graph-core/src/teleological/resolution.rs` -> `super::groups::`
`crates/context-graph-core/src/teleological/resolution.rs` -> `super::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/services/correlation_extractor.rs` -> `context_graph_core::teleological::SynergyMatrix`
`crates/context-graph-core/src/teleological/services/correlation_extractor.rs` -> `context_graph_core::teleological::services::CorrelationExtractor`
`crates/context-graph-core/src/teleological/services/correlation_extractor.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/correlation_extractor.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/fusion_engine.rs` -> `context_graph_core::teleological::services::FusionEngine`
`crates/context-graph-core/src/teleological/services/fusion_engine.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/fusion_engine.rs` -> `crate::teleological::NUM_EMBEDDERS`
`crates/context-graph-core/src/teleological/services/fusion_engine.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/fusion_engine.rs` -> `super::synergy_service::SynergyService`
`crates/context-graph-core/src/teleological/services/group_aggregator.rs` -> `context_graph_core::teleological::services::GroupAggregator`
`crates/context-graph-core/src/teleological/services/group_aggregator.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/group_aggregator.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `context_graph_core::teleological::services::MeaningPipeline`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `crate::teleological::CROSS_CORRELATION_COUNT`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `super::correlation_extractor::`
`crates/context-graph-core/src/teleological/services/meaning_pipeline.rs` -> `super::synergy_service::SynergyService`
`crates/context-graph-core/src/teleological/services/multi_space_retriever.rs` -> `context_graph_core::teleological::TeleologicalVector`
`crates/context-graph-core/src/teleological/services/multi_space_retriever.rs` -> `context_graph_core::teleological::services::MultiSpaceRetriever`
`crates/context-graph-core/src/teleological/services/multi_space_retriever.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/multi_space_retriever.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/profile_manager/builtin.rs` -> `crate::teleological::TeleologicalProfile`
`crates/context-graph-core/src/teleological/services/profile_manager/manager.rs` -> `context_graph_core::teleological::services::ProfileManager`
`crates/context-graph-core/src/teleological/services/profile_manager/manager.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/profile_manager/manager.rs` -> `super::builtin`
`crates/context-graph-core/src/teleological/services/profile_manager/manager.rs` -> `super::types::`
`crates/context-graph-core/src/teleological/services/profile_manager/tests.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/profile_manager/tests.rs` -> `crate::teleological::services::profile_manager::ProfileManager`
`crates/context-graph-core/src/teleological/services/profile_manager/types.rs` -> `crate::teleological::ProfileId`
`crates/context-graph-core/src/teleological/services/synergy_service.rs` -> `context_graph_core::teleological::SynergyMatrix`
`crates/context-graph-core/src/teleological/services/synergy_service.rs` -> `context_graph_core::teleological::services::SynergyService`
`crates/context-graph-core/src/teleological/services/synergy_service.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/synergy_service.rs` -> `super::*`
`crates/context-graph-core/src/teleological/services/tucker_decomposer.rs` -> `context_graph_core::teleological::services::TuckerDecomposer`
`crates/context-graph-core/src/teleological/services/tucker_decomposer.rs` -> `crate::teleological::`
`crates/context-graph-core/src/teleological/services/tucker_decomposer.rs` -> `super::*`
`crates/context-graph-core/src/teleological/synergy_matrix/accessors.rs` -> `super::constants::SYNERGY_DIM`
`crates/context-graph-core/src/teleological/synergy_matrix/accessors.rs` -> `super::types::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/analysis.rs` -> `super::constants::`
`crates/context-graph-core/src/teleological/synergy_matrix/analysis.rs` -> `super::types::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/constructors.rs` -> `super::constants::`
`crates/context-graph-core/src/teleological/synergy_matrix/constructors.rs` -> `super::types::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/presets.rs` -> `super::types::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/tests/analysis.rs` -> `crate::teleological::synergy_matrix::`
`crates/context-graph-core/src/teleological/synergy_matrix/tests/basic.rs` -> `crate::teleological::synergy_matrix::`
`crates/context-graph-core/src/teleological/synergy_matrix/tests/presets.rs` -> `crate::teleological::synergy_matrix::`
`crates/context-graph-core/src/teleological/synergy_matrix/tests/validation.rs` -> `crate::teleological::comparison_error::ComparisonValidationError`
`crates/context-graph-core/src/teleological/synergy_matrix/tests/validation.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/types.rs` -> `super::constants::SYNERGY_DIM`
`crates/context-graph-core/src/teleological/synergy_matrix/validation.rs` -> `context_graph_core::teleological::SynergyMatrix`
`crates/context-graph-core/src/teleological/synergy_matrix/validation.rs` -> `crate::teleological::comparison_error::`
`crates/context-graph-core/src/teleological/synergy_matrix/validation.rs` -> `super::constants::SYNERGY_DIM`
`crates/context-graph-core/src/teleological/synergy_matrix/validation.rs` -> `super::types::SynergyMatrix`
`crates/context-graph-core/src/teleological/tucker.rs` -> `crate::teleological::types::`
`crates/context-graph-core/src/teleological/tucker.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/teleological/tucker.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/teleological/tucker.rs` -> `super::*`
`crates/context-graph-core/src/teleological/types.rs` -> `super::*`
`crates/context-graph-core/src/teleological/vector.rs` -> `super::*`
`crates/context-graph-core/src/teleological/vector.rs` -> `super::groups::GroupAlignments`
`crates/context-graph-core/src/teleological/vector.rs` -> `super::synergy_matrix::CROSS_CORRELATION_COUNT`
`crates/context-graph-core/src/teleological/vector.rs` -> `super::types::`
`crates/context-graph-core/src/training/computation.rs` -> `crate::teleological::groups::GroupAlignments`
`crates/context-graph-core/src/training/computation.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/training/computation.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/training/computation.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/training/edge_signature.rs` -> `crate::graph_linking::`
`crates/context-graph-core/src/training/edge_signature.rs` -> `super::*`
`crates/context-graph-core/src/training/mod.rs` -> `crate::teleological::types::`
`crates/context-graph-core/src/training/temporal.rs` -> `crate::types::SourceMetadata`
`crates/context-graph-core/src/training/temporal.rs` -> `crate::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-core/src/training/tests.rs` -> `crate::graph_linking::GraphLinkEdgeType`
`crates/context-graph-core/src/training/tests.rs` -> `crate::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-core/src/training/tests.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/training/tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/training/tests.rs` -> `super::*`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `context_graph_core::error::CoreResult`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `context_graph_core::traits::`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `crate::error::CoreError`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `crate::error::CoreResult`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/traits/multi_array_embedding.rs` -> `super::*`
`crates/context-graph-core/src/traits/teleological_memory_store/backend.rs` -> `super::*`
`crates/context-graph-core/src/traits/teleological_memory_store/ext.rs` -> `context_graph_core::traits::TeleologicalMemoryStoreExt`
`crates/context-graph-core/src/traits/teleological_memory_store/ext.rs` -> `crate::error::CoreResult`
`crates/context-graph-core/src/traits/teleological_memory_store/ext.rs` -> `crate::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-core/src/traits/teleological_memory_store/ext.rs` -> `super::store::TeleologicalMemoryStore`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `context_graph_core::code::CodeQueryType`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `context_graph_core::fusion::FusionStrategy`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `context_graph_core::traits::`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `context_graph_core::traits::TeleologicalSearchOptions`
`crates/context-graph-core/src/traits/teleological_memory_store/options.rs` -> `crate::causal::asymmetric::CausalDirection`
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs` -> `crate::embeddings::category::category_for`
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs` -> `crate::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-core/src/traits/teleological_memory_store/result.rs` -> `super::*`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `context_graph_core::stubs::InMemoryTeleologicalStore`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `context_graph_core::traits::TeleologicalMemoryStore`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `crate::error::`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `crate::types::SourceMetadata`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/traits/teleological_memory_store/store.rs` -> `super::backend::TeleologicalStorageBackend`
`crates/context-graph-core/src/traits/teleological_memory_store_tests.rs` -> `crate::stubs::InMemoryTeleologicalStore`
`crates/context-graph-core/src/traits/teleological_memory_store_tests.rs` -> `crate::traits::`
`crates/context-graph-core/src/traits/teleological_memory_store_tests.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/typed_edge_export/mod.rs` -> `crate::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-core/src/typed_edge_export/tests.rs` -> `super::*`
`crates/context-graph-core/src/types/audit.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-core/src/types/audit.rs` -> `super::*`
`crates/context-graph-core/src/types/causal_relationship.rs` -> `super::*`
`crates/context-graph-core/src/types/code_entity.rs` -> `super::*`
`crates/context-graph-core/src/types/file_index.rs` -> `super::*`
`crates/context-graph-core/src/types/fingerprint/mod.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `super::constants::`
`crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` -> `super::slice::EmbeddingSlice`
`crates/context-graph-core/src/types/fingerprint/semantic/slice.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/core_tests.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/core_tests.rs` -> `crate::types::fingerprint::semantic::*`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/storage_tests.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/storage_tests.rs` -> `crate::types::fingerprint::semantic::*`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` -> `crate::teleological::EmbedderDims`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` -> `crate::types::fingerprint::semantic::`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/task_core_003_tests.rs` -> `crate::types::fingerprint::semantic::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/validation_tests.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/validation_tests.rs` -> `crate::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/semantic/tests/validation_tests.rs` -> `crate::types::fingerprint::semantic::*`
`crates/context-graph-core/src/types/fingerprint/semantic/validation.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/types/fingerprint/semantic/validation.rs` -> `crate::teleological::Embedder`
`crates/context-graph-core/src/types/fingerprint/semantic/validation.rs` -> `super::constants::`
`crates/context-graph-core/src/types/fingerprint/semantic/validation.rs` -> `super::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/sparse.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/sparse.rs` -> `context_graph_core::types::fingerprint::SparseVector`
`crates/context-graph-core/src/types/fingerprint/sparse.rs` -> `super::*`
`crates/context-graph-core/src/types/fingerprint/teleological/core.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/types/fingerprint/teleological/core.rs` -> `super::types::TeleologicalFingerprint`
`crates/context-graph-core/src/types/fingerprint/teleological/test_helpers.rs` -> `crate::types::fingerprint::SemanticFingerprint`
`crates/context-graph-core/src/types/fingerprint/teleological/tests.rs` -> `super::TeleologicalFingerprint`
`crates/context-graph-core/src/types/fingerprint/teleological/tests.rs` -> `super::test_helpers::`
`crates/context-graph-core/src/types/fingerprint/teleological/types.rs` -> `crate::types::fingerprint::`
`crates/context-graph-core/src/types/graph_edge/edge.rs` -> `crate::types::`
`crates/context-graph-core/src/types/graph_edge/edge.rs` -> `super::*`
`crates/context-graph-core/src/types/graph_edge/modulation.rs` -> `super::edge::GraphEdge`
`crates/context-graph-core/src/types/graph_edge/tests_constructor.rs` -> `super::*`
`crates/context-graph-core/src/types/graph_edge/tests_modulation.rs` -> `super::*`
`crates/context-graph-core/src/types/graph_edge/tests_struct.rs` -> `crate::types::NodeId`
`crates/context-graph-core/src/types/graph_edge/tests_struct.rs` -> `super::*`
`crates/context-graph-core/src/types/graph_edge/tests_traversal.rs` -> `super::*`
`crates/context-graph-core/src/types/graph_edge/traversal.rs` -> `super::edge::GraphEdge`
`crates/context-graph-core/src/types/memory_node/metadata.rs` -> `context_graph_core::types::NodeMetadata`
`crates/context-graph-core/src/types/memory_node/metadata.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/metadata.rs` -> `super::Modality`
`crates/context-graph-core/src/types/memory_node/mod.rs` -> `context_graph_core::types::MemoryNode`
`crates/context-graph-core/src/types/memory_node/mod.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/tests_metadata.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/tests_node.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/tests_serialization.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/validation.rs` -> `context_graph_core::types::ValidationError`
`crates/context-graph-core/src/types/memory_node/validation.rs` -> `super::*`
`crates/context-graph-core/src/types/memory_node/validation.rs` -> `super::DEFAULT_EMBEDDING_DIM`
`crates/context-graph-core/src/types/source_metadata.rs` -> `super::*`
`crates/context-graph-core/src/weights/mod.rs` -> `crate::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-core/src/weights/mod.rs` -> `super::*`
`crates/context-graph-cuda/src/context/green_contexts.rs` -> `context_graph_cuda::context::GreenContextsConfig`
`crates/context-graph-cuda/src/context/green_contexts.rs` -> `crate::error::`
`crates/context-graph-cuda/src/context/green_contexts.rs` -> `crate::safe::device::GpuDevice`
`crates/context-graph-cuda/src/context/green_contexts.rs` -> `super::*`
`crates/context-graph-cuda/src/ffi/cuda_driver.rs` -> `context_graph_cuda::ffi::`
`crates/context-graph-cuda/src/ffi/cuda_driver.rs` -> `context_graph_cuda::ffi::decode_driver_version`
`crates/context-graph-cuda/src/ffi/cuda_driver.rs` -> `super::*`
`crates/context-graph-cuda/src/ffi/faiss.rs` -> `crate::error::`
`crates/context-graph-cuda/src/ffi/faiss.rs` -> `super::*`
`crates/context-graph-cuda/src/ffi/knn.rs` -> `crate::error::`
`crates/context-graph-cuda/src/ffi/knn.rs` -> `crate::ffi::cuda_driver::`
`crates/context-graph-cuda/src/ffi/knn.rs` -> `super::*`
`crates/context-graph-cuda/src/hdbscan/clusterer.rs` -> `super::error::`
`crates/context-graph-cuda/src/hdbscan/clusterer.rs` -> `super::gpu_knn::GpuKnnIndex`
`crates/context-graph-cuda/src/hdbscan/error.rs` -> `super::*`
`crates/context-graph-cuda/src/hdbscan/gpu_knn.rs` -> `crate::ffi::knn::`
`crates/context-graph-cuda/src/hdbscan/gpu_knn.rs` -> `super::error::`
`crates/context-graph-cuda/src/hdbscan/tests.rs` -> `super::*`
`crates/context-graph-cuda/src/ops.rs` -> `crate::error::CudaResult`
`crates/context-graph-cuda/src/safe/device.rs` -> `context_graph_cuda::GpuDevice`
`crates/context-graph-cuda/src/safe/device.rs` -> `crate::error::`
`crates/context-graph-cuda/src/safe/device.rs` -> `crate::ffi::cuda_driver::`
`crates/context-graph-cuda/src/safe/device.rs` -> `super::*`
`crates/context-graph-cuda/src/similarity.rs` -> `crate::error::`
`crates/context-graph-cuda/src/similarity.rs` -> `super::*`
`crates/context-graph-cuda/src/stub.rs` -> `crate::error::`
`crates/context-graph-cuda/src/stub.rs` -> `crate::ops::VectorOps`
`crates/context-graph-cuda/src/stub.rs` -> `super::*`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_core::memory::`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_embeddings::adapters::E7CodeEmbeddingProvider`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-embeddings/src/adapters/code_provider.rs` -> `context_graph_embeddings::provider::ProductionMultiArrayProvider`
`crates/context-graph-embeddings/src/batch/mod.rs` -> `context_graph_embeddings::batch::`
`crates/context-graph-embeddings/src/batch/mod.rs` -> `context_graph_embeddings::config::BatchConfig`
`crates/context-graph-embeddings/src/batch/mod.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/batch/processor/config.rs` -> `crate::config::BatchConfig`
`crates/context-graph-embeddings/src/batch/processor/config.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/batch/processor/config.rs` -> `super::*`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `context_graph_embeddings::batch::BatchProcessorConfig`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `crate::batch::`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `crate::models::ModelRegistry`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/batch/processor/core.rs` -> `super::config::BatchProcessorConfig`
`crates/context-graph-embeddings/src/batch/processor/stats.rs` -> `super::*`
`crates/context-graph-embeddings/src/batch/processor/submit.rs` -> `crate::batch::BatchRequest`
`crates/context-graph-embeddings/src/batch/processor/submit.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/batch/processor/submit.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/batch/processor/submit.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/batch/processor/submit.rs` -> `super::core::BatchProcessor`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `crate::batch::`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `crate::models::ModelRegistry`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `super::*`
`crates/context-graph-embeddings/src/batch/processor/worker.rs` -> `super::stats::BatchProcessorStatsInternal`
`crates/context-graph-embeddings/src/batch/types/batch.rs` -> `context_graph_embeddings::batch::`
`crates/context-graph-embeddings/src/batch/types/batch.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/batch/types/batch.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/batch/types/batch.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/batch/types/batch.rs` -> `super::request::BatchRequest`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `context_graph_embeddings::batch::`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `context_graph_embeddings::config::BatchConfig`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `crate::config::BatchConfig`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/batch/types/queue.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/batch/types/request.rs` -> `context_graph_embeddings::batch::BatchRequest`
`crates/context-graph-embeddings/src/batch/types/request.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/batch/types/request.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/batch/types/request.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/batch/types/stats.rs` -> `context_graph_embeddings::batch::BatchQueueStats`
`crates/context-graph-embeddings/src/batch/types/tests.rs` -> `crate::config::BatchConfig`
`crates/context-graph-embeddings/src/batch/types/tests.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/batch/types/tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/batch/types/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/bin/train_causal.rs` -> `context_graph_embeddings::models::pretrained::`
`crates/context-graph-embeddings/src/bin/train_causal.rs` -> `context_graph_embeddings::training::data::`
`crates/context-graph-embeddings/src/bin/train_causal.rs` -> `context_graph_embeddings::training::lora::LoraConfig`
`crates/context-graph-embeddings/src/bin/train_causal.rs` -> `context_graph_embeddings::training::pipeline::`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `crate::config::CacheConfig`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `crate::types::MultiArrayEmbedding`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `crate::types::dimensions::`
`crates/context-graph-embeddings/src/cache/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/config/batch.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/config/cache.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/config/gpu.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/config/mod.rs` -> `context_graph_embeddings::EmbeddingConfig`
`crates/context-graph-embeddings/src/config/mod.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/config/models.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/config/models.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/error/tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/error/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/error/tests_extended.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/error/tests_extended.rs` -> `super::*`
`crates/context-graph-embeddings/src/error/types.rs` -> `context_graph_embeddings::EmbeddingError`
`crates/context-graph-embeddings/src/error/types.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/global_provider.rs` -> `context_graph_core::traits::MultiArrayEmbeddingProvider`
`crates/context-graph-embeddings/src/global_provider.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/global_provider.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/global_provider.rs` -> `crate::models::pretrained::`
`crates/context-graph-embeddings/src/global_provider.rs` -> `crate::provider::ProductionMultiArrayProvider`
`crates/context-graph-embeddings/src/global_provider.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `context_graph_embeddings::error::EmbeddingError`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `context_graph_embeddings::gpu::require_gpu`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `crate::gpu::GpuInfo`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/device/accessors.rs` -> `super::core::`
`crates/context-graph-embeddings/src/gpu/device/core.rs` -> `context_graph_embeddings::gpu::init_gpu`
`crates/context-graph-embeddings/src/gpu/device/core.rs` -> `crate::gpu::GpuInfo`
`crates/context-graph-embeddings/src/gpu/device/core.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/device/core.rs` -> `super::utils::query_gpu_info`
`crates/context-graph-embeddings/src/gpu/device/mod.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/gpu/device/utils.rs` -> `context_graph_cuda::ffi::`
`crates/context-graph-embeddings/src/gpu/device/utils.rs` -> `crate::gpu::GpuInfo`
`crates/context-graph-embeddings/src/gpu/device/utils.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/budget.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/mod.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/gpu/memory/mod.rs` -> `context_graph_embeddings::gpu::GpuMemoryPool`
`crates/context-graph-embeddings/src/gpu/memory/pool.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/pool.rs` -> `super::error::MemoryError`
`crates/context-graph-embeddings/src/gpu/memory/pool.rs` -> `super::pressure::MemoryPressure`
`crates/context-graph-embeddings/src/gpu/memory/pool.rs` -> `super::stats::MemoryStats`
`crates/context-graph-embeddings/src/gpu/memory/pool.rs` -> `super::tracker::VramTracker`
`crates/context-graph-embeddings/src/gpu/memory/pressure.rs` -> `context_graph_embeddings::gpu::MemoryPressure`
`crates/context-graph-embeddings/src/gpu/memory/pressure.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/slots.rs` -> `context_graph_core::teleological::embedder::Embedder`
`crates/context-graph-embeddings/src/gpu/memory/slots.rs` -> `context_graph_embeddings::gpu::ModelSlotManager`
`crates/context-graph-embeddings/src/gpu/memory/slots.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/slots.rs` -> `super::error::MemoryError`
`crates/context-graph-embeddings/src/gpu/memory/slots.rs` -> `super::pressure::MemoryPressure`
`crates/context-graph-embeddings/src/gpu/memory/stats.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/tracker.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/memory/tracker.rs` -> `super::error::MemoryError`
`crates/context-graph-embeddings/src/gpu/memory/tracker.rs` -> `super::stats::MemoryStats`
`crates/context-graph-embeddings/src/gpu/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/model_loader/batch_loader.rs` -> `super::error::ModelLoadError`
`crates/context-graph-embeddings/src/gpu/model_loader/batch_loader.rs` -> `super::loader::GpuModelLoader`
`crates/context-graph-embeddings/src/gpu/model_loader/batch_loader.rs` -> `super::weights::BertWeights`
`crates/context-graph-embeddings/src/gpu/model_loader/config.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/model_loader/embedding_loader.rs` -> `super::config::BertConfig`
`crates/context-graph-embeddings/src/gpu/model_loader/embedding_loader.rs` -> `super::error::ModelLoadError`
`crates/context-graph-embeddings/src/gpu/model_loader/embedding_loader.rs` -> `super::tensor_utils::`
`crates/context-graph-embeddings/src/gpu/model_loader/embedding_loader.rs` -> `super::weights::`
`crates/context-graph-embeddings/src/gpu/model_loader/error.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/model_loader/layer_loader.rs` -> `super::config::BertConfig`
`crates/context-graph-embeddings/src/gpu/model_loader/layer_loader.rs` -> `super::error::ModelLoadError`
`crates/context-graph-embeddings/src/gpu/model_loader/layer_loader.rs` -> `super::tensor_utils::get_tensor`
`crates/context-graph-embeddings/src/gpu/model_loader/layer_loader.rs` -> `super::weights::`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `context_graph_embeddings::gpu::GpuModelLoader`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `crate::gpu::init_gpu`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `super::batch_loader`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `super::config::BertConfig`
`crates/context-graph-embeddings/src/gpu/model_loader/loader.rs` -> `super::embedding_loader::`
`crates/context-graph-embeddings/src/gpu/model_loader/mod.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/gpu/model_loader/mod.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/gpu/model_loader/tensor_utils.rs` -> `super::error::ModelLoadError`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `context_graph_core::teleological::embedder::Embedder`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `crate::gpu::memory::`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `super::*`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `super::error::ModelLoadError`
`crates/context-graph-embeddings/src/gpu/model_loader/unified.rs` -> `super::loader::GpuModelLoader`
`crates/context-graph-embeddings/src/gpu/model_loader/weights.rs` -> `super::config::BertConfig`
`crates/context-graph-embeddings/src/gpu/ops.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/gpu/tensor/core.rs` -> `context_graph_embeddings::gpu::GpuTensor`
`crates/context-graph-embeddings/src/gpu/tensor/core.rs` -> `context_graph_embeddings::gpu::init_gpu`
`crates/context-graph-embeddings/src/gpu/tensor/core.rs` -> `crate::gpu::device::`
`crates/context-graph-embeddings/src/gpu/tensor/mod.rs` -> `context_graph_embeddings::gpu::GpuTensor`
`crates/context-graph-embeddings/src/gpu/tensor/mod.rs` -> `context_graph_embeddings::gpu::init_gpu`
`crates/context-graph-embeddings/src/gpu/tensor/ops.rs` -> `super::core::GpuTensor`
`crates/context-graph-embeddings/src/learner/mod.rs` -> `context_graph_core::learner::`
`crates/context-graph-embeddings/src/learner/mod.rs` -> `context_graph_core::learner::LEARNER_MODALITY_COUNT`
`crates/context-graph-embeddings/src/learner/mod.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/learner/mod.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/learner/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/attention/dense.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/attention/dense.rs` -> `super::AttentionStrategy`
`crates/context-graph-embeddings/src/models/attention/mod.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/attention/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/attention/sliding_window.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/attention/sliding_window.rs` -> `super::AttentionStrategy`
`crates/context-graph-embeddings/src/models/attention/tiled.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/attention/tiled.rs` -> `super::AttentionStrategy`
`crates/context-graph-embeddings/src/models/custom/hdc/encoding.rs` -> `super::operations::`
`crates/context-graph-embeddings/src/models/custom/hdc/encoding.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `super::encoding::`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `super::operations::`
`crates/context-graph-embeddings/src/models/custom/hdc/model.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/custom/hdc/operations.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/construction.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/embedding.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/encoding.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/hypervector.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/mod.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/mod.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/hdc/tests/operations.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/embed_impl.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/embed_impl.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/embed_impl.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/embed_impl.rs` -> `super::model::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/encoding.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/encoding.rs` -> `super::constants::TEMPORAL_PERIODIC_DIMENSION`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/encoding.rs` -> `super::model::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/model.rs` -> `context_graph_embeddings::models::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/model.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/construction.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/construction.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/edge_cases.rs` -> `super::periodicity::cosine_similarity`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/embedding.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/embedding.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/embedding.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/periodicity.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/periodicity.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/periodicity.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/thread_safety.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/timestamp.rs` -> `crate::models::custom::temporal_periodic::TemporalPeriodicModel`
`crates/context-graph-embeddings/src/models/custom/temporal_periodic/tests/timestamp.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/encoding.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/encoding.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `context_graph_embeddings::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `context_graph_embeddings::models::TemporalPositionalModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/session_signature.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/session_signature.rs` -> `super::constants::SESSION_SIGNATURE_DIMENSION`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/construction.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/construction.rs` -> `super::super::`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/embedding.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/embedding.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/embedding.rs` -> `super::super::TemporalPositionalModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/hybrid.rs` -> `crate::models::custom::temporal_positional::TemporalPositionalModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/hybrid.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/hybrid.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/uniqueness.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/uniqueness.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/tests/uniqueness.rs` -> `super::super::TemporalPositionalModel`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/timestamp.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_positional/timestamp.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/compute.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/constants.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/model.rs` -> `context_graph_embeddings::models::TemporalRecentModel`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/model.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/model.rs` -> `super::constants::DEFAULT_DECAY_RATES`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/tests.rs` -> `crate::models::custom::temporal_recent::`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/tests.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/tests.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/timestamp.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/timestamp.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/custom/temporal_recent/traits.rs` -> `super::compute::compute_decay_embedding`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `context_graph_embeddings::models::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `context_graph_embeddings::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/default_factory.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/factory/mod.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/mod.rs` -> `context_graph_embeddings::models::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/mod.rs` -> `context_graph_embeddings::traits::`
`crates/context-graph-embeddings/src/models/factory/mod.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/factory/tests/factory_tests.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/tests/factory_tests.rs` -> `crate::models::factory::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/tests/factory_tests.rs` -> `crate::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/factory/tests/memory_tests.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/tests/memory_tests.rs` -> `crate::models::factory::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/tests/memory_tests.rs` -> `crate::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/factory/tests/memory_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs` -> `crate::models::factory::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/factory/tests/model_creation_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `context_graph_embeddings::models::DefaultModelFactory`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `context_graph_embeddings::traits::`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/factory/trait_impl.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs` -> `context_graph_embeddings::MemoryTracker`
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/memory_tracker/core.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/memory_tracker/tests/advanced.rs` -> `crate::models::memory_tracker::MemoryTracker`
`crates/context-graph-embeddings/src/models/memory_tracker/tests/advanced.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/memory_tracker/tests/basic.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/models/memory_tracker/tests/basic.rs` -> `crate::models::memory_tracker::MemoryTracker`
`crates/context-graph-embeddings/src/models/memory_tracker/tests/basic.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/mod.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/mod.rs` -> `context_graph_embeddings::models::`
`crates/context-graph-embeddings/src/models/mod.rs` -> `context_graph_embeddings::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/mod.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/attention.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/embeddings.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/embeddings.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/embeddings.rs` -> `super::constants::XLM_R_POSITION_OFFSET`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/embeddings.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs` -> `super::attention::self_attention_forward`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs` -> `super::ffn::ffn_forward`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/encoder.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/ffn.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/ffn.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `super::embeddings::compute_embeddings`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/gpu_forward.rs` -> `super::encoder::encoder_layer_forward`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/layer_norm.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/loader.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs` -> `super::gpu_forward::gpu_forward`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/model.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/pooling.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/pooling.rs` -> `crate::gpu::normalize_gpu`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `crate::types::ImageFormat`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/tests.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/trait_impl.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/trait_impl.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/trait_impl.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/trait_impl.rs` -> `super::types::BgeM3DenseModel`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/types.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/types.rs` -> `crate::models::pretrained::shared::ModelState`
`crates/context-graph-embeddings/src/models/pretrained/bge_m3_dense/types.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/attention.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/attention.rs` -> `super::super::config::NomicConfig`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/attention.rs` -> `super::super::weights::NomicAttentionWeights`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs` -> `super::attention::`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs` -> `super::ops::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/encoder.rs` -> `super::super::weights::`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs` -> `super::config::CAUSAL_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/mod.rs` -> `super::weights::NomicWeights`
`crates/context-graph-embeddings/src/models/pretrained/causal/forward/ops.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs` -> `super::config::NomicConfig`
`crates/context-graph-embeddings/src/models/pretrained/causal/loader.rs` -> `super::weights::`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `context_graph_embeddings::models::CausalModel`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `crate::gpu::init_gpu`
`crates/context-graph-embeddings/src/models/pretrained/causal/model.rs` -> `crate::models::attention::`
`crates/context-graph-embeddings/src/models/pretrained/causal/tests.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/causal/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/causal/weights.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/causal/weights.rs` -> `super::config::NomicConfig`
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs` -> `super::config::QwenConfig`
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs` -> `super::position::`
`crates/context-graph-embeddings/src/models/pretrained/code/attention.rs` -> `super::weights::QwenAttentionWeights`
`crates/context-graph-embeddings/src/models/pretrained/code/config.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/config.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `super::constants::CODE_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `super::layers::`
`crates/context-graph-embeddings/src/models/pretrained/code/forward.rs` -> `super::position::RopeCache`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `crate::models::attention::AttentionStrategy`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `super::attention::gqa_forward`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `super::config::QwenConfig`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `super::position::RopeCache`
`crates/context-graph-embeddings/src/models/pretrained/code/layers.rs` -> `super::weights::`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `context_graph_embeddings::models::CodeModel`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `crate::gpu::init_gpu`
`crates/context-graph-embeddings/src/models/pretrained/code/model.rs` -> `crate::models::attention::`
`crates/context-graph-embeddings/src/models/pretrained/code/position.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests.rs` -> `crate::models::pretrained::code::CodeModel`
`crates/context-graph-embeddings/src/models/pretrained/code/tests.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_batch.rs` -> `crate::models::pretrained::code::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_batch.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_batch.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_edge_cases.rs` -> `crate::models::pretrained::code::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_edge_cases.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/code/tests_edge_cases.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/code/weights.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/code/weights.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/code/weights.rs` -> `super::config::QwenConfig`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `context_graph_embeddings::models::ContextualModel`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/contextual/model.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/contextual/state.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/contextual/state.rs` -> `crate::models::pretrained::shared::ModelState`
`crates/context-graph-embeddings/src/models/pretrained/graph/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/attention.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `super::attention::self_attention_forward`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `super::ffn::ffn_forward`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoder.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoding.rs` -> `context_graph_embeddings::models::GraphModel`
`crates/context-graph-embeddings/src/models/pretrained/graph/encoding.rs` -> `super::constants::MAX_CONTEXT_NEIGHBORS`
`crates/context-graph-embeddings/src/models/pretrained/graph/ffn.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/ffn.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `super::constants::GRAPH_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `super::encoder::encoder_layer_forward`
`crates/context-graph-embeddings/src/models/pretrained/graph/forward.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/graph/layer_norm.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `context_graph_embeddings::models::GraphModel`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/graph/model.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/graph/projections.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/graph/projections.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/graph/projections.rs` -> `super::constants::GRAPH_DIMENSION`
`crates/context-graph-embeddings/src/models/pretrained/graph/state.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/graph/state.rs` -> `super::projections::GraphProjectionWeights`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests.rs` -> `crate::models::pretrained::graph::`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests_edge_cases.rs` -> `crate::models::pretrained::graph::GraphModel`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests_edge_cases.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/pretrained/graph/tests_edge_cases.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/kepler/encoding.rs` -> `context_graph_embeddings::models::pretrained::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/encoding.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/kepler/encoding.rs` -> `super::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs` -> `super::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward.rs` -> `super::types::KEPLER_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs` -> `super::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/forward_batch.rs` -> `super::types::KEPLER_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/model.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/pooling.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/pooling.rs` -> `crate::gpu::BertConfig`
`crates/context-graph-embeddings/src/models/pretrained/kepler/pooling.rs` -> `super::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/kepler/trait_impl.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/trait_impl.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/trait_impl.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/trait_impl.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/kepler/transe.rs` -> `context_graph_embeddings::models::pretrained::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/transe.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/kepler/transe.rs` -> `super::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/transe.rs` -> `super::types::KEPLER_DIMENSION`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `context_graph_embeddings::models::pretrained::KeplerModel`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `crate::models::pretrained::shared::ModelState`
`crates/context-graph-embeddings/src/models/pretrained/kepler/types.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `super::gpu_forward::gpu_forward_tokens`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `super::model::LateInteractionModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/embedding.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention.rs` -> `crate::gpu::AttentionWeights`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention.rs` -> `super::gpu_attention_ops::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_attention_ops.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_encoder.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_encoder.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_encoder.rs` -> `super::gpu_attention::self_attention_forward`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_encoder.rs` -> `super::gpu_utils::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `super::gpu_encoder::run_encoder_layers`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `super::gpu_projection::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_forward.rs` -> `super::gpu_utils::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_projection.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_projection.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_utils.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/gpu_utils.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `context_graph_embeddings::models::LateInteractionModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/model.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/scoring.rs` -> `super::model::LateInteractionModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/scoring.rs` -> `super::types::TokenEmbeddings`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_extended.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_extended.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_extended.rs` -> `super::tests::create_and_load_model`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/tests_token.rs` -> `super::tests::create_and_load_model`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs` -> `super::model::LateInteractionModel`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/trait_impl.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/types.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/late_interaction/types.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/rgcn/mod.rs` -> `context_graph_embeddings::models::pretrained::rgcn::RelationalGCN`
`crates/context-graph-embeddings/src/models/pretrained/rgcn/model.rs` -> `context_graph_embeddings::models::pretrained::rgcn::RelationalGCN`
`crates/context-graph-embeddings/src/models/pretrained/rgcn/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/rgcn/model.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/rgcn/model.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/attention.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/embeddings.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/embeddings.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/semantic/embeddings.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs` -> `super::attention::self_attention_forward`
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs` -> `super::ffn::ffn_forward`
`crates/context-graph-embeddings/src/models/pretrained/semantic/encoder.rs` -> `super::layer_norm::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/semantic/ffn.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/ffn.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `super::constants::SEMANTIC_MAX_TOKENS`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `super::embeddings::compute_embeddings`
`crates/context-graph-embeddings/src/models/pretrained/semantic/gpu_forward.rs` -> `super::encoder::encoder_layer_forward`
`crates/context-graph-embeddings/src/models/pretrained/semantic/layer_norm.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs` -> `super::constants::SEMANTIC_DIMENSION`
`crates/context-graph-embeddings/src/models/pretrained/semantic/loader.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `super::gpu_forward::gpu_forward`
`crates/context-graph-embeddings/src/models/pretrained/semantic/model.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/pooling.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/pooling.rs` -> `crate::gpu::normalize_gpu`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/basic.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/basic.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/basic.rs` -> `super::helpers::create_test_model`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/edge_cases.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/edge_cases.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/edge_cases.rs` -> `super::helpers::create_test_model`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/edge_cases.rs` -> `super::super::SEMANTIC_DIMENSION`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/embedding.rs` -> `crate::types::ModelInput`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/embedding.rs` -> `super::helpers::create_and_load_model`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/embedding.rs` -> `super::super::SEMANTIC_DIMENSION`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/helpers.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/semantic/tests/helpers.rs` -> `super::super::SemanticModel`
`crates/context-graph-embeddings/src/models/pretrained/semantic/trait_impl.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/trait_impl.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/semantic/trait_impl.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/semantic/trait_impl.rs` -> `super::types::SemanticModel`
`crates/context-graph-embeddings/src/models/pretrained/semantic/types.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/semantic/types.rs` -> `crate::models::pretrained::shared::ModelState`
`crates/context-graph-embeddings/src/models/pretrained/semantic/types.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/sparse/attention.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/attention.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/embeddings.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/embeddings.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/sparse/embeddings.rs` -> `super::encoder::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/sparse/encoder.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/encoder.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/encoder.rs` -> `super::attention::self_attention_forward`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `super::embeddings::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `super::mlm_head::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/forward.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/loader.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/loader.rs` -> `crate::gpu::BertConfig`
`crates/context-graph-embeddings/src/models/pretrained/sparse/loader.rs` -> `super::types::MlmHeadWeights`
`crates/context-graph-embeddings/src/models/pretrained/sparse/mlm_head.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/mlm_head.rs` -> `super::encoder::layer_norm`
`crates/context-graph-embeddings/src/models/pretrained/sparse/mlm_head.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `context_graph_embeddings::models::SparseModel`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `crate::gpu::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/model.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_batch.rs` -> `super::error::ProjectionError`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_batch.rs` -> `super::super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_batch.rs` -> `super::types::ProjectionMatrix`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_core.rs` -> `super::error::ProjectionError`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_core.rs` -> `super::super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/impl_core.rs` -> `super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/tests.rs` -> `super::error::ProjectionError`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/tests.rs` -> `super::types::ProjectionMatrix`
`crates/context-graph-embeddings/src/models/pretrained/sparse/projection/types.rs` -> `super::super::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/tests.rs` -> `crate::models::pretrained::sparse::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/tests.rs` -> `crate::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/models/pretrained/sparse/traits.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/pretrained/sparse/traits.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/models/pretrained/sparse/traits.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/pretrained/sparse/traits.rs` -> `super::model::SparseModel`
`crates/context-graph-embeddings/src/models/pretrained/sparse/types.rs` -> `crate::gpu::BertWeights`
`crates/context-graph-embeddings/src/models/pretrained/sparse/types.rs` -> `super::projection::ProjectionMatrix`
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/constants.rs` -> `context_graph_core::clustering::`
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/mod.rs` -> `context_graph_embeddings::models::pretrained::weight_projection::LearnedWeightProjection`
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/model.rs` -> `super::*`
`crates/context-graph-embeddings/src/models/pretrained/weight_projection/model.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/models/registry/config.rs` -> `context_graph_embeddings::models::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/config.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/config.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/registry/config.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `context_graph_embeddings::models::`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `context_graph_embeddings::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/registry/core.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/loader.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/registry/loader.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/registry/loader.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/loader.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/mod.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/registry/mod.rs` -> `context_graph_embeddings::models::`
`crates/context-graph-embeddings/src/models/registry/mod.rs` -> `context_graph_embeddings::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/registry/mod.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `context_graph_embeddings::error::EmbeddingResult`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `context_graph_embeddings::models::`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `context_graph_embeddings::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/registry/operations.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/tests.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/models/registry/tests.rs` -> `crate::traits::`
`crates/context-graph-embeddings/src/models/registry/tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/registry/tests.rs` -> `super::config::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/tests.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/tests_concurrent.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/tests_concurrent.rs` -> `super::config::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/tests_concurrent.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/tests_concurrent.rs` -> `super::tests::TestFactory`
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs` -> `crate::traits::ModelFactory`
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs` -> `super::config::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/tests_mov.rs` -> `super::tests::TestFactory`
`crates/context-graph-embeddings/src/models/registry/tests_stats.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/tests_stats.rs` -> `super::config::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/tests_stats.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/tests_stats.rs` -> `super::tests::TestFactory`
`crates/context-graph-embeddings/src/models/registry/tests_unload.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/models/registry/tests_unload.rs` -> `super::config::ModelRegistryConfig`
`crates/context-graph-embeddings/src/models/registry/tests_unload.rs` -> `super::core::ModelRegistry`
`crates/context-graph-embeddings/src/models/registry/tests_unload.rs` -> `super::tests::TestFactory`
`crates/context-graph-embeddings/src/provider/causal_hint.rs` -> `context_graph_core::traits::`
`crates/context-graph-embeddings/src/provider/causal_hint.rs` -> `super::*`
`crates/context-graph-embeddings/src/provider/diagnostic_test.rs` -> `crate::config::GpuConfig`
`crates/context-graph-embeddings/src/provider/diagnostic_test.rs` -> `crate::provider::multi_array::MultiArrayEmbeddingProvider`
`crates/context-graph-embeddings/src/provider/diagnostic_test.rs` -> `crate::traits::MultiSpaceEmbedder`
`crates/context-graph-embeddings/src/provider/mod.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_core::error::`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_core::traits::`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_core::weights::`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_embeddings::config::GpuConfig`
`crates/context-graph-embeddings/src/provider/multi_array.rs` -> `context_graph_embeddings::provider::ProductionMultiArrayProvider`
`crates/context-graph-embeddings/src/pruning/config.rs` -> `context_graph_embeddings::pruning::ImportanceScoringMethod`
`crates/context-graph-embeddings/src/pruning/config.rs` -> `context_graph_embeddings::pruning::PrunedEmbeddings`
`crates/context-graph-embeddings/src/pruning/config.rs` -> `context_graph_embeddings::pruning::TokenPruningConfig`
`crates/context-graph-embeddings/src/pruning/config.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/pruning/config.rs` -> `super::*`
`crates/context-graph-embeddings/src/pruning/mod.rs` -> `context_graph_embeddings::pruning::`
`crates/context-graph-embeddings/src/pruning/token_pruner.rs` -> `context_graph_embeddings::pruning::`
`crates/context-graph-embeddings/src/pruning/token_pruner.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/pruning/token_pruner.rs` -> `crate::pruning::config::`
`crates/context-graph-embeddings/src/pruning/token_pruner.rs` -> `super::*`
`crates/context-graph-embeddings/src/quantization/binary.rs` -> `super::*`
`crates/context-graph-embeddings/src/quantization/binary.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/edge_case_verification.rs` -> `crate::quantization::`
`crates/context-graph-embeddings/src/quantization/edge_case_verification.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/float8.rs` -> `super::*`
`crates/context-graph-embeddings/src/quantization/float8.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/pq8/encoder.rs` -> `crate::quantization::types::`
`crates/context-graph-embeddings/src/quantization/pq8/encoder.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/pq8/persistence.rs` -> `crate::quantization::types::PQ8Codebook`
`crates/context-graph-embeddings/src/quantization/pq8/persistence.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/pq8/tests/dimension_tests.rs` -> `crate::quantization::pq8::encoder::PQ8Encoder`
`crates/context-graph-embeddings/src/quantization/pq8/tests/dimension_tests.rs` -> `crate::quantization::pq8::training::generate_realistic_embeddings`
`crates/context-graph-embeddings/src/quantization/pq8/tests/dimension_tests.rs` -> `crate::quantization::types::PQ8Codebook`
`crates/context-graph-embeddings/src/quantization/pq8/tests/encoder_tests.rs` -> `crate::quantization::pq8::encoder::PQ8Encoder`
`crates/context-graph-embeddings/src/quantization/pq8/tests/encoder_tests.rs` -> `crate::quantization::pq8::types::PQ8QuantizationError`
`crates/context-graph-embeddings/src/quantization/pq8/tests/encoder_tests.rs` -> `crate::quantization::types::`
`crates/context-graph-embeddings/src/quantization/pq8/tests/persistence_tests.rs` -> `crate::quantization::pq8::encoder::PQ8Encoder`
`crates/context-graph-embeddings/src/quantization/pq8/tests/persistence_tests.rs` -> `crate::quantization::pq8::training::generate_realistic_embeddings`
`crates/context-graph-embeddings/src/quantization/pq8/tests/persistence_tests.rs` -> `crate::quantization::pq8::types::PQ8QuantizationError`
`crates/context-graph-embeddings/src/quantization/pq8/tests/persistence_tests.rs` -> `crate::quantization::types::PQ8Codebook`
`crates/context-graph-embeddings/src/quantization/pq8/tests/training_tests.rs` -> `crate::quantization::pq8::encoder::PQ8Encoder`
`crates/context-graph-embeddings/src/quantization/pq8/tests/training_tests.rs` -> `crate::quantization::pq8::training::generate_realistic_embeddings`
`crates/context-graph-embeddings/src/quantization/pq8/tests/training_tests.rs` -> `crate::quantization::pq8::types::`
`crates/context-graph-embeddings/src/quantization/pq8/tests/training_tests.rs` -> `crate::quantization::types::`
`crates/context-graph-embeddings/src/quantization/pq8/training.rs` -> `crate::quantization::types::PQ8Codebook`
`crates/context-graph-embeddings/src/quantization/pq8/training.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::quantization::binary::BinaryQuantizationError`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::quantization::float8::Float8QuantizationError`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::quantization::pq8::PQ8QuantizationError`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::quantization::types::QuantizedEmbedding`
`crates/context-graph-embeddings/src/quantization/router/encoders.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/mod.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/quantization/router/mod.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/mod.rs` -> `super::float8::Float8E4M3Encoder`
`crates/context-graph-embeddings/src/quantization/router/mod.rs` -> `super::pq8::PQ8Encoder`
`crates/context-graph-embeddings/src/quantization/router/mod.rs` -> `super::types::`
`crates/context-graph-embeddings/src/quantization/router/tests/binary_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/binary_tests.rs` -> `crate::quantization::types::QuantizationMethod`
`crates/context-graph-embeddings/src/quantization/router/tests/binary_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/tests/edge_case_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/quantization/router/tests/edge_case_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/edge_case_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/tests/error_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/quantization/router/tests/error_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/error_tests.rs` -> `crate::quantization::types::`
`crates/context-graph-embeddings/src/quantization/router/tests/error_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/tests/float8_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/float8_tests.rs` -> `crate::quantization::types::QuantizationMethod`
`crates/context-graph-embeddings/src/quantization/router/tests/float8_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/tests/pq8_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/pq8_tests.rs` -> `crate::quantization::types::QuantizationMethod`
`crates/context-graph-embeddings/src/quantization/router/tests/pq8_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/router/tests/routing_tests.rs` -> `crate::quantization::router::QuantizationRouter`
`crates/context-graph-embeddings/src/quantization/router/tests/routing_tests.rs` -> `crate::quantization::types::QuantizationMethod`
`crates/context-graph-embeddings/src/quantization/router/tests/routing_tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/types.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/quantization/types.rs` -> `super::*`
`crates/context-graph-embeddings/src/storage/full_state_verification.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/storage/full_state_verification.rs` -> `crate::storage::multi_space::`
`crates/context-graph-embeddings/src/storage/full_state_verification.rs` -> `crate::storage::types::`
`crates/context-graph-embeddings/src/storage/multi_space.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/storage/multi_space.rs` -> `super::*`
`crates/context-graph-embeddings/src/storage/multi_space.rs` -> `super::types::`
`crates/context-graph-embeddings/src/storage/types/fingerprint.rs` -> `crate::quantization::`
`crates/context-graph-embeddings/src/storage/types/fingerprint.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/storage/types/fingerprint.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/storage/types/query_results.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/storage/types/tests.rs` -> `crate::quantization::`
`crates/context-graph-embeddings/src/storage/types/tests.rs` -> `crate::storage::types::`
`crates/context-graph-embeddings/src/training/data.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/distillation.rs` -> `crate::training::data::TrainingDirection`
`crates/context-graph-embeddings/src/training/distillation.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/distillation.rs` -> `super::data::CausalTrainingPair`
`crates/context-graph-embeddings/src/training/distillation.rs` -> `super::evaluation::EvaluationMetrics`
`crates/context-graph-embeddings/src/training/evaluation.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/evaluation.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/lora.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/lora.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/loss.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/loss.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/multitask.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/multitask.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/optimizer.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/optimizer.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::models::attention::`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::models::pretrained::causal::config::CAUSAL_DIMENSION`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::models::pretrained::causal::forward::`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::models::pretrained::causal::weights::`
`crates/context-graph-embeddings/src/training/pipeline.rs` -> `crate::training::data::`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `crate::models::pretrained::TrainableProjection`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `super::*`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `super::evaluation::EvaluationMetrics`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `super::loss::`
`crates/context-graph-embeddings/src/training/trainer.rs` -> `super::optimizer::`
`crates/context-graph-embeddings/src/traits/embedding_model/mod.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/core_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/core_tests.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/core_tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/core_tests.rs` -> `super::test_model::TestModel`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/integration_tests.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/integration_tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/integration_tests.rs` -> `super::test_model::TestModel`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/test_model.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/test_model.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/traits/embedding_model/tests/test_model.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs` -> `context_graph_embeddings::error::EmbeddingError`
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/traits/embedding_model/trait_def.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/traits/model_factory/config.rs` -> `context_graph_embeddings::traits::`
`crates/context-graph-embeddings/src/traits/model_factory/config.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/traits/model_factory/config.rs` -> `crate::models::attention::AttentionMode`
`crates/context-graph-embeddings/src/traits/model_factory/config.rs` -> `super::*`
`crates/context-graph-embeddings/src/traits/model_factory/device.rs` -> `super::*`
`crates/context-graph-embeddings/src/traits/model_factory/memory.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/traits/model_factory/memory.rs` -> `super::*`
`crates/context-graph-embeddings/src/traits/model_factory/quantization.rs` -> `super::*`
`crates/context-graph-embeddings/src/traits/model_factory/tests.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/traits/model_factory/tests.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/traits/model_factory/tests.rs` -> `crate::traits::model_factory::`
`crates/context-graph-embeddings/src/traits/model_factory/tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `context_graph_embeddings::traits::`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `context_graph_embeddings::traits::SingleModelConfig`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `crate::traits::EmbeddingModel`
`crates/context-graph-embeddings/src/traits/model_factory/trait_def.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/types/concatenated/core.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/types/concatenated/core.rs` -> `crate::types::dimensions::MODEL_COUNT`
`crates/context-graph-embeddings/src/types/concatenated/mod.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/concatenated/operations.rs` -> `crate::error::EmbeddingResult`
`crates/context-graph-embeddings/src/types/concatenated/operations.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/types/concatenated/operations.rs` -> `crate::types::dimensions::MODEL_COUNT`
`crates/context-graph-embeddings/src/types/concatenated/operations.rs` -> `super::MultiArrayEmbedding`
`crates/context-graph-embeddings/src/types/concatenated/tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/concatenated/tests.rs` -> `crate::types::`
`crates/context-graph-embeddings/src/types/concatenated/tests.rs` -> `crate::types::dimensions::`
`crates/context-graph-embeddings/src/types/concatenated/tests.rs` -> `super::MultiArrayEmbedding`
`crates/context-graph-embeddings/src/types/dimensions/aggregates.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/types/dimensions/arrays.rs` -> `super::aggregates::MODEL_COUNT`
`crates/context-graph-embeddings/src/types/dimensions/arrays.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/types/dimensions/arrays.rs` -> `super::helpers::offset_by_index`
`crates/context-graph-embeddings/src/types/dimensions/helpers.rs` -> `context_graph_embeddings::types::dimensions`
`crates/context-graph-embeddings/src/types/dimensions/helpers.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/types/dimensions/mod.rs` -> `context_graph_embeddings::types::dimensions`
`crates/context-graph-embeddings/src/types/dimensions/mod.rs` -> `super::*`
`crates/context-graph-embeddings/src/types/embedding/operations.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/embedding/operations.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/types/embedding/operations.rs` -> `super::ModelEmbedding`
`crates/context-graph-embeddings/src/types/embedding/tensor.rs` -> `context_graph_embeddings::gpu::`
`crates/context-graph-embeddings/src/types/embedding/tensor.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/embedding/tensor.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/types/embedding/tensor.rs` -> `super::ModelEmbedding`
`crates/context-graph-embeddings/src/types/embedding/tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/embedding/tests.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/types/embedding/tests.rs` -> `crate::types::embedding::ModelEmbedding`
`crates/context-graph-embeddings/src/types/embedding/types.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/embedding/types.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/types/embedding/validation.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/types/embedding/validation.rs` -> `super::ModelEmbedding`
`crates/context-graph-embeddings/src/types/input/accessors.rs` -> `context_graph_embeddings::types::ModelInput`
`crates/context-graph-embeddings/src/types/input/image_format.rs` -> `context_graph_embeddings::types::ImageFormat`
`crates/context-graph-embeddings/src/types/input/input_type.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/input/input_type.rs` -> `context_graph_embeddings::types::InputType`
`crates/context-graph-embeddings/src/types/input/input_type.rs` -> `super::ModelInput`
`crates/context-graph-embeddings/src/types/input/model_input.rs` -> `context_graph_embeddings::types::`
`crates/context-graph-embeddings/src/types/input/model_input.rs` -> `context_graph_embeddings::types::ModelInput`
`crates/context-graph-embeddings/src/types/input/model_input.rs` -> `crate::error::`
`crates/context-graph-embeddings/src/types/input/model_input.rs` -> `super::ImageFormat`
`crates/context-graph-embeddings/src/types/input/tests/audio_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/input/tests/audio_tests.rs` -> `crate::types::input::ModelInput`
`crates/context-graph-embeddings/src/types/input/tests/code_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/input/tests/code_tests.rs` -> `crate::types::input::ModelInput`
`crates/context-graph-embeddings/src/types/input/tests/hash_and_size_tests.rs` -> `crate::types::input::`
`crates/context-graph-embeddings/src/types/input/tests/image_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/input/tests/image_tests.rs` -> `crate::types::input::`
`crates/context-graph-embeddings/src/types/input/tests/input_type_tests.rs` -> `crate::types::input::`
`crates/context-graph-embeddings/src/types/input/tests/serde_tests.rs` -> `crate::types::input::`
`crates/context-graph-embeddings/src/types/input/tests/text_tests.rs` -> `crate::error::EmbeddingError`
`crates/context-graph-embeddings/src/types/input/tests/text_tests.rs` -> `crate::types::input::ModelInput`
`crates/context-graph-embeddings/src/types/input/tests/type_tests.rs` -> `crate::types::input::`
`crates/context-graph-embeddings/src/types/model_id/conversions.rs` -> `context_graph_core::teleological::embedder::Embedder`
`crates/context-graph-embeddings/src/types/model_id/conversions.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/types/model_id/conversions.rs` -> `super::*`
`crates/context-graph-embeddings/src/types/model_id/conversions.rs` -> `super::core::ModelId`
`crates/context-graph-embeddings/src/types/model_id/core.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/types/model_id/core.rs` -> `super::tokenizer::TokenizerFamily`
`crates/context-graph-embeddings/src/types/model_id/display.rs` -> `super::core::ModelId`
`crates/context-graph-embeddings/src/types/model_id/repository.rs` -> `context_graph_embeddings::types::ModelId`
`crates/context-graph-embeddings/src/types/model_id/repository.rs` -> `super::core::ModelId`
`crates/context-graph-embeddings/src/types/model_id/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/config.rs` -> `context_graph_embeddings::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/config.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/config.rs` -> `super::error::WarmError`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocation.rs` -> `super::constants::GB`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs` -> `super::gpu_info::GpuInfo`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `context_graph_cuda::ffi::`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `super::allocation::VramAllocation`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `super::allocator::WarmCudaAllocator`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` -> `super::gpu_info::GpuInfo`
`crates/context-graph-embeddings/src/warm/cuda_alloc/gpu_info.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/warm/cuda_alloc/helpers.rs` -> `super::constants::GB`
`crates/context-graph-embeddings/src/warm/cuda_alloc/tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/cuda_alloc/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/diagnostics/helpers.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/diagnostics/mod.rs` -> `context_graph_embeddings::warm::`
`crates/context-graph-embeddings/src/warm/diagnostics/mod.rs` -> `context_graph_embeddings::warm::diagnostics::WarmDiagnostics`
`crates/context-graph-embeddings/src/warm/diagnostics/output.rs` -> `super::helpers::format_bytes`
`crates/context-graph-embeddings/src/warm/diagnostics/output.rs` -> `super::types::WarmDiagnosticReport`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `crate::warm::loader::WarmLoader`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `super::helpers::format_bytes`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `super::output::dump_report_to_stderr`
`crates/context-graph-embeddings/src/warm/diagnostics/service.rs` -> `super::types::`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::cuda_alloc::GpuInfo`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::loader::WarmLoader`
`crates/context-graph-embeddings/src/warm/diagnostics/tests.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/diagnostics/types.rs` -> `crate::warm::cuda_alloc::GpuInfo`
`crates/context-graph-embeddings/src/warm/diagnostics/types.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/diagnostics/types.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/diagnostics/types.rs` -> `super::helpers::current_timestamp`
`crates/context-graph-embeddings/src/warm/handle.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/health/check.rs` -> `super::status::WarmHealthStatus`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `crate::warm::loader::WarmLoader`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `crate::warm::registry::SharedWarmRegistry`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `super::check::WarmHealthCheck`
`crates/context-graph-embeddings/src/warm/health/checker.rs` -> `super::status::WarmHealthStatus`
`crates/context-graph-embeddings/src/warm/health/mod.rs` -> `context_graph_embeddings::warm::WarmLoader`
`crates/context-graph-embeddings/src/warm/health/mod.rs` -> `context_graph_embeddings::warm::health::`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `crate::warm::loader::WarmLoader`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/health/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/health/tests_additional.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/inference/engine.rs` -> `crate::gpu::GpuTensor`
`crates/context-graph-embeddings/src/warm/inference/engine.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/inference/engine.rs` -> `crate::warm::loader::types::LoadedModelWeights`
`crates/context-graph-embeddings/src/warm/inference/engine.rs` -> `crate::warm::validation::TestInput`
`crates/context-graph-embeddings/src/warm/inference/engine.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/inference/validation.rs` -> `crate::warm::cuda_alloc::`
`crates/context-graph-embeddings/src/warm/inference/validation.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/inference/validation.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/integration/mod.rs` -> `context_graph_embeddings::warm::`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::diagnostics::`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::health::`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::loader::WarmLoader`
`crates/context-graph-embeddings/src/warm/integration/pipeline.rs` -> `crate::warm::registry::SharedWarmRegistry`
`crates/context-graph-embeddings/src/warm/integration/tests/advanced.rs` -> `crate::warm::health::WarmHealthStatus`
`crates/context-graph-embeddings/src/warm/integration/tests/advanced.rs` -> `crate::warm::integration::WarmEmbeddingPipeline`
`crates/context-graph-embeddings/src/warm/integration/tests/advanced.rs` -> `super::test_config`
`crates/context-graph-embeddings/src/warm/integration/tests/basic.rs` -> `crate::warm::health::`
`crates/context-graph-embeddings/src/warm/integration/tests/basic.rs` -> `crate::warm::integration::WarmEmbeddingPipeline`
`crates/context-graph-embeddings/src/warm/integration/tests/basic.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs` -> `crate::warm::health::WarmHealthChecker`
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs` -> `crate::warm::integration::WarmEmbeddingPipeline`
`crates/context-graph-embeddings/src/warm/integration/tests/mod.rs` -> `crate::warm::registry::EMBEDDING_MODEL_IDS`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::cuda_alloc::`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/loader/engine.rs` -> `crate::warm::validation::WarmValidator`
`crates/context-graph-embeddings/src/warm/loader/helpers.rs` -> `super::constants::GB`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::types::ModelId`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::warm::cuda_alloc::WarmCudaAllocator`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/loader/operations.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/loader/preflight.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/loader/preflight.rs` -> `crate::warm::cuda_alloc::`
`crates/context-graph-embeddings/src/warm/loader/preflight.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/loader/preflight.rs` -> `super::constants::`
`crates/context-graph-embeddings/src/warm/loader/preflight.rs` -> `super::helpers::format_bytes`
`crates/context-graph-embeddings/src/warm/loader/summary.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/loader/summary.rs` -> `super::helpers::format_bytes`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::cuda_alloc::`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::loader::operations::load_weights`
`crates/context-graph-embeddings/src/warm/loader/tests/loader_tests.rs` -> `crate::warm::loader::operations::verify_checksum`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `crate::warm::registry::EMBEDDING_MODEL_IDS`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `super::super::constants::`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `super::super::engine::WarmLoader`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `super::super::helpers::format_bytes`
`crates/context-graph-embeddings/src/warm/loader/tests/summary_tests.rs` -> `super::super::summary::LoadingSummary`
`crates/context-graph-embeddings/src/warm/loader/types/inference.rs` -> `context_graph_embeddings::warm::loader::types::InferenceValidation`
`crates/context-graph-embeddings/src/warm/loader/types/metadata.rs` -> `context_graph_embeddings::warm::loader::types::`
`crates/context-graph-embeddings/src/warm/loader/types/metadata.rs` -> `context_graph_embeddings::warm::loader::types::TensorMetadata`
`crates/context-graph-embeddings/src/warm/loader/types/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/loader/types/vram.rs` -> `context_graph_embeddings::warm::loader::types::VramAllocationTracking`
`crates/context-graph-embeddings/src/warm/loader/types/weights.rs` -> `context_graph_embeddings::gpu::GpuTensor`
`crates/context-graph-embeddings/src/warm/loader/types/weights.rs` -> `context_graph_embeddings::warm::loader::types::LoadedModelWeights`
`crates/context-graph-embeddings/src/warm/loader/types/weights.rs` -> `crate::gpu::GpuTensor`
`crates/context-graph-embeddings/src/warm/memory_pool/mod.rs` -> `context_graph_embeddings::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/memory_pool/model_pool.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/memory_pool/model_pool.rs` -> `super::types::ModelAllocation`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `context_graph_embeddings::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `crate::warm::error::WarmResult`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `super::model_pool::ModelMemoryPool`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `super::types::ModelAllocation`
`crates/context-graph-embeddings/src/warm/memory_pool/pools.rs` -> `super::working_pool::WorkingMemoryPool`
`crates/context-graph-embeddings/src/warm/memory_pool/tests.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/memory_pool/tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/memory_pool/tests.rs` -> `super::pools::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/memory_pool/working_pool.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/registry/core.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/registry/core.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/core.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/core.rs` -> `super::types::WarmModelEntry`
`crates/context-graph-embeddings/src/warm/registry/mod.rs` -> `context_graph_embeddings::warm::registry::`
`crates/context-graph-embeddings/src/warm/registry/operations.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/registry/operations.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/operations.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/operations.rs` -> `super::core::WarmModelRegistry`
`crates/context-graph-embeddings/src/warm/registry/queries.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/queries.rs` -> `super::core::WarmModelRegistry`
`crates/context-graph-embeddings/src/warm/registry/tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/registry/tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/tests.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/tests.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/registry/tests_errors.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/registry/tests_errors.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/tests_errors.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/tests_errors.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/registry/tests_queries.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/tests_queries.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/registry/tests_transitions.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/registry/tests_transitions.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/tests_transitions.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/tests_transitions.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/registry/types.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/registry/types.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/registry/types.rs` -> `super::core::WarmModelRegistry`
`crates/context-graph-embeddings/src/warm/state.rs` -> `super::*`
`crates/context-graph-embeddings/src/warm/tests/config_tests.rs` -> `crate::warm::config::`
`crates/context-graph-embeddings/src/warm/tests/config_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/config_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/diagnostics_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/diagnostics_tests.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/tests/diagnostics_tests.rs` -> `crate::warm::registry::WarmModelRegistry`
`crates/context-graph-embeddings/src/warm/tests/diagnostics_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/error_tests.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/tests/error_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/handle_tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/tests/handle_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/health_tests.rs` -> `crate::warm::registry::WarmModelRegistry`
`crates/context-graph-embeddings/src/warm/tests/health_tests.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/tests/health_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/helpers.rs` -> `crate::warm::config::WarmConfig`
`crates/context-graph-embeddings/src/warm/tests/helpers.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs` -> `crate::warm::validation::`
`crates/context-graph-embeddings/src/warm/tests/integration_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/loader_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/loader_tests.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/tests/loader_tests.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/tests/loader_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/memory_pool_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/memory_pool_tests.rs` -> `crate::warm::memory_pool::WarmMemoryPools`
`crates/context-graph-embeddings/src/warm/tests/memory_pool_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/registry_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/registry_tests.rs` -> `crate::warm::registry::`
`crates/context-graph-embeddings/src/warm/tests/registry_tests.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/tests/registry_tests.rs` -> `super::helpers::`
`crates/context-graph-embeddings/src/warm/tests/state_tests.rs` -> `crate::warm::state::WarmModelState`
`crates/context-graph-embeddings/src/warm/tests/validation_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/tests/validation_tests.rs` -> `crate::warm::validation::`
`crates/context-graph-embeddings/src/warm/tests/validation_tests.rs` -> `super::helpers::test_handle_full`
`crates/context-graph-embeddings/src/warm/validation/comparisons.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/validation/result.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/validation/tests/config_tests.rs` -> `crate::warm::validation::`
`crates/context-graph-embeddings/src/warm/validation/tests/model_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/validation/tests/model_tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/validation/tests/model_tests.rs` -> `crate::warm::validation::`
`crates/context-graph-embeddings/src/warm/validation/tests/result_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/validation/tests/result_tests.rs` -> `crate::warm::validation::ValidationResult`
`crates/context-graph-embeddings/src/warm/validation/tests/validator_tests.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/validation/tests/validator_tests.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/validation/tests/validator_tests.rs` -> `crate::warm::validation::`
`crates/context-graph-embeddings/src/warm/validation/validator.rs` -> `crate::warm::error::`
`crates/context-graph-embeddings/src/warm/validation/validator.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/validation/validator.rs` -> `super::comparisons::compare_output_impl`
`crates/context-graph-embeddings/src/warm/validation/validator.rs` -> `super::config::TestInferenceConfig`
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs` -> `crate::warm::error::WarmError`
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs` -> `crate::warm::handle::ModelHandle`
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs` -> `super::config::TestInferenceConfig`
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs` -> `super::result::ValidationResult`
`crates/context-graph-embeddings/src/warm/validation/validator_model.rs` -> `super::validator::WarmValidator`
`crates/context-graph-graph-agent/src/activator/mod.rs` -> `context_graph_embeddings::models::GraphModel`
`crates/context-graph-graph-agent/src/activator/mod.rs` -> `crate::error::`
`crates/context-graph-graph-agent/src/activator/mod.rs` -> `crate::types::`
`crates/context-graph-graph-agent/src/activator/mod.rs` -> `super::*`
`crates/context-graph-graph-agent/src/lib.rs` -> `context_graph_causal_agent::CausalDiscoveryLLM`
`crates/context-graph-graph-agent/src/lib.rs` -> `context_graph_core::types::`
`crates/context-graph-graph-agent/src/llm/mod.rs` -> `context_graph_causal_agent::CausalDiscoveryLLM`
`crates/context-graph-graph-agent/src/llm/mod.rs` -> `context_graph_causal_agent::llm::GrammarType`
`crates/context-graph-graph-agent/src/llm/mod.rs` -> `crate::error::`
`crates/context-graph-graph-agent/src/llm/mod.rs` -> `crate::types::`
`crates/context-graph-graph-agent/src/llm/mod.rs` -> `super::*`
`crates/context-graph-graph-agent/src/llm/prompt.rs` -> `crate::B", "from B import")`
`crates/context-graph-graph-agent/src/llm/prompt.rs` -> `crate::foo`
`crates/context-graph-graph-agent/src/llm/prompt.rs` -> `crate::module`
`crates/context-graph-graph-agent/src/llm/prompt.rs` -> `crate::types::`
`crates/context-graph-graph-agent/src/llm/prompt.rs` -> `super::*`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `crate::error::GraphAgentResult`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `crate::foo`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `crate::module`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `crate::types::`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `crate::types::ContentDomain`
`crates/context-graph-graph-agent/src/scanner/mod.rs` -> `super::*`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `context_graph_causal_agent::CausalDiscoveryLLM`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `context_graph_core::types::DiscoveryCycleResult`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `context_graph_core::types::ServiceStatus`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `context_graph_embeddings::models::GraphModel`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `crate::activator::`
`crates/context-graph-graph-agent/src/service/mod.rs` -> `crate::error::`
`crates/context-graph-graph-agent/src/stubs.rs` -> `context_graph_graph_agent::stubs::create_stub_graph_discovery_service`
`crates/context-graph-graph-agent/src/stubs.rs` -> `crate::service::`
`crates/context-graph-graph-agent/src/stubs.rs` -> `super::*`
`crates/context-graph-graph-agent/src/types/mod.rs` -> `crate::foo`
`crates/context-graph-graph-agent/src/types/mod.rs` -> `crate::module`
`crates/context-graph-graph-agent/src/types/mod.rs` -> `super::*`
`crates/context-graph-mcp/src/adapters/causal_hint.rs` -> `context_graph_causal_agent::CausalDiscoveryLLM`
`crates/context-graph-mcp/src/adapters/causal_hint.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/adapters/causal_hint.rs` -> `context_graph_embeddings::provider::`
`crates/context-graph-mcp/src/adapters/causal_hint.rs` -> `context_graph_mcp::adapters::LlmCausalHintProvider`
`crates/context-graph-mcp/src/adapters/causal_hint.rs` -> `super::*`
`crates/context-graph-mcp/src/adapters/lazy_provider.rs` -> `context_graph_core::error::`
`crates/context-graph-mcp/src/adapters/lazy_provider.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/adapters/lazy_provider.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-mcp/src/adapters/lazy_provider.rs` -> `context_graph_core::weights::E11_ENTITY_ENABLED`
`crates/context-graph-mcp/src/adapters/lazy_provider.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/core/dispatch.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/core/dispatch.rs` -> `super::handlers::Handlers`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_core::clustering::`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_core::memory::`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_core::monitoring::LayerStatusProvider`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_embeddings::models::CausalModel`
`crates/context-graph-mcp/src/handlers/core/handlers.rs` -> `context_graph_graph_agent::GraphDiscoveryService`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `context_graph_core::types::audit::MergeRecord`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `context_graph_core::types::fingerprint::`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `crate::handlers::tools::helpers::ToolErrorKind`
`crates/context-graph-mcp/src/handlers/merge.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tests/error_codes.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tests/initialize.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/mcp_protocol_e2e_test.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/mcp_protocol_e2e_test.rs` -> `super::create_test_handlers_with_real_embeddings_store_access`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `context_graph_core::monitoring::`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `context_graph_graph_agent::create_stub_graph_discovery_service`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `crate::handlers::Handlers`
`crates/context-graph-mcp/src/handlers/tests/mod.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tests/search_periodic_test.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/tcp_transport_integration.rs` -> `context_graph_core::config::Config`
`crates/context-graph-mcp/src/handlers/tests/tcp_transport_integration.rs` -> `context_graph_core::config::McpConfig`
`crates/context-graph-mcp/src/handlers/tests/tcp_transport_integration.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tests/tcp_transport_integration.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/tools_call.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/tools_list.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tests/topic_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_core::types::SourceMetadata`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_core::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-mcp/src/handlers/tools/causal_discovery_tools.rs` -> `context_graph_graph_agent::MemoryForGraphAnalysis`
`crates/context-graph-mcp/src/handlers/tools/causal_dtos.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-mcp/src/handlers/tools/causal_dtos.rs` -> `context_graph_core::causal::chain::HOP_ATTENUATION`
`crates/context-graph-mcp/src/handlers/tools/causal_dtos.rs` -> `context_graph_core::traits::SearchStrategy`
`crates/context-graph-mcp/src/handlers/tools/causal_dtos.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs` -> `context_graph_core::types::MultiEmbedderConfig`
`crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/causal_relationship_tools.rs` -> `super::super::Handlers`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `context_graph_core::causal::chain::rank_causes_by_abduction`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `context_graph_core::causal::chain::rank_effects_by_prediction`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `context_graph_core::types::fingerprint::SemanticFingerprint`
`crates/context-graph-mcp/src/handlers/tools/causal_tools.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tools/code_dtos.rs` -> `context_graph_core::traits::SearchStrategy`
`crates/context-graph-mcp/src/handlers/tools/code_dtos.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `crate::protocol::JsonRpcId`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `crate::protocol::JsonRpcResponse`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `super::code_dtos::`
`crates/context-graph-mcp/src/handlers/tools/code_tools.rs` -> `super::helpers::`
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs` -> `context_graph_core::causal::asymmetric::`
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs` -> `crate::handlers::Handlers`
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs` -> `crate::handlers::tools::helpers::cosine_similarity`
`crates/context-graph-mcp/src/handlers/tools/consolidation.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `context_graph_core::constellation::`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `context_graph_core::teleological::synergy_matrix::SynergyMatrix`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `context_graph_core::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `crate::handlers::Handlers`
`crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `context_graph_core::contrastive::`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `context_graph_core::teleological::types::NUM_EMBEDDERS`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `context_graph_core::types::fingerprint::TeleologicalFingerprint`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `context_graph_storage::teleological::RocksDbTeleologicalStore`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `crate::handlers::Handlers`
`crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `context_graph_core::types::audit::ImportanceChangeRecord`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `super::curation_dtos::`
`crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` -> `super::helpers::ToolErrorKind`
`crates/context-graph-mcp/src/handlers/tools/daemon_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/daemon_tools.rs` -> `super::super::Handlers`
`crates/context-graph-mcp/src/handlers/tools/dispatch.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/dispatch.rs` -> `crate::tools::`
`crates/context-graph-mcp/src/handlers/tools/dispatch.rs` -> `super::super::Handlers`
`crates/context-graph-mcp/src/handlers/tools/embedder_dtos.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `context_graph_core::causal::asymmetric::CausalDirection`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `context_graph_core::teleological::Embedder`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `context_graph_core::types::audit::`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `context_graph_core::types::fingerprint::NUM_EMBEDDERS`
`crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` -> `crate::protocol::`
`crates/context-graph-mcp/src/handlers/tools/entity_dtos.rs` -> `context_graph_core::entity::`
`crates/context-graph-mcp/src/handlers/tools/entity_dtos.rs` -> `context_graph_core::traits::SearchStrategy`
`crates/context-graph-mcp/src/handlers/tools/entity_dtos.rs` -> `super::*`
`crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` -> `context_graph_core::entity::`
`crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` -> `context_graph_core::traits::`
`crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` -> `context_graph_embeddings::models::KeplerModel`
`crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` -> `crate::protocol::JsonRpcId`

## 5. Build Configuration
| Item | Value |
| --- | --- |
| Workspace packages | 10 |
| Default workspace members | 9 |
| Release profile | thin LTO, codegen-units=1, strip=true |
| MCP defaults | cuda,llm |
| CLI defaults | cuda |
| Benchmark crate | workspace member, not default member |
