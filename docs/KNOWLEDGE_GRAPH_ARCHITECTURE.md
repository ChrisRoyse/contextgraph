# Context Graph Knowledge Graph Architecture

## Complete Technical Reference

**Version**: 6.5.0
**Last Updated**: 2026-01-25
**Architecture**: GPU-First 13-Perspectives Multi-Space System

---

## Table of Contents

1. [Overview](#1-overview)
2. [Core Data Structures](#2-core-data-structures)
3. [The 13 Embedders](#3-the-13-embedders)
4. [Storage Architecture](#4-storage-architecture)
5. [How the Graph Grows](#5-how-the-graph-grows)
6. [How the Graph is Traversed](#6-how-the-graph-is-traversed)
7. [Multi-Space Retrieval Pipeline](#7-multi-space-retrieval-pipeline)
8. [Topic Detection & Clustering](#8-topic-detection--clustering)
9. [Session & Sequence Tracking](#9-session--sequence-tracking)
10. [AI Navigation System](#10-ai-navigation-system)
11. [Performance Characteristics](#11-performance-characteristics)
12. [Architecture Diagrams](#12-architecture-diagrams)

---

## 1. Overview

The Context Graph is a **13-perspectives knowledge graph** where every memory is viewed through 13 different embedding lenses simultaneously. Unlike traditional single-embedding systems, this architecture captures multiple dimensions of meaning that a single embedder cannot see.

### Core Philosophy

**"ALL EMBEDDERS ARE SIGNAL"** - Every embedder provides unique signal, never noise. Each captures a dimension of meaning that others miss.

| Perspective | What It Captures | What E1 (Semantic) Misses |
|-------------|------------------|---------------------------|
| E7 (Code) | Function signatures, syntax patterns | Treats code as natural language |
| E5 (Causal) | Cause→effect direction | Direction lost in averaging |
| E11 (Entity) | "Diesel" = database ORM | Entity relationships |
| E10 (Intent) | Same goal, different words | Intent alignment |

### Key Design Principles

1. **Atomic 13-Embedding Storage**: All 13 embeddings stored together or nothing (ARCH-01)
2. **No Fusion**: Embeddings kept separate, combined only at query time via Weighted RRF
3. **E1 Foundation**: All retrieval starts with E1, other embedders ENHANCE (ARCH-12)
4. **Asymmetric Similarity**: E5 (causal) and E8 (graph) preserve directionality (ARCH-18)
5. **Temporal Exclusion**: E2-E4 (temporal) NEVER count toward topic detection (AP-60)

---

## 2. Core Data Structures

### 2.1 TeleologicalFingerprint

The fundamental unit of storage. Each memory node is stored as a `TeleologicalFingerprint`.

**Location**: `crates/context-graph-core/src/types/fingerprint/teleological/types.rs`

```rust
pub struct TeleologicalFingerprint {
    pub id: Uuid,                    // Unique identifier
    pub semantic: SemanticFingerprint, // 13-embedding array
    pub content_hash: [u8; 32],      // SHA-256 of source content
    pub created_at: DateTime<Utc>,   // Creation timestamp
    pub last_updated: DateTime<Utc>, // Last modification
    pub access_count: u64,           // Access tracking for importance
    pub importance: f32,             // Priority score [0.0, 1.0]
    pub e6_sparse: Option<SparseVector>, // Original E6 sparse (optional)
}
```

**Storage Size**: ~63KB per fingerprint (vs ~6KB if fused = 67% information preserved)

### 2.2 SemanticFingerprint (TeleologicalArray)

The 13-embedding array containing all perspectives on a memory.

**Location**: `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs`

```rust
pub struct SemanticFingerprint {
    // FOUNDATION
    pub e1_semantic: [f32; 1024],         // E1: Dense semantic (e5-large-v2)

    // TEMPORAL (POST-RETRIEVAL ONLY)
    pub e2_temporal_recent: [f32; 512],   // E2: Exponential decay freshness
    pub e3_temporal_periodic: [f32; 512], // E3: Fourier time-of-day patterns
    pub e4_temporal_positional: [f32; 512], // E4: Sinusoidal sequence position

    // SEMANTIC ENHANCERS
    pub e5_causal_as_cause: [f32; 768],   // E5a: When this memory IS the cause
    pub e5_causal_as_effect: [f32; 768],  // E5b: When this memory IS the effect
    pub e6_sparse_indices: Vec<u16>,       // E6: Sparse lexical term IDs
    pub e6_sparse_values: Vec<f32>,        // E6: Sparse lexical weights
    pub e7_code: [f32; 1536],             // E7: Code patterns (Qodo-Embed)

    // RELATIONAL ENHANCERS
    pub e8_graph_as_source: [f32; 384],   // E8a: When this IS the source
    pub e8_graph_as_target: [f32; 384],   // E8b: When this IS the target

    // STRUCTURAL
    pub e9_hdc: [f32; 1024],              // E9: Noise-robust HDC projection

    // INTENT & ENTITY
    pub e10_multimodal: [f32; 768],       // E10: Intent/goal alignment (CLIP)
    pub e11_entity: [f32; 768],           // E11: Entity knowledge (KEPLER)

    // PRECISION (RERANKING ONLY)
    pub e12_late_interaction: Vec<[f32; 128]>, // E12: Per-token ColBERT

    // RECALL (STAGE 1 ONLY)
    pub e13_splade_indices: Vec<u16>,     // E13: SPLADE term IDs
    pub e13_splade_values: Vec<f32>,      // E13: SPLADE term weights
}
```

### 2.3 Memory Sources

Memories enter the graph from different sources:

**Location**: `crates/context-graph-core/src/memory/source.rs`

| Source | Trigger | Content |
|--------|---------|---------|
| `HookDescription` | Claude Code tool use | Claude's description of action |
| `ClaudeResponse` | SessionEnd, Stop | Session summaries, significant responses |
| `MDFileChunk` | File watcher | Markdown chunks (200 words, 50 overlap) |
| `CodeEntity` | Git watcher | AST-parsed code (functions, structs, traits) |

---

## 3. The 13 Embedders

### 3.1 Embedder Specifications

| ID | Name | Dimension | Model | What It Finds |
|----|------|-----------|-------|---------------|
| **E1** | V_meaning | 1024D | e5-large-v2 | Semantic similarity (FOUNDATION) |
| **E2** | V_freshness | 512D | Exponential decay | Recency |
| **E3** | V_periodicity | 512D | Fourier encoding | Time-of-day patterns |
| **E4** | V_ordering | 512D | Sinusoidal PE | Conversation sequence |
| **E5** | V_causality | 768D×2 | Longformer SCM | Causal chains (asymmetric) |
| **E6** | V_selectivity | ~30K sparse | SPLADE | Exact keyword matches |
| **E7** | V_correctness | 1536D | Qodo-Embed-1.5B | Code patterns, signatures |
| **E8** | V_connectivity | 384D×2 | MiniLM | Graph structure (asymmetric) |
| **E9** | V_robustness | 1024D | HDC projection | Noise-robust structure |
| **E10** | V_multimodality | 768D | CLIP | Intent/goal alignment |
| **E11** | V_factuality | 768D | KEPLER | Entity knowledge |
| **E12** | V_precision | 128D/token | ColBERT | Exact phrase matches |
| **E13** | V_keyword | ~30K sparse | SPLADE v3 | Term expansions |

### 3.2 Embedder Categories & Topic Weights

| Category | Embedders | Topic Weight | Purpose |
|----------|-----------|--------------|---------|
| **SEMANTIC** | E1, E5, E6, E7, E10, E12, E13 | 1.0 | Core meaning capture |
| **RELATIONAL** | E8, E11 | 0.5 | Structural relationships |
| **STRUCTURAL** | E9 | 0.5 | Noise-robust patterns |
| **TEMPORAL** | E2, E3, E4 | 0.0 | POST-RETRIEVAL ONLY |

**Critical**: Temporal embedders (E2-E4) NEVER contribute to topic detection (AP-60). Temporal proximity ≠ semantic relationship.

### 3.3 Asymmetric Embedders

Two embedders use **asymmetric similarity** where direction matters:

**E5 Causal** (ARCH-18, AP-77):
- `e5_causal_as_cause`: Query "What did X cause?" → boost 1.2×
- `e5_causal_as_effect`: Query "What caused X?" → dampen 0.8×
- NEVER use symmetric cosine for causal queries

**E8 Graph** (ARCH-18):
- `e8_graph_as_source`: Query "What does X import?" → source perspective
- `e8_graph_as_target`: Query "What imports X?" → target perspective

### 3.4 How Embedders Collaborate

Each embedder finds what others MISS. Example query: "What databases work with Rust?"

```
┌─────────────────────────────────────────────────────────────────┐
│ Query: "What databases work with Rust?"                         │
├─────────────────────────────────────────────────────────────────┤
│ E1 (Semantic):  Finds "database", "Rust" by word similarity     │
│ E11 (Entity):   Finds "Diesel" (knows Diesel IS a database ORM) │
│ E7 (Code):      Finds code using `sqlx`, `diesel` crates        │
│ E5 (Causal):    Finds "migration that broke production"         │
│ E6 (Keyword):   Finds exact match "PostgreSQL", "RocksDB"       │
├─────────────────────────────────────────────────────────────────┤
│ COMBINED via Weighted RRF = Superior answer                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. Storage Architecture

### 4.1 RocksDB Column Families (36 Total)

The storage layer uses RocksDB with 36 column families for efficient separation.

**Location**: `crates/context-graph-storage/src/column_families.rs`

**Base Column Families (8)**:
| CF Name | Purpose | Key Format |
|---------|---------|------------|
| `nodes` | Primary node storage | UUID (16 bytes) |
| `edges` | Graph edge storage | Prefix-based range scan |
| `embeddings` | Dense embeddings | 64KB blocks |
| `metadata` | Node metadata | UUID |
| `temporal` | Time-based indexing | Timestamp prefix |
| `tags` | Tag-based indexing | Tag string |
| `sources` | Source URI indexing | URI string |
| `system` | System metadata | Fixed keys |

**Teleological Column Families (15)**:
| CF Name | Purpose | Size |
|---------|---------|------|
| `fingerprints` | Primary TeleologicalFingerprint storage | ~63KB/entry |
| `topic_profiles` | 13D topic strength profiles | ~52 bytes/entry |
| `e13_splade_inverted` | Inverted index for E13 sparse recall | Variable |
| `e6_sparse_inverted` | Inverted index for E6 keywords | Variable |
| `e1_matryoshka_128` | Truncated 128D E1 for fast ANN | 512 bytes/entry |
| `e12_late_interaction` | ColBERT token embeddings | Variable |
| `synergy_matrix` | Topic portfolio singleton | ~1KB |

**Quantized Embedder Column Families (13)**:
| CF Name | Embedder | Purpose |
|---------|----------|---------|
| `emb_0` | E1 | Quantized semantic embeddings |
| `emb_1` | E2 | Quantized temporal-recent |
| `emb_2` | E3 | Quantized temporal-periodic |
| ... | ... | ... |
| `emb_12` | E13 | Quantized SPLADE |

### 4.2 Index Types

**HNSW Indexes (10 embedders)**:
- E1, E1-Matryoshka128, E2, E3, E4, E5 (×2), E7, E8 (×2), E9, E10, E11
- GPU-accelerated via faiss-gpu
- Sub-millisecond ANN search

**Inverted Indexes (2 sparse embedders)**:
- E6: Term ID → [fingerprint IDs with scores]
- E13: Term ID → [fingerprint IDs with scores]
- Used for Stage 1 sparse recall

**Token-level Index (1 embedder)**:
- E12: Variable-length token sequences
- Used only for final-stage MaxSim reranking

### 4.3 Key Format Strategy

All keys use fixed-size formats for efficient range scans:

```rust
// Fingerprint key: UUID as 16 bytes
fn fingerprint_key(id: Uuid) -> [u8; 16] {
    *id.as_bytes()
}

// Inverted index key: term_id as 2 bytes (u16 big-endian)
fn inverted_key(term_id: u16) -> [u8; 2] {
    term_id.to_be_bytes()
}

// Singleton key for synergy matrix
const SYNERGY_MATRIX_KEY: &[u8] = b"synergy"; // 7 bytes
```

---

## 5. How the Graph Grows

### 5.1 Memory Injection Flow

**Location**: `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs`

When a memory is injected via `inject_context` or `store_memory`:

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. RECEIVE CONTENT                                              │
│    • User input, tool description, file chunk, or code entity   │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. GENERATE 13 EMBEDDINGS (GPU batch, <200ms)                   │
│    • E1-E13 computed in parallel on GPU                         │
│    • All 13 or FAIL FAST (ARCH-01)                             │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. CREATE TELEOLOGICAL FINGERPRINT                              │
│    • UUID generated                                             │
│    • Content hash computed (SHA-256)                            │
│    • Metadata attached (importance, session_id, source)         │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. STORE IN ROCKSDB (WriteBatch - atomic)                       │
│    • Serialize fingerprint to `fingerprints` CF (~63KB)         │
│    • Store quantized vectors to `emb_0`..`emb_12` CFs           │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. UPDATE INDEXES                                               │
│    • Add to 10 HNSW indexes (E1, E2-E5, E7-E11)                │
│    • Add to 2 inverted indexes (E6, E13 sparse terms)          │
│    • Update E12 token sequences                                 │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. RETURN UUID                                                  │
│    • Memory is now searchable from all 13 perspectives          │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Storage Growth Characteristics

| Memory Count | Fingerprint Storage | Index Overhead | Total |
|--------------|---------------------|----------------|-------|
| 1,000 | ~63 MB | ~6 MB | ~69 MB |
| 10,000 | ~630 MB | ~60 MB | ~690 MB |
| 100,000 | ~6.3 GB | ~600 MB | ~6.9 GB |
| 1,000,000 | ~63 GB | ~6 GB | ~69 GB |

### 5.3 Update Flow

When updating an existing memory:

```rust
pub async fn update_async(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<bool> {
    // 1. Check existence
    if !self.exists(fingerprint.id) { return Ok(false); }

    // 2. Load old fingerprint
    let old = self.retrieve_async(fingerprint.id).await?;

    // 3. Remove old terms from inverted indexes (atomic WriteBatch)
    let mut batch = WriteBatch::new();
    self.remove_from_inverted_indexes(&old, &mut batch);

    // 4. Update primary fingerprint storage
    self.put_fingerprint(&fingerprint, &mut batch);

    // 5. Add new terms to inverted indexes
    self.add_to_inverted_indexes(&fingerprint, &mut batch);

    // 6. Commit atomically
    self.db.write(batch)?;

    // 7. Update HNSW indexes (remove old, add new)
    self.update_hnsw_indexes(&old, &fingerprint).await?;

    Ok(true)
}
```

### 5.4 Soft Delete (30-Day Recovery)

Per SEC-06, deletions are soft by default:

```rust
pub async fn soft_delete(&self, id: Uuid) -> CoreResult<()> {
    let mut fingerprint = self.retrieve_async(id).await?;
    fingerprint.deleted_at = Some(Utc::now());
    fingerprint.recovery_deadline = Some(Utc::now() + Duration::days(30));
    self.update_async(fingerprint).await?;
    Ok(())
}
```

---

## 6. How the Graph is Traversed

### 6.1 Graph Traversal Algorithms

**Location**: `crates/context-graph-graph/src/traversal/`

**Depth-First Search (DFS)**:
```rust
// Iterative DFS with explicit stack (avoids stack overflow on 100K+ nodes)
pub fn dfs_traverse<F>(
    start: NodeId,
    graph: &Graph,
    mut visitor: F,
) -> Vec<NodeId>
where
    F: FnMut(&Node) -> bool,
{
    let mut visited = HashSet::new();
    let mut stack = vec![(start, 0)];  // (node_id, depth)
    let mut result = Vec::new();

    while let Some((node_id, depth)) = stack.pop() {
        if visited.contains(&node_id) { continue; }
        visited.insert(node_id);

        let node = graph.get_node(node_id);
        if visitor(node) {
            result.push(node_id);
        }

        // Push neighbors to stack
        for neighbor in graph.neighbors(node_id) {
            if !visited.contains(&neighbor) {
                stack.push((neighbor, depth + 1));
            }
        }
    }
    result
}
```

**Breadth-First Search (BFS)**:
```rust
// BFS for layer-by-layer neighborhood discovery
pub fn bfs_traverse(start: NodeId, graph: &Graph, max_depth: usize) -> Vec<NodeId> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start, 0));

    let mut result = Vec::new();

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth > max_depth { continue; }
        if visited.contains(&node_id) { continue; }
        visited.insert(node_id);
        result.push(node_id);

        for neighbor in graph.neighbors(node_id) {
            queue.push_back((neighbor, depth + 1));
        }
    }
    result
}
```

### 6.2 Semantic Search

**Location**: `crates/context-graph-graph/src/query/semantic.rs`

```rust
pub fn semantic_search(
    query_embedding: &[f32],
    index: &HnswIndex,
    storage: &TeleologicalStore,
    options: SearchOptions,
) -> Vec<SearchResult> {
    // 1. E1 Foundation search (ARCH-12)
    let candidates = index.search_e1(query_embedding, options.top_k * 10);

    // 2. Apply domain filter if specified
    let filtered = if let Some(domain) = options.domain {
        candidates.into_iter()
            .filter(|c| storage.get_domain(c.id) == domain)
            .collect()
    } else {
        candidates
    };

    // 3. Apply minimum similarity threshold
    let thresholded: Vec<_> = filtered.into_iter()
        .filter(|c| c.similarity >= options.min_similarity)
        .collect();

    // 4. Return top-k results
    thresholded.into_iter()
        .take(options.top_k)
        .collect()
}
```

### 6.3 Query Builder Pattern

**Location**: `crates/context-graph-graph/src/query/builder.rs`

```rust
// Fluent API for building queries
let results = QueryBuilder::semantic(&query_embedding)
    .with_domain(Domain::Code)
    .with_min_similarity(0.7)
    .with_top_k(50)
    .with_enhancers(&[Embedder::E7, Embedder::E11])
    .execute(&index, &storage)?;
```

---

## 7. Multi-Space Retrieval Pipeline

### 7.1 4-Stage Pipeline Architecture

**Location**: `crates/context-graph-storage/src/teleological/search/pipeline/`

The retrieval pipeline uses 4 stages to efficiently narrow candidates while maximizing precision:

```
┌─────────────────────────────────────────────────────────────────┐
│ STAGE 1: E13 SPLADE Sparse Recall                               │
│ • Input: Query                                                  │
│ • Output: 10,000 candidates                                     │
│ • Method: Inverted index lookup (NOT HNSW)                      │
│ • Latency: <5ms                                                 │
│ • Purpose: Cast wide net using term expansion                   │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STAGE 2: E1 Matryoshka 128D Fast ANN                            │
│ • Input: 10,000 candidates                                      │
│ • Output: 1,000 candidates                                      │
│ • Method: Truncated 128D HNSW search (ARCH-16)                  │
│ • Latency: <10ms                                                │
│ • Purpose: Fast approximate filtering                           │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STAGE 3: Multi-Space RRF Rerank                                 │
│ • Input: 1,000 candidates                                       │
│ • Output: 100 candidates                                        │
│ • Method: Weighted Reciprocal Rank Fusion across embedders      │
│ • Latency: <20ms                                                │
│ • Purpose: Combine 13 perspectives (AP-79: RRF not weighted sum)│
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ STAGE 4: E12 ColBERT MaxSim Rerank                              │
│ • Input: 100 candidates                                         │
│ • Output: k final results                                       │
│ • Method: Token-level late interaction scoring                  │
│ • Latency: <15ms                                                │
│ • Purpose: Maximum precision on final candidates                │
└─────────────────────────────────────────────────────────────────┘

Total Target: <60ms at 1M memories
```

### 7.2 Weighted Reciprocal Rank Fusion (RRF)

**Why RRF, not weighted sum** (AP-79):

Weighted sum: `score = w1*s1 + w2*s2 + ... + w13*s13`
- Problem: Embedders have different score distributions
- Problem: High score in one space dominates

RRF: `score = Σ (weight_i / (k + rank_i))`
- Benefit: Rank-based, distribution-agnostic
- Benefit: Rewards agreement across multiple embedders

```rust
fn weighted_rrf(
    rankings: &[Vec<(Uuid, f32)>],  // Per-embedder rankings
    weights: &[f32; 13],            // Weight profile
    k: f32,                          // Smoothing constant (default: 60.0)
) -> Vec<(Uuid, f32)> {
    let mut scores: HashMap<Uuid, f32> = HashMap::new();

    for (embedder_idx, ranking) in rankings.iter().enumerate() {
        let weight = weights[embedder_idx];
        for (rank, (id, _similarity)) in ranking.iter().enumerate() {
            let rrf_contribution = weight / (k + rank as f32 + 1.0);
            *scores.entry(*id).or_default() += rrf_contribution;
        }
    }

    let mut results: Vec<_> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results
}
```

### 7.3 Weight Profiles

Different query types use different weight profiles:

| Profile | E1 | E5 | E6 | E7 | E10 | E11 | Purpose |
|---------|----|----|----|----|-----|-----|---------|
| `semantic_search` | 0.40 | 0.10 | 0.10 | 0.10 | 0.10 | 0.10 | General queries |
| `code_search` | 0.20 | 0.05 | 0.10 | **0.40** | 0.05 | 0.10 | Code patterns |
| `causal_reasoning` | 0.15 | **0.45** | 0.05 | 0.05 | 0.10 | 0.10 | Why queries |
| `entity_search` | 0.20 | 0.05 | 0.10 | 0.05 | 0.10 | **0.40** | Named entities |
| `intent_search` | 0.30 | 0.10 | 0.05 | 0.05 | **0.25** | 0.15 | Goal alignment |

### 7.4 Strategy Selection

| Strategy | When to Use | Pipeline |
|----------|-------------|----------|
| `E1Only` | Simple semantic queries | E1 only |
| `MultiSpace` | E1 blind spots matter | E1 + enhancers via RRF |
| `Pipeline` | Maximum precision | E13 → E1 → RRF → E12 |
| `EmbedderFirst` | Explore specific perspective | Any embedder as primary |

---

## 8. Topic Detection & Clustering

### 8.1 Topic System

**Location**: `crates/context-graph-core/src/clustering/topic.rs`

Topics emerge when memories cluster in **semantic** spaces (NOT temporal).

**TopicProfile** (13D strength vector):
```rust
pub struct TopicProfile {
    pub id: Uuid,
    pub name: String,
    pub strengths: [f32; 13],  // Per-embedder strength [0.0, 1.0]
    pub phase: TopicPhase,
    pub created_at: DateTime<Utc>,
    pub member_count: usize,
}

impl TopicProfile {
    pub fn weighted_agreement(&self) -> f32 {
        let weights = [
            1.0, // E1 Semantic
            0.0, // E2 Temporal (EXCLUDED per AP-60)
            0.0, // E3 Temporal (EXCLUDED)
            0.0, // E4 Temporal (EXCLUDED)
            1.0, // E5 Causal
            1.0, // E6 Sparse
            1.0, // E7 Code
            0.5, // E8 Graph
            0.5, // E9 HDC
            1.0, // E10 Intent
            0.5, // E11 Entity
            1.0, // E12 ColBERT
            1.0, // E13 SPLADE
        ];

        self.strengths.iter()
            .zip(weights.iter())
            .map(|(s, w)| s * w)
            .sum()
    }
}
```

### 8.2 Weighted Agreement Formula

```
weighted_agreement = Σ(topic_weight_i × strength_i)

Category weights:
- SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0
- RELATIONAL (E8, E11): 0.5
- STRUCTURAL (E9): 0.5
- TEMPORAL (E2, E3, E4): 0.0 (ALWAYS EXCLUDED)

max_weighted_agreement = 7×1.0 + 2×0.5 + 1×0.5 = 8.5
topic_confidence = weighted_agreement / 8.5
```

**Topic Threshold** (ARCH-09): `weighted_agreement >= 2.5`

| Scenario | Calculation | Result |
|----------|-------------|--------|
| 3 semantic spaces agree | 3 × 1.0 = 3.0 | TOPIC |
| 2 semantic + 1 relational | 2 × 1.0 + 1 × 0.5 = 2.5 | TOPIC |
| 5 temporal spaces agree | 5 × 0.0 = 0.0 | NOT TOPIC |
| 2 relational + 1 structural | 2 × 0.5 + 1 × 0.5 = 1.5 | NOT TOPIC |

### 8.3 HDBSCAN Clustering

**Location**: `crates/context-graph-core/src/clustering/synthesizer.rs`

Topic detection uses HDBSCAN (GPU-accelerated via cuML):

```rust
pub struct TopicSynthesizer {
    min_cluster_size: usize,      // Default: 3
    min_samples: usize,           // Default: 2
    silhouette_threshold: f32,    // Default: 0.3
    merge_threshold: f32,         // Default: 0.9
}

impl TopicSynthesizer {
    pub fn detect_topics(&self, fingerprints: &[TeleologicalFingerprint]) -> Vec<TopicProfile> {
        let mut all_clusters = Vec::new();

        // Run HDBSCAN per semantic embedder (exclude temporal E2-E4)
        for embedder in [E1, E5, E6, E7, E8, E9, E10, E11, E12, E13] {
            let vectors = self.extract_vectors(fingerprints, embedder);
            let clusters = hdbscan_gpu(&vectors, self.min_cluster_size);
            all_clusters.extend(clusters);
        }

        // Synthesize topics from cross-space clustering
        self.synthesize_topics(all_clusters, fingerprints)
    }
}
```

### 8.4 Topic Lifecycle

```rust
pub enum TopicPhase {
    Emerging,   // <1 hour old, membership churn > 0.3
    Stable,     // 24+ hours consistent, churn < 0.1
    Declining,  // Decreasing access, churn > 0.5
    Merging,    // Being absorbed into another topic
}
```

---

## 9. Session & Sequence Tracking

### 9.1 Session Lifecycle

**Location**: `crates/context-graph-core/src/memory/session.rs`

```rust
pub struct Session {
    pub id: String,              // UUID string
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub memory_count: u64,
}

pub enum SessionStatus {
    Active,     // Session in progress
    Completed,  // Normal termination
    Abandoned,  // Timeout or crash
}
```

**Lifecycle**:
1. `SessionStart` hook → `Session::new()` creates Active session
2. Memory captured → `increment_memory_count()`
3. `SessionEnd` hook → `complete()` marks Completed
4. Timeout/crash → `abandon()` marks Abandoned

### 9.2 E4 Sequence Tracking

E4 (V_ordering) tracks conversation sequence position:

```rust
// Sinusoidal positional encoding (like Transformer PE)
pub fn encode_sequence_position(position: usize, dim: usize) -> Vec<f32> {
    let mut encoding = vec![0.0; dim];
    for i in 0..(dim / 2) {
        let angle = position as f32 / (10000.0_f32).powf(2.0 * i as f32 / dim as f32);
        encoding[2 * i] = angle.sin();
        encoding[2 * i + 1] = angle.cos();
    }
    encoding
}
```

This enables queries like:
- "What was discussed before X?"
- "What came after Y?"
- "Navigate conversation timeline"

---

## 10. AI Navigation System

### 10.1 Full 13-Embedder Visibility

The system provides AI models with complete visibility into all 13 embedder perspectives:

**Response Structure**:
```json
{
  "fingerprintId": "uuid",
  "similarity": 0.75,
  "dominantEmbedder": "E7_Code",
  "e1Score": 0.25,
  "embedderScores": {
    "semantic": {
      "E1_Semantic": 0.25,
      "E5_Causal": 0.30,
      "E6_Sparse": 0.45,
      "E7_Code": 0.85,
      "E10_Intent": 0.35,
      "E12_ColBERT": 0.40,
      "E13_SPLADE": 0.50
    },
    "relational": {
      "E8_Graph": 0.20,
      "E11_Entity": 0.72
    },
    "structural": {
      "E9_HDC": 0.45
    },
    "temporal": {
      "E2_Recency": 0.90,
      "E3_Periodic": 0.30,
      "E4_Sequence": 0.65
    }
  },
  "agreementCount": 5,
  "blindSpots": [
    {
      "embedder": "E7_Code",
      "score": 0.85,
      "e1Score": 0.25,
      "finding": "E7_Code found via code patterns but E1 missed"
    }
  ],
  "navigationHints": [
    "E7 (code) found more than E1 - try search_code for code patterns",
    "E11 (entity) found related entities - try search_by_entities"
  ]
}
```

### 10.2 Blind Spot Detection

Blind spots are detected when:
- Enhancer score >= 0.5 (found something)
- E1 score < 0.3 (semantic missed it)

```rust
const E1_MISS_THRESHOLD: f32 = 0.3;
const ENHANCER_FIND_THRESHOLD: f32 = 0.5;

fn compute_blind_spots(scores: &[f32; 13], e1_score: f32) -> Vec<BlindSpot> {
    if e1_score >= E1_MISS_THRESHOLD {
        return vec![];  // E1 found it, no blind spot
    }

    let enhancers = [
        (4, "E5_Causal", "causal chains"),
        (5, "E6_Sparse", "exact keywords"),
        (6, "E7_Code", "code patterns"),
        (7, "E8_Graph", "graph structure"),
        (8, "E9_HDC", "noise-robust matches"),
        (9, "E10_Intent", "intent alignment"),
        (10, "E11_Entity", "entity knowledge"),
        (11, "E12_ColBERT", "phrase precision"),
        (12, "E13_SPLADE", "term expansion"),
    ];

    enhancers.iter()
        .filter(|(idx, _, _)| scores[*idx] >= ENHANCER_FIND_THRESHOLD)
        .map(|(idx, name, finding)| BlindSpot {
            embedder: name.to_string(),
            score: scores[*idx],
            e1_score,
            finding: format!("{} found via {} but E1 missed", name, finding),
        })
        .collect()
}
```

### 10.3 Navigation Hints

The system suggests which specialized tools to explore based on score patterns:

```rust
fn compute_navigation_hints(scores: &[f32; 13]) -> Vec<String> {
    let mut hints = Vec::new();
    let e1 = scores[0];

    if scores[6] > e1 + 0.2 {  // E7 Code
        hints.push("E7 (code) found more than E1 - try search_code for code patterns");
    }
    if scores[10] > e1 + 0.2 {  // E11 Entity
        hints.push("E11 (entity) found related entities - try search_by_entities");
    }
    if scores[4] > e1 + 0.15 {  // E5 Causal
        hints.push("E5 (causal) found causal links - try search_causes for 'why' queries");
    }
    if scores[7] > e1 + 0.15 {  // E8 Graph
        hints.push("E8 (graph) found structural relationships - try search_connections");
    }
    if scores[9] > e1 + 0.15 {  // E10 Intent
        hints.push("E10 (intent) found goal alignment - try search_by_intent");
    }

    hints
}
```

---

## 11. Performance Characteristics

### 11.1 Latency Budgets (GPU-Accelerated)

| Operation | Target | GPU Acceleration |
|-----------|--------|------------------|
| All 13 embeddings | <200ms | Batched Tensor Core FP16 |
| Per-space HNSW | <1ms | faiss-gpu IVF/HNSW |
| inject_context P95 | <500ms | Full GPU pipeline |
| store_memory P95 | <800ms | GPU embed + index |
| Any tool P99 | <1000ms | Worst case with GPU |
| Topic detection | <20ms | cuML HDBSCAN |
| Warm-load startup | <30s | All 13 models to VRAM |

### 11.2 Comparison: GPU vs CPU

| Operation | GPU (RTX 5090) | CPU (baseline) | Speedup |
|-----------|----------------|----------------|---------|
| All 13 embeddings | <200ms | ~2000ms | 10x |
| HNSW search | <1ms | ~5ms | 5x |
| HDBSCAN clustering | <20ms | ~500ms | 25x |
| Full pipeline | <500ms | ~3000ms | 6x |

### 11.3 VRAM Budget (32GB)

| Allocation | Size | Purpose |
|------------|------|---------|
| 13 Embedders | ~10GB | Warm-loaded, FP16 weights |
| E7 Code (separate) | ~3GB | Qodo-Embed-1.5B dedicated |
| FAISS Indexes | ~8GB | Per-space HNSW |
| Batch Buffers | ~4GB | Inference batches |
| cuML Workspace | ~2GB | Clustering, analytics |
| Reserved | ~5GB | Spike headroom |

---

## 12. Architecture Diagrams

### 12.1 Memory Injection Flow

```
┌─────────────────────────────────────────────────────────────────┐
│              MEMORY INJECTION (inject_context)                   │
│  [User Input] → [Generate 13 embeddings] → [Create Fingerprint] │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│            TeleologicalFingerprint (~63KB)                       │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ SemanticFingerprint (13-embedding array):               │    │
│  │ • E1 (1024D), E2-E4 (512D each), E5 (768D×2)           │    │
│  │ • E6/E13 (sparse), E7 (1536D), E8 (384D×2)             │    │
│  │ • E9 (1024D), E10-E11 (768D each), E12 (variable)      │    │
│  │ • Metadata: id, content_hash, importance, access_count │    │
│  └─────────────────────────────────────────────────────────┘    │
└────────────────────────────┬────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
   RocksDB Storage      HNSW Indexes (10)    Inverted Indexes
   (36 Column Families) • E1, E1-128D        • E6, E13 sparse
   • fingerprints       • E2-E5, E7-E11      • Term ID → [FP IDs]
   • topic_profiles
   • emb_0..emb_12
```

### 12.2 Multi-Space Retrieval Pipeline

```
┌────────────────────────────────────────────────────────────────┐
│                        QUERY INPUT                              │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│ STAGE 1: E13 SPLADE Sparse Recall                              │
│ [Query] → [Inverted Index] → [10,000 candidates]               │
│ Latency: <5ms                                                  │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│ STAGE 2: E1 Matryoshka 128D Fast ANN                           │
│ [10,000] → [HNSW 128D] → [1,000 candidates]                    │
│ Latency: <10ms                                                 │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│ STAGE 3: Multi-Space Weighted RRF                              │
│ [1,000] → [13 embedder rankings] → [RRF fusion] → [100]        │
│ Latency: <20ms                                                 │
│                                                                │
│  E1  E5  E6  E7  E8  E9  E10 E11 E12 E13                      │
│  ↓   ↓   ↓   ↓   ↓   ↓   ↓   ↓   ↓   ↓                        │
│  └───┴───┴───┴───┴───┴───┴───┴───┴───┘                        │
│              ↓                                                  │
│      Weighted RRF Fusion                                        │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│ STAGE 4: E12 ColBERT MaxSim Rerank                             │
│ [100] → [Token-level scoring] → [k final results]              │
│ Latency: <15ms                                                 │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│                      FINAL RESULTS                              │
│ • Per-result: all 13 embedder scores                           │
│ • Blind spot alerts                                             │
│ • Navigation hints                                              │
│ • Agreement metrics                                             │
└────────────────────────────────────────────────────────────────┘
```

### 12.3 Topic Detection Flow

```
┌────────────────────────────────────────────────────────────────┐
│                    FINGERPRINTS IN STORAGE                      │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│            PER-EMBEDDER HDBSCAN CLUSTERING (GPU)                │
│                                                                │
│  E1 Space    E5 Space    E7 Space    E11 Space    ...          │
│  ┌───┐       ┌───┐       ┌───┐       ┌───┐                     │
│  │ ● │       │ ● │       │ ● │       │ ● │                     │
│  │●●●│       │ ● │       │●●●│       │●● │                     │
│  │ ● │       │●●●│       │ ● │       │ ● │                     │
│  └───┘       └───┘       └───┘       └───┘                     │
│                                                                │
│  (E2-E4 EXCLUDED per AP-60 - temporal proximity ≠ topic)       │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│              WEIGHTED AGREEMENT CALCULATION                     │
│                                                                │
│  weighted_agreement = Σ(topic_weight × strength)               │
│                                                                │
│  SEMANTIC (1.0): E1=0.8 + E5=0.6 + E7=0.9 = 2.3               │
│  RELATIONAL (0.5): E11=0.7 × 0.5 = 0.35                        │
│  STRUCTURAL (0.5): E9=0.4 × 0.5 = 0.2                          │
│  TEMPORAL (0.0): E2=0.9 × 0.0 = 0.0 (EXCLUDED)                 │
│                                                                │
│  TOTAL = 2.3 + 0.35 + 0.2 = 2.85 >= 2.5 → TOPIC!              │
└────────────────────────────┬───────────────────────────────────┘
                             │
                             ▼
┌────────────────────────────────────────────────────────────────┐
│                    TOPIC PROFILE OUTPUT                         │
│                                                                │
│  {                                                             │
│    "id": "uuid",                                               │
│    "name": "Rust Database Integration",                        │
│    "strengths": [0.8, 0.0, 0.0, 0.0, 0.6, 0.3, 0.9, ...],     │
│    "weighted_agreement": 2.85,                                 │
│    "confidence": 0.335,  // 2.85 / 8.5                         │
│    "phase": "Emerging",                                        │
│    "dominant_spaces": ["E1", "E5", "E7"]                       │
│  }                                                             │
└────────────────────────────────────────────────────────────────┘
```

---

## Constitution Compliance Summary

| Rule | Description | Compliance |
|------|-------------|------------|
| ARCH-01 | TeleologicalArray is atomic (all 13 or nothing) | ✓ |
| ARCH-09 | Topic threshold: weighted_agreement >= 2.5 | ✓ |
| ARCH-12 | E1 is foundation - all retrieval starts with E1 | ✓ |
| ARCH-13 | 4-stage pipeline (E13→E1→RRF→E12) | ✓ |
| ARCH-18 | E5/E8 asymmetric similarity | ✓ |
| ARCH-NAV-01 | AI receives full visibility into all 13 embedder scores | ✓ |
| AP-60 | Temporal (E2-E4) NEVER count toward topics | ✓ |
| AP-77 | E5 MUST NOT use symmetric cosine | ✓ |
| AP-79 | Use Weighted RRF, not weighted sum | ✓ |
| SEC-06 | Soft delete with 30-day recovery | ✓ |

---

## Key File References

| Component | File Path |
|-----------|-----------|
| TeleologicalFingerprint | `crates/context-graph-core/src/types/fingerprint/teleological/types.rs` |
| SemanticFingerprint | `crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs` |
| Storage CRUD | `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs` |
| Column Families | `crates/context-graph-storage/src/column_families.rs` |
| Retrieval Pipeline | `crates/context-graph-storage/src/teleological/search/pipeline/` |
| Topic Detection | `crates/context-graph-core/src/clustering/synthesizer.rs` |
| Graph Traversal | `crates/context-graph-graph/src/traversal/` |
| MCP Handlers | `crates/context-graph-mcp/src/handlers/tools/` |
| Weight Profiles | `crates/context-graph-core/src/weights/mod.rs` |

---

*This document is the authoritative reference for understanding how the Context Graph knowledge graph works, grows, and is traversed using the 13-embedder multi-perspective architecture.*
