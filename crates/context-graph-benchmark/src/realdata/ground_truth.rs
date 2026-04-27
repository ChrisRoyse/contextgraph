//! Per-embedder ground truth generation from real data.
//!
//! Each embedder has different semantics for "relevance":
//! - E1, E5-E7, E10-E13: Same topic_hint = relevant (semantic)
//! - E2: Recent timestamps within window = relevant
//! - E3: Same time-of-day/day-of-week pattern = relevant
//! - E4: Same document, adjacent chunks = relevant
//! - E8: Within-document relationships = relevant
//!
//! This module generates ground truth sets per query, per embedder.

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::config::EmbedderName;
use super::loader::{ChunkRecord, RealDataset};
use super::temporal_injector::{InjectedTemporalMetadata, TemporalSession};

/// Ground truth for a single query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryGroundTruth {
    /// Query ID.
    pub query_id: String,
    /// Query chunk (the query is based on this chunk).
    pub query_chunk_id: Uuid,
    /// Relevant chunk IDs for this query.
    pub relevant_ids: HashSet<Uuid>,
    /// Relevance scores (1.0 = highly relevant, 0.5 = somewhat relevant).
    pub relevance_scores: HashMap<Uuid, f64>,
    /// Metadata about the query (for debugging).
    pub metadata: QueryMetadata,
}

/// Metadata about a query for debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetadata {
    /// Topic of the query chunk.
    pub topic: String,
    /// Document ID of the query chunk.
    pub doc_id: String,
    /// Additional info based on embedder type.
    pub extra: HashMap<String, String>,
}

/// Ground truth collection for an embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderGroundTruth {
    /// Embedder name.
    pub embedder: EmbedderName,
    /// Ground truth per query.
    pub queries: Vec<QueryGroundTruth>,
    /// Total relevant pairs.
    pub total_relevant_pairs: usize,
    /// Average relevant docs per query.
    pub avg_relevant_per_query: f64,
}

/// Complete ground truth for all embedders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedGroundTruth {
    /// Ground truth per embedder.
    pub by_embedder: HashMap<EmbedderName, EmbedderGroundTruth>,
    /// Query chunks (shared across embedders).
    pub query_chunk_ids: Vec<Uuid>,
    /// Corpus size.
    pub corpus_size: usize,
    /// Number of queries.
    pub num_queries: usize,
}

impl UnifiedGroundTruth {
    /// Get ground truth for a specific embedder.
    pub fn get(&self, embedder: EmbedderName) -> Option<&EmbedderGroundTruth> {
        self.by_embedder.get(&embedder)
    }

    /// Get query IDs.
    pub fn query_ids(&self) -> &[Uuid] {
        &self.query_chunk_ids
    }
}

/// Ground truth generator for real data.
pub struct GroundTruthGenerator {
    /// Random number generator.
    rng: ChaCha8Rng,
    /// Number of queries to generate.
    num_queries: usize,
    /// Minimum relevant docs per query.
    min_relevant: usize,
}

