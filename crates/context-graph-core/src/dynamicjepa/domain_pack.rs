use crate::dynamicjepa::adapter::AdapterSpec;
use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{validate_string_id, DomainPackId, InstrumentId};
use crate::dynamicjepa::instrument::InstrumentSpec;
use crate::dynamicjepa::record_header::{validate_semver, DjRecordHeader};
use crate::dynamicjepa::schema::{
    ActionSchema, EntitySchema, FieldKind, OutcomeSchema, StateSchema, TimeSchema,
};
use crate::dynamicjepa::state_action_outcome::FieldValue;
use crate::dynamicjepa::validation::{ensure_no_duplicates, Validate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DOMAIN_PACK_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainPack {
    pub header: DjRecordHeader,
    pub id: DomainPackId,
    pub version: String,
    pub title: String,
    pub schema_version: u8,
    pub state_schema: StateSchema,
    pub action_schema: ActionSchema,
    pub outcome_schema: OutcomeSchema,
    pub entity_schema: EntitySchema,
    pub time_schema: TimeSchema,
    pub instrument_specs: Vec<InstrumentSpec>,
    pub adapter_specs: Vec<AdapterSpec>,
    pub objective_specs: Vec<ObjectiveSpec>,
    pub invariants: Vec<InvariantSpec>,
    pub dataset_policy: DatasetPolicy,
    pub planner_policy: PlannerPolicy,
    #[serde(default)]
    pub constellation: Option<ConstellationConfig>,
    pub verification_policy: VerificationPolicy,
    pub source_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveSpec {
    pub id: String,
    pub kind: String,
    pub input_panel: String,
    pub target: String,
    pub loss_weight: f64,
    pub required_dataset_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantSpec {
    pub id: String,
    pub expression: String,
    pub severity: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetPolicy {
    pub default_split: String,
    pub split_key: Option<String>,
    pub split_buckets: Option<BTreeMap<String, f64>>,
    pub default_objective: String,
    pub negative_sampling: String,
    pub min_negatives_per_row: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerPolicy {
    pub candidate_actions: CandidateActionConfig,
    pub guards: Vec<String>,
    pub surprise_threshold: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CandidateActionConfig {
    EnumeratedDeltas {
        deltas: Vec<i64>,
    },
    EnumeratedMoves {
        moves: Vec<String>,
    },
    EnumeratedActionRecords {
        records: Vec<BTreeMap<String, FieldValue>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub required_cfs: Vec<String>,
    pub expected_event_count: Option<u64>,
    pub expected_panel_count: Option<u64>,
    pub expected_dataset_row_count: Option<u64>,
    pub expected_trajectory_count: Option<u64>,
    pub expected_train_rows_min: Option<u64>,
    pub expected_train_rows_max: Option<u64>,
    pub expected_val_rows_min: Option<u64>,
    pub expected_val_rows_max: Option<u64>,
    pub expected_test_rows_min: Option<u64>,
    pub expected_test_rows_max: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstellationConfig {
    pub global: Option<ConstellationGlobalSpec>,
    pub subjects: Vec<ConstellationSubjectSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstellationGlobalSpec {
    pub modalities: ConstellationModalities,
    pub require_loo_min: f32,
    pub calibration_percentile: u8,
    pub calibration_set_size: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstellationSubjectSpec {
    pub id: String,
    pub field: String,
    pub value: String,
    pub modalities: ConstellationModalities,
    pub require_loo_min: f32,
    pub calibration_percentile: u8,
    pub calibration_set_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstellationModalities {
    All,
    List(Vec<InstrumentId>),
}

impl Validate for ObjectiveSpec {
    fn validate(&self) -> DynamicJepaResult<()> {
        crate::dynamicjepa::ids::validate_string_id(&self.id, "ObjectiveSpec.id")?;
        if self.kind != "latent_delta" {
            return Err(DynamicJepaError::validation(
                "ObjectiveSpec.kind",
                format!("unsupported objective kind {:?}", self.kind),
                "Phase 1/5090 demo supports latent_delta only",
            ));
        }
        if self.target.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "ObjectiveSpec.target",
                "objective target must not be empty",
                "point the objective at a declared state/action/outcome field",
            ));
        }
        if !self.loss_weight.is_finite() || self.loss_weight <= 0.0 {
            return Err(DynamicJepaError::validation(
                "ObjectiveSpec.loss_weight",
                format!(
                    "loss_weight must be finite and >0, got {}",
                    self.loss_weight
                ),
                "set a positive objective loss weight",
            ));
        }
        if self.required_dataset_fields.is_empty() {
            return Err(DynamicJepaError::validation(
                "ObjectiveSpec.required_dataset_fields",
                "objective must declare required dataset fields",
                "declare input_panel_id, target_panel_id, and action_id requirements",
            ));
        }
        Ok(())
    }
}

impl Validate for InvariantSpec {
    fn validate(&self) -> DynamicJepaResult<()> {
        crate::dynamicjepa::ids::validate_string_id(&self.id, "InvariantSpec.id")?;
        if self.expression.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "InvariantSpec.expression",
                "invariant expression must not be empty",
                "write the domain invariant expression explicitly",
            ));
        }
        if self.severity != "fatal" {
            return Err(DynamicJepaError::validation(
                "InvariantSpec.severity",
                format!("unsupported invariant severity {:?}", self.severity),
                "Phase 1/5090 demo uses fatal invariants only",
            ));
        }
        Ok(())
    }
}

impl Validate for DatasetPolicy {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.default_split.trim().is_empty() || self.default_objective.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "DatasetPolicy",
                "default_split and default_objective must not be empty",
                "declare deterministic dataset policy values",
            ));
        }
        if self.negative_sampling != "same_domain_different_outcome" {
            return Err(DynamicJepaError::validation(
                "DatasetPolicy.negative_sampling",
                format!("unsupported negative sampling {:?}", self.negative_sampling),
                "use same_domain_different_outcome for the 5090 demo",
            ));
        }
        if self.min_negatives_per_row == 0 {
            return Err(DynamicJepaError::validation(
                "DatasetPolicy.min_negatives_per_row",
                "min_negatives_per_row must be >= 1",
                "declare at least one real negative per row",
            ));
        }
        if let Some(buckets) = &self.split_buckets {
            let sum: f64 = buckets.values().sum();
            if (sum - 1.0).abs() > 1e-6 {
                return Err(DynamicJepaError::validation(
                    "DatasetPolicy.split_buckets",
                    format!("split bucket probabilities must sum to 1.0, got {sum}"),
                    "fix train/val/test bucket probabilities",
                ));
            }
        }
        Ok(())
    }
}

impl Validate for PlannerPolicy {
    fn validate(&self) -> DynamicJepaResult<()> {
        match &self.candidate_actions {
            CandidateActionConfig::EnumeratedDeltas { deltas } => {
                if deltas.is_empty() {
                    return Err(DynamicJepaError::validation(
                        "PlannerPolicy.candidate_actions.deltas",
                        "candidate deltas must not be empty",
                        "declare the finite action set",
                    ));
                }
            }
            CandidateActionConfig::EnumeratedMoves { moves } => {
                if moves.is_empty() {
                    return Err(DynamicJepaError::validation(
                        "PlannerPolicy.candidate_actions.moves",
                        "candidate moves must not be empty",
                        "declare the finite action set",
                    ));
                }
                ensure_no_duplicates(moves.iter().map(String::as_str), "PlannerPolicy.moves")?;
            }
            CandidateActionConfig::EnumeratedActionRecords { records } => {
                if records.is_empty() {
                    return Err(DynamicJepaError::validation(
                        "PlannerPolicy.candidate_actions.records",
                        "candidate action records must not be empty",
                        "declare the finite structured action set",
                    ));
                }
                for (idx, record) in records.iter().enumerate() {
                    if record.is_empty() {
                        return Err(DynamicJepaError::validation(
                            format!("PlannerPolicy.candidate_actions.records[{idx}]"),
                            "candidate action record must contain at least one field",
                            "declare every required action field in each structured candidate",
                        ));
                    }
                    for (field, value) in record {
                        if field.trim().is_empty() || field.contains('.') {
                            return Err(DynamicJepaError::validation(
                                format!("PlannerPolicy.candidate_actions.records[{idx}]"),
                                format!("invalid action field name {field:?}"),
                                "use bare action schema field names inside structured candidate records",
                            ));
                        }
                        value.validate(&format!(
                            "PlannerPolicy.candidate_actions.records[{idx}].{field}"
                        ))?;
                    }
                }
            }
        }
        if self.guards.is_empty() {
            return Err(DynamicJepaError::validation(
                "PlannerPolicy.guards",
                "planner must declare at least one guard",
                "use bounds_check for the 5090 demo",
            ));
        }
        if !self.surprise_threshold.is_finite() || !(0.0..=1.0).contains(&self.surprise_threshold) {
            return Err(DynamicJepaError::validation(
                "PlannerPolicy.surprise_threshold",
                format!(
                    "threshold must be finite in [0,1], got {}",
                    self.surprise_threshold
                ),
                "set a cosine threshold such as 0.85",
            ));
        }
        Ok(())
    }
}

impl Validate for VerificationPolicy {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.required_cfs.is_empty() {
            return Err(DynamicJepaError::validation(
                "VerificationPolicy.required_cfs",
                "required_cfs must not be empty",
                "list every CF the verification harness must inspect",
            ));
        }
        ensure_no_duplicates(
            self.required_cfs.iter().map(String::as_str),
            "VerificationPolicy.required_cfs",
        )
    }
}

