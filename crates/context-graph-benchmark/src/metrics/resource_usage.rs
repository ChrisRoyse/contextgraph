//! Resource usage metrics for measuring embedder overhead.
//!
//! This module tracks:
//! - Index sizes per embedder
//! - Embedding and query latencies
//! - Memory footprint breakdown
//! - Storage efficiency metrics

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Complete resource impact analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceImpact {
    /// Per-embedder index statistics.
    pub index_stats: HashMap<EmbedderIndex, IndexStats>,
    /// Per-embedder embedding latency statistics.
    pub embedding_latency: HashMap<EmbedderIndex, LatencyStats>,
    /// Per-embedder query latency statistics.
    pub query_latency: HashMap<EmbedderIndex, LatencyStats>,
    /// Total memory footprint.
    pub total_memory: MemoryFootprint,
    /// Storage efficiency metrics.
    pub storage_efficiency: StorageEfficiency,
}

impl ResourceImpact {
    /// Create new resource impact.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add index stats for an embedder.
    pub fn add_index_stats(&mut self, stats: IndexStats) {
        self.index_stats.insert(stats.embedder, stats);
    }

    /// Add embedding latency stats for an embedder.
    pub fn add_embedding_latency(&mut self, embedder: EmbedderIndex, stats: LatencyStats) {
        self.embedding_latency.insert(embedder, stats);
    }

    /// Add query latency stats for an embedder.
    pub fn add_query_latency(&mut self, embedder: EmbedderIndex, stats: LatencyStats) {
        self.query_latency.insert(embedder, stats);
    }

    /// Compute totals and efficiencies.
    pub fn finalize(&mut self) {
        // Sum up total memory
        let mut total_bytes: usize = 0;
        for stats in self.index_stats.values() {
            total_bytes += stats.size_bytes;
        }
        self.total_memory.total_bytes = total_bytes;

        // Compute per-embedder bytes
        for stats in self.index_stats.values() {
            if let Some(idx) = stats.embedder.to_index() {
                self.total_memory.per_embedder_bytes[idx] = stats.size_bytes;
            }
        }

        // Compute bytes per document
        let total_vectors: usize = self.index_stats.values().map(|s| s.vector_count).sum();
        if total_vectors > 0 {
            self.total_memory.bytes_per_document =
                total_bytes as f64 / (total_vectors as f64 / 13.0); // Divide by 13 for per-doc
        }

        // Compute storage efficiency
        self.storage_efficiency.compute(&self.index_stats);
    }

    /// Get embedders sorted by index size (largest first).
    pub fn sorted_by_size(&self) -> Vec<(EmbedderIndex, usize)> {
        let mut sorted: Vec<_> = self
            .index_stats
            .iter()
            .map(|(&e, s)| (e, s.size_bytes))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted
    }

    /// Get embedders sorted by query latency (slowest first).
    pub fn sorted_by_query_latency(&self) -> Vec<(EmbedderIndex, u64)> {
        let mut sorted: Vec<_> = self
            .query_latency
            .iter()
            .map(|(&e, l)| (e, l.avg_us))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted
    }

    /// Total index size in bytes.
    pub fn total_index_size(&self) -> usize {
        self.index_stats.values().map(|s| s.size_bytes).sum()
    }

    /// Total index size in MB.
    pub fn total_index_size_mb(&self) -> f64 {
        self.total_index_size() as f64 / (1024.0 * 1024.0)
    }
}

/// Index type enumeration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexType {
    /// HNSW (Hierarchical Navigable Small World) for dense vectors.
    #[default]
    HNSW,
    /// Inverted index for sparse vectors.
    Inverted,
    /// IVF (Inverted File) index.
    IVF,
    /// Flat (brute force) index.
    Flat,
    /// Product Quantization index.
    PQ,
    /// Unknown/custom index type.
    Unknown,
}

