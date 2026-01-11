# TASK-STORAGE-P2-001: Implement MaxSim Late Interaction for Stage 5

```xml
<task_spec id="TASK-STORAGE-P2-001" version="1.0">
<metadata>
  <title>Implement MaxSim Late Interaction Scoring for Stage 5</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>2</sequence>
  <implements>
    <item>PRD: Stage 5 Late Interaction MaxSim reranking (<15ms for 50 candidates)</item>
    <item>SHERLOCK-08: Complete MaxSim implementation (currently 20% complete)</item>
    <item>L2F: Late Interaction Index (E12 MaxSim) storage and scoring</item>
  </implements>
  <depends_on>
    <task_ref>TASK-STORAGE-P1-001</task_ref>
    <!-- Note: TASK-STORAGE-P1-001 is the HNSW Graph Traversal implementation -->
    <!-- This task can proceed in parallel as it doesn't require HNSW for Stage 5 -->
    <!-- Stage 5 uses direct token-level scoring, NOT HNSW -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Stage 5 of the 5-stage retrieval pipeline performs ColBERT-style late interaction
reranking using MaxSim scoring. Currently, the pipeline has a placeholder
`InMemoryTokenStorage` that stores token embeddings but lacks:
1. Proper persistent storage for E12 ColBERT token embeddings
2. SIMD-optimized MaxSim scoring algorithm
3. Batch processing for efficient candidate reranking

The MaxSim algorithm computes: score = (1/|Q|) * sum_i(max_j(cos(q_i, d_j)))
where q_i are query token embeddings and d_j are document token embeddings.

E12 embeddings are 128D per token (ColBERT-style), stored as Vec<Vec<f32>>.
</context>

<input_context_files>
  <file purpose="Stage 5 pipeline implementation with compute_maxsim stub">
    /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/pipeline.rs
  </file>
  <file purpose="Column families including CF_E12_LATE_INTERACTION (if exists)">
    /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/column_families.rs
  </file>
  <file purpose="E12 dimension constant (128D per token)">
    /home/cabdru/contextgraph/crates/context-graph-core/src/types/fingerprint/semantic/constants.rs
  </file>
  <file purpose="SemanticFingerprint with e12_late_interaction field">
    /home/cabdru/contextgraph/crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs
  </file>
  <file purpose="Distance metric definitions including MaxSim">
    /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/indexes/metrics.rs
  </file>
  <file purpose="RocksDB store for persistence patterns">
    /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store.rs
  </file>
</input_context_files>

<prerequisites>
  <check>E12_TOKEN_DIM constant defined as 128 in constants.rs</check>
  <check>TokenStorage trait exists in pipeline.rs with get_tokens() method</check>
  <check>InMemoryTokenStorage implements TokenStorage trait</check>
  <check>compute_maxsim() method exists in RetrievalPipeline</check>
  <check>stage_maxsim_rerank() calls compute_maxsim() for scoring</check>
</prerequisites>

<scope>
  <in_scope>
    - RocksDB-backed TokenStorage implementation with CF_E12_LATE_INTERACTION
    - SIMD-optimized MaxSim scoring algorithm (using packed_simd or std::simd)
    - Batch token retrieval for efficient candidate processing
    - Token embedding serialization/deserialization (bincode or postcard)
    - Unit tests for MaxSim correctness
    - Benchmark tests for <15ms @ 50 candidates target
    - Integration with existing pipeline.rs Stage 5
  </in_scope>
  <out_of_scope>
    - ColBERT model inference (embeddings assumed pre-computed)
    - Query token generation (handled by embedding service)
    - HNSW indexing for tokens (E12 uses direct MaxSim, not ANN)
    - Changes to other pipeline stages (S1-S4)
    - ScyllaDB backend (separate P1 task)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/token_storage.rs">
/// RocksDB-backed token storage for E12 ColBERT embeddings.
pub struct RocksDbTokenStorage {
    db: Arc<DB>,
}

impl RocksDbTokenStorage {
    /// Create new token storage with RocksDB handle.
    pub fn new(db: Arc<DB>) -> Self;

    /// Store token embeddings for a memory ID.
    /// Tokens: Vec of 128D embeddings, one per token.
    pub fn store(&self, id: Uuid, tokens: &[Vec<f32>]) -> Result<(), TokenStorageError>;

    /// Batch store multiple token sets.
    pub fn store_batch(&self, batch: &[(Uuid, Vec<Vec<f32>>)]) -> Result<(), TokenStorageError>;

    /// Delete token embeddings for a memory ID.
    pub fn delete(&self, id: Uuid) -> Result<bool, TokenStorageError>;
}

impl TokenStorage for RocksDbTokenStorage {
    fn get_tokens(&self, id: Uuid) -> Option<Vec<Vec<f32>>>;
}
    </signature>
    <signature file="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/maxsim.rs">
/// SIMD-optimized MaxSim scoring for ColBERT late interaction.
pub struct MaxSimScorer {
    /// Cache for precomputed query token norms
    query_norms: Vec<f32>,
}

impl MaxSimScorer {
    /// Create scorer with precomputed query token norms.
    pub fn new(query_tokens: &[Vec<f32>]) -> Self;

    /// Compute MaxSim score for a single document.
    /// Returns (1/|Q|) * sum_i(max_j(cos(q_i, d_j)))
    pub fn score(&self, query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32;

    /// Batch score multiple documents (parallelized).
    pub fn score_batch(&self, query_tokens: &[Vec<f32>], doc_batch: &[Vec<Vec<f32>>]) -> Vec<f32>;
}

/// Compute cosine similarity between two 128D vectors (SIMD-optimized).
#[inline]
pub fn cosine_similarity_128d(a: &[f32; 128], b: &[f32; 128]) -> f32;
    </signature>
    <signature file="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/column_families.rs">
/// Column family for E12 ColBERT token embeddings.
/// Key: UUID (16 bytes), Value: Serialized Vec<Vec<f32>>
pub const CF_E12_LATE_INTERACTION: &str = "e12_late_interaction";
    </signature>
  </signatures>

  <constraints>
    - MaxSim scoring MUST achieve <15ms for 50 candidates (benchmark required)
    - Token embeddings MUST be 128D (E12_TOKEN_DIM constant)
    - Cosine similarity MUST use SIMD when available (cfg attribute for fallback)
    - Storage MUST use RocksDB column family CF_E12_LATE_INTERACTION
    - Serialization MUST be efficient (bincode with LZ4 or postcard)
    - FAIL FAST: Invalid dimensions must panic with clear message
    - FAIL FAST: NaN/Inf in vectors must return error immediately
    - Thread safety: RocksDbTokenStorage must be Send + Sync
    - Memory efficiency: Batch operations should not load all tokens into memory
  </constraints>

  <verification>
    - `cargo test -p context-graph-storage maxsim` passes all tests
    - `cargo bench -p context-graph-storage maxsim` shows <15ms @ 50 candidates
    - MaxSim score for identical query/doc equals 1.0
    - MaxSim score for orthogonal vectors equals 0.0
    - RocksDB store/retrieve roundtrip preserves exact token values
    - Parallel scoring produces same results as sequential
    - SIMD and non-SIMD paths produce identical results (within f32 epsilon)
  </verification>
</definition_of_done>

<pseudo_code>
RocksDbTokenStorage (token_storage.rs):
  new(db):
    Store Arc<DB> handle
    Verify CF_E12_LATE_INTERACTION column family exists

  store(id, tokens):
    FAIL FAST if any token dimension != 128
    Serialize tokens with bincode/postcard
    Compress with LZ4
    Write to CF_E12_LATE_INTERACTION with id as key

  store_batch(batch):
    Create WriteBatch
    For each (id, tokens): add to batch
    Atomic write batch

  get_tokens(id):
    Read from CF_E12_LATE_INTERACTION
    Decompress LZ4
    Deserialize tokens
    Return Option<Vec<Vec<f32>>>

  delete(id):
    Delete from CF_E12_LATE_INTERACTION
    Return whether key existed

MaxSimScorer (maxsim.rs):
  new(query_tokens):
    Precompute L2 norms for all query tokens
    Store in query_norms vector

  score(query_tokens, doc_tokens):
    If empty: return 0.0
    total_max_sim = 0.0
    For each q_token in query_tokens:
      max_sim = -infinity
      For each d_token in doc_tokens:
        sim = cosine_similarity_128d(q_token, d_token)
        max_sim = max(max_sim, sim)
      total_max_sim += max_sim
    Return total_max_sim / query_tokens.len()

  score_batch(query_tokens, doc_batch):
    Use rayon par_iter for parallel scoring
    Map each doc -> score(query_tokens, doc)
    Collect results

cosine_similarity_128d(a, b):
  #[cfg(target_arch = "x86_64")]
    Use AVX2 intrinsics for 8-wide f32 operations
    dot = sum(a[i] * b[i]) using _mm256_fmadd_ps
    norm_a = sqrt(sum(a[i]^2))
    norm_b = sqrt(sum(b[i]^2))
  #[cfg(not(target_arch = "x86_64"))]
    Fallback to scalar loop
  Return dot / (norm_a * norm_b)

Pipeline Integration (pipeline.rs modifications):
  Update compute_maxsim():
    Use MaxSimScorer for optimized scoring

  Update stage_maxsim_rerank():
    Batch retrieve tokens for all candidates
    Use MaxSimScorer.score_batch() for parallel scoring
    Sort and truncate to k results
</pseudo_code>

<files_to_create>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/token_storage.rs">
    RocksDbTokenStorage implementation with RocksDB persistence
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/maxsim.rs">
    SIMD-optimized MaxSimScorer and cosine_similarity_128d
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/token_storage_tests.rs">
    Unit tests for RocksDbTokenStorage
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/maxsim_tests.rs">
    Unit tests for MaxSimScorer correctness
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/benches/maxsim_bench.rs">
    Criterion benchmarks for MaxSim performance
  </file>
</files_to_create>

<files_to_modify>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/mod.rs">
    Add pub mod token_storage; pub mod maxsim; exports
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/column_families.rs">
    Add CF_E12_LATE_INTERACTION constant if not present
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/search/pipeline.rs">
    Update compute_maxsim() to use MaxSimScorer
    Update stage_maxsim_rerank() to use batch scoring
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store.rs">
    Add token storage population during store() operations
    Extract E12 embeddings from TeleologicalFingerprint.semantic.e12_late_interaction
  </file>
  <file path="/home/cabdru/contextgraph/crates/context-graph-storage/Cargo.toml">
    Add criterion benchmark configuration
    Add bincode/postcard dependency if not present
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>RocksDbTokenStorage::store() and get_tokens() roundtrip preserves all token values exactly</criterion>
  <criterion>MaxSimScorer::score() returns 1.0 for identical query and document tokens</criterion>
  <criterion>MaxSimScorer::score() returns 0.0 for orthogonal token vectors</criterion>
  <criterion>MaxSimScorer::score_batch() produces identical results to sequential scoring</criterion>
  <criterion>cosine_similarity_128d SIMD matches scalar fallback within f32::EPSILON</criterion>
  <criterion>Benchmark: MaxSim scoring for 50 candidates completes in less than 15ms</criterion>
  <criterion>Benchmark: Token retrieval for 50 IDs completes in less than 5ms</criterion>
  <criterion>Memory: Batch processing does not exceed 100MB for 50 candidates with 512 tokens each</criterion>
  <criterion>Thread safety: Concurrent get_tokens() calls do not cause data races</criterion>
  <criterion>FAIL FAST: Storing 127D or 129D tokens panics with dimension error</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage token_storage -- --nocapture</command>
  <command>cargo test -p context-graph-storage maxsim -- --nocapture</command>
  <command>cargo test -p context-graph-storage stage_maxsim -- --nocapture</command>
  <command>cargo bench -p context-graph-storage --bench maxsim_bench</command>
  <command>cargo clippy -p context-graph-storage -- -D warnings</command>
</test_commands>
</task_spec>
```

