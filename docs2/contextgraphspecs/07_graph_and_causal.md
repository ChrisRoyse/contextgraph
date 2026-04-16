# Graph and Causal Subsystem

Source files covered:

- `crates/context-graph-core/src/graph/mod.rs`, `asymmetric.rs`
- `crates/context-graph-core/src/graph_linking/mod.rs`, `edge_type.rs`, `direction.rs`, `typed_edge.rs`, `embedder_edge.rs`, `knn_graph.rs`, `nn_descent.rs`, `edge_builder.rs`, `service.rs`, `storage_keys.rs`, `thresholds.rs`, `weight_projector.rs`
- `crates/context-graph-core/src/causal/mod.rs`, `asymmetric.rs`, `chain.rs`, `inference.rs`, `scm.rs`
- `crates/context-graph-core/src/entity/mod.rs`
- `crates/context-graph-causal-agent/src/lib.rs`, `llm/mod.rs`, `llm/prompt.rs`, `scanner/mod.rs`, `activator/mod.rs`, `types/mod.rs`
- `crates/context-graph-graph-agent/src/lib.rs`, `llm/prompt.rs`, `types/mod.rs`, `stubs.rs`
- `crates/context-graph-storage/src/graph_edges/mod.rs`, `repository.rs`, `serialization.rs`, `types.rs`
- `crates/context-graph-mcp/src/handlers/tools/graph_link_tools.rs`, `graph_tools.rs`

---

## Table of Contents

