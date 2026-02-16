# MCP Context-Graph Full System Verification Plan

> End-to-end operational verification for all 55 MCP tools, 13 embedders,
> 51 column families, 5 CLI hooks, and supporting subsystems.

## How to Use This Plan

Each phase has numbered test cases with:
- **Action**: The MCP tool call or CLI command to execute
- **Expect**: What a passing result looks like
- **Failure**: What to investigate if it fails

Run phases sequentially (1-11). Some tests in later phases depend on data
created in earlier phases. Estimated wall-clock time: 30-45 minutes manual,
5-10 minutes automated.

---

## Phase 1: Infrastructure Health Check

Verify the server starts, RocksDB opens all 51 CFs, all 13 embedders load,
and the LLM (if present) initializes.

### 1.1 Server Startup

**Action**: Start the MCP server
```bash
cargo run --release -p context-graph-mcp 2>&1 | head -50
```
**Expect**: No panics, no "Failed to open column family" errors.
Stdout shows `Listening on stdio` (or configured transport).

**Failure**: If a CF is missing, RocksDB will panic on open. Delete the
database directory and restart. If an embedder fails to load its model,
check `models/` directory contents against `models/models_config.toml`.

### 1.2 System Status

**Action**: `get_memetic_status`
```json
{}
```
**Expect**:
- `embedder_count`: 13
- `storage_backend`: "rocksdb"
- `e5_causal_gate.enabled`: true
- `e5_causal_gate.causal_threshold`: 0.04
- `e5_causal_gate.non_causal_threshold`: 0.008
- `available_profiles` lists all 14 profiles
- `total_fingerprints` >= 0 (count of stored memories)

**Failure**: If `embedder_count` < 13, a model failed to load.
Check `models/` for missing `.safetensors` or `.onnx` files.
If `storage_backend` is not "rocksdb", check data directory permissions.

### 1.3 CUDA/GPU Check (if applicable)

**Action**:
```bash
nvidia-smi
cargo test -p context-graph-cuda -- --nocapture 2>&1 | tail -5
```
**Expect**: CUDA device visible, Poincare distance and cone check kernels compile.

**Failure**: Check CUDA toolkit version matches `nvcc` path in build.rs.
Current: CUDA 13.1, SM 120 (RTX 5090).

---

## Phase 2: Core Memory Operations (CRUD)

Test the fundamental store/search/delete cycle that all other features build on.

### 2.1 Store a Memory

**Action**: `store_memory`
```json
{
  "content": "RocksDB uses log-structured merge trees (LSM) for write-optimized storage. Column families provide logical separation of data with independent compaction.",
  "rationale": "Testing core store operation with technical content"
}
```
**Expect**:
- Returns `memory_id` (UUID format)
- `embedding_latency_ms` < 5000 (first call may be slower due to model warmup)
- `fingerprint_dimensions` shows 13 entries (one per embedder)

**Failure**: If embedding latency is extremely high (>30s), check GPU memory
with `nvidia-smi`. If it returns an error, check RocksDB write permissions.

> **Save the returned `memory_id`** — it is used in phases 3-11.

### 2.2 Store a Second Memory (Different Domain)

**Action**: `store_memory`
```json
{
  "content": "The HTTP/2 protocol uses multiplexed streams over a single TCP connection, reducing head-of-line blocking. HPACK header compression minimizes overhead.",
  "rationale": "Second memory for search relevance testing"
}
```
**Expect**: Different `memory_id`, successful storage.

### 2.3 Store a Third Memory (Code Content)

**Action**: `store_memory`
```json
{
  "content": "fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 { let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt(); let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt(); if norm_a == 0.0 || norm_b == 0.0 { return 0.0; } dot / (norm_a * norm_b) }",
  "rationale": "Code content for E7 code search testing"
}
```
**Expect**: Successful storage with code-relevant fingerprint.

### 2.4 Store a Causal Memory

**Action**: `store_memory`
```json
{
  "content": "Memory leaks in long-running services cause gradual heap growth, which leads to increased GC pressure, which causes latency spikes, which triggers timeout errors in downstream services.",
  "rationale": "Causal chain content for E5 causal testing"
}
```
**Expect**: Successful storage.

### 2.5 Store an Entity-Rich Memory

**Action**: `store_memory`
```json
{
  "content": "PostgreSQL and Redis are used together in the authentication service. PostgreSQL stores user credentials while Redis caches session tokens. The API gateway routes requests through Nginx to the auth service.",
  "rationale": "Entity-rich content for E11 entity testing"
}
```
**Expect**: Successful storage.

