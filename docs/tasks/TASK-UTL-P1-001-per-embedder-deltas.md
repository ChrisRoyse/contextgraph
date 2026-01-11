# TASK-UTL-P1-001: Implement Per-Embedder DeltaS Entropy Methods

```xml
<task_spec id="TASK-UTL-P1-001" version="1.0">
<metadata>
  <title>Implement Per-Embedder DeltaS Entropy Methods</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>1</sequence>
  <implements>
    <item>PRD Section: Per-embedder entropy calculation (Sherlock-05)</item>
    <item>constitution.yaml delta_sc.ΔS_methods - E1: GMM+Mahalanobis, E5: Asymmetric KNN, E9: Hamming, E13: Jaccard</item>
    <item>ARCH-02: Compare Only Compatible Embedding Types (Apples-to-Apples)</item>
  </implements>
  <depends_on>
    <!-- Foundation types already exist -->
    <task_ref>context-graph-core::teleological::embedder::Embedder</task_ref>
    <task_ref>context-graph-utl::surprise::SurpriseCalculator</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<context>
The current UTL system uses a UNIFIED KNN/cosine-based entropy (deltaS) calculation for ALL 13 embedders.
This violates the constitution.yaml specification which mandates per-embedder entropy methods that leverage
each embedding space's semantic properties:

- E1 (Semantic): GMM + Mahalanobis distance for capturing distribution-based surprise
- E5 (Causal): Asymmetric KNN with directional modifier for cause/effect relationships
- E9 (HDC): Hamming distance to learned prototypes for hyperdimensional binary patterns
- E13 (SPLADE): 1 - Jaccard(active_dims) for sparse lexical coverage

The current `SurpriseCalculator` applies cosine distance uniformly, losing the semantic richness of each
embedding type. This task creates an `EmbedderEntropy` trait and implements type-specific entropy calculators.
</context>

<input_context_files>
  <file purpose="Embedder enum with all 13 types and their properties">crates/context-graph-core/src/teleological/embedder.rs</file>
  <file purpose="Current unified surprise calculator to understand existing API">crates/context-graph-utl/src/surprise/embedding_distance.rs</file>
  <file purpose="Current SurpriseCalculator main compute methods">crates/context-graph-utl/src/surprise/calculator/compute.rs</file>
  <file purpose="SurpriseCalculator type definition">crates/context-graph-utl/src/surprise/calculator/types.rs</file>
  <file purpose="Constitution specification for deltaS methods">docs2/constitution.yaml</file>
  <file purpose="UTL crate public API and module structure">crates/context-graph-utl/src/lib.rs</file>
  <file purpose="SurpriseConfig for configuration">crates/context-graph-utl/src/config/surprise.rs</file>
</input_context_files>

<prerequisites>
  <check>Embedder enum exists in context-graph-core with all 13 variants</check>
  <check>SurpriseCalculator exists with compute_surprise() method</check>
  <check>EmbeddingDistanceCalculator exists with cosine distance implementation</check>
  <check>context-graph-utl compiles without errors</check>
</prerequisites>

<scope>
  <in_scope>
    - Create EmbedderEntropy trait with compute_delta_s() method
    - Implement GmmMahalanobisEntropy for E1 (Semantic)
    - Implement AsymmetricKnnEntropy for E5 (Causal)
    - Implement HammingPrototypeEntropy for E9 (HDC)
    - Implement JaccardActiveEntropy for E13 (SPLADE)
    - Implement DefaultKnnEntropy fallback for other embedders (E2-E4, E6-E8, E10-E12)
    - Create EmbedderEntropyFactory to instantiate correct calculator per Embedder
    - Wire into existing SurpriseCalculator via composition
    - Unit tests for each entropy implementation
  </in_scope>
  <out_of_scope>
    - GPU/CUDA acceleration (handled by separate task)
    - Adaptive threshold calibration (NORTH-009)
    - Multi-embedder aggregation strategy (handled by UtlProcessor)
    - Token-level E12 entropy (complex MaxSim - separate task)
    - GMM model training/persistence (uses pretrained GMM or fits online)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs">
/// Per-embedder entropy computation trait.
///
/// Each embedder type has a specialized deltaS calculation that leverages
/// the semantic properties of that embedding space.
pub trait EmbedderEntropy: Send + Sync {
    /// Compute deltaS (surprise/entropy) for this embedder type.
    ///
    /// # Arguments
    /// * `current` - The current embedding vector
    /// * `history` - Recent embedding vectors (most recent first)
    /// * `k` - Number of neighbors for KNN-based methods
    ///
    /// # Returns
    /// DeltaS value in [0.0, 1.0], or error if computation fails.
    fn compute_delta_s(
        &amp;self,
        current: &amp;[f32],
        history: &amp;[Vec&lt;f32&gt;],
        k: usize,
    ) -> UtlResult&lt;f32&gt;;

    /// Get the embedder type this calculator handles.
    fn embedder_type(&amp;self) -> Embedder;

    /// Reset any internal state (e.g., running statistics).
    fn reset(&amp;mut self);
}
    </signature>

    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/gmm_mahalanobis.rs">
/// E1 (Semantic) entropy calculator using GMM + Mahalanobis distance.
///
/// Computes: ΔS = 1 - P(e|GMM)
/// where P(e|GMM) is the probability of embedding under a fitted GMM.
pub struct GmmMahalanobisEntropy {
    /// Number of GMM components
    n_components: usize,
    /// Component means (n_components x dim)
    means: Vec&lt;Vec&lt;f32&gt;&gt;,
    /// Component covariances (diagonal or full)
    covariances: Vec&lt;Vec&lt;f32&gt;&gt;,
    /// Component weights (mixing coefficients)
    weights: Vec&lt;f32&gt;,
    /// Minimum probability floor to avoid deltaS=1 for all novel inputs
    min_probability: f32,
}

impl GmmMahalanobisEntropy {
    pub fn new(n_components: usize) -> Self;
    pub fn fit(&amp;mut self, embeddings: &amp;[Vec&lt;f32&gt;]) -> UtlResult&lt;()&gt;;
    pub fn fit_incremental(&amp;mut self, embedding: &amp;[f32]) -> UtlResult&lt;()&gt;;
}

impl EmbedderEntropy for GmmMahalanobisEntropy { ... }
    </signature>

    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/asymmetric_knn.rs">
/// E5 (Causal) entropy calculator using asymmetric KNN.
///
/// Computes: ΔS = d_k × direction_mod
/// where direction_mod accounts for causal direction (cause→effect vs effect→cause).
pub struct AsymmetricKnnEntropy {
    /// Direction modifier for cause→effect
    cause_to_effect_mod: f32,  // Default: 1.2
    /// Direction modifier for effect→cause
    effect_to_cause_mod: f32,  // Default: 0.8
    /// Base k for KNN
    k: usize,
}

impl AsymmetricKnnEntropy {
    pub fn new(k: usize) -> Self;
    pub fn with_direction_modifiers(self, cause_to_effect: f32, effect_to_cause: f32) -> Self;
}

impl EmbedderEntropy for AsymmetricKnnEntropy { ... }
    </signature>

    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/hamming_prototype.rs">
/// E9 (HDC) entropy calculator using Hamming distance to prototypes.
///
/// Computes: ΔS = min_hamming(e, prototypes) / dim
/// For HDC binary vectors projected to dense representation.
pub struct HammingPrototypeEntropy {
    /// Learned prototype vectors
    prototypes: Vec&lt;Vec&lt;f32&gt;&gt;,
    /// Threshold for binarization (0.0 for already-projected)
    binarization_threshold: f32,
    /// Maximum number of prototypes to maintain
    max_prototypes: usize,
}

impl HammingPrototypeEntropy {
    pub fn new(max_prototypes: usize) -> Self;
    pub fn add_prototype(&amp;mut self, embedding: &amp;[f32]);
    pub fn learn_prototypes(&amp;mut self, embeddings: &amp;[Vec&lt;f32&gt;], n_prototypes: usize);
}

impl EmbedderEntropy for HammingPrototypeEntropy { ... }
    </signature>

    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/jaccard_active.rs">
/// E13 (SPLADE) entropy calculator using Jaccard similarity of active dimensions.
///
/// Computes: ΔS = 1 - jaccard(active_dims(current), active_dims(history))
/// For sparse vectors where most dimensions are zero.
pub struct JaccardActiveEntropy {
    /// Threshold for considering a dimension "active"
    activation_threshold: f32,
    /// Smoothing factor for empty intersection
    smoothing: f32,
}

impl JaccardActiveEntropy {
    pub fn new() -> Self;
    pub fn with_threshold(self, threshold: f32) -> Self;
}

impl EmbedderEntropy for JaccardActiveEntropy { ... }
    </signature>

    <signature file="crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs">
/// Factory for creating per-embedder entropy calculators.
pub struct EmbedderEntropyFactory;

impl EmbedderEntropyFactory {
    /// Create the appropriate entropy calculator for an embedder.
    pub fn create(embedder: Embedder, config: &amp;SurpriseConfig) -> Box&lt;dyn EmbedderEntropy&gt;;

    /// Create all 13 entropy calculators.
    pub fn create_all(config: &amp;SurpriseConfig) -> [Box&lt;dyn EmbedderEntropy&gt;; 13];
}
    </signature>
  </signatures>

  <constraints>
    - All deltaS outputs MUST be clamped to [0.0, 1.0] (AP-10: no NaN/Infinity)
    - Each calculator MUST handle empty history (return 1.0 = maximum surprise)
    - Each calculator MUST handle empty current embedding (return error)
    - GMM fitting MUST handle insufficient data gracefully (fall back to KNN)
    - Hamming prototype learning MUST limit memory (max_prototypes parameter)
    - Jaccard MUST handle sparse vectors without materializing dense representation
    - All implementations MUST be Send + Sync for concurrent usage
    - Factory MUST return correct type based on Embedder variant (no runtime panics)
  </constraints>

  <verification>
    - cargo test --package context-graph-utl --lib surprise::embedder_entropy passes
    - Each entropy calculator has >90% test coverage
    - GMM computes meaningful probabilities for fitted distributions
    - Asymmetric KNN direction modifiers affect output predictably
    - Hamming distance is 0 for identical prototypes, max for orthogonal
    - Jaccard is 0 for disjoint active sets, 1 for identical active sets
    - Factory creates correct type for each of 13 embedders
  </verification>
</definition_of_done>

<pseudo_code>
EmbedderEntropy Trait (crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs):
  - Define trait with compute_delta_s(), embedder_type(), reset()
  - Export all implementations

GmmMahalanobisEntropy (E1):
  - new(n_components): Initialize empty GMM
  - fit(embeddings): Run EM algorithm to fit GMM
    - Initialize means via k-means++
    - E-step: compute responsibilities
    - M-step: update means, covariances, weights
    - Repeat until convergence or max_iter
  - fit_incremental(embedding): Online EM update
  - compute_delta_s(current, history, k):
    - If not fitted, call fit(history)
    - For each component i:
      - Compute Mahalanobis distance: d_i = sqrt((x-μ_i)ᵀ Σ_i⁻¹ (x-μ_i))
      - Compute probability: p_i = w_i × N(x; μ_i, Σ_i)
    - Total probability: P = Σ p_i
    - Return: 1.0 - P.clamp(min_probability, 1.0)

AsymmetricKnnEntropy (E5):
  - new(k): Store k and default direction modifiers
  - compute_delta_s(current, history, k):
    - Compute cosine distances to all history items
    - Sort and take k nearest
    - Average distance: d_k = mean(top_k_distances)
    - Apply direction modifier based on embedding metadata (if available)
    - Return: (d_k × direction_mod).clamp(0.0, 1.0)

HammingPrototypeEntropy (E9):
  - new(max_prototypes): Initialize empty prototype set
  - add_prototype(embedding): Add if under limit or replace least-used
  - learn_prototypes(embeddings, n): Cluster and extract centroids
  - compute_delta_s(current, history, k):
    - If no prototypes, learn from history
    - Binarize current (threshold > 0) or use directly
    - For each prototype:
      - Compute Hamming distance: sum(current[i] != prototype[i])
    - min_hamming = min(all distances)
    - Return: (min_hamming / dim).clamp(0.0, 1.0)

JaccardActiveEntropy (E13):
  - new(): Default threshold = 0.0 (any non-zero is active)
  - with_threshold(t): Set custom activation threshold
  - compute_delta_s(current, history, k):
    - active_current = {i : current[i] > threshold}
    - For each history item h in most_recent(k):
      - active_h = {i : h[i] > threshold}
      - intersection = active_current ∩ active_h
      - union = active_current ∪ active_h
      - jaccard = |intersection| / |union| if |union| > 0 else 0
    - avg_jaccard = mean(all jaccard scores)
    - Return: (1.0 - avg_jaccard).clamp(0.0, 1.0)

DefaultKnnEntropy (fallback for E2-E4, E6-E8, E10-E12):
  - Wrap existing EmbeddingDistanceCalculator
  - compute_delta_s: delegate to compute_surprise()

EmbedderEntropyFactory:
  - create(embedder, config):
    - match embedder:
      - Semantic => Box::new(GmmMahalanobisEntropy::new(config.gmm_components))
      - Causal => Box::new(AsymmetricKnnEntropy::new(config.knn_k))
      - Hdc => Box::new(HammingPrototypeEntropy::new(config.max_prototypes))
      - KeywordSplade => Box::new(JaccardActiveEntropy::new())
      - _ => Box::new(DefaultKnnEntropy::from_config(config))
  - create_all(config):
    - [create(E1), create(E2), ..., create(E13)]
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs">Module root with EmbedderEntropy trait and re-exports</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/gmm_mahalanobis.rs">E1 GMM + Mahalanobis implementation</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/asymmetric_knn.rs">E5 Asymmetric KNN implementation</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/hamming_prototype.rs">E9 Hamming distance to prototypes</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/jaccard_active.rs">E13 Jaccard active dimensions</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/default_knn.rs">Default KNN wrapper for other embedders</file>
  <file path="crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs">Factory for creating per-embedder calculators</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-utl/src/surprise/mod.rs">Add pub mod embedder_entropy and re-exports</file>
  <file path="crates/context-graph-utl/src/config/surprise.rs">Add GMM, prototype, and Jaccard config fields</file>
</files_to_modify>

<validation_criteria>
  <criterion>EmbedderEntropy trait compiles with all required methods</criterion>
  <criterion>GmmMahalanobisEntropy computes deltaS in [0,1] for E1 embeddings</criterion>
  <criterion>AsymmetricKnnEntropy applies direction modifiers correctly</criterion>
  <criterion>HammingPrototypeEntropy returns 0 for exact prototype match</criterion>
  <criterion>JaccardActiveEntropy returns 0 for identical sparse vectors</criterion>
  <criterion>Factory creates correct type for each Embedder variant</criterion>
  <criterion>All implementations handle edge cases (empty history, empty embedding)</criterion>
  <criterion>No panics in any code path (all errors return Result)</criterion>
  <criterion>cargo clippy --package context-graph-utl passes with no warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --package context-graph-utl</command>
  <command>cargo test --package context-graph-utl --lib surprise::embedder_entropy</command>
  <command>cargo test --package context-graph-utl --lib surprise::embedder_entropy -- --nocapture</command>
  <command>cargo clippy --package context-graph-utl -- -D warnings</command>
</test_commands>
</task_spec>
```

