# TASK-L008: Teleological Retrieval Pipeline

```yaml
metadata:
  id: "TASK-L008"
  title: "Teleological Retrieval Pipeline"
  layer: "logic"
  priority: "P0"
  estimated_hours: 12
  created: "2026-01-04"
  status: "pending"
  dependencies:
    - "TASK-L001"  # Multi-Embedding Query Executor
    - "TASK-L002"  # Purpose Vector Computation
    - "TASK-L003"  # Goal Alignment Calculator
    - "TASK-L004"  # Johari Transition Manager
    - "TASK-L005"  # Per-Space HNSW Index Builder
    - "TASK-L006"  # Purpose Pattern Index
    - "TASK-L007"  # Cross-Space Similarity Engine
  spec_refs:
    - "projectionplan1.md:retrieval-pipeline"
    - "projectionplan2.md:teleological-retrieval"
```

## Problem Statement

Implement the complete teleological retrieval pipeline that orchestrates multi-stage retrieval across semantic, purpose, and goal dimensions, producing purpose-aligned, explainable results.

## Context

The Teleological Retrieval Pipeline is the capstone of the Logic Layer, integrating all preceding components into a unified 5-stage retrieval flow optimized for sub-60ms latency:

1. **Stage 1: SPARSE PRE-FILTER (BM25 + E13 SPLADE)** - <5ms, 10K candidates
   - Hybrid sparse retrieval combining BM25 lexical matching with SPLADE learned sparse representations
   - E13 SPLADE embedding provides semantic-aware sparse vectors
   - Configurable weighting between BM25 and SPLADE scores
   - Supports up to 1M candidate corpus with efficient pruning

2. **Stage 2: FAST DENSE ANN (Matryoshka 128D)** - <10ms, 1K candidates
   - HNSW search using Matryoshka truncated embeddings (128D by default)
   - Adaptive dimension selection based on corpus size (128/256/512/1024)
   - 4-8x faster than full-dimension search with minimal recall loss
   - Cross-space search across active embedding spaces

3. **Stage 3: MULTI-SPACE RERANK (RRF Fusion)** - <20ms, 100 candidates
   - Reciprocal Rank Fusion (RRF) combines rankings from multiple embedding spaces
   - Configurable RRF k parameter (default 60.0) for score calibration
   - Optional weighted sum or relative score fusion alternatives
   - Purpose-aware weighting for teleological relevance

4. **Stage 4: TELEOLOGICAL ALIGNMENT FILTER** - <10ms, 50 candidates
   - Purpose vector alignment scoring
   - Goal alignment calculation against active goals
   - Combined purpose/goal scoring with configurable weights
   - Filters candidates below alignment thresholds

5. **Stage 5: LATE INTERACTION RERANK (E12 MaxSim)** - <15ms, final 10
   - Fine-grained token-level similarity using E12 ColBERT embeddings
   - MaxSim computation for precise relevance scoring
   - Blends with prior scores for final ranking
   - Misalignment flagging for off-purpose results

This enables retrieval that balances content relevance with goal alignment, producing not just similar memories but *purposefully relevant* memories with end-to-end latency under 60ms.

## Technical Specification

### Data Structures

