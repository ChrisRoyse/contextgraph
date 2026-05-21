use crate::chunk_skill_membership::{
    lifecycle_audit_id_from_parts, read_level2_skill_row,
    write_skill_materialization_sync_readback, SkillLifecycleAuditRow, SkillLifecycleDecision,
    SkillMaterialization,
};
use crate::error::{TrainerError, TrainerErrorCode};
use crate::label_bridge::accepted_label_signature_hash;
use crate::mistake_log::{
    mistake_id_from_evidence_parts, write_mistake_log_row_sync_readback, MistakeLogRow,
};
use crate::online_head_state::{
    apply_online_mistake_update_sync_readback, OnlineHeadUpdateConfig, OnlineHeadUpdateInput,
};
use crate::replay_buffer::{
    ability_aware_replay_cell_id, prediction_hex, write_replay_row_sync_readback, ReplayBufferRow,
    ReplayBufferSource, ReplayRetentionTier,
};
use crate::skill_sequence_discovery::{
    Level2SkillRow, SkillOutcomeDistribution, SkillPromotionStatus, SkillStepTemplate,
    SkillTransitionEdge, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use context_graph_mejepa::{PredictionLabelContext, Verdict};
use rocksdb::DB;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{
    invalid, validate_id, AbilityContext, AbilityRefutationInput, AbilityRefutationReport,
};

pub fn record_ability_refutation_sync_readback(
    db: &DB,
    context: &AbilityContext,
    label_context: &PredictionLabelContext,
    input: AbilityRefutationInput,
) -> Result<AbilityRefutationReport, TrainerError> {
    context.validate()?;
    validate_refutation_input(&input)?;
    validate_label_context_matches(context, label_context)?;
    let label_signature = label_context.label_signature_hash.clone().ok_or_else(|| {
        invalid(
            "label_signature_hash",
            "must be present for mistake logging",
        )
    })?;
    let replay_cell_id = ability_aware_replay_cell_id(
        &input.language,
        &input.mutation_or_live_cell,
        &context.code_state_key,
        &input.named_failure_mode,
        &label_signature,
        label_context.skill_signature_hash.as_deref(),
        label_context.ability_signature_hash.as_deref(),
        label_context.membership_signature_hash.as_deref(),
    )?;
    let replay_row = ReplayBufferRow {
        prediction_id: input.prediction_id,
        surprise_z: input.surprise_z,
        cell_id: replay_cell_id,
        coverage_gap_score: input.coverage_gap_score,
        accepted_label_ids: label_context.accepted_label_ids.clone(),
        active_skill_ids: label_context.active_skill_ids.clone(),
        active_higher_ability_ids: label_context.active_higher_ability_ids.clone(),
        source_membership_keys: label_context.source_membership_keys.clone(),
        label_signature_hash: label_context.label_signature_hash.clone(),
        skill_signature_hash: label_context.skill_signature_hash.clone(),
        ability_signature_hash: label_context.ability_signature_hash.clone(),
        membership_signature_hash: label_context.membership_signature_hash.clone(),
        last_replayed_ts: None,
        replay_count: 0,
        retention_weight: 1.0,
        protected: true,
        retention_tier: ReplayRetentionTier::Hot,
        source: ReplayBufferSource::ConstellationSkillMistake,
        created_at_unix_ms: input.created_at_unix_ms,
        updated_at_unix_ms: input.created_at_unix_ms,
    };
    write_replay_row_sync_readback(db, &replay_row)?;
    let mistake_id = mistake_id_from_evidence_parts(
        input.prediction_id,
        &context.code_state_key,
        &label_signature,
        label_context.skill_signature_hash.as_deref(),
        label_context.ability_signature_hash.as_deref(),
        label_context.membership_signature_hash.as_deref(),
        input.ground_truth_verdict,
    )?;
    let mistake_row = MistakeLogRow {
        schema_version: 1,
        mistake_id: mistake_id.clone(),
        prediction_id: input.prediction_id,
        predicted_verdict: input.predicted_verdict,
        ground_truth_verdict: input.ground_truth_verdict,
        truth_source: input.truth_source,
        code_state_key: context.code_state_key.clone(),
        named_failure_mode: Some(input.named_failure_mode.clone()),
        accepted_label_ids: label_context.accepted_label_ids.clone(),
        active_skill_ids: label_context.active_skill_ids.clone(),
        active_higher_ability_ids: label_context.active_higher_ability_ids.clone(),
        source_membership_keys: label_context.source_membership_keys.clone(),
        label_signature_hash: label_signature,
        skill_signature_hash: label_context.skill_signature_hash.clone(),
        ability_signature_hash: label_context.ability_signature_hash.clone(),
        membership_signature_hash: label_context.membership_signature_hash.clone(),
        failure_evidence_set_ids: label_context.failure_evidence_set_ids.clone(),
        replay_row_key: prediction_hex(input.prediction_id),
        accepted_registry_sha256: label_context.accepted_registry_sha256.clone(),
        usefulness_metrics_sha256: label_context.usefulness_metrics_sha256.clone(),
        learning_bridge_manifest_sha256: label_context.learning_bridge_manifest_sha256.clone(),
        created_at_unix_ms: input.created_at_unix_ms,
    };
    write_mistake_log_row_sync_readback(db, &mistake_row)?;
    let lifecycle_repair = lifecycle_repair_for_refutation(db, context, &mistake_row, &input)?;
    write_skill_materialization_sync_readback(
        db,
        &SkillMaterialization {
            level2_skills: lifecycle_repair
                .updated_skill_rows
                .iter()
                .chain(lifecycle_repair.candidate_skill_rows.iter())
                .cloned()
                .collect(),
            chunk_memberships: Vec::new(),
            reverse_indexes: Vec::new(),
            lifecycle_audits: lifecycle_repair.lifecycle_audits.clone(),
        },
    )?;
    let online_head_update_report = apply_online_mistake_update_sync_readback(
        db,
        OnlineHeadUpdateInput {
            panel_signature_hash: input.panel_signature_hash.clone(),
            mistake_row: mistake_row.clone(),
            replay_row: replay_row.clone(),
            base_verdict_before_update: input.predicted_verdict,
            now_unix_ms: input.created_at_unix_ms,
        },
        OnlineHeadUpdateConfig::default(),
    )?;
    let candidate_created_when_no_existing_ability = context.active_skill_ids.is_empty()
        && context.active_higher_ability_ids.is_empty()
        && lifecycle_repair
            .lifecycle_audits
            .iter()
            .any(|row| row.decision == SkillLifecycleDecision::CreateNewCandidateSkill);
    let report = AbilityRefutationReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        label_skill_ability_membership_ids_agree: report_rows_agree(
            context,
            &mistake_row,
            &replay_row,
            &lifecycle_repair.lifecycle_audits,
        ),
        mistake_row,
        replay_row,
        online_head_update_report,
        lifecycle_audits: lifecycle_repair.lifecycle_audits,
        updated_skill_rows: lifecycle_repair.updated_skill_rows,
        candidate_skill_rows: lifecycle_repair.candidate_skill_rows,
        candidate_created_when_no_existing_ability,
        no_new_prediction_head_introduced: true,
        hidden_intent_inference_used: false,
        target_side_labels_used_as_live_inputs: false,
        flat_vector_concat_used: false,
    };
    if !report.label_skill_ability_membership_ids_agree {
        return Err(invalid(
            "refutation_report",
            "mistake, replay, and lifecycle rows disagree on ability evidence",
        ));
    }
    Ok(report)
}

