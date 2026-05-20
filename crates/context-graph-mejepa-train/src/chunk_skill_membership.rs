//! TASK-PY-G-118 chunk-to-skill membership persistence.
//!
//! These rows let a compiled chunk explain which ordered skills or higher-level
//! consequence patterns it participates in. The rows are many-to-many and are
//! persisted with independent RocksDB readback before they can feed replay.

use crate::error::TrainerError;
use crate::skill_sequence_discovery::{
    Level2SkillRow, SkillEpisodeRow, SkillStepEvidence, SKILL_SEQUENCE_SCHEMA_VERSION,
};
use crate::skill_validation;
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS,
    CF_MEJEPA_SKILL_LIFECYCLE_AUDIT, CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use rocksdb::{IteratorMode, WriteBatch, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROW_IDS: usize = 4096;
const SOURCE_FILE: &str = "file:crates/context-graph-mejepa-train/src/chunk_skill_membership.rs";
const REMEDIATION: &str =
    "chunk skill membership must stay many-to-many, ordered, live-input-safe, and RocksDB-readback verified";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChunkSkillMembershipRow {
    pub schema_version: u32,
    pub membership_key: String,
    pub chunk_id: String,
    pub file_path: String,
    pub code_state_key: String,
    pub skill_id: String,
    pub hierarchy_level: u8,
    pub membership_score: f64,
    pub source_accepted_label_ids: Vec<String>,
    pub ordered_step_evidence: Vec<SkillStepEvidence>,
    pub live_input_allowed: bool,
    pub provenance_hashes: Vec<String>,
    pub first_seen_unix_ms: i64,
    pub last_seen_unix_ms: i64,
}

impl ChunkSkillMembershipRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("membership_key", &self.membership_key)?;
        validate_id("chunk_id", &self.chunk_id)?;
        validate_project_relative_path("file_path", &self.file_path)?;
        validate_id("code_state_key", &self.code_state_key)?;
        validate_id("skill_id", &self.skill_id)?;
        if self.hierarchy_level < 2 {
            return Err(invalid("hierarchy_level", "must be Level-2 or higher"));
        }
        validate_finite_unit("membership_score", self.membership_score)?;
        validate_live_id_list(
            "source_accepted_label_ids",
            &self.source_accepted_label_ids,
            MAX_ROW_IDS,
        )?;
        if self.ordered_step_evidence.is_empty() {
            return Err(invalid(
                "ordered_step_evidence",
                "must cite at least one ordered step",
            ));
        }
        for step in &self.ordered_step_evidence {
            validate_step_evidence(step)?;
            if step.chunk_id != self.chunk_id || step.code_state_key != self.code_state_key {
                return Err(invalid(
                    "ordered_step_evidence",
                    "step evidence must match the membership chunk and code_state_key",
                ));
            }
        }
        if !self.live_input_allowed {
            return Err(invalid(
                "live_input_allowed",
                "membership rows with leaky live inputs must not be persisted",
            ));
        }
        validate_id_list("provenance_hashes", &self.provenance_hashes, MAX_ROW_IDS)?;
        if self.first_seen_unix_ms <= 0 || self.last_seen_unix_ms < self.first_seen_unix_ms {
            return Err(invalid(
                "timestamps",
                "first_seen must be positive and no later than last_seen",
            ));
        }
        let expected = membership_key(&self.chunk_id, &self.code_state_key, &self.skill_id)?;
        if self.membership_key != expected {
            return Err(invalid("membership_key", "does not match row identity"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OrderedStepSpan {
    pub step_index: u32,
    pub chunk_id: String,
    pub file_path: String,
    pub code_state_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillReverseIndexRow {
    pub schema_version: u32,
    pub reverse_index_key: String,
    pub skill_id: String,
    pub code_state_key: String,
    pub chunk_ids: Vec<String>,
    pub file_paths: Vec<String>,
    pub ordered_step_spans: Vec<OrderedStepSpan>,
    pub support: u64,
    pub latest_membership_hash: String,
}

impl SkillReverseIndexRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("reverse_index_key", &self.reverse_index_key)?;
        validate_id("skill_id", &self.skill_id)?;
        validate_id("code_state_key", &self.code_state_key)?;
        validate_id_list("chunk_ids", &self.chunk_ids, MAX_ROW_IDS)?;
        validate_path_list("file_paths", &self.file_paths, MAX_ROW_IDS)?;
        if self.chunk_ids.is_empty() || self.ordered_step_spans.is_empty() {
            return Err(invalid(
                "reverse_index",
                "must index at least one chunk and one ordered step span",
            ));
        }
        for span in &self.ordered_step_spans {
            validate_id("span.chunk_id", &span.chunk_id)?;
            validate_project_relative_path("span.file_path", &span.file_path)?;
            validate_id("span.code_state_key", &span.code_state_key)?;
            if span.code_state_key != self.code_state_key {
                return Err(invalid(
                    "ordered_step_spans",
                    "span code_state_key must match reverse index key",
                ));
            }
        }
        if self.support == 0 {
            return Err(invalid("support", "must be positive"));
        }
        validate_id("latest_membership_hash", &self.latest_membership_hash)?;
        let expected = reverse_index_key(&self.skill_id, &self.code_state_key)?;
        if self.reverse_index_key != expected {
            return Err(invalid(
                "reverse_index_key",
                "does not match skill_id/code_state_key",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SkillLifecycleDecision {
    UpdateExistingSkill,
    SplitMixedSkill,
    CreateNewCandidateSkill,
    DemoteUnstableSkill,
    NoChangeWithEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillLifecycleAuditRow {
    pub schema_version: u32,
    pub skill_audit_id: String,
    pub prediction_id: Option<String>,
    pub mistake_id: Option<String>,
    pub previous_skill_id: Option<String>,
    pub decision: SkillLifecycleDecision,
    pub candidate_skill_id: Option<String>,
    pub evidence_label_ids: Vec<String>,
    pub evidence_chunk_ids: Vec<String>,
    pub reason: String,
    pub created_at_unix_ms: i64,
    #[serde(default)]
    pub evidence_skill_ids: Vec<String>,
    #[serde(default)]
    pub evidence_higher_ability_ids: Vec<String>,
    #[serde(default)]
    pub source_membership_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacySkillLifecycleAuditRow {
    schema_version: u32,
    skill_audit_id: String,
    prediction_id: Option<String>,
    mistake_id: Option<String>,
    previous_skill_id: Option<String>,
    decision: SkillLifecycleDecision,
    candidate_skill_id: Option<String>,
    evidence_label_ids: Vec<String>,
    evidence_chunk_ids: Vec<String>,
    reason: String,
    created_at_unix_ms: i64,
}

impl From<LegacySkillLifecycleAuditRow> for SkillLifecycleAuditRow {
    fn from(value: LegacySkillLifecycleAuditRow) -> Self {
        Self {
            schema_version: value.schema_version,
            skill_audit_id: value.skill_audit_id,
            prediction_id: value.prediction_id,
            mistake_id: value.mistake_id,
            previous_skill_id: value.previous_skill_id,
            decision: value.decision,
            candidate_skill_id: value.candidate_skill_id,
            evidence_label_ids: value.evidence_label_ids,
            evidence_chunk_ids: value.evidence_chunk_ids,
            reason: value.reason,
            created_at_unix_ms: value.created_at_unix_ms,
            evidence_skill_ids: Vec::new(),
            evidence_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
        }
    }
}

impl SkillLifecycleAuditRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_schema(self.schema_version)?;
        validate_id("skill_audit_id", &self.skill_audit_id)?;
        for (field, value) in [
            ("prediction_id", &self.prediction_id),
            ("mistake_id", &self.mistake_id),
            ("previous_skill_id", &self.previous_skill_id),
            ("candidate_skill_id", &self.candidate_skill_id),
        ] {
            if let Some(value) = value {
                validate_id(field, value)?;
            }
        }
        validate_live_id_list("evidence_label_ids", &self.evidence_label_ids, MAX_ROW_IDS)?;
        validate_id_list("evidence_skill_ids", &self.evidence_skill_ids, MAX_ROW_IDS)?;
        validate_id_list(
            "evidence_higher_ability_ids",
            &self.evidence_higher_ability_ids,
            MAX_ROW_IDS,
        )?;
        validate_id_list("evidence_chunk_ids", &self.evidence_chunk_ids, MAX_ROW_IDS)?;
        validate_id_list(
            "source_membership_keys",
            &self.source_membership_keys,
            MAX_ROW_IDS,
        )?;
        validate_id("reason", &self.reason)?;
        if self.created_at_unix_ms <= 0 {
            return Err(invalid("created_at_unix_ms", "must be positive"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkillMaterialization {
    pub level2_skills: Vec<Level2SkillRow>,
    pub chunk_memberships: Vec<ChunkSkillMembershipRow>,
    pub reverse_indexes: Vec<SkillReverseIndexRow>,
    pub lifecycle_audits: Vec<SkillLifecycleAuditRow>,
}

pub fn materialize_skill_memberships(
    skill: &Level2SkillRow,
    episodes: &[SkillEpisodeRow],
    created_at_unix_ms: i64,
) -> Result<SkillMaterialization, TrainerError> {
    skill.validate()?;
    if episodes.is_empty() {
        return Err(invalid("episodes", "must not be empty"));
    }
    if created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }

    let source_ids = skill
        .source_episode_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut membership_map: BTreeMap<String, ChunkSkillMembershipRow> = BTreeMap::new();
    let mut reverse_map: BTreeMap<String, ReverseAccumulator> = BTreeMap::new();
    let mut matched_ids = BTreeSet::new();

    for episode in episodes {
        if !source_ids.contains(&episode.episode_id) {
            continue;
        }
        if !matched_ids.insert(episode.episode_id.clone()) {
            return Err(invalid(
                "episodes",
                "duplicate source episode id in materialization input",
            ));
        }
        let provenance = provenance_hash(&episode.episode_id, &skill.skill_id);
        for step in &episode.ordered_steps {
            validate_step_against_skill(step, skill)?;
            let key = membership_key(&step.chunk_id, &step.code_state_key, &skill.skill_id)?;
            let row = membership_map.entry(key.clone()).or_insert_with(|| {
                let mut labels = step.accepted_label_ids.clone();
                labels.sort();
                ChunkSkillMembershipRow {
                    schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
                    membership_key: key,
                    chunk_id: step.chunk_id.clone(),
                    file_path: step.file_path.clone(),
                    code_state_key: step.code_state_key.clone(),
                    skill_id: skill.skill_id.clone(),
                    hierarchy_level: 2,
                    membership_score: (skill.confidence * skill.stability).clamp(0.0, 1.0),
                    source_accepted_label_ids: labels,
                    ordered_step_evidence: Vec::new(),
                    live_input_allowed: true,
                    provenance_hashes: Vec::new(),
                    first_seen_unix_ms: created_at_unix_ms,
                    last_seen_unix_ms: created_at_unix_ms,
                }
            });
            row.ordered_step_evidence.push(step.clone());
            merge_sorted_unique(&mut row.source_accepted_label_ids, &step.accepted_label_ids);
            if !row.provenance_hashes.contains(&provenance) {
                row.provenance_hashes.push(provenance.clone());
            }
            let reverse_key = reverse_index_key(&skill.skill_id, &step.code_state_key)?;
            reverse_map
                .entry(reverse_key.clone())
                .or_insert_with(|| ReverseAccumulator::new(reverse_key, &skill.skill_id, step))
                .push(step);
        }
    }
    if matched_ids.len() as u64 != skill.support || matched_ids != source_ids {
        return Err(invalid(
            "episodes",
            "matched source episode ids must equal skill support",
        ));
    }

    let mut chunk_memberships = membership_map.into_values().collect::<Vec<_>>();
    for row in &mut chunk_memberships {
        row.provenance_hashes.sort();
        row.validate()?;
    }
    let membership_hash_value = membership_hash(&chunk_memberships)?;
    let mut reverse_indexes = reverse_map
        .into_values()
        .map(|acc| acc.into_row(skill.support, membership_hash_value.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    for row in &mut reverse_indexes {
        row.validate()?;
    }
    let audit = SkillLifecycleAuditRow {
        schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
        skill_audit_id: lifecycle_audit_id_from_parts(
            Some(&skill.skill_id),
            SkillLifecycleDecision::CreateNewCandidateSkill,
            created_at_unix_ms,
        )?,
        prediction_id: None,
        mistake_id: None,
        previous_skill_id: None,
        decision: SkillLifecycleDecision::CreateNewCandidateSkill,
        candidate_skill_id: Some(skill.skill_id.clone()),
        evidence_label_ids: skill.prerequisite_label_ids.clone(),
        evidence_skill_ids: vec![skill.skill_id.clone()],
        evidence_higher_ability_ids: Vec::new(),
        evidence_chunk_ids: chunk_memberships
            .iter()
            .map(|row| row.chunk_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        source_membership_keys: chunk_memberships
            .iter()
            .map(|row| row.membership_key.clone())
            .collect(),
        reason: "materialized_from_ordered_skill_sequence".to_string(),
        created_at_unix_ms,
    };
    audit.validate()?;
    Ok(SkillMaterialization {
        level2_skills: vec![skill.clone()],
        chunk_memberships,
        reverse_indexes,
        lifecycle_audits: vec![audit],
    })
}

pub fn write_skill_materialization_sync_readback(
    db: &DB,
    materialization: &SkillMaterialization,
) -> Result<(), TrainerError> {
    for row in &materialization.level2_skills {
        row.validate()?;
    }
    for row in &materialization.chunk_memberships {
        row.validate()?;
    }
    for row in &materialization.reverse_indexes {
        row.validate()?;
    }
    for row in &materialization.lifecycle_audits {
        row.validate()?;
    }
    let level2_cf = cf(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?;
    let membership_cf = cf(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?;
    let reverse_cf = cf(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?;
    let audit_cf = cf(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?;
    let mut batch = WriteBatch::default();
    for row in &materialization.level2_skills {
        batch.put_cf(level2_cf, row.skill_id.as_bytes(), serialize(row)?);
    }
    for row in &materialization.chunk_memberships {
        batch.put_cf(
            membership_cf,
            row.membership_key.as_bytes(),
            serialize(row)?,
        );
    }
    for row in &materialization.reverse_indexes {
        batch.put_cf(
            reverse_cf,
            row.reverse_index_key.as_bytes(),
            serialize(row)?,
        );
    }
    for row in &materialization.lifecycle_audits {
        batch.put_cf(audit_cf, row.skill_audit_id.as_bytes(), serialize(row)?);
    }
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.write_opt(batch, &write_opts)
        .map_err(map_rocksdb_error)?;
    for cf_handle in [level2_cf, membership_cf, reverse_cf, audit_cf] {
        db.flush_cf(cf_handle).map_err(map_rocksdb_error)?;
    }
    for row in &materialization.level2_skills {
        let readback = read_level2_skill_row(db, &row.skill_id)?
            .ok_or_else(|| invalid("readback", "level2 skill row missing after write"))?;
        if readback != *row {
            return Err(invalid("readback", "level2 skill row changed"));
        }
    }
    for row in &materialization.chunk_memberships {
        let readback = read_chunk_skill_membership_row(db, &row.membership_key)?
            .ok_or_else(|| invalid("readback", "chunk skill membership row missing after write"))?;
        if readback != *row {
            return Err(invalid("readback", "chunk skill membership row changed"));
        }
    }
    for row in &materialization.reverse_indexes {
        let readback = read_skill_reverse_index_row(db, &row.reverse_index_key)?
            .ok_or_else(|| invalid("readback", "skill reverse index row missing after write"))?;
        if readback != *row {
            return Err(invalid("readback", "skill reverse index row changed"));
        }
    }
    for row in &materialization.lifecycle_audits {
        let readback = read_skill_lifecycle_audit_row(db, &row.skill_audit_id)?
            .ok_or_else(|| invalid("readback", "skill lifecycle audit row missing after write"))?;
        if readback != *row {
            return Err(invalid("readback", "skill lifecycle audit row changed"));
        }
    }
    Ok(())
}

pub fn read_level2_skill_row(
    db: &DB,
    skill_id: &str,
) -> Result<Option<Level2SkillRow>, TrainerError> {
    let row = read_row::<Level2SkillRow>(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, skill_id)?;
    if let Some(row) = &row {
        row.validate()?;
    }
    Ok(row)
}

pub fn read_all_level2_skill_rows(db: &DB) -> Result<Vec<Level2SkillRow>, TrainerError> {
    let cf_handle = cf(db, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid("level2_skill.key", err.to_string()))?;
        let row: Level2SkillRow = bincode::deserialize(&value).map_err(map_bincode_error)?;
        if key != row.skill_id {
            return Err(invalid("level2_skill.key", "key does not match payload"));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn read_chunk_skill_membership_row(
    db: &DB,
    membership_key: &str,
) -> Result<Option<ChunkSkillMembershipRow>, TrainerError> {
    let row =
        read_row::<ChunkSkillMembershipRow>(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, membership_key)?;
    if let Some(row) = &row {
        row.validate()?;
    }
    Ok(row)
}

pub fn read_skill_reverse_index_row(
    db: &DB,
    reverse_index_key: &str,
) -> Result<Option<SkillReverseIndexRow>, TrainerError> {
    let row =
        read_row::<SkillReverseIndexRow>(db, CF_MEJEPA_SKILL_REVERSE_INDEX, reverse_index_key)?;
    if let Some(row) = &row {
        row.validate()?;
    }
    Ok(row)
}

pub fn read_all_skill_reverse_index_rows(
    db: &DB,
) -> Result<Vec<SkillReverseIndexRow>, TrainerError> {
    let cf_handle = cf(db, CF_MEJEPA_SKILL_REVERSE_INDEX)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid("skill_reverse_index.key", err.to_string()))?;
        let row: SkillReverseIndexRow = bincode::deserialize(&value).map_err(map_bincode_error)?;
        if key != row.reverse_index_key {
            return Err(invalid(
                "skill_reverse_index.key",
                "key does not match payload",
            ));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn read_skill_lifecycle_audit_row(
    db: &DB,
    audit_id: &str,
) -> Result<Option<SkillLifecycleAuditRow>, TrainerError> {
    validate_id("audit_id", audit_id)?;
    let cf_handle = cf(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?;
    let Some(bytes) = db
        .get_cf(cf_handle, audit_id.as_bytes())
        .map_err(map_rocksdb_error)?
    else {
        return Ok(None);
    };
    let row = Some(decode_skill_lifecycle_audit_row(&bytes)?);
    if let Some(row) = &row {
        row.validate()?;
    }
    Ok(row)
}

pub fn read_all_chunk_skill_membership_rows(
    db: &DB,
) -> Result<Vec<ChunkSkillMembershipRow>, TrainerError> {
    let cf_handle = cf(db, CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid("chunk_skill_membership.key", err.to_string()))?;
        let row: ChunkSkillMembershipRow =
            bincode::deserialize(&value).map_err(map_bincode_error)?;
        if key != row.membership_key {
            return Err(invalid(
                "chunk_skill_membership.key",
                "key does not match payload",
            ));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn read_all_skill_lifecycle_audit_rows(
    db: &DB,
) -> Result<Vec<SkillLifecycleAuditRow>, TrainerError> {
    let cf_handle = cf(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf_handle, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        let key = String::from_utf8(key.to_vec())
            .map_err(|err| invalid("skill_lifecycle_audit.key", err.to_string()))?;
        let row = decode_skill_lifecycle_audit_row(&value)?;
        if key != row.skill_audit_id {
            return Err(invalid(
                "skill_lifecycle_audit.key",
                "key does not match payload",
            ));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn decode_skill_lifecycle_audit_row(bytes: &[u8]) -> Result<SkillLifecycleAuditRow, TrainerError> {
    bincode::deserialize::<SkillLifecycleAuditRow>(bytes)
        .or_else(|_| bincode::deserialize::<LegacySkillLifecycleAuditRow>(bytes).map(Into::into))
        .map_err(map_bincode_error)
}

pub fn write_skill_lifecycle_audit_row_sync_readback(
    db: &DB,
    row: &SkillLifecycleAuditRow,
) -> Result<(), TrainerError> {
    row.validate()?;
    let cf_handle = cf(db, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT)?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(
        cf_handle,
        row.skill_audit_id.as_bytes(),
        serialize(row)?,
        &write_opts,
    )
    .map_err(map_rocksdb_error)?;
    db.flush_cf(cf_handle).map_err(map_rocksdb_error)?;
    let readback = read_skill_lifecycle_audit_row(db, &row.skill_audit_id)?
        .ok_or_else(|| invalid("readback", "skill lifecycle audit row missing after write"))?;
    if readback != *row {
        return Err(invalid(
            "readback",
            "skill lifecycle audit row changed during write/readback",
        ));
    }
    Ok(())
}

pub fn count_cf_rows(db: &DB, cf_name: &str) -> Result<u64, TrainerError> {
    let cf = cf(db, cf_name)?;
    let mut count = 0_u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item.map_err(map_rocksdb_error)?;
        count += 1;
    }
    Ok(count)
}

pub fn membership_key(
    chunk_id: &str,
    code_state_key: &str,
    skill_id: &str,
) -> Result<String, TrainerError> {
    validate_id("chunk_id", chunk_id)?;
    validate_id("code_state_key", code_state_key)?;
    validate_id("skill_id", skill_id)?;
    Ok(format!(
        "chunk_skill::{chunk_id}::{code_state_key}::{skill_id}"
    ))
}

pub fn reverse_index_key(skill_id: &str, code_state_key: &str) -> Result<String, TrainerError> {
    validate_id("skill_id", skill_id)?;
    validate_id("code_state_key", code_state_key)?;
    Ok(format!("skill_reverse::{skill_id}::{code_state_key}"))
}

pub fn lifecycle_audit_id_from_parts(
    skill_id: Option<&str>,
    decision: SkillLifecycleDecision,
    created_at_unix_ms: i64,
) -> Result<String, TrainerError> {
    if let Some(value) = skill_id {
        validate_id("skill_id", value)?;
    }
    if created_at_unix_ms <= 0 {
        return Err(invalid("created_at_unix_ms", "must be positive"));
    }
    let mut hasher = Sha256::new();
    if let Some(value) = skill_id {
        hasher.update(value.as_bytes());
    }
    hasher.update(format!("{decision:?}").as_bytes());
    hasher.update(created_at_unix_ms.to_le_bytes());
    Ok(format!(
        "skill_audit:{}",
        &hex::encode(hasher.finalize())[..24]
    ))
}

fn validate_step_against_skill(
    step: &SkillStepEvidence,
    skill: &Level2SkillRow,
) -> Result<(), TrainerError> {
    validate_step_evidence(step)?;
    let template = skill
        .ordered_steps
        .get(step.step_index as usize)
        .ok_or_else(|| invalid("step_index", "step outside skill template"))?;
    let mut labels = step.accepted_label_ids.clone();
    labels.sort();
    let mut groups = step.group_ids.clone();
    groups.sort();
    if labels != template.accepted_label_ids || groups != template.group_ids {
        return Err(invalid(
            "ordered_step_evidence",
            "step labels/groups do not match skill template",
        ));
    }
    Ok(())
}

fn validate_step_evidence(step: &SkillStepEvidence) -> Result<(), TrainerError> {
    validate_id("chunk_id", &step.chunk_id)?;
    validate_project_relative_path("file_path", &step.file_path)?;
    validate_id("code_state_key", &step.code_state_key)?;
    validate_live_id_list(
        "step.accepted_label_ids",
        &step.accepted_label_ids,
        MAX_ROW_IDS,
    )?;
    validate_live_id_list("step.group_ids", &step.group_ids, MAX_ROW_IDS)?;
    Ok(())
}

fn read_row<T>(db: &DB, cf_name: &str, key: &str) -> Result<Option<T>, TrainerError>
where
    T: for<'de> Deserialize<'de>,
{
    validate_id("key", key)?;
    let cf = cf(db, cf_name)?;
    let Some(bytes) = db.get_cf(cf, key.as_bytes()).map_err(map_rocksdb_error)? else {
        return Ok(None);
    };
    bincode::deserialize(&bytes)
        .map(Some)
        .map_err(map_bincode_error)
}

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, TrainerError> {
    bincode::serialize(value).map_err(map_bincode_error)
}

fn cf<'a>(db: &'a DB, cf_name: &str) -> Result<&'a rocksdb::ColumnFamily, TrainerError> {
    db.cf_handle(cf_name)
        .ok_or_else(|| invalid("rocksdb.column_family", format!("missing {cf_name}")))
}

#[derive(Debug, Clone)]
struct ReverseAccumulator {
    reverse_index_key: String,
    skill_id: String,
    code_state_key: String,
    chunk_ids: BTreeSet<String>,
    file_paths: BTreeSet<String>,
    ordered_step_spans: Vec<OrderedStepSpan>,
}

impl ReverseAccumulator {
    fn new(key: String, skill_id: &str, step: &SkillStepEvidence) -> Self {
        Self {
            reverse_index_key: key,
            skill_id: skill_id.to_string(),
            code_state_key: step.code_state_key.clone(),
            chunk_ids: BTreeSet::new(),
            file_paths: BTreeSet::new(),
            ordered_step_spans: Vec::new(),
        }
    }

    fn push(&mut self, step: &SkillStepEvidence) {
        self.chunk_ids.insert(step.chunk_id.clone());
        self.file_paths.insert(step.file_path.clone());
        self.ordered_step_spans.push(OrderedStepSpan {
            step_index: step.step_index,
            chunk_id: step.chunk_id.clone(),
            file_path: step.file_path.clone(),
            code_state_key: step.code_state_key.clone(),
        });
    }

    fn into_row(
        self,
        support: u64,
        latest_membership_hash: String,
    ) -> Result<SkillReverseIndexRow, TrainerError> {
        let row = SkillReverseIndexRow {
            schema_version: SKILL_SEQUENCE_SCHEMA_VERSION,
            reverse_index_key: self.reverse_index_key,
            skill_id: self.skill_id,
            code_state_key: self.code_state_key,
            chunk_ids: self.chunk_ids.into_iter().collect(),
            file_paths: self.file_paths.into_iter().collect(),
            ordered_step_spans: self.ordered_step_spans,
            support,
            latest_membership_hash,
        };
        row.validate()?;
        Ok(row)
    }
}

fn membership_hash(rows: &[ChunkSkillMembershipRow]) -> Result<String, TrainerError> {
    if rows.is_empty() {
        return Err(invalid("chunk_memberships", "must not be empty"));
    }
    let mut row_hashes = rows
        .iter()
        .map(|row| {
            let bytes = serialize(row)?;
            let digest = hex::encode(Sha256::digest(&bytes));
            Ok(format!("{}:{digest}", row.membership_key))
        })
        .collect::<Result<Vec<_>, TrainerError>>()?;
    row_hashes.sort();
    let mut hasher = Sha256::new();
    for row_hash in row_hashes {
        hasher.update(row_hash.as_bytes());
        hasher.update([0]);
    }
    Ok(format!(
        "membership_hash:{}",
        hex::encode(hasher.finalize())
    ))
}

fn provenance_hash(episode_id: &str, skill_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(episode_id.as_bytes());
    hasher.update([0]);
    hasher.update(skill_id.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn merge_sorted_unique(target: &mut Vec<String>, incoming: &[String]) {
    target.extend(incoming.iter().cloned());
    target.sort();
    target.dedup();
}

fn validate_schema(schema_version: u32) -> Result<(), TrainerError> {
    if schema_version != SKILL_SEQUENCE_SCHEMA_VERSION {
        return Err(invalid(
            "schema_version",
            format!(
                "expected {}, got {schema_version}",
                SKILL_SEQUENCE_SCHEMA_VERSION
            ),
        ));
    }
    Ok(())
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

fn validate_path_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_path_list(SOURCE_FILE, REMEDIATION, field, values, max_items)
}

fn validate_project_relative_path(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_project_relative_path(SOURCE_FILE, REMEDIATION, field, value)
}

fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    skill_validation::validate_id(SOURCE_FILE, REMEDIATION, field, value)
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

#[cfg(test)]
#[path = "chunk_skill_membership_tests.rs"]
mod chunk_skill_membership_tests;
