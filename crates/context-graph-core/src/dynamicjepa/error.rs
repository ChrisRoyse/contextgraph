use crate::error::ContextGraphError;
use thiserror::Error;
use uuid::Uuid;

pub type DynamicJepaResult<T> = std::result::Result<T, DynamicJepaError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum DynamicJepaError {
    #[error("validation failed: {message} (field={field}) remediation={remediation}")]
    Validation {
        message: String,
        field: String,
        remediation: String,
    },
    #[error("source-of-truth missing: {cf} key={key:?}")]
    SourceOfTruthMissing { cf: String, key: Vec<u8> },
    #[error("storage: {operation} cf={cf}: {message} remediation={remediation}")]
    Storage {
        operation: String,
        cf: String,
        message: String,
        remediation: String,
    },
    #[error("codec: expected_version={expected} actual_version={actual} type={payload_type} remediation={remediation}")]
    Codec {
        expected: u8,
        actual: u8,
        payload_type: String,
        remediation: String,
    },
    #[error(
        "instrument {instrument_id} failed for event {event_id}: {message} field_path={field_path}"
    )]
    InstrumentFailed {
        instrument_id: String,
        event_id: Uuid,
        message: String,
        field_path: String,
    },
    #[error("required instrument {instrument_id} missing for transition {transition_id}")]
    RequiredInstrumentMissing {
        instrument_id: String,
        transition_id: Uuid,
    },
    #[error("panel shape mismatch: domain_pack={domain_pack_id} expected={expected:?} actual={actual:?}")]
    PanelShapeMismatch {
        domain_pack_id: String,
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    #[error("binding evidence missing: binding={binding_id} ref={evidence_ref}")]
    BindingEvidenceMissing {
        binding_id: Uuid,
        evidence_ref: String,
    },
    #[error("trajectory invariant violation: {message} trajectory_id={trajectory_id}")]
    TrajectoryInvariantViolation {
        message: String,
        trajectory_id: Uuid,
    },
    #[error("dataset leakage detected: {message} dataset_id={dataset_id}")]
    DatasetLeakageDetected { message: String, dataset_id: Uuid },
    #[error("training failed: run={training_run_id}: {message} remediation={remediation}")]
    TrainingFailed {
        training_run_id: Uuid,
        message: String,
        remediation: String,
    },
    #[error("artifact hash mismatch: artifact={artifact_id} file={file} expected={expected} actual={actual}")]
    ArtifactHashMismatch {
        artifact_id: Uuid,
        file: String,
        expected: String,
        actual: String,
    },
    #[error("prediction input missing: {target_id} cf={cf}")]
    PredictionInputMissing { target_id: String, cf: String },
    #[error("guard rejected action: guard={guard_id} reason={reason_code} action_id={action_id}")]
    GuardRejected {
        guard_id: String,
        reason_code: String,
        action_id: Uuid,
    },
    #[error("verification failed: {test_name}: {message} evidence_path={evidence_path}")]
    VerificationFailed {
        test_name: String,
        message: String,
        evidence_path: String,
    },
    #[error("storage invariant violation: {message}")]
    StorageInvariantViolation { message: String },
    #[error("domain pack not found: {id}")]
    DomainPackNotFound { id: String },
    #[error("domain pack version mismatch: id={id} expected={expected} actual={actual}")]
    DomainPackVersionMismatch {
        id: String,
        expected: String,
        actual: String,
    },
    #[error("schema validation failed: {message} (field={field}) remediation={remediation}")]
    SchemaValidationFailed {
        message: String,
        field: String,
        remediation: String,
    },
    #[error(
        "adapter failed: adapter={adapter_id} event={event_id}: {message} field_path={field_path}"
    )]
    AdapterFailed {
        adapter_id: String,
        event_id: Uuid,
        message: String,
        field_path: String,
    },
    #[error("bundle schema version mismatch: expected={expected} found={found} manifest={manifest_path}")]
    BundleSchemaVersionMismatch {
        expected: u32,
        found: u32,
        manifest_path: String,
    },
    #[error("pairwise cosine kind missing: instrument={instrument_id} pair_kinds={pair_kinds:?}")]
    PairwiseCosineKindMissing {
        instrument_id: String,
        pair_kinds: Vec<String>,
    },
    #[error("unsupported pairwise kind {kind} for pair {instrument_j}/{instrument_k}: {message}")]
    PairwiseUnsupportedKind {
        instrument_j: String,
        instrument_k: String,
        kind: String,
        message: String,
    },
    #[error(
        "pairwise asymmetric ordering: instrument_j={instrument_j} instrument_k={instrument_k}"
    )]
    PairwiseAsymmetricOrdering {
        instrument_j: String,
        instrument_k: String,
    },
    #[error("pairwise row count mismatch: event={event_id} expected={expected} actual={actual}")]
    PairwiseRowCountMismatch {
        event_id: Uuid,
        expected: u64,
        actual: u64,
    },
    #[error(
        "pairwise cosine drift: pairwise_id={pairwise_id} expected={expected} actual={actual}"
    )]
    PairwiseCosineDrift {
        pairwise_id: Uuid,
        expected: f32,
        actual: f32,
    },
    #[error(
        "pairwise instrument hash drift: pairwise_id={pairwise_id} instrument={instrument_id}"
    )]
    PairwiseInstrumentHashDrift {
        pairwise_id: Uuid,
        instrument_id: String,
    },
    #[error("constellation reference set too small: subject={subject_id} actual={actual} minimum={minimum}")]
    ConstellationReferenceSetTooSmall {
        subject_id: String,
        actual: usize,
        minimum: usize,
    },
    #[error("constellation zero-norm majority: subject={subject_id} modality={modality_id} dropped={dropped} reference_set_count={reference_set_count}")]
    ConstellationZeroNormMajority {
        subject_id: String,
        modality_id: String,
        dropped: u32,
        reference_set_count: u32,
    },
    #[error("constellation leave-one-out stability below threshold: subject={subject_id} modality={modality_id} loo={loo_stability} required={required}")]
    ConstellationLooBelowThreshold {
        subject_id: String,
        modality_id: String,
        loo_stability: f32,
        required: f32,
    },
    #[error("constellation instrument hash mismatch: constellation={constellation_id} modality={modality_id}")]
    ConstellationInstrumentHashMismatch {
        constellation_id: Uuid,
        modality_id: String,
    },
    #[error("calibration set is not disjoint: subject={subject_id} overlap_count={overlap_count}")]
    CalibrationSetNotDisjoint {
        subject_id: String,
        overlap_count: usize,
    },
    #[error("constellation subject field undeclared: subject={subject_id} field={field}")]
    ConstellationSubjectFieldUndeclared { subject_id: String, field: String },
    #[error(
        "constellation subject value undeclared: subject={subject_id} field={field} value={value}"
    )]
    ConstellationSubjectValueUndeclared {
        subject_id: String,
        field: String,
        value: String,
    },
    #[error("calibration supersession required: subject={subject_id} modality={modality_id}")]
    CalibrationSupersessionRequired {
        subject_id: String,
        modality_id: String,
    },
    #[error("guard constellation missing: domain={domain_id} subject={subject_id} modality={modality_id}")]
    GuardConstellationMissing {
        domain_id: String,
        subject_id: String,
        modality_id: String,
    },
    #[error(
        "guard threshold missing: domain={domain_id} subject={subject_id} modality={modality_id}"
    )]
    GuardThresholdMissing {
        domain_id: String,
        subject_id: String,
        modality_id: String,
    },
    #[error("guard instrument hash drift: guard={guard_id} modality={modality_id}")]
    GuardInstrumentHashDrift {
        guard_id: String,
        modality_id: String,
    },
    #[error("guard threshold duplicated: domain={domain_id} subject={subject_id} modality={modality_id}")]
    GuardThresholdDuplicated {
        domain_id: String,
        subject_id: String,
        modality_id: String,
    },
}