### 2.6 Basic Search (Default Strategy)

**Action**: `search_graph`
```json
{
  "query": "LSM tree write optimization",
  "limit": 5
}
```
**Expect**:
- Memory from 2.1 ranks first or second (similarity > 0.7)
- Results include `similarity` scores in descending order
- `search_strategy` is "multi_space" (default)

**Failure**: If similarity is 0.0 for all results, embedders may not have
generated vectors. Check `get_memory_fingerprint` for the stored memory.

### 2.7 Negative Search (No Relevant Results)

**Action**: `search_graph`
```json
{
  "query": "quantum entanglement teleportation protocol",
  "limit": 5
}
```
**Expect**:
- Results returned but with lower similarity scores (< 0.5)
- No false high-confidence matches

---

## Phase 3: Search Strategy Verification

Test all 4 search strategies to confirm each codepath works.

### 3.1 E1Only Strategy

**Action**: `search_graph`
```json
{
  "query": "database storage engine compaction",
  "limit": 5,
  "search_strategy": "e1_only"
}
```
**Expect**: Results ranked by E1 (1024D semantic) similarity only.
Fastest strategy — should complete in < 100ms after warmup.

### 3.2 MultiSpace Strategy (Default)

**Action**: `search_graph`
```json
{
  "query": "database storage engine compaction",
  "limit": 5,
  "search_strategy": "multi_space"
}
```
**Expect**: Results use weighted fusion of E1, E5, E7, E8, E10, E11.
May reorder results vs E1Only if other embedders contribute signal.

### 3.3 Pipeline Strategy (E13 -> E1 -> E12)

**Action**: `search_graph`
```json
{
  "query": "database storage engine compaction",
  "limit": 10,
  "search_strategy": "pipeline"
}
```
**Expect**:
- 3-stage pipeline: E13 sparse recall -> E1 dense scoring -> E12 ColBERT rerank
- Results should be high quality (E12 token-level reranking is most precise)
- Slightly slower than other strategies (3 passes over data)

### 3.4 Embedder-First Strategy

**Action**: `search_by_embedder`
```json
{
  "query": "database storage engine compaction",
  "embedder": "E1_Semantic",
  "limit": 5
}
```
**Expect**: Results ranked purely by E1, with per-embedder breakdown available.

---

## Phase 4: Individual Embedder Verification

Verify each of the 13 embedders generates meaningful vectors and returns
non-trivial results for appropriate queries.

### 4.1 E1 Semantic (1024D, e5-large-v2)

**Action**: `search_by_embedder`
```json
{
  "query": "write-optimized storage engine",
  "embedder": "E1_Semantic",
  "limit": 3
}
```
**Expect**: RocksDB memory (2.1) ranks highest. Similarity > 0.6.

### 4.2 E2 Temporal Recent (512D, Recency Decay)

**Action**: `search_recent`
```json
{
  "query": "storage",
  "limit": 5,
  "recency_boost": 2.0
}
```
**Expect**: Most recently stored memories boosted toward top of results.

### 4.3 E3 Temporal Periodic (512D, Fourier)

**Action**: `search_periodic`
```json
{
  "query": "storage",
  "limit": 5,
  "periodicity_boost": 2.0
}
```
**Expect**: Returns results. With few memories, periodicity signal is weak —
just verify no errors.

### 4.4 E4 Temporal Positional (512D, Sequence)

**Action**: `get_session_timeline`
```json
{
  "session_id": "<current_session_id>",
  "limit": 10
}
```
**Expect**: Returns memories in creation order for the session. E4 encodes
conversation position (V_ordering).

### 4.5 E5 Causal Asymmetric (768D, Longformer)

**Action**: `search_causes`
```json
{
  "query": "timeout errors in downstream services",
  "limit": 5
}
```
**Expect**:
- Causal memory (2.4) ranks high (it contains the causal chain)
- Uses cause->effect direction (1.2x boost)
- E5 causal gate applies: scores above `CAUSAL_THRESHOLD` (0.04) get 1.10x boost

**Action**: `search_effects`
```json
{
  "query": "memory leaks in long-running services",
  "limit": 5
}
```
**Expect**: Same memory found, but searching in effect direction (0.8x).

### 4.6 E6 Sparse Keyword (30K vocab, Inverted Index)

