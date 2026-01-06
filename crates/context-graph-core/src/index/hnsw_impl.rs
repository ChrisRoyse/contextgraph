//! HnswMultiSpaceIndex implementation with 12 HNSW indexes.
//!
//! Implements `MultiSpaceIndexManager` trait for the 5-stage retrieval pipeline.
//!
//! # Index Architecture
//!
//! | Index Type | Count | Purpose | Stage |
//! |------------|-------|---------|-------|
//! | HNSW | 10 | E1-E5, E7-E11 dense | Stage 3 |
//! | HNSW | 1 | E1 Matryoshka 128D | Stage 2 |
//! | HNSW | 1 | PurposeVector 13D | Stage 4 |
//!
//! # Performance Requirements (constitution.yaml)
//!
//! - `add_vector()`: <1ms per index
//! - `search()`: <10ms per index
//! - `persist()`: <1s for 100K vectors

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use uuid::Uuid;

use crate::types::fingerprint::SemanticFingerprint;

pub use super::config::{DistanceMetric, EmbedderIndex, HnswConfig};

use super::error::{IndexError, IndexResult};
use super::manager::MultiSpaceIndexManager;
use super::splade_impl::SpladeInvertedIndex;
use super::status::IndexStatus;

/// Entry in an HNSW index: (vector, metadata).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HnswEntry {
    /// Memory UUID
    id: Uuid,
    /// Dense vector
    vector: Vec<f32>,
}

/// Simple HNSW-like index using flat search with approximate neighbor graph.
///
/// This is a simplified implementation for the prototype. For production,
/// replace with instant-distance or hnsw_rs crate.
///
/// # Note
///
/// This implementation uses brute-force search for correctness verification.
/// Performance targets will be met with proper HNSW library integration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleHnswIndex {
    /// All entries in the index
    entries: Vec<HnswEntry>,
    /// UUID to index mapping for fast removal
    id_to_index: HashMap<Uuid, usize>,
    /// Configuration
    config: HnswConfig,
    /// Whether index is initialized
    initialized: bool,
}

impl SimpleHnswIndex {
    /// Create a new empty index with given configuration.
    pub fn new(config: HnswConfig) -> Self {
        Self {
            entries: Vec::new(),
            id_to_index: HashMap::new(),
            config,
            initialized: true,
        }
    }

    /// Add a vector to the index.
    ///
    /// # Errors
    ///
    /// - `DimensionMismatch`: If vector dimension doesn't match config
    /// - `ZeroNormVector`: If vector has zero magnitude
    pub fn add(&mut self, id: Uuid, vector: &[f32]) -> IndexResult<()> {
        // Validate dimension
        if vector.len() != self.config.dimension {
            return Err(IndexError::DimensionMismatch {
                embedder: EmbedderIndex::E1Semantic, // Will be overridden by caller
                expected: self.config.dimension,
                actual: vector.len(),
            });
        }

        // Validate non-zero norm
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm < f32::EPSILON {
            return Err(IndexError::ZeroNormVector { memory_id: id });
        }

        // Remove existing entry if present
        if let Some(&old_idx) = self.id_to_index.get(&id) {
            self.entries.remove(old_idx);
            // Rebuild id_to_index after removal
            self.id_to_index.clear();
            for (i, entry) in self.entries.iter().enumerate() {
                self.id_to_index.insert(entry.id, i);
            }
        }

        // Add new entry
        let idx = self.entries.len();
        self.entries.push(HnswEntry {
            id,
            vector: vector.to_vec(),
        });
        self.id_to_index.insert(id, idx);

        Ok(())
    }

