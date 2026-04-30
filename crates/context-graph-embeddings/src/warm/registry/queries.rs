//! Query methods for the warm model registry.
//!
//! Contains methods for querying registry state:
//! - `all_warm`: Check if all models are warm
//! - `any_failed`: Check if any model has failed
//! - `warm_count`: Get count of warm models
//! - `loading_order`: Get optimal loading order by size
//! - `failed_entries`: Get information about failed models

use super::core::WarmModelRegistry;
use crate::warm::state::WarmModelState;

impl WarmModelRegistry {
    /// Check if all registered models are in Warm state.
    ///
    /// Returns `true` only if:
    /// - At least one model is registered
    /// - All registered models have `state == Warm`
    ///
    /// # Example
    ///
    /// ```ignore
    /// if registry.all_warm() {
    ///     println!("All {} models ready for inference", registry.model_count());
    /// }
    /// ```
    #[must_use]
    pub fn all_warm(&self) -> bool {
        !self.entries.is_empty() && self.entries.values().all(|e| e.state.is_warm())
    }

    /// Check if any registered model is in Failed state.
    ///
    /// Returns `true` if at least one model has `state == Failed`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if registry.any_failed() {
    ///     for (id, code, msg) in registry.failed_entries() {
    ///         eprintln!("Model {} failed: [{}] {}", id, code, msg);
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn any_failed(&self) -> bool {
        self.entries.values().any(|e| e.state.is_failed())
    }

    /// Get model IDs sorted by expected_bytes in descending order.
    ///
    /// This determines the optimal loading order: largest models first
    /// to maximize VRAM utilization and catch memory issues early.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` of model IDs sorted from largest to smallest.
    ///
    /// # Example
    ///
    /// ```ignore
    /// for model_id in registry.loading_order() {
    ///     registry.start_loading(&model_id)?;
    ///     // ... load model ...
    /// }
    /// ```
    #[must_use]
    pub fn loading_order(&self) -> Vec<String> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|(id, e)| (id.clone(), e.expected_bytes))
            .collect();

        // Sort by expected_bytes descending (largest first)
        entries.sort_by_key(|(_, expected_bytes)| std::cmp::Reverse(*expected_bytes));

        entries.into_iter().map(|(id, _)| id).collect()
    }

    /// Get the number of models in Warm state.
    #[must_use]
    pub fn warm_count(&self) -> usize {
        self.entries.values().filter(|e| e.state.is_warm()).count()
    }

    /// Get information about all failed models.
    ///
    /// # Returns
    ///
    /// A `Vec` of tuples containing `(model_id, error_code, error_message)`
    /// for each model in Failed state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// for (model_id, error_code, error_message) in registry.failed_entries() {
    ///     log::error!("Model {} failed with code {}: {}", model_id, error_code, error_message);
    /// }
    /// ```
    #[must_use]
    pub fn failed_entries(&self) -> Vec<(String, u16, String)> {
        self.entries
            .iter()
            .filter_map(|(id, entry)| {
                if let WarmModelState::Failed {
                    error_code,
                    error_message,
                } = &entry.state
                {
                    Some((id.clone(), *error_code, error_message.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}