**Action**: `search_by_keywords`
```json
{
  "query": "RocksDB LSM compaction column families",
  "limit": 5
}
```
**Expect**:
- RocksDB memory (2.1) ranks first (exact keyword match)
- Uses inverted index, not HNSW
- Fast: < 50ms

### 4.7 E7 Code (1536D, Qodo-Embed)

**Action**: `search_code`
```json
{
  "query": "cosine similarity vector dot product",
  "limit": 5
}
```
**Expect**:
- Code memory (2.3) ranks first
- Similarity > 0.7 (E7 understands code semantics)

### 4.8 E8 Graph/Connectivity (1024D, Asymmetric)

**Action**: `search_connections`
```json
{
  "query": "database",
  "limit": 5
}
```
**Expect**: Returns results with connection-based scoring.
E8 uses source/target asymmetric vectors.

### 4.9 E9 HDC Robustness (1024D, Hyperdimensional)

**Action**: `search_robust`
```json
{
  "query": "RocksDb LSN tree compactionn",
  "limit": 5
}
```
**Expect**:
- Finds RocksDB memory despite typos ("RocksDb" -> "RocksDB", "LSN" -> "LSM",
  "compactionn" -> "compaction")
- E9 uses hyperdimensional computing for typo tolerance

### 4.10 E10 Paraphrase (768D, Asymmetric, e5-base-v2)

**Action**: `search_by_embedder`
```json
{
  "query": "a tree-based data structure that optimizes for sequential writes",
  "embedder": "E10_Multimodal",
  "limit": 3
}
```
**Expect**: RocksDB memory found via paraphrase match (not keyword overlap).

### 4.11 E11 Entity (768D, KEPLER)

**Action**: `search_by_entities`
```json
{
  "query": "PostgreSQL Redis",
  "limit": 5
}
```
**Expect**: Entity-rich memory (2.5) ranks first. E11 focuses on named entities.

### 4.12 E12 ColBERT Late Interaction (128D/token, MaxSim)

**Action**: `search_graph`
```json
{
  "query": "log structured merge tree",
  "limit": 5,
  "search_strategy": "pipeline"
}
```
**Expect**: Pipeline uses E12 for final reranking (stage 3). Results should
be highly precise due to token-level matching.

### 4.13 E13 SPLADE (30K vocab, Inverted Index)

This is tested implicitly by the Pipeline strategy (3.3) where E13 is stage 1.
To test directly:

**Action**: `search_by_embedder`
```json
{
  "query": "storage engine write optimization",
  "embedder": "E13_SPLADE",
  "limit": 5
}
```
**Expect**: Returns results using SPLADE sparse expansion. E13 expands query
terms to related vocabulary (e.g., "storage" -> includes "database", "disk").

---

## Phase 5: Weight Profiles

Test that weight profile selection changes search behavior.

### 5.1 Semantic Search Profile (Default)

**Action**: `search_graph`
```json
{
  "query": "data storage optimization",
  "limit": 5,
  "weight_profile": "semantic_search"
}
```
**Expect**: E1-dominated results (E1 weight: 0.33).

### 5.2 Code Search Profile

**Action**: `search_graph`
```json
{
  "query": "cosine similarity implementation",
  "limit": 5,
  "weight_profile": "code_search"
}
```
**Expect**: Code memory (2.3) ranks higher than with semantic_search profile
(E7 weight: 0.40 vs 0.05).

### 5.3 Causal Reasoning Profile

**Action**: `search_graph`
```json
{
  "query": "what causes timeout errors",
  "limit": 5,
  "weight_profile": "causal_reasoning"
}
```
**Expect**: Causal memory (2.4) ranks high. E5 direction-aware scoring active.

### 5.4 Fact Checking Profile

**Action**: `search_graph`
```json
{
  "query": "PostgreSQL Redis authentication",
  "limit": 5,
  "weight_profile": "fact_checking"
}
```
**Expect**: Entity memory (2.5) ranks first (E11 weight: 0.40).

### 5.5 Typo Tolerant Profile

**Action**: `search_graph`
```json
{
  "query": "RocksDV compacton",
  "limit": 5,
  "weight_profile": "typo_tolerant"
}
```
**Expect**: RocksDB memory still found despite typos (E9 weight: 0.15 boosted).

### 5.6 Custom Weight Profile

