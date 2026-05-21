use std::collections::{BTreeMap, BTreeSet};

use rocksdb::DB;
use serde::{Deserialize, Serialize};

use crate::chunk_skill_membership::{
    read_all_level2_skill_rows, read_all_skill_lifecycle_audit_rows, ChunkSkillMembershipRow,
    SkillLifecycleAuditRow, SkillLifecycleDecision,
};
use crate::error::TrainerError;
use crate::skill_sequence_discovery::{
    is_target_only_live_label, skill_candidate_kind_for_row, skill_genericity_score_for_steps,
    Level2SkillRow, SkillCandidateKind, SkillOutcomeDistribution, SkillPromotionStatus,
    SKILL_SEQUENCE_SCHEMA_VERSION,
};

use super::{
    chunk_from_membership, invalid, scan_memberships, skill_linkage_cfs, validate_id_list,
    validate_limit, ChunkSourceIndex, SkillCodeChunk,
};

const DEFAULT_EXAMPLE_LIMIT: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillUsefulnessAuditOptions {
    pub limit: usize,
    pub representative_example_limit: usize,
    pub expected_code_state_keys: Vec<String>,
    pub unknown_ordered_constellation_ids: Vec<String>,
    pub rejected_patterns: Vec<SkillAuditRejectedPattern>,
    pub source_fsv_artifact: Option<String>,
}

