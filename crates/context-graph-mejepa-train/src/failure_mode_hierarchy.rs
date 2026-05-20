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
                labels.extend(row.source_accepted_label_ids.into_iter());
                evidence.extend(row.ordered_step_evidence.into_iter());
                provenance.extend(row.provenance_hashes.into_iter());
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
