//! Storage traits for the pipeline.
//!
//! This module defines the storage interfaces for SPLADE index (Stage 1)
//! and token embeddings (Stage 5 MaxSim).

use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
// HIGH-17 FIX: parking_lot::RwLock is non-poisonable.
use parking_lot::RwLock;
use tracing::warn;
use uuid::Uuid;

// ============================================================================
// TOKEN STORAGE TRAIT (for Stage 5 MaxSim)
// ============================================================================

/// Storage interface for E12 ColBERT token embeddings.
///
/// Stage 5 requires token-level embeddings for MaxSim scoring.
/// This trait abstracts the storage backend.
pub trait TokenStorage: Send + Sync {
    /// Retrieve token embeddings for a memory ID.
    ///
    /// Returns Vec of 128D token embeddings.
    fn get_tokens(&self, id: Uuid) -> Option<Vec<Vec<f32>>>;
}

/// Maximum entries in the in-memory token cache.
/// Each entry is ~262 KB (512 tokens x 128D x 4 bytes).
/// 5,000 entries = ~1.3 GB cap.
const MAX_TOKEN_ENTRIES: usize = 5_000;

/// Internal state for token storage, protected by a single RwLock.
#[derive(Debug)]
struct TokenCacheInner {
    tokens: HashMap<Uuid, Vec<Vec<f32>>>,
    /// FIFO insertion order for eviction.
    insertion_order: VecDeque<Uuid>,
}

/// In-memory token storage with bounded capacity.
///
/// Evicts oldest entries (FIFO) when capacity is reached.
/// Each entry stores ~512 x 128D ColBERT token embeddings (~262 KB).
/// Bounded to MAX_TOKEN_ENTRIES (5,000) to prevent OOM.
/// Single RwLock guards both map and insertion order for atomicity.
#[derive(Debug)]
pub struct InMemoryTokenStorage {
    inner: RwLock<TokenCacheInner>,
}

impl Default for InMemoryTokenStorage {
    fn default() -> Self {
        Self {
            inner: RwLock::new(TokenCacheInner {
                tokens: HashMap::new(),
                insertion_order: VecDeque::new(),
            }),
        }
    }
}

impl InMemoryTokenStorage {
    /// Create new empty storage.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert tokens for an ID.
    ///
    /// If the cache is at capacity (MAX_TOKEN_ENTRIES), evicts the oldest entry
    /// and logs a warning. Updates in-place if the ID already exists.
    pub fn insert(&self, id: Uuid, tokens: Vec<Vec<f32>>) {
        let mut inner = self.inner.write();

        // If ID already exists, just update the data (no order change needed)
        if let std::collections::hash_map::Entry::Occupied(mut entry) = inner.tokens.entry(id) {
            entry.insert(tokens);
            return;
        }

        // Evict oldest entries until we have room
        while inner.tokens.len() >= MAX_TOKEN_ENTRIES {
            if let Some(evicted_id) = inner.insertion_order.pop_front() {
                inner.tokens.remove(&evicted_id);
                warn!(
                    evicted_id = %evicted_id,
                    cache_size = MAX_TOKEN_ENTRIES,
                    "Token storage cache full — evicted oldest entry. \
                     Consider using a RocksDB-backed TokenStorage implementation."
                );
            } else {
                // Safety: insertion_order and tokens are inconsistent — clear both
                warn!("Token storage insertion order desync — clearing cache");
                inner.tokens.clear();
                inner.insertion_order.clear();
                break;
            }
        }

        inner.tokens.insert(id, tokens);
        inner.insertion_order.push_back(id);
    }

    /// Get number of stored IDs.
    pub fn len(&self) -> usize {
        self.inner.read().tokens.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().tokens.is_empty()
    }

    /// Maximum capacity of this cache.
    pub fn capacity(&self) -> usize {
        MAX_TOKEN_ENTRIES
    }
}

impl TokenStorage for InMemoryTokenStorage {
    fn get_tokens(&self, id: Uuid) -> Option<Vec<Vec<f32>>> {
        self.inner.read().tokens.get(&id).cloned()
    }
}

// ============================================================================
// SPLADE INDEX TRAIT (for Stage 1)
// ============================================================================