impl Default for SkillUsefulnessAuditOptions {
    fn default() -> Self {
        Self {
            limit: 100,
            representative_example_limit: DEFAULT_EXAMPLE_LIMIT,
            expected_code_state_keys: Vec::new(),
            unknown_ordered_constellation_ids: Vec::new(),
            rejected_patterns: Vec::new(),
            source_fsv_artifact: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditRejectedPattern {
    pub pattern_id: String,
    pub candidate_kind: SkillCandidateKind,
    pub reason: String,
    pub support: u64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub metric_before: Option<f64>,
    pub metric_after: Option<f64>,
    pub fsv_artifact: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditSkillBucketCounts {
    pub failure_skill: u64,
    pub pass_stability_skill: u64,
    pub context_negative_evidence: u64,
    pub neutral_diagnostic: u64,
    pub reject_overbroad_or_leaky: u64,
    pub demoted: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditLifecycleSummary {
    pub audit_id: String,
    pub decision: SkillLifecycleDecision,
    pub reason: String,
    pub prediction_id: Option<String>,
    pub mistake_id: Option<String>,
    pub candidate_skill_id: Option<String>,
    pub evidence_chunk_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditRepresentativeSequence {
    pub membership_key: String,
    pub chunk_id: String,
    pub file_path: String,
    pub code_state_key: String,
    pub live_input_label_ids: Vec<String>,
    pub ordered_group_ids: Vec<String>,
    pub source: SkillCodeChunk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditRow {
    pub skill_id: String,
    pub skill_name: String,
    pub candidate_kind: SkillCandidateKind,
    pub promotion_status: SkillPromotionStatus,
    pub support: u64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub genericity_score: f64,
    pub calibration_impact_proxy: f64,
    pub calibration_impact_source: String,
    pub per_cell_contribution_count: u64,
    pub membership_count: u64,
    pub distinct_chunk_count: u64,
    pub live_input_label_ids: Vec<String>,
    pub target_supervision_label_ids: Vec<String>,
    pub representative_sequences: Vec<SkillAuditRepresentativeSequence>,
    pub lifecycle_history: Vec<SkillAuditLifecycleSummary>,
    pub source_fsv_artifact: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillAuditGapView {
    pub expected_code_state_count: u64,
    pub covered_code_state_count: u64,
    pub uncovered_code_state_count: u64,
    pub uncovered_code_state_keys: Vec<String>,
    pub unknown_ordered_constellation_ids: Vec<String>,
    pub generic_high_support_rejections: Vec<SkillAuditRejectedPattern>,
    pub metric_regression_rejections: Vec<SkillAuditRejectedPattern>,
    pub demoted_or_regressing_skill_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillUsefulnessAuditReport {
    pub schema_version: u32,
    pub skill_rows: Vec<SkillAuditRow>,
    pub persisted_skill_count: u64,
    pub persisted_membership_count: u64,
    pub persisted_lifecycle_audit_count: u64,
    pub returned_skill_limit: usize,
    pub bucket_counts: SkillAuditSkillBucketCounts,
    pub lifecycle_decision_counts: BTreeMap<String, u64>,
    pub gap_view: SkillAuditGapView,
    pub audit_buckets_sum_to_persisted_skill_count: bool,
    pub no_new_prediction_head_introduced: bool,
    pub target_outcomes_used_as_live_inputs: bool,
    pub hidden_intent_inference_used: bool,
    pub flat_vector_concat_used: bool,
    pub source_of_truth_cfs: Vec<String>,
}

pub fn skill_usefulness_audit(
    db: &DB,
    source_index: Option<&ChunkSourceIndex>,
    options: SkillUsefulnessAuditOptions,
) -> Result<SkillUsefulnessAuditReport, TrainerError> {
    validate_options(&options)?;
    let skills = read_all_level2_skill_rows(db)?;
    let memberships = scan_memberships(db)?;
    let lifecycle_rows = read_all_skill_lifecycle_audit_rows(db)?;
    let persisted_lifecycle_audit_count = lifecycle_rows.len() as u64;

    let mut membership_by_skill = BTreeMap::<String, Vec<ChunkSkillMembershipRow>>::new();
    let mut code_state_coverage = BTreeSet::<String>::new();
    for row in &memberships {
        row.validate()?;
        membership_by_skill
            .entry(row.skill_id.clone())
            .or_default()
            .push(row.clone());
        code_state_coverage.insert(row.code_state_key.clone());
    }

    let mut lifecycle_by_skill = BTreeMap::<String, Vec<SkillLifecycleAuditRow>>::new();
    let mut lifecycle_decision_counts = BTreeMap::<String, u64>::new();
    for row in lifecycle_rows {
        row.validate()?;
        *lifecycle_decision_counts
            .entry(decision_label(row.decision).to_string())
            .or_default() += 1;
        for skill_id in lifecycle_skill_ids(&row) {
            lifecycle_by_skill
                .entry(skill_id)
                .or_default()
                .push(row.clone());
        }
    }

    let mut bucket_counts = SkillAuditSkillBucketCounts::default();
    let mut audit_rows = Vec::new();
    let mut demoted_or_regressing_skill_ids = BTreeSet::new();
    for skill in &skills {
        observe_bucket(&mut bucket_counts, skill);
        if skill.promotion_status == SkillPromotionStatus::Demoted {
            demoted_or_regressing_skill_ids.insert(skill.skill_id.clone());
        }
        let rows = membership_by_skill
            .get(&skill.skill_id)
            .cloned()
            .unwrap_or_default();
        audit_rows.push(row_for_skill(
            skill,
            rows,
            lifecycle_by_skill
                .get(&skill.skill_id)
                .cloned()
                .unwrap_or_default(),
            source_index,
            &options,
        )?);
    }

    audit_rows.sort_by(|left, right| {
        right
            .calibration_impact_proxy
            .total_cmp(&left.calibration_impact_proxy)
            .then_with(|| right.support.cmp(&left.support))
            .then_with(|| left.skill_id.cmp(&right.skill_id))
    });
    audit_rows.truncate(options.limit);

    let expected_code_state_key_set = options
        .expected_code_state_keys
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let uncovered_code_state_keys =
        uncovered_keys(&options.expected_code_state_keys, &code_state_coverage);
    let generic_high_support_rejections = options
        .rejected_patterns
        .iter()
        .filter(|row| is_generic_rejection(row))
        .cloned()
        .collect::<Vec<_>>();
    let metric_regression_rejections = options
        .rejected_patterns
        .iter()
        .filter(|row| is_metric_regression(row))
        .cloned()
        .collect::<Vec<_>>();
    for row in &metric_regression_rejections {
        demoted_or_regressing_skill_ids.insert(row.pattern_id.clone());
    }

    let bucket_sum = bucket_counts.failure_skill
        + bucket_counts.pass_stability_skill
        + bucket_counts.context_negative_evidence
        + bucket_counts.neutral_diagnostic
        + bucket_counts.reject_overbroad_or_leaky;
    let audit_buckets_sum_to_persisted_skill_count = bucket_sum == skills.len() as u64;

    Ok(SkillUsefulnessAuditReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_rows: audit_rows,
        persisted_skill_count: skills.len() as u64,
        persisted_membership_count: memberships.len() as u64,
        persisted_lifecycle_audit_count,
        returned_skill_limit: options.limit,
        bucket_counts,
        lifecycle_decision_counts,
        gap_view: SkillAuditGapView {
            expected_code_state_count: options.expected_code_state_keys.len() as u64,
            covered_code_state_count: code_state_coverage
                .intersection(&expected_code_state_key_set)
                .count() as u64,
            uncovered_code_state_count: uncovered_code_state_keys.len() as u64,
            uncovered_code_state_keys,
            unknown_ordered_constellation_ids: options.unknown_ordered_constellation_ids,
            generic_high_support_rejections,
            metric_regression_rejections,
            demoted_or_regressing_skill_ids: demoted_or_regressing_skill_ids.into_iter().collect(),
        },
        audit_buckets_sum_to_persisted_skill_count,
        no_new_prediction_head_introduced: true,
        target_outcomes_used_as_live_inputs: false,
        hidden_intent_inference_used: false,
        flat_vector_concat_used: false,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

fn row_for_skill(
    skill: &Level2SkillRow,
    memberships: Vec<ChunkSkillMembershipRow>,
    lifecycle_rows: Vec<SkillLifecycleAuditRow>,
    source_index: Option<&ChunkSourceIndex>,
    options: &SkillUsefulnessAuditOptions,
) -> Result<SkillAuditRow, TrainerError> {
    let distinct_chunk_count = memberships
        .iter()
        .map(|row| row.chunk_id.clone())
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let membership_count = memberships.len() as u64;
    let mut live_labels = BTreeSet::<String>::new();
    for label in &skill.prerequisite_label_ids {
        live_labels.insert(label.clone());
    }
    for step in &skill.ordered_steps {
        live_labels.extend(step.accepted_label_ids.iter().cloned());
        live_labels.extend(step.group_ids.iter().cloned());
    }
    for row in &memberships {
        live_labels.extend(row.source_accepted_label_ids.iter().cloned());
        for step in &row.ordered_step_evidence {
            live_labels.extend(step.accepted_label_ids.iter().cloned());
            live_labels.extend(step.group_ids.iter().cloned());
        }
    }
    if live_labels
        .iter()
        .any(|label| is_target_only_live_label(label))
    {
        return Err(invalid(
            "live_input_label_ids",
            format!("skill {} carries target-only live label", skill.skill_id),
        ));
    }

    let mut ordered_memberships = memberships;
    ordered_memberships.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.code_state_key.cmp(&right.code_state_key))
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
            .then_with(|| left.membership_key.cmp(&right.membership_key))
    });
    let representative_sequences = ordered_memberships
        .into_iter()
        .take(options.representative_example_limit)
        .map(|row| representative_sequence(row, source_index))
        .collect::<Result<Vec<_>, _>>()?;

    let lifecycle_history = lifecycle_rows
        .into_iter()
        .map(lifecycle_summary)
        .collect::<Vec<_>>();
    let calibration_impact_proxy = skill.confidence * skill.lift_over_cell_baseline.max(0.0);

    Ok(SkillAuditRow {
        skill_id: skill.skill_id.clone(),
        skill_name: skill.skill_name.clone(),
        candidate_kind: skill_candidate_kind_for_row(skill),
        promotion_status: skill.promotion_status,
        support: skill.support,
        oracle_outcome_distribution: skill.oracle_outcome_distribution,
        confidence: skill.confidence,
        lift_over_cell_baseline: skill.lift_over_cell_baseline,
        stability: skill.stability,
        genericity_score: skill_genericity_score_for_steps(&skill.ordered_steps),
        calibration_impact_proxy,
        calibration_impact_source:
            "confidence_times_positive_lift_proxy_until_per_cell_calibration_rows_are_attached"
                .to_string(),
        per_cell_contribution_count: skill.code_state_keys.len() as u64,
        membership_count,
        distinct_chunk_count,
        live_input_label_ids: live_labels.into_iter().collect(),
        target_supervision_label_ids: target_supervision_labels(skill.oracle_outcome_distribution),
        representative_sequences,
        lifecycle_history,
        source_fsv_artifact: options.source_fsv_artifact.clone(),
    })
}

fn representative_sequence(
    row: ChunkSkillMembershipRow,
    source_index: Option<&ChunkSourceIndex>,
) -> Result<SkillAuditRepresentativeSequence, TrainerError> {
    let live_input_label_ids = row
        .ordered_step_evidence
        .iter()
        .flat_map(|step| step.accepted_label_ids.iter().cloned())
        .chain(row.source_accepted_label_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let ordered_group_ids = row
        .ordered_step_evidence
        .iter()
        .flat_map(|step| step.group_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let source = chunk_from_membership(row.clone(), source_index, false)?;
    Ok(SkillAuditRepresentativeSequence {
        membership_key: row.membership_key,
        chunk_id: row.chunk_id,
        file_path: row.file_path,
        code_state_key: row.code_state_key,
        live_input_label_ids,
        ordered_group_ids,
        source,
    })
}

fn lifecycle_summary(row: SkillLifecycleAuditRow) -> SkillAuditLifecycleSummary {
    SkillAuditLifecycleSummary {
        audit_id: row.skill_audit_id,
        decision: row.decision,
        reason: row.reason,
        prediction_id: row.prediction_id,
        mistake_id: row.mistake_id,
        candidate_skill_id: row.candidate_skill_id,
        evidence_chunk_ids: row.evidence_chunk_ids,
        source_membership_keys: row.source_membership_keys,
        created_at_unix_ms: row.created_at_unix_ms,
    }
}

fn lifecycle_skill_ids(row: &SkillLifecycleAuditRow) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    if let Some(skill_id) = &row.previous_skill_id {
        ids.insert(skill_id.clone());
    }
    if let Some(skill_id) = &row.candidate_skill_id {
        ids.insert(skill_id.clone());
    }
    ids.extend(row.evidence_skill_ids.iter().cloned());
    ids
}

fn observe_bucket(counts: &mut SkillAuditSkillBucketCounts, skill: &Level2SkillRow) {
    if skill.promotion_status == SkillPromotionStatus::Demoted {
        counts.demoted += 1;
    }
    match skill_candidate_kind_for_row(skill) {
        SkillCandidateKind::FailureSkill => counts.failure_skill += 1,
        SkillCandidateKind::PassStabilitySkill => counts.pass_stability_skill += 1,
        SkillCandidateKind::ContextNegativeEvidence => counts.context_negative_evidence += 1,
        SkillCandidateKind::NeutralDiagnostic => counts.neutral_diagnostic += 1,
        SkillCandidateKind::RejectOverbroadOrLeaky => counts.reject_overbroad_or_leaky += 1,
    }
}

fn uncovered_keys(expected: &[String], covered: &BTreeSet<String>) -> Vec<String> {
    expected
        .iter()
        .filter(|key| !covered.contains(*key))
        .cloned()
        .collect()
}

fn target_supervision_labels(distribution: SkillOutcomeDistribution) -> Vec<String> {
    let mut labels = Vec::new();
    if distribution.pass > 0 {
        labels.push(format!("oracle:pass:{}", distribution.pass));
    }
    if distribution.fail > 0 {
        labels.push(format!("oracle:fail:{}", distribution.fail));
    }
    if distribution.unknown > 0 {
        labels.push(format!("oracle:unknown:{}", distribution.unknown));
    }
    labels
}

fn is_generic_rejection(row: &SkillAuditRejectedPattern) -> bool {
    row.candidate_kind == SkillCandidateKind::RejectOverbroadOrLeaky
        || row.reason.contains("overbroad")
        || row.reason.contains("blanket")
        || row.reason.contains("generic")
}

fn is_metric_regression(row: &SkillAuditRejectedPattern) -> bool {
    row.metric_before
        .zip(row.metric_after)
        .is_some_and(|(before, after)| after < before)
        || row.reason.contains("regress")
}

fn decision_label(decision: SkillLifecycleDecision) -> &'static str {
    match decision {
        SkillLifecycleDecision::UpdateExistingSkill => "update_existing_skill",
        SkillLifecycleDecision::SplitMixedSkill => "split_mixed_skill",
        SkillLifecycleDecision::CreateNewCandidateSkill => "create_new_candidate_skill",
        SkillLifecycleDecision::DemoteUnstableSkill => "demote_unstable_skill",
        SkillLifecycleDecision::NoChangeWithEvidence => "no_change_with_evidence",
    }
}

fn validate_options(options: &SkillUsefulnessAuditOptions) -> Result<(), TrainerError> {
    validate_limit(options.limit)?;
    validate_limit(options.representative_example_limit)?;
    validate_id_list(
        "expected_code_state_keys",
        &options.expected_code_state_keys,
        usize::MAX,
    )?;
    validate_id_list(
        "unknown_ordered_constellation_ids",
        &options.unknown_ordered_constellation_ids,
        usize::MAX,
    )?;
    for row in &options.rejected_patterns {
        if row.pattern_id.trim().is_empty() || row.reason.trim().is_empty() {
            return Err(invalid(
                "rejected_patterns",
                "pattern_id and reason must be non-empty",
            ));
        }
        if let Some(value) = row.metric_before {
            if !value.is_finite() {
                return Err(invalid("metric_before", "must be finite"));
            }
        }
        if let Some(value) = row.metric_after {
            if !value.is_finite() {
                return Err(invalid("metric_after", "must be finite"));
            }
        }
    }
    Ok(())
}
