# E8 Graph Embedder (V_connectivity) - Upgrade Analysis

**Generated:** 2026-01-22
**Updated:** 2026-01-22 (Added E5/E2-E4 patterns analysis)
**Status:** Analysis Complete - Implementation Patterns Defined

## Executive Summary

E8 (V_connectivity) is designed to capture **graph structure and relational connectivity** in the 13-embedder system. Our analysis reveals that E8 is **significantly under-utilized and under-validated** compared to other supporting embedders.

**Key Insight:** By studying E5 (Causal) and E2-E4 (Temporal) implementations, we've identified concrete patterns that can elevate E8 from a generic paraphrase model to a purpose-built relational embedder.

| Finding | Impact | Severity | Solution Pattern |
|---------|--------|----------|------------------|
| **Missing stress tests** | Cannot measure E8's unique contribution | HIGH | Follow E5/E6 stress corpus pattern |
| **No dedicated benchmark** | E5, E6 have benchmarks; E8 does not | HIGH | Create `graph-bench` like `causal-realdata-bench` |
| **No asymmetric embeddings** | Cannot distinguish A→B from B→A | HIGH | Adopt E5's dual projection approach |
| **No marker detection** | Cannot identify structural indicators | MEDIUM | Adapt E5's 100+ marker pattern system |
| **No MCP tools** | No graph traversal operations | MEDIUM | Create tools like E4's sequence tools |
| **Generic model** | Uses paraphrase model, not graph-specialized | MEDIUM | Consider custom projections |
| **Naming confusion** | "Graph" → "Emotional" despite V_connectivity | LOW | Standardize naming |

---

## Part 1: Current Implementation Analysis

### Model Specification

| Property | Value | Source |
|----------|-------|--------|
| Model | `sentence-transformers/paraphrase-MiniLM-L6-v2` | `constants.rs:L12` |
| Dimension | 384D | `constants.rs:L6` |
| Max Tokens | 512 | `constants.rs:L7` |
| Latency Budget | 5ms P95 | `constants.rs:L8` |
| Parameters | 22M | model.rs |
| Memory | ~40-80MB (FP16-FP32) | - |
| Category | Relational | `category.rs:L194` |
| Topic Weight | 0.5 | `category.rs:L94` |

### File Locations

```
crates/context-graph-embeddings/src/models/pretrained/graph/
├── mod.rs         - Module re-exports
├── model.rs       - GraphModel struct, EmbeddingModel trait impl
├── constants.rs   - Configuration (384D, 512 tokens, 5ms budget)
├── encoding.rs    - encode_relation(), encode_context() utilities
├── forward.rs     - GPU forward pass (BERT pipeline)
├── state.rs       - Model state management
└── tests.rs       - Basic functionality tests
```

### Existing Graph Encoding Utilities

E8 already has specialized encoding functions (underutilized):

```rust
// encoding.rs:32-35 - Relation triple encoding
encode_relation(subject, predicate, object) -> String
// "Alice", "works_at", "Anthropic" → "Alice works at Anthropic"

// encoding.rs:63-77 - Context with neighbors
encode_context(node, neighbors: &[(String, String)]) -> String
// "Alice", [("works_at", "Anthropic"), ("knows", "Bob")]
// → "Alice: works at Anthropic, knows Bob"
```

### Current Gaps vs E5/E2-E4

| Feature | E5 Causal | E2-E4 Temporal | E8 Graph (Current) |
|---------|-----------|----------------|-------------------|
| Asymmetric embeddings | `as_cause`, `as_effect` | N/A | **MISSING** |
| Marker detection | 100+ patterns | N/A | **MISSING** |
| Direction modifiers | 1.2x / 0.8x | N/A | **MISSING** |
| Dedicated MCP tools | `search_causes`, `get_causal_chain` | `get_session_timeline`, `traverse_memory_chain` | **MISSING** |
| Stress tests | YES (7 docs, 3 queries) | N/A (excluded from topic tests) | **MISSING** |
| Dedicated benchmark | `causal-realdata-bench` | `temporal-bench`, `temporal-realdata-bench` | **MISSING** |
| Comprehensive metrics | Direction detection, asymmetry ratio, COPA | Kendall's tau, sequence accuracy, boundary F1 | **MISSING** |

---

## Part 2: Learning from E5 Causal Embedder

### E5's Three-Layer Pattern

E5 achieves meaningful asymmetric retrieval through a sophisticated three-layer architecture:

