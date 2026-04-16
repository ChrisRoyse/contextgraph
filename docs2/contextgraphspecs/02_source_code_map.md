# Source Code Map

**Source files covered:** All .rs files in workspace (~1,150 files across 10 crates; 1,073 .rs files under `crates/*/src/`, plus tests/examples/benches)

**Workspace:** `/home/cabdru/contextgraph`
**Language:** Rust (edition 2021, MSRV 1.75)
**Resolver:** Cargo workspace v2

---

## 1. File Tree by Crate

### 1.1 context-graph-core (~99K lines)

Core domain types, traits, and business logic for the 13-embedder semantic memory system.

```
crates/context-graph-core/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: re-exports all public types and traits
    monitoring.rs                            # System health monitoring traits (LayerStatusProvider, SystemMonitor)
    causal/
      mod.rs                                 # Causal inference module: SCM, omni-directional inference, E5 asymmetric similarity
      asymmetric.rs                          # E5 asymmetric causal similarity (cause->effect=1.2, effect->cause=0.8)
      chain.rs                               # Transitive causal chain reasoning with multi-hop attenuation
      inference.rs                           # Omni-directional inference (forward, backward, bidirectional, bridge, abduction)
      scm.rs                                 # Structural Causal Model (SCM) with DAG, adjacency, and interventions
    clustering/
      mod.rs                                 # Multi-space clustering types for topic synthesis (HDBSCAN + BIRCH)
      birch.rs                               # BIRCH incremental clustering (online topic detection)
      cluster.rs                             # Cluster type with centroid, quality metrics, and member tracking
      error.rs                               # Clustering error types
      fingerprint_matrix.rs                  # Matrix representation of fingerprints for clustering
      hdbscan.rs                             # HDBSCAN density-based clustering implementation
      manager.rs                             # ClusteringManager: topic detection, lifecycle, and cap enforcement
      membership.rs                          # ClusterMembership: per-space cluster assignment tracking
      persistence.rs                         # Cluster state persistence to RocksDB
      stability.rs                           # TopicStability metrics (churn, drift, phase transitions)
      synthesizer.rs                         # Topic synthesizer: merges per-space clusters into cross-space topics
      topic.rs                               # Topic and TopicProfile types with weighted agreement scoring
    code/
      mod.rs                                 # Code query type detection (Code2Code vs Text2Code) for E7 similarity
    constellation/
      mod.rs                                 # Phase 2 constellation compiler entry (CONSTELLATION_VERSION=1, RESERVOIR_SAMPLE_SIZE=1024, MIN_CONSTELLATION_MEMBERS=3)
      compiler.rs                            # ConstellationAccumulator two-pass build (observe -> finalize), compile_constellation, score_memory_against_constellation
      reservoir.rs                           # ReservoirSample: Vitter Algorithm R for streaming percentiles (bounded memory)
      types.rs                               # Constellation, ConstellationSelector, EmbedderStats, VectorKind, ConstellationScoringResult
      welford.rs                             # WelfordStats / WelfordVector online mean + variance (per-coordinate centroid)
      tests.rs                               # Constellation compiler unit tests
    contrastive/
      mod.rs                                 # Phase 3 contrastive pair miner entry (CONTRASTIVE_PAIR_VERSION=1, SRC-3 [0,1] similarity contract)
      mining.rs                              # Pure-fn miner: similarity_profile, classify_anomaly, mine_pair_from_candidate
      types.rs                               # ContrastivePair, AnomalyKind (6 variants), MiningConfig, MiningSummary, ContrastiveError
      tests.rs                               # Miner unit tests
    config/
      mod.rs                                 # Configuration management (Config struct, loading, validation)
      constants.rs                           # System-wide constants (thresholds, limits, defaults)
      embedder_config.rs                     # Per-embedder static configuration (dimensions, quantization)
      sub_configs.rs                         # Sub-configuration structs (Cuda, Embedding, Index, Logging, Mcp, etc.)
      tests/
        mod.rs                               # Test module declarations
        default_tests.rs                     # Default config value tests
        mcp_config_tests.rs                  # MCP-specific config tests
        serialization_tests.rs               # Config serialization roundtrip tests
        validation_tests.rs                  # Config validation constraint tests
    embeddings/
      mod.rs                                 # Embedding types for 13-model teleological array
      category.rs                            # EmbedderCategory enum (Semantic, Temporal, Relational, Structural)
      config.rs                              # EmbedderConfig: static config per embedder (dim, quantization, etc.)
      provider.rs                            # MultiArrayEmbeddingProvider trait re-exports
      stubs.rs                               # StubMultiArrayProvider and stub embedders (test-only)
      token_pruning.rs                       # TokenPruningEmbedding type for E12 ColBERT token-level vectors
      vector.rs                              # DenseVector and BinaryVector types for similarity computation
    entity/
      mod.rs                                 # Entity types for E11 KEPLER embeddings (TransE-based entity detection)
    error/
      mod.rs                                 # Central error types (ContextGraphError, CoreError, sub-errors)
      conversions.rs                         # Error conversion impls (From<> for unified error hierarchy)
      legacy.rs                              # Legacy CoreError type (retained for backward compatibility)
      sub_errors.rs                          # Sub-error types: EmbeddingError, StorageError, IndexError, etc.
      tests.rs                               # Error construction and conversion tests
      unified.rs                             # ContextGraphError: top-level unified error enum
    fusion/
      mod.rs                                 # Multi-embedder fusion: Weighted RRF, weighted sum, normalize minmax
    graph/
      mod.rs                                 # Graph relationship module (E8 directional embeddings)
      asymmetric.rs                          # E8 asymmetric similarity (source->target=1.2, target->source=0.8)
    graph_linking/
      mod.rs                                 # K-NN graph construction and multi-relation typed edges
      direction.rs                           # DirectedRelation for asymmetric edges (E5, E8)
      edge_builder.rs                        # Edge builder for constructing typed edges from embedder patterns
      edge_type.rs                           # 8 graph linking edge types derived from embedder agreement
      embedder_edge.rs                       # K-NN graph edge per embedder (neighbor ID, similarity, rank)
      error.rs                               # Fail-fast error types for graph linking operations
      knn_graph.rs                           # KnnGraph: per-embedder K-NN graph with NN-Descent construction
      nn_descent.rs                          # NN-Descent algorithm for approximate K-NN graph building
      service.rs                             # GraphLinkingService: orchestrates K-NN graph build and typed edge creation
      storage_keys.rs                        # EdgeStorageKey and TypedEdgeStorageKey for RocksDB keys
      thresholds.rs                          # EdgeThresholds: per-type similarity thresholds for edge creation
      typed_edge.rs                          # TypedEdge: multi-relation edge with embedder agreement metadata
      weight_projector.rs                    # Learned weight projection for edge weights (MLP inference)
    index/
      mod.rs                                 # Index layer: multi-space HNSW and inverted indexes
      config.rs                              # Index configuration (DistanceMetric, HNSW params)
      error.rs                               # Index error types
      manager.rs                             # IndexManager: HNSW index lifecycle and search coordination
      splade_impl.rs                         # InMemorySpladeIndex: in-memory sparse inverted index for E13
      status.rs                              # Index status types (loaded, loading, error)
      hnsw_impl/
        mod.rs                               # HNSW implementation module
        multi_space.rs                       # MultiSpaceHnswIndex: manages 15 HNSW indexes per embedder
        multi_space_trait.rs                 # Trait for multi-space HNSW operations
        persistence.rs                       # HNSW index persistence to/from disk
        real_hnsw.rs                         # Real HNSW index implementation using hnsw_rs crate
        types.rs                             # HNSW types (HnswSearchResult, HnswEntry)
        tests/
          mod.rs                             # HNSW test module declarations
          multi_space_tests.rs               # Multi-space HNSW search tests
          persistence_tests.rs               # HNSW save/load roundtrip tests
          real_hnsw_tests.rs                 # Real HNSW index correctness tests
    injection/
      mod.rs                                 # Injection pipeline types for context injection
      budget.rs                              # TokenBudget: token allocation limits for context injection
      candidate.rs                           # InjectionCandidate: memory candidate with scores and category
      formatter.rs                           # Injection output formatter (compact/standard/verbose)
      pipeline.rs                            # InjectionPipeline: full context injection orchestrator
      priority.rs                            # InjectionCategory and priority ordering logic
      result.rs                              # InjectionResult: output from context injection pipeline
      temporal_enrichment.rs                 # TemporalBadge: temporal context annotations (freshness, recurrence)
    memory/
      mod.rs                                 # Memory capture types (Memory, MemorySource, Session)
      ast_chunker.rs                         # AST-based code chunker using tree-sitter (Rust)
      capture.rs                             # Memory capture pipeline (content -> chunks -> fingerprints)
      chunker.rs                             # TextChunker: text splitting with overlap and metadata
      code_capture.rs                        # Code file capture with AST chunking
      code_watcher.rs                        # Code directory watcher for auto-indexing source files
      manager.rs                             # MemoryManager: topic detection, lifecycle, and cap enforcement
      session.rs                             # Session types (SessionStatus, SessionCache)
      source.rs                              # MemorySource enum (Hook, Response, MDChunk, FileChunk)
      store.rs                               # MemoryStore trait and InMemoryMemoryStore
      watcher.rs                             # File watcher for markdown file changes
    quantization/
      mod.rs                                 # Embedding quantization (INT4, INT8, FP16) for compression
      accuracy.rs                            # Quantization accuracy metrics (NRMSE, RMSE)
      batch.rs                               # Batch quantize/dequantize operations
      fingerprint.rs                         # SemanticFingerprint quantization (~46KB -> ~11KB)
      traits.rs                              # Quantizable trait definition
      types.rs                               # Quantized embedding types (QuantizedFloat8, QuantizedPQ8, etc.)
    retrieval/
      mod.rs                                 # Multi-embedding query executor with 5-stage pipeline
      aggregation.rs                         # Score aggregation across embedding spaces
      config.rs                              # Retrieval configuration (PipelineStageConfig)
      detector.rs                            # Query type detection for pipeline routing
      distance.rs                            # Distance metric implementations
      divergence.rs                          # Divergence detection from recent activity patterns
      executor.rs                            # Multi-space query executor trait
      in_memory_executor.rs                  # In-memory executor for testing
      insight_annotation.rs                  # Insight annotations on search results
      multi_space.rs                         # Multi-space search with RRF fusion
      query.rs                               # Query types and parsing
      result.rs                              # Search result types
      retriever.rs                           # Retriever orchestrator
      similarity.rs                          # Similarity computation helpers
      sparse_index.rs                        # Sparse index operations for E6/E13
      teleological_query.rs                  # Teleological query with weight profiles
      teleological_result.rs                 # Teleological search result types
      tests.rs                               # Retrieval tests
    similarity/
      mod.rs                                 # Cross-space similarity engine (RRF, weighted average, MaxPooling)
      config.rs                              # Similarity engine configuration
      default_engine.rs                      # DefaultCrossSpaceEngine implementation
      engine.rs                              # CrossSpaceSimilarityEngine trait definition
      error.rs                               # Similarity computation error types
      explanation.rs                         # Similarity explanation types for debugging
      multi_utl.rs                           # Multi-space UTL (Unified Teleological Layer) computation
      result.rs                              # Similarity result types
      sparse.rs                              # Sparse vector similarity (Jaccard, dot product)
      tests.rs                               # Similarity engine tests
      token_level.rs                         # Token-level similarity for E12 ColBERT MaxSim
      dense/
        mod.rs                               # Dense vector similarity module
        error.rs                             # Dense similarity error types
        primitives.rs                        # Cosine similarity, dot product, L2 distance
        simd.rs                              # SIMD-accelerated similarity primitives
        tests/
          mod.rs                             # Dense similarity test module
          integration_tests.rs               # Integration tests for dense similarity
          primitives_tests.rs                # Primitive operation tests
          simd_tests.rs                      # SIMD acceleration tests
    stubs/
      mod.rs                                 # Stub implementations (test-only, gated by test-utils feature)
      multi_array_stub.rs                    # StubMultiArrayProvider for test embedding generation
      teleological_store_stub/
        mod.rs                               # InMemoryTeleologicalStore module
        content.rs                           # Content storage for in-memory store
        search.rs                            # In-memory search implementation (O(n) scan)
        similarity.rs                        # Similarity computation for in-memory store
        tests.rs                             # In-memory store tests
        trait_impl.rs                        # TeleologicalMemoryStore trait implementation
    teleological/
      mod.rs                                 # Teleological module: cross-embedding synergy and fusion
      comparison_error.rs                    # Comparison error types
      embedder.rs                            # Embedder enum and EmbedderGroup classification
      groups.rs                              # EmbedderGroup types and group aggregation
      meaning.rs                             # MeaningExtractionConfig for semantic extraction
      resolution.rs                          # Resolution types for multi-resolution hierarchy
      types.rs                               # Core teleological types (DomainType, TuckerCore, etc.)
      vector.rs                              # TeleologicalVector: fused multi-embedding representation
      comparator/
        mod.rs                               # Teleological comparator module
        batch.rs                             # Batch comparison operations
        result.rs                            # Comparison result types
        strategies.rs                        # Comparison strategy implementations
        teleological.rs                      # TeleologicalComparator: cross-space comparison
        tests.rs                             # Comparator tests
      matrix_search/
        mod.rs                               # Matrix-based search module
        config.rs                            # Matrix search configuration
        embedder_names.rs                    # Embedder name constants for matrix search
        types.rs                             # Matrix search types
        weights.rs                           # Matrix search weight management
        search/
          mod.rs                             # Search submodule
          clustering.rs                      # Clustering-based matrix search
          comprehensive.rs                   # Comprehensive matrix search
          core.rs                            # Core matrix search implementation
        strategies/
          mod.rs                             # Strategy submodule
          computer.rs                        # Strategy score computation
          helpers.rs                         # Strategy helper functions
        tests/
          mod.rs                             # Matrix search test module
          advanced.rs                        # Advanced matrix search tests
          basic.rs                           # Basic matrix search tests
      profile/
        mod.rs                               # Teleological profile module
        fusion_strategy.rs                   # Profile fusion strategies (MultiSpace, Pipeline, etc.)
        metrics.rs                           # ProfileMetrics: performance tracking per profile
        task_type.rs                         # TaskType enum for task-specific profiles
        teleological.rs                      # TeleologicalProfile: per-task fusion configuration
        tests.rs                             # Profile tests
      services/
        mod.rs                               # Teleological services module
        correlation_extractor.rs             # Cross-embedding correlation extraction
        fusion_engine.rs                     # FusionEngine: multi-strategy fusion orchestrator
        group_aggregator.rs                  # Group aggregation service
        meaning_pipeline.rs                  # Meaning extraction pipeline
        multi_space_retriever.rs             # Multi-space retrieval service
        synergy_service.rs                   # Synergy matrix computation service
        tucker_decomposer.rs                 # Tucker decomposition for tensor reduction
        profile_manager/
          mod.rs                             # Profile manager module
          builtin.rs                         # Built-in profile definitions
          manager.rs                         # ProfileManager: CRUD for teleological profiles
          tests.rs                           # Profile manager tests
          types.rs                           # Profile manager types
      synergy_matrix/
        mod.rs                               # SynergyMatrix: 13x13 cross-embedding weights
        accessors.rs                         # Matrix element accessor methods
        analysis.rs                          # Synergy matrix analysis utilities
        constants.rs                         # Synergy matrix constants
        constructors.rs                      # SynergyMatrix construction methods
        presets.rs                           # Predefined synergy matrix presets
        types.rs                             # Synergy matrix types
        validation.rs                        # Synergy matrix validation
        tests/
          mod.rs                             # Synergy matrix test module
          analysis.rs                        # Analysis tests
          basic.rs                           # Basic construction tests
          presets.rs                         # Preset tests
          validation.rs                      # Validation tests
    training/
      mod.rs                                 # TrainingRecord v2 bincode struct (TRAINING_RECORD_VERSION=2, NUM_CROSS_CORRELATIONS=78, NUM_GROUP_ALIGNMENTS=6, NUM_EDGE_TYPE_DISTRIBUTION=8)
      computation.rs                         # Pure fusion helpers: topic_profile_or_fallback, compute_cross_correlations, compute_group_alignments
      edge_signature.rs                      # compute_edge_type_distribution: count outgoing typed edges by GraphLinkEdgeType into 8-dim array
      temporal.rs                            # Phase 5 temporal labels: PeriodicBucket, TemporalLabels (hour/day/month, age, session position)
      tests.rs                               # Training-record computation tests
    typed_edge_export/
      mod.rs                                 # Phase 4 typed-edge export types: TypedEdgeTrainingRecord, LLMValidationSummary, LLMVerdict (TYPED_EDGE_RECORD_VERSION=1)
      tests.rs                               # Typed-edge export unit tests
    llm_edge_validation/
      mod.rs                                 # Phase 4 LLM validation types: LLMEdgeValidation (LLM_EDGE_VALIDATION_VERSION=1), prompt_hash, validator_version
      tests.rs                               # LLM edge validation unit tests
    traits/
      mod.rs                                 # Core trait definitions (TeleologicalMemoryStore, MultiArrayEmbeddingProvider)
      multi_array_embedding.rs               # MultiArrayEmbeddingProvider: 13-embedding generation trait
      teleological_memory_store_tests.rs     # TeleologicalMemoryStore conformance tests
      teleological_memory_store/
        mod.rs                               # TeleologicalMemoryStore module
        backend.rs                           # Storage backend abstraction
        ext.rs                               # Extension methods for store trait
        options.rs                           # TeleologicalSearchOptions and store options
        result.rs                            # Store result types
        store.rs                             # TeleologicalMemoryStore trait definition
    types/
      mod.rs                                 # Core domain types
      audit.rs                               # AuditLogEntry for provenance tracking
      causal_relationship.rs                 # CausalRelationship, LLMProvenance, CausalSearchResult
      code_entity.rs                         # CodeEntity, CodeEntityType, CodeLanguage, Visibility
      discovery.rs                           # DiscoveryCycleResult, ServiceStatus for LLM agents
      file_index.rs                          # FileIndexEntry, FileWatcherStats
      source_metadata.rs                     # SourceMetadata: origin tracking for memories
      fingerprint/
        mod.rs                               # Fingerprint types module
        sparse.rs                            # SparseVector for E6/E13 keyword embeddings
        semantic/
          mod.rs                             # SemanticFingerprint module
          constants.rs                       # Dimension constants (E1_DIM=1024, E7_DIM=1536, etc.)
          fingerprint.rs                     # SemanticFingerprint: 13-embedding array struct
          slice.rs                           # Safe slice accessors for fingerprint embeddings
          validation.rs                      # Fingerprint validation (dimensions, NaN checks)
          tests/
            mod.rs                           # Fingerprint test module
            core_tests.rs                    # Core fingerprint tests
            storage_tests.rs                 # Fingerprint storage roundtrip tests
            task_core_003_tests.rs           # TASK-CORE-003 compliance tests
            validation_tests.rs              # Validation constraint tests
        teleological/
          mod.rs                             # TeleologicalFingerprint module
          core.rs                            # TeleologicalFingerprint: ~63KB full fingerprint struct
          test_helpers.rs                    # Test helper functions for fingerprints
          tests.rs                           # Teleological fingerprint tests
          types.rs                           # Teleological fingerprint types (TeleologicalArray, TopicProfile)
      graph_edge/
        mod.rs                               # GraphEdge type module
        edge.rs                              # GraphEdge struct (source, target, type, weight, metadata)
        modulation.rs                        # Edge weight modulation logic
        traversal.rs                         # Graph traversal helpers
        tests_constructor.rs                 # Edge construction tests
        tests_modulation.rs                  # Modulation tests
        tests_struct.rs                      # Struct field tests
        tests_traversal.rs                   # Traversal tests
      memory_node/
        mod.rs                               # MemoryNode type module
        metadata.rs                          # Memory metadata types
        node.rs                              # MemoryNode struct
        validation.rs                        # Memory node validation
        tests_metadata.rs                    # Metadata tests
        tests_node.rs                        # Node tests
        tests_serialization.rs               # Serialization roundtrip tests
    weights/
      mod.rs                                 # Weight profile configuration for 13-embedder search
  examples/
    divergence_fsv_test.rs                   # Divergence detection FSV example
    fsv_line_numbers.rs                      # Full state verification with line numbers example
    fsv_source_metadata.rs                   # FSV source metadata example
    tree_sitter_test.rs                      # Tree-sitter AST parsing example
  tests/
    file_watcher_integration.rs              # File watcher integration test (requires test-utils)
    injection_manual_test.rs                 # Manual injection pipeline test
    rrf_edge_cases.rs                        # RRF fusion edge case tests
    source_metadata_integration.rs           # Source metadata integration test (requires test-utils)
    synthesizer_edge_case_test.rs            # Topic synthesizer edge case tests
```