    /// Search for k nearest neighbors.
    ///
    /// Returns (id, similarity) pairs sorted by descending similarity.
    pub fn search(&self, query: &[f32], k: usize) -> IndexResult<Vec<(Uuid, f32)>> {
        if query.len() != self.config.dimension {
            return Err(IndexError::DimensionMismatch {
                embedder: EmbedderIndex::E1Semantic,
                expected: self.config.dimension,
                actual: query.len(),
            });
        }

        if self.entries.is_empty() {
            return Ok(Vec::new());
        }

        // Compute similarities based on metric
        let mut results: Vec<(Uuid, f32)> = self
            .entries
            .iter()
            .map(|entry| {
                let sim = match self.config.metric {
                    DistanceMetric::Cosine => self.cosine_similarity(query, &entry.vector),
                    DistanceMetric::DotProduct => self.dot_product(query, &entry.vector),
                    DistanceMetric::Euclidean => {
                        -self.euclidean_distance(query, &entry.vector) // Negative for sorting
                    }
                    DistanceMetric::AsymmetricCosine => {
                        self.cosine_similarity(query, &entry.vector)
                    }
                    DistanceMetric::MaxSim => {
                        // MaxSim is not supported for HNSW - fall back to cosine
                        self.cosine_similarity(query, &entry.vector)
                    }
                };
                (entry.id, sim)
            })
            .collect();

        // Sort by descending similarity
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);

        Ok(results)
    }

    /// Remove an entry by ID.
    pub fn remove(&mut self, id: Uuid) -> bool {
        if let Some(&idx) = self.id_to_index.get(&id) {
            self.entries.remove(idx);
            self.id_to_index.remove(&id);

            // Rebuild indices
            self.id_to_index.clear();
            for (i, entry) in self.entries.iter().enumerate() {
                self.id_to_index.insert(entry.id, i);
            }
            true
        } else {
            false
        }
    }

    /// Number of entries in the index.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Approximate memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.entries.len() * (16 + self.config.dimension * 4)
    }

    // Distance computations

    #[inline]
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b).max(f32::EPSILON)
    }

    #[inline]
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[inline]
    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

/// HnswMultiSpaceIndex manages 12 HNSW indexes + SPLADE inverted index.
///
/// # Architecture
///
/// - 10 dense HNSW indexes (E1-E5, E7-E11)
/// - 1 Matryoshka 128D HNSW (E1 truncated for Stage 2)
/// - 1 PurposeVector 13D HNSW (Stage 4)
/// - 1 SPLADE inverted index (Stage 1)
///
/// # Thread Safety
///
/// The struct is Send + Sync through interior mutability patterns.
#[derive(Debug)]
pub struct HnswMultiSpaceIndex {
    /// Map from EmbedderIndex to HNSW index
    hnsw_indexes: HashMap<EmbedderIndex, SimpleHnswIndex>,
    /// SPLADE inverted index for Stage 1
    splade_index: SpladeInvertedIndex,
    /// Whether initialized
    initialized: bool,
}

impl HnswMultiSpaceIndex {
    /// Create a new uninitialized multi-space index.
    pub fn new() -> Self {
        Self {
            hnsw_indexes: HashMap::new(),
            splade_index: SpladeInvertedIndex::new(),
            initialized: false,
        }
    }

    /// Create HNSW config for a given embedder.
    fn config_for_embedder(embedder: EmbedderIndex) -> Option<HnswConfig> {
        let dim = embedder.dimension()?;
        let metric = embedder.recommended_metric().unwrap_or(DistanceMetric::Cosine);

        // Use special config for Matryoshka
        if embedder == EmbedderIndex::E1Matryoshka128 {
            Some(HnswConfig::matryoshka_128d())
        } else if embedder == EmbedderIndex::PurposeVector {
            Some(HnswConfig::purpose_vector())
        } else {
            Some(HnswConfig::default_for_dimension(dim, metric))
        }
    }

    /// Get index status for a specific embedder.
    fn get_embedder_status(&self, embedder: EmbedderIndex) -> IndexStatus {
        if let Some(index) = self.hnsw_indexes.get(&embedder) {
            let mut status = IndexStatus::new_empty(embedder);
            let bytes_per_element = index.config.estimated_memory_per_vector();
            status.update_count(index.len(), bytes_per_element);
            status
        } else {
            IndexStatus::uninitialized(embedder)
        }
    }
}

