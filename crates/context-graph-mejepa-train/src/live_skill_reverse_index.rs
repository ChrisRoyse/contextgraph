//! TASK-SKILL-003 live chunk constellation -> ordered skill reverse index.
//!
//! The live path receives already-labeled chunk constellations, matches them
//! against ordered Level-2 skill templates, persists the derived many-to-many
//! chunk memberships with RocksDB sync readback, then delegates prediction-time
//! skill aggregation to the runtime ability resolver.

use crate::ability_resolver::{
    resolve_ability_context, AbilityContext, AbilityResolverRequest, LiveChunkInput,
};
use crate::chunk_skill_membership::{
    lifecycle_audit_id_from_parts, membership_key, read_all_level2_skill_rows, reverse_index_key,
    write_skill_materialization_sync_readback, ChunkSkillMembershipRow, OrderedStepSpan,
    SkillLifecycleAuditRow, SkillLifecycleDecision, SkillMaterialization, SkillReverseIndexRow,
};
use crate::error::TrainerError;
use crate::skill_sequence_discovery::{Level2SkillRow, SkillPromotionStatus};
use crate::skill_sequence_types::{SkillStepEvidence, SkillStepTemplate};
use crate::skill_validation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_LIVE_CHUNKS: usize = 4096;
const MAX_IDS: usize = 4096;
const SOURCE_FILE: &str = "file:crates/context-graph-mejepa-train/src/live_skill_reverse_index.rs";
const REMEDIATION: &str =
    "live skill reverse indexing must preserve ordered chunk evidence and reject target-only labels";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveSkillChunkInput {
    pub chunk_id: String,
    pub file_path: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub sequence_index: u32,
    pub accepted_label_ids: Vec<String>,
    pub group_ids: Vec<String>,
}