### 1.2 context-graph-benchmark (~87K lines)

Benchmarking suite comparing 13-embedder multi-space fingerprinting vs single-embedder RAG.

```
crates/context-graph-benchmark/
  Cargo.toml
  benches/
    comparative_suite.rs                     # Criterion benchmark: multi-space vs single-embedder
    scaling_analysis.rs                      # Criterion benchmark: scaling analysis across tiers
  src/
    lib.rs                                   # Crate root: re-exports BenchmarkConfig, DatasetGenerator, runners, metrics
    ablation.rs                              # Ablation studies: measure impact of removing individual embedders
    config.rs                                # BenchmarkConfig, TierConfig (100 to 10M memories, 5 to 500 topics)
    stress_corpus.rs                         # Stress corpus generation for load testing
    util.rs                                  # Benchmark utility functions
    baseline/
      mod.rs                                 # Baseline module: single-embedder comparison targets
      single_embedder.rs                     # Single-embedder RAG baseline implementation
      single_hnsw.rs                         # Single-HNSW baseline for ANN comparison
    bin/
      causal_accuracy_bench.rs               # Causal direction accuracy benchmark
      causal_discovery_mcp_bench.rs          # Causal discovery via MCP protocol benchmark (real-embeddings)
      causal_e2e_bench.rs                    # End-to-end causal discovery benchmark
      causal_embedding_bench.rs              # Causal embedding quality benchmark
      causal_perf_bench.rs                   # Causal pipeline performance benchmark
      causal_prompt_experiment_bench.rs      # Causal prompt engineering experiments (real-embeddings)
      causal_provenance_bench.rs             # Causal provenance tracking benchmark
      causal_provenance_e2e_bench.rs         # End-to-end causal provenance benchmark
      causal_provenance_llm_bench.rs         # LLM causal provenance benchmark (real-embeddings)
      causal_realdata_bench.rs               # Causal benchmark with real dataset
      causal_span_fix_bench.rs              # Causal span fix verification (real-embeddings)
      code_fingerprint_bench.rs              # E7 code fingerprint benchmark (real-embeddings)
      disk_space_bench.rs                    # Disk space usage benchmark
      e10_modifier_tuning.rs                 # E10 multimodal modifier tuning benchmark
      e11_entity_bench.rs                    # E11 entity embedding benchmark
      e1_semantic_bench.rs                   # E1 semantic embedding benchmark
      e2_manual_verification.rs              # E2 temporal recency manual verification (real-embeddings)
      e4_hybrid_session_bench.rs             # E4 hybrid session benchmark
      e7_bench.rs                            # E7 code embedding benchmark
      e7_realdata_bench.rs                   # E7 real data benchmark (real-embeddings)
      embedder_impact_bench.rs               # Individual embedder impact measurement
      embedder_impact_realdata_bench.rs      # Embedder impact with real data
      embedder_stress.rs                     # Embedder stress test binary
      gpu_bench.rs                           # GPU acceleration benchmark
      graph_bench.rs                         # Graph structure benchmark
      graph_linking_bench.rs                 # K-NN graph linking benchmark
      graph_linking_gpu_bench.rs             # GPU-accelerated graph linking (real-embeddings)
      graph_linking_real_bench.rs            # Graph linking with real data
      graph_realdata_bench.rs                # Graph benchmark with real data
      hf_bench.rs                            # HuggingFace model loading benchmark
      mcp_bench.rs                           # MCP protocol round-trip benchmark
      model_injection_bench.rs               # Model injection benchmark (real-embeddings)
      model_injection_rigorous_bench.rs      # Rigorous model injection verification (real-embeddings)
      multimodal_bench.rs                    # E10 multimodal benchmark
      multimodal_realdata_bench.rs           # E10 multimodal with real data
      realdata_bench.rs                      # General real data benchmark
      report.rs                              # Benchmark report generator binary
      sparse_bench.rs                        # E6/E13 sparse embedding benchmark
      spawn_blocking_bench.rs                # tokio::spawn_blocking overhead benchmark (real-embeddings)
      stub_fix_verification.rs               # Stub fix verification binary
      temporal_bench.rs                      # E2/E3/E4 temporal benchmark
      temporal_realdata_bench.rs             # Temporal benchmark with real data
      unified_realdata_bench.rs              # Unified multi-space real data benchmark
      validation_bench.rs                    # Validation suite benchmark
      validation_realdata_bench.rs           # Validation with real data
      weight_profile_bench.rs                # Weight profile tuning benchmark
    causal_bench/
      mod.rs                                 # Causal benchmark module
      comparison.rs                          # Causal baseline comparison logic
      data_loader.rs                         # Causal benchmark data loader
      metrics.rs                             # Causal accuracy metrics
      phases.rs                              # Benchmark phases (training, evaluation)
      provider.rs                            # Embedding provider for causal benchmarks
      report.rs                              # Causal benchmark report generation
    datasets/
      mod.rs                                 # Dataset module: synthetic data generation
      beir_loader.rs                         # BEIR dataset loader for IR benchmarks
      causal.rs                              # Causal relationship dataset generator
      e11_entity.rs                          # E11 entity benchmark dataset
      e1_semantic.rs                         # E1 semantic similarity dataset
      embedder_impact.rs                     # Embedder impact analysis dataset
      generator.rs                           # DatasetGenerator: synthetic corpus builder
      graph_linking.rs                       # Graph linking benchmark dataset
      ground_truth.rs                        # GroundTruth: expected retrieval results
      multimodal.rs                          # E10 multimodal benchmark dataset
      sparse.rs                              # E6/E13 sparse benchmark dataset
      temporal.rs                            # Temporal benchmark dataset
      temporal_sessions.rs                   # Temporal session-based dataset
      topic_clusters.rs                      # Topic cluster benchmark dataset
    metrics/
      mod.rs                                 # Metrics module: retrieval, clustering, performance
      causal.rs                              # Causal accuracy metrics (precision, recall, F1)
      clustering.rs                          # Clustering metrics (NMI, ARI, Silhouette)
      divergence.rs                          # Divergence detection metrics
      e11_entity.rs                          # E11 entity metrics
      e1_semantic.rs                         # E1 semantic metrics
      e4_hybrid_session.rs                   # E4 hybrid session metrics
      e7_code.rs                             # E7 code retrieval metrics
      e7_iou.rs                              # E7 code IoU (Intersection over Union)
      embedder_contribution.rs               # Per-embedder contribution analysis
      graph_linking.rs                       # Graph linking quality metrics
      graph_structure.rs                     # Graph structure metrics
      multimodal.rs                          # Multimodal retrieval metrics
      performance.rs                         # Latency, throughput, memory profiling
      resource_usage.rs                      # Resource usage tracking
      retrieval.rs                           # Retrieval metrics (P@K, R@K, MRR, NDCG, MAP)
      sparse.rs                              # Sparse retrieval metrics
      temporal.rs                            # Temporal retrieval metrics
      temporal_realdata.rs                   # Temporal real data metrics
      validation.rs                          # Validation metrics
    realdata/
      mod.rs                                 # Real data benchmark module
      config.rs                              # Real data benchmark configuration
      embedder.rs                            # Real data embedder wrapper
      ground_truth.rs                        # Real data ground truth loader
      loader.rs                              # Dataset loader for real benchmark data
      report.rs                              # Real data benchmark report
      results.rs                             # Real data benchmark results
      runner.rs                              # Real data benchmark runner
      temporal_injector.rs                   # Temporal signal injector for real data
    reports/
      mod.rs                                 # Report generation module
      json.rs                                # JSON report formatter
      markdown.rs                            # Markdown report formatter
    runners/
      mod.rs                                 # Benchmark runner module
      causal.rs                              # Causal benchmark runner
      comparative.rs                         # Comparative benchmark runner (multi-space vs single)
      e11_entity.rs                          # E11 entity benchmark runner
      e1_semantic.rs                         # E1 semantic benchmark runner
      e4_hybrid_session.rs                   # E4 hybrid session benchmark runner
      embedder_impact.rs                     # Embedder impact benchmark runner
      failfast.rs                            # Fail-fast benchmark runner (panics on failure)
      graph_linking.rs                       # Graph linking benchmark runner
      graph_realdata.rs                      # Graph real data benchmark runner
      multimodal.rs                          # Multimodal benchmark runner
      retrieval.rs                           # Retrieval benchmark runner
      scaling.rs                             # Scaling analysis runner (100 to 10M memories)
      sparse.rs                              # Sparse benchmark runner
      temporal.rs                            # Temporal benchmark runner
      temporal_realdata.rs                   # Temporal real data runner
      topic.rs                               # Topic benchmark runner
      unified_realdata.rs                    # Unified real data runner
    scaling/
      mod.rs                                 # Scaling analysis module
      corpus_builder.rs                      # Large corpus builder for scaling tests
      degradation.rs                         # Accuracy degradation analysis at scale
      memory_profiler.rs                     # Memory usage profiler
    tuning/
      mod.rs                                 # Tuning module
      e7_tuning.rs                           # E7 code embedder parameter tuning
    validation/
      mod.rs                                 # Validation module
      ablation_validation.rs                 # Ablation study validation
      arch_compliance.rs                     # Architecture compliance validation
      asymmetric_validation.rs               # Asymmetric similarity validation (E5, E8)
      config_validation.rs                   # Configuration validation
      e1_foundation.rs                       # E1 foundation model validation
      fusion_validation.rs                   # Fusion strategy validation
      ground_truth_validation.rs             # Ground truth verification
      results_aggregator.rs                  # Results aggregation across tiers
      temporal_validation.rs                 # Temporal embedder validation
```