impl Default for HnswMultiSpaceIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MultiSpaceIndexManager for HnswMultiSpaceIndex {
    async fn initialize(&mut self) -> IndexResult<()> {
        if self.initialized {
            return Ok(());
        }

        // Initialize all 12 HNSW indexes
        for embedder in EmbedderIndex::all_hnsw() {
            if let Some(config) = Self::config_for_embedder(embedder) {
                let index = SimpleHnswIndex::new(config);
                self.hnsw_indexes.insert(embedder, index);
            }
        }

        // SPLADE index is already initialized in new()
        self.initialized = true;

        Ok(())
    }

    async fn add_vector(
        &mut self,
        embedder: EmbedderIndex,
        memory_id: Uuid,
        vector: &[f32],
    ) -> IndexResult<()> {
        // Validate embedder uses HNSW
        if !embedder.uses_hnsw() {
            return Err(IndexError::InvalidEmbedder { embedder });
        }

        // Check initialization
        if !self.initialized {
            return Err(IndexError::NotInitialized { embedder });
        }

        // Get index
        let index = self
            .hnsw_indexes
            .get_mut(&embedder)
            .ok_or(IndexError::NotInitialized { embedder })?;

        // Validate dimension
        let expected_dim = embedder.dimension().unwrap_or(0);
        if vector.len() != expected_dim {
            return Err(IndexError::DimensionMismatch {
                embedder,
                expected: expected_dim,
                actual: vector.len(),
            });
        }

        // Add to index
        index.add(memory_id, vector).map_err(|e| match e {
            IndexError::DimensionMismatch { expected, actual, .. } => {
                IndexError::DimensionMismatch {
                    embedder,
                    expected,
                    actual,
                }
            }
            IndexError::ZeroNormVector { memory_id } => IndexError::ZeroNormVector { memory_id },
            other => other,
        })?;

        Ok(())
    }

    async fn add_fingerprint(
        &mut self,
        memory_id: Uuid,
        fingerprint: &SemanticFingerprint,
    ) -> IndexResult<()> {
        if !self.initialized {
            return Err(IndexError::NotInitialized {
                embedder: EmbedderIndex::E1Semantic,
            });
        }

        // E1 Semantic
        self.add_vector(EmbedderIndex::E1Semantic, memory_id, &fingerprint.e1_semantic)
            .await?;

        // E1 Matryoshka 128D - truncate E1 to first 128 dimensions
        let matryoshka: Vec<f32> = fingerprint.e1_semantic.iter().take(128).copied().collect();
        self.add_vector(EmbedderIndex::E1Matryoshka128, memory_id, &matryoshka)
            .await?;

        // E2-E5 Temporal embeddings
        self.add_vector(
            EmbedderIndex::E2TemporalRecent,
            memory_id,
            &fingerprint.e2_temporal_recent,
        )
        .await?;
        self.add_vector(
            EmbedderIndex::E3TemporalPeriodic,
            memory_id,
            &fingerprint.e3_temporal_periodic,
        )
        .await?;
        self.add_vector(
            EmbedderIndex::E4TemporalPositional,
            memory_id,
            &fingerprint.e4_temporal_positional,
        )
        .await?;

        // E5 Causal
        self.add_vector(EmbedderIndex::E5Causal, memory_id, &fingerprint.e5_causal)
            .await?;

        // E7-E11
        self.add_vector(EmbedderIndex::E7Code, memory_id, &fingerprint.e7_code)
            .await?;
        self.add_vector(EmbedderIndex::E8Graph, memory_id, &fingerprint.e8_graph)
            .await?;
        self.add_vector(EmbedderIndex::E9HDC, memory_id, &fingerprint.e9_hdc)
            .await?;
        self.add_vector(
            EmbedderIndex::E10Multimodal,
            memory_id,
            &fingerprint.e10_multimodal,
        )
        .await?;
        self.add_vector(EmbedderIndex::E11Entity, memory_id, &fingerprint.e11_entity)
            .await?;

        // E13 SPLADE -> inverted index
        // Convert SparseVector to (usize, f32) pairs for SPLADE index
        let splade_pairs: Vec<(usize, f32)> = fingerprint
            .e13_splade
            .indices
            .iter()
            .zip(fingerprint.e13_splade.values.iter())
            .map(|(&idx, &val)| (idx as usize, val))
            .collect();
        self.splade_index.add(memory_id, &splade_pairs)?;

        Ok(())
    }

