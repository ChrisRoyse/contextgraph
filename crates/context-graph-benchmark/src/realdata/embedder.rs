//! Embedder for real dataset chunks.
//!
//! Generates 13-embedder fingerprints from text chunks.
//!
//! ## E4 Sequence-Aware Embedding
//!
//! This module supports sequence-aware embedding for E4 (V_ordering) via
//! `embed_with_sequences()`. When embedding chunks that belong to sessions
//! with known sequence positions, this method passes "sequence:N" instructions
//! to E4, enabling proper "before/after" queries within sessions.

use std::collections::HashMap;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

#[cfg(feature = "real-embeddings")]
use context_graph_core::traits::EmbeddingMetadata;

#[cfg(feature = "real-embeddings")]
use super::loader::RealDataset;

/// Configuration for the real data embedder.
#[derive(Debug, Clone)]
pub struct EmbedderConfig {
    /// Batch size for embedding.
    pub batch_size: usize,
    /// Whether to show progress.
    pub show_progress: bool,
    /// Device to use (cuda:0, cpu, etc.).
    pub device: String,
}

impl Default for EmbedderConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            show_progress: true,
            device: "cuda:0".to_string(),
        }
    }
}

/// Result of embedding a dataset.
#[derive(Debug)]
pub struct EmbeddedDataset {
    /// Fingerprints indexed by chunk UUID.
    pub fingerprints: HashMap<Uuid, SemanticFingerprint>,
    /// Topic assignments.
    pub topic_assignments: HashMap<Uuid, usize>,
    /// Original chunk metadata (id -> text, title, etc.).
    pub chunk_info: HashMap<Uuid, ChunkInfo>,
    /// Number of topics.
    pub topic_count: usize,
}

/// Minimal chunk info for reference.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkInfo {
    pub doc_id: String,
    pub title: String,
    pub chunk_idx: usize,
    pub topic_hint: String,
}

/// Checkpoint for resumable embedding operations.
///
/// Saves embedding progress periodically to allow resuming after interruption.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingCheckpoint {
    /// Number of embeddings completed.
    pub embedded_count: usize,
    /// Fingerprints computed so far.
    pub fingerprints: HashMap<Uuid, SemanticFingerprint>,
    /// Chunk info computed so far.
    pub chunk_info: HashMap<Uuid, ChunkInfo>,
    /// ID of the last processed chunk.
    pub last_chunk_id: String,
}

impl EmbeddingCheckpoint {
    const CHECKPOINT_FILE: &'static str = "embedding_checkpoint.json";

    /// Save checkpoint to directory.
    pub fn save(&self, dir: &std::path::Path) -> Result<(), EmbedError> {
        use std::fs::File;
        use std::io::BufWriter;

        std::fs::create_dir_all(dir).map_err(|e| EmbedError::Checkpoint(e.to_string()))?;

        let path = dir.join(Self::CHECKPOINT_FILE);
        let file = File::create(&path).map_err(|e| EmbedError::Checkpoint(e.to_string()))?;
        let writer = BufWriter::new(file);

        serde_json::to_writer(writer, self).map_err(|e| EmbedError::Checkpoint(e.to_string()))?;

        Ok(())
    }

    /// Load checkpoint from directory.
    pub fn load(dir: &std::path::Path) -> Result<Self, EmbedError> {
        use std::fs::File;
        use std::io::BufReader;

        let path = dir.join(Self::CHECKPOINT_FILE);
        let file = File::open(&path).map_err(|e| EmbedError::Checkpoint(e.to_string()))?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).map_err(|e| EmbedError::Checkpoint(e.to_string()))
    }

    /// Check if a checkpoint exists.
    pub fn exists(dir: &std::path::Path) -> bool {
        dir.join(Self::CHECKPOINT_FILE).exists()
    }

    /// Remove checkpoint file.
    pub fn cleanup(dir: &std::path::Path) {
        let path = dir.join(Self::CHECKPOINT_FILE);
        let _ = std::fs::remove_file(path);
    }
}

/// Embedder for real data using the 13-embedder system.
pub struct RealDataEmbedder {
    #[allow(dead_code)] // Used by bin targets, not lib tests
    config: EmbedderConfig,
}

impl RealDataEmbedder {
    /// Create a new embedder with default config.
    pub fn new() -> Self {
        Self {
            config: EmbedderConfig::default(),
        }
    }