```rust
/// Configuration for the teleological retrieval pipeline
#[derive(Clone, Debug)]
pub struct TeleologicalRetrievalConfig {
    /// Stage 1: Pre-filter configuration
    pub prefilter: PrefilterConfig,

    /// Stage 2: Multi-embedding rerank configuration
    pub rerank: RerankConfig,

    /// Stage 3: Teleological alignment configuration
    pub alignment: AlignmentStageConfig,

    /// Stage 4: Late interaction configuration
    pub late_interaction: LateInteractionConfig,

    /// Stage 5: Misalignment check configuration
    pub misalignment: MisalignmentConfig,

    /// Final result limit
    pub final_limit: usize,

    /// Whether to include full breakdown
    pub include_breakdown: bool,

    /// Timeout for entire pipeline
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct PrefilterConfig {
    /// Which embedding spaces to search
    pub active_spaces: EmbeddingSpaceMask,

    /// Results per space
    pub per_space_k: usize,

    /// Minimum similarity threshold
    pub min_similarity: f32,

    /// HNSW ef_search override
    pub ef_search: Option<usize>,

    /// Sparse pre-filter configuration (BM25 + SPLADE)
    pub sparse_prefilter: SparsePrefilterConfig,
}

#[derive(Clone, Debug)]
pub struct SparsePrefilterConfig {
    /// Enable sparse pre-filtering
    pub enabled: bool,

    /// Weight balance between BM25 and SPLADE (0.0 = pure BM25, 1.0 = pure SPLADE)
    pub sparse_weight: f32,

    /// Top-K results from SPLADE retrieval
    pub splade_top_k: usize,

    /// BM25 k1 parameter for term frequency saturation
    pub bm25_k1: f32,

    /// BM25 b parameter for document length normalization
    pub bm25_b: f32,

    /// Maximum candidates from sparse stage
    pub max_candidates: usize,
}

impl Default for SparsePrefilterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sparse_weight: 0.5,  // Balanced BM25 + SPLADE
            splade_top_k: 10_000,
            bm25_k1: 1.2,
            bm25_b: 0.75,
            max_candidates: 10_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatryoshkaConfig {
    /// Truncation dimension for Matryoshka embeddings
    /// Options: 128, 256, 512, 1024 (full)
    pub truncation_dim: MatryoshkaDim,

    /// Automatically select dimension based on corpus size
    /// - < 10K: use 128D
    /// - 10K-100K: use 256D
    /// - 100K-1M: use 512D
    /// - > 1M: use 1024D
    pub adaptive_dim: bool,

    /// Minimum recall threshold for adaptive selection
    pub min_recall_threshold: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatryoshkaDim {
    D128 = 128,
    D256 = 256,
    D512 = 512,
    D1024 = 1024,
}

impl Default for MatryoshkaConfig {
    fn default() -> Self {
        Self {
            truncation_dim: MatryoshkaDim::D128,
            adaptive_dim: true,
            min_recall_threshold: 0.95,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RerankConfig {
    /// Weighting strategy for aggregation
    pub weighting_strategy: WeightingStrategy,

    /// How many to pass to next stage
    pub pass_through_k: usize,

    /// Whether to use purpose weighting
    pub use_purpose_weighting: bool,

    /// Fusion method for combining multi-space rankings
    pub fusion_method: FusionMethod,

    /// RRF k parameter (default 60.0) - higher values give more weight to lower ranks
    pub rrf_k: f32,

    /// Matryoshka embedding configuration for fast ANN
    pub matryoshka: MatryoshkaConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FusionMethod {
    /// Reciprocal Rank Fusion: score = sum(1 / (k + rank_i))
    RRF,
    /// Weighted sum of normalized scores
    WeightedSum,
    /// Relative score normalization before fusion
    RelativeScore,
}

impl Default for FusionMethod {
    fn default() -> Self {
        FusionMethod::RRF
    }
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            weighting_strategy: WeightingStrategy::Adaptive,
            pass_through_k: 100,
            use_purpose_weighting: true,
            fusion_method: FusionMethod::RRF,
            rrf_k: 60.0,
            matryoshka: MatryoshkaConfig::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AlignmentStageConfig {
    /// Weight for purpose alignment in final score
    pub purpose_weight: f32,

    /// Weight for goal alignment in final score
    pub goal_weight: f32,

    /// Active goals for alignment
    pub active_goals: Vec<GoalId>,

    /// How many to pass to next stage
    pub pass_through_k: usize,
}

#[derive(Clone, Debug)]
pub struct LateInteractionConfig {
    /// Whether to enable late interaction
    pub enabled: bool,

    /// Weight in final score
    pub weight: f32,

    /// Token similarity threshold
    pub min_token_similarity: f32,
}

#[derive(Clone, Debug)]
pub struct MisalignmentConfig {
    /// Whether to flag misaligned results
    pub enabled: bool,

    /// Threshold below which to flag
    pub alignment_threshold: f32,

    /// Whether to filter out misaligned (vs just flag)
    pub filter_misaligned: bool,
}

/// Query for teleological retrieval
#[derive(Clone, Debug)]
pub struct TeleologicalQuery {
    /// Query text
    pub text: String,

    /// Pre-computed query embeddings (optional)
    pub embeddings: Option<SemanticFingerprint>,

    /// Query's purpose (optional, computed if not provided)
    pub purpose: Option<PurposeVector>,

    /// Target goals (optional)
    pub target_goals: Option<Vec<GoalId>>,

    /// Johari filter (only return specific quadrants)
    pub johari_filter: Option<Vec<JohariQuadrant>>,

    /// Configuration overrides
    pub config: Option<TeleologicalRetrievalConfig>,
}

/// Result from teleological retrieval
#[derive(Clone, Debug)]
pub struct TeleologicalRetrievalResult {
    /// Final ranked results
    pub results: Vec<ScoredMemory>,

    /// Query metadata
    pub query_metadata: QueryMetadata,

    /// Pipeline execution stats
    pub pipeline_stats: PipelineStats,

    /// Detailed breakdown (if requested)
    pub breakdown: Option<PipelineBreakdown>,
}

/// A scored memory result
#[derive(Clone, Debug)]
pub struct ScoredMemory {
    /// Memory identifier
    pub memory_id: MemoryId,

    /// Final composite score
    pub score: f32,

    /// Content similarity score
    pub content_similarity: f32,

    /// Purpose alignment score
    pub purpose_alignment: f32,

    /// Goal alignment score
    pub goal_alignment: f32,

    /// Johari quadrant
    pub johari_quadrant: JohariQuadrant,

    /// Misalignment flag
    pub is_misaligned: bool,

    /// Explanation for ranking
    pub explanation: Option<String>,
}

/// Metadata about the query
#[derive(Clone, Debug)]
pub struct QueryMetadata {
    pub computed_embeddings: bool,
    pub computed_purpose: bool,
    pub active_spaces: Vec<usize>,
    pub active_goals: Vec<GoalId>,
}

/// Statistics about pipeline execution (5-stage pipeline)
#[derive(Clone, Debug)]
pub struct PipelineStats {
    /// Total end-to-end latency
    pub total_time_ms: f32,

    /// Stage 1: Sparse pre-filter (BM25 + E13 SPLADE) - target <5ms
    pub sparse_prefilter_time_ms: f32,

    /// Stage 2: Fast dense ANN (Matryoshka 128D) - target <10ms
    pub matryoshka_ann_time_ms: f32,

    /// Stage 3: Multi-space rerank (RRF Fusion) - target <20ms
    pub rrf_rerank_time_ms: f32,

    /// Stage 4: Teleological alignment filter - target <10ms
    pub alignment_time_ms: f32,

    /// Stage 5: Late interaction rerank (E12 MaxSim) - target <15ms
    pub late_interaction_time_ms: f32,

    /// Candidates remaining after each stage
    /// [sparse_out, matryoshka_out, rrf_out, alignment_out, final_out]
    pub candidates_per_stage: [usize; 5],

    /// Matryoshka dimension used (128/256/512/1024)
    pub matryoshka_dim_used: usize,

    /// Whether SPLADE was used in pre-filter
    pub splade_enabled: bool,

    /// RRF k parameter used
    pub rrf_k_used: f32,
}

/// Detailed breakdown per stage
#[derive(Clone, Debug)]
pub struct PipelineBreakdown {
    pub prefilter_results: Vec<(MemoryId, f32)>,
    pub rerank_results: Vec<(MemoryId, CrossSpaceSimilarity)>,
    pub alignment_results: Vec<(MemoryId, GoalAlignmentScore)>,
    pub late_interaction_results: Option<Vec<(MemoryId, f32)>>,
    pub filtered_misaligned: Vec<MemoryId>,
}
```