### 1.3 context-graph-embeddings (~75K lines)

GPU-accelerated embedding pipeline with 13 embedder model implementations.

```
crates/context-graph-embeddings/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: mandatory candle feature gate, re-exports all public API
    global_provider.rs                       # Global warm-loaded embedding provider singleton
    adapters/
      mod.rs                                 # Adapter module for bridging models to external traits
      code_provider.rs                       # E7CodeEmbeddingProvider: wraps CodeModel for code embedding
    batch/
      mod.rs                                 # Batch processing infrastructure for embedding requests
      processor/
        mod.rs                               # BatchProcessor module
        config.rs                            # Batch processor configuration
        core.rs                              # BatchProcessor core logic
        stats.rs                             # Batch processing statistics
        submit.rs                            # Batch request submission
        worker.rs                            # Batch worker thread
      types/
        mod.rs                               # Batch types module
        batch.rs                             # Batch type (assembled batch ready for GPU)
        queue.rs                             # BatchQueue: per-model pending request queue
        request.rs                           # BatchRequest: individual request with async channel
        stats.rs                             # Batch statistics types
        tests.rs                             # Batch type tests
    bin/
      train_causal.rs                        # E5 causal embedder training binary
    cache/
      mod.rs                                 # High-performance embedding cache using moka (concurrent)
    config/
      mod.rs                                 # Embedding configuration module
      batch.rs                               # BatchConfig: batch size, timeout, queue depth
      cache.rs                               # CacheConfig: TTL, max entries, eviction policy
      gpu.rs                                 # GpuConfig: device selection, memory limits
      models.rs                              # ModelPathConfig: HuggingFace model paths and checksums
    error/
      mod.rs                                 # Error module
      tests.rs                               # Error construction tests
      tests_extended.rs                      # Extended error scenario tests
      types.rs                               # EmbeddingError and EmbeddingResult types
    gpu/
      mod.rs                                 # GPU acceleration module (Candle + CUDA)
      ops.rs                                 # GPU-accelerated ops (L2 norm, cosine sim, matmul)
      device/
        mod.rs                               # GPU device module
        accessors.rs                         # Device accessor methods
        core.rs                              # GpuDevice singleton with CUDA detection
        utils.rs                             # GPU utility functions
      memory/
        mod.rs                               # GPU memory management module
        budget.rs                            # VRAM budget tracking
        error.rs                             # GPU memory error types
        pool.rs                              # GPU memory pool
        pressure.rs                          # Memory pressure detection
        slots.rs                             # GPU memory slot management
        stats.rs                             # Memory usage statistics
        tracker.rs                           # VRAM allocation tracker
      model_loader/
        mod.rs                               # Model loader module
        batch_loader.rs                      # Batch model loader (loads multiple models)
        config.rs                            # Model loader configuration
        embedding_loader.rs                  # Embedding layer loader (token embeddings, position)
        error.rs                             # Model loader error types
        layer_loader.rs                      # Transformer layer loader (attention, FFN)
        loader.rs                            # Main model loader orchestrator
        tensor_utils.rs                      # Tensor utility functions
        unified.rs                           # Unified model loader for all 13 models
        weights.rs                           # Weight loading and verification
      tensor/
        mod.rs                               # Tensor module
        core.rs                              # GpuTensor: type-safe tensor wrapper
        ops.rs                               # Tensor operations
    models/
      mod.rs                                 # Model management and ModelRegistry
      memory_tracker/
        mod.rs                               # MemoryTracker module
        core.rs                              # GPU memory allocation tracker
        tests/
          mod.rs                             # Memory tracker test module
          advanced.rs                        # Advanced memory tracking tests
          basic.rs                           # Basic memory tracking tests
      factory/
        mod.rs                               # ModelFactory module
        default_factory.rs                   # DefaultModelFactory: creates all 13 model instances
        trait_impl.rs                        # ModelFactory trait implementation
        tests/
          mod.rs                             # Factory test module
          factory_tests.rs                   # Factory construction tests
          memory_tests.rs                    # Memory estimation tests
          model_creation_tests.rs            # Model creation tests
      registry/
        mod.rs                               # ModelRegistry module
        config.rs                            # ModelRegistryConfig: concurrent loads, cache, etc.
        core.rs                              # ModelRegistry core: manages model lifecycle
        loader.rs                            # Registry model loading logic
        operations.rs                        # Registry operations (load, unload, get)
        stats.rs                             # RegistryStats: load counts, hit rates
        tests.rs                             # Registry tests
        tests_concurrent.rs                  # Concurrent access tests
        tests_mov.rs                         # Model move/swap tests
        tests_stats.rs                       # Statistics tests
        tests_unload.rs                      # Model unload tests
      attention/
        mod.rs                               # Attention mechanism module
        dense.rs                             # Dense (full) attention implementation
        sliding_window.rs                    # Sliding window attention
        tiled.rs                             # Tiled attention for memory efficiency
      pretrained/
        mod.rs                               # Pretrained models module (E1-E13)
        shared.rs                            # Shared utilities across pretrained models
        semantic/
          mod.rs                             # E1 Semantic model (e5-large-v2, 1024D)
          attention.rs                       # Semantic model attention layers
          constants.rs                       # E1 model constants
          embeddings.rs                      # Token/position embedding layers
          encoder.rs                         # BERT encoder stack
          ffn.rs                             # Feed-forward network layers
          gpu_forward.rs                     # GPU-accelerated forward pass
          layer_norm.rs                      # Layer normalization
          loader.rs                          # SafeTensors weight loader
          model.rs                           # SemanticModel struct and construction
          pooling.rs                         # Output pooling (mean, CLS)
          trait_impl.rs                      # EmbeddingModel trait implementation
          types.rs                           # Semantic model types
          tests/
            mod.rs                           # Semantic model test module
            basic.rs                         # Basic model tests
            edge_cases.rs                    # Edge case tests
            embedding.rs                     # Embedding output tests
            helpers.rs                       # Test helpers
            verification.rs                  # Model verification tests
        causal/
          mod.rs                             # E5 Causal model (asymmetric cause/effect)
          config.rs                          # Causal model configuration
          loader.rs                          # Causal weight loader
          model.rs                           # CausalModel struct
          weights.rs                         # Causal model weight types
          tests.rs                           # Causal model tests
          forward/
            mod.rs                           # Forward pass module
            attention.rs                     # Causal attention with direction hints
            encoder.rs                       # Causal encoder with asymmetric projection
            ops.rs                           # Causal forward operations
        code/
          mod.rs                             # E7 Code model (Qodo-Embed-1-1.5B, 1536D)
          attention.rs                       # Code model attention layers
          config.rs                          # Code model configuration
          constants.rs                       # E7 model constants
          forward.rs                         # Code model forward pass
          layers.rs                          # Code model transformer layers
          model.rs                           # CodeModel struct
          position.rs                        # Rotary position embeddings (RoPE)
          weights.rs                         # Code model weight loader
          tests.rs                           # Code model tests
          tests_batch.rs                     # Batch embedding tests
          tests_edge_cases.rs                # Edge case tests
        contextual/
          mod.rs                             # E10 Contextual/Multimodal model (paraphrase, 768D)
          constants.rs                       # E10 model constants
          model.rs                           # ContextualModel struct
          state.rs                           # Model state management
        graph/
          mod.rs                             # E8 Graph model (e5-large-v2, 1024D asymmetric)
          attention.rs                       # Graph model attention
          constants.rs                       # E8 model constants
          encoder.rs                         # Graph encoder
          encoding.rs                        # Structural encoding
          ffn.rs                             # Feed-forward layers
          forward.rs                         # Forward pass
          layer_norm.rs                      # Layer normalization
          model.rs                           # GraphModel struct
          projections.rs                     # Source/target projection heads
          state.rs                           # Model state
          tests.rs                           # Graph model tests
          tests_edge_cases.rs                # Edge case tests
        kepler/
          mod.rs                             # E11 KEPLER/TransE entity model (768D, currently disabled)
          encoding.rs                        # Entity encoding
          forward.rs                         # Single forward pass
          forward_batch.rs                   # Batch forward pass
          model.rs                           # KeplerModel struct
          pooling.rs                         # Output pooling
          trait_impl.rs                      # EmbeddingModel trait implementation
          transe.rs                          # TransE knowledge embedding
          types.rs                           # KEPLER types
          tests.rs                           # KEPLER model tests
        late_interaction/
          mod.rs                             # E12 ColBERT late interaction model (128D/token)
          embedding.rs                       # Token-level embedding generation
          gpu_attention.rs                   # GPU-accelerated attention
          gpu_attention_ops.rs               # GPU attention operations
          gpu_encoder.rs                     # GPU encoder
          gpu_forward.rs                     # GPU forward pass
          gpu_projection.rs                  # GPU projection head
          gpu_utils.rs                       # GPU utility functions
          model.rs                           # LateInteractionModel struct
          scoring.rs                         # MaxSim scoring
          trait_impl.rs                      # EmbeddingModel trait implementation
          types.rs                           # Late interaction types
          tests.rs                           # ColBERT model tests
          tests_batch.rs                     # Batch tests
          tests_extended.rs                  # Extended tests
          tests_token.rs                     # Token-level tests
        rgcn/
          mod.rs                             # R-GCN (Relational Graph Convolutional Network)
          constants.rs                       # R-GCN constants
          model.rs                           # R-GCN model struct
        sparse/
          mod.rs                             # E6/E13 Sparse models (BM25 keyword + SPLADE)
          attention.rs                       # Sparse model attention
          embeddings.rs                      # Sparse token embeddings
          encoder.rs                         # Sparse encoder
          forward.rs                         # Forward pass
          loader.rs                          # Weight loader
          mlm_head.rs                        # Masked language model head (SPLADE)
          model.rs                           # SparseModel struct
          traits.rs                          # Sparse model traits
          types.rs                           # Sparse model types
          tests.rs                           # Sparse model tests
          projection/
            mod.rs                           # Projection module
            error.rs                         # Projection error types
            impl_batch.rs                    # Batch projection
            impl_core.rs                     # Core projection implementation
            types.rs                         # Projection types
            tests.rs                         # Projection tests
        weight_projection/
          mod.rs                             # Learned weight projection for graph edges (MLP)
          constants.rs                       # Weight projection constants
          model.rs                           # CrossEmbedderProjection model
      custom/
        mod.rs                               # Custom (non-pretrained) model module
        hdc/
          mod.rs                             # E9 Hyperdimensional Computing model (1024D)
          encoding.rs                        # HDC character-level encoding
          model.rs                           # HdcModel struct
          operations.rs                      # HDC vector operations (bind, bundle, permute)
          types.rs                           # HDC types
          tests/
            mod.rs                           # HDC test module
            construction.rs                  # Construction tests
            edge_cases.rs                    # Edge case tests
            embedding.rs                     # Embedding output tests
            encoding.rs                      # Encoding tests
            hypervector.rs                   # Hypervector operation tests
            operations.rs                    # Operation correctness tests
        temporal_recent/
          mod.rs                             # E2 Temporal Recent model (sinusoidal PE, 512D)
          compute.rs                         # Temporal decay/freshness computation
          constants.rs                       # E2 constants
          model.rs                           # TemporalRecentModel struct
          timestamp.rs                       # Timestamp processing
          traits.rs                          # Temporal model traits
          tests.rs                           # Temporal recent tests
        temporal_periodic/
          mod.rs                             # E3 Temporal Periodic model (recurring patterns, 512D)
          constants.rs                       # E3 constants
          embed_impl.rs                      # Embedding implementation
          encoding.rs                        # Periodic pattern encoding
          model.rs                           # TemporalPeriodicModel struct
          tests/
            mod.rs                           # Periodic test module
            construction.rs                  # Construction tests
            edge_cases.rs                    # Edge case tests
            embedding.rs                     # Embedding tests
            periodicity.rs                   # Periodicity detection tests
            thread_safety.rs                 # Thread safety tests
            timestamp.rs                     # Timestamp tests
        temporal_positional/
          mod.rs                             # E4 Temporal Positional model (document position, 512D)
          constants.rs                       # E4 constants
          encoding.rs                        # Positional encoding
          model.rs                           # TemporalPositionalModel struct
          session_signature.rs               # Session signature computation
          timestamp.rs                       # Timestamp processing
          tests/
            mod.rs                           # Positional test module
            construction.rs                  # Construction tests
            edge_cases.rs                    # Edge case tests
            embedding.rs                     # Embedding tests
            hybrid.rs                        # Hybrid mode tests
            uniqueness.rs                    # Uniqueness tests
            verification.rs                  # Verification tests
    provider/
      mod.rs                                 # Embedding provider traits and ProductionMultiArrayProvider
      causal_hint.rs                         # CausalHintProvider trait for E5 enhancement
      diagnostic_test.rs                     # Provider diagnostic self-test
      multi_array.rs                         # ProductionMultiArrayProvider: orchestrates all 13 embedders
    pruning/
      mod.rs                                 # Token pruning for E12 ColBERT late-interaction
      config.rs                              # TokenPruningConfig: compression ratio, min tokens
      token_pruner.rs                        # TokenPruningQuantizer implementation
    quantization/
      mod.rs                                 # Quantization module (PQ-8, Float8, Binary, Sparse)
      binary.rs                              # BinaryEncoder: 32x compression for E9 HDC
      edge_case_verification.rs              # Quantization edge case verification
      float8.rs                              # Float8Encoder: 4x compression for E2-E4, E8, E11
      types.rs                               # Quantization types (QuantizedEmbedding, QuantizationMetadata)
      pq8/
        mod.rs                               # PQ-8 Product Quantization module (32x compression)
        encoder.rs                           # PQ-8 encoder/decoder
        persistence.rs                       # Codebook persistence
        training.rs                          # Codebook training (k-means)
        types.rs                             # PQ-8 types (Codebook, Subquantizer)
        tests/
          mod.rs                             # PQ-8 test module
          dimension_tests.rs                 # Dimension handling tests
          encoder_tests.rs                   # Encoder correctness tests
          persistence_tests.rs               # Persistence roundtrip tests
          training_tests.rs                  # Training convergence tests
      router/
        mod.rs                               # QuantizationRouter: selects method per embedder
        encoders.rs                          # Encoder dispatch logic
        tests/
          mod.rs                             # Router test module
          binary_tests.rs                    # Binary encoding tests
          edge_case_tests.rs                 # Edge case tests
          error_tests.rs                     # Error handling tests
          float8_tests.rs                    # Float8 encoding tests
          pq8_tests.rs                       # PQ-8 tests
          routing_tests.rs                   # Routing decision tests
    storage/
      mod.rs                                 # Storage types for quantized embeddings
      multi_space.rs                         # MultiSpaceSearchEngine: Stage 3 multi-space search with RRF
      full_state_verification.rs             # Full state verification tests
      types/
        mod.rs                               # Storage types module
        constants.rs                         # Storage constants (version, sizes)
        fingerprint.rs                       # StoredQuantizedFingerprint (~17KB)
        index_entry.rs                       # IndexEntry for per-embedder HNSW
        query_results.rs                     # EmbedderQueryResult, MultiSpaceQueryResult
        tests.rs                             # Storage type tests
    training/
      mod.rs                                 # Causal embedder fine-tuning infrastructure
      data.rs                                # Training data loader and pair generation
      distillation.rs                        # Knowledge distillation
      evaluation.rs                          # Training evaluation metrics
      lora.rs                                # LoRA (Low-Rank Adaptation) for E5 fine-tuning
      loss.rs                                # Loss functions (InfoNCE, directional, separation)
      multitask.rs                           # Multi-task training
      optimizer.rs                           # AdamW optimizer
      pipeline.rs                            # Training pipeline orchestrator
      trainer.rs                             # Trainer: training loop and checkpoint management
    traits/
      mod.rs                                 # Trait definitions for embedding models and factories
      embedding_model/
        mod.rs                               # EmbeddingModel trait module
        trait_def.rs                         # EmbeddingModel trait definition
        tests/
          mod.rs                             # Trait test module
          core_tests.rs                      # Core trait compliance tests
          integration_tests.rs               # Integration tests
          test_model.rs                      # Test model implementation
      model_factory/
        mod.rs                               # ModelFactory trait module
        config.rs                            # Factory configuration types
        device.rs                            # DevicePlacement enum
        memory.rs                            # Memory estimate constants
        quantization.rs                      # QuantizationMode enum
        trait_def.rs                         # ModelFactory trait definition
        tests.rs                             # Factory trait tests
    types/
      mod.rs                                 # Core embedding types (ModelId, ModelInput, MultiArrayEmbedding)
      concatenated/
        mod.rs                               # MultiArrayEmbedding: 13-embedding array
        core.rs                              # MultiArrayEmbedding core struct
        operations.rs                        # Embedding operations (normalize, slice)
        tests.rs                             # MultiArrayEmbedding tests
      dimensions/
        mod.rs                               # Dimension constants and helpers
        aggregates.rs                        # Aggregate dimension calculations
        arrays.rs                            # Dimension array constants
        constants.rs                         # Per-model dimension constants
        helpers.rs                           # Dimension helper functions
      embedding/
        mod.rs                               # ModelEmbedding type
        operations.rs                        # Embedding operations
        tensor.rs                            # Tensor conversion helpers
        tests.rs                             # Embedding tests
        types.rs                             # Embedding data types
        validation.rs                        # Embedding validation
      input/
        mod.rs                               # ModelInput type
        accessors.rs                         # Input accessor methods
        image_format.rs                      # ImageFormat enum
        input_type.rs                        # InputType enum (text, code, image, audio)
        model_input.rs                       # ModelInput struct
        tests/
          mod.rs                             # Input test module
          audio_tests.rs                     # Audio input tests
          code_tests.rs                      # Code input tests
          hash_and_size_tests.rs             # Hash/size tests
          image_tests.rs                     # Image input tests
          input_type_tests.rs                # Input type tests
          serde_tests.rs                     # Serialization tests
          text_tests.rs                      # Text input tests
          type_tests.rs                      # Type system tests
      model_id/
        mod.rs                               # ModelId enum (14 variants for 13 embedders)
        conversions.rs                       # ModelId conversions (to/from index, name)
        core.rs                              # ModelId enum definition and dimension lookup
        display.rs                           # Display implementation
        repository.rs                        # HuggingFace repository paths per model
        tokenizer.rs                         # TokenizerFamily per model
        tests.rs                             # ModelId tests
    warm/
      mod.rs                                 # Warm model loading system (pre-loads all models to VRAM)
      config.rs                              # WarmConfig: startup loading parameters
      error.rs                               # WarmError types and exit codes
      handle.rs                              # ModelHandle: protected VRAM allocation
      state.rs                               # WarmModelState lifecycle enum
      diagnostics/
        mod.rs                               # WarmDiagnostics module
        helpers.rs                           # Diagnostic helper functions
        output.rs                            # Diagnostic output formatter
        service.rs                           # Diagnostic service
        tests.rs                             # Diagnostic tests
        types.rs                             # Diagnostic types
      health/
        mod.rs                               # Health check module
        check.rs                             # Health check execution
        checker.rs                           # HealthChecker struct
        status.rs                            # HealthStatus types
        tests.rs                             # Health check tests
        tests_additional.rs                  # Additional health tests
      inference/
        mod.rs                               # InferenceEngine module (GPU inference validation)
        engine.rs                            # InferenceEngine: validates model inference
        validation.rs                        # Inference output validation
      integration/
        mod.rs                               # WarmEmbeddingPipeline integration module
        pipeline.rs                          # Unified pipeline access
        tests/
          mod.rs                             # Integration test module
          advanced.rs                        # Advanced integration tests
          basic.rs                           # Basic integration tests
      loader/
        mod.rs                               # WarmLoader module
        constants.rs                         # Loader constants
        engine.rs                            # Loader engine
        helpers.rs                           # Loader helper functions
        operations.rs                        # Loading operations
        preflight.rs                         # Pre-flight checks
        summary.rs                           # Loading summary
        tests.rs                             # Loader tests (directory module)
        tests/
          loader_tests.rs                    # Loader-specific tests
          summary_tests.rs                   # Summary tests
        types/
          mod.rs                             # Loader types module
          inference.rs                       # Inference types
          metadata.rs                        # Model metadata
          tests.rs                           # Loader type tests
          vram.rs                            # VRAM allocation types
          weights.rs                         # Weight metadata types
      memory_pool/
        mod.rs                               # WarmMemoryPools module
        model_pool.rs                        # Per-model memory pool
        pools.rs                             # Pool management
        tests.rs                             # Memory pool tests
        types.rs                             # Pool types
        working_pool.rs                      # Working memory pool
      registry/
        mod.rs                               # WarmModelRegistry module
        core.rs                              # Registry core
        operations.rs                        # Registry operations
        queries.rs                           # Registry query methods
        tests.rs                             # Registry tests
        tests_errors.rs                      # Error handling tests
        tests_queries.rs                     # Query tests
        tests_transitions.rs                 # State transition tests
        types.rs                             # Registry types
      cuda_alloc/
        mod.rs                               # WarmCudaAllocator module (non-evictable VRAM)
        allocation.rs                        # Allocation tracking
        allocator.rs                         # Main allocator logic
        allocator_cuda.rs                    # CUDA-specific allocator implementation
        constants.rs                         # Allocation constants
        gpu_info.rs                          # GPU info queries
        helpers.rs                           # Allocator helpers
        tests.rs                             # Allocator tests
      validation/
        mod.rs                               # WarmValidator module
        comparisons.rs                       # Validation comparison logic
        config.rs                            # Validation configuration
        result.rs                            # Validation result types
        validator.rs                         # Validator implementation
        validator_model.rs                   # Model-specific validation
        tests.rs                             # Validation tests (directory module)
        tests/
          config_tests.rs                    # Config validation tests
          model_tests.rs                     # Model validation tests
          result_tests.rs                    # Result tests
          validator_tests.rs                 # Validator tests
      tests/
        mod.rs                               # Warm system test module
        config_tests.rs                      # Config tests
        diagnostics_tests.rs                 # Diagnostics tests
        error_tests.rs                       # Error handling tests
        handle_tests.rs                      # Handle tests
        health_tests.rs                      # Health tests
        helpers.rs                           # Test helpers
        integration_tests.rs                 # Integration tests
        loader_tests.rs                      # Loader tests
        memory_pool_tests.rs                 # Memory pool tests
        registry_tests.rs                    # Registry tests
        state_tests.rs                       # State transition tests
        validation_tests.rs                  # Validation tests
  tests/
    attention_strategy_tests.rs              # Attention strategy tests
    benchmark_test.rs                        # Benchmark integration test
    e9_typo_tolerance_test.rs                # E9 HDC typo tolerance verification
    e9_vector_differentiation_test.rs        # E9 vector differentiation test
    embed_dual_with_hint_integration.rs      # Dual embedding with causal hint integration
    global_provider_fsv.rs                   # Global provider full state verification
    gpu_memory_slots_test.rs                 # GPU memory slot management tests
    no_fake_data_test.rs                     # Ensures no fake/mock data in production code
    qodo_embed_integration.rs               # Qodo-Embed E7 model integration test
    search_test.rs                           # Multi-space search integration test
    dimension_test/
      main.rs                                # Dimension test suite entry point
      aggregate_dimensions.rs                # Aggregate dimension tests
      comprehensive.rs                       # Comprehensive dimension tests
      constants.rs                           # Expected dimension constants
      edge_cases.rs                          # Dimension edge cases
      model_metadata.rs                      # Model metadata tests
      native_dimensions.rs                   # Native dimension tests
      offsets.rs                              # Embedding offset tests
      projected_dimensions.rs                # Projected dimension tests
      quantization_methods.rs                # Quantization method tests
    fixtures/
      create_fixture.rs                      # Test fixture creation utility
      physical_verification_test.rs          # Physical fixture verification
    storage_roundtrip_test/
      main.rs                                # Storage roundtrip test entry point
      comprehensive_validation.rs            # Comprehensive validation
      edge_cases.rs                          # Edge case tests
      fingerprint_creation.rs                # Fingerprint creation tests
      helpers.rs                             # Test helpers
      index_entry.rs                         # Index entry tests
      query_result.rs                        # Query result tests
      rrf_formula.rs                         # RRF formula verification
      serialization_roundtrip.rs             # Serialization roundtrip
```