#[derive(Debug, Clone, Default)]
struct LifecycleRepair {
    lifecycle_audits: Vec<SkillLifecycleAuditRow>,
    updated_skill_rows: Vec<Level2SkillRow>,
    candidate_skill_rows: Vec<Level2SkillRow>,
}

fn lifecycle_repair_for_refutation(
    db: &DB,
    context: &AbilityContext,
    mistake: &MistakeLogRow,
    input: &AbilityRefutationInput,
) -> Result<LifecycleRepair, TrainerError> {
    let mut repair = LifecycleRepair::default();
    let mut involved = context
        .active_skill_ids
        .iter()
        .chain(context.active_higher_ability_ids.iter())
        .cloned()
        .collect::<Vec<_>>();
    if involved.is_empty() {
        involved.push(candidate_skill_id(context, input)?);
    }
    for (idx, skill_or_ability_id) in involved.iter().enumerate() {
        let existing = context.active_skill_ids.contains(skill_or_ability_id)
            || context
                .active_higher_ability_ids
                .contains(skill_or_ability_id);
        let skill_row = if context.active_skill_ids.contains(skill_or_ability_id) {
            read_level2_skill_row(db, skill_or_ability_id)?
        } else {
            None
        };
        let decision = lifecycle_decision_for_skill(skill_or_ability_id, skill_row.as_ref(), input);
        let candidate_skill_id = match decision {
            SkillLifecycleDecision::CreateNewCandidateSkill => {
                Some(candidate_skill_id(context, input)?)
            }
            SkillLifecycleDecision::SplitMixedSkill => Some(derived_candidate_skill_id(
                "split",
                Some(skill_or_ability_id),
                context,
                input,
            )?),
            _ => None,
        };
        match decision {
            SkillLifecycleDecision::UpdateExistingSkill => {
                if let Some(skill) = skill_row {
                    repair
                        .updated_skill_rows
                        .push(updated_skill_after_mistake(skill, mistake)?);
                }
            }
            SkillLifecycleDecision::DemoteUnstableSkill => {
                if let Some(skill) = skill_row {
                    repair
                        .updated_skill_rows
                        .push(demoted_skill_after_mistake(skill, mistake)?);
                }
            }
            SkillLifecycleDecision::SplitMixedSkill => {
                if let Some(skill) = skill_row {
                    repair.candidate_skill_rows.push(candidate_skill_row(
                        candidate_skill_id
                            .as_deref()
                            .ok_or_else(|| invalid("candidate_skill_id", "missing split id"))?,
                        "split_mixed_skill_candidate",
                        Some(&skill),
                        context,
                        mistake,
                        input,
                    )?);
                }
            }
            SkillLifecycleDecision::CreateNewCandidateSkill => {
                repair.candidate_skill_rows.push(candidate_skill_row(
                    candidate_skill_id
                        .as_deref()
                        .ok_or_else(|| invalid("candidate_skill_id", "missing candidate id"))?,
                    "unknown_ordered_constellation_candidate",
                    None,
                    context,
                    mistake,
                    input,
                )?);
            }
            SkillLifecycleDecision::NoChangeWithEvidence => {}
        }
        let audit = SkillLifecycleAuditRow {
            schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
            skill_audit_id: lifecycle_audit_id_from_parts(
                Some(skill_or_ability_id),
                decision,
                input.created_at_unix_ms + idx as i64,
            )?,
            prediction_id: Some(prediction_hex(input.prediction_id)),
            mistake_id: Some(mistake.mistake_id.clone()),
            previous_skill_id: existing.then_some(skill_or_ability_id.clone()),
            decision,
            candidate_skill_id,
            evidence_label_ids: context.accepted_label_ids.clone(),
            evidence_skill_ids: context.active_skill_ids.clone(),
            evidence_higher_ability_ids: context.active_higher_ability_ids.clone(),
            evidence_chunk_ids: context.chunk_ids.clone(),
            source_membership_keys: context.source_membership_keys.clone(),
            reason: lifecycle_reason(decision).to_string(),
            created_at_unix_ms: input.created_at_unix_ms + idx as i64,
        };
        audit.validate()?;
        repair.lifecycle_audits.push(audit);
    }
    Ok(repair)
}