## Rationale

### Why Per-Embedder Entropy?

The constitution.yaml (section `delta_sc.ΔS_methods`) specifies that each embedder type should use semantically appropriate entropy calculations:

| Embedder | Method | Rationale |
|----------|--------|-----------|
| E1 (Semantic) | GMM + Mahalanobis | Dense semantic space is well-modeled by mixture of Gaussians; Mahalanobis captures covariance structure |
| E5 (Causal) | Asymmetric KNN | Causal relationships are directional; cause→effect is different from effect→cause |
| E9 (HDC) | Hamming to prototypes | HDC binary vectors use Hamming distance naturally; prototypes capture learned categories |
| E13 (SPLADE) | 1 - Jaccard(active) | Sparse vectors are defined by which dimensions are active; Jaccard measures overlap |

### Current Problem

The existing `SurpriseCalculator` uses a single `EmbeddingDistanceCalculator` that computes cosine distance uniformly:

```rust
// Current code in embedding_distance.rs
let dist = compute_cosine_distance(current, past);
```

This loses semantic richness because:
- GMM can capture multi-modal distributions in E1 that KNN misses
- Causal direction is ignored in E5 (should boost cause→effect surprise)
- Hamming distance is natural for HDC but cosine is used instead
- Sparse vectors are treated as dense (inefficient and semantically wrong)

