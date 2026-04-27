//! Temporal metadata injection for real data benchmarks.
//!
//! Real Wikipedia data lacks timestamps, so we inject synthetic but realistic
//! temporal metadata to enable E2/E3/E4 benchmarking.
//!
//! ## Architecture
//!
//! Per CLAUDE.md ARCH-22 to ARCH-27:
//! - E2 (Recency): Configurable decay functions (linear, exponential, step)
//! - E3 (Periodic): Time-of-day/day-of-week pattern matching
//! - E4 (Sequence): Before/after temporal ordering with directional filtering
//!
//! All temporal embedders are POST-RETRIEVAL only - they NEVER participate in
//! similarity fusion (AP-73).

use chrono::{DateTime, Duration, Utc};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::config::TemporalInjectionConfig;
use super::loader::{ChunkRecord, RealDataset};

/// Injected timestamp for E2 recency benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedTimestamp {
    /// Chunk UUID.
    pub chunk_id: Uuid,
    /// Injected timestamp.
    pub timestamp: DateTime<Utc>,
    /// Age in seconds from "now" (base timestamp).
    pub age_secs: i64,
    /// Recency score using default exponential decay.
    pub default_recency_score: f64,
}

/// Periodic metadata for E3 benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicMetadata {
    /// Chunk UUID.
    pub chunk_id: Uuid,
    /// Hour of day (0-23).
    pub hour: u32,
    /// Day of week (0 = Sunday, 6 = Saturday).
    pub day_of_week: u32,
    /// Hour cluster assignment (for ground truth).
    pub hour_cluster: usize,
    /// Periodic embedding vector [sin(2pi*h/24), cos(2pi*h/24), sin(2pi*d/7), cos(2pi*d/7)].
    pub periodic_embedding: [f64; 4],
}

/// Session chunk for E4 sequence benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionChunk {
    /// Chunk UUID.
    pub chunk_id: Uuid,
    /// Session ID.
    pub session_id: String,
    /// Sequence position within session (0-indexed).
    pub sequence_position: usize,
    /// Document ID (for document-based sessions).
    pub doc_id: String,
    /// Timestamp within session.
    pub timestamp: DateTime<Utc>,
}

/// Temporal session for E4 benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSession {
    /// Session ID.
    pub session_id: String,
    /// Chunks in sequence order.
    pub chunks: Vec<SessionChunk>,
    /// Session start time.
    pub start_time: DateTime<Utc>,
    /// Session end time.
    pub end_time: DateTime<Utc>,
}

impl TemporalSession {
    /// Get session length.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Check if session is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Get chunks before a given sequence position.
    pub fn chunks_before(&self, pos: usize) -> Vec<&SessionChunk> {
        self.chunks
            .iter()
            .filter(|c| c.sequence_position < pos)
            .collect()
    }

    /// Get chunks after a given sequence position.
    pub fn chunks_after(&self, pos: usize) -> Vec<&SessionChunk> {
        self.chunks
            .iter()
            .filter(|c| c.sequence_position > pos)
            .collect()
    }
}

/// Complete injected temporal metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectedTemporalMetadata {
    /// E2: Timestamps and recency scores.
    pub timestamps: HashMap<Uuid, InjectedTimestamp>,
    /// E3: Periodic patterns.
    pub periodic: HashMap<Uuid, PeriodicMetadata>,
    /// E4: Session sequences.
    pub sessions: Vec<TemporalSession>,
    /// Session lookup by chunk ID.
    pub chunk_to_session: HashMap<Uuid, String>,
    /// Configuration used.
    pub config: TemporalInjectionConfig,
}

impl InjectedTemporalMetadata {
    /// Get timestamp for a chunk.
    pub fn get_timestamp(&self, chunk_id: &Uuid) -> Option<&InjectedTimestamp> {
        self.timestamps.get(chunk_id)
    }

    /// Get periodic metadata for a chunk.
    pub fn get_periodic(&self, chunk_id: &Uuid) -> Option<&PeriodicMetadata> {
        self.periodic.get(chunk_id)
    }

    /// Get session ID for a chunk.
    pub fn get_session_id(&self, chunk_id: &Uuid) -> Option<&String> {
        self.chunk_to_session.get(chunk_id)
    }

