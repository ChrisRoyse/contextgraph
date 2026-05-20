use crate::chunk_skill_membership::{
    lifecycle_audit_id_from_parts, write_skill_lifecycle_audit_row_sync_readback,
    SkillLifecycleAuditRow, SkillLifecycleDecision,
};
use crate::error::{TrainerError, TrainerErrorCode};
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
use crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION;
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
    let lifecycle_audits = lifecycle_decisions_for_refutation(context, &mistake_row, &input)?;
    for audit in &lifecycle_audits {
        write_skill_lifecycle_audit_row_sync_readback(db, audit)?;
    }
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
        && lifecycle_audits
            .iter()
            .any(|row| row.decision == SkillLifecycleDecision::CreateNewCandidateSkill);
    let report = AbilityRefutationReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        label_skill_ability_membership_ids_agree: report_rows_agree(
            context,
            &mistake_row,
            &replay_row,
            &lifecycle_audits,
        ),
        mistake_row,
        replay_row,
        online_head_update_report,
        lifecycle_audits,
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

fn lifecycle_decisions_for_refutation(
    context: &AbilityContext,
    mistake: &MistakeLogRow,
    input: &AbilityRefutationInput,
) -> Result<Vec<SkillLifecycleAuditRow>, TrainerError> {
    let mut rows = Vec::new();
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
        let decision = if existing {
            SkillLifecycleDecision::UpdateExistingSkill
        } else {
            SkillLifecycleDecision::CreateNewCandidateSkill
        };
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
            candidate_skill_id: (!existing).then_some(skill_or_ability_id.clone()),
            evidence_label_ids: context.accepted_label_ids.clone(),
            evidence_skill_ids: context.active_skill_ids.clone(),
            evidence_higher_ability_ids: context.active_higher_ability_ids.clone(),
            evidence_chunk_ids: context.chunk_ids.clone(),
            source_membership_keys: context.source_membership_keys.clone(),
            reason: if existing {
                "reality_refuted_prediction_with_active_ability_evidence".to_string()
            } else {
                "reality_refuted_prediction_without_safe_existing_ability".to_string()
            },
            created_at_unix_ms: input.created_at_unix_ms + idx as i64,
        };
        audit.validate()?;
        rows.push(audit);
    }
    Ok(rows)
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
    mistake.accepted_label_ids == *labels
        && mistake.active_skill_ids == *skills
        && mistake.active_higher_ability_ids == *abilities
        && mistake.source_membership_keys == *memberships
        && replay.accepted_label_ids == *labels
        && replay.active_skill_ids == *skills
        && replay.active_higher_ability_ids == *abilities
        && replay.source_membership_keys == *memberships
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
    Ok(())
}