fn lifecycle_decision_for_skill(
    skill_or_ability_id: &str,
    skill: Option<&Level2SkillRow>,
    input: &AbilityRefutationInput,
) -> SkillLifecycleDecision {
    let Some(skill) = skill else {
        return if skill_or_ability_id.starts_with("ability:") {
            SkillLifecycleDecision::UpdateExistingSkill
        } else {
            SkillLifecycleDecision::CreateNewCandidateSkill
        };
    };
    if is_background_or_generic_skill(skill) {
        return SkillLifecycleDecision::DemoteUnstableSkill;
    }
    if is_mixed_consequence_skill(skill, input) {
        return SkillLifecycleDecision::SplitMixedSkill;
    }
    if weak_causal_skill_evidence(input) {
        return SkillLifecycleDecision::NoChangeWithEvidence;
    }
    SkillLifecycleDecision::UpdateExistingSkill
}

fn is_background_or_generic_skill(skill: &Level2SkillRow) -> bool {
    skill.promotion_status == SkillPromotionStatus::Demoted
        || skill.skill_id.contains("background")
        || skill.skill_name.contains("background")
        || skill
            .prerequisite_label_ids
            .iter()
            .chain(skill.parent_group_ids.iter())
            .any(|id| id.contains("background") || id.contains("stable_context"))
}