1. [Graph Data Model](#1-graph-data-model)
2. [Graph Edge Storage](#2-graph-edge-storage)
3. [K-NN Graph](#3-k-nn-graph)
4. [Graph Traversal Algorithms](#4-graph-traversal-algorithms)
5. [Graph Linking](#5-graph-linking)
6. [Causal Discovery Agent](#6-causal-discovery-agent)
7. [CausalGraph / SCM](#7-causalgraph--scm)
8. [Causal Chain Inference](#8-causal-chain-inference)
9. [Causal Direction: Asymmetric Scoring](#9-causal-direction-asymmetric-scoring)
10. [Graph Agent](#10-graph-agent)
11. [Entity Extraction and Linking](#11-entity-extraction-and-linking)
12. [Graph-Based Scoring in Search](#12-graph-based-scoring-in-search)

---

## 1. Graph Data Model

### 1.1 Nodes

Every node in the graph is a **memory** -- the fundamental unit stored in the teleological store. Each memory has a UUID, content text, creation timestamp, a `SemanticFingerprint` containing vectors from all 13 embedders, importance score, and optional session metadata. Memories serve as nodes in both the K-NN graph and the CausalGraph.

### 1.2 Edge Types

The system maintains two distinct classes of edges:

**K-NN Embedder Edges** (`EmbedderEdge`) -- per-embedder nearest-neighbor links. Each of the 13 embedders can produce its own set of K-NN edges connecting a memory to its nearest neighbors in that embedder's space. Fields:

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Uuid` | Source node UUID |
| `target` | `Uuid` | Target node UUID (a neighbor) |
| `embedder_id` | `u8` | Embedder index (0-12) |
| `similarity` | `f32` | Cosine similarity in [-1.0, 1.0] |
| `direction` | `DirectedRelation` | For E5/E8: Forward, Backward, or Symmetric |

Validation constraints enforced at construction:
- `embedder_id` must be 0-12
- `similarity` must be in [-1.0, 1.0]
- Embedders E5 (id=4) and E8 (id=7) are asymmetric: calling `EmbedderEdge::new()` on them returns `Err(SymmetricCosineViolation)`. They require `EmbedderEdge::with_direction()` (per AP-77).
- Temporal embedders E2-E4 (ids 1-3) are flagged via `is_temporal()` and excluded from edge type detection (per AP-60).

**Multi-Relation Typed Edges** (`TypedEdge`) -- classified edges derived from embedder agreement patterns. Fields:

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Uuid` | Source node UUID |
| `target` | `Uuid` | Target node UUID |
| `edge_type` | `GraphLinkEdgeType` | One of 8 types (see below) |
| `weight` | `f32` | Computed weight in [0.0, 1.0] |
| `direction` | `DirectedRelation` | Symmetric, Forward, or Backward |
| `embedder_scores` | `[f32; 13]` | Per-embedder similarity scores |
| `agreement_count` | `u8` | Number of embedders above threshold |
| `agreeing_embedders` | `u16` | Bitset (bit 0 = E1, bit 12 = E13) |

### 1.3 Graph Link Edge Types

Eight edge types are derived from which embedders agree two nodes are similar. Defined in `GraphLinkEdgeType` (`repr(u8)`):

| Value | Type | Primary Embedder | Default Threshold | Asymmetric? | Description |
|-------|------|-----------------|-------------------|-------------|-------------|
| 0 | `SemanticSimilar` | E1 (idx 0) | 0.75 | No | High semantic similarity |
| 1 | `CodeRelated` | E7 (idx 6) | 0.70 | No | Code/implementation relationship |
| 2 | `EntityShared` | E11 (idx 10) | 0.65 | No | Shared entities (KEPLER) |
| 3 | `CausalChain` | E5 (idx 4) | 0.60 | Yes | Causal relationship (cause/effect) |
| 4 | `GraphConnected` | E8 (idx 7) | 0.60 | Yes | Graph connectivity (source/target) |
| 5 | `ParaphraseAligned` | E10 (idx 9) | 0.70 | No | Same meaning, different words |
| 6 | `KeywordOverlap` | E6/E13 (idx 5/12) | 0.50 | No | Keyword/lexical similarity |
| 7 | `MultiAgreement` | 3+ embedders | 0.60 | No | Multiple embedders agree |

### 1.4 Directed Relations

The `DirectedRelation` enum (`repr(u8)`) models edge directionality:

| Value | Variant | Modifier | Usage |
|-------|---------|----------|-------|
| 0 | `Symmetric` | 1.0x | All embedders except E5, E8 |
| 1 | `Forward` | 1.2x | E5: cause->effect; E8: source->target |
| 2 | `Backward` | 0.8x | E5: effect->cause; E8: target->source |

`reverse()` swaps Forward/Backward while leaving Symmetric unchanged.

### 1.5 Edge Type Auto-Detection

`TypedEdge::from_scores()` auto-detects the edge type from per-embedder similarity scores:

1. Compute which embedders exceed their threshold (excluding temporal E2-E4 per AP-60).
2. If 3+ embedders agree, classify as `MultiAgreement`.
3. Otherwise, check specialized embedders in priority order: E7 (Code) > E11 (Entity) > E5 (Causal) > E8 (Graph) > E10 (Paraphrase) > E6/E13 (Keyword) > E1 (Semantic).
4. Weight is the primary embedder's score (or average of all agreeing embedders for `MultiAgreement`).
5. For asymmetric types (`CausalChain`, `GraphConnected`), if no explicit direction is provided, default to `Forward`.

---

## 2. Graph Edge Storage

Edges are persisted in RocksDB across three column families, managed by `EdgeRepository` in `crates/context-graph-storage/src/graph_edges/repository.rs`.

### 2.1 Column Families

| Column Family | Key Format | Key Size | Value |
|--------------|------------|----------|-------|
| `embedder_edges` | `[embedder_id: u8][source_uuid: 16B]` | 17 bytes | Serialized `Vec<EmbedderEdge>` |
| `typed_edges` | `[source_uuid: 16B][target_uuid: 16B]` | 32 bytes | Serialized `TypedEdge` |
| `typed_edges_by_type` | `[edge_type: u8][source_uuid: 16B][target_uuid: 16B]` | 33 bytes | Target UUID (16 bytes) |

**Key design rationale:**
- `embedder_edges` sorts by embedder first, then by source UUID, enabling efficient prefix scans for all neighbors of a node within an embedder.
- `typed_edges` sorts by source then target, enabling prefix scans for all edges from a source node.
- `typed_edges_by_type` is a secondary index enabling queries like "all CausalChain edges from node X." The key includes the target UUID (33 bytes total) to prevent overwrites when the same source has multiple edges of the same type to different targets (BUG-3 fix).

### 2.2 Serialization Format

**Embedder edges** use a compact binary format:
```
[version: u8][count: u32][edges...]
```
Each edge within the list is stored as `[target_uuid: 16B][similarity: f32]` = 20 bytes per edge. The source UUID and embedder ID are derived from the key (not stored in the value).

**Typed edges** use bincode serialization with a version prefix byte (`GRAPH_EDGE_VERSION = 1`).

### 2.3 EdgeRepository API

Key operations on `EdgeRepository`:

| Method | Description |
|--------|-------------|
| `store_embedder_edges(embedder_id, source, &[EmbedderEdge])` | Replaces all K-NN edges for a (embedder, source) pair (repository.rs:80) |
| `get_embedder_edges(embedder_id, source)` | Returns K-NN neighbors (empty vec if none) (repository.rs:114) |
| `store_typed_edge(&TypedEdge)` | Stores a typed edge and updates secondary index (repository.rs:215) |
| `get_typed_edges_from(source)` | All outgoing typed edges from a source node (repository.rs:312) |
| `get_typed_edges_to(target)` | All incoming typed edges pointing at a target node (invoked by the `get_typed_edges` handler for the `incoming` direction per graph_link_tools.rs:390,410,449,473 — BUG-2 fix uses `edge.source()` as the peer UUID) |
| `get_typed_edges_by_type(edge_type, source, direction)` | Type-filtered query using secondary index; direction-aware branching (BUG-1 fix) (repository.rs:342) |
| `delete_typed_edge(source, target)` | Removes a typed edge and its secondary-index entries (repository.rs:276) |
| `store_typed_edges_batch(&[TypedEdge])` | Batch insert via `WriteBatch` (repository.rs:458) |
| `is_empty()` | Whether any K-NN edges exist (repository.rs:145) |

All methods return `GraphEdgeStorageResult<T>` and follow a fail-fast policy -- no fallbacks, no mock data.

---

## 3. K-NN Graph

### 3.1 Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `KNN_K` | 20 | Neighbors per node |
| `NN_DESCENT_ITERATIONS` | 8 | Max NN-Descent iterations |
| `NN_DESCENT_SAMPLE_RATE` | 0.5 | Neighbor sampling rate (rho) |
| `MIN_KNN_SIMILARITY` | 0.3 | Minimum similarity for edges |

### 3.2 KnnGraph In-Memory Structure

`KnnGraph` stores an adjacency map (`HashMap<Uuid, Vec<EmbedderEdge>>`) for a single embedder. Key behaviors:

1. **Add edge:** If the node has fewer than `k` neighbors, the edge is appended. If at capacity, the new edge replaces the lowest-similarity neighbor only if it has higher similarity.
2. **Duplicate handling:** If the target already exists in the neighbor list, the edge is updated only if the new similarity is higher.
3. **Sorted retrieval:** `get_neighbors_sorted()` returns neighbors in descending similarity order.
4. **Node removal:** `remove_node()` removes the node and all its outgoing edges.

Statistics tracked: node count, edge count, min/max/avg neighbors per node, min/max/avg similarity.

### 3.3 NN-Descent Algorithm

The K-NN graph is constructed using the NN-Descent algorithm (`nn_descent.rs`), based on the principle "a neighbor of my neighbor is likely my neighbor."

**Algorithm steps:**

1. **Initialization:** Assign random neighbors to each node with similarity 0.0.
2. **Iterative improvement** (up to `NN_DESCENT_ITERATIONS` iterations):
   a. For each node, sample `k * sample_rate` current neighbors.
   b. Collect candidates from those neighbors' own neighbor lists (neighbors-of-neighbors).
   c. Add random candidates for exploration (`sample_size / 2` additional random nodes).
   d. Compute similarity between the node and each candidate.
   e. If similarity >= `MIN_KNN_SIMILARITY`, attempt to insert into the neighbor list (replacing the lowest-similarity neighbor if the new one is better).
   f. Recompute similarity for existing neighbors whose similarity is still 0.0 (from initialization).
3. **Early termination:** If `update_rate < 0.001` (fewer than 0.1% of edges updated), stop iterating.
4. **Small graph fallback:** For graphs with <= `2 * k` nodes, use brute-force O(n^2) comparison instead of NN-Descent.

**Complexity:** O(n * k * iterations) versus naive O(n^2).

**Configuration** (`NnDescentConfig`):

| Field | Default | Description |
|-------|---------|-------------|
| `k` | 20 | Neighbors per node |
| `iterations` | 8 | Max iterations |
| `sample_rate` | 0.5 | Fraction of neighbors to sample |
| `min_similarity` | 0.3 | Minimum edge similarity |
| `early_termination_threshold` | 0.001 | Stop if update rate < this |
| `seed` | None | Optional random seed |

### 3.4 Asymmetric K-NN Construction

For asymmetric embedders (E5 causal, E8 graph), the dedicated function `build_asymmetric_knn()` is used instead of standard NN-Descent. This function:

1. Caches both source embeddings and target embeddings for all nodes.
2. For each node `i`, computes `similarity(source_embedding_i, target_embedding_j)` for all other nodes `j`.
3. Creates directed edges with `DirectedRelation::Forward` for all edges meeting the similarity threshold.

The key difference: standard NN-Descent uses `similarity(A, B)` symmetrically, while asymmetric K-NN computes `similarity(A_as_source, B_as_target)`, capturing the directional nature of causal/graph relationships.

---

## 4. Graph Traversal Algorithms

### 4.1 BFS Path Finding (CausalGraph)

`CausalGraph::find_path(source, target)` implements BFS:

1. Initialize a queue with `(source, [source])` and a visited set.
2. Dequeue `(current, path)`. For each direct effect of `current`:
   a. If the effect is the target, return `path ++ [target]`.
   b. Otherwise, if not visited, enqueue `(effect, path ++ [effect])` and mark as visited.
3. If the queue is exhausted, return `None`.

### 4.2 K-NN Neighbor Retrieval

The MCP tool `get_memory_neighbors` retrieves K-NN neighbors:

1. Validate request and verify the memory exists.
2. Query `EdgeRepository.get_embedder_edges(embedder_id, memory_id)` for K-NN neighbors.
3. Apply `min_similarity` filter.
4. Optionally hydrate content and source metadata.
5. Return sorted by similarity descending.

### 4.3 Typed Edge Queries

The MCP tool `get_typed_edges` queries typed edges:

1. Query `EdgeRepository.get_typed_edges(source_id)` or `get_typed_edges_by_type(type, source, direction)`.
2. Direction-aware branching (BUG-1 fix): For incoming edges, the repository scans by target instead of source, and returns the peer node's UUID (using `edge.source()`) as `target_id` (BUG-2 fix).
3. Filter by optional `edge_type` and `direction`.
4. Return with agreement summary (which embedders agreed, at what scores).

### 4.4 Multi-Hop Graph Traversal

The MCP tool `traverse_graph` implements multi-hop BFS over typed edges:

1. Start from a source memory with a specified `edge_type` filter.
2. Use a BFS queue bounded by `max_depth` (default 3).
3. At each hop, follow typed edges matching the filter.
4. Track the path and accumulated score.
5. Return all reachable nodes with their traversal paths.

---

## 5. Graph Linking

### 5.1 GraphLinkService Architecture

```
+------------------------------------------------------------+
|                   GraphLinkService                          |
+------------------------------------------------------------+
|  NnDescent      |  EdgeBuilder      |  KnnGraph            |
|  (Algorithm)    |  (Typed Edges)    |  (In-Memory)         |
+-----------------+-------------------+----------------------+
                  |
         +--------v--------+
         |  EdgeRepository  |
         |  (RocksDB)       |
         +-----------------+
```

**Active embedders** for K-NN graph construction (default): E1 (0), E5 (4), E7 (6), E8 (7), E10 (9), E11 (10). Temporal embedders (E2-E4) are excluded.

### 5.2 EdgeBuilder: Deriving Typed Edges from K-NN Graphs

The `EdgeBuilder` analyzes multiple K-NN graphs to produce typed edges. The algorithm:

1. For each memory pair that appears in any K-NN graph, collect similarity scores from all embedders.
2. Determine which embedders agree (score >= per-embedder threshold). Temporal embedders (E2-E4) are excluded per AP-60.
3. Compute edge type based on agreement pattern (using the auto-detection logic described in Section 1.5).
4. Compute edge weight using either:
   - **Constitution-based:** Primary embedder score for single-embedder types; average of all agreeing scores for `MultiAgreement`.
   - **Learned projection:** If a `WeightProjector` is configured, project the 13-dimensional score vector to a single weight via a trained neural network.
5. Create a `TypedEdge` if the weighted agreement meets the threshold.

**Per-embedder agreement thresholds** (`DEFAULT_EMBEDDER_THRESHOLDS`):

| Embedder | Index | Threshold |
|----------|-------|-----------|
| E1 Semantic | 0 | 0.75 |
| E2 Recency | 1 | 0.70 |
| E3 Periodic | 2 | 0.70 |
| E4 Sequence | 3 | 0.70 |
| E5 Causal | 4 | 0.60 |
| E6 Sparse | 5 | 0.50 |
| E7 Code | 6 | 0.70 |
| E8 Graph | 7 | 0.60 |
| E9 HDC | 8 | 0.60 |
| E10 Paraphrase | 9 | 0.70 |
| E11 Entity | 10 | 0.65 |
| E12 ColBERT | 11 | 0.60 |
| E13 SPLADE | 12 | 0.50 |

Minimum weighted agreement for typed edge creation: **2.5** (per ARCH-09, same as topic threshold).

### 5.3 WeightProjector Trait

The `WeightProjector` trait allows plugging in a learned model for edge weight computation:

```rust
pub trait WeightProjector: Send + Sync {
    fn project(&self, scores: &[f32; 13]) -> f32;
    fn project_batch(&self, batch_scores: &[[f32; 13]]) -> Vec<f32>;
    fn is_learned(&self) -> bool;
}
```

When no projector is configured, the system falls back to constitution-based weighted agreement.

---

## 6. Causal Discovery Agent

### 6.1 Architecture

```
+------------------------------------------------------------+
|                 CAUSAL DISCOVERY AGENT                       |
+------------------------------------------------------------+
| Memory Scanner -> LLM Analysis (GBNF) -> E5 Activator      |
+------------------------------------------------------------+
```

The causal discovery agent lives in `crates/context-graph-causal-agent/` and consists of four components:

1. **Memory Scanner** -- finds candidate memory pairs for causal analysis.
2. **LLM Module** -- analyzes pairs using Hermes-2-Pro-Mistral-7B with grammar-constrained JSON output.
3. **E5 Embedder Activator** -- generates asymmetric E5 embeddings for confirmed relationships and updates the CausalGraph.
4. **Service Module** -- background service for scheduled discovery cycles.

### 6.2 LLM Configuration

| Setting | Value | Description |
|---------|-------|-------------|
| Model | Hermes-2-Pro-Mistral-7B Q5_K_M | GGUF quantized |
| VRAM | ~5GB model + ~1GB KV cache = ~6GB | Within 9GB budget |
| Context size | 4096 tokens | Default |
| Temperature | 0.0 | Deterministic for analysis |
| Max tokens | 512 | Supports multi-paragraph responses |
| GPU layers | All (u32::MAX) | Full CUDA offload |
| Batch size | 2048 | Accommodates few-shot prompts |
| Few-shot | Enabled by default | Improves direction detection |
| JSON output | GBNF grammar-constrained | 100% valid JSON (vs ~40% with previous Candle/Qwen) |

The LLM is shared via `Arc` between the causal agent and graph agent to avoid duplicate VRAM usage.

### 6.3 Memory Scanner

The scanner (`scanner/mod.rs`) finds candidate memory pairs using heuristics:

**Algorithm:**

1. **Cluster by E1 similarity:** Greedy clustering with configurable threshold (default: 0.5-0.9 band).
2. **Pair within clusters:** Compare all pairs within each cluster.
3. **Temporal ordering:** Earlier memory is the potential cause, later is the potential effect.
4. **Time gap constraints:** Minimum gap 10 seconds (avoid same-event), maximum gap 24 hours.
5. **Score candidates** using:
   - Causal marker count in text: +0.15 per marker found
   - Temporal ordering bonus: +0.1
   - Same session bonus: +0.1
   - Content length bonus: +0.05 if >100 chars, +0.05 if >300 chars
   - Semantic similarity in sweet spot (0.5-0.8): +0.1
6. **Filter:** Keep candidates with score >= 0.3 (default `min_initial_score`).
7. **Sort and truncate:** Top 100 candidates by score.

Pairs are NOT marked as analyzed until after successful LLM confirmation (AGT-01 fix).

**Causal markers** scanned in text:

| Cause Markers | Effect Markers |
|--------------|----------------|
| because, due to, caused by, result of, led to, since, as a result of, owing to, thanks to, on account of, stems from, arises from, originates from, triggered by | therefore, consequently, as a result, thus, hence, so, accordingly, resulting in, leading to, causing, produces, yields, generates, brings about |

### 6.4 LLM Prompt Templates

All prompts use the ChatML format (`<|im_start|>system ... <|im_end|>`).

**Pair Analysis Prompt** (with few-shot examples):

The system prompt instructs the model to analyze causal relationships and output JSON:

```json
{
  "causal_link": true/false,
  "direction": "A_causes_B" | "B_causes_A" | "bidirectional" | "none",
  "confidence": 0.0-1.0,
  "mechanism": "Specific causal mechanism (1-2 sentences)",
  "mechanism_type": "direct" | "mediated" | "feedback" | "temporal"
}
```

The prompt includes:
- Direction decision rules (A produces B's outcome? B produces A's outcome?)
- Confidence calibration scale (0.9-1.0: established mechanism, 0.0-0.2: no causal link)
- Explicit warning: "Correlation, semantic similarity, or topical overlap are NOT causation."
- Five few-shot examples covering: mediated causation, reverse direction, multi-hop, feedback loops, and no relationship.

Content is sanitized before embedding in prompts: double quotes are escaped, and ChatML markers (`<|im_start|>`, `<|im_end|>`) are neutralized to prevent prompt injection (CAUSAL-M1 fix). Content is truncated at word boundaries using `floor_char_boundary()` for UTF-8 safety.

**Single-Text Analysis Prompt** (V3 Structured Criteria):

Used during memory storage to hint the E5 embedder. Follows a 6-step decision procedure:

1. Scan for causal markers (forward: "causes", "leads to"; backward: "caused by", "due to").
2. Determine direction from marker type.
3. Calibrate confidence.
4. Extract key phrases.
5. Write mechanism description.
6. Set asymmetry strength and entity spans.

Benchmarked at 73.3% direction accuracy, 559ms avg latency, 0.80 confidence separation.

**Multi-Relationship Extraction Prompt:**

Extracts ALL cause-effect relationships from a single text. Each relationship includes:
- Cause/effect statements
- 1-2 paragraph explanation (critical for E5 embedding quality)
- Confidence score
- Mechanism type
- Source spans with exact character offsets for provenance

The prompt emphasizes that `text_excerpt` must be an exact character-for-character copy from the input (not paraphrased).

### 6.5 E5 Embedder Activator

The `E5EmbedderActivator` bridges LLM analysis to storage:

**Activation flow:**

1. Check confidence against threshold (default: 0.6). Skip if below.
2. Check if relationship already exists in `CausalGraph` (skip if `update_existing` is false).
3. Generate E5 asymmetric embeddings using `CausalModel.embed_dual_guided()`:
   - `ACausesB`: embed A as cause, B as effect.
   - `BCausesA`: embed B as cause, A as effect.
   - `Bidirectional`: embed both A and B as both cause and effect (4 vectors total).
4. Construct `CausalHintGuidance` from the analysis result (key phrases from mechanism text, asymmetry strength from confidence).
5. Add nodes to `CausalGraph` if not present (`auto_create_nodes` enabled by default).
6. Add directed edge(s) to `CausalGraph`:
   - Unidirectional: single edge with full confidence.
   - Bidirectional: two edges at `confidence * 0.8` each.

Without a `CausalModel`, the activator fails fast -- no mock embeddings are generated.

---

## 7. CausalGraph / SCM

### 7.1 Structural Causal Model

The `CausalGraph` (`causal/scm.rs`) is a directed graph representing causal relationships:

**Node** (`CausalNode`):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique identifier |
| `name` | `String` | Human-readable name |
| `domain` | `String` | Domain category (e.g., "physics", "memory") |

**Edge** (`CausalEdge`):

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Uuid` | Cause node UUID |
| `target` | `Uuid` | Effect node UUID |
| `strength` | `f32` | Causal strength [0, 1] (clamped at construction) |
| `mechanism` | `String` | Description of the causal mechanism |
| `cause_embedding` | `Option<Vec<f32>>` | 768D E5 cause vector |
| `effect_embedding` | `Option<Vec<f32>>` | 768D E5 effect vector |
| `is_bidirectional` | `bool` | Whether this is a feedback loop |
| `reverse_embeddings` | `Option<(Vec<f32>, Vec<f32>)>` | Secondary cause/effect for bidirectional |
| `llm_confidence` | `f32` | LLM's original confidence [0, 1] |
| `mechanism_type` | `Option<String>` | "direct", "mediated", "feedback", "temporal" |
| `embedding_hint_provenance` | `Option<EmbeddingHintProvenance>` | LLM guidance provenance |

Edges are classified as:
- **Strong** if `strength >= 0.7`
- **Weak** if `strength < 0.3`

### 7.2 Capacity Limits

| Limit | Value | Description |
|-------|-------|-------------|
| `MAX_CAUSAL_NODES` | 500,000 | Maximum nodes in graph |
| `MAX_CAUSAL_EDGES` | 2,000,000 | Maximum edges in graph |

`add_node()` and `add_edge()` return `false` when the respective limit is reached.

### 7.3 Internal Indexing

The CausalGraph maintains two HashMap indexes for O(1) lookups:

- `effects_index: HashMap<Uuid, Vec<usize>>` -- maps a node to indices of its outgoing edges (effects).
- `causes_index: HashMap<Uuid, Vec<usize>>` -- maps a node to indices of its incoming edges (causes).

Both indexes are rebuilt after edge removal to maintain consistency.

### 7.4 CausalGraph API

| Method | Description |
|--------|-------------|
| `add_node(CausalNode) -> bool` | Add a node (returns false if at capacity) |
| `add_edge(CausalEdge) -> bool` | Add a directed edge (returns false if at capacity) |
| `remove_node(Uuid)` | Remove node and all connected edges |
| `remove_edge(source, target)` | Remove a specific edge |
| `get_effects(cause_id)` | All outgoing edges from a cause |
| `get_causes(effect_id)` | All incoming edges to an effect |
| `get_direct_effects(source)` | UUIDs of direct effect nodes |
| `get_direct_causes(target)` | UUIDs of direct cause nodes |
| `has_direct_cause(source, target)` | Check if direct causal edge exists |
| `find_path(source, target)` | BFS shortest path |
| `nodes_by_domain(domain)` | Filter nodes by domain |
| `average_strength()` | Mean edge strength across graph |

---

## 8. Causal Chain Inference

### 8.1 Transitive Chain Scoring

Multi-hop causal chains (A -> B -> C -> ... -> Z) are scored with attenuation (`chain.rs`):

**Formula:**

```
chain_score = product( hop_score_i * HOP_ATTENUATION^i )
```

where `HOP_ATTENUATION = 0.9` and each hop's score is computed using asymmetric similarity.

**Constants:**

| Constant | Value | Description |
|----------|-------|-------------|
| `HOP_ATTENUATION` | 0.9 | Per-hop attenuation factor |
| `MIN_CHAIN_SCORE` | 0.1 | Minimum valid chain score |
| `MAX_CHAIN_LENGTH` | 10 | Maximum chain length (longer returns 0.0) |

**Embedding-based chain scoring** (`score_causal_chain`):

For a chain of `CausalPairEmbedding` objects [P1, P2, P3, ...]:
1. For each consecutive pair (Pi, Pi+1), compute cosine similarity between Pi's effect embedding and Pi+1's cause embedding.
2. Apply cause->effect direction boost (1.2x).
3. Average hop similarities across the chain.

**Attenuated variant** (`score_causal_chain_attenuated`):
Same as above but weights each hop by `HOP_ATTENUATION^i`, giving closer causes more weight.

### 8.2 Abductive Reasoning

Given an observed effect, `rank_causes_by_abduction()` finds the most likely cause:

1. For each candidate cause, compute:
   - E5 asymmetric similarity (effect -> cause direction).
   - E1 semantic similarity (topical discrimination).
   - Blend: **80% E1 + 20% E5** (E1 proven 17x better than E5 at topical discrimination).
2. Apply effect->cause dampening (0.8x modifier) to reflect uncertainty in backward inference.
3. Sort candidates by score descending.

**Predictive reasoning** (`rank_effects_by_prediction()`) works symmetrically:
- Blend: 80% E1 + 20% E5.
- Apply cause->effect boost (1.2x modifier).
- Clamp final score to [0.0, 1.0].

### 8.3 Deductive Chain Building

`build_causal_chain()` constructs a chain from a starting cause embedding:

1. Start with `current_effect = start_cause_embedding`.
2. For up to `MAX_CHAIN_LENGTH` iterations:
   a. Find the unvisited `CausalPairEmbedding` whose cause embedding has the highest cosine similarity to `current_effect`.
   b. If the best match exceeds `similarity_threshold`, add it to the chain, mark as visited, and set `current_effect` to this pair's effect embedding.
   c. Otherwise, stop (no valid continuation found).

### 8.4 Inference Directions

The `InferenceDirection` enum supports five modes for the `omni_infer` tool:

| Direction | Description | Requires Target? |
|-----------|-------------|-----------------|
| `Forward` | A -> B: What effect does A have on B? | Yes |
| `Backward` | B -> A: What caused B? | Yes |
| `Bidirectional` | A <-> B: Mutual influence | Yes |
| `Bridge` | Cross-domain causal relationships | No |
| `Abduction` | Best hypothesis for observation | No |

`InferenceResult` captures: direction, source/target UUIDs, strength [0,1], confidence [0,1], path through the causal graph, and human-readable explanation.

---

## 9. Causal Direction: Asymmetric Scoring

### 9.1 E5 Causal Asymmetric Similarity

Defined in `causal/asymmetric.rs`. The core formula:

```
sim = base_cos * direction_mod
```

**Direction modifiers:**

| Direction | Modifier | Constant |
|-----------|----------|----------|
| Cause -> Effect | 1.2x | `direction_mod::CAUSE_TO_EFFECT` |
| Effect -> Cause | 0.8x | `direction_mod::EFFECT_TO_CAUSE` |
| Same direction | 1.0x | `direction_mod::SAME_DIRECTION` |
| Unknown | 1.0x | `direction_mod::UNKNOWN` |

The intervention overlap factor `(0.7 + 0.3 * overlap)` from the original constitution was disabled based on benchmark analysis showing -15.9% correlation. The formula now uses `overlap_factor = 1.0`.

### 9.2 E5 Causal Content Gate

A binary boost/demotion gate applied to search results based on E5 scores. Thresholds calibrated from GPU benchmark (2026-02-14) with prefix-aligned LoRA+projection model:

| Threshold | Value | Description |
|-----------|-------|-------------|
| `CAUSAL_THRESHOLD` | 0.04 | Above this: "definitely causal" (~85% causal content captured) |
| `NON_CAUSAL_THRESHOLD` | 0.008 | Below this: "definitely non-causal" (~85% non-causal caught) |
| `CAUSAL_BOOST` | 1.10x | Applied to results above CAUSAL_THRESHOLD for causal queries |
| `NON_CAUSAL_DEMOTION` | 0.85x | Applied to results below NON_CAUSAL_THRESHOLD for causal queries |

Scores between the two thresholds are in the "ambiguous zone" and receive no adjustment.

The gate function `apply_causal_gate(original_score, e5_score, is_causal_query)`:
- If `is_causal_query` is false, return `original_score` unchanged.
- If `e5_score >= 0.04`, return `original_score * 1.10`.
- If `e5_score <= 0.008`, return `original_score * 0.85`.
- Otherwise, return `original_score` unchanged.

### 9.3 E8 Graph Asymmetric Similarity

Defined in `graph/asymmetric.rs`, paralleling E5 but for structural relationships:

```
sim = base_cos * direction_mod * (0.7 + 0.3 * connectivity_overlap)
```

**Direction modifiers** (same values as E5 for consistency):

| Direction | Modifier |
|-----------|----------|
| Source -> Target | 1.2x |
| Target -> Source | 0.8x |
| Same direction / Unknown | 1.0x |

The connectivity overlap term IS active for E8 (unlike E5 where it was disabled). It uses `ConnectivityContext` which tracks:
- Connected entities (neighbors)
- Relationship types
- Module/namespace context
- Depth in dependency graph

**Overlap formula:**
```
entity_blended = 0.5 * jaccard + 0.5 * containment
overlap = 0.5 * entity_blended + 0.3 * rel_jaccard + module_bonus + depth_bonus
```
Where `module_bonus` = 0.1 for exact match, 0.05 for prefix match; `depth_bonus` = 0.1 for same depth, 0.05 for adjacent depth. Both bonuses only apply if `base_overlap > 0.1`.

### 9.4 Causal Direction Inference from Fingerprints

`infer_direction_from_fingerprint()` determines whether a memory is more "cause-like" or "effect-like" by comparing the component variance of E5's dual vectors:

1. Get the cause and effect projections from the `SemanticFingerprint`.
2. Compute component variance for each vector: `var = (1/D) * sum((x_i - mean)^2)`.
3. Higher variance indicates a more peaked/concentrated distribution (the projection head found a more specific representation for that causal role).
4. If `cause_var > effect_var * 1.05`, classify as `Cause`.
5. If `effect_var > cause_var * 1.05`, classify as `Effect`.
6. Otherwise, classify as `Unknown`.

---

## 10. Graph Agent

### 10.1 Architecture

The graph agent (`crates/context-graph-graph-agent/`) mirrors the causal agent but focuses on structural relationships. It shares the Hermes-2-Pro-Mistral-7B LLM via `Arc`.

```
Memories -> Scanner -> Candidates -> LLM Analysis -> Activator -> Graph Edges
              |            |              |              |
          Heuristics   E1 Similarity  Hermes-2-Pro   E8 1024D
```

**E8 dimension:** 1024D (upgraded from MiniLM 384D). Uses e5-large-v2, sharing the model with E1 for zero additional VRAM.

### 10.2 Content Domains

| Domain | Description |
|--------|-------------|
| `Code` | Programming code, APIs, software documentation |
| `Legal` | Cases, statutes, contracts, regulations |
| `Academic` | Research papers, studies, citations |
| `General` | Other content |

Domain detection uses heuristic markers (`DomainMarkers::detect_domain_pair()`).

### 10.3 Relationship Types

The graph agent detects 20 relationship types (19 + None) across 6 categories:

| Category | Relationship Types |
|----------|-------------------|
| **Containment** | `contains`, `scoped_by` |
| **Dependency** | `depends_on`, `imports`, `requires` |
| **Reference** | `references`, `cites`, `interprets`, `distinguishes` |
| **Implementation** | `implements`, `complies_with`, `fulfills` |
| **Extension** | `extends`, `modifies`, `supersedes`, `overrules` |
| **Invocation** | `calls`, `applies`, `used_by` |

### 10.4 LLM Prompt for Graph Relationships

The graph agent prompt instructs the LLM to:

1. Look for EXPLICIT structural links, not semantic similarity.
2. Output "none" when contents are unrelated.
3. Choose the most specific relationship type from the domain-specific list.
4. Detect domain automatically and provide domain-aware relationship hints.

**Output JSON format:**
```json
{
  "has_connection": true/false,
  "direction": "a_to_b" | "b_to_a" | "bidirectional" | "none",
  "relationship_type": "imports" | "cites" | ... | "none",
  "category": "dependency" | "reference" | ...,
  "domain": "code" | "legal" | "academic" | "general",
  "confidence": 0.0-1.0,
  "description": "why this relationship exists"
}
```

### 10.5 E8 Activator

The `E8Activator` generates 1024D asymmetric E8 embeddings for confirmed structural relationships. It creates graph edges stored via the `GraphStorage` trait, with support for source/target direction and relationship type metadata.

### 10.6 Test Stubs

For tests that need a `GraphDiscoveryService` in their handler struct but do not exercise graph discovery, `create_stub_graph_discovery_service()` provides a service with an UNLOADED LLM. Any analysis calls return `Err(LlmNotInitialized)`.

---

## 11. Entity Extraction and Linking

### 11.1 KEPLER-Based Entity Detection

Entity detection is handled by E11 KEPLER embeddings (`entity/mod.rs`). KEPLER was trained on Wikidata5M (4.8M entities, 20M triples) with a TransE objective. The key insight: KEPLER embeddings ARE the entity detection -- no keyword lookup table is needed because:

1. KEPLER was trained on 4.8M entities from Wikidata.
2. TransE training encodes relationships (`h + r approx t`).
3. Embedding similarity captures entity relationships automatically.

Entity "detection" is: embed with KEPLER, search by similarity.

### 11.2 Entity Types

| Type | Description |
|------|-------------|
| `ProgrammingLanguage` | Programming languages |
| `Framework` | Frameworks and libraries |
| `Database` | Databases |
| `Cloud` | Cloud providers and services |
| `Company` | Companies and products |
| `TechnicalTerm` | Technical terms and protocols |
| `Unknown` | General entity (default) |

These types are for UI/API display only -- KEPLER does not need them for search.

### 11.3 EntityLink

An entity reference with surface form and confidence:

| Field | Type | Description |
|-------|------|-------------|
| `surface_form` | `String` | Raw entity text |
| `canonical_id` | `String` | Lowercase normalized form |
| `entity_type` | `EntityType` | Display category |
| `confidence` | `f32` | Extraction confidence [0.0, 1.0] |

Equality and hashing ignore `confidence` -- two EntityLinks to the same entity are equal regardless of confidence.

### 11.4 Entity Extraction Methods

| Method | Confidence | Description |
|--------|-----------|-------------|
| `KnowledgeBase` | Highest | Matched against known knowledge base |
| `HeuristicPattern` | Medium | Capitalization and technical patterns |
| `TransEInferred` | Medium | TransE relationship prediction |
| `LLMExtracted` | Variable | Extracted by LLM analysis |

### 11.5 Entity Jaccard Similarity

`entity_jaccard_similarity(a, b)` computes set overlap between entity collections:

```
Jaccard(A, B) = |A intersection B| / |A union B|
```

This is for string-level overlap only. KEPLER embedding similarity is the primary entity relationship measure.

### 11.6 Entity Provenance

`EntityProvenance` records link an entity back to its source memory:
- Source memory UUID
- Character spans in source text
- Extraction method
- Confidence score
- Timestamp

Stored in `CF_ENTITY_PROVENANCE` for full audit trail.

---

## 12. Graph-Based Scoring in Search

### 12.1 E8 Connection Search

The `search_connections` MCP tool uses E8 asymmetric similarity for graph-based search:

1. Embed the query using all 13 embedders.
2. Search using the `graph_reasoning` weight profile with 5x over-fetch.
3. For each candidate, compute E8 asymmetric similarity: `sim = base_cos * direction_mod * (0.7 + 0.3 * connectivity_overlap)`.
4. Apply direction modifier: source->target gets 1.2x, target->source gets 0.8x.
5. Filter by `minScore` and return top-K.

### 12.2 Graph Path Building

The `get_graph_path` tool builds multi-hop paths using E8 embeddings with hop attenuation. Each hop's score is attenuated, and the path's total score decreases with length.

### 12.3 LLM-Based Graph Discovery in Search

The `discover_graph_relationships` and `validate_graph_link` MCP tools use the graph agent's LLM to:
- Discover structural relationships between memories on demand.
- Validate proposed graph links before storing them.

Both require the `llm` feature flag and use the shared Hermes-2-Pro-Mistral-7B model.

### 12.4 Unified Neighbor Search

The `get_unified_neighbors` tool combines K-NN results from multiple embedders using Reciprocal Rank Fusion (RRF) to produce a single ranked neighbor list. It queries several semantic embedder indices (configurable, typically E1, E5, E7, E8, E9, E10, E11) and merges the per-embedder rankings.

---

## Cross-References

- For embedder specifications (E1-E13 dimensions, models, and training): see `05_embedding_subsystem.md`.
- For search strategies (multi_space, pipeline, embedder_first) and RRF fusion: see `06_search_and_fusion.md`.
- For MCP tool schemas and handler details: see `08_mcp_tools_reference.md`.
- For weight profiles and per-embedder weighting: see `06_search_and_fusion.md`.
- For storage architecture and column families: see `09_storage_layer.md`.
