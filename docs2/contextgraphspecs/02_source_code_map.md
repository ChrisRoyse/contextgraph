# 02 — Source Code Map

**Source files covered:** all `.rs` files under `crates/*/src/`, tests and examples, plus `scripts/v4_2/*.py`. Workspace root: `/home/cabdru/contextgraph` (Rust edition 2021, MSRV 1.75, Cargo workspace v2).

See [01_system_overview.md](01_system_overview.md) for the architecture overview. This file describes **where things live**.

---

## 1. Workspace Layout

| Crate | default-member | Approx size | Role |
|---|---|---|---|
| `context-graph-core` | yes | ~99 K LOC | Domain types, traits, business logic |
| `context-graph-embeddings` | yes | ~76 K LOC | 13-embedder GPU pipeline + `train-causal` |
| `context-graph-mcp` | yes | ~42 K LOC | JSON-RPC server binary |
| `context-graph-storage` | yes | ~36 K LOC | RocksDB + HNSW + inverted indexes |
| `context-graph-cli` | yes | ~16 K LOC | `context-graph-cli` binary |
| `context-graph-cuda` | yes | ~5 K LOC | GPU abstractions |
| `context-graph-causal-agent` | yes | ~5 K LOC | LLM causal discovery |
| `context-graph-graph-agent` | yes | ~4 K LOC | LLM graph discovery |
| `context-graph-test-utils` | yes | <1 K LOC | Shared test fixtures |
| `context-graph-benchmark` | **no** | ~87 K LOC | Benchmark suite — build via `cargo -p` |

---

## 2. File Trees (`crates/*/src/`)

### 2.1 `context-graph-core`

