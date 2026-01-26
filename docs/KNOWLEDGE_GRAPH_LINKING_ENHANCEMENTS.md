# Knowledge Graph Linking Enhancements

## Research Report: Leveraging 13 Embedders for Advanced Memory Linking

**Version**: 1.0.0
**Date**: 2026-01-25
**Purpose**: Research and recommendations for enhancing the Context Graph knowledge graph through advanced linking techniques using all 13 embedders

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [Research Findings](#3-research-findings)
4. [Linking Strategies](#4-linking-strategies)
5. [Graph Neural Network Approaches](#5-graph-neural-network-approaches)
6. [Multi-View Learning Techniques](#6-multi-view-learning-techniques)
7. [Implementation Options](#7-implementation-options)
8. [Priority Recommendations](#8-priority-recommendations)
9. [Architecture Proposals](#9-architecture-proposals)
10. [References](#10-references)

---

## 1. Executive Summary

### The Opportunity

The Context Graph currently stores 13 different embeddings per memory but treats them primarily as **independent similarity spaces** during retrieval. This research explores how to create **explicit links between memories** using the rich multi-perspective information from all 13 embedders, potentially transforming the system from a multi-space vector store into a true **graph neural network-enhanced knowledge graph**.

### Key Findings

1. **Multi-View Graph Neural Networks** can learn unified representations from multiple embedding spaces while preserving view-specific information
2. **Adaptive Graph Construction** can dynamically create edges based on learned similarity thresholds across embedders
3. **Contrastive Learning** can align and enhance embeddings without labels using cross-view agreement
4. **Hypergraph Neural Networks** can model higher-order relationships where multiple memories share common concepts
5. **Link Prediction** models (TransE, R-GCN, GAT) can infer missing connections between memories

### Top 3 Recommendations

| Priority | Enhancement | Impact | Complexity |
|----------|-------------|--------|------------|
| **1** | K-NN Graph Construction per Embedder | Creates explicit edges; enables GNN message passing | Medium |
| **2** | Multi-View Contrastive Learning | Self-supervised edge weight learning | Medium-High |
| **3** | Relational Graph Attention Network | Learns importance of different relation types | High |

---

## 2. Current State Analysis

### 2.1 What We Have

The Context Graph currently stores **13 embeddings per memory**:

```
Memory Node (TeleologicalFingerprint)
├── E1: Semantic (1024D) - Dense similarity
├── E2: Recency (512D) - Temporal freshness
├── E3: Periodic (512D) - Time-of-day patterns
├── E4: Sequence (512D) - Conversation order
├── E5: Causal (768D×2) - Cause→effect (asymmetric)
├── E6: Sparse (~30K) - Exact keywords
├── E7: Code (1536D) - Code patterns
├── E8: Graph (384D×2) - Structural (asymmetric)
├── E9: HDC (1024D) - Noise-robust
├── E10: Intent (768D) - Goal alignment
├── E11: Entity (768D) - Entity knowledge
├── E12: ColBERT (128D/token) - Phrase precision
└── E13: SPLADE (~30K) - Term expansion
```

### 2.2 Current Retrieval Approach

```
Query → E1 Foundation → Multi-Space RRF → Results
         ↓
    [Compare independently in each space]
    [Fuse rankings via Weighted RRF]
```

**Limitation**: Memories are **not explicitly linked**. Each query computes similarities independently without leveraging pre-computed relationships.

### 2.3 The Gap

| Current State | Potential Enhancement |
|---------------|----------------------|
| Independent similarity search per space | Pre-computed edges enable graph traversal |
| Query-time fusion only | Persistent graph structure for navigation |
| No learned relationships | GNN-learned edge weights and types |
| Flat retrieval | Multi-hop reasoning along edges |

---

## 3. Research Findings

### 3.1 Graph Neural Networks for Knowledge Graphs

Recent research (2024-2025) shows significant advances in using GNNs for knowledge graph enhancement:

**Key Papers:**
- [Enhancing KGE with GNNs (2025)](https://link.springer.com/article/10.1007/s10115-025-02619-8) - Framework incorporating domain-oriented regularizations achieving 90-97% ROC-AUC
- [KSG-GNN](https://link.springer.com/article/10.1007/s11280-024-01320-0) - Data-centric framework based on relational homophily
- [Extended Relational GAT](https://www.sciencedirect.com/science/article/abs/pii/S0957417424021274) - Captures complex hidden information in neighborhoods

**Key Insight**: GNNs leverage the principle that **"connected nodes tend to have similar features"** - in our case, memories that share concepts across multiple embedders should be explicitly linked.

### 3.2 Multi-View Learning

Research on combining multiple embedding views:

**Key Papers:**
- [Disentangled Multi-view GNN (DMGNN)](https://www.sciencedirect.com/science/article/abs/pii/S1568494625009160) - Learns from entity, relation, and triplet views independently to avoid interference
- [MV-HetGNN](https://arxiv.org/abs/2108.13650) - Multi-view representation learning for heterogeneous graphs
- [MFAE](https://dl.acm.org/doi/10.1016/j.knosys.2022.109721) - Multiview feature augmented neural network

**Key Insight**: Different views (embedders) contain **distinct neighborhood knowledge** - learning to combine them requires careful handling to avoid **inter-view interference**.

### 3.3 GraphRAG

[Microsoft's GraphRAG](https://microsoft.github.io/graphrag/) and related work show how knowledge graphs enhance retrieval:

**Key Concepts:**
- Entity linking maps query entities to graph nodes
- Graph traversal broadens retrieval scope
- Multi-hop questions benefit most from structured retrieval
- Community detection creates hierarchical summaries

**Key Insight**: GraphRAG's approach of **building a knowledge graph from content** and using it at query time is directly applicable to our 13-embedder system.

### 3.4 Contrastive Learning for Graphs

Self-supervised approaches for learning graph representations:

**Key Papers:**
- [HeCo](https://dl.acm.org/doi/10.1145/3447548.3467415) - Co-contrastive learning using network schema and meta-path views
- [GraphCL](https://papers.nips.cc/paper/2020/file/3fe230348e9a12c13120749e3f9fa4cd-Paper.pdf) - Graph contrastive learning with augmentations
- [MCCLK](https://www.nature.com/articles/s41598-023-33324-7) - Multi-view contrastive learning across collaborative, semantic, and structural views

**Key Insight**: Contrastive learning can create edge weights by **maximizing agreement between embedder views** without requiring labeled data.

### 3.5 Hypergraph Neural Networks

For modeling higher-order relationships:

**Key Papers:**
- [H2GNN](https://arxiv.org/abs/2412.12158) - Hyperbolic hypergraph neural networks in hyperbolic space
- [Hyper-FM](https://arxiv.org/html/2503.01203v1) - Hypergraph foundation model for multi-domain knowledge extraction
- [MMHCL](https://arxiv.org/html/2504.16576) - Multi-modal hypergraph contrastive learning

**Key Insight**: Hyperedges can connect **multiple memories that share the same concept/topic** rather than just pairwise edges.

---

## 4. Linking Strategies

### 4.1 Strategy 1: K-NN Graph per Embedder

**Concept**: Build explicit k-nearest neighbor graphs for each embedder space, creating 13 separate edge sets.

```
┌─────────────────────────────────────────────────────────────────┐
│                    K-NN GRAPH PER EMBEDDER                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  E1 Space          E7 Space          E11 Space                  │
│  ┌───────┐         ┌───────┐         ┌───────┐                  │
│  │ M1───M3│         │ M1───M5│         │ M2───M4│                │
│  │ │╲  ╱│ │         │ │    │ │         │ │╲  ╱│ │                │
│  │ │ M2 │ │         │ M2───M3│         │ │ M1 │ │                │
│  │ │╱  ╲│ │         │       │ │         │ │╱  ╲│ │                │
│  │ M4───M5│         │ M4    │ │         │ M3───M5│                │
│  └───────┘         └───────┘         └───────┘                  │
│                                                                 │
│  Each embedder creates different neighborhood relationships     │
└─────────────────────────────────────────────────────────────────┘
```

**Implementation**:
```rust
struct EmbedderGraph {
    embedder: EmbedderId,           // E1, E2, ..., E13
    edges: Vec<(Uuid, Uuid, f32)>,  // (source, target, similarity)
    k: usize,                        // neighbors per node
}

// Build k-NN graph using NN-Descent algorithm
fn build_knn_graph(
    embedder: EmbedderId,
    fingerprints: &[TeleologicalFingerprint],
    k: usize,
) -> EmbedderGraph {
    // NN-Descent: "neighbor of neighbor is likely a neighbor"
    // O(n * k * iterations) instead of O(n²)
}
```

**Benefits**:
- Pre-computed edges enable fast graph traversal
- Each embedder's unique perspective preserved
- Enables message passing GNN architectures

**Reference**: [NN-Descent Algorithm](https://www.cs.princeton.edu/cass/papers/www11.pdf)

### 4.2 Strategy 2: Multi-Relation Edge Types

**Concept**: Create different edge types based on which embedders agree on the connection.

```
┌─────────────────────────────────────────────────────────────────┐
│                   MULTI-RELATION EDGES                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Memory A ═══════════════════════════════════════════ Memory B  │
│            │                                       │            │
│            ├── semantic_similar (E1 > 0.7)        │            │
│            ├── code_related (E7 > 0.6)            │            │
│            ├── same_entity (E11 > 0.8)            │            │
│            └── causal_link (E5 > 0.5)             │            │
│                                                                 │
│  Edge Type = f(which embedders agree)                           │
│  Edge Weight = weighted_agreement across embedders              │
└─────────────────────────────────────────────────────────────────┘
```

**Edge Type Taxonomy**:

| Edge Type | Primary Embedder | Threshold | Meaning |
|-----------|------------------|-----------|---------|
| `semantic_similar` | E1 | > 0.7 | General meaning overlap |
| `code_related` | E7 | > 0.6 | Same code pattern/function |
| `entity_shared` | E11 | > 0.8 | Share named entity |
| `causal_chain` | E5 | > 0.5 | Cause→effect relationship |
| `graph_connected` | E8 | > 0.5 | Structural relationship |
| `intent_aligned` | E10 | > 0.6 | Same goal/purpose |
| `keyword_overlap` | E6 | > 0.4 | Exact term matches |
| `temporal_proximate` | E2 | > 0.7 | Recent co-occurrence |

**Implementation**:
```rust
enum EdgeType {
    SemanticSimilar,
    CodeRelated,
    EntityShared,
    CausalChain { direction: CausalDirection },
    GraphConnected { direction: GraphDirection },
    IntentAligned,
    KeywordOverlap,
    TemporalProximate,
    MultiAgreement { embedders: Vec<EmbedderId> },
}

struct TypedEdge {
    source: Uuid,
    target: Uuid,
    edge_type: EdgeType,
    weight: f32,
    embedder_scores: [f32; 13],  // Full breakdown
}
```

### 4.3 Strategy 3: Weighted Agreement Edges

**Concept**: Create edges only when multiple embedders agree, using weighted_agreement as edge weight.

```
┌─────────────────────────────────────────────────────────────────┐
│                  WEIGHTED AGREEMENT EDGES                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Agreement Calculation (per memory pair):                       │
│                                                                 │
│  weighted_agreement = Σ(category_weight × sim_above_threshold)  │
│                                                                 │
│  SEMANTIC (1.0): E1, E5, E6, E7, E10, E12, E13                 │
│  RELATIONAL (0.5): E8, E11                                      │
│  STRUCTURAL (0.5): E9                                           │
│  TEMPORAL (0.0): E2, E3, E4 (excluded from linking)             │
│                                                                 │
│  Create edge if: weighted_agreement >= 2.5 (topic threshold)    │
│  Edge weight = weighted_agreement / 8.5 (normalized)            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Benefits**:
- Reuses existing topic threshold logic
- Edges represent "strongly related" memory pairs
- Naturally sparse (only high-agreement pairs linked)

### 4.4 Strategy 4: Causal & Graph Directed Edges

**Concept**: Use asymmetric embedders (E5, E8) to create directed edges.

```
┌─────────────────────────────────────────────────────────────────┐
│                    DIRECTED EDGE TYPES                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  E5 Causal Edges:                                               │
│  Memory A ─────causes────────→ Memory B                         │
│            (A.e5_as_cause · B.e5_as_effect > threshold)         │
│                                                                 │
│  E8 Graph Edges:                                                │
│  Memory A ─────imports───────→ Memory B                         │
│            (A.e8_as_source · B.e8_as_target > threshold)        │
│                                                                 │
│  Enables:                                                       │
│  - "What caused this error?" → traverse causal edges backward   │
│  - "What does this depend on?" → traverse graph edges forward   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Implementation**:
```rust
struct DirectedEdge {
    source: Uuid,
    target: Uuid,
    relation: DirectedRelation,
    score: f32,
}

enum DirectedRelation {
    Causes,           // E5: source causes target
    CausedBy,         // E5: source caused by target
    Imports,          // E8: source imports target
    ImportedBy,       // E8: source imported by target
}

fn compute_causal_edges(fingerprints: &[TeleologicalFingerprint]) -> Vec<DirectedEdge> {
    let mut edges = Vec::new();
    for a in fingerprints {
        for b in fingerprints {
            if a.id == b.id { continue; }

            // A causes B: A's "as_cause" · B's "as_effect"
            let cause_score = cosine(&a.e5_causal_as_cause, &b.e5_causal_as_effect);
            if cause_score > CAUSAL_THRESHOLD {
                edges.push(DirectedEdge {
                    source: a.id,
                    target: b.id,
                    relation: DirectedRelation::Causes,
                    score: cause_score,
                });
            }
        }
    }
    edges
}
```

---

## 5. Graph Neural Network Approaches

### 5.1 Relational Graph Convolutional Network (R-GCN)

**Concept**: Apply different transformation weights for different edge types (embedder-based relations).

```
┌─────────────────────────────────────────────────────────────────┐
│                    R-GCN ARCHITECTURE                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  For each node i, aggregate neighbors by relation type:         │
│                                                                 │
│  h_i^(l+1) = σ(Σ_r Σ_{j∈N_r(i)} (1/c_{i,r}) W_r^(l) h_j^(l))   │
│                                                                 │
│  Where:                                                         │
│  - r ∈ {semantic, code, entity, causal, ...} (13 relation types)│
│  - W_r^(l) = learned weight matrix per relation type            │
│  - N_r(i) = neighbors of i connected by relation r              │
│  - c_{i,r} = normalization constant                             │
│                                                                 │
│  This learns WHICH embedder relationships matter most!          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Application to 13-Embedder System**:
- Each embedder defines a relation type
- R-GCN learns relation-specific transformation matrices
- Aggregates information from all 13 perspectives

**Reference**: [R-GCN Paper](https://arxiv.org/abs/1703.06103)

### 5.2 Graph Attention Network (GAT)

**Concept**: Learn attention weights over neighbors, potentially conditioned on embedder type.

```
┌─────────────────────────────────────────────────────────────────┐
│                    GAT ARCHITECTURE                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Attention coefficient:                                         │
│  α_ij = softmax_j(LeakyReLU(a^T [W h_i || W h_j]))              │
│                                                                 │
│  Node update:                                                   │
│  h_i' = σ(Σ_{j∈N(i)} α_ij W h_j)                               │
│                                                                 │
│  Multi-head extension (one head per embedder category):         │
│  h_i' = ||_{k=1}^{K} σ(Σ_j α_ij^k W^k h_j)                     │
│                                                                 │
│  K = 4 heads: SEMANTIC, RELATIONAL, STRUCTURAL, TEMPORAL        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Edge-Featured GAT Extension**:
- Include embedder agreement scores as edge features
- Attention conditioned on which embedders agree

**Reference**: [GAT Paper](https://petar-v.com/GAT/)

### 5.3 Message Passing Neural Network (MPNN)

**Concept**: Generic framework for learning on graphs through message passing.

```
┌─────────────────────────────────────────────────────────────────┐
│                    MPNN FRAMEWORK                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. MESSAGE: m_ij = M(h_i, h_j, e_ij)                           │
│     - Compute message from neighbor j to node i                 │
│     - e_ij = edge features (13 embedder similarities)           │
│                                                                 │
│  2. AGGREGATE: m_i = Σ_{j∈N(i)} m_ij                            │
│     - Sum/mean/max over all neighbor messages                   │
│     - Can use attention-weighted aggregation                    │
│                                                                 │
│  3. UPDATE: h_i' = U(h_i, m_i)                                  │
│     - Update node representation with aggregated messages       │
│                                                                 │
│  For 13-embedder system:                                        │
│  - e_ij = [sim_E1, sim_E2, ..., sim_E13] (13D edge features)   │
│  - Message function learns importance of each embedder          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Reference**: [PyTorch Geometric MessagePassing](https://pytorch-geometric.readthedocs.io/en/latest/notes/create_gnn.html)

### 5.4 Hyperbolic GNN for Hierarchical Structure

**Concept**: Embed graph in hyperbolic space to better capture hierarchical relationships.

```
┌─────────────────────────────────────────────────────────────────┐
│                    HYPERBOLIC EMBEDDING                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Euclidean space: Poor at representing hierarchies              │
│  Hyperbolic space: Naturally captures tree-like structure       │
│                                                                 │
│  Memory Graph Hierarchy:                                        │
│                                                                 │
│          [Session]                                              │
│         /    |    \                                             │
│    [Topic] [Topic] [Topic]                                      │
│      /|\     |      /|\                                         │
│   [M1][M2] [M3]  [M4][M5][M6]                                   │
│                                                                 │
│  Hyperbolic distance captures:                                  │
│  - Parent-child (topic → memory)                                │
│  - Sibling relationships (memories in same topic)               │
│  - Cross-topic connections                                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Reference**: [H2GNN Paper](https://arxiv.org/abs/2412.12158)

---

## 6. Multi-View Learning Techniques

### 6.1 Disentangled Multi-View Learning

**Concept**: Keep embedder views separate during learning to avoid interference, fuse only at final stage.

```
┌─────────────────────────────────────────────────────────────────┐
│                DISENTANGLED MULTI-VIEW LEARNING                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  View 1 (Semantic): E1, E10                                     │
│       GNN_semantic(G_semantic) → H_semantic                     │
│                                    ↓                            │
│  View 2 (Code): E7                                              │
│       GNN_code(G_code) → H_code    ↓                            │
│                           ↓        ↓                            │
│  View 3 (Entity): E11             ↓                             │
│       GNN_entity(G_entity) → H_entity                           │
│                               ↓    ↓                            │
│  View 4 (Causal): E5              ↓                             │
│       GNN_causal(G_causal) → H_causal                           │
│                                    ↓                            │
│                            ┌───────┴───────┐                    │
│                            │  FUSION LAYER  │                   │
│                            │  (Attention or │                   │
│                            │   Gating)      │                   │
│                            └───────┬───────┘                    │
│                                    ↓                            │
│                              H_unified                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Benefits**:
- Each view learns independently (no interference)
- Fusion layer learns which views matter per query
- Preserves embedder-specific signals

### 6.2 Contrastive Multi-View Learning

**Concept**: Use contrastive learning to align views and create edge weights.

```
┌─────────────────────────────────────────────────────────────────┐
│              CONTRASTIVE MULTI-VIEW LEARNING                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Positive Pairs:                                                │
│  - Same memory in different embedder views                      │
│  - (M1.E1, M1.E7) should be close                               │
│                                                                 │
│  Negative Pairs:                                                │
│  - Different memories                                           │
│  - (M1.E1, M2.E7) should be distant                             │
│                                                                 │
│  Loss Function:                                                 │
│  L = -log(exp(sim(z_i, z_i') / τ) /                            │
│           Σ_k exp(sim(z_i, z_k) / τ))                           │
│                                                                 │
│  Where:                                                         │
│  - z_i = projection of memory i in view 1 (e.g., E1)           │
│  - z_i' = projection of same memory i in view 2 (e.g., E7)     │
│  - τ = temperature parameter                                    │
│                                                                 │
│  Result: Learned representations where agreement = edge weight  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Self-Supervised Edge Weight Learning**:
- High cross-view agreement → strong edge
- Low agreement → weak or no edge
- No labels required

**Reference**: [HeCo Paper](https://dl.acm.org/doi/10.1145/3447548.3467415)

### 6.3 Cross-Embedder Attention

**Concept**: Learn attention weights across embedder spaces for each memory pair.

```
┌─────────────────────────────────────────────────────────────────┐
│                 CROSS-EMBEDDER ATTENTION                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  For memory pair (A, B), compute 13 similarities:               │
│  S = [sim_E1(A,B), sim_E2(A,B), ..., sim_E13(A,B)]             │
│                                                                 │
│  Learn attention over these similarities:                       │
│  α = softmax(W_attn · S + b_attn)                               │
│                                                                 │
│  Final edge weight:                                             │
│  edge_weight = Σ_i α_i · S_i                                    │
│                                                                 │
│  This learns WHICH embedder similarities matter for linking!    │
│                                                                 │
│  Training signal:                                               │
│  - Topic co-membership (positive)                               │
│  - Random pairs (negative)                                      │
│  - Causal chains (directional positive)                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Implementation Options

### 7.1 Option A: Static K-NN Graph Construction

**Description**: Build k-NN graphs offline for each embedder, store edges in RocksDB.

**Implementation Steps**:
1. For each embedder (E1-E13), run NN-Descent to build k-NN graph
2. Store edges in new column family `embedder_edges`
3. At query time, traverse pre-computed edges for graph-based retrieval

**Storage Schema**:
```rust
// Column Family: embedder_edges
// Key: [embedder_id: u8][source_uuid: 16 bytes]
// Value: [(target_uuid, similarity)]

struct EmbedderEdge {
    source: Uuid,
    target: Uuid,
    embedder: EmbedderId,
    similarity: f32,
}
```

**Complexity**: Medium
**Benefit**: Fast query-time graph traversal
**Drawback**: Static edges, need periodic rebuilds

### 7.2 Option B: Learned Edge Weights with GNN

**Description**: Train a GNN to learn optimal edge weights from cross-embedder similarities.

**Implementation Steps**:
1. Build initial k-NN graph from E1 (foundation)
2. Compute 13-dimensional edge features (all embedder similarities)
3. Train GNN to predict useful links (using topic co-membership as signal)
4. Use learned model to score edges at query time

**Architecture**:
```
Input: Memory nodes + 13D edge features
       ↓
GNN Layer 1: Message passing with edge features
       ↓
GNN Layer 2: Aggregate neighbor information
       ↓
Output: Refined node embeddings + edge scores
```

**Complexity**: High
**Benefit**: Learns optimal embedder weighting
**Drawback**: Requires training infrastructure

### 7.3 Option C: Hypergraph Construction

**Description**: Create hyperedges connecting all memories that share a topic/concept.

**Implementation Steps**:
1. Run topic detection (existing HDBSCAN)
2. Create hyperedge for each topic containing all member memories
3. Hypergraph convolution aggregates within topics

**Structure**:
```
Hyperedge 1 (Topic: "Rust Database")
├── Memory A
├── Memory B
└── Memory C

Hyperedge 2 (Topic: "Error Handling")
├── Memory B  (shared with Hyperedge 1)
├── Memory D
└── Memory E
```

**Complexity**: Medium
**Benefit**: Models higher-order relationships
**Drawback**: Requires good topic detection

### 7.4 Option D: GraphRAG-Style Entity Linking

**Description**: Extract entities from memories, create entity graph, link memories through shared entities.

**Implementation Steps**:
1. Extract entities using E11 (entity embeddings) + NER
2. Create entity nodes in addition to memory nodes
3. Link memories to entities they mention
4. Entity graph enables multi-hop traversal

**Graph Structure**:
```
Memory Nodes ←→ Entity Nodes ←→ Memory Nodes

[Memory: "Diesel ORM setup"] ──mentions──→ [Entity: Diesel]
                                              ↑
[Memory: "Diesel migration"] ──mentions────────┘
```

**Complexity**: Medium-High
**Benefit**: Explicit entity relationships
**Drawback**: Requires entity extraction pipeline

---

## 8. Priority Recommendations

### 8.1 Recommendation Matrix

| Enhancement | Impact | Complexity | GPU Fit | Recommendation |
|-------------|--------|------------|---------|----------------|
| K-NN Graph per Embedder | High | Medium | Excellent | **Priority 1** |
| Multi-Relation Edges | High | Medium | Good | **Priority 2** |
| Contrastive Edge Learning | High | Medium-High | Excellent | **Priority 3** |
| R-GCN Message Passing | Very High | High | Excellent | Priority 4 |
| Hypergraph Convolution | Medium | Medium | Good | Priority 5 |
| GraphRAG Entity Linking | Medium | High | Medium | Priority 6 |

### 8.2 Priority 1: K-NN Graph Construction

**Why First**:
- Foundation for all other enhancements
- Uses existing FAISS/HNSW infrastructure
- Clear implementation path
- Immediate query-time benefit

**Implementation Plan**:
```
Phase 1: Build E1 k-NN graph (k=20)
Phase 2: Build E7 (code), E11 (entity) k-NN graphs
Phase 3: Store edges in RocksDB column family
Phase 4: Add graph traversal to retrieval pipeline
```

### 8.3 Priority 2: Multi-Relation Edge Types

**Why Second**:
- Builds on k-NN graph infrastructure
- Leverages existing embedder categories
- Enables relation-aware retrieval

**Implementation Plan**:
```
Phase 1: Define edge type taxonomy (semantic, code, entity, causal, etc.)
Phase 2: Compute edge types from embedder threshold crossings
Phase 3: Store typed edges with multi-key indexing
Phase 4: Add relation-filtered graph queries
```

### 8.4 Priority 3: Contrastive Edge Weight Learning

**Why Third**:
- Self-supervised (no labels needed)
- GPU-accelerated training
- Learns optimal embedder weighting

**Implementation Plan**:
```
Phase 1: Implement contrastive loss for cross-view pairs
Phase 2: Train projection heads for each embedder category
Phase 3: Use learned similarity for edge weight refinement
Phase 4: Fine-tune on topic co-membership signal
```

---

## 9. Architecture Proposals

### 9.1 Proposed Architecture: Multi-View Graph Memory System

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   MULTI-VIEW GRAPH MEMORY SYSTEM                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    MEMORY INJECTION                              │   │
│  │  Content → 13 Embeddings → TeleologicalFingerprint              │   │
│  │                     ↓                                            │   │
│  │            ┌────────┴────────┐                                   │   │
│  │            ↓                 ↓                                   │   │
│  │     Store Fingerprint   Update K-NN Graphs (13)                 │   │
│  │            ↓                 ↓                                   │   │
│  │     RocksDB CF          Compute New Edges                        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    GRAPH STORAGE LAYER                           │   │
│  │                                                                  │   │
│  │  CF: fingerprints        CF: embedder_edges                     │   │
│  │  ┌─────────────┐         ┌─────────────────────────────────┐    │   │
│  │  │ UUID → FP   │         │ E1: [(A,B,0.85), (A,C,0.72)...] │    │   │
│  │  │ (63KB each) │         │ E7: [(A,D,0.91), (B,D,0.68)...] │    │   │
│  │  └─────────────┘         │ E11: [(B,C,0.88), ...]          │    │   │
│  │                          └─────────────────────────────────┘    │   │
│  │                                                                  │   │
│  │  CF: typed_edges         CF: hyperedges                         │   │
│  │  ┌─────────────────┐     ┌─────────────────────────────────┐    │   │
│  │  │ A→B: semantic   │     │ Topic1: [A, B, C]               │    │   │
│  │  │ A→D: code       │     │ Topic2: [B, D, E]               │    │   │
│  │  │ B→C: entity     │     └─────────────────────────────────┘    │   │
│  │  │ A→C: causal     │                                            │   │
│  │  └─────────────────┘                                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    GRAPH-ENHANCED RETRIEVAL                      │   │
│  │                                                                  │   │
│  │  Query                                                           │   │
│  │    ↓                                                             │   │
│  │  Stage 1: E13 Sparse Recall (existing)                          │   │
│  │    ↓                                                             │   │
│  │  Stage 2: E1 Dense Search (existing)                            │   │
│  │    ↓                                                             │   │
│  │  Stage 3: Graph Expansion ← NEW                                  │   │
│  │    ├── Traverse E7 edges for code queries                       │   │
│  │    ├── Traverse E11 edges for entity queries                    │   │
│  │    ├── Traverse E5 edges for causal queries                     │   │
│  │    └── Follow hyperedges for topic expansion                    │   │
│  │    ↓                                                             │   │
│  │  Stage 4: Multi-View RRF (existing, enhanced)                   │   │
│  │    ↓                                                             │   │
│  │  Stage 5: E12 Rerank (existing)                                 │   │
│  │    ↓                                                             │   │
│  │  Results with Graph Context                                      │   │
│  │    ├── Direct matches                                            │   │
│  │    ├── Graph-connected memories                                  │   │
│  │    └── Multi-hop paths (A → B → C)                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 9.2 New MCP Tools for Graph Navigation

| Tool | Purpose | Graph Feature Used |
|------|---------|-------------------|
| `get_memory_neighbors` | Get k nearest neighbors in specific embedder space | K-NN edges |
| `traverse_causal_chain` | Follow cause→effect links | E5 directed edges |
| `find_code_related` | Find memories with similar code patterns | E7 edges |
| `expand_by_entity` | Find memories sharing entities | E11 edges |
| `get_topic_graph` | Get all memories in topic hyperedge | Hyperedges |
| `multi_hop_search` | Search with graph expansion | All edge types |

### 9.3 GNN Training Pipeline (Future)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    GNN TRAINING PIPELINE (OPTIONAL)                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Training Data:                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Positive Pairs:                                                  │   │
│  │   - Same topic members                                           │   │
│  │   - Causal chains (from E5)                                      │   │
│  │   - Entity co-occurrence (from E11)                              │   │
│  │                                                                  │   │
│  │ Negative Pairs:                                                  │   │
│  │   - Random memory pairs                                          │   │
│  │   - Different topic members                                      │   │
│  │   - Temporally distant (low E2 similarity)                       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Model Architecture:                                                    │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                                                                  │   │
│  │  Input: Node features (13D summary of embeddings)               │   │
│  │         Edge features (13D pairwise similarities)               │   │
│  │              ↓                                                   │   │
│  │  R-GCN Layer 1: Relation-specific message passing               │   │
│  │              ↓                                                   │   │
│  │  R-GCN Layer 2: Aggregate multi-relation messages               │   │
│  │              ↓                                                   │   │
│  │  Output: Refined node embeddings                                │   │
│  │          Edge importance scores                                  │   │
│  │              ↓                                                   │   │
│  │  Loss: Link prediction (predict missing edges)                  │   │
│  │        + Contrastive (align positive pairs)                     │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  Training Loop (GPU):                                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ for epoch in epochs:                                            │   │
│  │     batch = sample_subgraph(memories, edges, batch_size=1024)   │   │
│  │     embeddings = gnn_forward(batch)                             │   │
│  │     loss = link_pred_loss(embeddings) + contrastive_loss(...)   │   │
│  │     loss.backward()                                              │   │
│  │     optimizer.step()                                             │   │
│  │                                                                  │   │
│  │ # Update edge weights based on learned model                    │   │
│  │ new_edge_weights = gnn_predict_edges(all_memories)              │   │
│  │ store_edges(new_edge_weights)                                   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 10. References

### Research Papers

1. **GNN for Knowledge Graphs**
   - [Enhancing KGE with GNNs (2025)](https://link.springer.com/article/10.1007/s10115-025-02619-8)
   - [KSG-GNN Data-Centric Framework](https://link.springer.com/article/10.1007/s11280-024-01320-0)
   - [Extended Relational GAT](https://www.sciencedirect.com/science/article/abs/pii/S0957417424021274)

2. **Multi-View Learning**
   - [Disentangled Multi-view GNN (DMGNN)](https://www.sciencedirect.com/science/article/abs/pii/S1568494625009160)
   - [MV-HetGNN](https://arxiv.org/abs/2108.13650)
   - [Multi-view Contrastive Learning](https://www.nature.com/articles/s41598-023-33324-7)

3. **GraphRAG**
   - [GraphRAG Survey (arXiv)](https://arxiv.org/abs/2501.00309)
   - [Microsoft GraphRAG](https://microsoft.github.io/graphrag/)
   - [Neo4j GraphRAG Guide](https://neo4j.com/blog/genai/what-is-graphrag/)

4. **Contrastive Learning**
   - [HeCo: Self-supervised Heterogeneous GNN](https://dl.acm.org/doi/10.1145/3447548.3467415)
   - [GraphCL Framework](https://papers.nips.cc/paper/2020/file/3fe230348e9a12c13120749e3f9fa4cd-Paper.pdf)

5. **Hypergraph Neural Networks**
   - [H2GNN: Hyperbolic Hypergraph NN](https://arxiv.org/abs/2412.12158)
   - [Hyper-FM Foundation Model](https://arxiv.org/html/2503.01203v1)

6. **Core GNN Architectures**
   - [R-GCN Paper](https://arxiv.org/abs/1703.06103)
   - [GAT Paper](https://petar-v.com/GAT/)
   - [MPNN in PyTorch Geometric](https://pytorch-geometric.readthedocs.io/en/latest/notes/create_gnn.html)

7. **K-NN Graph Construction**
   - [NN-Descent Algorithm](https://www.cs.princeton.edu/cass/papers/www11.pdf)
   - [Neo4j KNN Algorithm](https://neo4j.com/docs/graph-data-science/current/algorithms/knn/)

8. **Knowledge Graph Embeddings**
   - [KGE Survey](https://www.sciencedirect.com/science/article/abs/pii/S1574013724000996)
   - [TransE, DistMult, ComplEx Analysis](https://github.com/tranhungnghiep/AnalyzeKGE)

9. **Adaptive Graph Learning**
   - [Graph Structure Learning Chapter](https://graph-neural-networks.github.io/static/file/chapter14.pdf)
   - [IDGL: Iterative Deep Graph Learning](https://proceedings.neurips.cc/paper/2020/file/e05c7ba4e087beea9410929698dc41a6-Paper.pdf)

---

## Appendix A: Glossary

| Term | Definition |
|------|------------|
| **K-NN Graph** | Graph where each node connects to its k nearest neighbors |
| **NN-Descent** | Algorithm for efficient k-NN graph construction |
| **R-GCN** | Relational Graph Convolutional Network - GNN with relation-specific weights |
| **GAT** | Graph Attention Network - GNN with learned attention over neighbors |
| **MPNN** | Message Passing Neural Network - generic GNN framework |
| **Hypergraph** | Graph where edges (hyperedges) can connect multiple nodes |
| **Contrastive Learning** | Self-supervised learning by contrasting positive/negative pairs |
| **Multi-View Learning** | Learning from multiple representations of the same data |
| **GraphRAG** | Retrieval Augmented Generation enhanced with knowledge graphs |
| **Link Prediction** | Task of predicting missing edges in a graph |

---

## Appendix B: Implementation Checklist

### Phase 1: K-NN Graph Foundation
- [ ] Implement NN-Descent for E1 space
- [ ] Create `embedder_edges` column family
- [ ] Store E1 k-NN edges (k=20)
- [ ] Add `get_memory_neighbors` MCP tool
- [ ] Extend to E7, E11 spaces

### Phase 2: Multi-Relation Edges
- [ ] Define edge type taxonomy
- [ ] Implement edge type classification
- [ ] Create `typed_edges` column family
- [ ] Add relation-filtered queries
- [ ] Update retrieval pipeline with graph expansion

### Phase 3: Contrastive Learning
- [ ] Implement cross-view contrastive loss
- [ ] Create projection heads for embedder categories
- [ ] Train on topic co-membership signal
- [ ] Use learned weights for edge refinement

### Phase 4: GNN Integration (Optional)
- [ ] Implement R-GCN layer in Rust/CUDA
- [ ] Train on link prediction task
- [ ] Integrate GNN inference into retrieval
- [ ] Evaluate impact on retrieval quality

---

*This document provides a research-backed roadmap for enhancing the Context Graph knowledge graph through advanced linking techniques, leveraging all 13 embedders for superior memory navigation and retrieval.*
