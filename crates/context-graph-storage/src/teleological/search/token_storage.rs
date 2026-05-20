//! RocksDB-backed token storage for E12 ColBERT late interaction.
//!
//! # Overview
//!
//! Provides persistent storage for E12 token embeddings used in Stage 5
//! MaxSim reranking of the 5-stage retrieval pipeline.
//!
//! # Storage Format
//!
//! - Key: UUID (16 bytes) - memory ID
//! - Value: bincode serialized `Vec<Vec<f32>>` - token embeddings
//!   - Each inner Vec is 128D (E12_TOKEN_DIM)
//!   - Variable number of tokens per memory (typically 20-50)
//!
//! # Performance Targets
//!
//! - Retrieve 50 token sets: <5ms
//! - Single token set retrieval: <100μs
//!
//! # FAIL FAST Policy
//!
//! All errors are fatal. No recovery attempts.

use std::sync::Arc;

use rocksdb::{ColumnFamily, DB};
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::pipeline::TokenStorage;
use crate::teleological::column_families::CF_E12_LATE_INTERACTION;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Expected dimension for each E12 token embedding.
pub const E12_TOKEN_DIM: usize = 128;

/// Maximum tokens per memory (for validation).
pub const MAX_TOKENS_PER_MEMORY: usize = 512;

// ============================================================================
// ERRORS
// ============================================================================

/// Token storage errors. FAIL FAST - no recovery.
#[derive(Debug, thiserror::Error)]
pub enum TokenStorageError {
    /// RocksDB operation failed.
    #[error("FAIL FAST: RocksDB {operation} failed for {cf}: {source}")]
    RocksDb {
        operation: &'static str,
        cf: &'static str,
        #[source]
        source: rocksdb::Error,
    },

    /// Column family not found.
    #[error("FAIL FAST: Column family '{cf}' not found - database schema mismatch")]
    ColumnFamilyNotFound { cf: &'static str },

    /// Deserialization failed.
    #[error("FAIL FAST: Failed to deserialize tokens for {id}: {message}")]
    Deserialization { id: Uuid, message: String },

    /// Serialization failed.
    #[error("FAIL FAST: Failed to serialize tokens for {id}: {message}")]
    Serialization { id: Uuid, message: String },

    /// Invalid token dimension.
    #[error("FAIL FAST: Token {token_idx} has dimension {actual}, expected {expected}")]
    InvalidTokenDimension {
        token_idx: usize,
        actual: usize,
        expected: usize,
    },

    /// Token contains invalid values (NaN/Inf).
    #[error("FAIL FAST: Token {token_idx} contains {value_type} at index {value_idx}")]
    InvalidTokenValue {
        token_idx: usize,
        value_idx: usize,
        value_type: &'static str,
    },

    /// Too many tokens.
    #[error("FAIL FAST: Memory has {actual} tokens, maximum is {max}")]
    TooManyTokens { actual: usize, max: usize },
}

/// Result type for token storage operations.
pub type TokenStorageResult<T> = Result<T, TokenStorageError>;

// ============================================================================
// ROCKSDB TOKEN STORAGE
// ============================================================================

/// RocksDB-backed implementation of TokenStorage.
///
/// Stores E12 ColBERT token embeddings in the CF_E12_LATE_INTERACTION
/// column family for persistent, efficient retrieval in Stage 5.
pub struct RocksDbTokenStorage {
    /// Shared reference to the RocksDB database.
    db: Arc<DB>,
}

impl RocksDbTokenStorage {
    /// Create a new RocksDB token storage.
    ///
    /// # Arguments
    /// * `db` - Shared RocksDB database handle
    ///
    /// # FAIL FAST
    /// Panics if the CF_E12_LATE_INTERACTION column family does not exist.
    pub fn new(db: Arc<DB>) -> Self {
        // Verify column family exists - FAIL FAST
        db.cf_handle(CF_E12_LATE_INTERACTION)
            .unwrap_or_else(|| {
                panic!(
                    "FAIL FAST: Column family '{}' not found. Database was not opened with correct schema.",
                    CF_E12_LATE_INTERACTION
                )
            });

        Self { db }
    }

