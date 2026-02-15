#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! # Context Graph Benchmark Suite
//!
//! Comprehensive benchmarking system comparing the 13-embedder multi-space fingerprinting
//! system against a traditional single-embedder RAG baseline.
//!
//! ## Key Hypotheses
//!
//! 1. **Single-embedder RAG scaling problem**: Accuracy degrades as corpus grows because
//!    semantic similarity becomes less discriminative.
//! 2. **Multi-space advantage**: 13 orthogonal signals reduce false positives and maintain
//!    accuracy at scale.
//! 3. **Topic isolation**: Multi-space should maintain topic separation where single-embedder
//!    confuses topics.
//!
//! ## Tier Configurations
//!
//! | Tier | Memories | Topics | Use Case |
//! |------|----------|--------|----------|
//! | 0 | 100 | 5 | Baseline calibration |
//! | 1 | 1,000 | 20 | Small project |
//! | 2 | 10,000 | 50 | Medium project |
//! | 3 | 100,000 | 100 | Large project |
//! | 4 | 1,000,000 | 200 | Enterprise |
//! | 5 | 10,000,000 | 500 | Theoretical limit |
//!
//! ## Metrics Tracked
//!
//! - **Retrieval**: P@K, R@K, MRR, NDCG, MAP
//! - **Clustering**: Purity, NMI, ARI, Silhouette
//! - **Performance**: Latency percentiles, throughput, memory
//!
//! ## Usage
//!
//! ```bash
//! # Run full benchmark suite
//! cargo bench -p context-graph-benchmark
//!
//! # Run specific tier
//! cargo bench -p context-graph-benchmark -- tier_2
//!
//! # Run scaling analysis only
//! cargo bench -p context-graph-benchmark -- scaling
//!
//! # Generate reports
//! cargo run -p context-graph-benchmark --bin benchmark-report -- --format json --output results.json
//! ```

pub mod ablation;
pub mod baseline;
pub mod causal_bench;
pub mod config;
pub mod datasets;
pub mod metrics;
pub mod realdata;
pub mod reports;
pub mod runners;
pub mod scaling;
pub mod stress_corpus;
pub mod tuning;
pub mod util;
pub mod validation;

// Re-export key types for convenience
pub use config::{BenchmarkConfig, TierConfig};
pub use datasets::{BenchmarkDataset, DatasetGenerator, GroundTruth};
pub use metrics::{
    ClusteringMetrics, PerformanceMetrics, RetrievalMetrics, ScalingMetrics, TemporalMetrics,
};
pub use reports::{BenchmarkReport, ReportFormat};
pub use runners::{BenchmarkHarness, BenchmarkResults, TemporalBenchmarkConfig, TemporalBenchmarkResults, TemporalBenchmarkRunner};
pub use realdata::{
    ChunkRecord, DatasetLoader, DatasetMetadata, RealDataBenchConfig, RealDataBenchRunner,
    RealDataEmbedder, RealDataResults, RealDataset,
};
pub use scaling::{DegradationAnalysis, ScalingLimits};
pub use datasets::{TemporalBenchmarkDataset, TemporalDatasetConfig, TemporalDatasetGenerator};
