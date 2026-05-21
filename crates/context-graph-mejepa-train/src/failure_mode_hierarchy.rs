//! TASK-PY-G-110 Level-1/2/3 failure-mode hierarchy materialization.
//!
//! The Level-2 skill substrate is discovered from live-safe chunk labels. This
//! module derives the surrounding hierarchy without flattening embedder slots:
//! Level-1 rows aggregate co-firing live label groups, Level-3 rows aggregate
//! repeated ordered skill constellations, and named prediction rows expose the
//! observed consequence tendency as an auditable ontology surface.

use crate::chunk_skill_membership::{
    membership_key, ChunkSkillMembershipRow, OrderedStepSpan, SkillReverseIndexRow,
};
use crate::error::TrainerError;
use crate::skill_sequence_discovery::{
    Level2SkillRow, SkillOutcomeDistribution, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use crate::skill_validation;
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL1,
    CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS, CF_MEJEPA_NAMED_PREDICTIONS,
};
use rocksdb::{IteratorMode, WriteBatch, WriteOptions, DB};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const MAX_IDS: usize = 4096;
const SOURCE_FILE: &str = "file:crates/context-graph-mejepa-train/src/failure_mode_hierarchy.rs";
const REMEDIATION: &str =
    "failure-mode hierarchy rows must be deterministic, live-input-safe, slot-preserving, and RocksDB-readback verified";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Level1FailureModeGroupRow {
    pub schema_version: u32,
    pub group_id: String,
    pub group_name: String,
    pub child_skill_ids: Vec<String>,
    pub live_label_ids: Vec<String>,
    pub support: u64,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub live_input_allowed: bool,
    pub source_provenance_hash: String,
    pub created_at_unix_ms: i64,
}

impl Level1FailureModeGroupRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("group_id", &self.group_id)?;
        validate_id("group_name", &self.group_name)?;
        validate_id_list("child_skill_ids", &self.child_skill_ids)?;
        validate_live_id_list("live_label_ids", &self.live_label_ids)?;
        validate_positive("support", self.support)?;
        validate_finite_unit("confidence", self.confidence)?;
        validate_finite_unit("stability", self.stability)?;
        if !self.lift_over_cell_baseline.is_finite() {
            return Err(invalid("lift_over_cell_baseline", "must be finite"));
        }
        if !self.live_input_allowed {
            return Err(invalid(
                "live_input_allowed",
                "Level-1 groups must be live-input-safe",
            ));
        }
        validate_id("source_provenance_hash", &self.source_provenance_hash)?;
        validate_timestamp(self.created_at_unix_ms)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Level3ConstellationRow {
    pub schema_version: u32,
    pub constellation_id: String,
    pub constellation_name: String,
    pub member_skill_ids: Vec<String>,
    pub member_group_ids: Vec<String>,
    pub source_code_state_keys: Vec<String>,
    pub source_chunk_ids: Vec<String>,
    pub ordered_step_spans: Vec<OrderedStepSpan>,
    pub support: u64,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub live_input_allowed: bool,
    pub target_outcomes_used_for_calibration: bool,
    pub source_provenance_hash: String,
    pub created_at_unix_ms: i64,
}

