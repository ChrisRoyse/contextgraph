//! Session generation from real documents for E4 temporal benchmarks.
//!
//! This module generates plausible "conversation sessions" from topically related
//! documents in the HuggingFace dataset. Sessions have:
//! - Coherent content (high E1 similarity within session)
//! - Assigned sequence positions (for E4 testing)
//! - Ground truth for before/after queries
//!
//! ## Algorithm
//!
//! 1. Group chunks by topic_hint from the source dataset
//! 2. For each topic group, build coherent sessions using E1 similarity
//! 3. Assign sequence positions within each session
//! 4. Generate ground truth queries (direction, chain, boundary)

use std::collections::{HashMap, HashSet};

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::realdata::loader::RealDataset;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for session generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGeneratorConfig {
    /// Minimum chunks per session.
    pub min_session_length: usize,

    /// Maximum chunks per session.
    pub max_session_length: usize,

    /// Number of sessions to generate.
    pub num_sessions: usize,

    /// Minimum E1 cosine similarity for session coherence.
    /// Chunks must have at least this similarity to be in the same session.
    pub coherence_threshold: f32,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Number of direction queries to generate (before/after).
    pub num_direction_queries: usize,

    /// Number of chain traversal queries to generate.
    pub num_chain_queries: usize,

    /// Number of boundary detection queries to generate.
    pub num_boundary_queries: usize,
}

impl Default for SessionGeneratorConfig {
    fn default() -> Self {
        Self {
            min_session_length: 5,
            max_session_length: 25,
            num_sessions: 200,
            coherence_threshold: 0.5,
            seed: 42,
            num_direction_queries: 500,
            num_chain_queries: 100,
            num_boundary_queries: 50,
        }
    }
}

// ============================================================================
// Session Data Structures
// ============================================================================

/// A synthetic session generated from real documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSession {
    /// Unique session identifier.
    pub session_id: String,

    /// Chunks in this session, ordered by sequence position.
    pub chunks: Vec<SessionChunk>,

    /// Primary topic of this session.
    pub topic: String,

    /// Average pairwise E1 similarity within session.
    pub coherence_score: f32,

    /// Source dataset(s) for chunks in this session.
    pub source_datasets: Vec<String>,
}

impl TemporalSession {
    /// Get the number of chunks in this session.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Check if the session is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Get chunk IDs in sequence order.
    pub fn chunk_ids(&self) -> Vec<Uuid> {
        self.chunks.iter().map(|c| c.id).collect()
    }

    /// Get chunk at a specific sequence position.
    pub fn get_at_position(&self, position: usize) -> Option<&SessionChunk> {
        self.chunks.iter().find(|c| c.sequence_position == position)
    }

    /// Get chunks before a given position.
    pub fn chunks_before(&self, position: usize) -> Vec<&SessionChunk> {
        self.chunks
            .iter()
            .filter(|c| c.sequence_position < position)
            .collect()
    }

    /// Get chunks after a given position.
    pub fn chunks_after(&self, position: usize) -> Vec<&SessionChunk> {
        self.chunks
            .iter()
            .filter(|c| c.sequence_position > position)
            .collect()
    }
}

/// A chunk within a session with sequence metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionChunk {
    /// Unique chunk ID (from original dataset).
    pub id: Uuid,

    /// Position in session sequence (0-indexed).
    pub sequence_position: usize,

    /// Chunk text content.
    pub text: String,

    /// Original document ID.
    pub source_doc_id: String,

    /// Original topic hint from source.
    pub original_topic: String,

    /// Source dataset name.
    pub source_dataset: String,
}

// ============================================================================
// Ground Truth Queries
// ============================================================================

/// Direction for sequence queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceDirection {
    Before,
    After,
    Both,
}

impl std::fmt::Display for SequenceDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Before => write!(f, "before"),
            Self::After => write!(f, "after"),
            Self::Both => write!(f, "both"),
        }
    }
}