    /// Find chunks within a time window.
    pub fn chunks_in_window(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<Uuid> {
        self.timestamps
            .iter()
            .filter(|(_, ts)| ts.timestamp >= start && ts.timestamp <= end)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find chunks with same hour cluster (for E3 ground truth).
    pub fn chunks_with_same_hour_cluster(&self, chunk_id: &Uuid) -> Vec<Uuid> {
        if let Some(target) = self.periodic.get(chunk_id) {
            self.periodic
                .iter()
                .filter(|(_, p)| p.hour_cluster == target.hour_cluster)
                .map(|(id, _)| *id)
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// Temporal metadata injector for real datasets.
pub struct TemporalMetadataInjector {
    config: TemporalInjectionConfig,
    rng: ChaCha8Rng,
}

impl TemporalMetadataInjector {
    /// Create a new injector with configuration.
    pub fn new(config: TemporalInjectionConfig, seed: u64) -> Self {
        Self {
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Inject all temporal metadata for a dataset.
    pub fn inject(&mut self, dataset: &RealDataset) -> InjectedTemporalMetadata {
        let timestamps = self.inject_timestamps(dataset);
        let periodic = self.inject_periodic(dataset);
        let sessions = self.inject_sessions(dataset);

        // Build chunk-to-session lookup
        let mut chunk_to_session = HashMap::new();
        for session in &sessions {
            for chunk in &session.chunks {
                chunk_to_session.insert(chunk.chunk_id, session.session_id.clone());
            }
        }

        InjectedTemporalMetadata {
            timestamps,
            periodic,
            sessions,
            chunk_to_session,
            config: self.config.clone(),
        }
    }

    /// Inject E2 recency timestamps.
    ///
    /// Assigns timestamps based on document order with configurable decay.
    /// Documents are spread over the configured time span.
    pub fn inject_timestamps(&mut self, dataset: &RealDataset) -> HashMap<Uuid, InjectedTimestamp> {
        let base_time =
            DateTime::from_timestamp_millis(self.config.base_timestamp_ms).unwrap_or_else(Utc::now);
        let span_secs = self.config.recency_span_days as i64 * 24 * 3600;

        let mut timestamps = HashMap::with_capacity(dataset.chunks.len());

        // Group chunks by document for temporal coherence
        let mut doc_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            doc_chunks.entry(&chunk.doc_id).or_default().push(chunk);
        }

        // Sort documents and assign times
        let mut doc_ids: Vec<_> = doc_chunks.keys().cloned().collect();
        doc_ids.sort();

        let docs_count = doc_ids.len();
        for (doc_idx, doc_id) in doc_ids.iter().enumerate() {
            // Document timestamp: spread over time span
            let doc_age_ratio = doc_idx as f64 / docs_count as f64;
            let doc_age_secs = (doc_age_ratio * span_secs as f64) as i64;
            let doc_time = base_time - Duration::seconds(doc_age_secs);

            // Chunks within document get timestamps in order
            if let Some(chunks) = doc_chunks.get(doc_id) {
                let mut sorted_chunks: Vec<_> = chunks.iter().collect();
                sorted_chunks.sort_by_key(|c| c.chunk_idx);

                for (chunk_idx, chunk) in sorted_chunks.iter().enumerate() {
                    // Small offset within document (minutes)
                    let chunk_offset = chunk_idx as i64 * 60; // 1 minute per chunk
                    let chunk_time = doc_time + Duration::seconds(chunk_offset);
                    let age_secs = (base_time - chunk_time).num_seconds();

                    // Default recency score using exponential decay
                    // Clamp age to non-negative to prevent scores > 1.0 when timestamps
                    // are in the future relative to base_time (negative age)
                    let half_life = self.config.recency_half_life_secs as f64;
                    let clamped_age = age_secs.max(0);
                    let default_recency_score = (-clamped_age as f64 * 0.693 / half_life).exp();

                    timestamps.insert(
                        chunk.uuid(),
                        InjectedTimestamp {
                            chunk_id: chunk.uuid(),
                            timestamp: chunk_time,
                            age_secs,
                            default_recency_score,
                        },
                    );
                }
            }
        }

        timestamps
    }

    /// Inject E3 periodic patterns.
    ///
    /// Assigns time-of-day patterns based on topic clusters.
    /// Different topics get different hour distributions.
    pub fn inject_periodic(&mut self, dataset: &RealDataset) -> HashMap<Uuid, PeriodicMetadata> {
        let mut periodic = HashMap::with_capacity(dataset.chunks.len());

        if !self.config.periodic_enabled {
            return periodic;
        }

        // Assign each topic to an hour cluster
        let topics: Vec<_> = dataset.topic_to_idx.keys().collect();
        let num_clusters = self.config.periodic_hour_clusters;

        let topic_to_cluster: HashMap<_, _> = topics
            .iter()
            .enumerate()
            .map(|(i, topic)| (*topic, i % num_clusters))
            .collect();

        // Assign periodic metadata to chunks
        for chunk in &dataset.chunks {
            let hour_cluster = *topic_to_cluster.get(&chunk.topic_hint).unwrap_or(&0);

            // Generate hour within cluster (3-hour windows)
            let cluster_base_hour = (hour_cluster * 24 / num_clusters) as u32;
            let hour_offset: u32 = self.rng.gen_range(0..3);
            let hour = (cluster_base_hour + hour_offset) % 24;

            // Random day of week with slight bias
            let day_of_week: u32 = self.rng.gen_range(0..7);

            // Compute periodic embedding
            let h_rad = 2.0 * std::f64::consts::PI * hour as f64 / 24.0;
            let d_rad = 2.0 * std::f64::consts::PI * day_of_week as f64 / 7.0;
            let periodic_embedding = [h_rad.sin(), h_rad.cos(), d_rad.sin(), d_rad.cos()];

            periodic.insert(
                chunk.uuid(),
                PeriodicMetadata {
                    chunk_id: chunk.uuid(),
                    hour,
                    day_of_week,
                    hour_cluster,
                    periodic_embedding,
                },
            );
        }

        periodic
    }

    /// Inject E4 session sequences.
    ///
    /// Creates sessions from document chunks (document = session).
    /// Also creates cross-document sessions by topic for variety.
    pub fn inject_sessions(&mut self, dataset: &RealDataset) -> Vec<TemporalSession> {
        let mut sessions = Vec::new();
        let base_time =
            DateTime::from_timestamp_millis(self.config.base_timestamp_ms).unwrap_or_else(Utc::now);

        // Strategy 1: Document-based sessions (chunks within a document)
        let mut doc_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            doc_chunks.entry(&chunk.doc_id).or_default().push(chunk);
        }

        for (doc_id, chunks) in &doc_chunks {
            if chunks.len() < 2 {
                continue; // Need at least 2 chunks for sequence
            }

            let mut sorted_chunks: Vec<_> = chunks.to_vec();
            sorted_chunks.sort_by_key(|c| c.chunk_idx);

            let session_id = format!("doc_{}", doc_id);
            let session_start = base_time - Duration::hours(self.rng.gen_range(0..1000));

            let session_chunks: Vec<SessionChunk> = sorted_chunks
                .iter()
                .enumerate()
                .map(|(pos, chunk)| SessionChunk {
                    chunk_id: chunk.uuid(),
                    session_id: session_id.clone(),
                    sequence_position: pos,
                    doc_id: chunk.doc_id.clone(),
                    timestamp: session_start + Duration::minutes(pos as i64 * 5),
                })
                .collect();

            let session_end = session_chunks
                .last()
                .map(|c| c.timestamp)
                .unwrap_or(session_start);

            sessions.push(TemporalSession {
                session_id,
                chunks: session_chunks,
                start_time: session_start,
                end_time: session_end,
            });

            if sessions.len() >= self.config.sequence_num_sessions / 2 {
                break; // Half from documents
            }
        }

        // Strategy 2: Topic-based sessions (random chunks from same topic)
        let mut topic_chunks: HashMap<&str, Vec<&ChunkRecord>> = HashMap::new();
        for chunk in &dataset.chunks {
            topic_chunks
                .entry(&chunk.topic_hint)
                .or_default()
                .push(chunk);
        }

        let topics: Vec<_> = topic_chunks.keys().cloned().collect();
        for i in 0..(self.config.sequence_num_sessions / 2) {
            let topic = topics[i % topics.len()];
            if let Some(chunks) = topic_chunks.get(topic) {
                if chunks.len() < self.config.sequence_chunks_per_session {
                    continue;
                }

                // Sample random chunks from topic
                let mut sampled: Vec<_> = chunks
                    .choose_multiple(&mut self.rng, self.config.sequence_chunks_per_session)
                    .cloned()
                    .collect();
                sampled.shuffle(&mut self.rng);

                let session_id = format!("topic_{}_{}", topic, i);
                let session_start = base_time - Duration::hours(self.rng.gen_range(0..1000));

                let session_chunks: Vec<SessionChunk> = sampled
                    .iter()
                    .enumerate()
                    .map(|(pos, chunk)| SessionChunk {
                        chunk_id: chunk.uuid(),
                        session_id: session_id.clone(),
                        sequence_position: pos,
                        doc_id: chunk.doc_id.clone(),
                        timestamp: session_start + Duration::minutes(pos as i64 * 3),
                    })
                    .collect();

                let session_end = session_chunks
                    .last()
                    .map(|c| c.timestamp)
                    .unwrap_or(session_start);

                sessions.push(TemporalSession {
                    session_id,
                    chunks: session_chunks,
                    start_time: session_start,
                    end_time: session_end,
                });
            }
        }

        sessions
    }
}

/// Calculate cosine similarity between periodic embeddings.
pub fn periodic_similarity(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Recency decay functions per ARCH-22.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DecayFunction {
    /// Linear decay: score = max(0, 1 - age/max_age)
    Linear,
    /// Exponential decay: score = exp(-age * 0.693 / half_life)
    Exponential,
    /// Step decay: Fresh(<5min)=1.0, Recent(<1h)=0.8, Today(<1d)=0.5, Older=0.1
    Step,
    /// No decay: score = 1.0
    NoDecay,
}

impl DecayFunction {
    /// Calculate recency score.
    pub fn score(&self, age_secs: i64, half_life_secs: u64) -> f64 {
        match self {
            DecayFunction::Linear => {
                let max_age = half_life_secs as f64 * 10.0; // 10 half-lives = ~0.1% remaining
                (1.0 - age_secs as f64 / max_age).max(0.0)
            }
            DecayFunction::Exponential => {
                // Clamp age to non-negative to prevent scores > 1.0
                let clamped_age = age_secs.max(0);
                (-clamped_age as f64 * 0.693 / half_life_secs as f64).exp()
            }
            DecayFunction::Step => {
                if age_secs < 300 {
                    // < 5 min
                    1.0
                } else if age_secs < 3600 {
                    // < 1 hour
                    0.8
                } else if age_secs < 86400 {
                    // < 1 day
                    0.5
                } else {
                    0.1
                }
            }
            DecayFunction::NoDecay => 1.0,
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset() -> (TempDir, RealDataset) {
        let dir = TempDir::new().unwrap();

        // Create metadata
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

        // Create chunks
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
                    word_count: 10,
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
    fn test_inject_timestamps() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig::default();
        let mut injector = TemporalMetadataInjector::new(config, 42);

        let timestamps = injector.inject_timestamps(&dataset);

        assert_eq!(timestamps.len(), dataset.chunks.len());

        // Verify all chunks have timestamps
        for chunk in &dataset.chunks {
            assert!(timestamps.contains_key(&chunk.uuid()));
        }

        // Verify recency scores are in valid range
        for ts in timestamps.values() {
            assert!(ts.default_recency_score >= 0.0);
            assert!(ts.default_recency_score <= 1.0);
        }
    }

    #[test]
    fn test_inject_periodic() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig {
            periodic_enabled: true,
            periodic_hour_clusters: 4,
            ..Default::default()
        };
        let mut injector = TemporalMetadataInjector::new(config, 42);

        let periodic = injector.inject_periodic(&dataset);

        assert_eq!(periodic.len(), dataset.chunks.len());

        // Verify hour values are valid
        for p in periodic.values() {
            assert!(p.hour < 24);
            assert!(p.day_of_week < 7);
            assert!(p.hour_cluster < 4);
        }
    }

    #[test]
    fn test_inject_sessions() {
        let (_dir, dataset) = create_test_dataset();
        let config = TemporalInjectionConfig {
            sequence_num_sessions: 10,
            sequence_chunks_per_session: 3,
            ..Default::default()
        };
        let mut injector = TemporalMetadataInjector::new(config, 42);

        let sessions = injector.inject_sessions(&dataset);

        // Should have created some sessions
        assert!(!sessions.is_empty());

        // Each session should have multiple chunks in order
        for session in &sessions {
            assert!(session.chunks.len() >= 2);

            // Verify sequence positions are correct
            for (i, chunk) in session.chunks.iter().enumerate() {
                assert_eq!(chunk.sequence_position, i);
            }
        }
    }

    #[test]
    fn test_decay_functions() {
        let half_life = 3600; // 1 hour

        // Exponential decay at half-life should be ~0.5
        let exp_score = DecayFunction::Exponential.score(half_life as i64, half_life);
        assert!((exp_score - 0.5).abs() < 0.01);

        // Step decay
        assert_eq!(DecayFunction::Step.score(60, half_life), 1.0); // < 5 min
        assert_eq!(DecayFunction::Step.score(1800, half_life), 0.8); // < 1 hour
        assert_eq!(DecayFunction::Step.score(7200, half_life), 0.5); // < 1 day
        assert_eq!(DecayFunction::Step.score(100000, half_life), 0.1); // > 1 day

        // No decay
        assert_eq!(DecayFunction::NoDecay.score(1000000, half_life), 1.0);
    }

    #[test]
    fn test_negative_age_clamped() {
        let half_life = 3600; // 1 hour

        // Negative age (future timestamps) should be clamped to 0, giving score = 1.0
        let score = DecayFunction::Exponential.score(-1000, half_life);
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "Negative age should give score 1.0, got {}",
            score
        );
        assert!(score <= 1.0, "Score should never exceed 1.0, got {}", score);

        // Very negative age should still give 1.0
        let score = DecayFunction::Exponential.score(-100000, half_life);
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "Negative age should give score 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_periodic_similarity() {
        let a = [0.5, 0.866, 0.0, 1.0]; // 2am, Sunday
        let b = [0.5, 0.866, 0.0, 1.0]; // Same
        assert!((periodic_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = [-0.5, 0.866, 0.0, -1.0]; // Different
        let sim = periodic_similarity(&a, &c);
        assert!(sim < 0.9); // Should be less similar
    }
}
