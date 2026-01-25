//! CodeStore implementation for code entity and fingerprint storage.
//!
//! # Constitution Compliance
//!
//! - ARCH-01: "TeleologicalArray is atomic - all 13 embeddings or nothing"
//! - ARCH-05: "All 13 embedders required - missing = fatal"
//! - Code entities stored in SEPARATE database from teleological memories
//! - But each entity gets the FULL 13-embedder treatment
//! - E7 (V_correctness) provides code-specific patterns
//!
//! # Storage Architecture
//!
//! Each code entity is stored with its full SemanticFingerprint (all 13 embeddings).
//! This enables multi-space search: E7 for code patterns, E1 for semantic meaning,
//! E5 for causal understanding, etc.

use super::error::{CodeStorageError, CodeStorageResult};
use crate::teleological::column_families::{
    CF_CODE_ENTITIES, CF_CODE_E7_EMBEDDINGS, CF_CODE_FILE_INDEX, CF_CODE_NAME_INDEX,
    CF_CODE_SIGNATURE_INDEX,
};
use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_core::types::{CodeEntity, CodeFileIndexEntry, CodeLanguage, CodeStats};
use rocksdb::{ColumnFamily, IteratorMode, DB};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// E7 embedding dimension (Qodo-Embed-1-1.5B).
/// Kept for validation - E7 within SemanticFingerprint should be 1536D.
pub const E7_CODE_DIM: usize = 1536;

/// CodeStore provides storage for code entities with full SemanticFingerprint.
///
/// # Architecture
/// - Uses separate column families from teleological storage
/// - Full SemanticFingerprint (all 13 embeddings) stored per entity
/// - This enables multi-space search: E7 for code, E1 for semantic, etc.
/// - Secondary indexes for efficient name and signature search
/// - File-level tracking for change detection and cleanup
///
/// # Constitution Compliance
/// - ARCH-01: TeleologicalArray is atomic - all 13 embeddings stored together
/// - Separate from teleological store, but same embedding structure
///
/// # Thread Safety
/// CodeStore uses `Arc<DB>` and is `Send + Sync`.
pub struct CodeStore {
    /// RocksDB database handle.
    db: Arc<DB>,
}

impl CodeStore {
    /// Create a new CodeStore wrapping an existing RocksDB database.
    ///
    /// The database must have been opened with the code column families.
    /// Use `get_code_cf_descriptors()` when opening the database.
    ///
    /// # Arguments
    /// * `db` - Existing RocksDB database with code column families
    ///
    /// # Errors
    /// Returns error if required column families are not found.
    pub fn new(db: Arc<DB>) -> CodeStorageResult<Self> {
        // Verify all required column families exist
        for cf_name in &[
            CF_CODE_ENTITIES,
            CF_CODE_E7_EMBEDDINGS,
            CF_CODE_FILE_INDEX,
            CF_CODE_NAME_INDEX,
            CF_CODE_SIGNATURE_INDEX,
        ] {
            if db.cf_handle(cf_name).is_none() {
                return Err(CodeStorageError::cf_not_found(*cf_name));
            }
        }

        Ok(Self { db })
    }

    /// Open or create a CodeStore at the given path.
    ///
    /// This is a convenience method that opens a RocksDB database with
    /// the code column families.
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    ///
    /// # Errors
    /// Returns error if database cannot be opened or created.
    pub fn open(path: impl AsRef<Path>) -> CodeStorageResult<Self> {
        use crate::teleological::column_families::get_code_cf_descriptors;
        use rocksdb::{Cache, Options};

        let cache = Cache::new_lru_cache(64 * 1024 * 1024); // 64MB cache
        let cf_descriptors = get_code_cf_descriptors(&cache);

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let db = DB::open_cf_descriptors(&db_opts, path, cf_descriptors)?;
        Self::new(Arc::new(db))
    }

    // =========================================================================
    // Column Family Helpers
    // =========================================================================