    /// Get the column family handle.
    ///
    /// # FAIL FAST
    /// Panics if column family is not found (should never happen after new()).
    #[inline]
    fn get_cf(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_E12_LATE_INTERACTION)
            .expect("FAIL FAST: CF_E12_LATE_INTERACTION disappeared after initialization")
    }

    /// Store token embeddings for a memory.
    ///
    /// # Arguments
    /// * `id` - Memory UUID
    /// * `tokens` - Vector of token embeddings (each 128D)
    ///
    /// # FAIL FAST Errors
    /// - `InvalidTokenDimension` if any token is not 128D
    /// - `InvalidTokenValue` if any token contains NaN/Inf
    /// - `TooManyTokens` if more than MAX_TOKENS_PER_MEMORY
    /// - `RocksDb` if database write fails
    pub fn store(&self, id: Uuid, tokens: &[Vec<f32>]) -> TokenStorageResult<()> {
        // Validate tokens - FAIL FAST
        self.validate_tokens(tokens)?;

        let key = id.as_bytes();
        let value = self.serialize_tokens(id, tokens)?;

        let cf = self.get_cf();
        self.db.put_cf(cf, key, &value).map_err(|e| {
            error!("Failed to store tokens for {}: {}", id, e);
            TokenStorageError::RocksDb {
                operation: "put",
                cf: CF_E12_LATE_INTERACTION,
                source: e,
            }
        })?;

        debug!(
            "Stored {} tokens ({} bytes) for {}",
            tokens.len(),
            value.len(),
            id
        );
        Ok(())
    }

    /// Retrieve token embeddings for a memory.
    ///
    /// # Arguments
    /// * `id` - Memory UUID
    ///
    /// # Returns
    /// - `Ok(Some(tokens))` if found
    /// - `Ok(None)` if not found
    /// - `Err` on database or deserialization errors
    pub fn get(&self, id: Uuid) -> TokenStorageResult<Option<Vec<Vec<f32>>>> {
        let key = id.as_bytes();
        let cf = self.get_cf();

        match self.db.get_cf(cf, key) {
            Ok(Some(data)) => {
                let tokens = self.deserialize_tokens(id, &data)?;
                debug!("Retrieved {} tokens for {}", tokens.len(), id);
                Ok(Some(tokens))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get tokens for {}: {}", id, e);
                Err(TokenStorageError::RocksDb {
                    operation: "get",
                    cf: CF_E12_LATE_INTERACTION,
                    source: e,
                })
            }
        }
    }

    /// Delete token embeddings for a memory.
    ///
    /// # Arguments
    /// * `id` - Memory UUID
    pub fn delete(&self, id: Uuid) -> TokenStorageResult<()> {
        let key = id.as_bytes();
        let cf = self.get_cf();

        self.db.delete_cf(cf, key).map_err(|e| {
            error!("Failed to delete tokens for {}: {}", id, e);
            TokenStorageError::RocksDb {
                operation: "delete",
                cf: CF_E12_LATE_INTERACTION,
                source: e,
            }
        })?;

        debug!("Deleted tokens for {}", id);
        Ok(())
    }

    /// Batch retrieve tokens for multiple memories.
    ///
    /// Returns a vector of (id, tokens) pairs for all found memories.
    /// Missing memories are logged at warn level and skipped.
    ///
    /// # Performance
    /// Uses RocksDB multi_get for efficient batch retrieval.
    pub fn get_batch(&self, ids: &[Uuid]) -> TokenStorageResult<Vec<(Uuid, Vec<Vec<f32>>)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let cf = self.get_cf();
        let keys: Vec<_> = ids
            .iter()
            .map(|id| (cf, id.as_bytes().as_slice()))
            .collect();