**Action**: `create_weight_profile`
```json
{
  "name": "test_custom_e1_only",
  "weights": {
    "E1_Semantic": 1.0,
    "E5_Causal": 0.0,
    "E6_Sparse": 0.0,
    "E7_Code": 0.0,
    "E8_Emotional": 0.0,
    "E9_HDC": 0.0,
    "E10_Multimodal": 0.0,
    "E11_Entity": 0.0
  }
}
```
**Expect**: Profile created successfully and persisted to CF_CUSTOM_WEIGHT_PROFILES.

**Action**: `search_graph`
```json
{
  "query": "storage engine",
  "limit": 5,
  "weight_profile": "test_custom_e1_only"
}
```
**Expect**: Results identical to e1_only strategy since only E1 has weight.

---

## Phase 6: Causal Subsystem (E5 + LLM)

### 6.1 Causal Chain Traversal

**Action**: `get_causal_chain`
```json
{
  "query": "memory leaks",
  "max_depth": 3,
  "limit": 5
}
```
**Expect**: Returns chain links from "memory leaks" -> "heap growth" ->
"GC pressure" -> "latency spikes" (if LLM causal discovery has run).

### 6.2 Causal Relationship Search

**Action**: `search_causal_relationships`
```json
{
  "description": "memory management problems",
  "limit": 5
}
```
**Expect**: Returns relationships involving memory/heap/GC concepts.

### 6.3 LLM Causal Discovery (Requires Hermes-2-Pro-Mistral-7B)

**Action**: `trigger_causal_discovery`
```json
{
  "content": "Increasing the connection pool size reduces database connection wait times, which improves API response latency."
}
```
**Expect**:
- If LLM loaded: Returns discovered causal relationships
- If LLM not loaded: Returns error "LLM not available" (graceful degradation)

**Action**: `get_causal_discovery_status`
```json
{}
```
**Expect**: Shows status of last discovery run.

### 6.4 Causal Repair

**Action**: `repair_causal_relationships`
```json
{
  "dry_run": true
}
```
**Expect**: Reports count of relationships checked, any corruption found.
Dry-run mode does not modify data.

---

## Phase 7: Graph Subsystem (E8 + LLM)

### 7.1 Graph Path

**Action**: `get_graph_path`
```json
{
  "source_id": "<memory_id_from_2.1>",
  "target_id": "<memory_id_from_2.2>",
  "max_depth": 3
}
```
**Expect**: Returns path (if edges exist) or empty path (if no connection yet).

### 7.2 Memory Neighbors (K-NN)

**Action**: `get_memory_neighbors`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "limit": 5
}
```
**Expect**: Returns K nearest neighbors with similarity scores.

### 7.3 Typed Edges

**Action**: `get_typed_edges`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "limit": 10
}
```
**Expect**: Returns typed edges (if any exist). Edge types include:
similar_to, caused_by, related_to, etc.

### 7.4 Unified Neighbors (Weighted RRF)

