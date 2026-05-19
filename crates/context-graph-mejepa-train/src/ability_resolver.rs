//! TASK-PY-G-121 runtime resolver for chunk -> skill/ability evidence.
//!
//! Prediction may consume live-safe labels and persisted chunk-skill
//! memberships. It must not consume target-only oracle labels, collapse to one
//! "best" skill, or turn the embedder panel into a flat vector.

use crate::chunk_skill_membership::{
    read_all_chunk_skill_membership_rows, ChunkSkillMembershipRow,
};
use crate::error::TrainerError;
use crate::label_bridge::{
    ability_signature_hash, build_prediction_label_context_with_abilities,
    membership_signature_hash, skill_signature_hash, LabelLearningBridge,
};
use crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION;
use crate::skill_validation;
use context_graph_mejepa::PredictionLabelContext;
use rocksdb::DB;
use std::collections::{BTreeMap, BTreeSet};

#[path = "ability_resolver_refutation.rs"]
mod ability_resolver_refutation;
#[path = "ability_resolver_storage.rs"]
mod ability_resolver_storage;
#[path = "ability_resolver_types.rs"]
mod ability_resolver_types;

pub use ability_resolver_refutation::record_ability_refutation_sync_readback;
pub use ability_resolver_storage::{ability_resolver_cfs, open_ability_resolver_rocksdb};
pub use ability_resolver_types::{
    AbilityContext, AbilityRefutationInput, AbilityRefutationReport, AbilityResolverRequest,
    LiveChunkInput, ResolvedChunkAbility,
};

pub(super) const MAX_CHUNKS: usize = 4096;
pub(super) const MAX_IDS: usize = 4096;
pub(super) const SOURCE_FILE: &str =
    "file:crates/context-graph-mejepa-train/src/ability_resolver.rs";
const REMEDIATION: &str = "runtime ability resolution must preserve chunk memberships, use only live-safe labels, and repair ontology state on refuted predictions";

pub fn resolve_ability_context(
    db: &DB,
    request: AbilityResolverRequest,
) -> Result<AbilityContext, TrainerError> {
    validate_resolver_request(&request)?;
    let memberships = memberships_by_chunk(db, &request.code_state_key)?;
    let mut chunk_contexts = Vec::with_capacity(request.chunks.len());
    let mut labels = BTreeSet::new();
    let mut active_skill_ids = Vec::new();
    let mut active_higher_ability_ids = Vec::new();
    let mut source_membership_keys = Vec::new();
    let mut seen_skills = BTreeSet::new();
    let mut seen_abilities = BTreeSet::new();

    for chunk in &request.chunks {
        labels.extend(chunk.accepted_label_ids.iter().cloned());
        let mut rows = memberships
            .get(&chunk.chunk_id)
            .cloned()
            .unwrap_or_default();
        rows.sort_by(compare_memberships);
        for row in &rows {
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
        chunk_contexts.push(ResolvedChunkAbility {
            chunk_id: chunk.chunk_id.clone(),
            file_path: chunk.file_path.clone(),
            live_accepted_label_ids: chunk.accepted_label_ids.clone(),
            memberships: rows,
        });
    }
    let context = AbilityContext {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        code_state_key: request.code_state_key,
        chunk_ids: request
            .chunks
            .iter()
            .map(|chunk| chunk.chunk_id.clone())
            .collect(),
        chunk_contexts,
        accepted_label_ids: labels.into_iter().collect(),
        skill_signature_hash: optional_ordered_hash(&active_skill_ids, skill_signature_hash)?,
        ability_signature_hash: optional_ordered_hash(
            &active_higher_ability_ids,
            ability_signature_hash,
        )?,
        membership_signature_hash: optional_ordered_hash(
            &source_membership_keys,
            membership_signature_hash,
        )?,
        active_skill_ids,
        active_higher_ability_ids,
        source_membership_keys,
        live_input_allowed: true,
        no_new_prediction_head_introduced: true,
        hidden_intent_inference_used: false,
        target_side_labels_used_as_live_inputs: false,
        flat_vector_concat_used: false,
    };
    context.validate()?;
    Ok(context)
}

pub fn prediction_label_context_from_ability_context(
    bridge: &LabelLearningBridge,
    context: &AbilityContext,
    failure_evidence_set_ids: Vec<String>,
) -> Result<PredictionLabelContext, TrainerError> {
    context.validate()?;
    build_prediction_label_context_with_abilities(
        bridge,
        context.accepted_label_ids.clone(),
        Some(context.code_state_key.clone()),
        failure_evidence_set_ids,
        context.active_skill_ids.clone(),
        context.active_higher_ability_ids.clone(),
        context.source_membership_keys.clone(),
    )
}

fn memberships_by_chunk(
    db: &DB,
    code_state_key: &str,
) -> Result<BTreeMap<String, Vec<ChunkSkillMembershipRow>>, TrainerError> {
    let mut map = BTreeMap::<String, Vec<ChunkSkillMembershipRow>>::new();
    for row in read_all_chunk_skill_membership_rows(db)? {
        if row.code_state_key == code_state_key {
            map.entry(row.chunk_id.clone()).or_default().push(row);
        }
    }
    Ok(map)
}

fn validate_resolver_request(request: &AbilityResolverRequest) -> Result<(), TrainerError> {
    validate_id("code_state_key", &request.code_state_key)?;
    if request.chunks.is_empty() {
        return Err(invalid("chunks", "must include at least one live chunk"));
    }
    if request.chunks.len() > MAX_CHUNKS {
        return Err(invalid(
            "chunks",
            format!("too many chunks: {}", request.chunks.len()),
        ));
    }
    let mut seen = BTreeSet::new();
    for chunk in &request.chunks {
        chunk.validate()?;
        if !seen.insert(&chunk.chunk_id) {
            return Err(invalid("chunks", "duplicate chunk_id in request"));
        }
    }
    Ok(())
}

fn compare_memberships(
    left: &ChunkSkillMembershipRow,
    right: &ChunkSkillMembershipRow,
) -> std::cmp::Ordering {
    membership_min_step(left)
        .cmp(&membership_min_step(right))
        .then_with(|| left.hierarchy_level.cmp(&right.hierarchy_level))
        .then_with(|| left.skill_id.cmp(&right.skill_id))
        .then_with(|| left.membership_key.cmp(&right.membership_key))
}

fn membership_min_step(row: &ChunkSkillMembershipRow) -> u32 {
    row.ordered_step_evidence
        .iter()
        .map(|step| step.step_index)
        .min()
        .unwrap_or(0)
}

fn push_first_seen(target: &mut Vec<String>, seen: &mut BTreeSet<String>, value: &str) {
    if seen.insert(value.to_string()) {
        target.push(value.to_string());
    }
}

pub(super) fn optional_ordered_hash(
    ids: &[String],
    hash: fn(&[String]) -> Result<String, TrainerError>,
) -> Result<Option<String>, TrainerError> {
    if ids.is_empty() {
        Ok(None)
    } else {
        hash(ids).map(Some)
    }
}

pub(super) fn validate_live_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

pub(super) fn validate_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

pub(super) fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

pub(super) fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

pub(super) fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    skill_validation::invalid(SOURCE_FILE, REMEDIATION, field, message)
}

#[cfg(test)]
#[path = "ability_resolver_tests.rs"]
mod ability_resolver_tests;
