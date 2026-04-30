use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{ActionId, EventId, OutcomeId, StateId, TransitionId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::{ensure_finite, Validate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NORMALIZED_STATE_RECORD_VERSION: u8 = 1;
pub const NORMALIZED_ACTION_RECORD_VERSION: u8 = 1;
pub const NORMALIZED_OUTCOME_RECORD_VERSION: u8 = 1;
pub const STATE_TRANSITION_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedState {
    pub header: DjRecordHeader,
    pub state_id: StateId,
    pub fields: BTreeMap<String, FieldValue>,
    pub source_event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedAction {
    pub header: DjRecordHeader,
    pub action_id: ActionId,
    pub fields: BTreeMap<String, FieldValue>,
    pub source_event_id: EventId,
    pub action_origin: ActionOrigin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedOutcome {
    pub header: DjRecordHeader,
    pub outcome_id: OutcomeId,
    pub fields: BTreeMap<String, FieldValue>,
    pub source_event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionOrigin {
    Observed,
    Hypothetical,
    Planned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateTransition {
    pub header: DjRecordHeader,
    pub transition_id: TransitionId,
    pub prior_state: StateId,
    pub action: ActionId,
    pub outcome: OutcomeId,
    pub next_state: StateId,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Categorical { variant: String },
    Vector(Vec<f32>),
    UnixMs(i64),
}

impl FieldValue {
    pub fn validate(&self, field: &str) -> DynamicJepaResult<()> {
        match self {
            FieldValue::F64(value) if !value.is_finite() => {
                return Err(DynamicJepaError::validation(
                    field,
                    format!("f64 value must be finite, got {value}"),
                    "reject NaN and infinity before normalization",
                ));
            }
            FieldValue::Vector(values) => {
                if values.is_empty() {
                    return Err(DynamicJepaError::validation(
                        field,
                        "vector field must not be empty",
                        "write the declared vector dimension",
                    ));
                }
                for (idx, value) in values.iter().enumerate() {
                    ensure_finite(*value, &format!("{field}[{idx}]"))?;
                }
            }
            FieldValue::String(value) if value.contains("${") => {
                return Err(DynamicJepaError::validation(
                    field,
                    "environment variable substitution is forbidden",
                    "write explicit fixture values",
                ));
            }
            FieldValue::Categorical { variant } if variant.trim().is_empty() => {
                return Err(DynamicJepaError::validation(
                    field,
                    "categorical variant must not be empty",
                    "write one of the declared categorical variants",
                ));
            }
            FieldValue::UnixMs(value) if *value < 0 => {
                return Err(DynamicJepaError::validation(
                    field,
                    "UnixMs must be non-negative",
                    "write Unix epoch milliseconds",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_fields(fields: &BTreeMap<String, FieldValue>, field: &str) -> DynamicJepaResult<()> {
    if fields.is_empty() {
        return Err(DynamicJepaError::validation(
            field,
            "normalized field map must not be empty",
            "adapter output must contain typed fields",
        ));
    }
    for (name, value) in fields {
        if name.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                field,
                "field name must not be empty",
                "adapter mappings must use non-empty field names",
            ));
        }
        value.validate(&format!("{field}.{name}"))?;
    }
    Ok(())
}

impl Validate for NormalizedState {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.state_id.validate()?;
        self.source_event_id.validate()?;
        validate_fields(&self.fields, "NormalizedState.fields")
    }
}

impl Validate for NormalizedAction {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.action_id.validate()?;
        self.source_event_id.validate()?;
        validate_fields(&self.fields, "NormalizedAction.fields")
    }
}

impl Validate for NormalizedOutcome {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.outcome_id.validate()?;
        self.source_event_id.validate()?;
        validate_fields(&self.fields, "NormalizedOutcome.fields")
    }
}

impl Validate for StateTransition {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.transition_id.validate()?;
        self.prior_state.validate()?;
        self.action.validate()?;
        self.outcome.validate()?;
        self.next_state.validate()?;
        if self.timestamp_ms < 0 {
            return Err(DynamicJepaError::validation(
                "StateTransition.timestamp_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds or step-index milliseconds consistently",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    NormalizedState,
    NORMALIZED_STATE_RECORD_VERSION,
    "NormalizedState"
);
crate::impl_dynamic_jepa_record!(
    NormalizedAction,
    NORMALIZED_ACTION_RECORD_VERSION,
    "NormalizedAction"
);
crate::impl_dynamic_jepa_record!(
    NormalizedOutcome,
    NORMALIZED_OUTCOME_RECORD_VERSION,
    "NormalizedOutcome"
);
crate::impl_dynamic_jepa_record!(
    StateTransition,
    STATE_TRANSITION_RECORD_VERSION,
    "StateTransition"
);
