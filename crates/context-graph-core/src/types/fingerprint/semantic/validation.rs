//! Validation logic for SemanticFingerprint.
//!
//! This module provides dimension validation for the 13-embedding teleological array.
//! Per constitution.yaml:
//! - ARCH-01: "TeleologicalArray is atomic - store all 13 embeddings or nothing"
//! - ARCH-05: "All 13 embedders required - missing embedder is fatal error"
//! - AP-14: "No .unwrap() in library code"

use super::constants::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM,
    E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use super::fingerprint::{SemanticFingerprint, ValidationError};
use crate::teleological::Embedder;

impl SemanticFingerprint {
    /// Validate all embeddings have correct dimensions (fail-fast).
    ///
    /// This method performs a fail-fast validation of all embedding dimensions.
    /// Returns an error immediately if any dimension is incorrect.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All embeddings have correct dimensions
    /// * `Err(ValidationError)` - First validation failure found
    ///
    /// # Validation Rules
    ///
    /// 1. E1: Must have exactly 1024 dimensions
    /// 2. E2-E4: Must have exactly 512 dimensions each
    /// 3. E5: Must have exactly 768 dimensions
    /// 4. E6: Sparse indices must be < E6_SPARSE_VOCAB, indices/values must match length
    /// 5. E7: Must have exactly 1536 dimensions (Qodo-Embed)
    /// 6. E8, E11: Must have exactly 384 dimensions each
    /// 7. E9: Must have exactly 1024 dimensions (projected)
    /// 8. E10: Must have exactly 768 dimensions
    /// 9. E12: Each token must have exactly 128 dimensions
    /// 10. E13: Sparse indices must be < E13_SPLADE_VOCAB, indices/values must match length
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "test-utils")]
    /// # {
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let fp = SemanticFingerprint::zeroed();
    /// assert!(fp.validate().is_ok());
    /// # }
    /// ```
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_e1()?;
        self.validate_e2()?;
        self.validate_e3()?;
        self.validate_e4()?;
        self.validate_e5()?;
        self.validate_e6()?;
        self.validate_e7()?;
        self.validate_e8()?;
        self.validate_e9()?;
        self.validate_e10()?;
        self.validate_e11()?;
        self.validate_e12()?;
        self.validate_e13()?;
        Ok(())
    }

    /// Validate and collect ALL errors.
    ///
    /// Unlike `validate()` which returns on the first error,
    /// this method continues checking all embedders and returns
    /// a vector of all validation errors found.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All embeddings are valid
    /// * `Err(Vec<ValidationError>)` - All validation errors found
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "test-utils")]
    /// # {
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let mut fp = SemanticFingerprint::zeroed();
    /// fp.e1_semantic = vec![0.0; 100]; // Wrong dimension
    /// fp.e7_code = vec![0.0; 100];     // Wrong dimension
    ///
    /// let result = fp.validate_all();
    /// assert!(result.is_err());
    /// let errors = result.unwrap_err();
    /// assert_eq!(errors.len(), 2); // Both E1 and E7 errors
    /// # }
    /// ```
    pub fn validate_all(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if let Err(e) = self.validate_e1() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e2() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e3() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e4() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e5() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e6() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e7() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e8() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e9() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e10() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e11() {
            errors.push(e);
        }
        // E12 can have multiple errors (one per token), collect first only for consistency
        if let Err(e) = self.validate_e12() {
            errors.push(e);
        }
        if let Err(e) = self.validate_e13() {
            errors.push(e);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate E1 semantic embedding (1024D dense).
    fn validate_e1(&self) -> Result<(), ValidationError> {
        if self.e1_semantic.len() != E1_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Semantic,
                expected: E1_DIM,
                actual: self.e1_semantic.len(),
            });
        }
        Ok(())
    }

    /// Validate E2 temporal-recent embedding (512D dense).
    fn validate_e2(&self) -> Result<(), ValidationError> {
        if self.e2_temporal_recent.len() != E2_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalRecent,
                expected: E2_DIM,
                actual: self.e2_temporal_recent.len(),
            });
        }
        Ok(())
    }

    /// Validate E3 temporal-periodic embedding (512D dense).
    fn validate_e3(&self) -> Result<(), ValidationError> {
        if self.e3_temporal_periodic.len() != E3_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalPeriodic,
                expected: E3_DIM,
                actual: self.e3_temporal_periodic.len(),
            });
        }
        Ok(())
    }

    /// Validate E4 temporal-positional embedding (512D dense).
    fn validate_e4(&self) -> Result<(), ValidationError> {
        if self.e4_temporal_positional.len() != E4_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalPositional,
                expected: E4_DIM,
                actual: self.e4_temporal_positional.len(),
            });
        }
        Ok(())
    }

    /// Validate E5 causal embedding (768D dense).
    fn validate_e5(&self) -> Result<(), ValidationError> {
        if self.e5_causal.len() != E5_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Causal,
                expected: E5_DIM,
                actual: self.e5_causal.len(),
            });
        }
        Ok(())
    }

    /// Validate E6 sparse lexical embedding.
    ///
    /// Checks:
    /// 1. Indices and values have matching lengths
    /// 2. All indices are within vocabulary bounds (< 30522)
    fn validate_e6(&self) -> Result<(), ValidationError> {
        // Check length match first
        if self.e6_sparse.indices.len() != self.e6_sparse.values.len() {
            return Err(ValidationError::SparseIndicesValuesMismatch {
                embedder: Embedder::Sparse,
                indices_len: self.e6_sparse.indices.len(),
                values_len: self.e6_sparse.values.len(),
            });
        }

        // Check index bounds
        for &idx in &self.e6_sparse.indices {
            if idx as usize >= E6_SPARSE_VOCAB {
                return Err(ValidationError::SparseIndexOutOfBounds {
                    embedder: Embedder::Sparse,
                    index: idx as u32,
                    vocab_size: E6_SPARSE_VOCAB,
                });
            }
        }

        Ok(())
    }

    /// Validate E7 code embedding (1536D dense).
    fn validate_e7(&self) -> Result<(), ValidationError> {
        if self.e7_code.len() != E7_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Code,
                expected: E7_DIM,
                actual: self.e7_code.len(),
            });
        }
        Ok(())
    }

    /// Validate E8 graph/emotional embedding (384D dense).
    fn validate_e8(&self) -> Result<(), ValidationError> {
        if self.e8_graph.len() != E8_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Emotional,
                expected: E8_DIM,
                actual: self.e8_graph.len(),
            });
        }
        Ok(())
    }

    /// Validate E9 HDC embedding (1024D projected dense).
    fn validate_e9(&self) -> Result<(), ValidationError> {
        if self.e9_hdc.len() != E9_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Hdc,
                expected: E9_DIM,
                actual: self.e9_hdc.len(),
            });
        }
        Ok(())
    }

    /// Validate E10 multimodal embedding (768D dense).
    fn validate_e10(&self) -> Result<(), ValidationError> {
        if self.e10_multimodal.len() != E10_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Multimodal,
                expected: E10_DIM,
                actual: self.e10_multimodal.len(),
            });
        }
        Ok(())
    }

    /// Validate E11 entity embedding (384D dense).
    fn validate_e11(&self) -> Result<(), ValidationError> {
        if self.e11_entity.len() != E11_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Entity,
                expected: E11_DIM,
                actual: self.e11_entity.len(),
            });
        }
        Ok(())
    }

    /// Validate E12 late-interaction embedding (128D per token).
    ///
    /// Each token in the sequence must have exactly 128 dimensions.
    /// Empty token sequences (0 tokens) are valid.
    fn validate_e12(&self) -> Result<(), ValidationError> {
        for (i, token) in self.e12_late_interaction.iter().enumerate() {
            if token.len() != E12_TOKEN_DIM {
                return Err(ValidationError::TokenDimensionMismatch {
                    embedder: Embedder::LateInteraction,
                    token_index: i,
                    expected: E12_TOKEN_DIM,
                    actual: token.len(),
                });
            }
        }
        Ok(())
    }

    /// Validate E13 SPLADE sparse embedding.
    ///
    /// Checks:
    /// 1. Indices and values have matching lengths
    /// 2. All indices are within vocabulary bounds (< 30522)
    fn validate_e13(&self) -> Result<(), ValidationError> {
        // Check length match first
        if self.e13_splade.indices.len() != self.e13_splade.values.len() {
            return Err(ValidationError::SparseIndicesValuesMismatch {
                embedder: Embedder::KeywordSplade,
                indices_len: self.e13_splade.indices.len(),
                values_len: self.e13_splade.values.len(),
            });
        }

        // Check index bounds
        for &idx in &self.e13_splade.indices {
            if idx as usize >= E13_SPLADE_VOCAB {
                return Err(ValidationError::SparseIndexOutOfBounds {
                    embedder: Embedder::KeywordSplade,
                    index: idx as u32,
                    vocab_size: E13_SPLADE_VOCAB,
                });
            }
        }

        Ok(())
    }
}