### Core Trait

```rust
/// Teleological retrieval pipeline
#[async_trait]
pub trait TeleologicalRetrievalPipeline: Send + Sync {
    /// Execute full retrieval pipeline
    async fn retrieve(
        &self,
        query: TeleologicalQuery,
    ) -> Result<TeleologicalRetrievalResult, RetrievalError>;

    /// Execute with streaming results
    async fn retrieve_streaming(
        &self,
        query: TeleologicalQuery,
    ) -> Result<impl Stream<Item = ScoredMemory>, RetrievalError>;

    /// Get recommended configuration for query
    fn recommend_config(
        &self,
        query: &TeleologicalQuery,
    ) -> TeleologicalRetrievalConfig;

    /// Warm up pipeline components
    async fn warm_up(&self) -> Result<(), RetrievalError>;

    /// Get pipeline health status
    fn health_status(&self) -> PipelineHealth;
}

/// Health status of the pipeline
#[derive(Clone, Debug)]
pub struct PipelineHealth {
    pub is_ready: bool,
    pub indexes_loaded: Vec<usize>,
    pub missing_components: Vec<String>,
    pub last_query_time: Option<Timestamp>,
}
```

### Implementation

```rust
pub struct DefaultTeleologicalPipeline {
    query_executor: Arc<dyn MultiEmbeddingQueryExecutor>,
    purpose_computer: Arc<dyn PurposeVectorComputer>,
    alignment_calculator: Arc<dyn GoalAlignmentCalculator>,
    johari_manager: Arc<dyn JohariTransitionManager>,
    index_manager: Arc<dyn MultiSpaceIndexManager>,
    purpose_index: Arc<dyn PurposePatternIndex>,
    similarity_engine: Arc<dyn CrossSpaceSimilarityEngine>,
    embedding_provider: Arc<dyn EmbeddingProvider>,
    memory_store: Arc<dyn MemoryStore>,
    default_config: TeleologicalRetrievalConfig,
}

#[async_trait]
impl TeleologicalRetrievalPipeline for DefaultTeleologicalPipeline {
    async fn retrieve(
        &self,
        query: TeleologicalQuery,
    ) -> Result<TeleologicalRetrievalResult, RetrievalError> {
        let start = Instant::now();
        let config = query.config.as_ref().unwrap_or(&self.default_config);

        // Prepare query embeddings
        let query_embeddings = match query.embeddings {
            Some(ref emb) => emb.clone(),
            None => self.embedding_provider.embed(&query.text).await?,
        };

        // Prepare query purpose
        let query_purpose = match query.purpose {
            Some(ref p) => p.clone(),
            None => self.purpose_computer.compute_purpose(
                &query_embeddings,
                &PurposeComputeConfig::default(),
            ).await?,
        };

        // Stage 1: Pre-filter
        let prefilter_start = Instant::now();
        let prefilter_results = self.execute_prefilter(
            &query_embeddings,
            &config.prefilter,
        ).await?;
        let prefilter_time = prefilter_start.elapsed();

        // Stage 2: Multi-embedding Rerank
        let rerank_start = Instant::now();
        let rerank_results = self.execute_rerank(
            &query_embeddings,
            &query_purpose,
            prefilter_results,
            &config.rerank,
        ).await?;
        let rerank_time = rerank_start.elapsed();

        // Stage 3: Teleological Alignment
        let alignment_start = Instant::now();
        let alignment_results = self.execute_alignment(
            &query_purpose,
            &query.target_goals.as_ref().unwrap_or(&config.alignment.active_goals),
            rerank_results,
            &config.alignment,
        ).await?;
        let alignment_time = alignment_start.elapsed();

        // Stage 4: Late Interaction (optional)
        let late_start = Instant::now();
        let late_results = if config.late_interaction.enabled {
            Some(self.execute_late_interaction(
                &query_embeddings,
                alignment_results.clone(),
                &config.late_interaction,
            ).await?)
        } else {
            None
        };
        let late_time = late_start.elapsed();

        // Stage 5: Misalignment Check
        let misalign_start = Instant::now();
        let final_results = self.execute_misalignment_check(
            late_results.as_ref().unwrap_or(&alignment_results),
            &config.misalignment,
        ).await?;
        let misalign_time = misalign_start.elapsed();

        // Build final results
        let results = self.build_scored_memories(
            final_results,
            &query_purpose,
            query.johari_filter.as_ref(),
            config.final_limit,
        ).await?;

        Ok(TeleologicalRetrievalResult {
            results,
            query_metadata: QueryMetadata {
                computed_embeddings: query.embeddings.is_none(),
                computed_purpose: query.purpose.is_none(),
                active_spaces: config.prefilter.active_spaces.to_list(),
                active_goals: config.alignment.active_goals.clone(),
            },
            pipeline_stats: PipelineStats {
                total_time_ms: start.elapsed().as_secs_f32() * 1000.0,
                prefilter_time_ms: prefilter_time.as_secs_f32() * 1000.0,
                rerank_time_ms: rerank_time.as_secs_f32() * 1000.0,
                alignment_time_ms: alignment_time.as_secs_f32() * 1000.0,
                late_interaction_time_ms: late_time.as_secs_f32() * 1000.0,
                misalignment_time_ms: misalign_time.as_secs_f32() * 1000.0,
                candidates_per_stage: [
                    prefilter_results.len(),
                    rerank_results.len(),
                    alignment_results.len(),
                    late_results.as_ref().map(|r| r.len()).unwrap_or(0),
                    final_results.len(),
                ],
            },
            breakdown: if config.include_breakdown {
                Some(PipelineBreakdown { /* ... */ })
            } else {
                None
            },
        })
    }
}
```