```
crates/context-graph-core/
  Cargo.toml
  src/
    lib.rs                                   # Crate root, re-exports, test-only stubs
    monitoring.rs                            # LayerStatusProvider, SystemMonitor traits (AP-007)
    causal/
      mod.rs                                 # Causal module root
      asymmetric.rs                          # E5 cause→effect 1.2× / effect→cause 0.8×
      chain.rs                               # Transitive chains with 0.9^hop attenuation
      inference.rs                           # Forward, backward, bridge, abduction
      scm.rs                                 # Structural Causal Model + DAG + interventions
    clustering/
      mod.rs                                 # Multi-space clustering types
      birch.rs                               # BIRCH incremental clustering
      cluster.rs                             # Cluster struct (centroid + quality)
      error.rs                               # ClusterError
      fingerprint_matrix.rs                  # Dense matrix view for clustering
      hdbscan.rs                             # HDBSCAN density clustering
      manager.rs                             # ClusteringManager (lifecycle, caps, topics)
      membership.rs                          # ClusterMembership per-space tracking
      persistence.rs                         # Persistence hooks to RocksDB
      stability.rs                           # TopicStability (churn, drift, phase)
      synthesizer.rs                         # Per-space → cross-space topic merger
      topic.rs                               # Topic, TopicProfile (13D weighted)
    code/
      mod.rs                                 # Code2Code vs Text2Code query detection for E7
    config/
      mod.rs                                 # Config struct + load + validate
      constants.rs                           # System-wide thresholds
      embedder_config.rs                     # Static per-embedder config
      sub_configs.rs                         # Cuda, Embedding, Index, Logging, Mcp structs
      tests/ {mod, default, mcp_config, serialization, validation}.rs
    constellation/
      mod.rs                                 # CONSTELLATION_VERSION=1, MIN_MEMBERS=3, RESERVOIR=1024
      compiler.rs                            # Two-pass observe → finalize accumulator
      reservoir.rs                           # Vitter Algorithm R reservoir sampler
      types.rs                               # Constellation, Selector, EmbedderStats, VectorKind
      welford.rs                             # Online mean + variance (per-coordinate)
      tests.rs
    contrastive/
      mod.rs                                 # CONTRASTIVE_PAIR_VERSION=1, SRC-3 [0,1] contract
      mining.rs                              # similarity_profile, classify_anomaly, mine_pair
      types.rs                               # ContrastivePair, AnomalyKind (6), MiningConfig
      tests.rs
    embeddings/
      mod.rs                                 # Public embedding types
      category.rs                            # EmbedderCategory enum
      config.rs                              # Per-embedder runtime config
      provider.rs                            # MultiArrayEmbeddingProvider re-export
      stubs.rs                               # StubMultiArrayProvider (test-only)
      token_pruning.rs                       # E12 token-level vector type
      vector.rs                              # DenseVector, BinaryVector
    entity/
      mod.rs                                 # E11 KEPLER / TransE entity types
    error/
      mod.rs                                 # Error module root
      conversions.rs                         # From<> impls
      legacy.rs                              # CoreError retained for back-compat
      sub_errors.rs                          # EmbeddingError, StorageError, IndexError, …
      tests.rs
      unified.rs                             # ContextGraphError top-level enum
    fusion/
      mod.rs                                 # Weighted RRF, weighted sum, normalize_minmax
    graph/
      mod.rs                                 # E8 graph-relation module
      asymmetric.rs                          # E8 source→target 1.2× / target→source 0.8×
    graph_linking/
      mod.rs                                 # K-NN graph + typed edges
      direction.rs                           # DirectedRelation
      edge_builder.rs                        # Typed-edge builder from embedder agreement
      edge_type.rs                           # 8 graph-linking edge types
      embedder_edge.rs                       # Per-embedder K-NN edge
      error.rs                               # EdgeError (fail-fast)
      knn_graph.rs                           # KnnGraph container
      nn_descent.rs                          # NN-Descent approximate K-NN build
      service.rs                             # GraphLinkingService orchestrator
      storage_keys.rs                        # EdgeStorageKey, TypedEdgeStorageKey
      thresholds.rs                          # EdgeThresholds per type
      typed_edge.rs                          # TypedEdge with embedder-agreement metadata
      weight_projector.rs                    # Learned MLP for edge weights
    index/
      mod.rs                                 # Index layer (multi-space HNSW + inverted)
      config.rs                              # DistanceMetric, HNSW params
      error.rs                               # IndexError
      manager.rs                             # IndexManager (lifecycle + search)
      splade_impl.rs                         # InMemorySpladeIndex (E13)
      status.rs                              # Index status enum
      hnsw_impl/
        mod.rs, multi_space.rs, multi_space_trait.rs,
        persistence.rs, real_hnsw.rs, types.rs
        tests/ {mod, multi_space, persistence, real_hnsw}_tests.rs
    injection/
      mod.rs                                 # Injection pipeline types
      budget.rs                              # TokenBudget
      candidate.rs                           # InjectionCandidate
      formatter.rs                           # compact / standard / verbose
      pipeline.rs                            # InjectionPipeline orchestrator
      priority.rs                            # InjectionCategory ordering
      result.rs                              # InjectionResult
      temporal_enrichment.rs                 # TemporalBadge (freshness, recurrence)
    llm_edge_validation/
      mod.rs                                 # LLMEdgeValidation (LLM_EDGE_VALIDATION_VERSION=1)
      tests.rs
    memory/
      mod.rs                                 # Memory, MemorySource, Session
      ast_chunker.rs                         # tree-sitter AST chunker (Rust)
      capture.rs                             # content → chunks → fingerprints pipeline
      chunker.rs                             # TextChunker with overlap
      code_capture.rs                        # Code-file capture using AST chunker
      code_watcher.rs                        # .rs directory watcher
      manager.rs                             # MemoryManager (topic detection, caps)
      session.rs                             # SessionStatus, SessionCache
      source.rs                              # MemorySource enum
      store.rs                               # MemoryStore trait + InMemory impl
      watcher.rs                             # Markdown file watcher
    quantization/
      mod.rs                                 # Quantization (INT4 / INT8 / FP16 / PQ-8)
      accuracy.rs                            # NRMSE / RMSE metrics
      batch.rs                               # Batch quant / dequant
      fingerprint.rs                         # SemanticFingerprint quantization
      traits.rs                              # Quantizable
      types.rs                               # QuantizedFloat8, QuantizedPQ8, …
    retrieval/
      mod.rs                                 # 5-stage pipeline root
      aggregation.rs                         # Score aggregation across spaces
      config.rs                              # PipelineStageConfig
      detector.rs                            # Query-type detection → strategy
      distance.rs                            # Distance metric impls
      divergence.rs                          # Divergence detection vs recent
      executor.rs                            # Multi-space executor trait
      in_memory_executor.rs                  # Test executor
      insight_annotation.rs                  # Insight annotations on results
      multi_space.rs                         # Multi-space search with RRF
      query.rs                               # Query types + parsing
      result.rs                              # Result types
      retriever.rs                           # Retriever orchestrator
      similarity.rs                          # Similarity helpers
      sparse_index.rs                        # Sparse index ops (E6/E13)
      teleological_query.rs                  # Query with weight profile
      teleological_result.rs                 # Teleological result types
      tests.rs
    similarity/
      mod.rs                                 # Cross-space similarity engine
      config.rs                              # SimilarityEngineConfig
      default_engine.rs                      # DefaultCrossSpaceEngine
      engine.rs                              # CrossSpaceSimilarityEngine trait
      error.rs                               # SimilarityError
      explanation.rs                         # Explanations for debugging
      multi_utl.rs                           # Unified Teleological Layer compute
      result.rs
      sparse.rs                              # Jaccard, dot product
      tests.rs
      token_level.rs                         # E12 MaxSim token scoring
      dense/ {mod, error, primitives, simd}.rs + tests/{integration, primitives, simd}_tests.rs
    stubs/
      mod.rs
      multi_array_stub.rs                    # Test multi-array provider
      teleological_store_stub/
        mod.rs, content.rs, search.rs, similarity.rs, trait_impl.rs, tests.rs
    teleological/
      mod.rs                                 # Cross-embedding synergy + fusion
      comparison_error.rs, embedder.rs, groups.rs, meaning.rs,
      resolution.rs, types.rs, vector.rs, tucker.rs
      comparator/ {mod, batch, result, strategies, teleological, tests}.rs
      matrix_search/
        mod.rs, config.rs, embedder_names.rs, types.rs, weights.rs
        search/ {mod, clustering, comprehensive, core}.rs
        strategies/ {mod, computer, helpers}.rs
        tests/ {mod, advanced, basic}.rs
      profile/ {mod, fusion_strategy, metrics, task_type, teleological, tests}.rs
      services/
        mod.rs, correlation_extractor.rs, fusion_engine.rs, group_aggregator.rs,
        meaning_pipeline.rs, multi_space_retriever.rs, synergy_service.rs, tucker_decomposer.rs
        profile_manager/ {mod, builtin, manager, tests, types}.rs
      synergy_matrix/
        mod.rs, accessors.rs, analysis.rs, constants.rs, constructors.rs,
        presets.rs, types.rs, validation.rs
        tests/ {mod, analysis, basic, presets, validation}.rs
    training/
      mod.rs                                 # TrainingRecord v1 (78 cross-corr + 6D groups)
      computation.rs                         # topic_profile_or_fallback, cross_correlations, group_alignments
      edge_signature.rs                      # compute_edge_type_distribution (8-dim)
      temporal.rs                            # PeriodicBucket, TemporalLabels
      tests.rs
    typed_edge_export/
      mod.rs                                 # TypedEdgeTrainingRecord (TYPED_EDGE_RECORD_VERSION=1)
      tests.rs
    traits/
      mod.rs, multi_array_embedding.rs, teleological_memory_store_tests.rs
      teleological_memory_store/
        mod.rs, backend.rs, ext.rs, options.rs, result.rs, store.rs
    types/
      mod.rs, audit.rs, causal_relationship.rs, code_entity.rs, discovery.rs,
      file_index.rs, source_metadata.rs
      fingerprint/
        mod.rs, sparse.rs
        semantic/ {mod, constants, fingerprint, slice, validation}.rs + tests/…
        teleological/ {mod, core, test_helpers, tests, types}.rs
      graph_edge/
        mod.rs, edge.rs, modulation.rs, traversal.rs,
        tests_{constructor, modulation, struct, traversal}.rs
      memory_node/
        mod.rs, metadata.rs, node.rs, validation.rs,
        tests_{metadata, node, serialization}.rs
    weights/
      mod.rs                                 # 14 built-in weight profiles
  examples/
    divergence_fsv_test.rs, fsv_line_numbers.rs,
    fsv_source_metadata.rs, tree_sitter_test.rs
  tests/
    file_watcher_integration.rs, injection_manual_test.rs,
    rrf_edge_cases.rs, source_metadata_integration.rs,
    synthesizer_edge_case_test.rs
```

### 2.2 `context-graph-embeddings`