### 1.4 context-graph-mcp (~42K lines)

MCP JSON-RPC 2.0 server for the 13-embedder semantic memory system.

```
crates/context-graph-mcp/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: exposes handlers, protocol, server, tools, adapters
    main.rs                                  # MCP server binary entry point (stdio/TCP/daemon transport)
    monitoring.rs                            # Live system monitoring (replaces stubs with real health checks)
    protocol.rs                              # JSON-RPC 2.0 types (JsonRpcRequest, JsonRpcResponse, JsonRpcError)
    weights.rs                               # Weight profile wrapper over core::weights
    adapters/
      mod.rs                                 # Adapter module
      causal_hint.rs                         # LLM-based CausalHintProvider for E5 enhancement
      lazy_provider.rs                       # LazyMultiArrayProvider: allows immediate startup before models load
    handlers/
      mod.rs                                 # Request handlers module
      merge.rs                               # merge_concepts handler
      core/
        mod.rs                               # Core handler module
        dispatch.rs                          # Request dispatch (method -> handler routing)
        handlers.rs                          # Handlers struct: holds all shared state (store, provider, etc.)
      tests/
        mod.rs                               # Handler test module
        error_codes.rs                       # Error code verification tests
        initialize.rs                        # Initialize handshake tests
        mcp_protocol_e2e_test.rs             # Full MCP protocol end-to-end test
        search_periodic_test.rs              # Periodic search tests
        tcp_transport_integration.rs         # TCP transport integration test
        tools_call.rs                        # tools/call handler tests
        tools_list.rs                        # tools/list handler tests
        topic_tools.rs                       # Topic tool handler tests
      tools/
        mod.rs                               # Tool call handlers module (75 tools: 71 non-LLM + 4 LLM-gated)
        causal_discovery_tools.rs            # trigger_causal_discovery, get_causal_discovery_status (LLM-gated via --features llm)
        causal_relationship_tools.rs         # store_causal_relationship, get_causal_relationships handlers
        causal_tools.rs                      # search_causes, search_effects, get_causal_chain, search_causal_relationships handlers (E5)
        code_tools.rs                        # search_code handler (E7)
        consolidation.rs                     # trigger_consolidation handler
        constellation_tools.rs               # Phase 2 constellation tools: compile/list/get/score_against/delete_constellation (5 handlers)
        contrastive_tools.rs                 # Phase 3 contrastive pair tools: mine/list/get/count_contrastive_pairs (4 handlers)
        typed_edges_tools.rs                 # Phase 4 typed-edge training tools: export_typed_edges_corpus, derive_anomalies_from_edges, validate_low_confidence_edges, list_typed_edge_records (4 handlers)
        curation_tools.rs                    # forget_concept (soft-delete with 30-day recovery), boost_importance (delta [-1.0,1.0] clamped to [0,1]) handlers
        daemon_tools.rs                      # daemon_status handler (multi-agent observability: connections, models, background tasks, PID, uptime)
        dispatch.rs                          # Tool dispatch: tool name -> handler function routing
        embedder_tools.rs                    # search_by_embedder, get_embedder_clusters, compare_embedder_views, list_embedder_indexes, get_memory_fingerprint, create_weight_profile, search_cross_embedder_anomalies, search_by_tokens (E12), search_by_expansion (E13)
        entity_tools.rs                      # extract_entities, search_by_entities, infer_relationship, etc.
        file_watcher_tools.rs                # list_watched_files, get_file_watcher_stats, delete_file_content, reconcile_files
        graph_link_tools.rs                  # get_memory_neighbors, get_typed_edges, traverse_graph, get_unified_neighbors (Weighted RRF across 13 embedders per ARCH-21)
        graph_tools.rs                       # search_connections, get_graph_path handlers (E8)
        helpers.rs                           # Handler helper functions (parameter parsing, response formatting)
        keyword_tools.rs                     # search_by_keywords handler (E6)
        maintenance_tools.rs                 # repair_causal_relationships handler (scans CF_CAUSAL_RELATIONSHIPS, removes corrupted/truncated entries)
        memory_tools.rs                      # store_memory (merged inject_context) and search_graph handlers
        provenance_tools.rs                  # get_audit_trail, get_merge_history, get_provenance_chain handlers
        robustness_tools.rs                  # search_robust handler (E9 HDC)
        sequence_tools.rs                    # get_conversation_context, get_session_timeline, traverse_memory_chain, compare_session_states (E4)
        status_tools.rs                      # get_memetic_status handler
        temporal_tools.rs                    # search_recent (E2) and search_periodic (E3) handlers
        topic_tools.rs                       # get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
        training_tools.rs                    # Phase 1 training corpus export: export/list/get/count_training_records (4 handlers; assembles full TrainingRecord per memory)
        validate.rs                          # Input validation helpers (parameter type checking)
        causal_dtos.rs                       # DTOs for causal tools
        code_dtos.rs                         # DTOs for code tools
        curation_dtos.rs                     # DTOs for curation tools
        embedder_dtos.rs                     # DTOs for embedder tools
        entity_dtos.rs                       # DTOs for entity tools
        graph_dtos.rs                        # DTOs for graph tools
        graph_link_dtos.rs                   # DTOs for graph link tools
        keyword_dtos.rs                      # DTOs for keyword tools
        provenance_dtos.rs                   # DTOs for provenance tools
        robustness_dtos.rs                   # DTOs for robustness tools
        temporal_dtos.rs                     # DTOs for temporal tools
        topic_dtos.rs                        # DTOs for topic tools
    server/
      mod.rs                                 # McpServer struct: constructor, stdio transport, path resolution
      transport.rs                           # TCP transport (run_tcp, handle_tcp_client, read_line_bounded)
      watchers.rs                            # Background tasks: file watcher, code watcher, graph builder
    tools/
      mod.rs                                 # Tool definitions module (MCP 2024-11-05 protocol)
      aliases.rs                             # Tool name aliases for backward compatibility
      names.rs                               # Tool name constants for dispatch matching
      types.rs                               # ToolDefinition struct
      definitions/
        mod.rs                               # Tool definitions organized by category
        causal.rs                            # Causal tool definitions (search_causes, get_causal_chain)
        causal_discovery.rs                  # Causal discovery tool definitions
        code.rs                              # Code tool definitions (search_code)
        constellation.rs                     # Phase 2 constellation schemas (5 tools; additionalProperties:false)
        contrastive.rs                       # Phase 3 contrastive pair schemas (4 tools; additionalProperties:false)
        typed_edges.rs                       # Phase 4 typed-edge training schemas (4 tools: export_typed_edges_corpus, derive_anomalies_from_edges, validate_low_confidence_edges, list_typed_edge_records)
        core.rs                              # Core tool definitions (store_memory, search_graph, get_memetic_status)
        curation.rs                          # Curation tool definitions (forget_concept, boost_importance)
        daemon.rs                            # Daemon tool definitions
        embedder.rs                          # Embedder tool definitions (search_by_embedder, etc.)
        entity.rs                            # Entity tool definitions (extract_entities, etc.)
        file_watcher.rs                      # File watcher tool definitions
        graph.rs                             # Graph tool definitions (search_connections, get_graph_path)
        graph_link.rs                        # Graph link tool definitions
        keyword.rs                           # Keyword tool definitions (search_by_keywords)
        maintenance.rs                       # Maintenance tool definitions
        merge.rs                             # Merge tool definitions
        provenance.rs                        # Provenance tool definitions
        robustness.rs                        # Robustness tool definitions
        sequence.rs                          # Sequence tool definitions
        temporal.rs                          # Temporal tool definitions (search_recent)
        topic.rs                             # Topic tool definitions
        training.rs                          # Phase 1 training corpus export schema (4 tools)
  tests/
    graph_linking_integration.rs             # Graph linking integration test
```