    /// Create with specific config.
    pub fn with_config(config: EmbedderConfig) -> Self {
        Self { config }
    }

    /// Embed a dataset using the 13-embedder system.
    ///
    /// This is the main entry point for generating fingerprints from real data.
    /// It uses the context-graph-embeddings crate for actual embedding.
    ///
    /// Note: This is an async function that requires tokio runtime.
    #[cfg(feature = "real-embeddings")]
    pub async fn embed_dataset(
        &self,
        dataset: &RealDataset,
    ) -> Result<EmbeddedDataset, EmbedError> {
        use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};

        // Initialize the warm provider (loads all 13 models to GPU)
        initialize_global_warm_provider()
            .await
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let provider = get_warm_provider().map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let total = dataset.chunks.len();
        let mut fingerprints = HashMap::with_capacity(total);
        let mut chunk_info = HashMap::with_capacity(total);

        for (i, chunk) in dataset.chunks.iter().enumerate() {
            if self.config.show_progress && i % 100 == 0 {
                eprint!(
                    "\rEmbedding: {}/{} chunks ({:.1}%)",
                    i,
                    total,
                    i as f64 / total as f64 * 100.0
                );
            }

            // Embed using the warm provider (all 13 embedders)
            let output = provider
                .embed_all(&chunk.text)
                .await
                .map_err(|e| EmbedError::Embedding(e.to_string()))?;

            let uuid = chunk.uuid();
            fingerprints.insert(uuid, output.fingerprint);
            chunk_info.insert(
                uuid,
                ChunkInfo {
                    doc_id: chunk.doc_id.clone(),
                    title: chunk.title.clone(),
                    chunk_idx: chunk.chunk_idx,
                    topic_hint: chunk.topic_hint.clone(),
                },
            );
        }

        if self.config.show_progress {
            eprintln!("\rEmbedding: {}/{} chunks (100.0%)", total, total);
        }

