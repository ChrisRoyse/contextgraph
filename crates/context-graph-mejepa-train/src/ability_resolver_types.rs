use crate::chunk_skill_membership::{ChunkSkillMembershipRow, SkillLifecycleAuditRow};
use crate::error::TrainerError;
use crate::label_bridge::{
    ability_signature_hash, membership_signature_hash, skill_signature_hash,
};
use crate::mistake_log::{MistakeLogRow, MistakeTruthSource};
use crate::online_head_state::OnlineHeadUpdateReport;
use crate::replay_buffer::ReplayBufferRow;
use crate::skill_sequence_discovery::Level2SkillRow;
use context_graph_mejepa::{PredictionId, Verdict};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::{
    invalid, optional_ordered_hash, validate_id, validate_id_list, validate_live_id_list,
    validate_project_relative_path, MAX_CHUNKS, MAX_IDS,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveChunkInput {
    pub chunk_id: String,
    pub file_path: Option<String>,
    pub accepted_label_ids: Vec<String>,
}

impl LiveChunkInput {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("chunk_id", &self.chunk_id)?;
        if let Some(path) = &self.file_path {
            validate_project_relative_path("file_path", path)?;
        }
        validate_live_id_list("accepted_label_ids", &self.accepted_label_ids, MAX_IDS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AbilityResolverRequest {
    pub code_state_key: String,
    pub chunks: Vec<LiveChunkInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResolvedChunkAbility {
    pub chunk_id: String,
    pub file_path: Option<String>,
    pub live_accepted_label_ids: Vec<String>,
    pub memberships: Vec<ChunkSkillMembershipRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AbilityContext {
    pub schema_version: u32,
    pub code_state_key: String,
    pub chunk_ids: Vec<String>,
    pub chunk_contexts: Vec<ResolvedChunkAbility>,
    pub accepted_label_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub active_higher_ability_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub skill_signature_hash: Option<String>,
    pub ability_signature_hash: Option<String>,
    pub membership_signature_hash: Option<String>,
    pub live_input_allowed: bool,
    pub no_new_prediction_head_introduced: bool,
    pub hidden_intent_inference_used: bool,
    pub target_side_labels_used_as_live_inputs: bool,
    pub flat_vector_concat_used: bool,
}

impl AbilityContext {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("code_state_key", &self.code_state_key)?;
        validate_id_list("chunk_ids", &self.chunk_ids, MAX_CHUNKS)?;
        if self.chunk_contexts.len() != self.chunk_ids.len() {
            return Err(invalid(
                "chunk_contexts",
                "must contain exactly one context per requested chunk",
            ));
        }
        validate_live_id_list("accepted_label_ids", &self.accepted_label_ids, MAX_IDS)?;
        validate_id_list("active_skill_ids", &self.active_skill_ids, MAX_IDS)?;
        validate_id_list(
            "active_higher_ability_ids",
            &self.active_higher_ability_ids,
            MAX_IDS,
        )?;
        validate_id_list(
            "source_membership_keys",
            &self.source_membership_keys,
            MAX_IDS,
        )?;
        let expected_skill = optional_ordered_hash(&self.active_skill_ids, skill_signature_hash)?;
        if self.skill_signature_hash != expected_skill {
            return Err(invalid(
                "skill_signature_hash",
                "does not match active_skill_ids",
            ));
        }
        let expected_ability =
            optional_ordered_hash(&self.active_higher_ability_ids, ability_signature_hash)?;
        if self.ability_signature_hash != expected_ability {
            return Err(invalid(
                "ability_signature_hash",
                "does not match active_higher_ability_ids",
            ));
        }
        let expected_membership =
            optional_ordered_hash(&self.source_membership_keys, membership_signature_hash)?;
        if self.membership_signature_hash != expected_membership {
            return Err(invalid(
                "membership_signature_hash",
                "does not match source_membership_keys",
            ));
        }
        if !self.live_input_allowed
            || self.hidden_intent_inference_used
            || self.target_side_labels_used_as_live_inputs
            || self.flat_vector_concat_used
        {
            return Err(invalid(
                "runtime_policy",
                "ability context must be live-safe and slot-preserving",
            ));
        }
        if !self.no_new_prediction_head_introduced {
            return Err(invalid(
                "no_new_prediction_head_introduced",
                "resolver is evidence plumbing, not a prediction head",
            ));
        }
        for (expected_chunk, context) in self.chunk_ids.iter().zip(&self.chunk_contexts) {
            if &context.chunk_id != expected_chunk {
                return Err(invalid(
                    "chunk_contexts",
                    "chunk context order must match request order",
                ));
            }
            validate_live_id_list(
                "chunk_context.live_accepted_label_ids",
                &context.live_accepted_label_ids,
                MAX_IDS,
            )?;
            for row in &context.memberships {
                row.validate()?;
                if row.chunk_id != context.chunk_id || row.code_state_key != self.code_state_key {
                    return Err(invalid(
                        "membership",
                        "membership must match chunk_id and code_state_key",
                    ));
                }
            }
        }
        let expected = recompute_aggregate(self)?;
        if self.accepted_label_ids != expected.accepted_label_ids
            || self.active_skill_ids != expected.active_skill_ids
            || self.active_higher_ability_ids != expected.active_higher_ability_ids
            || self.source_membership_keys != expected.source_membership_keys
        {
            return Err(invalid(
                "aggregate_ids",
                "aggregate label/skill/ability/membership ids must be derived from chunk contexts",
            ));
        }
        Ok(())
    }
}

struct RecomputedAggregate {
    accepted_label_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    active_higher_ability_ids: Vec<String>,
    source_membership_keys: Vec<String>,
}

fn recompute_aggregate(context: &AbilityContext) -> Result<RecomputedAggregate, TrainerError> {
    let mut labels = BTreeSet::new();
    let mut active_skill_ids = Vec::new();
    let mut active_higher_ability_ids = Vec::new();
    let mut source_membership_keys = Vec::new();
    let mut seen_skills = BTreeSet::new();
    let mut seen_abilities = BTreeSet::new();
    for chunk_context in &context.chunk_contexts {
        labels.extend(chunk_context.live_accepted_label_ids.iter().cloned());
        for row in &chunk_context.memberships {
            labels.extend(row.source_accepted_label_ids.iter().cloned());
            source_membership_keys.push(row.membership_key.clone());
            if row.hierarchy_level == 2 {
                push_first_seen(&mut active_skill_ids, &mut seen_skills, &row.skill_id);
            } else {
                push_first_seen(
                    &mut active_higher_ability_ids,
                    &mut seen_abilities,
                    &row.skill_id,
                );
            }
        }
    }
    Ok(RecomputedAggregate {
        accepted_label_ids: labels.into_iter().collect(),
        active_skill_ids,
        active_higher_ability_ids,
        source_membership_keys,
    })
}

fn push_first_seen(target: &mut Vec<String>, seen: &mut BTreeSet<String>, value: &str) {
    if seen.insert(value.to_string()) {
        target.push(value.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AbilityRefutationInput {
    pub prediction_id: PredictionId,
    pub panel_signature_hash: String,
    pub predicted_verdict: Verdict,
    pub ground_truth_verdict: Verdict,
    pub truth_source: MistakeTruthSource,
    pub language: String,
    pub mutation_or_live_cell: String,
    pub named_failure_mode: String,
    pub surprise_z: f32,
    pub coverage_gap_score: f32,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AbilityRefutationReport {
    pub schema_version: u32,
    pub mistake_row: MistakeLogRow,
    pub replay_row: ReplayBufferRow,
    pub online_head_update_report: OnlineHeadUpdateReport,
    pub lifecycle_audits: Vec<SkillLifecycleAuditRow>,
    pub updated_skill_rows: Vec<Level2SkillRow>,
    pub candidate_skill_rows: Vec<Level2SkillRow>,
    pub label_skill_ability_membership_ids_agree: bool,
    pub candidate_created_when_no_existing_ability: bool,
    pub no_new_prediction_head_introduced: bool,
    pub hidden_intent_inference_used: bool,
    pub target_side_labels_used_as_live_inputs: bool,
    pub flat_vector_concat_used: bool,
}
