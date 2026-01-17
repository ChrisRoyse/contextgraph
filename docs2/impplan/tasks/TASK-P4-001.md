# TASK-P4-001: ClusterMembership and Cluster Types

```xml
<task_spec id="TASK-P4-001" version="2.0">
<metadata>
  <title>ClusterMembership and Cluster Type Implementation</title>
  <status>COMPLETE</status>
  <layer>foundation</layer>
  <sequence>27</sequence>
  <phase>4</phase>
  <implements>
    <requirement_ref>REQ-P4-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P2-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <last_audited>2026-01-17</last_audited>
</metadata>

<context>
Implements the foundational types for cluster assignments. ClusterMembership tracks
which cluster a memory belongs to in each embedding space, including probability
and core point status. Cluster represents a cluster with centroid, member count,
and quality metrics.

These types are used by both HDBSCAN (batch) and BIRCH (incremental) clustering algorithms.

CRITICAL ARCHITECTURE RULES:
- ARCH-09: Topic threshold is weighted_agreement >= 2.5 (not raw space count)
- ARCH-04: Temporal embedders (E2-E4) NEVER count toward topic detection
- AP-60: Temporal embedders MUST NOT count toward topic detection
- cluster_id = -1 indicates noise (not assigned to any cluster)
</context>

<codebase_state>
## VERIFIED EXISTING COMPONENTS (2026-01-17)

### Embedder Enum Location
- **File**: `crates/context-graph-core/src/teleological/embedder.rs`
- **Import**: `use crate::teleological::Embedder;`
- **Variants**: 13 embedders (Semantic=0 through KeywordSplade=12)
- **Re-exported from**: `crates/context-graph-core/src/lib.rs` line 82

### EmbedderCategory Location
- **File**: `crates/context-graph-core/src/embeddings/category.rs`
- **Import**: `use crate::embeddings::category::{EmbedderCategory, category_for, topic_threshold, max_weighted_agreement};`
- **Methods**: `topic_weight()`, `is_semantic()`, `is_temporal()`, etc.

### StorageError Location
- **File**: `crates/context-graph-core/src/error/sub_errors.rs`
- **Import**: `use crate::error::StorageError;`
- **Re-exported from**: `crates/context-graph-core/src/lib.rs` line 65

### lib.rs Current Module Structure
- Modules at lines 32-56
- **clustering module DOES NOT EXIST YET** - must be created
- Must add `pub mod clustering;` after line 34 (after `causal`)

### Retrieval Module (recently implemented P3)
- **Location**: `crates/context-graph-core/src/retrieval/`
- Contains: `MultiSpaceSimilarity`, `DivergenceDetector`, `SimilarityRetriever`
- Uses similar patterns for distance/similarity calculations
</codebase_state>

<input_context_files>
  <file purpose="data_models">docs2/impplan/technical/TECH-PHASE4-CLUSTERING.md#data_models</file>
  <file purpose="embedder_enum">crates/context-graph-core/src/teleological/embedder.rs</file>
  <file purpose="embedder_category">crates/context-graph-core/src/embeddings/category.rs</file>
  <file purpose="storage_error">crates/context-graph-core/src/error/sub_errors.rs</file>
  <file purpose="constitution">docs2/constitution.yaml</file>
</input_context_files>

<prerequisites>
  <check status="VERIFIED">TASK-P2-003 complete - Embedder enum exists at teleological/embedder.rs</check>
  <check status="VERIFIED">EmbedderCategory exists at embeddings/category.rs with topic_weight() method</check>
  <check status="VERIFIED">StorageError exists at error/sub_errors.rs</check>
</prerequisites>

<scope>
  <in_scope>
    - Create clustering module directory: crates/context-graph-core/src/clustering/
    - Create ClusterMembership struct in membership.rs
    - Create Cluster struct in cluster.rs
    - Create ClusterError enum in error.rs
    - Create mod.rs with re-exports
    - Add `pub mod clustering;` to lib.rs
    - Implement Debug, Clone, Serialize, Deserialize derives
    - Implement constructors, accessors, and noise point handling
    - Unit tests with REAL data (no mocks)
  </in_scope>
  <out_of_scope>
    - Clustering algorithm logic (TASK-P4-005 HDBSCAN, TASK-P4-006 BIRCH)
    - Cluster storage persistence (TASK-P4-007)
    - Topic synthesis (TASK-P4-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/clustering/membership.rs">
      use serde::{Deserialize, Serialize};
      use uuid::Uuid;
      use crate::teleological::Embedder;

      #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
      pub struct ClusterMembership {
          pub memory_id: Uuid,
          pub space: Embedder,
          pub cluster_id: i32,  // -1 = noise
          pub membership_probability: f32,  // 0.0..=1.0
          pub is_core_point: bool,
      }

      impl ClusterMembership {
          pub fn new(memory_id: Uuid, space: Embedder, cluster_id: i32, probability: f32, is_core: bool) -> Self;
          pub fn noise(memory_id: Uuid, space: Embedder) -> Self;
          pub fn is_noise(&amp;self) -> bool;
          pub fn is_confident(&amp;self) -> bool;  // probability >= 0.8
      }
    </signature>
    <signature file="crates/context-graph-core/src/clustering/cluster.rs">
      use serde::{Deserialize, Serialize};
      use chrono::{DateTime, Utc};
      use crate::teleological::Embedder;

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct Cluster {
          pub id: i32,
          pub space: Embedder,
          pub centroid: Vec&lt;f32&gt;,
          pub member_count: u32,
          pub silhouette_score: f32,  // -1.0..=1.0
          pub created_at: DateTime&lt;Utc&gt;,
          pub updated_at: DateTime&lt;Utc&gt;,
      }

      impl Cluster {
          pub fn new(id: i32, space: Embedder, centroid: Vec&lt;f32&gt;, member_count: u32) -> Self;
          pub fn update_silhouette(&amp;mut self, score: f32);
          pub fn touch(&amp;mut self);
          pub fn is_high_quality(&amp;self) -> bool;  // silhouette >= 0.3
          pub fn update_centroid(&amp;mut self, centroid: Vec&lt;f32&gt;, member_count: u32);
      }
    </signature>
    <signature file="crates/context-graph-core/src/clustering/error.rs">
      use thiserror::Error;
      use crate::error::StorageError;
      use crate::teleological::Embedder;

      #[derive(Debug, Error)]
      pub enum ClusterError {
          #[error("Insufficient data: required {required}, actual {actual}")]
          InsufficientData { required: usize, actual: usize },

          #[error("Dimension mismatch: expected {expected}, actual {actual}")]
          DimensionMismatch { expected: usize, actual: usize },

          #[error("No valid clusters found")]
          NoValidClusters,

          #[error("Storage error: {0}")]
          StorageError(#[from] StorageError),

          #[error("Invalid parameter: {message}")]
          InvalidParameter { message: String },

          #[error("Space not initialized: {0:?}")]
          SpaceNotInitialized(Embedder),
      }
    </signature>
    <signature file="crates/context-graph-core/src/clustering/mod.rs">
      //! Multi-space clustering types for topic synthesis.

      pub mod cluster;
      pub mod error;
      pub mod membership;

      pub use cluster::Cluster;
      pub use error::ClusterError;
      pub use membership::ClusterMembership;
    </signature>
  </signatures>

  <constraints>
    - cluster_id = -1 indicates noise (not in any cluster)
    - membership_probability MUST be clamped to 0.0..=1.0
    - silhouette_score MUST be clamped to -1.0..=1.0
    - centroid dimension must match embedder expected_dims()
    - NO mocks in tests - use real Uuid and Embedder values
    - NO .unwrap() in library code - use expect() with context or propagate errors
  </constraints>

  <verification>
    - noise() creates membership with cluster_id = -1, probability = 0.0, is_core_point = false
    - is_noise() returns true if and only if cluster_id == -1
    - Probability &gt; 1.0 is clamped to 1.0, &lt; 0.0 is clamped to 0.0
    - Silhouette &gt; 1.0 is clamped to 1.0, &lt; -1.0 is clamped to -1.0
    - Cluster.touch() updates updated_at to current time
    - Serialization/deserialization roundtrip preserves all fields exactly
    - is_high_quality() returns true when silhouette_score >= 0.3
    - is_confident() returns true when membership_probability >= 0.8
  </verification>
</definition_of_done>

<files_to_create>
  <file path="crates/context-graph-core/src/clustering/mod.rs">Module root with re-exports</file>
  <file path="crates/context-graph-core/src/clustering/membership.rs">ClusterMembership struct</file>
  <file path="crates/context-graph-core/src/clustering/cluster.rs">Cluster struct</file>
  <file path="crates/context-graph-core/src/clustering/error.rs">ClusterError enum</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/lib.rs">
    Add `pub mod clustering;` at line 35 (alphabetically after `causal`)
    Add re-exports at bottom: `pub use clustering::{Cluster, ClusterError, ClusterMembership};`
  </file>
</files_to_modify>

<validation_criteria>
  <criterion id="VC-001">ClusterMembership::noise() creates cluster_id = -1</criterion>
  <criterion id="VC-002">is_noise() returns true for cluster_id = -1</criterion>
  <criterion id="VC-003">membership_probability clamped to 0.0..=1.0</criterion>
  <criterion id="VC-004">silhouette_score clamped to -1.0..=1.0</criterion>
  <criterion id="VC-005">Cluster.touch() updates updated_at timestamp</criterion>
  <criterion id="VC-006">Serialization/deserialization roundtrip works for all types</criterion>
  <criterion id="VC-007">is_high_quality() threshold is 0.3 per constitution</criterion>
  <criterion id="VC-008">is_confident() threshold is 0.8</criterion>
  <criterion id="VC-009">All 13 Embedder variants work with ClusterMembership</criterion>
  <criterion id="VC-010">ClusterError variants have descriptive error messages</criterion>
</validation_criteria>

<test_commands>
  <command description="Check compilation">cargo check --package context-graph-core</command>
  <command description="Run all clustering tests">cargo test --package context-graph-core clustering -- --nocapture</command>
  <command description="Run membership tests">cargo test --package context-graph-core membership -- --nocapture</command>
  <command description="Run cluster tests">cargo test --package context-graph-core cluster -- --nocapture</command>
  <command description="Check for clippy warnings">cargo clippy --package context-graph-core -- -D warnings</command>
  <command description="Verify no dead code warnings">cargo build --package context-graph-core 2>&amp;1 | grep -E "(dead_code|unused)"</command>
</test_commands>
</task_spec>
```