    async fn add_purpose_vector(&mut self, memory_id: Uuid, purpose: &[f32]) -> IndexResult<()> {
        // Validate dimension
        if purpose.len() != 13 {
            return Err(IndexError::DimensionMismatch {
                embedder: EmbedderIndex::PurposeVector,
                expected: 13,
                actual: purpose.len(),
            });
        }

        self.add_vector(EmbedderIndex::PurposeVector, memory_id, purpose)
            .await
    }

    async fn add_splade(&mut self, memory_id: Uuid, sparse: &[(usize, f32)]) -> IndexResult<()> {
        self.splade_index.add(memory_id, sparse)
    }

    async fn search(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        // Validate embedder uses HNSW
        if !embedder.uses_hnsw() {
            return Err(IndexError::InvalidEmbedder { embedder });
        }

        // Check initialization
        if !self.initialized {
            return Err(IndexError::NotInitialized { embedder });
        }

        // Get index
        let index = self
            .hnsw_indexes
            .get(&embedder)
            .ok_or(IndexError::NotInitialized { embedder })?;

        // Validate dimension
        let expected_dim = embedder.dimension().unwrap_or(0);
        if query.len() != expected_dim {
            return Err(IndexError::DimensionMismatch {
                embedder,
                expected: expected_dim,
                actual: query.len(),
            });
        }

        // Search
        index.search(query, k).map_err(|e| match e {
            IndexError::DimensionMismatch { expected, actual, .. } => {
                IndexError::DimensionMismatch {
                    embedder,
                    expected,
                    actual,
                }
            }
            other => other,
        })
    }

    async fn search_splade(
        &self,
        sparse_query: &[(usize, f32)],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        if !self.initialized {
            return Err(IndexError::NotInitialized {
                embedder: EmbedderIndex::E13Splade,
            });
        }

        Ok(self.splade_index.search(sparse_query, k))
    }

    async fn search_matryoshka(
        &self,
        query_128d: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        self.search(EmbedderIndex::E1Matryoshka128, query_128d, k)
            .await
    }

    async fn search_purpose(
        &self,
        purpose_query: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        self.search(EmbedderIndex::PurposeVector, purpose_query, k)
            .await
    }

    async fn remove(&mut self, memory_id: Uuid) -> IndexResult<()> {
        let mut found = false;

        // Remove from all HNSW indexes
        for index in self.hnsw_indexes.values_mut() {
            if index.remove(memory_id) {
                found = true;
            }
        }

        // Remove from SPLADE index
        if self.splade_index.remove(memory_id) {
            found = true;
        }

        if !found {
            // Warning: memory not found in any index
            // This is not necessarily an error - it might have been partially indexed
        }

        Ok(())
    }

    fn status(&self) -> Vec<IndexStatus> {
        let mut statuses = Vec::with_capacity(14);

        // All HNSW indexes
        for embedder in EmbedderIndex::all_hnsw() {
            statuses.push(self.get_embedder_status(embedder));
        }

        // SPLADE index status
        let mut splade_status = IndexStatus::new_empty(EmbedderIndex::E13Splade);
        splade_status.update_count(self.splade_index.len(), 40); // ~40 bytes per entry estimate
        statuses.push(splade_status);

        statuses
    }