## Implementation Requirements

### Prerequisites

- [ ] TASK-L001 complete (Multi-Embedding Query Executor)
- [ ] TASK-L002 complete (Purpose Vector Computation)
- [ ] TASK-L003 complete (Goal Alignment Calculator)
- [ ] TASK-L004 complete (Johari Transition Manager)
- [ ] TASK-L005 complete (Per-Space HNSW Index Builder)
- [ ] TASK-L006 complete (Purpose Pattern Index)
- [ ] TASK-L007 complete (Cross-Space Similarity Engine)

### Scope

#### In Scope

- 5-stage retrieval pipeline
- Configuration for each stage
- Pipeline orchestration
- Result aggregation and scoring
- Streaming results
- Pipeline health monitoring

#### Out of Scope

- Individual component implementations (prior tasks)
- UI/API layer (Surface Layer)
- Learning from feedback (future enhancement)

### Constraints

- End-to-end latency < 60ms for typical queries (was 100ms)
- Stage timing budgets:
  - Stage 1 (Sparse Pre-filter): < 5ms
  - Stage 2 (Matryoshka ANN): < 10ms
  - Stage 3 (RRF Rerank): < 20ms
  - Stage 4 (Alignment Filter): < 10ms
  - Stage 5 (Late Interaction): < 15ms