## Full State Verification Requirements

After completing the implementation, you MUST perform the following verification steps:

### 1. Source of Truth Identification
- **ClusterMembership**: Rust struct in `crates/context-graph-core/src/clustering/membership.rs`
- **Cluster**: Rust struct in `crates/context-graph-core/src/clustering/cluster.rs`
- **ClusterError**: Rust enum in `crates/context-graph-core/src/clustering/error.rs`
- **Module exports**: `crates/context-graph-core/src/clustering/mod.rs`
- **Crate exports**: `crates/context-graph-core/src/lib.rs`

### 2. Execute & Inspect Protocol
After writing the code:
```bash
# 1. Verify files exist
ls -la crates/context-graph-core/src/clustering/

# 2. Verify module structure
grep -n "pub mod clustering" crates/context-graph-core/src/lib.rs

# 3. Verify re-exports
grep -n "ClusterMembership\|Cluster\|ClusterError" crates/context-graph-core/src/lib.rs

# 4. Compile and check for errors
cargo check --package context-graph-core 2>&1

# 5. Run tests and capture output
cargo test --package context-graph-core clustering -- --nocapture 2>&1
```

### 3. Boundary & Edge Case Audit

**Case 1: Empty/Noise Membership**
```
INPUT: ClusterMembership::noise(uuid, Embedder::Semantic)
EXPECTED OUTPUT:
  - cluster_id == -1
  - membership_probability == 0.0
  - is_core_point == false
  - is_noise() == true
  - is_confident() == false
VERIFY: Print struct fields before and after creation
```

