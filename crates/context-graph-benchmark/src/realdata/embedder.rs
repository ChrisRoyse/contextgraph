//! Embedder for real dataset chunks.
//!
//! Generates 13-embedder fingerprints from text chunks.

use std::collections::HashMap;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;
use rand::Rng;

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
    pub async fn embed_dataset(&self, dataset: &RealDataset) -> Result<EmbeddedDataset, EmbedError> {
        use context_graph_embeddings::{get_warm_provider, initialize_global_warm_provider};

        // Initialize the warm provider (loads all 13 models to GPU)
        initialize_global_warm_provider()
            .await
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let provider = get_warm_provider()
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let total = dataset.chunks.len();
        let mut fingerprints = HashMap::with_capacity(total);
        let mut chunk_info = HashMap::with_capacity(total);

        for (i, chunk) in dataset.chunks.iter().enumerate() {
            if self.config.show_progress && i % 100 == 0 {
                eprint!("\rEmbedding: {}/{} chunks ({:.1}%)", i, total, i as f64 / total as f64 * 100.0);
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

        let provider = get_warm_provider()
            .map_err(|e| EmbedError::Pipeline(e.to_string()))?;

        let total = dataset.chunks.len();
        let batch_size = self.config.batch_size;
        let checkpoint_interval = if checkpoint_interval == 0 { 1000 } else { checkpoint_interval };

        // Try to load existing checkpoint
        let (mut fingerprints, mut chunk_info, start_idx) = if let Some(dir) = checkpoint_dir {
            match EmbeddingCheckpoint::load(dir) {
                Ok(checkpoint) => {
                    if self.config.show_progress {
                        eprintln!("Resuming from checkpoint: {} embeddings", checkpoint.embedded_count);
                    }
                    (checkpoint.fingerprints, checkpoint.chunk_info, checkpoint.embedded_count)
                }
                Err(_) => (HashMap::with_capacity(total), HashMap::with_capacity(total), 0),
            }
        } else {
            (HashMap::with_capacity(total), HashMap::with_capacity(total), 0)
        };

        let start_time = Instant::now();
        let mut last_checkpoint_time = Instant::now();
        let mut embeddings_since_checkpoint = 0;

        // Process in batches
        let chunks_to_process: Vec<_> = dataset.chunks.iter().skip(start_idx).collect();
        let num_batches = (chunks_to_process.len() + batch_size - 1) / batch_size;

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
                        global_idx, total,
                        global_idx as f64 / total as f64 * 100.0,
                        rate, eta
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

    /// Embed a dataset using synthetic fingerprints (for testing without GPU).
    ///
    /// This generates deterministic pseudo-random fingerprints based on text hash.
    /// Useful for testing the benchmark infrastructure without real embeddings.
    pub fn embed_dataset_synthetic(
        &self,
        dataset: &RealDataset,
        seed: u64,
    ) -> Result<EmbeddedDataset, EmbedError> {
        use context_graph_core::types::fingerprint::{
            E11_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E7_DIM, E8_DIM, E9_DIM,
        };
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;
        use rand_distr::Normal;

        let total = dataset.chunks.len();
        let mut fingerprints = HashMap::with_capacity(total);
        let mut chunk_info = HashMap::with_capacity(total);

        // Create base RNG from seed
        let mut base_rng = ChaCha8Rng::seed_from_u64(seed);
        let normal = Normal::new(0.0, 1.0).unwrap();

        // Generate topic centroids for more realistic clustering
        let topic_centroids: Vec<Vec<f32>> = (0..dataset.topic_count())
            .map(|_| {
                let mut centroid: Vec<f32> = (0..E1_DIM).map(|_| normal.sample(&mut base_rng) as f32).collect();
                normalize(&mut centroid);
                centroid
            })
            .collect();

        for (i, chunk) in dataset.chunks.iter().enumerate() {
            if self.config.show_progress && i % 1000 == 0 {
                eprint!(
                    "\rGenerating synthetic fingerprints: {}/{} ({:.1}%)",
                    i,
                    total,
                    i as f64 / total as f64 * 100.0
                );
            }

            // Create chunk-specific RNG from text hash
            let text_hash = hash_text(&chunk.text);
            let mut rng = ChaCha8Rng::seed_from_u64(seed ^ text_hash);

            // Get topic centroid
            let topic_idx = dataset.get_topic_idx(chunk);
            let centroid = &topic_centroids[topic_idx % topic_centroids.len()];

            // Generate E1 (semantic) - biased towards topic centroid
            let mut e1: Vec<f32> = centroid
                .iter()
                .map(|&c| c + normal.sample(&mut rng) as f32 * 0.3)
                .collect();
            normalize(&mut e1);

            // Generate other embeddings
            let e2 = random_normalized(E2_DIM, &mut rng);
            let e3 = random_normalized(E3_DIM, &mut rng);
            let e4 = random_normalized(E4_DIM, &mut rng);

            // E5 (causal) - some correlation with E1
            let mut e5: Vec<f32> = (0..768)
                .map(|j| {
                    let base = if j < E1_DIM { e1[j] * 0.5 } else { 0.0 };
                    base + normal.sample(&mut rng) as f32 * 0.5
                })
                .collect();
            normalize(&mut e5);

            let e7 = random_normalized(E7_DIM, &mut rng);
            let e8 = random_normalized(E8_DIM, &mut rng);

            // E9 (HDC) - binary-like
            let e9: Vec<f32> = (0..E9_DIM)
                .map(|_| if rng.gen::<bool>() { 1.0 } else { -1.0 })
                .collect();

            // E10 (multimodal) - some correlation with E1
            let mut e10: Vec<f32> = (0..768)
                .map(|j| {
                    let base = if j < E1_DIM { e1[j] * 0.3 } else { 0.0 };
                    base + normal.sample(&mut rng) as f32 * 0.7
                })
                .collect();
            normalize(&mut e10);

            let e11 = random_normalized(E11_DIM, &mut rng);

            // E12 (late interaction) - per-token embeddings
            let n_tokens = (chunk.word_count / 2).max(5).min(50);
            let e12: Vec<Vec<f32>> = (0..n_tokens)
                .map(|_| random_normalized(128, &mut rng))
                .collect();

            // E6 and E13 (sparse)
            let e6 = random_sparse(30000, 100, &mut rng);
            let e13 = random_sparse(30000, 100, &mut rng);

            let fp = SemanticFingerprint {
                e1_semantic: e1,
                e2_temporal_recent: e2,
                e3_temporal_periodic: e3,
                e4_temporal_positional: e4,
                e5_causal_as_cause: e5.clone(),
                e5_causal_as_effect: e5,
                e5_causal: Vec::new(), // Empty - using new dual format
                e6_sparse: e6,
                e7_code: e7,
                e8_graph: e8,
                e9_hdc: e9,
                e10_multimodal: e10,
                e11_entity: e11,
                e12_late_interaction: e12,
                e13_splade: e13,
            };

            let uuid = chunk.uuid();
            fingerprints.insert(uuid, fp);
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
            eprintln!(
                "\rGenerating synthetic fingerprints: {}/{} (100.0%)",
                total, total
            );
        }

        Ok(EmbeddedDataset {
            fingerprints,
            topic_assignments: dataset.topic_assignments(),
            chunk_info,
            topic_count: dataset.topic_count(),
        })
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

// Helper functions

fn hash_text(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}

fn random_normalized(dim: usize, rng: &mut impl Rng) -> Vec<f32> {
    use rand_distr::{Distribution, Normal};
    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut vec: Vec<f32> = (0..dim).map(|_| normal.sample(rng) as f32).collect();
    normalize(&mut vec);
    vec
}

fn random_sparse(
    vocab_size: usize,
    n_entries: usize,
    rng: &mut impl Rng,
) -> context_graph_core::types::fingerprint::SparseVector {
    use context_graph_core::types::fingerprint::SparseVector;
    use rand::seq::SliceRandom;
    use rand_distr::{Distribution, Normal};

    let normal = Normal::new(0.0, 1.0).unwrap();

    let mut indices: Vec<u16> = (0..vocab_size as u16).collect();
    indices.shuffle(rng);
    indices.truncate(n_entries);
    indices.sort();

    let values: Vec<f32> = (0..n_entries)
        .map(|_| normal.sample(rng) as f32)
        .collect();

    SparseVector { indices, values }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realdata::loader::DatasetLoader;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset(dir: &std::path::Path, n_chunks: usize) {
        use crate::realdata::loader::{ChunkRecord, DatasetMetadata};

        let metadata = DatasetMetadata {
            total_documents: n_chunks / 2,
            total_chunks: n_chunks,
            total_words: n_chunks * 200,
            skipped_short: 0,
            chunk_size: 200,
            overlap: 50,
            source: "test".to_string(),
            source_datasets: vec!["test".to_string()],
            dataset_stats: HashMap::new(),
            top_topics: vec!["science".to_string(), "history".to_string()],
            topic_counts: HashMap::new(),
        };

        let metadata_path = dir.join("metadata.json");
        let mut f = File::create(&metadata_path).unwrap();
        serde_json::to_writer(&mut f, &metadata).unwrap();

        let chunks_path = dir.join("chunks.jsonl");
        let mut f = File::create(&chunks_path).unwrap();

        for i in 0..n_chunks {
            let chunk = ChunkRecord {
                id: format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", i, i, i, i, i),
                doc_id: format!("doc_{}", i / 2),
                title: format!("Test Document {}", i / 2),
                chunk_idx: i % 2,
                text: format!("This is test chunk {} with some sample text content.", i),
                word_count: 10,
                start_word: (i % 2) * 200,
                end_word: (i % 2) * 200 + 200,
                topic_hint: if i % 2 == 0 { "science" } else { "history" }.to_string(),
                source_dataset: Some("test".to_string()),
            };
            writeln!(f, "{}", serde_json::to_string(&chunk).unwrap()).unwrap();
        }
    }

    #[test]
    fn test_synthetic_embedding() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 20);

        let loader = DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        let embedder = RealDataEmbedder::new();
        let embedded = embedder.embed_dataset_synthetic(&dataset, 42).unwrap();

        assert_eq!(embedded.fingerprints.len(), 20);
        assert_eq!(embedded.topic_count, 2);

        // Check fingerprint dimensions
        let (_, fp) = embedded.fingerprints.iter().next().unwrap();
        assert_eq!(fp.e1_semantic.len(), 1024);
        // E5 uses dual vectors (cause + effect) for asymmetric causal similarity
        assert_eq!(fp.e5_causal_as_cause.len(), 768);
        assert_eq!(fp.e5_causal_as_effect.len(), 768);
        assert!(fp.e5_causal.is_empty(), "Legacy e5_causal should be empty");
        assert_eq!(fp.e7_code.len(), 1536);
    }

    #[test]
    fn test_synthetic_embedding_reproducibility() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 10);

        let loader = DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        let embedder = RealDataEmbedder::new();

        let embedded1 = embedder.embed_dataset_synthetic(&dataset, 42).unwrap();
        let embedded2 = embedder.embed_dataset_synthetic(&dataset, 42).unwrap();

        // Same seed should give same results
        for (uuid, fp1) in &embedded1.fingerprints {
            let fp2 = embedded2.fingerprints.get(uuid).unwrap();
            assert_eq!(fp1.e1_semantic, fp2.e1_semantic);
        }
    }
}