- Support up to 1M candidates in pre-filter (was 100K)
- Memory efficient (no full materialization until needed)
- Thread-safe and parallelizable
- Matryoshka truncation must maintain > 95% recall vs full dimension
- SPLADE index must support incremental updates

## Pseudo Code

```
FUNCTION retrieve(query):
    config = query.config OR default_config
    start = now()

    // ===== PREPARE QUERY =====
    IF query.embeddings IS NULL:
        query_embeddings = embedding_provider.embed(query.text)
    ELSE:
        query_embeddings = query.embeddings

    IF query.purpose IS NULL:
        query_purpose = purpose_computer.compute(query_embeddings)
    ELSE:
        query_purpose = query.purpose

    // ===== STAGE 1: SPARSE PRE-FILTER (BM25 + E13 SPLADE) =====
    // Target: <5ms, 10K candidates from up to 1M corpus
    sparse_start = now()
    sparse_candidates = []

    IF config.prefilter.sparse_prefilter.enabled:
        // Get E13 SPLADE sparse vector from query embeddings
        query_splade = query_embeddings.embeddings[12]  // E13: SPLADE

        // BM25 retrieval
        bm25_results = bm25_index.search(
            query.text,
            config.prefilter.sparse_prefilter.splade_top_k,
            k1 = config.prefilter.sparse_prefilter.bm25_k1,
            b = config.prefilter.sparse_prefilter.bm25_b
        )

        // SPLADE retrieval (learned sparse)
        splade_results = splade_index.search(
            query_splade,
            config.prefilter.sparse_prefilter.splade_top_k
        )

        // Combine BM25 + SPLADE scores
        sparse_weight = config.prefilter.sparse_prefilter.sparse_weight
        FOR memory_id IN UNION(bm25_results, splade_results):
            bm25_score = bm25_results.get(memory_id, 0.0)
            splade_score = splade_results.get(memory_id, 0.0)
            combined = (1.0 - sparse_weight) * bm25_score + sparse_weight * splade_score
            sparse_candidates.push((memory_id, combined))

        // Sort and truncate to max_candidates
        sparse_candidates.sort_by_score_desc()
        sparse_candidates.truncate(config.prefilter.sparse_prefilter.max_candidates)
    ELSE:
        // Fallback: use all indexed memories
        sparse_candidates = index_manager.get_all_ids()

    sparse_time = now() - sparse_start  // Target: <5ms

    // ===== STAGE 2: FAST DENSE ANN (Matryoshka 128D) =====
    // Target: <10ms, 1K candidates from 10K
    matryoshka_start = now()

    // Select Matryoshka dimension adaptively or use configured
    IF config.rerank.matryoshka.adaptive_dim:
        corpus_size = sparse_candidates.len()
        matryoshka_dim = SELECT_DIM(corpus_size):
            < 10K:   MatryoshkaDim::D128
            10K-100K: MatryoshkaDim::D256
            100K-1M:  MatryoshkaDim::D512
            > 1M:    MatryoshkaDim::D1024
    ELSE:
        matryoshka_dim = config.rerank.matryoshka.truncation_dim

    // Truncate query embeddings to Matryoshka dimension
    truncated_query_embeddings = truncate_matryoshka(
        query_embeddings,
        matryoshka_dim
    )

    // HNSW search across active spaces using truncated embeddings
    ann_candidates = []
    FOR space_idx IN config.prefilter.active_spaces:
        query_vector = truncated_query_embeddings.embeddings[space_idx]
        IF query_vector IS NOT NULL:
            // Only search within sparse pre-filtered candidates
            space_results = index_manager.search_filtered(
                space_idx,
                query_vector,
                config.prefilter.per_space_k,
                config.prefilter.ef_search,
                filter_ids = sparse_candidates.ids()
            )
            ann_candidates.extend(space_results)

    // Deduplicate by memory_id, keeping highest similarity
    ann_candidates = deduplicate_by_id(ann_candidates)
    ann_candidates.truncate(1000)  // 1K candidates for next stage

    matryoshka_time = now() - matryoshka_start  // Target: <10ms

    // ===== STAGE 3: MULTI-SPACE RERANK (RRF Fusion) =====
    // Target: <20ms, 100 candidates from 1K
    rrf_start = now()

    // Collect rankings from each embedding space
    space_rankings = {}  // space_idx -> [(memory_id, rank)]
    FOR space_idx IN config.prefilter.active_spaces:
        query_vector = query_embeddings.embeddings[space_idx]  // Full dimension now
        IF query_vector IS NOT NULL:
            space_results = []
            FOR (memory_id, _) IN ann_candidates:
                memory_fp = memory_store.get_fingerprint(memory_id)
                sim = cosine_similarity(query_vector, memory_fp.embeddings[space_idx])
                space_results.push((memory_id, sim))

            space_results.sort_by_sim_desc()
            space_rankings[space_idx] = space_results.enumerate()  // Add rank

    // Apply fusion method
    rrf_scores = {}
    MATCH config.rerank.fusion_method:
        FusionMethod::RRF:
            // Reciprocal Rank Fusion: score = sum(1 / (k + rank))
            k = config.rerank.rrf_k  // default 60.0
            FOR (space_idx, rankings) IN space_rankings:
                FOR (memory_id, rank) IN rankings:
                    rrf_scores[memory_id] += 1.0 / (k + rank)

        FusionMethod::WeightedSum:
            // Normalize and sum weighted scores
            FOR (space_idx, rankings) IN space_rankings:
                weight = get_space_weight(space_idx, config)
                FOR (memory_id, sim) IN rankings:
                    rrf_scores[memory_id] += weight * sim

        FusionMethod::RelativeScore:
            // Normalize scores relative to max in each space
            FOR (space_idx, rankings) IN space_rankings:
                max_sim = rankings[0].sim
                FOR (memory_id, sim) IN rankings:
                    rrf_scores[memory_id] += sim / max_sim

    // Apply purpose weighting if enabled
    IF config.rerank.use_purpose_weighting:
        FOR memory_id IN rrf_scores.keys():
            memory_fp = memory_store.get_fingerprint(memory_id)
            purpose_boost = cosine_similarity(
                query_purpose.alignment,
                memory_fp.purpose_vector.alignment
            )
            rrf_scores[memory_id] *= (1.0 + 0.2 * purpose_boost)  // 20% max boost

    // Sort and truncate
    rerank_results = rrf_scores.to_sorted_vec()
    rerank_results.truncate(config.rerank.pass_through_k)  // 100 candidates

    rrf_time = now() - rrf_start  // Target: <20ms

    // ===== STAGE 4: TELEOLOGICAL ALIGNMENT FILTER =====
    // Target: <10ms, 50 candidates from 100
    alignment_start = now()
    alignment_results = []

    FOR (memory_id, rrf_score) IN rerank_results:
        memory_fingerprint = memory_store.get_fingerprint(memory_id)

        // Purpose alignment
        purpose_sim = cosine_similarity(
            query_purpose.alignment,
            memory_fingerprint.purpose_vector.alignment
        )

        // Goal alignment
        goal_score = alignment_calculator.calculate(
            memory_fingerprint,
            config.alignment.active_goals
        )

        // Combined score with teleological weighting
        combined = rrf_score * (1.0 - config.alignment.purpose_weight - config.alignment.goal_weight)
                 + purpose_sim * config.alignment.purpose_weight
                 + goal_score.composite_score * config.alignment.goal_weight

        alignment_results.push((memory_id, combined, purpose_sim, goal_score, rrf_score))

    // Sort by combined score and truncate
    alignment_results.sort_by_key(|r| -r.1)
    alignment_results.truncate(config.alignment.pass_through_k)  // 50 candidates

    alignment_time = now() - alignment_start  // Target: <10ms

    // ===== STAGE 5: LATE INTERACTION RERANK (E12 MaxSim) =====
    // Target: <15ms, final 10 results from 50
    late_start = now()
    final_results = []
    filtered_misaligned = []

    // Get E12 ColBERT token embeddings for query
    query_tokens = query_embeddings.embeddings[11]  // E12: ColBERT late interaction

    FOR (memory_id, combined, purpose_sim, goal_score, rrf_score) IN alignment_results:
        memory_fingerprint = memory_store.get_fingerprint(memory_id)
        memory_tokens = memory_fingerprint.semantic_fingerprint.embeddings[11]  // E12

        IF config.late_interaction.enabled AND query_tokens IS NOT NULL AND memory_tokens IS NOT NULL:
            // MaxSim computation: for each query token, find max similarity to any doc token
            max_sim_score = compute_max_sim(query_tokens, memory_tokens)

            // Blend with previous score
            final_score = combined * (1.0 - config.late_interaction.weight)
                        + max_sim_score * config.late_interaction.weight
        ELSE:
            final_score = combined

        // Misalignment check
        is_misaligned = goal_score.composite_score < config.misalignment.alignment_threshold
                     OR goal_score.misalignment_flags.any_set()

        IF is_misaligned AND config.misalignment.filter_misaligned:
            filtered_misaligned.push(memory_id)
            CONTINUE

        // Get Johari quadrant
        johari = johari_manager.get_johari(memory_id)
        dominant_quadrant = get_dominant_quadrant(johari)

        // Apply Johari filter if specified
        IF query.johari_filter IS NOT NULL:
            IF dominant_quadrant NOT IN query.johari_filter:
                CONTINUE

        final_results.push(ScoredMemory {
            memory_id,
            score: final_score,
            content_similarity: rrf_score,
            purpose_alignment: purpose_sim,
            goal_alignment: goal_score.composite_score,
            johari_quadrant: dominant_quadrant,
            is_misaligned,
            explanation: generate_explanation(...)
        })

    // Final sort and limit
    final_results.sort_by_key(|m| -m.score)
    final_results.truncate(config.final_limit)  // Final 10 results

    late_time = now() - late_start  // Target: <15ms

    RETURN TeleologicalRetrievalResult {
        results: final_results,
        query_metadata: QueryMetadata {
            computed_embeddings: query.embeddings.is_none(),
            computed_purpose: query.purpose.is_none(),
            active_spaces: config.prefilter.active_spaces.to_list(),
            active_goals: config.alignment.active_goals.clone(),
        },
        pipeline_stats: PipelineStats {
            total_time_ms: (now() - start).as_ms(),  // Target: <60ms total
            sparse_prefilter_time_ms: sparse_time.as_ms(),
            matryoshka_ann_time_ms: matryoshka_time.as_ms(),
            rrf_rerank_time_ms: rrf_time.as_ms(),
            alignment_time_ms: alignment_time.as_ms(),
            late_interaction_time_ms: late_time.as_ms(),
            candidates_per_stage: [
                sparse_candidates.len(),   // ~10K after sparse
                ann_candidates.len(),      // ~1K after Matryoshka
                rerank_results.len(),      // ~100 after RRF
                alignment_results.len(),   // ~50 after alignment
                final_results.len(),       // ~10 final
            ],
            matryoshka_dim_used: matryoshka_dim as usize,
            splade_enabled: config.prefilter.sparse_prefilter.enabled,
            rrf_k_used: config.rerank.rrf_k,
        },
        breakdown: IF config.include_breakdown THEN Some(...) ELSE None
    }
```