/// Storage interface for SPLADE/E13 inverted index.
///
/// Stage 1 requires inverted index search, NOT HNSW.
pub trait SpladeIndex: Send + Sync {
    /// Search with BM25+SPLADE scoring.
    ///
    /// # Arguments
    /// * `query` - Sparse query vector as (term_id, weight) pairs
    /// * `k` - Number of results to return
    ///
    /// # Returns
    /// Vec of (id, score) pairs sorted by descending score.
    fn search(&self, query: &[(usize, f32)], k: usize) -> Vec<(Uuid, f32)>;

    /// Get the number of documents in the index.
    fn len(&self) -> usize;

    /// Check if index is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Maximum documents in the in-memory SPLADE index.
/// At 200 unique terms/doc average: ~160 MB posting lists + overhead.
/// 100,000 docs is a reasonable ceiling for in-memory usage.
const MAX_SPLADE_DOCS: usize = 100_000;

/// Internal state for SPLADE index, protected by a single RwLock.
/// Matches the `TokenCacheInner` pattern used by `InMemoryTokenStorage`.
#[derive(Debug)]
struct SpladeIndexInner {
    /// Posting lists: term_id -> [(doc_id, weight), ...]
    posting_lists: HashMap<usize, Vec<(Uuid, f32)>>,
    /// Document L2 norms
    doc_norms: HashMap<Uuid, f32>,
    /// Document frequency per term
    doc_freq: HashMap<usize, usize>,
    /// Total documents (plain usize, no atomic needed under single lock)
    num_docs: usize,
    /// FIFO insertion order + reverse term index for efficient eviction
    insertion_order: VecDeque<(Uuid, Vec<usize>)>,
}

/// In-memory SPLADE index with bounded capacity.
///
/// Evicts oldest documents (FIFO) when capacity is reached.
/// Bounded to MAX_SPLADE_DOCS (100,000) to prevent unbounded O(n*m) growth.
/// Single RwLock guards all internal state for atomicity (L2 consolidation).
#[derive(Debug)]
pub struct InMemorySpladeIndex {
    inner: RwLock<SpladeIndexInner>,
}

impl Default for InMemorySpladeIndex {
    fn default() -> Self {
        Self {
            inner: RwLock::new(SpladeIndexInner {
                posting_lists: HashMap::new(),
                doc_norms: HashMap::new(),
                doc_freq: HashMap::new(),
                num_docs: 0,
                insertion_order: VecDeque::new(),
            }),
        }
    }
}

impl InMemorySpladeIndex {
    /// Create new empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove a document from all index structures.
    fn evict_doc(inner: &mut SpladeIndexInner, doc_id: Uuid, doc_terms: &[usize]) {
        inner.doc_norms.remove(&doc_id);
        for &term_id in doc_terms {
            if let Some(term_postings) = inner.posting_lists.get_mut(&term_id) {
                term_postings.retain(|(id, _)| *id != doc_id);
                if term_postings.is_empty() {
                    inner.posting_lists.remove(&term_id);
                    inner.doc_freq.remove(&term_id);
                } else if let Some(df) = inner.doc_freq.get_mut(&term_id) {
                    *df = df.saturating_sub(1);
                }
            }
        }
    }