**Action**: `get_unified_neighbors`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "limit": 5
}
```
**Expect**: Returns neighbors fused from both K-NN and typed edges via
Reciprocal Rank Fusion.

### 7.5 Graph Traversal

**Action**: `traverse_graph`
```json
{
  "start_id": "<memory_id_from_2.1>",
  "max_depth": 2,
  "limit": 10
}
```
**Expect**: Returns traversal tree with depth annotations.

### 7.6 LLM Graph Discovery (Requires Hermes-2-Pro-Mistral-7B)

**Action**: `discover_graph_relationships`
```json
{
  "content": "RocksDB uses LSM trees. LSM trees optimize sequential writes. Sequential writes are efficient on SSDs.",
  "limit": 5
}
```
**Expect**:
- If LLM loaded: Discovers relationships (e.g., "uses", "optimizes")
- If LLM not loaded: Returns error "LLM not available"

### 7.7 LLM Graph Link Validation

**Action**: `validate_graph_link`
```json
{
  "source_content": "RocksDB uses LSM trees for storage",
  "target_content": "LSM trees optimize write performance",
  "relationship_type": "enables"
}
```
**Expect**:
- If LLM loaded: Returns confidence score and validation reasoning
- If LLM not loaded: Returns error "LLM not available"

---

## Phase 8: Entity Subsystem (E11)

### 8.1 Entity Extraction

**Action**: `extract_entities`
```json
{
  "content": "PostgreSQL 16 was released by the PostgreSQL Global Development Group. It includes improvements to logical replication and query parallelism."
}
```
**Expect**:
- Extracts entities: "PostgreSQL 16" (Software), "PostgreSQL Global Development Group" (Organization)
- Each entity has `type`, `confidence`, and extraction `method`

### 8.2 Entity Search

**Action**: `search_by_entities`
```json
{
  "query": "PostgreSQL",
  "limit": 5
}
```
**Expect**: Entity memory (2.5) found with high relevance.

### 8.3 Relationship Inference

**Action**: `infer_relationship`
```json
{
  "source_entity": "PostgreSQL",
  "target_entity": "Redis",
  "context": "authentication service"
}
```
**Expect**: Returns inferred relationship (e.g., "co-used-in", confidence score).

### 8.4 Related Entities

**Action**: `find_related_entities`
```json
{
  "entity": "PostgreSQL",
  "limit": 5
}
```
**Expect**: Returns related entities found in stored memories (Redis, Nginx, etc.).

### 8.5 Knowledge Validation

**Action**: `validate_knowledge`
```json
{
  "statement": "PostgreSQL is a relational database",
  "limit": 5
}
```
**Expect**: Returns validation result with supporting evidence from stored memories.

### 8.6 Entity Graph

**Action**: `get_entity_graph`
```json
{
  "entity": "PostgreSQL",
  "depth": 2,
  "limit": 10
}
```
**Expect**: Returns entity relationship graph centered on PostgreSQL.

---

## Phase 9: Topic, Session, and Temporal Subsystems

### 9.1 Topic Detection

**Action**: `detect_topics`
```json
{
  "content": "The database uses write-ahead logging for crash recovery. Transaction isolation levels prevent dirty reads.",
  "limit": 5
}
```
**Expect**: Detects database/storage-related topics.

### 9.2 Topic Portfolio

**Action**: `get_topic_portfolio`
```json
{}
```
**Expect**: Returns current topic distribution across stored memories.
Each topic has a weight reflecting its prevalence.

### 9.3 Topic Stability

**Action**: `get_topic_stability`
```json
{}
```
**Expect**: Returns stability metrics (churn rate, drift detection).
With few memories, stability should be high (low churn).

### 9.4 Divergence Alerts

**Action**: `get_divergence_alerts`
```json
{}
```
**Expect**: Returns any topic drift alerts. With consistent test data, should
return few or no alerts.

### 9.5 Conversation Context

**Action**: `get_conversation_context`
```json
{
  "limit": 10
}
```
**Expect**: Returns recent memories in conversation order (E4 positional encoding).

### 9.6 Session Timeline

**Action**: `get_session_timeline`
```json
{
  "limit": 10
}
```
**Expect**: Returns memories ordered by creation timestamp within current session.

### 9.7 Memory Chain Traversal

**Action**: `traverse_memory_chain`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "direction": "forward",
  "limit": 5
}
```
**Expect**: Returns chain of memories following temporal/causal links forward.

### 9.8 Session State Comparison

**Action**: `compare_session_states`
```json
{
  "session_id_a": "<current_session>",
  "session_id_b": "<previous_session_or_same>"
}
```
**Expect**: Returns topic/content comparison between sessions.

---

## Phase 10: Provenance, Audit Trail, and Curation

### 10.1 Audit Trail

**Action**: `get_audit_trail`
```json
{
  "target_id": "<memory_id_from_2.1>",
  "limit": 10
}
```
**Expect**:
- At least 1 audit record: `MemoryCreated`
- If searched: also `SearchPerformed`
- Each record has `timestamp`, `operation`, `operator`

### 10.2 Provenance Chain

**Action**: `get_provenance_chain`
```json
{
  "memory_id": "<memory_id_from_2.1>"
}
```
**Expect**:
- Full chain: source metadata, creation timestamp, audit trail
- `source_type` present
- Chain is consistent (no orphaned references)

### 10.3 Merge History