## Definition of Done

### Implementation Checklist

- [ ] `TeleologicalRetrievalConfig` with all stage configs
- [ ] `TeleologicalQuery` with all query options
- [ ] `TeleologicalRetrievalResult` with stats and breakdown
- [ ] `ScoredMemory` with all scoring components
- [ ] `TeleologicalRetrievalPipeline` trait
- [ ] Default implementation with 5 stages
- [ ] Pre-filter stage (multi-space HNSW)
- [ ] Rerank stage (cross-space similarity)
- [ ] Alignment stage (purpose + goal)
- [ ] Late interaction stage (MaxSim)
- [ ] Misalignment check stage
- [ ] Pipeline health monitoring
- [ ] Streaming results support

### Testing Requirements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_pipeline() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 1000).await;

        let query = TeleologicalQuery {
            text: "How to implement authentication?".into(),
            embeddings: None,
            purpose: None,
            target_goals: None,
            johari_filter: None,
            config: None,
        };

        let result = pipeline.retrieve(query).await.unwrap();

        assert!(!result.results.is_empty());
        assert!(result.results.len() <= DEFAULT_FINAL_LIMIT);
        assert!(result.pipeline_stats.total_time_ms < 100.0);
    }

    #[tokio::test]
    async fn test_purpose_weighted_retrieval() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 1000).await;

        let query = TeleologicalQuery {
            text: "authentication".into(),
            purpose: Some(create_security_focused_purpose()),
            ..Default::default()
        };

        let result = pipeline.retrieve(query).await.unwrap();

        // Results should favor security-aligned memories
        let avg_purpose_alignment: f32 = result.results.iter()
            .map(|r| r.purpose_alignment)
            .sum::<f32>() / result.results.len() as f32;

        assert!(avg_purpose_alignment > 0.5);
    }

    #[tokio::test]
    async fn test_goal_filtering() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 1000).await;

        let query = TeleologicalQuery {
            text: "implementation".into(),
            target_goals: Some(vec![GoalId("security".into())]),
            ..Default::default()
        };

        let config = TeleologicalRetrievalConfig {
            misalignment: MisalignmentConfig {
                enabled: true,
                alignment_threshold: 0.3,
                filter_misaligned: true,
            },
            ..Default::default()
        };

        let result = pipeline.retrieve(TeleologicalQuery {
            config: Some(config),
            ..query
        }).await.unwrap();

        // No misaligned results should be present
        for r in &result.results {
            assert!(!r.is_misaligned);
            assert!(r.goal_alignment >= 0.3);
        }
    }

    #[tokio::test]
    async fn test_johari_filter() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 1000).await;

        let query = TeleologicalQuery {
            text: "test query".into(),
            johari_filter: Some(vec![JohariQuadrant::Open]),
            ..Default::default()
        };

        let result = pipeline.retrieve(query).await.unwrap();

        for r in &result.results {
            assert_eq!(r.johari_quadrant, JohariQuadrant::Open);
        }
    }

    #[tokio::test]
    async fn test_pipeline_stages_timing() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 10000).await;

        let query = TeleologicalQuery {
            text: "complex query with many matches".into(),
            ..Default::default()
        };

        let config = TeleologicalRetrievalConfig {
            include_breakdown: true,
            ..Default::default()
        };

        let result = pipeline.retrieve(TeleologicalQuery {
            config: Some(config),
            ..query
        }).await.unwrap();

        // Verify all stages executed
        assert!(result.pipeline_stats.prefilter_time_ms > 0.0);
        assert!(result.pipeline_stats.rerank_time_ms > 0.0);
        assert!(result.pipeline_stats.alignment_time_ms > 0.0);

        // Verify breakdown present
        assert!(result.breakdown.is_some());
    }

    #[tokio::test]
    async fn test_late_interaction_toggle() {
        let pipeline = create_test_pipeline().await;
        populate_test_data(&pipeline, 100).await;

        let query = TeleologicalQuery {
            text: "test".into(),
            ..Default::default()
        };

        let config_with = TeleologicalRetrievalConfig {
            late_interaction: LateInteractionConfig {
                enabled: true,
                weight: 0.3,
                min_token_similarity: 0.5,
            },
            ..Default::default()
        };

        let config_without = TeleologicalRetrievalConfig {
            late_interaction: LateInteractionConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let result_with = pipeline.retrieve(TeleologicalQuery {
            config: Some(config_with),
            ..query.clone()
        }).await.unwrap();

        let result_without = pipeline.retrieve(TeleologicalQuery {
            config: Some(config_without),
            ..query
        }).await.unwrap();

        // Late interaction should affect timing
        assert!(result_with.pipeline_stats.late_interaction_time_ms > 0.0);
        assert_eq!(result_without.pipeline_stats.late_interaction_time_ms, 0.0);
    }
}
```

### Verification Commands

```bash
# Run unit tests
cargo test -p context-graph-core teleological_retrieval

