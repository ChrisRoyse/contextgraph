//! TASK-PY-G-120 byte-grounded read surface over the persisted #414 chunk-skill substrate.

use crate::chunk_skill_membership::{
    membership_key, read_level2_skill_row, ChunkSkillMembershipRow,
};
use crate::error::TrainerError;
use crate::skill_sequence_discovery::{Level2SkillRow, SKILL_SEQUENCE_SCHEMA_VERSION};
use crate::skill_validation;
use context_graph_mejepa_cf::CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP;
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[path = "skill_linkage_audit.rs"]
mod skill_linkage_audit;
#[path = "skill_linkage_graph.rs"]
mod skill_linkage_graph;
#[path = "skill_linkage_source.rs"]
mod skill_linkage_source;
#[path = "skill_linkage_storage.rs"]
mod skill_linkage_storage;

pub use skill_linkage_audit::{
    skill_usefulness_audit, SkillAuditGapView, SkillAuditLifecycleSummary,
    SkillAuditRejectedPattern, SkillAuditRepresentativeSequence, SkillAuditRow,
    SkillAuditSkillBucketCounts, SkillUsefulnessAuditOptions, SkillUsefulnessAuditReport,
};
pub use skill_linkage_graph::{
    skill_browse, skill_conflict_graph, skill_graph_inspect, skill_impact, SkillBrowseReport,
    SkillBrowseRow, SkillConflictGraphReport, SkillConflictPair, SkillGraphEdge,
    SkillGraphInspectReport, SkillImpactChunk, SkillImpactReport,
};
pub use skill_linkage_source::{
    load_chunk_source_index_jsonl, load_chunk_source_index_jsonl_with_limit, ChunkSourceIndex,
    ChunkSourceRow,
};
pub use skill_linkage_storage::{open_skill_linkage_rocksdb, skill_linkage_cfs};

const MAX_QUERY_LIMIT: usize = 10_000;
const MAX_ROW_IDS: usize = 4096;
const SOURCE_FILE: &str = "file:crates/context-graph-mejepa-train/src/skill_linkage.rs";
const REMEDIATION: &str = "skill linkage must expose byte-grounded many-to-many chunk and skill evidence without adding prediction heads";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillLinkageOptions {
    pub limit: usize,
    pub require_source_text: bool,
}