```
crates/context-graph-embeddings/
  Cargo.toml
  src/
    lib.rs                                   # Crate root; candle compile_error! gate
    global_provider.rs                       # Warm-loaded provider singleton
    adapters/
      mod.rs, code_provider.rs               # E7CodeEmbeddingProvider wrapper
    batch/
      mod.rs
      processor/ {mod, config, core, stats, submit, worker}.rs
      types/ {mod, batch, queue, request, stats, tests}.rs
    bin/
      train_causal.rs                        # train-causal binary (E5 LoRA fine-tune)
    cache/
      mod.rs                                 # moka concurrent cache
    config/
      mod.rs, batch.rs, cache.rs, gpu.rs, models.rs
    error/
      mod.rs, types.rs, tests.rs, tests_extended.rs
    gpu/
      mod.rs, ops.rs
      device/ {mod, accessors, core, utils}.rs
      memory/ {mod, budget, error, pool, pressure, slots, stats, tracker}.rs
      model_loader/
        mod.rs, batch_loader.rs, config.rs, embedding_loader.rs, error.rs,
        layer_loader.rs, loader.rs, tensor_utils.rs, unified.rs, weights.rs
      tensor/ {mod, core, ops}.rs
    models/
      mod.rs
      memory_tracker/ {mod, core}.rs + tests/{basic, advanced, mod}.rs
      factory/ {mod, default_factory, trait_impl}.rs + tests/…
      registry/ {mod, config, core, loader, operations, stats}.rs + tests[*].rs
      attention/ {mod, dense, sliding_window, tiled}.rs
      pretrained/
        mod.rs, shared.rs
        semantic/   # E1 e5-large-v2 (1024D)
        causal/     # E5 nomic-embed fine-tuned (768D asymmetric)
        code/       # E7 Qodo-Embed-1-1.5B (1536D)
        contextual/ # E10 paraphrase contextual variant
        graph/      # E8 e5-large-v2 asymmetric (1024D)
        kepler/     # E11 KEPLER/TransE (768D; disabled)
        late_interaction/  # E12 ColBERT (128D/token, MaxSim)
        rgcn/       # R-GCN relational graph conv
        sparse/     # E6 keyword + E13 SPLADE (~30K vocab)
        weight_projection/  # Learned MLP for graph edges
      custom/
        mod.rs
        hdc/                         # E9 HDC (1024D binary hypervectors)
        temporal_recent/             # E2 sinusoidal PE (512D)
        temporal_periodic/           # E3 periodic pattern (512D)
        temporal_positional/         # E4 document position (512D)
    provider/
      mod.rs, causal_hint.rs, diagnostic_test.rs, multi_array.rs
    pruning/
      mod.rs, config.rs, token_pruner.rs
    quantization/
      mod.rs, binary.rs, edge_case_verification.rs, float8.rs, types.rs
      pq8/ {mod, encoder, persistence, training, types}.rs + tests/…
      router/ {mod, encoders}.rs + tests/…
    storage/
      mod.rs, multi_space.rs                 # MultiSpaceSearchEngine (Stage 3 RRF)
      full_state_verification.rs
      types/ {mod, constants, fingerprint, index_entry, query_results, tests}.rs
    training/
      mod.rs, data.rs, distillation.rs, evaluation.rs,
      lora.rs, loss.rs, multitask.rs, optimizer.rs, pipeline.rs, trainer.rs
    traits/
      mod.rs
      embedding_model/ {mod, trait_def}.rs + tests/{core, integration, test_model}.rs
      model_factory/ {mod, config, device, memory, quantization, trait_def}.rs + tests.rs
    types/
      mod.rs
      concatenated/ {mod, core, operations, tests}.rs
      dimensions/ {mod, aggregates, arrays, constants, helpers}.rs
      embedding/ {mod, operations, tensor, tests, types, validation}.rs
      input/ {mod, accessors, image_format, input_type, model_input}.rs + tests/…
      model_id/ {mod, conversions, core, display, repository, tokenizer, tests}.rs
    warm/
      mod.rs, config.rs, error.rs, handle.rs, state.rs
      cuda_alloc/ {mod, allocation, allocator, allocator_cuda, constants, gpu_info, helpers}.rs + tests.rs
      diagnostics/ {mod, helpers, output, service, types, tests}.rs
      health/ {mod, check, checker, status}.rs + tests.rs + tests_additional.rs
      inference/ {mod, engine, validation}.rs
      integration/ {mod, pipeline}.rs + tests/{basic, advanced, mod}.rs
      loader/
        mod.rs, constants.rs, engine.rs, helpers.rs, operations.rs,
        preflight.rs, summary.rs, tests.rs
        tests/ {loader_tests, summary_tests}.rs
        types/ {mod, inference, metadata, vram, weights, tests}.rs
      memory_pool/ {mod, model_pool, pools, working_pool, types}.rs + tests.rs
      registry/ {mod, core, operations, queries, types}.rs + tests[*].rs
      validation/ {mod, comparisons, config, result, validator, validator_model}.rs + tests/…
      tests/ {mod, config, diagnostics, error, handle, health, helpers,
              integration, loader, memory_pool, registry, state, validation}_tests.rs
  tests/
    attention_strategy_tests.rs, benchmark_test.rs,
    e9_typo_tolerance_test.rs, e9_vector_differentiation_test.rs,
    embed_dual_with_hint_integration.rs, global_provider_fsv.rs,
    gpu_memory_slots_test.rs, no_fake_data_test.rs,
    qodo_embed_integration.rs, search_test.rs,
    dimension_test/ {main, aggregate_dimensions, comprehensive, constants,
                    edge_cases, model_metadata, native_dimensions, offsets,
                    projected_dimensions, quantization_methods}.rs
    fixtures/ {create_fixture, physical_verification_test}.rs
    storage_roundtrip_test/ {main, comprehensive_validation, edge_cases,
                            fingerprint_creation, helpers, index_entry,
                            query_result, rrf_formula, serialization_roundtrip}.rs
```

### 2.3 `context-graph-mcp`