**Action**: `get_merge_history`
```json
{
  "memory_id": "<memory_id_from_2.1>"
}
```
**Expect**: Empty history (memory hasn't been merged). No errors.

### 10.4 Importance Boost

**Action**: `boost_importance`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "boost": 0.1
}
```
**Expect**:
- Returns new importance value (old + 0.1)
- Clamped to [0.0, 1.0] range
- Verify: importance never exceeds 1.0

### 10.5 Importance Clamping (Edge Case)

**Action**: `boost_importance`
```json
{
  "memory_id": "<memory_id_from_2.1>",
  "boost": 5.0
}
```
**Expect**: Importance clamped at 1.0 (not 5.7 or whatever unclamped would be).

### 10.6 Merge Concepts

**Action**: `merge_concepts`
```json
{
  "source_id": "<memory_id_from_2.2>",
  "target_id": "<memory_id_from_2.1>",
  "dry_run": true
}
```
**Expect**:
- Dry-run reports what would be merged without executing
- No data modified

### 10.7 Forget Concept (Soft Delete)

**Action**: `forget_concept`
```json
{
  "memory_id": "<memory_id_from_2.2>",
  "reason": "Test verification - removing test data"
}
```
**Expect**:
- Memory marked as deleted (soft delete)
- Subsequent searches should not return this memory
- `get_audit_trail` shows `MemoryDeleted` record with reason

> **Note**: Store a replacement memory after this test to maintain test data
> for remaining phases.

### 10.8 Consolidation Trigger

**Action**: `trigger_consolidation`
```json
{}
```
**Expect**: Returns consolidation status. With few memories, may report
"no candidates for consolidation" — this is normal.

---

## Phase 11: Embedder-First Advanced Tools

### 11.1 Embedder Clusters

**Action**: `get_embedder_clusters`
```json
{
  "embedder": "E1_Semantic",
  "num_clusters": 3,
  "limit": 10
}
```
**Expect**: Returns cluster assignments for stored memories based on E1 vectors.

### 11.2 Compare Embedder Views

**Action**: `compare_embedder_views`
```json
{
  "query": "database storage",
  "embedders": ["E1_Semantic", "E7_Code", "E11_Entity"],
  "limit": 5
}
```
**Expect**: Returns side-by-side ranking from each embedder, showing how
different embedding spaces interpret the same query differently.

### 11.3 List Embedder Indexes

**Action**: `list_embedder_indexes`
```json
{}
```
**Expect**: Lists all 13 embedder indexes with vector counts and dimensions.

### 11.4 Memory Fingerprint

**Action**: `get_memory_fingerprint`
```json
{
  "memory_id": "<memory_id_from_2.1>"
}
```
**Expect**: Returns full 13-embedder fingerprint with per-embedder vector
dimensions and metadata. ~63KB total.

### 11.5 Cross-Embedder Anomalies

**Action**: `search_cross_embedder_anomalies`
```json
{
  "query": "database optimization",
  "limit": 5
}
```
**Expect**: Returns memories where embedders disagree significantly on
relevance (e.g., E1 ranks high but E7 ranks low, suggesting the memory
is semantically relevant but not code).

---

## Phase 12: File Watcher Subsystem

### 12.1 List Watched Files

**Action**: `list_watched_files`
```json
{}
```
**Expect**: Returns list of currently watched files/directories (may be empty
if no watchers configured for current session).

### 12.2 File Watcher Stats

**Action**: `get_file_watcher_stats`
```json
{}
```
**Expect**: Returns watcher statistics (events processed, files indexed, etc.).

### 12.3 File Content Deletion

**Action**: `delete_file_content`
```json
{
  "file_path": "/nonexistent/test/path.rs"
}
```
**Expect**: Returns appropriate error or "no content found" (not a panic).

### 12.4 File Reconciliation

**Action**: `reconcile_files`
```json
{}
```
**Expect**: Returns reconciliation status (files checked, discrepancies found).

---

## Phase 13: CLI Hooks (E2E)

Test all 5 hook types via the CLI binary.

### 13.1 Session Start Hook

**Action**:
```bash
echo '{"session_id":"test-verify-001","timestamp":"2026-02-15T00:00:00Z"}' | \
  ./target/release/context-graph-cli hooks session-start --stdin --format json
```
**Expect**: JSON response with `success: true`. Session registered in cache.

### 13.2 Pre-Tool Use Hook

**Action**:
```bash
echo '{"hook_type":"pre_tool_use","session_id":"test-verify-001","timestamp_ms":1739577600000,"payload":{"type":"pre_tool_use","data":{"tool_name":"store_memory","tool_input":{"content":"test"}}}}' | \
  timeout 2s ./target/release/context-graph-cli hooks pre-tool --stdin --format json
```
**Expect**: JSON response within 500ms (fast path). May include context injection.

### 13.3 Post-Tool Use Hook

**Action**:
```bash
echo '{"hook_type":"post_tool_use","session_id":"test-verify-001","timestamp_ms":1739577600000,"payload":{"type":"post_tool_use","data":{"tool_name":"store_memory","tool_result":{"memory_id":"test-id"}}}}' | \
  timeout 5s ./target/release/context-graph-cli hooks post-tool --stdin --format json