        Ok(EmbeddedDataset {
            fingerprints,
            topic_assignments: dataset.topic_assignments(),
            chunk_info,
            topic_count: dataset.topic_count(),
        })
    }

    /// Embed a dataset with batching and checkpointing support.
    ///
    /// This version supports:
    /// - Batched processing for better GPU utilization
    /// - Periodic checkpointing for resume capability
    /// - Progress tracking with ETA estimation
    ///
    /// Checkpoint files are saved to `checkpoint_dir` every `checkpoint_interval` embeddings.
    #[cfg(feature = "real-embeddings")]
    pub async fn embed_dataset_batched(
        &self,
        dataset: &RealDataset,
        checkpoint_dir: Option<&std::path::Path>,
        checkpoint_interval: usize,
    ) -> Result<EmbeddedDataset, EmbedError> {
        use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};
        use std::time::Instant;

        // Initialize the warm provider (loads all 13 models to GPU)
        initialize_global_warm_provider()
            .await
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let provider = get_warm_provider().map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let total = dataset.chunks.len();
        let batch_size = self.config.batch_size;
        let checkpoint_interval = if checkpoint_interval == 0 {
            1000
        } else {
            checkpoint_interval
        };

        // Try to load existing checkpoint
        let (mut fingerprints, mut chunk_info, start_idx) = if let Some(dir) = checkpoint_dir {
            match EmbeddingCheckpoint::load(dir) {
                Ok(checkpoint) => {
                    if self.config.show_progress {
                        eprintln!(
                            "Resuming from checkpoint: {} embeddings",
                            checkpoint.embedded_count
                        );
                    }
                    (
                        checkpoint.fingerprints,
                        checkpoint.chunk_info,
                        checkpoint.embedded_count,
                    )
                }
                Err(_) => (
                    HashMap::with_capacity(total),
                    HashMap::with_capacity(total),
                    0,
                ),
            }
        } else {
            (
                HashMap::with_capacity(total),
                HashMap::with_capacity(total),
                0,
            )
        };

        let start_time = Instant::now();
        let mut last_checkpoint_time = Instant::now();
        let mut embeddings_since_checkpoint = 0;

        // Process in batches
        let chunks_to_process: Vec<_> = dataset.chunks.iter().skip(start_idx).collect();

        for (batch_idx, batch) in chunks_to_process.chunks(batch_size).enumerate() {
            let batch_start = start_idx + batch_idx * batch_size;

            // Embed each chunk in the batch
            for (i, chunk) in batch.iter().enumerate() {
                let global_idx = batch_start + i;

                if self.config.show_progress && global_idx % 100 == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let rate = if global_idx > start_idx {
                        (global_idx - start_idx) as f64 / elapsed
                    } else {
                        0.0
                    };
                    let eta = if rate > 0.0 {
                        (total - global_idx) as f64 / rate
                    } else {
                        0.0
                    };
                    eprint!(
                        "\rEmbedding: {}/{} ({:.1}%) | {:.1} chunks/s | ETA: {:.0}s",
                        global_idx,
                        total,
                        global_idx as f64 / total as f64 * 100.0,
                        rate,
                        eta
                    );
                }

                // Embed using the warm provider (all 13 embedders)
                let output = provider
                    .embed_all(&chunk.text)
                    .await
                    .map_err(|e| EmbedError::Embedding(e.to_string()))?;

                let uuid = chunk.uuid();
                fingerprints.insert(uuid, output.fingerprint);
                chunk_info.insert(
                    uuid,
                    ChunkInfo {
                        doc_id: chunk.doc_id.clone(),
                        title: chunk.title.clone(),
                        chunk_idx: chunk.chunk_idx,
                        topic_hint: chunk.topic_hint.clone(),
                    },
                );

                embeddings_since_checkpoint += 1;
            }

            // Save checkpoint periodically
            if let Some(dir) = checkpoint_dir {
                if embeddings_since_checkpoint >= checkpoint_interval
                    || last_checkpoint_time.elapsed().as_secs() >= 60
                {
                    let checkpoint = EmbeddingCheckpoint {
                        embedded_count: fingerprints.len(),
                        fingerprints: fingerprints.clone(),
                        chunk_info: chunk_info.clone(),
                        last_chunk_id: batch.last().map(|c| c.id.clone()).unwrap_or_default(),
                    };
                    checkpoint.save(dir)?;
                    embeddings_since_checkpoint = 0;
                    last_checkpoint_time = Instant::now();

                    if self.config.show_progress {
                        eprintln!("\n  Checkpoint saved: {} embeddings", fingerprints.len());
                    }
                }
            }
        }

        if self.config.show_progress {
            let elapsed = start_time.elapsed().as_secs_f64();
            let rate = (total - start_idx) as f64 / elapsed;
            eprintln!(
                "\rEmbedding complete: {}/{} chunks | {:.1} chunks/s | {:.1}s total",
                total, total, rate, elapsed
            );
        }

        // Clean up checkpoint on successful completion
        if let Some(dir) = checkpoint_dir {
            EmbeddingCheckpoint::cleanup(dir);
        }

        Ok(EmbeddedDataset {
            fingerprints,
            topic_assignments: dataset.topic_assignments(),
            chunk_info,
            topic_count: dataset.topic_count(),
        })
    }

    /// Embed session chunks with sequence positions for E4 benchmarking.
    ///
    /// This method embeds a collection of session chunks, passing the sequence
    /// position to E4 via `embed_all_with_metadata()`. This enables E4 to encode
    /// proper session ordering (not just timestamps).
    ///
    /// # Arguments
    ///
    /// * `sessions` - Sessions containing chunks with sequence positions
    ///
    /// # Returns
    ///
    /// Map of chunk UUID to fingerprint, where E4 encodings reflect sequence positions.
    #[cfg(feature = "real-embeddings")]
    pub async fn embed_with_sequences(
        &self,
        sessions: &[crate::datasets::TemporalSession],
    ) -> Result<HashMap<Uuid, SemanticFingerprint>, EmbedError> {
        use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};
        use std::time::Instant;

        // Initialize the warm provider (loads all 13 models to GPU)
        initialize_global_warm_provider()
            .await
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let provider = get_warm_provider().map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let total: usize = sessions.iter().map(|s| s.len()).sum();
        let mut fingerprints = HashMap::with_capacity(total);
        let start_time = Instant::now();
        let mut processed = 0;

        for session in sessions {
            for chunk in &session.chunks {
                if self.config.show_progress && processed % 100 == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let rate = if processed > 0 {
                        processed as f64 / elapsed
                    } else {
                        0.0
                    };
                    let eta = if rate > 0.0 {
                        (total - processed) as f64 / rate
                    } else {
                        0.0
                    };
                    eprint!(
                        "\rEmbedding sequences: {}/{} ({:.1}%) | {:.1} chunks/s | ETA: {:.0}s",
                        processed,
                        total,
                        processed as f64 / total as f64 * 100.0,
                        rate,
                        eta
                    );
                }

                // Create metadata with session sequence for E4
                let metadata = EmbeddingMetadata::with_sequence(
                    session.session_id.clone(),
                    chunk.sequence_position as u64,
                );

                // Embed using the warm provider with sequence metadata
                let output = provider
                    .embed_all_with_metadata(&chunk.text, metadata)
                    .await
                    .map_err(|e| EmbedError::Embedding(e.to_string()))?;

                fingerprints.insert(chunk.id, output.fingerprint);
                processed += 1;
            }
        }

        if self.config.show_progress {
            let elapsed = start_time.elapsed().as_secs_f64();
            let rate = processed as f64 / elapsed;
            eprintln!(
                "\rSequence embedding complete: {} chunks | {:.1} chunks/s | {:.1}s total",
                total, rate, elapsed
            );
        }

        Ok(fingerprints)
    }

    /// Embed session chunks with parallel batching for maximum throughput.
    ///
    /// This is an optimized version of `embed_with_sequences()` that processes
    /// multiple chunks in parallel batches while still preserving sequence metadata.
    #[cfg(feature = "real-embeddings")]
    pub async fn embed_sessions_batched(
        &self,
        sessions: &[crate::datasets::TemporalSession],
    ) -> Result<HashMap<Uuid, SemanticFingerprint>, EmbedError> {
        use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};
        use std::time::Instant;

        // Initialize the warm provider (loads all 13 models to GPU)
        initialize_global_warm_provider()
            .await
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let provider = get_warm_provider().map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        // Flatten sessions into (chunk, session_id, sequence) tuples
        let all_chunks: Vec<_> = sessions
            .iter()
            .flat_map(|session| {
                session
                    .chunks
                    .iter()
                    .map(move |chunk| (chunk, session.session_id.clone(), chunk.sequence_position))
            })
            .collect();

        let total = all_chunks.len();
        let batch_size = self.config.batch_size;
        let mut fingerprints = HashMap::with_capacity(total);
        let start_time = Instant::now();
        let mut processed = 0;

        // Process in batches
        for batch in all_chunks.chunks(batch_size) {
            // Embed each chunk in the batch
            for (chunk, session_id, sequence_pos) in batch {
                if self.config.show_progress && processed % 100 == 0 && processed > 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let rate = processed as f64 / elapsed;
                    let eta = (total - processed) as f64 / rate;
                    eprint!(
                        "\rEmbedding sessions: {}/{} ({:.1}%) | {:.1} chunks/s | ETA: {:.0}s",
                        processed,
                        total,
                        processed as f64 / total as f64 * 100.0,
                        rate,
                        eta
                    );
                }

                // Create metadata with session sequence for E4
                let metadata =
                    EmbeddingMetadata::with_sequence(session_id.clone(), *sequence_pos as u64);

                // Embed using the warm provider with sequence metadata
                let output = provider
                    .embed_all_with_metadata(&chunk.text, metadata)
                    .await
                    .map_err(|e| EmbedError::Embedding(e.to_string()))?;

                fingerprints.insert(chunk.id, output.fingerprint);
                processed += 1;
            }
        }

        if self.config.show_progress {
            let elapsed = start_time.elapsed().as_secs_f64();
            let rate = processed as f64 / elapsed;
            eprintln!(
                "\rSession embedding complete: {} chunks | {:.1} chunks/s | {:.1}s total",
                total, rate, elapsed
            );
        }

        Ok(fingerprints)
    }
}

impl Default for RealDataEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from embedding.
#[derive(Debug)]
pub enum EmbedError {
    /// Pipeline initialization error.
    Pipeline(String),
    /// Embedding error.
    Embedding(String),
    /// Checkpoint error.
    Checkpoint(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::Pipeline(e) => write!(f, "Pipeline error: {}", e),
            EmbedError::Checkpoint(e) => write!(f, "Checkpoint error: {}", e),
            EmbedError::Embedding(e) => write!(f, "Embedding error: {}", e),
        }
    }
}

impl std::error::Error for EmbedError {}