        let results = self.db.multi_get_cf(keys);
        let mut output = Vec::with_capacity(ids.len());

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Some(data)) => {
                    let tokens = self.deserialize_tokens(ids[i], &data)?;
                    output.push((ids[i], tokens));
                }
                Ok(None) => {
                    warn!(
                        memory_id = %ids[i],
                        batch_size = ids.len(),
                        "Token data missing for memory in batch retrieval — \
                         memory may have been deleted or tokens were never stored"
                    );
                }
                Err(e) => {
                    error!("Failed to get tokens for {} in batch: {}", ids[i], e);
                    return Err(TokenStorageError::RocksDb {
                        operation: "multi_get",
                        cf: CF_E12_LATE_INTERACTION,
                        source: e,
                    });
                }
            }
        }

        debug!(
            "Batch retrieved tokens for {}/{} memories",
            output.len(),
            ids.len()
        );
        Ok(output)
    }

    /// Check if tokens exist for a memory (without reading data).
    pub fn exists(&self, id: Uuid) -> TokenStorageResult<bool> {
        let key = id.as_bytes();
        let cf = self.get_cf();

        // Use key_may_exist for fast negative lookup, then verify with get
        if !self.db.key_may_exist_cf(cf, key) {
            return Ok(false);
        }

        // Verify with actual get (key_may_exist can have false positives)
        match self.db.get_cf(cf, key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(TokenStorageError::RocksDb {
                operation: "get",
                cf: CF_E12_LATE_INTERACTION,
                source: e,
            }),
        }
    }

    /// Validate tokens before storage.
    fn validate_tokens(&self, tokens: &[Vec<f32>]) -> TokenStorageResult<()> {
        // Check token count
        if tokens.len() > MAX_TOKENS_PER_MEMORY {
            return Err(TokenStorageError::TooManyTokens {
                actual: tokens.len(),
                max: MAX_TOKENS_PER_MEMORY,
            });
        }

        // Validate each token
        for (token_idx, token) in tokens.iter().enumerate() {
            // Check dimension
            if token.len() != E12_TOKEN_DIM {
                return Err(TokenStorageError::InvalidTokenDimension {
                    token_idx,
                    actual: token.len(),
                    expected: E12_TOKEN_DIM,
                });
            }

            // Check for NaN/Inf
            for (value_idx, &value) in token.iter().enumerate() {
                if value.is_nan() {
                    return Err(TokenStorageError::InvalidTokenValue {
                        token_idx,
                        value_idx,
                        value_type: "NaN",
                    });
                }
                if value.is_infinite() {
                    return Err(TokenStorageError::InvalidTokenValue {
                        token_idx,
                        value_idx,
                        value_type: "Inf",
                    });
                }
            }
        }

        Ok(())
    }

    /// Serialize tokens to bytes.
    fn serialize_tokens(&self, id: Uuid, tokens: &[Vec<f32>]) -> TokenStorageResult<Vec<u8>> {
        bincode::serialize(tokens).map_err(|e| TokenStorageError::Serialization {
            id,
            message: e.to_string(),
        })
    }

    /// Deserialize tokens from bytes.
    fn deserialize_tokens(&self, id: Uuid, data: &[u8]) -> TokenStorageResult<Vec<Vec<f32>>> {
        bincode::deserialize(data).map_err(|e| TokenStorageError::Deserialization {
            id,
            message: e.to_string(),
        })
    }
}

// ============================================================================
// TRAIT IMPLEMENTATION
// ============================================================================

