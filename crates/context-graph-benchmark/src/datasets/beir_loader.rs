//! BEIR dataset loader for embedder impact benchmarks.
//!
//! Loads BEIR-format datasets (chunks.jsonl, queries.jsonl, qrels.json)
//! for retrieval benchmarking with ground truth relevance judgments.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A BEIR query with relevance judgments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeirQuery {
    /// Query ID.
    pub query_id: String,
    /// Query text.
    pub text: String,
    /// UUID for internal tracking.
    #[serde(skip)]
    pub uuid: Uuid,
}

/// A BEIR document chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeirChunk {
    /// Unique chunk ID (UUID format).
    pub id: String,
    /// Document ID.
    pub doc_id: String,
    /// Original document ID from BEIR.
    #[serde(default)]
    pub original_doc_id: String,
    /// Document title.
    pub title: String,
    /// Chunk index.
    pub chunk_idx: usize,
    /// Chunk text.
    pub text: String,
    /// Word count.
    pub word_count: usize,
    /// Topic hint.
    #[serde(default)]
    pub topic_hint: String,
    /// Source dataset.
    #[serde(default)]
    pub source_dataset: String,
}

impl BeirChunk {
    /// Get UUID from the string ID.
    pub fn uuid(&self) -> Uuid {
        Uuid::parse_str(&self.id).unwrap_or_else(|_| {
            let hash = md5::compute(self.id.as_bytes());
            Uuid::from_slice(&hash.0).unwrap_or(Uuid::nil())
        })
    }
}

/// BEIR relevance judgments (qrels).
/// Format: query_id -> doc_id -> relevance_score
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BeirQrels {
    /// Relevance judgments: query_id -> original_doc_id -> relevance
    pub judgments: HashMap<String, HashMap<String, i32>>,
}

impl BeirQrels {
    /// Get relevant documents for a query.
    pub fn get_relevant(&self, query_id: &str) -> HashSet<String> {
        self.judgments
            .get(query_id)
            .map(|docs| docs.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get relevance score for a query-document pair.
    pub fn get_relevance(&self, query_id: &str, doc_id: &str) -> Option<i32> {
        self.judgments
            .get(query_id)
            .and_then(|docs| docs.get(doc_id).copied())
    }

    /// Check if a document is relevant to a query.
    pub fn is_relevant(&self, query_id: &str, doc_id: &str) -> bool {
        self.get_relevance(query_id, doc_id).is_some_and(|r| r > 0)
    }

    /// Total number of relevance judgments.
    pub fn total_judgments(&self) -> usize {
        self.judgments.values().map(|d| d.len()).sum()
    }
}

/// BEIR dataset metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BeirMetadata {
    /// Total documents.
    pub total_documents: usize,
    /// Total chunks.
    pub total_chunks: usize,
    /// Total words.
    pub total_words: usize,
    /// Total queries.
    #[serde(default)]
    pub total_queries: usize,
    /// Total qrels.
    #[serde(default)]
    pub total_qrels: usize,
    /// Source dataset name.
    #[serde(default)]
    pub source: String,
    /// Expected NDCG@10 (from BEIR leaderboard).
    #[serde(default)]
    pub expected_ndcg10: f64,
    /// Expected MRR@10.
    #[serde(default)]
    pub expected_mrr10: f64,
    /// Expected MAP@10.
    #[serde(default)]
    pub expected_map10: f64,
}

/// Loaded BEIR dataset with queries and relevance judgments.
#[derive(Debug)]
pub struct BeirDataset {
    /// Document chunks.
    pub chunks: Vec<BeirChunk>,
    /// Queries.
    pub queries: Vec<BeirQuery>,
    /// Relevance judgments.
    pub qrels: BeirQrels,
    /// Metadata.
    pub metadata: BeirMetadata,
    /// Mapping from original doc ID to chunk UUIDs.
    pub doc_id_to_chunks: HashMap<String, Vec<Uuid>>,
    /// Mapping from chunk UUID to original doc ID.
    pub chunk_to_doc_id: HashMap<Uuid, String>,
}

impl BeirDataset {
    /// Get chunks for a document.
    pub fn chunks_for_doc(&self, original_doc_id: &str) -> Vec<&BeirChunk> {
        self.chunks
            .iter()
            .filter(|c| c.original_doc_id == original_doc_id)
            .collect()
    }