    /// Add a sparse vector to the index.
    ///
    /// If the document ID already exists, updates in-place.
    /// If the index is at capacity (MAX_SPLADE_DOCS), evicts the oldest
    /// document and logs a warning.
    pub fn add(&self, id: Uuid, sparse: &[(usize, f32)]) {
        // Compute norm outside lock (pure computation)
        let norm: f32 = sparse.iter().map(|(_, w)| w * w).sum::<f32>().sqrt();
        if norm < f32::EPSILON {
            return;
        }

        let mut inner = self.inner.write();

        // Guard: if ID already exists, evict old data first (prevents num_docs drift)
        if inner.doc_norms.contains_key(&id) {
            // Find and remove old entry from insertion_order
            if let Some(pos) = inner.insertion_order.iter().position(|(oid, _)| *oid == id) {
                let (_, old_terms) = inner.insertion_order.remove(pos).unwrap();
                Self::evict_doc(&mut inner, id, &old_terms);
                inner.num_docs = inner.num_docs.saturating_sub(1);
            }
        }

        // Evict oldest if at capacity
        while inner.num_docs >= MAX_SPLADE_DOCS {
            if let Some((evicted_id, evicted_terms)) = inner.insertion_order.pop_front() {
                Self::evict_doc(&mut inner, evicted_id, &evicted_terms);
                inner.num_docs = inner.num_docs.saturating_sub(1);
                warn!(
                    evicted_id = %evicted_id,
                    max_docs = MAX_SPLADE_DOCS,
                    "SPLADE index full — evicted oldest document. \
                     Consider using a RocksDB-backed SpladeIndex implementation."
                );
            } else {
                break;
            }
        }

        inner.doc_norms.insert(id, norm);

        let mut added_terms = HashSet::new();
        let mut doc_term_ids = Vec::new();

        for &(term_id, weight) in sparse {
            if weight.abs() < f32::EPSILON {
                continue;
            }

            inner
                .posting_lists
                .entry(term_id)
                .or_default()
                .push((id, weight));
            doc_term_ids.push(term_id);

            if added_terms.insert(term_id) {
                *inner.doc_freq.entry(term_id).or_insert(0) += 1;
            }
        }

        inner.insertion_order.push_back((id, doc_term_ids));
        inner.num_docs += 1;
    }
}

impl SpladeIndex for InMemorySpladeIndex {
    fn search(&self, query: &[(usize, f32)], k: usize) -> Vec<(Uuid, f32)> {
        let inner = self.inner.read();

        if inner.num_docs == 0 {
            return Vec::new();
        }

        let mut scores: HashMap<Uuid, f32> = HashMap::new();
        let n_f = inner.num_docs as f32;

        for &(term_id, query_weight) in query {
            if let Some(term_postings) = inner.posting_lists.get(&term_id) {
                let df = inner.doc_freq.get(&term_id).copied().unwrap_or(1) as f32;
                let idf = ((n_f - df + 0.5) / (df + 0.5) + 1.0).ln();

                for &(doc_id, doc_weight) in term_postings {
                    let norm = inner.doc_norms.get(&doc_id).copied().unwrap_or(1.0);
                    let tf = doc_weight / norm.max(f32::EPSILON);
                    *scores.entry(doc_id).or_insert(0.0) += query_weight * tf * idf;
                }
            }
        }

        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    fn len(&self) -> usize {
        self.inner.read().num_docs
    }
}

// ============================================================================
// E6 SPARSE INDEX TRAIT (for dual Stage 1 recall per e6upgrade.md)
// ============================================================================
// NOTE: These E6 types are test-only. Production E6 search uses
// TeleologicalMemoryStore::search_e6_sparse() which delegates to the RocksDB
// inverted index implementation.
// ============================================================================

/// Storage interface for E6 sparse (V_selectivity) inverted index.
///
/// E6 provides exact keyword matching to complement E13 SPLADE's learned expansion.
/// Used in dual Stage 1 recall: E6 catches exact technical terms, E13 catches
/// semantic variations.
#[cfg(test)]
pub trait E6SparseIndex: Send + Sync {
    /// Search with term overlap scoring.
    fn search(&self, query: &[(usize, f32)], k: usize) -> Vec<(Uuid, f32)>;

    /// Get sparse vector for a document (for tie-breaking).
    fn get_sparse(&self, id: Uuid) -> Option<Vec<(usize, f32)>>;

    /// Get the number of documents in the index.
    fn len(&self) -> usize;

    /// Check if index is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory E6 sparse index for testing.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct InMemoryE6SparseIndex {
    posting_lists: RwLock<HashMap<usize, Vec<(Uuid, f32)>>>,
    doc_vectors: RwLock<HashMap<Uuid, Vec<(usize, f32)>>>,
    num_docs: AtomicUsize,
}

#[cfg(test)]
impl InMemoryE6SparseIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&self, id: Uuid, sparse: &[(usize, f32)]) {
        self.doc_vectors.write().insert(id, sparse.to_vec());

        let mut postings = self.posting_lists.write();
        for &(term_id, weight) in sparse {
            if weight.abs() < f32::EPSILON {
                continue;
            }
            postings.entry(term_id).or_default().push((id, weight));
        }