/// Statistics for a single embedder's index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    /// Which embedder this index is for.
    pub embedder: EmbedderIndex,
    /// Type of index.
    pub index_type: IndexType,
    /// Index size in bytes.
    pub size_bytes: usize,
    /// Embedding dimension.
    pub dimension: usize,
    /// Number of vectors in index.
    pub vector_count: usize,
    /// Average query latency in microseconds.
    pub avg_query_latency_us: u64,
    /// P99 query latency in microseconds.
    pub p99_query_latency_us: u64,
    /// Whether index is on GPU.
    pub is_gpu_resident: bool,
    /// GPU memory usage if on GPU (bytes).
    pub gpu_memory_bytes: Option<usize>,
    /// Build time in milliseconds.
    pub build_time_ms: u64,
}

impl IndexStats {
    /// Create new index stats.
    pub fn new(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            index_type: IndexType::default(),
            size_bytes: 0,
            dimension: 0,
            vector_count: 0,
            avg_query_latency_us: 0,
            p99_query_latency_us: 0,
            is_gpu_resident: false,
            gpu_memory_bytes: None,
            build_time_ms: 0,
        }
    }

    /// Set index dimensions and compute estimated size.
    pub fn set_dimensions(&mut self, dimension: usize, vector_count: usize) {
        self.dimension = dimension;
        self.vector_count = vector_count;

        // Estimate size based on index type
        let vector_bytes = dimension * 4; // f32 = 4 bytes
        let base_size = vector_count * vector_bytes;

        self.size_bytes = match self.index_type {
            IndexType::HNSW => {
                // HNSW has overhead for graph structure (~1.5-2x)
                (base_size as f64 * 1.7) as usize
            }
            IndexType::Inverted => {
                // Sparse inverted indices are typically smaller
                (base_size as f64 * 0.3) as usize
            }
            IndexType::IVF => {
                // IVF has centroid overhead
                (base_size as f64 * 1.1) as usize
            }
            IndexType::Flat => base_size,
            IndexType::PQ => {
                // PQ compressed, typically 8-16x smaller
                base_size / 8
            }
            IndexType::Unknown => base_size,
        };
    }

    /// Size in MB.
    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Bytes per vector.
    pub fn bytes_per_vector(&self) -> f64 {
        if self.vector_count == 0 {
            return 0.0;
        }
        self.size_bytes as f64 / self.vector_count as f64
    }
}

/// Latency statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Number of operations measured.
    pub count: usize,
    /// Minimum latency in microseconds.
    pub min_us: u64,
    /// Maximum latency in microseconds.
    pub max_us: u64,
    /// Average latency in microseconds.
    pub avg_us: u64,
    /// Median latency in microseconds.
    pub median_us: u64,
    /// P95 latency in microseconds.
    pub p95_us: u64,
    /// P99 latency in microseconds.
    pub p99_us: u64,
    /// Standard deviation in microseconds.
    pub std_dev_us: u64,
    /// Raw samples (for computing percentiles).
    #[serde(skip)]
    samples: Vec<u64>,
}

impl LatencyStats {
    /// Create new latency stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a latency sample.
    pub fn record(&mut self, latency_us: u64) {
        self.count += 1;
        self.samples.push(latency_us);

        if self.count == 1 {
            self.min_us = latency_us;
            self.max_us = latency_us;
        } else {
            self.min_us = self.min_us.min(latency_us);
            self.max_us = self.max_us.max(latency_us);
        }
    }

    /// Finalize statistics.
    pub fn finalize(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Sort for percentiles
        self.samples.sort_unstable();

        // Average
        let sum: u64 = self.samples.iter().sum();
        self.avg_us = sum / self.count as u64;

        // Median
        let mid = self.samples.len() / 2;
        self.median_us = if self.samples.len() % 2 == 0 {
            (self.samples[mid - 1] + self.samples[mid]) / 2
        } else {
            self.samples[mid]
        };

        // P95
        let p95_idx = (self.samples.len() as f64 * 0.95) as usize;
        self.p95_us = self.samples[p95_idx.min(self.samples.len() - 1)];

        // P99
        let p99_idx = (self.samples.len() as f64 * 0.99) as usize;
        self.p99_us = self.samples[p99_idx.min(self.samples.len() - 1)];

        // Standard deviation
        let mean = self.avg_us as f64;
        let variance: f64 = self
            .samples
            .iter()
            .map(|&v| (v as f64 - mean).powi(2))
            .sum::<f64>()
            / self.count as f64;
        self.std_dev_us = variance.sqrt() as u64;

        // Clear samples to save memory
        self.samples.clear();
    }