```
crates/context-graph-mcp/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: handlers, protocol, server, tools, adapters
    main.rs                                  # Binary entry point (stdio / TCP / daemon)
    monitoring.rs                            # Real LayerStatusProvider + SystemMonitor
    protocol.rs                              # JsonRpcRequest / Response / Error + error_codes
    weights.rs                               # Thin wrapper over core::weights
    adapters/
      mod.rs
      causal_hint.rs                         # LLM-backed CausalHintProvider for E5
      lazy_provider.rs                       # LazyMultiArrayProvider (start before models ready)
    handlers/
      mod.rs
      merge.rs                               # merge_concepts entry
      core/
        mod.rs
        dispatch.rs                          # method → handler routing
        handlers.rs                          # Handlers struct (store, provider, agents, watchers)
      tests/
        mod.rs, error_codes.rs, initialize.rs,
        mcp_protocol_e2e_test.rs, search_periodic_test.rs,
        tcp_transport_integration.rs, tools_call.rs, tools_list.rs, topic_tools.rs
      tools/
        mod.rs                               # Tool handler module root
        dispatch.rs                          # tool_dispatch! macro + 75-tool match
        helpers.rs                           # Param parsing + response formatting
        validate.rs                          # Input validation helpers
        consolidation.rs                     # trigger_consolidation
        memory_tools.rs                      # store_memory, search_graph
        status_tools.rs                      # get_memetic_status
        curation_tools.rs                    # forget_concept, boost_importance
        file_watcher_tools.rs                # list/stats/delete/reconcile files
        sequence_tools.rs                    # E4 conversation/timeline/chain/state-compare
        causal_tools.rs                      # E5 search_causes/effects/chain/relationships
        causal_discovery_tools.rs            # LLM-gated trigger + status
        causal_relationship_tools.rs         # CF_CAUSAL_RELATIONSHIPS CRUD
        keyword_tools.rs                     # E6 search_by_keywords
        code_tools.rs                        # E7 search_code
        graph_tools.rs                       # E8 search_connections, get_graph_path, LLM-gated discover/validate
        graph_link_tools.rs                  # K-NN neighbors, typed-edges, traverse, unified-neighbors (ARCH-21)
        robustness_tools.rs                  # E9 search_robust
        entity_tools.rs                      # E11 extract/search/infer/related/validate/graph
        embedder_tools.rs                    # 9 embedder-first tools (incl. E12, E13)
        temporal_tools.rs                    # E2 search_recent, E3 search_periodic
        topic_tools.rs                       # portfolio/stability/detect/divergence
        provenance_tools.rs                  # audit_trail, merge_history, provenance_chain
        maintenance_tools.rs                 # repair_causal_relationships
        daemon_tools.rs                      # daemon_status
        training_tools.rs                    # export/list/get/count_training_records
        constellation_tools.rs               # compile/list/get/score/delete_constellation
        contrastive_tools.rs                 # mine/list/get/count_contrastive_pairs
        typed_edges_tools.rs                 # export/derive/validate/list typed-edge corpora
        causal_dtos.rs, code_dtos.rs, curation_dtos.rs, embedder_dtos.rs,
        entity_dtos.rs, graph_dtos.rs, graph_link_dtos.rs, keyword_dtos.rs,
        provenance_dtos.rs, robustness_dtos.rs, temporal_dtos.rs, topic_dtos.rs
    server/
      mod.rs                                 # McpServer struct, stdio loop
      transport.rs                           # TCP transport (run_tcp, handle_client, read_line_bounded)
      watchers.rs                            # File + code watchers, BackgroundGraphBuilder spawn
    tools/
      mod.rs                                 # Tool definitions root (MCP 2024-11-05)
      aliases.rs                             # Back-compat name aliases
      names.rs                               # 75 tool-name constants
      types.rs                               # ToolDefinition struct
      definitions/
        mod.rs                               # get_tool_definitions() aggregator
        core.rs, curation.rs, merge.rs, topic.rs, file_watcher.rs,
        sequence.rs, causal.rs, causal_discovery.rs, keyword.rs, code.rs,
        graph.rs, robustness.rs, entity.rs, embedder.rs, temporal.rs,
        graph_link.rs, provenance.rs, maintenance.rs, daemon.rs,
        training.rs, constellation.rs, contrastive.rs, typed_edges.rs
  tests/
    graph_linking_integration.rs
```

### 2.4 `context-graph-storage`

```
crates/context-graph-storage/
  Cargo.toml
  benches/
    causal_relationships_bench.rs, e11_quality_bench.rs,
    hybrid_effectiveness_bench.rs, maxsim_bench.rs
  src/
    lib.rs                                   # Re-exports RocksDbTeleologicalStore, CodeStore, EdgeRepository
    column_families.rs                       # TOTAL_COLUMN_FAMILIES=59 (line 397)
    error.rs                                 # StorageError
    indexes.rs                               # Secondary index ops (tags, temporal, sources)
    code/
      mod.rs, error.rs, store.rs             # CodeStore (5 code CFs + E7)
    graph_edges/
      mod.rs
      builder.rs                             # BackgroundGraphBuilder (async K-NN)
      repository.rs                          # EdgeRepository CRUD
      serialization.rs                       # bincode edge serialization
      types.rs                               # GraphEdgeStats, BuilderStats, RebuildResult
    teleological/
      mod.rs                                 # Teleological storage root
      column_families.rs                     # TELEOLOGICAL_CF_COUNT=28 (line 551)
      schema.rs                              # Key formats
      serialization.rs                       # Fingerprint/profile ser
      indexes/
        mod.rs, embedder_index.rs, metrics.rs, registry.rs
        hnsw_config/ {mod, config, constants, distance, embedder, functions}.rs
        hnsw_impl/ {mod, ops, tests, types}.rs
      quantized/
        mod.rs, error.rs, helpers.rs, rocksdb_impl.rs, tests.rs, trait_def.rs
      rocksdb_store/
        mod.rs
        store.rs                             # RocksDbTeleologicalStore + constructor
        crud.rs, content.rs, helpers.rs, types.rs
        audit_log.rs                         # CF_AUDIT_LOG / CF_AUDIT_BY_TARGET
        merge_history.rs via provenance_storage.rs (logical, not a separate file)
        provenance_storage.rs                # CF_MERGE_HISTORY, CF_IMPORTANCE_HISTORY
        source_metadata.rs                   # CF_SOURCE_METADATA
        file_index.rs                        # CF_FILE_INDEX
        causal_hnsw_index.rs                 # E5 asymmetric HNSW (cause/effect)
        causal_relationships.rs              # CF_CAUSAL_RELATIONSHIPS
        inverted_index.rs                    # E6 / E13 sparse inverted
        index_ops.rs                         # Rebuild / compact
        persistence.rs                       # HNSW persist ↔ RocksDB
        fusion.rs                            # RRF / pipeline / e1_only fusion
        search.rs                            # Full search impl
        export.rs                            # CF_TRAINING_RECORDS put/get/list/count/iter
        constellation.rs                     # CF_CONSTELLATIONS + selector index (WriteBatch)
        contrastive.rs                       # CF_CONTRASTIVE_* (primary + by-kind + by-anchor)
        typed_edge_keys.rs                   # Shared 33-byte key [source:16][target:16][type:1]
        typed_edge_export.rs                 # CF_TYPED_EDGE_RECORDS CRUD
        llm_validation.rs                    # CF_TYPED_EDGE_VALIDATIONS (Valid/Invalid/Reclassify)
        anomaly_derivation.rs                # Derive 5-of-6 AnomalyKinds from typed-edge scores
        versioned_bincode.rs                 # Version-byte + bincode helpers
        trait_impl.rs                        # TeleologicalMemoryStore impl
        tests.rs
      search/
        mod.rs, error.rs, maxsim.rs, result.rs,
        temporal_boost.rs, token_storage.rs
        matrix/ {mod, analysis, builder, results, search_matrix, strategy_search,
                 tests_boundary, tests_unit}.rs
        multi/ {mod, builder, executor, types, tests}.rs
        pipeline/ {mod, builder, execution, stages, traits, types, tests}.rs
        single/ {mod, config, search}.rs + tests/{mod, integration, search, validation}.rs
      tests/
        mod.rs, column_family.rs, content.rs, helpers.rs,
        key_format.rs, panic.rs, serialization.rs, stale_lock.rs
  examples/
    demo_typed_edge_capabilities.rs, fsv_typed_edges_physical.rs
  tests/
    anomaly_derivation_integration.rs, constellation_integration.rs,
    contrastive_integration.rs, crash_recovery_tests.rs,
    e12_e13_source_of_truth_verification.rs, e9_storage_roundtrip_test.rs,
    llm_validation_integration.rs, teleological_integration.rs,
    training_records_integration.rs, typed_edge_export_integration.rs,
    full_integration_real_data/ {main, helpers, operations_tests,
                                 performance_tests, pipeline_tests,
                                 roundtrip_tests, search_tests}.rs
```

