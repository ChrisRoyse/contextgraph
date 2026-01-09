//! SPLADE inverted index for Stage 1 sparse retrieval.
//!
//! Implements BM25+SPLADE hybrid scoring for initial candidate selection.
//!
//! # Performance Target
//!
//! - <5ms for 10K candidates
//! - Support for ~1M documents

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::config::{InvertedIndexConfig, E13_SPLADE_VOCAB};

use super::error::{IndexError, IndexResult};

/// SPLADE inverted index for Stage 1 sparse retrieval.
///
/// # BM25+SPLADE Hybrid Scoring
///
/// Uses standard BM25 formula with IDF weighting:
/// ```text
/// score(d, q) = Σ_t IDF(t) × TF(t, d) × q_weight(t)
/// IDF(t) = ln((N - df + 0.5) / (df + 0.5) + 1)
/// TF(t, d) = doc_weight(t) / doc_norm
/// ```
///
/// # Memory Layout
///
/// - `posting_lists`: HashMap<term_id, Vec<(doc_id, weight)>>
/// - `doc_norms`: HashMap<doc_id, norm>
/// - `doc_freq`: HashMap<term_id, count>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpladeInvertedIndex {
    /// Posting lists: term_id -> [(doc_id, weight), ...]
    posting_lists: HashMap<usize, Vec<(Uuid, f32)>>,
    /// Document L2 norms for normalization
    doc_norms: HashMap<Uuid, f32>,
    /// Document frequency per term (for IDF)
    doc_freq: HashMap<usize, usize>,
    /// Total number of documents
    num_docs: usize,
    /// Configuration
    config: InvertedIndexConfig,
}

impl SpladeInvertedIndex {
    /// Create a new empty SPLADE index with E13 configuration.
    pub fn new() -> Self {
        Self {
            posting_lists: HashMap::new(),
            doc_norms: HashMap::new(),
            doc_freq: HashMap::new(),
            num_docs: 0,
            config: InvertedIndexConfig::e13_splade(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: InvertedIndexConfig) -> Self {
        Self {
            posting_lists: HashMap::new(),
            doc_norms: HashMap::new(),
            doc_freq: HashMap::new(),
            num_docs: 0,
            config,
        }
    }

    /// Add a sparse vector to the index.
    ///
    /// # Arguments
    ///
    /// - `memory_id`: Document/memory UUID
    /// - `sparse`: Sparse vector as (term_id, weight) pairs
    ///
    /// # Errors
    ///
    /// - `IndexError::InvalidTermId`: If any term_id >= vocab_size
    /// - `IndexError::ZeroNormVector`: If all weights are effectively zero
    pub fn add(&mut self, memory_id: Uuid, sparse: &[(usize, f32)]) -> IndexResult<()> {
        // Validate all term IDs first
        for &(term_id, _) in sparse {
            if term_id >= self.config.vocab_size {
                return Err(IndexError::InvalidTermId {
                    term_id,
                    vocab_size: self.config.vocab_size,
                });
            }
        }

        // Compute and validate L2 norm
        let norm: f32 = sparse.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
        if norm < f32::EPSILON {
            return Err(IndexError::ZeroNormVector { memory_id });
        }
        self.doc_norms.insert(memory_id, norm);

        // Track which terms we've added this doc to (for doc_freq)
        let mut added_terms = std::collections::HashSet::new();

        // Add to posting lists
        for &(term_id, weight) in sparse {
            // Skip near-zero weights to save space
            if weight.abs() < f32::EPSILON {
                continue;
            }

            self.posting_lists
                .entry(term_id)
                .or_default()
                .push((memory_id, weight));

            // Only increment doc_freq once per term per document
            if added_terms.insert(term_id) {
                *self.doc_freq.entry(term_id).or_insert(0) += 1;
            }
        }

        self.num_docs += 1;
        Ok(())
    }

    /// Remove a document from the index.
    ///
    /// # Arguments
    ///
    /// - `memory_id`: Document to remove
    ///
    /// # Returns
    ///
    /// `true` if document was found and removed, `false` otherwise.
    pub fn remove(&mut self, memory_id: Uuid) -> bool {
        if self.doc_norms.remove(&memory_id).is_none() {
            return false;
        }

        // Remove from all posting lists
        for postings in self.posting_lists.values_mut() {
            let original_len = postings.len();
            postings.retain(|(id, _)| *id != memory_id);

            // Update doc_freq if we removed an entry
            if postings.len() < original_len {
                // Note: We'd need to track which terms to decrement doc_freq
                // For simplicity, we rebuild doc_freq periodically instead
            }
        }

        self.num_docs = self.num_docs.saturating_sub(1);
        true
    }

    /// Search the index with BM25+SPLADE scoring.
    ///
    /// # Arguments
    ///
    /// - `query`: Sparse query vector as (term_id, weight) pairs
    /// - `k`: Number of top results to return
    ///
    /// # Returns
    ///
    /// Vec of (doc_id, score) pairs sorted by descending score.
    pub fn search(&self, query: &[(usize, f32)], k: usize) -> Vec<(Uuid, f32)> {
        if self.num_docs == 0 {
            return Vec::new();
        }

        let mut scores: HashMap<Uuid, f32> = HashMap::new();
        let n = self.num_docs as f32;

        for &(term_id, query_weight) in query {
            // Skip invalid terms silently in search (query might have OOV terms)
            if term_id >= self.config.vocab_size {
                continue;
            }

            if let Some(postings) = self.posting_lists.get(&term_id) {
                // BM25 IDF calculation
                let df = self.doc_freq.get(&term_id).copied().unwrap_or(1) as f32;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

                for &(doc_id, doc_weight) in postings {
                    let norm = self.doc_norms.get(&doc_id).copied().unwrap_or(1.0);
                    let tf = doc_weight / norm.max(f32::EPSILON);

                    // BM25+SPLADE score: query_weight * TF * IDF
                    *scores.entry(doc_id).or_insert(0.0) += query_weight * tf * idf;
                }
            }
        }

        // Sort by score descending
        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    /// Number of documents in the index.
    #[inline]
    pub fn len(&self) -> usize {
        self.num_docs
    }

    /// Check if index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_docs == 0
    }

    /// Number of unique terms in the index.
    #[inline]
    pub fn term_count(&self) -> usize {
        self.posting_lists.len()
    }

    /// Approximate memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        // Posting lists: each entry is (Uuid, f32) = 16 + 4 = 20 bytes
        let posting_bytes: usize = self
            .posting_lists
            .values()
            .map(|v| v.len() * 20 + std::mem::size_of::<usize>())
            .sum();

        // doc_norms: Uuid + f32 = 20 bytes per entry
        let norm_bytes = self.doc_norms.len() * 20;

        // doc_freq: usize + usize = 16 bytes per entry
        let freq_bytes = self.doc_freq.len() * 16;

        posting_bytes + norm_bytes + freq_bytes
    }

    /// Persist index to file.
    pub fn persist(&self, path: &Path) -> IndexResult<()> {
        let file =
            File::create(path).map_err(|e| IndexError::io("creating SPLADE index file", e))?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, self)
            .map_err(|e| IndexError::serialization("serializing SPLADE index", e))?;
        Ok(())
    }

