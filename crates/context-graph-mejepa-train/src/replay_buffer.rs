//! TASK-RWD-210 / TASK-PY-G-068 prioritized replay buffer.
//!
//! The replay buffer is durable training pressure for M(t) consolidation. Rows
//! are keyed by `prediction_id` in `CF_MEJEPA_REPLAY_BUFFER`; sampling updates
//! the source-of-truth row before returning the batch report.

use crate::error::{TrainerError, TrainerErrorCode};
use crate::label_bridge::{
    ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
    skill_signature_hash,
};
use crate::skill_validation;
use context_graph_mejepa::{PredictionId, OPERATOR_OVERRIDE_SAMPLING_WEIGHT};
use context_graph_mejepa_cf::CF_MEJEPA_REPLAY_BUFFER;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;

const MAX_CELL_ID_BYTES: usize = 512;
const MAX_LABEL_IDS: usize = 256;
const MAX_SKILL_IDS: usize = 128;
const MAX_HIGHER_ABILITY_IDS: usize = 128;
const MAX_SOURCE_MEMBERSHIP_KEYS: usize = 512;
pub const DEFAULT_REPLAY_TTL_DAYS: i64 = 90;
pub const DEFAULT_REPLAY_COLD_TIER_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplayPriorityConfig {
    pub alpha_surprise: f32,
    pub beta_cell_coverage_gap: f32,
    pub gamma_replay_count_decay: f32,
}