### 2.5 `context-graph-cli`

```
crates/context-graph-cli/
  Cargo.toml
  benches/
    cli_benchmarks.rs
  src/
    main.rs                                  # clap entry point
    lib.rs                                   # Re-exports for tests
    error.rs                                 # CliExitCode, exit_code_for_error (AP-26)
    mcp_client.rs                            # JSON-RPC client to daemon
    mcp_helpers.rs                           # Request builders
    commands/
      mod.rs
      divergence.rs, setup.rs, topic.rs, warmup.rs, watch.rs
      hooks/
        mod.rs, args.rs, error.rs, memory_cache.rs, types.rs, session_state.rs,
        session_start.rs, session_end.rs, pre_tool_use.rs, post_tool_use.rs,
        user_prompt_submit.rs, task_completed.rs, pre_compact.rs
      memory/
        mod.rs, capture.rs, inject.rs
      session/
        mod.rs, persist.rs, restore.rs
  tests/
    e2e/ {mod, error_recovery_test, full_session_test, helpers}.rs
    integration/ {mod, exit_code_test, helpers, hook_lifecycle_test, timeout_test}.rs
    export_training_integration.rs
```

### 2.6 `context-graph-cuda`

```
crates/context-graph-cuda/
  Cargo.toml
  build.rs                                   # CUDA compilation shim
  src/
    lib.rs                                   # Public CUDA API
    error.rs                                 # CudaError
    ops.rs                                   # VectorOps trait (cosine / dot / L2)
    similarity.rs                            # GPU-batched cosine (grouped by dimension)
    stub.rs                                  # StubVectorOps (test-only)
    context/
      mod.rs
      green_contexts.rs                      # Green Contexts (GPU partitioning)
    ffi/
      mod.rs, cuda_driver.rs, faiss.rs, knn.rs
    hdbscan/
      mod.rs, clusterer.rs, error.rs, gpu_knn.rs, tests.rs
    safe/
      mod.rs, device.rs                      # RAII CUDA device wrapper
```

### 2.7 `context-graph-causal-agent`

```
crates/context-graph-causal-agent/
  Cargo.toml
  src/
    lib.rs                                   # LLM causal-discovery entry
    error.rs                                 # CausalAgentError
    activator/
      mod.rs                                 # E5EmbedderActivator
    llm/
      mod.rs                                 # CausalDiscoveryLLM (llama-cpp-2, feature-gated)
      prompt.rs                              # ChatML causal-analysis templates
    scanner/
      mod.rs                                 # MemoryScanner (candidate pair finder)
    service/
      mod.rs                                 # CausalDiscoveryService (background loop)
    types/
      mod.rs                                 # CausalCandidate, MemoryForAnalysis, …
  examples/
    benchmark_causal.rs, benchmark_causal_enhanced.rs,
    benchmark_causal_large.rs, test_llm.rs
  tests/
    analyze_single_text_integration.rs, background_loop_integration.rs
```

### 2.8 `context-graph-graph-agent`

```
crates/context-graph-graph-agent/
  Cargo.toml
  src/
    lib.rs                                   # LLM graph-discovery entry (20 rel types, 4 domains)
    error.rs                                 # GraphAgentError
    stubs.rs                                 # Test stubs (test-utils feature)
    activator/
      mod.rs                                 # E8Activator
    llm/
      mod.rs                                 # GraphRelationshipLLM (shares Hermes-2-Pro)
      prompt.rs                              # GraphPromptBuilder
    scanner/
      mod.rs                                 # MemoryScanner for graph analysis
    service/
      mod.rs                                 # GraphDiscoveryService orchestrator
    types/
      mod.rs                                 # RelationshipType, ContentDomain, …
  examples/
    benchmark_graph.rs, benchmark_graph_code.rs, benchmark_graph_multi.rs
  tests/
    scanner_integration.rs
```

### 2.9 `context-graph-test-utils`

```
crates/context-graph-test-utils/
  Cargo.toml
  src/
    lib.rs                                   # Re-exports fingerprint + store helpers
    fingerprints.rs                          # Unit-vector / sparse / teleological generators
    stores.rs                                # create_test_store, create_initialized_store
```

### 2.10 `context-graph-benchmark` (excluded from default build)