---

## Algorithm Reference: MaxSim Late Interaction

### ColBERT MaxSim Formula

```
MaxSim(Q, D) = (1/|Q|) * sum_{i=1}^{|Q|} max_{j=1}^{|D|} cos(q_i, d_j)
```

Where:
- `Q = [q_1, q_2, ..., q_m]` are query token embeddings (m tokens, 128D each)
- `D = [d_1, d_2, ..., d_n]` are document token embeddings (n tokens, 128D each)
- `cos(a, b)` is cosine similarity

### Why MaxSim (Not HNSW)

E12 late interaction explicitly does NOT use HNSW because:
1. Token-level matching requires exhaustive comparison (all query tokens vs all doc tokens)
2. ANN approximation would miss critical token alignments
3. The 50-candidate input is small enough for exact computation
4. MaxSim aggregation across tokens cannot be pre-indexed

### SIMD Optimization Strategy

For 128D cosine similarity on x86_64 with AVX2:
```
128 dimensions / 8 floats per 256-bit register = 16 SIMD iterations
```

Using `_mm256_fmadd_ps` for fused multiply-add:
- 16 iterations compute dot product
- Horizontal sum reduces to scalar
- Norm computation shares same pattern

Expected speedup: 4-8x over scalar loop.

---

## Dependency Note

This task depends on TASK-STORAGE-P1-001 (HNSW Graph Traversal) only for the overall pipeline to function correctly. However, Stage 5 itself does NOT use HNSW - it uses direct MaxSim computation. Therefore, this task can be implemented in parallel with P1-001.

The dependency is structural (full pipeline requires both) rather than technical (MaxSim code doesn't call HNSW).

---

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| MaxSim 50 candidates | <15ms | Criterion benchmark |
| Token retrieval 50 IDs | <5ms | Criterion benchmark |
| Memory per candidate | <200KB | 512 tokens * 128D * 4 bytes |
| Parallel efficiency | >80% | 8-core scaling test |

---

## Related Documentation

- [SHERLOCK-08: Storage Architecture](../sherlock-08-storage-architecture.md) - Gap analysis identifying MaxSim as 20% complete
- [SHERLOCK-04: Teleological Fingerprint](../sherlock-04-teleological-fingerprint.md) - E12 specification details
- [PRD: 5-Stage Pipeline](../../README.md) - Stage 5 latency requirements
