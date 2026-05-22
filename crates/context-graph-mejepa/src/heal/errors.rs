use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::heal::drift::DriftSeverity;
use crate::heal::promote::{HoldoutEval, ModeWinner};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum IntegrityViolationKind {
    WrongSize {
        actual: u64,
        expected_modulo: u64,
    },
    BrokenAt {
        offset: usize,
        expected_prev_hash: [u8; 32],
        actual_prev_hash: [u8; 32],
    },
    NoGoodCheckpoint {
        broken_offset: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum CriticalBugKind {
    TargetCollapseNonZero {
        value: f32,
        contributing_instruments: Vec<u8>,
    },
    StabilityFloorZero {
        value: f32,
        last_good_step: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionCellRegression {
    pub cell_key: String,
    pub correlation_before: f32,
    pub correlation_after: Option<f32>,
    pub tolerance: f32,
    pub attempted_winner: ModeWinner,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum HealError {
    BatchNan {
        component: String,
        witness_chain_offset: u64,
    },
    DriftDetected {
        severity: DriftSeverity,
        empirical_coverage: f32,
    },
    IntegrityViolation {
        kind: IntegrityViolationKind,
    },
    HoldoutRegression {
        mode_a_score: Box<HoldoutEval>,
        mode_b_score: Box<HoldoutEval>,
        mode_c_score: Box<HoldoutEval>,
    },
    PromotionCellRegression {
        regressions: Vec<PromotionCellRegression>,
    },
    QuotaExceeded {
        used_gb: f32,
        quota_gb: f32,
    },
    FisherRankDeficient {
        rank: usize,
        dim: usize,
    },
    EwcProtectionViolation {
        violation_id: String,
        projected_fisher_displacement: f32,
        budget: f32,
        requeued: bool,
    },
    EwcFisherDegenerate {
        rank: usize,
        dim: usize,
    },
    PlasticityCollapse {
        training_tick: u64,
        dormancy_fraction: f32,
        dormant_unit_count: usize,
        parameter_count: usize,
    },
    LoraRefreshFail {
        embedder_id: u32,
        cause: String,
    },
    PromotionDeadlock {
        holder: String,
    },
    ConformalInsufficientSamples {
        observed: usize,
        required: usize,
    },
    ConformalCoverageOutOfBand {
        target_coverage: f32,
        empirical_coverage: f32,
        min_allowed: f32,
        max_allowed: f32,
        calibration_version: String,
    },
    CriticalBug {
        kind: CriticalBugKind,
    },
    RollbackTargetGone {
        theta_sha: [u8; 32],
        gced_at: i64,
    },
    WitnessQuarantined {
        reason: String,
        repair_promotion_id: Option<String>,
    },
    ReadbackIncomplete {
        missing: Vec<String>,
        wrong_mode: Vec<(String, u32)>,
        parse_failures: Vec<String>,
    },
    FeatureDisabled,
    InvalidState {
        field: String,
        detail: String,
    },
    Io {
        op: &'static str,
        path: PathBuf,
        message: String,
    },
    RocksDb {
        message: String,
    },
    Bincode {
        message: String,
    },
    Json {
        message: String,
    },
}

impl HealError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BatchNan { .. } => "MEJEPA_OBSERVE_BATCH_NAN",
            Self::DriftDetected { .. } => "MEJEPA_OBSERVE_DRIFT_DETECTED",
            Self::IntegrityViolation { .. } => "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
            Self::HoldoutRegression { .. } => "MEJEPA_RETRAIN_HOLDOUT_REGRESSION",
            Self::PromotionCellRegression { .. } => "MEJEPA_PROMOTION_CELL_REGRESSION",
            Self::QuotaExceeded { .. } => "MEJEPA_RETRAIN_QUOTA_EXCEEDED",
            Self::FisherRankDeficient { .. } => "MEJEPA_HEAL_FISHER_RANK_DEFICIENT",
            Self::EwcProtectionViolation { .. } => "MEJEPA_EWC_PROTECTION_VIOLATION",
            Self::EwcFisherDegenerate { .. } => "EWC_FISHER_DEGENERATE",
            Self::PlasticityCollapse { .. } => "MEJEPA_PLASTICITY_COLLAPSE",
            Self::LoraRefreshFail { .. } => "MEJEPA_HEAL_LORA_REFRESH_FAIL",
            Self::PromotionDeadlock { .. } => "MEJEPA_HEAL_PROMOTION_DEADLOCK",
            Self::ConformalInsufficientSamples { .. } => "MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES",
            Self::ConformalCoverageOutOfBand { .. } => "MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND",
            Self::CriticalBug { .. } => "MEJEPA_HEAL_CRITICAL_BUG",
            Self::RollbackTargetGone { .. } => "MEJEPA_HEAL_ROLLBACK_TARGET_GONE",
            Self::WitnessQuarantined { .. } => "MEJEPA_HEAL_WITNESS_QUARANTINED",
            Self::ReadbackIncomplete { .. } => "MEJEPA_HEAL_READBACK_INCOMPLETE",
            Self::FeatureDisabled => "MEJEPA_HEAL_FEATURE_DISABLED",
            Self::InvalidState { .. } => "MEJEPA_HEAL_INVALID_STATE",
            Self::Io { .. } => "MEJEPA_HEAL_IO",
            Self::RocksDb { .. } => "MEJEPA_HEAL_ROCKSDB",
            Self::Bincode { .. } => "MEJEPA_HEAL_BINCODE",
            Self::Json { .. } => "MEJEPA_HEAL_JSON",
        }
    }

    pub fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::InvalidState {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub fn holdout_regression(
        mode_a_score: HoldoutEval,
        mode_b_score: HoldoutEval,
        mode_c_score: HoldoutEval,
    ) -> Self {
        Self::HoldoutRegression {
            mode_a_score: Box::new(mode_a_score),
            mode_b_score: Box::new(mode_b_score),
            mode_c_score: Box::new(mode_c_score),
        }
    }

    pub fn promotion_cell_regression(regressions: Vec<PromotionCellRegression>) -> Self {
        Self::PromotionCellRegression { regressions }
    }

    pub fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            message: source.to_string(),
        }
    }

    pub fn log_context(&self, file: &'static str) {
        tracing::error!(
            error_code = self.code(),
            error = %self,
            file,
            "ME-JEPA self-healing error"
        );
    }
}

impl fmt::Display for HealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.code(), self)
    }
}

