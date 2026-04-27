//! Extension trait for TeleologicalMemoryStore with convenience methods.

use async_trait::async_trait;

use crate::error::CoreResult;
use crate::types::fingerprint::TeleologicalFingerprint;
use uuid::Uuid;

use super::store::TeleologicalMemoryStore;

/// Extension trait for convenient TeleologicalMemoryStore operations.
///
/// Provides helper methods built on top of the core trait.
#[async_trait]
pub trait TeleologicalMemoryStoreExt: TeleologicalMemoryStore {
    /// Check if a fingerprint exists by ID.
    async fn exists(&self, id: Uuid) -> CoreResult<bool> {
        Ok(self.retrieve(id).await?.is_some())
    }

    /// Validate a fingerprint before storage.
    ///
    /// Performs comprehensive validation of the TeleologicalFingerprint:
    /// - Validates all 13 embedder dimensions in the SemanticFingerprint
    /// - Validates sparse vector vocabulary bounds (E6, E13)
    /// - Validates ColBERT token dimensions (E12)
    ///
    /// # FAIL FAST
    ///
    /// Returns immediately on first validation failure with a detailed error
    /// message. No partial validation or fallback values.
    ///
    /// # Arguments
    ///
    /// * `fingerprint` - The fingerprint to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Fingerprint is valid for storage
    /// * `Err(CoreError::ValidationError)` - Validation failed with details
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::traits::TeleologicalMemoryStoreExt;
    ///
    /// let store = get_store();
    /// let fingerprint = build_fingerprint();
    ///
    /// // Validate before storing - FAIL FAST on invalid data
    /// store.validate_for_storage(&fingerprint)?;
    /// let id = store.store(fingerprint).await?;
    /// ```
    fn validate_for_storage(&self, fingerprint: &TeleologicalFingerprint) -> CoreResult<()> {
        fingerprint
            .semantic
            .validate()
            .map_err(|err| crate::error::CoreError::ValidationError {
                field: "semantic".to_string(),
                message: err.to_string(),
            })
    }
}

// Blanket implementation for all TeleologicalMemoryStore implementations
impl<T: TeleologicalMemoryStore> TeleologicalMemoryStoreExt for T {}