# Run integration tests
cargo test -p context-graph-core --features integration retrieval_pipeline

# Benchmark full pipeline
cargo bench -p context-graph-core -- teleological_pipeline

# Load test
cargo test -p context-graph-core retrieval_load_test -- --ignored --nocapture
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/retrieval/pipeline.rs` | TeleologicalRetrievalPipeline trait and impl |
| `crates/context-graph-core/src/retrieval/config.rs` | All configuration structs |
| `crates/context-graph-core/src/retrieval/query.rs` | TeleologicalQuery |
| `crates/context-graph-core/src/retrieval/result.rs` | Result types |
| `crates/context-graph-core/src/retrieval/stages/mod.rs` | Stage implementations |
| `crates/context-graph-core/src/retrieval/stages/prefilter.rs` | Pre-filter stage |
| `crates/context-graph-core/src/retrieval/stages/rerank.rs` | Rerank stage |
| `crates/context-graph-core/src/retrieval/stages/alignment.rs` | Alignment stage |
| `crates/context-graph-core/src/retrieval/stages/late_interaction.rs` | Late interaction stage |
| `crates/context-graph-core/src/retrieval/stages/misalignment.rs` | Misalignment check stage |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/retrieval/mod.rs` | Add pipeline exports |
| `crates/context-graph-core/src/lib.rs` | Re-export pipeline |

## Traceability

| Requirement | Source | Coverage |
|-------------|--------|----------|
| 5-stage pipeline | projectionplan1.md:retrieval | Complete |
| Purpose alignment | projectionplan1.md:purpose | Complete |
| Goal alignment | projectionplan2.md:goals | Complete |
| Late interaction | projectionplan2.md:colbert | Complete |
| Misalignment check | projectionplan2.md:drift | Complete |
| Johari filtering | projectionplan1.md:johari | Complete |

---

*Task created: 2026-01-04*
*Layer: Logic*
*Priority: P0 - Capstone integration*