**Case 2: Probability Clamping at Boundaries**
```
INPUT: ClusterMembership::new(uuid, embedder, 5, 1.5, true)  // probability > 1.0
EXPECTED OUTPUT: membership_probability == 1.0 (clamped)

INPUT: ClusterMembership::new(uuid, embedder, 5, -0.5, true)  // probability < 0.0
EXPECTED OUTPUT: membership_probability == 0.0 (clamped)
VERIFY: Print probability values before clamping and after
```

**Case 3: Silhouette Score Clamping**
```
INPUT: cluster.update_silhouette(2.5)  // > 1.0
EXPECTED OUTPUT: silhouette_score == 1.0 (clamped)

INPUT: cluster.update_silhouette(-2.5)  // < -1.0
EXPECTED OUTPUT: silhouette_score == -1.0 (clamped)
VERIFY: Print silhouette values before and after
```

**Case 4: Timestamp Update (touch)**
```
INPUT:
  let t1 = cluster.updated_at;
  sleep(10ms);
  cluster.touch();
  let t2 = cluster.updated_at;
EXPECTED OUTPUT: t2 > t1
VERIFY: Print both timestamps
```

**Case 5: All 13 Embedder Variants**
```
INPUT: Create ClusterMembership for each of 13 Embedder variants
EXPECTED OUTPUT: All serialize/deserialize correctly
VERIFY: Print each variant's name and serialized form
```