```
crates/context-graph-benchmark/
  Cargo.toml
  benches/
    comparative_suite.rs                     # Multi-space vs single-embedder (Criterion)
    scaling_analysis.rs                      # 100 → 10 M memory scaling (Criterion)
  src/
    lib.rs, ablation.rs, config.rs, stress_corpus.rs, util.rs
    baseline/ {mod, single_embedder, single_hnsw}.rs
    bin/                                     # 40+ standalone benchmark binaries
      e1_semantic_bench.rs, e2_manual_verification.rs, e4_hybrid_session_bench.rs,
      e7_bench.rs, e7_realdata_bench.rs, e10_modifier_tuning.rs, e11_entity_bench.rs,
      causal_accuracy_bench.rs, causal_discovery_mcp_bench.rs, causal_e2e_bench.rs,
      causal_embedding_bench.rs, causal_perf_bench.rs, causal_prompt_experiment_bench.rs,
      causal_provenance_bench.rs, causal_provenance_e2e_bench.rs,
      causal_provenance_llm_bench.rs, causal_realdata_bench.rs, causal_span_fix_bench.rs,
      code_fingerprint_bench.rs, disk_space_bench.rs,
      embedder_impact_bench.rs, embedder_impact_realdata_bench.rs, embedder_stress.rs,
      gpu_bench.rs, graph_bench.rs, graph_linking_bench.rs,
      graph_linking_gpu_bench.rs, graph_linking_real_bench.rs, graph_realdata_bench.rs,
      hf_bench.rs, mcp_bench.rs,
      model_injection_bench.rs, model_injection_rigorous_bench.rs,
      multimodal_bench.rs, multimodal_realdata_bench.rs,
      realdata_bench.rs, report.rs, sparse_bench.rs, spawn_blocking_bench.rs,
      stub_fix_verification.rs, temporal_bench.rs, temporal_realdata_bench.rs,
      unified_realdata_bench.rs, validation_bench.rs, validation_realdata_bench.rs,
      weight_profile_bench.rs
    causal_bench/ {mod, comparison, data_loader, metrics, phases, provider, report}.rs
    datasets/
      mod.rs, beir_loader.rs, causal.rs, e11_entity.rs, e1_semantic.rs,
      embedder_impact.rs, generator.rs, graph_linking.rs, ground_truth.rs,
      multimodal.rs, sparse.rs, temporal.rs, temporal_sessions.rs, topic_clusters.rs
    metrics/
      mod.rs, causal.rs, clustering.rs, divergence.rs, e11_entity.rs, e1_semantic.rs,
      e4_hybrid_session.rs, e7_code.rs, e7_iou.rs, embedder_contribution.rs,
      graph_linking.rs, graph_structure.rs, multimodal.rs, performance.rs,
      resource_usage.rs, retrieval.rs, sparse.rs, temporal.rs, temporal_realdata.rs,
      validation.rs
    realdata/
      mod.rs, config.rs, embedder.rs, ground_truth.rs, loader.rs,
      report.rs, results.rs, runner.rs, temporal_injector.rs
    reports/ {mod, json, markdown}.rs
    runners/
      mod.rs, causal.rs, comparative.rs, e11_entity.rs, e1_semantic.rs,
      e4_hybrid_session.rs, embedder_impact.rs, failfast.rs, graph_linking.rs,
      graph_realdata.rs, multimodal.rs, retrieval.rs, scaling.rs, sparse.rs,
      temporal.rs, temporal_realdata.rs, topic.rs, unified_realdata.rs
    scaling/ {mod, corpus_builder, degradation, memory_profiler}.rs
    tuning/ {mod, e7_tuning}.rs
    validation/
      mod.rs, ablation_validation.rs, arch_compliance.rs, asymmetric_validation.rs,
      config_validation.rs, e1_foundation.rs, fusion_validation.rs,
      ground_truth_validation.rs, results_aggregator.rs, temporal_validation.rs
```

---

## 3. `scripts/v4_2/` (Python — CAMD-DPO)

All scripts expect the MCP daemon on `127.0.0.1:3100`. Deep dive: [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md).

| File | LOC | Purpose |
|---|---:|---|
| `__init__.py` | 1 | Package marker ("v4.2 novel-method implementations") |
| `camd_lib.py` | 491 | ConstellationScorer + CAMDPairFilter + STYLE_EMBEDDER_WEIGHTS (daemon-13) |
| `mine_camd_pairs.py` | 251 | CLI miner: CF_CONTRASTIVE_PAIRS → JSONL DPO pairs (daemon-13 reward) |
| `mine_camd_pairs_extended.py` | 353 | CLI miner with 18-view reward (daemon-13 + extended-5) |
| `mine_camd_from_lmdb.py` | 300 | Score-and-sort miner reading LMDB sidecar directly |
| `extended_ensemble.py` | 499 | 4 dense + 1 reranker (BGE-M3, E5-Mistral-7B, Jina-v3, Qwen3-Reranker, Jina colBERT) |
| `extended_store.py` | 420 | LMDB sidecar CRUD for extended-5 vectors (`~/.contextgraph/v4_2/extended_store.lmdb`) |
| `backfill_extended.py` | 420 | Batch-compute extended-5 embeddings for all memories; populates sidecar |
| `extended_constellation.py` | 261 | Sonnets constellation centroids + covariance over extended-5 |
| `extended_camd.py` | 428 | Extended CAMD reward combining daemon-13 + extended-5 views |
| `analyze_correlations.py` | 286 | Cross-embedder correlation analysis |
| `validate_camd.py` | 307 | 3-part validation: reward sanity, pair discrimination, stats |
| `smoke_dpo_v4_2.py` | 252 | End-to-end DPO smoke test (pre-flight before 3-4 h run) |
| `train_camd_dpo.py` | 227 | TRL-based DPO trainer (Phase 4) |
| `README.md` | — | Subsystem documentation (non-Python) |

---

## 4. Inter-Crate Dependency Graph

Derived from `[dependencies]` in each `Cargo.toml`.

```
context-graph-mcp
├── context-graph-core
├── context-graph-cuda
├── context-graph-embeddings
├── context-graph-storage
├── context-graph-causal-agent
└── context-graph-graph-agent

context-graph-cli
├── context-graph-core
├── context-graph-mcp
├── context-graph-storage
└── context-graph-embeddings

context-graph-storage
├── context-graph-core
├── context-graph-embeddings
└── context-graph-cuda (optional, cuda feature)

context-graph-embeddings
├── context-graph-core
└── context-graph-cuda

context-graph-causal-agent
├── context-graph-core
├── context-graph-storage
└── context-graph-embeddings

context-graph-graph-agent
├── context-graph-core
├── context-graph-storage
├── context-graph-embeddings
└── context-graph-causal-agent

context-graph-cuda
└── context-graph-core

context-graph-test-utils
├── context-graph-core
└── context-graph-storage

context-graph-benchmark (NOT default-member)
├── context-graph-core
├── context-graph-storage
├── context-graph-embeddings  (real-embeddings feature)
├── context-graph-mcp         (real-embeddings feature)
├── context-graph-graph-agent (optional)
└── context-graph-causal-agent(optional)
```

Layer diagram (bottom-up):

| Layer | Crate | Depends on |
|---|---|---|
| 0 | `context-graph-core` | — |
| 1 | `context-graph-cuda` | core |
| 2 | `context-graph-embeddings` | core, cuda |
| 3 | `context-graph-storage` | core, embeddings, cuda (opt) |
| 4 | `context-graph-causal-agent` | core, storage, embeddings |
| 5 | `context-graph-graph-agent` | core, storage, embeddings, causal-agent |
| 6 | `context-graph-mcp` | all above |
| 7 | `context-graph-cli` | core, mcp, storage, embeddings |

---

## 5. Internal Module Graphs (major crates)