impl Level3ConstellationRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("constellation_id", &self.constellation_id)?;
        if !self.constellation_id.starts_with("ability:") {
            return Err(invalid(
                "constellation_id",
                "Level-3 constellations must use ability:* ids for runtime resolver compatibility",
            ));
        }
        validate_id("constellation_name", &self.constellation_name)?;
        validate_id_list("member_skill_ids", &self.member_skill_ids)?;
        if self.member_skill_ids.len() < 2 {
            return Err(invalid(
                "member_skill_ids",
                "Level-3 constellations require at least two member skills",
            ));
        }
        validate_live_id_list("member_group_ids", &self.member_group_ids)?;
        validate_id_list("source_code_state_keys", &self.source_code_state_keys)?;
        validate_id_list("source_chunk_ids", &self.source_chunk_ids)?;
        if self.ordered_step_spans.len() > MAX_IDS {
            return Err(invalid(
                "ordered_step_spans",
                format!("too many spans: {}", self.ordered_step_spans.len()),
            ));
        }
        for span in &self.ordered_step_spans {
            validate_id("ordered_step_span.chunk_id", &span.chunk_id)?;
            validate_project_relative_path("ordered_step_span.file_path", &span.file_path)?;
            validate_id("ordered_step_span.code_state_key", &span.code_state_key)?;
        }
        validate_positive("support", self.support)?;
        validate_finite_unit("confidence", self.confidence)?;
        validate_finite_unit("stability", self.stability)?;
        if !self.lift_over_cell_baseline.is_finite() {
            return Err(invalid("lift_over_cell_baseline", "must be finite"));
        }
        if !self.live_input_allowed {
            return Err(invalid(
                "live_input_allowed",
                "Level-3 constellations must be live-input-safe",
            ));
        }
        if !self.target_outcomes_used_for_calibration {
            return Err(invalid(
                "target_outcomes_used_for_calibration",
                "Level-3 rows must record the post-reality calibration boundary",
            ));
        }
        validate_id("source_provenance_hash", &self.source_provenance_hash)?;
        validate_timestamp(self.created_at_unix_ms)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum NamedPredictionVerdict {
    PassLikely,
    FailLikely,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NamedPredictionRow {
    pub schema_version: u32,
    pub named_prediction_id: String,
    pub prediction_name: String,
    pub source_hierarchy_id: String,
    pub hierarchy_level: u8,
    pub predicted_verdict: NamedPredictionVerdict,
    pub confidence: f64,
    pub support: u64,
    pub consequence_label_ids: Vec<String>,
    pub evidence_group_ids: Vec<String>,
    pub evidence_skill_ids: Vec<String>,
    pub evidence_constellation_ids: Vec<String>,
    pub live_input_allowed: bool,
    pub target_labels_used_as_live_inputs: bool,
    pub target_outcomes_used_for_calibration: bool,
    pub created_at_unix_ms: i64,
}

impl NamedPredictionRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("named_prediction_id", &self.named_prediction_id)?;
        validate_id("prediction_name", &self.prediction_name)?;
        validate_id("source_hierarchy_id", &self.source_hierarchy_id)?;
        if self.hierarchy_level < 2 || self.hierarchy_level > 3 {
            return Err(invalid(
                "hierarchy_level",
                "named predictions are emitted for Level-2 skills or Level-3 abilities",
            ));
        }
        validate_finite_unit("confidence", self.confidence)?;
        validate_positive("support", self.support)?;
        validate_id_list("consequence_label_ids", &self.consequence_label_ids)?;
        validate_live_id_list("evidence_group_ids", &self.evidence_group_ids)?;
        validate_id_list("evidence_skill_ids", &self.evidence_skill_ids)?;
        validate_id_list(
            "evidence_constellation_ids",
            &self.evidence_constellation_ids,
        )?;
        if !self.live_input_allowed || self.target_labels_used_as_live_inputs {
            return Err(invalid(
                "runtime_policy",
                "named predictions may use target outcomes for calibration, never as live inputs",
            ));
        }
        if !self.target_outcomes_used_for_calibration {
            return Err(invalid(
                "target_outcomes_used_for_calibration",
                "named predictions must preserve their calibration source",
            ));
        }
        validate_timestamp(self.created_at_unix_ms)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FailureModeHierarchyMaterialization {
    pub level1_groups: Vec<Level1FailureModeGroupRow>,
    pub level3_constellations: Vec<Level3ConstellationRow>,
    pub level3_memberships: Vec<ChunkSkillMembershipRow>,
    pub named_predictions: Vec<NamedPredictionRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CodeStateOutcomeRow {
    pub code_state_key: String,
    pub task_instance_id: String,
    pub mutation_category: String,
    pub docker_label: String,
    pub oracle_all_passed: bool,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
}

impl CodeStateOutcomeRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("code_state_key", &self.code_state_key)?;
        validate_id("task_instance_id", &self.task_instance_id)?;
        validate_id("mutation_category", &self.mutation_category)?;
        validate_id("docker_label", &self.docker_label)?;
        if !self.slot_identity_preserved || self.flat_vector_concat_used {
            return Err(invalid(
                "code_state_outcome",
                "code-state evaluation rows must preserve slot identity and reject flat-vector semantics",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PostFixPredictionRow {
    pub task_instance_id: String,
    pub mutation_category: String,
    pub actual_oracle_pass: bool,
    pub predicted_oracle_pass: f64,
    pub predicted_pass: bool,
    pub patch_path: String,
    pub patch_sha256: String,
}

impl PostFixPredictionRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("task_instance_id", &self.task_instance_id)?;
        validate_id("mutation_category", &self.mutation_category)?;
        if !self.predicted_oracle_pass.is_finite()
            || self.predicted_oracle_pass < 0.0
            || self.predicted_oracle_pass > 1.0
        {
            return Err(invalid(
                "predicted_oracle_pass",
                "must be finite probability in [0,1]",
            ));
        }
        validate_id("patch_path", &self.patch_path)?;
        validate_id("patch_sha256", &self.patch_sha256)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum HierarchyEvalPartition {
    Train,
    Calibration,
    Holdout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MissingNamedModeExample {
    pub code_state_key: String,
    pub mutation_category: String,
    pub docker_label: String,
    pub active_skill_ids: Vec<String>,
    pub active_higher_ability_ids: Vec<String>,
    pub named_prediction_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClosestExemplarExample {
    pub code_state_key: String,
    pub mutation_category: String,
    pub docker_label: String,
    pub named_prediction_ids: Vec<String>,
    pub exemplar_code_state_key: String,
    pub exemplar_docker_label: String,
    pub exemplar_named_prediction_ids: Vec<String>,
    pub shared_named_mode_count: usize,
    pub jaccard_similarity: f64,
    pub target_oracle_all_passed: bool,
    pub exemplar_oracle_all_passed: bool,
    pub oracle_outcome_agrees: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HoldoutNamedModeEvaluation {
    pub schema_version: u32,
    pub code_state_rows: usize,
    pub train_rows: usize,
    pub calibration_rows: usize,
    pub holdout_rows: usize,
    pub holdout_fail_rows: usize,
    pub holdout_fail_rows_with_any_named_mode: usize,
    pub holdout_fail_rows_with_fail_likely_named_mode: usize,
    pub named_mode_coverage_on_fail: f64,
    pub closest_exemplar_count: usize,
    pub closest_exemplar_coverage_on_fail: f64,
    pub min_closest_exemplar_coverage_on_fail: f64,
    pub closest_exemplar_oracle_agreement_count: usize,
    pub closest_exemplar_oracle_agreement: f64,
    pub min_closest_exemplar_oracle_agreement: f64,
    pub coverage_pass: bool,
    pub closest_exemplar_agreement_pass: bool,
    pub passes: bool,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
    pub target_labels_used_as_live_inputs: bool,
    pub missing_named_mode_examples: Vec<MissingNamedModeExample>,
    pub closest_exemplar_examples: Vec<ClosestExemplarExample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PostFixPredictedFailNamedModeEvaluation {
    pub schema_version: u32,
    pub holdout_prediction_rows: usize,
    pub post_fix_predicted_fail_rows: usize,
    pub post_fix_predicted_fail_true_fail_rows: usize,
    pub post_fix_predicted_fail_false_positive_rows: usize,
    pub post_fix_predicted_fail_rows_with_any_named_mode: usize,
    pub post_fix_predicted_fail_rows_with_fail_likely_named_mode: usize,
    pub post_fix_predicted_fail_named_mode_coverage: f64,
    pub post_fix_predicted_true_fail_rows_with_fail_likely_named_mode: usize,
    pub post_fix_predicted_true_fail_named_mode_coverage: f64,
    pub post_fix_predicted_false_positive_rows_with_fail_likely_named_mode: usize,
    pub post_fix_predicted_false_positive_rows_without_any_named_mode: usize,
    pub closest_exemplar_count: usize,
    pub closest_exemplar_coverage_on_predicted_fail: f64,
    pub min_closest_exemplar_coverage_on_predicted_fail: f64,
    pub closest_exemplar_oracle_agreement_count: usize,
    pub closest_exemplar_oracle_agreement: f64,
    pub min_closest_exemplar_oracle_agreement: f64,
    pub coverage_pass: bool,
    pub true_fail_coverage_pass: bool,
    pub closest_exemplar_agreement_pass: bool,
    pub passes: bool,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
    pub target_labels_used_as_live_inputs: bool,
    pub missing_named_mode_examples: Vec<MissingNamedModeExample>,
    pub closest_exemplar_examples: Vec<ClosestExemplarExample>,
    pub unmapped_prediction_examples: Vec<String>,
}

pub fn materialize_failure_mode_hierarchy(
    level2_skills: &[Level2SkillRow],
    memberships: &[ChunkSkillMembershipRow],
    reverse_indexes: &[SkillReverseIndexRow],
    created_at_unix_ms: i64,
) -> Result<FailureModeHierarchyMaterialization, TrainerError> {
    if level2_skills.is_empty() {
        return Err(invalid("level2_skills", "must not be empty"));
    }
    validate_timestamp(created_at_unix_ms)?;
    let mut skill_by_id = BTreeMap::new();
    for skill in level2_skills {
        skill.validate()?;
        skill_by_id.insert(skill.skill_id.clone(), skill.clone());
    }
    for row in memberships {
        row.validate()?;
        if !skill_by_id.contains_key(&row.skill_id) {
            return Err(invalid(
                "memberships",
                format!("membership references missing skill {}", row.skill_id),
            ));
        }
    }
    for row in reverse_indexes {
        row.validate()?;
        if !skill_by_id.contains_key(&row.skill_id) {
            return Err(invalid(
                "reverse_indexes",
                format!("reverse index references missing skill {}", row.skill_id),
            ));
        }
    }

    let level1_groups = materialize_level1(level2_skills, created_at_unix_ms)?;
    let (level3_constellations, level3_memberships) = materialize_level3(
        &skill_by_id,
        memberships,
        reverse_indexes,
        created_at_unix_ms,
    )?;
    if level3_constellations.is_empty() {
        return Err(invalid(
            "level3_constellations",
            "no repeated multi-skill constellations were found",
        ));
    }

    let mut named_predictions = Vec::new();
    for skill in level2_skills {
        named_predictions.push(named_prediction_for_skill(skill, created_at_unix_ms)?);
    }
    for constellation in &level3_constellations {
        named_predictions.push(named_prediction_for_constellation(
            constellation,
            created_at_unix_ms,
        )?);
    }
    for row in &level1_groups {
        row.validate()?;
    }
    for row in &level3_constellations {
        row.validate()?;
    }
    for row in &level3_memberships {
        row.validate()?;
    }
    for row in &named_predictions {
        row.validate()?;
    }
    Ok(FailureModeHierarchyMaterialization {
        level1_groups,
        level3_constellations,
        level3_memberships,
        named_predictions,
    })
}

pub fn evaluate_post_fix_predicted_fail_named_mode_coverage(
    code_states: &[CodeStateOutcomeRow],
    post_fix_predictions: &[PostFixPredictionRow],
    reverse_indexes: &[SkillReverseIndexRow],
    memberships: &[ChunkSkillMembershipRow],
    named_predictions: &[NamedPredictionRow],
    min_closest_exemplar_oracle_agreement: f64,
) -> Result<PostFixPredictedFailNamedModeEvaluation, TrainerError> {
    if post_fix_predictions.is_empty() {
        return Err(invalid(
            "post_fix_predictions",
            "must contain active holdout prediction rows",
        ));
    }
    validate_finite_unit(
        "min_closest_exemplar_oracle_agreement",
        min_closest_exemplar_oracle_agreement,
    )?;
    let states = build_eval_states(code_states, reverse_indexes, memberships, named_predictions)?;
    let mut state_keys_by_task_category = BTreeMap::<(String, String), Vec<String>>::new();
    for state in states.values() {
        state_keys_by_task_category
            .entry((
                state.task_instance_id.clone(),
                state.mutation_category.clone(),
            ))
            .or_default()
            .push(state.code_state_key.clone());
    }
    for keys in state_keys_by_task_category.values_mut() {
        keys.sort();
    }

    let exemplar_pool = states
        .values()
        .filter(|state| {
            state.partition != HierarchyEvalPartition::Holdout
                && !state.named_mode_sources.is_empty()
        })
        .collect::<Vec<_>>();

    let mut post_fix_predicted_fail_rows = 0_usize;
    let mut post_fix_predicted_fail_true_fail_rows = 0_usize;
    let mut post_fix_predicted_fail_false_positive_rows = 0_usize;
    let mut post_fix_predicted_fail_rows_with_any_named_mode = 0_usize;
    let mut post_fix_predicted_fail_rows_with_fail_likely_named_mode = 0_usize;
    let mut post_fix_predicted_true_fail_rows_with_fail_likely_named_mode = 0_usize;
    let mut post_fix_predicted_false_positive_rows_with_fail_likely_named_mode = 0_usize;
    let mut post_fix_predicted_false_positive_rows_without_any_named_mode = 0_usize;
    let mut closest_exemplar_count = 0_usize;
    let mut closest_exemplar_oracle_agreement_count = 0_usize;
    let mut missing_named_mode_examples = Vec::new();
    let mut closest_exemplar_examples = Vec::new();
    let mut unmapped_prediction_examples = Vec::new();

    for prediction in post_fix_predictions {
        prediction.validate()?;
        if prediction.predicted_pass {
            continue;
        }
        post_fix_predicted_fail_rows += 1;
        let predicted_fail_is_false_positive = prediction.actual_oracle_pass;
        if predicted_fail_is_false_positive {
            post_fix_predicted_fail_false_positive_rows += 1;
        } else {
            post_fix_predicted_fail_true_fail_rows += 1;
        }
        let state_key = match state_key_for_prediction(prediction, &state_keys_by_task_category) {
            Ok(value) => value,
            Err(err) => {
                if unmapped_prediction_examples.len() < 12 {
                    unmapped_prediction_examples.push(err.to_string());
                }
                continue;
            }
        };
        let state = states
            .get(&state_key)
            .ok_or_else(|| invalid("post_fix_predictions", "mapped state missing"))?;
        if state.oracle_all_passed != prediction.actual_oracle_pass {
            return Err(invalid(
                "post_fix_predictions",
                format!(
                    "oracle label mismatch for {}:{}",
                    prediction.task_instance_id, prediction.mutation_category
                ),
            ));
        }
        if !state.named_prediction_ids.is_empty() {
            post_fix_predicted_fail_rows_with_any_named_mode += 1;
        } else if predicted_fail_is_false_positive {
            post_fix_predicted_false_positive_rows_without_any_named_mode += 1;
        }
        if !state.fail_likely_named_prediction_ids.is_empty() {
            post_fix_predicted_fail_rows_with_fail_likely_named_mode += 1;
            if predicted_fail_is_false_positive {
                post_fix_predicted_false_positive_rows_with_fail_likely_named_mode += 1;
            } else {
                post_fix_predicted_true_fail_rows_with_fail_likely_named_mode += 1;
            }
        } else if missing_named_mode_examples.len() < 12 {
            missing_named_mode_examples.push(state.missing_example());
        }
        if state.named_mode_sources.is_empty() {
            continue;
        }
        if let Some(best) = closest_exemplar(state, &exemplar_pool) {
            closest_exemplar_count += 1;
            if best.oracle_outcome_agrees {
                closest_exemplar_oracle_agreement_count += 1;
            }
            if closest_exemplar_examples.len() < 12 {
                closest_exemplar_examples.push(best);
            }
        }
    }

    let post_fix_predicted_fail_named_mode_coverage = ratio(
        post_fix_predicted_fail_rows_with_fail_likely_named_mode,
        post_fix_predicted_fail_rows,
    );
    let post_fix_predicted_true_fail_named_mode_coverage = ratio(
        post_fix_predicted_true_fail_rows_with_fail_likely_named_mode,
        post_fix_predicted_fail_true_fail_rows,
    );
    let closest_exemplar_coverage_on_predicted_fail =
        ratio(closest_exemplar_count, post_fix_predicted_fail_rows);
    let min_closest_exemplar_coverage_on_predicted_fail = 0.95;
    let closest_exemplar_oracle_agreement = ratio(
        closest_exemplar_oracle_agreement_count,
        closest_exemplar_count,
    );
    let coverage_pass = post_fix_predicted_fail_rows > 0
        && post_fix_predicted_fail_rows_with_fail_likely_named_mode == post_fix_predicted_fail_rows;
    let true_fail_coverage_pass = post_fix_predicted_fail_true_fail_rows > 0
        && post_fix_predicted_true_fail_rows_with_fail_likely_named_mode
            == post_fix_predicted_fail_true_fail_rows;
    let closest_exemplar_agreement_pass = post_fix_predicted_fail_rows > 0
        && unmapped_prediction_examples.is_empty()
        && closest_exemplar_coverage_on_predicted_fail
            >= min_closest_exemplar_coverage_on_predicted_fail
        && closest_exemplar_oracle_agreement >= min_closest_exemplar_oracle_agreement;
    let passes = coverage_pass && closest_exemplar_agreement_pass;

    Ok(PostFixPredictedFailNamedModeEvaluation {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        holdout_prediction_rows: post_fix_predictions.len(),
        post_fix_predicted_fail_rows,
        post_fix_predicted_fail_true_fail_rows,
        post_fix_predicted_fail_false_positive_rows,
        post_fix_predicted_fail_rows_with_any_named_mode,
        post_fix_predicted_fail_rows_with_fail_likely_named_mode,
        post_fix_predicted_fail_named_mode_coverage,
        post_fix_predicted_true_fail_rows_with_fail_likely_named_mode,
        post_fix_predicted_true_fail_named_mode_coverage,
        post_fix_predicted_false_positive_rows_with_fail_likely_named_mode,
        post_fix_predicted_false_positive_rows_without_any_named_mode,
        closest_exemplar_count,
        closest_exemplar_coverage_on_predicted_fail,
        min_closest_exemplar_coverage_on_predicted_fail,
        closest_exemplar_oracle_agreement_count,
        closest_exemplar_oracle_agreement,
        min_closest_exemplar_oracle_agreement,
        coverage_pass,
        true_fail_coverage_pass,
        closest_exemplar_agreement_pass,
        passes,
        slot_identity_preserved: true,
        flat_vector_concat_used: false,
        target_labels_used_as_live_inputs: false,
        missing_named_mode_examples,
        closest_exemplar_examples,
        unmapped_prediction_examples,
    })
}

pub fn evaluate_holdout_named_mode_coverage(
    code_states: &[CodeStateOutcomeRow],
    reverse_indexes: &[SkillReverseIndexRow],
    memberships: &[ChunkSkillMembershipRow],
    named_predictions: &[NamedPredictionRow],
    min_closest_exemplar_oracle_agreement: f64,
) -> Result<HoldoutNamedModeEvaluation, TrainerError> {
    if code_states.is_empty() {
        return Err(invalid("code_states", "must not be empty"));
    }
    validate_finite_unit(
        "min_closest_exemplar_oracle_agreement",
        min_closest_exemplar_oracle_agreement,
    )?;
    let states = build_eval_states(code_states, reverse_indexes, memberships, named_predictions)?;

    let mut holdout_fail_rows = 0_usize;
    let mut holdout_fail_rows_with_any_named_mode = 0_usize;
    let mut holdout_fail_rows_with_fail_likely_named_mode = 0_usize;
    let mut missing_named_mode_examples = Vec::new();
    let mut closest_exemplar_count = 0_usize;
    let mut closest_exemplar_oracle_agreement_count = 0_usize;
    let mut closest_exemplar_examples = Vec::new();

    let exemplar_pool = states
        .values()
        .filter(|state| {
            state.partition != HierarchyEvalPartition::Holdout
                && !state.named_mode_sources.is_empty()
        })
        .collect::<Vec<_>>();

    for state in states.values().filter(|state| {
        state.partition == HierarchyEvalPartition::Holdout && !state.oracle_all_passed
    }) {
        holdout_fail_rows += 1;
        if !state.named_prediction_ids.is_empty() {
            holdout_fail_rows_with_any_named_mode += 1;
        }
        if !state.fail_likely_named_prediction_ids.is_empty() {
            holdout_fail_rows_with_fail_likely_named_mode += 1;
        } else if missing_named_mode_examples.len() < 12 {
            missing_named_mode_examples.push(state.missing_example());
        }
        if state.named_mode_sources.is_empty() {
            continue;
        }
        if let Some(best) = closest_exemplar(state, &exemplar_pool) {
            closest_exemplar_count += 1;
            if best.oracle_outcome_agrees {
                closest_exemplar_oracle_agreement_count += 1;
            }
            if closest_exemplar_examples.len() < 12 {
                closest_exemplar_examples.push(best);
            }
        }
    }

    let named_mode_coverage_on_fail = ratio(
        holdout_fail_rows_with_fail_likely_named_mode,
        holdout_fail_rows,
    );
    let closest_exemplar_coverage_on_fail = ratio(closest_exemplar_count, holdout_fail_rows);
    let min_closest_exemplar_coverage_on_fail = 0.95;
    let closest_exemplar_oracle_agreement = ratio(
        closest_exemplar_oracle_agreement_count,
        closest_exemplar_count,
    );
    let coverage_pass =
        holdout_fail_rows > 0 && holdout_fail_rows_with_fail_likely_named_mode == holdout_fail_rows;
    let closest_exemplar_agreement_pass = holdout_fail_rows > 0
        && closest_exemplar_coverage_on_fail >= min_closest_exemplar_coverage_on_fail
        && closest_exemplar_oracle_agreement >= min_closest_exemplar_oracle_agreement;
    let passes = coverage_pass && closest_exemplar_agreement_pass;

    Ok(HoldoutNamedModeEvaluation {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        code_state_rows: states.len(),
        train_rows: states
            .values()
            .filter(|state| state.partition == HierarchyEvalPartition::Train)
            .count(),
        calibration_rows: states
            .values()
            .filter(|state| state.partition == HierarchyEvalPartition::Calibration)
            .count(),
        holdout_rows: states
            .values()
            .filter(|state| state.partition == HierarchyEvalPartition::Holdout)
            .count(),
        holdout_fail_rows,
        holdout_fail_rows_with_any_named_mode,
        holdout_fail_rows_with_fail_likely_named_mode,
        named_mode_coverage_on_fail,
        closest_exemplar_count,
        closest_exemplar_coverage_on_fail,
        min_closest_exemplar_coverage_on_fail,
        closest_exemplar_oracle_agreement_count,
        closest_exemplar_oracle_agreement,
        min_closest_exemplar_oracle_agreement,
        coverage_pass,
        closest_exemplar_agreement_pass,
        passes,
        slot_identity_preserved: true,
        flat_vector_concat_used: false,
        target_labels_used_as_live_inputs: false,
        missing_named_mode_examples,
        closest_exemplar_examples,
    })
}

fn build_eval_states(
    code_states: &[CodeStateOutcomeRow],
    reverse_indexes: &[SkillReverseIndexRow],
    memberships: &[ChunkSkillMembershipRow],
    named_predictions: &[NamedPredictionRow],
) -> Result<BTreeMap<String, EvalState>, TrainerError> {
    if code_states.is_empty() {
        return Err(invalid("code_states", "must not be empty"));
    }
    let mut states = BTreeMap::<String, EvalState>::new();
    for row in code_states {
        row.validate()?;
        let partition = stable_hierarchy_eval_partition(&row.code_state_key)?;
        if states
            .insert(row.code_state_key.clone(), EvalState::new(row, partition))
            .is_some()
        {
            return Err(invalid("code_states", "duplicate code_state_key"));
        }
    }

    let mut predictions_by_source = BTreeMap::<String, Vec<&NamedPredictionRow>>::new();
    for row in named_predictions {
        row.validate()?;
        if row.target_labels_used_as_live_inputs {
            return Err(invalid(
                "named_predictions",
                "target labels cannot be live named-mode inputs",
            ));
        }
        predictions_by_source
            .entry(row.source_hierarchy_id.clone())
            .or_default()
            .push(row);
    }
    if predictions_by_source.is_empty() {
        return Err(invalid(
            "named_predictions",
            "must contain at least one named prediction",
        ));
    }

    for row in reverse_indexes {
        row.validate()?;
        let state = states.get_mut(&row.code_state_key).ok_or_else(|| {
            invalid(
                "reverse_indexes",
                format!(
                    "reverse index references unknown code_state {}",
                    row.code_state_key
                ),
            )
        })?;
        state.active_skill_ids.insert(row.skill_id.clone());
    }
    for row in memberships {
        row.validate()?;
        if row.hierarchy_level < 3 {
            continue;
        }
        let state = states.get_mut(&row.code_state_key).ok_or_else(|| {
            invalid(
                "memberships",
                format!(
                    "membership references unknown code_state {}",
                    row.code_state_key
                ),
            )
        })?;
        state.active_higher_ability_ids.insert(row.skill_id.clone());
    }

    for state in states.values_mut() {
        state.attach_named_predictions(&predictions_by_source);
    }

    Ok(states)
}

fn state_key_for_prediction(
    prediction: &PostFixPredictionRow,
    states_by_task_category: &BTreeMap<(String, String), Vec<String>>,
) -> Result<String, TrainerError> {
    let key = (
        prediction.task_instance_id.clone(),
        prediction.mutation_category.clone(),
    );
    let Some(candidates) = states_by_task_category.get(&key) else {
        return Err(invalid(
            "post_fix_predictions",
            format!(
                "no code-state row for {}:{}",
                prediction.task_instance_id, prediction.mutation_category
            ),
        ));
    };
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    if prediction.mutation_category == "known_good" {
        for suffix in ["|gold", "|base", "|test_patch"] {
            if let Some(value) = candidates
                .iter()
                .find(|candidate| candidate.ends_with(suffix))
            {
                return Ok(value.clone());
            }
        }
    }
    Err(invalid(
        "post_fix_predictions",
        format!(
            "ambiguous code-state rows for {}:{} ({})",
            prediction.task_instance_id,
            prediction.mutation_category,
            candidates.len()
        ),
    ))
}

pub fn stable_hierarchy_eval_partition(
    row_key: &str,
) -> Result<HierarchyEvalPartition, TrainerError> {
    validate_id("row_key", row_key)?;
    let digest = Sha256::digest(row_key.as_bytes());
    let bucket = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 10;
    Ok(match bucket {
        0 => HierarchyEvalPartition::Holdout,
        1 => HierarchyEvalPartition::Calibration,
        _ => HierarchyEvalPartition::Train,
    })
}

#[derive(Debug, Clone)]
struct EvalState {
    code_state_key: String,
    task_instance_id: String,
    mutation_category: String,
    docker_label: String,
    oracle_all_passed: bool,
    partition: HierarchyEvalPartition,
    active_skill_ids: BTreeSet<String>,
    active_higher_ability_ids: BTreeSet<String>,
    named_mode_sources: BTreeSet<String>,
    failure_coverage_named_mode_sources: BTreeSet<String>,
    fail_likely_named_mode_sources: BTreeSet<String>,
    named_prediction_ids: BTreeSet<String>,
    fail_likely_named_prediction_ids: BTreeSet<String>,
}

impl EvalState {
    fn new(row: &CodeStateOutcomeRow, partition: HierarchyEvalPartition) -> Self {
        Self {
            code_state_key: row.code_state_key.clone(),
            task_instance_id: row.task_instance_id.clone(),
            mutation_category: row.mutation_category.clone(),
            docker_label: row.docker_label.clone(),
            oracle_all_passed: row.oracle_all_passed,
            partition,
            active_skill_ids: BTreeSet::new(),
            active_higher_ability_ids: BTreeSet::new(),
            named_mode_sources: BTreeSet::new(),
            failure_coverage_named_mode_sources: BTreeSet::new(),
            fail_likely_named_mode_sources: BTreeSet::new(),
            named_prediction_ids: BTreeSet::new(),
            fail_likely_named_prediction_ids: BTreeSet::new(),
        }
    }

    fn attach_named_predictions(
        &mut self,
        predictions_by_source: &BTreeMap<String, Vec<&NamedPredictionRow>>,
    ) {
        for source_id in self
            .active_skill_ids
            .iter()
            .chain(self.active_higher_ability_ids.iter())
        {
            let Some(predictions) = predictions_by_source.get(source_id) else {
                continue;
            };
            self.named_mode_sources.insert(source_id.clone());
            for prediction in predictions {
                self.named_prediction_ids
                    .insert(prediction.named_prediction_id.clone());
                if prediction.predicted_verdict == NamedPredictionVerdict::FailLikely {
                    self.fail_likely_named_mode_sources
                        .insert(source_id.clone());
                    self.fail_likely_named_prediction_ids
                        .insert(prediction.named_prediction_id.clone());
                    if source_id.starts_with("skill:failure_coverage_") {
                        self.failure_coverage_named_mode_sources
                            .insert(source_id.clone());
                    }
                }
            }
        }
    }

    fn missing_example(&self) -> MissingNamedModeExample {
        MissingNamedModeExample {
            code_state_key: self.code_state_key.clone(),
            mutation_category: self.mutation_category.clone(),
            docker_label: self.docker_label.clone(),
            active_skill_ids: self.active_skill_ids.iter().take(16).cloned().collect(),
            active_higher_ability_ids: self
                .active_higher_ability_ids
                .iter()
                .take(16)
                .cloned()
                .collect(),
            named_prediction_ids: self.named_prediction_ids.iter().take(16).cloned().collect(),
        }
    }

    fn exemplar_sources(&self) -> &BTreeSet<String> {
        if !self.failure_coverage_named_mode_sources.is_empty() {
            &self.failure_coverage_named_mode_sources
        } else if !self.fail_likely_named_mode_sources.is_empty() {
            &self.fail_likely_named_mode_sources
        } else {
            &self.named_mode_sources
        }
    }
}

fn closest_exemplar(
    target: &EvalState,
    exemplar_pool: &[&EvalState],
) -> Option<ClosestExemplarExample> {
    let target_sources = target.exemplar_sources();
    let mut best: Option<(&EvalState, usize, usize, f64)> = None;
    for candidate in exemplar_pool {
        if candidate.code_state_key == target.code_state_key {
            continue;
        }
        let (shared, union, similarity) = jaccard(target_sources, candidate.exemplar_sources());
        if shared == 0 || union == 0 {
            continue;
        }
        let should_replace = match best {
            None => true,
            Some((current, current_shared, _current_union, current_similarity)) => {
                let similarity_order = similarity.total_cmp(&current_similarity);
                similarity_order == Ordering::Greater
                    || (similarity_order == Ordering::Equal && shared > current_shared)
                    || (similarity_order == Ordering::Equal
                        && shared == current_shared
                        && candidate.code_state_key < current.code_state_key)
            }
        };
        if should_replace {
            best = Some((*candidate, shared, union, similarity));
        }
    }
    best.map(
        |(exemplar, shared, _union, similarity)| ClosestExemplarExample {
            code_state_key: target.code_state_key.clone(),
            mutation_category: target.mutation_category.clone(),
            docker_label: target.docker_label.clone(),
            named_prediction_ids: target
                .named_prediction_ids
                .iter()
                .take(16)
                .cloned()
                .collect(),
            exemplar_code_state_key: exemplar.code_state_key.clone(),
            exemplar_docker_label: exemplar.docker_label.clone(),
            exemplar_named_prediction_ids: exemplar
                .named_prediction_ids
                .iter()
                .take(16)
                .cloned()
                .collect(),
            shared_named_mode_count: shared,
            jaccard_similarity: similarity,
            target_oracle_all_passed: target.oracle_all_passed,
            exemplar_oracle_all_passed: exemplar.oracle_all_passed,
            oracle_outcome_agrees: target.oracle_all_passed == exemplar.oracle_all_passed,
        },
    )
}

fn jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> (usize, usize, f64) {
    let shared = left.intersection(right).count();
    let union = left.union(right).count();
    let similarity = if union == 0 {
        0.0
    } else {
        shared as f64 / union as f64
    };
    (shared, union, similarity)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

pub fn write_failure_mode_hierarchy_sync_readback(
    db: &DB,
    materialization: &FailureModeHierarchyMaterialization,
) -> Result<(), TrainerError> {
    for row in &materialization.level1_groups {
        row.validate()?;
    }
    for row in &materialization.level3_constellations {
        row.validate()?;
    }
    for row in &materialization.level3_memberships {
        row.validate()?;
    }
    for row in &materialization.named_predictions {
        row.validate()?;
    }
    let level1_cf = cf(db, CF_MEJEPA_FAILURE_MODE_LEVEL1)?;
    let level3_cf = cf(db, CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS)?;
    let membership_cf = cf(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?;
    let named_cf = cf(db, CF_MEJEPA_NAMED_PREDICTIONS)?;
    let mut batch = WriteBatch::default();
    for row in &materialization.level1_groups {
        batch.put_cf(level1_cf, row.group_id.as_bytes(), serialize(row)?);
    }
    for row in &materialization.level3_constellations {
        batch.put_cf(level3_cf, row.constellation_id.as_bytes(), serialize(row)?);
    }
    for row in &materialization.level3_memberships {
        batch.put_cf(
            membership_cf,
            row.membership_key.as_bytes(),
            serialize(row)?,
        );
    }
    for row in &materialization.named_predictions {
        batch.put_cf(
            named_cf,
            row.named_prediction_id.as_bytes(),
            serialize(row)?,
        );
    }
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.write_opt(batch, &write_opts)
        .map_err(map_rocksdb_error)?;
    for cf_handle in [level1_cf, level3_cf, membership_cf, named_cf] {
        db.flush_cf(cf_handle).map_err(map_rocksdb_error)?;
    }
    for row in &materialization.level1_groups {
        let readback = read_row::<Level1FailureModeGroupRow>(
            db,
            CF_MEJEPA_FAILURE_MODE_LEVEL1,
            &row.group_id,
        )?
        .ok_or_else(|| invalid("readback", "Level-1 group row missing"))?;
        if readback != *row {
            return Err(invalid("readback", "Level-1 group row changed"));
        }
    }
    for row in &materialization.level3_constellations {
        let readback = read_row::<Level3ConstellationRow>(
            db,
            CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
            &row.constellation_id,
        )?
        .ok_or_else(|| invalid("readback", "Level-3 constellation row missing"))?;
        if readback != *row {
            return Err(invalid("readback", "Level-3 constellation row changed"));
        }
    }
    for row in &materialization.level3_memberships {
        let readback = read_row::<ChunkSkillMembershipRow>(
            db,
            CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP,
            &row.membership_key,
        )?
        .ok_or_else(|| invalid("readback", "Level-3 membership row missing"))?;
        if readback != *row {
            return Err(invalid("readback", "Level-3 membership row changed"));
        }
    }
    for row in &materialization.named_predictions {
        let readback = read_row::<NamedPredictionRow>(
            db,
            CF_MEJEPA_NAMED_PREDICTIONS,
            &row.named_prediction_id,
        )?
        .ok_or_else(|| invalid("readback", "named prediction row missing"))?;
        if readback != *row {
            return Err(invalid("readback", "named prediction row changed"));
        }
    }
    Ok(())
}

pub fn read_all_level1_failure_mode_group_rows(
    db: &DB,
) -> Result<Vec<Level1FailureModeGroupRow>, TrainerError> {
    read_all_rows(
        db,
        CF_MEJEPA_FAILURE_MODE_LEVEL1,
        |row: &Level1FailureModeGroupRow| row.group_id.as_str(),
    )
}

pub fn read_all_level3_constellation_rows(
    db: &DB,
) -> Result<Vec<Level3ConstellationRow>, TrainerError> {
    read_all_rows(
        db,
        CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS,
        |row: &Level3ConstellationRow| row.constellation_id.as_str(),
    )
}

pub fn read_all_named_prediction_rows(db: &DB) -> Result<Vec<NamedPredictionRow>, TrainerError> {
    read_all_rows(
        db,
        CF_MEJEPA_NAMED_PREDICTIONS,
        |row: &NamedPredictionRow| row.named_prediction_id.as_str(),
    )
}

fn materialize_level1(
    skills: &[Level2SkillRow],
    created_at_unix_ms: i64,
) -> Result<Vec<Level1FailureModeGroupRow>, TrainerError> {
    let mut accumulators = BTreeMap::<String, Level1Accumulator>::new();
    for skill in skills {
        for group_id in &skill.parent_group_ids {
            accumulators
                .entry(group_id.clone())
                .or_insert_with(|| Level1Accumulator::new(group_id))
                .push_skill(skill);
        }
    }
    let mut rows = Vec::new();
    for acc in accumulators.into_values() {
        rows.push(acc.into_row(created_at_unix_ms)?);
    }
    rows.sort_by(|left, right| left.group_id.cmp(&right.group_id));
    Ok(rows)
}

fn materialize_level3(
    skill_by_id: &BTreeMap<String, Level2SkillRow>,
    memberships: &[ChunkSkillMembershipRow],
    reverse_indexes: &[SkillReverseIndexRow],
    created_at_unix_ms: i64,
) -> Result<(Vec<Level3ConstellationRow>, Vec<ChunkSkillMembershipRow>), TrainerError> {
    let membership_by_state_skill = memberships
        .iter()
        .map(|row| {
            (
                (row.code_state_key.clone(), row.skill_id.clone()),
                row.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut state_map = BTreeMap::<String, Vec<&SkillReverseIndexRow>>::new();
    for row in reverse_indexes {
        state_map
            .entry(row.code_state_key.clone())
            .or_default()
            .push(row);
    }
    let mut accumulators = BTreeMap::<String, Level3Accumulator>::new();
    for (code_state_key, rows) in state_map {
        let mut skill_ids = rows
            .iter()
            .map(|row| row.skill_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        skill_ids.sort();
        if skill_ids.len() < 2 {
            continue;
        }
        let signature = skill_set_signature(&skill_ids)?;
        let acc = accumulators
            .entry(signature)
            .or_insert_with(|| Level3Accumulator::new(skill_ids.clone()));
        acc.push_state(&code_state_key, &skill_ids, &membership_by_state_skill)?;
    }

    let mut constellations = Vec::new();
    let mut higher_memberships = BTreeMap::<String, ChunkSkillMembershipRow>::new();
    for acc in accumulators.into_values() {
        if acc.support == 0 {
            continue;
        }
        let row = acc.to_row(skill_by_id, created_at_unix_ms)?;
        for membership in acc.to_memberships(&row, skill_by_id, created_at_unix_ms)? {
            higher_memberships.insert(membership.membership_key.clone(), membership);
        }
        constellations.push(row);
    }
    constellations.sort_by(|left, right| {
        right
            .support
            .cmp(&left.support)
            .then_with(|| left.constellation_id.cmp(&right.constellation_id))
    });
    Ok((
        constellations,
        higher_memberships.into_values().collect::<Vec<_>>(),
    ))
}

#[derive(Debug, Clone)]
struct Level1Accumulator {
    group_id: String,
    child_skill_ids: BTreeSet<String>,
    live_label_ids: BTreeSet<String>,
    support: u64,
    confidence_sum: f64,
    lift_sum: f64,
    stability_sum: f64,
    outcome_distribution: SkillOutcomeDistribution,
}

impl Level1Accumulator {
    fn new(group_id: &str) -> Self {
        Self {
            group_id: group_id.to_string(),
            child_skill_ids: BTreeSet::new(),
            live_label_ids: BTreeSet::new(),
            support: 0,
            confidence_sum: 0.0,
            lift_sum: 0.0,
            stability_sum: 0.0,
            outcome_distribution: SkillOutcomeDistribution::default(),
        }
    }

    fn push_skill(&mut self, skill: &Level2SkillRow) {
        self.child_skill_ids.insert(skill.skill_id.clone());
        self.live_label_ids
            .extend(skill.prerequisite_label_ids.iter().cloned());
        self.support += skill.support;
        self.confidence_sum += skill.confidence;
        self.lift_sum += skill.lift_over_cell_baseline;
        self.stability_sum += skill.stability;
        add_distribution(
            &mut self.outcome_distribution,
            skill.oracle_outcome_distribution,
        );
    }

    fn into_row(self, created_at_unix_ms: i64) -> Result<Level1FailureModeGroupRow, TrainerError> {
        let child_skill_ids = self.child_skill_ids.into_iter().collect::<Vec<_>>();
        let divisor = child_skill_ids.len().max(1) as f64;
        let row = Level1FailureModeGroupRow {
            schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
            group_name: format!("group_{}", slugify(&self.group_id)),
            group_id: self.group_id,
            child_skill_ids,
            live_label_ids: capped(self.live_label_ids),
            support: self.support,
            confidence: (self.confidence_sum / divisor).clamp(0.0, 1.0),
            lift_over_cell_baseline: self.lift_sum / divisor,
            stability: (self.stability_sum / divisor).clamp(0.0, 1.0),
            oracle_outcome_distribution: self.outcome_distribution,
            live_input_allowed: true,
            source_provenance_hash: provenance_hash("level1", &[&format!("{:?}", self.support)]),
            created_at_unix_ms,
        };
        row.validate()?;
        Ok(row)
    }
}

#[derive(Debug, Clone)]
struct Level3Accumulator {
    member_skill_ids: Vec<String>,
    source_code_state_keys: BTreeSet<String>,
    source_rows: Vec<ChunkSkillMembershipRow>,
    support: u64,
}

impl Level3Accumulator {
    fn new(member_skill_ids: Vec<String>) -> Self {
        Self {
            member_skill_ids,
            source_code_state_keys: BTreeSet::new(),
            source_rows: Vec::new(),
            support: 0,
        }
    }

    fn push_state(
        &mut self,
        code_state_key: &str,
        skill_ids: &[String],
        membership_by_state_skill: &BTreeMap<(String, String), ChunkSkillMembershipRow>,
    ) -> Result<(), TrainerError> {
        self.support += 1;
        self.source_code_state_keys
            .insert(code_state_key.to_string());
        for skill_id in skill_ids {
            let key = (code_state_key.to_string(), skill_id.clone());
            let row = membership_by_state_skill.get(&key).ok_or_else(|| {
                invalid(
                    "level3_membership_source",
                    format!("missing membership for {code_state_key}/{skill_id}"),
                )
            })?;
            self.source_rows.push(row.clone());
        }
        Ok(())
    }

    fn to_row(
        &self,
        skill_by_id: &BTreeMap<String, Level2SkillRow>,
        created_at_unix_ms: i64,
    ) -> Result<Level3ConstellationRow, TrainerError> {
        let mut groups = BTreeSet::new();
        let mut distribution = SkillOutcomeDistribution::default();
        let mut confidence_sum = 0.0;
        let mut lift_sum = 0.0;
        let mut stability_sum = 0.0;
        for skill_id in &self.member_skill_ids {
            let skill = skill_by_id
                .get(skill_id)
                .ok_or_else(|| invalid("member_skill_ids", format!("missing {skill_id}")))?;
            groups.extend(skill.parent_group_ids.iter().cloned());
            add_distribution(&mut distribution, skill.oracle_outcome_distribution);
            confidence_sum += skill.confidence;
            lift_sum += skill.lift_over_cell_baseline;
            stability_sum += skill.stability;
        }
        let source_chunk_ids = self
            .source_rows
            .iter()
            .map(|row| row.chunk_id.clone())
            .collect::<BTreeSet<_>>();
        let ordered_step_spans = self
            .source_rows
            .iter()
            .flat_map(|row| {
                row.ordered_step_evidence
                    .iter()
                    .map(|step| {
                        let span = OrderedStepSpan {
                            step_index: step.step_index,
                            chunk_id: step.chunk_id.clone(),
                            file_path: step.file_path.clone(),
                            code_state_key: step.code_state_key.clone(),
                        };
                        (
                            (
                                span.code_state_key.clone(),
                                span.chunk_id.clone(),
                                span.step_index,
                            ),
                            span,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .take(MAX_IDS)
            .collect::<Vec<_>>();
        let divisor = self.member_skill_ids.len().max(1) as f64;
        let id = constellation_id(&self.member_skill_ids)?;
        let row = Level3ConstellationRow {
            schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
            constellation_name: format!(
                "ability_{}_skill_constellation",
                self.member_skill_ids.len()
            ),
            constellation_id: id,
            member_skill_ids: self.member_skill_ids.clone(),
            member_group_ids: capped(groups),
            source_code_state_keys: capped(self.source_code_state_keys.clone()),
            source_chunk_ids: capped(source_chunk_ids),
            ordered_step_spans,
            support: self.support,
            confidence: (confidence_sum / divisor).clamp(0.0, 1.0),
            lift_over_cell_baseline: lift_sum / divisor,
            stability: (stability_sum / divisor).clamp(0.0, 1.0),
            oracle_outcome_distribution: distribution,
            live_input_allowed: true,
            target_outcomes_used_for_calibration: true,
            source_provenance_hash: provenance_hash(
                "level3",
                &self
                    .member_skill_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            created_at_unix_ms,
        };
        row.validate()?;
        Ok(row)
    }

    fn to_memberships(
        &self,
        constellation: &Level3ConstellationRow,
        skill_by_id: &BTreeMap<String, Level2SkillRow>,
        created_at_unix_ms: i64,
    ) -> Result<Vec<ChunkSkillMembershipRow>, TrainerError> {
        let mut by_chunk_state = BTreeMap::<(String, String), Vec<ChunkSkillMembershipRow>>::new();
        for row in &self.source_rows {
            by_chunk_state
                .entry((row.chunk_id.clone(), row.code_state_key.clone()))
                .or_default()
                .push(row.clone());
        }
        let mut memberships = Vec::new();
        for ((chunk_id, code_state_key), rows) in by_chunk_state {
            let first = rows
                .first()
                .ok_or_else(|| invalid("level3_membership", "missing source row"))?;
            let file_path = first.file_path.clone();
            let mut labels = BTreeSet::new();
            let mut evidence = Vec::new();
            let mut provenance = Vec::new();
            for row in rows {
                labels.extend(row.source_accepted_label_ids);
                evidence.extend(row.ordered_step_evidence);
                provenance.extend(row.provenance_hashes);
            }
            provenance.push(constellation.source_provenance_hash.clone());
            let key = membership_key(&chunk_id, &code_state_key, &constellation.constellation_id)?;
            let row = ChunkSkillMembershipRow {
                schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
                membership_key: key,
                chunk_id,
                file_path,
                code_state_key,
                skill_id: constellation.constellation_id.clone(),
                hierarchy_level: 3,
                membership_score: constellation.confidence * constellation.stability,
                source_accepted_label_ids: capped(labels),
                ordered_step_evidence: evidence,
                live_input_allowed: true,
                provenance_hashes: provenance
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .take(MAX_IDS)
                    .collect(),
                first_seen_unix_ms: created_at_unix_ms,
                last_seen_unix_ms: created_at_unix_ms,
            };
            row.validate()?;
            for skill_id in &constellation.member_skill_ids {
                if !skill_by_id.contains_key(skill_id) {
                    return Err(invalid(
                        "member_skill_ids",
                        format!("missing source skill {skill_id}"),
                    ));
                }
            }
            memberships.push(row);
        }
        Ok(memberships)
    }
}

fn named_prediction_for_skill(
    skill: &Level2SkillRow,
    created_at_unix_ms: i64,
) -> Result<NamedPredictionRow, TrainerError> {
    let verdict = predicted_verdict(skill.oracle_outcome_distribution);
    let row = NamedPredictionRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        named_prediction_id: named_prediction_id(2, &skill.skill_id, verdict.clone())?,
        prediction_name: format!("predict_{}", skill.skill_name),
        source_hierarchy_id: skill.skill_id.clone(),
        hierarchy_level: 2,
        predicted_verdict: verdict,
        confidence: skill.confidence,
        support: skill.support,
        consequence_label_ids: consequence_labels(skill.oracle_outcome_distribution),
        evidence_group_ids: skill.parent_group_ids.clone(),
        evidence_skill_ids: vec![skill.skill_id.clone()],
        evidence_constellation_ids: Vec::new(),
        live_input_allowed: true,
        target_labels_used_as_live_inputs: false,
        target_outcomes_used_for_calibration: true,
        created_at_unix_ms,
    };
    row.validate()?;
    Ok(row)
}

fn named_prediction_for_constellation(
    constellation: &Level3ConstellationRow,
    created_at_unix_ms: i64,
) -> Result<NamedPredictionRow, TrainerError> {
    let verdict = predicted_verdict(constellation.oracle_outcome_distribution);
    let row = NamedPredictionRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        named_prediction_id: named_prediction_id(
            3,
            &constellation.constellation_id,
            verdict.clone(),
        )?,
        prediction_name: format!("predict_{}", constellation.constellation_name),
        source_hierarchy_id: constellation.constellation_id.clone(),
        hierarchy_level: 3,
        predicted_verdict: verdict,
        confidence: constellation.confidence,
        support: constellation.support,
        consequence_label_ids: consequence_labels(constellation.oracle_outcome_distribution),
        evidence_group_ids: constellation.member_group_ids.clone(),
        evidence_skill_ids: constellation.member_skill_ids.clone(),
        evidence_constellation_ids: vec![constellation.constellation_id.clone()],
        live_input_allowed: true,
        target_labels_used_as_live_inputs: false,
        target_outcomes_used_for_calibration: true,
        created_at_unix_ms,
    };
    row.validate()?;
    Ok(row)
}

fn predicted_verdict(distribution: SkillOutcomeDistribution) -> NamedPredictionVerdict {
    if distribution.fail > distribution.pass {
        NamedPredictionVerdict::FailLikely
    } else if distribution.pass > distribution.fail {
        NamedPredictionVerdict::PassLikely
    } else {
        NamedPredictionVerdict::Unknown
    }
}

fn consequence_labels(distribution: SkillOutcomeDistribution) -> Vec<String> {
    let verdict = predicted_verdict(distribution);
    vec![
        match verdict {
            NamedPredictionVerdict::PassLikely => "prediction:pass_likely",
            NamedPredictionVerdict::FailLikely => "prediction:fail_likely",
            NamedPredictionVerdict::Unknown => "prediction:unknown",
        }
        .to_string(),
        format!(
            "outcome_distribution:pass_{}_fail_{}_unknown_{}",
            distribution.pass, distribution.fail, distribution.unknown
        ),
    ]
}

fn add_distribution(target: &mut SkillOutcomeDistribution, source: SkillOutcomeDistribution) {
    target.pass += source.pass;
    target.fail += source.fail;
    target.unknown += source.unknown;
}

fn skill_set_signature(skill_ids: &[String]) -> Result<String, TrainerError> {
    validate_id_list("skill_set_signature.skill_ids", skill_ids)?;
    Ok(provenance_hash(
        "skill_set",
        &skill_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    ))
}

fn constellation_id(skill_ids: &[String]) -> Result<String, TrainerError> {
    validate_id_list("constellation_id.skill_ids", skill_ids)?;
    let digest = provenance_hash(
        "constellation",
        &skill_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    Ok(format!("ability:skill_constellation:{}", &digest[7..31]))
}

fn named_prediction_id(
    hierarchy_level: u8,
    source_hierarchy_id: &str,
    verdict: NamedPredictionVerdict,
) -> Result<String, TrainerError> {
    validate_id("source_hierarchy_id", source_hierarchy_id)?;
    let verdict = format!("{verdict:?}");
    let digest = provenance_hash("named_prediction", &[source_hierarchy_id, &verdict]);
    Ok(format!(
        "named_prediction:l{hierarchy_level}:{}",
        &digest[7..31]
    ))
}

fn provenance_hash(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part.as_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn capped(values: BTreeSet<String>) -> Vec<String> {
    values.into_iter().take(MAX_IDS).collect()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').chars().take(80).collect()
}

fn read_all_rows<T, F>(db: &DB, cf_name: &str, key_fn: F) -> Result<Vec<T>, TrainerError>
where
    T: DeserializeOwned + ValidateRow,
    F: Fn(&T) -> &str,
{
    let cf_handle = cf(db, cf_name)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid(format!("{cf_name}.key"), err.to_string()))?;
        let row: T = bincode::deserialize(&value).map_err(map_bincode_error)?;
        if key != key_fn(&row) {
            return Err(invalid(
                format!("{cf_name}.key"),
                "key does not match payload",
            ));
        }
        row.validate_row()?;
        rows.push(row);
    }
    Ok(rows)
}

trait ValidateRow {
    fn validate_row(&self) -> Result<(), TrainerError>;
}

impl ValidateRow for Level1FailureModeGroupRow {
    fn validate_row(&self) -> Result<(), TrainerError> {
        self.validate()
    }
}

impl ValidateRow for Level3ConstellationRow {
    fn validate_row(&self) -> Result<(), TrainerError> {
        self.validate()
    }
}

impl ValidateRow for NamedPredictionRow {
    fn validate_row(&self) -> Result<(), TrainerError> {
        self.validate()
    }
}

fn read_row<T: DeserializeOwned>(
    db: &DB,
    cf_name: &str,
    key: &str,
) -> Result<Option<T>, TrainerError> {
    validate_id("read_row.key", key)?;
    let cf_handle = cf(db, cf_name)?;
    db.get_cf(cf_handle, key.as_bytes())
        .map_err(map_rocksdb_error)?
        .map(|bytes| bincode::deserialize(&bytes).map_err(map_bincode_error))
        .transpose()
}

fn serialize<T: Serialize>(row: &T) -> Result<Vec<u8>, TrainerError> {
    bincode::serialize(row).map_err(map_bincode_error)
}

fn cf<'a>(db: &'a DB, name: &str) -> Result<&'a rocksdb::ColumnFamily, TrainerError> {
    db.cf_handle(name)
        .ok_or_else(|| invalid("rocksdb.column_family", format!("missing {name}")))
}

fn validate_schema(schema_version: u32) -> Result<(), TrainerError> {
    if schema_version != SKILL_SEQUENCE_SCHEMA_VERSION {
        return Err(invalid(
            "schema_version",
            format!(
                "expected {}, got {}",
                SKILL_SEQUENCE_SCHEMA_VERSION, schema_version
            ),
        ));
    }
    Ok(())
}

fn validate_positive(field: &str, value: u64) -> Result<(), TrainerError> {
    if value == 0 {
        return Err(invalid(field, "must be positive"));
    }
    Ok(())
}

fn validate_timestamp(value: i64) -> Result<(), TrainerError> {
    if value <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_id_list(field: &str, values: &[String]) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(SOURCE_FILE, REMEDIATION, field, values, MAX_IDS)
}

fn validate_live_id_list(field: &str, values: &[String]) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(SOURCE_FILE, REMEDIATION, field, values, MAX_IDS)
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_finite_unit(field: &str, value: f64) -> Result<(), TrainerError> {
    skill_validation::validate_finite_unit(SOURCE_FILE, REMEDIATION, field, value)
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    skill_validation::invalid(SOURCE_FILE, REMEDIATION, field, message)
}

fn map_rocksdb_error(err: rocksdb::Error) -> TrainerError {
    invalid("rocksdb", err.to_string())
}

fn map_bincode_error(err: Box<bincode::ErrorKind>) -> TrainerError {
    invalid("bincode", err.to_string())
}
