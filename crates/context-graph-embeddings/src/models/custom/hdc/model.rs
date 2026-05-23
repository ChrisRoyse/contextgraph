//! HdcModel struct and EmbeddingModel trait implementation.
//!
//! The main HDC model that encodes text/code into hypervectors and projects
//! them to floating-point embeddings for fusion.

use super::encoding::{
    apply_text_identity_residual, char_hypervector, encode_text, position_hypervector,
    project_to_float, random_hypervector,
};
use super::operations::{bind, bundle, hamming_distance, permute, similarity};
use super::types::{Hypervector, DEFAULT_NGRAM_SIZE, DEFAULT_SEED, HDC_PROJECTED_DIMENSION};
use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tracing::{debug, instrument};

/// Hyperdimensional Computing (HDC) embedding model.
///
/// Encodes text/code into 10K-bit binary hypervectors using character n-grams
/// with positional binding. Projects to 1024D float for fusion.
///
/// Thread-safe via atomic operations. `Send + Sync`.
#[derive(Debug)]
pub struct HdcModel {
    ngram_size: usize,
    seed: u64,
    initialized: AtomicBool,
}

impl HdcModel {
    /// Creates a new HDC model with specified parameters.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if ngram_size is 0.
    pub fn new(ngram_size: usize, seed: u64) -> EmbeddingResult<Self> {
        if ngram_size == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "HDC ngram_size must be >= 1".to_string(),
            });
        }

        debug!(ngram_size = ngram_size, seed = seed, "Creating HdcModel");

        Ok(Self {
            ngram_size,
            seed,
            initialized: AtomicBool::new(true),
        })
    }

    /// Creates a new HDC model with default parameters (trigrams, default seed).
    #[must_use]
    pub fn default_model() -> Self {
        Self {
            ngram_size: DEFAULT_NGRAM_SIZE,
            seed: DEFAULT_SEED,
            initialized: AtomicBool::new(true),
        }
    }

    /// Returns the n-gram size.
    #[must_use]
    pub const fn ngram_size(&self) -> usize {
        self.ngram_size
    }

    /// Returns the seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Generates a random hypervector with ~50% bits set.
    #[must_use]
    pub fn random_hypervector(&self, key: u64) -> Hypervector {
        random_hypervector(self.seed, key)
    }

    /// Generates a hypervector for a character.
    #[must_use]
    pub fn char_hypervector(&self, c: char) -> Hypervector {
        char_hypervector(self.seed, c)
    }

    /// Generates a position permutation vector.
    #[must_use]
    pub fn position_hypervector(&self, position: usize) -> Hypervector {
        position_hypervector(self.seed, position)
    }

    /// Binds two hypervectors using XOR (commutative, self-inverse).
    #[must_use]
    pub fn bind(a: &Hypervector, b: &Hypervector) -> Hypervector {
        bind(a, b)
    }

    /// Bundles multiple hypervectors using majority vote.
    #[must_use]
    pub fn bundle(vectors: &[Hypervector]) -> Hypervector {
        bundle(vectors)
    }

    /// Permutes a hypervector by circular left shift.
    #[must_use]
    pub fn permute(hv: &Hypervector, shift: usize) -> Hypervector {
        permute(hv, shift)
    }

    /// Computes Hamming distance between two hypervectors.
    #[must_use]
    pub fn hamming_distance(a: &Hypervector, b: &Hypervector) -> usize {
        hamming_distance(a, b)
    }

    /// Computes normalized similarity [0,1] where 1=identical, 0.5=orthogonal.
    #[must_use]
    pub fn similarity(a: &Hypervector, b: &Hypervector) -> f32 {
        similarity(a, b)
    }

    /// Encodes text into a hypervector using character n-grams.
    #[must_use]
    pub fn encode_text(&self, text: &str) -> Hypervector {
        encode_text(self.seed, self.ngram_size, text)
    }

    /// Projects a hypervector to 1024D float (L2 normalized).
    #[must_use]
    pub fn project_to_float(&self, hv: &Hypervector) -> Vec<f32> {
        project_to_float(hv)
    }

    /// Validates the projected embedding.
    fn validate_embedding(&self, vector: &[f32]) -> EmbeddingResult<()> {
        if vector.len() != HDC_PROJECTED_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: HDC_PROJECTED_DIMENSION,
                actual: vector.len(),
            });
        }

        for (idx, &val) in vector.iter().enumerate() {
            if !val.is_finite() {
                return Err(EmbeddingError::InvalidValue {
                    index: idx,
                    value: val,
                });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EmbeddingModel for HdcModel {
    fn model_id(&self) -> ModelId {
        ModelId::Hdc
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text, InputType::Code]
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    #[instrument(skip(self, input), fields(model = "Hdc"))]
    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        let start = Instant::now();

        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Hdc,
            });
        }

        self.validate_input(input)?;

        let text = match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                if let Some(inst) = instruction {
                    format!("{} {}", inst, content)
                } else {
                    content.clone()
                }
            }
            ModelInput::Code { content, language } => {
                format!("<{}> {}", language, content)
            }
            _ => {
                return Err(EmbeddingError::UnsupportedModality {
                    model_id: ModelId::Hdc,
                    input_type: InputType::from(input),
                });
            }
        };

        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let hypervector = self.encode_text(&text);
        let mut vector = self.project_to_float(&hypervector);
        apply_text_identity_residual(&mut vector, self.seed, &text);
        self.validate_embedding(&vector)?;

        let latency_us = start.elapsed().as_micros() as u64;
        debug!(latency_us = latency_us, "HDC embedding complete");

        Ok(ModelEmbedding::new(ModelId::Hdc, vector, latency_us))
    }
}