### Implementation Strategy

1. **Trait-based design**: `EmbedderEntropy` trait allows polymorphic dispatch while maintaining type safety
2. **Factory pattern**: `EmbedderEntropyFactory` creates the correct implementation based on `Embedder` variant
3. **Composition over inheritance**: SurpriseCalculator will compose 13 entropy calculators
4. **Fallback strategy**: Unknown embedders use DefaultKnnEntropy (existing behavior)

### Dependencies

```
context-graph-core::teleological::Embedder (exists)
    |
    v
context-graph-utl::surprise::embedder_entropy::EmbedderEntropy (NEW)
    |
    +-- GmmMahalanobisEntropy (E1)
    +-- AsymmetricKnnEntropy (E5)
    +-- HammingPrototypeEntropy (E9)
    +-- JaccardActiveEntropy (E13)
    +-- DefaultKnnEntropy (E2-E4, E6-E8, E10-E12)
    |
    v
context-graph-utl::surprise::SurpriseCalculator (modify to use factory)
```

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| GMM fitting fails on small history | Fall back to KNN if < n_components samples |
| Hamming prototypes consume memory | max_prototypes limit with LRU eviction |
| Jaccard division by zero | Smoothing factor and union size check |
| Direction metadata unavailable for E5 | Default to 1.0 modifier (neutral) |

## References

- constitution.yaml lines 791-807 (delta_sc.ΔS_methods)
- constitution.yaml lines 618-620 (E5 Causal asymmetric: true)
- constitution.yaml lines 648-656 (E9 HDC XOR_Hamming)
- constitution.yaml lines 683-687 (E13 SPLADE sparse)
- embedder.rs lines 44, 52, 58-60 (Embedder variants)
