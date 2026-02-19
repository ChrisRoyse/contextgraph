//! Index operations for RocksDbTeleologicalStore.
//!
//! Contains methods for adding/removing fingerprints to/from HNSW indexes.

use tracing::debug;
use uuid::Uuid;

use context_graph_core::types::fingerprint::{SemanticFingerprint, TeleologicalFingerprint};

use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexOps, IndexError};

use super::store::RocksDbTeleologicalStore;

impl RocksDbTeleologicalStore {
    /// Add fingerprint to per-embedder HNSW indexes for O(log n) search.
    ///
    /// Inserts vectors into all HNSW-capable embedder indexes.
    /// E6, E12, E13 are skipped (they use different index types).
    ///
    /// # FAIL FAST
    ///
    /// - DimensionMismatch: panic with detailed error
    /// - InvalidVector (NaN/Inf): panic with location
    pub(crate) fn add_to_indexes(&self, fp: &TeleologicalFingerprint) -> Result<(), IndexError> {
        // DATA-5 FIX: Acquire read lock — concurrent with other store/delete,
        // but blocked during rebuild (write lock). Prevents duplicate entries
        // from concurrent insert + rebuild race.
        let _guard = self.compaction_lock.read();
        self.add_to_indexes_unlocked(fp)
    }

    /// Add fingerprint to indexes WITHOUT acquiring compaction_lock.
    /// Used by rebuild_indexes_from_store which holds the write lock.
    pub(crate) fn add_to_indexes_unlocked(&self, fp: &TeleologicalFingerprint) -> Result<(), IndexError> {
        let id = fp.id;

        // Add to all HNSW-capable dense embedder indexes
        for embedder in EmbedderIndex::all_hnsw() {
            if let Some(index) = self.index_registry.get(embedder) {
                let vector = Self::get_embedder_vector(&fp.semantic, embedder);
                index.insert(id, vector)?;
            }
        }

        debug!(
            "Added fingerprint {} to {} indexes",
            id,
            self.index_registry.len()
        );
        Ok(())
    }

    /// Extract vector for specific embedder from SemanticFingerprint.
    ///
    /// Returns the appropriate vector slice for the given embedder index.
    ///
    /// # ARCH-15, AP-77: E5 Asymmetric Indexes
    ///
    /// - E5CausalCause: Returns e5_causal_as_cause vector (for effect-seeking queries)
    /// - E5CausalEffect: Returns e5_causal_as_effect vector (for cause-seeking queries)
    /// - E5Causal: Returns active vector (legacy, for backward compatibility)
    ///
    /// # FAIL FAST
    ///
    /// Panics for embedders that don't use HNSW:
    /// - E6Sparse: Use inverted index
    /// - E12LateInteraction: Use MaxSim
    /// - E13Splade: Use inverted index
    pub(crate) fn get_embedder_vector(
        semantic: &SemanticFingerprint,
        embedder: EmbedderIndex,
    ) -> &[f32] {
        match embedder {
            EmbedderIndex::E1Semantic => &semantic.e1_semantic,
            EmbedderIndex::E1Matryoshka128 => {
                // Truncate E1 to 128D - return first 128 elements
                &semantic.e1_semantic[..128.min(semantic.e1_semantic.len())]
            }
            EmbedderIndex::E2TemporalRecent => &semantic.e2_temporal_recent,
            EmbedderIndex::E3TemporalPeriodic => &semantic.e3_temporal_periodic,
            EmbedderIndex::E4TemporalPositional => &semantic.e4_temporal_positional,
            // E5 legacy - uses active vector (whichever is populated)
            EmbedderIndex::E5Causal => semantic.e5_active_vector(),
            // E5 asymmetric indexes (ARCH-15, AP-77)
            // Cause index stores cause vectors - queried when seeking effects
            EmbedderIndex::E5CausalCause => semantic.get_e5_as_cause(),
            // Effect index stores effect vectors - queried when seeking causes
            EmbedderIndex::E5CausalEffect => semantic.get_e5_as_effect(),
            EmbedderIndex::E6Sparse => {
                panic!("FAIL FAST: E6 is sparse - use inverted index, not HNSW")
            }
            EmbedderIndex::E7Code => &semantic.e7_code,
            EmbedderIndex::E8Graph => semantic.e8_active_vector(),
            EmbedderIndex::E9HDC => &semantic.e9_hdc,
            // E10 legacy - uses active vector (whichever is populated)
            EmbedderIndex::E10Multimodal => semantic.e10_active_vector(),
            // E10 asymmetric indexes (ARCH-15, AP-77)
            // Paraphrase index stores paraphrase vectors - queried when seeking contexts
            EmbedderIndex::E10MultimodalParaphrase => semantic.get_e10_as_paraphrase(),
            // Context index stores context vectors - queried when seeking paraphrases
            EmbedderIndex::E10MultimodalContext => semantic.get_e10_as_context(),
            EmbedderIndex::E11Entity => &semantic.e11_entity,
            EmbedderIndex::E12LateInteraction => {
                panic!("FAIL FAST: E12 is late-interaction - use MaxSim, not HNSW")
            }
            EmbedderIndex::E13Splade => {
                panic!("FAIL FAST: E13 is sparse - use inverted index, not HNSW")
            }
        }
    }

    /// Remove fingerprint from all per-embedder indexes.
    ///
    /// Removes the ID from all 13 HNSW indexes (including E5CausalCause and E5CausalEffect).
    pub(crate) fn remove_from_indexes(&self, id: Uuid) -> Result<(), IndexError> {
        // DATA-5 FIX: Acquire read lock — concurrent with other store/delete,
        // but blocked during rebuild (write lock).
        let _guard = self.compaction_lock.read();

        for (_embedder, index) in self.index_registry.iter() {
            // Remove returns bool (found or not), we ignore it
            let _ = index.remove(id)?;
        }
        debug!("Removed fingerprint {} from all indexes", id);
        Ok(())
    }
}