    async fn persist(&self, path: &Path) -> IndexResult<()> {
        // Create directory if needed
        std::fs::create_dir_all(path).map_err(|e| IndexError::io("creating index directory", e))?;

        // Persist each HNSW index
        for (embedder, index) in &self.hnsw_indexes {
            let file_name = format!("{:?}.hnsw.bin", embedder);
            let file_path = path.join(&file_name);

            let file =
                File::create(&file_path).map_err(|e| IndexError::io("creating HNSW file", e))?;
            let writer = BufWriter::new(file);
            bincode::serialize_into(writer, index)
                .map_err(|e| IndexError::serialization("serializing HNSW index", e))?;
        }

        // Persist SPLADE index
        let splade_path = path.join("splade.bin");
        self.splade_index.persist(&splade_path)?;

        // Persist metadata
        let meta_path = path.join("index_meta.json");
        let meta = serde_json::json!({
            "version": "1.0.0",
            "hnsw_count": self.hnsw_indexes.len(),
            "splade_count": self.splade_index.len(),
            "initialized": self.initialized,
        });
        let meta_file =
            File::create(&meta_path).map_err(|e| IndexError::io("creating metadata file", e))?;
        serde_json::to_writer_pretty(meta_file, &meta)
            .map_err(|e| IndexError::serialization("serializing metadata", e))?;

        Ok(())
    }