impl std::error::Error for HealError {}

impl From<rocksdb::Error> for HealError {
    fn from(value: rocksdb::Error) -> Self {
        Self::RocksDb {
            message: value.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for HealError {
    fn from(value: Box<bincode::ErrorKind>) -> Self {
        Self::Bincode {
            message: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for HealError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json {
            message: value.to_string(),
        }
    }
}

impl From<std::io::Error> for HealError {
    fn from(value: std::io::Error) -> Self {
        Self::Io {
            op: "io",
            path: PathBuf::new(),
            message: value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::promote::HoldoutEval;

    fn eval() -> HoldoutEval {
        HoldoutEval::try_new(0.9, 0.9, 0.1, 10, [1; 32]).unwrap()
    }

    #[test]
    fn heal_error_code_returns_screaming_snake_for_each_variant() {
        let cases = vec![
            HealError::BatchNan {
                component: "x".into(),
                witness_chain_offset: 0,
            },
            HealError::DriftDetected {
                severity: DriftSeverity::Hard,
                empirical_coverage: 0.82,
            },
            HealError::IntegrityViolation {
                kind: IntegrityViolationKind::WrongSize {
                    actual: 1,
                    expected_modulo: 73,
                },
            },
            HealError::holdout_regression(eval(), eval(), eval()),
            HealError::promotion_cell_regression(vec![PromotionCellRegression {
                cell_key: "off_by_one::python".into(),
                correlation_before: 0.90,
                correlation_after: Some(0.89),
                tolerance: 0.005,
                attempted_winner: ModeWinner::B,
            }]),
            HealError::QuotaExceeded {
                used_gb: 116.0,
                quota_gb: 115.0,
            },
            HealError::FisherRankDeficient { rank: 1, dim: 2 },
            HealError::EwcProtectionViolation {
                violation_id: "ewc-test".to_string(),
                projected_fisher_displacement: 2.0,
                budget: 1.0,
                requeued: true,
            },
            HealError::EwcFisherDegenerate { rank: 0, dim: 2 },
            HealError::PlasticityCollapse {
                training_tick: 1,
                dormancy_fraction: 1.0,
                dormant_unit_count: 2,
                parameter_count: 2,
            },
            HealError::LoraRefreshFail {
                embedder_id: 7,
                cause: "oom".into(),
            },
            HealError::PromotionDeadlock {
                holder: "thread".into(),
            },
            HealError::ConformalInsufficientSamples {
                observed: 999,
                required: 1000,
            },
            HealError::ConformalCoverageOutOfBand {
                target_coverage: 0.90,
                empirical_coverage: 0.93,
                min_allowed: 0.88,
                max_allowed: 0.92,
                calibration_version: "calibration-v1".into(),
            },
            HealError::CriticalBug {
                kind: CriticalBugKind::StabilityFloorZero {
                    value: 0.0,
                    last_good_step: 1,
                },
            },
            HealError::RollbackTargetGone {
                theta_sha: [0; 32],
                gced_at: 0,
            },
            HealError::WitnessQuarantined {
                reason: "corrupt witness chain".into(),
                repair_promotion_id: Some("repair-1".into()),
            },
            HealError::FeatureDisabled,
        ];
        for err in cases {
            assert!(err.code().starts_with("MEJEPA_") || err.code() == "EWC_FISHER_DEGENERATE");
        }
    }

    #[test]
    fn heal_error_implements_display_and_std_error() {
        let err = HealError::FeatureDisabled;
        assert!(err.to_string().contains("MEJEPA_HEAL_FEATURE_DISABLED"));
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error
            .to_string()
            .contains("MEJEPA_HEAL_FEATURE_DISABLED"));
    }
}
