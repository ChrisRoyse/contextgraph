//! Dataset loader for pre-processed real data.
//!
//! Loads chunks from JSONL files prepared by the Python script.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use uuid::Uuid;

/// A single text chunk from the dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    /// Unique chunk ID (deterministic based on doc_id + chunk_idx).
    pub id: String,
    /// Source document ID.
    pub doc_id: String,
    /// Document title.
    pub title: String,
    /// Chunk index within document.
    pub chunk_idx: usize,
    /// Chunk text content.
    pub text: String,
    /// Word count.
    pub word_count: usize,
    /// Start word index in original document.
    pub start_word: usize,
    /// End word index in original document.
    pub end_word: usize,
    /// Topic hint (rough categorization).
    pub topic_hint: String,
    /// Source dataset name (e.g., "arxiv", "stackoverflow", "wikipedia").
    /// Used for multi-dataset benchmarks.
    #[serde(default)]
    pub source_dataset: Option<String>,
}

impl ChunkRecord {
    /// Get UUID from the string ID.
    pub fn uuid(&self) -> Uuid {
        // Parse the UUID-formatted string
        Uuid::parse_str(&self.id).unwrap_or_else(|_| {
            // Fallback: generate from hash
            let hash = md5::compute(self.id.as_bytes());
            Uuid::from_slice(&hash.0).unwrap_or(Uuid::nil())
        })
    }
}

/// Metadata about the loaded dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Total number of source documents.
    pub total_documents: usize,
    /// Total number of chunks.
    pub total_chunks: usize,
    /// Total word count.
    pub total_words: usize,
    /// Number of short documents skipped.
    #[serde(default)]
    pub skipped_short: usize,
    /// Chunk size in words.
    pub chunk_size: usize,
    /// Overlap in words.
    pub overlap: usize,
    /// Data source (single source for backward compat).
    #[serde(default)]
    pub source: String,
    /// Source datasets (for multi-dataset benchmarks).
    #[serde(default)]
    pub source_datasets: Vec<String>,
    /// Per-dataset statistics.
    #[serde(default)]
    pub dataset_stats: HashMap<String, DatasetStats>,
    /// Top topics for ground truth.
    #[serde(default)]
    pub top_topics: Vec<String>,
    /// Topic counts.
    #[serde(default)]
    pub topic_counts: HashMap<String, usize>,
}

/// Statistics for a single source dataset.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatasetStats {
    /// Documents from this source.
    pub documents: usize,
    /// Chunks from this source.
    pub chunks: usize,
    /// Words from this source.
    pub words: usize,
    /// Documents skipped (too short).
    #[serde(default)]
    pub skipped_short: usize,
    /// Documents skipped (empty).
    #[serde(default)]
    pub skipped_empty: usize,
}

/// A loaded real dataset ready for benchmarking.
#[derive(Debug)]
pub struct RealDataset {
    /// All chunks.
    pub chunks: Vec<ChunkRecord>,
    /// Metadata.
    pub metadata: DatasetMetadata,
    /// Topic to index mapping (for ground truth).
    pub topic_to_idx: HashMap<String, usize>,
    /// Source dataset to index mapping.
    pub source_to_idx: HashMap<String, usize>,
}

impl RealDataset {
    /// Get the topic index for a chunk (for clustering ground truth).
    pub fn get_topic_idx(&self, chunk: &ChunkRecord) -> usize {
        self.topic_to_idx
            .get(&chunk.topic_hint)
            .copied()
            .unwrap_or(0) // Default to topic 0 for unknown
    }

    /// Get the source dataset index for a chunk.
    pub fn get_source_idx(&self, chunk: &ChunkRecord) -> usize {
        chunk
            .source_dataset
            .as_ref()
            .and_then(|s| self.source_to_idx.get(s))
            .copied()
            .unwrap_or(0)
    }

    /// Get topic assignments for all chunks.
    pub fn topic_assignments(&self) -> HashMap<Uuid, usize> {
        self.chunks
            .iter()
            .map(|c| (c.uuid(), self.get_topic_idx(c)))
            .collect()
    }

    /// Get source dataset assignments for all chunks.
    pub fn source_assignments(&self) -> HashMap<Uuid, usize> {
        self.chunks
            .iter()
            .map(|c| (c.uuid(), self.get_source_idx(c)))
            .collect()
    }