impl Validate for DomainPack {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.id.validate()?;
        validate_semver(&self.version, "DomainPack.version")?;
        if self.version != self.header.domain_pack_version {
            return Err(DynamicJepaError::DomainPackVersionMismatch {
                id: self.id.to_string(),
                expected: self.version.clone(),
                actual: self.header.domain_pack_version.clone(),
            });
        }
        if !matches!(self.schema_version, 1 | 2) {
            return Err(DynamicJepaError::schema(
                "DomainPack.schema_version",
                format!("unsupported schema_version {}", self.schema_version),
                "supported DynamicJEPA domain-pack schema versions are 1 and 2",
            ));
        }
        if self.title.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "DomainPack.title",
                "title must not be empty",
                "write an operator-readable title",
            ));
        }
        self.state_schema.validate()?;
        self.action_schema.validate()?;
        self.outcome_schema.validate()?;
        self.entity_schema.validate()?;
        self.time_schema.validate()?;
        if self.instrument_specs.is_empty() {
            return Err(DynamicJepaError::validation(
                "DomainPack.instrument_specs",
                "at least one instrument is required",
                "declare the latent panel instrument set",
            ));
        }
        for spec in &self.instrument_specs {
            spec.validate()?;
        }
        ensure_no_duplicates(
            self.instrument_specs
                .iter()
                .map(|spec| spec.instrument_id.as_str()),
            "DomainPack.instrument_specs.instrument_id",
        )?;
        if self.adapter_specs.is_empty() {
            return Err(DynamicJepaError::validation(
                "DomainPack.adapter_specs",
                "at least one adapter is required",
                "declare json_event adapter mapping",
            ));
        }
        for adapter in &self.adapter_specs {
            adapter.validate()?;
        }
        ensure_no_duplicates(
            self.adapter_specs
                .iter()
                .map(|spec| spec.adapter_id.as_str()),
            "DomainPack.adapter_specs.adapter_id",
        )?;
        for instrument in &self.instrument_specs {
            if instrument.required {
                for input in &instrument.input_fields {
                    let produced = self
                        .adapter_specs
                        .iter()
                        .any(|adapter| adapter.mapping.contains_key(input));
                    if !produced {
                        return Err(DynamicJepaError::validation(
                            "DomainPack.instrument_specs.input_fields",
                            format!("required instrument input {input:?} is not produced by any adapter"),
                            "add an adapter mapping for every required instrument input",
                        ));
                    }
                }
            }
        }
        for objective in &self.objective_specs {
            objective.validate()?;
            if !self.path_exists(&objective.target) {
                return Err(DynamicJepaError::validation(
                    "DomainPack.objective_specs.target",
                    format!(
                        "objective target {:?} is not declared in schemas",
                        objective.target
                    ),
                    "point objectives at declared schema fields",
                ));
            }
        }
        for invariant in &self.invariants {
            invariant.validate()?;
        }
        self.dataset_policy.validate()?;
        self.planner_policy.validate()?;
        if let Some(constellation) = &self.constellation {
            self.validate_constellation_config(constellation)?;
        }
        self.verification_policy.validate()
    }
}