fn is_mixed_consequence_skill(skill: &Level2SkillRow, input: &AbilityRefutationInput) -> bool {
    (skill.oracle_outcome_distribution.pass > 0 && skill.oracle_outcome_distribution.fail > 0)
        || input.named_failure_mode.contains("mixed_consequence")
}

fn weak_causal_skill_evidence(input: &AbilityRefutationInput) -> bool {
    input.surprise_z <= 0.05 && input.coverage_gap_score <= 0.05
}

fn lifecycle_reason(decision: SkillLifecycleDecision) -> &'static str {
    match decision {
        SkillLifecycleDecision::UpdateExistingSkill => {
            "reality_refuted_prediction_update_existing_skill"
        }
        SkillLifecycleDecision::SplitMixedSkill => {
            "reality_refuted_prediction_split_mixed_consequence_skill"
        }
        SkillLifecycleDecision::CreateNewCandidateSkill => {
            "reality_refuted_prediction_create_unknown_ordered_constellation"
        }
        SkillLifecycleDecision::DemoteUnstableSkill => {
            "reality_refuted_prediction_demote_unstable_or_background_skill"
        }
        SkillLifecycleDecision::NoChangeWithEvidence => {
            "reality_refuted_prediction_skill_evidence_not_causal"
        }
    }
}

fn updated_skill_after_mistake(
    mut skill: Level2SkillRow,
    mistake: &MistakeLogRow,
) -> Result<Level2SkillRow, TrainerError> {
    observe_skill_outcome(&mut skill, mistake);
    recalibrate_skill_usefulness(&mut skill);
    skill.validate()?;
    Ok(skill)
}

fn demoted_skill_after_mistake(
    mut skill: Level2SkillRow,
    mistake: &MistakeLogRow,
) -> Result<Level2SkillRow, TrainerError> {
    observe_skill_outcome(&mut skill, mistake);
    skill.confidence = (skill.confidence * 0.25).clamp(0.0, 0.49);
    skill.lift_over_cell_baseline = 0.0;
    skill.promotion_status = SkillPromotionStatus::Demoted;
    skill.operator_approved = false;
    skill.validate()?;
    Ok(skill)
}

fn observe_skill_outcome(skill: &mut Level2SkillRow, mistake: &MistakeLogRow) {
    match mistake.ground_truth_verdict {
        Verdict::Pass => skill.oracle_outcome_distribution.pass += 1,
        Verdict::Fail => skill.oracle_outcome_distribution.fail += 1,
        _ => skill.oracle_outcome_distribution.unknown += 1,
    }
    push_unique(&mut skill.code_state_keys, &mistake.code_state_key);
    push_unique(&mut skill.source_episode_ids, &mistake.mistake_id);
    for evidence_id in &mistake.failure_evidence_set_ids {
        push_unique(&mut skill.failure_evidence_set_ids, evidence_id);
    }
    skill.support = skill.source_episode_ids.len() as u64;
}

fn recalibrate_skill_usefulness(skill: &mut Level2SkillRow) {
    let supervised =
        skill.oracle_outcome_distribution.pass + skill.oracle_outcome_distribution.fail;
    if supervised == 0 {
        return;
    }
    let dominant = skill
        .oracle_outcome_distribution
        .pass
        .max(skill.oracle_outcome_distribution.fail);
    skill.confidence = dominant as f64 / supervised as f64;
    skill.lift_over_cell_baseline = (skill.confidence - 0.5).max(0.0);
}