impl LiveSkillChunkInput {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("chunk_id", &self.chunk_id)?;
        validate_project_relative_path("file_path", &self.file_path)?;
        if self.byte_end < self.byte_start {
            return Err(invalid("byte_span", "byte_end must be >= byte_start"));
        }
        validate_live_id_list("accepted_label_ids", &self.accepted_label_ids, MAX_IDS)?;
        validate_live_id_list("group_ids", &self.group_ids, MAX_IDS)?;
        if self.accepted_label_ids.is_empty() {
            return Err(invalid(
                "accepted_label_ids",
                "live chunk needs at least one accepted label",
            ));
        }
        Ok(())
    }

    fn labels_contain_all(&self, expected: &[String]) -> bool {
        contains_all(&self.accepted_label_ids, expected)
    }

    fn groups_contain_all(&self, expected: &[String]) -> bool {
        contains_all(&self.group_ids, expected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveSkillReverseIndexRequest {
    pub code_state_key: String,
    pub chunks: Vec<LiveSkillChunkInput>,
    pub created_at_unix_ms: i64,
    pub emit_partial_candidates: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveSkillMatch {
    pub skill_id: String,
    pub skill_name: String,
    pub matched_chunk_ids: Vec<String>,
    pub matched_step_indices: Vec<u32>,
    pub membership_keys: Vec<String>,
    pub reverse_index_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveSkillPartialCandidate {
    pub candidate_skill_id: String,
    pub previous_skill_id: Option<String>,
    pub matched_chunk_ids: Vec<String>,
    pub matched_step_indices: Vec<u32>,
    pub missing_step_indices: Vec<u32>,
    pub evidence_label_ids: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LiveSkillReverseIndexReport {
    pub schema_version: u32,
    pub code_state_key: String,
    pub matched_skills: Vec<LiveSkillMatch>,
    pub partial_candidates: Vec<LiveSkillPartialCandidate>,
    pub materialization: SkillMaterialization,
    pub ability_context: AbilityContext,
    pub live_input_allowed: bool,
    pub no_new_prediction_head_introduced: bool,
    pub hidden_intent_inference_used: bool,
    pub target_side_labels_used_as_live_inputs: bool,
    pub flat_vector_concat_used: bool,
}

impl LiveSkillReverseIndexReport {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("code_state_key", &self.code_state_key)?;
        for item in &self.matched_skills {
            validate_id("matched.skill_id", &item.skill_id)?;
            validate_id("matched.skill_name", &item.skill_name)?;
            validate_id_list(
                "matched.matched_chunk_ids",
                &item.matched_chunk_ids,
                MAX_IDS,
            )?;
            validate_id_list("matched.membership_keys", &item.membership_keys, MAX_IDS)?;
            validate_id("matched.reverse_index_key", &item.reverse_index_key)?;
            if item.matched_chunk_ids.is_empty()
                || item.matched_step_indices.len() != item.matched_chunk_ids.len()
            {
                return Err(invalid(
                    "matched_skills",
                    "matched skills must cite aligned chunk and step evidence",
                ));
            }
        }
        for item in &self.partial_candidates {
            validate_id("partial.candidate_skill_id", &item.candidate_skill_id)?;
            if let Some(skill_id) = &item.previous_skill_id {
                validate_id("partial.previous_skill_id", skill_id)?;
            }
            validate_id_list(
                "partial.matched_chunk_ids",
                &item.matched_chunk_ids,
                MAX_IDS,
            )?;
            validate_live_id_list(
                "partial.evidence_label_ids",
                &item.evidence_label_ids,
                MAX_IDS,
            )?;
            validate_id("partial.reason", &item.reason)?;
        }
        self.ability_context.validate()?;
        if !self.live_input_allowed
            || !self.no_new_prediction_head_introduced
            || self.hidden_intent_inference_used
            || self.target_side_labels_used_as_live_inputs
            || self.flat_vector_concat_used
        {
            return Err(invalid(
                "runtime_policy",
                "live reverse index report must be live-safe and slot-preserving",
            ));
        }
        Ok(())
    }
}

pub fn reverse_index_live_skills_sync_readback(
    db: &rocksdb::DB,
    request: LiveSkillReverseIndexRequest,
) -> Result<LiveSkillReverseIndexReport, TrainerError> {
    validate_request(&request)?;
    let mut skills = read_all_level2_skill_rows(db)?
        .into_iter()
        .filter(skill_is_live_matchable)
        .collect::<Vec<_>>();
    skills.sort_by(|left, right| {
        skill_sort_key(left)
            .cmp(&skill_sort_key(right))
            .then_with(|| left.skill_id.cmp(&right.skill_id))
    });

    let mut materialization = SkillMaterialization {
        level2_skills: Vec::new(),
        chunk_memberships: Vec::new(),
        reverse_indexes: Vec::new(),
        lifecycle_audits: Vec::new(),
    };
    let mut matches = Vec::new();
    let mut partials = Vec::new();

    for (skill_offset, skill) in skills.iter().enumerate() {
        match match_skill(skill, &request)? {
            SkillMatchOutcome::Full(steps) => {
                let rows = membership_rows_for_match(
                    skill,
                    &request.code_state_key,
                    request.created_at_unix_ms,
                    &steps,
                )?;
                let reverse =
                    reverse_index_row_for_match(skill, &request.code_state_key, &steps, &rows)?;
                let membership_keys = rows
                    .iter()
                    .map(|row| row.membership_key.clone())
                    .collect::<Vec<_>>();
                let matched_chunk_ids = steps
                    .iter()
                    .map(|step| step.chunk_id.clone())
                    .collect::<Vec<_>>();
                let matched_step_indices =
                    steps.iter().map(|step| step.step_index).collect::<Vec<_>>();
                let audit = full_match_audit(
                    skill,
                    &request.code_state_key,
                    request.created_at_unix_ms + skill_offset as i64,
                    &steps,
                    &membership_keys,
                )?;
                matches.push(LiveSkillMatch {
                    skill_id: skill.skill_id.clone(),
                    skill_name: skill.skill_name.clone(),
                    matched_chunk_ids,
                    matched_step_indices,
                    membership_keys,
                    reverse_index_key: reverse.reverse_index_key.clone(),
                });
                materialization.chunk_memberships.extend(rows);
                materialization.reverse_indexes.push(reverse);
                materialization.lifecycle_audits.push(audit);
            }
            SkillMatchOutcome::Partial(partial) => {
                if request.emit_partial_candidates && !partial.matched_steps.is_empty() {
                    let candidate = partial_candidate_for_match(
                        skill,
                        &request.code_state_key,
                        request.created_at_unix_ms + skill_offset as i64,
                        partial,
                    )?;
                    let audit = partial_candidate_audit(
                        &candidate,
                        request.created_at_unix_ms + skill_offset as i64,
                    )?;
                    materialization.lifecycle_audits.push(audit);
                    partials.push(candidate);
                }
            }
            SkillMatchOutcome::None => {}
        }
    }

    write_skill_materialization_sync_readback(db, &materialization)?;
    let ability_context = resolve_ability_context(db, ability_request_from_live(&request))?;
    let report = LiveSkillReverseIndexReport {
        schema_version: 1,
        code_state_key: request.code_state_key,
        matched_skills: matches,
        partial_candidates: partials,
        materialization,
        ability_context,
        live_input_allowed: true,
        no_new_prediction_head_introduced: true,
        hidden_intent_inference_used: false,
        target_side_labels_used_as_live_inputs: false,
        flat_vector_concat_used: false,
    };
    report.validate()?;
    Ok(report)
}

#[derive(Debug, Clone)]
enum SkillMatchOutcome {
    Full(Vec<SkillStepEvidence>),
    Partial(PartialTemplateMatch),
    None,
}

#[derive(Debug, Clone)]
struct PartialTemplateMatch {
    matched_steps: Vec<SkillStepEvidence>,
    missing_step_indices: Vec<u32>,
    reason: String,
}

fn validate_request(request: &LiveSkillReverseIndexRequest) -> Result<(), TrainerError> {
    validate_id("code_state_key", &request.code_state_key)?;
    if request.created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }
    if request.chunks.is_empty() || request.chunks.len() > MAX_LIVE_CHUNKS {
        return Err(invalid("chunks", "must contain 1..4096 live chunks"));
    }
    let mut seen_chunks = BTreeSet::new();
    let mut previous_sequence = None;
    let mut previous_canonical_order = None;
    let mut file_last_end = BTreeMap::<String, u64>::new();
    for chunk in &request.chunks {
        chunk.validate()?;
        if !seen_chunks.insert(chunk.chunk_id.clone()) {
            return Err(invalid("chunks", "duplicate chunk_id in live request"));
        }
        let canonical_order = (
            chunk.file_path.clone(),
            chunk.byte_start,
            chunk.byte_end,
            chunk.sequence_index,
        );
        if let Some(previous) = &previous_canonical_order {
            if canonical_order <= *previous {
                return Err(invalid(
                    "chunk_order",
                    "live chunks must be ordered by file_path, byte span, then sequence_index",
                ));
            }
        }
        previous_canonical_order = Some(canonical_order);
        if let Some(previous) = previous_sequence {
            if chunk.sequence_index <= previous {
                return Err(invalid(
                    "sequence_index",
                    "live chunks must be strictly ordered by sequence_index",
                ));
            }
        }
        previous_sequence = Some(chunk.sequence_index);
        if let Some(previous_end) = file_last_end.insert(chunk.file_path.clone(), chunk.byte_end) {
            if chunk.byte_start < previous_end {
                return Err(invalid(
                    "byte_span",
                    "chunks in the same file must be presented in non-overlapping byte order",
                ));
            }
        }
    }
    Ok(())
}

fn skill_is_live_matchable(skill: &Level2SkillRow) -> bool {
    skill.live_input_allowed
        && !matches!(skill.promotion_status, SkillPromotionStatus::Demoted)
        && skill.operator_approved
}

fn skill_sort_key(skill: &Level2SkillRow) -> (usize, u64, String) {
    (
        skill.ordered_steps.len(),
        std::cmp::Reverse(skill.support).0,
        skill.skill_name.clone(),
    )
}

fn match_skill(
    skill: &Level2SkillRow,
    request: &LiveSkillReverseIndexRequest,
) -> Result<SkillMatchOutcome, TrainerError> {
    skill.validate()?;
    if skill.ordered_steps.is_empty() {
        return Ok(SkillMatchOutcome::None);
    }
    let mut cursor = 0_usize;
    let mut matched_steps = Vec::new();
    let mut missing_step_indices = Vec::new();

    for template in &skill.ordered_steps {
        let Some((position, chunk)) = find_next_matching_chunk(&request.chunks, cursor, template)
        else {
            missing_step_indices.push(template.step_index);
            continue;
        };
        matched_steps.push(step_evidence_from_chunk(
            template.step_index,
            &request.code_state_key,
            chunk,
        ));
        cursor = position + 1;
    }

    if matched_steps.is_empty() {
        return Ok(SkillMatchOutcome::None);
    }
    let prerequisite_ids = matched_steps
        .iter()
        .flat_map(|step| step.accepted_label_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let missing_prereqs = skill
        .prerequisite_label_ids
        .iter()
        .filter(|label| !prerequisite_ids.contains(*label))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_prereqs.is_empty() {
        let mut indices = missing_step_indices;
        indices.extend(0..skill.ordered_steps.len() as u32);
        indices.sort();
        indices.dedup();
        return Ok(SkillMatchOutcome::Partial(PartialTemplateMatch {
            matched_steps,
            missing_step_indices: indices,
            reason: "partial_live_ordered_skill_match_missing_prerequisite".to_string(),
        }));
    }
    if !missing_step_indices.is_empty() {
        return Ok(SkillMatchOutcome::Partial(PartialTemplateMatch {
            matched_steps,
            missing_step_indices,
            reason: "partial_live_ordered_skill_match_missing_required_step".to_string(),
        }));
    }
    let transition_edges_pass = transition_edges_satisfied(skill, &matched_steps);
    if !transition_edges_pass {
        return Ok(SkillMatchOutcome::Partial(PartialTemplateMatch {
            matched_steps,
            missing_step_indices,
            reason: "partial_live_ordered_skill_match_transition_gap".to_string(),
        }));
    }
    Ok(SkillMatchOutcome::Full(matched_steps))
}

fn find_next_matching_chunk<'a>(
    chunks: &'a [LiveSkillChunkInput],
    start: usize,
    template: &SkillStepTemplate,
) -> Option<(usize, &'a LiveSkillChunkInput)> {
    chunks.iter().enumerate().skip(start).find(|(_, chunk)| {
        chunk.labels_contain_all(&template.accepted_label_ids)
            && chunk.groups_contain_all(&template.group_ids)
    })
}

fn transition_edges_satisfied(skill: &Level2SkillRow, steps: &[SkillStepEvidence]) -> bool {
    let positions = steps
        .iter()
        .enumerate()
        .map(|(position, step)| (step.step_index, position))
        .collect::<BTreeMap<_, _>>();
    skill.transition_edges.iter().all(|edge| {
        match (
            positions.get(&edge.from_step_index),
            positions.get(&edge.to_step_index),
        ) {
            (Some(from), Some(to)) => from < to,
            _ => false,
        }
    })
}

fn step_evidence_from_chunk(
    step_index: u32,
    code_state_key: &str,
    chunk: &LiveSkillChunkInput,
) -> SkillStepEvidence {
    let mut accepted_label_ids = chunk.accepted_label_ids.clone();
    accepted_label_ids.sort();
    let mut group_ids = chunk.group_ids.clone();
    group_ids.sort();
    SkillStepEvidence {
        step_index,
        chunk_id: chunk.chunk_id.clone(),
        file_path: chunk.file_path.clone(),
        code_state_key: code_state_key.to_string(),
        accepted_label_ids,
        group_ids,
    }
}

fn membership_rows_for_match(
    skill: &Level2SkillRow,
    code_state_key: &str,
    created_at_unix_ms: i64,
    steps: &[SkillStepEvidence],
) -> Result<Vec<ChunkSkillMembershipRow>, TrainerError> {
    let mut rows = Vec::with_capacity(steps.len());
    let provenance = live_match_hash(&skill.skill_id, code_state_key, steps);
    for step in steps {
        let key = membership_key(&step.chunk_id, code_state_key, &skill.skill_id)?;
        let row = ChunkSkillMembershipRow {
            schema_version: crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION,
            membership_key: key,
            chunk_id: step.chunk_id.clone(),
            file_path: step.file_path.clone(),
            code_state_key: code_state_key.to_string(),
            skill_id: skill.skill_id.clone(),
            hierarchy_level: 2,
            membership_score: (skill.confidence * skill.stability).clamp(0.0, 1.0),
            source_accepted_label_ids: step.accepted_label_ids.clone(),
            ordered_step_evidence: vec![step.clone()],
            live_input_allowed: true,
            provenance_hashes: vec![provenance.clone()],
            first_seen_unix_ms: created_at_unix_ms,
            last_seen_unix_ms: created_at_unix_ms,
        };
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn reverse_index_row_for_match(
    skill: &Level2SkillRow,
    code_state_key: &str,
    steps: &[SkillStepEvidence],
    rows: &[ChunkSkillMembershipRow],
) -> Result<SkillReverseIndexRow, TrainerError> {
    let reverse_index_key = reverse_index_key(&skill.skill_id, code_state_key)?;
    let chunk_ids = steps
        .iter()
        .map(|step| step.chunk_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let file_paths = steps
        .iter()
        .map(|step| step.file_path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let ordered_step_spans = steps
        .iter()
        .map(|step| OrderedStepSpan {
            step_index: step.step_index,
            chunk_id: step.chunk_id.clone(),
            file_path: step.file_path.clone(),
            code_state_key: code_state_key.to_string(),
        })
        .collect::<Vec<_>>();
    let row = SkillReverseIndexRow {
        schema_version: crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION,
        reverse_index_key,
        skill_id: skill.skill_id.clone(),
        code_state_key: code_state_key.to_string(),
        chunk_ids,
        file_paths,
        ordered_step_spans,
        support: steps.len() as u64,
        latest_membership_hash: membership_rows_hash(rows)?,
    };
    row.validate()?;
    Ok(row)
}

fn full_match_audit(
    skill: &Level2SkillRow,
    code_state_key: &str,
    created_at_unix_ms: i64,
    steps: &[SkillStepEvidence],
    membership_keys: &[String],
) -> Result<SkillLifecycleAuditRow, TrainerError> {
    let evidence_labels = live_evidence_labels(steps);
    let evidence_chunks = steps
        .iter()
        .map(|step| step.chunk_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let row = SkillLifecycleAuditRow {
        schema_version: crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_audit_id: lifecycle_audit_id_from_parts(
            Some(&format!("{}:{code_state_key}", skill.skill_id)),
            SkillLifecycleDecision::NoChangeWithEvidence,
            created_at_unix_ms,
        )?,
        prediction_id: None,
        mistake_id: None,
        previous_skill_id: Some(skill.skill_id.clone()),
        decision: SkillLifecycleDecision::NoChangeWithEvidence,
        candidate_skill_id: None,
        evidence_label_ids: evidence_labels,
        evidence_chunk_ids: evidence_chunks,
        reason: "live_reverse_index_full_skill_match".to_string(),
        created_at_unix_ms,
        evidence_skill_ids: vec![skill.skill_id.clone()],
        evidence_higher_ability_ids: Vec::new(),
        source_membership_keys: membership_keys.to_vec(),
    };
    row.validate()?;
    Ok(row)
}

fn partial_candidate_for_match(
    skill: &Level2SkillRow,
    code_state_key: &str,
    created_at_unix_ms: i64,
    partial: PartialTemplateMatch,
) -> Result<LiveSkillPartialCandidate, TrainerError> {
    let candidate_skill_id = deterministic_candidate_id(
        &skill.skill_id,
        code_state_key,
        &partial.matched_steps,
        &partial.missing_step_indices,
        created_at_unix_ms,
    );
    let candidate = LiveSkillPartialCandidate {
        candidate_skill_id,
        previous_skill_id: Some(skill.skill_id.clone()),
        matched_chunk_ids: partial
            .matched_steps
            .iter()
            .map(|step| step.chunk_id.clone())
            .collect(),
        matched_step_indices: partial
            .matched_steps
            .iter()
            .map(|step| step.step_index)
            .collect(),
        missing_step_indices: partial.missing_step_indices,
        evidence_label_ids: live_evidence_labels(&partial.matched_steps),
        reason: partial.reason,
    };
    validate_id("candidate_skill_id", &candidate.candidate_skill_id)?;
    Ok(candidate)
}

fn partial_candidate_audit(
    candidate: &LiveSkillPartialCandidate,
    created_at_unix_ms: i64,
) -> Result<SkillLifecycleAuditRow, TrainerError> {
    let row = SkillLifecycleAuditRow {
        schema_version: crate::skill_sequence_discovery::SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_audit_id: lifecycle_audit_id_from_parts(
            Some(&candidate.candidate_skill_id),
            SkillLifecycleDecision::CreateNewCandidateSkill,
            created_at_unix_ms,
        )?,
        prediction_id: None,
        mistake_id: None,
        previous_skill_id: candidate.previous_skill_id.clone(),
        decision: SkillLifecycleDecision::CreateNewCandidateSkill,
        candidate_skill_id: Some(candidate.candidate_skill_id.clone()),
        evidence_label_ids: candidate.evidence_label_ids.clone(),
        evidence_chunk_ids: candidate.matched_chunk_ids.clone(),
        reason: candidate.reason.clone(),
        created_at_unix_ms,
        evidence_skill_ids: candidate.previous_skill_id.iter().cloned().collect(),
        evidence_higher_ability_ids: Vec::new(),
        source_membership_keys: Vec::new(),
    };
    row.validate()?;
    Ok(row)
}

fn ability_request_from_live(request: &LiveSkillReverseIndexRequest) -> AbilityResolverRequest {
    AbilityResolverRequest {
        code_state_key: request.code_state_key.clone(),
        chunks: request
            .chunks
            .iter()
            .map(|chunk| {
                let mut accepted_label_ids = chunk.accepted_label_ids.clone();
                accepted_label_ids.sort();
                LiveChunkInput {
                    chunk_id: chunk.chunk_id.clone(),
                    file_path: Some(chunk.file_path.clone()),
                    accepted_label_ids,
                }
            })
            .collect(),
    }
}

fn live_evidence_labels(steps: &[SkillStepEvidence]) -> Vec<String> {
    let mut labels = BTreeSet::new();
    for step in steps {
        labels.extend(step.accepted_label_ids.iter().cloned());
        labels.extend(step.group_ids.iter().cloned());
    }
    labels.into_iter().collect()
}

fn membership_rows_hash(rows: &[ChunkSkillMembershipRow]) -> Result<String, TrainerError> {
    if rows.is_empty() {
        return Err(invalid("chunk_memberships", "must not be empty"));
    }
    let mut hasher = Sha256::new();
    for row in rows {
        row.validate()?;
        hasher.update(row.membership_key.as_bytes());
        hasher.update([0]);
        hasher.update(row.skill_id.as_bytes());
        hasher.update([0]);
        hasher.update(row.chunk_id.as_bytes());
        hasher.update([0]);
        for label in &row.source_accepted_label_ids {
            hasher.update(label.as_bytes());
            hasher.update([0]);
        }
    }
    Ok(format!(
        "membership_hash:{}",
        hex::encode(hasher.finalize())
    ))
}

fn live_match_hash(skill_id: &str, code_state_key: &str, steps: &[SkillStepEvidence]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill_id.as_bytes());
    hasher.update([0]);
    hasher.update(code_state_key.as_bytes());
    for step in steps {
        hasher.update([0xff]);
        hasher.update(step.step_index.to_le_bytes());
        hasher.update(step.chunk_id.as_bytes());
        hasher.update([0]);
        for label in &step.accepted_label_ids {
            hasher.update(label.as_bytes());
            hasher.update([0]);
        }
        for group in &step.group_ids {
            hasher.update(group.as_bytes());
            hasher.update([1]);
        }
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn deterministic_candidate_id(
    skill_id: &str,
    code_state_key: &str,
    matched_steps: &[SkillStepEvidence],
    missing_step_indices: &[u32],
    created_at_unix_ms: i64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill_id.as_bytes());
    hasher.update([0]);
    hasher.update(code_state_key.as_bytes());
    hasher.update(created_at_unix_ms.to_le_bytes());
    for step in matched_steps {
        hasher.update(step.step_index.to_le_bytes());
        hasher.update(step.chunk_id.as_bytes());
    }
    for index in missing_step_indices {
        hasher.update(index.to_le_bytes());
    }
    format!(
        "skill_candidate:live_partial:{}",
        &hex::encode(hasher.finalize())[..24]
    )
}

fn contains_all(haystack: &[String], needles: &[String]) -> bool {
    needles.iter().all(|needle| haystack.contains(needle))
}

fn validate_live_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    skill_validation::invalid(SOURCE_FILE, REMEDIATION, field, message)
}