impl TokenStorage for RocksDbTokenStorage {
    /// Retrieve token embeddings for a memory ID.
    ///
    /// This is the trait method used by the pipeline for Stage 5 MaxSim.
    fn get_tokens(&self, id: Uuid) -> Option<Vec<Vec<f32>>> {
        match self.get(id) {
            Ok(tokens) => tokens,
            Err(e) => {
                // Log error but return None to allow pipeline to continue
                // The pipeline will skip candidates without tokens
                error!("TokenStorage::get_tokens failed for {}: {}", id, e);
                None
            }
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::Cache;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    use crate::teleological::column_families::get_teleological_cf_descriptors;

    /// Shared test database — since every test uses `Uuid::new_v4()` for its
    /// own keys there is no cross-test interference, so we amortise the ~100ms
    /// RocksDB open cost across all tests in this module.
    struct SharedDb {
        _temp_dir: TempDir,
        db: Arc<DB>,
    }

    fn shared_db() -> &'static SharedDb {
        static DB: OnceLock<SharedDb> = OnceLock::new();
        DB.get_or_init(|| {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let cache = Cache::new_lru_cache(64 * 1024 * 1024); // 64MB
            let cf_descriptors = get_teleological_cf_descriptors(&cache);

            let mut db_opts = rocksdb::Options::default();
            db_opts.create_if_missing(true);
            db_opts.create_missing_column_families(true);

            let db = DB::open_cf_descriptors(&db_opts, temp_dir.path(), cf_descriptors)
                .expect("Failed to open test database");

            SharedDb {
                _temp_dir: temp_dir,
                db: Arc::new(db),
            }
        })
    }

    /// Legacy per-test DB creator — kept for tests that need an isolated DB
    /// (e.g. tests that scan/iterate all keys). Most tests should prefer
    /// `shared_db()` which is much cheaper.
    #[allow(dead_code)]
    fn create_test_db() -> (TempDir, Arc<DB>) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = Cache::new_lru_cache(64 * 1024 * 1024); // 64MB
        let cf_descriptors = get_teleological_cf_descriptors(&cache);

        let mut db_opts = rocksdb::Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let db = DB::open_cf_descriptors(&db_opts, temp_dir.path(), cf_descriptors)
            .expect("Failed to open test database");

        (temp_dir, Arc::new(db))
    }

    /// Generate test tokens.
    fn generate_test_tokens(num_tokens: usize) -> Vec<Vec<f32>> {
        (0..num_tokens)
            .map(|i| {
                (0..E12_TOKEN_DIM)
                    .map(|j| ((i * 128 + j) as f32 / 1000.0).sin())
                    .collect()
            })
            .collect()
    }

    // ========================================================================
    // BASIC FUNCTIONALITY TESTS
    // ========================================================================

    #[test]
    fn test_store_and_retrieve() {
        println!("=== TEST: Store and Retrieve ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(30);

        println!("[BEFORE] Storing {} tokens for {}", tokens.len(), id);

        // Store
        storage.store(id, &tokens).expect("Store failed");

        // Retrieve
        let retrieved = storage.get(id).expect("Get failed");

        println!(
            "[AFTER] Retrieved {:?} tokens",
            retrieved.as_ref().map(|t| t.len())
        );

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.len(), tokens.len());

        // Verify content
        for (i, (orig, ret)) in tokens.iter().zip(retrieved.iter()).enumerate() {
            assert_eq!(orig.len(), ret.len(), "Token {} dimension mismatch", i);
            for (j, (o, r)) in orig.iter().zip(ret.iter()).enumerate() {
                assert!(
                    (o - r).abs() < 1e-6,
                    "Token {} value {} mismatch: {} vs {}",
                    i,
                    j,
                    o,
                    r
                );
            }
        }

        println!("[VERIFIED] Store and retrieve works correctly");
    }

