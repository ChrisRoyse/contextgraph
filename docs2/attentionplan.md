# Cross-Space Attention Architecture Plan

**Document**: `attentionplan.md`
**Version**: 1.0.0
**Created**: 2026-01-05
**Status**: PROPOSED
**Authors**: Architecture Team
**Dependencies**: projectionplan1.md, projectionplan2.md, contextprd.md

---

## Executive Summary

This document defines the architecture for integrating **transformer-style attention mechanisms** into the existing 13-embedder Multi-Array Semantic Fingerprint system. The goal is to enable **learned cross-space interactions** while preserving the **100% information preservation** property of the multi-array approach.

**Key Decisions**:
1. **Storage remains multi-array** — No information loss
2. **Attention applied at retrieval only** — Stage 3 reranking
3. **Hybrid dense/sparse** — E6/E13 sparse stay outside attention
4. **Asymmetric causal attention** — E5 direction preserved via masking
5. **Optional/toggleable** — Fall back to RRF when attention doesn't help

**Expected Benefits**:
- 15-25% improvement in retrieval relevance (estimated)
- Query-adaptive fusion (vs static RRF weights)
- Learned cross-space patterns (semantic↔causal, code↔graph)
- Compositional reasoning ("similar AND different" queries)

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Attention Mechanisms](#2-attention-mechanisms)
3. [Cross-Space Attention Module](#3-cross-space-attention-module)
4. [Asymmetric Causal Attention](#4-asymmetric-causal-attention)
5. [Multi-Head Attention Design](#5-multi-head-attention-design)
6. [Integration with 5-Stage Pipeline](#6-integration-with-5-stage-pipeline)
7. [Sparse Embedding Handling](#7-sparse-embedding-handling)
8. [Training Architecture](#8-training-architecture)
9. [Inference Optimization](#9-inference-optimization)
10. [Implementation Plan](#10-implementation-plan)
11. [Mathematical Foundations](#11-mathematical-foundations)
12. [Validation & Benchmarks](#12-validation--benchmarks)

---

## 1. Architecture Overview

### 1.1 Current State (Multi-Array)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CURRENT: Multi-Array Architecture                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  SemanticFingerprint = [E1, E2, E3, E4, E5, E6, E7, E8, E9, E10,    │
│                         E11, E12, E13]                               │
│                                                                      │
│  Similarity Computation:                                             │
│    sim_i = similarity(query.E_i, memory.E_i)  for i in 1..13        │
│    final_score = RRF_fusion(sim_1, sim_2, ..., sim_13)              │
│                                                                      │
│  Properties:                                                         │
│    ✓ 100% information preserved                                      │
│    ✓ Per-space searchable                                           │
│    ✓ Interpretable                                                  │
│    ✗ No cross-space learning                                        │
│    ✗ Static fusion weights                                          │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Target State (Hybrid Multi-Array + Attention)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TARGET: Hybrid Architecture                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  STORAGE LAYER (unchanged):                                          │
│    SemanticFingerprint = [E1, E2, ..., E13]  // 100% preserved      │
│                                                                      │
│  RETRIEVAL LAYER (enhanced):                                         │
│    Stage 1-2: Unchanged (SPLADE + Matryoshka ANN)                   │
│    Stage 3:   CrossSpaceAttention(query, candidates)  // NEW        │
│    Stage 4-5: Unchanged (Teleological + MaxSim)                     │
│                                                                      │
│  CrossSpaceAttention:                                                │
│    ┌─────────────────────────────────────────────────────────┐      │
│    │  Dense Spaces (11)        Sparse Spaces (2)             │      │
│    │  [E1,E2,E3,E4,E5,         [E6, E13]                     │      │
│    │   E7,E8,E9,E10,E11,E12]   ↓                             │      │
│    │         ↓                  Sparse Attention              │      │
│    │   Self-Attention          (separate path)               │      │
│    │   Cross-Attention                ↓                      │      │
│    │         ↓                        │                      │      │
│    │   Attention Output ─────────────►│                      │      │
│    │         ↓                        ↓                      │      │
│    │         └────────► Fusion ◄──────┘                      │      │
│    │                      ↓                                  │      │
│    │               Final Score                               │      │
│    └─────────────────────────────────────────────────────────┘      │
│                                                                      │
│  Properties:                                                         │
│    ✓ 100% information preserved (storage)                           │
│    ✓ Cross-space learning (retrieval)                               │
│    ✓ Query-adaptive fusion                                          │
│    ✓ Asymmetric causal preserved                                    │
│    ✓ Sparse embeddings handled separately                           │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.3 Embedding Space Classification

| ID | Name | Dim | Type | Attention Path | Reason |
|----|------|-----|------|----------------|--------|
| E1 | Semantic | 1024D | Dense | Main attention | Core meaning |
| E2 | Temporal-Recent | 512D | Dense | Main attention | Time-aware |
| E3 | Temporal-Periodic | 512D | Dense | Main attention | Cyclical patterns |
| E4 | Temporal-Positional | 512D | Dense | Main attention | Sequence order |
| E5 | Causal | 768D | Dense | **Asymmetric attention** | Direction matters |
| E6 | Sparse | ~30K | **Sparse** | Sparse attention | 5% active |
| E7 | Code | 1536D | Dense | Main attention | AST structure |
| E8 | Graph/GNN | 384D | Dense | Main attention | Structural |
| E9 | HDC | 1024D | Dense | Main attention | Holographic |
| E10 | Multimodal | 768D | Dense | Main attention | Cross-modal |
| E11 | Entity/TransE | 384D | Dense | Main attention | Knowledge graph |
| E12 | Late-Interaction | 128D/tok | Dense | Token attention | ColBERT-style |
| E13 | SPLADE | ~30K | **Sparse** | Sparse attention | Lexical |

---

## 2. Attention Mechanisms

### 2.1 Self-Attention Between Embedding Spaces

**Concept**: Treat the 11 dense embedding spaces as a "sequence of 11 tokens", where each token represents one embedding space.

```
Input: [E1, E2, E3, E4, E5, E7, E8, E9, E10, E11, E12]  // 11 dense spaces
       Each E_i projected to common dimension d_model = 128

Self-Attention computes:
  Q = W_Q @ [E1, E2, ..., E12]  // [11, 128]
  K = W_K @ [E1, E2, ..., E12]  // [11, 128]
  V = W_V @ [E1, E2, ..., E12]  // [11, 128]

  Attention = softmax(Q @ K.T / √d_model)  // [11, 11] attention matrix
  Output = Attention @ V  // [11, 128]

What the 11×11 attention matrix learns:
  attention[i][j] = "how much space i should attend to space j"

  Example learned patterns:
  - attention[semantic][causal] = 0.8  → "semantic attends strongly to causal"
  - attention[code][graph] = 0.7       → "code attends to graph structure"
  - attention[temporal][entity] = 0.1  → "temporal ignores entity"
```

**Benefits**:
- Learns which spaces are relevant to each other
- Query-independent space relationships
- Can discover latent cross-space patterns

### 2.2 Cross-Attention Between Query and Memory

**Concept**: Query fingerprint attends to memory fingerprint to compute relevance.

```
Query Fingerprint:  Q_emb = [Q_E1, Q_E2, ..., Q_E12]  // 11 dense spaces
Memory Fingerprint: M_emb = [M_E1, M_E2, ..., M_E12]  // 11 dense spaces

Cross-Attention:
  Q = W_Q @ Q_emb  // [11, 128] - Query representation
  K = W_K @ M_emb  // [11, 128] - Memory keys
  V = W_V @ M_emb  // [11, 128] - Memory values

  Attention = softmax(Q @ K.T / √d_model)  // [11, 11]
  Output = Attention @ V  // [11, 128]

What the 11×11 cross-attention matrix learns:
  attention[query_space_i][memory_space_j] =
    "how much query's space i should match memory's space j"

  Example learned patterns:
  - attention[Q_semantic][M_causal] = 0.6
    → "query's semantic meaning relates to memory's causal structure"
  - attention[Q_code][M_code] = 0.9
    → "code queries should match code memories directly"
  - attention[Q_causal][M_entity] = 0.5
    → "causal queries benefit from entity knowledge"
```

**Benefits**:
- Query-adaptive: different queries produce different attention patterns
- Can learn asymmetric relevance (Q→M ≠ M→Q)
- More expressive than per-space cosine similarity

### 2.3 Combined Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                 COMBINED ATTENTION ARCHITECTURE                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Query Fingerprint ───────────────────────────────────────┐         │
│         │                                                  │         │
│         ▼                                                  │         │
│  ┌─────────────────┐                                       │         │
│  │ Query Self-Attn │  "Contextualize query spaces"        │         │
│  │    11 × 11      │                                       │         │
│  └────────┬────────┘                                       │         │
│           │                                                │         │
│           ▼                                                ▼         │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │              Cross-Attention (Query → Memory)                │    │
│  │                                                              │    │
│  │  Q (from query self-attn) ──┐                               │    │
│  │                              ├──► Attention ──► Output      │    │
│  │  K, V (from memory) ────────┘      [11,11]     [11,128]     │    │
│  │                                                              │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              ▼                                       │
│                    ┌──────────────────┐                             │
│                    │  Output Pooling   │                             │
│                    │  [11,128] → [1]   │                             │
│                    └────────┬─────────┘                             │
│                             │                                        │
│                             ▼                                        │
│                      Relevance Score                                 │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Cross-Space Attention Module

### 3.1 Core Data Structures

```rust
/// Configuration for Cross-Space Attention
pub struct CrossSpaceAttentionConfig {
    /// Common embedding dimension for all spaces after projection
    pub d_model: usize,  // Default: 128

    /// Number of attention heads
    pub num_heads: usize,  // Default: 4

    /// Dimension per head
    pub d_head: usize,  // d_model / num_heads = 32

    /// Dropout rate for attention weights
    pub dropout: f32,  // Default: 0.1

    /// Whether to use asymmetric causal attention for E5
    pub asymmetric_causal: bool,  // Default: true

    /// Temperature for attention softmax
    pub temperature: f32,  // Default: 1.0

    /// Number of dense embedding spaces (excluding E6, E13)
    pub num_dense_spaces: usize,  // 11

    /// Original dimensions of each dense space
    pub space_dims: [usize; 11],  // [1024, 512, 512, 512, 768, 1536, 384, 1024, 768, 384, 128]
}

impl Default for CrossSpaceAttentionConfig {
    fn default() -> Self {
        Self {
            d_model: 128,
            num_heads: 4,
            d_head: 32,
            dropout: 0.1,
            asymmetric_causal: true,
            temperature: 1.0,
            num_dense_spaces: 11,
            space_dims: [1024, 512, 512, 512, 768, 1536, 384, 1024, 768, 384, 128],
        }
    }
}
```

### 3.2 Per-Space Projection Layers

```rust
/// Projects each embedding space to common dimension
pub struct SpaceProjections {
    /// One projection layer per dense embedding space
    /// E1 (1024) → 128, E2 (512) → 128, ..., E12 (128) → 128
    projections: [Linear; 11],

    /// Layer normalization per space
    layer_norms: [LayerNorm; 11],

    /// Space position embeddings (learnable)
    /// Tells attention which space is which
    space_embeddings: Embedding<11, 128>,
}

impl SpaceProjections {
    pub fn new(config: &CrossSpaceAttentionConfig) -> Self {
        let projections = config.space_dims.iter().enumerate()
            .map(|(i, &dim)| Linear::new(dim, config.d_model))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let layer_norms = (0..11)
            .map(|_| LayerNorm::new(config.d_model))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let space_embeddings = Embedding::new(11, config.d_model);

        Self { projections, layer_norms, space_embeddings }
    }

    /// Project all dense spaces to common dimension
    pub fn forward(&self, fingerprint: &DenseFingerprint) -> Tensor<[11, 128]> {
        let mut output = Tensor::zeros([11, 128]);

        for (i, space_vec) in fingerprint.dense_spaces().enumerate() {
            // Project to common dim
            let projected = self.projections[i].forward(space_vec);

            // Add space position embedding
            let with_pos = projected + self.space_embeddings.forward(i);

            // Layer norm
            output[i] = self.layer_norms[i].forward(with_pos);
        }

        output
    }
}

/// Dense portion of SemanticFingerprint (11 spaces)
pub struct DenseFingerprint {
    pub e1_semantic: Vector1024,
    pub e2_temporal_recent: Vector512,
    pub e3_temporal_periodic: Vector512,
    pub e4_temporal_positional: Vector512,
    pub e5_causal: Vector768,
    // E6 sparse - excluded
    pub e7_code: Vector1536,
    pub e8_graph: Vector384,
    pub e9_hdc: Vector1024,
    pub e10_multimodal: Vector768,
    pub e11_entity: Vector384,
    pub e12_late_interaction_pooled: Vector128,  // Pooled from per-token
    // E13 sparse - excluded
}

impl DenseFingerprint {
    pub fn from_semantic_fingerprint(sf: &SemanticFingerprint) -> Self {
        Self {
            e1_semantic: sf.semantic.full.clone(),
            e2_temporal_recent: sf.temporal_recent.clone(),
            e3_temporal_periodic: sf.temporal_periodic.clone(),
            e4_temporal_positional: sf.temporal_positional.clone(),
            e5_causal: sf.causal.vector.clone(),
            e7_code: sf.code.clone(),
            e8_graph: sf.graph.vector.clone(),
            e9_hdc: sf.hdc.vector.clone(),
            e10_multimodal: sf.multimodal.vector.clone(),
            e11_entity: sf.entity.vector.clone(),
            e12_late_interaction_pooled: pool_late_interaction(&sf.late_interaction),
        }
    }

    pub fn dense_spaces(&self) -> impl Iterator<Item = &[f32]> {
        [
            self.e1_semantic.as_slice(),
            self.e2_temporal_recent.as_slice(),
            self.e3_temporal_periodic.as_slice(),
            self.e4_temporal_positional.as_slice(),
            self.e5_causal.as_slice(),
            self.e7_code.as_slice(),
            self.e8_graph.as_slice(),
            self.e9_hdc.as_slice(),
            self.e10_multimodal.as_slice(),
            self.e11_entity.as_slice(),
            self.e12_late_interaction_pooled.as_slice(),
        ].into_iter()
    }
}
```

### 3.3 Multi-Head Attention Implementation

```rust
/// Multi-Head Attention with optional asymmetric masking
pub struct MultiHeadAttention {
    /// Query projection
    w_q: Linear,  // [d_model, d_model]

    /// Key projection
    w_k: Linear,  // [d_model, d_model]

    /// Value projection
    w_v: Linear,  // [d_model, d_model]

    /// Output projection
    w_o: Linear,  // [d_model, d_model]

    /// Configuration
    config: CrossSpaceAttentionConfig,
}

impl MultiHeadAttention {
    pub fn new(config: &CrossSpaceAttentionConfig) -> Self {
        Self {
            w_q: Linear::new(config.d_model, config.d_model),
            w_k: Linear::new(config.d_model, config.d_model),
            w_v: Linear::new(config.d_model, config.d_model),
            w_o: Linear::new(config.d_model, config.d_model),
            config: config.clone(),
        }
    }

    /// Forward pass with optional attention mask
    pub fn forward(
        &self,
        query: &Tensor<[11, 128]>,
        key: &Tensor<[11, 128]>,
        value: &Tensor<[11, 128]>,
        mask: Option<&Tensor<[11, 11]>>,
    ) -> (Tensor<[11, 128]>, Tensor<[4, 11, 11]>) {
        let batch_size = 1;  // Single query-memory pair
        let seq_len = 11;    // 11 dense spaces
        let num_heads = self.config.num_heads;
        let d_head = self.config.d_head;

        // Project Q, K, V
        let q = self.w_q.forward(query);  // [11, 128]
        let k = self.w_k.forward(key);    // [11, 128]
        let v = self.w_v.forward(value);  // [11, 128]

        // Reshape for multi-head: [11, 128] → [11, 4, 32] → [4, 11, 32]
        let q = q.reshape([seq_len, num_heads, d_head]).permute([1, 0, 2]);
        let k = k.reshape([seq_len, num_heads, d_head]).permute([1, 0, 2]);
        let v = v.reshape([seq_len, num_heads, d_head]).permute([1, 0, 2]);

        // Scaled dot-product attention
        let scale = (d_head as f32).sqrt();
        let scores = q.matmul(&k.transpose(-2, -1)) / scale;  // [4, 11, 11]

        // Apply mask if provided (for asymmetric causal attention)
        let scores = if let Some(mask) = mask {
            scores + mask.unsqueeze(0).expand([num_heads, seq_len, seq_len])
        } else {
            scores
        };

        // Temperature scaling
        let scores = scores / self.config.temperature;

        // Softmax
        let attention_weights = softmax(scores, dim=-1);  // [4, 11, 11]

        // Dropout (training only)
        let attention_weights = dropout(attention_weights, self.config.dropout);

        // Apply attention to values
        let output = attention_weights.matmul(&v);  // [4, 11, 32]

        // Reshape back: [4, 11, 32] → [11, 4, 32] → [11, 128]
        let output = output.permute([1, 0, 2]).reshape([seq_len, self.config.d_model]);

        // Output projection
        let output = self.w_o.forward(&output);

        (output, attention_weights)
    }
}
```

### 3.4 Complete Cross-Space Attention Module

```rust
/// Complete Cross-Space Attention Module for Stage 3 Reranking
pub struct CrossSpaceAttentionModule {
    /// Configuration
    config: CrossSpaceAttentionConfig,

    /// Per-space projections (query and memory share)
    space_projections: SpaceProjections,

    /// Query self-attention (contextualizes query spaces)
    query_self_attention: MultiHeadAttention,

    /// Cross-attention (query attends to memory)
    cross_attention: MultiHeadAttention,

    /// Asymmetric causal attention mask for E5
    causal_mask: Tensor<[11, 11]>,

    /// Output feed-forward network
    ffn: FeedForward,

    /// Final score projection
    score_projection: Linear,

    /// Layer norms
    ln_query_self: LayerNorm,
    ln_cross: LayerNorm,
    ln_ffn: LayerNorm,
}

impl CrossSpaceAttentionModule {
    pub fn new(config: CrossSpaceAttentionConfig) -> Self {
        let causal_mask = Self::build_causal_mask(&config);

        Self {
            space_projections: SpaceProjections::new(&config),
            query_self_attention: MultiHeadAttention::new(&config),
            cross_attention: MultiHeadAttention::new(&config),
            causal_mask,
            ffn: FeedForward::new(config.d_model, config.d_model * 4),
            score_projection: Linear::new(config.d_model, 1),
            ln_query_self: LayerNorm::new(config.d_model),
            ln_cross: LayerNorm::new(config.d_model),
            ln_ffn: LayerNorm::new(config.d_model),
            config,
        }
    }

    /// Build asymmetric attention mask for E5 causal embedding
    fn build_causal_mask(config: &CrossSpaceAttentionConfig) -> Tensor<[11, 11]> {
        let mut mask = Tensor::zeros([11, 11]);

        if config.asymmetric_causal {
            // E5 (causal) is at index 4 in dense spaces
            let causal_idx = 4;

            // Asymmetric weights for causal attention
            // cause→effect = 1.2x, effect→cause = 0.8x
            // Implemented via additive mask in log-space

            for i in 0..11 {
                for j in 0..11 {
                    if i == causal_idx && j != causal_idx {
                        // Causal space attending to others: boost
                        mask[[i, j]] = 0.2_f32.ln();  // ~1.2x after softmax
                    } else if i != causal_idx && j == causal_idx {
                        // Others attending to causal: dampen
                        mask[[i, j]] = (-0.2_f32).ln().max(-10.0);  // ~0.8x after softmax
                    }
                }
            }
        }

        mask
    }

    /// Score a single query-memory pair
    pub fn score(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> CrossSpaceScore {
        // Extract dense portions
        let query_dense = DenseFingerprint::from_semantic_fingerprint(query);
        let memory_dense = DenseFingerprint::from_semantic_fingerprint(memory);

        // Project to common dimension
        let query_projected = self.space_projections.forward(&query_dense);  // [11, 128]
        let memory_projected = self.space_projections.forward(&memory_dense); // [11, 128]

        // Query self-attention (contextualize query spaces)
        let (query_contextualized, query_self_attn) = self.query_self_attention.forward(
            &query_projected,
            &query_projected,
            &query_projected,
            None,  // No mask for self-attention
        );
        let query_contextualized = self.ln_query_self.forward(
            &(query_projected + query_contextualized)  // Residual
        );

        // Cross-attention (query attends to memory)
        let mask = if self.config.asymmetric_causal {
            Some(&self.causal_mask)
        } else {
            None
        };

        let (cross_output, cross_attn) = self.cross_attention.forward(
            &query_contextualized,
            &memory_projected,
            &memory_projected,
            mask,
        );
        let cross_output = self.ln_cross.forward(
            &(query_contextualized + cross_output)  // Residual
        );

        // Feed-forward
        let ffn_output = self.ffn.forward(&cross_output);
        let ffn_output = self.ln_ffn.forward(&(cross_output + ffn_output));

        // Pool across spaces and project to score
        let pooled = ffn_output.mean(dim=0);  // [128]
        let score = self.score_projection.forward(&pooled).sigmoid();  // [1] → scalar

        CrossSpaceScore {
            score: score.item(),
            query_self_attention: query_self_attn,
            cross_attention: cross_attn,
            per_space_contributions: self.compute_per_space_contributions(&cross_attn),
        }
    }

    /// Batch scoring for efficiency
    pub fn score_batch(
        &self,
        query: &SemanticFingerprint,
        memories: &[SemanticFingerprint],
    ) -> Vec<CrossSpaceScore> {
        // For production: batch all memories together
        // Here showing simple iteration for clarity
        memories.iter()
            .map(|mem| self.score(query, mem))
            .collect()
    }

    /// Compute per-space contribution to final score
    fn compute_per_space_contributions(
        &self,
        cross_attn: &Tensor<[4, 11, 11]>,
    ) -> [f32; 11] {
        // Average attention received by each memory space across all heads
        let avg_attn = cross_attn.mean(dim=0);  // [11, 11]
        let contributions = avg_attn.sum(dim=0);  // [11] - total attention to each memory space

        let mut result = [0.0f32; 11];
        for i in 0..11 {
            result[i] = contributions[i].item();
        }

        // Normalize to sum to 1
        let total: f32 = result.iter().sum();
        for v in &mut result {
            *v /= total;
        }

        result
    }
}

/// Result of cross-space attention scoring
pub struct CrossSpaceScore {
    /// Final relevance score [0, 1]
    pub score: f32,

    /// Query self-attention weights [4 heads, 11 query spaces, 11 query spaces]
    pub query_self_attention: Tensor<[4, 11, 11]>,

    /// Cross-attention weights [4 heads, 11 query spaces, 11 memory spaces]
    pub cross_attention: Tensor<[4, 11, 11]>,

    /// Per-space contribution to score (interpretability)
    pub per_space_contributions: [f32; 11],
}
```

---

## 4. Asymmetric Causal Attention

### 4.1 The Problem

E5 (causal embedding) has **asymmetric similarity**:
- `sim(cause → effect) ≈ 1.2 × base_sim`
- `sim(effect → cause) ≈ 0.8 × base_sim`

Standard attention is symmetric. We need to preserve this asymmetry.

### 4.2 Solution: Asymmetric Attention Mask

```rust
/// Asymmetric causal attention implementation
pub struct AsymmetricCausalAttention {
    /// Base attention module
    base_attention: MultiHeadAttention,

    /// Asymmetric mask for causal space
    asymmetric_mask: AsymmetricMask,

    /// Index of E5 (causal) in dense space array
    causal_space_idx: usize,  // = 4
}

/// Defines asymmetric attention relationships
pub struct AsymmetricMask {
    /// Additive mask values (in log-space for softmax)
    mask: Tensor<[11, 11]>,

    /// Causal direction weights
    cause_to_effect_boost: f32,  // 1.2
    effect_to_cause_dampen: f32, // 0.8
}

impl AsymmetricMask {
    pub fn new(causal_idx: usize, boost: f32, dampen: f32) -> Self {
        let mut mask = Tensor::zeros([11, 11]);

        // Convert multiplicative factors to additive log-space
        // softmax(x + log(k)) ≈ k × softmax(x) for small k adjustments
        let boost_log = boost.ln();    // ln(1.2) ≈ 0.182
        let dampen_log = dampen.ln();  // ln(0.8) ≈ -0.223

        // When causal space is QUERY (row), boost attention TO other spaces
        // This means: "causal meaning should strongly influence other spaces"
        for j in 0..11 {
            if j != causal_idx {
                mask[[causal_idx, j]] = boost_log;
            }
        }

        // When other spaces QUERY causal space (column), dampen
        // This means: "other spaces should not over-attend to causal"
        for i in 0..11 {
            if i != causal_idx {
                mask[[i, causal_idx]] = dampen_log;
            }
        }

        Self {
            mask,
            cause_to_effect_boost: boost,
            effect_to_cause_dampen: dampen,
        }
    }

    /// Apply mask to attention scores
    pub fn apply(&self, scores: &Tensor<[11, 11]>) -> Tensor<[11, 11]> {
        scores + &self.mask
    }
}

impl AsymmetricCausalAttention {
    /// Score with asymmetric causal attention
    pub fn forward(
        &self,
        query: &Tensor<[11, 128]>,
        key: &Tensor<[11, 128]>,
        value: &Tensor<[11, 128]>,
        query_causal_direction: CausalDirection,
        memory_causal_direction: CausalDirection,
    ) -> (Tensor<[11, 128]>, Tensor<[11, 11]>) {
        // Compute base attention scores
        let q = self.base_attention.w_q.forward(query);
        let k = self.base_attention.w_k.forward(key);
        let v = self.base_attention.w_v.forward(value);

        let scale = (32.0_f32).sqrt();  // d_head
        let scores = q.matmul(&k.transpose(-1, -2)) / scale;

        // Apply asymmetric mask based on causal directions
        let direction_mask = self.compute_direction_mask(
            query_causal_direction,
            memory_causal_direction,
        );
        let scores = scores + &self.asymmetric_mask.mask + &direction_mask;

        // Softmax
        let attention = softmax(scores, dim=-1);

        // Apply to values
        let output = attention.matmul(&v);

        (output, attention)
    }

    /// Compute additional mask based on specific causal directions
    fn compute_direction_mask(
        &self,
        query_dir: CausalDirection,
        memory_dir: CausalDirection,
    ) -> Tensor<[11, 11]> {
        let mut mask = Tensor::zeros([11, 11]);
        let idx = self.causal_space_idx;

        match (query_dir, memory_dir) {
            // Query is cause, memory is effect → boost causal attention
            (CausalDirection::Cause, CausalDirection::Effect) => {
                mask[[idx, idx]] = 0.3;  // Strong causal-to-causal attention
            }
            // Query is effect, memory is cause → dampen causal attention
            (CausalDirection::Effect, CausalDirection::Cause) => {
                mask[[idx, idx]] = -0.3;  // Weak causal-to-causal attention
            }
            // Same direction → neutral
            _ => {}
        }

        mask
    }
}
```

### 4.3 Visualizing Asymmetric Attention

```
ASYMMETRIC ATTENTION MATRIX (11×11)
Query spaces (rows) attending to Memory spaces (columns)

                M_E1  M_E2  M_E3  M_E4  M_E5   M_E7  M_E8  M_E9  M_E10 M_E11 M_E12
              │ sem   t_r   t_p   t_pos causal code  graph hdc   multi ent   late
    ──────────┼──────────────────────────────────────────────────────────────────
Q_E1  sem     │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E2  t_r     │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E3  t_p     │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E4  t_pos   │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E5  causal  │ 1.2↑  1.2↑  1.2↑  1.2↑  1.0    1.2↑  1.2↑  1.2↑  1.2↑  1.2↑  1.2↑
Q_E7  code    │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E8  graph   │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E9  hdc     │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E10 multi   │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E11 ent     │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0
Q_E12 late    │ 1.0   1.0   1.0   1.0   0.8↓   1.0   1.0   1.0   1.0   1.0   1.0

Legend:
  1.2↑ = Causal space boosts attention to other spaces (cause→effect logic)
  0.8↓ = Other spaces dampen attention to causal (effect→cause dampening)
  1.0  = Neutral (no asymmetric adjustment)

This ensures:
  - When causal information is in query, it propagates strongly
  - When other information queries causal, it's appropriately weighted
```

---

## 5. Multi-Head Attention Design

### 5.1 Head Specialization

With 4 attention heads, we can encourage each head to learn different cross-space patterns:

```rust
/// Multi-head configuration with specialized heads
pub struct SpecializedHeads {
    /// Head 0: Semantic-Causal relationships
    /// Learns how meaning relates to causality
    semantic_causal_head: AttentionHead,

    /// Head 1: Code-Structure relationships
    /// Learns how code relates to graph/entity structure
    code_structure_head: AttentionHead,

    /// Head 2: Temporal relationships
    /// Learns how time-related spaces interact
    temporal_head: AttentionHead,

    /// Head 3: Global coherence
    /// Learns overall cross-space patterns
    global_head: AttentionHead,
}

impl SpecializedHeads {
    /// Initialize with head-specific biases to encourage specialization
    pub fn new(config: &CrossSpaceAttentionConfig) -> Self {
        let mut heads = Self {
            semantic_causal_head: AttentionHead::new(config.d_head),
            code_structure_head: AttentionHead::new(config.d_head),
            temporal_head: AttentionHead::new(config.d_head),
            global_head: AttentionHead::new(config.d_head),
        };

        // Initialize with biases toward specialization
        // These are soft biases that can be overridden by training

        // Head 0: Bias toward E1 (semantic) and E5 (causal)
        heads.semantic_causal_head.initialize_bias(&[
            (0, 0, 0.5),  // E1→E1
            (0, 4, 0.5),  // E1→E5
            (4, 0, 0.5),  // E5→E1
            (4, 4, 0.5),  // E5→E5
        ]);

        // Head 1: Bias toward E7 (code), E8 (graph), E11 (entity)
        heads.code_structure_head.initialize_bias(&[
            (5, 5, 0.5),   // E7→E7
            (5, 6, 0.5),   // E7→E8
            (5, 9, 0.5),   // E7→E11
            (6, 5, 0.5),   // E8→E7
            (9, 5, 0.5),   // E11→E7
        ]);

        // Head 2: Bias toward E2, E3, E4 (temporal)
        heads.temporal_head.initialize_bias(&[
            (1, 1, 0.5), (1, 2, 0.5), (1, 3, 0.5),
            (2, 1, 0.5), (2, 2, 0.5), (2, 3, 0.5),
            (3, 1, 0.5), (3, 2, 0.5), (3, 3, 0.5),
        ]);

        // Head 3: No bias - learns global patterns
        // (default initialization)

        heads
    }

    /// Forward pass through all heads
    pub fn forward(
        &self,
        query: &Tensor<[11, 32]>,
        key: &Tensor<[11, 32]>,
        value: &Tensor<[11, 32]>,
    ) -> (Tensor<[4, 11, 32]>, Tensor<[4, 11, 11]>) {
        let outputs = [
            self.semantic_causal_head.forward(query, key, value),
            self.code_structure_head.forward(query, key, value),
            self.temporal_head.forward(query, key, value),
            self.global_head.forward(query, key, value),
        ];

        let output_tensor = Tensor::stack(outputs.iter().map(|(o, _)| o), dim=0);
        let attn_tensor = Tensor::stack(outputs.iter().map(|(_, a)| a), dim=0);

        (output_tensor, attn_tensor)
    }
}
```

### 5.2 Attention Pattern Analysis

```rust
/// Analyze learned attention patterns for interpretability
pub struct AttentionPatternAnalyzer {
    /// Space names for display
    space_names: [&'static str; 11],
}

impl AttentionPatternAnalyzer {
    pub fn new() -> Self {
        Self {
            space_names: [
                "semantic", "temp_recent", "temp_periodic", "temp_positional",
                "causal", "code", "graph", "hdc", "multimodal", "entity", "late_int"
            ],
        }
    }

    /// Summarize attention patterns across heads
    pub fn summarize(&self, attention: &Tensor<[4, 11, 11]>) -> AttentionSummary {
        let mut summary = AttentionSummary::default();

        // Average attention across heads
        let avg_attn = attention.mean(dim=0);  // [11, 11]

        // Find top attention pairs
        let mut pairs: Vec<(usize, usize, f32)> = Vec::new();
        for i in 0..11 {
            for j in 0..11 {
                pairs.push((i, j, avg_attn[[i, j]].item()));
            }
        }
        pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        summary.top_attention_pairs = pairs.into_iter()
            .take(10)
            .map(|(i, j, w)| AttentionPair {
                from_space: self.space_names[i].to_string(),
                to_space: self.space_names[j].to_string(),
                weight: w,
            })
            .collect();

        // Per-head dominant patterns
        for head in 0..4 {
            let head_attn = attention.select(0, head);  // [11, 11]
            let dominant = self.find_dominant_pattern(&head_attn);
            summary.head_patterns.push(dominant);
        }

        // Cross-space flow
        summary.semantic_to_causal_flow = avg_attn[[0, 4]].item();
        summary.code_to_graph_flow = avg_attn[[5, 6]].item();
        summary.temporal_coherence = (
            avg_attn[[1, 2]].item() +
            avg_attn[[2, 3]].item() +
            avg_attn[[1, 3]].item()
        ) / 3.0;

        summary
    }

    fn find_dominant_pattern(&self, head_attn: &Tensor<[11, 11]>) -> HeadPattern {
        // Find which spaces this head focuses on
        let row_sums = head_attn.sum(dim=1);  // [11] - how much each query space attends
        let col_sums = head_attn.sum(dim=0);  // [11] - how much each key space is attended

        let top_query_idx = row_sums.argmax().item();
        let top_key_idx = col_sums.argmax().item();

        HeadPattern {
            dominant_query_space: self.space_names[top_query_idx].to_string(),
            dominant_key_space: self.space_names[top_key_idx].to_string(),
            concentration: head_attn[[top_query_idx, top_key_idx]].item(),
        }
    }
}

#[derive(Default)]
pub struct AttentionSummary {
    pub top_attention_pairs: Vec<AttentionPair>,
    pub head_patterns: Vec<HeadPattern>,
    pub semantic_to_causal_flow: f32,
    pub code_to_graph_flow: f32,
    pub temporal_coherence: f32,
}

pub struct AttentionPair {
    pub from_space: String,
    pub to_space: String,
    pub weight: f32,
}

pub struct HeadPattern {
    pub dominant_query_space: String,
    pub dominant_key_space: String,
    pub concentration: f32,
}
```

---

## 6. Integration with 5-Stage Pipeline

### 6.1 Updated Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ENHANCED 5-STAGE RETRIEVAL PIPELINE                       │
│                    With Cross-Space Attention at Stage 3                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  STAGE 1: SPLADE SPARSE PRE-FILTER (E13)                          <5ms     │
│  ├─ Input: 1M memories                                                      │
│  ├─ Operation: Sparse dot product on inverted index                        │
│  ├─ Output: Top 10K candidates                                              │
│  └─ [UNCHANGED]                                                             │
│                          │                                                   │
│                          ▼                                                   │
│  STAGE 2: MATRYOSHKA ANN (E1 truncated)                           <10ms    │
│  ├─ Input: 10K candidates from Stage 1                                     │
│  ├─ Operation: HNSW search on 128D truncated E1 vectors                    │
│  ├─ Output: Top 500 candidates (expanded from 1K for attention rerank)     │
│  └─ [MODIFIED: expanded candidate set for attention]                       │
│                          │                                                   │
│                          ▼                                                   │
│  STAGE 3: CROSS-SPACE ATTENTION RERANK                            <30ms    │
│  ├─ Input: 500 candidates from Stage 2                             [NEW]   │
│  ├─ Operation:                                                              │
│  │   ┌─────────────────────────────────────────────────────────────────┐   │
│  │   │ For each candidate in parallel:                                  │   │
│  │   │   1. Extract dense fingerprints (query, memory)                 │   │
│  │   │   2. Project to common dimension (11 spaces × 128D)             │   │
│  │   │   3. Query self-attention (contextualize)                       │   │
│  │   │   4. Cross-attention (query → memory) with causal mask          │   │
│  │   │   5. FFN + pooling → relevance score                            │   │
│  │   └─────────────────────────────────────────────────────────────────┘   │
│  ├─ Fallback: If attention model unavailable, use RRF fusion              │
│  ├─ Output: Top 100 candidates with attention scores                       │
│  └─ Interpretability: Return attention weights for debugging               │
│                          │                                                   │
│                          ▼                                                   │
│  STAGE 4: TELEOLOGICAL ALIGNMENT FILTER                           <10ms    │
│  ├─ Input: 100 candidates from Stage 3                                     │
│  ├─ Operation: Compute A(memory, North Star)                               │
│  │   Filter: alignment < 0.55 → discard                                    │
│  │   Enhanced: Use attention-weighted alignment                            │
│  ├─ Output: Top 50 candidates                                              │
│  └─ [ENHANCED: attention weights inform alignment importance]              │
│                          │                                                   │
│                          ▼                                                   │
│  STAGE 5: LATE INTERACTION MAXSIM (E12)                           <15ms    │
│  ├─ Input: 50 candidates from Stage 4                                      │
│  ├─ Operation: ColBERT MaxSim token-level scoring                          │
│  ├─ Output: Final top 10 results                                           │
│  └─ [UNCHANGED]                                                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

TOTAL LATENCY: <70ms for 1M memories (was <60ms)
               +10ms for attention computation on 500 candidates
```

### 6.2 Pipeline Implementation

```rust
/// Enhanced retrieval pipeline with cross-space attention
pub struct EnhancedRetrievalPipeline {
    /// Stage 1: SPLADE sparse pre-filter
    splade_index: SpladeInvertedIndex,

    /// Stage 2: Matryoshka ANN
    matryoshka_index: MatryoshkaHnswIndex,

    /// Stage 3: Cross-space attention reranker [NEW]
    attention_reranker: CrossSpaceAttentionModule,

    /// Stage 3 fallback: RRF fusion
    rrf_reranker: RRFFusion,

    /// Stage 4: Teleological filter
    teleological_filter: TeleologicalFilter,

    /// Stage 5: Late interaction MaxSim
    maxsim_reranker: MaxSimReranker,

    /// Configuration
    config: PipelineConfig,

    /// Whether attention is enabled (for A/B testing)
    attention_enabled: bool,
}

pub struct PipelineConfig {
    /// Stage 1 output size
    pub stage1_top_k: usize,  // 10_000

    /// Stage 2 output size (expanded for attention)
    pub stage2_top_k: usize,  // 500 (was 1000)

    /// Stage 3 output size
    pub stage3_top_k: usize,  // 100

    /// Stage 4 alignment threshold
    pub alignment_threshold: f32,  // 0.55

    /// Stage 4 output size
    pub stage4_top_k: usize,  // 50

    /// Stage 5 output size
    pub stage5_top_k: usize,  // 10

    /// Attention batch size
    pub attention_batch_size: usize,  // 64
}

impl EnhancedRetrievalPipeline {
    /// Main retrieval entry point
    pub async fn retrieve(
        &self,
        query: &SemanticFingerprint,
        goal: Option<&TeleologicalGoal>,
    ) -> RetrievalResult {
        let start = Instant::now();
        let mut metrics = PipelineMetrics::default();

        // Stage 1: SPLADE sparse pre-filter
        let stage1_start = Instant::now();
        let stage1_candidates = self.splade_index.search(
            &query.splade,
            self.config.stage1_top_k,
        );
        metrics.stage1_latency = stage1_start.elapsed();
        metrics.stage1_candidates = stage1_candidates.len();

        // Stage 2: Matryoshka ANN
        let stage2_start = Instant::now();
        let stage2_candidates = self.matryoshka_index.search_from_candidates(
            query.semantic.truncate(128),
            &stage1_candidates,
            self.config.stage2_top_k,
        );
        metrics.stage2_latency = stage2_start.elapsed();
        metrics.stage2_candidates = stage2_candidates.len();

        // Stage 3: Cross-space attention OR RRF fallback
        let stage3_start = Instant::now();
        let stage3_candidates = if self.attention_enabled {
            self.attention_rerank(query, &stage2_candidates).await
        } else {
            self.rrf_rerank(query, &stage2_candidates)
        };
        metrics.stage3_latency = stage3_start.elapsed();
        metrics.stage3_candidates = stage3_candidates.len();
        metrics.attention_used = self.attention_enabled;

        // Stage 4: Teleological alignment filter
        let stage4_start = Instant::now();
        let stage4_candidates = if let Some(goal) = goal {
            self.teleological_filter.filter(
                &stage3_candidates,
                goal,
                self.config.alignment_threshold,
                self.config.stage4_top_k,
            )
        } else {
            stage3_candidates.into_iter()
                .take(self.config.stage4_top_k)
                .collect()
        };
        metrics.stage4_latency = stage4_start.elapsed();
        metrics.stage4_candidates = stage4_candidates.len();

        // Stage 5: Late interaction MaxSim
        let stage5_start = Instant::now();
        let final_results = self.maxsim_reranker.rerank(
            &query.late_interaction,
            &stage4_candidates,
            self.config.stage5_top_k,
        );
        metrics.stage5_latency = stage5_start.elapsed();

        metrics.total_latency = start.elapsed();

        RetrievalResult {
            results: final_results,
            metrics,
        }
    }

    /// Stage 3 attention reranking
    async fn attention_rerank(
        &self,
        query: &SemanticFingerprint,
        candidates: &[RankedCandidate],
    ) -> Vec<RankedCandidate> {
        // Load full fingerprints for candidates
        let fingerprints: Vec<_> = candidates.iter()
            .map(|c| self.load_fingerprint(c.memory_id))
            .collect::<FuturesUnordered<_>>()
            .collect()
            .await;

        // Batch attention scoring
        let mut scored: Vec<(RankedCandidate, CrossSpaceScore)> = Vec::new();

        for batch in fingerprints.chunks(self.config.attention_batch_size) {
            let batch_scores = self.attention_reranker.score_batch(query, batch);

            for (candidate, score) in candidates.iter().zip(batch_scores) {
                scored.push((candidate.clone(), score));
            }
        }

        // Sort by attention score
        scored.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap());

        // Return top-k with attention metadata
        scored.into_iter()
            .take(self.config.stage3_top_k)
            .map(|(mut candidate, score)| {
                candidate.attention_score = Some(score.score);
                candidate.attention_breakdown = Some(score.per_space_contributions);
                candidate
            })
            .collect()
    }

    /// Stage 3 RRF fallback
    fn rrf_rerank(
        &self,
        query: &SemanticFingerprint,
        candidates: &[RankedCandidate],
    ) -> Vec<RankedCandidate> {
        self.rrf_reranker.rerank(query, candidates, self.config.stage3_top_k)
    }
}

#[derive(Default)]
pub struct PipelineMetrics {
    pub stage1_latency: Duration,
    pub stage1_candidates: usize,
    pub stage2_latency: Duration,
    pub stage2_candidates: usize,
    pub stage3_latency: Duration,
    pub stage3_candidates: usize,
    pub attention_used: bool,
    pub stage4_latency: Duration,
    pub stage4_candidates: usize,
    pub stage5_latency: Duration,
    pub total_latency: Duration,
}

pub struct RankedCandidate {
    pub memory_id: MemoryId,
    pub stage2_score: f32,
    pub attention_score: Option<f32>,
    pub attention_breakdown: Option<[f32; 11]>,
    pub alignment_score: Option<f32>,
    pub final_score: f32,
}
```

---

## 7. Sparse Embedding Handling

### 7.1 The Problem

E6 (sparse activation) and E13 (SPLADE) are sparse vectors (~30K dimensions, 5% active).
They don't fit cleanly into dense attention matrices.

### 7.2 Solution: Parallel Sparse Attention Path

```rust
/// Handles sparse embeddings separately from dense attention
pub struct SparseAttentionPath {
    /// Sparse-to-dense projection for E6
    e6_projector: SparseProjector,

    /// Sparse-to-dense projection for E13 SPLADE
    e13_projector: SparseProjector,

    /// Learned sparse attention weights
    sparse_attention: SparseAttention,

    /// Fusion weight with dense attention output
    fusion_weight: f32,  // Default: 0.2
}

/// Projects sparse vector to dense via learned transformation
pub struct SparseProjector {
    /// Codebook of dense embeddings for top-k active dimensions
    codebook: Embedding<1024, 64>,  // Top 1024 most common sparse indices → 64D

    /// Attention over active dimensions
    sparse_attention: Linear,  // Maps (active_count, 64) → (1, 64)
}

impl SparseProjector {
    /// Project sparse vector to dense representation
    pub fn forward(&self, sparse: &SparseVector) -> Tensor<[64]> {
        // Look up codebook embeddings for active indices
        let active_embeddings: Vec<Tensor<[64]>> = sparse.indices.iter()
            .filter_map(|&idx| {
                if idx < 1024 {
                    Some(self.codebook.forward(idx))
                } else {
                    None  // Rare indices not in codebook
                }
            })
            .collect();

        if active_embeddings.is_empty() {
            return Tensor::zeros([64]);
        }

        // Stack and weight by sparse values
        let stacked = Tensor::stack(&active_embeddings, dim=0);  // [active, 64]
        let values = Tensor::from_slice(&sparse.values[..active_embeddings.len()]);
        let weighted = stacked * values.unsqueeze(1);  // [active, 64]

        // Attention pooling
        let attn_weights = self.sparse_attention.forward(&weighted);  // [active, 1]
        let attn_weights = softmax(attn_weights, dim=0);
        let pooled = (weighted * attn_weights).sum(dim=0);  // [64]

        pooled
    }
}

/// Sparse attention between query and memory sparse vectors
pub struct SparseAttention {
    /// Query projection
    w_q: Linear,  // [64, 64]

    /// Key projection
    w_k: Linear,  // [64, 64]

    /// Value projection
    w_v: Linear,  // [64, 64]

    /// Output projection
    w_o: Linear,  // [128, 64]  // Outputs to match dense path
}

impl SparseAttention {
    /// Compute sparse attention score
    pub fn forward(
        &self,
        query_e6: &Tensor<[64]>,
        query_e13: &Tensor<[64]>,
        memory_e6: &Tensor<[64]>,
        memory_e13: &Tensor<[64]>,
    ) -> Tensor<[64]> {
        // Stack query and memory sparse projections
        let query = Tensor::stack(&[query_e6, query_e13], dim=0);  // [2, 64]
        let memory = Tensor::stack(&[memory_e6, memory_e13], dim=0);  // [2, 64]

        // Project
        let q = self.w_q.forward(&query);  // [2, 64]
        let k = self.w_k.forward(&memory);  // [2, 64]
        let v = self.w_v.forward(&memory);  // [2, 64]

        // Attention
        let scores = q.matmul(&k.transpose(-1, -2)) / 8.0;  // [2, 2]
        let attention = softmax(scores, dim=-1);
        let output = attention.matmul(&v);  // [2, 64]

        // Pool and project
        let pooled = output.mean(dim=0);  // [64]
        self.w_o.forward(&pooled)  // [64]
    }
}

impl SparseAttentionPath {
    /// Compute sparse contribution to final score
    pub fn forward(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> SparseContribution {
        // Project sparse vectors to dense
        let query_e6_dense = self.e6_projector.forward(&query.sparse);
        let query_e13_dense = self.e13_projector.forward(&query.splade);
        let memory_e6_dense = self.e6_projector.forward(&memory.sparse);
        let memory_e13_dense = self.e13_projector.forward(&memory.splade);

        // Sparse attention
        let sparse_output = self.sparse_attention.forward(
            &query_e6_dense,
            &query_e13_dense,
            &memory_e6_dense,
            &memory_e13_dense,
        );

        // Also compute raw sparse similarities for interpretability
        let e6_sim = sparse_jaccard(&query.sparse, &memory.sparse);
        let e13_sim = sparse_dot(&query.splade, &memory.splade);

        SparseContribution {
            attention_output: sparse_output,
            e6_raw_similarity: e6_sim,
            e13_raw_similarity: e13_sim,
            fusion_weight: self.fusion_weight,
        }
    }
}

pub struct SparseContribution {
    pub attention_output: Tensor<[64]>,
    pub e6_raw_similarity: f32,
    pub e13_raw_similarity: f32,
    pub fusion_weight: f32,
}
```

### 7.3 Fusing Dense and Sparse Attention

```rust
/// Combines dense and sparse attention outputs
pub struct AttentionFusion {
    /// Dense attention module
    dense_attention: CrossSpaceAttentionModule,

    /// Sparse attention path
    sparse_attention: SparseAttentionPath,

    /// Learned fusion layer
    fusion_layer: FusionLayer,
}

pub struct FusionLayer {
    /// Projects concatenated outputs to final score
    projection: Linear,  // [128 + 64, 1] → [1]

    /// Learned gate for dense vs sparse contribution
    gate: Linear,  // [128 + 64, 2] → softmax → [dense_weight, sparse_weight]
}

impl AttentionFusion {
    /// Compute final score combining dense and sparse attention
    pub fn score(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> FusedScore {
        // Dense attention on 11 spaces
        let dense_result = self.dense_attention.score(query, memory);

        // Sparse attention on E6, E13
        let sparse_result = self.sparse_attention.forward(query, memory);

        // Fusion
        let dense_output = dense_result.pooled_output;  // [128]
        let sparse_output = sparse_result.attention_output;  // [64]
        let concatenated = Tensor::cat(&[dense_output, sparse_output], dim=0);  // [192]

        // Gated fusion
        let gate_logits = self.fusion_layer.gate.forward(&concatenated);  // [2]
        let gate_weights = softmax(gate_logits, dim=0);  // [dense_w, sparse_w]

        let dense_contribution = gate_weights[0].item() * dense_result.score;
        let sparse_contribution = gate_weights[1].item() * (
            0.5 * sparse_result.e6_raw_similarity +
            0.5 * sparse_result.e13_raw_similarity
        );

        let final_score = dense_contribution + sparse_contribution;

        FusedScore {
            score: final_score,
            dense_score: dense_result.score,
            sparse_e6_score: sparse_result.e6_raw_similarity,
            sparse_e13_score: sparse_result.e13_raw_similarity,
            dense_weight: gate_weights[0].item(),
            sparse_weight: gate_weights[1].item(),
            dense_attention: dense_result.cross_attention,
            per_space_contributions: dense_result.per_space_contributions,
        }
    }
}

pub struct FusedScore {
    pub score: f32,
    pub dense_score: f32,
    pub sparse_e6_score: f32,
    pub sparse_e13_score: f32,
    pub dense_weight: f32,
    pub sparse_weight: f32,
    pub dense_attention: Tensor<[4, 11, 11]>,
    pub per_space_contributions: [f32; 11],
}
```

---

## 8. Training Architecture

### 8.1 Training Data Requirements

```rust
/// Training data structure
pub struct AttentionTrainingData {
    /// Query fingerprint
    pub query: SemanticFingerprint,

    /// Positive memories (relevant to query)
    pub positives: Vec<SemanticFingerprint>,

    /// Hard negatives (similar but not relevant)
    pub hard_negatives: Vec<SemanticFingerprint>,

    /// Relevance labels (optional, for supervised learning)
    pub relevance_labels: Option<Vec<f32>>,
}

/// Data sources for training
pub enum TrainingSource {
    /// User click-through data
    ClickThrough {
        query: String,
        clicked_memory_ids: Vec<MemoryId>,
        skipped_memory_ids: Vec<MemoryId>,
    },

    /// Explicit relevance labels (e.g., from evaluation)
    Labeled {
        query: String,
        memory_id: MemoryId,
        relevance: f32,  // 0-1
    },

    /// Contrastive pairs (same topic = positive, different = negative)
    Contrastive {
        anchor: MemoryId,
        positive: MemoryId,  // Same topic/context
        negatives: Vec<MemoryId>,  // Different topics
    },

    /// Synthetic from existing graph structure
    Synthetic {
        /// Use graph edges as relevance signal
        edge_based: bool,
        /// Use causal paths as relevance signal
        causal_path_based: bool,
        /// Use cluster membership as relevance signal
        cluster_based: bool,
    },
}
```

### 8.2 Loss Functions

```rust
/// Combined loss for attention training
pub struct AttentionLoss {
    /// Contrastive loss weight
    pub contrastive_weight: f32,  // 0.4

    /// Ranking loss weight (listwise)
    pub ranking_weight: f32,  // 0.3

    /// Regularization weight
    pub regularization_weight: f32,  // 0.1

    /// Attention sparsity weight (encourage interpretable patterns)
    pub sparsity_weight: f32,  // 0.1

    /// Asymmetric causal consistency weight
    pub causal_consistency_weight: f32,  // 0.1
}

impl AttentionLoss {
    /// Compute total loss
    pub fn compute(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> (Tensor<[]>, LossBreakdown) {
        // 1. Contrastive loss (InfoNCE)
        let contrastive = self.contrastive_loss(model, batch);

        // 2. Ranking loss (ListMLE)
        let ranking = self.ranking_loss(model, batch);

        // 3. Regularization (attention entropy)
        let regularization = self.regularization_loss(model, batch);

        // 4. Sparsity loss (encourage focused attention)
        let sparsity = self.sparsity_loss(model, batch);

        // 5. Causal consistency (asymmetric attention should match direction)
        let causal = self.causal_consistency_loss(model, batch);

        let total =
            self.contrastive_weight * contrastive +
            self.ranking_weight * ranking +
            self.regularization_weight * regularization +
            self.sparsity_weight * sparsity +
            self.causal_consistency_weight * causal;

        (total, LossBreakdown {
            contrastive: contrastive.item(),
            ranking: ranking.item(),
            regularization: regularization.item(),
            sparsity: sparsity.item(),
            causal_consistency: causal.item(),
            total: total.item(),
        })
    }

    /// InfoNCE contrastive loss
    fn contrastive_loss(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> Tensor<[]> {
        let mut loss = Tensor::zeros([]);

        for sample in &batch.samples {
            let query = &sample.query;

            // Score positive
            let pos_score = model.score(query, &sample.positive).score;

            // Score negatives
            let neg_scores: Vec<f32> = sample.negatives.iter()
                .map(|neg| model.score(query, neg).score)
                .collect();

            // InfoNCE: -log(exp(pos) / (exp(pos) + sum(exp(neg))))
            let temperature = 0.07;
            let pos_exp = (pos_score / temperature).exp();
            let neg_exp_sum: f32 = neg_scores.iter()
                .map(|s| (s / temperature).exp())
                .sum();

            loss = loss - (pos_exp / (pos_exp + neg_exp_sum)).ln();
        }

        loss / batch.samples.len() as f32
    }

    /// ListMLE ranking loss
    fn ranking_loss(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> Tensor<[]> {
        let mut loss = Tensor::zeros([]);

        for sample in &batch.samples {
            if let Some(labels) = &sample.relevance_labels {
                let query = &sample.query;
                let memories = &sample.all_memories;

                // Score all memories
                let scores: Vec<f32> = memories.iter()
                    .map(|mem| model.score(query, mem).score)
                    .collect();

                // ListMLE loss
                let sorted_indices = argsort_desc(labels);
                let mut remaining_scores = scores.clone();

                for &idx in &sorted_indices {
                    let score = remaining_scores[idx];
                    let partition = remaining_scores.iter().map(|s| s.exp()).sum::<f32>();
                    loss = loss - (score.exp() / partition).ln();
                    remaining_scores[idx] = f32::NEG_INFINITY;
                }
            }
        }

        loss / batch.samples.len() as f32
    }

    /// Attention entropy regularization (prevent uniform attention)
    fn regularization_loss(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> Tensor<[]> {
        // Encourage attention to be neither too uniform nor too peaked
        // Target entropy: mid-range
        let target_entropy = 2.0;  // bits

        let mut entropy_loss = Tensor::zeros([]);

        for sample in &batch.samples {
            let result = model.score(&sample.query, &sample.positive);
            let attention = result.cross_attention;  // [4, 11, 11]

            // Compute entropy per head
            for head in 0..4 {
                let head_attn = attention.select(0, head);  // [11, 11]
                let entropy = -(head_attn * head_attn.ln()).sum();
                entropy_loss = entropy_loss + (entropy - target_entropy).abs();
            }
        }

        entropy_loss / (batch.samples.len() * 4) as f32
    }

    /// Sparsity loss (encourage focused attention patterns)
    fn sparsity_loss(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> Tensor<[]> {
        // L1 penalty on attention weights to encourage sparsity
        let mut sparsity = Tensor::zeros([]);

        for sample in &batch.samples {
            let result = model.score(&sample.query, &sample.positive);
            sparsity = sparsity + result.cross_attention.abs().mean();
        }

        sparsity / batch.samples.len() as f32
    }

    /// Causal consistency loss
    fn causal_consistency_loss(
        &self,
        model: &CrossSpaceAttentionModule,
        batch: &AttentionTrainingBatch,
    ) -> Tensor<[]> {
        // Attention from causal space should be higher than attention to causal space
        // (matches asymmetric 1.2x cause→effect, 0.8x effect→cause)
        let causal_idx = 4;
        let mut loss = Tensor::zeros([]);

        for sample in &batch.samples {
            let result = model.score(&sample.query, &sample.positive);
            let attention = result.cross_attention.mean(dim=0);  // Average across heads

            // Sum of attention FROM causal to others
            let causal_to_others: f32 = (0..11)
                .filter(|&j| j != causal_idx)
                .map(|j| attention[[causal_idx, j]].item())
                .sum();

            // Sum of attention TO causal from others
            let others_to_causal: f32 = (0..11)
                .filter(|&i| i != causal_idx)
                .map(|i| attention[[i, causal_idx]].item())
                .sum();

            // Loss if others_to_causal > causal_to_others (violates asymmetry)
            let margin = 0.2;  // causal_to_others should be at least 20% higher
            loss = loss + relu(others_to_causal - causal_to_others + margin);
        }

        loss / batch.samples.len() as f32
    }
}

pub struct LossBreakdown {
    pub contrastive: f32,
    pub ranking: f32,
    pub regularization: f32,
    pub sparsity: f32,
    pub causal_consistency: f32,
    pub total: f32,
}
```

### 8.3 Training Loop

```rust
/// Training configuration
pub struct TrainingConfig {
    pub learning_rate: f32,        // 1e-4
    pub warmup_steps: usize,       // 1000
    pub total_steps: usize,        // 100_000
    pub batch_size: usize,         // 32
    pub gradient_accumulation: usize,  // 4
    pub eval_every: usize,         // 1000
    pub save_every: usize,         // 5000
    pub max_grad_norm: f32,        // 1.0
    pub weight_decay: f32,         // 0.01
}

/// Training loop
pub struct AttentionTrainer {
    model: CrossSpaceAttentionModule,
    optimizer: AdamW,
    scheduler: LinearWarmupScheduler,
    loss_fn: AttentionLoss,
    config: TrainingConfig,
}

impl AttentionTrainer {
    pub async fn train(
        &mut self,
        train_data: &AttentionDataset,
        eval_data: &AttentionDataset,
    ) -> TrainingResult {
        let mut best_eval_score = 0.0;
        let mut training_history = Vec::new();

        for step in 0..self.config.total_steps {
            // Sample batch
            let batch = train_data.sample_batch(self.config.batch_size);

            // Forward pass
            let (loss, breakdown) = self.loss_fn.compute(&self.model, &batch);

            // Backward pass
            loss.backward();

            // Gradient accumulation
            if (step + 1) % self.config.gradient_accumulation == 0 {
                // Gradient clipping
                clip_grad_norm(self.model.parameters(), self.config.max_grad_norm);

                // Optimizer step
                self.optimizer.step();
                self.optimizer.zero_grad();

                // Learning rate scheduling
                self.scheduler.step();
            }

            // Logging
            if step % 100 == 0 {
                log::info!(
                    "Step {}: loss={:.4}, contrastive={:.4}, ranking={:.4}, lr={:.6}",
                    step,
                    breakdown.total,
                    breakdown.contrastive,
                    breakdown.ranking,
                    self.scheduler.get_lr(),
                );
            }

            // Evaluation
            if step % self.config.eval_every == 0 {
                let eval_result = self.evaluate(eval_data).await;
                training_history.push((step, breakdown.clone(), eval_result.clone()));

                if eval_result.ndcg_at_10 > best_eval_score {
                    best_eval_score = eval_result.ndcg_at_10;
                    self.save_checkpoint("best_model.pt");
                }

                log::info!(
                    "Eval at step {}: NDCG@10={:.4}, MRR={:.4}, Recall@10={:.4}",
                    step,
                    eval_result.ndcg_at_10,
                    eval_result.mrr,
                    eval_result.recall_at_10,
                );
            }

            // Checkpointing
            if step % self.config.save_every == 0 {
                self.save_checkpoint(&format!("checkpoint_{}.pt", step));
            }
        }

        TrainingResult {
            final_eval: self.evaluate(eval_data).await,
            best_eval_score,
            training_history,
        }
    }

    async fn evaluate(&self, eval_data: &AttentionDataset) -> EvalResult {
        self.model.eval();

        let mut ndcg_scores = Vec::new();
        let mut mrr_scores = Vec::new();
        let mut recall_scores = Vec::new();

        for sample in eval_data.iter() {
            // Get scores for all candidates
            let scores: Vec<f32> = sample.all_memories.iter()
                .map(|mem| self.model.score(&sample.query, mem).score)
                .collect();

            // Compute metrics
            let labels = sample.relevance_labels.as_ref().unwrap();
            ndcg_scores.push(ndcg_at_k(&scores, labels, 10));
            mrr_scores.push(mrr(&scores, labels));
            recall_scores.push(recall_at_k(&scores, labels, 10));
        }

        self.model.train();

        EvalResult {
            ndcg_at_10: ndcg_scores.iter().sum::<f32>() / ndcg_scores.len() as f32,
            mrr: mrr_scores.iter().sum::<f32>() / mrr_scores.len() as f32,
            recall_at_10: recall_scores.iter().sum::<f32>() / recall_scores.len() as f32,
        }
    }
}

#[derive(Clone)]
pub struct EvalResult {
    pub ndcg_at_10: f32,
    pub mrr: f32,
    pub recall_at_10: f32,
}
```

### 8.4 Synthetic Training Data Generation

```rust
/// Generate training data from existing graph structure
pub struct SyntheticDataGenerator {
    graph: KnowledgeGraph,
    embedder: MultiEmbedder,
}

impl SyntheticDataGenerator {
    /// Generate contrastive pairs from graph structure
    pub async fn generate_contrastive_pairs(
        &self,
        num_samples: usize,
    ) -> Vec<AttentionTrainingSample> {
        let mut samples = Vec::new();

        for _ in 0..num_samples {
            // Sample anchor node
            let anchor = self.graph.sample_random_node();

            // Positive: nodes connected by strong edges
            let positives: Vec<_> = self.graph
                .get_neighbors(&anchor.id, EdgeFilter::StrongConnection(0.7))
                .into_iter()
                .take(3)
                .collect();

            if positives.is_empty() {
                continue;
            }

            // Hard negatives: similar embedding but no edge connection
            let hard_negatives = self.find_hard_negatives(&anchor, &positives, 10);

            // Create sample with fingerprints
            let query_fp = self.node_to_fingerprint(&anchor).await;
            let positive_fps: Vec<_> = stream::iter(&positives)
                .then(|n| self.node_to_fingerprint(n))
                .collect()
                .await;
            let negative_fps: Vec<_> = stream::iter(&hard_negatives)
                .then(|n| self.node_to_fingerprint(n))
                .collect()
                .await;

            samples.push(AttentionTrainingSample {
                query: query_fp,
                positive: positive_fps[0].clone(),
                negatives: negative_fps,
                relevance_labels: None,
            });
        }

        samples
    }

    /// Generate pairs from causal paths
    pub async fn generate_causal_pairs(
        &self,
        num_samples: usize,
    ) -> Vec<AttentionTrainingSample> {
        let mut samples = Vec::new();

        for _ in 0..num_samples {
            // Find a causal path in the graph
            if let Some(path) = self.graph.sample_causal_path(3, 6) {
                let query = &path[0];
                let positive = &path[path.len() - 1];  // End of causal chain

                // Negatives: nodes not on causal path
                let negatives = self.sample_non_causal_nodes(query, 10);

                let query_fp = self.node_to_fingerprint(query).await;
                let positive_fp = self.node_to_fingerprint(positive).await;
                let negative_fps: Vec<_> = stream::iter(&negatives)
                    .then(|n| self.node_to_fingerprint(n))
                    .collect()
                    .await;

                // Mark with causal directions
                let mut query_fp = query_fp;
                query_fp.causal.direction = CausalDirection::Cause;
                let mut positive_fp = positive_fp;
                positive_fp.causal.direction = CausalDirection::Effect;

                samples.push(AttentionTrainingSample {
                    query: query_fp,
                    positive: positive_fp,
                    negatives: negative_fps,
                    relevance_labels: None,
                });
            }
        }

        samples
    }

    /// Find hard negatives (similar but not connected)
    fn find_hard_negatives(
        &self,
        anchor: &KnowledgeNode,
        positives: &[KnowledgeNode],
        k: usize,
    ) -> Vec<KnowledgeNode> {
        // Get nodes similar to anchor in E1 semantic space
        let similar = self.graph.search_similar(
            &anchor.fingerprint.semantic,
            k * 3,
        );

        // Filter out positives and anchor
        let positive_ids: HashSet<_> = positives.iter()
            .map(|n| n.id)
            .chain(std::iter::once(anchor.id))
            .collect();

        similar.into_iter()
            .filter(|n| !positive_ids.contains(&n.id))
            .filter(|n| !self.graph.has_edge(&anchor.id, &n.id))
            .take(k)
            .collect()
    }
}
```

---

## 9. Inference Optimization

### 9.1 Model Optimization

```rust
/// Optimized inference module
pub struct OptimizedCrossSpaceAttention {
    /// Base model (for training)
    base_model: CrossSpaceAttentionModule,

    /// Quantized model (for inference)
    quantized_model: Option<QuantizedAttentionModule>,

    /// TensorRT/ONNX optimized model
    tensorrt_model: Option<TensorRTModel>,

    /// Configuration
    optimization_config: OptimizationConfig,
}

pub struct OptimizationConfig {
    /// Use FP16 inference
    pub use_fp16: bool,  // Default: true

    /// Use INT8 quantization
    pub use_int8: bool,  // Default: false (requires calibration)

    /// Use TensorRT optimization
    pub use_tensorrt: bool,  // Default: true if available

    /// Batch inference size
    pub inference_batch_size: usize,  // Default: 64

    /// Use Flash Attention
    pub use_flash_attention: bool,  // Default: true
}

impl OptimizedCrossSpaceAttention {
    /// Score with optimized inference
    pub fn score_optimized(
        &self,
        query: &SemanticFingerprint,
        memories: &[SemanticFingerprint],
    ) -> Vec<f32> {
        if let Some(ref trt) = self.tensorrt_model {
            // TensorRT path (fastest)
            return trt.score_batch(query, memories);
        }

        if let Some(ref quantized) = self.quantized_model {
            // Quantized path
            return quantized.score_batch(query, memories);
        }

        // Base model path
        self.base_model.score_batch(query, memories)
            .into_iter()
            .map(|s| s.score)
            .collect()
    }

    /// Export to ONNX for optimization
    pub fn export_onnx(&self, path: &Path) -> Result<()> {
        // Create dummy inputs
        let dummy_query = DenseFingerprint::zeros();
        let dummy_memory = DenseFingerprint::zeros();

        // Trace model
        let traced = torch::jit::trace(
            &self.base_model,
            &[dummy_query.to_tensor(), dummy_memory.to_tensor()],
        )?;

        // Export to ONNX
        traced.save(path)?;

        Ok(())
    }

    /// Quantize model for faster inference
    pub fn quantize(&mut self, calibration_data: &[AttentionTrainingSample]) -> Result<()> {
        // Dynamic quantization (no calibration needed)
        if !self.optimization_config.use_int8 {
            self.quantized_model = Some(QuantizedAttentionModule::from_dynamic(
                &self.base_model,
            ));
            return Ok(());
        }

        // Static INT8 quantization (requires calibration)
        let calibration_stats = self.collect_calibration_stats(calibration_data);
        self.quantized_model = Some(QuantizedAttentionModule::from_static(
            &self.base_model,
            calibration_stats,
        ));

        Ok(())
    }
}
```

### 9.2 Caching Strategy

```rust
/// Cache for attention computations
pub struct AttentionCache {
    /// Cache of projected fingerprints
    projection_cache: LruCache<MemoryId, ProjectedFingerprint>,

    /// Cache of attention scores (query_hash, memory_id) → score
    score_cache: LruCache<(u64, MemoryId), f32>,

    /// Cache configuration
    config: CacheConfig,
}

pub struct CacheConfig {
    /// Max cached projections
    pub max_projections: usize,  // 100_000

    /// Max cached scores
    pub max_scores: usize,  // 1_000_000

    /// TTL for score cache (queries change frequently)
    pub score_ttl: Duration,  // 5 minutes

    /// TTL for projection cache (memories change rarely)
    pub projection_ttl: Duration,  // 1 hour
}

impl AttentionCache {
    /// Get or compute projected fingerprint
    pub fn get_projection(
        &mut self,
        memory_id: MemoryId,
        fingerprint: &SemanticFingerprint,
        projector: &SpaceProjections,
    ) -> ProjectedFingerprint {
        if let Some(cached) = self.projection_cache.get(&memory_id) {
            return cached.clone();
        }

        let dense = DenseFingerprint::from_semantic_fingerprint(fingerprint);
        let projected = projector.forward(&dense);

        let result = ProjectedFingerprint {
            projected,
            computed_at: Instant::now(),
        };

        self.projection_cache.put(memory_id, result.clone());
        result
    }

    /// Get or compute attention score
    pub fn get_score(
        &mut self,
        query_hash: u64,
        memory_id: MemoryId,
        compute_fn: impl FnOnce() -> f32,
    ) -> f32 {
        let key = (query_hash, memory_id);

        if let Some(&cached) = self.score_cache.get(&key) {
            return cached;
        }

        let score = compute_fn();
        self.score_cache.put(key, score);
        score
    }

    /// Invalidate cache for updated memory
    pub fn invalidate_memory(&mut self, memory_id: MemoryId) {
        self.projection_cache.pop(&memory_id);
        // Score cache entries will expire via TTL
    }
}
```

### 9.3 Latency Budget

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ATTENTION LATENCY BUDGET (Stage 3)                        │
│                    Target: <30ms for 500 candidates                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Operation                         | Per-Item | Batch (500) | Optimization  │
│  ─────────────────────────────────────────────────────────────────────────  │
│  Load fingerprints from cache      | 0.01ms   | 5ms         | LRU cache     │
│  Extract dense portions            | 0.005ms  | 2.5ms       | SIMD          │
│  Project to common dim (11×)       | 0.01ms   | 5ms         | Batched matmul│
│  Query self-attention              | 0.005ms  | 2.5ms       | Flash Attn    │
│  Cross-attention                   | 0.01ms   | 5ms         | Flash Attn    │
│  FFN + output projection           | 0.005ms  | 2.5ms       | Fused ops     │
│  Sparse attention path             | 0.005ms  | 2.5ms       | Sparse ops    │
│  Fusion + final score              | 0.002ms  | 1ms         | Simple ops    │
│  ─────────────────────────────────────────────────────────────────────────  │
│  TOTAL                             | 0.052ms  | 26ms        |               │
│  + Overhead (scheduling, etc.)     |          | 4ms         |               │
│  ─────────────────────────────────────────────────────────────────────────  │
│  FINAL                             |          | <30ms       | ✓ Within budget│
│                                                                              │
│  With TensorRT optimization: <20ms                                          │
│  With INT8 quantization: <15ms                                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 10. Implementation Plan

### 10.1 Phase Overview

| Phase | Duration | Focus | Deliverables |
|-------|----------|-------|--------------|
| **Phase 1** | 2 weeks | Foundation | Core attention module, projections |
| **Phase 2** | 2 weeks | Training | Loss functions, training loop, synthetic data |
| **Phase 3** | 2 weeks | Integration | Pipeline integration, caching, optimization |
| **Phase 4** | 1 week | Evaluation | A/B testing, benchmarks, tuning |
| **Phase 5** | 1 week | Deployment | Production rollout, monitoring |

### 10.2 Phase 1: Foundation (Weeks 1-2)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 1.1 | Define `DenseFingerprint` struct | Core | Pending |
| 1.2 | Implement `SpaceProjections` module | Core | Pending |
| 1.3 | Implement `MultiHeadAttention` base | Core | Pending |
| 1.4 | Implement `AsymmetricCausalAttention` | Core | Pending |
| 1.5 | Implement `CrossSpaceAttentionModule` | Core | Pending |
| 1.6 | Implement `SparseAttentionPath` | Core | Pending |
| 1.7 | Implement `AttentionFusion` | Core | Pending |
| 1.8 | Unit tests for all components | QA | Pending |

### 10.3 Phase 2: Training (Weeks 3-4)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 2.1 | Define training data structures | ML | Pending |
| 2.2 | Implement `AttentionLoss` functions | ML | Pending |
| 2.3 | Implement `AttentionTrainer` | ML | Pending |
| 2.4 | Implement `SyntheticDataGenerator` | ML | Pending |
| 2.5 | Create training pipeline | ML | Pending |
| 2.6 | Initial training run (10K samples) | ML | Pending |
| 2.7 | Hyperparameter tuning | ML | Pending |
| 2.8 | Full training run (100K+ samples) | ML | Pending |

### 10.4 Phase 3: Integration (Weeks 5-6)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 3.1 | Modify `EnhancedRetrievalPipeline` Stage 3 | Core | Pending |
| 3.2 | Implement `AttentionCache` | Core | Pending |
| 3.3 | Implement `OptimizedCrossSpaceAttention` | Perf | Pending |
| 3.4 | ONNX/TensorRT export | Perf | Pending |
| 3.5 | Quantization pipeline | Perf | Pending |
| 3.6 | Integration tests | QA | Pending |
| 3.7 | Latency benchmarks | QA | Pending |
| 3.8 | Memory usage profiling | QA | Pending |

### 10.5 Phase 4: Evaluation (Week 7)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 4.1 | Set up A/B testing framework | Infra | Pending |
| 4.2 | Relevance evaluation (NDCG, MRR) | QA | Pending |
| 4.3 | Latency regression testing | QA | Pending |
| 4.4 | Attention pattern analysis | ML | Pending |
| 4.5 | Interpretability review | ML | Pending |
| 4.6 | Fine-tune based on results | ML | Pending |

### 10.6 Phase 5: Deployment (Week 8)

| Task | Description | Owner | Status |
|------|-------------|-------|--------|
| 5.1 | Staged rollout (1% → 10% → 50% → 100%) | Infra | Pending |
| 5.2 | Monitoring dashboard setup | Infra | Pending |
| 5.3 | Alerting for latency regressions | Infra | Pending |
| 5.4 | Documentation | Docs | Pending |
| 5.5 | Post-launch review | All | Pending |

---

## 11. Mathematical Foundations

### 11.1 Scaled Dot-Product Attention

$$
\text{Attention}(Q, K, V) = \text{softmax}\left(\frac{QK^T}{\sqrt{d_k}}\right)V
$$

Where:
- $Q \in \mathbb{R}^{n \times d_k}$ — Query matrix (n = 11 spaces)
- $K \in \mathbb{R}^{n \times d_k}$ — Key matrix
- $V \in \mathbb{R}^{n \times d_v}$ — Value matrix
- $d_k$ — Key dimension (32 per head)
- $\sqrt{d_k}$ — Scaling factor for stable gradients

### 11.2 Multi-Head Attention

$$
\text{MultiHead}(Q, K, V) = \text{Concat}(\text{head}_1, ..., \text{head}_h)W^O
$$

$$
\text{head}_i = \text{Attention}(QW_i^Q, KW_i^K, VW_i^V)
$$

Where:
- $h = 4$ — Number of heads
- $W_i^Q, W_i^K \in \mathbb{R}^{d_{model} \times d_k}$
- $W_i^V \in \mathbb{R}^{d_{model} \times d_v}$
- $W^O \in \mathbb{R}^{hd_v \times d_{model}}$

### 11.3 Asymmetric Causal Attention

For the causal space (index 4), we modify attention:

$$
\text{Attention}_{\text{asym}}(Q, K, V) = \text{softmax}\left(\frac{QK^T}{\sqrt{d_k}} + M_{\text{causal}}\right)V
$$

Where $M_{\text{causal}} \in \mathbb{R}^{11 \times 11}$:

$$
M_{\text{causal}}[i,j] = \begin{cases}
\ln(1.2) & \text{if } i = 4 \text{ (causal is query)} \\
\ln(0.8) & \text{if } j = 4 \text{ (causal is key)} \\
0 & \text{otherwise}
\end{cases}
$$

This ensures:
- When causal space queries others: $\times 1.2$ boost
- When others query causal space: $\times 0.8$ dampen

### 11.4 InfoNCE Contrastive Loss

$$
\mathcal{L}_{\text{InfoNCE}} = -\log \frac{\exp(s(q, p^+) / \tau)}{\exp(s(q, p^+) / \tau) + \sum_{n \in N} \exp(s(q, n) / \tau)}
$$

Where:
- $s(q, p)$ — Attention score between query and memory
- $p^+$ — Positive memory
- $N$ — Set of negative memories
- $\tau = 0.07$ — Temperature

### 11.5 Attention Entropy for Regularization

$$
H(\text{Attention}) = -\sum_{i,j} A_{ij} \log A_{ij}
$$

Target entropy: $H^* \approx 2$ bits (neither uniform nor peaked)

Regularization loss:
$$
\mathcal{L}_{\text{reg}} = |H(\text{Attention}) - H^*|
$$

### 11.6 Cross-Space Information Flow

Define information flow from space $i$ to space $j$:

$$
\text{Flow}_{i \to j} = \sum_{\text{heads}} A^{(h)}_{ij}
$$

The attention module learns optimal flow patterns:
- Semantic → Causal: High flow for reasoning tasks
- Code → Graph: High flow for structure understanding
- Temporal → Entity: Low flow (orthogonal information)

---

## 12. Validation & Benchmarks

### 12.1 Relevance Metrics

| Metric | Formula | Target |
|--------|---------|--------|
| NDCG@10 | $\frac{DCG@10}{IDCG@10}$ | > 0.75 |
| MRR | $\frac{1}{|Q|} \sum_{i=1}^{|Q|} \frac{1}{\text{rank}_i}$ | > 0.65 |
| Recall@10 | $\frac{|\text{relevant} \cap \text{retrieved}|}{|\text{relevant}|}$ | > 0.85 |
| MAP | Mean Average Precision | > 0.70 |

### 12.2 Latency Benchmarks

| Scenario | Current (RRF) | Target (Attention) | Max Regression |
|----------|--------------|-------------------|----------------|
| Stage 3 (100 candidates) | 5ms | 10ms | 2x |
| Stage 3 (500 candidates) | 15ms | 30ms | 2x |
| Full pipeline (1M corpus) | 60ms | 75ms | 25% |
| Full pipeline (100K corpus) | 30ms | 40ms | 33% |

### 12.3 Quality Gates

| Gate | Condition | Action if Failed |
|------|-----------|------------------|
| Relevance regression | NDCG@10 drops > 5% | Block deployment |
| Latency regression | P99 > 100ms | Disable attention, fallback to RRF |
| Memory usage | > 2GB model size | Increase quantization |
| Attention collapse | Entropy < 0.5 bits | Retrain with higher regularization |

### 12.4 A/B Testing Plan

```
Week 1: 1% traffic with attention
  - Monitor latency, errors, user satisfaction proxy

Week 2: 10% traffic
  - Statistical significance on relevance metrics

Week 3: 50% traffic
  - Full evaluation, edge case discovery

Week 4: 100% traffic (if all gates pass)
  - Continue monitoring for 2 weeks post-launch
```

### 12.5 Interpretability Validation

For each test query, verify:
1. **Attention makes sense**: Human review of top attention pairs
2. **Causal asymmetry preserved**: Cause→effect scores > effect→cause
3. **Space contributions match expectations**: Code queries attend to E7, causal queries attend to E5
4. **No attention collapse**: All heads have distinct patterns

---

## Appendix A: Glossary

- **Cross-Space Attention**: Attention between different embedding spaces (E1-E13)
- **Self-Attention**: Query attending to itself (contextualizing query spaces)
- **Asymmetric Attention**: Different weights for A→B vs B→A
- **Flash Attention**: Memory-efficient attention implementation
- **InfoNCE**: Contrastive loss function (Noise Contrastive Estimation)
- **RRF**: Reciprocal Rank Fusion (current Stage 3 method)

## Appendix B: References

1. Vaswani et al. (2017) - "Attention Is All You Need"
2. Devlin et al. (2019) - "BERT: Pre-training of Deep Bidirectional Transformers"
3. Khattab & Zaharia (2020) - "ColBERT: Efficient and Effective Passage Search"
4. Dao et al. (2022) - "FlashAttention: Fast and Memory-Efficient Exact Attention"
5. Hofstätter et al. (2021) - "Efficiently Teaching an Effective Dense Retriever"
6. Xiong et al. (2020) - "Approximate Nearest Neighbor Negative Contrastive Learning"
7. projectionplan1.md - Multi-Array Semantic Fingerprint Architecture
8. projectionplan2.md - Teleological Storage Architecture
9. contextprd.md - Context Graph PRD

---

## Appendix C: Configuration Defaults

```yaml
# attention_config.yaml

model:
  d_model: 128
  num_heads: 4
  d_head: 32
  dropout: 0.1
  num_dense_spaces: 11
  asymmetric_causal: true
  temperature: 1.0

training:
  learning_rate: 1e-4
  warmup_steps: 1000
  total_steps: 100000
  batch_size: 32
  gradient_accumulation: 4
  max_grad_norm: 1.0
  weight_decay: 0.01

loss:
  contrastive_weight: 0.4
  ranking_weight: 0.3
  regularization_weight: 0.1
  sparsity_weight: 0.1
  causal_consistency_weight: 0.1

optimization:
  use_fp16: true
  use_int8: false
  use_tensorrt: true
  use_flash_attention: true
  inference_batch_size: 64

cache:
  max_projections: 100000
  max_scores: 1000000
  score_ttl_minutes: 5
  projection_ttl_minutes: 60

pipeline:
  stage2_top_k: 500
  stage3_top_k: 100
  attention_enabled: true
  fallback_to_rrf: true
```

---

**Document Status**: PROPOSED
**Next Steps**: Review by architecture team, then begin Phase 1 implementation