```
Layer 1: MODEL ASYMMETRY (Dual Projections)
├─ Single encoder pass (allenai/longformer-base-4096, 768D)
├─ Dual output projections: W_cause (768×768), W_effect (768×768)
├─ Perturbed identity matrices: I + N(0, 0.02) for each
└─ Result: Two distinct 768D vectors from one text

Layer 2: MARKER DETECTION (Content-Aware)
├─ 47 cause indicators: "because", "due", "reason", "trigger", "source"
├─ 47 effect indicators: "therefore", "result", "leads", "consequence"
├─ Global attention strategy based on marker positions
└─ Direction inference from marker prevalence

Layer 3: SEARCH INTEGRATION (Asymmetric Similarity)
├─ Direction-aware similarity: cos × mod × (0.7 + 0.3 × overlap)
├─ Direction modifiers: 1.2x (forward), 0.8x (backward)
├─ Asymmetric fingerprint comparison: query.as_cause vs doc.as_effect
└─ Weight profile: 45% E5 in causal_reasoning
```

### E5's Key Innovations Applicable to E8

1. **Perturbed Identity Projections** - Create asymmetry without retraining:
   ```rust
   // E5 approach (weights.rs:152-316)
   W_cause = I + N(0, 0.02)   // 768×768 perturbed identity
   W_effect = I + N(0, 0.02)  // Different perturbation
   // Same base embedding, different projections → asymmetry
   ```

2. **Dual Embedding API** - Single encode, dual output:
   ```rust
   // E5 approach (model.rs:225-258)
   fn embed_dual(text) -> (cause_vec, effect_vec)
   // E8 equivalent:
   fn embed_dual(text) -> (source_vec, target_vec)
   ```

3. **Marker-Based Direction Detection** - Content-aware classification:
   ```rust
   // E5 uses 100+ linguistic markers
   // E8 could use 50+ structural markers:
   SOURCE_MARKERS: ["imports", "uses", "requires", "depends on", "calls", "extends", "owns"]
   TARGET_MARKERS: ["imported by", "used by", "required by", "depended on", "called by", "extended by"]
   ```

4. **Direction Modifiers** - Amplify/dampen based on query type:
   ```rust
   // E5 (asymmetric.rs:34-43)
   CAUSE_TO_EFFECT: 1.2  // Forward inference amplified
   EFFECT_TO_CAUSE: 0.8  // Backward inference dampened
   // E8 equivalent:
   SOURCE_TO_TARGET: 1.2  // "What does X use?" amplified
   TARGET_TO_SOURCE: 0.8  // "What uses X?" dampened
   ```

5. **Specialized MCP Tools** - Graph-specific operations:
   ```rust
   // E5 has: search_causes, get_causal_chain
   // E8 should have: search_connections, get_graph_path
   ```

---

## Part 3: Learning from E2-E4 Temporal Embedders

### E2-E4's Pattern: Custom Models + POST-Retrieval Boosts

Temporal embedders use a different pattern - **custom mathematical models** without pretrained weights:

```
E2 (V_freshness) - 512D Exponential Decay
├─ No pretrained weights - pure computation
├─ 4 time scales: hour, day, week, month
├─ Formula: base_decay × cos(phase + time_delta × decay_rate)
└─ Applied as POST-retrieval boost, not similarity

E3 (V_periodicity) - 512D Fourier Harmonics
├─ No pretrained weights - pure computation
├─ 5 periods: hour, day, week, month, year
├─ Formula: sin(n×phase), cos(n×phase) for n=1..51
└─ Captures cyclical patterns (morning vs evening)

E4 (V_ordering) - 512D Transformer PE
├─ No pretrained weights - pure computation
├─ Formula: PE(pos, 2i) = sin(pos / base^(2i/d))
├─ Prefers session sequence over calendar time
└─ Enables "before/after" queries
```

### E4's MCP Tools Pattern

E4 has four specialized tools that E8 should emulate:

| E4 Tool | Purpose | E8 Equivalent |
|---------|---------|---------------|
| `get_conversation_context` | Get memories around current turn | `get_graph_neighbors` - Get connected memories |
| `get_session_timeline` | Ordered timeline of session | `get_graph_topology` - Map of memory connections |
| `traverse_memory_chain` | Multi-hop navigation | `traverse_graph_path` - Multi-hop graph traversal |
| `compare_session_states` | Before/after comparison | `compare_graph_states` - Connectivity changes |

### E2-E4's Metric Pattern

| E2-E4 Metric | What It Measures | E8 Equivalent |
|--------------|------------------|---------------|
| Freshness Precision@K | Fresh items in top-K | Connection Precision@K - Connected items in top-K |
| Sequence Accuracy | Before/after ordering | Direction Accuracy - A→B vs B→A |
| Kendall's Tau | Rank correlation | Path Recall@K - Multi-hop paths found |
| Boundary F1 | Session boundary detection | Centrality Correlation - Hub node detection |

---

## Part 4: Proposed E8 Upgrade Architecture

### Phase 1: Asymmetric Graph Embeddings (High Priority)

Following E5's pattern, upgrade E8 to produce **dual vectors** for directional relationships:

```rust
// NEW: crates/context-graph-embeddings/src/models/pretrained/graph/projections.rs

pub struct GraphProjectionWeights {
    pub w_source: Vec<Vec<f32>>,  // 384×384 perturbed identity
    pub w_target: Vec<Vec<f32>>,  // 384×384 different perturbation
}

impl GraphProjectionWeights {
    pub fn new(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        Self {
            w_source: create_perturbed_identity(384, &mut rng, 0.02),
            w_target: create_perturbed_identity(384, &mut rng, 0.02),
        }
    }

    pub fn project_source(&self, base: &[f32]) -> Vec<f32> {
        matmul(&self.w_source, base)
    }

    pub fn project_target(&self, base: &[f32]) -> Vec<f32> {
        matmul(&self.w_target, base)
    }
}
```

**Fingerprint Changes:**
```rust
// UPDATE: crates/context-graph-core/src/types/fingerprint/semantic/mod.rs

pub struct SemanticFingerprint {
    // ... existing fields ...

    // E8 upgrade: dual vectors for directional relationships
    pub e8_as_source: Vec<f32>,  // 384D - "this memory points TO others"
    pub e8_as_target: Vec<f32>,  // 384D - "this memory is pointed TO by others"
    pub e8_graph: Vec<f32>,      // DEPRECATED - backward compat
}
```

**Dual Embedding API:**
```rust
// UPDATE: crates/context-graph-embeddings/src/models/pretrained/graph/model.rs

impl GraphModel {
    /// Embed text as a graph source (outgoing relationships)
    pub async fn embed_as_source(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>;

    /// Embed text as a graph target (incoming relationships)
    pub async fn embed_as_target(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding>;

    /// Embed text with both projections (single encoder pass)
    pub async fn embed_dual(&self, input: &ModelInput) -> EmbeddingResult<(ModelEmbedding, ModelEmbedding)> {
        // 1. Single encoder forward pass
        let base = self.encode_base(input).await?;

        // 2. Dual projection
        let source_vec = self.projections.project_source(&base);
        let target_vec = self.projections.project_target(&base);

        // 3. L2 normalize both
        Ok((
            ModelEmbedding::new(ModelId::Graph, l2_normalize(source_vec)),
            ModelEmbedding::new(ModelId::Graph, l2_normalize(target_vec)),
        ))
    }
}
```

### Phase 2: Structural Marker Detection (Medium Priority)

Following E5's 100+ marker pattern, create structural markers for E8:

```rust
// NEW: crates/context-graph-embeddings/src/models/pretrained/graph/marker_detection.rs

/// Source markers: indicate outgoing relationships
pub const SOURCE_INDICATORS: &[&str] = &[
    // Direct relationships
    "imports", "uses", "requires", "needs", "depends on", "calls", "invokes",
    "extends", "implements", "inherits", "derives from", "based on",
    "contains", "includes", "has", "owns", "holds", "wraps",
    "connects to", "links to", "references", "points to", "accesses",

    // Code-specific
    "instantiates", "creates", "constructs", "initializes", "configures",
    "reads from", "writes to", "subscribes to", "publishes to",

    // Dependency
    "depends", "dependency", "prerequisite", "requires", "needs",
];

/// Target markers: indicate incoming relationships
pub const TARGET_INDICATORS: &[&str] = &[
    // Passive forms
    "imported by", "used by", "required by", "needed by", "called by",
    "extended by", "implemented by", "inherited by", "derived by",
    "contained by", "included by", "owned by", "wrapped by",
    "connected from", "linked from", "referenced by", "pointed to by",

    // Code-specific
    "instantiated by", "created by", "constructed by", "configured by",
    "read by", "written by", "subscribed by", "consumed by",

    // Dependency (passive)
    "depended on by", "dependent of", "prerequisite of",
];

/// Detect structural markers in text
pub fn detect_structural_markers(text: &str) -> StructuralMarkerResult {
    let text_lower = text.to_lowercase();

    let source_count = SOURCE_INDICATORS.iter()
        .filter(|m| text_lower.contains(*m))
        .count();

    let target_count = TARGET_INDICATORS.iter()
        .filter(|m| text_lower.contains(*m))
        .count();

    StructuralMarkerResult {
        source_markers: source_count,
        target_markers: target_count,
        direction: infer_direction(source_count, target_count),
        confidence: compute_confidence(source_count, target_count),
    }
}

/// Infer relationship direction from marker prevalence
pub fn infer_direction(source: usize, target: usize) -> RelationshipDirection {
    let diff_ratio = (source as f32 - target as f32) / (source + target + 1) as f32;

    if diff_ratio > 0.2 {
        RelationshipDirection::Source  // "This imports X"
    } else if diff_ratio < -0.2 {
        RelationshipDirection::Target  // "This is imported by X"
    } else {
        RelationshipDirection::Unknown
    }
}
```

### Phase 3: Asymmetric Similarity (High Priority)

Following E5's direction modifiers pattern:

