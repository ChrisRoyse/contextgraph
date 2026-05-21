use std::collections::{BTreeMap, BTreeSet};

use rocksdb::DB;
use serde::{Deserialize, Serialize};

use crate::chunk_skill_membership::{
    read_all_level2_skill_rows, read_all_skill_lifecycle_audit_rows, ChunkSkillMembershipRow,
    SkillLifecycleAuditRow, SkillLifecycleDecision,
};
use crate::error::TrainerError;
use crate::skill_sequence_discovery::{
    skill_candidate_kind_for_row, skill_genericity_score_for_steps, SkillCandidateKind,
    SkillOutcomeDistribution, SkillPromotionStatus, SKILL_SEQUENCE_SCHEMA_VERSION,
};

use super::{invalid, scan_memberships, skill_linkage_cfs, validate_id, validate_limit};

const MAX_IMPACT_DEPTH: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillImpactChunk {
    pub chunk_id: String,
    pub min_depth: u32,
    pub code_state_keys: Vec<String>,
    pub file_paths: Vec<String>,
    pub via_skill_ids: Vec<String>,
    pub membership_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillImpactReport {
    pub schema_version: u32,
    pub root_chunk_id: String,
    pub root_code_state_key: Option<String>,
    pub depth: u32,
    pub impacted_chunks: Vec<SkillImpactChunk>,
    pub total_impacted_chunks: u64,
    pub touched_skill_ids: Vec<String>,
    pub returned_chunk_limit: usize,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillGraphEdge {
    pub skill_a_id: String,
    pub skill_b_id: String,
    pub relation: String,
    pub support_count: u64,
    pub shared_chunk_ids: Vec<String>,
    pub derived_from: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillGraphInspectReport {
    pub schema_version: u32,
    pub skill_id: Option<String>,
    pub edges: Vec<SkillGraphEdge>,
    pub total_edges: u64,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillConflictPair {
    pub skill_a_id: String,
    pub skill_b_id: String,
    pub relation: String,
    pub support_count: u64,
    pub derived_from: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillConflictGraphReport {
    pub schema_version: u32,
    pub skill_count: u64,
    pub evaluated_pair_count: u64,
    pub conflict_pairs: Vec<SkillConflictPair>,
    pub total_conflict_pairs: u64,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillBrowseRow {
    pub skill_id: String,
    pub skill_name: String,
    pub candidate_kind: SkillCandidateKind,
    pub parent_group_ids: Vec<String>,
    pub parent_skill_ids: Vec<String>,
    pub support: u64,
    pub confidence: f64,
    pub lift_over_cell_baseline: f64,
    pub stability: f64,
    pub genericity_score: f64,
    pub oracle_outcome_distribution: SkillOutcomeDistribution,
    pub promotion_status: SkillPromotionStatus,
    pub operator_approved: bool,
    pub live_input_allowed: bool,
    pub lifecycle_decision_counts: BTreeMap<String, u64>,
    pub source_episode_count: u64,
    pub failure_evidence_set_count: u64,
    pub membership_count: u64,
    pub distinct_chunk_count: u64,
    pub code_state_keys: Vec<String>,
    pub source_membership_keys_sample: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillBrowseReport {
    pub schema_version: u32,
    pub filter: Option<String>,
    pub skills: Vec<SkillBrowseRow>,
    pub total_matching_skills: u64,
    pub total_catalog_skills: u64,
    pub returned_skill_limit: usize,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

pub fn skill_impact(
    db: &DB,
    chunk_id: &str,
    code_state_key: Option<&str>,
    depth: u32,
    limit: usize,
) -> Result<SkillImpactReport, TrainerError> {
    validate_id("chunk_id", chunk_id)?;
    if let Some(value) = code_state_key {
        validate_id("code_state_key", value)?;
    }
    validate_depth(depth)?;
    validate_limit(limit)?;

    let rows = scan_memberships(db)?;
    let root_rows = rows
        .iter()
        .filter(|row| {
            row.chunk_id == chunk_id
                && code_state_key.is_none_or(|value| value == row.code_state_key)
        })
        .cloned()
        .collect::<Vec<_>>();
    if root_rows.is_empty() {
        return Err(invalid(
            "chunk_id",
            "root chunk has no persisted skill memberships",
        ));
    }

    let mut rows_by_chunk = BTreeMap::<String, Vec<ChunkSkillMembershipRow>>::new();
    let mut rows_by_skill = BTreeMap::<String, Vec<ChunkSkillMembershipRow>>::new();
    for row in rows {
        rows_by_chunk
            .entry(row.chunk_id.clone())
            .or_default()
            .push(row.clone());
        rows_by_skill
            .entry(row.skill_id.clone())
            .or_default()
            .push(row);
    }

    let mut impacted = BTreeMap::<String, ImpactAccumulator>::new();
    let mut touched_skill_ids = BTreeSet::<String>::new();
    let mut frontier = BTreeSet::from([chunk_id.to_string()]);
    let mut visited_depth = BTreeMap::<String, u32>::new();
    visited_depth.insert(chunk_id.to_string(), 0);

    for current_depth in 0..=depth {
        let mut next_frontier = BTreeSet::new();
        for current_chunk in &frontier {
            let chunk_rows = if current_depth == 0 && current_chunk == chunk_id {
                root_rows.clone()
            } else {
                rows_by_chunk
                    .get(current_chunk)
                    .cloned()
                    .unwrap_or_default()
            };
            for row in &chunk_rows {
                touched_skill_ids.insert(row.skill_id.clone());
                impacted
                    .entry(row.chunk_id.clone())
                    .or_insert_with(|| ImpactAccumulator::new(current_depth))
                    .observe(current_depth, row);
            }
            if current_depth == depth {
                continue;
            }
            for row in &chunk_rows {
                if let Some(skill_rows) = rows_by_skill.get(&row.skill_id) {
                    for member in skill_rows {
                        if !visited_depth.contains_key(&member.chunk_id) {
                            visited_depth.insert(member.chunk_id.clone(), current_depth + 1);
                            next_frontier.insert(member.chunk_id.clone());
                        }
                        impacted
                            .entry(member.chunk_id.clone())
                            .or_insert_with(|| ImpactAccumulator::new(current_depth + 1))
                            .observe(current_depth + 1, member);
                    }
                }
            }
        }
        frontier = next_frontier;
    }

    let total_impacted_chunks = impacted.len() as u64;
    let mut impacted_chunks = impacted
        .into_iter()
        .map(|(chunk_id, value)| value.into_chunk(chunk_id))
        .collect::<Vec<_>>();
    impacted_chunks.sort_by(|left, right| {
        left.min_depth
            .cmp(&right.min_depth)
            .then_with(|| left.chunk_id.cmp(&right.chunk_id))
    });
    impacted_chunks.truncate(limit);

    Ok(SkillImpactReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        root_chunk_id: chunk_id.to_string(),
        root_code_state_key: code_state_key.map(ToOwned::to_owned),
        depth,
        impacted_chunks,
        total_impacted_chunks,
        touched_skill_ids: touched_skill_ids.into_iter().collect(),
        returned_chunk_limit: limit,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn skill_graph_inspect(
    db: &DB,
    skill_id: Option<&str>,
    limit: usize,
) -> Result<SkillGraphInspectReport, TrainerError> {
    if let Some(value) = skill_id {
        validate_id("skill_id", value)?;
    }
    validate_limit(limit)?;
    let mut edges = derive_skill_edges(&scan_memberships(db)?)?;
    if let Some(skill_id) = skill_id {
        edges.retain(|edge| edge.skill_a_id == skill_id || edge.skill_b_id == skill_id);
    }
    let total_edges = edges.len() as u64;
    edges.sort_by(|left, right| {
        right
            .support_count
            .cmp(&left.support_count)
            .then_with(|| left.skill_a_id.cmp(&right.skill_a_id))
            .then_with(|| left.skill_b_id.cmp(&right.skill_b_id))
    });
    edges.truncate(limit);
    Ok(SkillGraphInspectReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_id: skill_id.map(ToOwned::to_owned),
        edges,
        total_edges,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn skill_conflict_graph(
    db: &DB,
    limit: usize,
) -> Result<SkillConflictGraphReport, TrainerError> {
    validate_limit(limit)?;
    let rows = scan_memberships(db)?;
    let mut skill_ids = BTreeSet::new();
    for row in &rows {
        skill_ids.insert(row.skill_id.clone());
    }
    let co_occurring = derive_skill_edges(&rows)?
        .into_iter()
        .map(|edge| (edge.skill_a_id, edge.skill_b_id))
        .collect::<BTreeSet<_>>();
    let skill_ids = skill_ids.into_iter().collect::<Vec<_>>();
    let mut conflicts = Vec::new();
    let mut evaluated_pair_count = 0_u64;
    for i in 0..skill_ids.len() {
        for j in (i + 1)..skill_ids.len() {
            evaluated_pair_count += 1;
            let pair = (skill_ids[i].clone(), skill_ids[j].clone());
            if !co_occurring.contains(&pair) {
                conflicts.push(SkillConflictPair {
                    skill_a_id: pair.0,
                    skill_b_id: pair.1,
                    relation: "mutually_exclusive_candidate".to_string(),
                    support_count: 0,
                    derived_from: "zero_cooccurrence_over_chunk_skill_memberships".to_string(),
                });
            }
        }
    }
    let total_conflict_pairs = conflicts.len() as u64;
    conflicts.truncate(limit);
    Ok(SkillConflictGraphReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_count: skill_ids.len() as u64,
        evaluated_pair_count,
        conflict_pairs: conflicts,
        total_conflict_pairs,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn skill_browse(
    db: &DB,
    filter: Option<&str>,
    limit: usize,
) -> Result<SkillBrowseReport, TrainerError> {
    validate_browse_filter(filter)?;
    validate_limit(limit)?;

    let filter_normalized = filter.map(|value| value.to_ascii_lowercase());
    let skills = read_all_level2_skill_rows(db)?;
    let total_catalog_skills = skills.len() as u64;
    let rows = scan_memberships(db)?;
    let mut lifecycle_counts_by_skill = BTreeMap::<String, BTreeMap<String, u64>>::new();
    for row in read_all_skill_lifecycle_audit_rows(db)? {
        for skill_id in lifecycle_skill_ids(&row) {
            *lifecycle_counts_by_skill
                .entry(skill_id)
                .or_default()
                .entry(decision_label(row.decision).to_string())
                .or_default() += 1;
        }
    }
    let mut membership_by_skill = BTreeMap::<String, Vec<ChunkSkillMembershipRow>>::new();
    for row in rows {
        membership_by_skill
            .entry(row.skill_id.clone())
            .or_default()
            .push(row);
    }

    let mut browse_rows = Vec::new();
    for skill in skills {
        if let Some(filter) = &filter_normalized {
            let matches = skill.skill_id.to_ascii_lowercase().contains(filter)
                || skill.skill_name.to_ascii_lowercase().contains(filter)
                || skill
                    .parent_group_ids
                    .iter()
                    .any(|group| group.to_ascii_lowercase().contains(filter));
            if !matches {
                continue;
            }
        }
        let memberships = membership_by_skill
            .get(&skill.skill_id)
            .cloned()
            .unwrap_or_default();
        let distinct_chunks = memberships
            .iter()
            .map(|row| row.chunk_id.clone())
            .collect::<BTreeSet<_>>();
        let mut source_membership_keys_sample = memberships
            .iter()
            .map(|row| row.membership_key.clone())
            .collect::<Vec<_>>();
        source_membership_keys_sample.sort();
        source_membership_keys_sample.truncate(limit.min(20));
        let candidate_kind = skill_candidate_kind_for_row(&skill);
        let genericity_score = skill_genericity_score_for_steps(&skill.ordered_steps);
        let lifecycle_decision_counts = lifecycle_counts_by_skill
            .remove(&skill.skill_id)
            .unwrap_or_default();
        browse_rows.push(SkillBrowseRow {
            skill_id: skill.skill_id.clone(),
            skill_name: skill.skill_name.clone(),
            candidate_kind,
            parent_group_ids: skill.parent_group_ids.clone(),
            parent_skill_ids: skill.parent_skill_ids.clone(),
            support: skill.support,
            confidence: skill.confidence,
            lift_over_cell_baseline: skill.lift_over_cell_baseline,
            stability: skill.stability,
            genericity_score,
            oracle_outcome_distribution: skill.oracle_outcome_distribution,
            promotion_status: skill.promotion_status,
            operator_approved: skill.operator_approved,
            live_input_allowed: skill.live_input_allowed,
            lifecycle_decision_counts,
            source_episode_count: skill.source_episode_ids.len() as u64,
            failure_evidence_set_count: skill.failure_evidence_set_ids.len() as u64,
            membership_count: memberships.len() as u64,
            distinct_chunk_count: distinct_chunks.len() as u64,
            code_state_keys: skill.code_state_keys.clone(),
            source_membership_keys_sample,
        });
    }
    let total_matching_skills = browse_rows.len() as u64;
    browse_rows.sort_by(|left, right| {
        right
            .membership_count
            .cmp(&left.membership_count)
            .then_with(|| left.skill_id.cmp(&right.skill_id))
    });
    browse_rows.truncate(limit);
    Ok(SkillBrowseReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        filter: filter.map(ToOwned::to_owned),
        skills: browse_rows,
        total_matching_skills,
        total_catalog_skills,
        returned_skill_limit: limit,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

fn derive_skill_edges(
    rows: &[ChunkSkillMembershipRow],
) -> Result<Vec<SkillGraphEdge>, TrainerError> {
    let mut chunk_skill_sets = BTreeMap::<String, (String, BTreeSet<String>)>::new();
    for row in rows {
        row.validate()?;
        let key = format!("{}::{}", row.chunk_id, row.code_state_key);
        let entry = chunk_skill_sets
            .entry(key)
            .or_insert_with(|| (row.chunk_id.clone(), BTreeSet::new()));
        entry.1.insert(row.skill_id.clone());
    }
    let mut edges = BTreeMap::<(String, String), EdgeAccumulator>::new();
    for (_chunk_state, (chunk_id, skills)) in chunk_skill_sets {
        let skills = skills.into_iter().collect::<Vec<_>>();
        for i in 0..skills.len() {
            for j in (i + 1)..skills.len() {
                let key = ordered_pair(&skills[i], &skills[j]);
                edges.entry(key).or_default().observe(&chunk_id);
            }
        }
    }
    Ok(edges
        .into_iter()
        .map(|((skill_a_id, skill_b_id), acc)| SkillGraphEdge {
            skill_a_id,
            skill_b_id,
            relation: "co_member_chunk".to_string(),
            support_count: acc.support_count,
            shared_chunk_ids: acc.shared_chunk_ids.into_iter().collect(),
            derived_from: "chunk_skill_membership_cooccurrence".to_string(),
        })
        .collect())
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

fn decision_label(decision: SkillLifecycleDecision) -> &'static str {
    match decision {
        SkillLifecycleDecision::UpdateExistingSkill => "update_existing_skill",
        SkillLifecycleDecision::SplitMixedSkill => "split_mixed_skill",
        SkillLifecycleDecision::CreateNewCandidateSkill => "create_new_candidate_skill",
        SkillLifecycleDecision::DemoteUnstableSkill => "demote_unstable_skill",
        SkillLifecycleDecision::NoChangeWithEvidence => "no_change_with_evidence",
    }
}

fn ordered_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

fn validate_depth(depth: u32) -> Result<(), TrainerError> {
    if depth > MAX_IMPACT_DEPTH {
        return Err(invalid(
            "depth",
            format!("must be within 0..={MAX_IMPACT_DEPTH}"),
        ));
    }
    Ok(())
}

fn validate_browse_filter(filter: Option<&str>) -> Result<(), TrainerError> {
    let Some(value) = filter else {
        return Ok(());
    };
    if value.trim().is_empty() || value.len() > 256 {
        return Err(invalid(
            "filter",
            "must be non-empty and no longer than 256 bytes",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct ImpactAccumulator {
    min_depth: u32,
    code_state_keys: BTreeSet<String>,
    file_paths: BTreeSet<String>,
    via_skill_ids: BTreeSet<String>,
    membership_keys: BTreeSet<String>,
}

impl ImpactAccumulator {
    fn new(depth: u32) -> Self {
        Self {
            min_depth: depth,
            code_state_keys: BTreeSet::new(),
            file_paths: BTreeSet::new(),
            via_skill_ids: BTreeSet::new(),
            membership_keys: BTreeSet::new(),
        }
    }

    fn observe(&mut self, depth: u32, row: &ChunkSkillMembershipRow) {
        self.min_depth = self.min_depth.min(depth);
        self.code_state_keys.insert(row.code_state_key.clone());
        self.file_paths.insert(row.file_path.clone());
        self.via_skill_ids.insert(row.skill_id.clone());
        self.membership_keys.insert(row.membership_key.clone());
    }

    fn into_chunk(self, chunk_id: String) -> SkillImpactChunk {
        SkillImpactChunk {
            chunk_id,
            min_depth: self.min_depth,
            code_state_keys: self.code_state_keys.into_iter().collect(),
            file_paths: self.file_paths.into_iter().collect(),
            via_skill_ids: self.via_skill_ids.into_iter().collect(),
            membership_keys: self.membership_keys.into_iter().collect(),
        }
    }
}

#[derive(Debug, Default)]
struct EdgeAccumulator {
    support_count: u64,
    shared_chunk_ids: BTreeSet<String>,
}

impl EdgeAccumulator {
    fn observe(&mut self, chunk_id: &str) {
        self.support_count += 1;
        self.shared_chunk_ids.insert(chunk_id.to_string());
    }
}
