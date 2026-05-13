//! TASK-PREDICT-LABEL-011 — Cross-language label gating.
//!
//! Per `TECH-SECURITY §4`, labels learned in one language only transfer
//! to another language when:
//!
//! 1. The two language sites share the same `RootCauseClass`.
//! 2. The patch-similarity score is strictly greater than
//!    `SIMILARITY_THRESHOLD`.
//! 3. The cross-language correlation field on the source label is at
//!    least `CORRELATION_THRESHOLD`.
//!
//! Transferred labels are downweighted by `TRANSFERRED_LABEL_WEIGHT`
//! (`0.5×`) so the trainer never assumes cross-language transfer is as
//! authoritative as a same-language label.
//!
//! Fail-closed: malformed inputs (NaN, negative weight, missing fields)
//! return `MEJEPA_LABEL_TRANSFER_*` error codes rather than silently
//! defaulting to no-transfer.

use crate::types::{Language, RootCauseClass};
use serde::{Deserialize, Serialize};

pub const SIMILARITY_THRESHOLD: f32 = 0.85;
pub const CORRELATION_THRESHOLD: f32 = 0.60;
pub const TRANSFERRED_LABEL_WEIGHT: f32 = 0.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LabelTransferQuery {
    pub source_language: Language,
    pub target_language: Language,
    pub source_root_cause: RootCauseClass,
    pub target_root_cause: RootCauseClass,
    pub similarity: f32,
    pub source_cross_language_correlation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelTransferDecision {
    /// Source and target are the same language — no transfer needed.
    SameLanguage { weight: f32 },
    /// Transfer permitted at `TRANSFERRED_LABEL_WEIGHT`.
    Transfer { weight: f32 },
    /// Transfer rejected with a reason code.
    Reject { reason: RejectReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    RootCauseMismatch,
    SimilarityBelowThreshold,
    CorrelationBelowThreshold,
}

#[derive(Debug, thiserror::Error)]
pub enum LabelTransferError {
    #[error("MEJEPA_LABEL_TRANSFER_SIMILARITY_NOT_FINITE: similarity must be finite")]
    SimilarityNotFinite,
    #[error("MEJEPA_LABEL_TRANSFER_SIMILARITY_OUT_OF_RANGE: similarity must be in [0,1]; got {0}")]
    SimilarityOutOfRange(f32),
    #[error("MEJEPA_LABEL_TRANSFER_CORRELATION_NOT_FINITE: correlation must be finite")]
    CorrelationNotFinite,
    #[error(
        "MEJEPA_LABEL_TRANSFER_CORRELATION_OUT_OF_RANGE: correlation must be in [-1,1]; got {0}"
    )]
    CorrelationOutOfRange(f32),
}

/// Returns the label-transfer decision for the supplied
/// cross-language query, fail-closing on malformed inputs.
pub fn label_can_transfer(
    query: &LabelTransferQuery,
) -> Result<LabelTransferDecision, LabelTransferError> {
    validate(query)?;

    if query.source_language == query.target_language {
        return Ok(LabelTransferDecision::SameLanguage { weight: 1.0 });
    }
    if query.source_root_cause != query.target_root_cause {
        return Ok(LabelTransferDecision::Reject {
            reason: RejectReason::RootCauseMismatch,
        });
    }
    if query.similarity <= SIMILARITY_THRESHOLD {
        return Ok(LabelTransferDecision::Reject {
            reason: RejectReason::SimilarityBelowThreshold,
        });
    }
    if query.source_cross_language_correlation < CORRELATION_THRESHOLD {
        return Ok(LabelTransferDecision::Reject {
            reason: RejectReason::CorrelationBelowThreshold,
        });
    }
    Ok(LabelTransferDecision::Transfer {
        weight: TRANSFERRED_LABEL_WEIGHT,
    })
}

fn validate(query: &LabelTransferQuery) -> Result<(), LabelTransferError> {
    if !query.similarity.is_finite() {
        return Err(LabelTransferError::SimilarityNotFinite);
    }
    if !(0.0..=1.0).contains(&query.similarity) {
        return Err(LabelTransferError::SimilarityOutOfRange(query.similarity));
    }
    if !query.source_cross_language_correlation.is_finite() {
        return Err(LabelTransferError::CorrelationNotFinite);
    }
    if !(-1.0..=1.0).contains(&query.source_cross_language_correlation) {
        return Err(LabelTransferError::CorrelationOutOfRange(
            query.source_cross_language_correlation,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(
        same_lang: bool,
        same_root: bool,
        similarity: f32,
        correlation: f32,
    ) -> LabelTransferQuery {
        LabelTransferQuery {
            source_language: Language::Python,
            target_language: if same_lang {
                Language::Python
            } else {
                Language::Rust
            },
            source_root_cause: RootCauseClass::LogicError,
            target_root_cause: if same_root {
                RootCauseClass::LogicError
            } else {
                RootCauseClass::EnvironmentError
            },
            similarity,
            source_cross_language_correlation: correlation,
        }
    }

    #[test]
    fn same_language_passes_at_full_weight() {
        let decision = label_can_transfer(&query(true, true, 0.5, 0.5)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::SameLanguage { weight: 1.0 }
        );
    }

    #[test]
    fn cross_language_transfers_at_half_weight_when_gates_pass() {
        let decision = label_can_transfer(&query(false, true, 0.9, 0.8)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::Transfer {
                weight: TRANSFERRED_LABEL_WEIGHT,
            }
        );
    }

    #[test]
    fn root_cause_mismatch_rejects() {
        let decision = label_can_transfer(&query(false, false, 0.9, 0.8)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::Reject {
                reason: RejectReason::RootCauseMismatch,
            }
        );
    }

    #[test]
    fn low_similarity_rejects() {
        let decision = label_can_transfer(&query(false, true, 0.5, 0.9)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::Reject {
                reason: RejectReason::SimilarityBelowThreshold,
            }
        );
    }

    #[test]
    fn exact_similarity_threshold_rejects() {
        let decision = label_can_transfer(&query(false, true, SIMILARITY_THRESHOLD, 0.9)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::Reject {
                reason: RejectReason::SimilarityBelowThreshold,
            }
        );
    }

    #[test]
    fn low_correlation_rejects() {
        let decision = label_can_transfer(&query(false, true, 0.9, 0.3)).unwrap();
        assert_eq!(
            decision,
            LabelTransferDecision::Reject {
                reason: RejectReason::CorrelationBelowThreshold,
            }
        );
    }

    #[test]
    fn nan_similarity_fails_closed() {
        let err = label_can_transfer(&query(false, true, f32::NAN, 0.9)).unwrap_err();
        assert!(matches!(err, LabelTransferError::SimilarityNotFinite));
    }

    #[test]
    fn out_of_range_similarity_fails_closed() {
        let err = label_can_transfer(&query(false, true, 1.5, 0.9)).unwrap_err();
        assert!(matches!(err, LabelTransferError::SimilarityOutOfRange(_)));
    }

    #[test]
    fn out_of_range_correlation_fails_closed() {
        let err = label_can_transfer(&query(false, true, 0.9, -1.5)).unwrap_err();
        assert!(matches!(err, LabelTransferError::CorrelationOutOfRange(_)));
    }
}