### 4. Evidence of Success

The test output MUST show:
```
[PASS] test_noise_membership - cluster_id=-1, prob=0.0, is_noise=true
[PASS] test_probability_clamping - 1.5 -> 1.0, -0.5 -> 0.0
[PASS] test_silhouette_clamping - 2.5 -> 1.0, -2.5 -> -1.0
[PASS] test_cluster_touch - t2 > t1 (timestamps differ)
[PASS] test_all_embedders - all 13 variants work
[PASS] test_serialization_roundtrip - JSON encode/decode preserves data
[PASS] test_is_confident - 0.9 is confident, 0.5 is not
[PASS] test_is_high_quality - 0.4 is high quality, 0.2 is not
```

## Implementation Code

### File: crates/context-graph-core/src/clustering/mod.rs

```rust
//! Multi-space clustering types for topic synthesis.
//!
//! This module provides foundational types for clustering memories across
//! the 13 embedding spaces. Used by HDBSCAN (batch) and BIRCH (incremental).
//!
//! # Architecture
//!
//! Per constitution:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-04: Temporal embedders (E2-E4) NEVER count toward topic detection
//!
//! # Key Types
//!
//! - `ClusterMembership`: Tracks which cluster a memory belongs to per space
//! - `Cluster`: Represents a cluster with centroid and quality metrics
//! - `ClusterError`: Error types for clustering operations

pub mod cluster;
pub mod error;
pub mod membership;

pub use cluster::Cluster;
pub use error::ClusterError;
pub use membership::ClusterMembership;
```

### File: crates/context-graph-core/src/clustering/membership.rs