    /// Get relevant chunk UUIDs for a query.
    pub fn relevant_chunks_for_query(&self, query_id: &str) -> HashSet<Uuid> {
        let relevant_docs = self.qrels.get_relevant(query_id);
        let mut relevant_chunks = HashSet::new();

        for doc_id in relevant_docs {
            if let Some(chunk_uuids) = self.doc_id_to_chunks.get(&doc_id) {
                relevant_chunks.extend(chunk_uuids.iter().copied());
            }
        }

        relevant_chunks
    }

    /// Get number of queries with relevance judgments.
    pub fn queries_with_judgments(&self) -> usize {
        self.queries
            .iter()
            .filter(|q| self.qrels.judgments.contains_key(&q.query_id))
            .count()
    }

    /// Sample queries with relevance judgments.
    pub fn sample_queries(&self, n: usize, seed: u64) -> Vec<&BeirQuery> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        // Filter to queries with judgments
        let mut valid_queries: Vec<_> = self
            .queries
            .iter()
            .filter(|q| self.qrels.judgments.contains_key(&q.query_id))
            .collect();

        valid_queries.shuffle(&mut rng);
        valid_queries.truncate(n);
        valid_queries
    }
}

/// BEIR dataset loader.
pub struct BeirLoader {
    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,
    /// Maximum queries to load (0 = unlimited).
    pub max_queries: usize,
}

impl BeirLoader {
    /// Create a new loader.
    pub fn new() -> Self {
        Self {
            max_chunks: 0,
            max_queries: 0,
        }
    }

    /// Set maximum chunks to load.
    pub fn with_max_chunks(mut self, max: usize) -> Self {
        self.max_chunks = max;
        self
    }

    /// Set maximum queries to load.
    pub fn with_max_queries(mut self, max: usize) -> Self {
        self.max_queries = max;
        self
    }

    /// Load BEIR dataset from directory.
    pub fn load_from_dir<P: AsRef<Path>>(&self, dir: P) -> Result<BeirDataset, BeirLoadError> {
        let dir = dir.as_ref();

        // Load metadata
        let metadata = self.load_metadata(dir.join("metadata.json"))?;

        // Load chunks
        let chunks = self.load_chunks(dir.join("chunks.jsonl"))?;

        // Load queries
        let queries = self.load_queries(dir.join("queries.jsonl"))?;

        // Load qrels
        let qrels = self.load_qrels(dir.join("qrels.json"))?;

        // Build doc ID mappings
        let mut doc_id_to_chunks: HashMap<String, Vec<Uuid>> = HashMap::new();
        let mut chunk_to_doc_id: HashMap<Uuid, String> = HashMap::new();

        for chunk in &chunks {
            let uuid = chunk.uuid();
            let doc_id = if !chunk.original_doc_id.is_empty() {
                chunk.original_doc_id.clone()
            } else {
                chunk.doc_id.clone()
            };

            doc_id_to_chunks
                .entry(doc_id.clone())
                .or_default()
                .push(uuid);
            chunk_to_doc_id.insert(uuid, doc_id);
        }

        Ok(BeirDataset {
            chunks,
            queries,
            qrels,
            metadata,
            doc_id_to_chunks,
            chunk_to_doc_id,
        })
    }