impl GroundTruthGenerator {
    /// Create a new generator.
    pub fn new(seed: u64, num_queries: usize) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            num_queries,
            min_relevant: 3,
        }
    }

    /// Set minimum relevant documents per query.
    pub fn with_min_relevant(mut self, min: usize) -> Self {
        self.min_relevant = min;
        self
    }

    /// Generate unified ground truth for all embedders.
    pub fn generate(
        &mut self,
        dataset: &RealDataset,
        temporal_metadata: &InjectedTemporalMetadata,
    ) -> UnifiedGroundTruth {
        // Select query chunks (must have enough related chunks)
        let query_chunk_ids = self.select_query_chunks(dataset);

        let mut by_embedder = HashMap::new();

        // Generate ground truth for each embedder
        for embedder in EmbedderName::all() {
            let gt = match embedder {
                // Semantic embedders: topic-based relevance
                EmbedderName::E1Semantic
                | EmbedderName::E5Causal
                | EmbedderName::E6Sparse
                | EmbedderName::E7Code
                | EmbedderName::E10Multimodal
                | EmbedderName::E11Entity
                | EmbedderName::E12LateInteraction
                | EmbedderName::E13SPLADE => {
                    self.generate_topic_ground_truth(&query_chunk_ids, dataset, embedder)
                }

                // E2: Recency-based relevance
                EmbedderName::E2Recency => {
                    self.generate_recency_ground_truth(&query_chunk_ids, dataset, temporal_metadata)
                }

                // E3: Periodic pattern relevance
                EmbedderName::E3Periodic => self.generate_periodic_ground_truth(
                    &query_chunk_ids,
                    dataset,
                    temporal_metadata,
                ),

                // E4: Sequence-based relevance
                EmbedderName::E4Sequence => {
                    self.generate_sequence_ground_truth(&query_chunk_ids, temporal_metadata)
                }

                // E8: Graph/document structure relevance
                EmbedderName::E8Graph => {
                    self.generate_graph_ground_truth(&query_chunk_ids, dataset)
                }

                // E9: Structural (same format/structure = relevant)
                EmbedderName::E9HDC => {
                    self.generate_structural_ground_truth(&query_chunk_ids, dataset)
                }
            };

            by_embedder.insert(embedder, gt);
        }

        UnifiedGroundTruth {
            by_embedder,
            query_chunk_ids: query_chunk_ids.clone(),
            corpus_size: dataset.chunks.len(),
            num_queries: query_chunk_ids.len(),
        }
    }

    /// Select chunks to use as queries.
    ///
    /// Selects chunks that have enough related chunks in the corpus
    /// to make meaningful retrieval evaluations.
    fn select_query_chunks(&mut self, dataset: &RealDataset) -> Vec<Uuid> {
        // Group chunks by topic
        let mut topic_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            topic_chunks
                .entry(&chunk.topic_hint)
                .or_default()
                .push(chunk);
        }

        // Filter topics with enough chunks
        let valid_topics: Vec<_> = topic_chunks
            .iter()
            .filter(|(_, chunks)| chunks.len() > self.min_relevant)
            .map(|(topic, _)| *topic)
            .collect();

        if valid_topics.is_empty() {
            // Fallback: use any chunks
            return dataset
                .chunks
                .choose_multiple(&mut self.rng, self.num_queries.min(dataset.chunks.len()))
                .map(|c| c.uuid())
                .collect();
        }

        // Sample queries stratified by topic
        let queries_per_topic = (self.num_queries / valid_topics.len()).max(1);
        let mut query_ids = Vec::new();

        for topic in &valid_topics {
            if let Some(chunks) = topic_chunks.get(topic) {
                let samples: Vec<_> = chunks
                    .choose_multiple(&mut self.rng, queries_per_topic)
                    .map(|c| c.uuid())
                    .collect();
                query_ids.extend(samples);
            }

            if query_ids.len() >= self.num_queries {
                break;
            }
        }

        // Shuffle and truncate
        query_ids.shuffle(&mut self.rng);
        query_ids.truncate(self.num_queries);

        query_ids
    }

    /// Generate topic-based ground truth for semantic embedders.
    fn generate_topic_ground_truth(
        &self,
        query_ids: &[Uuid],
        dataset: &RealDataset,
        embedder: EmbedderName,
    ) -> EmbedderGroundTruth {
        // Build UUID to chunk lookup
        let id_to_chunk: HashMap<Uuid, &ChunkRecord> =
            dataset.chunks.iter().map(|c| (c.uuid(), c)).collect();

        // Group chunks by topic
        let mut topic_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            topic_chunks
                .entry(&chunk.topic_hint)
                .or_default()
                .push(chunk);
        }

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let Some(query_chunk) = id_to_chunk.get(query_id) {
                let topic = &query_chunk.topic_hint;

                // Relevant: same topic, different chunk
                let relevant_chunks: Vec<_> = topic_chunks
                    .get(topic.as_str())
                    .map(|v| v.iter().filter(|c| c.uuid() != *query_id).collect())
                    .unwrap_or_default();

                let relevant_ids: HashSet<Uuid> =
                    relevant_chunks.iter().map(|c| c.uuid()).collect();

                // Relevance scores: same document = 1.0, same topic = 0.8
                let mut relevance_scores = HashMap::new();
                for chunk in &relevant_chunks {
                    let score = if chunk.doc_id == query_chunk.doc_id {
                        1.0 // Same document = highly relevant
                    } else {
                        0.8 // Same topic = relevant
                    };
                    relevance_scores.insert(chunk.uuid(), score);
                }

                total_relevant += relevant_ids.len();

                queries.push(QueryGroundTruth {
                    query_id: format!("{}_{}", embedder.as_str(), query_id),
                    query_chunk_id: *query_id,
                    relevant_ids,
                    relevance_scores,
                    metadata: QueryMetadata {
                        topic: topic.clone(),
                        doc_id: query_chunk.doc_id.clone(),
                        extra: HashMap::new(),
                    },
                });
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }

    /// Generate recency-based ground truth for E2.
    fn generate_recency_ground_truth(
        &self,
        query_ids: &[Uuid],
        dataset: &RealDataset,
        temporal: &InjectedTemporalMetadata,
    ) -> EmbedderGroundTruth {
        let id_to_chunk: HashMap<Uuid, &ChunkRecord> =
            dataset.chunks.iter().map(|c| (c.uuid(), c)).collect();

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let (Some(query_chunk), Some(query_ts)) =
                (id_to_chunk.get(query_id), temporal.get_timestamp(query_id))
            {
                // Relevant: chunks within 1 hour of query timestamp
                let window_secs = 3600;
                let query_time = query_ts.timestamp;

                let relevant_ids: HashSet<Uuid> = temporal
                    .timestamps
                    .iter()
                    .filter(|(id, ts)| {
                        *id != query_id
                            && (ts.timestamp - query_time).num_seconds().abs() < window_secs
                    })
                    .map(|(id, _)| *id)
                    .collect();

                let mut relevance_scores = HashMap::new();
                for (id, ts) in &temporal.timestamps {
                    if relevant_ids.contains(id) {
                        // Score based on time distance
                        let distance = (ts.timestamp - query_time).num_seconds().abs() as f64;
                        let score = 1.0 - (distance / window_secs as f64);
                        relevance_scores.insert(*id, score.max(0.5));
                    }
                }

                total_relevant += relevant_ids.len();

                let mut extra = HashMap::new();
                extra.insert("window_secs".to_string(), window_secs.to_string());

                queries.push(QueryGroundTruth {
                    query_id: format!("E2_Recency_{}", query_id),
                    query_chunk_id: *query_id,
                    relevant_ids,
                    relevance_scores,
                    metadata: QueryMetadata {
                        topic: query_chunk.topic_hint.clone(),
                        doc_id: query_chunk.doc_id.clone(),
                        extra,
                    },
                });
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder: EmbedderName::E2Recency,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }

    /// Generate periodic pattern ground truth for E3.
    fn generate_periodic_ground_truth(
        &self,
        query_ids: &[Uuid],
        dataset: &RealDataset,
        temporal: &InjectedTemporalMetadata,
    ) -> EmbedderGroundTruth {
        let id_to_chunk: HashMap<Uuid, &ChunkRecord> =
            dataset.chunks.iter().map(|c| (c.uuid(), c)).collect();

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let (Some(query_chunk), Some(query_periodic)) =
                (id_to_chunk.get(query_id), temporal.get_periodic(query_id))
            {
                // Relevant: same hour cluster
                let relevant_ids: HashSet<Uuid> = temporal
                    .periodic
                    .iter()
                    .filter(|(id, p)| {
                        *id != query_id && p.hour_cluster == query_periodic.hour_cluster
                    })
                    .map(|(id, _)| *id)
                    .collect();

                let mut relevance_scores = HashMap::new();
                for (id, p) in &temporal.periodic {
                    if relevant_ids.contains(id) {
                        // Score based on hour similarity
                        let hour_diff = (p.hour as i32 - query_periodic.hour as i32).abs();
                        let score = 1.0 - (hour_diff as f64 / 12.0);
                        relevance_scores.insert(*id, score.max(0.5));
                    }
                }

                total_relevant += relevant_ids.len();

                let mut extra = HashMap::new();
                extra.insert(
                    "hour_cluster".to_string(),
                    query_periodic.hour_cluster.to_string(),
                );
                extra.insert("hour".to_string(), query_periodic.hour.to_string());

                queries.push(QueryGroundTruth {
                    query_id: format!("E3_Periodic_{}", query_id),
                    query_chunk_id: *query_id,
                    relevant_ids,
                    relevance_scores,
                    metadata: QueryMetadata {
                        topic: query_chunk.topic_hint.clone(),
                        doc_id: query_chunk.doc_id.clone(),
                        extra,
                    },
                });
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder: EmbedderName::E3Periodic,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }

    /// Generate sequence-based ground truth for E4.
    fn generate_sequence_ground_truth(
        &self,
        query_ids: &[Uuid],
        temporal: &InjectedTemporalMetadata,
    ) -> EmbedderGroundTruth {
        // Build session lookup
        let mut chunk_to_session: HashMap<Uuid, &TemporalSession> = HashMap::new();
        for session in &temporal.sessions {
            for chunk in &session.chunks {
                chunk_to_session.insert(chunk.chunk_id, session);
            }
        }

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let Some(session) = chunk_to_session.get(query_id) {
                // Find query position in session
                let query_pos = session.chunks.iter().position(|c| c.chunk_id == *query_id);

                if let Some(pos) = query_pos {
                    // Relevant: adjacent chunks in same session
                    let relevant_ids: HashSet<Uuid> = session
                        .chunks
                        .iter()
                        .filter(|c| c.chunk_id != *query_id)
                        .map(|c| c.chunk_id)
                        .collect();

                    let mut relevance_scores = HashMap::new();
                    for chunk in &session.chunks {
                        if chunk.chunk_id != *query_id {
                            // Score based on sequence distance
                            let distance =
                                (chunk.sequence_position as i32 - pos as i32).abs() as f64;
                            let score = 1.0 - (distance / session.chunks.len() as f64).min(0.5);
                            relevance_scores.insert(chunk.chunk_id, score);
                        }
                    }

                    total_relevant += relevant_ids.len();

                    let mut extra = HashMap::new();
                    extra.insert("session_id".to_string(), session.session_id.clone());
                    extra.insert("sequence_position".to_string(), pos.to_string());
                    extra.insert(
                        "session_length".to_string(),
                        session.chunks.len().to_string(),
                    );

                    queries.push(QueryGroundTruth {
                        query_id: format!("E4_Sequence_{}", query_id),
                        query_chunk_id: *query_id,
                        relevant_ids,
                        relevance_scores,
                        metadata: QueryMetadata {
                            topic: String::new(),
                            doc_id: session.chunks[pos].doc_id.clone(),
                            extra,
                        },
                    });
                }
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder: EmbedderName::E4Sequence,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }

    /// Generate graph-based ground truth for E8.
    fn generate_graph_ground_truth(
        &self,
        query_ids: &[Uuid],
        dataset: &RealDataset,
    ) -> EmbedderGroundTruth {
        let id_to_chunk: HashMap<Uuid, &ChunkRecord> =
            dataset.chunks.iter().map(|c| (c.uuid(), c)).collect();

        // Group chunks by document
        let mut doc_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            doc_chunks.entry(&chunk.doc_id).or_default().push(chunk);
        }

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let Some(query_chunk) = id_to_chunk.get(query_id) {
                // Relevant: chunks in same document (graph neighborhood)
                let doc_id = &query_chunk.doc_id;
                let relevant_chunks: Vec<_> = doc_chunks
                    .get(doc_id.as_str())
                    .map(|v| v.iter().filter(|c| c.uuid() != *query_id).collect())
                    .unwrap_or_default();

                let relevant_ids: HashSet<Uuid> =
                    relevant_chunks.iter().map(|c| c.uuid()).collect();

                // Score based on chunk distance within document
                let mut relevance_scores = HashMap::new();
                let query_chunk_idx = query_chunk.chunk_idx;
                for chunk in &relevant_chunks {
                    let distance = (chunk.chunk_idx as i32 - query_chunk_idx as i32).abs() as f64;
                    let score = 1.0 / (1.0 + distance * 0.1);
                    relevance_scores.insert(chunk.uuid(), score);
                }

                total_relevant += relevant_ids.len();

                let mut extra = HashMap::new();
                extra.insert("doc_chunks".to_string(), relevant_ids.len().to_string());

                queries.push(QueryGroundTruth {
                    query_id: format!("E8_Graph_{}", query_id),
                    query_chunk_id: *query_id,
                    relevant_ids,
                    relevance_scores,
                    metadata: QueryMetadata {
                        topic: query_chunk.topic_hint.clone(),
                        doc_id: query_chunk.doc_id.clone(),
                        extra,
                    },
                });
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder: EmbedderName::E8Graph,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }

    /// Generate structural ground truth for E9.
    fn generate_structural_ground_truth(
        &self,
        query_ids: &[Uuid],
        dataset: &RealDataset,
    ) -> EmbedderGroundTruth {
        let id_to_chunk: HashMap<Uuid, &ChunkRecord> =
            dataset.chunks.iter().map(|c| (c.uuid(), c)).collect();

        // Group by word count range (structural similarity proxy)
        let mut length_groups: HashMap<usize, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            let group = chunk.word_count / 50; // 50-word buckets
            length_groups.entry(group).or_default().push(chunk);
        }

        let mut queries = Vec::new();
        let mut total_relevant = 0;

        for query_id in query_ids {
            if let Some(query_chunk) = id_to_chunk.get(query_id) {
                let query_group = query_chunk.word_count / 50;

                // Relevant: similar length chunks (structural similarity)
                let relevant_chunks: Vec<_> = length_groups
                    .get(&query_group)
                    .map(|v| {
                        v.iter()
                            .filter(|c| c.uuid() != *query_id)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();

                let relevant_ids: HashSet<Uuid> =
                    relevant_chunks.iter().map(|c| c.uuid()).collect();

                let mut relevance_scores = HashMap::new();
                for chunk in &relevant_chunks {
                    let length_diff =
                        (chunk.word_count as i32 - query_chunk.word_count as i32).abs() as f64;
                    let score = 1.0 / (1.0 + length_diff * 0.01);
                    relevance_scores.insert(chunk.uuid(), score);
                }

                total_relevant += relevant_ids.len();

                let mut extra = HashMap::new();
                extra.insert("word_count".to_string(), query_chunk.word_count.to_string());
                extra.insert("length_group".to_string(), query_group.to_string());

                queries.push(QueryGroundTruth {
                    query_id: format!("E9_HDC_{}", query_id),
                    query_chunk_id: *query_id,
                    relevant_ids,
                    relevance_scores,
                    metadata: QueryMetadata {
                        topic: query_chunk.topic_hint.clone(),
                        doc_id: query_chunk.doc_id.clone(),
                        extra,
                    },
                });
            }
        }

        let avg = if queries.is_empty() {
            0.0
        } else {
            total_relevant as f64 / queries.len() as f64
        };

        EmbedderGroundTruth {
            embedder: EmbedderName::E9HDC,
            queries,
            total_relevant_pairs: total_relevant,
            avg_relevant_per_query: avg,
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::realdata::config::TemporalInjectionConfig;
    use crate::realdata::temporal_injector::TemporalMetadataInjector;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset() -> (TempDir, RealDataset) {
        let dir = TempDir::new().unwrap();

        let metadata = super::super::loader::DatasetMetadata {
            total_documents: 5,
            total_chunks: 20,
            total_words: 4000,
            skipped_short: 0,
            chunk_size: 200,
            overlap: 50,
            source: "test".to_string(),
            source_datasets: vec!["test".to_string()],
            dataset_stats: HashMap::new(),
            top_topics: vec![
                "science".to_string(),
                "history".to_string(),
                "tech".to_string(),
            ],
            topic_counts: HashMap::new(),
        };

        let metadata_path = dir.path().join("metadata.json");
        let mut f = File::create(&metadata_path).unwrap();
        serde_json::to_writer(&mut f, &metadata).unwrap();

        let chunks_path = dir.path().join("chunks.jsonl");
        let mut f = File::create(&chunks_path).unwrap();

        let topics = ["science", "history", "tech"];
        for doc_idx in 0..5 {
            for chunk_idx in 0..4 {
                let i = doc_idx * 4 + chunk_idx;
                let chunk = super::super::loader::ChunkRecord {
                    id: format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", i, i, i, i, i),
                    doc_id: format!("doc_{}", doc_idx),
                    title: format!("Document {}", doc_idx),
                    chunk_idx,
                    text: format!("Chunk {} of document {}", chunk_idx, doc_idx),
                    word_count: 100 + chunk_idx * 20,
                    start_word: chunk_idx * 200,
                    end_word: chunk_idx * 200 + 200,
                    topic_hint: topics[doc_idx % 3].to_string(),
                    source_dataset: Some("test".to_string()),
                };
                writeln!(f, "{}", serde_json::to_string(&chunk).unwrap()).unwrap();
            }
        }

        let loader = super::super::loader::DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        (dir, dataset)
    }

    #[test]
    fn test_generate_unified_ground_truth() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig::default();
        let mut injector = TemporalMetadataInjector::new(config, 42);
        let temporal = injector.inject(&dataset);

        let mut generator = GroundTruthGenerator::new(42, 10);
        let gt = generator.generate(&dataset, &temporal);

        // Should have ground truth for all 13 embedders
        assert_eq!(gt.by_embedder.len(), 13);

        // Each embedder should have queries
        for (embedder, egt) in &gt.by_embedder {
            assert!(!egt.queries.is_empty(), "No queries for {}", embedder);
        }
    }

    #[test]
    fn test_topic_ground_truth() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig::default();
        let mut injector = TemporalMetadataInjector::new(config, 42);
        let temporal = injector.inject(&dataset);

        let mut generator = GroundTruthGenerator::new(42, 5);
        let gt = generator.generate(&dataset, &temporal);

        let e1_gt = gt.get(EmbedderName::E1Semantic).unwrap();

        // Each query should have relevant docs with same topic
        for query in &e1_gt.queries {
            assert!(!query.relevant_ids.is_empty());
            // All relevance scores should be in valid range
            for score in query.relevance_scores.values() {
                assert!(*score >= 0.5 && *score <= 1.0);
            }
        }
    }

    #[test]
    fn test_sequence_ground_truth() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig {
            sequence_num_sessions: 5,
            sequence_chunks_per_session: 4,
            ..Default::default()
        };
        let mut injector = TemporalMetadataInjector::new(config, 42);
        let temporal = injector.inject(&dataset);

        let mut generator = GroundTruthGenerator::new(42, 5);
        let gt = generator.generate(&dataset, &temporal);

        let e4_gt = gt.get(EmbedderName::E4Sequence).unwrap();

        // E4 should have queries for chunks in sessions
        // Note: some queries may not have E4 ground truth if not in a session
        for query in &e4_gt.queries {
            assert!(query.metadata.extra.contains_key("session_id"));
        }
    }
}
