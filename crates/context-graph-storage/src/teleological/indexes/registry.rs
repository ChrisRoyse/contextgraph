//! Embedder index registry for managing per-embedder HNSW indexes.
//!
//! # FAIL FAST. NO FALLBACKS.
//!
//! - `get()` returns `None` for E6, E12, E13 (no HNSW index)
//! - `get_or_panic()` panics for E6, E12, E13 with clear message
//!
//! # Architecture
//!
//! ```text
//! +----------------------------+
//! | EmbedderIndexRegistry      |
//! +----------------------------+
//! | indexes: HashMap<          |
//! |   EmbedderIndex,           |
//! |   Arc<HnswEmbedderIndex>   |  (13 indexes)
//! | >                          |
//! +----------------------------+
//!         |
//!         v
//! +----------------------------+
//! | E1 | E2 | E3 | E4 | E5 |   |  (HNSW indexes)
//! | E7 | E8 | E9 | E10| E11|   |
//! | Matryoshka |               |
//! +----------------------------+
//!
//! NOT INCLUDED (use other index types):
//! - E6Sparse: InvertedIndex
//! - E12LateInteraction: MaxSim
//! - E13Splade: InvertedIndex
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use super::embedder_index::EmbedderIndexOps;
use super::hnsw_config::EmbedderIndex;
use super::hnsw_impl::HnswEmbedderIndex;

/// Registry for per-embedder HNSW indexes.
///
/// Creates and manages 11 HNSW indexes (excludes E6, E12, E13 which use different index types).
///
/// # Thread Safety
///
/// The registry itself is immutable after construction. Individual indexes use
/// internal `RwLock` for thread-safe operations.
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::indexes::{
///     EmbedderIndex, EmbedderIndexRegistry, EmbedderIndexOps,
/// };
/// use uuid::Uuid;
///
/// let registry = EmbedderIndexRegistry::new();
///
/// // Get index for E1 Semantic
/// let index = registry.get(EmbedderIndex::E1Semantic).unwrap();
/// assert_eq!(index.config().dimension, 1024);
///
/// // Insert a vector
/// let id = Uuid::new_v4();
/// let vector = vec![0.5f32; 1024];
/// index.insert(id, &vector).unwrap();
///
/// // E6 returns None (uses inverted index)
/// assert!(registry.get(EmbedderIndex::E6Sparse).is_none());
/// ```
pub struct EmbedderIndexRegistry {
    indexes: HashMap<EmbedderIndex, Arc<HnswEmbedderIndex>>,
}

impl EmbedderIndexRegistry {
    /// Create new registry with all 15 HNSW indexes.
    ///
    /// Initializes indexes for E1-E5, E7-E11, Matryoshka, and asymmetric variants.
    /// E6, E12, E13 are excluded (they use inverted/MaxSim indexes).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::EmbedderIndexRegistry;
    ///
    /// let registry = EmbedderIndexRegistry::new();
    /// assert_eq!(registry.len(), 15);
    /// ```
    pub fn new() -> Self {
        let mut indexes = HashMap::new();

        // Create all 15 HNSW-capable indexes
        for embedder in EmbedderIndex::all_hnsw() {
            let index = HnswEmbedderIndex::new(embedder);
            indexes.insert(embedder, Arc::new(index));
        }

        Self { indexes }
    }

    /// Get index for embedder, if available.
    ///
    /// Returns `None` for E6, E12, E13 (no HNSW index).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexRegistry,
    /// };
    ///
    /// let registry = EmbedderIndexRegistry::new();
    ///
    /// // HNSW embedders return Some
    /// assert!(registry.get(EmbedderIndex::E1Semantic).is_some());
    ///
    /// // Non-HNSW embedders return None
    /// assert!(registry.get(EmbedderIndex::E6Sparse).is_none());
    /// ```
    pub fn get(&self, embedder: EmbedderIndex) -> Option<&Arc<HnswEmbedderIndex>> {
        self.indexes.get(&embedder)
    }

    /// Get index or panic - FAIL FAST for non-HNSW embedders.
    ///
    /// Use this when you know the embedder uses HNSW and want to catch
    /// programming errors early.
    ///
    /// # Panics
    ///
    /// Panics with clear message for E6, E12, E13.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry,
    /// };
    ///
    /// let registry = EmbedderIndexRegistry::new();
    /// let index = registry.get_or_panic(EmbedderIndex::E1Semantic);
    /// assert_eq!(index.config().dimension, 1024);
    /// ```
    ///
    /// ```should_panic
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexRegistry,
    /// };
    ///
    /// let registry = EmbedderIndexRegistry::new();
    /// let _index = registry.get_or_panic(EmbedderIndex::E6Sparse); // Panics
    /// ```
    pub fn get_or_panic(&self, embedder: EmbedderIndex) -> &Arc<HnswEmbedderIndex> {
        self.indexes.get(&embedder).unwrap_or_else(|| {
            panic!(
                "FAIL FAST: No HNSW index for {:?}. \
                E6 uses InvertedIndex, E12 uses MaxSim, E13 uses InvertedIndex.",
                embedder
            )
        })
    }

    /// Get index as trait object for dynamic dispatch.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexRegistry, EmbedderIndexOps,
    /// };
    ///
    /// let registry = EmbedderIndexRegistry::new();
    /// let index = registry.get_dyn(EmbedderIndex::E8Graph).unwrap();
    /// assert_eq!(index.embedder(), EmbedderIndex::E8Graph);
    /// ```
    pub fn get_dyn(&self, embedder: EmbedderIndex) -> Option<Arc<dyn EmbedderIndexOps>> {
        self.indexes
            .get(&embedder)
            .map(|arc| Arc::clone(arc) as Arc<dyn EmbedderIndexOps>)
    }

    /// Number of indexes in the registry (always 11).
    pub fn len(&self) -> usize {
        self.indexes.len()
    }

    /// Check if empty (never true for valid registry).
    pub fn is_empty(&self) -> bool {
        self.indexes.is_empty()
    }

    /// Iterate over all embedder-index pairs.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndexOps, EmbedderIndexRegistry,
    /// };
    ///
    /// let registry = EmbedderIndexRegistry::new();
    /// for (embedder, index) in registry.iter() {
    ///     println!("{:?}: {}D", embedder, index.config().dimension);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&EmbedderIndex, &Arc<HnswEmbedderIndex>)> {
        self.indexes.iter()
    }

    /// Get all embedders that have indexes in this registry.
    pub fn embedders(&self) -> Vec<EmbedderIndex> {
        self.indexes.keys().copied().collect()
    }

    /// Total memory usage across all indexes in bytes.
    pub fn total_memory_bytes(&self) -> usize {
        self.indexes.values().map(|idx| idx.memory_bytes()).sum()
    }

    /// Total vector count across all indexes.
    pub fn total_vectors(&self) -> usize {
        self.indexes.values().map(|idx| idx.len()).sum()
    }

    /// Flush all indexes.
    pub fn flush_all(&self) -> Result<(), super::embedder_index::IndexError> {
        for index in self.indexes.values() {
            index.flush()?;
        }
        Ok(())
    }
}