/// A direction query with ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionQuery {
    /// Query ID.
    pub id: Uuid,

    /// Anchor chunk ID.
    pub anchor_id: Uuid,

    /// Anchor's sequence position.
    pub anchor_sequence: usize,

    /// Session this query belongs to.
    pub session_id: String,

    /// Query direction.
    pub direction: SequenceDirection,

    /// Ground truth: IDs that should be retrieved.
    pub expected_ids: Vec<Uuid>,

    /// Ground truth: IDs in correct order.
    pub expected_order: Vec<Uuid>,
}

/// A chain traversal query with ground truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainQuery {
    /// Query ID.
    pub id: Uuid,

    /// Session this query belongs to.
    pub session_id: String,

    /// Starting sequence position.
    pub start_sequence: usize,

    /// Ending sequence position.
    pub end_sequence: usize,

    /// Start chunk ID.
    pub start_id: Uuid,

    /// End chunk ID.
    pub end_id: Uuid,

    /// Expected chain in order (inclusive of start and end).
    pub expected_chain: Vec<Uuid>,

    /// Chain length (number of hops).
    pub chain_length: usize,
}

/// A session boundary detection query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryQuery {
    /// Query ID.
    pub id: Uuid,

    /// Two chunk IDs to compare.
    pub chunk_a_id: Uuid,
    pub chunk_b_id: Uuid,

    /// Whether chunks are in the same session.
    pub same_session: bool,

    /// Session ID of chunk A.
    pub session_a: String,

    /// Session ID of chunk B.
    pub session_b: String,
}

/// Complete ground truth for E4 sequence benchmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceGroundTruth {
    /// Generated sessions.
    pub sessions: Vec<TemporalSession>,

    /// Direction queries (before/after).
    pub direction_queries: Vec<DirectionQuery>,

    /// Chain traversal queries.
    pub chain_queries: Vec<ChainQuery>,

    /// Boundary detection queries.
    pub boundary_queries: Vec<BoundaryQuery>,

    /// Configuration used.
    pub config: SessionGeneratorConfig,

    /// Statistics.
    pub stats: SessionGenerationStats,
}

/// Statistics from session generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGenerationStats {
    /// Total sessions generated.
    pub total_sessions: usize,

    /// Total chunks assigned to sessions.
    pub total_session_chunks: usize,

    /// Average session length.
    pub avg_session_length: f32,

    /// Min session length.
    pub min_session_length: usize,

    /// Max session length.
    pub max_session_length: usize,

    /// Average coherence score.
    pub avg_coherence: f32,

    /// Topics covered.
    pub topics_covered: usize,

    /// Source datasets used.
    pub source_datasets: Vec<String>,
}

// ============================================================================
// Session Generator
// ============================================================================

/// Generator for temporal sessions from real data.
pub struct SessionGenerator {
    config: SessionGeneratorConfig,
    rng: ChaCha8Rng,
}

