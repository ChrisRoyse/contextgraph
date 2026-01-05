# Phase 2 Implementation Guide: Johari Quadrants per Embedder

**Document**: `phase2imp.md`
**Version**: 1.0.0
**Created**: 2026-01-05
**Status**: IMPLEMENTATION SPECIFICATION
**Prerequisites**: Phase 1 complete (12-embedding SemanticFingerprint storage operational)

---

## Executive Summary

This guide provides a complete implementation specification for the Johari Quadrants per Embedder feature. The Johari Window, originally a psychological self-awareness model, is adapted here as an **information-theoretic classification system** that operates independently across each of the 12 embedding spaces.

**Core Insight**: A memory can be "Open" (well-understood) in the semantic space but "Blind" (novel but unintegrated) in the causal space — enabling targeted knowledge gap detection.

---

## Table of Contents

1. [Theoretical Foundation](#1-theoretical-foundation)
2. [Mathematical Framework](#2-mathematical-framework)
3. [Computing ΔS (Entropy/Novelty)](#3-computing-δs-entropynovelty)
4. [Computing ΔC (Coherence/Integration)](#4-computing-δc-coherenceintegration)
5. [Johari Classification Logic](#5-johari-classification-logic)
6. [Data Structures](#6-data-structures)
7. [Implementation: Core Classifier](#7-implementation-core-classifier)
8. [Implementation: Per-Space Strategies](#8-implementation-per-space-strategies)
9. [Calibration and Thresholds](#9-calibration-and-thresholds)
10. [Integration with SemanticFingerprint](#10-integration-with-semanticfingerprint)
11. [Cross-Space Analysis](#11-cross-space-analysis)
12. [Testing and Validation](#12-testing-and-validation)
13. [Performance Optimization](#13-performance-optimization)
14. [Monitoring and Observability](#14-monitoring-and-observability)
15. [Migration Strategy](#15-migration-strategy)
16. [References](#16-references)

---

## 1. Theoretical Foundation

### 1.1 Why Johari Quadrants Are NOT Geometric Regions

A common misconception: the Johari quadrants are **not** geometric regions within an embedding space. You cannot draw boundaries in a 1024-dimensional space and say "this region is Open, that region is Blind."

Instead, Johari classification operates at the **relationship level** — classifying the information-theoretic state of a specific memory node relative to your entire knowledge corpus.

### 1.2 The Correct Interpretation

For each embedding space `i` (E1-E12), we compute two metrics:

| Metric | Symbol | Meaning | Computation Method |
|--------|--------|---------|-------------------|
| **Entropy** | ΔSᵢ | How novel/surprising is this embedding? | Density estimation, OOD detection |
| **Coherence** | ΔCᵢ | How well does it integrate with existing knowledge? | Graph connectivity, cluster fit |

The Johari quadrant is then determined by the combination:

```
           ┌─────────────────┬─────────────────┐
           │   High ΔC       │   High ΔC       │
           │   Low ΔS        │   High ΔS       │
           │                 │                 │
           │     OPEN        │    UNKNOWN      │
           │  (Well-known)   │   (Frontier)    │
           ├─────────────────┼─────────────────┤
           │   Low ΔC        │   Low ΔC        │
           │   Low ΔS        │   High ΔS       │
           │                 │                 │
           │    HIDDEN       │     BLIND       │
           │   (Dormant)     │  (Discovery)    │
           └─────────────────┴─────────────────┘
                        ΔS →
```

### 1.3 Research Foundation

| Concept | Research Source | Application |
|---------|-----------------|-------------|
| Density in latent space | [Postels et al., 2020](https://deepai.org/publication/quantifying-aleatoric-and-epistemic-uncertainty-using-density-estimation-in-latent-space) | ΔS computation via GMM |
| OOD detection | [Sun et al., ICML 2022](https://proceedings.mlr.press/v162/sun22d/sun22d.pdf) | KNN distance for novelty |
| Epistemic uncertainty | [DDU Method](http://www.gatsby.ucl.ac.uk/~balaji/udl2021/accepted-papers/UDL2021-paper-022.pdf) | Feature-space density = uncertainty |
| Semantic entropy | [Farquhar et al., 2024](https://www.researchgate.net/publication/381666455_Semantic_Entropy_Probes_Robust_and_Cheap_Hallucination_Detection_in_LLMs) | Semantic uncertainty estimation |
| KG coherence | [WikiData Classification, 2025](https://arxiv.org/html/2511.04926) | Embedding-based consistency detection |
| Topic coherence | [ACM SIGIR 2016](https://dl.acm.org/doi/10.1145/2911451.2914729) | Word embedding coherence metrics |

---

## 2. Mathematical Framework

### 2.1 Formal Definitions

Let:
- `E = {E₁, E₂, ..., E₁₂}` be the 12 embedding spaces
- `eᵢ(m) ∈ ℝ^{dᵢ}` be the embedding of memory `m` in space `Eᵢ`
- `C` be the existing corpus of memories
- `G` be the knowledge graph

**Entropy (ΔS) for space i:**
```
ΔSᵢ(m) = 1 - P(eᵢ(m) | Cᵢ)
```
Where `P(eᵢ(m) | Cᵢ)` is the probability density of `eᵢ(m)` under the corpus distribution in space `i`.

**Coherence (ΔC) for space i:**
```
ΔCᵢ(m) = α · connectivity(m, G, i) + β · cluster_fit(eᵢ(m), Cᵢ) + γ · consistency(m, G, i)
```
Where `α + β + γ = 1` are tunable weights.

### 2.2 Johari Classification Function

```
Jᵢ(m) = {
    Open    if ΔSᵢ ≤ θ_S ∧ ΔCᵢ > θ_C
    Blind   if ΔSᵢ > θ_S ∧ ΔCᵢ ≤ θ_C
    Hidden  if ΔSᵢ ≤ θ_S ∧ ΔCᵢ ≤ θ_C
    Unknown if ΔSᵢ > θ_S ∧ ΔCᵢ > θ_C
}
```

Default thresholds: `θ_S = 0.5`, `θ_C = 0.5` (calibrate empirically)

### 2.3 Confidence Estimation

Each classification includes a confidence score:
```
confidence(Jᵢ) = |ΔSᵢ - θ_S| + |ΔCᵢ - θ_C|
```
Higher distance from thresholds = higher confidence.

---

## 3. Computing ΔS (Entropy/Novelty)

### 3.1 Method Selection by Embedding Space

Different embedding spaces require different entropy estimation strategies:

| Space | Dim | Recommended ΔS Method | Rationale |
|-------|-----|----------------------|-----------|
| E1 Semantic | 1024 | GMM + Mahalanobis | High-dim, Gaussian assumption valid |
| E2-E4 Temporal | 512 | KNN distance | Time-aware, local density |
| E5 Causal | 768 | Asymmetric KNN | Directional (cause→effect) |
| E6 Sparse | ~30K | Inverse Document Frequency | Sparse = use TF-IDF style |
| E7 Code | 1536 | GMM + KNN ensemble | High-dim, structured |
| E8 Graph | 384 | KNN distance | Lower dim, local density |
| E9 HDC | 1024 | Hamming distance to prototypes | Binary/hyperdimensional |
| E10 Multimodal | 768 | Cross-modal KNN | Multi-modal density |
| E11 Entity | 384 | TransE distance | Relation-aware |
| E12 Late | 128/tok | Token-level KNN | Per-token novelty |

### 3.2 KNN-Based Entropy (Primary Method)

```rust
/// KNN-based entropy estimation
/// Research: https://proceedings.mlr.press/v162/sun22d/sun22d.pdf
pub struct KnnEntropyEstimator {
    /// HNSW index for fast approximate nearest neighbor
    index: HnswIndex,
    /// Number of neighbors for density estimation
    k: usize,
    /// Normalization statistics (computed from corpus)
    mean_knn_dist: f32,
    std_knn_dist: f32,
}

impl KnnEntropyEstimator {
    pub fn new(k: usize) -> Self {
        Self {
            index: HnswIndex::new(HnswConfig::default()),
            k,
            mean_knn_dist: 0.0,
            std_knn_dist: 1.0,
        }
    }

    /// Compute entropy for a new embedding
    pub fn compute_entropy(&self, embedding: &[f32]) -> f32 {
        // Get k-th nearest neighbor distance
        let neighbors = self.index.search(embedding, self.k);
        let kth_distance = neighbors.last()
            .map(|(_, dist)| *dist)
            .unwrap_or(f32::MAX);

        // Z-score normalization
        let z_score = (kth_distance - self.mean_knn_dist) / self.std_knn_dist;

        // Sigmoid to [0, 1] where 1 = high entropy (novel)
        sigmoid(z_score)
    }

    /// Update normalization statistics from corpus
    pub fn calibrate(&mut self, corpus: &[Vec<f32>]) {
        let distances: Vec<f32> = corpus.iter()
            .map(|emb| {
                let neighbors = self.index.search(emb, self.k + 1); // +1 to exclude self
                neighbors.get(self.k)
                    .map(|(_, dist)| *dist)
                    .unwrap_or(0.0)
            })
            .collect();

        self.mean_knn_dist = mean(&distances);
        self.std_knn_dist = std_dev(&distances).max(1e-6);
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
```

### 3.3 GMM-Based Entropy (High-Dimensional Spaces)

```rust
/// Gaussian Mixture Model for density estimation
/// Research: https://deepai.org/publication/quantifying-aleatoric-and-epistemic-uncertainty-using-density-estimation-in-latent-space
pub struct GmmEntropyEstimator {
    /// Fitted GMM model
    gmm: GaussianMixture,
    /// Number of components
    n_components: usize,
    /// Normalization: log-likelihood range from corpus
    min_ll: f32,
    max_ll: f32,
}

impl GmmEntropyEstimator {
    pub fn new(n_components: usize) -> Self {
        Self {
            gmm: GaussianMixture::new(n_components),
            n_components,
            min_ll: f32::NEG_INFINITY,
            max_ll: 0.0,
        }
    }

    /// Fit GMM to corpus embeddings
    pub fn fit(&mut self, corpus: &[Vec<f32>]) -> Result<(), GmmError> {
        self.gmm.fit(corpus)?;

        // Compute normalization from corpus log-likelihoods
        let log_likelihoods: Vec<f32> = corpus.iter()
            .map(|emb| self.gmm.log_likelihood(emb))
            .collect();

        self.min_ll = log_likelihoods.iter().cloned().fold(f32::INFINITY, f32::min);
        self.max_ll = log_likelihoods.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        Ok(())
    }

    /// Compute entropy (novelty) for a new embedding
    pub fn compute_entropy(&self, embedding: &[f32]) -> f32 {
        let ll = self.gmm.log_likelihood(embedding);

        // Normalize to [0, 1] where 1 = low likelihood = high entropy
        let normalized = (self.max_ll - ll) / (self.max_ll - self.min_ll + 1e-6);
        normalized.clamp(0.0, 1.0)
    }
}
```

### 3.4 Mahalanobis Distance (Class-Conditional)

```rust
/// Mahalanobis distance for OOD detection
/// Research: https://arxiv.org/abs/1807.03888
pub struct MahalanobisEntropyEstimator {
    /// Per-cluster mean vectors
    cluster_means: Vec<Vec<f32>>,
    /// Per-cluster precision matrices (inverse covariance)
    cluster_precisions: Vec<Array2<f32>>,
    /// Normalization threshold (e.g., 95th percentile of corpus)
    threshold: f32,
}

impl MahalanobisEntropyEstimator {
    /// Compute Mahalanobis distance to nearest cluster
    pub fn compute_entropy(&self, embedding: &[f32]) -> f32 {
        let min_distance = self.cluster_means.iter()
            .zip(&self.cluster_precisions)
            .map(|(mean, precision)| {
                let diff: Vec<f32> = embedding.iter()
                    .zip(mean.iter())
                    .map(|(a, b)| a - b)
                    .collect();

                // d = sqrt((x - μ)ᵀ Σ⁻¹ (x - μ))
                mahalanobis_distance(&diff, precision)
            })
            .fold(f32::INFINITY, f32::min);

        // Normalize: higher distance = higher entropy
        (min_distance / self.threshold).min(1.0)
    }
}
```

### 3.5 Ensemble Entropy Estimator

```rust
/// Ensemble of entropy estimators for robust ΔS computation
pub struct EnsembleEntropyEstimator {
    knn: KnnEntropyEstimator,
    gmm: GmmEntropyEstimator,
    mahalanobis: MahalanobisEntropyEstimator,
    weights: [f32; 3], // Tunable weights
}

impl EnsembleEntropyEstimator {
    pub fn compute_entropy(&self, embedding: &[f32]) -> f32 {
        let knn_entropy = self.knn.compute_entropy(embedding);
        let gmm_entropy = self.gmm.compute_entropy(embedding);
        let maha_entropy = self.mahalanobis.compute_entropy(embedding);

        // Weighted average
        self.weights[0] * knn_entropy +
        self.weights[1] * gmm_entropy +
        self.weights[2] * maha_entropy
    }
}
```

---

## 4. Computing ΔC (Coherence/Integration)

### 4.1 Coherence Components

Coherence measures how well a memory integrates with existing knowledge. We compute three sub-metrics:

| Component | Weight | What It Measures |
|-----------|--------|------------------|
| **Connectivity** | 0.4 | Potential graph edges to existing nodes |
| **Cluster Fit** | 0.4 | Similarity to nearest cluster centroid |
| **Consistency** | 0.2 | Absence of contradictions |

### 4.2 Connectivity Score

```rust
/// Measures potential graph connectivity
pub struct ConnectivityScorer {
    /// Similarity threshold for potential edge
    edge_threshold: f32,
    /// Maximum edges to consider
    max_edges: usize,
    /// HNSW index for similarity search
    index: HnswIndex,
}

impl ConnectivityScorer {
    pub fn compute_connectivity(&self, embedding: &[f32], space_index: usize) -> f32 {
        // Find similar nodes
        let neighbors = self.index.search(embedding, self.max_edges);

        // Count potential edges (similarity > threshold)
        let potential_edges = neighbors.iter()
            .filter(|(_, sim)| *sim > self.edge_threshold)
            .count();

        // Normalize by max_edges
        (potential_edges as f32 / self.max_edges as f32).min(1.0)
    }
}
```

### 4.3 Cluster Fit Score

```rust
/// Measures fit to existing semantic clusters
pub struct ClusterFitScorer {
    /// Cluster centroids
    centroids: Vec<Vec<f32>>,
    /// Cluster radii (std dev from centroid)
    radii: Vec<f32>,
}

impl ClusterFitScorer {
    pub fn compute_cluster_fit(&self, embedding: &[f32]) -> f32 {
        // Find nearest centroid
        let (nearest_idx, nearest_dist) = self.centroids.iter()
            .enumerate()
            .map(|(i, centroid)| (i, cosine_distance(embedding, centroid)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // Score based on distance relative to cluster radius
        let radius = self.radii[nearest_idx];
        let normalized_dist = nearest_dist / radius;

        // Sigmoid: close to centroid = high fit
        1.0 / (1.0 + normalized_dist)
    }
}
```

### 4.4 Consistency Score (Contradiction Detection)

```rust
/// Detects contradictions with existing knowledge
/// Research: https://arxiv.org/html/2511.04926
pub struct ConsistencyScorer {
    /// Contradiction detection model
    contradiction_model: ContradictionDetector,
    /// Graph for relational consistency
    graph: KnowledgeGraph,
}

impl ConsistencyScorer {
    pub fn compute_consistency(
        &self,
        memory: &KnowledgeNode,
        space_index: usize,
    ) -> f32 {
        // Get neighbors in this embedding space
        let neighbors = self.graph.get_neighbors_in_space(
            &memory.fingerprint.embeddings[space_index],
            space_index,
            k: 10,
        );

        // Check for contradictions with each neighbor
        let contradictions: Vec<f32> = neighbors.iter()
            .map(|neighbor| {
                self.contradiction_model.detect(
                    &memory.content,
                    &neighbor.content,
                    space_index,
                )
            })
            .collect();

        // Consistency = 1 - max_contradiction
        let max_contradiction = contradictions.iter()
            .cloned()
            .fold(0.0f32, f32::max);

        1.0 - max_contradiction
    }
}
```

### 4.5 Space-Specific Consistency

Different spaces have different consistency semantics:

```rust
/// Causal consistency: check for causal contradictions
pub struct CausalConsistencyScorer {
    pub fn compute(&self, memory: &KnowledgeNode, graph: &KnowledgeGraph) -> f32 {
        // A causes B, but B also causes A? → contradiction
        let causal_edges = graph.get_causal_edges(&memory.id);

        for edge in &causal_edges {
            let reverse = graph.find_edge(edge.target, edge.source, EdgeType::Causal);
            if reverse.is_some() && reverse.unwrap().weight > 0.5 {
                // Bidirectional causality detected (usually invalid)
                return 0.3; // Low consistency
            }
        }

        // Check temporal consistency (cause before effect)
        for edge in &causal_edges {
            let source_time = graph.get_node(edge.source).created_at;
            let target_time = graph.get_node(edge.target).created_at;

            if source_time > target_time {
                // Effect before cause? Suspicious
                return 0.5;
            }
        }

        1.0 // No contradictions
    }
}

/// Entity consistency: check for factual contradictions
pub struct EntityConsistencyScorer {
    pub fn compute(&self, memory: &KnowledgeNode, graph: &KnowledgeGraph) -> f32 {
        // Extract entities from memory
        let entities = extract_entities(&memory.content);

        for entity in &entities {
            // Check if entity has conflicting attributes
            let existing = graph.find_entity(entity.name);
            if let Some(existing) = existing {
                let attribute_conflicts = count_attribute_conflicts(
                    &entity.attributes,
                    &existing.attributes,
                );

                if attribute_conflicts > 0 {
                    return 1.0 - (attribute_conflicts as f32 / entity.attributes.len() as f32);
                }
            }
        }

        1.0 // No conflicts
    }
}
```

### 4.6 Combined Coherence Score

```rust
/// Full coherence computation
pub struct CoherenceComputer {
    connectivity: ConnectivityScorer,
    cluster_fit: ClusterFitScorer,
    consistency: ConsistencyScorer,

    // Component weights (tunable per space)
    weights: CoherenceWeights,
}

#[derive(Clone)]
pub struct CoherenceWeights {
    pub connectivity: f32,
    pub cluster_fit: f32,
    pub consistency: f32,
}

impl Default for CoherenceWeights {
    fn default() -> Self {
        Self {
            connectivity: 0.4,
            cluster_fit: 0.4,
            consistency: 0.2,
        }
    }
}

impl CoherenceComputer {
    pub fn compute_coherence(
        &self,
        memory: &KnowledgeNode,
        space_index: usize,
    ) -> f32 {
        let embedding = &memory.fingerprint.embeddings[space_index];

        let conn = self.connectivity.compute_connectivity(embedding, space_index);
        let fit = self.cluster_fit.compute_cluster_fit(embedding);
        let cons = self.consistency.compute_consistency(memory, space_index);

        self.weights.connectivity * conn +
        self.weights.cluster_fit * fit +
        self.weights.consistency * cons
    }
}
```

---

## 5. Johari Classification Logic

### 5.1 Basic Classifier

```rust
/// Johari quadrant classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JohariQuadrant {
    /// Low ΔS, High ΔC: Well-understood, well-connected
    Open,
    /// High ΔS, Low ΔC: Novel but poorly integrated (discovery opportunity)
    Blind,
    /// Low ΔS, Low ΔC: Known but dormant
    Hidden,
    /// High ΔS, High ΔC: Novel and well-integrated (frontier)
    Unknown,
}

impl JohariQuadrant {
    /// Get recommended action for this quadrant
    pub fn recommended_action(&self) -> &'static str {
        match self {
            JohariQuadrant::Open => "Leverage existing knowledge",
            JohariQuadrant::Blind => "Investigate: potential discovery",
            JohariQuadrant::Hidden => "Consider: dormant or obsolete?",
            JohariQuadrant::Unknown => "Explore: frontier territory",
        }
    }

    /// Is this quadrant actionable for learning?
    pub fn is_learning_opportunity(&self) -> bool {
        matches!(self, JohariQuadrant::Blind | JohariQuadrant::Unknown)
    }
}
```

### 5.2 Per-Space Classification Result

```rust
/// Classification result for a single embedding space
#[derive(Debug, Clone)]
pub struct SpaceJohariResult {
    /// The embedding space index (0-11)
    pub space_index: usize,
    /// The classified quadrant
    pub quadrant: JohariQuadrant,
    /// Entropy (novelty) score [0, 1]
    pub delta_s: f32,
    /// Coherence (integration) score [0, 1]
    pub delta_c: f32,
    /// Classification confidence [0, 1]
    pub confidence: f32,
    /// Component breakdown
    pub components: JohariComponents,
}

#[derive(Debug, Clone)]
pub struct JohariComponents {
    // Entropy components
    pub knn_entropy: f32,
    pub gmm_entropy: f32,
    pub mahalanobis_entropy: f32,

    // Coherence components
    pub connectivity: f32,
    pub cluster_fit: f32,
    pub consistency: f32,
}

impl SpaceJohariResult {
    pub fn new(
        space_index: usize,
        delta_s: f32,
        delta_c: f32,
        thresholds: &JohariThresholds,
        components: JohariComponents,
    ) -> Self {
        let quadrant = Self::classify(delta_s, delta_c, thresholds);
        let confidence = Self::compute_confidence(delta_s, delta_c, thresholds);

        Self {
            space_index,
            quadrant,
            delta_s,
            delta_c,
            confidence,
            components,
        }
    }

    fn classify(delta_s: f32, delta_c: f32, thresholds: &JohariThresholds) -> JohariQuadrant {
        let high_entropy = delta_s > thresholds.entropy;
        let high_coherence = delta_c > thresholds.coherence;

        match (high_entropy, high_coherence) {
            (false, true) => JohariQuadrant::Open,
            (true, false) => JohariQuadrant::Blind,
            (false, false) => JohariQuadrant::Hidden,
            (true, true) => JohariQuadrant::Unknown,
        }
    }

    fn compute_confidence(delta_s: f32, delta_c: f32, thresholds: &JohariThresholds) -> f32 {
        // Distance from decision boundary
        let s_margin = (delta_s - thresholds.entropy).abs();
        let c_margin = (delta_c - thresholds.coherence).abs();

        // Normalize: farther from boundary = more confident
        let raw_confidence = (s_margin + c_margin) / 2.0;
        raw_confidence.min(1.0)
    }
}
```

### 5.3 Full Fingerprint Classification

```rust
/// Complete Johari classification for all 12 embedding spaces
#[derive(Debug, Clone)]
pub struct JohariClassification {
    /// Per-space results
    pub spaces: [SpaceJohariResult; 12],
    /// Aggregate statistics
    pub summary: JohariSummary,
    /// Cross-space insights
    pub insights: Vec<CrossSpaceInsight>,
}

#[derive(Debug, Clone)]
pub struct JohariSummary {
    /// Count per quadrant across all spaces
    pub quadrant_counts: HashMap<JohariQuadrant, usize>,
    /// Dominant quadrant (most frequent)
    pub dominant_quadrant: JohariQuadrant,
    /// Average confidence across spaces
    pub avg_confidence: f32,
    /// Spaces with low confidence (need calibration)
    pub uncertain_spaces: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CrossSpaceInsight {
    pub insight_type: InsightType,
    pub spaces_involved: Vec<usize>,
    pub description: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsightType {
    /// Open in one space, Blind in another
    KnowledgeGap,
    /// Blind in multiple related spaces
    SystemicBlindSpot,
    /// Hidden in most spaces
    DormantKnowledge,
    /// Unknown in multiple spaces
    FrontierTerritory,
    /// Contradictory classifications
    InternalConflict,
}
```

---

## 6. Data Structures

### 6.1 Configuration

```rust
/// Johari classifier configuration
#[derive(Debug, Clone)]
pub struct JohariConfig {
    /// Per-space thresholds (can be calibrated independently)
    pub thresholds: [JohariThresholds; 12],
    /// Per-space coherence weights
    pub coherence_weights: [CoherenceWeights; 12],
    /// Per-space entropy estimator selection
    pub entropy_methods: [EntropyMethod; 12],
    /// Enable cross-space insight generation
    pub enable_insights: bool,
    /// Minimum confidence to report classification
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct JohariThresholds {
    /// Entropy threshold (above = high entropy)
    pub entropy: f32,
    /// Coherence threshold (above = high coherence)
    pub coherence: f32,
}

impl Default for JohariThresholds {
    fn default() -> Self {
        Self {
            entropy: 0.5,
            coherence: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EntropyMethod {
    Knn { k: usize },
    Gmm { n_components: usize },
    Mahalanobis,
    Ensemble { knn_weight: f32, gmm_weight: f32, maha_weight: f32 },
}
```

### 6.2 Storage Schema Extension

```rust
/// Extended TeleologicalFingerprint with Johari
pub struct TeleologicalFingerprint {
    // ... existing fields ...

    /// Per-embedder Johari classification
    pub johari_quadrants: [JohariQuadrant; 12],

    /// Per-embedder classification confidence [0, 1]
    pub johari_confidence: [f32; 12],

    /// Delta-S per space (for recomputation)
    pub delta_s: [f32; 12],

    /// Delta-C per space (for recomputation)
    pub delta_c: [f32; 12],

    /// Last Johari update timestamp
    pub johari_updated_at: chrono::DateTime<chrono::Utc>,
}

/// SQL schema addition
const JOHARI_SCHEMA: &str = r#"
ALTER TABLE knowledge_nodes ADD COLUMN IF NOT EXISTS johari_quadrants SMALLINT[12];
ALTER TABLE knowledge_nodes ADD COLUMN IF NOT EXISTS johari_confidence REAL[12];
ALTER TABLE knowledge_nodes ADD COLUMN IF NOT EXISTS delta_s REAL[12];
ALTER TABLE knowledge_nodes ADD COLUMN IF NOT EXISTS delta_c REAL[12];
ALTER TABLE knowledge_nodes ADD COLUMN IF NOT EXISTS johari_updated_at TIMESTAMPTZ DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_johari_quadrants ON knowledge_nodes USING GIN(johari_quadrants);
CREATE INDEX IF NOT EXISTS idx_johari_blind_spots ON knowledge_nodes ((johari_quadrants[5]))
    WHERE johari_quadrants[5] = 2; -- Index for Blind causal memories
"#;
```

---

## 7. Implementation: Core Classifier

### 7.1 Main Classifier Struct

```rust
/// The main Johari classifier
pub struct JohariClassifier {
    /// Configuration
    config: JohariConfig,

    /// Per-space entropy estimators
    entropy_estimators: [Box<dyn EntropyEstimator>; 12],

    /// Per-space coherence computers
    coherence_computers: [CoherenceComputer; 12],

    /// Knowledge graph reference
    graph: Arc<RwLock<KnowledgeGraph>>,

    /// Per-space HNSW indices
    indices: [Arc<RwLock<HnswIndex>>; 12],

    /// Calibration state
    calibration: CalibrationState,
}

#[derive(Debug, Clone)]
pub struct CalibrationState {
    /// Has each space been calibrated?
    pub calibrated: [bool; 12],
    /// Last calibration timestamp per space
    pub calibrated_at: [Option<chrono::DateTime<chrono::Utc>>; 12],
    /// Corpus size at calibration
    pub corpus_size_at_calibration: [usize; 12],
}

impl JohariClassifier {
    /// Create new classifier with default configuration
    pub fn new(
        graph: Arc<RwLock<KnowledgeGraph>>,
        indices: [Arc<RwLock<HnswIndex>>; 12],
    ) -> Self {
        Self::with_config(graph, indices, JohariConfig::default())
    }

    /// Create classifier with custom configuration
    pub fn with_config(
        graph: Arc<RwLock<KnowledgeGraph>>,
        indices: [Arc<RwLock<HnswIndex>>; 12],
        config: JohariConfig,
    ) -> Self {
        // Initialize entropy estimators based on config
        let entropy_estimators = std::array::from_fn(|i| {
            create_entropy_estimator(config.entropy_methods[i])
        });

        // Initialize coherence computers
        let coherence_computers = std::array::from_fn(|i| {
            CoherenceComputer::new(config.coherence_weights[i].clone())
        });

        Self {
            config,
            entropy_estimators,
            coherence_computers,
            graph,
            indices,
            calibration: CalibrationState::default(),
        }
    }

    /// Classify a memory's Johari quadrants
    pub fn classify(&self, memory: &KnowledgeNode) -> JohariClassification {
        // Classify each space in parallel
        let spaces: [SpaceJohariResult; 12] = std::array::from_fn(|i| {
            self.classify_space(memory, i)
        });

        // Compute summary
        let summary = self.compute_summary(&spaces);

        // Generate cross-space insights
        let insights = if self.config.enable_insights {
            self.generate_insights(&spaces)
        } else {
            vec![]
        };

        JohariClassification {
            spaces,
            summary,
            insights,
        }
    }

    /// Classify a single embedding space
    fn classify_space(&self, memory: &KnowledgeNode, space_index: usize) -> SpaceJohariResult {
        let embedding = &memory.fingerprint.embeddings[space_index];

        // Compute ΔS (entropy/novelty)
        let (delta_s, entropy_components) = self.compute_delta_s(embedding, space_index);

        // Compute ΔC (coherence/integration)
        let (delta_c, coherence_components) = self.compute_delta_c(memory, space_index);

        let components = JohariComponents {
            knn_entropy: entropy_components.knn,
            gmm_entropy: entropy_components.gmm,
            mahalanobis_entropy: entropy_components.mahalanobis,
            connectivity: coherence_components.connectivity,
            cluster_fit: coherence_components.cluster_fit,
            consistency: coherence_components.consistency,
        };

        SpaceJohariResult::new(
            space_index,
            delta_s,
            delta_c,
            &self.config.thresholds[space_index],
            components,
        )
    }

    fn compute_delta_s(&self, embedding: &[f32], space_index: usize) -> (f32, EntropyComponents) {
        let estimator = &self.entropy_estimators[space_index];
        estimator.compute_with_components(embedding)
    }

    fn compute_delta_c(&self, memory: &KnowledgeNode, space_index: usize) -> (f32, CoherenceComponents) {
        let computer = &self.coherence_computers[space_index];
        computer.compute_with_components(memory, space_index)
    }

    fn compute_summary(&self, spaces: &[SpaceJohariResult; 12]) -> JohariSummary {
        let mut quadrant_counts: HashMap<JohariQuadrant, usize> = HashMap::new();

        for space in spaces {
            *quadrant_counts.entry(space.quadrant).or_insert(0) += 1;
        }

        let dominant_quadrant = quadrant_counts.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(q, _)| *q)
            .unwrap_or(JohariQuadrant::Open);

        let avg_confidence = spaces.iter()
            .map(|s| s.confidence)
            .sum::<f32>() / 12.0;

        let uncertain_spaces: Vec<usize> = spaces.iter()
            .filter(|s| s.confidence < self.config.min_confidence)
            .map(|s| s.space_index)
            .collect();

        JohariSummary {
            quadrant_counts,
            dominant_quadrant,
            avg_confidence,
            uncertain_spaces,
        }
    }
}
```

### 7.2 Cross-Space Insight Generation

```rust
impl JohariClassifier {
    fn generate_insights(&self, spaces: &[SpaceJohariResult; 12]) -> Vec<CrossSpaceInsight> {
        let mut insights = vec![];

        // Insight 1: Knowledge Gap (Open in one, Blind in related)
        insights.extend(self.detect_knowledge_gaps(spaces));

        // Insight 2: Systemic Blind Spots (Blind in multiple spaces)
        insights.extend(self.detect_systemic_blind_spots(spaces));

        // Insight 3: Dormant Knowledge (Hidden in most spaces)
        insights.extend(self.detect_dormant_knowledge(spaces));

        // Insight 4: Frontier Territory (Unknown in multiple)
        insights.extend(self.detect_frontier(spaces));

        insights
    }

    fn detect_knowledge_gaps(&self, spaces: &[SpaceJohariResult; 12]) -> Vec<CrossSpaceInsight> {
        let mut insights = vec![];

        // Define related space pairs
        let related_pairs = [
            (0, 4, "semantic-causal"),    // E1 Semantic, E5 Causal
            (0, 6, "semantic-code"),      // E1 Semantic, E7 Code
            (4, 6, "causal-code"),        // E5 Causal, E7 Code
            (0, 10, "semantic-entity"),   // E1 Semantic, E11 Entity
        ];

        for (a, b, name) in related_pairs {
            let qa = spaces[a].quadrant;
            let qb = spaces[b].quadrant;

            if qa == JohariQuadrant::Open && qb == JohariQuadrant::Blind {
                insights.push(CrossSpaceInsight {
                    insight_type: InsightType::KnowledgeGap,
                    spaces_involved: vec![a, b],
                    description: format!(
                        "Open in {} (space {}) but Blind in {} (space {})",
                        space_name(a), a, space_name(b), b
                    ),
                    recommended_action: format!(
                        "Investigate {} relationships for this memory",
                        name.split('-').last().unwrap()
                    ),
                });
            }
        }

        insights
    }

    fn detect_systemic_blind_spots(&self, spaces: &[SpaceJohariResult; 12]) -> Vec<CrossSpaceInsight> {
        let blind_spaces: Vec<usize> = spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Blind)
            .map(|s| s.space_index)
            .collect();

        if blind_spaces.len() >= 4 {
            vec![CrossSpaceInsight {
                insight_type: InsightType::SystemicBlindSpot,
                spaces_involved: blind_spaces.clone(),
                description: format!(
                    "Blind in {} spaces: {}",
                    blind_spaces.len(),
                    blind_spaces.iter().map(|i| space_name(*i)).collect::<Vec<_>>().join(", ")
                ),
                recommended_action: "Priority investigation: this memory has broad knowledge gaps".into(),
            }]
        } else {
            vec![]
        }
    }

    fn detect_dormant_knowledge(&self, spaces: &[SpaceJohariResult; 12]) -> Vec<CrossSpaceInsight> {
        let hidden_count = spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Hidden)
            .count();

        if hidden_count >= 8 {
            vec![CrossSpaceInsight {
                insight_type: InsightType::DormantKnowledge,
                spaces_involved: (0..12).collect(),
                description: format!("Hidden in {}/12 spaces", hidden_count),
                recommended_action: "Consider: is this memory obsolete or under-utilized?".into(),
            }]
        } else {
            vec![]
        }
    }

    fn detect_frontier(&self, spaces: &[SpaceJohariResult; 12]) -> Vec<CrossSpaceInsight> {
        let unknown_spaces: Vec<usize> = spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Unknown)
            .map(|s| s.space_index)
            .collect();

        if unknown_spaces.len() >= 3 {
            vec![CrossSpaceInsight {
                insight_type: InsightType::FrontierTerritory,
                spaces_involved: unknown_spaces.clone(),
                description: format!("Unknown/frontier in {} spaces", unknown_spaces.len()),
                recommended_action: "Explore: this memory represents new territory".into(),
            }]
        } else {
            vec![]
        }
    }
}

fn space_name(index: usize) -> &'static str {
    match index {
        0 => "Semantic",
        1 => "Temporal-Recent",
        2 => "Temporal-Periodic",
        3 => "Temporal-Positional",
        4 => "Causal",
        5 => "Sparse",
        6 => "Code",
        7 => "Graph",
        8 => "HDC",
        9 => "Multimodal",
        10 => "Entity",
        11 => "Late-Interaction",
        _ => "Unknown",
    }
}
```

---

## 8. Implementation: Per-Space Strategies

### 8.1 E1 Semantic Space (1024D)

```rust
/// Semantic space: GMM + Mahalanobis ensemble
pub fn create_semantic_johari(config: &JohariConfig) -> SpaceJohariStrategy {
    SpaceJohariStrategy {
        entropy: Box::new(EnsembleEntropyEstimator {
            knn: KnnEntropyEstimator::new(10),
            gmm: GmmEntropyEstimator::new(32), // 32 components for semantic clusters
            mahalanobis: MahalanobisEntropyEstimator::new(),
            weights: [0.3, 0.4, 0.3],
        }),
        coherence: CoherenceComputer::new(CoherenceWeights {
            connectivity: 0.3,
            cluster_fit: 0.5,  // Semantic = cluster-heavy
            consistency: 0.2,
        }),
        thresholds: JohariThresholds {
            entropy: 0.5,
            coherence: 0.5,
        },
    }
}
```

### 8.2 E5 Causal Space (768D) - Asymmetric

```rust
/// Causal space: Asymmetric entropy (cause ≠ effect)
pub fn create_causal_johari(config: &JohariConfig) -> SpaceJohariStrategy {
    SpaceJohariStrategy {
        entropy: Box::new(AsymmetricKnnEntropy {
            forward_k: 10,   // Cause → Effect direction
            backward_k: 5,   // Effect → Cause direction
            asymmetry_weight: 0.7,
        }),
        coherence: CoherenceComputer::new(CoherenceWeights {
            connectivity: 0.2,
            cluster_fit: 0.2,
            consistency: 0.6,  // Causal = consistency-heavy
        }),
        thresholds: JohariThresholds {
            entropy: 0.55,   // Slightly higher: causal relationships are rarer
            coherence: 0.45,
        },
    }
}

/// Asymmetric KNN for causal direction
pub struct AsymmetricKnnEntropy {
    forward_k: usize,
    backward_k: usize,
    asymmetry_weight: f32,
}

impl EntropyEstimator for AsymmetricKnnEntropy {
    fn compute(&self, embedding: &[f32], index: &HnswIndex) -> f32 {
        // Forward: distance to potential effects
        let forward_dist = index.search_with_direction(embedding, self.forward_k, Direction::Forward)
            .last()
            .map(|(_, d)| *d)
            .unwrap_or(f32::MAX);

        // Backward: distance to potential causes
        let backward_dist = index.search_with_direction(embedding, self.backward_k, Direction::Backward)
            .last()
            .map(|(_, d)| *d)
            .unwrap_or(f32::MAX);

        // Combine asymmetrically
        let combined = self.asymmetry_weight * forward_dist +
                       (1.0 - self.asymmetry_weight) * backward_dist;

        sigmoid(combined)
    }
}
```

### 8.3 E7 Code Space (1536D)

```rust
/// Code space: AST-aware entropy
pub fn create_code_johari(config: &JohariConfig) -> SpaceJohariStrategy {
    SpaceJohariStrategy {
        entropy: Box::new(CodeEntropyEstimator {
            knn: KnnEntropyEstimator::new(8),
            ast_similarity: AstSimilarityEstimator::new(),
            weights: [0.4, 0.6],  // Favor AST structure
        }),
        coherence: CoherenceComputer::new(CoherenceWeights {
            connectivity: 0.4,
            cluster_fit: 0.3,
            consistency: 0.3,  // Code has type/compile consistency
        }),
        thresholds: JohariThresholds {
            entropy: 0.45,   // Code patterns are more structured
            coherence: 0.55,
        },
    }
}
```

### 8.4 E9 HDC Space (Binary)

```rust
/// Hyperdimensional Computing space: Hamming distance
pub fn create_hdc_johari(config: &JohariConfig) -> SpaceJohariStrategy {
    SpaceJohariStrategy {
        entropy: Box::new(HdcEntropyEstimator {
            prototypes: vec![],  // Populated during calibration
            threshold: 0.3,      // Hamming distance threshold
        }),
        coherence: CoherenceComputer::new(CoherenceWeights {
            connectivity: 0.5,
            cluster_fit: 0.3,
            consistency: 0.2,
        }),
        thresholds: JohariThresholds {
            entropy: 0.4,    // HDC is more robust
            coherence: 0.5,
        },
    }
}

pub struct HdcEntropyEstimator {
    prototypes: Vec<BitVec>,
    threshold: f32,
}

impl EntropyEstimator for HdcEntropyEstimator {
    fn compute(&self, embedding: &[f32], _index: &HnswIndex) -> f32 {
        // Convert to binary
        let binary = embedding.iter()
            .map(|v| *v > 0.0)
            .collect::<BitVec>();

        // Find minimum Hamming distance to prototypes
        let min_dist = self.prototypes.iter()
            .map(|proto| hamming_distance(&binary, proto))
            .min()
            .unwrap_or(1.0);

        // Normalize by embedding dimension
        min_dist / embedding.len() as f32
    }
}
```

### 8.5 E12 Late Interaction (Per-Token)

```rust
/// Late interaction space: Token-level entropy aggregation
pub fn create_late_interaction_johari(config: &JohariConfig) -> SpaceJohariStrategy {
    SpaceJohariStrategy {
        entropy: Box::new(TokenLevelEntropyEstimator {
            per_token_knn_k: 5,
            aggregation: TokenAggregation::Mean,
        }),
        coherence: CoherenceComputer::new(CoherenceWeights {
            connectivity: 0.3,
            cluster_fit: 0.4,
            consistency: 0.3,
        }),
        thresholds: JohariThresholds {
            entropy: 0.5,
            coherence: 0.5,
        },
    }
}

pub struct TokenLevelEntropyEstimator {
    per_token_knn_k: usize,
    aggregation: TokenAggregation,
}

pub enum TokenAggregation {
    Mean,
    Max,
    Min,
    Median,
}

impl EntropyEstimator for TokenLevelEntropyEstimator {
    fn compute(&self, embeddings: &[f32], index: &HnswIndex) -> f32 {
        // Embeddings are [num_tokens × 128]
        let token_dim = 128;
        let num_tokens = embeddings.len() / token_dim;

        let token_entropies: Vec<f32> = (0..num_tokens)
            .map(|i| {
                let token_emb = &embeddings[i * token_dim..(i + 1) * token_dim];
                let knn_dist = index.kth_neighbor_distance(token_emb, self.per_token_knn_k);
                sigmoid(knn_dist)
            })
            .collect();

        match self.aggregation {
            TokenAggregation::Mean => mean(&token_entropies),
            TokenAggregation::Max => token_entropies.iter().cloned().fold(0.0f32, f32::max),
            TokenAggregation::Min => token_entropies.iter().cloned().fold(1.0f32, f32::min),
            TokenAggregation::Median => median(&token_entropies),
        }
    }
}
```

---

## 9. Calibration and Thresholds

### 9.1 Threshold Calibration Protocol

```rust
/// Calibration for Johari thresholds
pub struct JohariCalibrator {
    /// Percentile for entropy threshold (default: 50th)
    entropy_percentile: f32,
    /// Percentile for coherence threshold (default: 50th)
    coherence_percentile: f32,
    /// Minimum samples for calibration
    min_samples: usize,
}

impl JohariCalibrator {
    pub fn calibrate_space(
        &self,
        classifier: &mut JohariClassifier,
        corpus: &[KnowledgeNode],
        space_index: usize,
    ) -> Result<JohariThresholds, CalibrationError> {
        if corpus.len() < self.min_samples {
            return Err(CalibrationError::InsufficientSamples {
                required: self.min_samples,
                actual: corpus.len(),
            });
        }

        // Compute ΔS for all corpus items
        let entropies: Vec<f32> = corpus.iter()
            .map(|node| {
                let emb = &node.fingerprint.embeddings[space_index];
                classifier.entropy_estimators[space_index].compute(emb)
            })
            .collect();

        // Compute ΔC for all corpus items
        let coherences: Vec<f32> = corpus.iter()
            .map(|node| classifier.coherence_computers[space_index].compute(node, space_index))
            .collect();

        // Find percentile thresholds
        let entropy_threshold = percentile(&entropies, self.entropy_percentile);
        let coherence_threshold = percentile(&coherences, self.coherence_percentile);

        Ok(JohariThresholds {
            entropy: entropy_threshold,
            coherence: coherence_threshold,
        })
    }

    /// Calibrate all spaces
    pub fn calibrate_all(
        &self,
        classifier: &mut JohariClassifier,
        corpus: &[KnowledgeNode],
    ) -> Result<[JohariThresholds; 12], CalibrationError> {
        let mut thresholds = [JohariThresholds::default(); 12];

        for i in 0..12 {
            thresholds[i] = self.calibrate_space(classifier, corpus, i)?;
            classifier.calibration.calibrated[i] = true;
            classifier.calibration.calibrated_at[i] = Some(chrono::Utc::now());
            classifier.calibration.corpus_size_at_calibration[i] = corpus.len();
        }

        Ok(thresholds)
    }
}

fn percentile(values: &[f32], p: f32) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let index = ((p / 100.0) * (sorted.len() - 1) as f32).round() as usize;
    sorted[index]
}
```

### 9.2 Adaptive Threshold Adjustment

```rust
/// Adaptive threshold adjustment based on feedback
pub struct AdaptiveThresholdAdjuster {
    /// Learning rate for threshold updates
    learning_rate: f32,
    /// Decay for exponential moving average
    ema_decay: f32,
    /// Running statistics
    stats: [AdaptiveStats; 12],
}

#[derive(Default)]
struct AdaptiveStats {
    /// EMA of false positive rate per quadrant
    false_positive_ema: HashMap<JohariQuadrant, f32>,
    /// EMA of true positive rate per quadrant
    true_positive_ema: HashMap<JohariQuadrant, f32>,
}

impl AdaptiveThresholdAdjuster {
    /// Adjust thresholds based on human feedback
    pub fn adjust_from_feedback(
        &mut self,
        classifier: &mut JohariClassifier,
        space_index: usize,
        predicted: JohariQuadrant,
        actual: JohariQuadrant,
    ) {
        if predicted == actual {
            // True positive: strengthen current thresholds
            self.stats[space_index]
                .true_positive_ema
                .entry(predicted)
                .and_modify(|v| *v = self.ema_decay * *v + (1.0 - self.ema_decay))
                .or_insert(1.0);
        } else {
            // False positive: adjust thresholds
            self.stats[space_index]
                .false_positive_ema
                .entry(predicted)
                .and_modify(|v| *v = self.ema_decay * *v + (1.0 - self.ema_decay))
                .or_insert(1.0);

            // Adjust thresholds to move boundary
            let thresholds = &mut classifier.config.thresholds[space_index];

            match (predicted, actual) {
                (JohariQuadrant::Blind, JohariQuadrant::Open) => {
                    // Predicted Blind but was Open: entropy threshold too low
                    thresholds.entropy += self.learning_rate;
                }
                (JohariQuadrant::Open, JohariQuadrant::Blind) => {
                    // Predicted Open but was Blind: entropy threshold too high
                    thresholds.entropy -= self.learning_rate;
                }
                (JohariQuadrant::Hidden, JohariQuadrant::Open) => {
                    // Coherence threshold too high
                    thresholds.coherence -= self.learning_rate;
                }
                (JohariQuadrant::Open, JohariQuadrant::Hidden) => {
                    // Coherence threshold too low
                    thresholds.coherence += self.learning_rate;
                }
                _ => {
                    // Handle other misclassifications
                }
            }

            // Clamp thresholds to valid range
            thresholds.entropy = thresholds.entropy.clamp(0.2, 0.8);
            thresholds.coherence = thresholds.coherence.clamp(0.2, 0.8);
        }
    }
}
```

---

## 10. Integration with SemanticFingerprint

### 10.1 Fingerprint Extension

```rust
/// Extend SemanticFingerprint with Johari
impl SemanticFingerprint {
    /// Classify Johari quadrants (lazy, cached)
    pub fn johari(&self, classifier: &JohariClassifier) -> &JohariClassification {
        self.johari_cache.get_or_init(|| classifier.classify_from_fingerprint(self))
    }

    /// Get Johari for specific space
    pub fn johari_for_space(&self, space: usize, classifier: &JohariClassifier) -> JohariQuadrant {
        self.johari(classifier).spaces[space].quadrant
    }

    /// Check if any space is Blind (learning opportunity)
    pub fn has_blind_spots(&self, classifier: &JohariClassifier) -> bool {
        self.johari(classifier).spaces.iter()
            .any(|s| s.quadrant == JohariQuadrant::Blind)
    }

    /// Get all Blind space indices
    pub fn blind_spaces(&self, classifier: &JohariClassifier) -> Vec<usize> {
        self.johari(classifier).spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Blind)
            .map(|s| s.space_index)
            .collect()
    }
}
```

### 10.2 Storage Update

```rust
/// Update Johari on memory storage
impl MemoryStore {
    pub async fn store_with_johari(
        &self,
        node: KnowledgeNode,
        classifier: &JohariClassifier,
    ) -> Result<Uuid, StoreError> {
        // Compute Johari classification
        let johari = classifier.classify(&node);

        // Store node with Johari fields
        let mut node = node;
        node.fingerprint.johari_quadrants = johari.spaces.map(|s| s.quadrant);
        node.fingerprint.johari_confidence = johari.spaces.map(|s| s.confidence);
        node.fingerprint.delta_s = johari.spaces.map(|s| s.delta_s);
        node.fingerprint.delta_c = johari.spaces.map(|s| s.delta_c);
        node.fingerprint.johari_updated_at = chrono::Utc::now();

        // Log insights
        for insight in &johari.insights {
            tracing::info!(
                node_id = %node.id,
                insight_type = ?insight.insight_type,
                spaces = ?insight.spaces_involved,
                "Johari insight: {}",
                insight.description
            );
        }

        self.store_node(node).await
    }
}
```

### 10.3 Retrieval Integration

```rust
/// Use Johari for retrieval filtering
impl RetrievalPipeline {
    /// Filter candidates by Johari quadrant
    pub fn filter_by_johari(
        &self,
        candidates: Vec<RetrievalCandidate>,
        query_johari: &JohariClassification,
        filter: JohariFilter,
    ) -> Vec<RetrievalCandidate> {
        candidates.into_iter()
            .filter(|c| {
                let node_johari = &c.node.fingerprint;

                match filter {
                    JohariFilter::SameQuadrant(space) => {
                        node_johari.johari_quadrants[space] == query_johari.spaces[space].quadrant
                    }
                    JohariFilter::ExcludeBlind => {
                        !node_johari.johari_quadrants.iter()
                            .any(|q| *q == JohariQuadrant::Blind)
                    }
                    JohariFilter::OnlyOpen => {
                        node_johari.johari_quadrants.iter()
                            .all(|q| *q == JohariQuadrant::Open)
                    }
                    JohariFilter::LearningOpportunities => {
                        node_johari.johari_quadrants.iter()
                            .any(|q| *q == JohariQuadrant::Blind || *q == JohariQuadrant::Unknown)
                    }
                }
            })
            .collect()
    }
}

pub enum JohariFilter {
    SameQuadrant(usize),
    ExcludeBlind,
    OnlyOpen,
    LearningOpportunities,
}
```

---

## 11. Cross-Space Analysis

### 11.1 Johari Matrix Visualization

```rust
/// Generate Johari matrix for visualization
pub fn generate_johari_matrix(classification: &JohariClassification) -> JohariMatrix {
    let mut matrix = [[0u8; 4]; 12]; // 12 spaces × 4 quadrants

    for (i, space) in classification.spaces.iter().enumerate() {
        let quadrant_idx = match space.quadrant {
            JohariQuadrant::Open => 0,
            JohariQuadrant::Blind => 1,
            JohariQuadrant::Hidden => 2,
            JohariQuadrant::Unknown => 3,
        };
        matrix[i][quadrant_idx] = (space.confidence * 100.0) as u8;
    }

    JohariMatrix {
        data: matrix,
        space_names: SPACE_NAMES,
        quadrant_names: ["Open", "Blind", "Hidden", "Unknown"],
    }
}

/// ASCII visualization
pub fn visualize_johari(classification: &JohariClassification) -> String {
    let mut output = String::new();

    output.push_str("┌────────────────────┬──────┬───────┬────────┬─────────┐\n");
    output.push_str("│ Space              │ Open │ Blind │ Hidden │ Unknown │\n");
    output.push_str("├────────────────────┼──────┼───────┼────────┼─────────┤\n");

    for space in &classification.spaces {
        let marker = |q: JohariQuadrant| {
            if space.quadrant == q {
                format!("██{:.0}%", space.confidence * 100.0)
            } else {
                "     ".to_string()
            }
        };

        output.push_str(&format!(
            "│ {:18} │{:5}│{:6}│{:7}│{:8}│\n",
            space_name(space.space_index),
            marker(JohariQuadrant::Open),
            marker(JohariQuadrant::Blind),
            marker(JohariQuadrant::Hidden),
            marker(JohariQuadrant::Unknown),
        ));
    }

    output.push_str("└────────────────────┴──────┴───────┴────────┴─────────┘\n");
    output
}
```

### 11.2 Learning Opportunity Detection

```rust
/// Identify high-value learning opportunities
pub fn identify_learning_opportunities(
    memories: &[KnowledgeNode],
    classifier: &JohariClassifier,
) -> Vec<LearningOpportunity> {
    let mut opportunities = vec![];

    for memory in memories {
        let johari = classifier.classify(memory);

        // Priority 1: Semantic-Open but Causal-Blind (understand meaning but not causality)
        if johari.spaces[0].quadrant == JohariQuadrant::Open &&
           johari.spaces[4].quadrant == JohariQuadrant::Blind {
            opportunities.push(LearningOpportunity {
                memory_id: memory.id,
                priority: Priority::High,
                opportunity_type: OpportunityType::CausalGap,
                description: "Semantically understood but causal relationships unknown".into(),
                suggested_action: "Investigate: What causes this? What does it cause?".into(),
            });
        }

        // Priority 2: Multiple Blind spaces
        let blind_count = johari.spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Blind)
            .count();

        if blind_count >= 4 {
            opportunities.push(LearningOpportunity {
                memory_id: memory.id,
                priority: Priority::High,
                opportunity_type: OpportunityType::BroadBlindSpot,
                description: format!("Blind in {} embedding spaces", blind_count),
                suggested_action: "Deep investigation required".into(),
            });
        }

        // Priority 3: Unknown frontier (high novelty + high coherence)
        let unknown_count = johari.spaces.iter()
            .filter(|s| s.quadrant == JohariQuadrant::Unknown)
            .count();

        if unknown_count >= 3 {
            opportunities.push(LearningOpportunity {
                memory_id: memory.id,
                priority: Priority::Medium,
                opportunity_type: OpportunityType::Frontier,
                description: format!("Frontier territory in {} spaces", unknown_count),
                suggested_action: "Explore: this is novel but integrating well".into(),
            });
        }
    }

    // Sort by priority
    opportunities.sort_by(|a, b| a.priority.cmp(&b.priority));
    opportunities
}

#[derive(Debug)]
pub struct LearningOpportunity {
    pub memory_id: Uuid,
    pub priority: Priority,
    pub opportunity_type: OpportunityType,
    pub description: String,
    pub suggested_action: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

#[derive(Debug)]
pub enum OpportunityType {
    CausalGap,
    BroadBlindSpot,
    Frontier,
    DormantRevival,
}
```

---

## 12. Testing and Validation

### 12.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_johari_classification_boundaries() {
        let thresholds = JohariThresholds { entropy: 0.5, coherence: 0.5 };

        // Test exact boundaries
        assert_eq!(
            classify(0.49, 0.51, &thresholds),
            JohariQuadrant::Open
        );
        assert_eq!(
            classify(0.51, 0.49, &thresholds),
            JohariQuadrant::Blind
        );
        assert_eq!(
            classify(0.49, 0.49, &thresholds),
            JohariQuadrant::Hidden
        );
        assert_eq!(
            classify(0.51, 0.51, &thresholds),
            JohariQuadrant::Unknown
        );
    }

    #[test]
    fn test_knn_entropy_estimator() {
        let mut estimator = KnnEntropyEstimator::new(5);

        // Create corpus with known structure
        let corpus: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![i as f32 / 100.0; 128])
            .collect();

        estimator.index.add_batch(&corpus).unwrap();
        estimator.calibrate(&corpus);

        // In-distribution point
        let in_dist = vec![0.5; 128];
        let in_entropy = estimator.compute_entropy(&in_dist);
        assert!(in_entropy < 0.5, "In-distribution should have low entropy");

        // Out-of-distribution point
        let ood = vec![10.0; 128];
        let ood_entropy = estimator.compute_entropy(&ood);
        assert!(ood_entropy > 0.8, "OOD should have high entropy");
    }

    #[test]
    fn test_coherence_components() {
        let mut computer = CoherenceComputer::new(CoherenceWeights::default());

        // Well-connected node
        let connected = create_test_node_with_edges(10);
        let conn_coherence = computer.compute(&connected, 0);

        // Isolated node
        let isolated = create_test_node_with_edges(0);
        let iso_coherence = computer.compute(&isolated, 0);

        assert!(conn_coherence > iso_coherence);
    }

    #[test]
    fn test_cross_space_insight_detection() {
        let classifier = create_test_classifier();

        // Create node with semantic-open, causal-blind pattern
        let node = create_test_node_with_pattern(&[
            (0, JohariQuadrant::Open),
            (4, JohariQuadrant::Blind),
        ]);

        let classification = classifier.classify(&node);

        assert!(
            classification.insights.iter().any(|i| i.insight_type == InsightType::KnowledgeGap),
            "Should detect knowledge gap between semantic and causal"
        );
    }
}
```

### 12.2 Integration Tests

```rust
#[tokio::test]
async fn test_johari_storage_roundtrip() {
    let store = create_test_store().await;
    let classifier = create_test_classifier();

    // Create and store node with Johari
    let node = create_test_node();
    let id = store.store_with_johari(node.clone(), &classifier).await.unwrap();

    // Retrieve and verify Johari preserved
    let retrieved = store.get_node(id).await.unwrap();

    assert_eq!(
        retrieved.fingerprint.johari_quadrants,
        node.fingerprint.johari_quadrants
    );
    assert_eq!(
        retrieved.fingerprint.johari_confidence,
        node.fingerprint.johari_confidence
    );
}

#[tokio::test]
async fn test_calibration_stability() {
    let mut classifier = create_test_classifier();
    let corpus = generate_test_corpus(1000);

    // Calibrate
    let calibrator = JohariCalibrator {
        entropy_percentile: 50.0,
        coherence_percentile: 50.0,
        min_samples: 100,
    };

    let thresholds = calibrator.calibrate_all(&mut classifier, &corpus).unwrap();

    // Verify thresholds are reasonable
    for t in &thresholds {
        assert!(t.entropy > 0.3 && t.entropy < 0.7);
        assert!(t.coherence > 0.3 && t.coherence < 0.7);
    }

    // Verify calibration is stable (run again, similar results)
    let thresholds2 = calibrator.calibrate_all(&mut classifier, &corpus).unwrap();

    for (t1, t2) in thresholds.iter().zip(thresholds2.iter()) {
        assert!((t1.entropy - t2.entropy).abs() < 0.05);
        assert!((t1.coherence - t2.coherence).abs() < 0.05);
    }
}
```

### 12.3 Validation Protocol

```rust
/// Human validation protocol for Johari accuracy
pub struct JohariValidationProtocol {
    /// Sample size per quadrant
    samples_per_quadrant: usize,
    /// Minimum confidence for inclusion
    min_confidence: f32,
}

impl JohariValidationProtocol {
    /// Generate validation set
    pub fn generate_validation_set(
        &self,
        memories: &[KnowledgeNode],
        classifier: &JohariClassifier,
    ) -> ValidationSet {
        let mut samples: HashMap<(usize, JohariQuadrant), Vec<Uuid>> = HashMap::new();

        for memory in memories {
            let johari = classifier.classify(memory);

            for space in &johari.spaces {
                if space.confidence >= self.min_confidence {
                    samples
                        .entry((space.space_index, space.quadrant))
                        .or_default()
                        .push(memory.id);
                }
            }
        }

        // Sample equally from each
        ValidationSet {
            samples: samples.into_iter()
                .map(|(key, mut ids)| {
                    ids.shuffle(&mut rand::thread_rng());
                    (key, ids.into_iter().take(self.samples_per_quadrant).collect())
                })
                .collect(),
        }
    }

    /// Compute accuracy from human labels
    pub fn compute_accuracy(
        &self,
        predictions: &[(Uuid, usize, JohariQuadrant)],
        labels: &[(Uuid, usize, JohariQuadrant)],
    ) -> AccuracyReport {
        let mut correct = 0;
        let mut total = 0;
        let mut per_quadrant: HashMap<JohariQuadrant, (usize, usize)> = HashMap::new();

        for (pred, label) in predictions.iter().zip(labels.iter()) {
            assert_eq!(pred.0, label.0, "ID mismatch");
            assert_eq!(pred.1, label.1, "Space mismatch");

            total += 1;
            if pred.2 == label.2 {
                correct += 1;
                per_quadrant.entry(pred.2).or_default().0 += 1;
            }
            per_quadrant.entry(label.2).or_default().1 += 1;
        }

        AccuracyReport {
            overall_accuracy: correct as f32 / total as f32,
            per_quadrant_accuracy: per_quadrant.into_iter()
                .map(|(q, (c, t))| (q, c as f32 / t as f32))
                .collect(),
        }
    }
}
```

---

## 13. Performance Optimization

### 13.1 Batch Classification

```rust
/// Batch classification for efficiency
impl JohariClassifier {
    pub fn classify_batch(&self, memories: &[KnowledgeNode]) -> Vec<JohariClassification> {
        // Parallel classification across memories
        memories.par_iter()
            .map(|memory| self.classify(memory))
            .collect()
    }

    pub fn classify_batch_space(
        &self,
        embeddings: &[Vec<f32>],
        space_index: usize,
    ) -> Vec<SpaceJohariResult> {
        // Batch entropy computation
        let entropies = self.entropy_estimators[space_index]
            .compute_batch(embeddings);

        // Note: coherence requires full node, so can't batch as easily
        // In practice, batch the entropy and compute coherence individually
        embeddings.iter()
            .zip(entropies.iter())
            .enumerate()
            .map(|(i, (emb, delta_s))| {
                let delta_c = 0.5; // Placeholder: need full node for coherence
                SpaceJohariResult::new(
                    space_index,
                    *delta_s,
                    delta_c,
                    &self.config.thresholds[space_index],
                    JohariComponents::default(),
                )
            })
            .collect()
    }
}
```

### 13.2 Caching Strategy

```rust
/// Johari cache to avoid recomputation
pub struct JohariCache {
    /// LRU cache: memory_id -> classification
    cache: lru::LruCache<Uuid, (JohariClassification, chrono::DateTime<chrono::Utc>)>,
    /// Max age before invalidation
    max_age: chrono::Duration,
    /// Invalidate on corpus change
    corpus_version: u64,
}

impl JohariCache {
    pub fn get(
        &mut self,
        memory_id: Uuid,
        corpus_version: u64,
    ) -> Option<&JohariClassification> {
        // Invalidate if corpus changed
        if corpus_version != self.corpus_version {
            self.cache.clear();
            self.corpus_version = corpus_version;
            return None;
        }

        self.cache.get(&memory_id)
            .filter(|(_, cached_at)| {
                chrono::Utc::now() - *cached_at < self.max_age
            })
            .map(|(c, _)| c)
    }

    pub fn insert(&mut self, memory_id: Uuid, classification: JohariClassification) {
        self.cache.put(memory_id, (classification, chrono::Utc::now()));
    }
}
```

### 13.3 Incremental Updates

```rust
/// Incremental Johari update on new memory insertion
impl JohariClassifier {
    pub fn update_affected_johari(
        &self,
        new_memory: &KnowledgeNode,
        affected_neighbors: &[Uuid],
        store: &mut MemoryStore,
    ) -> Result<(), UpdateError> {
        // Reclassify neighbors whose coherence may have changed
        for neighbor_id in affected_neighbors {
            let neighbor = store.get_node(*neighbor_id)?;

            // Only update coherence (entropy is stable)
            for space_index in 0..12 {
                let old_coherence = neighbor.fingerprint.delta_c[space_index];
                let new_coherence = self.coherence_computers[space_index]
                    .compute(&neighbor, space_index);

                // Only update if significant change
                if (new_coherence - old_coherence).abs() > 0.1 {
                    let new_result = SpaceJohariResult::new(
                        space_index,
                        neighbor.fingerprint.delta_s[space_index],
                        new_coherence,
                        &self.config.thresholds[space_index],
                        JohariComponents::default(),
                    );

                    store.update_johari_for_space(*neighbor_id, space_index, &new_result)?;
                }
            }
        }

        Ok(())
    }
}
```

---

## 14. Monitoring and Observability

### 14.1 Metrics

```rust
/// Johari-specific metrics
pub struct JohariMetrics {
    /// Classification latency histogram
    pub classification_latency_ms: Histogram,
    /// Quadrant distribution gauge
    pub quadrant_distribution: GaugeVec,
    /// Confidence distribution
    pub confidence_distribution: Histogram,
    /// Insight generation count
    pub insights_generated: CounterVec,
    /// Cache hit rate
    pub cache_hit_rate: Gauge,
    /// Calibration staleness (days since last calibration)
    pub calibration_staleness_days: GaugeVec,
}

impl JohariMetrics {
    pub fn record_classification(
        &self,
        classification: &JohariClassification,
        latency: Duration,
    ) {
        self.classification_latency_ms.observe(latency.as_millis() as f64);

        for space in &classification.spaces {
            self.quadrant_distribution
                .with_label_values(&[
                    &space.space_index.to_string(),
                    &format!("{:?}", space.quadrant),
                ])
                .set(1.0);

            self.confidence_distribution.observe(space.confidence as f64);
        }

        for insight in &classification.insights {
            self.insights_generated
                .with_label_values(&[&format!("{:?}", insight.insight_type)])
                .inc();
        }
    }
}
```

### 14.2 Alerting Rules

```yaml
# Prometheus alerting rules for Johari
groups:
  - name: johari
    rules:
      - alert: JohariCalibrationStale
        expr: johari_calibration_staleness_days > 7
        for: 1h
        labels:
          severity: warning
        annotations:
          summary: "Johari calibration is stale for space {{ $labels.space }}"
          description: "Recalibration recommended"

      - alert: JohariLowConfidence
        expr: histogram_quantile(0.5, johari_confidence_distribution) < 0.3
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Johari classifications have low confidence"
          description: "Thresholds may need recalibration"

      - alert: JohariBlindSpotSpike
        expr: increase(johari_quadrant_distribution{quadrant="Blind"}[1h]) > 100
        for: 15m
        labels:
          severity: info
        annotations:
          summary: "Spike in Blind quadrant classifications"
          description: "May indicate new domain or knowledge gap"
```

### 14.3 Dashboard Queries

```sql
-- Johari distribution over time
SELECT
    date_trunc('hour', created_at) as hour,
    johari_quadrants[1] as semantic_quadrant,
    COUNT(*) as count
FROM knowledge_nodes
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY hour, semantic_quadrant
ORDER BY hour;

-- Learning opportunities (Blind in important spaces)
SELECT
    id,
    content,
    johari_quadrants,
    johari_confidence
FROM knowledge_nodes
WHERE johari_quadrants[5] = 2  -- Blind in causal (E5)
  AND johari_confidence[5] > 0.6
ORDER BY created_at DESC
LIMIT 20;

-- Cross-space anomalies (Open semantic, Blind code)
SELECT
    id,
    content
FROM knowledge_nodes
WHERE johari_quadrants[1] = 1  -- Open semantic
  AND johari_quadrants[7] = 2  -- Blind code
  AND content LIKE '%function%';  -- Contains code-related content
```

---

## 15. Migration Strategy

### 15.1 Phased Rollout

**Week 1-2: Infrastructure**
- [ ] Add Johari columns to database schema
- [ ] Implement core `JohariClassifier` without calibration
- [ ] Add unit tests for classification logic

**Week 3-4: Entropy Estimators**
- [ ] Implement KNN entropy estimator
- [ ] Implement GMM entropy estimator
- [ ] Implement per-space strategies (E1-E12)
- [ ] Benchmark latency

**Week 5-6: Coherence Computers**
- [ ] Implement connectivity scorer
- [ ] Implement cluster fit scorer
- [ ] Implement consistency scorer (basic)
- [ ] Wire up per-space coherence weights

**Week 7-8: Calibration & Integration**
- [ ] Implement calibration protocol
- [ ] Run initial calibration on corpus
- [ ] Integrate with `store_memory`
- [ ] Add caching

**Week 9-10: Cross-Space Analysis**
- [ ] Implement insight generation
- [ ] Add learning opportunity detection
- [ ] Build visualization tools
- [ ] Add monitoring/metrics

**Week 11-12: Validation & Optimization**
- [ ] Human validation protocol
- [ ] Threshold tuning based on feedback
- [ ] Performance optimization
- [ ] Documentation & training

### 15.2 Backfill Strategy

```rust
/// Backfill Johari for existing memories
pub async fn backfill_johari(
    store: &MemoryStore,
    classifier: &JohariClassifier,
    batch_size: usize,
    progress_callback: impl Fn(usize, usize),
) -> Result<BackfillReport, BackfillError> {
    let total = store.count_nodes().await?;
    let mut processed = 0;
    let mut errors = vec![];

    loop {
        let batch = store.get_nodes_without_johari(batch_size).await?;
        if batch.is_empty() {
            break;
        }

        for node in batch {
            match classifier.classify(&node) {
                Ok(classification) => {
                    store.update_johari(node.id, &classification).await?;
                }
                Err(e) => {
                    errors.push((node.id, e));
                }
            }
            processed += 1;
            progress_callback(processed, total);
        }
    }

    Ok(BackfillReport {
        total_processed: processed,
        errors,
    })
}
```

### 15.3 Rollback Plan

```sql
-- Rollback: remove Johari columns
ALTER TABLE knowledge_nodes DROP COLUMN IF EXISTS johari_quadrants;
ALTER TABLE knowledge_nodes DROP COLUMN IF EXISTS johari_confidence;
ALTER TABLE knowledge_nodes DROP COLUMN IF EXISTS delta_s;
ALTER TABLE knowledge_nodes DROP COLUMN IF EXISTS delta_c;
ALTER TABLE knowledge_nodes DROP COLUMN IF EXISTS johari_updated_at;

DROP INDEX IF EXISTS idx_johari_quadrants;
DROP INDEX IF EXISTS idx_johari_blind_spots;
```

---

## 16. References

### Research Papers

1. **Density Estimation in Latent Space**
   - Postels et al., 2020: [Quantifying Aleatoric and Epistemic Uncertainty Using Density Estimation in Latent Space](https://deepai.org/publication/quantifying-aleatoric-and-epistemic-uncertainty-using-density-estimation-in-latent-space)

2. **Out-of-Distribution Detection**
   - Sun et al., ICML 2022: [Out-of-Distribution Detection with Deep Nearest Neighbors](https://proceedings.mlr.press/v162/sun22d/sun22d.pdf)
   - ACM CSUR 2025: [Out-of-Distribution Detection: A Task-Oriented Survey](https://arxiv.org/html/2409.11884v4)

3. **Uncertainty Estimation**
   - ICLR 2025: [Reexamining the Aleatoric and Epistemic Uncertainty Dichotomy](https://iclr-blogposts.github.io/2025/blog/reexamining-the-aleatoric-and-epistemic-uncertainty-dichotomy/)
   - DDU Method: [Deterministic Neural Networks with Inductive Biases](http://www.gatsby.ucl.ac.uk/~balaji/udl2021/accepted-papers/UDL2021-paper-022.pdf)

4. **Semantic Entropy**
   - Farquhar et al., 2024: [Semantic Entropy Probes for Hallucination Detection](https://www.researchgate.net/publication/381666455_Semantic_Entropy_Probes_Robust_and_Cheap_Hallucination_Detection_in_LLMs)

5. **Knowledge Graph Consistency**
   - WikiData Classification, 2025: [Embedding-based Consistency Detection](https://arxiv.org/html/2511.04926)

6. **Topic Coherence**
   - ACM SIGIR 2016: [Word Embedding Topic Coherence Metrics](https://dl.acm.org/doi/10.1145/2911451.2914729)

7. **Latent Space Dynamics**
   - arXiv 2025: [Navigating the Latent Space Dynamics of Neural Models](https://arxiv.org/abs/2505.22785)

### Internal Documents

- `contextprd.md` - Main PRD with TeleologicalFingerprint specification
- `improvementreport.md` - Research synthesis and optimization recommendations
- `projectionplan1.md` / `projectionplan2.md` - Implementation roadmap

---

## Appendix A: Space Index Reference

| Index | Space | Dimension | Primary Use |
|-------|-------|-----------|-------------|
| 0 | E1 Semantic | 1024D | Core meaning |
| 1 | E2 Temporal-Recent | 512D | Recency decay |
| 2 | E3 Temporal-Periodic | 512D | Cyclical patterns |
| 3 | E4 Temporal-Positional | 512D | Sequence order |
| 4 | E5 Causal | 768D | Cause-effect |
| 5 | E6 Sparse | ~30K | Keyword precision |
| 6 | E7 Code | 1536D | AST semantics |
| 7 | E8 Graph | 384D | Structural |
| 8 | E9 HDC | 1024D | Robustness |
| 9 | E10 Multimodal | 768D | Cross-modal |
| 10 | E11 Entity | 384D | Factual |
| 11 | E12 Late | 128/tok | Token precision |

---

## Appendix B: Johari Quadrant Quick Reference

| Quadrant | ΔS | ΔC | Interpretation | Action |
|----------|----|----|----------------|--------|
| **Open** | Low | High | Well-understood, integrated | Leverage |
| **Blind** | High | Low | Novel but isolated | Investigate |
| **Hidden** | Low | Low | Known but dormant | Consider revival |
| **Unknown** | High | High | Frontier territory | Explore |

---

*Document generated: 2026-01-05*
*Implementation Target: Phase 2 (Weeks 5-8)*