### 5.1 `context-graph-core`

```
lib.rs
├── config         (serde + toml)
├── error          (thiserror)
├── types          (fingerprint, graph_edge, memory_node, audit, code_entity, …)
├── traits         (TeleologicalMemoryStore, MultiArrayEmbeddingProvider)
├── embeddings     (types + traits + config)
├── entity         (types + embeddings)
├── monitoring     (error)
├── weights        (types::fingerprint constants)
├── fusion         (uuid)
├── graph          (types::fingerprint)
├── graph_linking  (types + error + traits + config)
├── causal         (types + error + graph_linking)
├── clustering     (types + config + error + embeddings)
├── code           (types::fingerprint)
├── index          (types + config + error + hnsw_rs)
├── injection      (types + traits + clustering + fusion)
├── memory         (types + traits + config + error + clustering)
├── quantization   (types::fingerprint + error)
├── retrieval      (types + traits + config + error + fusion + similarity)
├── similarity     (types + config + error)
├── stubs          (types + traits + error) [test-utils]
├── teleological   (types + config + traits + embeddings + error + clustering)
├── training       (types::fingerprint + teleological::{synergy_matrix, groups, types})
├── constellation  (types::fingerprint + teleological::{synergy_matrix, types})
├── contrastive    (types::fingerprint + teleological::synergy_matrix)
├── typed_edge_export (types + graph_linking)
└── llm_edge_validation (types)
```

### 5.2 `context-graph-embeddings`

```
lib.rs
├── config
├── error
├── types              (ModelId, ModelInput, MultiArrayEmbedding, dimensions)
├── traits             (EmbeddingModel, ModelFactory)
├── gpu                (candle-core/nn + device + memory + model_loader + tensor)
├── models             (factory + registry + memory_tracker + attention + pretrained + custom)
├── provider           (ProductionMultiArrayProvider + CausalHintProvider)
├── cache              (moka)
├── batch              (processor + types)
├── quantization       (PQ-8, Float8, Binary, router)
├── pruning            (E12 token pruner)
├── storage            (StoredQuantizedFingerprint + MultiSpaceSearchEngine)
├── training           (E5 LoRA pipeline)
├── adapters           (code_provider → E7)
├── global_provider    (warm-loaded singleton)
└── warm               (loader + registry + memory_pool + cuda_alloc + health +
                        validation + diagnostics + inference + integration)
```

### 5.3 `context-graph-mcp`

```
main.rs (binary)
└── server
    ├── transport        (TCP, read_line_bounded)
    └── watchers         (file watcher + code watcher + BackgroundGraphBuilder)
lib.rs
├── protocol             (JSON-RPC 2.0 types + error codes)
├── handlers
│   ├── core             (Handlers struct, dispatch)
│   ├── merge            (merge_concepts)
│   └── tools            (75 handlers via tool_dispatch! macro)
├── tools
│   ├── names            (75 constants)
│   ├── aliases
│   ├── types            (ToolDefinition)
│   └── definitions      (24 per-category modules)
├── adapters             (LazyMultiArrayProvider, LLM CausalHintProvider)
├── monitoring           (real system monitor)
└── weights              (wrapper over core::weights)
```

### 5.4 `context-graph-storage`

```
lib.rs
├── error
├── column_families      (59 total CF descriptors + TOTAL_COLUMN_FAMILIES)
├── indexes              (secondary indexes on tags/temporal/sources)
├── code
│   └── store            (CodeStore: code_entities + E7 + indexes)
├── graph_edges
│   ├── builder          (BackgroundGraphBuilder)
│   ├── repository       (EdgeRepository)
│   ├── serialization
│   └── types
└── teleological
    ├── column_families  (TELEOLOGICAL_CF_COUNT=28)
    ├── schema
    ├── serialization
    ├── indexes
    │   ├── hnsw_config  (15 per-embedder configs)
    │   └── hnsw_impl    (usearch)
    ├── quantized        (PQ-8 / Float8 / Binary persistence)
    ├── rocksdb_store    (RocksDbTeleologicalStore)
    │   ├── crud, content, fusion, search
    │   ├── audit_log, provenance_storage, source_metadata, file_index
    │   ├── causal_hnsw_index, causal_relationships
    │   ├── inverted_index, index_ops, persistence
    │   ├── export                    (CF_TRAINING_RECORDS)
    │   ├── constellation             (primary + selector index)
    │   ├── contrastive               (primary + by-kind + by-anchor)
    │   ├── typed_edge_keys           (shared 33-byte composite key)
    │   ├── typed_edge_export         (CF_TYPED_EDGE_RECORDS)
    │   ├── llm_validation            (CF_TYPED_EDGE_VALIDATIONS)
    │   ├── anomaly_derivation        (scores → AnomalyKinds)
    │   ├── versioned_bincode         (encode/decode with version byte)
    │   └── trait_impl                (TeleologicalMemoryStore)
    └── search
        ├── single, multi, pipeline, matrix
        ├── maxsim (E12), temporal_boost, token_storage
        └── result / error
```

---

## 6. Entry-Point Traces

### 6.1 `context-graph-mcp` → runtime

```
main.rs
├── CliArgs::parse()                     (manual, no clap)
├── print_help() or continue
├── init tracing_subscriber               (json + env-filter)
├── load Config from --config / env / defaults   (context_graph_core::config)
├── apply_overrides(config, cli)          (CONTEXT_GRAPH_* env → config)
├── determine_transport_mode              (fails on SSE)
├── determine_warm_first
├── spawn_daemon_process() if --daemon (not server-only)
│   └── exec self with --daemon-server-only
├── McpServer::new(config)                (server/mod.rs)
│   ├── acquire mcp.pid flock
│   ├── RocksDbTeleologicalStore::new    (context_graph_storage)
│   ├── CodeStore::new
│   ├── EdgeRepository::new
│   ├── LazyMultiArrayProvider::new      (adapters/lazy_provider.rs)
│   ├── initialize_global_warm_provider  (context_graph_embeddings::global_provider)
│   │   └── WarmLoader loads 13 models into VRAM
│   ├── CausalDiscoveryLLM::new          (feature = "llm")
│   ├── GraphDiscoveryService::new       (feature = "llm")
│   └── Handlers::new(store, provider, agents, …)
├── server.run_stdio() | run_tcp()       (server/transport.rs)
│   └── per-line loop
│       ├── parse JsonRpcRequest         (protocol.rs)
│       ├── Handlers::handle_request     (handlers/core/dispatch.rs)
│       │   ├── "initialize"             → capabilities handshake
│       │   ├── "tools/list"             → tools/definitions/mod.rs
│       │   └── "tools/call"             → handlers/tools/dispatch.rs
│       │       ├── store_memory         → memory_tools.rs
│       │       ├── search_graph         → memory_tools.rs
│       │       ├── search_code          → code_tools.rs
│       │       ├── search_by_embedder   → embedder_tools.rs
│       │       ├── compile_constellation → constellation_tools.rs
│       │       ├── mine_contrastive_pairs → contrastive_tools.rs
│       │       ├── export_training_corpus → training_tools.rs
│       │       ├── export_typed_edges_corpus → typed_edges_tools.rs
│       │       ├── trigger_causal_discovery [llm]
│       │       ├── discover_graph_relationships [llm]
│       │       └── … (75 total)
│       └── serialize JsonRpcResponse → write line
└── background tasks (server/watchers.rs)
    ├── file watcher                      (memory::watcher)
    ├── code watcher                      (memory::code_watcher + AST chunker)
    └── BackgroundGraphBuilder            (graph_edges::builder, ~60 s cycle)
```