```rust
// NEW: crates/context-graph-core/src/graph/asymmetric.rs

/// Direction modifiers for graph relationships (per E5 pattern)
pub const SOURCE_TO_TARGET: f32 = 1.2;  // Forward lookup amplified
pub const TARGET_TO_SOURCE: f32 = 0.8;  // Reverse lookup dampened
pub const SAME_DIRECTION: f32 = 1.0;    // No modification

/// Compute asymmetric E8 fingerprint similarity
pub fn compute_e8_asymmetric_fingerprint_similarity(
    query: &SemanticFingerprint,
    doc: &SemanticFingerprint,
    query_is_source: bool,
) -> f32 {
    let base_sim = if query_is_source {
        // "What does X use?" → query.as_source vs doc.as_target
        cosine_similarity(&query.e8_as_source, &doc.e8_as_target)
    } else {
        // "What uses X?" → query.as_target vs doc.as_source
        cosine_similarity(&query.e8_as_target, &doc.e8_as_source)
    };

    let direction_mod = if query_is_source {
        SOURCE_TO_TARGET  // 1.2x for forward lookup
    } else {
        TARGET_TO_SOURCE  // 0.8x for reverse lookup
    };

    base_sim * direction_mod
}

/// Rank documents by graph connectivity (following E5's rank_causes_by_abduction)
pub fn rank_by_connectivity(
    query_fingerprint: &SemanticFingerprint,
    candidates: &[(Uuid, SemanticFingerprint)],
    query_is_source: bool,
) -> Vec<(Uuid, f32)> {
    candidates.iter()
        .map(|(id, fp)| {
            let score = compute_e8_asymmetric_fingerprint_similarity(
                query_fingerprint,
                fp,
                query_is_source,
            );
            (*id, score)
        })
        .sorted_by(|a, b| b.1.partial_cmp(&a.1).unwrap())
        .collect()
}
```

### Phase 4: MCP Tools for Graph Operations (Medium Priority)

Following E4's sequence tools pattern:

```rust
// NEW: crates/context-graph-mcp/src/handlers/tools/graph_tools.rs

/// Tool: search_connections
/// Find memories connected to a given concept
pub async fn search_connections(
    query: &str,
    direction: ConnectionDirection,  // Source, Target, Both
    top_k: usize,
    min_score: f32,
    include_content: bool,
) -> Result<Vec<GraphSearchResult>, ToolError> {
    // 1. Embed query with dual projections
    let query_embedding = embed_graph_dual(query).await?;

    // 2. Search with graph_reasoning weight profile
    let options = TeleologicalSearchOptions::quick(top_k * 3)
        .with_strategy(SearchStrategy::MultiSpace)
        .with_weight_profile("graph_reasoning");  // 45% E8

    // 3. Fetch candidates
    let candidates = store.search_semantic(&query_embedding, options).await?;

    // 4. Apply asymmetric E8 reranking
    let is_source = matches!(direction, ConnectionDirection::Source | ConnectionDirection::Both);
    let ranked = rank_by_connectivity(&query_embedding, &candidates, is_source);

    // 5. Filter and return
    Ok(ranked.into_iter()
        .filter(|(_, score)| *score >= min_score)
        .take(top_k)
        .map(|(id, score)| GraphSearchResult { id, score, content: ... })
        .collect())
}

/// Tool: get_graph_path
/// Find multi-hop path between two memories (following E5's get_causal_chain)
pub async fn get_graph_path(
    source_id: Uuid,
    target_id: Option<Uuid>,  // None = explore from source
    max_hops: usize,
    min_similarity: f32,
    include_content: bool,
) -> Result<GraphPath, ToolError> {
    let mut path = Vec::new();
    let mut visited = HashSet::new();
    let mut current = store.get_fingerprint(source_id).await?;

    for hop in 0..max_hops {
        visited.insert(current.id);

        // Search for next hop using asymmetric E8
        let candidates = store.search_semantic(&current.fingerprint, options).await?;

        // Filter visited and apply E8 asymmetric scoring
        let next = candidates.iter()
            .filter(|c| !visited.contains(&c.id))
            .map(|c| {
                let e8_score = compute_e8_asymmetric_fingerprint_similarity(
                    &current.fingerprint,
                    &c.fingerprint,
                    true,  // source → target direction
                );
                (c.id, c.fingerprint.clone(), e8_score)
            })
            .filter(|(_, _, score)| *score >= min_similarity)
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

        match next {
            Some((id, fp, score)) => {
                path.push(GraphHop {
                    memory_id: id,
                    hop_index: hop,
                    e8_score: score,
                    cumulative: score * 0.9f32.powi(hop as i32),  // Hop attenuation
                });

                // Check if we reached target
                if Some(id) == target_id {
                    break;
                }
                current = fp;
            }
            None => break,  // No valid next hop
        }
    }

    Ok(GraphPath { hops: path, total_score: compute_total_score(&path) })
}

/// Tool: get_graph_neighbors
/// Get directly connected memories (1-hop neighborhood)
pub async fn get_graph_neighbors(
    anchor_id: Uuid,
    direction: ConnectionDirection,
    limit: usize,
    include_content: bool,
) -> Result<Vec<GraphNeighbor>, ToolError>;

/// Tool: get_graph_centrality
/// Find hub memories with high connectivity
pub async fn get_graph_centrality(
    scope: CentralityScope,  // Session, Recent, All
    metric: CentralityMetric,  // Degree, Betweenness, PageRank
    top_k: usize,
) -> Result<Vec<CentralityResult>, ToolError>;
```