### 1.5 context-graph-storage (~36K lines)

RocksDB persistent storage layer for teleological fingerprints, graph edges, and code entities.

```
crates/context-graph-storage/
  Cargo.toml
  benches/
    causal_relationships_bench.rs            # Causal relationship storage benchmark
    e11_quality_bench.rs                     # E11 entity quality benchmark
    hybrid_effectiveness_bench.rs            # Hybrid search effectiveness benchmark
    maxsim_bench.rs                          # MaxSim (E12 ColBERT) performance benchmark
  src/
    lib.rs                                   # Crate root: re-exports RocksDbTeleologicalStore, CodeStore, etc.
    column_families.rs                       # Column family definitions and descriptors (TOTAL_COLUMN_FAMILIES = 59: 11 base + 28 teleological + 13 quantized + 5 code + 2 causal; see line 397)
    error.rs                                 # StorageError and StorageResult types
    indexes.rs                               # Secondary index operations (tags, temporal, sources)
    code/
      mod.rs                                 # Code entity storage module
      error.rs                               # CodeStorageError types
      store.rs                               # CodeStore: RocksDB-backed code entity/E7 storage
    graph_edges/
      mod.rs                                 # Graph edge storage (K-NN edges + typed edges)
      builder.rs                             # BackgroundGraphBuilder: async K-NN graph construction
      repository.rs                          # EdgeRepository: RocksDB CRUD for graph edges
      serialization.rs                       # Edge serialization/deserialization (bincode)
      types.rs                               # GraphEdgeStats, BuilderStats, RebuildResult types
    teleological/
      mod.rs                                 # Teleological storage module (28 column families: 20 core + training_records + 2 constellation + 3 contrastive + 2 typed-edge)
      column_families.rs                     # Teleological-specific column family definitions (TELEOLOGICAL_CF_COUNT=28 at line 551; CF_TRAINING_RECORDS, CF_CONSTELLATIONS, CF_CONSTELLATION_BY_SELECTOR, CF_CONTRASTIVE_PAIRS, CF_CONTRASTIVE_BY_KIND, CF_CONTRASTIVE_BY_ANCHOR, CF_TYPED_EDGE_RECORDS@499, CF_TYPED_EDGE_VALIDATIONS@516)
      schema.rs                              # Schema types and key format definitions
      serialization.rs                       # Fingerprint/profile serialization
      indexes/
        mod.rs                               # Teleological index module
        embedder_index.rs                    # Per-embedder index management (15 HNSW indexes)
        metrics.rs                           # Index metrics tracking
        registry.rs                          # Index registry
        hnsw_config/
          mod.rs                             # HNSW configuration module
          config.rs                          # HnswConfig struct
          constants.rs                       # HNSW constants (M=16, ef_construction=200, etc.)
          distance.rs                        # DistanceMetric enum
          embedder.rs                        # Per-embedder HNSW config (dimensions, distance)
          functions.rs                       # HNSW helper functions
        hnsw_impl/
          mod.rs                             # HNSW implementation (usearch-based)
          ops.rs                             # HNSW operations (add, search, remove)
          tests.rs                           # HNSW implementation tests
          types.rs                           # HNSW types
      quantized/
        mod.rs                               # Quantized fingerprint storage module
        error.rs                             # Quantized storage error types
        helpers.rs                           # Quantization helpers
        rocksdb_impl.rs                      # RocksDB-backed quantized storage implementation
        tests.rs                             # Quantized storage tests
        trait_def.rs                         # QuantizedFingerprintStorage trait
      rocksdb_store/
        mod.rs                               # RocksDbTeleologicalStore module
        audit_log.rs                         # Audit log operations (append, query by target)
        causal_hnsw_index.rs                 # Causal HNSW index for E5 asymmetric search
        causal_relationships.rs              # Causal relationship CRUD
        constellation.rs                     # Phase 2 constellation persistence: store/get/list/delete (primary + selector index in single WriteBatch, version-byte prefix)
        content.rs                           # Content storage and retrieval
        contrastive.rs                       # Phase 3 contrastive pair persistence: primary + by-kind + by-anchor indexes atomic (single WriteBatch)
        crud.rs                              # Core CRUD operations (create, read, update, delete)
        export.rs                            # Phase 1 training record persistence for export_training_corpus: put/get/list/count/multi_get/iterate CF_TRAINING_RECORDS (version-byte prefix, fail-fast on RocksDB errors)
        typed_edge_export.rs                 # Phase 4 F1: typed-edge training record CRUD for CF_TYPED_EDGE_RECORDS (33-byte composite key, version-byte prefix)
        typed_edge_keys.rs                   # Shared 33-byte composite key layout [source:16][target:16][edge_type:1] for both CF_TYPED_EDGE_RECORDS and CF_TYPED_EDGE_VALIDATIONS
        llm_validation.rs                    # Phase 4 F4: LLM edge validation CRUD for CF_TYPED_EDGE_VALIDATIONS (Valid/Invalid/Reclassify verdicts with prompt_hash)
        anomaly_derivation.rs                # Phase 4 F2: derive contrastive anomaly pairs directly from typed-edge embedder scores (5 of 6 AnomalyKind variants)
        versioned_bincode.rs                 # Shared version-byte + bincode encode/decode helpers for CF_TYPED_EDGE_RECORDS and CF_TYPED_EDGE_VALIDATIONS
        file_index.rs                        # File index operations
        fusion.rs                            # Multi-space search fusion (RRF, pipeline, e1_only)
        helpers.rs                           # Store helper functions
        index_ops.rs                         # Index operations (rebuild, compact)
        inverted_index.rs                    # Inverted index for E6/E13 sparse vectors
        persistence.rs                       # HNSW persistence to/from RocksDB
        provenance_storage.rs                # Provenance chain storage
        search.rs                            # Full search implementation
        source_metadata.rs                   # Source metadata storage
        store.rs                             # RocksDbTeleologicalStore struct and constructor
        tests.rs                             # Store tests
        trait_impl.rs                        # TeleologicalMemoryStore trait implementation
        types.rs                             # Store-internal types
      search/
        mod.rs                               # Search module
        error.rs                             # Search error types
        maxsim.rs                            # MaxSim scoring for E12 ColBERT reranking
        result.rs                            # Search result types
        temporal_boost.rs                    # Temporal boost for recency-weighted results
        token_storage.rs                     # Token storage for E12 ColBERT
        matrix/
          mod.rs                             # Matrix search module
          analysis.rs                        # Search matrix analysis
          builder.rs                         # SearchMatrix builder
          results.rs                         # Matrix search results
          search_matrix.rs                   # SearchMatrix: cross-embedder matrix search
          strategy_search.rs                 # Strategy-based matrix search
          tests_boundary.rs                  # Boundary condition tests
          tests_unit.rs                      # Unit tests
        multi/
          mod.rs                             # Multi-space search module
          builder.rs                         # MultiSpaceSearch builder
          executor.rs                        # Multi-space search executor with RRF fusion
          tests.rs                           # Multi-space search tests
          types.rs                           # Multi-space types
        pipeline/
          mod.rs                             # Pipeline search module (E13->E1->E12)
          builder.rs                         # PipelineBuilder
          execution.rs                       # Pipeline execution engine
          stages.rs                          # Pipeline stage definitions
          tests.rs                           # Pipeline tests
          traits.rs                          # Pipeline traits
          types.rs                           # Pipeline types
        single/
          mod.rs                             # Single embedder search module
          config.rs                          # Single search configuration
          search.rs                          # Single embedder search implementation
          tests/
            mod.rs                           # Single search test module
            integration.rs                   # Integration tests
            search.rs                        # Search tests
            validation.rs                    # Validation tests
      tests/
        mod.rs                               # Teleological test module
        column_family.rs                     # Column family tests
        content.rs                           # Content storage tests
        helpers.rs                           # Test helpers
        key_format.rs                        # Key format tests
        panic.rs                             # Panic recovery tests
        serialization.rs                     # Serialization roundtrip tests
        stale_lock.rs                        # Stale lock detection tests
  tests/
    constellation_integration.rs             # Phase 2 constellation integration test (primary + selector-index round-trip, 544 lines)
    contrastive_integration.rs               # Phase 3 contrastive pair integration test (primary + by-kind + by-anchor round-trip, 413 lines)
    typed_edge_export_integration.rs         # Phase 4 F1 typed-edge export integration test (store/get/list/count/clear, version mismatch, 219 lines)
    llm_validation_integration.rs            # Phase 4 F4 LLM validation integration test (Valid/Invalid/Reclassify round-trip, version mismatch, 231 lines)
    anomaly_derivation_integration.rs        # Phase 4 F2 anomaly derivation end-to-end test (6 synthetic edges, 5 expected anomaly pairs, per_kind_counts, 251 lines)
    crash_recovery_tests.rs                  # Crash recovery integration test
    e12_e13_source_of_truth_verification.rs  # E12/E13 storage source of truth test
    e9_storage_roundtrip_test.rs             # E9 HDC storage roundtrip test
    teleological_integration.rs              # Teleological store integration test
    training_records_integration.rs          # Phase 1 training record persistence integration test (put/get/list/count/multi_get/iterate, 523 lines)
    full_integration_real_data/
      main.rs                                # Full integration test entry point
      helpers.rs                             # Integration test helpers
      operations_tests.rs                    # CRUD operation tests
      performance_tests.rs                   # Performance tests
      pipeline_tests.rs                      # Pipeline tests
      roundtrip_tests.rs                     # Roundtrip tests
      search_tests.rs                        # Search tests
```