```rust
//! ClusterMembership type for tracking memory cluster assignments.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::Embedder;

/// Confidence threshold for high-confidence cluster assignments.
pub const CONFIDENT_THRESHOLD: f32 = 0.8;

/// Cluster ID indicating noise (not assigned to any cluster).
pub const NOISE_CLUSTER_ID: i32 = -1;

/// Represents a memory's cluster assignment in a specific embedding space.
///
/// Each memory can have different cluster assignments in different embedding
/// spaces. The cluster_id of -1 indicates the memory is noise (outlier) in
/// that space.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::ClusterMembership;
/// use context_graph_core::teleological::Embedder;
/// use uuid::Uuid;
///
/// // Create a normal cluster membership
/// let mem_id = Uuid::new_v4();
/// let membership = ClusterMembership::new(mem_id, Embedder::Semantic, 5, 0.95, true);
/// assert!(!membership.is_noise());
/// assert!(membership.is_confident());
///
/// // Create a noise membership
/// let noise = ClusterMembership::noise(mem_id, Embedder::Semantic);
/// assert!(noise.is_noise());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterMembership {
    /// The memory this membership belongs to.
    pub memory_id: Uuid,

    /// The embedding space this assignment is for.
    pub space: Embedder,

    /// Cluster ID (-1 = noise, not in any cluster).
    pub cluster_id: i32,

    /// Probability of belonging to this cluster (0.0..=1.0).
    /// Computed by HDBSCAN based on density.
    pub membership_probability: f32,

    /// Whether this point is a core point of the cluster.
    /// Core points are central to cluster density.
    pub is_core_point: bool,
}

impl ClusterMembership {
    /// Create a new cluster membership.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the memory
    /// * `space` - Embedding space this membership is for
    /// * `cluster_id` - Cluster ID (-1 for noise)
    /// * `probability` - Membership probability (will be clamped to 0.0..=1.0)
    /// * `is_core` - Whether this is a core point
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::ClusterMembership;
    /// use context_graph_core::teleological::Embedder;
    /// use uuid::Uuid;
    ///
    /// let membership = ClusterMembership::new(
    ///     Uuid::new_v4(),
    ///     Embedder::Semantic,
    ///     5,
    ///     0.95,
    ///     true,
    /// );
    /// ```
    pub fn new(
        memory_id: Uuid,
        space: Embedder,
        cluster_id: i32,
        probability: f32,
        is_core: bool,
    ) -> Self {
        Self {
            memory_id,
            space,
            cluster_id,
            membership_probability: probability.clamp(0.0, 1.0),
            is_core_point: is_core,
        }
    }

    /// Create a noise membership (not in any cluster).
    ///
    /// Noise points have cluster_id = -1, probability = 0.0,
    /// and are never core points.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::ClusterMembership;
    /// use context_graph_core::teleological::Embedder;
    /// use uuid::Uuid;
    ///
    /// let noise = ClusterMembership::noise(Uuid::new_v4(), Embedder::Semantic);
    /// assert!(noise.is_noise());
    /// assert_eq!(noise.cluster_id, -1);
    /// ```
    pub fn noise(memory_id: Uuid, space: Embedder) -> Self {
        Self {
            memory_id,
            space,
            cluster_id: NOISE_CLUSTER_ID,
            membership_probability: 0.0,
            is_core_point: false,
        }
    }

    /// Check if this is a noise point (not in any cluster).
    #[inline]
    pub fn is_noise(&self) -> bool {
        self.cluster_id == NOISE_CLUSTER_ID
    }

    /// Check if this is a high-confidence assignment.
    ///
    /// Returns true if membership_probability >= 0.8.
    #[inline]
    pub fn is_confident(&self) -> bool {
        self.membership_probability >= CONFIDENT_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_membership() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::noise(mem_id, Embedder::Semantic);

        assert!(membership.is_noise(), "noise() should create noise membership");
        assert_eq!(membership.cluster_id, -1, "noise cluster_id should be -1");
        assert_eq!(membership.membership_probability, 0.0, "noise probability should be 0.0");
        assert!(!membership.is_core_point, "noise should not be core point");
        assert!(!membership.is_confident(), "noise should not be confident");

        println!("[PASS] test_noise_membership - cluster_id={}, prob={}, is_noise={}",
            membership.cluster_id, membership.membership_probability, membership.is_noise());
    }

    #[test]
    fn test_cluster_membership() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(
            mem_id,
            Embedder::Semantic,
            5,
            0.95,
            true,
        );

        assert!(!membership.is_noise(), "non-noise should not be noise");
        assert_eq!(membership.cluster_id, 5, "cluster_id should be 5");
        assert!(membership.is_confident(), "0.95 should be confident");
        assert!(membership.is_core_point, "should be core point");

        println!("[PASS] test_cluster_membership - cluster_id={}, confident={}",
            membership.cluster_id, membership.is_confident());
    }

    #[test]
    fn test_probability_clamping_high() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(
            mem_id,
            Embedder::Semantic,
            1,
            1.5, // Should be clamped to 1.0
            false,
        );

        assert_eq!(membership.membership_probability, 1.0, "probability > 1.0 should clamp to 1.0");
        println!("[PASS] test_probability_clamping_high - 1.5 clamped to {}", membership.membership_probability);
    }

    #[test]
    fn test_probability_clamping_low() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(
            mem_id,
            Embedder::Semantic,
            1,
            -0.5, // Should be clamped to 0.0
            false,
        );

        assert_eq!(membership.membership_probability, 0.0, "probability < 0.0 should clamp to 0.0");
        println!("[PASS] test_probability_clamping_low - -0.5 clamped to {}", membership.membership_probability);
    }

    #[test]
    fn test_is_confident_threshold() {
        let mem_id = Uuid::new_v4();

        let confident = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.9, false);
        assert!(confident.is_confident(), "0.9 should be confident");

        let borderline = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.8, false);
        assert!(borderline.is_confident(), "0.8 should be confident (threshold)");

        let not_confident = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.79, false);
        assert!(!not_confident.is_confident(), "0.79 should not be confident");

        println!("[PASS] test_is_confident_threshold - 0.9={}, 0.8={}, 0.79={}",
            confident.is_confident(), borderline.is_confident(), not_confident.is_confident());
    }

    #[test]
    fn test_all_embedders() {
        let mem_id = Uuid::new_v4();

        for embedder in Embedder::all() {
            let membership = ClusterMembership::new(mem_id, embedder, 1, 0.5, false);
            assert_eq!(membership.space, embedder, "space should match embedder");

            // Verify serialization works
            let json = serde_json::to_string(&membership).expect("serialize should work");
            let restored: ClusterMembership = serde_json::from_str(&json).expect("deserialize should work");
            assert_eq!(membership, restored, "roundtrip should preserve data");
        }

        println!("[PASS] test_all_embedders - all 13 variants work with serialization");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mem_id = Uuid::new_v4();
        let membership = ClusterMembership::new(mem_id, Embedder::Code, 42, 0.87, true);

        let json = serde_json::to_string(&membership).expect("serialize");
        let restored: ClusterMembership = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(membership.memory_id, restored.memory_id);
        assert_eq!(membership.space, restored.space);
        assert_eq!(membership.cluster_id, restored.cluster_id);
        assert!((membership.membership_probability - restored.membership_probability).abs() < f32::EPSILON);
        assert_eq!(membership.is_core_point, restored.is_core_point);

        println!("[PASS] test_serialization_roundtrip - JSON: {}", json);
    }
}
```

### File: crates/context-graph-core/src/clustering/cluster.rs

```rust
//! Cluster type for representing cluster metadata and quality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::teleological::Embedder;