### Phase 5: Weight Profile for Graph Reasoning (Medium Priority)

```rust
// UPDATE: crates/context-graph-core/src/similarity/config.rs

/// Create static weights that emphasize graph/relational reasoning
pub fn graph_reasoning_weights() -> [f32; NUM_EMBEDDERS] {
    let mut weights = [0.015; NUM_EMBEDDERS];
    weights[7] = 0.45;  // E8 Graph (PRIMARY)
    weights[0] = 0.25;  // E1 Semantic (supporting)
    weights[10] = 0.15; // E11 Entity (entity relationships)
    weights
}

// Also update code_search_weights to use E8 asymmetric
pub fn code_search_weights() -> [f32; NUM_EMBEDDERS] {
    let mut weights = [0.015; NUM_EMBEDDERS];
    weights[6] = 0.45;  // E7 Code (PRIMARY)
    weights[0] = 0.20;  // E1 Semantic
    weights[7] = 0.20;  // E8 Graph (UPGRADED from 0.15)
    weights
}
```

---

## Part 5: Benchmark & Metrics (Following E5/E4 Patterns)

### Stress Test Corpus (Following E5 Pattern)

```rust
// ADD TO: crates/context-graph-benchmark/src/stress_corpus.rs

/// Build E8 Graph stress test corpus.
/// Following E5's pattern: test asymmetric retrieval for structural relationships.
pub fn build_e8_graph_corpus() -> EmbedderStressConfig {
    EmbedderStressConfig {
        embedder: EmbedderIndex::E8Graph,
        name: "E8 Graph",
        description: "Structural connectivity and directional relationships",
        corpus: vec![
            // Direction test: A imports B
            StressCorpusEntry {
                content: "Module auth imports utils for helper functions. Auth handles login.".into(),
                doc_id: 0,
                e1_limitation: Some("E1 sees 'auth' and 'utils' but not import direction".into()),
                metadata: Some(serde_json::json!({
                    "relations": [["auth", "imports", "utils"]]
                })),
            },
            // Direction test: B imported by A (same relationship, different phrasing)
            StressCorpusEntry {
                content: "Utils module is imported by auth, api, and tests for shared helpers.".into(),
                doc_id: 1,
                e1_limitation: Some("Same keywords, opposite perspective (target)".into()),
                metadata: Some(serde_json::json!({
                    "relations": [["auth", "imports", "utils"], ["api", "imports", "utils"]]
                })),
            },
            // Transitivity test: A → B → C
            StressCorpusEntry {
                content: "Config module provides database settings. Database uses config for connection parameters.".into(),
                doc_id: 2,
                e1_limitation: Some("E1 sees config and database but not the dependency chain".into()),
                metadata: Some(serde_json::json!({
                    "relations": [["database", "uses", "config"]]
                })),
            },
            // Transitivity test: path through intermediate
            StressCorpusEntry {
                content: "Repository depends on database which depends on config for connection pooling.".into(),
                doc_id: 3,
                e1_limitation: Some("Two-hop dependency: repo → db → config".into()),
                metadata: Some(serde_json::json!({
                    "relations": [["repository", "depends_on", "database"], ["database", "depends_on", "config"]]
                })),
            },
            // Hub detection: high-centrality node
            StressCorpusEntry {
                content: "Error module is used by auth, api, database, and repository for consistent error handling.".into(),
                doc_id: 4,
                e1_limitation: Some("E1 doesn't understand centrality (many incoming edges)".into()),
                metadata: Some(serde_json::json!({
                    "relations": [
                        ["auth", "uses", "error"],
                        ["api", "uses", "error"],
                        ["database", "uses", "error"],
                        ["repository", "uses", "error"]
                    ],
                    "centrality": "high"
                })),
            },
            // Leaf node: low-centrality
            StressCorpusEntry {
                content: "Logging module provides debug output. It has no dependencies on other modules.".into(),
                doc_id: 5,
                e1_limitation: Some("Leaf node with no outgoing edges".into()),
                metadata: Some(serde_json::json!({
                    "relations": [],
                    "centrality": "low"
                })),
            },
            // Code-specific: extends/implements
            StressCorpusEntry {
                content: "UserRepository extends BaseRepository and implements CrudOperations interface.".into(),
                doc_id: 6,
                e1_limitation: Some("Inheritance and interface implementation".into()),
                metadata: Some(serde_json::json!({
                    "relations": [
                        ["UserRepository", "extends", "BaseRepository"],
                        ["UserRepository", "implements", "CrudOperations"]
                    ]
                })),
            },
        ],
        queries: vec![
            // Forward direction: "What does X import?"
            StressQuery {
                query: "What does auth import?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![0],  // auth imports utils
                anti_expected_docs: vec![1], // utils imported BY auth (wrong direction)
                e1_failure_reason: "E1 ranks both highly due to 'auth' and 'utils' overlap".into(),
            },
            // Reverse direction: "What imports X?"
            StressQuery {
                query: "What imports utils?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![1, 0],  // Both mention imports, but doc 1 is target-focused
                anti_expected_docs: vec![],
                e1_failure_reason: "E1 doesn't distinguish source vs target perspective".into(),
            },
            // Transitivity: path query
            StressQuery {
                query: "What is the dependency path to config?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![3, 2],  // Docs with transitive dependencies
                anti_expected_docs: vec![5],   // Logging has no dependencies
                e1_failure_reason: "E1 doesn't understand transitive relationships".into(),
            },
            // Centrality: hub detection
            StressQuery {
                query: "Which module is most central to the system?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![4],  // Error module (4 incoming edges)
                anti_expected_docs: vec![5], // Logging (leaf node)
                e1_failure_reason: "E1 has no notion of graph centrality".into(),
            },
            // Code structure: inheritance
            StressQuery {
                query: "What extends BaseRepository?".into(),
                target_embedder: EmbedderIndex::E8Graph,
                expected_top_docs: vec![6],  // UserRepository extends BaseRepository
                anti_expected_docs: vec![],
                e1_failure_reason: "E1 might miss inheritance keyword significance".into(),
            },
        ],
    }
}

// UPDATE: get_all_stress_configs() to include E8
pub fn get_all_stress_configs() -> Vec<EmbedderStressConfig> {
    vec![
        build_e5_causal_corpus(),
        build_e6_sparse_corpus(),
        build_e7_code_corpus(),
        build_e8_graph_corpus(),  // NEW
        build_e9_hdc_corpus(),
        build_e10_multimodal_corpus(),
        build_e11_entity_corpus(),
        build_e12_late_interaction_corpus(),
        build_e13_splade_corpus(),
    ]
}
```