impl DomainPack {
    fn path_exists(&self, path: &str) -> bool {
        let Some((scope, name)) = path.split_once('.') else {
            return false;
        };
        let fields = match scope {
            "state" => &self.state_schema.fields,
            "action" => &self.action_schema.fields,
            "outcome" => &self.outcome_schema.fields,
            "entity" => &self.entity_schema.fields,
            "time" => return self.time_schema.field.name == name,
            _ => return false,
        };
        if fields.iter().any(|field| field.name == name) {
            return true;
        }
        if scope == "outcome" && name == "next_xy_onehot" {
            return fields.iter().any(|field| field.name == "next_x")
                && fields.iter().any(|field| field.name == "next_y");
        }
        false
    }

    pub fn action_field_kind(&self, name: &str) -> Option<&FieldKind> {
        self.action_schema
            .fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.kind)
    }

    fn validate_constellation_config(&self, config: &ConstellationConfig) -> DynamicJepaResult<()> {
        if config.global.is_none() && config.subjects.is_empty() {
            return Err(DynamicJepaError::validation(
                "DomainPack.constellation",
                "constellation config must declare a global spec or at least one subject spec",
                "remove the empty table or declare [constellation.global] / [[constellation.subjects]]",
            ));
        }
        if let Some(global) = &config.global {
            validate_constellation_common(
                &global.modalities,
                global.require_loo_min,
                global.calibration_percentile,
                global.calibration_set_size,
                &self.instrument_specs,
                "DomainPack.constellation.global",
            )?;
        }
        ensure_no_duplicates(
            config.subjects.iter().map(|subject| subject.id.as_str()),
            "DomainPack.constellation.subjects.id",
        )?;
        for subject in &config.subjects {
            validate_string_id(&subject.id, "DomainPack.constellation.subjects.id")?;
            let field = self
                .state_schema
                .fields
                .iter()
                .find(|field| field.name == subject.field)
                .ok_or_else(|| DynamicJepaError::ConstellationSubjectFieldUndeclared {
                    subject_id: subject.id.clone(),
                    field: subject.field.clone(),
                })?;
            match &field.kind {
                FieldKind::Categorical { variants } => {
                    if !variants.iter().any(|variant| variant == &subject.value) {
                        return Err(DynamicJepaError::ConstellationSubjectValueUndeclared {
                            subject_id: subject.id.clone(),
                            field: subject.field.clone(),
                            value: subject.value.clone(),
                        });
                    }
                }
                FieldKind::String => {
                    if subject.value.trim().is_empty() {
                        return Err(DynamicJepaError::ConstellationSubjectValueUndeclared {
                            subject_id: subject.id.clone(),
                            field: subject.field.clone(),
                            value: subject.value.clone(),
                        });
                    }
                }
                _ => {
                    return Err(DynamicJepaError::ConstellationSubjectValueUndeclared {
                        subject_id: subject.id.clone(),
                        field: subject.field.clone(),
                        value: subject.value.clone(),
                    });
                }
            }
            validate_constellation_common(
                &subject.modalities,
                subject.require_loo_min,
                subject.calibration_percentile,
                subject.calibration_set_size,
                &self.instrument_specs,
                "DomainPack.constellation.subjects",
            )?;
        }
        Ok(())
    }
}

