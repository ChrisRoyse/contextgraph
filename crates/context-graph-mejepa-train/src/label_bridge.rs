//! TASK-PY-G-112/#413 label bridge for mistake-driven learning.
//!
//! This module validates the deterministic baseline label artifacts produced by
//! TASK-PY-G-117 and turns them into compact runtime/training identities. It
//! deliberately preserves label and skill IDs instead of turning the embedder
//! panel into one fused semantic vector.

use crate::error::{TrainerError, TrainerErrorCode};
use crate::skill_validation;
use context_graph_mejepa::PredictionLabelContext;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

const BRIDGE_MANIFEST_FILE: &str = "learning_bridge_manifest.json";
const MAX_ID_BYTES: usize = 512;
const MAX_LABEL_IDS: usize = 256;
const MAX_SKILL_IDS: usize = 128;
const MAX_HIGHER_ABILITY_IDS: usize = 128;
const MAX_SOURCE_MEMBERSHIP_KEYS: usize = 512;
const MAX_FAILURE_EVIDENCE_SET_IDS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningUpdatePolicy {
    pub baseline_seeded_from: String,
    pub online_rescore: bool,
    pub promote_demote_with_new_outcomes: bool,
    pub target_labels_are_supervision_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedLabelRegistryRow {
    pub artifact_kind: String,
    pub schema_version: u32,
    pub formula_version: String,
    pub label_id: String,
    pub label_hash: String,
    pub family: String,
    pub status: String,
    pub support: u64,
    pub fail_rate: f64,
    pub lift_over_weighted_category_baseline: f64,
    pub live_prediction_input_allowed: bool,
    pub learning_update_policy: LearningUpdatePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelUsefulnessMetricRow {
    pub label_id: String,
    pub family: String,
    pub status: String,
    pub support: u64,
    pub leaky_target: bool,
    pub lift_over_weighted_category_baseline: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureEvidenceSetRow {
    #[serde(default)]
    pub artifact_kind: String,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub formula_version: String,
    pub code_state_key: String,
    pub labels: Vec<String>,
    #[serde(default)]
    pub live_predictor_labels: Vec<String>,
    #[serde(default)]
    pub target_supervision_labels: Vec<String>,
    pub localization_mode: String,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
    #[serde(default)]
    pub live_prediction_input_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeStateConstellationRow {
    #[serde(default)]
    pub group_key: String,
    #[serde(default)]
    pub code_state_key: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub live_predictor_labels: Vec<String>,
    #[serde(default)]
    pub target_supervision_labels: Vec<String>,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningBridgePaths {
    pub accepted_label_registry: String,
    pub label_usefulness_metrics: String,
    pub chunk_constellation_labels: String,
    pub code_state_constellations: String,
    pub failure_evidence_sets: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicLearningPolicy {
    pub baseline_only: bool,
    pub new_data_uses_same_compiler: bool,
    pub usefulness_rescored_over_time: bool,
    pub target_outcomes_supervise_but_are_not_live_inputs: bool,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningBridgeManifest {
    pub artifact_kind: String,
    pub schema_version: u32,
    pub formula_version: String,
    pub purpose: String,
    pub paths: LearningBridgePaths,
    pub consumers: BTreeMap<String, String>,
    pub dynamic_learning_policy: DynamicLearningPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelLearningBridge {
    pub manifest: LearningBridgeManifest,
    pub manifest_sha256: String,
    pub accepted_registry_sha256: String,
    pub usefulness_metrics_sha256: String,
    pub code_state_constellations_sha256: String,
    pub failure_evidence_sets_sha256: String,
    pub accepted_labels: BTreeMap<String, AcceptedLabelRegistryRow>,
    pub usefulness_metrics: BTreeMap<String, LabelUsefulnessMetricRow>,
    pub code_state_constellations: BTreeMap<String, CodeStateConstellationRow>,
    pub failure_evidence_sets: BTreeMap<String, FailureEvidenceSetRow>,
}

impl LabelLearningBridge {
    pub fn accepted_label(&self, label_id: &str) -> Option<&AcceptedLabelRegistryRow> {
        self.accepted_labels.get(label_id)
    }

    pub fn validate_prediction_labels(&self, label_ids: &[String]) -> Result<(), TrainerError> {
        validate_id_list("accepted_label_ids", label_ids, MAX_LABEL_IDS)?;
        for label_id in label_ids {
            let row = self.accepted_labels.get(label_id).ok_or_else(|| {
                invalid(
                    "accepted_label_ids",
                    format!("label_id {label_id} is not in accepted registry"),
                )
            })?;
            validate_accepted_label(row)?;
        }
        Ok(())
    }
}

pub fn load_label_learning_bridge(
    root: impl AsRef<Path>,
) -> Result<LabelLearningBridge, TrainerError> {
    let root = root.as_ref();
    let manifest_path = root.join(BRIDGE_MANIFEST_FILE);
    let manifest: LearningBridgeManifest = read_json_file(&manifest_path)?;
    validate_manifest(&manifest)?;
    validate_path_set(&manifest.paths)?;

    let accepted_path = resolve_bridge_path(root, &manifest.paths.accepted_label_registry);
    let usefulness_path = resolve_bridge_path(root, &manifest.paths.label_usefulness_metrics);
    let code_state_path = resolve_bridge_path(root, &manifest.paths.code_state_constellations);
    let failure_sets_path = resolve_bridge_path(root, &manifest.paths.failure_evidence_sets);
    let accepted_rows = read_jsonl::<AcceptedLabelRegistryRow>(&accepted_path)?;
    let usefulness_rows = read_jsonl::<LabelUsefulnessMetricRow>(&usefulness_path)?;
    let code_state_rows = read_jsonl::<CodeStateConstellationRow>(&code_state_path)?;
    let failure_rows = read_jsonl::<FailureEvidenceSetRow>(&failure_sets_path)?;

    let mut accepted_labels = BTreeMap::new();
    for row in accepted_rows {
        validate_accepted_label(&row)?;
        if accepted_labels.insert(row.label_id.clone(), row).is_some() {
            return Err(invalid("accepted_label_registry", "duplicate label_id"));
        }
    }
    if accepted_labels.is_empty() {
        return Err(invalid(
            "accepted_label_registry",
            "must contain at least one accepted label",
        ));
    }

    let mut usefulness_metrics = BTreeMap::new();
    for row in usefulness_rows {
        validate_label_id("label_usefulness_metrics.label_id", &row.label_id)?;
        if row.leaky_target && row.status.starts_with("accepted") {
            return Err(invalid(
                "label_usefulness_metrics.leaky_target",
                format!("accepted label {} is target-leaky", row.label_id),
            ));
        }
        usefulness_metrics.insert(row.label_id.clone(), row);
    }

    let mut code_state_constellations = BTreeMap::new();
    for row in code_state_rows {
        let key = validate_code_state_constellation(&row)?;
        if code_state_constellations.insert(key, row).is_some() {
            return Err(invalid(
                "code_state_constellations",
                "duplicate code-state key",
            ));
        }
    }

    let mut failure_evidence_sets = BTreeMap::new();
    for row in failure_rows {
        validate_failure_evidence_set(&row)?;
        if failure_evidence_sets
            .insert(row.code_state_key.clone(), row)
            .is_some()
        {
            return Err(invalid("failure_evidence_sets", "duplicate code_state_key"));
        }
    }

    Ok(LabelLearningBridge {
        manifest_sha256: sha256_file(&manifest_path)?,
        accepted_registry_sha256: sha256_file(&accepted_path)?,
        usefulness_metrics_sha256: sha256_file(&usefulness_path)?,
        code_state_constellations_sha256: sha256_file(&code_state_path)?,
        failure_evidence_sets_sha256: sha256_file(&failure_sets_path)?,
        manifest,
        accepted_labels,
        usefulness_metrics,
        code_state_constellations,
        failure_evidence_sets,
    })
}

pub fn build_prediction_label_context(
    bridge: &LabelLearningBridge,
    accepted_label_ids: Vec<String>,
    code_state_key: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    active_skill_ids: Vec<String>,
) -> Result<PredictionLabelContext, TrainerError> {
    build_prediction_label_context_with_abilities(
        bridge,
        accepted_label_ids,
        code_state_key,
        failure_evidence_set_ids,
        active_skill_ids,
        Vec::new(),
        Vec::new(),
    )
}

pub fn build_prediction_label_context_with_abilities(
    bridge: &LabelLearningBridge,
    accepted_label_ids: Vec<String>,
    code_state_key: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    active_higher_ability_ids: Vec<String>,
    source_membership_keys: Vec<String>,
) -> Result<PredictionLabelContext, TrainerError> {
    bridge.validate_prediction_labels(&accepted_label_ids)?;
    validate_id_list(
        "failure_evidence_set_ids",
        &failure_evidence_set_ids,
        MAX_FAILURE_EVIDENCE_SET_IDS,
    )?;
    validate_id_list("active_skill_ids", &active_skill_ids, MAX_SKILL_IDS)?;
    validate_id_list(
        "active_higher_ability_ids",
        &active_higher_ability_ids,
        MAX_HIGHER_ABILITY_IDS,
    )?;
    validate_id_list(
        "source_membership_keys",
        &source_membership_keys,
        MAX_SOURCE_MEMBERSHIP_KEYS,
    )?;
    if let Some(key) = &code_state_key {
        validate_label_id("code_state_key", key)?;
        let row = bridge.code_state_constellations.get(key).ok_or_else(|| {
            invalid(
                "code_state_key",
                format!("{key} is not present in code_state_constellations"),
            )
        })?;
        validate_code_state_live_input(bridge, row)?;
    }
    for id in &failure_evidence_set_ids {
        let row = bridge.failure_evidence_sets.get(id).ok_or_else(|| {
            invalid(
                "failure_evidence_set_ids",
                format!("{id} is not present in failure_evidence_sets"),
            )
        })?;
        if !row.live_prediction_input_allowed {
            return Err(invalid(
                "failure_evidence_set_ids",
                format!(
                    "{id} is target-side failure evidence and cannot be a live prediction input"
                ),
            ));
        }
        validate_failure_evidence_live_input(bridge, row)?;
    }
    Ok(PredictionLabelContext {
        accepted_label_ids: accepted_label_ids.clone(),
        code_state_key,
        failure_evidence_set_ids,
        active_skill_ids: active_skill_ids.clone(),
        active_higher_ability_ids: active_higher_ability_ids.clone(),
        source_membership_keys: source_membership_keys.clone(),
        accepted_registry_sha256: Some(bridge.accepted_registry_sha256.clone()),
        usefulness_metrics_sha256: Some(bridge.usefulness_metrics_sha256.clone()),
        learning_bridge_manifest_sha256: Some(bridge.manifest_sha256.clone()),
        label_signature_hash: Some(accepted_label_signature_hash(&accepted_label_ids)?),
        skill_signature_hash: if active_skill_ids.is_empty() {
            None
        } else {
            Some(skill_signature_hash(&active_skill_ids)?)
        },
        ability_signature_hash: if active_higher_ability_ids.is_empty() {
            None
        } else {
            Some(ability_signature_hash(&active_higher_ability_ids)?)
        },
        membership_signature_hash: if source_membership_keys.is_empty() {
            None
        } else {
            Some(membership_signature_hash(&source_membership_keys)?)
        },
        ..PredictionLabelContext::default()
    })
}

pub fn accepted_label_signature_hash(label_ids: &[String]) -> Result<String, TrainerError> {
    signature_hash("labels", label_ids)
}

pub fn skill_signature_hash(skill_ids: &[String]) -> Result<String, TrainerError> {
    ordered_signature_hash("skills", skill_ids, MAX_SKILL_IDS)
}

pub fn ability_signature_hash(ability_ids: &[String]) -> Result<String, TrainerError> {
    ordered_signature_hash("abilities", ability_ids, MAX_HIGHER_ABILITY_IDS)
}

pub fn membership_signature_hash(membership_keys: &[String]) -> Result<String, TrainerError> {
    ordered_signature_hash("memberships", membership_keys, MAX_SOURCE_MEMBERSHIP_KEYS)
}

fn signature_hash(prefix: &'static str, ids: &[String]) -> Result<String, TrainerError> {
    validate_id_list(prefix, ids, MAX_LABEL_IDS)?;
    if ids.is_empty() {
        return Err(invalid(prefix, "signature requires at least one id"));
    }
    let mut sorted = ids.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    for id in sorted {
        hasher.update(id.as_bytes());
        hasher.update([0]);
    }
    let digest = hex::encode(hasher.finalize());
    Ok(format!("{prefix}:{}", &digest[..12]))
}

fn ordered_signature_hash(
    prefix: &'static str,
    ids: &[String],
    max_items: usize,
) -> Result<String, TrainerError> {
    validate_id_list(prefix, ids, max_items)?;
    if ids.is_empty() {
        return Err(invalid(prefix, "signature requires at least one id"));
    }
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update([0]);
    for (idx, id) in ids.iter().enumerate() {
        hasher.update((idx as u64).to_le_bytes());
        hasher.update([0]);
        hasher.update(id.as_bytes());
        hasher.update([0]);
    }
    let digest = hex::encode(hasher.finalize());
    Ok(format!("{prefix}:{}", &digest[..12]))
}

fn validate_manifest(manifest: &LearningBridgeManifest) -> Result<(), TrainerError> {
    if manifest.schema_version == 0 {
        return Err(invalid("schema_version", "must be positive"));
    }
    if manifest.dynamic_learning_policy.flat_vector_concat_used {
        return Err(invalid(
            "dynamic_learning_policy.flat_vector_concat_used",
            "flat vector concatenation is forbidden as a semantic learning path",
        ));
    }
    if !manifest.dynamic_learning_policy.slot_identity_preserved {
        return Err(invalid(
            "dynamic_learning_policy.slot_identity_preserved",
            "slot identity must be preserved",
        ));
    }
    if !manifest
        .dynamic_learning_policy
        .target_outcomes_supervise_but_are_not_live_inputs
    {
        return Err(invalid(
            "dynamic_learning_policy.target_outcomes_supervise_but_are_not_live_inputs",
            "target labels may supervise but must not be live inputs",
        ));
    }
    Ok(())
}

fn validate_path_set(paths: &LearningBridgePaths) -> Result<(), TrainerError> {
    for (field, path) in [
        ("accepted_label_registry", &paths.accepted_label_registry),
        ("label_usefulness_metrics", &paths.label_usefulness_metrics),
        (
            "chunk_constellation_labels",
            &paths.chunk_constellation_labels,
        ),
        (
            "code_state_constellations",
            &paths.code_state_constellations,
        ),
        ("failure_evidence_sets", &paths.failure_evidence_sets),
    ] {
        validate_label_id(field, path)?;
    }
    Ok(())
}

fn resolve_bridge_path(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn validate_accepted_label(row: &AcceptedLabelRegistryRow) -> Result<(), TrainerError> {
    validate_label_id("accepted_label_registry.label_id", &row.label_id)?;
    validate_label_id("accepted_label_registry.label_hash", &row.label_hash)?;
    validate_label_id("accepted_label_registry.family", &row.family)?;
    if !row.status.starts_with("accepted") {
        return Err(invalid(
            "accepted_label_registry.status",
            format!("{} is not accepted", row.label_id),
        ));
    }
    if row.support == 0 {
        return Err(invalid(
            "accepted_label_registry.support",
            format!("{} has zero support", row.label_id),
        ));
    }
    if !row.live_prediction_input_allowed {
        return Err(invalid(
            "accepted_label_registry.live_prediction_input_allowed",
            format!(
                "{} is target-leaky or disallowed for live prediction",
                row.label_id
            ),
        ));
    }
    if !row.learning_update_policy.online_rescore
        || !row.learning_update_policy.promote_demote_with_new_outcomes
        || !row
            .learning_update_policy
            .target_labels_are_supervision_only
    {
        return Err(invalid(
            "accepted_label_registry.learning_update_policy",
            format!(
                "{} is missing online learning policy guarantees",
                row.label_id
            ),
        ));
    }
    Ok(())
}

fn validate_failure_evidence_set(row: &FailureEvidenceSetRow) -> Result<(), TrainerError> {
    validate_label_id("failure_evidence_set.code_state_key", &row.code_state_key)?;
    validate_id_list("failure_evidence_set.labels", &row.labels, MAX_LABEL_IDS)?;
    validate_id_list(
        "failure_evidence_set.live_predictor_labels",
        &row.live_predictor_labels,
        MAX_LABEL_IDS,
    )?;
    validate_id_list(
        "failure_evidence_set.target_supervision_labels",
        &row.target_supervision_labels,
        MAX_LABEL_IDS,
    )?;
    validate_live_label_list(
        "failure_evidence_set.live_predictor_labels",
        &row.live_predictor_labels,
    )?;
    validate_label_id(
        "failure_evidence_set.localization_mode",
        &row.localization_mode,
    )?;
    if !row.slot_identity_preserved || row.flat_vector_concat_used {
        return Err(invalid(
            "failure_evidence_set.slot_policy",
            "failure evidence must preserve slots and forbid flat-vector semantics",
        ));
    }
    Ok(())
}

fn validate_code_state_constellation(
    row: &CodeStateConstellationRow,
) -> Result<String, TrainerError> {
    let key = if row.group_key.trim().is_empty() {
        row.code_state_key.clone().unwrap_or_default()
    } else {
        row.group_key.clone()
    };
    validate_label_id("code_state_constellation.key", &key)?;
    validate_id_list(
        "code_state_constellation.labels",
        &row.labels,
        MAX_LABEL_IDS,
    )?;
    validate_id_list(
        "code_state_constellation.live_predictor_labels",
        &row.live_predictor_labels,
        MAX_LABEL_IDS,
    )?;
    validate_id_list(
        "code_state_constellation.target_supervision_labels",
        &row.target_supervision_labels,
        MAX_LABEL_IDS,
    )?;
    validate_live_label_list(
        "code_state_constellation.live_predictor_labels",
        &row.live_predictor_labels,
    )?;
    if !row.slot_identity_preserved || row.flat_vector_concat_used {
        return Err(invalid(
            "code_state_constellation.slot_policy",
            "code-state constellations must preserve slots and forbid flat-vector semantics",
        ));
    }
    Ok(key)
}

fn validate_code_state_live_input(
    bridge: &LabelLearningBridge,
    row: &CodeStateConstellationRow,
) -> Result<(), TrainerError> {
    validate_live_registry_membership(
        bridge,
        "code_state_constellation.live_predictor_labels",
        &row.live_predictor_labels,
    )
}

fn validate_failure_evidence_live_input(
    bridge: &LabelLearningBridge,
    row: &FailureEvidenceSetRow,
) -> Result<(), TrainerError> {
    validate_live_registry_membership(
        bridge,
        "failure_evidence_set.live_predictor_labels",
        &row.live_predictor_labels,
    )
}

fn validate_live_registry_membership(
    bridge: &LabelLearningBridge,
    field: &str,
    label_ids: &[String],
) -> Result<(), TrainerError> {
    if label_ids.is_empty() {
        return Err(invalid(
            field,
            "live prediction input requires explicit live_predictor_labels; raw labels and target_supervision_labels are provenance only",
        ));
    }
    validate_live_label_list(field, label_ids)?;
    for label_id in label_ids {
        let row = bridge.accepted_labels.get(label_id).ok_or_else(|| {
            invalid(
                field,
                format!("{label_id} is not present in accepted live label registry"),
            )
        })?;
        validate_accepted_label(row)?;
    }
    Ok(())
}

fn validate_live_label_list(field: &str, label_ids: &[String]) -> Result<(), TrainerError> {
    for label_id in label_ids {
        if skill_validation::is_target_only_live_label(label_id) {
            return Err(invalid(
                field,
                format!(
                    "{label_id} is target-side supervision and cannot be a live prediction input"
                ),
            ));
        }
    }
    Ok(())
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TrainerError> {
    let file = File::open(path).map_err(map_io_error)?;
    serde_json::from_reader(file).map_err(map_json_error)
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, TrainerError> {
    let file = File::open(path).map_err(map_io_error)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(map_io_error)?;
        if line.trim().is_empty() {
            continue;
        }
        let row = serde_json::from_str(&line).map_err(|err| {
            invalid(
                "jsonl",
                format!("{}:{} failed JSON parse: {err}", path.display(), idx + 1),
            )
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn sha256_file(path: &Path) -> Result<String, TrainerError> {
    let mut file = File::open(path).map_err(map_io_error)?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(map_io_error)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    if values.len() > max_items {
        return Err(invalid(field, format!("too many ids: {}", values.len())));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_label_id(field, value)?;
        if !seen.insert(value) {
            return Err(invalid(field, format!("duplicate id {value}")));
        }
    }
    Ok(())
}

fn validate_label_id(field: &str, value: &str) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must be non-empty"));
    }
    if value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(invalid(field, "must be single-line text up to 512 bytes"));
    }
    Ok(())
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": "file:crates/context-graph-mejepa-train/src/label_bridge.rs",
        "remediation": "regenerate or repair the #413 label bridge; mistake-driven learning requires non-leaky slot-preserving labels"
    }))
}

fn map_io_error(err: std::io::Error) -> TrainerError {
    invalid("io", err.to_string())
}

fn map_json_error(err: serde_json::Error) -> TrainerError {
    invalid("json", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_bridge_fixture(root: &Path, live_allowed: bool) {
        let accepted = root.join("accepted_label_registry.jsonl");
        let usefulness = root.join("label_usefulness_metrics.jsonl");
        let chunks = root.join("chunk_constellation_labels.jsonl");
        let code_states = root.join("code_state_constellations.jsonl");
        let failure_sets = root.join("failure_evidence_sets.jsonl");
        fs::write(
            &accepted,
            format!(
                r#"{{"artifact_kind":"python_auto_label_discovery_accepted_label_registry_row","schema_version":1,"formula_version":"unit","label_id":"ast_surface:function","label_hash":"sha256:label","family":"ast_surface","status":"accepted_live_input","support":42,"fail_rate":0.75,"lift_over_weighted_category_baseline":0.31,"live_prediction_input_allowed":{},"learning_update_policy":{{"baseline_seeded_from":"TASK-PY-G-117","online_rescore":true,"promote_demote_with_new_outcomes":true,"target_labels_are_supervision_only":true}}}}"#,
                live_allowed
            ),
        )
        .unwrap();
        fs::write(
            &usefulness,
            r#"{"label_id":"ast_surface:function","family":"ast_surface","status":"accepted_live_input","support":42,"leaky_target":false,"lift_over_weighted_category_baseline":0.31}"#,
        )
        .unwrap();
        fs::write(&chunks, "{}\n").unwrap();
        fs::write(
            &code_states,
            r#"{"group_key":"python:before:unit","labels":["group_scope:code_state","dominant_surface:function","code_state_outcome:fail"],"live_predictor_labels":["ast_surface:function"],"target_supervision_labels":["code_state_outcome:fail","oracle:fail"],"slot_identity_preserved":true,"flat_vector_concat_used":false}"#,
        )
        .unwrap();
        fs::write(
            &failure_sets,
            r#"{"code_state_key":"python:before:unit","labels":["evidence:multi_point","oracle:fail"],"target_supervision_labels":["oracle:fail"],"localization_mode":"failure_localization:multi_point","slot_identity_preserved":true,"flat_vector_concat_used":false}"#,
        )
        .unwrap();
        let manifest = serde_json::json!({
            "artifact_kind": "python_auto_label_discovery_learning_bridge_manifest",
            "schema_version": 1,
            "formula_version": "unit",
            "purpose": "unit",
            "paths": {
                "accepted_label_registry": accepted,
                "label_usefulness_metrics": usefulness,
                "chunk_constellation_labels": chunks,
                "code_state_constellations": code_states,
                "failure_evidence_sets": failure_sets,
            },
            "consumers": {
                "ReplayBufferRow.cell_id": "label aware"
            },
            "dynamic_learning_policy": {
                "baseline_only": true,
                "new_data_uses_same_compiler": true,
                "usefulness_rescored_over_time": true,
                "target_outcomes_supervise_but_are_not_live_inputs": true,
                "slot_identity_preserved": true,
                "flat_vector_concat_used": false
            }
        });
        fs::write(root.join(BRIDGE_MANIFEST_FILE), manifest.to_string()).unwrap();
    }

    #[test]
    fn bridge_builds_prediction_label_context_with_skill_signature() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), true);

        let bridge = load_label_learning_bridge(temp.path()).unwrap();
        let context = build_prediction_label_context(
            &bridge,
            vec!["ast_surface:function".to_string()],
            Some("python:before:unit".to_string()),
            Vec::new(),
            vec!["skill:unit_sequence".to_string()],
        )
        .unwrap();

        assert_eq!(context.accepted_label_ids, vec!["ast_surface:function"]);
        assert_eq!(context.active_skill_ids, vec!["skill:unit_sequence"]);
        assert!(context.label_signature_hash.unwrap().starts_with("labels:"));
        assert!(context.skill_signature_hash.unwrap().starts_with("skills:"));
        assert!(context
            .accepted_registry_sha256
            .unwrap()
            .starts_with("sha256:"));
    }

    #[test]
    fn skill_signature_preserves_sequence_order() {
        let forward =
            skill_signature_hash(&["skill:first".to_string(), "skill:second".to_string()]).unwrap();
        let reverse =
            skill_signature_hash(&["skill:second".to_string(), "skill:first".to_string()]).unwrap();

        assert_ne!(forward, reverse);
    }

    #[test]
    fn bridge_rejects_target_side_failure_evidence_as_live_input() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), true);
        let bridge = load_label_learning_bridge(temp.path()).unwrap();

        let err = build_prediction_label_context(
            &bridge,
            vec!["ast_surface:function".to_string()],
            Some("python:before:unit".to_string()),
            vec!["python:before:unit".to_string()],
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn code_state_target_labels_remain_provenance_only_when_live_split_is_safe() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), true);
        let bridge = load_label_learning_bridge(temp.path()).unwrap();
        let row = bridge
            .code_state_constellations
            .get("python:before:unit")
            .expect("fixture code-state row");

        assert!(row
            .labels
            .iter()
            .any(|label| label == "code_state_outcome:fail"));
        assert!(row
            .target_supervision_labels
            .iter()
            .any(|label| label == "oracle:fail"));

        let context = build_prediction_label_context(
            &bridge,
            vec!["ast_surface:function".to_string()],
            Some("python:before:unit".to_string()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        assert_eq!(context.accepted_label_ids, vec!["ast_surface:function"]);
        assert!(!context
            .accepted_label_ids
            .iter()
            .any(|label| label.starts_with("oracle:") || label.starts_with("code_state_outcome:")));
    }

    #[test]
    fn bridge_rejects_target_label_in_code_state_live_predictor_split() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), true);
        fs::write(
            temp.path().join("code_state_constellations.jsonl"),
            r#"{"group_key":"python:before:unit","labels":["group_scope:code_state","code_state_outcome:fail"],"live_predictor_labels":["oracle:fail"],"target_supervision_labels":["oracle:fail"],"slot_identity_preserved":true,"flat_vector_concat_used":false}"#,
        )
        .unwrap();

        let err = load_label_learning_bridge(temp.path()).unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn bridge_rejects_missing_code_state_live_predictor_split() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), true);
        fs::write(
            temp.path().join("code_state_constellations.jsonl"),
            r#"{"group_key":"python:before:unit","labels":["group_scope:code_state","dominant_surface:function"],"slot_identity_preserved":true,"flat_vector_concat_used":false}"#,
        )
        .unwrap();
        let bridge = load_label_learning_bridge(temp.path()).unwrap();

        let err = build_prediction_label_context(
            &bridge,
            vec!["ast_surface:function".to_string()],
            Some("python:before:unit".to_string()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn bridge_rejects_target_leaky_accepted_label() {
        let temp = tempfile::tempdir().unwrap();
        write_bridge_fixture(temp.path(), false);

        let err = load_label_learning_bridge(temp.path()).unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }
}