### 1.6 context-graph-cli (~16K lines)

CLI tools for memory management, hooks integration, and session persistence.

```
crates/context-graph-cli/
  Cargo.toml
  benches/
    cli_benchmarks.rs                        # CLI command performance benchmarks
  src/
    main.rs                                  # CLI binary entry point (clap-based command dispatch)
    error.rs                                 # CliExitCode, exit_code_for_error, corruption detection
    mcp_client.rs                            # MCP client for communicating with running server
    mcp_helpers.rs                           # MCP client helper functions
    commands/
      mod.rs                                 # Command module declarations
      divergence.rs                          # Divergence detection commands
      setup.rs                               # Setup command: creates .claude/settings.json and hooks
      topic.rs                               # Topic portfolio and stability commands
      warmup.rs                              # Warmup command: pre-loads embedding models to VRAM
      watch.rs                               # Watch command: monitors directory for file changes
      hooks/
        mod.rs                               # Claude Code native hooks command module
        args.rs                              # Hook command argument types
        error.rs                             # Hook error types
        memory_cache.rs                      # Memory cache for hooks
        post_tool_use.rs                     # PostToolUse hook handler
        pre_compact.rs                       # PreCompact hook handler
        pre_tool_use.rs                      # PreToolUse hook handler
        session_end.rs                       # SessionEnd hook handler
        session_start.rs                     # SessionStart hook handler
        session_state.rs                     # Session state management
        task_completed.rs                    # TaskCompleted hook handler
        types.rs                             # Hook types
        user_prompt_submit.rs                # UserPromptSubmit hook handler
      memory/
        mod.rs                               # Memory commands module
        capture.rs                           # Memory capture command
        inject.rs                            # Context injection command
      session/
        mod.rs                               # Session commands module
        persist.rs                           # Session persist command
        restore.rs                           # Session restore command
  tests/
    e2e/
      mod.rs                                 # E2E test module
      error_recovery_test.rs                 # Error recovery E2E test
      full_session_test.rs                   # Full session lifecycle E2E test
      helpers.rs                             # E2E test helpers
    integration/
      mod.rs                                 # Integration test module
      exit_code_test.rs                      # Exit code verification test
      helpers.rs                             # Integration test helpers
      hook_lifecycle_test.rs                 # Hook lifecycle integration test
      timeout_test.rs                        # Timeout handling test
```