    /// Average latency in milliseconds.
    pub fn avg_ms(&self) -> f64 {
        self.avg_us as f64 / 1000.0
    }

    /// P99 latency in milliseconds.
    pub fn p99_ms(&self) -> f64 {
        self.p99_us as f64 / 1000.0
    }

    /// Merge another latency stats into this one.
    pub fn merge(&mut self, other: &LatencyStats) {
        self.count += other.count;
        self.min_us = self.min_us.min(other.min_us);
        self.max_us = self.max_us.max(other.max_us);
        // Note: This is an approximation; proper merge would need samples
    }
}

/// Memory footprint breakdown.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFootprint {
    /// Total memory usage in bytes.
    pub total_bytes: usize,
    /// Per-embedder memory usage in bytes.
    pub per_embedder_bytes: [usize; 14],
    /// Average bytes per document (across all embedders).
    pub bytes_per_document: f64,
    /// GPU memory usage if applicable.
    pub gpu_bytes: Option<usize>,
    /// Overhead memory (structures, metadata).
    pub overhead_bytes: usize,
}

impl MemoryFootprint {
    /// Create new memory footprint.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total memory in MB.
    pub fn total_mb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Total memory in GB.
    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// Get memory for an embedder in MB.
    pub fn embedder_mb(&self, embedder: EmbedderIndex) -> f64 {
        embedder
            .to_index()
            .map(|idx| self.per_embedder_bytes[idx] as f64 / (1024.0 * 1024.0))
            .unwrap_or(0.0)
    }

    /// Get percentage of total memory for an embedder.
    pub fn embedder_percentage(&self, embedder: EmbedderIndex) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        embedder
            .to_index()
            .map(|idx| (self.per_embedder_bytes[idx] as f64 / self.total_bytes as f64) * 100.0)
            .unwrap_or(0.0)
    }
}

/// Storage efficiency metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageEfficiency {
    /// Compression ratio (raw bytes / stored bytes).
    pub compression_ratio: f64,
    /// Space amplification factor.
    pub space_amplification: f64,
    /// Overhead percentage (non-vector data).
    pub overhead_percentage: f64,
    /// Most efficient embedder (lowest bytes per dimension).
    pub most_efficient: Option<EmbedderIndex>,
    /// Least efficient embedder (highest bytes per dimension).
    pub least_efficient: Option<EmbedderIndex>,
}

impl StorageEfficiency {
    /// Compute storage efficiency from index stats.
    pub fn compute(&mut self, index_stats: &HashMap<EmbedderIndex, IndexStats>) {
        if index_stats.is_empty() {
            return;
        }

        let mut total_raw = 0usize;
        let mut total_stored = 0usize;
        let mut best_efficiency = f64::MAX;
        let mut worst_efficiency = 0.0f64;

        for stats in index_stats.values() {
            // Raw size = vectors * dimension * 4 bytes (f32)
            let raw_size = stats.vector_count * stats.dimension * 4;
            total_raw += raw_size;
            total_stored += stats.size_bytes;

            // Bytes per dimension
            if stats.dimension > 0 {
                let bytes_per_dim = stats.bytes_per_vector() / stats.dimension as f64;

                if bytes_per_dim < best_efficiency && bytes_per_dim > 0.0 {
                    best_efficiency = bytes_per_dim;
                    self.most_efficient = Some(stats.embedder);
                }
                if bytes_per_dim > worst_efficiency {
                    worst_efficiency = bytes_per_dim;
                    self.least_efficient = Some(stats.embedder);
                }
            }
        }

        if total_stored > 0 {
            self.compression_ratio = total_raw as f64 / total_stored as f64;
            self.space_amplification = total_stored as f64 / total_raw as f64;
        }

        // Overhead = (stored - raw) / stored
        if total_stored > total_raw {
            self.overhead_percentage =
                ((total_stored - total_raw) as f64 / total_stored as f64) * 100.0;
        }
    }
}