        self.num_docs
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
impl E6SparseIndex for InMemoryE6SparseIndex {
    fn search(&self, query: &[(usize, f32)], k: usize) -> Vec<(Uuid, f32)> {
        let n = self.num_docs.load(std::sync::atomic::Ordering::SeqCst);
        if n == 0 {
            return Vec::new();
        }

        let postings = self.posting_lists.read();
        let mut term_counts: HashMap<Uuid, usize> = HashMap::new();
        let mut weighted_scores: HashMap<Uuid, f32> = HashMap::new();

        for &(term_id, query_weight) in query {
            if let Some(term_postings) = postings.get(&term_id) {
                for &(doc_id, doc_weight) in term_postings {
                    *term_counts.entry(doc_id).or_insert(0) += 1;
                    *weighted_scores.entry(doc_id).or_insert(0.0) += query_weight * doc_weight;
                }
            }
        }

        let query_term_count = query.len() as f32;
        let mut results: Vec<_> = term_counts
            .into_iter()
            .map(|(id, count)| {
                let overlap_ratio = count as f32 / query_term_count.max(1.0);
                let weighted = weighted_scores.get(&id).copied().unwrap_or(0.0);
                let score = overlap_ratio * (1.0 + weighted.ln().max(0.0));
                (id, score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    fn get_sparse(&self, id: Uuid) -> Option<Vec<(usize, f32)>> {
        self.doc_vectors.read().get(&id).cloned()
    }

    fn len(&self) -> usize {
        self.num_docs.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Detect query type and compute E6 weight boost factor.
#[cfg(test)]
pub fn compute_e6_boost(query: &str) -> f32 {
    let mut boost = 1.0f32;

    if contains_api_path(query) {
        boost += 0.5;
    }
    if contains_version_string(query) {
        boost += 0.3;
    }
    if contains_acronym(query) {
        boost += 0.3;
    }
    if contains_proper_noun(query) {
        boost += 0.2;
    }
    if high_common_word_ratio(query) {
        boost -= 0.3;
    }

    boost.clamp(0.5, 2.0)
}

#[cfg(test)]
fn contains_api_path(query: &str) -> bool {
    query.contains("::")
        || query.contains("->")
        || query.contains(".")
            && (query.contains("fn ")
                || query.contains("impl ")
                || query.contains("struct ")
                || query.contains("trait "))
}

#[cfg(test)]
fn contains_version_string(query: &str) -> bool {
    let words: Vec<&str> = query.split_whitespace().collect();
    for word in words {
        if word.starts_with('v')
            && word.len() >= 2
            && word.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        {
            return true;
        }
        if word.contains('.') {
            let parts: Vec<&str> = word.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().take_while(|c| c.is_ascii_digit()).count() > 0
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
fn contains_acronym(query: &str) -> bool {
    query.split_whitespace().any(|word| {
        word.len() >= 2
            && word
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    })
}

#[cfg(test)]
fn contains_proper_noun(query: &str) -> bool {
    let words: Vec<&str> = query.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            if let Some(c) = word.chars().next() {
                if c.is_ascii_uppercase() && word.len() > 1 {
                    let rest_lower = word.chars().skip(1).all(|c| c.is_lowercase());
                    if rest_lower {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
fn high_common_word_ratio(query: &str) -> bool {
    const COMMON_WORDS: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "can",
        "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "or", "and", "but", "if",
        "then", "else", "when", "where", "why", "how", "what", "which", "who", "this", "that",
        "these", "those", "it", "its", "i", "you", "he", "she", "we", "they", "me", "him", "her",
        "us", "them", "my", "your", "his", "her", "our", "their",
    ];

    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    let common_count = words
        .iter()
        .filter(|w| COMMON_WORDS.contains(&w.to_lowercase().as_str()))
        .count();

    let ratio = common_count as f32 / words.len() as f32;
    ratio > 0.5
}

/// Apply E6 tie-breaker to candidates with close scores.
#[cfg(test)]
pub fn apply_e6_tiebreaker(
    candidates: &mut [(Uuid, f32)],
    query_sparse: &[(usize, f32)],
    e6_index: &dyn E6SparseIndex,
    tie_threshold: f32,
    max_boost: f32,
) {
    if candidates.len() < 2 || query_sparse.is_empty() {
        return;
    }

    let query_terms: HashSet<usize> = query_sparse.iter().map(|(t, _)| *t).collect();
    let query_term_count = query_terms.len() as f32;

    let mut overlap_scores: Vec<f32> = Vec::with_capacity(candidates.len());
    for (id, _) in candidates.iter() {
        let overlap = if let Some(doc_sparse) = e6_index.get_sparse(*id) {
            let doc_terms: HashSet<usize> = doc_sparse.iter().map(|(t, _)| *t).collect();
            let shared = query_terms.intersection(&doc_terms).count() as f32;
            shared / query_term_count.max(1.0)
        } else {
            0.0
        };
        overlap_scores.push(overlap);
    }

    for i in 1..candidates.len() {
        let score_diff = (candidates[i - 1].1 - candidates[i].1).abs();
        if score_diff < tie_threshold {
            candidates[i].1 += overlap_scores[i] * max_boost;
        }
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
}