impl SessionGenerator {
    /// Create a new session generator with the given configuration.
    pub fn new(config: SessionGeneratorConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate sessions and ground truth from a dataset.
    ///
    /// This version uses topic grouping without E1 embeddings.
    /// For higher coherence, use `generate_with_embeddings()`.
    pub fn generate(&mut self, dataset: &RealDataset) -> SequenceGroundTruth {
        // 1. Group chunks by topic
        let topic_groups = self.group_by_topic(dataset);

        // 2. Generate sessions from topic groups
        let sessions = self.build_sessions_from_topics(&topic_groups, dataset);

        // 3. Generate ground truth queries
        let direction_queries = self.generate_direction_queries(&sessions);
        let chain_queries = self.generate_chain_queries(&sessions);
        let boundary_queries = self.generate_boundary_queries(&sessions);

        // 4. Compute statistics
        let stats = self.compute_stats(&sessions, dataset);

        SequenceGroundTruth {
            sessions,
            direction_queries,
            chain_queries,
            boundary_queries,
            config: self.config.clone(),
            stats,
        }
    }

    /// Generate sessions with E1 embeddings for coherence filtering.
    ///
    /// This produces higher-quality sessions by ensuring chunks within
    /// a session have high E1 (semantic) similarity.
    pub fn generate_with_embeddings(
        &mut self,
        dataset: &RealDataset,
        e1_embeddings: &HashMap<Uuid, Vec<f32>>,
    ) -> SequenceGroundTruth {
        // 1. Group chunks by topic
        let topic_groups = self.group_by_topic(dataset);

        // 2. Generate sessions with coherence filtering
        let sessions = self.build_sessions_with_coherence(&topic_groups, dataset, e1_embeddings);

        // 3. Generate ground truth queries
        let direction_queries = self.generate_direction_queries(&sessions);
        let chain_queries = self.generate_chain_queries(&sessions);
        let boundary_queries = self.generate_boundary_queries(&sessions);

        // 4. Compute statistics
        let stats = self.compute_stats(&sessions, dataset);

        SequenceGroundTruth {
            sessions,
            direction_queries,
            chain_queries,
            boundary_queries,
            config: self.config.clone(),
            stats,
        }
    }

    /// Group chunks by their topic hint.
    fn group_by_topic(&self, dataset: &RealDataset) -> HashMap<String, Vec<usize>> {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();

        for (idx, chunk) in dataset.chunks.iter().enumerate() {
            let topic = if chunk.topic_hint.is_empty() {
                "unknown".to_string()
            } else {
                chunk.topic_hint.clone()
            };
            groups.entry(topic).or_default().push(idx);
        }

        groups
    }

    /// Build sessions from topic groups without embeddings.
    fn build_sessions_from_topics(
        &mut self,
        topic_groups: &HashMap<String, Vec<usize>>,
        dataset: &RealDataset,
    ) -> Vec<TemporalSession> {
        let mut sessions = Vec::new();
        let mut session_counter = 0;

        // Sort topics for deterministic ordering
        let mut topics: Vec<_> = topic_groups.keys().cloned().collect();
        topics.sort();

        for topic in topics {
            let chunk_indices = &topic_groups[&topic];

            // Skip topics with too few chunks
            if chunk_indices.len() < self.config.min_session_length {
                continue;
            }

            // Generate multiple sessions from this topic
            let sessions_from_topic =
                self.build_sessions_for_topic(&topic, chunk_indices, dataset, &mut session_counter);

            sessions.extend(sessions_from_topic);

            // Stop if we have enough sessions
            if sessions.len() >= self.config.num_sessions {
                break;
            }
        }

        // Trim to exact count
        sessions.truncate(self.config.num_sessions);
        sessions
    }

    /// Build sessions for a single topic.
    fn build_sessions_for_topic(
        &mut self,
        topic: &str,
        chunk_indices: &[usize],
        dataset: &RealDataset,
        session_counter: &mut usize,
    ) -> Vec<TemporalSession> {
        let mut sessions = Vec::new();
        let mut used_indices: HashSet<usize> = HashSet::new();
        let mut indices: Vec<usize> = chunk_indices.to_vec();

        // Shuffle for variety
        indices.shuffle(&mut self.rng);

        while used_indices.len() < chunk_indices.len() {
            // Determine session length
            let remaining = chunk_indices.len() - used_indices.len();
            if remaining < self.config.min_session_length {
                break;
            }

            let session_length = self
                .rng
                .gen_range(self.config.min_session_length..=self.config.max_session_length)
                .min(remaining);

            // Select chunks for this session
            let mut session_chunks = Vec::new();
            for &idx in &indices {
                if used_indices.contains(&idx) {
                    continue;
                }
                if session_chunks.len() >= session_length {
                    break;
                }

                let chunk = &dataset.chunks[idx];
                used_indices.insert(idx);

                session_chunks.push(SessionChunk {
                    id: chunk.uuid(),
                    sequence_position: session_chunks.len(),
                    text: chunk.text.clone(),
                    source_doc_id: chunk.doc_id.clone(),
                    original_topic: chunk.topic_hint.clone(),
                    source_dataset: chunk.source_dataset.clone().unwrap_or_default(),
                });
            }

            if session_chunks.len() >= self.config.min_session_length {
                let session_id = format!("session_{:04}", *session_counter);
                *session_counter += 1;

                let source_datasets: Vec<_> = session_chunks
                    .iter()
                    .map(|c| c.source_dataset.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                sessions.push(TemporalSession {
                    session_id,
                    chunks: session_chunks,
                    topic: topic.to_string(),
                    coherence_score: 0.0, // Will compute with embeddings
                    source_datasets,
                });
            }

            // Stop if we have enough total
            if sessions.len() >= self.config.num_sessions {
                break;
            }
        }

        sessions
    }

    /// Build sessions with E1 coherence filtering.
    fn build_sessions_with_coherence(
        &mut self,
        topic_groups: &HashMap<String, Vec<usize>>,
        dataset: &RealDataset,
        e1_embeddings: &HashMap<Uuid, Vec<f32>>,
    ) -> Vec<TemporalSession> {
        let mut sessions = Vec::new();
        let mut session_counter = 0;

        let mut topics: Vec<_> = topic_groups.keys().cloned().collect();
        topics.sort();

        for topic in topics {
            let chunk_indices = &topic_groups[&topic];

            if chunk_indices.len() < self.config.min_session_length {
                continue;
            }

            // Build coherent sessions using greedy nearest-neighbor
            let sessions_from_topic = self.build_coherent_sessions(
                &topic,
                chunk_indices,
                dataset,
                e1_embeddings,
                &mut session_counter,
            );

            sessions.extend(sessions_from_topic);

            if sessions.len() >= self.config.num_sessions {
                break;
            }
        }

        sessions.truncate(self.config.num_sessions);
        sessions
    }

    /// Build coherent sessions using E1 similarity.
    fn build_coherent_sessions(
        &mut self,
        topic: &str,
        chunk_indices: &[usize],
        dataset: &RealDataset,
        e1_embeddings: &HashMap<Uuid, Vec<f32>>,
        session_counter: &mut usize,
    ) -> Vec<TemporalSession> {
        let mut sessions = Vec::new();
        let mut used_indices: HashSet<usize> = HashSet::new();

        // Get chunks with embeddings
        let chunks_with_emb: Vec<_> = chunk_indices
            .iter()
            .filter_map(|&idx| {
                let chunk = &dataset.chunks[idx];
                let uuid = chunk.uuid();
                e1_embeddings.get(&uuid).map(|emb| (idx, emb.clone()))
            })
            .collect();

        if chunks_with_emb.len() < self.config.min_session_length {
            return sessions;
        }

        let mut indices: Vec<usize> = chunks_with_emb.iter().map(|(idx, _)| *idx).collect();
        indices.shuffle(&mut self.rng);

        while used_indices.len() < chunks_with_emb.len() {
            let remaining = chunks_with_emb.len() - used_indices.len();
            if remaining < self.config.min_session_length {
                break;
            }

            // Pick a seed chunk
            let seed_idx = indices
                .iter()
                .find(|&&idx| !used_indices.contains(&idx))
                .copied();

            let seed_idx = match seed_idx {
                Some(idx) => idx,
                None => break,
            };

            // Build session by adding nearest neighbors
            let session = self.build_session_from_seed(
                seed_idx,
                &chunks_with_emb,
                &mut used_indices,
                dataset,
                topic,
                session_counter,
            );

            if let Some(s) = session {
                sessions.push(s);
            }

            if sessions.len() >= self.config.num_sessions {
                break;
            }
        }

        sessions
    }

    /// Build a single session starting from a seed chunk.
    fn build_session_from_seed(
        &mut self,
        seed_idx: usize,
        chunks_with_emb: &[(usize, Vec<f32>)],
        used_indices: &mut HashSet<usize>,
        dataset: &RealDataset,
        topic: &str,
        session_counter: &mut usize,
    ) -> Option<TemporalSession> {
        let target_length = self
            .rng
            .gen_range(self.config.min_session_length..=self.config.max_session_length);

        let mut session_indices = vec![seed_idx];
        used_indices.insert(seed_idx);

        // Build lookup for embeddings
        let idx_to_emb: HashMap<usize, &Vec<f32>> = chunks_with_emb
            .iter()
            .map(|(idx, emb)| (*idx, emb))
            .collect();

        // Verify seed has embedding (required for coherent session)
        idx_to_emb.get(&seed_idx)?;

        // Greedily add nearest neighbors
        while session_indices.len() < target_length {
            let mut best_idx = None;
            let mut best_sim = self.config.coherence_threshold;

            for (idx, emb) in chunks_with_emb {
                if used_indices.contains(idx) {
                    continue;
                }

                // Compute average similarity to all session members
                let avg_sim = session_indices
                    .iter()
                    .filter_map(|&s_idx| idx_to_emb.get(&s_idx))
                    .map(|s_emb| cosine_similarity(emb, s_emb))
                    .sum::<f32>()
                    / session_indices.len() as f32;

                if avg_sim > best_sim {
                    best_sim = avg_sim;
                    best_idx = Some(*idx);
                }
            }

            match best_idx {
                Some(idx) => {
                    session_indices.push(idx);
                    used_indices.insert(idx);
                }
                None => break, // No more chunks meet threshold
            }
        }

        if session_indices.len() < self.config.min_session_length {
            // Release unused indices
            for idx in &session_indices[1..] {
                used_indices.remove(idx);
            }
            return None;
        }

        // Shuffle to randomize sequence order
        session_indices.shuffle(&mut self.rng);

        // Build session chunks
        let mut session_chunks = Vec::new();
        for (seq_pos, &idx) in session_indices.iter().enumerate() {
            let chunk = &dataset.chunks[idx];
            session_chunks.push(SessionChunk {
                id: chunk.uuid(),
                sequence_position: seq_pos,
                text: chunk.text.clone(),
                source_doc_id: chunk.doc_id.clone(),
                original_topic: chunk.topic_hint.clone(),
                source_dataset: chunk.source_dataset.clone().unwrap_or_default(),
            });
        }

        // Compute coherence
        let coherence = self.compute_session_coherence(&session_indices, chunks_with_emb);

        let source_datasets: Vec<_> = session_chunks
            .iter()
            .map(|c| c.source_dataset.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let session_id = format!("session_{:04}", *session_counter);
        *session_counter += 1;

        Some(TemporalSession {
            session_id,
            chunks: session_chunks,
            topic: topic.to_string(),
            coherence_score: coherence,
            source_datasets,
        })
    }

    /// Compute average pairwise E1 similarity for session coherence.
    fn compute_session_coherence(
        &self,
        indices: &[usize],
        chunks_with_emb: &[(usize, Vec<f32>)],
    ) -> f32 {
        if indices.len() < 2 {
            return 1.0;
        }

        let idx_to_emb: HashMap<usize, &Vec<f32>> = chunks_with_emb
            .iter()
            .map(|(idx, emb)| (*idx, emb))
            .collect();

        let mut total_sim = 0.0;
        let mut count = 0;

        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                if let (Some(emb_i), Some(emb_j)) =
                    (idx_to_emb.get(&indices[i]), idx_to_emb.get(&indices[j]))
                {
                    total_sim += cosine_similarity(emb_i, emb_j);
                    count += 1;
                }
            }
        }

        if count > 0 {
            total_sim / count as f32
        } else {
            0.0
        }
    }

    /// Generate direction queries (before/after).
    fn generate_direction_queries(&mut self, sessions: &[TemporalSession]) -> Vec<DirectionQuery> {
        let mut queries = Vec::new();

        for session in sessions {
            if session.len() < 3 {
                continue;
            }

            // Generate queries for random anchors in this session
            let queries_per_session = (self.config.num_direction_queries / sessions.len()).max(1);

            for _ in 0..queries_per_session {
                if queries.len() >= self.config.num_direction_queries {
                    break;
                }

                // Pick anchor not at edges
                let anchor_pos = self.rng.gen_range(1..session.len() - 1);
                let anchor = session.get_at_position(anchor_pos).unwrap();

                // Alternate between before and after
                let direction = if self.rng.gen_bool(0.5) {
                    SequenceDirection::Before
                } else {
                    SequenceDirection::After
                };

                let (expected_ids, expected_order) = match direction {
                    SequenceDirection::Before => {
                        let chunks = session.chunks_before(anchor_pos);
                        let ids: Vec<_> = chunks.iter().map(|c| c.id).collect();
                        let mut ordered = ids.clone();
                        ordered.sort_by_key(|id| {
                            session
                                .chunks
                                .iter()
                                .find(|c| c.id == *id)
                                .map(|c| c.sequence_position)
                                .unwrap_or(0)
                        });
                        ordered.reverse(); // Closest first
                        (ids, ordered)
                    }
                    SequenceDirection::After => {
                        let chunks = session.chunks_after(anchor_pos);
                        let ids: Vec<_> = chunks.iter().map(|c| c.id).collect();
                        let mut ordered = ids.clone();
                        ordered.sort_by_key(|id| {
                            session
                                .chunks
                                .iter()
                                .find(|c| c.id == *id)
                                .map(|c| c.sequence_position)
                                .unwrap_or(0)
                        });
                        (ids, ordered)
                    }
                    SequenceDirection::Both => {
                        let all: Vec<_> = session
                            .chunks
                            .iter()
                            .filter(|c| c.sequence_position != anchor_pos)
                            .map(|c| c.id)
                            .collect();
                        (all.clone(), all)
                    }
                };

                queries.push(DirectionQuery {
                    id: Uuid::new_v4(),
                    anchor_id: anchor.id,
                    anchor_sequence: anchor_pos,
                    session_id: session.session_id.clone(),
                    direction,
                    expected_ids,
                    expected_order,
                });
            }
        }

        queries.truncate(self.config.num_direction_queries);
        queries
    }

    /// Generate chain traversal queries.
    fn generate_chain_queries(&mut self, sessions: &[TemporalSession]) -> Vec<ChainQuery> {
        let mut queries = Vec::new();

        for session in sessions {
            if session.len() < 4 {
                continue;
            }

            let queries_per_session = (self.config.num_chain_queries / sessions.len()).max(1);

            for _ in 0..queries_per_session {
                if queries.len() >= self.config.num_chain_queries {
                    break;
                }

                // Pick random start and end with at least 2 hops between
                let start_pos = self.rng.gen_range(0..session.len() - 2);
                let min_end = start_pos + 2;
                let end_pos = self.rng.gen_range(min_end..session.len());

                let start_chunk = session.get_at_position(start_pos).unwrap();
                let end_chunk = session.get_at_position(end_pos).unwrap();

                let expected_chain: Vec<Uuid> = session
                    .chunks
                    .iter()
                    .filter(|c| c.sequence_position >= start_pos && c.sequence_position <= end_pos)
                    .map(|c| c.id)
                    .collect();

                queries.push(ChainQuery {
                    id: Uuid::new_v4(),
                    session_id: session.session_id.clone(),
                    start_sequence: start_pos,
                    end_sequence: end_pos,
                    start_id: start_chunk.id,
                    end_id: end_chunk.id,
                    expected_chain: expected_chain.clone(),
                    chain_length: expected_chain.len(),
                });
            }
        }

        queries.truncate(self.config.num_chain_queries);
        queries
    }

    /// Generate boundary detection queries.
    fn generate_boundary_queries(&mut self, sessions: &[TemporalSession]) -> Vec<BoundaryQuery> {
        let mut queries = Vec::new();

        if sessions.len() < 2 {
            return queries;
        }

        // Generate same-session pairs
        let same_session_count = self.config.num_boundary_queries / 2;
        for _ in 0..same_session_count {
            let session = &sessions[self.rng.gen_range(0..sessions.len())];
            if session.len() < 2 {
                continue;
            }

            let idx_a = self.rng.gen_range(0..session.len());
            let mut idx_b = self.rng.gen_range(0..session.len());
            while idx_b == idx_a {
                idx_b = self.rng.gen_range(0..session.len());
            }

            queries.push(BoundaryQuery {
                id: Uuid::new_v4(),
                chunk_a_id: session.chunks[idx_a].id,
                chunk_b_id: session.chunks[idx_b].id,
                same_session: true,
                session_a: session.session_id.clone(),
                session_b: session.session_id.clone(),
            });
        }

        // Generate different-session pairs
        let diff_session_count = self.config.num_boundary_queries - same_session_count;
        for _ in 0..diff_session_count {
            let session_a = &sessions[self.rng.gen_range(0..sessions.len())];
            let mut session_b_idx = self.rng.gen_range(0..sessions.len());
            while sessions[session_b_idx].session_id == session_a.session_id {
                session_b_idx = self.rng.gen_range(0..sessions.len());
            }
            let session_b = &sessions[session_b_idx];

            if session_a.is_empty() || session_b.is_empty() {
                continue;
            }

            let chunk_a = &session_a.chunks[self.rng.gen_range(0..session_a.len())];
            let chunk_b = &session_b.chunks[self.rng.gen_range(0..session_b.len())];

            queries.push(BoundaryQuery {
                id: Uuid::new_v4(),
                chunk_a_id: chunk_a.id,
                chunk_b_id: chunk_b.id,
                same_session: false,
                session_a: session_a.session_id.clone(),
                session_b: session_b.session_id.clone(),
            });
        }

        queries.truncate(self.config.num_boundary_queries);
        queries
    }

    /// Compute generation statistics.
    fn compute_stats(
        &self,
        sessions: &[TemporalSession],
        _dataset: &RealDataset,
    ) -> SessionGenerationStats {
        let total_sessions = sessions.len();
        let total_chunks: usize = sessions.iter().map(|s| s.len()).sum();

        let avg_length = if total_sessions > 0 {
            total_chunks as f32 / total_sessions as f32
        } else {
            0.0
        };

        let min_length = sessions.iter().map(|s| s.len()).min().unwrap_or(0);
        let max_length = sessions.iter().map(|s| s.len()).max().unwrap_or(0);

        let avg_coherence = if total_sessions > 0 {
            sessions.iter().map(|s| s.coherence_score).sum::<f32>() / total_sessions as f32
        } else {
            0.0
        };

        let topics: HashSet<_> = sessions.iter().map(|s| s.topic.clone()).collect();
        let sources: HashSet<_> = sessions
            .iter()
            .flat_map(|s| s.source_datasets.clone())
            .collect();

        SessionGenerationStats {
            total_sessions,
            total_session_chunks: total_chunks,
            avg_session_length: avg_length,
            min_session_length: min_length,
            max_session_length: max_length,
            avg_coherence,
            topics_covered: topics.len(),
            source_datasets: sources.into_iter().collect(),
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute cosine similarity between two vectors (raw [-1, 1] range).
///
/// Delegates to the canonical implementation in `crate::util`.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    crate::util::cosine_similarity_raw(a, b)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::realdata::loader::{ChunkRecord, DatasetMetadata};

    fn create_mock_dataset(num_chunks: usize) -> RealDataset {
        let chunks: Vec<ChunkRecord> = (0..num_chunks)
            .map(|i| {
                let topic = format!("topic_{}", i % 5);
                // Create a deterministic UUID-like string ID
                let id = format!(
                    "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                    i,
                    i % 0xFFFF,
                    i / 0xFFFF,
                    (i * 7) % 0xFFFF,
                    i * 13
                );
                ChunkRecord {
                    id,
                    doc_id: format!("doc_{}", i / 10),
                    chunk_idx: i % 10,
                    text: format!("This is chunk {} about {}", i, topic),
                    title: format!("Document {}", i / 10),
                    topic_hint: topic,
                    source_dataset: Some("test".to_string()),
                    start_word: 0,
                    end_word: 10,
                    word_count: 10,
                }
            })
            .collect();

        RealDataset {
            chunks,
            metadata: DatasetMetadata {
                total_documents: num_chunks / 10,
                total_chunks: num_chunks,
                total_words: num_chunks * 10,
                skipped_short: 0,
                chunk_size: 200,
                overlap: 50,
                source: "test".to_string(),
                source_datasets: vec!["test".to_string()],
                dataset_stats: HashMap::new(),
                top_topics: vec![],
                topic_counts: HashMap::new(),
            },
            topic_to_idx: HashMap::new(),
            source_to_idx: HashMap::new(),
        }
    }

    #[test]
    fn test_session_generation_basic() {
        let config = SessionGeneratorConfig {
            min_session_length: 3,
            max_session_length: 8,
            num_sessions: 5,
            coherence_threshold: 0.0,
            seed: 42,
            num_direction_queries: 10,
            num_chain_queries: 5,
            num_boundary_queries: 6,
        };

        let dataset = create_mock_dataset(100);
        let mut generator = SessionGenerator::new(config);
        let ground_truth = generator.generate(&dataset);

        assert!(!ground_truth.sessions.is_empty());
        assert!(ground_truth.sessions.len() <= 5);

        for session in &ground_truth.sessions {
            assert!(session.len() >= 3);
            assert!(session.len() <= 8);

            // Check sequence positions are valid
            for (i, chunk) in session.chunks.iter().enumerate() {
                assert_eq!(chunk.sequence_position, i);
            }
        }
    }

    #[test]
    fn test_direction_query_generation() {
        let config = SessionGeneratorConfig {
            min_session_length: 5,
            max_session_length: 10,
            num_sessions: 3,
            num_direction_queries: 20,
            ..Default::default()
        };

        let dataset = create_mock_dataset(100);
        let mut generator = SessionGenerator::new(config);
        let ground_truth = generator.generate(&dataset);

        assert!(!ground_truth.direction_queries.is_empty());

        for query in &ground_truth.direction_queries {
            let session = ground_truth
                .sessions
                .iter()
                .find(|s| s.session_id == query.session_id)
                .unwrap();

            match query.direction {
                SequenceDirection::Before => {
                    for id in &query.expected_ids {
                        let chunk = session.chunks.iter().find(|c| c.id == *id).unwrap();
                        assert!(chunk.sequence_position < query.anchor_sequence);
                    }
                }
                SequenceDirection::After => {
                    for id in &query.expected_ids {
                        let chunk = session.chunks.iter().find(|c| c.id == *id).unwrap();
                        assert!(chunk.sequence_position > query.anchor_sequence);
                    }
                }
                SequenceDirection::Both => {}
            }
        }
    }

    #[test]
    fn test_chain_query_generation() {
        let config = SessionGeneratorConfig {
            min_session_length: 6,
            max_session_length: 12,
            num_sessions: 3,
            num_chain_queries: 10,
            ..Default::default()
        };

        let dataset = create_mock_dataset(100);
        let mut generator = SessionGenerator::new(config);
        let ground_truth = generator.generate(&dataset);

        assert!(!ground_truth.chain_queries.is_empty());

        for query in &ground_truth.chain_queries {
            assert!(query.start_sequence < query.end_sequence);
            assert!(query.chain_length >= 2);
            assert_eq!(
                query.chain_length,
                query.end_sequence - query.start_sequence + 1
            );
        }
    }

    #[test]
    fn test_boundary_query_generation() {
        let config = SessionGeneratorConfig {
            min_session_length: 4,
            max_session_length: 8,
            num_sessions: 5,
            num_boundary_queries: 20,
            ..Default::default()
        };

        let dataset = create_mock_dataset(100);
        let mut generator = SessionGenerator::new(config);
        let ground_truth = generator.generate(&dataset);

        assert!(!ground_truth.boundary_queries.is_empty());

        let same_session_count = ground_truth
            .boundary_queries
            .iter()
            .filter(|q| q.same_session)
            .count();
        let diff_session_count = ground_truth
            .boundary_queries
            .iter()
            .filter(|q| !q.same_session)
            .count();

        // Should have roughly equal split
        assert!(same_session_count > 0);
        assert!(diff_session_count > 0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 0.001);
    }
}