fn candidate_skill_row(
    skill_id: &str,
    skill_name: &str,
    previous_skill: Option<&Level2SkillRow>,
    context: &AbilityContext,
    mistake: &MistakeLogRow,
    input: &AbilityRefutationInput,
) -> Result<Level2SkillRow, TrainerError> {
    validate_id("candidate_skill_id", skill_id)?;
    let ordered_steps = previous_skill
        .map(|skill| skill.ordered_steps.clone())
        .unwrap_or_else(|| candidate_steps_from_context(context));
    let parent_skill_ids = previous_skill
        .map(|skill| vec![skill.skill_id.clone()])
        .unwrap_or_default();
    let mut parent_group_ids = previous_skill
        .map(|skill| skill.parent_group_ids.clone())
        .unwrap_or_else(|| vec!["group:skill_lifecycle:mistake_loop".to_string()]);
    push_unique(
        &mut parent_group_ids,
        if previous_skill.is_some() {
            "group:skill_lifecycle:split_mixed"
        } else {
            "group:skill_lifecycle:unknown_ordered_constellation"
        },
    );
    let mut prerequisite_label_ids = ordered_steps
        .first()
        .map(|step| step.accepted_label_ids.clone())
        .unwrap_or_else(|| vec!["skill_lifecycle:unknown_live_evidence".to_string()]);
    prerequisite_label_ids.sort();
    prerequisite_label_ids.dedup();
    let mut row = Level2SkillRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        parent_group_ids,
        parent_skill_ids,
        transition_edges: transition_edges_for_steps(&ordered_steps),
        ordered_steps,
        prerequisite_label_ids,
        support: 1,
        confidence: 1.0,
        lift_over_cell_baseline: 0.5,
        stability: 1.0,
        oracle_outcome_distribution: SkillOutcomeDistribution::default(),
        code_state_keys: vec![context.code_state_key.clone()],
        source_episode_ids: vec![mistake.mistake_id.clone()],
        failure_evidence_set_ids: mistake.failure_evidence_set_ids.clone(),
        live_input_allowed: true,
        promotion_status: SkillPromotionStatus::ActiveLearning,
        operator_approved: false,
        created_at_unix_ms: input.created_at_unix_ms,
    };
    match mistake.ground_truth_verdict {
        Verdict::Pass => row.oracle_outcome_distribution.pass = 1,
        Verdict::Fail => row.oracle_outcome_distribution.fail = 1,
        _ => row.oracle_outcome_distribution.unknown = 1,
    }
    row.validate()?;
    Ok(row)
}

fn candidate_steps_from_context(context: &AbilityContext) -> Vec<SkillStepTemplate> {
    context
        .chunk_contexts
        .iter()
        .enumerate()
        .map(|(idx, chunk)| {
            let mut labels = chunk.live_accepted_label_ids.clone();
            if labels.is_empty() {
                labels = context.accepted_label_ids.clone();
            }
            if labels.is_empty() {
                labels.push("skill_lifecycle:unknown_live_evidence".to_string());
            }
            labels.sort();
            labels.dedup();
            SkillStepTemplate {
                step_index: idx as u32,
                accepted_label_ids: labels,
                group_ids: vec!["group:skill_lifecycle:mistake_loop".to_string()],
            }
        })
        .collect()
}

fn transition_edges_for_steps(steps: &[SkillStepTemplate]) -> Vec<SkillTransitionEdge> {
    steps
        .windows(2)
        .map(|pair| SkillTransitionEdge {
            from_step_index: pair[0].step_index,
            to_step_index: pair[1].step_index,
            edge_label: "sequence:next".to_string(),
        })
        .collect()
}