    /// Get number of unique topics.
    pub fn topic_count(&self) -> usize {
        self.topic_to_idx.len()
    }

    /// Get number of source datasets.
    pub fn source_count(&self) -> usize {
        self.source_to_idx.len()
    }

    /// Get list of source dataset names.
    pub fn source_names(&self) -> Vec<&str> {
        let mut sources: Vec<_> = self.source_to_idx.keys().map(|s| s.as_str()).collect();
        sources.sort();
        sources
    }

    /// Get chunks from a specific source dataset.
    pub fn chunks_by_source(&self, source: &str) -> Vec<&ChunkRecord> {
        self.chunks
            .iter()
            .filter(|c| c.source_dataset.as_deref() == Some(source))
            .collect()
    }

    /// Sample a subset of chunks for smaller-scale testing.
    pub fn sample(&self, n: usize, seed: u64) -> Vec<&ChunkRecord> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut indices: Vec<usize> = (0..self.chunks.len()).collect();
        indices.shuffle(&mut rng);
        indices.truncate(n);
        indices.sort(); // Maintain some order for reproducibility

        indices.iter().map(|&i| &self.chunks[i]).collect()
    }

    /// Sample chunks stratified by source dataset.
    pub fn sample_stratified(&self, n_per_source: usize, seed: u64) -> Vec<&ChunkRecord> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut result = Vec::new();

        for source in self.source_names() {
            let source_chunks: Vec<_> = self.chunks_by_source(source);
            let mut indices: Vec<usize> = (0..source_chunks.len()).collect();
            indices.shuffle(&mut rng);
            indices.truncate(n_per_source);

            for idx in indices {
                result.push(source_chunks[idx]);
            }
        }

        result
    }
}

/// Loader for real datasets.
pub struct DatasetLoader {
    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,
    /// Maximum topics to track.
    pub max_topics: usize,
}

impl DatasetLoader {
    /// Create a new loader with default settings.
    pub fn new() -> Self {
        Self {
            max_chunks: 0,
            max_topics: 100,
        }
    }

    /// Set maximum chunks to load.
    pub fn with_max_chunks(mut self, max: usize) -> Self {
        self.max_chunks = max;
        self
    }

    /// Set maximum topics to track.
    pub fn with_max_topics(mut self, max: usize) -> Self {
        self.max_topics = max;
        self
    }

    /// Load dataset from a directory containing chunks.jsonl and metadata.json.
    pub fn load_from_dir<P: AsRef<Path>>(&self, dir: P) -> Result<RealDataset, LoadError> {
        let dir = dir.as_ref();
        let chunks_path = dir.join("chunks.jsonl");
        let metadata_path = dir.join("metadata.json");

        // Load metadata
        let metadata = self.load_metadata(&metadata_path)?;

        // Load chunks
        let chunks = self.load_chunks(&chunks_path)?;

        // Build topic index
        let topic_to_idx = self.build_topic_index(&metadata, &chunks);

        // Build source index
        let source_to_idx = self.build_source_index(&metadata, &chunks);

        Ok(RealDataset {
            chunks,
            metadata,
            topic_to_idx,
            source_to_idx,
        })
    }