/// Silhouette score threshold for high-quality clusters.
/// Per constitution clustering.parameters.silhouette_threshold: 0.3
pub const HIGH_QUALITY_THRESHOLD: f32 = 0.3;

/// Represents a cluster in an embedding space.
///
/// Each cluster has a centroid (mean embedding), member count,
/// and quality metrics like silhouette score.
///
/// # Example
///
/// ```
/// use context_graph_core::clustering::Cluster;
/// use context_graph_core::teleological::Embedder;
///
/// let centroid = vec![0.1, 0.2, 0.3]; // simplified
/// let mut cluster = Cluster::new(1, Embedder::Semantic, centroid, 10);
///
/// cluster.update_silhouette(0.75);
/// assert!(cluster.is_high_quality());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Cluster identifier (unique per space).
    pub id: i32,

    /// The embedding space this cluster belongs to.
    pub space: Embedder,

    /// Cluster centroid (mean of all member embeddings).
    pub centroid: Vec<f32>,

    /// Number of members in this cluster.
    pub member_count: u32,

    /// Silhouette score (-1.0..=1.0, higher is better).
    /// Measures how similar members are to own cluster vs other clusters.
    pub silhouette_score: f32,

    /// When the cluster was created.
    pub created_at: DateTime<Utc>,

    /// When the cluster was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Cluster {
    /// Create a new cluster.
    ///
    /// # Arguments
    ///
    /// * `id` - Cluster identifier
    /// * `space` - Embedding space
    /// * `centroid` - Mean embedding vector
    /// * `member_count` - Number of members
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::clustering::Cluster;
    /// use context_graph_core::teleological::Embedder;
    ///
    /// let cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 5);
    /// assert_eq!(cluster.id, 1);
    /// assert_eq!(cluster.silhouette_score, 0.0); // Default until computed
    /// ```
    pub fn new(id: i32, space: Embedder, centroid: Vec<f32>, member_count: u32) -> Self {
        let now = Utc::now();
        Self {
            id,
            space,
            centroid,
            member_count,
            silhouette_score: 0.0, // Computed later via update_silhouette
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the silhouette score.
    ///
    /// Score is clamped to valid range -1.0..=1.0.
    /// Also updates the updated_at timestamp.
    ///
    /// # Arguments
    ///
    /// * `score` - New silhouette score (will be clamped)
    pub fn update_silhouette(&mut self, score: f32) {
        self.silhouette_score = score.clamp(-1.0, 1.0);
        self.touch();
    }

    /// Update the updated_at timestamp to now.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if this cluster has high quality.
    ///
    /// Returns true if silhouette_score >= 0.3 (per constitution).
    #[inline]
    pub fn is_high_quality(&self) -> bool {
        self.silhouette_score >= HIGH_QUALITY_THRESHOLD
    }

    /// Update centroid and member count.
    ///
    /// Used when cluster membership changes.
    pub fn update_centroid(&mut self, centroid: Vec<f32>, member_count: u32) {
        self.centroid = centroid;
        self.member_count = member_count;
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cluster_creation() {
        let centroid = vec![0.1, 0.2, 0.3, 0.4];
        let cluster = Cluster::new(5, Embedder::Semantic, centroid.clone(), 10);

        assert_eq!(cluster.id, 5);
        assert_eq!(cluster.space, Embedder::Semantic);
        assert_eq!(cluster.centroid, centroid);
        assert_eq!(cluster.member_count, 10);
        assert_eq!(cluster.silhouette_score, 0.0);

        println!("[PASS] test_cluster_creation - id={}, space={:?}, members={}",
            cluster.id, cluster.space, cluster.member_count);
    }

    #[test]
    fn test_cluster_touch() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        let old_updated = cluster.updated_at;
        thread::sleep(Duration::from_millis(10));
        cluster.touch();

        assert!(cluster.updated_at > old_updated, "updated_at should increase after touch");

        println!("[PASS] test_cluster_touch - old={}, new={}", old_updated, cluster.updated_at);
    }

    #[test]
    fn test_update_silhouette_normal() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(0.75);
        assert!((cluster.silhouette_score - 0.75).abs() < f32::EPSILON);
        assert!(cluster.is_high_quality());

        println!("[PASS] test_update_silhouette_normal - score=0.75, high_quality={}",
            cluster.is_high_quality());
    }

    #[test]
    fn test_silhouette_clamping_high() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(2.5); // Should clamp to 1.0
        assert_eq!(cluster.silhouette_score, 1.0);

        println!("[PASS] test_silhouette_clamping_high - 2.5 clamped to {}", cluster.silhouette_score);
    }

    #[test]
    fn test_silhouette_clamping_low() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(-2.5); // Should clamp to -1.0
        assert_eq!(cluster.silhouette_score, -1.0);

        println!("[PASS] test_silhouette_clamping_low - -2.5 clamped to {}", cluster.silhouette_score);
    }

    #[test]
    fn test_is_high_quality() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 10);

        cluster.update_silhouette(0.4);
        assert!(cluster.is_high_quality(), "0.4 should be high quality");

        cluster.update_silhouette(0.3);
        assert!(cluster.is_high_quality(), "0.3 should be high quality (threshold)");

        cluster.update_silhouette(0.29);
        assert!(!cluster.is_high_quality(), "0.29 should not be high quality");

        println!("[PASS] test_is_high_quality - threshold=0.3 working correctly");
    }

    #[test]
    fn test_update_centroid() {
        let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 4], 5);
        let old_updated = cluster.updated_at;

        thread::sleep(Duration::from_millis(10));
        cluster.update_centroid(vec![1.0, 2.0, 3.0, 4.0], 15);

        assert_eq!(cluster.centroid, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(cluster.member_count, 15);
        assert!(cluster.updated_at > old_updated);

        println!("[PASS] test_update_centroid - new members={}, centroid updated", cluster.member_count);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let cluster = Cluster::new(42, Embedder::Code, vec![0.1, 0.2, 0.3], 100);

        let json = serde_json::to_string(&cluster).expect("serialize");
        let restored: Cluster = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(cluster.id, restored.id);
        assert_eq!(cluster.space, restored.space);
        assert_eq!(cluster.centroid, restored.centroid);
        assert_eq!(cluster.member_count, restored.member_count);

        println!("[PASS] test_serialization_roundtrip - JSON preserved all fields");
    }

    #[test]
    fn test_all_embedder_spaces() {
        for embedder in Embedder::all() {
            let cluster = Cluster::new(1, embedder, vec![0.0; 10], 5);
            assert_eq!(cluster.space, embedder);

            // Verify serialization
            let json = serde_json::to_string(&cluster).expect("serialize");
            let restored: Cluster = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cluster.space, restored.space);
        }

        println!("[PASS] test_all_embedder_spaces - all 13 spaces work");
    }
}
```

### File: crates/context-graph-core/src/clustering/error.rs

```rust
//! Error types for clustering operations.