    fn cf_entities(&self) -> CodeStorageResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CODE_ENTITIES)
            .ok_or_else(|| CodeStorageError::cf_not_found(CF_CODE_ENTITIES))
    }

    fn cf_embeddings(&self) -> CodeStorageResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CODE_E7_EMBEDDINGS)
            .ok_or_else(|| CodeStorageError::cf_not_found(CF_CODE_E7_EMBEDDINGS))
    }

    fn cf_file_index(&self) -> CodeStorageResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CODE_FILE_INDEX)
            .ok_or_else(|| CodeStorageError::cf_not_found(CF_CODE_FILE_INDEX))
    }

    fn cf_name_index(&self) -> CodeStorageResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CODE_NAME_INDEX)
            .ok_or_else(|| CodeStorageError::cf_not_found(CF_CODE_NAME_INDEX))
    }

    fn cf_signature_index(&self) -> CodeStorageResult<&ColumnFamily> {
        self.db
            .cf_handle(CF_CODE_SIGNATURE_INDEX)
            .ok_or_else(|| CodeStorageError::cf_not_found(CF_CODE_SIGNATURE_INDEX))
    }

    // =========================================================================
    // Entity CRUD Operations
    // =========================================================================

    /// Store a code entity with its full SemanticFingerprint.
    ///
    /// # Arguments
    /// * `entity` - The code entity to store
    /// * `fingerprint` - Complete 13-embedding fingerprint
    ///
    /// # Errors
    /// Returns error if fingerprint is incomplete or storage fails.
    ///
    /// # Constitution Compliance
    /// - ARCH-01: Stores all 13 embeddings together atomically
    /// - ARCH-05: Validates fingerprint has all required embeddings
    #[instrument(skip(self, entity, fingerprint), fields(id = %entity.id, name = %entity.name))]
    pub fn store(
        &self,
        entity: &CodeEntity,
        fingerprint: &SemanticFingerprint,
    ) -> CodeStorageResult<()> {
        // Validate fingerprint has all 13 embeddings (ARCH-05)
        if !fingerprint.is_complete() {
            return Err(CodeStorageError::InvalidDimension {
                expected: 13,
                actual: 0, // Incomplete fingerprint
            });
        }

        // Validate E7 dimension specifically
        if fingerprint.e7_code.len() != E7_CODE_DIM {
            return Err(CodeStorageError::InvalidDimension {
                expected: E7_CODE_DIM,
                actual: fingerprint.e7_code.len(),
            });
        }

        let id_bytes = entity.id.as_bytes();

        // Serialize entity
        let entity_bytes = bincode::serialize(entity)
            .map_err(|e| CodeStorageError::serialization(e.to_string()))?;

        // Serialize full fingerprint (all 13 embeddings)
        let fingerprint_bytes = bincode::serialize(fingerprint)
            .map_err(|e| CodeStorageError::serialization(e.to_string()))?;

        // Store entity
        self.db
            .put_cf(&self.cf_entities()?, id_bytes, &entity_bytes)?;

        // Store fingerprint (in the "embeddings" CF, now stores full fingerprint)
        self.db
            .put_cf(&self.cf_embeddings()?, id_bytes, &fingerprint_bytes)?;

        // Update file index
        self.update_file_index(&entity.file_path, entity.id, &entity.language)?;

        // Update name index
        self.update_name_index(&entity.name, entity.id)?;

        // Update signature index if present
        if let Some(ref sig) = entity.signature {
            self.update_signature_index(sig, entity.id)?;
        }

        debug!(
            id = %entity.id,
            name = %entity.name,
            entity_type = %entity.entity_type,
            file = %entity.file_path,
            fingerprint_size_bytes = fingerprint_bytes.len(),
            "Stored code entity with full 13-embedding fingerprint"
        );

        Ok(())
    }

    /// Get a code entity by ID.
    ///
    /// # Arguments
    /// * `id` - Entity UUID
    ///
    /// # Returns
    /// The entity if found, None otherwise.
    pub fn get(&self, id: Uuid) -> CodeStorageResult<Option<CodeEntity>> {
        let id_bytes = id.as_bytes();

        match self.db.get_cf(&self.cf_entities()?, id_bytes)? {
            Some(bytes) => {
                let entity: CodeEntity = bincode::deserialize(&bytes)
                    .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;
                Ok(Some(entity))
            }
            None => Ok(None),
        }
    }

    /// Get a code entity by ID, returning error if not found.
    pub fn get_or_error(&self, id: Uuid) -> CodeStorageResult<CodeEntity> {
        self.get(id)?.ok_or(CodeStorageError::not_found(id))
    }

    /// Get the full SemanticFingerprint for an entity.
    ///
    /// # Arguments
    /// * `id` - Entity UUID
    ///
    /// # Returns
    /// The complete 13-embedding fingerprint if found.
    pub fn get_fingerprint(&self, id: Uuid) -> CodeStorageResult<Option<SemanticFingerprint>> {
        let id_bytes = id.as_bytes();

        match self.db.get_cf(&self.cf_embeddings()?, id_bytes)? {
            Some(bytes) => {
                let fingerprint: SemanticFingerprint = bincode::deserialize(&bytes)
                    .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;
                Ok(Some(fingerprint))
            }
            None => Ok(None),
        }
    }

    /// Get just the E7 embedding for an entity (convenience method).
    ///
    /// # Arguments
    /// * `id` - Entity UUID
    ///
    /// # Returns
    /// The E7 (1536D) embedding if found.
    ///
    /// # Note
    /// For full multi-space search, use `get_fingerprint()` instead.
    pub fn get_e7_embedding(&self, id: Uuid) -> CodeStorageResult<Option<Vec<f32>>> {
        self.get_fingerprint(id).map(|opt| opt.map(|fp| fp.e7_code))
    }

    /// Get entity and its full fingerprint together.
    pub fn get_with_fingerprint(
        &self,
        id: Uuid,
    ) -> CodeStorageResult<Option<(CodeEntity, SemanticFingerprint)>> {
        let entity = match self.get(id)? {
            Some(e) => e,
            None => return Ok(None),
        };

        let fingerprint = self
            .get_fingerprint(id)?
            .ok_or(CodeStorageError::embedding_not_found(id))?;

        Ok(Some((entity, fingerprint)))
    }

    /// Delete a code entity and its embedding.
    ///
    /// Also removes from all indexes.
    #[instrument(skip(self), fields(id = %id))]
    pub fn delete(&self, id: Uuid) -> CodeStorageResult<bool> {
        // Get entity first to update indexes
        let entity = match self.get(id)? {
            Some(e) => e,
            None => return Ok(false),
        };

        let id_bytes = id.as_bytes();

        // Delete from entity store
        self.db.delete_cf(&self.cf_entities()?, id_bytes)?;

        // Delete embedding
        self.db.delete_cf(&self.cf_embeddings()?, id_bytes)?;

        // Remove from file index
        self.remove_from_file_index(&entity.file_path, id)?;

        // Remove from name index
        self.remove_from_name_index(&entity.name, id)?;

        // Remove from signature index if present
        if let Some(ref sig) = entity.signature {
            self.remove_from_signature_index(sig, id)?;
        }

        debug!(id = %id, name = %entity.name, "Deleted code entity");
        Ok(true)
    }

    /// Check if an entity exists.
    pub fn exists(&self, id: Uuid) -> CodeStorageResult<bool> {
        let id_bytes = id.as_bytes();
        Ok(self.db.get_cf(&self.cf_entities()?, id_bytes)?.is_some())
    }

    // =========================================================================
    // File-Level Operations
    // =========================================================================

    /// Get all entities for a file.
    ///
    /// # Arguments
    /// * `file_path` - Absolute path to the file
    ///
    /// # Returns
    /// Vector of entities in the file.
    pub fn get_by_file(&self, file_path: &str) -> CodeStorageResult<Vec<CodeEntity>> {
        let index_entry = self.get_file_index(file_path)?;

        match index_entry {
            Some(entry) => {
                let mut entities = Vec::with_capacity(entry.entity_ids.len());
                for id in &entry.entity_ids {
                    if let Some(entity) = self.get(*id)? {
                        entities.push(entity);
                    }
                }
                Ok(entities)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Get the file index entry for a file.
    pub fn get_file_index(&self, file_path: &str) -> CodeStorageResult<Option<CodeFileIndexEntry>> {
        match self.db.get_cf(&self.cf_file_index()?, file_path.as_bytes())? {
            Some(bytes) => {
                let entry: CodeFileIndexEntry = bincode::deserialize(&bytes)
                    .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Delete all entities for a file.
    ///
    /// # Returns
    /// Number of entities deleted.
    #[instrument(skip(self), fields(file = %file_path))]
    pub fn delete_file(&self, file_path: &str) -> CodeStorageResult<usize> {
        let index_entry = match self.get_file_index(file_path)? {
            Some(entry) => entry,
            None => return Ok(0),
        };

        let mut deleted = 0;
        for id in &index_entry.entity_ids {
            if self.delete(*id)? {
                deleted += 1;
            }
        }

        // Delete the file index entry itself
        self.db
            .delete_cf(&self.cf_file_index()?, file_path.as_bytes())?;

        info!(file = %file_path, deleted = deleted, "Deleted all entities for file");
        Ok(deleted)
    }

    /// List all indexed files.
    ///
    /// # Arguments
    /// * `path_filter` - Optional glob pattern to filter paths
    ///
    /// # Returns
    /// Vector of (file_path, entity_count) tuples.
    pub fn list_files(&self, path_filter: Option<&str>) -> CodeStorageResult<Vec<(String, usize)>> {
        let cf = self.cf_file_index()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        let mut files = Vec::new();
        for item in iter {
            let (key, value) = item?;
            let path = String::from_utf8_lossy(&key).to_string();

            // Apply optional filter
            if let Some(filter) = path_filter {
                if !Self::glob_match(&path, filter) {
                    continue;
                }
            }

            let entry: CodeFileIndexEntry = bincode::deserialize(&value)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            files.push((path, entry.entity_count()));
        }

        Ok(files)
    }

    // =========================================================================
    // Search Operations
    // =========================================================================

    /// Search entities by name prefix.
    ///
    /// # Arguments
    /// * `prefix` - Name prefix to search for
    /// * `limit` - Maximum results to return
    ///
    /// # Returns
    /// Vector of matching entities.
    pub fn search_by_name(&self, prefix: &str, limit: usize) -> CodeStorageResult<Vec<CodeEntity>> {
        let cf = self.cf_name_index()?;
        let iter = self.db.prefix_iterator_cf(&cf, prefix.as_bytes());

        let mut entities = Vec::new();
        for item in iter {
            if entities.len() >= limit {
                break;
            }

            let (key, value) = item?;
            let name = String::from_utf8_lossy(&key);

            // Verify prefix match (prefix_iterator may return non-matches)
            if !name.starts_with(prefix) {
                break;
            }

            let ids: Vec<Uuid> = bincode::deserialize(&value)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            for id in ids {
                if entities.len() >= limit {
                    break;
                }
                if let Some(entity) = self.get(id)? {
                    entities.push(entity);
                }
            }
        }

        Ok(entities)
    }

    /// Search entities by exact name.
    pub fn search_by_exact_name(&self, name: &str) -> CodeStorageResult<Vec<CodeEntity>> {
        match self.db.get_cf(&self.cf_name_index()?, name.as_bytes())? {
            Some(bytes) => {
                let ids: Vec<Uuid> = bincode::deserialize(&bytes)
                    .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

                let mut entities = Vec::with_capacity(ids.len());
                for id in ids {
                    if let Some(entity) = self.get(id)? {
                        entities.push(entity);
                    }
                }
                Ok(entities)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Search entities by signature pattern.
    ///
    /// # Arguments
    /// * `signature` - Function signature to search for
    ///
    /// # Returns
    /// Vector of matching entities.
    pub fn search_by_signature(&self, signature: &str) -> CodeStorageResult<Vec<CodeEntity>> {
        let hash = Self::hash_signature(signature);

        match self.db.get_cf(&self.cf_signature_index()?, &hash)? {
            Some(bytes) => {
                let ids: Vec<Uuid> = bincode::deserialize(&bytes)
                    .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

                let mut entities = Vec::with_capacity(ids.len());
                for id in ids {
                    if let Some(entity) = self.get(id)? {
                        entities.push(entity);
                    }
                }
                Ok(entities)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Search entities by fingerprint similarity.
    ///
    /// Performs a linear scan over all fingerprints and returns the top-K
    /// most similar entities by cosine similarity.
    ///
    /// # Arguments
    /// * `query_fingerprint` - Query fingerprint (all 13 embeddings)
    /// * `top_k` - Maximum number of results to return
    /// * `min_similarity` - Minimum similarity threshold (0.0 to 1.0)
    /// * `use_e7_primary` - If true, use E7 (code) for scoring; else use E1 (semantic)
    ///
    /// # Returns
    /// Vector of (entity_id, similarity_score) pairs, sorted by decreasing similarity.
    ///
    /// # Note
    /// This is a linear scan implementation. For large codebases, consider
    /// building an HNSW index for better performance.
    pub fn search_by_fingerprint(
        &self,
        query_fingerprint: &SemanticFingerprint,
        top_k: usize,
        min_similarity: f32,
        use_e7_primary: bool,
    ) -> CodeStorageResult<Vec<(Uuid, f32)>> {
        // Select query embedding based on mode
        let query = if use_e7_primary {
            &query_fingerprint.e7_code
        } else {
            &query_fingerprint.e1_semantic
        };

        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Pre-compute query norm for cosine similarity
        let query_norm = Self::vector_norm(query);
        if query_norm < f32::EPSILON {
            return Ok(Vec::new());
        }

        // Collect all candidates with similarity scores
        let cf = self.cf_embeddings()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        let mut candidates: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (key, value) = item?;

            // Parse UUID from key
            if key.len() != 16 {
                continue;
            }
            let id = Uuid::from_slice(&key).map_err(|e| {
                CodeStorageError::deserialization(format!("Invalid UUID in fingerprint index: {}", e))
            })?;

            // Deserialize fingerprint
            let stored_fp: SemanticFingerprint = bincode::deserialize(&value)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            // Select stored embedding based on mode
            let stored_embedding = if use_e7_primary {
                &stored_fp.e7_code
            } else {
                &stored_fp.e1_semantic
            };

            // Compute cosine similarity
            let similarity = Self::cosine_similarity_with_norm(query, stored_embedding, query_norm);

            if similarity >= min_similarity {
                candidates.push((id, similarity));
            }
        }

        // Sort by similarity descending and take top_k
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(top_k);

        debug!(
            candidates_found = candidates.len(),
            top_k = top_k,
            min_similarity = min_similarity,
            use_e7 = use_e7_primary,
            "CodeStore: search_by_fingerprint completed"
        );

        Ok(candidates)
    }

    /// Search entities by fingerprint and return full entity data.
    ///
    /// Convenience wrapper around `search_by_fingerprint` that also fetches
    /// the entity data for each result.
    ///
    /// # Arguments
    /// * `query_fingerprint` - Query fingerprint (all 13 embeddings)
    /// * `top_k` - Maximum number of results to return
    /// * `min_similarity` - Minimum similarity threshold (0.0 to 1.0)
    /// * `use_e7_primary` - If true, use E7 (code) for scoring; else use E1 (semantic)
    ///
    /// # Returns
    /// Vector of (entity, similarity_score) pairs.
    pub fn search_by_fingerprint_with_entities(
        &self,
        query_fingerprint: &SemanticFingerprint,
        top_k: usize,
        min_similarity: f32,
        use_e7_primary: bool,
    ) -> CodeStorageResult<Vec<(CodeEntity, f32)>> {
        let results = self.search_by_fingerprint(query_fingerprint, top_k, min_similarity, use_e7_primary)?;

        let mut entities_with_scores = Vec::with_capacity(results.len());
        for (id, score) in results {
            if let Some(entity) = self.get(id)? {
                entities_with_scores.push((entity, score));
            }
        }

        Ok(entities_with_scores)
    }

    // =========================================================================
    // Vector Math Helpers
    // =========================================================================

    /// Compute the L2 norm of a vector.
    fn vector_norm(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Compute cosine similarity with pre-computed query norm.
    fn cosine_similarity_with_norm(query: &[f32], candidate: &[f32], query_norm: f32) -> f32 {
        if query.len() != candidate.len() {
            return 0.0;
        }

        let dot: f32 = query.iter().zip(candidate.iter()).map(|(a, b)| a * b).sum();
        let candidate_norm = Self::vector_norm(candidate);

        if candidate_norm < f32::EPSILON {
            return 0.0;
        }

        (dot / (query_norm * candidate_norm)).clamp(-1.0, 1.0)
    }

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get storage statistics.
    pub fn get_stats(&self) -> CodeStorageResult<CodeStats> {
        let files = self.list_files(None)?;
        let total_files = files.len();

        let mut total_entities = 0;
        let mut entities_by_type = HashMap::new();
        let mut entities_by_language = HashMap::new();
        let mut total_lines = 0;

        // Iterate all entities
        let cf = self.cf_entities()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);

        for item in iter {
            let (_key, value) = item?;
            let entity: CodeEntity = bincode::deserialize(&value)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            total_entities += 1;
            *entities_by_type.entry(entity.entity_type).or_insert(0) += 1;
            *entities_by_language.entry(entity.language).or_insert(0) += 1;
            total_lines += entity.line_count();
        }

        let avg_entities_per_file = if total_files > 0 {
            total_entities as f64 / total_files as f64
        } else {
            0.0
        };

        Ok(CodeStats {
            total_files,
            total_entities,
            entities_by_type,
            entities_by_language,
            avg_entities_per_file,
            total_lines,
        })
    }

    /// Count total entities.
    pub fn count(&self) -> CodeStorageResult<usize> {
        let cf = self.cf_entities()?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        Ok(iter.count())
    }

    // =========================================================================
    // Index Management Helpers
    // =========================================================================

    fn update_file_index(
        &self,
        file_path: &str,
        entity_id: Uuid,
        language: &CodeLanguage,
    ) -> CodeStorageResult<()> {
        let cf = self.cf_file_index()?;
        let key = file_path.as_bytes();

        let mut entry = match self.db.get_cf(&cf, key)? {
            Some(bytes) => bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?,
            None => CodeFileIndexEntry::new(
                file_path.to_string(),
                *language,
                String::new(), // Hash will be set by watcher
            ),
        };

        entry.add_entity(entity_id);

        let bytes = bincode::serialize(&entry)
            .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
        self.db.put_cf(&cf, key, bytes)?;

        Ok(())
    }

    fn remove_from_file_index(&self, file_path: &str, entity_id: Uuid) -> CodeStorageResult<()> {
        let cf = self.cf_file_index()?;
        let key = file_path.as_bytes();

        if let Some(bytes) = self.db.get_cf(&cf, key)? {
            let mut entry: CodeFileIndexEntry = bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            entry.remove_entity(entity_id);

            if entry.is_empty() {
                self.db.delete_cf(&cf, key)?;
            } else {
                let bytes = bincode::serialize(&entry)
                    .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
                self.db.put_cf(&cf, key, bytes)?;
            }
        }

        Ok(())
    }

    fn update_name_index(&self, name: &str, entity_id: Uuid) -> CodeStorageResult<()> {
        let cf = self.cf_name_index()?;
        let key = name.as_bytes();

        let mut ids: Vec<Uuid> = match self.db.get_cf(&cf, key)? {
            Some(bytes) => bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?,
            None => Vec::new(),
        };

        if !ids.contains(&entity_id) {
            ids.push(entity_id);
            let bytes = bincode::serialize(&ids)
                .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
            self.db.put_cf(&cf, key, bytes)?;
        }

        Ok(())
    }

    fn remove_from_name_index(&self, name: &str, entity_id: Uuid) -> CodeStorageResult<()> {
        let cf = self.cf_name_index()?;
        let key = name.as_bytes();

        if let Some(bytes) = self.db.get_cf(&cf, key)? {
            let mut ids: Vec<Uuid> = bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            if let Some(pos) = ids.iter().position(|&id| id == entity_id) {
                ids.remove(pos);

                if ids.is_empty() {
                    self.db.delete_cf(&cf, key)?;
                } else {
                    let bytes = bincode::serialize(&ids)
                        .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
                    self.db.put_cf(&cf, key, bytes)?;
                }
            }
        }

        Ok(())
    }

    fn update_signature_index(&self, signature: &str, entity_id: Uuid) -> CodeStorageResult<()> {
        let cf = self.cf_signature_index()?;
        let key = Self::hash_signature(signature);

        let mut ids: Vec<Uuid> = match self.db.get_cf(&cf, &key)? {
            Some(bytes) => bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?,
            None => Vec::new(),
        };

        if !ids.contains(&entity_id) {
            ids.push(entity_id);
            let bytes = bincode::serialize(&ids)
                .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
            self.db.put_cf(&cf, &key, bytes)?;
        }

        Ok(())
    }

    fn remove_from_signature_index(
        &self,
        signature: &str,
        entity_id: Uuid,
    ) -> CodeStorageResult<()> {
        let cf = self.cf_signature_index()?;
        let key = Self::hash_signature(signature);

        if let Some(bytes) = self.db.get_cf(&cf, &key)? {
            let mut ids: Vec<Uuid> = bincode::deserialize(&bytes)
                .map_err(|e| CodeStorageError::deserialization(e.to_string()))?;

            if let Some(pos) = ids.iter().position(|&id| id == entity_id) {
                ids.remove(pos);

                if ids.is_empty() {
                    self.db.delete_cf(&cf, &key)?;
                } else {
                    let bytes = bincode::serialize(&ids)
                        .map_err(|e| CodeStorageError::serialization(e.to_string()))?;
                    self.db.put_cf(&cf, &key, bytes)?;
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Utility Helpers
    // =========================================================================

    fn hash_signature(signature: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(signature.as_bytes());
        hasher.finalize().into()
    }

    fn glob_match(path: &str, pattern: &str) -> bool {
        // Simple glob matching - supports * and **
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        Self::glob_match_parts(&path_parts, &pattern_parts)
    }

    /// Simple wildcard matching within a single path segment.
    /// Supports * as a wildcard matching any characters.
    fn wildcard_match(text: &str, pattern: &str) -> bool {
        let mut text_idx = 0;
        let mut pattern_idx = 0;
        let mut star_idx: Option<usize> = None;
        let mut match_idx = 0;

        let text_chars: Vec<char> = text.chars().collect();
        let pattern_chars: Vec<char> = pattern.chars().collect();

        while text_idx < text_chars.len() {
            if pattern_idx < pattern_chars.len()
                && (pattern_chars[pattern_idx] == '?'
                    || pattern_chars[pattern_idx] == text_chars[text_idx])
            {
                text_idx += 1;
                pattern_idx += 1;
            } else if pattern_idx < pattern_chars.len() && pattern_chars[pattern_idx] == '*' {
                star_idx = Some(pattern_idx);
                match_idx = text_idx;
                pattern_idx += 1;
            } else if let Some(star) = star_idx {
                pattern_idx = star + 1;
                match_idx += 1;
                text_idx = match_idx;
            } else {
                return false;
            }
        }

        while pattern_idx < pattern_chars.len() && pattern_chars[pattern_idx] == '*' {
            pattern_idx += 1;
        }

        pattern_idx == pattern_chars.len()
    }

    fn glob_match_parts(path: &[&str], pattern: &[&str]) -> bool {
        if pattern.is_empty() {
            return path.is_empty();
        }

        if path.is_empty() {
            return pattern.iter().all(|p| *p == "**");
        }

        match pattern[0] {
            "**" => {
                // Match zero or more path segments
                for i in 0..=path.len() {
                    if Self::glob_match_parts(&path[i..], &pattern[1..]) {
                        return true;
                    }
                }
                false
            }
            "*" => {
                // Match exactly one segment (any content)
                Self::glob_match_parts(&path[1..], &pattern[1..])
            }
            p if p.contains('*') => {
                // Match with wildcards in segment using simple pattern matching
                if Self::wildcard_match(path[0], p) {
                    Self::glob_match_parts(&path[1..], &pattern[1..])
                } else {
                    false
                }
            }
            p => {
                // Exact match
                path[0] == p && Self::glob_match_parts(&path[1..], &pattern[1..])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::types::CodeEntityType;
    use tempfile::tempdir;

    fn create_test_entity(name: &str) -> CodeEntity {
        CodeEntity::new(
            CodeEntityType::Function,
            name.to_string(),
            format!("fn {}() {{}}", name),
            CodeLanguage::Rust,
            "/test/file.rs".to_string(),
            1,
            3,
        )
    }

    /// Create a test fingerprint with proper E7 dimension.
    ///
    /// Uses zeroed base and sets E7 to a non-zero vector for testing.
    fn create_test_fingerprint() -> SemanticFingerprint {
        let mut fp = SemanticFingerprint::zeroed();
        // Set E7 to a recognizable test pattern
        fp.e7_code = vec![0.1; E7_CODE_DIM];
        fp
    }

    #[test]
    fn test_store_and_get() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity = create_test_entity("my_function");
        let fingerprint = create_test_fingerprint();
        let id = entity.id;

        store.store(&entity, &fingerprint).unwrap();

        let retrieved = store.get(id).unwrap().unwrap();
        assert_eq!(retrieved.name, "my_function");
        assert_eq!(retrieved.entity_type, CodeEntityType::Function);

        // Verify fingerprint retrieval
        let retrieved_fp = store.get_fingerprint(id).unwrap().unwrap();
        assert!(retrieved_fp.is_complete(), "Fingerprint should have all 13 embeddings");
        assert_eq!(retrieved_fp.e7_code.len(), E7_CODE_DIM);

        // Also test the convenience method
        let e7_embedding = store.get_e7_embedding(id).unwrap().unwrap();
        assert_eq!(e7_embedding.len(), E7_CODE_DIM);
    }

    #[test]
    fn test_delete() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity = create_test_entity("to_delete");
        let fingerprint = create_test_fingerprint();
        let id = entity.id;

        store.store(&entity, &fingerprint).unwrap();
        assert!(store.exists(id).unwrap());

        store.delete(id).unwrap();
        assert!(!store.exists(id).unwrap());
    }

    #[test]
    fn test_get_by_file() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity1 = CodeEntity::new(
            CodeEntityType::Function,
            "func1".to_string(),
            "fn func1() {}".to_string(),
            CodeLanguage::Rust,
            "/test/myfile.rs".to_string(),
            1,
            3,
        );
        let entity2 = CodeEntity::new(
            CodeEntityType::Function,
            "func2".to_string(),
            "fn func2() {}".to_string(),
            CodeLanguage::Rust,
            "/test/myfile.rs".to_string(),
            5,
            7,
        );

        store.store(&entity1, &create_test_fingerprint()).unwrap();
        store.store(&entity2, &create_test_fingerprint()).unwrap();

        let entities = store.get_by_file("/test/myfile.rs").unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn test_search_by_name() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity1 = create_test_entity("process_data");
        let entity2 = create_test_entity("process_items");
        let entity3 = create_test_entity("handle_event");

        store.store(&entity1, &create_test_fingerprint()).unwrap();
        store.store(&entity2, &create_test_fingerprint()).unwrap();
        store.store(&entity3, &create_test_fingerprint()).unwrap();

        let results = store.search_by_name("process", 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete_file() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity1 = CodeEntity::new(
            CodeEntityType::Function,
            "func1".to_string(),
            "fn func1() {}".to_string(),
            CodeLanguage::Rust,
            "/test/to_delete.rs".to_string(),
            1,
            3,
        );
        let entity2 = CodeEntity::new(
            CodeEntityType::Struct,
            "MyStruct".to_string(),
            "struct MyStruct {}".to_string(),
            CodeLanguage::Rust,
            "/test/to_delete.rs".to_string(),
            5,
            7,
        );

        store.store(&entity1, &create_test_fingerprint()).unwrap();
        store.store(&entity2, &create_test_fingerprint()).unwrap();

        let deleted = store.delete_file("/test/to_delete.rs").unwrap();
        assert_eq!(deleted, 2);

        let entities = store.get_by_file("/test/to_delete.rs").unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn test_stats() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity1 = create_test_entity("func1");
        let entity2 = create_test_entity("func2");

        store.store(&entity1, &create_test_fingerprint()).unwrap();
        store.store(&entity2, &create_test_fingerprint()).unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_entities, 2);
        assert_eq!(stats.total_files, 1);
    }

    #[test]
    fn test_invalid_fingerprint() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        let entity = create_test_entity("test");
        // Create incomplete fingerprint by setting E7 to wrong dimension
        let mut bad_fp = SemanticFingerprint::zeroed();
        bad_fp.e7_code = vec![0.1; 100]; // Wrong E7 dimension

        let result = store.store(&entity, &bad_fp);
        assert!(matches!(
            result,
            Err(CodeStorageError::InvalidDimension { .. })
        ));
    }

    #[test]
    fn test_search_by_fingerprint() {
        let dir = tempdir().unwrap();
        let store = CodeStore::open(dir.path()).unwrap();

        // Create entities with distinct fingerprints
        let entity1 = create_test_entity("search_target");
        let mut fp1 = create_test_fingerprint();
        fp1.e7_code = vec![1.0; E7_CODE_DIM]; // Normalized unit vector

        let entity2 = create_test_entity("other_func");
        let mut fp2 = create_test_fingerprint();
        fp2.e7_code = vec![-0.5; E7_CODE_DIM]; // Different direction

        store.store(&entity1, &fp1).unwrap();
        store.store(&entity2, &fp2).unwrap();

        // Search with query similar to entity1
        let mut query_fp = create_test_fingerprint();
        query_fp.e7_code = vec![0.9; E7_CODE_DIM];

        let results = store.search_by_fingerprint(&query_fp, 10, 0.0, true).unwrap();
        assert!(!results.is_empty());

        // entity1 should be first (more similar)
        let (top_id, top_score) = results[0];
        assert_eq!(top_id, entity1.id);
        assert!(top_score > 0.9, "Top result should have high similarity");
    }

    #[test]
    fn test_glob_match() {
        assert!(CodeStore::glob_match("/foo/bar/baz.rs", "**/baz.rs"));
        assert!(CodeStore::glob_match("/foo/bar/baz.rs", "/foo/**/*.rs"));
        assert!(CodeStore::glob_match("/foo/bar/baz.rs", "/foo/*/baz.rs"));
        assert!(!CodeStore::glob_match("/foo/bar/baz.rs", "/other/**/*.rs"));
    }
}
