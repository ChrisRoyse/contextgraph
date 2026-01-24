//! Repository and path-related methods for ModelId.

use std::path::{Path, PathBuf};

use super::core::ModelId;

impl ModelId {
    /// Returns the HuggingFace repository name for pretrained models.
    ///
    /// # Returns
    /// - `Some("repo/name")` for pretrained models
    /// - `None` for custom implementations
    #[must_use]
    pub const fn model_repo(&self) -> Option<&'static str> {
        match self {
            Self::Semantic => Some("intfloat/e5-large-v2"),
            Self::Causal => Some("allenai/longformer-base-4096"),
            Self::Sparse => Some("naver/splade-cocondenser-ensembledistil"),
            Self::Code => Some("Qodo/Qodo-Embed-1-1.5B"),
            Self::Graph => Some("sentence-transformers/paraphrase-MiniLM-L6-v2"),
            Self::Multimodal => Some("openai/clip-vit-large-patch14"),
            Self::Entity => Some("sentence-transformers/all-MiniLM-L6-v2"),
            Self::Kepler => Some("THU-KEG/KEPLER-Wiki5M-KE"), // KEPLER knowledge embeddings
            Self::LateInteraction => Some("colbert-ir/colbertv2.0"),
            Self::Splade => Some("prithivida/Splade_PP_en_v1"),
            Self::TemporalRecent
            | Self::TemporalPeriodic
            | Self::TemporalPositional
            | Self::Hdc => None,
        }
    }

    /// Returns the local directory name for this model's files.
    ///
    /// Maps to the subdirectory under the models base path.
    #[must_use]
    pub const fn directory_name(&self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::TemporalRecent | Self::TemporalPeriodic | Self::TemporalPositional => "temporal",
            Self::Causal => "causal",
            Self::Sparse => "sparse",
            Self::Code => "code",
            Self::Graph => "graph",
            Self::Hdc => "hdc",
            Self::Multimodal => "multimodal",
            Self::Entity => "entity",
            Self::Kepler => "kepler",
            Self::LateInteraction => "late-interaction",
            Self::Splade => "splade-v3",
        }
    }

    /// Constructs the full path to this model's directory.
    ///
    /// # Arguments
    /// * `base_dir` - Base directory containing all model subdirectories
    ///
    /// # Example
    /// ```rust
    /// use std::path::Path;
    /// use context_graph_embeddings::types::ModelId;
    ///
    /// let path = ModelId::Semantic.model_path(Path::new("/models"));
    /// assert_eq!(path, Path::new("/models/semantic"));
    /// ```
    #[must_use]
    pub fn model_path(&self, base_dir: &Path) -> PathBuf {
        base_dir.join(self.directory_name())
    }
}