impl Default for ReplayPriorityConfig {
    fn default() -> Self {
        Self {
            alpha_surprise: 0.5,
            beta_cell_coverage_gap: 0.3,
            gamma_replay_count_decay: 0.2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ReplayBufferSource {
    OperatorOverride,
    AgentSurprise,
    DriftFlaggedCell,
    AdversarialCorpus,
    UnknownFingerprint,
    SchedulerBackfill,
    AcceptedLabelMistake,
    ConstellationSkillMistake,
}

impl ReplayBufferSource {
    pub fn replay_weight_multiplier(self) -> f32 {
        match self {
            Self::OperatorOverride => OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
            Self::DriftFlaggedCell => 3.0,
            Self::AdversarialCorpus => 4.0,
            Self::AcceptedLabelMistake => 8.0,
            Self::ConstellationSkillMistake => 8.0,
            Self::AgentSurprise | Self::UnknownFingerprint | Self::SchedulerBackfill => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ReplayRetentionTier {
    Hot,
    Cold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum ReplayConsolidationWindow {
    Normal,
    EmaSwa,
    Distillation,
    CatastrophicPromotion { ticks_since_promotion: u32 },
}

impl ReplayConsolidationWindow {
    pub fn k_replay(self) -> u32 {
        match self {
            Self::Normal => 1,
            Self::EmaSwa => 2,
            Self::Distillation => 3,
            Self::CatastrophicPromotion {
                ticks_since_promotion,
            } if ticks_since_promotion < 100 => 4,
            Self::CatastrophicPromotion { .. } => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplayBufferRow {
    pub prediction_id: PredictionId,
    pub surprise_z: f32,
    pub cell_id: String,
    pub coverage_gap_score: f32,
    pub last_replayed_ts: Option<i64>,
    pub replay_count: u64,
    pub retention_weight: f32,
    pub protected: bool,
    pub retention_tier: ReplayRetentionTier,
    pub source: ReplayBufferSource,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    #[serde(default)]
    pub accepted_label_ids: Vec<String>,
    #[serde(default)]
    pub active_skill_ids: Vec<String>,
    #[serde(default)]
    pub active_higher_ability_ids: Vec<String>,
    #[serde(default)]
    pub source_membership_keys: Vec<String>,
    #[serde(default)]
    pub label_signature_hash: Option<String>,
    #[serde(default)]
    pub skill_signature_hash: Option<String>,
    #[serde(default)]
    pub ability_signature_hash: Option<String>,
    #[serde(default)]
    pub membership_signature_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LegacyReplayBufferRow {
    prediction_id: PredictionId,
    surprise_z: f32,
    cell_id: String,
    coverage_gap_score: f32,
    last_replayed_ts: Option<i64>,
    replay_count: u64,
    retention_weight: f32,
    protected: bool,
    retention_tier: ReplayRetentionTier,
    source: ReplayBufferSource,
    created_at_unix_ms: i64,
    updated_at_unix_ms: i64,
}

impl From<LegacyReplayBufferRow> for ReplayBufferRow {
    fn from(value: LegacyReplayBufferRow) -> Self {
        Self {
            prediction_id: value.prediction_id,
            surprise_z: value.surprise_z,
            cell_id: value.cell_id,
            coverage_gap_score: value.coverage_gap_score,
            last_replayed_ts: value.last_replayed_ts,
            replay_count: value.replay_count,
            retention_weight: value.retention_weight,
            protected: value.protected,
            retention_tier: value.retention_tier,
            source: value.source,
            created_at_unix_ms: value.created_at_unix_ms,
            updated_at_unix_ms: value.updated_at_unix_ms,
            accepted_label_ids: Vec::new(),
            active_skill_ids: Vec::new(),
            active_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            label_signature_hash: None,
            skill_signature_hash: None,
            ability_signature_hash: None,
            membership_signature_hash: None,
        }
    }
}

impl ReplayBufferRow {
    pub fn priority_score(&self, config: ReplayPriorityConfig) -> Result<f32, TrainerError> {
        self.validate()?;
        validate_priority_config(config)?;
        let replay_decay = config.gamma_replay_count_decay / (1.0 + self.replay_count as f32);
        let base_score = config.alpha_surprise * self.surprise_z
            + config.beta_cell_coverage_gap * self.coverage_gap_score
            + replay_decay;
        let score = base_score * self.source.replay_weight_multiplier() * self.retention_weight;
        if !score.is_finite() {
            return Err(degenerate_priority(
                "priority_score",
                "priority formula produced a non-finite value",
            ));
        }
        Ok(score)
    }

    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.prediction_id.0 == [0_u8; 16] {
            return Err(invalid("prediction_id", "must be non-zero"));
        }
        validate_finite("surprise_z", self.surprise_z)?;
        validate_unit("coverage_gap_score", self.coverage_gap_score)?;
        validate_finite("retention_weight", self.retention_weight)?;
        if self.retention_weight < 0.0 {
            return Err(invalid("retention_weight", "must be non-negative"));
        }
        validate_cell_id(&self.cell_id)?;
        validate_label_payload(self)?;
        if self.created_at_unix_ms <= 0 || self.updated_at_unix_ms <= 0 {
            return Err(invalid(
                "timestamps",
                "created/updated timestamps must be positive",
            ));
        }
        if let Some(ts) = self.last_replayed_ts {
            if ts <= 0 {
                return Err(invalid("last_replayed_ts", "must be positive when present"));
            }
        }
        Ok(())
    }
}

fn validate_label_payload(row: &ReplayBufferRow) -> Result<(), TrainerError> {
    validate_live_id_list("accepted_label_ids", &row.accepted_label_ids, MAX_LABEL_IDS)?;
    validate_id_list("active_skill_ids", &row.active_skill_ids, MAX_SKILL_IDS)?;
    validate_id_list(
        "active_higher_ability_ids",
        &row.active_higher_ability_ids,
        MAX_HIGHER_ABILITY_IDS,
    )?;
    validate_id_list(
        "source_membership_keys",
        &row.source_membership_keys,
        MAX_SOURCE_MEMBERSHIP_KEYS,
    )?;
    let expected_label_signature = if row.accepted_label_ids.is_empty() {
        None
    } else {
        Some(accepted_label_signature_hash(&row.accepted_label_ids)?)
    };
    if row.label_signature_hash != expected_label_signature {
        return Err(invalid(
            "label_signature_hash",
            "does not match accepted_label_ids",
        ));
    }
    let expected_skill_signature = if row.active_skill_ids.is_empty() {
        None
    } else {
        Some(skill_signature_hash(&row.active_skill_ids)?)
    };
    if row.skill_signature_hash != expected_skill_signature {
        return Err(invalid(
            "skill_signature_hash",
            "does not match active_skill_ids",
        ));
    }
    let expected_ability_signature = if row.active_higher_ability_ids.is_empty() {
        None
    } else {
        Some(ability_signature_hash(&row.active_higher_ability_ids)?)
    };
    if row.ability_signature_hash != expected_ability_signature {
        return Err(invalid(
            "ability_signature_hash",
            "does not match active_higher_ability_ids",
        ));
    }
    let expected_membership_signature = if row.source_membership_keys.is_empty() {
        None
    } else {
        Some(membership_signature_hash(&row.source_membership_keys)?)
    };
    if row.membership_signature_hash != expected_membership_signature {
        return Err(invalid(
            "membership_signature_hash",
            "does not match source_membership_keys",
        ));
    }
    for (field, value) in [
        ("label_signature_hash", &row.label_signature_hash),
        ("skill_signature_hash", &row.skill_signature_hash),
        ("ability_signature_hash", &row.ability_signature_hash),
        ("membership_signature_hash", &row.membership_signature_hash),
    ] {
        if let Some(value) = value {
            validate_cell_component(field, value)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScoredReplayRow {
    pub row: ReplayBufferRow,
    pub priority_score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplayBatchRequest {
    pub top_k: usize,
    pub window: ReplayConsolidationWindow,
    pub now_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplayBatchReport {
    pub requested_top_k: usize,
    pub k_replay: u32,
    pub requested_rows: usize,
    pub selected_rows: Vec<ScoredReplayRow>,
    pub replay_rows_sampled_total: BTreeMap<String, u64>,
    pub empty_buffer_fresh_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplayRetentionReport {
    pub rows_before: usize,
    pub rows_after: usize,
    pub expired_deleted: Vec<String>,
    pub quota_deleted: Vec<String>,
    pub cold_tiered: Vec<String>,
    pub protected_preserved: Vec<String>,
    pub quota_still_exceeded_by_protected_count: bool,
}

pub fn write_replay_row_sync_readback(db: &DB, row: &ReplayBufferRow) -> Result<(), TrainerError> {
    row.validate()?;
    let cf = replay_cf(db)?;
    let bytes = bincode::serialize(row).map_err(map_bincode_error)?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, row.prediction_id.0, bytes, &write_opts)
        .map_err(map_rocksdb_error)?;
    db.flush_cf(cf).map_err(map_rocksdb_error)?;
    let readback = read_replay_row(db, row.prediction_id)?
        .ok_or_else(|| invalid("readback", "replay row missing after write"))?;
    if readback != *row {
        return Err(invalid(
            "readback",
            "replay row changed during write/readback",
        ));
    }
    Ok(())
}

pub fn read_replay_row(
    db: &DB,
    prediction_id: PredictionId,
) -> Result<Option<ReplayBufferRow>, TrainerError> {
    if prediction_id.0 == [0_u8; 16] {
        return Err(invalid("prediction_id", "must be non-zero"));
    }
    let cf = replay_cf(db)?;
    let Some(bytes) = db.get_cf(cf, prediction_id.0).map_err(map_rocksdb_error)? else {
        return Ok(None);
    };
    let row = decode_replay_buffer_row(&bytes)?;
    row.validate()?;
    Ok(Some(row))
}

pub fn read_all_replay_rows(db: &DB) -> Result<Vec<ReplayBufferRow>, TrainerError> {
    let cf = replay_cf(db)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item.map_err(map_rocksdb_error)?;
        if key.len() != 16 {
            return Err(invalid(
                "replay_buffer.key",
                format!("expected 16-byte prediction_id key, got {}", key.len()),
            ));
        }
        let row = decode_replay_buffer_row(&value)?;
        if key.as_ref() != &row.prediction_id.0[..] {
            return Err(invalid(
                "replay_buffer.key",
                "key does not match payload prediction_id",
            ));
        }
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

fn decode_replay_buffer_row(bytes: &[u8]) -> Result<ReplayBufferRow, TrainerError> {
    bincode::deserialize::<ReplayBufferRow>(bytes)
        .or_else(|_| bincode::deserialize::<LegacyReplayBufferRow>(bytes).map(Into::into))
        .map_err(map_bincode_error)
}

pub fn count_replay_rows(db: &DB) -> Result<u64, TrainerError> {
    Ok(read_all_replay_rows(db)?.len() as u64)
}

pub fn sample_replay_batch(
    db: &DB,
    request: ReplayBatchRequest,
    config: ReplayPriorityConfig,
) -> Result<ReplayBatchReport, TrainerError> {
    validate_batch_request(request)?;
    let mut scored = scored_rows(db, config)?;
    let k_replay = request.window.k_replay();
    let requested_rows = request
        .top_k
        .checked_mul(k_replay as usize)
        .ok_or_else(|| invalid("top_k", "top_k * k_replay overflowed usize"))?;
    if scored.is_empty() {
        return Ok(ReplayBatchReport {
            requested_top_k: request.top_k,
            k_replay,
            requested_rows,
            selected_rows: Vec::new(),
            replay_rows_sampled_total: BTreeMap::new(),
            empty_buffer_fresh_only: true,
        });
    }
    scored.sort_by(compare_scored_desc);
    let selected = scored.into_iter().take(requested_rows).collect::<Vec<_>>();
    let mut counters = BTreeMap::new();
    for scored_row in &selected {
        let mut row = scored_row.row.clone();
        row.replay_count = row.replay_count.saturating_add(1);
        row.last_replayed_ts = Some(request.now_unix_ms);
        row.updated_at_unix_ms = request.now_unix_ms;
        write_replay_row_sync_readback(db, &row)?;
        *counters.entry(row.cell_id.clone()).or_insert(0) += 1;
    }
    Ok(ReplayBatchReport {
        requested_top_k: request.top_k,
        k_replay,
        requested_rows,
        selected_rows: selected,
        replay_rows_sampled_total: counters,
        empty_buffer_fresh_only: false,
    })
}

pub fn enforce_replay_buffer_quota(
    db: &DB,
    max_rows: usize,
    now_unix_ms: i64,
    config: ReplayPriorityConfig,
) -> Result<ReplayRetentionReport, TrainerError> {
    if now_unix_ms <= 0 {
        return Err(invalid("now_unix_ms", "must be positive"));
    }
    let mut rows = read_all_replay_rows(db)?;
    let rows_before = rows.len();
    let mut expired_deleted = delete_expired_rows(db, &mut rows, now_unix_ms)?;
    let cold_tiered = tier_down_cold_rows(db, &mut rows, now_unix_ms)?;
    let quota_deleted = delete_lowest_priority_to_quota(db, &mut rows, max_rows, config)?;
    let protected_preserved = rows
        .iter()
        .filter(|row| row.protected)
        .map(|row| prediction_hex(row.prediction_id))
        .collect::<Vec<_>>();
    expired_deleted.sort();
    Ok(ReplayRetentionReport {
        rows_before,
        rows_after: rows.len(),
        expired_deleted,
        quota_deleted,
        cold_tiered,
        protected_preserved,
        quota_still_exceeded_by_protected_count: rows.len() > max_rows,
    })
}

fn scored_rows(
    db: &DB,
    config: ReplayPriorityConfig,
) -> Result<Vec<ScoredReplayRow>, TrainerError> {
    read_all_replay_rows(db)?
        .into_iter()
        .map(|row| {
            let priority_score = row.priority_score(config)?;
            Ok(ScoredReplayRow {
                row,
                priority_score,
            })
        })
        .collect()
}

fn delete_expired_rows(
    db: &DB,
    rows: &mut Vec<ReplayBufferRow>,
    now_unix_ms: i64,
) -> Result<Vec<String>, TrainerError> {
    let mut deleted = Vec::new();
    let mut retained = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if !row.protected && age_days(&row, now_unix_ms)? >= DEFAULT_REPLAY_TTL_DAYS {
            delete_replay_row(db, row.prediction_id)?;
            deleted.push(prediction_hex(row.prediction_id));
        } else {
            retained.push(row);
        }
    }
    *rows = retained;
    Ok(deleted)
}

fn tier_down_cold_rows(
    db: &DB,
    rows: &mut [ReplayBufferRow],
    now_unix_ms: i64,
) -> Result<Vec<String>, TrainerError> {
    let mut cold_tiered = Vec::new();
    for row in rows {
        if row.retention_tier == ReplayRetentionTier::Hot
            && age_days(row, now_unix_ms)? >= DEFAULT_REPLAY_COLD_TIER_DAYS
        {
            row.retention_tier = ReplayRetentionTier::Cold;
            row.updated_at_unix_ms = now_unix_ms;
            write_replay_row_sync_readback(db, row)?;
            cold_tiered.push(prediction_hex(row.prediction_id));
        }
    }
    Ok(cold_tiered)
}

fn delete_lowest_priority_to_quota(
    db: &DB,
    rows: &mut Vec<ReplayBufferRow>,
    max_rows: usize,
    config: ReplayPriorityConfig,
) -> Result<Vec<String>, TrainerError> {
    if rows.len() <= max_rows {
        return Ok(Vec::new());
    }
    let mut candidates = rows
        .iter()
        .filter(|row| !row.protected)
        .map(|row| Ok((row.prediction_id, row.priority_score(config)?)))
        .collect::<Result<Vec<_>, TrainerError>>()?;
    candidates.sort_by(compare_priority_asc);
    let delete_count = rows.len().saturating_sub(max_rows).min(candidates.len());
    let deleted_ids = candidates
        .into_iter()
        .take(delete_count)
        .map(|(prediction_id, _)| prediction_id)
        .collect::<Vec<_>>();
    for prediction_id in &deleted_ids {
        delete_replay_row(db, *prediction_id)?;
    }
    rows.retain(|row| !deleted_ids.contains(&row.prediction_id));
    Ok(deleted_ids.into_iter().map(prediction_hex).collect())
}

fn delete_replay_row(db: &DB, prediction_id: PredictionId) -> Result<(), TrainerError> {
    let cf = replay_cf(db)?;
    db.delete_cf(cf, prediction_id.0)
        .map_err(map_rocksdb_error)?;
    db.flush_cf(cf).map_err(map_rocksdb_error)
}

fn compare_scored_desc(a: &ScoredReplayRow, b: &ScoredReplayRow) -> Ordering {
    b.priority_score
        .partial_cmp(&a.priority_score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| a.row.prediction_id.cmp(&b.row.prediction_id))
}

fn compare_priority_asc(a: &(PredictionId, f32), b: &(PredictionId, f32)) -> Ordering {
    a.1.partial_cmp(&b.1)
        .unwrap_or(Ordering::Equal)
        .then_with(|| a.0.cmp(&b.0))
}

fn age_days(row: &ReplayBufferRow, now_unix_ms: i64) -> Result<i64, TrainerError> {
    if now_unix_ms < row.created_at_unix_ms {
        return Err(invalid(
            "now_unix_ms",
            "cannot be earlier than row.created_at_unix_ms",
        ));
    }
    Ok((now_unix_ms - row.created_at_unix_ms) / 86_400_000)
}

fn validate_batch_request(request: ReplayBatchRequest) -> Result<(), TrainerError> {
    if request.top_k == 0 {
        return Err(invalid("top_k", "must be positive"));
    }
    if request.now_unix_ms <= 0 {
        return Err(invalid("now_unix_ms", "must be positive"));
    }
    Ok(())
}

fn validate_priority_config(config: ReplayPriorityConfig) -> Result<(), TrainerError> {
    for (field, value) in [
        ("alpha_surprise", config.alpha_surprise),
        ("beta_cell_coverage_gap", config.beta_cell_coverage_gap),
        ("gamma_replay_count_decay", config.gamma_replay_count_decay),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(invalid(field, "must be finite and non-negative"));
        }
    }
    let sum =
        config.alpha_surprise + config.beta_cell_coverage_gap + config.gamma_replay_count_decay;
    if sum <= f32::EPSILON {
        return Err(invalid(
            "priority_config",
            "at least one priority coefficient must be positive",
        ));
    }
    Ok(())
}

fn validate_finite(field: &'static str, value: f32) -> Result<(), TrainerError> {
    if !value.is_finite() {
        return Err(degenerate_priority(field, "value must be finite"));
    }
    Ok(())
}

fn validate_unit(field: &'static str, value: f32) -> Result<(), TrainerError> {
    validate_finite(field, value)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(invalid(field, "must be in [0, 1]"));
    }
    Ok(())
}

fn validate_cell_id(cell_id: &str) -> Result<(), TrainerError> {
    if cell_id.trim().is_empty() {
        return Err(invalid("cell_id", "must be non-empty"));
    }
    if cell_id.len() > MAX_CELL_ID_BYTES || cell_id.chars().any(char::is_control) {
        return Err(invalid(
            "cell_id",
            "must be single-line text up to 512 bytes",
        ));
    }
    Ok(())
}

fn validate_id_list(field: &str, values: &[String], max_items: usize) -> Result<(), TrainerError> {
    skill_validation::validate_id_list(
        "file:crates/context-graph-mejepa-train/src/replay_buffer.rs",
        "replay rows must preserve label, skill, ability, and membership identities for mistake-driven learning",
        field,
        values,
        max_items,
    )
}

fn validate_live_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    skill_validation::validate_live_id_list(
        "file:crates/context-graph-mejepa-train/src/replay_buffer.rs",
        "target-side supervision labels must not become live replay inputs",
        field,
        values,
        max_items,
    )
}

fn replay_cf(db: &DB) -> Result<&rocksdb::ColumnFamily, TrainerError> {
    db.cf_handle(CF_MEJEPA_REPLAY_BUFFER)
        .ok_or_else(|| invalid("rocksdb.column_family", "missing CF_MEJEPA_REPLAY_BUFFER"))
}

fn invalid(field: &'static str, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field,
        "file": "file:crates/context-graph-mejepa-train/src/replay_buffer.rs",
        "remediation": "fix the replay-buffer input and retry; replay state must fail closed"
    }))
}

fn degenerate_priority(field: &'static str, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::ReplayBufferDegeneratePriority, message).with_context(json!({
        "field": field,
        "file": "file:crates/context-graph-mejepa-train/src/replay_buffer.rs",
        "remediation": "drop or quarantine the replay row; non-finite priorities must not enter training"
    }))
}

fn map_rocksdb_error(err: rocksdb::Error) -> TrainerError {
    invalid("rocksdb", err.to_string())
}

fn map_bincode_error(err: Box<bincode::ErrorKind>) -> TrainerError {
    invalid("bincode", err.to_string())
}

pub fn prediction_hex(prediction_id: PredictionId) -> String {
    hex::encode(prediction_id.0)
}

pub fn operator_override_replay_weight(operator_override: bool) -> f32 {
    if operator_override {
        OPERATOR_OVERRIDE_SAMPLING_WEIGHT
    } else {
        1.0
    }
}

pub fn label_aware_replay_cell_id(
    language: &str,
    mutation_or_live_cell: &str,
    code_state_key: &str,
    named_failure_mode: &str,
    label_signature_hash: &str,
    skill_signature_hash: Option<&str>,
) -> Result<String, TrainerError> {
    ability_aware_replay_cell_id(
        language,
        mutation_or_live_cell,
        code_state_key,
        named_failure_mode,
        label_signature_hash,
        skill_signature_hash,
        None,
        None,
    )
}

pub fn ability_aware_replay_cell_id(
    language: &str,
    mutation_or_live_cell: &str,
    code_state_key: &str,
    named_failure_mode: &str,
    label_signature_hash: &str,
    skill_signature_hash: Option<&str>,
    ability_signature_hash: Option<&str>,
    membership_signature_hash: Option<&str>,
) -> Result<String, TrainerError> {
    for (field, value) in [
        ("language", language),
        ("mutation_or_live_cell", mutation_or_live_cell),
        ("code_state_key", code_state_key),
        ("named_failure_mode", named_failure_mode),
        ("label_signature_hash", label_signature_hash),
    ] {
        validate_cell_component(field, value)?;
    }
    if let Some(value) = skill_signature_hash {
        validate_cell_component("skill_signature_hash", value)?;
    }
    if let Some(value) = ability_signature_hash {
        validate_cell_component("ability_signature_hash", value)?;
    }
    if let Some(value) = membership_signature_hash {
        validate_cell_component("membership_signature_hash", value)?;
    }
    let code_state = short_hash(code_state_key);
    let failure_mode = compact_component(named_failure_mode);
    let mut cell_id = format!(
        "{}:{}:state:{}:mode:{}:{}",
        compact_component(language),
        compact_component(mutation_or_live_cell),
        code_state,
        failure_mode,
        compact_component(label_signature_hash)
    );
    if let Some(value) = skill_signature_hash {
        cell_id.push(':');
        cell_id.push_str(&compact_component(value));
    }
    if let Some(value) = ability_signature_hash {
        cell_id.push(':');
        cell_id.push_str(&compact_component(value));
    }
    if let Some(value) = membership_signature_hash {
        cell_id.push(':');
        cell_id.push_str(&compact_component(value));
    }
    validate_cell_id(&cell_id)?;
    Ok(cell_id)
}

fn compact_component(value: &str) -> String {
    let mut compact = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if compact.len() > 96 {
        compact = short_hash(value);
    }
    compact
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hex::encode(hasher.finalize());
    digest[..12].to_string()
}

fn validate_cell_component(field: &'static str, value: &str) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must be non-empty"));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid(field, "must be single-line text"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(byte: u8, surprise_z: f32, replay_count: u64) -> ReplayBufferRow {
        ReplayBufferRow {
            prediction_id: PredictionId([byte; 16]),
            surprise_z,
            cell_id: "off_by_one::python".to_string(),
            coverage_gap_score: 0.5,
            accepted_label_ids: Vec::new(),
            active_skill_ids: Vec::new(),
            active_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            label_signature_hash: None,
            skill_signature_hash: None,
            ability_signature_hash: None,
            membership_signature_hash: None,
            last_replayed_ts: None,
            replay_count,
            retention_weight: 1.0,
            protected: false,
            retention_tier: ReplayRetentionTier::Hot,
            source: ReplayBufferSource::AgentSurprise,
            created_at_unix_ms: 1_779_000_000_000,
            updated_at_unix_ms: 1_779_000_000_000,
        }
    }

    #[test]
    fn priority_uses_expected_weights() {
        let score = row(1, 2.0, 1)
            .priority_score(ReplayPriorityConfig::default())
            .unwrap();
        assert!((score - 1.25).abs() < 1e-6);
    }

    #[test]
    fn operator_override_source_applies_six_x_replay_weight() {
        let base = row(1, 2.0, 1)
            .priority_score(ReplayPriorityConfig::default())
            .unwrap();
        let mut override_row = row(2, 2.0, 1);
        override_row.source = ReplayBufferSource::OperatorOverride;
        let boosted = override_row
            .priority_score(ReplayPriorityConfig::default())
            .unwrap();
        assert!((boosted / base - OPERATOR_OVERRIDE_SAMPLING_WEIGHT).abs() < 1e-6);
    }

    #[test]
    fn accepted_label_mistake_source_applies_eight_x_replay_weight() {
        let base = row(1, 2.0, 1)
            .priority_score(ReplayPriorityConfig::default())
            .unwrap();
        let mut label_row = row(2, 2.0, 1);
        label_row.source = ReplayBufferSource::AcceptedLabelMistake;
        let boosted = label_row
            .priority_score(ReplayPriorityConfig::default())
            .unwrap();
        assert!((boosted / base - 8.0).abs() < 1e-6);
    }

    #[test]
    fn label_aware_cell_id_keeps_compact_label_and_skill_signatures() {
        let cell_id = label_aware_replay_cell_id(
            "python",
            "swap_variable",
            "task:workspace:chunk:source-sha",
            "failure:multi_point",
            "labels:abc123def456",
            Some("skills:def456abc123"),
        )
        .unwrap();

        assert!(cell_id.contains("python:swap_variable"));
        assert!(cell_id.contains("labels:abc123def456"));
        assert!(cell_id.contains("skills:def456abc123"));
        assert!(cell_id.len() <= MAX_CELL_ID_BYTES);
    }

    #[test]
    fn ability_aware_cell_id_includes_ability_and_membership_signatures() {
        let cell_id = ability_aware_replay_cell_id(
            "python",
            "live_project",
            "state:unit",
            "failure:runtime_ability",
            "labels:abc123def456",
            Some("skills:def456abc123"),
            Some("abilities:fedcba654321"),
            Some("memberships:456abc123def"),
        )
        .unwrap();

        assert!(cell_id.contains("skills:def456abc123"));
        assert!(cell_id.contains("abilities:fedcba654321"));
        assert!(cell_id.contains("memberships:456abc123def"));
        assert!(cell_id.len() <= MAX_CELL_ID_BYTES);
    }

    #[test]
    fn nan_surprise_fails_closed_with_task_code() {
        let err = row(1, f32::NAN, 0).validate().unwrap_err();
        assert_eq!(err.code(), "REPLAY_BUFFER_DEGENERATE_PRIORITY");
    }

    #[test]
    fn catastrophic_window_multiplier_expires_after_100_ticks() {
        assert_eq!(
            ReplayConsolidationWindow::CatastrophicPromotion {
                ticks_since_promotion: 99
            }
            .k_replay(),
            4
        );
        assert_eq!(
            ReplayConsolidationWindow::CatastrophicPromotion {
                ticks_since_promotion: 100
            }
            .k_replay(),
            1
        );
    }

    #[test]
    fn replay_row_deserializes_legacy_rows_with_empty_label_context() {
        let legacy = LegacyReplayBufferRow {
            prediction_id: PredictionId([0x3c; 16]),
            surprise_z: 1.0,
            cell_id: "python:legacy".to_string(),
            coverage_gap_score: 0.25,
            last_replayed_ts: None,
            replay_count: 2,
            retention_weight: 1.0,
            protected: false,
            retention_tier: ReplayRetentionTier::Hot,
            source: ReplayBufferSource::AgentSurprise,
            created_at_unix_ms: 1_778_000_000_000,
            updated_at_unix_ms: 1_778_000_000_001,
        };
        let bytes = bincode::serialize(&legacy).unwrap();
        let decoded = decode_replay_buffer_row(&bytes).unwrap();

        decoded.validate().unwrap();
        assert!(decoded.accepted_label_ids.is_empty());
        assert!(decoded.active_higher_ability_ids.is_empty());
        assert!(decoded.source_membership_keys.is_empty());
    }
}