fn derived_candidate_skill_id(
    prefix: &str,
    previous_skill_id: Option<&str>,
    context: &AbilityContext,
    input: &AbilityRefutationInput,
) -> Result<String, TrainerError> {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    if let Some(previous) = previous_skill_id {
        hasher.update(previous.as_bytes());
    }
    hasher.update(context.code_state_key.as_bytes());
    hasher.update(input.named_failure_mode.as_bytes());
    for chunk_id in &context.chunk_ids {
        hasher.update(chunk_id.as_bytes());
    }
    let id = format!(
        "skill_candidate:{prefix}:{}",
        &hex::encode(hasher.finalize())[..16]
    );
    validate_id("candidate_skill_id", &id)?;
    Ok(id)
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn candidate_skill_id(
    context: &AbilityContext,
    input: &AbilityRefutationInput,
) -> Result<String, TrainerError> {
    let mut hasher = Sha256::new();
    hasher.update(context.code_state_key.as_bytes());
    hasher.update([0]);
    for chunk_id in &context.chunk_ids {
        hasher.update(chunk_id.as_bytes());
        hasher.update([0]);
    }
    for label_id in &context.accepted_label_ids {
        hasher.update(label_id.as_bytes());
        hasher.update([0]);
    }
    hasher.update(input.named_failure_mode.as_bytes());
    let id = format!(
        "candidate_ability:{}",
        &hex::encode(hasher.finalize())[..16]
    );
    validate_id("candidate_skill_id", &id)?;
    Ok(id)
}

fn report_rows_agree(
    context: &AbilityContext,
    mistake: &MistakeLogRow,
    replay: &ReplayBufferRow,
    audits: &[SkillLifecycleAuditRow],
) -> bool {
    let labels = &context.accepted_label_ids;
    let skills = &context.active_skill_ids;
    let abilities = &context.active_higher_ability_ids;
    let memberships = &context.source_membership_keys;
    let label_signature_matches = match accepted_label_signature_hash(labels) {
        Ok(expected) => {
            mistake.label_signature_hash == expected
                && replay.label_signature_hash.as_deref() == Some(expected.as_str())
        }
        Err(_) => false,
    };
    mistake.accepted_label_ids == *labels
        && mistake.active_skill_ids == *skills
        && mistake.active_higher_ability_ids == *abilities
        && mistake.source_membership_keys == *memberships
        && label_signature_matches
        && mistake.skill_signature_hash == context.skill_signature_hash
        && mistake.ability_signature_hash == context.ability_signature_hash
        && mistake.membership_signature_hash == context.membership_signature_hash
        && replay.accepted_label_ids == *labels
        && replay.active_skill_ids == *skills
        && replay.active_higher_ability_ids == *abilities
        && replay.source_membership_keys == *memberships
        && replay.skill_signature_hash == context.skill_signature_hash
        && replay.ability_signature_hash == context.ability_signature_hash
        && replay.membership_signature_hash == context.membership_signature_hash
        && audits.iter().all(|row| {
            row.evidence_label_ids == *labels
                && row.evidence_skill_ids == *skills
                && row.evidence_higher_ability_ids == *abilities
                && row.source_membership_keys == *memberships
        })
}

fn validate_refutation_input(input: &AbilityRefutationInput) -> Result<(), TrainerError> {
    if input.prediction_id.0 == [0_u8; 16] {
        return Err(invalid("prediction_id", "must be non-zero"));
    }
    validate_id("panel_signature_hash", &input.panel_signature_hash)?;
    if input.predicted_verdict == input.ground_truth_verdict {
        return Err(invalid(
            "verdicts",
            "refutation requires predicted_verdict != ground_truth_verdict",
        ));
    }
    if !matches!(input.predicted_verdict, Verdict::Pass | Verdict::Fail)
        || !matches!(input.ground_truth_verdict, Verdict::Pass | Verdict::Fail)
    {
        return Err(invalid(
            "verdicts",
            "online mistake refutation requires pass/fail verdicts",
        ));
    }
    for (field, value) in [
        ("language", &input.language),
        ("mutation_or_live_cell", &input.mutation_or_live_cell),
        ("named_failure_mode", &input.named_failure_mode),
    ] {
        validate_id(field, value)?;
    }
    if !input.surprise_z.is_finite() || input.surprise_z < 0.0 {
        return Err(invalid("surprise_z", "must be finite and non-negative"));
    }
    if !input.coverage_gap_score.is_finite() || !(0.0..=1.0).contains(&input.coverage_gap_score) {
        return Err(invalid(
            "coverage_gap_score",
            "must be finite and within [0, 1]",
        ));
    }
    if input.created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }
    Ok(())
}

fn validate_label_context_matches(
    context: &AbilityContext,
    label_context: &PredictionLabelContext,
) -> Result<(), TrainerError> {
    label_context.validate().map_err(|err| {
        TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, err.to_string())
            .with_context(json!({"field": "label_context", "file": super::SOURCE_FILE}))
    })?;
    if label_context.code_state_key.as_deref() != Some(context.code_state_key.as_str())
        || label_context.accepted_label_ids != context.accepted_label_ids
        || label_context.active_skill_ids != context.active_skill_ids
        || label_context.active_higher_ability_ids != context.active_higher_ability_ids
        || label_context.source_membership_keys != context.source_membership_keys
    {
        return Err(invalid(
            "label_context",
            "PredictionLabelContext must match resolved AbilityContext",
        ));
    }
    let expected_label_signature = accepted_label_signature_hash(&context.accepted_label_ids)?;
    if label_context.label_signature_hash.as_deref() != Some(expected_label_signature.as_str())
        || label_context.skill_signature_hash != context.skill_signature_hash
        || label_context.ability_signature_hash != context.ability_signature_hash
        || label_context.membership_signature_hash != context.membership_signature_hash
    {
        return Err(invalid(
            "label_context",
            "PredictionLabelContext signatures must match resolved AbilityContext",
        ));
    }
    Ok(())
}