### Metrics (Following E5/E4 Pattern)

```rust
// NEW: crates/context-graph-benchmark/src/metrics/graph.rs

/// Graph embedding metrics (following E5's CausalMetrics pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetrics {
    pub direction: DirectionDetectionMetrics,
    pub asymmetric: AsymmetricRetrievalMetrics,
    pub structure: StructuralMetrics,
}

/// Direction detection metrics (E5-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionDetectionMetrics {
    pub accuracy: f64,             // Overall direction accuracy
    pub source_precision: f64,     // TP_source / (TP_source + FP_source)
    pub source_recall: f64,        // TP_source / (TP_source + FN_source)
    pub target_precision: f64,
    pub target_recall: f64,
    pub direction_f1: f64,         // Macro F1
}

/// Asymmetric retrieval metrics (E5-inspired)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricRetrievalMetrics {
    pub source_to_target_mrr: f64,  // "What does X use?" queries
    pub target_to_source_mrr: f64,  // "What uses X?" queries
    pub asymmetry_ratio: f64,       // Should be ~1.5 (1.2/0.8)
    pub avg_rank_improvement: f64,  // vs symmetric baseline
}

/// Structural understanding metrics (E8-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralMetrics {
    pub path_recall_at_k: HashMap<usize, f64>,  // Multi-hop path finding
    pub centrality_correlation: f64,             // Spearman correlation with true centrality
    pub transitivity_accuracy: f64,              // A→B→C implies A→C detection
    pub hub_detection_precision: f64,            // High-degree node detection
    pub leaf_detection_precision: f64,           // Low-degree node detection
}

impl GraphMetrics {
    /// Overall quality score (following E5 pattern)
    pub fn quality_score(&self) -> f64 {
        0.30 * self.direction.direction_f1
        + 0.35 * self.asymmetric.overall_score()
        + 0.35 * self.structure.overall_score()
    }

    /// Check if metrics meet minimum thresholds
    pub fn meets_thresholds(
        &self,
        min_direction_accuracy: f64,  // e.g., 0.75
        min_asymmetry_ratio: f64,     // e.g., 1.3
        min_path_recall: f64,         // e.g., 0.60
    ) -> bool {
        self.direction.accuracy >= min_direction_accuracy
            && self.asymmetric.asymmetry_ratio >= min_asymmetry_ratio
            && self.structure.path_recall_at_k.get(&5).unwrap_or(&0.0) >= &min_path_recall
    }
}
```

### Benchmark Binary (Following `causal-realdata-bench` Pattern)

