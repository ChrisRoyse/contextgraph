//! Model path configuration.
//!
//! Controls model paths, lazy loading behavior, and preloaded models.

use serde::{Deserialize, Serialize};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

// ============================================================================
// DEFAULT FUNCTIONS
// ============================================================================

fn default_models_dir() -> String {
    "./models".to_string()
}

fn default_lazy_loading() -> bool {
    true
}

fn default_max_loaded_models() -> usize {
    12 // All models can be loaded by default
}

// ============================================================================
// MODEL PATH CONFIG
// ============================================================================

/// Configuration for model file paths and loading behavior.
///
/// Controls model paths, lazy loading behavior, and preloaded models.
/// Note: For runtime registry configuration (memory budget, concurrency),
/// use `models::ModelRegistryConfig` instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPathConfig {
    /// Directory containing model files.
    /// Relative paths are resolved from working directory.
    #[serde(default = "default_models_dir")]
    pub models_dir: String,

    /// Whether to load models lazily (on first use) or eagerly.
    /// Lazy loading reduces startup time but increases first-request latency.
    #[serde(default = "default_lazy_loading")]
    pub lazy_loading: bool,

    /// Models to preload on startup (by name).
    /// Only effective when lazy_loading is false.
    /// Valid values: "semantic", "temporal_recent", "temporal_periodic",
    /// "temporal_positional", "causal", "sparse", "code", "graph",
    /// "hdc", "contextual", "entity", "kepler", "late_interaction", "splade"
    #[serde(default)]
    pub preload_models: Vec<String>,

    /// Maximum number of models to keep loaded simultaneously.
    /// When exceeded, least recently used models are unloaded.
    /// 0 means unlimited (all 13 models can be loaded).
    #[serde(default = "default_max_loaded_models")]
    pub max_loaded_models: usize,
}

impl Default for ModelPathConfig {
    fn default() -> Self {
        Self {
            models_dir: default_models_dir(),
            lazy_loading: default_lazy_loading(),
            preload_models: Vec::new(),
            max_loaded_models: default_max_loaded_models(),
        }
    }
}

impl ModelPathConfig {
    /// Validate the configuration.
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if models_dir is empty
    /// - `EmbeddingError::ConfigError` if preload_models contains invalid model names
    pub fn validate(&self) -> EmbeddingResult<()> {
        if self.models_dir.is_empty() {
            return Err(EmbeddingError::ConfigError {
                message: "models_dir cannot be empty".to_string(),
            });
        }

        // Validate preload model names
        let valid_names: Vec<&str> = ModelId::all().iter().map(|id| id.as_str()).collect();

        for name in &self.preload_models {
            let normalized = name.to_lowercase().replace('-', "_");
            if !valid_names
                .iter()
                .any(|v| v.to_lowercase().replace('-', "_") == normalized)
            {
                return Err(EmbeddingError::ConfigError {
                    message: format!(
                        "Invalid preload model name: '{}'. Valid names: {:?}",
                        name, valid_names
                    ),
                });
            }
        }

        Ok(())
    }
}