/// Resource efficiency score for an embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderEfficiency {
    /// Which embedder this is for.
    pub embedder: EmbedderIndex,
    /// Quality contribution (from ablation study).
    pub quality_contribution: f64,
    /// Memory cost as percentage of total.
    pub memory_cost_pct: f64,
    /// Query latency cost (normalized).
    pub latency_cost_norm: f64,
    /// Efficiency score (quality / cost).
    pub efficiency_score: f64,
}

impl EmbedderEfficiency {
    /// Create new embedder efficiency.
    pub fn new(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            quality_contribution: 0.0,
            memory_cost_pct: 0.0,
            latency_cost_norm: 0.0,
            efficiency_score: 0.0,
        }
    }

    /// Compute efficiency score.
    pub fn compute(&mut self) {
        let cost = self.memory_cost_pct + self.latency_cost_norm;
        if cost > 0.0 {
            self.efficiency_score = self.quality_contribution / cost;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_stats() {
        let mut stats = IndexStats::new(EmbedderIndex::E1Semantic);
        stats.index_type = IndexType::HNSW;
        stats.set_dimensions(1024, 10000);

        // Should have overhead for HNSW
        let raw_size = 1024 * 10000 * 4; // 40MB
        assert!(stats.size_bytes > raw_size);
        assert!(stats.size_bytes < raw_size * 2);
    }

    #[test]
    fn test_latency_stats() {
        let mut stats = LatencyStats::new();
        stats.record(100);
        stats.record(200);
        stats.record(150);
        stats.record(1000); // Outlier
        stats.record(120);

        stats.finalize();

        assert_eq!(stats.count, 5);
        assert_eq!(stats.min_us, 100);
        assert_eq!(stats.max_us, 1000);
        assert_eq!(stats.median_us, 150);
    }

    #[test]
    fn test_memory_footprint() {
        let mut footprint = MemoryFootprint::new();
        footprint.total_bytes = 1024 * 1024 * 1024; // 1GB
        footprint.per_embedder_bytes[0] = 100 * 1024 * 1024; // 100MB for E1

        assert!((footprint.total_gb() - 1.0).abs() < 0.01);
        assert!((footprint.embedder_mb(EmbedderIndex::E1Semantic) - 100.0).abs() < 0.01);
        assert!((footprint.embedder_percentage(EmbedderIndex::E1Semantic) - 9.765).abs() < 0.1);
    }

    #[test]
    fn test_resource_impact() {
        let mut impact = ResourceImpact::new();

        let mut stats1 = IndexStats::new(EmbedderIndex::E1Semantic);
        stats1.set_dimensions(1024, 1000);
        impact.add_index_stats(stats1);

        let mut stats2 = IndexStats::new(EmbedderIndex::E7Code);
        stats2.set_dimensions(1536, 1000);
        impact.add_index_stats(stats2);

        impact.finalize();

        assert_eq!(impact.index_stats.len(), 2);
        assert!(impact.total_index_size() > 0);
    }

    #[test]
    fn test_storage_efficiency() {
        let mut index_stats = HashMap::new();

        // HNSW index (higher overhead)
        let mut hnsw = IndexStats::new(EmbedderIndex::E1Semantic);
        hnsw.index_type = IndexType::HNSW;
        hnsw.set_dimensions(1024, 1000);
        index_stats.insert(EmbedderIndex::E1Semantic, hnsw);

        // Inverted index (lower overhead)
        let mut inv = IndexStats::new(EmbedderIndex::E6Sparse);
        inv.index_type = IndexType::Inverted;
        inv.set_dimensions(30000, 1000);
        index_stats.insert(EmbedderIndex::E6Sparse, inv);

        let mut efficiency = StorageEfficiency::default();
        efficiency.compute(&index_stats);

        // Inverted should be more efficient
        assert_eq!(efficiency.most_efficient, Some(EmbedderIndex::E6Sparse));
    }
}