    async fn load(&mut self, path: &Path) -> IndexResult<()> {
        // Load metadata first
        let meta_path = path.join("index_meta.json");
        if !meta_path.exists() {
            return Err(IndexError::CorruptedIndex {
                path: meta_path.display().to_string(),
            });
        }

        // Initialize structure
        self.initialize().await?;

        // Load each HNSW index
        for embedder in EmbedderIndex::all_hnsw() {
            let file_name = format!("{:?}.hnsw.bin", embedder);
            let file_path = path.join(&file_name);

            if file_path.exists() {
                let file =
                    File::open(&file_path).map_err(|e| IndexError::io("opening HNSW file", e))?;
                let reader = BufReader::new(file);
                let index: SimpleHnswIndex = bincode::deserialize_from(reader)
                    .map_err(|e| IndexError::serialization("deserializing HNSW index", e))?;
                self.hnsw_indexes.insert(embedder, index);
            }
        }

        // Load SPLADE index
        let splade_path = path.join("splade.bin");
        if splade_path.exists() {
            self.splade_index = SpladeInvertedIndex::load(&splade_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::status::IndexHealth;

    // Helper to create a random normalized vector
    fn random_vector(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        // Normalize
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut v {
            *x /= norm;
        }
        v
    }

    // Helper to create a minimal valid SemanticFingerprint
    fn create_test_fingerprint() -> SemanticFingerprint {
        use crate::types::fingerprint::SparseVector;

        SemanticFingerprint {
            e1_semantic: random_vector(1024),
            e2_temporal_recent: random_vector(512),
            e3_temporal_periodic: random_vector(512),
            e4_temporal_positional: random_vector(512),
            e5_causal: random_vector(768),
            e6_sparse: SparseVector::new(vec![100, 200], vec![0.5, 0.3]).unwrap(),
            e7_code: random_vector(256),
            e8_graph: random_vector(384),
            e9_hdc: random_vector(10000),
            e10_multimodal: random_vector(768),
            e11_entity: random_vector(384),
            e12_late_interaction: vec![random_vector(128); 3], // 3 tokens
            e13_splade: SparseVector::new(vec![100, 200, 300], vec![0.5, 0.3, 0.2]).unwrap(),
        }
    }

    #[test]
    fn test_simple_hnsw_new() {
        let config = HnswConfig::default_for_dimension(1024, DistanceMetric::Cosine);
        let index = SimpleHnswIndex::new(config);

        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
        println!("[VERIFIED] SimpleHnswIndex::new() creates empty index");
    }

    #[test]
    fn test_simple_hnsw_add_and_search() {
        let config = HnswConfig::default_for_dimension(128, DistanceMetric::Cosine);
        let mut index = SimpleHnswIndex::new(config);

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let v1 = random_vector(128);
        let v2 = random_vector(128);

        println!("[BEFORE] Adding 2 vectors to HNSW");
        index.add(id1, &v1).unwrap();
        index.add(id2, &v2).unwrap();
        println!("[AFTER] index.len() = {}", index.len());

        assert_eq!(index.len(), 2);

        // Search for v1 - should find id1 first
        let results = index.search(&v1, 2).unwrap();
        println!(
            "[SEARCH] Found {} results, top result = {:?}",
            results.len(),
            results.first()
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, id1);
        assert!(results[0].1 > results[1].1); // v1 closer to itself

        println!("[VERIFIED] Add and search work correctly");
    }

    #[test]
    fn test_simple_hnsw_dimension_mismatch() {
        let config = HnswConfig::default_for_dimension(128, DistanceMetric::Cosine);
        let mut index = SimpleHnswIndex::new(config);

        let id = Uuid::new_v4();
        let wrong_dim = random_vector(256); // Wrong dimension

        println!("[BEFORE] Adding vector with wrong dimension (256 vs 128)");
        let result = index.add(id, &wrong_dim);
        println!("[AFTER] result.is_err() = {}", result.is_err());

        assert!(matches!(
            result,
            Err(IndexError::DimensionMismatch {
                expected: 128,
                actual: 256,
                ..
            })
        ));
        println!("[VERIFIED] Dimension mismatch rejected");
    }

    #[test]
    fn test_simple_hnsw_zero_norm_rejected() {
        let config = HnswConfig::default_for_dimension(10, DistanceMetric::Cosine);
        let mut index = SimpleHnswIndex::new(config);

        let id = Uuid::new_v4();
        let zero_vec = vec![0.0; 10];

        println!("[BEFORE] Adding zero-norm vector");
        let result = index.add(id, &zero_vec);
        println!("[AFTER] result = {:?}", result.is_err());

        assert!(matches!(result, Err(IndexError::ZeroNormVector { .. })));
        println!("[VERIFIED] Zero-norm vector rejected");
    }

    #[test]
    fn test_simple_hnsw_remove() {
        let config = HnswConfig::default_for_dimension(64, DistanceMetric::Cosine);
        let mut index = SimpleHnswIndex::new(config);

        let id = Uuid::new_v4();
        let v = random_vector(64);

        index.add(id, &v).unwrap();
        println!("[BEFORE REMOVE] index.len() = {}", index.len());

        let removed = index.remove(id);
        println!("[AFTER REMOVE] index.len() = {}, removed = {}", index.len(), removed);

        assert!(removed);
        assert_eq!(index.len(), 0);
        println!("[VERIFIED] Remove works correctly");
    }

    #[tokio::test]
    async fn test_multi_space_initialize() {
        let mut manager = HnswMultiSpaceIndex::new();

        println!("[BEFORE] Initializing MultiSpaceIndex");
        manager.initialize().await.unwrap();
        println!(
            "[AFTER] Initialized with {} HNSW indexes",
            manager.hnsw_indexes.len()
        );

        assert!(manager.initialized);
        // 12 HNSW indexes: E1-E5, E7-E11, E1Matryoshka128, PurposeVector
        assert_eq!(manager.hnsw_indexes.len(), 12);

        println!("[VERIFIED] Initialize creates all 12 HNSW indexes");
    }

    #[tokio::test]
    async fn test_multi_space_add_vector() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let v = random_vector(1024);

        println!("[BEFORE] Adding E1 vector");
        manager
            .add_vector(EmbedderIndex::E1Semantic, id, &v)
            .await
            .unwrap();

        let status = manager.get_embedder_status(EmbedderIndex::E1Semantic);
        println!("[AFTER] E1 index has {} elements", status.element_count);

        assert_eq!(status.element_count, 1);
        println!("[VERIFIED] add_vector adds to correct index");
    }

    #[tokio::test]
    async fn test_multi_space_invalid_embedder() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let v = random_vector(100);

        println!("[BEFORE] Adding to E6Sparse (invalid for HNSW)");
        let result = manager
            .add_vector(EmbedderIndex::E6Sparse, id, &v)
            .await;
        println!("[AFTER] result.is_err() = {}", result.is_err());

        assert!(matches!(result, Err(IndexError::InvalidEmbedder { .. })));
        println!("[VERIFIED] Invalid embedder rejected");
    }

    #[tokio::test]
    async fn test_multi_space_add_fingerprint() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let fingerprint = create_test_fingerprint();

        println!("[BEFORE] Adding complete fingerprint");
        manager.add_fingerprint(id, &fingerprint).await.unwrap();

        // Check all indexes have 1 entry
        let statuses = manager.status();
        println!(
            "[AFTER] Status: {} indexes, total elements = {}",
            statuses.len(),
            statuses.iter().map(|s| s.element_count).sum::<usize>()
        );

        // 12 HNSW + 1 SPLADE = 13 statuses
        assert_eq!(statuses.len(), 13);

        // Verify key indexes
        let e1_status = manager.get_embedder_status(EmbedderIndex::E1Semantic);
        assert_eq!(e1_status.element_count, 1);

        let matryoshka_status = manager.get_embedder_status(EmbedderIndex::E1Matryoshka128);
        assert_eq!(matryoshka_status.element_count, 1);

        assert_eq!(manager.splade_index.len(), 1);

        println!("[VERIFIED] add_fingerprint populates all indexes");
    }