fn validate_constellation_common(
    modalities: &ConstellationModalities,
    require_loo_min: f32,
    calibration_percentile: u8,
    calibration_set_size: u32,
    instruments: &[InstrumentSpec],
    field: &str,
) -> DynamicJepaResult<()> {
    if !require_loo_min.is_finite() || !(0.0..=1.0).contains(&require_loo_min) {
        return Err(DynamicJepaError::validation(
            format!("{field}.require_loo_min"),
            format!("require_loo_min must be finite in [0,1], got {require_loo_min}"),
            "choose a bounded cosine stability threshold",
        ));
    }
    if calibration_percentile > 100 {
        return Err(DynamicJepaError::validation(
            format!("{field}.calibration_percentile"),
            format!("calibration_percentile must be in [0,100], got {calibration_percentile}"),
            "choose a valid percentile",
        ));
    }
    if calibration_set_size == 0 {
        return Err(DynamicJepaError::validation(
            format!("{field}.calibration_set_size"),
            "calibration_set_size must be positive",
            "calibrate thresholds from at least one held-out event",
        ));
    }
    match modalities {
        ConstellationModalities::All => {
            if instruments.is_empty() {
                return Err(DynamicJepaError::validation(
                    format!("{field}.modalities"),
                    "modalities=\"all\" requires at least one instrument",
                    "declare instruments before enabling constellation guards",
                ));
            }
        }
        ConstellationModalities::List(ids) => {
            if ids.is_empty() {
                return Err(DynamicJepaError::validation(
                    format!("{field}.modalities"),
                    "modalities list must not be empty",
                    "use \"all\" or list concrete instrument ids",
                ));
            }
            ensure_no_duplicates(ids.iter().map(InstrumentId::as_str), field)?;
            for id in ids {
                if !instruments
                    .iter()
                    .any(|instrument| instrument.instrument_id == *id)
                {
                    return Err(DynamicJepaError::validation(
                        format!("{field}.modalities"),
                        format!("modality {id} is not declared in instrument_specs"),
                        "list only registered instrument ids",
                    ));
                }
            }
        }
    }
    Ok(())
}

crate::impl_dynamic_jepa_record!(DomainPack, DOMAIN_PACK_RECORD_VERSION, "DomainPack");