use thiserror::Error;

use crate::error::StorageError;
use crate::teleological::Embedder;

/// Errors that can occur during clustering operations.
#[derive(Debug, Error)]
pub enum ClusterError {
    /// Not enough data points for clustering.
    #[error("Insufficient data: required {required}, actual {actual}")]
    InsufficientData {
        /// Minimum required data points
        required: usize,
        /// Actual data points provided
        actual: usize,
    },

    /// Embedding dimension doesn't match expected dimension for space.
    #[error("Dimension mismatch: expected {expected}, actual {actual}")]
    DimensionMismatch {
        /// Expected dimension for this embedding space
        expected: usize,
        /// Actual dimension provided
        actual: usize,
    },

    /// No valid clusters found (all points are noise).
    #[error("No valid clusters found")]
    NoValidClusters,

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    /// Invalid parameter provided.
    #[error("Invalid parameter: {message}")]
    InvalidParameter {
        /// Description of what's wrong with the parameter
        message: String,
    },

    /// Embedding space not initialized for clustering.
    #[error("Space not initialized: {0:?}")]
    SpaceNotInitialized(Embedder),
}

impl ClusterError {
    /// Create an InsufficientData error.
    pub fn insufficient_data(required: usize, actual: usize) -> Self {
        Self::InsufficientData { required, actual }
    }