    /// Load metadata from JSON file.
    fn load_metadata<P: AsRef<Path>>(&self, path: P) -> Result<DatasetMetadata, LoadError> {
        let file = File::open(path.as_ref()).map_err(|e| LoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        serde_json::from_reader(file).map_err(|e| LoadError::Json {
            path: path.as_ref().to_path_buf(),
            error: e,
        })
    }

    /// Load chunks from JSONL file.
    fn load_chunks<P: AsRef<Path>>(&self, path: P) -> Result<Vec<ChunkRecord>, LoadError> {
        let file = File::open(path.as_ref()).map_err(|e| LoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        let reader = BufReader::new(file);
        let mut chunks = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            if self.max_chunks > 0 && chunks.len() >= self.max_chunks {
                break;
            }

            let line = line_result.map_err(|e| LoadError::Io {
                path: path.as_ref().to_path_buf(),
                error: e,
            })?;

            let chunk: ChunkRecord =
                serde_json::from_str(&line).map_err(|e| LoadError::JsonLine {
                    path: path.as_ref().to_path_buf(),
                    line: line_num + 1,
                    error: e,
                })?;

            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Build topic to index mapping.
    fn build_topic_index(
        &self,
        metadata: &DatasetMetadata,
        chunks: &[ChunkRecord],
    ) -> HashMap<String, usize> {
        let mut topic_to_idx = HashMap::new();

        // Use top topics from metadata if available
        if !metadata.top_topics.is_empty() {
            for (idx, topic) in metadata.top_topics.iter().take(self.max_topics).enumerate() {
                topic_to_idx.insert(topic.clone(), idx);
            }
        } else {
            // Build from chunks
            let mut topic_counts: HashMap<String, usize> = HashMap::new();
            for chunk in chunks {
                *topic_counts.entry(chunk.topic_hint.clone()).or_default() += 1;
            }

            let mut sorted: Vec<_> = topic_counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));

            for (idx, (topic, _)) in sorted.into_iter().take(self.max_topics).enumerate() {
                topic_to_idx.insert(topic, idx);
            }
        }

        topic_to_idx
    }

    /// Build source dataset to index mapping.
    fn build_source_index(
        &self,
        metadata: &DatasetMetadata,
        chunks: &[ChunkRecord],
    ) -> HashMap<String, usize> {
        let mut source_to_idx = HashMap::new();

        // Use source_datasets from metadata if available
        if !metadata.source_datasets.is_empty() {
            for (idx, source) in metadata.source_datasets.iter().enumerate() {
                source_to_idx.insert(source.clone(), idx);
            }
        } else {
            // Build from chunks
            let mut sources: Vec<String> = chunks
                .iter()
                .filter_map(|c| c.source_dataset.clone())
                .collect();
            sources.sort();
            sources.dedup();

            for (idx, source) in sources.into_iter().enumerate() {
                source_to_idx.insert(source, idx);
            }
        }

        // Add default source if none found (for backward compat with single-source datasets)
        if source_to_idx.is_empty() && !metadata.source.is_empty() {
            source_to_idx.insert(metadata.source.clone(), 0);
        }

        source_to_idx
    }
}

impl Default for DatasetLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during dataset loading.
#[derive(Debug)]
pub enum LoadError {
    /// IO error reading file.
    Io {
        path: std::path::PathBuf,
        error: std::io::Error,
    },
    /// JSON parsing error.
    Json {
        path: std::path::PathBuf,
        error: serde_json::Error,
    },
    /// JSON parsing error on specific line.
    JsonLine {
        path: std::path::PathBuf,
        line: usize,
        error: serde_json::Error,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io { path, error } => {
                write!(f, "IO error reading {}: {}", path.display(), error)
            }
            LoadError::Json { path, error } => {
                write!(f, "JSON error in {}: {}", path.display(), error)
            }
            LoadError::JsonLine { path, line, error } => {
                write!(
                    f,
                    "JSON error in {} at line {}: {}",
                    path.display(),
                    line,
                    error
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dataset(dir: &Path, n_chunks: usize) {
        // Create metadata
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

        // Create chunks
        let chunks_path = dir.join("chunks.jsonl");
        let mut f = File::create(&chunks_path).unwrap();

        for i in 0..n_chunks {
            let chunk = ChunkRecord {
                id: format!(
                    "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                    i, i, i, i, i
                ),
                doc_id: format!("doc_{}", i / 2),
                title: format!("Test Document {}", i / 2),
                chunk_idx: i % 2,
                text: format!("This is test chunk {} with some sample text.", i),
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
    fn test_load_dataset() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 10);

        let loader = DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        assert_eq!(dataset.chunks.len(), 10);
        assert_eq!(dataset.metadata.total_chunks, 10);
        assert_eq!(dataset.topic_count(), 2);
    }

    #[test]
    fn test_load_with_max_chunks() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 100);

        let loader = DatasetLoader::new().with_max_chunks(50);
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        assert_eq!(dataset.chunks.len(), 50);
    }

    #[test]
    fn test_sample_chunks() {
        let dir = TempDir::new().unwrap();
        create_test_dataset(dir.path(), 100);

        let loader = DatasetLoader::new();
        let dataset = loader.load_from_dir(dir.path()).unwrap();

        let sample = dataset.sample(10, 42);
        assert_eq!(sample.len(), 10);

        // Same seed should give same sample
        let sample2 = dataset.sample(10, 42);
        assert_eq!(
            sample.iter().map(|c| &c.id).collect::<Vec<_>>(),
            sample2.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }
}