```toml
# ADD TO: crates/context-graph-benchmark/Cargo.toml

[[bin]]
name = "graph-bench"
path = "src/bin/graph_bench.rs"
required-features = ["bin"]

[[bin]]
name = "graph-realdata-bench"
path = "src/bin/graph_realdata_bench.rs"
required-features = ["bin", "real-embeddings"]
```

---

## Part 6: Dataset Generation (Following E5/E4 Pattern)

### Graph Dataset Generator

```rust
// NEW: crates/context-graph-benchmark/src/datasets/graph.rs

/// Graph structure dataset configuration
#[derive(Debug, Clone)]
pub struct GraphDatasetConfig {
    pub num_nodes: usize,           // Number of unique entities
    pub num_edges: usize,           // Number of relationships
    pub edge_types: Vec<String>,    // ["imports", "uses", "extends", ...]
    pub hub_fraction: f32,          // Fraction of high-degree nodes
    pub chain_length: usize,        // Max transitive chain length
    pub seed: u64,
}

/// Generate synthetic graph dataset with known structure
pub fn generate_graph_dataset(config: &GraphDatasetConfig) -> GraphDataset {
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Generate nodes
    for i in 0..config.num_nodes {
        nodes.push(GraphNode {
            id: Uuid::new_v4(),
            name: format!("Module{}", i),
            is_hub: rng.gen::<f32>() < config.hub_fraction,
        });
    }

    // Generate edges with known structure
    for _ in 0..config.num_edges {
        let source = rng.gen_range(0..config.num_nodes);
        let target = rng.gen_range(0..config.num_nodes);
        if source != target {
            edges.push(GraphEdge {
                source: nodes[source].id,
                target: nodes[target].id,
                relation: config.edge_types.choose(&mut rng).unwrap().clone(),
            });
        }
    }

    // Generate text descriptions for each node
    let documents: Vec<GraphDocument> = nodes.iter()
        .map(|node| {
            let outgoing: Vec<_> = edges.iter()
                .filter(|e| e.source == node.id)
                .collect();
            let incoming: Vec<_> = edges.iter()
                .filter(|e| e.target == node.id)
                .collect();

            GraphDocument {
                id: node.id,
                text: generate_node_description(node, &outgoing, &incoming),
                true_outgoing: outgoing.len(),
                true_incoming: incoming.len(),
                is_hub: node.is_hub,
            }
        })
        .collect();

    // Generate queries with known answers
    let queries = generate_graph_queries(&nodes, &edges, &documents);

    GraphDataset { nodes, edges, documents, queries }
}

/// Generate description text for a node (for embedding)
fn generate_node_description(
    node: &GraphNode,
    outgoing: &[&GraphEdge],
    incoming: &[&GraphEdge],
) -> String {
    let mut parts = vec![format!("{} module", node.name)];

    // Describe outgoing edges
    for edge in outgoing.iter().take(5) {
        parts.push(format!("{} {}", edge.relation, edge.target));
    }

    // Describe incoming edges (passive voice)
    for edge in incoming.iter().take(3) {
        parts.push(format!("{}d by {}", edge.relation.trim_end_matches('s'), edge.source));
    }

    parts.join(". ")
}
```

### Real Data Graph Extraction

```rust
// NEW: crates/context-graph-benchmark/src/realdata/graph_extractor.rs

/// Extract graph structure from code files
pub fn extract_code_graph(code: &str, language: Language) -> Vec<GraphEdge> {
    match language {
        Language::Rust => extract_rust_imports(code),
        Language::Python => extract_python_imports(code),
        Language::TypeScript => extract_ts_imports(code),
    }
}

/// Extract imports from Rust code
fn extract_rust_imports(code: &str) -> Vec<GraphEdge> {
    let import_regex = Regex::new(r"use\s+([\w:]+)").unwrap();
    let mod_regex = Regex::new(r"mod\s+(\w+)").unwrap();

    let mut edges = Vec::new();

    for cap in import_regex.captures_iter(code) {
        edges.push(GraphEdge {
            relation: "imports".into(),
            target_name: cap[1].to_string(),
        });
    }

    edges
}

/// Extract graph structure from Wikipedia links
pub fn extract_wiki_graph(article: &WikiArticle) -> Vec<GraphEdge> {
    article.links.iter()
        .map(|link| GraphEdge {
            source: article.title.clone(),
            target: link.clone(),
            relation: "links_to".into(),
        })
        .collect()
}
```

---

## Part 7: Implementation Timeline