```
**Expect**: JSON response within 3000ms. State verification executed.

### 13.4 User Prompt Submit Hook

**Action**:
```bash
echo '{"hook_type":"user_prompt_submit","session_id":"test-verify-001","timestamp_ms":1739577600000,"payload":{"type":"user_prompt_submit","data":{"prompt":"What is RocksDB?"}}}' | \
  timeout 3s ./target/release/context-graph-cli hooks prompt-submit --stdin --format json
```
**Expect**: JSON response within 2000ms. May include memory context for the prompt.

### 13.5 Session End Hook

**Action**:
```bash
echo '{"hook_type":"session_end","session_id":"test-verify-001","timestamp_ms":1739577600000,"payload":{"type":"session_end","data":{"reason":"user_exit"}}}' | \
  timeout 35s ./target/release/context-graph-cli hooks session-end --stdin --format json
```
**Expect**: JSON response within 30000ms. Session data persisted.

---

## Phase 14: Error Handling and Edge Cases

### 14.1 Invalid Memory ID

**Action**: `get_memory_fingerprint`
```json
{
  "memory_id": "00000000-0000-0000-0000-000000000000"
}
```
**Expect**: Graceful error: "Memory not found" (not a panic or 500).

### 14.2 Empty Query

**Action**: `search_graph`
```json
{
  "query": "",
  "limit": 5
}
```
**Expect**: Returns error or empty results (not a panic).

### 14.3 Oversized Content

**Action**: `store_memory`
```json
{
  "content": "<10,000+ character string>",
  "rationale": "Testing content size limits"
}
```
**Expect**: Either stores successfully or returns a clear size limit error.
Content is chunked internally by `read_line_bounded` (1MB limit per line).

### 14.4 Unicode Content

**Action**: `store_memory`
```json
{
  "content": "Koenig's lemma (Konig's Lemma): Every infinite finitely-branching tree has an infinite path. Japanese: ケーニヒの補題. Chinese: 柯尼希引理. Arabic: مبرهنة كونيغ",
  "rationale": "Multi-script Unicode test"
}
```
**Expect**: Stored and searchable without corruption. `floor_char_boundary` /
`ceil_char_boundary` prevents mid-codepoint slicing.

### 14.5 Concurrent Store (Stress)

**Action**: Run 5 parallel store_memory calls simultaneously.
**Expect**: All succeed. RocksDB handles concurrent writes via WriteBatch.
No corruption or lost writes.

### 14.6 Search with Invalid Embedder

**Action**: `search_by_embedder`
```json
{
  "query": "test",
  "embedder": "E99_NonExistent",
  "limit": 5
}
```
**Expect**: Clear error message about invalid embedder name (not a panic).

### 14.7 Search with Invalid Weight Profile

**Action**: `search_graph`
```json
{
  "query": "test",
  "weight_profile": "nonexistent_profile",
  "limit": 5
}
```
**Expect**: Error message listing available profiles.

### 14.8 Negative/Zero Limit

**Action**: `search_graph`
```json
{
  "query": "test",
  "limit": 0
}
```
**Expect**: Returns empty results or auto-corrects to minimum (not a panic).

### 14.9 Importance Out of Range

**Action**: `boost_importance`
```json
{
  "memory_id": "<valid_id>",
  "boost": -999.0
}
```
**Expect**: Importance clamped to 0.0 (not negative).

---

## Phase 15: Performance Baselines

Establish timing baselines to detect future regressions.

### 15.1 Store Latency

| Metric | Target | Notes |
|--------|--------|-------|
| First store (cold) | < 10s | Model warmup included |
| Subsequent stores | < 3s | 13 embedders + RocksDB write |
| Embedding only | < 2s | GPU-accelerated |

### 15.2 Search Latency

| Strategy | Target | Notes |
|----------|--------|-------|
| e1_only | < 50ms | Single HNSW lookup |
| multi_space | < 200ms | 6 HNSW lookups + RRF fusion |
| pipeline | < 500ms | 3-stage: sparse + dense + ColBERT |
| search_by_keywords (E6) | < 30ms | Inverted index only |

### 15.3 Hook Latency

| Hook | Budget | Notes |
|------|--------|-------|
| pre_tool_use | < 500ms | Fast path, minimal work |
| post_tool_use | < 3000ms | State verification |
| session_start | < 5000ms | Includes disk check |
| user_prompt_submit | < 2000ms | Context injection |
| session_end | < 30000ms | Full persistence |

### 15.4 Memory Usage

| Metric | Target | Notes |
|--------|--------|-------|
| Server RSS at idle | < 8GB | Models loaded, no queries |
| Per-memory storage | ~63KB | SemanticFingerprint (13 vectors) |
| VRAM usage | < 6GB | Hermes-2-Pro Q5_K_M + CUDA kernels |

---

## Phase 16: Data Integrity Verification

### 16.1 Roundtrip Consistency

1. Store a memory
2. Retrieve its fingerprint
3. Verify all 13 embedder vectors are present
4. Verify dimensions match spec (E1=1024, E5=768, E7=1536, etc.)
5. Search for the memory by its own content
6. Verify it ranks #1 with similarity > 0.95

### 16.2 Persistence Across Restart

1. Store 3 memories
2. Restart the MCP server
3. Search for all 3 memories
4. Verify all found with correct content and similarity scores
5. Verify audit trail survives restart
6. Verify soft-deleted memories remain hidden after restart

### 16.3 Column Family Integrity

**Action**:
```bash
cargo test -p context-graph-storage -- --nocapture 2>&1 | tail -10
```
**Expect**: All storage integration tests pass. RocksDB opens all 51 CFs.

### 16.4 HNSW Index Consistency

1. Store 10+ memories
2. Search using e1_only strategy
3. Verify HNSW returns correct nearest neighbors
4. Run `trigger_consolidation` to trigger compaction
5. Re-search and verify results are identical

---

## Automated Test Suites

For regression testing, the following automated suites exist:

```bash
# Full workspace (8,015 tests, ~2 min)
cargo test --workspace