impl Default for SkillLinkageOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            require_source_text: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCodeChunk {
    pub chunk_id: String,
    pub file_path: String,
    pub code_state_key: String,
    pub skill_id: String,
    pub membership_key: String,
    pub membership_score: f64,
    pub hierarchy_level: u8,
    pub source_accepted_label_ids: Vec<String>,
    pub source_text: Option<String>,
    pub source_text_sha256: Option<String>,
    pub byte_span: Option<[u64; 2]>,
    pub source_status: SourceTextStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SourceTextStatus {
    Provided,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillToCodeReport {
    pub schema_version: u32,
    pub skill: Level2SkillRow,
    pub chunks: Vec<SkillCodeChunk>,
    pub total_matching_memberships: u64,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkSkillLink {
    pub skill: Level2SkillRow,
    pub membership: ChunkSkillMembershipRow,
    pub source: Option<SkillCodeChunk>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CodeToSkillReport {
    pub schema_version: u32,
    pub chunk_id: String,
    pub code_state_key: Option<String>,
    pub skills: Vec<ChunkSkillLink>,
    pub total_matching_memberships: u64,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillSetQueryReport {
    pub schema_version: u32,
    pub must_have: Vec<String>,
    pub must_not_have: Vec<String>,
    pub matching_chunk_ids: Vec<String>,
    pub returned_chunk_ids: Vec<String>,
    pub total_matching_chunks: u64,
    pub no_new_prediction_head_introduced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillCoverageAudit {
    pub schema_version: u32,
    pub total_chunk_universe: u64,
    pub chunks_with_membership: u64,
    pub chunks_without_membership: u64,
    pub one_skill_chunks: u64,
    pub multi_skill_chunks: u64,
    pub zero_membership_chunk_ids: Vec<String>,
    pub total_membership_rows: u64,
    pub no_new_prediction_head_introduced: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SlotVectorSummaryStatus {
    LabelOnlyNoVectorStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SlotParameterCard {
    pub slot_id: String,
    pub label_ids: Vec<String>,
    pub label_count: u64,
    pub occupancy: u64,
    pub vector_norm: Option<f64>,
    pub sparsity: Option<f64>,
    pub summary_status: SlotVectorSummaryStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConstellationSkillMembership {
    pub skill_id: String,
    pub skill_name: Option<String>,
    pub hierarchy_level: u8,
    pub membership_key: String,
    pub membership_score: f64,
    pub parent_group_ids: Vec<String>,
    pub source_accepted_label_ids: Vec<String>,
    pub ordered_step_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConstellationMembershipReport {
    pub schema_version: u32,
    pub chunk_id: String,
    pub code_state_key: Option<String>,
    pub live_level0_label_ids: Vec<String>,
    pub level1_group_ids: Vec<String>,
    pub level2_skill_ids: Vec<String>,
    pub higher_ability_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub memberships: Vec<ConstellationSkillMembership>,
    pub no_new_prediction_head_introduced: bool,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkAsStarReport {
    pub schema_version: u32,
    pub chunk_id: String,
    pub code_state_key: Option<String>,
    pub slot_parameter_cards: Vec<SlotParameterCard>,
    pub pair_relation_label_ids: Vec<String>,
    pub group_label_ids: Vec<String>,
    pub unrouted_label_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub active_higher_ability_ids: Vec<String>,
    pub source_membership_keys: Vec<String>,
    pub source: Option<SkillCodeChunk>,
    pub vector_stats_source: String,
    pub no_new_prediction_head_introduced: bool,
    pub flat_vector_concat_used: bool,
    pub target_outcomes_used_as_live_inputs: bool,
    pub source_of_truth_cfs: Vec<String>,
}

pub fn skill_to_code(
    db: &DB,
    skill_id: &str,
    source_index: Option<&ChunkSourceIndex>,
    options: SkillLinkageOptions,
) -> Result<SkillToCodeReport, TrainerError> {
    validate_query_options(options)?;
    validate_id("skill_id", skill_id)?;
    let skill = read_level2_skill_row(db, skill_id)?
        .ok_or_else(|| invalid("skill_id", format!("skill {skill_id} not found")))?;
    let mut rows = BTreeMap::<String, ChunkSkillMembershipRow>::new();
    for row in scan_memberships(db)? {
        if row.skill_id == skill_id {
            rows.insert(row.membership_key.clone(), row);
        }
    }
    let total_matching_memberships = rows.len() as u64;
    let mut chunks = rows
        .into_values()
        .map(|row| chunk_from_membership(row, source_index, options.require_source_text))
        .collect::<Result<Vec<_>, _>>()?;
    chunks.sort_by(|left, right| {
        right
            .membership_score
            .total_cmp(&left.membership_score)
            .then_with(|| left.membership_key.cmp(&right.membership_key))
    });
    chunks.truncate(options.limit);
    Ok(SkillToCodeReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill,
        chunks,
        total_matching_memberships,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn code_to_skill(
    db: &DB,
    chunk_id: &str,
    code_state_key: Option<&str>,
    source_index: Option<&ChunkSourceIndex>,
    options: SkillLinkageOptions,
) -> Result<CodeToSkillReport, TrainerError> {
    validate_query_options(options)?;
    validate_id("chunk_id", chunk_id)?;
    if let Some(value) = code_state_key {
        validate_id("code_state_key", value)?;
    }
    let mut links = Vec::new();
    for row in scan_memberships(db)? {
        if row.chunk_id != chunk_id {
            continue;
        }
        if code_state_key.is_some_and(|value| value != row.code_state_key) {
            continue;
        }
        let skill = read_level2_skill_row(db, &row.skill_id)?
            .ok_or_else(|| invalid("skill_id", format!("skill {} missing", row.skill_id)))?;
        let source = Some(chunk_from_membership(
            row.clone(),
            source_index,
            options.require_source_text,
        )?);
        links.push(ChunkSkillLink {
            skill,
            membership: row,
            source,
        });
    }
    let total_matching_memberships = links.len() as u64;
    links.sort_by(|left, right| {
        left.membership
            .skill_id
            .cmp(&right.membership.skill_id)
            .then_with(|| {
                left.membership
                    .membership_key
                    .cmp(&right.membership.membership_key)
            })
    });
    links.truncate(options.limit);
    Ok(CodeToSkillReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        chunk_id: chunk_id.to_string(),
        code_state_key: code_state_key.map(ToOwned::to_owned),
        skills: links,
        total_matching_memberships,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn constellation_membership(
    db: &DB,
    chunk_id: &str,
    code_state_key: Option<&str>,
    limit: usize,
) -> Result<ConstellationMembershipReport, TrainerError> {
    validate_limit(limit)?;
    let rows = membership_rows_for_chunk(db, chunk_id, code_state_key)?;
    let mut live_level0_label_ids = BTreeSet::new();
    let mut level1_group_ids = BTreeSet::new();
    let mut level2_skill_ids = Vec::new();
    let mut higher_ability_ids = Vec::new();
    let mut source_membership_keys = Vec::new();
    let mut memberships = Vec::new();
    for row in rows.into_iter().take(limit) {
        let skill = read_level2_skill_row(db, &row.skill_id)?;
        live_level0_label_ids.extend(row.source_accepted_label_ids.iter().cloned());
        source_membership_keys.push(row.membership_key.clone());
        if row.hierarchy_level == 2 {
            push_unique(&mut level2_skill_ids, &row.skill_id);
        } else {
            push_unique(&mut higher_ability_ids, &row.skill_id);
        }
        if let Some(skill) = &skill {
            level1_group_ids.extend(skill.parent_group_ids.iter().cloned());
        }
        for step in &row.ordered_step_evidence {
            live_level0_label_ids.extend(step.accepted_label_ids.iter().cloned());
            level1_group_ids.extend(step.group_ids.iter().cloned());
        }
        memberships.push(ConstellationSkillMembership {
            skill_id: row.skill_id.clone(),
            skill_name: skill.as_ref().map(|value| value.skill_name.clone()),
            hierarchy_level: row.hierarchy_level,
            membership_key: row.membership_key,
            membership_score: row.membership_score,
            parent_group_ids: skill
                .as_ref()
                .map(|value| value.parent_group_ids.clone())
                .unwrap_or_default(),
            source_accepted_label_ids: row.source_accepted_label_ids,
            ordered_step_count: row.ordered_step_evidence.len() as u64,
        });
    }
    Ok(ConstellationMembershipReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        chunk_id: chunk_id.to_string(),
        code_state_key: code_state_key.map(ToOwned::to_owned),
        live_level0_label_ids: live_level0_label_ids.into_iter().collect(),
        level1_group_ids: level1_group_ids.into_iter().collect(),
        level2_skill_ids,
        higher_ability_ids,
        source_membership_keys,
        memberships,
        no_new_prediction_head_introduced: true,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn chunk_as_star(
    db: &DB,
    chunk_id: &str,
    code_state_key: Option<&str>,
    source_index: Option<&ChunkSourceIndex>,
    options: SkillLinkageOptions,
) -> Result<ChunkAsStarReport, TrainerError> {
    validate_query_options(options)?;
    let membership = constellation_membership(db, chunk_id, code_state_key, options.limit)?;
    let mut pair_relation_label_ids = BTreeSet::new();
    let mut group_label_ids = BTreeSet::new();
    let mut unrouted_label_ids = BTreeSet::new();
    let mut slots = BTreeMap::<String, BTreeSet<String>>::new();
    for label in &membership.live_level0_label_ids {
        match label_route(label) {
            LabelRoute::Slot(slot_id) => {
                slots.entry(slot_id).or_default().insert(label.clone());
            }
            LabelRoute::Pair => {
                pair_relation_label_ids.insert(label.clone());
            }
            LabelRoute::Group => {
                group_label_ids.insert(label.clone());
            }
            LabelRoute::Unrouted => {
                unrouted_label_ids.insert(label.clone());
            }
        }
    }
    group_label_ids.extend(membership.level1_group_ids.iter().cloned());
    let slot_parameter_cards = slots
        .into_iter()
        .map(|(slot_id, labels)| {
            let label_ids = labels.into_iter().collect::<Vec<_>>();
            SlotParameterCard {
                slot_id,
                label_count: label_ids.len() as u64,
                occupancy: label_ids.len() as u64,
                label_ids,
                vector_norm: None,
                sparsity: None,
                summary_status: SlotVectorSummaryStatus::LabelOnlyNoVectorStats,
            }
        })
        .collect::<Vec<_>>();
    let rows = membership_rows_for_chunk(db, chunk_id, code_state_key)?;
    let source = rows
        .into_iter()
        .next()
        .map(|row| chunk_from_membership(row, source_index, options.require_source_text))
        .transpose()?;
    if options.require_source_text && source.is_none() {
        return Err(invalid(
            "source_index",
            "source text requires at least one chunk-skill membership row",
        ));
    }
    Ok(ChunkAsStarReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        chunk_id: chunk_id.to_string(),
        code_state_key: code_state_key.map(ToOwned::to_owned),
        slot_parameter_cards,
        pair_relation_label_ids: pair_relation_label_ids.into_iter().collect(),
        group_label_ids: group_label_ids.into_iter().collect(),
        unrouted_label_ids: unrouted_label_ids.into_iter().collect(),
        active_skill_ids: membership.level2_skill_ids,
        active_higher_ability_ids: membership.higher_ability_ids,
        source_membership_keys: membership.source_membership_keys,
        source,
        vector_stats_source:
            "label_registry_only_no_vector_norms_without_explicit_slot_tensor_join".to_string(),
        no_new_prediction_head_introduced: true,
        flat_vector_concat_used: false,
        target_outcomes_used_as_live_inputs: false,
        source_of_truth_cfs: skill_linkage_cfs(),
    })
}

pub fn skill_set_query(
    db: &DB,
    must_have: &[String],
    must_not_have: &[String],
    limit: usize,
) -> Result<SkillSetQueryReport, TrainerError> {
    validate_limit(limit)?;
    if must_have.is_empty() {
        return Err(invalid("must_have", "must include at least one skill id"));
    }
    validate_id_list("must_have", must_have, MAX_ROW_IDS)?;
    validate_id_list("must_not_have", must_not_have, MAX_ROW_IDS)?;
    let must_have_set = must_have.iter().cloned().collect::<BTreeSet<_>>();
    let must_not_have_set = must_not_have.iter().cloned().collect::<BTreeSet<_>>();
    if !must_have_set.is_disjoint(&must_not_have_set) {
        return Err(invalid(
            "skill_set_query",
            "must_have and must_not_have must be disjoint",
        ));
    }
    let mut chunk_skills = BTreeMap::<String, BTreeSet<String>>::new();
    for row in scan_memberships(db)? {
        chunk_skills
            .entry(row.chunk_id)
            .or_default()
            .insert(row.skill_id);
    }
    let matching_chunk_ids = chunk_skills
        .into_iter()
        .filter_map(|(chunk_id, skills)| {
            must_have_set
                .is_subset(&skills)
                .then_some(())
                .filter(|_| must_not_have_set.is_disjoint(&skills))
                .map(|_| chunk_id)
        })
        .collect::<Vec<_>>();
    let mut returned_chunk_ids = matching_chunk_ids.clone();
    returned_chunk_ids.truncate(limit);
    Ok(SkillSetQueryReport {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        must_have: must_have.to_vec(),
        must_not_have: must_not_have.to_vec(),
        total_matching_chunks: matching_chunk_ids.len() as u64,
        matching_chunk_ids,
        returned_chunk_ids,
        no_new_prediction_head_introduced: true,
    })
}

pub fn skill_coverage_audit(
    db: &DB,
    chunk_universe: &[String],
    sample_limit: usize,
) -> Result<SkillCoverageAudit, TrainerError> {
    validate_limit(sample_limit.max(1))?;
    validate_id_list("chunk_universe", chunk_universe, usize::MAX)?;
    let mut chunk_to_skills = BTreeMap::<String, BTreeSet<String>>::new();
    let mut total_membership_rows = 0_u64;
    for row in scan_memberships(db)? {
        total_membership_rows += 1;
        chunk_to_skills
            .entry(row.chunk_id)
            .or_default()
            .insert(row.skill_id);
    }
    let universe = if chunk_universe.is_empty() {
        chunk_to_skills.keys().cloned().collect::<Vec<_>>()
    } else {
        chunk_universe.to_vec()
    };
    let mut chunks_with_membership = 0_u64;
    let mut one_skill_chunks = 0_u64;
    let mut multi_skill_chunks = 0_u64;
    let mut zero_membership_chunk_ids = Vec::new();
    for chunk_id in &universe {
        match chunk_to_skills
            .get(chunk_id)
            .map(BTreeSet::len)
            .unwrap_or(0)
        {
            0 => {
                if zero_membership_chunk_ids.len() < sample_limit {
                    zero_membership_chunk_ids.push(chunk_id.clone());
                }
            }
            1 => {
                chunks_with_membership += 1;
                one_skill_chunks += 1;
            }
            _ => {
                chunks_with_membership += 1;
                multi_skill_chunks += 1;
            }
        }
    }
    Ok(SkillCoverageAudit {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        total_chunk_universe: universe.len() as u64,
        chunks_with_membership,
        chunks_without_membership: universe.len() as u64 - chunks_with_membership,
        one_skill_chunks,
        multi_skill_chunks,
        zero_membership_chunk_ids,
        total_membership_rows,
        no_new_prediction_head_introduced: true,
    })
}

fn chunk_from_membership(
    row: ChunkSkillMembershipRow,
    source_index: Option<&ChunkSourceIndex>,
    require_source_text: bool,
) -> Result<SkillCodeChunk, TrainerError> {
    row.validate()?;
    let expected_key = membership_key(&row.chunk_id, &row.code_state_key, &row.skill_id)?;
    if row.membership_key != expected_key {
        return Err(invalid("membership_key", "does not match row identity"));
    }
    let source = match source_index {
        Some(index) => resolve_source_row(index, &row)?,
        None => None,
    };
    if require_source_text && source_index.is_none() {
        return Err(invalid(
            "source_index",
            "source index is required when require_source_text=true",
        ));
    }
    if require_source_text && source.is_none() {
        return Err(invalid(
            "source_index",
            format!("missing source row for chunk {}", row.chunk_id),
        ));
    }
    if let Some(source) = source {
        source.validate()?;
        if source.file_path != row.file_path {
            return Err(invalid(
                "source.file_path",
                format!(
                    "source path {} does not match membership path {}",
                    source.file_path, row.file_path
                ),
            ));
        }
        if require_source_text && source.source_text.is_none() {
            return Err(invalid(
                "source_text",
                format!("source text missing for chunk {}", row.chunk_id),
            ));
        }
    }
    Ok(SkillCodeChunk {
        chunk_id: row.chunk_id,
        file_path: row.file_path,
        code_state_key: row.code_state_key,
        skill_id: row.skill_id,
        membership_key: row.membership_key,
        membership_score: row.membership_score,
        hierarchy_level: row.hierarchy_level,
        source_accepted_label_ids: row.source_accepted_label_ids,
        source_text: source.and_then(|value| value.source_text.clone()),
        source_text_sha256: source.and_then(|value| value.source_text_sha256.clone()),
        byte_span: source.map(|value| value.byte_span),
        source_status: if source
            .and_then(|value| value.source_text.as_ref())
            .is_some()
        {
            SourceTextStatus::Provided
        } else {
            SourceTextStatus::Unavailable
        },
    })
}

fn resolve_source_row<'a>(
    index: &'a ChunkSourceIndex,
    membership: &ChunkSkillMembershipRow,
) -> Result<Option<&'a ChunkSourceRow>, TrainerError> {
    let Some(candidates) = index.rows_by_chunk_id.get(&membership.chunk_id) else {
        return Ok(None);
    };
    let mut exact_path = candidates
        .iter()
        .filter(|row| row.file_path == membership.file_path)
        .collect::<Vec<_>>();
    if exact_path.len() == 1 {
        return Ok(Some(exact_path.remove(0)));
    }
    if exact_path.len() > 1 {
        return Err(invalid(
            "source_index",
            format!(
                "chunk {} has duplicate source rows for path {}",
                membership.chunk_id, membership.file_path
            ),
        ));
    }
    if candidates.len() == 1 {
        return Ok(candidates.first());
    }
    Err(invalid(
        "source_index",
        format!(
            "chunk {} has {} source rows and none match path {}",
            membership.chunk_id,
            candidates.len(),
            membership.file_path
        ),
    ))
}

fn scan_memberships(db: &DB) -> Result<Vec<ChunkSkillMembershipRow>, TrainerError> {
    let cf = db
        .cf_handle(CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)
        .ok_or_else(|| invalid("rocksdb.column_family", "missing chunk-skill membership CF"))?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, bytes) = item.map_err(map_rocksdb_error)?;
        let row: ChunkSkillMembershipRow =
            bincode::deserialize(&bytes).map_err(map_bincode_error)?;
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn membership_rows_for_chunk(
    db: &DB,
    chunk_id: &str,
    code_state_key: Option<&str>,
) -> Result<Vec<ChunkSkillMembershipRow>, TrainerError> {
    validate_id("chunk_id", chunk_id)?;
    if let Some(value) = code_state_key {
        validate_id("code_state_key", value)?;
    }
    let mut rows = scan_memberships(db)?
        .into_iter()
        .filter(|row| {
            row.chunk_id == chunk_id
                && code_state_key.is_none_or(|value| value == row.code_state_key)
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.code_state_key
            .cmp(&right.code_state_key)
            .then_with(|| left.hierarchy_level.cmp(&right.hierarchy_level))
            .then_with(|| left.skill_id.cmp(&right.skill_id))
            .then_with(|| left.membership_key.cmp(&right.membership_key))
    });
    Ok(rows)
}

enum LabelRoute {
    Slot(String),
    Pair,
    Group,
    Unrouted,
}

fn label_route(label: &str) -> LabelRoute {
    if let Some(rest) = label.strip_prefix("slot:") {
        let slot_id = rest.split(':').next().unwrap_or(rest);
        return LabelRoute::Slot(slot_id.to_string());
    }
    if label.starts_with("pair:") || label.starts_with("pair_relation:") {
        return LabelRoute::Pair;
    }
    if label.starts_with("group:") {
        return LabelRoute::Group;
    }
    if label.starts_with("ast_surface:") {
        return LabelRoute::Slot("e_ast".to_string());
    }
    LabelRoute::Unrouted
}

fn push_unique(target: &mut Vec<String>, value: &str) {
    if !target.iter().any(|existing| existing == value) {
        target.push(value.to_string());
    }
}

fn validate_query_options(options: SkillLinkageOptions) -> Result<(), TrainerError> {
    validate_limit(options.limit)
}

fn validate_limit(limit: usize) -> Result<(), TrainerError> {
    if limit == 0 || limit > MAX_QUERY_LIMIT {
        return Err(invalid(
            "limit",
            format!("must be within 1..={MAX_QUERY_LIMIT}"),
        ));
    }
    Ok(())
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
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

#[cfg(test)]
#[path = "skill_linkage_tests.rs"]
mod skill_linkage_tests;