    #[tokio::test]
    async fn test_multi_space_search() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        // Add multiple fingerprints
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        for id in &ids {
            let fp = create_test_fingerprint();
            manager.add_fingerprint(*id, &fp).await.unwrap();
        }

        println!("[BEFORE] Searching E1 semantic index");
        let query = random_vector(1024);
        let results = manager
            .search(EmbedderIndex::E1Semantic, &query, 3)
            .await
            .unwrap();
        println!(
            "[AFTER] Found {} results: {:?}",
            results.len(),
            results.iter().map(|r| r.1).collect::<Vec<_>>()
        );

        assert_eq!(results.len(), 3);
        // Results should be sorted by similarity
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);

        println!("[VERIFIED] Search returns sorted results");
    }

    #[tokio::test]
    async fn test_multi_space_search_matryoshka() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let fp = create_test_fingerprint();
        manager.add_fingerprint(id, &fp).await.unwrap();

        println!("[BEFORE] Searching Matryoshka 128D index");
        let query_128d = random_vector(128);
        let results = manager.search_matryoshka(&query_128d, 10).await.unwrap();
        println!("[AFTER] Found {} results", results.len());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);

        println!("[VERIFIED] search_matryoshka works");
    }

    #[tokio::test]
    async fn test_multi_space_search_purpose() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let purpose_vec = random_vector(13);

        println!("[BEFORE] Adding and searching purpose vector");
        manager.add_purpose_vector(id, &purpose_vec).await.unwrap();

        let results = manager.search_purpose(&purpose_vec, 10).await.unwrap();
        println!("[AFTER] Found {} results, top similarity = {}",
            results.len(),
            results.first().map(|r| r.1).unwrap_or(0.0)
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);
        // Searching with same vector should give high similarity
        assert!(results[0].1 > 0.99);

        println!("[VERIFIED] search_purpose works with high self-similarity");
    }

    #[tokio::test]
    async fn test_multi_space_search_splade() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let sparse = vec![(100, 0.5), (200, 0.3), (300, 0.2)];

        println!("[BEFORE] Adding and searching SPLADE");
        manager.add_splade(id, &sparse).await.unwrap();

        let results = manager.search_splade(&sparse, 10).await.unwrap();
        println!("[AFTER] Found {} results", results.len());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);

        println!("[VERIFIED] search_splade works");
    }

    #[tokio::test]
    async fn test_multi_space_remove() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let id = Uuid::new_v4();
        let fp = create_test_fingerprint();
        manager.add_fingerprint(id, &fp).await.unwrap();

        let before_count: usize = manager.status().iter().map(|s| s.element_count).sum();
        println!("[BEFORE REMOVE] Total elements = {}", before_count);

        manager.remove(id).await.unwrap();

        let after_count: usize = manager.status().iter().map(|s| s.element_count).sum();
        println!("[AFTER REMOVE] Total elements = {}", after_count);

        assert_eq!(after_count, 0);
        println!("[VERIFIED] remove clears all indexes");
    }

    #[tokio::test]
    async fn test_multi_space_persist_and_load() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        // Add data
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        for id in &ids {
            let fp = create_test_fingerprint();
            manager.add_fingerprint(*id, &fp).await.unwrap();
        }

        let before_count: usize = manager.status().iter().map(|s| s.element_count).sum();
        println!("[BEFORE PERSIST] Total elements = {}", before_count);

        // Persist to temp directory
        let temp_dir = std::env::temp_dir().join(format!("hnsw_test_{}", Uuid::new_v4()));
        manager.persist(&temp_dir).await.unwrap();

        // Verify files exist
        assert!(temp_dir.join("index_meta.json").exists());
        assert!(temp_dir.join("splade.bin").exists());
        println!("[PERSIST] Files created at {:?}", temp_dir);

        // Load into new manager
        let mut loaded_manager = HnswMultiSpaceIndex::new();
        loaded_manager.load(&temp_dir).await.unwrap();

        let after_count: usize = loaded_manager.status().iter().map(|s| s.element_count).sum();
        println!("[AFTER LOAD] Total elements = {}", after_count);

        assert_eq!(before_count, after_count);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();

        println!("[VERIFIED] persist/load round-trip preserves data");
    }

    #[tokio::test]
    async fn test_multi_space_not_initialized_error() {
        let manager = HnswMultiSpaceIndex::new();
        let id = Uuid::new_v4();
        let v = random_vector(1024);

        println!("[BEFORE] Attempting add without initialization");
        let result = manager
            .search(EmbedderIndex::E1Semantic, &v, 10)
            .await;
        println!("[AFTER] result.is_err() = {}", result.is_err());

        assert!(matches!(result, Err(IndexError::NotInitialized { .. })));
        println!("[VERIFIED] Operations fail before initialization");
    }

    #[tokio::test]
    async fn test_status_returns_all_indexes() {
        let mut manager = HnswMultiSpaceIndex::new();
        manager.initialize().await.unwrap();

        let statuses = manager.status();
        println!("[STATUS] {} index statuses returned", statuses.len());

        // 12 HNSW + 1 SPLADE = 13 total
        assert_eq!(statuses.len(), 13);

        // All should be healthy
        for status in &statuses {
            assert_eq!(status.health, IndexHealth::Healthy);
            println!("  {:?}: {} elements", status.embedder, status.element_count);
        }

        println!("[VERIFIED] status() returns all 13 indexes");
    }

    #[test]
    fn test_memory_usage_calculation() {
        let config = HnswConfig::default_for_dimension(1024, DistanceMetric::Cosine);
        let mut index = SimpleHnswIndex::new(config);

        let empty_usage = index.memory_usage();
        println!("[BEFORE] Empty index memory: {} bytes", empty_usage);

        for _ in 0..100 {
            let id = Uuid::new_v4();
            let v = random_vector(1024);
            index.add(id, &v).unwrap();
        }

        let full_usage = index.memory_usage();
        println!("[AFTER] 100 vectors memory: {} bytes", full_usage);

        assert!(full_usage > empty_usage);
        // 100 vectors * (16 bytes UUID + 1024 * 4 bytes) = ~409600 bytes
        assert!(full_usage > 400_000);

        println!("[VERIFIED] Memory usage calculation reasonable");
    }
}