### 1.7 context-graph-cuda (~5K lines)

CUDA GPU acceleration for vector operations and clustering (RTX 5090 / Blackwell).

```
crates/context-graph-cuda/
  Cargo.toml
  build.rs                                   # Build script for CUDA compilation
  src/
    lib.rs                                   # Crate root: CUDA driver API, HDBSCAN, similarity, green contexts
    error.rs                                 # CudaError and CudaResult types
    ops.rs                                   # VectorOps trait: cosine similarity, dot product, L2 norm
    similarity.rs                            # GPU-batched cosine similarity (grouped by dimension)
    stub.rs                                  # StubVectorOps (test-only, gated by #[cfg(test)])
    context/
      mod.rs                                 # GPU context module
      green_contexts.rs                      # Green Contexts: GPU partitioning for inference/background
    ffi/
      mod.rs                                 # FFI module (CUDA driver bindings)
      cuda_driver.rs                         # CUDA driver API bindings (cuInit, cuDeviceGet, etc.)
      faiss.rs                               # FAISS GPU bindings (optional, feature-gated)
      knn.rs                                 # Custom k-NN kernel bindings
    hdbscan/
      mod.rs                                 # GPU-accelerated HDBSCAN clustering module
      clusterer.rs                           # GpuHdbscanClusterer: GPU HDBSCAN implementation
      error.rs                               # HDBSCAN error types
      gpu_knn.rs                             # GPU k-NN for HDBSCAN distance computation
      tests.rs                               # HDBSCAN tests
    safe/
      mod.rs                                 # Safe RAII wrappers module
      device.rs                              # GpuDevice: safe RAII wrapper for CUDA device/context
```

### 1.8 context-graph-causal-agent (~5K lines)

Causal relationship discovery using LLM (Hermes-2-Pro-Mistral-7B via llama-cpp-2).

```
crates/context-graph-causal-agent/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: LLM-based causal discovery (GBNF grammar)
    error.rs                                 # CausalAgentError and CausalAgentResult types
    activator/
      mod.rs                                 # E5EmbedderActivator: triggers E5 for confirmed relationships
    llm/
      mod.rs                                 # CausalDiscoveryLLM: llama-cpp-2 model wrapper (feature-gated)
      prompt.rs                              # Causal analysis prompt templates (ChatML format)
    scanner/
      mod.rs                                 # MemoryScanner: finds candidate memory pairs for analysis
    service/
      mod.rs                                 # CausalDiscoveryService: background service for scheduled discovery
    types/
      mod.rs                                 # Causal agent types (CausalCandidate, MemoryForAnalysis, etc.)
  examples/
    benchmark_causal.rs                      # Causal discovery benchmark example
    benchmark_causal_enhanced.rs             # Enhanced causal benchmark
    benchmark_causal_large.rs                # Large-scale causal benchmark
    test_llm.rs                              # LLM integration test example
  tests/
    analyze_single_text_integration.rs       # Single text analysis integration test
    background_loop_integration.rs           # Background loop integration test
```

### 1.9 context-graph-graph-agent (~4K lines)

Graph relationship discovery using LLM (shares Hermes-2-Pro with causal agent).

```
crates/context-graph-graph-agent/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: LLM-based graph discovery (20 relationship types)
    error.rs                                 # GraphAgentError and GraphAgentResult types
    stubs.rs                                 # Stub service for tests (feature-gated by test-utils)
    activator/
      mod.rs                                 # E8Activator: triggers E8 embedding for graph edges
    llm/
      mod.rs                                 # GraphRelationshipLLM: LLM-based graph analysis (feature-gated)
      prompt.rs                              # GraphPromptBuilder: graph analysis prompt templates
    scanner/
      mod.rs                                 # MemoryScanner: finds candidate memory pairs for graph analysis
    service/
      mod.rs                                 # GraphDiscoveryService: orchestrates graph discovery cycles
    types/
      mod.rs                                 # Graph agent types (RelationshipType, ContentDomain, etc.)
  examples/
    benchmark_graph.rs                       # Graph discovery benchmark
    benchmark_graph_code.rs                  # Code domain graph benchmark
    benchmark_graph_multi.rs                 # Multi-domain graph benchmark
  tests/
    scanner_integration.rs                   # Scanner integration test
```

### 1.10 context-graph-test-utils (159 lines)

Shared test utilities for the workspace -- no mock data.

```
crates/context-graph-test-utils/
  Cargo.toml
  src/
    lib.rs                                   # Crate root: re-exports fingerprint generators and store factories
    fingerprints.rs                          # Real fingerprint generators (unit vectors, sparse, teleological)
    stores.rs                                # Test store factories (create_test_store, create_initialized_store)
```

---

## 2. Module Dependency Graph (Inter-Crate)

Derived from `[dependencies]` in each crate's `Cargo.toml`.

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
  └── context-graph-cuda (optional)

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

context-graph-benchmark
  ├── context-graph-core (with test-utils)
  ├── context-graph-storage
  ├── context-graph-embeddings (optional, real-embeddings feature)
  ├── context-graph-mcp (optional, real-embeddings feature)
  ├── context-graph-graph-agent (optional)
  └── context-graph-causal-agent (optional)

context-graph-test-utils
  ├── context-graph-core
  └── context-graph-storage
```

Dependency layering (bottom to top):

```
Layer 0: context-graph-core (no internal deps)
Layer 1: context-graph-cuda (depends on core)
Layer 2: context-graph-embeddings (depends on core, cuda)
Layer 3: context-graph-storage (depends on core, embeddings, cuda)
Layer 4: context-graph-causal-agent (depends on core, storage, embeddings)
Layer 5: context-graph-graph-agent (depends on core, storage, embeddings, causal-agent)
Layer 6: context-graph-mcp (depends on all above)
Layer 7: context-graph-cli (depends on core, mcp, storage, embeddings)
          context-graph-benchmark (depends on core, storage; optionally others)
Utility:  context-graph-test-utils (depends on core, storage)
```

---

## 3. Internal Module Dependency Graph (Major Crates)

### 3.1 context-graph-core Internal Dependencies

```
lib.rs
  ├── config          (standalone, uses serde, toml)
  ├── error           (standalone, uses thiserror)
  ├── types           (uses error, chrono, uuid, serde)
  │   ├── fingerprint (uses types::graph_edge, embeddings::config)
  │   ├── graph_edge  (uses types::fingerprint)
  │   ├── memory_node (uses types::fingerprint, types::source_metadata)
  │   └── code_entity (standalone)
  ├── traits          (uses types, error; defines TeleologicalMemoryStore, MultiArrayEmbeddingProvider)
  ├── embeddings      (uses types, traits, config)
  ├── entity          (uses types, embeddings)
  ├── monitoring      (uses error; standalone trait definitions)
  ├── weights         (uses types::fingerprint constants)
  ├── fusion          (uses types, uuid)
  ├── graph           (uses types::fingerprint)
  ├── graph_linking   (uses types, error, traits, config)
  ├── causal          (uses types, error, graph_linking)
  ├── clustering      (uses types, config, error, embeddings)
  ├── code            (uses types::fingerprint)
  ├── index           (uses types, config, error; uses hnsw_rs)
  ├── injection       (uses types, traits, clustering, fusion)
  ├── memory          (uses types, traits, config, error, clustering)
  ├── quantization    (uses types::fingerprint, error)
  ├── retrieval       (uses types, traits, config, error, fusion, similarity)
  ├── similarity      (uses types, config, error)
  ├── stubs           (uses types, traits, error; test-only)
  ├── teleological    (uses types, config, traits, embeddings, error, clustering)
  ├── training        (uses types::fingerprint, teleological::{synergy_matrix, groups, types}; pure computation, no I/O)
  ├── constellation   (uses types::fingerprint, teleological::{synergy_matrix, types}; Welford + reservoir aggregation)
  └── contrastive     (uses types::fingerprint, teleological::synergy_matrix; pure miner functions)
```

### 3.2 context-graph-embeddings Internal Dependencies

```
lib.rs
  ├── config          (standalone)
  ├── error           (standalone, uses thiserror)
  ├── types           (standalone: ModelId, ModelInput, MultiArrayEmbedding, dimensions)
  ├── traits          (uses types, error; defines EmbeddingModel, ModelFactory)
  ├── gpu             (uses traits, error, config; wraps candle-core/candle-nn)
  │   ├── device      (CUDA detection, singleton)
  │   ├── memory      (VRAM tracking)
  │   ├── model_loader (SafeTensors + weight loading)
  │   └── tensor      (type-safe tensor wrapper)
  ├── models          (uses traits, types, error, gpu, config)
  │   ├── factory     (creates all 13 model instances)
  │   ├── registry    (manages model lifecycle and GPU memory)
  │   ├── memory_tracker
  │   ├── attention   (dense, sliding_window, tiled)
  │   ├── pretrained  (E1, E5, E7, E8, E9, E10, E11, E12, E13, R-GCN, weight_projection)
  │   └── custom      (E2 temporal_recent, E3 temporal_periodic, E4 temporal_positional, E9 HDC)
  ├── provider        (uses models, types, traits; ProductionMultiArrayProvider)
  ├── cache           (uses types; moka concurrent cache)
  ├── batch           (uses types, error; batched request processing)
  ├── quantization    (uses types, error; PQ-8, Float8, Binary, router)
  ├── pruning         (uses types; E12 token pruning)
  ├── storage         (uses types, quantization; StoredQuantizedFingerprint, multi-space RRF)
  ├── training        (uses types, gpu, models; E5 fine-tuning with LoRA)
  ├── adapters        (uses models; bridges to external traits)
  ├── global_provider (uses provider, warm; global singleton)
  └── warm            (uses models, gpu, config, error; startup model loading)
      ├── loader      (orchestrates loading 14 models to VRAM -- TOTAL_MODEL_COUNT=14, includes Entity and Kepler as separate slots)
      ├── registry    (tracks loading state)
      ├── memory_pool (VRAM allocation management)
      ├── cuda_alloc  (non-evictable VRAM)
      ├── health      (post-load health checks)
      ├── validation  (dimension/weight/inference verification)
      ├── diagnostics (diagnostic reporting)
      ├── inference   (GPU inference validation)
      └── integration (WarmEmbeddingPipeline)