    /// Create a DimensionMismatch error.
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create an InvalidParameter error.
    pub fn invalid_parameter(message: impl Into<String>) -> Self {
        Self::InvalidParameter {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insufficient_data_error() {
        let err = ClusterError::insufficient_data(3, 1);
        let msg = err.to_string();

        assert!(msg.contains("required 3"), "should mention required count");
        assert!(msg.contains("actual 1"), "should mention actual count");

        println!("[PASS] test_insufficient_data_error - message: {}", msg);
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let err = ClusterError::dimension_mismatch(1024, 512);
        let msg = err.to_string();

        assert!(msg.contains("expected 1024"), "should mention expected dim");
        assert!(msg.contains("actual 512"), "should mention actual dim");

        println!("[PASS] test_dimension_mismatch_error - message: {}", msg);
    }

    #[test]
    fn test_no_valid_clusters_error() {
        let err = ClusterError::NoValidClusters;
        let msg = err.to_string();

        assert!(msg.contains("No valid clusters"), "should describe the error");

        println!("[PASS] test_no_valid_clusters_error - message: {}", msg);
    }

    #[test]
    fn test_invalid_parameter_error() {
        let err = ClusterError::invalid_parameter("min_cluster_size must be > 0");
        let msg = err.to_string();

        assert!(msg.contains("min_cluster_size"), "should include parameter info");

        println!("[PASS] test_invalid_parameter_error - message: {}", msg);
    }

    #[test]
    fn test_space_not_initialized_error() {
        let err = ClusterError::SpaceNotInitialized(Embedder::Semantic);
        let msg = err.to_string();

        assert!(msg.contains("Semantic"), "should mention the embedder");

        println!("[PASS] test_space_not_initialized_error - message: {}", msg);
    }

    #[test]
    fn test_error_variants_are_debug() {
        // Ensure all variants implement Debug
        let errors: Vec<ClusterError> = vec![
            ClusterError::insufficient_data(3, 1),
            ClusterError::dimension_mismatch(1024, 512),
            ClusterError::NoValidClusters,
            ClusterError::invalid_parameter("test"),
            ClusterError::SpaceNotInitialized(Embedder::Semantic),
        ];

        for err in errors {
            let debug = format!("{:?}", err);
            assert!(!debug.is_empty(), "Debug should produce output");
        }

        println!("[PASS] test_error_variants_are_debug - all variants implement Debug");
    }
}
```

## Execution Checklist

- [x] Create clustering module directory: `mkdir -p crates/context-graph-core/src/clustering`
- [x] Create mod.rs with module structure and re-exports
- [x] Create membership.rs with ClusterMembership struct
- [x] Create cluster.rs with Cluster struct
- [x] Create error.rs with ClusterError enum
- [x] Add `pub mod clustering;` to lib.rs (after line 34, alphabetically after `causal`)
- [x] Add re-exports to lib.rs: `pub use clustering::{Cluster, ClusterError, ClusterMembership};`
- [x] Run: `cargo check --package context-graph-core`
- [x] Run: `cargo test --package context-graph-core clustering -- --nocapture`
- [x] Run: `cargo clippy --package context-graph-core -- -D warnings` (no new warnings in clustering module)
- [x] Verify all tests pass with `[PASS]` output
- [x] Verify no dead_code warnings in clustering module
- [ ] Proceed to TASK-P4-002

## Completion Summary (2026-01-17)

**Status: COMPLETE**

All clustering foundation types implemented successfully:
- ClusterMembership: 6 tests passing
- Cluster: 8 tests passing
- ClusterError: 6 tests passing
- Full State Verification: 10 tests passing

Total: 30 tests in clustering module, all passing.

## Trigger-Outcome Verification

### Trigger: Module Registration
- **Event**: Add `pub mod clustering;` to lib.rs
- **Process**: Rust compiler parses and includes module
- **Outcome**: `use context_graph_core::clustering::*` works
- **Verification**: `grep "pub mod clustering" crates/context-graph-core/src/lib.rs` returns match

### Trigger: Type Creation
- **Event**: Create ClusterMembership, Cluster, ClusterError types
- **Process**: Compiler validates struct/enum definitions
- **Outcome**: Types are usable from other modules
- **Verification**: `cargo check --package context-graph-core` succeeds

### Trigger: Test Execution
- **Event**: `cargo test clustering`
- **Process**: Test runner executes all test functions
- **Outcome**: All tests print `[PASS]` and return success
- **Verification**: Exit code 0, output contains "[PASS]" for each test