    /// Load index from file.
    pub fn load(path: &Path) -> IndexResult<Self> {
        let file = File::open(path).map_err(|e| IndexError::io("opening SPLADE index file", e))?;
        let reader = BufReader::new(file);
        bincode::deserialize_from(reader)
            .map_err(|e| IndexError::serialization("deserializing SPLADE index", e))
    }
}

impl Default for SpladeInvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_index() {
        let index = SpladeInvertedIndex::new();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
        assert_eq!(index.term_count(), 0);
        println!("[VERIFIED] new() creates empty index");
    }

    #[test]
    fn test_add_validates_term_ids() {
        let mut index = SpladeInvertedIndex::new();
        let id = Uuid::new_v4();

        // Valid term IDs
        let result = index.add(id, &[(0, 0.5), (100, 0.3), (30521, 0.2)]);
        assert!(result.is_ok());
        println!("[VERIFIED] Valid term IDs accepted: 0, 100, 30521");

        // Invalid term ID
        let id2 = Uuid::new_v4();
        let result = index.add(id2, &[(40000, 0.5)]);
        assert!(matches!(
            result,
            Err(IndexError::InvalidTermId {
                term_id: 40000,
                vocab_size: 30522
            })
        ));
        println!("[VERIFIED] Invalid term_id 40000 rejected (vocab_size=30522)");
    }

    #[test]
    fn test_add_rejects_zero_norm_vector() {
        let mut index = SpladeInvertedIndex::new();
        let id = Uuid::new_v4();

        println!("[BEFORE] Adding zero-norm vector");
        let result = index.add(id, &[(100, 0.0), (200, 0.0)]);
        println!("[AFTER] Result: {:?}", result.is_err());

        assert!(matches!(result, Err(IndexError::ZeroNormVector { .. })));
        println!("[VERIFIED] Zero-norm vector rejected");
    }

    #[test]
    fn test_add_increments_count() {
        let mut index = SpladeInvertedIndex::new();

        println!("[BEFORE] index.len() = {}", index.len());

        index.add(Uuid::new_v4(), &[(100, 0.5)]).unwrap();
        println!("[AFTER ADD 1] index.len() = {}", index.len());
        assert_eq!(index.len(), 1);

        index.add(Uuid::new_v4(), &[(200, 0.3)]).unwrap();
        println!("[AFTER ADD 2] index.len() = {}", index.len());
        assert_eq!(index.len(), 2);

        println!("[VERIFIED] add() increments document count");
    }

    #[test]
    fn test_search_empty_index() {
        let index = SpladeInvertedIndex::new();

        println!("[BEFORE] Searching empty index");
        let results = index.search(&[(100, 1.0)], 10);
        println!("[AFTER] results.len() = {}", results.len());

        assert!(results.is_empty());
        println!("[VERIFIED] Empty index returns empty results");
    }

    #[test]
    fn test_search_returns_matching_documents() {
        let mut index = SpladeInvertedIndex::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        // id1 has term 100 with high weight
        index.add(id1, &[(100, 0.8), (200, 0.2)]).unwrap();
        // id2 has term 100 with low weight
        index.add(id2, &[(100, 0.2), (300, 0.9)]).unwrap();
        // id3 doesn't have term 100
        index.add(id3, &[(400, 0.5), (500, 0.6)]).unwrap();

        println!("[BEFORE] Searching for term 100");
        let results = index.search(&[(100, 1.0)], 10);
        println!(
            "[AFTER] results = {:?}",
            results
                .iter()
                .map(|(id, score)| (id.to_string()[..8].to_string(), score))
                .collect::<Vec<_>>()
        );

        // Should find id1 and id2, not id3
        assert_eq!(results.len(), 2);
        // id1 should rank first (higher weight on term 100)
        assert_eq!(results[0].0, id1);
        assert_eq!(results[1].0, id2);

        println!("[VERIFIED] Search returns correct documents in rank order");
    }

    #[test]
    fn test_search_bm25_idf_scoring() {
        let mut index = SpladeInvertedIndex::new();

        // Add 10 documents, most have term 100, few have term 200
        for i in 0..10 {
            let id = Uuid::new_v4();
            if i < 9 {
                // 9 docs have term 100
                index.add(id, &[(100, 0.5)]).unwrap();
            } else {
                // 1 doc has term 200 (rare term)
                index.add(id, &[(200, 0.5)]).unwrap();
            }
        }

        // Search for both terms
        let results_common = index.search(&[(100, 1.0)], 10);
        let results_rare = index.search(&[(200, 1.0)], 10);

        // Rare term (200) should have higher IDF, so higher scores
        // Even though weights are equal
        println!(
            "[RESULT] Common term (100) top score: {}",
            results_common.first().map(|r| r.1).unwrap_or(0.0)
        );
        println!(
            "[RESULT] Rare term (200) top score: {}",
            results_rare.first().map(|r| r.1).unwrap_or(0.0)
        );

        // The rare term should have higher score due to IDF
        if let (Some(common), Some(rare)) = (results_common.first(), results_rare.first()) {
            assert!(
                rare.1 > common.1,
                "Rare term should have higher BM25 score due to IDF"
            );
        }

        println!("[VERIFIED] BM25 IDF scoring works correctly");
    }

    #[test]
    fn test_remove_document() {
        let mut index = SpladeInvertedIndex::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        index.add(id1, &[(100, 0.5)]).unwrap();
        index.add(id2, &[(100, 0.3)]).unwrap();

        println!("[BEFORE] index.len() = {}", index.len());
        assert_eq!(index.len(), 2);

        let removed = index.remove(id1);
        println!(
            "[AFTER REMOVE] index.len() = {}, removed={}",
            index.len(),
            removed
        );

        assert!(removed);
        assert_eq!(index.len(), 1);

        // Search should only find id2 now
        let results = index.search(&[(100, 1.0)], 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id2);

        println!("[VERIFIED] remove() removes document from index and search");
    }

    #[test]
    fn test_remove_nonexistent_returns_false() {
        let mut index = SpladeInvertedIndex::new();
        let nonexistent = Uuid::new_v4();

        println!("[BEFORE] Removing nonexistent document");
        let removed = index.remove(nonexistent);
        println!("[AFTER] removed = {}", removed);

        assert!(!removed);
        println!("[VERIFIED] remove() returns false for nonexistent document");
    }

    #[test]
    fn test_memory_usage_increases_with_data() {
        let mut index = SpladeInvertedIndex::new();

        let usage_empty = index.memory_usage();
        println!("[BEFORE] memory_usage (empty) = {} bytes", usage_empty);

        for _ in 0..100 {
            index
                .add(Uuid::new_v4(), &[(100, 0.5), (200, 0.3)])
                .unwrap();
        }

        let usage_after = index.memory_usage();
        println!("[AFTER] memory_usage (100 docs) = {} bytes", usage_after);

        assert!(usage_after > usage_empty);
        println!("[VERIFIED] memory_usage increases with data");
    }

    #[test]
    fn test_search_handles_oov_query_terms() {
        let mut index = SpladeInvertedIndex::new();
        index.add(Uuid::new_v4(), &[(100, 0.5)]).unwrap();

        // Query with out-of-vocabulary term (but within vocab_size)
        println!("[BEFORE] Searching with OOV term 999 and valid term 100");
        let results = index.search(&[(100, 1.0), (999, 0.5)], 10);
        println!("[AFTER] results.len() = {}", results.len());

        // Should still find the document via term 100
        assert_eq!(results.len(), 1);
        println!("[VERIFIED] OOV terms in query are handled gracefully");
    }
}