    /// Load metadata.
    fn load_metadata<P: AsRef<Path>>(&self, path: P) -> Result<BeirMetadata, BeirLoadError> {
        let file = File::open(path.as_ref()).map_err(|e| BeirLoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        serde_json::from_reader(file).map_err(|e| BeirLoadError::Json {
            path: path.as_ref().to_path_buf(),
            error: e,
        })
    }

    /// Load chunks from JSONL.
    fn load_chunks<P: AsRef<Path>>(&self, path: P) -> Result<Vec<BeirChunk>, BeirLoadError> {
        let file = File::open(path.as_ref()).map_err(|e| BeirLoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        let reader = BufReader::new(file);
        let mut chunks = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            if self.max_chunks > 0 && chunks.len() >= self.max_chunks {
                break;
            }

            let line = line_result.map_err(|e| BeirLoadError::Io {
                path: path.as_ref().to_path_buf(),
                error: e,
            })?;

            let chunk: BeirChunk =
                serde_json::from_str(&line).map_err(|e| BeirLoadError::JsonLine {
                    path: path.as_ref().to_path_buf(),
                    line: line_num + 1,
                    error: e,
                })?;

            chunks.push(chunk);
        }

        Ok(chunks)
    }

    /// Load queries from JSONL.
    fn load_queries<P: AsRef<Path>>(&self, path: P) -> Result<Vec<BeirQuery>, BeirLoadError> {
        let file = File::open(path.as_ref()).map_err(|e| BeirLoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        let reader = BufReader::new(file);
        let mut queries = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            if self.max_queries > 0 && queries.len() >= self.max_queries {
                break;
            }

            let line = line_result.map_err(|e| BeirLoadError::Io {
                path: path.as_ref().to_path_buf(),
                error: e,
            })?;

            let mut query: BeirQuery =
                serde_json::from_str(&line).map_err(|e| BeirLoadError::JsonLine {
                    path: path.as_ref().to_path_buf(),
                    line: line_num + 1,
                    error: e,
                })?;

            // Generate UUID for query
            let hash = md5::compute(query.query_id.as_bytes());
            query.uuid = Uuid::from_slice(&hash.0).unwrap_or(Uuid::nil());

            queries.push(query);
        }

        Ok(queries)
    }

    /// Load qrels from JSON.
    fn load_qrels<P: AsRef<Path>>(&self, path: P) -> Result<BeirQrels, BeirLoadError> {
        let file = File::open(path.as_ref()).map_err(|e| BeirLoadError::Io {
            path: path.as_ref().to_path_buf(),
            error: e,
        })?;

        let judgments: HashMap<String, HashMap<String, i32>> = serde_json::from_reader(file)
            .map_err(|e| BeirLoadError::Json {
                path: path.as_ref().to_path_buf(),
                error: e,
            })?;

        Ok(BeirQrels { judgments })
    }
}

impl Default for BeirLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors during BEIR dataset loading.
#[derive(Debug)]
pub enum BeirLoadError {
    /// IO error.
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

impl std::fmt::Display for BeirLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BeirLoadError::Io { path, error } => {
                write!(f, "IO error reading {}: {}", path.display(), error)
            }
            BeirLoadError::Json { path, error } => {
                write!(f, "JSON error in {}: {}", path.display(), error)
            }
            BeirLoadError::JsonLine { path, line, error } => {
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

impl std::error::Error for BeirLoadError {}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_beir_qrels() {
        let mut judgments = HashMap::new();
        let mut docs = HashMap::new();
        docs.insert("doc1".to_string(), 1);
        docs.insert("doc2".to_string(), 2);
        judgments.insert("q1".to_string(), docs);

        let qrels = BeirQrels { judgments };

        assert!(qrels.is_relevant("q1", "doc1"));
        assert!(qrels.is_relevant("q1", "doc2"));
        assert!(!qrels.is_relevant("q1", "doc3"));
        assert!(!qrels.is_relevant("q2", "doc1"));

        assert_eq!(qrels.get_relevance("q1", "doc1"), Some(1));
        assert_eq!(qrels.get_relevance("q1", "doc2"), Some(2));
        assert_eq!(qrels.total_judgments(), 2);
    }
}