    #[test]
    fn test_get_nonexistent() {
        println!("=== TEST: Get Nonexistent ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let result = storage.get(id).expect("Get failed");

        assert!(result.is_none());
        println!("[VERIFIED] Nonexistent returns None");
    }

    #[test]
    fn test_delete() {
        println!("=== TEST: Delete ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(10);

        // Store
        storage.store(id, &tokens).expect("Store failed");
        assert!(storage.exists(id).expect("Exists failed"));

        // Delete
        storage.delete(id).expect("Delete failed");

        // Verify deleted
        let result = storage.get(id).expect("Get failed");
        assert!(result.is_none());

        println!("[VERIFIED] Delete works correctly");
    }

    #[test]
    fn test_batch_get() {
        println!("=== TEST: Batch Get ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        // Store 5 memories
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = Uuid::new_v4();
            let tokens = generate_test_tokens(10 + i);
            storage.store(id, &tokens).expect("Store failed");
            ids.push(id);
        }

        // Add a non-existent ID
        ids.push(Uuid::new_v4());

        println!(
            "[BEFORE] Batch getting {} IDs (5 exist, 1 missing)",
            ids.len()
        );

        // Batch get
        let results = storage.get_batch(&ids).expect("Batch get failed");

        println!("[AFTER] Got {} results", results.len());

        assert_eq!(results.len(), 5); // Only 5 exist

        println!("[VERIFIED] Batch get works correctly");
    }

    #[test]
    fn test_exists() {
        println!("=== TEST: Exists ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(5);

        // Before store
        assert!(!storage.exists(id).expect("Exists failed"));

        // After store
        storage.store(id, &tokens).expect("Store failed");
        assert!(storage.exists(id).expect("Exists failed"));

        println!("[VERIFIED] Exists check works correctly");
    }

    // ========================================================================
    // TRAIT IMPLEMENTATION TESTS
    // ========================================================================

    #[test]
    fn test_token_storage_trait() {
        println!("=== TEST: TokenStorage Trait ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(20);

        storage.store(id, &tokens).expect("Store failed");

        // Use trait method
        let result = storage.get_tokens(id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 20);

        // Nonexistent returns None
        let missing_id = Uuid::new_v4();
        assert!(storage.get_tokens(missing_id).is_none());

        println!("[VERIFIED] TokenStorage trait works correctly");
    }

    // ========================================================================
    // VALIDATION TESTS (FAIL FAST)
    // ========================================================================

    #[test]
    fn test_invalid_dimension_fails_fast() {
        println!("=== TEST: Invalid Dimension Fails Fast ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let mut tokens = generate_test_tokens(5);
        tokens[2] = vec![0.0; 64]; // Wrong dimension

        let result = storage.store(id, &tokens);
        assert!(result.is_err());

        if let Err(TokenStorageError::InvalidTokenDimension {
            token_idx,
            actual,
            expected,
        }) = result
        {
            assert_eq!(token_idx, 2);
            assert_eq!(actual, 64);
            assert_eq!(expected, 128);
        } else {
            panic!("Expected InvalidTokenDimension error");
        }

        println!("[VERIFIED] Invalid dimension causes FAIL FAST");
    }

    #[test]
    fn test_nan_fails_fast() {
        println!("=== TEST: NaN Fails Fast ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let mut tokens = generate_test_tokens(5);
        tokens[1][50] = f32::NAN;

        let result = storage.store(id, &tokens);
        assert!(result.is_err());

        if let Err(TokenStorageError::InvalidTokenValue {
            token_idx,
            value_idx,
            value_type,
        }) = result
        {
            assert_eq!(token_idx, 1);
            assert_eq!(value_idx, 50);
            assert_eq!(value_type, "NaN");
        } else {
            panic!("Expected InvalidTokenValue error");
        }

        println!("[VERIFIED] NaN causes FAIL FAST");
    }

    #[test]
    fn test_inf_fails_fast() {
        println!("=== TEST: Inf Fails Fast ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let mut tokens = generate_test_tokens(5);
        tokens[3][100] = f32::INFINITY;

        let result = storage.store(id, &tokens);
        assert!(result.is_err());

        if let Err(TokenStorageError::InvalidTokenValue {
            token_idx,
            value_idx,
            value_type,
        }) = result
        {
            assert_eq!(token_idx, 3);
            assert_eq!(value_idx, 100);
            assert_eq!(value_type, "Inf");
        } else {
            panic!("Expected InvalidTokenValue error");
        }

        println!("[VERIFIED] Inf causes FAIL FAST");
    }

    #[test]
    fn test_too_many_tokens_fails_fast() {
        println!("=== TEST: Too Many Tokens Fails Fast ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(MAX_TOKENS_PER_MEMORY + 1);

        let result = storage.store(id, &tokens);
        assert!(result.is_err());

        if let Err(TokenStorageError::TooManyTokens { actual, max }) = result {
            assert_eq!(actual, MAX_TOKENS_PER_MEMORY + 1);
            assert_eq!(max, MAX_TOKENS_PER_MEMORY);
        } else {
            panic!("Expected TooManyTokens error");
        }

        println!("[VERIFIED] Too many tokens causes FAIL FAST");
    }

    // ========================================================================
    // EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_empty_tokens() {
        println!("=== TEST: Empty Tokens ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens: Vec<Vec<f32>> = Vec::new();

        // Empty is valid
        storage.store(id, &tokens).expect("Store failed");

        let retrieved = storage.get(id).expect("Get failed");
        assert!(retrieved.is_some());
        assert!(retrieved.unwrap().is_empty());

        println!("[VERIFIED] Empty tokens handled correctly");
    }

    /// V-008 fix (#503): previously only `retrieved.unwrap().len() == 1`
    /// was checked. A storage backend bug that returned `Some(vec![vec![]])`
    /// would have passed. Now we compare token-by-token with float epsilon.
    #[test]
    fn test_single_token() {
        println!("=== TEST: Single Token ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(1);

        storage.store(id, &tokens).expect("Store failed");

        let retrieved = storage
            .get(id)
            .expect("Get failed")
            .expect("token row must be Some after store");
        assert_eq!(retrieved.len(), 1, "must round-trip exactly 1 token");
        assert_token_vectors_equal(&tokens, &retrieved);

        println!("[VERIFIED] Single token round-trip exact");
    }

    /// V-008 fix (#503): same pattern as test_single_token but at
    /// MAX_TOKENS_PER_MEMORY scale. Catches encoder/decoder bugs that
    /// preserve the outer length but corrupt inner f32 vectors.
    #[test]
    fn test_max_tokens() {
        println!("=== TEST: Max Tokens ===");

        let db = shared_db().db.clone();
        let storage = RocksDbTokenStorage::new(db);

        let id = Uuid::new_v4();
        let tokens = generate_test_tokens(MAX_TOKENS_PER_MEMORY);

        storage.store(id, &tokens).expect("Store failed");

        let retrieved = storage
            .get(id)
            .expect("Get failed")
            .expect("token row must be Some after store");
        assert_eq!(
            retrieved.len(),
            MAX_TOKENS_PER_MEMORY,
            "outer token count must match"
        );
        assert_token_vectors_equal(&tokens, &retrieved);

        println!("[VERIFIED] Max tokens round-trip exact");
    }

    /// Per-token, per-element equality with f32 epsilon. Fails fast on
    /// any divergence with concrete (i, j, original, retrieved) location.
    fn assert_token_vectors_equal(orig: &[Vec<f32>], retrieved: &[Vec<f32>]) {
        assert_eq!(
            orig.len(),
            retrieved.len(),
            "token count mismatch: orig={}, retrieved={}",
            orig.len(),
            retrieved.len()
        );
        for (i, (o, r)) in orig.iter().zip(retrieved.iter()).enumerate() {
            assert_eq!(
                o.len(),
                r.len(),
                "token[{i}] dim mismatch: orig={}, retrieved={}",
                o.len(),
                r.len()
            );
            for (j, (ov, rv)) in o.iter().zip(r.iter()).enumerate() {
                assert!(
                    (ov - rv).abs() < 1e-6,
                    "token[{i}][{j}] value diverged: orig={ov}, retrieved={rv}"
                );
            }
        }
    }
}