# Core library (2,527 tests, ~6s)
cargo test -p context-graph-core

# MCP server (1,678 tests, ~92s)
cargo test -p context-graph-mcp

# Storage layer (224 tests, ~1s)
cargo test -p context-graph-storage

# Embeddings (105 tests, ~1s)
cargo test -p context-graph-embeddings

# CLI E2E hooks (35 tests, ~2s)
cargo test -p context-graph-cli --test e2e

# Graph agent (9 tests, ~1s)
cargo test -p context-graph-graph-agent
```

---

## Quick Smoke Test (5-Minute Version)

If you only have 5 minutes, run these 8 tests in order:

1. `get_memetic_status` — Server alive, 13 embedders loaded
2. `store_memory` — Core write works
3. `search_graph` — Core read works (multi_space strategy)
4. `search_code` — E7 code embedder works
5. `search_causes` — E5 causal direction works
6. `extract_entities` — E11 entity extraction works
7. `get_audit_trail` — Provenance tracking works
8. `boost_importance` — Curation works

If all 8 pass, the core system is operational.

---

## Failure Recovery Procedures

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| Server won't start | Missing CF in existing DB | Delete `data/` dir, restart |
| All searches return 0.0 | Embedder model not loaded | Check `models/` directory |
| E5 always returns 0.0 | Missing CausalDirection | Set `direction: "cause"` in query |
| LLM tools return errors | Hermes model not found | Check `models/hermes-2-pro/` |
| Store takes > 30s | GPU OOM | Check `nvidia-smi`, reduce batch size |
| Hook times out | Disk at > 85% | Run `./scripts/clean-build-artifacts.sh` |
| Tests fail with DB error | Stale test DB | `rm -rf /tmp/context_graph_test_*` |
| Importance exceeds 1.0 | Clamping bypassed | Check handler-level validation |
| Search returns deleted memory | Soft-delete not persisted | Check CF_SYSTEM prefix keys |

---

## Checklist Summary

| Phase | Tests | Status |
|-------|-------|--------|
| 1. Infrastructure Health | 3 | [ ] |
| 2. Core CRUD | 7 | [ ] |
| 3. Search Strategies | 4 | [ ] |
| 4. Individual Embedders | 13 | [ ] |
| 5. Weight Profiles | 6 | [ ] |
| 6. Causal Subsystem | 4 | [ ] |
| 7. Graph Subsystem | 7 | [ ] |
| 8. Entity Subsystem | 6 | [ ] |
| 9. Topics/Sessions/Temporal | 8 | [ ] |
| 10. Provenance/Curation | 8 | [ ] |
| 11. Embedder-First Advanced | 5 | [ ] |
| 12. File Watcher | 4 | [ ] |
| 13. CLI Hooks (E2E) | 5 | [ ] |
| 14. Error Handling/Edge Cases | 9 | [ ] |
| 15. Performance Baselines | 4 | [ ] |
| 16. Data Integrity | 4 | [ ] |
| **Total** | **97** | |
