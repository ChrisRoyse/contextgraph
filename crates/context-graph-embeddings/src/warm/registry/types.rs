//! Type definitions for the warm model registry.
//!
//! Contains constants, type aliases, and the [`WarmModelEntry`] struct.

use std::sync::{Arc, RwLock};

use super::core::WarmModelRegistry;
use crate::warm::handle::ModelHandle;
use crate::warm::state::WarmModelState;

/// The 14 embedding model IDs in the system (13 pipeline slots + Kepler production E11).
pub const EMBEDDING_MODEL_IDS: [&str; 14] = [
    "E1_Semantic",
    "E2_TemporalRecent",
    "E3_TemporalPeriodic",
    "E4_TemporalPositional",
    "E5_Causal",
    "E6_Sparse",
    "E7_Code",
    "E8_Graph",
    "E9_HDC",
    "E10_Contextual",
    "E11_Entity",
    "E12_LateInteraction",
    "E13_Splade",
    "E11_Kepler",
];

/// Total number of model components (14 embeddings: 13 pipeline + Kepler).
pub const TOTAL_MODEL_COUNT: usize = 14;

/// Thread-safe shared registry for concurrent access.
///
/// Wraps [`WarmModelRegistry`] in `Arc<RwLock<_>>` for safe multi-threaded access.
/// Use `read()` for shared read access and `write()` for exclusive write access.
///
/// # Lock Poisoning
///
/// If a thread panics while holding the lock, subsequent access attempts will
/// encounter a poisoned lock. Handle this gracefully by returning
/// [`WarmError::RegistryLockPoisoned`].
pub type SharedWarmRegistry = Arc<RwLock<WarmModelRegistry>>;

/// Entry for a single model in the registry.
///
/// Tracks the complete lifecycle state of a model from registration through
/// warm state, including VRAM allocation metadata.
#[derive(Debug)]
pub struct WarmModelEntry {
    /// Current state in the loading lifecycle.
    pub state: WarmModelState,
    /// VRAM handle when model is in Warm state, None otherwise.
    pub handle: Option<ModelHandle>,
    /// Expected size of model weights in bytes.
    pub expected_bytes: usize,
    /// Expected output embedding dimension.
    pub expected_dimension: usize,
    /// Unique model identifier (e.g., "E1_Semantic").
    pub model_id: String,
}

impl WarmModelEntry {
    /// Create a new entry in Pending state.
    pub(crate) fn new(
        model_id: impl Into<String>,
        expected_bytes: usize,
        expected_dimension: usize,
    ) -> Self {
        Self {
            state: WarmModelState::Pending,
            handle: None,
            expected_bytes,
            expected_dimension,
            model_id: model_id.into(),
        }
    }
}
