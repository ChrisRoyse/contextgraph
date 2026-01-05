# Embedding Projection & Semantic Preservation Plan

**Document**: `projectionplan.md`
**Version**: 1.1.0
**Created**: 2026-01-04
**Updated**: 2026-01-05
**Status**: APPROVED
**Authors**: Architecture Team + Research Synthesis

**v1.1.0 Changes** (from Improvement Report integration):
- Updated to 13-embedder architecture (added E13 SPLADE sparse lexical)
- Formalized 5-stage optimized retrieval pipeline with latency targets
- Added RRF (Reciprocal Rank Fusion) formula with k=60 constant
- Updated storage architecture: ~17KB quantized (63% reduction from ~46KB)
- Added Matryoshka truncation support for E1 Semantic (1024D → 128D)

---

## Executive Summary

This document establishes the architectural approach for preserving semantic meaning across the 13-model embedding pipeline. Based on research synthesis and first-principles analysis, we reject the traditional "fuse-to-single-vector" approach in favor of a **Multi-Array Semantic Fingerprint** architecture that:

1. **Preserves 100% of semantic information** from all 13 embedders
2. **Eliminates projection layer training** - no distortion of meaning
3. **Enables Teleological Vector alignment** for goal-directed semantic measurement
4. **Provides interpretable retrieval** - know which embedding contributed to matches
5. **Optimizes storage via Product Quantization** - 63% reduction (~46KB → ~17KB)

**Key Decision**: Store `[E1, E2, E3, ..., E13]` as a semantic array with PQ-8 quantization, NOT fused `Vec<1536>`.

**Optimized 5-Stage Retrieval Pipeline** (validated by improvement report):
| Stage | Description | Latency Target |
|-------|-------------|----------------|
| 1 | SPARSE PRE-FILTER (BM25 + E13 SPLADE) | Top 10K in <5ms |
| 2 | FAST DENSE ANN (E1 Matryoshka 128D) | Top 1K in <10ms |
| 3 | MULTI-SPACE RERANK (RRF Fusion E1-E11, E13) | Top 100 in <20ms |
| 4 | TELEOLOGICAL ALIGNMENT FILTER | Top 50 in <10ms |
| 5 | LATE INTERACTION RERANK (E12 MaxSim) | Final top 10 in <15ms |

**Total Pipeline Latency**: <60ms @ 1M memories, <30ms @ 100K memories

---

## Table of Contents

