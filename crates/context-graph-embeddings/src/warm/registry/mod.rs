//! Warm Model Registry
//!
//! Tracks the loading state and VRAM handles for all 12 embedding models.
//!
//! # Overview
//!
//! The [`WarmModelRegistry`] is the central state manager for the warm loading system.
//! It maintains entries for each model component and tracks their lifecycle from
//! registration through loading, validation, and warm state.
//!
//! # Thread Safety
//!
//! The [`SharedWarmRegistry`] type alias provides thread-safe access via `Arc<RwLock<_>>`.
//! Multiple readers can access state concurrently, while writers have exclusive access.
//! All state-modifying methods return [`WarmResult`] to handle lock poisoning scenarios.
//!
//! # Model Components
//!
//! The registry tracks 12 embedding models:
//!
//! | Model ID | Description |
//! |----------|-------------|
//! | `E1_Semantic` | Semantic similarity embeddings |
//! | `E2_TemporalRecent` | Recent temporal context |
//! | `E3_TemporalPeriodic` | Periodic temporal patterns |
//! | `E4_TemporalPositional` | Positional temporal encoding |
//! | `E5_Causal` | Causal relationship embeddings |
//! | `E6_Sparse` | Sparse activation embeddings |
//! | `E7_Code` | Code/programming embeddings |
//! | `E8_Graph` | Graph structure embeddings |
//! | `E9_HDC` | Hyperdimensional computing embeddings |
//! | `E10_Multimodal` | Multimodal embeddings (CLIP) |
//! | `E11_Entity` | Named entity embeddings |
//! | `E12_LateInteraction` | Late interaction embeddings |
//!
//! # State Transitions
//!
//! Each model follows a strict state machine with the following valid transitions:
//!
//! ```text
//!                     +-------------+
//!                     |   Pending   |
//!                     +------+------+
//!                            |
//!                     start_loading()
//!                            |
//!                            v
//!                     +------+------+
//!                     |   Loading   |<----+
//!                     +------+------+     |
//!                            |            |
//!            +---------------+------------+
//!            |               |
//!     mark_validating()   update_progress()
//!            |
//!            v
//!     +------+------+
//!     |  Validating |
//!     +------+------+
//!            |
//!     mark_warm()
//!            |
//!            v
//!     +------+------+
//!     |    Warm     |
//!     +-------------+
//!
//! Note: mark_failed() can be called from Loading or Validating states
//!       to transition to Failed state.
//!
//!     Loading ----mark_failed()----> Failed
//!     Validating --mark_failed()---> Failed
//! ```
//!
//! # Requirements Fulfilled
//!
//! - **REQ-WARM-001**: Track all 12 embedding models
//! - **REQ-WARM-004**: Maintain VRAM residency via ModelHandle
//!
//! # Example
//!
//! ```ignore
//! use std::sync::{Arc, RwLock};
//! use context_graph_embeddings::warm::registry::{WarmModelRegistry, SharedWarmRegistry};
//!
//! // Create a shared registry
//! let registry: SharedWarmRegistry = Arc::new(RwLock::new(WarmModelRegistry::new()));
//!
//! // Register a model
//! {
//!     let mut reg = registry.write().unwrap();
//!     reg.register_model("E1_Semantic", 512 * 1024 * 1024, 768)?;
//!     reg.start_loading("E1_Semantic")?;
//!     reg.update_progress("E1_Semantic", 50, 256 * 1024 * 1024)?;
//!     reg.mark_validating("E1_Semantic")?;
//!     reg.mark_warm("E1_Semantic", handle)?;
//! }
//!
//! // Check status
//! {
//!     let reg = registry.read().unwrap();
//!     assert!(reg.get_state("E1_Semantic").map(|s| s.is_warm()).unwrap_or(false));
//! }
//! ```

mod core;
mod operations;
mod queries;
mod types;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_errors;
#[cfg(test)]
mod tests_queries;
#[cfg(test)]
mod tests_transitions;

// Re-export all public items for backwards compatibility
pub use self::core::WarmModelRegistry;
pub use self::types::{SharedWarmRegistry, WarmModelEntry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT};
