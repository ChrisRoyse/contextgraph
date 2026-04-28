# System Overview

**Source files covered:**
- `Cargo.toml`
- `crates/*/Cargo.toml`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-mcp/src/tools/definitions/mod.rs`
- `crates/context-graph-storage/src/column_families.rs`
- `crates/context-graph-core/src/types/fingerprint/semantic/constants.rs`
- `scripts/v4_2/*.py`

## 1. Summary
Context Graph is a Rust workspace that stores memory records as multi-embedding fingerprints, persists them in RocksDB, and exposes MCP JSON-RPC tools plus a local CLI. Current source defines 14 production embedder slots, 73 RocksDB column families, and 100 MCP tools with the `llm` feature (96 without it). Python CAMD-DPO scripts under `scripts/v4_2` call the daemon and optional LMDB sidecars.

## 2. Architecture
| Process | Port/address | Technology | Purpose |
| --- | --- | --- | --- |
| context-graph-mcp | stdio or TCP 3100; SSE rejected by main | tokio JSON-RPC | MCP tools/list and tools/call |
| context-graph-cli | local process | Rust clap | Operator commands |
| RocksDB | storage.path or CONTEXT_GRAPH_STORAGE_PATH | RocksDB CFs | Durable memory/graph/training/learner data |
| Embedding warm provider | in-process GPU | Candle/CUDA | Loads 14 production embedders by default |
| CAMD scripts | local Python | Python RPC + optional LMDB | Mine DPO data |

## 3. Technology Stack
| Layer | Technology | Source |
| --- | --- | --- |
| Rust | edition 2021, workspace rust-version 1.75 | Cargo.toml |
| Async/MCP | tokio 1.35 full, JSON-RPC 2.0 | Cargo.toml; mcp main |
| Storage | rocksdb 0.22, usearch 2 | Cargo.toml |
| Embeddings | candle 0.9.2-alpha, llama-cpp-2 optional | Cargo.toml |
| Search | hnsw_rs 0.3, RRF k=60 | Cargo.toml; retrieval/aggregation.rs |
| Parsing | tree-sitter 0.23, tree-sitter-rust 0.23 | Cargo.toml |
| Serialization | serde, serde_json, bincode, toml | Cargo.toml |

## 4. Workspace Crates
| Package | Version | Default features | Targets | Manifest |
| --- | --- | --- | --- | --- |
| context-graph-mcp | 0.1.0 | cuda,llm | context_graph_mcp,context-graph-mcp | crates/context-graph-mcp/Cargo.toml |
| context-graph-causal-agent | 0.1.0 | [] | context_graph_causal_agent | crates/context-graph-causal-agent/Cargo.toml |
| context-graph-core | 0.1.0 | [] | context_graph_core | crates/context-graph-core/Cargo.toml |
| context-graph-embeddings | 0.1.0 | candle | context_graph_embeddings,train-causal | crates/context-graph-embeddings/Cargo.toml |
| context-graph-cuda | 0.1.0 | cuda | context_graph_cuda | crates/context-graph-cuda/Cargo.toml |
| context-graph-storage | 0.1.0 | cuda | context_graph_storage | crates/context-graph-storage/Cargo.toml |
| context-graph-test-utils | 0.1.0 | [] | context_graph_test_utils | crates/context-graph-test-utils/Cargo.toml |
| context-graph-graph-agent | 0.1.0 | [] | context_graph_graph_agent | crates/context-graph-graph-agent/Cargo.toml |
| context-graph-cli | 0.1.0 | cuda | context_graph_cli,context-graph-cli | crates/context-graph-cli/Cargo.toml |
| context-graph-benchmark | 0.1.0 | [] | context_graph_benchmark,benchmark-report,causal-accuracy-bench,causal-discovery-mcp-bench,causal-e2e-bench,causal-embedding-bench,causal-perf-bench,causal-prompt-experiment,causal-provenance-bench,causal-provenance-e2e-bench,causal-provenance-llm-bench,causal-realdata-bench,causal-span-fix-bench,code-fingerprint-bench,disk-space-bench,e1-semantic-bench,e10_modifier_tuning,e11-entity-bench,e2-manual-verification,e4-hybrid-session-bench,e7-bench,e7-realdata-bench,embedder-impact-bench,embedder-impact-realdata-bench,embedder-stress,gpu-bench,graph-bench,graph-linking-bench,graph-linking-gpu-bench,graph-linking-real-bench,graph-realdata-bench,hf-bench,mcp-bench,model-injection-bench,model-injection-rigorous-bench,multimodal-bench,multimodal-realdata-bench,realdata-bench,sparse-bench,spawn-blocking-bench,temporal-bench,temporal-realdata-bench,unified-realdata-bench,validation-bench,validation-realdata-bench,weight-profile-bench | crates/context-graph-benchmark/Cargo.toml |

## 5. Public APIs And Tools
| Domain | Count | Source |
| --- | --- | --- |
| core | 4 |
| merge | 1 |
| curation | 2 |
| topic | 4 |
| file_watcher | 4 |
| sequence | 4 |
| causal | 4 |
| keyword | 1 |
| code | 1 |
| robustness | 1 |
| entity | 6 |
| embedder | 7 |
| temporal | 2 |
| graph_link | 4 |
| maintenance | 1 |
| provenance | 3 |
| daemon | 1 |
| training | 4 |
| learning | 8 |
| utl | 17 |
| constellation | 5 |
| contrastive | 4 |
| typed_edges | 4 |
| graph | 4 |
| causal_discovery | 2 |
| graph | 2 |
| causal_discovery | 0 |
| causal_discovery | 2 with llm / 0 without |
| graph | 4 with llm / 2 without |

## 6. Entry Points
| Entry | Kind | Source | Required features |
| --- | --- | --- | --- |
| context-graph-mcp | bin | crates/context-graph-mcp/src/main.rs |  |
| benchmark_causal | example | crates/context-graph-causal-agent/examples/benchmark_causal.rs |  |
| benchmark_causal_enhanced | example | crates/context-graph-causal-agent/examples/benchmark_causal_enhanced.rs |  |
| benchmark_causal_large | example | crates/context-graph-causal-agent/examples/benchmark_causal_large.rs |  |
| test_llm | example | crates/context-graph-causal-agent/examples/test_llm.rs |  |
| divergence_fsv_test | example | crates/context-graph-core/examples/divergence_fsv_test.rs |  |
| fsv_line_numbers | example | crates/context-graph-core/examples/fsv_line_numbers.rs |  |
| fsv_source_metadata | example | crates/context-graph-core/examples/fsv_source_metadata.rs |  |
| tree_sitter_test | example | crates/context-graph-core/examples/tree_sitter_test.rs |  |
| train-causal | bin | crates/context-graph-embeddings/src/bin/train_causal.rs |  |
| demo_typed_edge_capabilities | example | crates/context-graph-storage/examples/demo_typed_edge_capabilities.rs |  |
| fsv_typed_edges_physical | example | crates/context-graph-storage/examples/fsv_typed_edges_physical.rs |  |
| causal_relationships_bench | bench | crates/context-graph-storage/benches/causal_relationships_bench.rs |  |
| e11_quality_bench | bench | crates/context-graph-storage/benches/e11_quality_bench.rs |  |
| hybrid_effectiveness_bench | bench | crates/context-graph-storage/benches/hybrid_effectiveness_bench.rs |  |
| maxsim_bench | bench | crates/context-graph-storage/benches/maxsim_bench.rs |  |
| benchmark_graph | example | crates/context-graph-graph-agent/examples/benchmark_graph.rs | llm |
| benchmark_graph_code | example | crates/context-graph-graph-agent/examples/benchmark_graph_code.rs | llm |
| benchmark_graph_multi | example | crates/context-graph-graph-agent/examples/benchmark_graph_multi.rs | llm |
| context-graph-cli | bin | crates/context-graph-cli/src/main.rs |  |
| cli_benchmarks | bench | crates/context-graph-cli/benches/cli_benchmarks.rs |  |
| benchmark-report | bin | crates/context-graph-benchmark/src/bin/report.rs |  |
| causal-accuracy-bench | bin | crates/context-graph-benchmark/src/bin/causal_accuracy_bench.rs |  |
| causal-discovery-mcp-bench | bin | crates/context-graph-benchmark/src/bin/causal_discovery_mcp_bench.rs | real-embeddings |
| causal-e2e-bench | bin | crates/context-graph-benchmark/src/bin/causal_e2e_bench.rs |  |
| causal-embedding-bench | bin | crates/context-graph-benchmark/src/bin/causal_embedding_bench.rs |  |
| causal-perf-bench | bin | crates/context-graph-benchmark/src/bin/causal_perf_bench.rs |  |
| causal-prompt-experiment | bin | crates/context-graph-benchmark/src/bin/causal_prompt_experiment_bench.rs | real-embeddings |
| causal-provenance-bench | bin | crates/context-graph-benchmark/src/bin/causal_provenance_bench.rs |  |
| causal-provenance-e2e-bench | bin | crates/context-graph-benchmark/src/bin/causal_provenance_e2e_bench.rs |  |
| causal-provenance-llm-bench | bin | crates/context-graph-benchmark/src/bin/causal_provenance_llm_bench.rs | real-embeddings |
| causal-realdata-bench | bin | crates/context-graph-benchmark/src/bin/causal_realdata_bench.rs |  |
| causal-span-fix-bench | bin | crates/context-graph-benchmark/src/bin/causal_span_fix_bench.rs | real-embeddings |
| code-fingerprint-bench | bin | crates/context-graph-benchmark/src/bin/code_fingerprint_bench.rs | real-embeddings |
| disk-space-bench | bin | crates/context-graph-benchmark/src/bin/disk_space_bench.rs |  |
| e1-semantic-bench | bin | crates/context-graph-benchmark/src/bin/e1_semantic_bench.rs |  |
| e10_modifier_tuning | bin | crates/context-graph-benchmark/src/bin/e10_modifier_tuning.rs |  |
| e11-entity-bench | bin | crates/context-graph-benchmark/src/bin/e11_entity_bench.rs |  |
| e2-manual-verification | bin | crates/context-graph-benchmark/src/bin/e2_manual_verification.rs | real-embeddings |
| e4-hybrid-session-bench | bin | crates/context-graph-benchmark/src/bin/e4_hybrid_session_bench.rs |  |
| e7-bench | bin | crates/context-graph-benchmark/src/bin/e7_bench.rs |  |
| e7-realdata-bench | bin | crates/context-graph-benchmark/src/bin/e7_realdata_bench.rs | real-embeddings |
| embedder-impact-bench | bin | crates/context-graph-benchmark/src/bin/embedder_impact_bench.rs |  |
| embedder-impact-realdata-bench | bin | crates/context-graph-benchmark/src/bin/embedder_impact_realdata_bench.rs |  |
| embedder-stress | bin | crates/context-graph-benchmark/src/bin/embedder_stress.rs |  |
| gpu-bench | bin | crates/context-graph-benchmark/src/bin/gpu_bench.rs |  |
| graph-bench | bin | crates/context-graph-benchmark/src/bin/graph_bench.rs |  |
| graph-linking-bench | bin | crates/context-graph-benchmark/src/bin/graph_linking_bench.rs |  |
| graph-linking-gpu-bench | bin | crates/context-graph-benchmark/src/bin/graph_linking_gpu_bench.rs | real-embeddings |
| graph-linking-real-bench | bin | crates/context-graph-benchmark/src/bin/graph_linking_real_bench.rs |  |
| graph-realdata-bench | bin | crates/context-graph-benchmark/src/bin/graph_realdata_bench.rs |  |
| hf-bench | bin | crates/context-graph-benchmark/src/bin/hf_bench.rs |  |
| mcp-bench | bin | crates/context-graph-benchmark/src/bin/mcp_bench.rs |  |
| model-injection-bench | bin | crates/context-graph-benchmark/src/bin/model_injection_bench.rs | real-embeddings |
| model-injection-rigorous-bench | bin | crates/context-graph-benchmark/src/bin/model_injection_rigorous_bench.rs | real-embeddings |
| multimodal-bench | bin | crates/context-graph-benchmark/src/bin/multimodal_bench.rs |  |
| multimodal-realdata-bench | bin | crates/context-graph-benchmark/src/bin/multimodal_realdata_bench.rs |  |
| realdata-bench | bin | crates/context-graph-benchmark/src/bin/realdata_bench.rs |  |
| sparse-bench | bin | crates/context-graph-benchmark/src/bin/sparse_bench.rs |  |
| spawn-blocking-bench | bin | crates/context-graph-benchmark/src/bin/spawn_blocking_bench.rs | real-embeddings |
| temporal-bench | bin | crates/context-graph-benchmark/src/bin/temporal_bench.rs |  |
| temporal-realdata-bench | bin | crates/context-graph-benchmark/src/bin/temporal_realdata_bench.rs |  |
| unified-realdata-bench | bin | crates/context-graph-benchmark/src/bin/unified_realdata_bench.rs |  |
| validation-bench | bin | crates/context-graph-benchmark/src/bin/validation_bench.rs |  |
| validation-realdata-bench | bin | crates/context-graph-benchmark/src/bin/validation_realdata_bench.rs |  |
| weight-profile-bench | bin | crates/context-graph-benchmark/src/bin/weight_profile_bench.rs |  |
| comparative_suite | bench | crates/context-graph-benchmark/benches/comparative_suite.rs |  |
| scaling_analysis | bench | crates/context-graph-benchmark/benches/scaling_analysis.rs |  |

## 7. Storage Classification
| Path/source | Class | Behavior |
| --- | --- | --- |
| RocksDB path | sacred durable | Contains all CFs; MCP pid guard protects concurrent open |
| models path | regenerable runtime asset | Model weights are loaded by embedding providers |
| target | regenerable | Cargo artifacts |
| <db_path>/mcp.pid | ephemeral | flock lock file |
| ~/.contextgraph/extended_embeddings.lmdb | regenerable sidecar | Extended CAMD cache |

## 8. Current Source Gaps
| Gap | Source |
| --- | --- |
| Teleological compact vector is 13D while NUM_EMBEDDERS is 14 | teleological/types.rs; semantic/constants.rs |
| Config validates sse but MCP main rejects sse | config/sub_configs.rs; mcp/main.rs |
| Some comments still mention older 13/59/75 counts | multiple files; constants above are current |