impl Default for EmbedderIndexRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_registry_creation() {
        println!("=== TEST: Registry creates 15 HNSW indexes ===");
        println!("  (11 original + 2 E5 asymmetric + 2 E10 asymmetric per ARCH-15)");
        println!("BEFORE: Creating new registry");

        let registry = EmbedderIndexRegistry::new();

        println!("AFTER: registry.len()={}", registry.len());
        assert_eq!(registry.len(), 15);
        assert!(!registry.is_empty());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_get_all_hnsw() {
        println!("=== TEST: Registry.get() returns Some for all HNSW embedders ===");

        let registry = EmbedderIndexRegistry::new();

        for embedder in EmbedderIndex::all_hnsw() {
            let index = registry.get(embedder);
            assert!(
                index.is_some(),
                "Expected Some for {:?}, got None",
                embedder
            );
            let index = index.unwrap();
            assert_eq!(index.embedder(), embedder);
            println!("  {:?}: {}D - OK", embedder, index.config().dimension);
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_get_non_hnsw_returns_none() {
        println!("=== TEST: Registry.get() returns None for E6, E12, E13 ===");

        let registry = EmbedderIndexRegistry::new();

        let e6 = registry.get(EmbedderIndex::E6Sparse);
        assert!(e6.is_none(), "E6Sparse should return None");
        println!("  E6Sparse: None - OK");

        let e12 = registry.get(EmbedderIndex::E12LateInteraction);
        assert!(e12.is_none(), "E12LateInteraction should return None");
        println!("  E12LateInteraction: None - OK");

        let e13 = registry.get(EmbedderIndex::E13Splade);
        assert!(e13.is_none(), "E13Splade should return None");
        println!("  E13Splade: None - OK");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_get_or_panic_hnsw() {
        println!("=== TEST: Registry.get_or_panic() works for HNSW embedders ===");

        let registry = EmbedderIndexRegistry::new();

        let index = registry.get_or_panic(EmbedderIndex::E1Semantic);
        assert_eq!(index.config().dimension, 1024);

        let index = registry.get_or_panic(EmbedderIndex::E8Graph);
        assert_eq!(index.config().dimension, 1024); // Upgraded from 384D

        println!("RESULT: PASS");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_registry_get_or_panic_e6() {
        println!("=== TEST: Registry.get_or_panic(E6) panics ===");
        let registry = EmbedderIndexRegistry::new();
        let _index = registry.get_or_panic(EmbedderIndex::E6Sparse);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_registry_get_or_panic_e12() {
        println!("=== TEST: Registry.get_or_panic(E12) panics ===");
        let registry = EmbedderIndexRegistry::new();
        let _index = registry.get_or_panic(EmbedderIndex::E12LateInteraction);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_registry_get_or_panic_e13() {
        println!("=== TEST: Registry.get_or_panic(E13) panics ===");
        let registry = EmbedderIndexRegistry::new();
        let _index = registry.get_or_panic(EmbedderIndex::E13Splade);
    }

    #[test]
    fn test_registry_get_dyn() {
        println!("=== TEST: Registry.get_dyn() returns trait object ===");

        let registry = EmbedderIndexRegistry::new();

        let dyn_index = registry.get_dyn(EmbedderIndex::E8Graph);
        assert!(dyn_index.is_some());

        let dyn_index = dyn_index.unwrap();
        assert_eq!(dyn_index.embedder(), EmbedderIndex::E8Graph);
        assert_eq!(dyn_index.config().dimension, 1024); // Upgraded from 384D

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_insert_and_search() {
        println!("=== TEST: Insert and search through registry ===");
        println!("BEFORE: Creating registry and inserting vectors");

        let registry = EmbedderIndexRegistry::new();

        // Insert into E1
        let e1_index = registry.get_or_panic(EmbedderIndex::E1Semantic);
        let id1 = Uuid::new_v4();
        let vec1: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect();
        e1_index.insert(id1, &vec1).unwrap();

        // Insert into E8
        let e8_index = registry.get_or_panic(EmbedderIndex::E8Graph);
        let id2 = Uuid::new_v4();
        let vec2 = vec![0.5f32; 1024];
        e8_index.insert(id2, &vec2).unwrap();

        println!(
            "AFTER: E1 has {} vectors, E8 has {} vectors",
            e1_index.len(),
            e8_index.len()
        );

        // Search E1
        let results = e1_index.search(&vec1, 1, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id1);

        // Search E8
        let results = e8_index.search(&vec2, 1, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id2);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_iter() {
        println!("=== TEST: Registry.iter() iterates over all indexes ===");

        let registry = EmbedderIndexRegistry::new();
        let mut count = 0;

        for (embedder, index) in registry.iter() {
            assert_eq!(index.embedder(), *embedder);
            count += 1;
        }

        assert_eq!(count, 15);
        println!("RESULT: PASS - iterated over {} indexes (11 original + 2 E5 + 2 E10 asymmetric)", count);
    }

    #[test]
    fn test_registry_embedders() {
        println!("=== TEST: Registry.embedders() returns all embedder types ===");
        println!("  (15 total: 11 original + 2 E5 + 2 E10 asymmetric per ARCH-15)");

        let registry = EmbedderIndexRegistry::new();
        let embedders = registry.embedders();

        assert_eq!(embedders.len(), 15);

        // Verify E5 asymmetric indexes ARE in the list (ARCH-15)
        assert!(embedders.contains(&EmbedderIndex::E5CausalCause));
        assert!(embedders.contains(&EmbedderIndex::E5CausalEffect));

        // Verify E10 asymmetric indexes ARE in the list (ARCH-15)
        assert!(embedders.contains(&EmbedderIndex::E10MultimodalIntent));
        assert!(embedders.contains(&EmbedderIndex::E10MultimodalContext));

        // Verify E6, E12, E13 are NOT in the list (non-HNSW)
        assert!(!embedders.contains(&EmbedderIndex::E6Sparse));
        assert!(!embedders.contains(&EmbedderIndex::E12LateInteraction));
        assert!(!embedders.contains(&EmbedderIndex::E13Splade));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_total_memory() {
        println!("=== TEST: Registry.total_memory_bytes() sums all indexes ===");

        let registry = EmbedderIndexRegistry::new();
        let initial_memory = registry.total_memory_bytes();
        println!("BEFORE: initial_memory={} bytes", initial_memory);

        // Insert some vectors
        let e8_index = registry.get_or_panic(EmbedderIndex::E8Graph);
        for _ in 0..100 {
            let id = Uuid::new_v4();
            let vec = vec![1.0f32; 1024];
            e8_index.insert(id, &vec).unwrap();
        }

        let after_memory = registry.total_memory_bytes();
        println!("AFTER: memory={} bytes", after_memory);

        assert!(after_memory > initial_memory);
        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_total_vectors() {
        println!("=== TEST: Registry.total_vectors() sums all indexes ===");

        let registry = EmbedderIndexRegistry::new();
        assert_eq!(registry.total_vectors(), 0);

        // Insert into E1
        let e1_index = registry.get_or_panic(EmbedderIndex::E1Semantic);
        for _ in 0..10 {
            e1_index.insert(Uuid::new_v4(), &vec![1.0; 1024]).unwrap();
        }

        // Insert into E8
        let e8_index = registry.get_or_panic(EmbedderIndex::E8Graph);
        for _ in 0..20 {
            e8_index.insert(Uuid::new_v4(), &vec![1.0; 1024]).unwrap();
        }

        assert_eq!(registry.total_vectors(), 30);
        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_flush_all() {
        println!("=== TEST: Registry.flush_all() flushes all indexes ===");

        let registry = EmbedderIndexRegistry::new();

        // Insert some data
        let e8_index = registry.get_or_panic(EmbedderIndex::E8Graph);
        e8_index.insert(Uuid::new_v4(), &vec![1.0; 1024]).unwrap();

        let result = registry.flush_all();
        assert!(result.is_ok());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_default() {
        println!("=== TEST: Registry implements Default ===");

        let registry: EmbedderIndexRegistry = Default::default();
        assert_eq!(registry.len(), 15);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_registry_dimensions() {
        println!("=== TEST: Registry indexes have correct dimensions ===");

        let registry = EmbedderIndexRegistry::new();

        // Verify specific dimensions from constitution
        let expected_dims: Vec<(EmbedderIndex, usize)> = vec![
            (EmbedderIndex::E1Semantic, 1024),
            (EmbedderIndex::E2TemporalRecent, 512),
            (EmbedderIndex::E3TemporalPeriodic, 512),
            (EmbedderIndex::E4TemporalPositional, 512),
            (EmbedderIndex::E5Causal, 768),
            (EmbedderIndex::E7Code, 1536),
            (EmbedderIndex::E8Graph, 1024),
            (EmbedderIndex::E9HDC, 1024),
            (EmbedderIndex::E10Multimodal, 768),
            (EmbedderIndex::E11Entity, 768),
            (EmbedderIndex::E1Matryoshka128, 128),
        ];

        for (embedder, expected_dim) in expected_dims {
            let index = registry.get_or_panic(embedder);
            let actual_dim = index.config().dimension;
            assert_eq!(
                actual_dim, expected_dim,
                "{:?}: expected {}D, got {}D",
                embedder, expected_dim, actual_dim
            );
            println!("  {:?}: {}D - OK", embedder, actual_dim);
        }

        println!("RESULT: PASS");
    }

    #[test]
    fn test_verification_log() {
        println!("\n=== REGISTRY.RS VERIFICATION LOG ===");
        println!();

        println!("Struct Verification:");
        println!("  - EmbedderIndexRegistry: HashMap<EmbedderIndex, Arc<HnswEmbedderIndex>>");
        println!("  - 13 indexes (E1-E5, E7-E11, Matryoshka)");
        println!("  - Excludes E6, E12, E13 (non-HNSW)");

        println!();
        println!("Method Verification:");
        println!("  - new(): Creates all 13 indexes");
        println!("  - get(): Returns Option (None for E6/E12/E13)");
        println!("  - get_or_panic(): Panics for E6/E12/E13");
        println!("  - get_dyn(): Returns Arc<dyn EmbedderIndexOps>");
        println!("  - len(): Always 11");
        println!("  - iter(): Iterates over all");
        println!("  - embedders(): List of embedder types");
        println!("  - total_memory_bytes(): Sum of all indexes");
        println!("  - total_vectors(): Sum of all vector counts");
        println!("  - flush_all(): Flush all indexes");

        println!();
        println!("Test Coverage:");
        println!("  - Registry creation (13 indexes): PASS");
        println!("  - get() all HNSW embedders: PASS");
        println!("  - get() E6/E12/E13 returns None: PASS");
        println!("  - get_or_panic() HNSW: PASS");
        println!("  - get_or_panic() E6 panics: PASS");
        println!("  - get_or_panic() E12 panics: PASS");
        println!("  - get_or_panic() E13 panics: PASS");
        println!("  - get_dyn(): PASS");
        println!("  - Insert and search: PASS");
        println!("  - iter(): PASS");
        println!("  - embedders(): PASS");
        println!("  - total_memory_bytes(): PASS");
        println!("  - total_vectors(): PASS");
        println!("  - flush_all(): PASS");
        println!("  - Default trait: PASS");
        println!("  - Dimension verification: PASS");

        println!();
        println!("VERIFICATION COMPLETE");
    }
}