| Phase | Task | Effort | Priority | Dependencies |
|-------|------|--------|----------|--------------|
| **Phase 1** | Add E8 stress tests | 3-4 hours | HIGH | None |
| **Phase 1** | Run baseline stress tests | 1 hour | HIGH | Stress tests |
| **Phase 2** | Implement dual projections | 4-6 hours | HIGH | None |
| **Phase 2** | Update fingerprint structure | 2-3 hours | HIGH | Dual projections |
| **Phase 3** | Implement marker detection | 3-4 hours | MEDIUM | None |
| **Phase 3** | Implement asymmetric similarity | 3-4 hours | HIGH | Dual projections |
| **Phase 4** | Create graph MCP tools | 6-8 hours | MEDIUM | Asymmetric similarity |
| **Phase 4** | Add graph_reasoning weight profile | 1 hour | MEDIUM | None |
| **Phase 5** | Create graph metrics module | 4-5 hours | HIGH | Stress tests |
| **Phase 5** | Create graph-bench binary | 4-6 hours | HIGH | Metrics |
| **Phase 6** | Create graph dataset generator | 4-5 hours | MEDIUM | Metrics |
| **Phase 6** | Create graph-realdata-bench | 4-6 hours | MEDIUM | Dataset generator |
| **Phase 7** | Fix naming (Graph vs Emotional) | 2-3 hours | LOW | All phases |

**Total Estimated Effort:** 40-55 hours

---

## Part 8: Success Criteria

### Minimum Viable E8 Upgrade

| Metric | Target | Measurement |
|--------|--------|-------------|
| Direction Accuracy | >75% | Stress test queries |
| Asymmetry Ratio | >1.3 | source→target MRR / target→source MRR |
| Path Recall@5 | >60% | Multi-hop query accuracy |
| E8 Ablation Delta | >5% | Performance(all) - Performance(no E8) |
| Stress Test Pass Rate | 100% | All expected docs in top-3 |

### Stretch Goals

| Metric | Target | Notes |
|--------|--------|-------|
| Direction Accuracy | >85% | Competitive with E5 |
| Asymmetry Ratio | ~1.5 | Matching E5's 1.2/0.8 pattern |
| Centrality Correlation | r>0.7 | Strong hub detection |
| Code Search Improvement | >10% | E8 contribution to code_search |

---

## Appendix A: File References

| File | Purpose | Status |
|------|---------|--------|
| `embeddings/src/models/pretrained/graph/model.rs` | E8 GraphModel | UPDATE: Add dual API |
| `embeddings/src/models/pretrained/graph/encoding.rs` | encode_relation, encode_context | KEEP |
| `embeddings/src/models/pretrained/graph/projections.rs` | Dual projections | NEW |
| `embeddings/src/models/pretrained/graph/marker_detection.rs` | Structural markers | NEW |
| `core/src/graph/asymmetric.rs` | Asymmetric similarity | NEW |
| `core/src/similarity/config.rs` | Weight profiles | UPDATE: Add graph_reasoning |
| `core/src/types/fingerprint/semantic/mod.rs` | Fingerprint structure | UPDATE: Add e8_as_source/target |
| `mcp/src/handlers/tools/graph_tools.rs` | MCP tools | NEW |
| `mcp/src/tools/definitions/graph.rs` | Tool schemas | NEW |
| `benchmark/src/stress_corpus.rs` | Stress tests | UPDATE: Add E8 |
| `benchmark/src/metrics/graph.rs` | Graph metrics | NEW |
| `benchmark/src/datasets/graph.rs` | Dataset generator | NEW |
| `benchmark/src/bin/graph_bench.rs` | Benchmark binary | NEW |

## Appendix B: Constitution Compliance

| Rule | Description | E8 Relevance |
|------|-------------|--------------|
| ARCH-01 | TeleologicalArray is atomic | E8 dual vectors must both be present |
| ARCH-09 | Topic threshold >= 2.5 | E8 contributes 0.5 (Relational) |
| ARCH-10 | Divergence uses SEMANTIC only | E8 excluded (Relational category) |
| ARCH-15 | Asymmetric similarity (E5) | E8 will follow same pattern |
| AP-02 | No cross-embedder comparison | E8 internal only |
| AP-77 | Direction modifiers | E8 will use 1.2/0.8 pattern |

## Appendix C: E5 vs E8 Comparison

| Aspect | E5 Causal | E8 Graph (Proposed) |
|--------|-----------|---------------------|
| Purpose | Cause-effect relationships | Structural connectivity |
| Dimension | 768D | 384D |
| Dual vectors | as_cause, as_effect | as_source, as_target |
| Direction modifiers | 1.2 forward, 0.8 backward | 1.2 forward, 0.8 backward |
| Marker patterns | 94 (47 cause + 47 effect) | ~60 (30 source + 30 target) |
| MCP tools | search_causes, get_causal_chain | search_connections, get_graph_path |
| Weight profile | causal_reasoning (45%) | graph_reasoning (45%) |
| Category | Semantic (1.0) | Relational (0.5) |

## Appendix D: E4 vs E8 Tool Comparison

| E4 Sequence Tool | E8 Graph Tool (Proposed) |
|------------------|--------------------------|
| get_conversation_context | get_graph_neighbors |
| get_session_timeline | get_graph_topology |
| traverse_memory_chain | traverse_graph_path |
| compare_session_states | compare_graph_states |