```

### 3.3 context-graph-mcp Internal Dependencies

```
main.rs (binary entry point)
  ├── server          (McpServer, transport, watchers)
  │   ├── transport   (TCP server with read_line_bounded)
  │   └── watchers    (file watcher, code watcher, graph builder tasks)
  ├── protocol        (JSON-RPC 2.0 types)
  ├── handlers        (request dispatch and tool handlers)
  │   ├── core        (Handlers struct, dispatch routing)
  │   ├── merge       (merge_concepts)
  │   └── tools       (75-tool dispatch with 71 non-LLM + 4 LLM-gated; DTOs, helpers, validate; adds training_tools, constellation_tools, contrastive_tools, typed_edges_tools)
  ├── tools           (tool definitions following MCP protocol)
  │   ├── definitions (per-category tool definitions with JSON schemas)
  │   ├── names       (tool name constants)
  │   ├── types       (ToolDefinition struct)
  │   └── aliases     (backward-compatible name aliases)
  ├── adapters        (LazyMultiArrayProvider, CausalHintProvider)
  ├── monitoring      (live system health monitoring)
  └── weights         (thin wrapper over core::weights)
```

### 3.4 context-graph-storage Internal Dependencies

```
lib.rs
  ├── error            (StorageError, StorageResult)
  ├── column_families  (59 total CF definitions and descriptors)
  ├── indexes          (secondary index operations)
  ├── code             (CodeStore: code entity + E7 storage)
  │   ├── error        (CodeStorageError)
  │   └── store        (RocksDB-backed code storage)
  ├── graph_edges      (K-NN graph and typed edge storage)
  │   ├── builder      (BackgroundGraphBuilder)
  │   ├── repository   (EdgeRepository: RocksDB CRUD)
  │   ├── serialization
  │   └── types
  └── teleological     (main storage module, 28 CFs)
      ├── column_families (teleological-specific CFs)
      ├── schema         (key format definitions)
      ├── serialization  (fingerprint/profile serialization)
      ├── indexes        (HNSW index management)
      │   ├── hnsw_config (15 HNSW configs per embedder)
      │   ├── hnsw_impl   (usearch-based HNSW operations)
      │   ├── embedder_index
      │   ├── metrics
      │   └── registry
      ├── quantized      (quantized fingerprint storage trait + RocksDB impl)
      ├── rocksdb_store  (RocksDbTeleologicalStore: implements TeleologicalMemoryStore)
      │   ├── crud, search, fusion, content, helpers
      │   ├── audit_log, provenance_storage, source_metadata
      │   ├── causal_hnsw_index, causal_relationships
      │   ├── file_index, inverted_index
      │   ├── index_ops, persistence
      │   ├── export         (Phase 1: training record persistence for CF_TRAINING_RECORDS)
      │   ├── constellation  (Phase 2: constellation + selector index atomic writes)
      │   ├── contrastive    (Phase 3: pair + by-kind + by-anchor atomic writes)
      │   └── trait_impl
      ├── search         (search implementations)
      │   ├── single     (single-embedder search)
      │   ├── multi      (multi-space search with RRF)
      │   ├── pipeline   (E13->E1->E12 pipeline)
      │   ├── matrix     (cross-embedder matrix search)
      │   ├── maxsim     (E12 ColBERT MaxSim)
      │   ├── temporal_boost
      │   └── token_storage
      └── tests
```

---

## 4. Entry Point Traces

### 4.1 MCP Server (`context-graph-mcp/src/main.rs`)

The primary entry point for the system.

```
main.rs
  ├── parse_args()                       → CliArgs struct
  ├── init logging                       → tracing_subscriber
  ├── load Config                        → context_graph_core::config::Config
  ├── McpServer::new(config)             → server/mod.rs
  │   ├── RocksDbTeleologicalStore::new  → context_graph_storage
  │   ├── CodeStore::new                 → context_graph_storage::code
  │   ├── EdgeRepository::new            → context_graph_storage::graph_edges
  │   ├── LazyMultiArrayProvider::new    → adapters/lazy_provider.rs
  │   ├── initialize_global_warm_provider→ context_graph_embeddings::global_provider
  │   │   └── WarmLoader → loads all 13 models to VRAM
  │   ├── CausalDiscoveryLLM::new        → context_graph_causal_agent::llm (feature-gated)
  │   ├── GraphDiscoveryService::new     → context_graph_graph_agent::service (feature-gated)
  │   └── Handlers::new(store, provider, ...)→ handlers/core/handlers.rs
  ├── server.run_stdio() or server.run_tcp()
  │   └── loop: read JSON-RPC line
  │       ├── parse JsonRpcRequest       → protocol.rs
  │       ├── Handlers::handle_request   → handlers/core/dispatch.rs
  │       │   ├── "initialize"           → MCP handshake
  │       │   ├── "tools/list"           → tools/definitions/mod.rs
  │       │   └── "tools/call"           → handlers/tools/dispatch.rs
  │       │       ├── "store_memory"     → handlers/tools/memory_tools.rs
  │       │       ├── "search_graph"     → handlers/tools/memory_tools.rs
  │       │       ├── "search_code"      → handlers/tools/code_tools.rs
  │       │       ├── ... (75 tools total; 4 LLM-gated behind --features llm)
  │       │       └── response           → JsonRpcResponse
  │       └── write response line
  └── background tasks (watchers.rs)
      ├── file watcher                   → memory::watcher
      ├── code watcher                   → memory::code_watcher
      └── graph builder                  → graph_edges::builder
```

### 4.2 CLI (`context-graph-cli/src/main.rs`)

```
main.rs
  ├── Cli::parse()                       → clap command parsing
  ├── init logging                       → tracing_subscriber
  └── match command
      ├── Session { Persist }            → commands/session/persist.rs
      ├── Session { Restore }            → commands/session/restore.rs
      ├── Hooks { SessionStart }         → commands/hooks/session_start.rs
      ├── Hooks { SessionEnd }           → commands/hooks/session_end.rs
      ├── Hooks { PreToolUse }           → commands/hooks/pre_tool_use.rs
      ├── Hooks { PostToolUse }          → commands/hooks/post_tool_use.rs
      ├── Hooks { UserPromptSubmit }     → commands/hooks/user_prompt_submit.rs
      ├── Hooks { TaskCompleted }        → commands/hooks/task_completed.rs
      ├── Hooks { PreCompact }           → commands/hooks/pre_compact.rs
      ├── Memory { Capture }             → commands/memory/capture.rs
      ├── Memory { Inject }              → commands/memory/inject.rs
      ├── Topic { ... }                  → commands/topic.rs
      ├── Divergence { ... }             → commands/divergence.rs
      ├── Setup                          → commands/setup.rs
      ├── Warmup                         → commands/warmup.rs
      └── Watch                          → commands/watch.rs
```

### 4.3 Benchmark (`context-graph-benchmark/src/bin/report.rs`)

```
report.rs (benchmark-report binary)
  ├── parse CLI args                     → clap
  ├── BenchmarkConfig::from_tier         → config.rs
  ├── DatasetGenerator::new              → datasets/generator.rs
  ├── BenchmarkHarness::run              → runners/mod.rs
  │   ├── ComparativeRunner              → runners/comparative.rs
  │   ├── ScalingRunner                  → runners/scaling.rs
  │   └── ... (per-domain runners)
  └── BenchmarkReport::generate          → reports/mod.rs
```

---

## 5. Build / Package Configuration

### 5.1 Workspace Members

Defined in `/home/cabdru/contextgraph/Cargo.toml`:

| Crate | Default Member | Description |
|-------|---------------|-------------|
| context-graph-mcp | Yes | MCP JSON-RPC server (main binary) |
| context-graph-core | Yes | Core domain types and traits |
| context-graph-cuda | Yes | CUDA GPU acceleration |
| context-graph-embeddings | Yes | 13-model embedding pipeline |
| context-graph-storage | Yes | RocksDB storage layer |
| context-graph-cli | Yes | CLI tools |
| context-graph-causal-agent | Yes | Causal discovery with LLM |
| context-graph-graph-agent | Yes | Graph discovery with LLM |
| context-graph-test-utils | Yes | Shared test utilities |
| context-graph-benchmark | **No** (excluded) | Benchmarking suite (87K lines, build explicitly) |

Benchmark is excluded from default builds: `cargo build -p context-graph-benchmark` required.

### 5.2 Feature Flags

| Feature | Crate(s) | Description |
|---------|----------|-------------|
| `candle` | embeddings | Candle ML framework (REQUIRED, default) |
| `cuda` | cuda, embeddings, storage, mcp, cli, causal-agent, graph-agent | CUDA GPU acceleration (default) |
| `llm` | mcp, causal-agent, graph-agent | LLM backend via llama-cpp-2 (~500MB binary size) |
| `test-utils` | core, graph-agent | Test stubs and helpers (NEVER enable in production) |
| `real-embeddings` | benchmark | Real GPU embeddings for benchmarks |
| `benchmark-tests` | benchmark | Benchmark test suite |
| `faiss-working` | cuda | FAISS GPU support (requires manual rebuild) |

### 5.3 Profile Settings

From workspace `Cargo.toml`:

```toml
[profile.release]
lto = "thin"          # Thin link-time optimization
codegen-units = 1     # Single codegen unit for maximum optimization
strip = true          # Strip symbols from binary

[profile.dev]
opt-level = 0         # No optimization in debug
debug = 1             # Line info only (not full DWARF)
incremental = true    # Incremental compilation
split-debuginfo = "unpacked"

[profile.dev.package."*"]
debug = false         # No debug info for dependencies
```

### 5.4 Workspace Dependencies

Key shared dependencies specified in workspace `Cargo.toml`:

| Dependency | Version | Purpose |
|-----------|---------|---------|
| tokio | 1.35 (full) | Async runtime |
| serde | 1.0 (derive) | Serialization |
| serde_json | 1.0 | JSON |
| candle-core | 0.9.2-alpha (cuda) | GPU tensor operations |
| candle-nn | 0.9.2-alpha (cuda) | Neural network layers |
| llama-cpp-2 | 0.1 | LLM inference (GGUF models) |
| rocksdb | 0.22 | Persistent storage |
| uuid | 1.6 (v4, v5, serde) | ID generation |
| chrono | 0.4 (serde) | Time handling |
| thiserror | 1.0 | Error types |
| tracing | 0.1 | Logging |
| hnsw_rs | 0.3 | HNSW approximate nearest neighbor |
| usearch | 2 | HNSW index (storage layer) |
| parking_lot | 0.12 | Fast synchronization primitives |
| rayon | 1.10 | Parallel iteration |

### 5.5 Binaries

| Binary | Crate | Path |
|--------|-------|------|
| `context-graph-mcp` | context-graph-mcp | `src/main.rs` |
| `context-graph-cli` | context-graph-cli | `src/main.rs` |
| `train-causal` | context-graph-embeddings | `src/bin/train_causal.rs` |
| `benchmark-report` | context-graph-benchmark | `src/bin/report.rs` |
| `realdata-bench` | context-graph-benchmark | `src/bin/realdata_bench.rs` |
| `gpu-bench` | context-graph-benchmark | `src/bin/gpu_bench.rs` |
| `mcp-bench` | context-graph-benchmark | `src/bin/mcp_bench.rs` |
| ... (30+ benchmark binaries) | context-graph-benchmark | `src/bin/*.rs` |

---

## 6. Cross-References

- **01_project_overview.md**: High-level architecture and system description
- **03_architecture_deep_dive.md**: Detailed architecture decisions and rationale
- **04_data_flow.md**: Data flow through the embedding pipeline and storage
- **05_api_reference.md**: MCP tool definitions and JSON-RPC protocol
