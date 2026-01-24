//! Synthetic dataset generation for benchmarking.
//!
//! This module generates controlled test data with known ground truth for
//! evaluating retrieval and clustering performance.

pub mod causal;
pub mod e1_semantic;
pub mod e11_entity;
pub mod generator;
pub mod ground_truth;
pub mod mcp_intent;
pub mod multimodal;
pub mod sparse;
pub mod temporal;
pub mod temporal_sessions;
pub mod topic_clusters;

pub use causal::{CausalBenchmarkDataset, CausalDatasetConfig, CausalDatasetGenerator, CausalDomain};
pub use multimodal::{
    E10DatasetStats, E10MultimodalBenchmarkDataset, E10MultimodalDatasetConfig,
    E10MultimodalDatasetGenerator, IntentDirection, IntentDocument, IntentDomain, IntentQuery,
};
pub use generator::{DatasetGenerator, GeneratorConfig};
pub use ground_truth::GroundTruth;
pub use sparse::{
    E6SparseBenchmarkDataset, E6SparseDatasetConfig, E6SparseDatasetGenerator,
    E6SparseDatasetStats, KeywordDomain, SparseDocument, SparseQuery,
};
pub use temporal::{TemporalBenchmarkDataset, TemporalDatasetConfig, TemporalDatasetGenerator};
pub use temporal_sessions::{
    BoundaryQuery, ChainQuery, DirectionQuery, SequenceDirection, SequenceGroundTruth,
    SessionChunk, SessionGenerator, SessionGeneratorConfig, TemporalSession,
};
pub use mcp_intent::{
    AsymmetricPair, ContextToolQuery, E10Behavior, E1Strength, E1StrengthQuery,
    IntentMemory, IntentToolQuery, MCPIntentBenchmarkDataset, MCPIntentDatasetConfig,
    MCPIntentDatasetGenerator, MCPIntentDatasetStats,
};
pub use topic_clusters::{TopicCluster, TopicGenerator};
pub use e11_entity::{
    E11EntityBenchmarkDataset, E11EntityDatasetConfig, E11EntityDatasetLoader,
    E11EntityDatasetStats, EntityDocument, EntityGroundTruth, EntityLinkSerializable,
    EntityPair, KnowledgeTriple,
};

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::config::TierConfig;

/// A complete benchmark dataset with ground truth.
#[derive(Debug)]
pub struct BenchmarkDataset {
    /// Generated fingerprints with their IDs.
    pub fingerprints: Vec<(Uuid, SemanticFingerprint)>,

    /// Topic assignment for each fingerprint.
    pub topic_assignments: HashMap<Uuid, usize>,

    /// Topic centroids (for reference).
    pub topic_centroids: Vec<TopicCluster>,

    /// Query embeddings.
    pub queries: Vec<QueryData>,

    /// Configuration used to generate this dataset.
    pub config: TierConfig,

    /// Random seed used.
    pub seed: u64,
}

/// Query data with ground truth.
#[derive(Debug, Clone)]
pub struct QueryData {
    /// Query ID.
    pub id: Uuid,

    /// Query embedding (E1 semantic only for baseline comparison).
    pub embedding: Vec<f32>,

    /// Full query fingerprint (all 13 embeddings) for multi-space search.
    pub fingerprint: SemanticFingerprint,

    /// Topic this query is associated with.
    pub topic: usize,

    /// IDs of relevant documents (ground truth).
    pub relevant_docs: HashSet<Uuid>,

    /// Whether this query represents a divergence (new topic).
    pub is_divergent: bool,
}

impl BenchmarkDataset {
    /// Get number of documents.
    pub fn document_count(&self) -> usize {
        self.fingerprints.len()
    }

    /// Get number of topics.
    pub fn topic_count(&self) -> usize {
        self.topic_centroids.len()
    }

    /// Get number of queries.
    pub fn query_count(&self) -> usize {
        self.queries.len()
    }

    /// Get fingerprint by ID.
    pub fn get_fingerprint(&self, id: &Uuid) -> Option<&SemanticFingerprint> {
        self.fingerprints
            .iter()
            .find(|(fid, _)| fid == id)
            .map(|(_, fp)| fp)
    }

    /// Get documents for a specific topic.
    pub fn documents_for_topic(&self, topic: usize) -> Vec<Uuid> {
        self.topic_assignments
            .iter()
            .filter(|(_, &t)| t == topic)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check all fingerprints have topic assignments
        for (id, _) in &self.fingerprints {
            if !self.topic_assignments.contains_key(id) {
                return Err(format!("Missing topic assignment for {}", id));
            }
        }

        // Check topic count matches centroids
        let max_topic = self.topic_assignments.values().max().copied().unwrap_or(0);
        if max_topic >= self.topic_centroids.len() {
            return Err(format!(
                "Topic {} exceeds centroid count {}",
                max_topic,
                self.topic_centroids.len()
            ));
        }

        // Check queries have valid relevant docs
        for query in &self.queries {
            for doc_id in &query.relevant_docs {
                if !self.topic_assignments.contains_key(doc_id) {
                    return Err(format!(
                        "Query {} references unknown doc {}",
                        query.id, doc_id
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get ground truth for retrieval evaluation.
    pub fn retrieval_ground_truth(&self) -> Vec<(Vec<f32>, HashSet<Uuid>)> {
        self.queries
            .iter()
            .map(|q| (q.embedding.clone(), q.relevant_docs.clone()))
            .collect()
    }

    /// Get ground truth topic labels for clustering evaluation.
    pub fn clustering_ground_truth(&self) -> Vec<usize> {
        self.fingerprints
            .iter()
            .map(|(id, _)| self.topic_assignments[id])
            .collect()
    }

    /// Get divergence ground truth.
    pub fn divergence_ground_truth(&self) -> Vec<bool> {
        self.queries.iter().map(|q| q.is_divergent).collect()
    }
}