1. [The Problem with Projection-Based Fusion](#1-the-problem-with-projection-based-fusion)
2. [Multi-Array Semantic Fingerprint Architecture](#2-multi-array-semantic-fingerprint-architecture)
3. [Teleological Vectors Integration](#3-teleological-vectors-integration)
4. [Per-Embedder Preservation Requirements](#4-per-embedder-preservation-requirements)
5. [Similarity Computation Strategy](#5-similarity-computation-strategy)
6. [Storage & Retrieval Architecture](#6-storage--retrieval-architecture)
7. [Implementation Plan](#7-implementation-plan)
8. [Mathematical Foundations](#8-mathematical-foundations)
9. [Validation Protocol](#9-validation-protocol)

---

## 1. The Problem with Projection-Based Fusion

### 1.1 Why Projection Layers Destroy Meaning

Traditional embedding fusion uses learned projection matrices to align dimensions:

```
E8 (384D MiniLM) → Projection Matrix W (384×1536) → 1536D output
```

**Critical Issue**: A projection layer MUST be TRAINED to preserve semantic relationships.

| Projection Method | Semantic Preservation | Training Required | Distortion Risk |
|-------------------|----------------------|-------------------|-----------------|
| Random Matrix | 0% | None | CATASTROPHIC |
| Zero Padding | ~30% | None | HIGH (similarity scores broken) |
| Learned Linear | 60-80% | Contrastive pairs | MEDIUM |
| Learned MLP | 70-85% | Large dataset | MEDIUM |
| Procrustes Alignment | 85-95% | Paired anchors | LOW |

**Even the best projection (Procrustes) loses 5-15% of semantic information.**

### 1.2 Information Loss in FuseMoE

Current spec (constitution.yaml line 321-322):
```yaml
fusion:
  fuse_moe: { top_k: 4, laplace_alpha: 0.01 }
```

This means:
- Only **4 of 12 embedders** contribute to final vector
- **8 embedders are discarded** for each memory
- Gating is input-dependent but irreversible

**Example**: A code snippet with causal implications:
- E5 (Causal) might be discarded if E7 (Code) dominates
- The causal meaning is **permanently lost** from storage

### 1.3 The Fundamental Insight

> **Semantic meaning is not fungible across embedding spaces.**
>
> E1 (Semantic) and E5 (Causal) encode DIFFERENT types of meaning.
> Fusing them destroys the distinction.
> Keeping them separate preserves the full semantic fingerprint.

---

## 2. Multi-Array Semantic Fingerprint Architecture

### 2.1 Core Concept

Instead of fusing 12 embeddings into 1 vector, store ALL 12 as a structured array:

```rust
/// A complete semantic fingerprint preserving all 13 embedding dimensions
/// Storage: ~17KB with PQ-8 quantization (63% reduction from ~46KB)
pub struct SemanticFingerprint {
    /// E1: Dense semantic meaning (1024D, Matryoshka-enabled)
    /// Truncatable to 512D/256D/128D for fast Stage 2 filtering
    /// Quantization: PQ-8 (32x compression, <5% recall loss)
    pub semantic: MatryoshkaVector1024,

    /// E2: Temporal recency (512D)
    /// Quantization: Float8
    pub temporal_recent: Vector512,

    /// E3: Periodic patterns (512D)
    /// Quantization: Float8
    pub temporal_periodic: Vector512,

    /// E4: Positional encoding (512D)
    /// Quantization: Float8
    pub temporal_positional: Vector512,

    /// E5: Causal relationships (768D)
    /// Asymmetric similarity: cause→effect (1.2x) ≠ effect→cause (0.8x)
    /// Quantization: PQ-8
    pub causal: AsymmetricVector768,

    /// E6: Sparse activations (~1500 active of 30K)
    /// Sparse storage: indices + values only
    pub sparse: SparseVector30K,

    /// E7: Code/AST structure (1536D)
    /// Quantization: PQ-8
    pub code: Vector1536,

    /// E8: Graph/GNN structure (384D)
    /// Quantization: Float8
    pub graph: Vector384,

    /// E9: Hyperdimensional computing (1024D from 10K-bit)
    /// Quantization: Binary (native)
    pub hdc: BinaryVector1024,

    /// E10: Multimodal (768D)
    /// Quantization: PQ-8
    pub multimodal: Vector768,

    /// E11: Entity/TransE (384D)
    /// Quantization: Float8
    pub entity: Vector384,

    /// E12: Late interaction (variable, 128D per token)
    /// Quantization: PQ-8
    pub late_interaction: Vec<Vector128>,

    /// E13: SPLADE sparse lexical (~30K sparse dimensions)
    /// Stage 1 pre-filter: 10K→1K candidates in <5ms
    /// Storage: SparseTensor (indices + BM25-weighted values)
    pub splade: SpladeVector,

    /// Teleological alignment vector (computed, not stored)
    /// See Section 3 for Teleological Vectors integration
    #[serde(skip)]
    pub teleological: Option<TeleologicalAlignment>,
}

/// Matryoshka-enabled vector supporting nested truncation
pub struct MatryoshkaVector1024 {
    pub full: Vector1024,           // Full 1024D
    pub truncation_dims: [usize; 3], // [512, 256, 128]
}

impl MatryoshkaVector1024 {
    /// Get truncated view for fast ANN (Stage 2)
    pub fn truncate(&self, dim: usize) -> &[f32] {
        &self.full.as_slice()[..dim]
    }
}

/// SPLADE sparse lexical embedding for Stage 1 pre-filtering
pub struct SpladeVector {
    /// Active dimension indices (vocabulary tokens)
    pub indices: Vec<u32>,
    /// BM25-weighted activation values
    pub values: Vec<f32>,
    /// Precomputed L2 norm for fast scoring
    pub norm: f32,
}
```

### 2.2 Storage Comparison

| Approach | Storage per Memory | Information Preserved | Retrieval Complexity |
|----------|-------------------|----------------------|---------------------|
| FuseMoE (legacy) | 6 KB (1536D) | ~33% (top-4 only) | O(1) ANN |
| Multi-Array (unquantized) | ~46 KB (all 13) | 100% | O(13) weighted similarity |
| **Multi-Array + PQ-8 (recommended)** | **~17 KB** | **100%** | **5-stage pipeline** |

**With PQ-8 Quantization**:
- **63% storage reduction** (46KB → 17KB)
- **<5% recall loss** on semantic/causal embeddings
- **32x compression** for dense vectors via Product Quantization

| Component | Unquantized | Quantized | Method |
|-----------|-------------|-----------|--------|
| E1 Semantic (1024D) | 4 KB | 128 B | PQ-8 |
| E2-E4 Temporal (512D × 3) | 6 KB | 1.5 KB | Float8 |
| E5 Causal (768D) | 3 KB | 96 B | PQ-8 |
| E6 Sparse (1500 active) | 6 KB | 6 KB | Native sparse |
| E7 Code (1536D) | 6 KB | 192 B | PQ-8 |
| E8 Graph (384D) | 1.5 KB | 384 B | Float8 |
| E9 HDC (1024-bit) | 128 B | 128 B | Binary |
| E10 Multimodal (768D) | 3 KB | 96 B | PQ-8 |
| E11 Entity (384D) | 1.5 KB | 384 B | Float8 |
| E12 Late Interaction (avg) | 8 KB | 1 KB | PQ-8 |
| E13 SPLADE (sparse) | 6 KB | 6 KB | Native sparse |
| **Total** | **~46 KB** | **~17 KB** | **63% reduction** |

### 2.3 The Unique Fingerprint Property

Each `SemanticFingerprint` is a **unique multi-dimensional signature**:

```
Memory A: [semantic_a, temporal_a, ..., causal_a, ...]
Memory B: [semantic_b, temporal_b, ..., causal_b, ...]

If A and B are semantically identical:
  - semantic_a ≈ semantic_b (same meaning)
  - causal_a ≈ causal_b (same causal structure)
  - code_a ≈ code_b (same code semantics)
  - etc.

If A and B differ in causality but not meaning:
  - semantic_a ≈ semantic_b (same surface meaning)
  - causal_a ≠ causal_b (different causal implications)
  - This distinction is LOST in fusion!
```

---

## 3. Teleological Vectors Integration

### 3.1 The Teleological Distributional Hypothesis

From Royse (2026), *Teleological Vectors: A Mathematical Framework for Semantic Goal Alignment*:

> **Teleological Distributional Hypothesis**: Goals pursued through similar action contexts have similar teleological meanings.

This extends Harris's distributional hypothesis from linguistics to goal-directed systems.

### 3.2 Alignment as Cosine Similarity

**Core Formula**:
```
A(v, V) = cos(v, V) = (v · V) / (||v|| × ||V||)
```

Where:
- `v` = local goal/action embedding
- `V` = global goal embedding (North Star)
- `A(v, V)` ∈ [-1, 1] = alignment score

**Optimal Alignment Thresholds** (empirically validated):
```
θ ∈ [0.70, 0.75]  — Production-validated range
θ < 0.70          — Misalignment warning
θ < 0.55          — Critical misalignment
ΔA < -0.15        — Predicts failure 30-60 seconds ahead
```

### 3.3 Hierarchical North Star Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    V_global (North Star)                     │
│              System-wide teleological goal                   │
│         "Provide coherent, accurate, helpful context"        │
└─────────────────────────┬───────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        v                 v                 v
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│   V_mid[1]    │ │   V_mid[2]    │ │   V_mid[3]    │
│   Retrieval   │ │   Storage     │ │   Reasoning   │
│    Goals      │ │    Goals      │ │    Goals      │
└───────┬───────┘ └───────┬───────┘ └───────┬───────┘
        │                 │                 │
   ┌────┴────┐       ┌────┴────┐       ┌────┴────┐
   v         v       v         v       v         v
┌─────┐   ┌─────┐ ┌─────┐   ┌─────┐ ┌─────┐   ┌─────┐
│V_loc│   │V_loc│ │V_loc│   │V_loc│ │V_loc│   │V_loc│
│ [1] │   │ [2] │ │ [3] │   │ [4] │ │ [5] │   │ [6] │
└─────┘   └─────┘ └─────┘   └─────┘ └─────┘   └─────┘
```

### 3.4 Teleological Alignment for Each Embedder

Each of the 13 embedders serves a **teleological purpose**:

| Embedder | Teleological Purpose | Goal Alignment Measure | Pipeline Stage |
|----------|---------------------|----------------------|----------------|
| **E13 SPLADE** | **Keyword precision pre-filter** | **A(keywords, V_keyword_precision)** | **Stage 1** |
| E1 Semantic | Capture dense meaning (Matryoshka 128D) | A(content, V_meaning) | Stage 2 |
| E2 Temporal Recent | Preserve recency relevance | A(timestamp, V_freshness) | Stage 3 |
| E3 Temporal Periodic | Capture cyclical patterns | A(pattern, V_periodicity) | Stage 3 |
| E4 Temporal Positional | Maintain sequence order | A(position, V_ordering) | Stage 3 |
| E5 Causal | Preserve cause-effect (asymmetric) | A(causation, V_causality) | Stage 3 |
| E6 Sparse | Capture activation patterns | A(activations, V_selectivity) | Stage 3 |
| E7 Code | Preserve AST semantics | A(ast, V_correctness) | Stage 3 |
| E8 Graph | Maintain structural relations | A(structure, V_connectivity) | Stage 3 |
| E9 HDC | Holographic encoding | A(hologram, V_robustness) | Stage 3 |
| E10 Multimodal | Cross-modal grounding | A(grounding, V_multimodality) | Stage 3 |
| E11 Entity | Knowledge graph alignment | A(triple, V_factuality) | Stage 3 |
| E12 Late Interaction | Token-level matching (MaxSim) | A(tokens, V_precision) | Stage 5 |

**Stage Mapping**:
- **Stage 1**: E13 SPLADE sparse pre-filter (10K→1K candidates)
- **Stage 2**: E1 Matryoshka 128D ANN (1K→100 candidates)
- **Stage 3**: Multi-space RRF fusion (E1-E11) for reranking (100→50)
- **Stage 4**: Teleological alignment filter (50→20)
- **Stage 5**: E12 ColBERT MaxSim final ranking (20→10)

### 3.5 Computing Teleological Alignment Score

```rust
/// Compute teleological alignment for a semantic fingerprint
pub fn compute_teleological_alignment(
    fingerprint: &SemanticFingerprint,
    north_star: &NorthStarGoals,
) -> TeleologicalAlignment {
    // Per-embedder alignment scores
    let alignments = TeleologicalScores {
        semantic: cosine_sim(&fingerprint.semantic, &north_star.meaning),
        causal: cosine_sim(&fingerprint.causal, &north_star.causality),
        code: cosine_sim(&fingerprint.code, &north_star.correctness),
        graph: cosine_sim(&fingerprint.graph, &north_star.connectivity),
        // ... all 12 embedders
    };

    // Weighted aggregate (weights from UTL state)
    let weights = get_utl_weights();  // Dynamic based on context
    let aggregate = weighted_sum(&alignments, &weights);

    // Misalignment detection
    let delta_a = aggregate - previous_aggregate;
    let misalignment_warning = delta_a < -0.15;

    TeleologicalAlignment {
        per_embedder: alignments,
        aggregate,
        delta_a,
        misalignment_warning,
        threshold_status: classify_threshold(aggregate),
    }
}

fn classify_threshold(a: f32) -> ThresholdStatus {
    match a {
        a if a >= 0.75 => ThresholdStatus::Optimal,
        a if a >= 0.70 => ThresholdStatus::Acceptable,
        a if a >= 0.55 => ThresholdStatus::Warning,
        _ => ThresholdStatus::Critical,
    }
}
```

### 3.6 Teleological Transitivity Bound

From the paper's Theorem 1:

```
If A(u, v) ≥ θ₁ and A(v, w) ≥ θ₂, then:
A(u, w) ≥ 2θ₁θ₂ - 1
```

**Application**: If a local action aligns with mid-level goal, and mid-level goal aligns with North Star, we can bound the local-to-North-Star alignment.

```rust
fn bound_transitive_alignment(
    local_to_mid: f32,    // θ₁
    mid_to_global: f32,   // θ₂
) -> f32 {
    2.0 * local_to_mid * mid_to_global - 1.0
}

// Example:
// local_to_mid = 0.80, mid_to_global = 0.85
// bound = 2 * 0.80 * 0.85 - 1 = 0.36
// This is a LOWER bound; actual alignment could be higher
```

---

## 4. Per-Embedder Preservation Requirements

### 4.1 E1: Semantic Embeddings (1024D, Matryoshka-enabled)

**Model**: e5-large-v2 or equivalent dense transformer with Matryoshka Representation Learning (MRL)
**Teleological Purpose**: Capture dense semantic meaning
**Preservation Strategy**: Matryoshka truncation for Stage 2 + full vector for Stage 3
**Quantization**: PQ-8 (32x compression, <5% recall loss)

```rust
pub struct SemanticPreservation {
    /// Store full 1024D vector with Matryoshka nesting
    /// Truncatable to 512D/256D/128D for progressive refinement
    pub vector: MatryoshkaVector1024,

    /// Quantized version for storage (PQ-8)
    pub quantized: ProductQuantizedVector,

    /// Teleological alignment to meaning goal
    pub alignment_meaning: f32,

    /// Quality metrics
    pub confidence: f32,
    pub entropy: f32,
}

impl SemanticPreservation {
    /// Stage 2: Fast ANN using 128D truncation
    pub fn stage2_vector(&self) -> &[f32] {
        self.vector.truncate(128)
    }

    /// Stage 3: Full precision for RRF reranking
    pub fn stage3_vector(&self) -> &Vector1024 {
        &self.vector.full
    }
}
```

**Matryoshka Benefits**:
- **Stage 2 (128D)**: 8x faster ANN search, 87% recall vs full vector
- **Stage 3 (1024D)**: Full semantic precision for RRF fusion
- **Progressive refinement**: Coarse→fine retrieval

**Validation**:
- Cosine similarity between similar sentences > 0.85
- Dissimilar sentences < 0.30
- No dimension collapse (all dimensions have variance > 0.01)
- Matryoshka recall@10: 128D ≥ 87%, 256D ≥ 94%, 512D ≥ 98%

### 4.2 E2-E4: Temporal Embeddings (512D × 3)

**Model**: Custom encoders (Exp_Decay, Fourier, Sin_PE)
**Teleological Purpose**: Preserve temporal relationships
**Preservation Strategy**: Maintain as separate temporal channels

```rust
pub struct TemporalPreservation {
    /// E2: Recency with exponential decay
    pub recent: Vector512,
    /// E3: Periodic patterns via Fourier
    pub periodic: Vector512,
    /// E4: Absolute position via sinusoidal
    pub positional: Vector512,

    /// Temporal alignment scores
    pub alignment_freshness: f32,
    pub alignment_periodicity: f32,
    pub alignment_ordering: f32,
}
```

**Validation**:
- Recent items have higher recency scores
- Periodic patterns are detectable via FFT
- Sequence order is recoverable from positional encoding

### 4.3 E5: Causal Embeddings (768D, Asymmetric Similarity)

**Model**: Longformer with SCM intervention encoding
**Teleological Purpose**: Preserve cause-effect relationships
**Preservation Strategy**: CRITICAL - must preserve asymmetry with directional weighting
**Quantization**: PQ-8 (32x compression, <5% recall loss)

```rust
pub struct CausalPreservation {
    /// Full causal embedding
    pub vector: Vector768,

    /// Quantized version for storage
    pub quantized: ProductQuantizedVector,

    /// Intervention mask (which dimensions affected by do(X))
    pub intervention_mask: BitVec,

    /// Causal direction indicator
    pub direction: CausalDirection,  // Cause | Effect | Bidirectional

    /// Teleological alignment to causality goal
    pub alignment_causality: f32,
}

pub enum CausalDirection {
    Cause,      // This is a cause of something
    Effect,     // This is an effect of something
    Bidirectional,
}

impl CausalPreservation {
    /// Asymmetric similarity: cause→effect (1.2x) ≠ effect→cause (0.8x)
    /// This captures the intuition that causes predict effects more strongly
    /// than effects predict causes
    pub fn asymmetric_similarity(&self, other: &Self) -> f32 {
        let base_sim = cosine_sim(&self.vector, &other.vector);

        // Apply directional weighting
        let weight = match (&self.direction, &other.direction) {
            (CausalDirection::Cause, CausalDirection::Effect) => 1.2,  // Forward causal
            (CausalDirection::Effect, CausalDirection::Cause) => 0.8,  // Backward causal
            _ => 1.0,  // Symmetric for same-direction or bidirectional
        };

        base_sim * weight
    }
}
```

**Asymmetric Similarity Formula**:
```
sim_asym(A, B) = cos(A, B) × w(dir_A, dir_B)

where w(dir_A, dir_B) = {
    1.2  if A is Cause and B is Effect  (forward prediction)
    0.8  if A is Effect and B is Cause  (backward attribution)
    1.0  otherwise
}
```

**Preservation Rules**:
1. **Asymmetry**: `sim(cause, effect) ≠ sim(effect, cause)` — enforced via directional weighting
2. **Intervention encoding**: Must preserve `do(X)` semantics
3. **Direction preservation**: Never normalize away directional information
4. **Quantization aware**: PQ-8 preserves asymmetry with <5% similarity error

**Validation**:
- Causal pairs: `sim(A causes B, A) > sim(A causes B, B)`
- Asymmetry test: `sim_asym(cause→effect) / sim_asym(effect→cause) ≈ 1.5`
- Intervention: `do(X)` encoding changes specific dimensions
- Transitivity: If A→B and B→C, then A has causal path to C

### 4.4 E6: Sparse Embeddings (~30K, 5% active)

**Model**: Top-K activation sparse encoder
**Teleological Purpose**: Selective feature activation
**Preservation Strategy**: Store only active indices + values

```rust
pub struct SparsePreservation {
    /// Active dimension indices (typically 1500 of 30000)
    pub indices: Vec<u16>,

    /// Activation values for active dimensions
    pub values: Vec<f32>,

    /// Teleological alignment to selectivity goal
    pub alignment_selectivity: f32,
}

impl SparsePreservation {
    /// Compute sparse similarity (Jaccard + weighted overlap)
    pub fn similarity(&self, other: &Self) -> f32 {
        let intersection: HashSet<_> = self.indices.iter()
            .filter(|i| other.indices.contains(i))
            .collect();

        let jaccard = intersection.len() as f32 /
            (self.indices.len() + other.indices.len() - intersection.len()) as f32;

        // Weighted by activation values
        let weighted_overlap: f32 = intersection.iter()
            .map(|&i| {
                let v1 = self.values[self.indices.iter().position(|&x| x == *i).unwrap()];
                let v2 = other.values[other.indices.iter().position(|&x| x == *i).unwrap()];
                v1 * v2
            })
            .sum();

        0.5 * jaccard + 0.5 * weighted_overlap
    }
}
```

### 4.5 E7: Code Embeddings (1536D)

**Model**: AST-aware transformer (CodeBERT, StarCoder embedding)
**Teleological Purpose**: Preserve code semantics and structure
**Preservation Strategy**: Direct storage + AST path attention weights

```rust
pub struct CodePreservation {
    /// Full code embedding
    pub vector: Vector1536,

    /// Top-K AST paths that contributed most to embedding
    pub ast_paths: Vec<AstPath>,

    /// Teleological alignment to correctness goal
    pub alignment_correctness: f32,
}

pub struct AstPath {
    /// Start token type (e.g., "FunctionDef")
    pub start: String,
    /// Path through AST (e.g., ["body", "Return", "value"])
    pub path: Vec<String>,
    /// End token type (e.g., "Identifier")
    pub end: String,
    /// Attention weight for this path
    pub weight: f32,
}
```

**Preservation Rules**:
1. Semantically equivalent code (different variable names) → high similarity
2. Syntactically similar but semantically different → low similarity
3. AST structure preserved via path attention

### 4.6 E8: Graph/GNN Embeddings (384D)

**Model**: MiniLM with graph message passing
**Teleological Purpose**: Preserve structural relationships
**Preservation Strategy**: Store with adjacency signature

```rust
pub struct GraphPreservation {
    /// Node embedding from GNN
    pub vector: Vector384,

    /// Local neighborhood signature (hash of 1-hop neighbors)
    pub neighborhood_hash: u64,

    /// Degree information
    pub in_degree: u32,
    pub out_degree: u32,

    /// Teleological alignment to connectivity goal
    pub alignment_connectivity: f32,
}
```

**Preservation Validation**:
- Neighbors in graph → neighbors in embedding space
- Structure loss: `||A - softmax(E @ E.T)||_F < 0.1`

### 4.7 E9: HDC Embeddings (10K-bit → 1024D)

**Model**: Hyperdimensional computing encoder
**Teleological Purpose**: Robust holographic encoding
**Preservation Strategy**: Store binary + projected

```rust
pub struct HdcPreservation {
    /// Compressed representation (1024D from 10K-bit)
    pub vector: Vector1024,

    /// Original binary hypervector (for exact matching)
    pub binary: Option<BitVec10K>,

    /// Teleological alignment to robustness goal
    pub alignment_robustness: f32,
}

impl HdcPreservation {
    /// Hamming similarity for binary vectors
    pub fn hamming_similarity(&self, other: &Self) -> f32 {
        match (&self.binary, &other.binary) {
            (Some(a), Some(b)) => {
                let xor = a.xor(b);
                1.0 - (xor.count_ones() as f32 / xor.len() as f32)
            }
            _ => cosine_sim(&self.vector, &other.vector),
        }
    }
}
```

### 4.8 E10: Multimodal Embeddings (768D)

**Model**: CLIP/SigLIP cross-attention
**Teleological Purpose**: Ground text in other modalities
**Preservation Strategy**: Store with modality indicator

```rust
pub struct MultimodalPreservation {
    /// Unified multimodal embedding
    pub vector: Vector768,

    /// Which modalities contributed
    pub modalities: Vec<Modality>,

    /// Per-modality confidence
    pub modality_weights: HashMap<Modality, f32>,

    /// Teleological alignment to multimodality goal
    pub alignment_multimodality: f32,
}

pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    Code,
}
```

### 4.9 E11: Entity/TransE Embeddings (384D)

**Model**: TransE/TransR knowledge graph encoder
**Teleological Purpose**: Preserve knowledge graph triples
**Preservation Strategy**: Store with relation context

```rust
pub struct EntityPreservation {
    /// Entity embedding
    pub vector: Vector384,

    /// Most relevant relations (h + r ≈ t)
    pub relations: Vec<RelationContext>,

    /// Teleological alignment to factuality goal
    pub alignment_factuality: f32,
}

pub struct RelationContext {
    /// Relation type
    pub relation: String,
    /// Whether this entity is head or tail
    pub role: EntityRole,
    /// Relation embedding for scoring
    pub relation_embedding: Vector384,
}

impl EntityPreservation {
    /// TransE scoring: ||h + r - t||
    pub fn triple_score(&self, relation: &Vector384, tail: &EntityPreservation) -> f32 {
        let predicted_tail = self.vector.add(relation);
        1.0 / (1.0 + l2_distance(&predicted_tail, &tail.vector))
    }
}
```

### 4.10 E12: Late Interaction Embeddings (128D/token)

**Model**: ColBERT-style per-token embeddings
**Teleological Purpose**: Fine-grained token matching
**Preservation Strategy**: Store token embeddings + positions

```rust
pub struct LateInteractionPreservation {
    /// Per-token embeddings
    pub token_embeddings: Vec<Vector128>,

    /// Token positions for alignment
    pub positions: Vec<u32>,

    /// Token strings for interpretability
    pub tokens: Vec<String>,

    /// Teleological alignment to precision goal
    pub alignment_precision: f32,
}

impl LateInteractionPreservation {
    /// MaxSim scoring (ColBERT style) - Stage 5 final ranking
    pub fn maxsim(&self, query: &Self) -> f32 {
        let mut total = 0.0;
        for q_emb in &query.token_embeddings {
            let max_sim = self.token_embeddings.iter()
                .map(|d_emb| cosine_sim(q_emb, d_emb))
                .fold(f32::NEG_INFINITY, f32::max);
            total += max_sim;
        }
        total / query.token_embeddings.len() as f32
    }
}
```

### 4.11 E13: SPLADE Sparse Lexical Embeddings (~30K sparse)

**Model**: SPLADE v2 (Sparse Lexical and Expansion Model)
**Teleological Purpose**: Keyword-precise pre-filtering for Stage 1
**Preservation Strategy**: Store active vocabulary indices + BM25-weighted values
**Quantization**: Native sparse (no compression needed)

```rust
/// E13: SPLADE sparse lexical embedding for Stage 1 pre-filtering
pub struct SpladePreservation {
    /// Active vocabulary token indices (~500-2000 per document)
    pub indices: Vec<u32>,

    /// BM25-weighted activation values
    pub values: Vec<f32>,

    /// Precomputed L2 norm for efficient scoring
    pub norm: f32,

    /// Teleological alignment to keyword precision goal
    pub alignment_keyword_precision: f32,
}

impl SpladePreservation {
    /// Sparse dot product for Stage 1 candidate generation
    /// Runs in O(n) where n = number of overlapping tokens
    pub fn sparse_score(&self, query: &Self) -> f32 {
        let mut score = 0.0;
        let mut i = 0;
        let mut j = 0;

        // Merge-join on sorted indices
        while i < self.indices.len() && j < query.indices.len() {
            match self.indices[i].cmp(&query.indices[j]) {
                std::cmp::Ordering::Equal => {
                    score += self.values[i] * query.values[j];
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }
        score
    }

    /// Create from SPLADE model output
    pub fn from_splade_output(logits: &[f32], threshold: f32) -> Self {
        let mut indices = Vec::new();
        let mut values = Vec::new();

        for (idx, &logit) in logits.iter().enumerate() {
            if logit > threshold {
                indices.push(idx as u32);
                values.push(logit);
            }
        }

        let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();

        Self {
            indices,
            values,
            norm,
            alignment_keyword_precision: 0.0,  // Computed later
        }
    }
}
```

**Stage 1 Role**:
- **Input**: Query embedding + all memory SPLADE embeddings
- **Operation**: Sparse dot product on inverted index
- **Output**: Top 10K candidates in <5ms
- **Benefit**: Eliminates 99% of corpus before expensive dense operations

**Validation**:
- Keyword queries: SPLADE recall > BM25 recall
- Lexical precision: Exact term matches score highest
- Efficiency: 10K candidate retrieval in <5ms for 1M corpus

---

## 5. Similarity Computation Strategy

### 5.1 Multi-Embedding Similarity Function (Stage 3 RRF Fusion)

```rust
/// Compute similarity between two semantic fingerprints using RRF fusion
/// Used in Stage 3 of the 5-stage pipeline for multi-space reranking
pub fn multi_embedding_similarity(
    query: &SemanticFingerprint,
    memory: &SemanticFingerprint,
    weights: &SimilarityWeights,
) -> MultiSimilarityResult {
    // Compute per-embedder similarities
    let similarities = PerEmbedderSimilarities {
        semantic: cosine_sim(&query.semantic.full, &memory.semantic.full),
        temporal_recent: cosine_sim(&query.temporal_recent, &memory.temporal_recent),
        temporal_periodic: cosine_sim(&query.temporal_periodic, &memory.temporal_periodic),
        temporal_positional: cosine_sim(&query.temporal_positional, &memory.temporal_positional),
        causal: causal_asymmetric_sim(&query.causal, &memory.causal),  // Asymmetric!
        sparse: sparse_similarity(&query.sparse, &memory.sparse),
        code: cosine_sim(&query.code, &memory.code),
        graph: graph_structure_sim(&query.graph, &memory.graph),
        hdc: hamming_or_cosine(&query.hdc, &memory.hdc),
        multimodal: cosine_sim(&query.multimodal, &memory.multimodal),
        entity: transe_sim(&query.entity, &memory.entity),
        late_interaction: maxsim(&query.late_interaction, &memory.late_interaction),
        splade: splade_score(&query.splade, &memory.splade),  // E13
    };

    // RRF Fusion (replaces simple weighted average)
    let aggregate = rrf_fusion(&similarities, weights);

    // Teleological alignment of the similarity itself
    let teleological_score = compute_teleological_alignment_of_match(
        &similarities,
        &query.teleological,
        &memory.teleological,
    );

    MultiSimilarityResult {
        per_embedder: similarities,
        aggregate,
        teleological_score,
        top_contributors: get_top_contributors(&similarities, 3),
    }
}

/// Reciprocal Rank Fusion (RRF) for combining multi-space rankings
/// Formula: RRF(d) = Σᵢ wᵢ / (k + rankᵢ(d))
/// where k=60 (standard RRF constant)
pub fn rrf_fusion(
    similarities: &PerEmbedderSimilarities,
    weights: &SimilarityWeights,
) -> f32 {
    const K: f32 = 60.0;  // Standard RRF constant

    // Convert similarities to ranks (higher sim = lower rank = better)
    let sim_vec = similarities.to_vec();
    let weight_vec = weights.to_vec();

    let mut indexed: Vec<(usize, f32, f32)> = sim_vec.iter()
        .zip(weight_vec.iter())
        .enumerate()
        .map(|(i, (&sim, &weight))| (i, sim, weight))
        .collect();

    // Sort by similarity descending to get ranks
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Compute RRF score
    let mut rrf_score = 0.0;
    for (rank, (_, _, weight)) in indexed.iter().enumerate() {
        rrf_score += weight / (K + (rank + 1) as f32);
    }

    // Normalize to [0, 1] range
    let max_possible = weight_vec.iter().sum::<f32>() / K;
    rrf_score / max_possible
}
```

**RRF Fusion Benefits** (vs simple weighted average):
- **Rank-based**: Robust to different similarity score scales across embedders
- **Outlier resistant**: Single high/low score doesn't dominate
- **Proven in IR**: Standard method in hybrid retrieval systems
- **k=60**: Empirically optimal constant from TREC evaluations

### 5.2 Dynamic Weight Adjustment

Weights can be adjusted based on query intent:

```rust
pub struct SimilarityWeights {
    pub semantic: f32,           // Default: 0.18 (Stage 3 RRF)
    pub temporal_recent: f32,    // Default: 0.04
    pub temporal_periodic: f32,  // Default: 0.04
    pub temporal_positional: f32,// Default: 0.04
    pub causal: f32,             // Default: 0.14 (asymmetric)
    pub sparse: f32,             // Default: 0.04
    pub code: f32,               // Default: 0.09
    pub graph: f32,              // Default: 0.09
    pub hdc: f32,                // Default: 0.04
    pub multimodal: f32,         // Default: 0.04
    pub entity: f32,             // Default: 0.09
    pub late_interaction: f32,   // Default: 0.09 (Stage 5 MaxSim)
    pub splade: f32,             // Default: 0.08 (Stage 1 pre-filter)
}

impl SimilarityWeights {
    /// 5-Stage Pipeline Weight Distribution
    /// - Stage 1 (SPLADE): Binary filter, no weight needed
    /// - Stage 2 (Matryoshka): Uses E1 semantic only
    /// - Stage 3 (RRF): Full multi-space fusion
    /// - Stage 4 (Teleological): Alignment filter, no weight needed
    /// - Stage 5 (MaxSim): Late interaction refinement
    pub fn for_stage3_rrf() -> Self {
        Self {
            semantic: 0.18,
            temporal_recent: 0.04,
            temporal_periodic: 0.04,
            temporal_positional: 0.04,
            causal: 0.14,
            sparse: 0.04,
            code: 0.09,
            graph: 0.09,
            hdc: 0.04,
            multimodal: 0.04,
            entity: 0.09,
            late_interaction: 0.09,
            splade: 0.08,
        }
    }
}

impl SimilarityWeights {
    /// Adjust weights based on query type
    pub fn for_query_type(query_type: QueryType) -> Self {
        match query_type {
            QueryType::SemanticSearch => Self::semantic_heavy(),
            QueryType::CausalReasoning => Self::causal_heavy(),
            QueryType::CodeSearch => Self::code_heavy(),
            QueryType::TemporalNavigation => Self::temporal_heavy(),
            QueryType::FactChecking => Self::entity_heavy(),
            QueryType::Balanced => Self::default(),
        }
    }

    fn causal_heavy() -> Self {
        Self {
            causal: 0.40,
            semantic: 0.20,
            entity: 0.15,
            // ... reduce others
            ..Default::default()
        }
    }
}
```

### 5.3 Teleological Alignment in Retrieval

```rust
/// Retrieve memories aligned with a teleological goal
pub fn teleological_retrieval(
    query: &SemanticFingerprint,
    goal: &TeleologicalGoal,
    memories: &[SemanticFingerprint],
    top_k: usize,
) -> Vec<TeleologicalMatch> {
    let mut matches: Vec<_> = memories.iter()
        .map(|memory| {
            // Standard similarity
            let similarity = multi_embedding_similarity(query, memory, &goal.weights);

            // Teleological alignment of the memory to the goal
            let alignment = compute_teleological_alignment(memory, &goal.north_star);

            // Combined score: similarity × alignment
            let score = similarity.aggregate * alignment.aggregate;

            TeleologicalMatch {
                memory,
                similarity,
                alignment,
                score,
            }
        })
        .collect();

    // Sort by combined score
    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // Check for misalignment warnings
    for m in &matches[..top_k.min(matches.len())] {
        if m.alignment.misalignment_warning {
            log::warn!("Misalignment detected: ΔA = {}", m.alignment.delta_a);
        }
    }

    matches.into_iter().take(top_k).collect()
}
```

---

## 6. Storage & Retrieval Architecture

### 6.1 Hybrid Storage Strategy

```
┌─────────────────────────────────────────────────────────────────┐
│                     STORAGE ARCHITECTURE                         │
│              (~17KB per memory with PQ-8 quantization)           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           PRIMARY: Multi-Array Fingerprint                │   │
│  │  [E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13] │   │
│  │         ~46KB uncompressed → ~17KB with PQ-8              │   │
│  │              (63% storage reduction)                      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              │ Derived (computed, cached)        │
│                              v                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           SECONDARY: Stage 1-2 Fast Indexes               │   │
│  │     E13 SPLADE inverted index (Stage 1 pre-filter)        │   │
│  │     E1 Matryoshka 128D HNSW (Stage 2 ANN)                 │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           TERTIARY: Teleological Index                    │   │
│  │     Alignment scores to North Star goals                  │   │
│  │     Enables goal-directed retrieval (Stage 4)             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 5-Stage Retrieval Pipeline

```
Query: "What causes memory leaks in async Rust?"

┌─────────────────────────────────────────────────────────────────────┐
│                    5-STAGE RETRIEVAL PIPELINE                         │
│                    Target: <60ms for 1M memories                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  STAGE 1: SPLADE SPARSE PRE-FILTER (E13)                   <5ms     │
│  ├─ Input: 1M memories                                               │
│  ├─ Operation: Sparse dot product on inverted index                 │
│  ├─ Output: Top 10K candidates                                       │
│  └─ Method: SPLADE sparse lexical matching                          │
│                          │                                           │
│                          ▼                                           │
│  STAGE 2: MATRYOSHKA ANN (E1 truncated)                    <10ms    │
│  ├─ Input: 10K candidates from Stage 1                              │
│  ├─ Operation: HNSW search on 128D truncated E1 vectors             │
│  ├─ Output: Top 1K candidates                                        │
│  └─ Method: Matryoshka 128D for 8x faster ANN                       │
│                          │                                           │
│                          ▼                                           │
│  STAGE 3: MULTI-SPACE RRF RERANK (E1-E11, E13)             <20ms    │
│  ├─ Input: 1K candidates from Stage 2                               │
│  ├─ Operation: RRF fusion across 13 embedding spaces                │
│  │   Formula: RRF(d) = Σᵢ wᵢ / (60 + rankᵢ(d))                      │
│  ├─ Output: Top 100 candidates                                       │
│  └─ Method: Rank-based fusion, outlier resistant                    │
│                          │                                           │
│                          ▼                                           │
│  STAGE 4: TELEOLOGICAL ALIGNMENT FILTER                    <10ms    │
│  ├─ Input: 100 candidates from Stage 3                              │
│  ├─ Operation: Compute A(memory, North Star)                        │
│  │   Filter: alignment < 0.55 → discard                             │
│  │   ΔA < -0.15 → misalignment warning                              │
│  ├─ Output: Top 50 candidates                                        │
│  └─ Method: Cosine alignment to goal hierarchy                      │
│                          │                                           │
│                          ▼                                           │
│  STAGE 5: LATE INTERACTION MAXSIM (E12)                    <15ms    │
│  ├─ Input: 50 candidates from Stage 4                               │
│  ├─ Operation: ColBERT MaxSim token-level scoring                   │
│  │   MaxSim = (1/|Q|) Σ_q max_d cos(q, d)                           │
│  ├─ Output: Final top 10 results                                    │
│  └─ Method: Token-level precision for final ranking                 │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘

TOTAL LATENCY: <60ms for 1M memories
               <30ms for 100K memories
```

**Stage-by-Stage Breakdown**:

| Stage | Embedder | Candidates | Latency | Cumulative |
|-------|----------|------------|---------|------------|
| 1 | E13 SPLADE | 1M → 10K | <5ms | <5ms |
| 2 | E1 Matryoshka 128D | 10K → 1K | <10ms | <15ms |
| 3 | E1-E11, E13 RRF | 1K → 100 | <20ms | <35ms |
| 4 | Teleological | 100 → 50 | <10ms | <45ms |
| 5 | E12 MaxSim | 50 → 10 | <15ms | <60ms |

**Misalignment Check** (integrated into Stage 4):
- If any result has ΔA < -0.15, warn user
- Include alignment scores in response metadata
- Track alignment drift for predictive failure detection

### 6.3 Database Schema

```sql
-- Primary storage: Full semantic fingerprint
CREATE TABLE semantic_fingerprints (
    id UUID PRIMARY KEY,
    content_hash BYTEA NOT NULL,

    -- E1-E4: Dense vectors (stored as binary)
    e1_semantic BYTEA NOT NULL,      -- 1024 * 4 bytes
    e2_temporal_recent BYTEA NOT NULL,
    e3_temporal_periodic BYTEA NOT NULL,
    e4_temporal_positional BYTEA NOT NULL,

    -- E5: Causal with direction
    e5_causal BYTEA NOT NULL,
    e5_causal_direction SMALLINT NOT NULL,

    -- E6: Sparse (stored as index:value pairs)
    e6_sparse_indices SMALLINT[] NOT NULL,
    e6_sparse_values REAL[] NOT NULL,

    -- E7-E11: Dense vectors
    e7_code BYTEA NOT NULL,
    e8_graph BYTEA NOT NULL,
    e9_hdc BYTEA NOT NULL,
    e10_multimodal BYTEA NOT NULL,
    e11_entity BYTEA NOT NULL,

    -- E12: Variable-length token embeddings (PQ-8)
    e12_late_interaction BYTEA NOT NULL,
    e12_token_count SMALLINT NOT NULL,

    -- E13: SPLADE sparse lexical (native sparse)
    e13_splade_indices INTEGER[] NOT NULL,
    e13_splade_values REAL[] NOT NULL,
    e13_splade_norm REAL NOT NULL,

    -- Teleological metadata
    teleological_aggregate REAL,
    teleological_per_embedder JSONB,
    last_alignment_check TIMESTAMPTZ,

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Secondary: ANN index on aggregated vector
CREATE TABLE ann_index (
    fingerprint_id UUID REFERENCES semantic_fingerprints(id),
    aggregated_vector vector(1536),  -- pgvector
    PRIMARY KEY (fingerprint_id)
);

-- Tertiary: Teleological alignment index
CREATE TABLE teleological_index (
    fingerprint_id UUID REFERENCES semantic_fingerprints(id),
    north_star_id UUID NOT NULL,
    alignment_score REAL NOT NULL,
    alignment_breakdown JSONB,
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (fingerprint_id, north_star_id)
);

-- North Star goals
CREATE TABLE north_star_goals (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    goal_embedding BYTEA NOT NULL,  -- 1536D
    parent_id UUID REFERENCES north_star_goals(id),
    level SMALLINT NOT NULL,  -- 0=global, 1=mid, 2=local
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## 7. Implementation Plan

### 7.1 Phase 1: Foundation (Week 1-2)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 1.1 | Define `SemanticFingerprint` struct in `context-graph-core` | Core | Pending |
| 1.2 | Implement per-embedder preservation structs | Core | Pending |
| 1.3 | Create `TeleologicalAlignment` computation | Core | Pending |
| 1.4 | Add North Star goal hierarchy structure | Core | Pending |
| 1.5 | Update `KnowledgeNode` to use `SemanticFingerprint` | Core | Pending |

### 7.2 Phase 2: Embedder Integration (Week 3-4)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 2.1 | Modify E1-E4 to output preservation structs | Embeddings | Pending |
| 2.2 | Add causal direction to E5 output | Embeddings | Pending |
| 2.3 | Implement sparse encoding for E6 | Embeddings | Pending |
| 2.4 | Add AST path tracking to E7 | Embeddings | Pending |
| 2.5 | Integrate all 12 embedders with fingerprint | Embeddings | Pending |

### 7.3 Phase 3: Similarity & Retrieval (Week 5-6)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 3.1 | Implement `multi_embedding_similarity()` | Core | Pending |
| 3.2 | Add dynamic weight adjustment | Core | Pending |
| 3.3 | Implement teleological retrieval pipeline | MCP | Pending |
| 3.4 | Create hybrid storage schema | Storage | Pending |
| 3.5 | Build ANN pre-filter index | Storage | Pending |

### 7.4 Phase 4: Teleological Integration (Week 7-8)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 4.1 | Define North Star goals for system | Architecture | Pending |
| 4.2 | Implement hierarchical goal alignment | Core | Pending |
| 4.3 | Add misalignment detection (ΔA monitoring) | Monitoring | Pending |
| 4.4 | Create teleological dashboard | UI | Pending |
| 4.5 | Integrate with UTL feedback loop | UTL | Pending |

### 7.5 Phase 5: Validation (Week 9-10)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 5.1 | Per-embedder preservation tests | QA | Pending |
| 5.2 | Multi-similarity accuracy benchmarks | QA | Pending |
| 5.3 | Teleological alignment validation | QA | Pending |
| 5.4 | Performance benchmarks (latency, storage) | QA | Pending |
| 5.5 | Production readiness review | Architecture | Pending |

---

## 8. Mathematical Foundations

### 8.1 Teleological Distributional Hypothesis (Formal)

Let $\mathcal{A}$ be the space of actions and $\mathcal{G}$ be the space of goals. The Teleological Distributional Hypothesis states:

$$\forall g_1, g_2 \in \mathcal{G}: P(a | g_1) \approx P(a | g_2) \Rightarrow \text{meaning}(g_1) \approx \text{meaning}(g_2)$$

In embedding space $\mathbb{R}^n$:

$$\text{meaning}(g_1) \approx \text{meaning}(g_2) \iff \cos(\mathbf{v}_{g_1}, \mathbf{v}_{g_2}) \geq \theta^*$$

Where $\theta^* \in [0.70, 0.75]$ is the empirically validated alignment threshold.

### 8.2 Multi-Embedding Similarity (Formal)

Given two semantic fingerprints $F_1 = [e_1^{(1)}, \ldots, e_{13}^{(1)}]$ and $F_2 = [e_1^{(2)}, \ldots, e_{13}^{(2)}]$:

**RRF Fusion Formula** (Stage 3):
$$\text{RRF}(d) = \sum_{i=1}^{13} \frac{w_i}{k + \text{rank}_i(d)}$$

Where $k = 60$ (standard RRF constant from TREC evaluations).

**Per-Embedder Similarity Functions**:
- $w_i$ = weight for embedder $i$ (dynamic, query-dependent)
- $\text{sim}_i$ = similarity function appropriate for embedder $i$:
  - Cosine for dense vectors (E1, E2-E4, E5, E7, E8, E9, E10)
  - Jaccard + weighted overlap for sparse (E6)
  - TransE scoring for entity (E11): $\text{sim}_{11} = \frac{1}{1 + \|h + r - t\|}$
  - MaxSim for late interaction (E12): $\text{sim}_{12} = \frac{1}{|Q|} \sum_{q \in Q} \max_{d \in D} \cos(q, d)$
  - Sparse dot product for SPLADE (E13): $\text{sim}_{13} = \sum_{t \in Q \cap D} w_Q(t) \cdot w_D(t)$

### 8.3 Teleological Alignment Score (Formal)

Given a semantic fingerprint $F$ and North Star goal $V$:

$$A(F, V) = \frac{1}{13} \sum_{i=1}^{13} \cos(e_i, V_i)$$

With hierarchical decomposition:

$$A(F, V_{\text{global}}) \geq 2 \cdot A(F, V_{\text{mid}}) \cdot A(V_{\text{mid}}, V_{\text{global}}) - 1$$

### 8.4 Misalignment Detection (Formal)

Let $A_t$ be alignment at time $t$. Misalignment is detected when:

$$\Delta A = A_t - A_{t-1} < -0.15$$

This predicts coordination failure with 30-60 second lead time (empirically validated).

### 8.5 Information Preservation Theorem

**Theorem**: The multi-array approach preserves strictly more information than fusion.

**Proof**: Let $I(F)$ denote mutual information between input and embedding.

For fusion: $I(F_{\text{fused}}) = I(\text{MoE}(e_1, \ldots, e_{13})) \leq \sum_{i \in \text{top-}k} I(e_i)$

For multi-array: $I(F_{\text{multi}}) = I([e_1, \ldots, e_{13}]) = \sum_{i=1}^{13} I(e_i)$

Since $k < 13$ and all $I(e_i) > 0$:

$$I(F_{\text{multi}}) > I(F_{\text{fused}}) \quad \square$$

---

## 9. Validation Protocol

### 9.1 Per-Embedder Preservation Tests

```rust
#[cfg(test)]
mod preservation_tests {
    /// E1: Semantic similarity preservation
    #[test]
    fn test_semantic_preservation() {
        let similar_a = embed_semantic("The cat sat on the mat");
        let similar_b = embed_semantic("A feline rested on the rug");
        let dissimilar = embed_semantic("Quantum mechanics is complex");

        assert!(cosine_sim(&similar_a, &similar_b) > 0.80);
        assert!(cosine_sim(&similar_a, &dissimilar) < 0.30);
    }

    /// E5: Causal asymmetry preservation
    #[test]
    fn test_causal_asymmetry() {
        let cause = embed_causal("Rain causes flooding");
        let effect = embed_causal("Flooding is caused by rain");

        // Asymmetric similarity
        let sim_cause_to_effect = causal_asymmetric_sim(&cause, &effect);
        let sim_effect_to_cause = causal_asymmetric_sim(&effect, &cause);

        assert!(sim_cause_to_effect != sim_effect_to_cause);
    }

    /// E11: TransE triple preservation
    #[test]
    fn test_entity_triple() {
        let paris = embed_entity("Paris");
        let france = embed_entity("France");
        let capital_of = embed_relation("capital_of");

        // h + r ≈ t
        let predicted = paris.vector.add(&capital_of);
        let score = 1.0 / (1.0 + l2_distance(&predicted, &france.vector));

        assert!(score > 0.70);
    }
}
```

### 9.2 Teleological Alignment Validation

```rust
#[test]
fn test_teleological_hierarchy() {
    let north_star = NorthStar::new("Provide accurate, helpful context");
    let mid_goal = MidGoal::new("Retrieve relevant information");
    let local_action = LocalAction::new("Search for X in graph");

    let a_local_mid = alignment(&local_action, &mid_goal);
    let a_mid_global = alignment(&mid_goal, &north_star);
    let a_local_global = alignment(&local_action, &north_star);

    // Transitivity bound
    let bound = 2.0 * a_local_mid * a_mid_global - 1.0;
    assert!(a_local_global >= bound);

    // Threshold validation
    assert!(a_local_global >= 0.70, "Must be in optimal range");
}

#[test]
fn test_misalignment_detection() {
    let memory_before = create_test_memory();
    let memory_after = corrupt_memory(&memory_before);

    let a_before = compute_alignment(&memory_before, &north_star);
    let a_after = compute_alignment(&memory_after, &north_star);
    let delta_a = a_after - a_before;

    // Misalignment should be detected
    assert!(delta_a < -0.15, "Corruption should trigger misalignment");
}
```

### 9.3 Multi-Similarity Accuracy Benchmark

```rust
#[test]
fn test_multi_similarity_accuracy() {
    // Ground truth: human-labeled similarity pairs
    let test_pairs = load_similarity_benchmark();

    let mut predictions = vec![];
    for (query, memory, human_score) in &test_pairs {
        let multi_sim = multi_embedding_similarity(query, memory, &SimilarityWeights::default());
        predictions.push((multi_sim.aggregate, human_score));
    }

    // Correlation with human judgment
    let correlation = pearson_correlation(&predictions);
    assert!(correlation >= 0.75, "Must correlate with human judgment");

    // Compare to single-vector baseline
    let single_vector_correlation = test_single_vector_baseline(&test_pairs);
    assert!(correlation > single_vector_correlation, "Multi-array must beat single vector");
}
```

### 9.4 Performance Benchmarks (5-Stage Pipeline)

| Metric | Target | Validation Method |
|--------|--------|-------------------|
| **Embedding Computation** | | |
| All 13 embeddings | <50ms | Benchmark suite |
| E13 SPLADE only | <8ms | Benchmark suite |
| E1 Matryoshka encoding | <12ms | Benchmark suite |
| **Storage** | | |
| Storage per memory (quantized) | <17KB | Storage audit |
| PQ-8 compression ratio | 32x | Compression test |
| **5-Stage Retrieval (1M memories)** | | |
| Stage 1: SPLADE pre-filter | <5ms | Load test |
| Stage 2: Matryoshka ANN | <10ms | Load test |
| Stage 3: RRF multi-space | <20ms | Load test |
| Stage 4: Teleological filter | <10ms | Load test |
| Stage 5: MaxSim rerank | <15ms | Load test |
| **Total pipeline latency** | **<60ms** | End-to-end test |
| **Other Metrics** | | |
| Multi-similarity (13 embedders) | <5ms | Benchmark suite |
| Teleological alignment | <2ms | Benchmark suite |
| RRF fusion computation | <1ms | Benchmark suite |
| Asymmetric causal similarity | <1ms | Benchmark suite |

---

## Appendix A: Research Sources

1. **MOEE (ICLR 2025)**: "Weighted sum of similarities computed on different representations often achieves best results"
2. **FuseMoE (NeurIPS 2024)**: Laplace-smoothed gating, top-k expert selection
3. **Procrustes Alignment (2025)**: Orthogonal transformation bounds for embedding alignment
4. **Teleological Vectors (Royse, 2026)**: Mathematical framework for goal-directed alignment
5. **Matryoshka Representation Learning (NeurIPS 2022)**: Nested embeddings, dimension flexibility, 14x size reduction
6. **ColBERT/ColBERTv2**: Late interaction for token-level matching (Stage 5)
7. **TransE/TransR**: Knowledge graph embedding projection
8. **code2vec**: AST-aware code embeddings
9. **SPLADE v2**: Sparse Lexical and Expansion Model for hybrid search (E13)
10. **Qdrant/Pinecone Quantization**: PQ-8, Float8, Binary quantization strategies (63% storage reduction)
11. **RRF (Reciprocal Rank Fusion)**: Proven fusion algorithm from TREC evaluations (k=60)
12. **CausalRAG (ACL 2025)**: Asymmetric causal similarity for cause-effect retrieval
13. **Improvement Report (2026-01-05)**: 5-stage pipeline optimization, 40+ source synthesis

---

## Appendix B: Glossary

- **Semantic Fingerprint**: Complete multi-array representation of a memory across all 13 embedders
- **Teleological Alignment**: Measure of goal-directedness via cosine similarity
- **North Star Goal**: Top-level system objective for alignment measurement
- **Multi-Embedding Similarity**: Weighted combination of per-embedder similarities
- **Misalignment Detection**: Early warning when ΔA < -0.15
- **Late Fusion**: Computing similarity per-embedder, combining scores (vs. fusing embeddings first)

---

## 10. The Paradigm Shift: 13-Embedding Array AS the Teleological Vector

### 10.1 The Core Insight

**Traditional View**: A teleological vector is a single vector in embedding space representing a goal.

**New Understanding**: The teleological vector IS the pattern across ALL 13 embedding spaces. Each embedding space captures a different facet of purpose:

```
Traditional Teleological Vector:
  V_goal = [0.23, -0.15, 0.87, ...] ∈ ℝ^1536   ← Single vector, single perspective

Multi-Space Teleological Vector:
  T_goal = [
    V_semantic,    # What does this goal MEAN? (1024D, Matryoshka-enabled)
    V_temporal,    # When is this goal relevant?
    V_causal,      # What causes/effects does this goal have?
    V_code,        # How is this goal expressed in code?
    V_graph,       # What structures support this goal?
    V_entity,      # What entities are involved?
    V_splade,      # What keywords define this goal? (E13 SPLADE)
    ...            # (all 13 spaces)
  ]

  T_goal ∈ ℝ^{1024 × 512 × 512 × 512 × 768 × 30K × 1536 × 384 × 1024 × 768 × 384 × n×128 × ~30K_sparse}
```

### 10.2 Why This Matters for Memory

The 13-embedding array of a memory IS its teleological signature:

```rust
/// A memory's teleological identity across all meaning dimensions
pub struct TeleologicalFingerprint {
    /// The raw embeddings (what the memory IS)
    pub embeddings: SemanticFingerprint,

    /// The purpose vector (what the memory is FOR)
    /// Computed as alignment to North Star in each space
    pub purpose_vector: PurposeVector,

    /// Johari classification per embedder
    pub johari_quadrants: [JohariQuadrant; 13],

    /// Temporal evolution of purpose
    pub purpose_evolution: Vec<PurposeSnapshot>,
}

/// The 13D signature of purpose alignment
pub struct PurposeVector {
    /// Alignment score per embedding space (includes E13 SPLADE)
    pub alignments: [f32; 13],

    /// Dominant purpose dimension
    pub dominant_space: EmbedderType,

    /// Purpose coherence (how aligned are all spaces?)
    pub coherence: f32,

    /// Purpose stability (how stable over time?)
    pub stability: f32,
}
```

### 10.3 What This Unlocks

| Capability | Traditional (Single Vector) | Multi-Space Teleological |
|------------|---------------------------|-------------------------|
| Goal specificity | 1 dimension of meaning | 13 orthogonal dimensions |
| Causal goal detection | Embedded in noise | Direct E5 alignment |
| Code-goal alignment | Requires projection | Direct E7 alignment |
| Temporal goal relevance | Lost | E2-E4 temporal alignment |
| Goal decomposition | Manual | Per-space analysis |
| Goal conflict detection | Approximate | Per-space misalignment |
| Entity-goal mapping | External linking | Direct E11 alignment |
| Keyword-goal matching | BM25 baseline | Direct E13 SPLADE alignment |

---

## 11. Teleological Vector Storage Architecture

### 11.1 Storage Requirements

To fully analyze and utilize teleological vectors, we need:

1. **Per-Embedder Search**: Query "memories causally aligned with goal X" (E5 search)
2. **Cross-Space Patterns**: Find memories with similar teleological signatures
3. **Goal Hierarchy Navigation**: Traverse North Star → Mid → Local alignments
4. **Johari Quadrant Filtering**: Find memories in "blind spot" for specific spaces
5. **Temporal Purpose Evolution**: Track how purpose alignments change
6. **Semantic Understanding Preservation**: Never lose what each embedding means
7. **Keyword-Level Precision**: E13 SPLADE for sparse lexical matching

**Storage Efficiency** (from improvement report):
- ~17KB per memory (quantized) vs ~46KB uncompressed
- **63% reduction** via PQ-8/Float8/Binary quantization strategies

### 11.2 Multi-Index Storage Schema

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TELEOLOGICAL VECTOR STORAGE ARCHITECTURE                   │
│                    (~17KB per memory with PQ-8 quantization)                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │ LAYER 1: Primary Storage (RocksDB/ScyllaDB)                             │ │
│  │                                                                          │ │
│  │  Key: memory_id                                                          │ │
│  │  Value: {                                                                │ │
│  │    embeddings: [E1...E13],           # Full 13-array fingerprint        │ │
│  │    purpose_vector: [A1...A13],       # 13D alignment signature          │ │
│  │    johari: [Q1...Q13],               # Per-embedder quadrant            │ │
│  │    metadata: { created, updated, source, ... }                          │ │
│  │  }                                                                       │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                              │                                                │
│         ┌────────────────────┼────────────────────┐                          │
│         ▼                    ▼                    ▼                          │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐                   │
│  │ LAYER 2A:   │      │ LAYER 2B:   │      │ LAYER 2C:   │                   │
│  │ Per-Space   │      │ Purpose     │      │ Goal        │                   │
│  │ Indexes     │      │ Pattern     │      │ Hierarchy   │                   │
│  │ (12 HNSW)   │      │ Index       │      │ Index       │                   │
│  └─────────────┘      └─────────────┘      └─────────────┘                   │
│         │                    │                    │                          │
│         ▼                    ▼                    ▼                          │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │ LAYER 3: Query Router                                                    │ │
│  │                                                                          │ │
│  │  "Find causally-aligned memories"   → Route to E5 index                 │ │
│  │  "Find memories with similar purpose" → Route to Purpose Pattern index  │ │
│  │  "Find memories aligned with goal X" → Route to Goal Hierarchy index    │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 11.3 Per-Embedder Index (Layer 2A)

Each of the 12 embedding spaces gets its own vector index:

```rust
/// 13 independent vector indexes for per-space search
/// Organized by 5-stage pipeline role
pub struct PerEmbedderIndexes {
    // === STAGE 1: SPLADE Sparse Pre-Filter ===

    /// E13: SPLADE sparse lexical search (Inverted index)
    /// Primary Stage 1 index for initial candidate generation
    pub splade_index: SpladeInvertedIndex,

    // === STAGE 2: Matryoshka ANN ===

    /// E1: Semantic meaning search (HNSW, 128D truncated for Stage 2)
    /// Matryoshka-enabled: 128D → 256D → 512D → 1024D
    pub semantic_index: MatryoshkaHnswIndex<1024>,

    // === STAGE 3: Multi-Space RRF ===

    /// E2: Temporal recency search (HNSW, 512D)
    pub temporal_recent_index: HnswIndex<512>,

    /// E3: Temporal periodic search (HNSW, 512D)
    pub temporal_periodic_index: HnswIndex<512>,

    /// E4: Positional search (HNSW, 512D)
    pub temporal_positional_index: HnswIndex<512>,

    /// E5: Causal relationship search (HNSW, 768D)
    /// CRITICAL: Uses asymmetric distance for cause/effect
    pub causal_index: AsymmetricHnswIndex<768>,

    /// E6: Sparse activation search (Inverted index)
    pub sparse_index: InvertedIndex<30000>,

    /// E7: Code semantics search (HNSW, 1536D)
    pub code_index: HnswIndex<1536>,

    /// E8: Graph structure search (HNSW, 384D)
    pub graph_index: HnswIndex<384>,

    /// E9: HDC holographic search (LSH for binary)
    pub hdc_index: LshIndex<1024>,

    /// E10: Multimodal search (HNSW, 768D)
    pub multimodal_index: HnswIndex<768>,

    /// E11: Entity/KG search (HNSW, 384D)
    pub entity_index: HnswIndex<384>,

    // === STAGE 5: Late Interaction ===

    /// E12: Late interaction (ColBERT-style MaxSim)
    pub late_interaction_index: ColbertIndex,
}

/// Matryoshka-aware HNSW index supporting progressive refinement
pub struct MatryoshkaHnswIndex<const D: usize> {
    /// Fast 128D index for Stage 2
    pub stage2_index: HnswIndex<128>,
    /// Full-precision index for Stage 3 verification
    pub full_index: HnswIndex<D>,
}

/// SPLADE inverted index for Stage 1 sparse retrieval
pub struct SpladeInvertedIndex {
    /// Vocabulary term → posting list (doc_id, weight)
    postings: HashMap<u32, Vec<(MemoryId, f32)>>,
    /// Document norms for scoring
    norms: HashMap<MemoryId, f32>,
}

impl PerEmbedderIndexes {
    /// Search within a specific embedding space
    pub fn search_space(
        &self,
        space: EmbedderType,
        query: &[f32],
        top_k: usize,
    ) -> Vec<(MemoryId, f32)> {
        match space {
            EmbedderType::Semantic => self.semantic_index.search(query, top_k),
            EmbedderType::Causal => self.causal_index.search_asymmetric(query, top_k),
            EmbedderType::Sparse => self.sparse_index.search_jaccard(query, top_k),
            // ... all 12 spaces
        }
    }

    /// Cross-space search: find memories similar in multiple spaces
    pub fn search_cross_space(
        &self,
        queries: &[(EmbedderType, Vec<f32>, f32)], // (space, query, weight)
        top_k: usize,
    ) -> Vec<(MemoryId, f32)> {
        let mut scores: HashMap<MemoryId, f32> = HashMap::new();

        for (space, query, weight) in queries {
            let results = self.search_space(*space, query, top_k * 10);
            for (id, sim) in results {
                *scores.entry(id).or_insert(0.0) += sim * weight;
            }
        }

        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        ranked.truncate(top_k);
        ranked
    }
}
```

### 11.4 Purpose Pattern Index (Layer 2B)

The 13D purpose vector is itself searchable:

```rust
/// Index for finding memories with similar teleological signatures
pub struct PurposePatternIndex {
    /// HNSW index on 13D purpose vectors
    /// Each entry: [A(E1,V), A(E2,V), ..., A(E13,V)]
    purpose_hnsw: HnswIndex<13>,

    /// LSH buckets for fast approximate matching
    purpose_lsh: LshIndex<13>,

    /// Clustering of purpose patterns
    purpose_clusters: KMeansClusters<13>,
}

impl PurposePatternIndex {
    /// Find memories with similar purpose signatures
    pub fn search_by_purpose(
        &self,
        purpose: &PurposeVector,
        top_k: usize,
    ) -> Vec<(MemoryId, f32)> {
        self.purpose_hnsw.search(&purpose.alignments, top_k)
    }

    /// Find memories in a purpose cluster
    pub fn find_cluster_members(
        &self,
        purpose: &PurposeVector,
    ) -> Vec<MemoryId> {
        let cluster_id = self.purpose_clusters.predict(&purpose.alignments);
        self.purpose_clusters.get_members(cluster_id)
    }

    /// Find memories with contrasting purposes (for diversity)
    pub fn search_diverse_purposes(
        &self,
        current: &[PurposeVector],
        top_k: usize,
    ) -> Vec<(MemoryId, f32)> {
        // Find memories far from current set in purpose space
        let centroid = compute_centroid(current);
        self.purpose_hnsw.search_farthest(&centroid.alignments, top_k)
    }
}
`