### 6.2 `context-graph-cli`

```
main.rs
├── Cli::parse()                          (clap 4.5)
├── init tracing_subscriber
└── match command
    ├── Session::{Persist, Restore}       → commands/session/{persist,restore}.rs
    ├── Hooks::{SessionStart, SessionEnd, PreToolUse, PostToolUse,
    │            UserPromptSubmit, TaskCompleted, PreCompact}
    │                                      → commands/hooks/<name>.rs
    ├── Memory::{Capture, Inject}         → commands/memory/{capture,inject}.rs
    ├── Topic { … }                       → commands/topic.rs
    ├── Divergence { … }                  → commands/divergence.rs
    ├── Setup                              → commands/setup.rs   (writes .claude/settings.json)
    ├── Warmup                             → commands/warmup.rs  (pre-load VRAM)
    └── Watch                              → commands/watch.rs   (directory → auto-index)
```

### 6.3 `train-causal` (`context-graph-embeddings/src/bin/train_causal.rs`)

```
train_causal.rs
├── parse args (learning rate, stages, output dir)
├── build CausalModel (models/pretrained/causal)
├── load training data                    (training/data.rs)
├── 3-stage pipeline                       (training/pipeline.rs)
│   ├── Stage 1: projection warmup
│   ├── Stage 2: LoRA activation           (training/lora.rs)
│   └── Stage 3: directional emphasis
├── loss = InfoNCE + directional + separation   (training/loss.rs)
├── optimizer = AdamW                      (training/optimizer.rs)
└── checkpoint per-stage + final weights
```

---

## 7. Build / Package Configuration

### 7.1 Workspace `Cargo.toml`

| Section | Value |
|---|---|
| `resolver` | 2 |
| `members` | 10 crates (see §1) |
| `default-members` | 9 crates (all except `context-graph-benchmark`) |
| `workspace.package.version` | 0.1.0 |
| `workspace.package.edition` | 2021 |
| `workspace.package.rust-version` | 1.75 |
| `workspace.package.license` | PolyForm-Noncommercial-1.0.0 |

### 7.2 Key workspace.dependencies

| Dep | Version | Features |
|---|---|---|
| tokio | 1.35 | full |
| serde | 1.0 | derive |
| serde_json | 1.0 | — |
| bincode | 1.3 | — |
| toml | 0.8 | — |
| thiserror | 1.0 | — |
| anyhow | 1.0 | — |
| tracing | 0.1 | — |
| tracing-subscriber | 0.3 | json, env-filter |
| uuid | 1.6 | v4, v5, serde |
| chrono | 0.4 | serde |
| candle-core | 0.9.2-alpha | cuda |
| candle-nn | 0.9.2-alpha | cuda |
| llama-cpp-2 | 0.1 | — (cuda via crate feature) |
| async-trait | 0.1 | — |
| config | 0.14 | — |
| rand | 0.8 | — |
| rand_distr | 0.4 | — |

### 7.3 Feature Chains

| Feature | Defined on | Propagates to |
|---|---|---|
| `cuda` (default) | mcp | cuda, embeddings, storage, causal-agent, + candle |
| `llm` (default) | mcp | causal-agent/llm, graph-agent/llm (→ llama-cpp-2) |
| `candle` | embeddings | — (compile_error! if absent) |
| `test-utils` | core, graph-agent | enables stub modules |
| `real-embeddings` | benchmark | embeddings + mcp |
| `benchmark-tests` | benchmark | benchmark-only test suite |
| `faiss-working` | cuda | FAISS GPU (manual rebuild) |

### 7.4 Profiles

```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true

[profile.dev]
opt-level = 0
debug = 1                        # line info only
incremental = true
split-debuginfo = "unpacked"

[profile.dev.package."*"]
debug = false                    # external deps: no debug info
```

### 7.5 Binaries

| Binary | Crate | Path |
|---|---|---|
| `context-graph-mcp` | mcp | `src/main.rs` |
| `context-graph-cli` | cli | `src/main.rs` |
| `train-causal` | embeddings | `src/bin/train_causal.rs` |
| ~40 `*_bench` binaries | benchmark | `src/bin/*.rs` (build with `cargo -p context-graph-benchmark`) |

---

## 8. Cross-References

- Architecture + tool counts → [01_system_overview.md](01_system_overview.md)
- Config surface → [03_configuration.md](03_configuration.md)
- Full CF + key layout → [04_database_schema.md](04_database_schema.md)
- Embedder deep dives (E1-E13) → [05_embedding_subsystem.md](05_embedding_subsystem.md)
- RRF / pipeline / weight profiles → [06_search_and_fusion.md](06_search_and_fusion.md)
- K-NN / typed edges / causal → [07_graph_and_causal.md](07_graph_and_causal.md)
- Per-tool MCP reference (75) → [08_mcp_tools_reference.md](08_mcp_tools_reference.md)
- Storage internals → [09_storage_layer.md](09_storage_layer.md)
- CLI → [10_cli_reference.md](10_cli_reference.md)
- CUDA layer → [11_cuda_acceleration.md](11_cuda_acceleration.md)
- HDBSCAN / BIRCH / topics → [12_clustering_and_topics.md](12_clustering_and_topics.md)
- Synergy matrix + profiles → [13_teleological_subsystem.md](13_teleological_subsystem.md)
- Test suite → [14_test_suite.md](14_test_suite.md)
- Verification → [15_verification_report.md](15_verification_report.md)
- Training export → [16_training_subsystem.md](16_training_subsystem.md)
- Constellation compiler → [17_constellation_subsystem.md](17_constellation_subsystem.md)
- Contrastive + typed edges → [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md)
- CAMD-DPO (v4.2) → [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md)