impl DynamicJepaError {
    pub fn validation(
        field: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
            remediation: remediation.into(),
        }
    }

    pub fn schema(
        field: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::SchemaValidationFailed {
            field: field.into(),
            message: message.into(),
            remediation: remediation.into(),
        }
    }

    pub fn codec(
        expected: u8,
        actual: u8,
        payload_type: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::Codec {
            expected,
            actual,
            payload_type: payload_type.into(),
            remediation: remediation.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "VALIDATION",
            Self::SourceOfTruthMissing { .. } => "SOURCE_OF_TRUTH_MISSING",
            Self::Storage { .. } => "STORAGE",
            Self::Codec { .. } => "CODEC",
            Self::InstrumentFailed { .. } => "INSTRUMENT_FAILED",
            Self::RequiredInstrumentMissing { .. } => "REQUIRED_INSTRUMENT_MISSING",
            Self::PanelShapeMismatch { .. } => "PANEL_SHAPE_MISMATCH",
            Self::BindingEvidenceMissing { .. } => "BINDING_EVIDENCE_MISSING",
            Self::TrajectoryInvariantViolation { .. } => "TRAJECTORY_INVARIANT_VIOLATION",
            Self::DatasetLeakageDetected { .. } => "DATASET_LEAKAGE_DETECTED",
            Self::TrainingFailed { .. } => "TRAINING_FAILED",
            Self::ArtifactHashMismatch { .. } => "ARTIFACT_HASH_MISMATCH",
            Self::PredictionInputMissing { .. } => "PREDICTION_INPUT_MISSING",
            Self::GuardRejected { .. } => "GUARD_REJECTED",
            Self::VerificationFailed { .. } => "VERIFICATION_FAILED",
            Self::StorageInvariantViolation { .. } => "STORAGE_INVARIANT_VIOLATION",
            Self::DomainPackNotFound { .. } => "DOMAIN_PACK_NOT_FOUND",
            Self::DomainPackVersionMismatch { .. } => "DOMAIN_PACK_VERSION_MISMATCH",
            Self::SchemaValidationFailed { .. } => "SCHEMA_VALIDATION_FAILED",
            Self::AdapterFailed { .. } => "ADAPTER_FAILED",
            Self::BundleSchemaVersionMismatch { .. } => "BUNDLE_SCHEMA_VERSION_MISMATCH",
            Self::PairwiseCosineKindMissing { .. } => "PAIRWISE_COSINE_KIND_MISSING",
            Self::PairwiseUnsupportedKind { .. } => "PAIRWISE_UNSUPPORTED_KIND",
            Self::PairwiseAsymmetricOrdering { .. } => "PAIRWISE_ASYMMETRIC_ORDERING",
            Self::PairwiseRowCountMismatch { .. } => "PAIRWISE_ROW_COUNT_MISMATCH",
            Self::PairwiseCosineDrift { .. } => "PAIRWISE_COSINE_DRIFT",
            Self::PairwiseInstrumentHashDrift { .. } => "PAIRWISE_INSTRUMENT_HASH_DRIFT",
            Self::ConstellationReferenceSetTooSmall { .. } => {
                "CONSTELLATION_REFERENCE_SET_TOO_SMALL"
            }
            Self::ConstellationZeroNormMajority { .. } => "CONSTELLATION_ZERO_NORM_MAJORITY",
            Self::ConstellationLooBelowThreshold { .. } => "CONSTELLATION_LOO_BELOW_THRESHOLD",
            Self::ConstellationInstrumentHashMismatch { .. } => {
                "CONSTELLATION_INSTRUMENT_HASH_MISMATCH"
            }
            Self::CalibrationSetNotDisjoint { .. } => "CALIBRATION_SET_NOT_DISJOINT",
            Self::ConstellationSubjectFieldUndeclared { .. } => {
                "CONSTELLATION_SUBJECT_FIELD_UNDECLARED"
            }
            Self::ConstellationSubjectValueUndeclared { .. } => {
                "CONSTELLATION_SUBJECT_VALUE_UNDECLARED"
            }
            Self::CalibrationSupersessionRequired { .. } => "CALIBRATION_SUPERSESSION_REQUIRED",
            Self::GuardConstellationMissing { .. } => "GUARD_CONSTELLATION_MISSING",
            Self::GuardThresholdMissing { .. } => "GUARD_THRESHOLD_MISSING",
            Self::GuardInstrumentHashDrift { .. } => "GUARD_INSTRUMENT_HASH_DRIFT",
            Self::GuardThresholdDuplicated { .. } => "GUARD_THRESHOLD_DUPLICATED",
        }
    }
}

impl From<DynamicJepaError> for ContextGraphError {
    fn from(err: DynamicJepaError) -> Self {
        match err {
            DynamicJepaError::Codec { .. }
            | DynamicJepaError::SourceOfTruthMissing { .. }
            | DynamicJepaError::Storage { .. }
            | DynamicJepaError::StorageInvariantViolation { .. } => {
                ContextGraphError::Storage(crate::error::StorageError::Corruption(err.to_string()))
            }
            DynamicJepaError::TrainingFailed { .. } => ContextGraphError::Internal(err.to_string()),
            _ => ContextGraphError::Validation(err.to_string()),
        }
    }
}
