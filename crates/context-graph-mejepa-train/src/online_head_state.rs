use crate::error::TrainerError;
use crate::mistake_log::MistakeLogRow;
pub use crate::online_head_state_keys::{
    online_head_key, repeat_metric_key, unrelated_control_panel_signature_hash,
};
use crate::online_head_state_support::{
    invalid, map_bincode_error, map_rocksdb_error, online_head_cf, validate_id,
    validate_optional_id,
};
use crate::replay_buffer::{ReplayBufferRow, ReplayPriorityConfig};
use context_graph_mejepa::{PredictionId, Verdict};
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};

const ONLINE_HEAD_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadStateRow {
    pub schema_version: u32,
    pub head_key: String,
    pub panel_signature_hash: String,
    pub replay_cell_id: String,
    pub label_signature_hash: String,
    pub skill_signature_hash: Option<String>,
    pub ability_signature_hash: Option<String>,
    pub membership_signature_hash: Option<String>,
    pub support_count: u64,
    pub mistake_count: u64,
    pub correction_bias: f32,
    pub corrected_verdict: Verdict,
    pub last_prediction_id: PredictionId,
    pub last_mistake_id: String,
    pub last_replay_row_key: String,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadRepeatMetricRow {
    pub schema_version: u32,
    pub metric_key: String,
    pub replay_cell_id: String,
    pub label_signature_hash: String,
    pub skill_signature_hash: Option<String>,
    pub ability_signature_hash: Option<String>,
    pub membership_signature_hash: Option<String>,
    pub window_size: u64,
    pub mistake_count: u64,
    pub repeated_mistake_count: u64,
    pub mistake_repeat_rate: f32,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadUpdateConfig {
    pub learning_rate: f32,
    pub repeat_window_size: u64,
}

impl Default for OnlineHeadUpdateConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.25,
            repeat_window_size: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadUpdateInput {
    pub panel_signature_hash: String,
    pub mistake_row: MistakeLogRow,
    pub replay_row: ReplayBufferRow,
    pub base_verdict_before_update: Verdict,
    pub now_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadUpdateReport {
    pub schema_version: u32,
    pub head_state_before: Option<OnlineHeadStateRow>,
    pub head_state_after: OnlineHeadStateRow,
    pub repeat_metric_after: OnlineHeadRepeatMetricRow,
    pub base_verdict_before_update: Verdict,
    pub corrected_verdict_after_update: Verdict,
    pub replay_priority_score_after: f32,
    pub unrelated_cell_control_panel_signature_hash: String,
    pub unrelated_cell_verdict_before_update: Verdict,
    pub unrelated_cell_verdict_after_update: Verdict,
    pub unrelated_cell_accuracy_delta: f32,
    pub repeat_metric_byte_readable: bool,
    pub same_panel_signature_corrected: bool,
    pub label_skill_ability_membership_ids_agree: bool,
    pub flat_vector_concat_used: bool,
}

impl OnlineHeadStateRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.schema_version != ONLINE_HEAD_STATE_SCHEMA_VERSION {
            return Err(invalid("schema_version", "unsupported online head schema"));
        }
        validate_id("head_key", &self.head_key)?;
        validate_id("panel_signature_hash", &self.panel_signature_hash)?;
        validate_id("replay_cell_id", &self.replay_cell_id)?;
        validate_id("label_signature_hash", &self.label_signature_hash)?;
        validate_optional_id("skill_signature_hash", &self.skill_signature_hash)?;
        validate_optional_id("ability_signature_hash", &self.ability_signature_hash)?;
        validate_optional_id("membership_signature_hash", &self.membership_signature_hash)?;
        validate_id("last_mistake_id", &self.last_mistake_id)?;
        validate_id("last_replay_row_key", &self.last_replay_row_key)?;
        if self.last_prediction_id.0 == [0_u8; 16] {
            return Err(invalid("last_prediction_id", "must be non-zero"));
        }
        if self.support_count == 0 || self.mistake_count == 0 {
            return Err(invalid(
                "counts",
                "support_count and mistake_count must be positive",
            ));
        }
        if !self.correction_bias.is_finite() {
            return Err(invalid("correction_bias", "must be finite"));
        }
        if self.updated_at_unix_ms <= 0 {
            return Err(invalid("updated_at_unix_ms", "must be positive"));
        }
        Ok(())
    }
}

impl OnlineHeadRepeatMetricRow {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.schema_version != ONLINE_HEAD_STATE_SCHEMA_VERSION {
            return Err(invalid(
                "schema_version",
                "unsupported repeat metric schema",
            ));
        }
        validate_id("metric_key", &self.metric_key)?;
        validate_id("replay_cell_id", &self.replay_cell_id)?;
        validate_id("label_signature_hash", &self.label_signature_hash)?;
        validate_optional_id("skill_signature_hash", &self.skill_signature_hash)?;
        validate_optional_id("ability_signature_hash", &self.ability_signature_hash)?;
        validate_optional_id("membership_signature_hash", &self.membership_signature_hash)?;
        if self.window_size == 0 || self.mistake_count == 0 {
            return Err(invalid(
                "counts",
                "window_size and mistake_count must be positive",
            ));
        }
        if self.repeated_mistake_count >= self.mistake_count && self.mistake_count == 1 {
            return Err(invalid(
                "repeated_mistake_count",
                "first mistake cannot already be repeated",
            ));
        }
        if !self.mistake_repeat_rate.is_finite() || !(0.0..=1.0).contains(&self.mistake_repeat_rate)
        {
            return Err(invalid("mistake_repeat_rate", "must be finite in [0,1]"));
        }
        if self.updated_at_unix_ms <= 0 {
            return Err(invalid("updated_at_unix_ms", "must be positive"));
        }
        Ok(())
    }
}

pub fn apply_online_mistake_update_sync_readback(
    db: &DB,
    input: OnlineHeadUpdateInput,
    config: OnlineHeadUpdateConfig,
) -> Result<OnlineHeadUpdateReport, TrainerError> {
    validate_update_input(&input, config)?;
    let head_key = online_head_key(&input.panel_signature_hash)?;
    let control_panel_signature_hash = unrelated_control_panel_signature_hash(
        &input.panel_signature_hash,
        &input.replay_row.cell_id,
    )?;
    let control_before = predict_with_online_head(
        db,
        &control_panel_signature_hash,
        input.base_verdict_before_update,
    )?;
    let metric_key = repeat_metric_key(
        &input.replay_row.cell_id,
        &input.mistake_row.label_signature_hash,
        input.mistake_row.skill_signature_hash.as_deref(),
        input.mistake_row.ability_signature_hash.as_deref(),
        input.mistake_row.membership_signature_hash.as_deref(),
    )?;
    let before = read_online_head_state_row(db, &head_key)?;
    let previous_mistakes = before.as_ref().map(|row| row.mistake_count).unwrap_or(0);
    let correction_bias = before
        .as_ref()
        .map(|row| row.correction_bias)
        .unwrap_or(0.0)
        + target_sign(input.mistake_row.ground_truth_verdict)? * config.learning_rate;
    let after = OnlineHeadStateRow {
        schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
        head_key: head_key.clone(),
        panel_signature_hash: input.panel_signature_hash.clone(),
        replay_cell_id: input.replay_row.cell_id.clone(),
        label_signature_hash: input.mistake_row.label_signature_hash.clone(),
        skill_signature_hash: input.mistake_row.skill_signature_hash.clone(),
        ability_signature_hash: input.mistake_row.ability_signature_hash.clone(),
        membership_signature_hash: input.mistake_row.membership_signature_hash.clone(),
        support_count: before
            .as_ref()
            .map(|row| row.support_count.saturating_add(1))
            .unwrap_or(1),
        mistake_count: previous_mistakes.saturating_add(1),
        correction_bias,
        corrected_verdict: verdict_from_bias(correction_bias),
        last_prediction_id: input.mistake_row.prediction_id,
        last_mistake_id: input.mistake_row.mistake_id.clone(),
        last_replay_row_key: input.mistake_row.replay_row_key.clone(),
        updated_at_unix_ms: input.now_unix_ms,
    };
    write_online_head_state_row_sync_readback(db, &after)?;
    let repeated = previous_mistakes.min(config.repeat_window_size.saturating_sub(1));
    let observed = after.mistake_count.min(config.repeat_window_size);
    let metric = OnlineHeadRepeatMetricRow {
        schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
        metric_key: metric_key.clone(),
        replay_cell_id: input.replay_row.cell_id.clone(),
        label_signature_hash: input.mistake_row.label_signature_hash.clone(),
        skill_signature_hash: input.mistake_row.skill_signature_hash.clone(),
        ability_signature_hash: input.mistake_row.ability_signature_hash.clone(),
        membership_signature_hash: input.mistake_row.membership_signature_hash.clone(),
        window_size: config.repeat_window_size,
        mistake_count: observed,
        repeated_mistake_count: repeated,
        mistake_repeat_rate: repeated as f32 / observed as f32,
        updated_at_unix_ms: input.now_unix_ms,
    };
    write_repeat_metric_row_sync_readback(db, &metric)?;
    let corrected = predict_with_online_head(
        db,
        &input.panel_signature_hash,
        input.base_verdict_before_update,
    )?;
    let control_after = predict_with_online_head(
        db,
        &control_panel_signature_hash,
        input.base_verdict_before_update,
    )?;
    let replay_priority = input
        .replay_row
        .priority_score(ReplayPriorityConfig::default())?;
    let unrelated_cell_accuracy_delta = if control_before == control_after {
        0.0
    } else {
        1.0
    };
    Ok(OnlineHeadUpdateReport {
        schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
        head_state_before: before,
        head_state_after: after,
        repeat_metric_after: read_repeat_metric_row(db, &metric_key)?
            .ok_or_else(|| invalid("repeat_metric", "missing after write"))?,
        base_verdict_before_update: input.base_verdict_before_update,
        corrected_verdict_after_update: corrected,
        replay_priority_score_after: replay_priority,
        unrelated_cell_control_panel_signature_hash: control_panel_signature_hash,
        unrelated_cell_verdict_before_update: control_before,
        unrelated_cell_verdict_after_update: control_after,
        unrelated_cell_accuracy_delta,
        repeat_metric_byte_readable: db
            .get_cf(online_head_cf(db)?, metric_key.as_bytes())
            .map_err(map_rocksdb_error)?
            .is_some(),
        same_panel_signature_corrected: corrected == input.mistake_row.ground_truth_verdict,
        label_skill_ability_membership_ids_agree: contexts_agree(
            &input.mistake_row,
            &input.replay_row,
        ),
        flat_vector_concat_used: false,
    })
}

pub fn predict_with_online_head(
    db: &DB,
    panel_signature_hash: &str,
    base_verdict: Verdict,
) -> Result<Verdict, TrainerError> {
    let key = online_head_key(panel_signature_hash)?;
    Ok(read_online_head_state_row(db, &key)?
        .map(|row| row.corrected_verdict)
        .unwrap_or(base_verdict))
}

pub fn read_online_head_state_row(
    db: &DB,
    head_key: &str,
) -> Result<Option<OnlineHeadStateRow>, TrainerError> {
    validate_id("head_key", head_key)?;
    let Some(bytes) = db
        .get_cf(online_head_cf(db)?, head_key.as_bytes())
        .map_err(map_rocksdb_error)?
    else {
        return Ok(None);
    };
    let row: OnlineHeadStateRow = bincode::deserialize(&bytes).map_err(map_bincode_error)?;
    row.validate()?;
    Ok(Some(row))
}

pub fn read_repeat_metric_row(
    db: &DB,
    metric_key: &str,
) -> Result<Option<OnlineHeadRepeatMetricRow>, TrainerError> {
    validate_id("metric_key", metric_key)?;
    let Some(bytes) = db
        .get_cf(online_head_cf(db)?, metric_key.as_bytes())
        .map_err(map_rocksdb_error)?
    else {
        return Ok(None);
    };
    let row: OnlineHeadRepeatMetricRow = bincode::deserialize(&bytes).map_err(map_bincode_error)?;
    row.validate()?;
    Ok(Some(row))
}

fn write_online_head_state_row_sync_readback(
    db: &DB,
    row: &OnlineHeadStateRow,
) -> Result<(), TrainerError> {
    row.validate()?;
    put_sync(
        db,
        row.head_key.as_bytes(),
        &bincode::serialize(row).map_err(map_bincode_error)?,
    )?;
    let readback = read_online_head_state_row(db, &row.head_key)?
        .ok_or_else(|| invalid("online_head_state", "missing after write"))?;
    if readback != *row {
        return Err(invalid("online_head_state", "changed during readback"));
    }
    Ok(())
}

fn write_repeat_metric_row_sync_readback(
    db: &DB,
    row: &OnlineHeadRepeatMetricRow,
) -> Result<(), TrainerError> {
    row.validate()?;
    put_sync(
        db,
        row.metric_key.as_bytes(),
        &bincode::serialize(row).map_err(map_bincode_error)?,
    )?;
    let readback = read_repeat_metric_row(db, &row.metric_key)?
        .ok_or_else(|| invalid("repeat_metric", "missing after write"))?;
    if readback != *row {
        return Err(invalid("repeat_metric", "changed during readback"));
    }
    Ok(())
}

fn put_sync(db: &DB, key: &[u8], bytes: &[u8]) -> Result<(), TrainerError> {
    let cf = online_head_cf(db)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, bytes, &opts)
        .map_err(map_rocksdb_error)?;
    db.flush_cf(cf).map_err(map_rocksdb_error)
}

fn validate_update_input(
    input: &OnlineHeadUpdateInput,
    config: OnlineHeadUpdateConfig,
) -> Result<(), TrainerError> {
    validate_id("panel_signature_hash", &input.panel_signature_hash)?;
    input.mistake_row.validate()?;
    input.replay_row.validate()?;
    if input.mistake_row.prediction_id != input.replay_row.prediction_id {
        return Err(invalid("prediction_id", "mistake and replay rows disagree"));
    }
    if input.mistake_row.replay_row_key != hex::encode(input.replay_row.prediction_id.0) {
        return Err(invalid(
            "replay_row_key",
            "does not reference replay prediction id",
        ));
    }
    if !contexts_agree(&input.mistake_row, &input.replay_row) {
        return Err(invalid(
            "label_skill_context",
            "mistake and replay rows disagree on labels, skills, abilities, or memberships",
        ));
    }
    if input.now_unix_ms <= 0 {
        return Err(invalid("now_unix_ms", "must be positive"));
    }
    validate_binary_verdict(
        "ground_truth_verdict",
        input.mistake_row.ground_truth_verdict,
    )?;
    validate_binary_verdict(
        "base_verdict_before_update",
        input.base_verdict_before_update,
    )?;
    if !config.learning_rate.is_finite() || !(0.0..=1.0).contains(&config.learning_rate) {
        return Err(invalid("learning_rate", "must be finite in [0,1]"));
    }
    if config.learning_rate == 0.0 || config.repeat_window_size == 0 {
        return Err(invalid(
            "online_head_config",
            "learning_rate and repeat_window_size must be positive",
        ));
    }
    Ok(())
}

fn contexts_agree(mistake: &MistakeLogRow, replay: &ReplayBufferRow) -> bool {
    mistake.accepted_label_ids == replay.accepted_label_ids
        && mistake.active_skill_ids == replay.active_skill_ids
        && mistake.active_higher_ability_ids == replay.active_higher_ability_ids
        && mistake.source_membership_keys == replay.source_membership_keys
        && Some(mistake.label_signature_hash.clone()) == replay.label_signature_hash
        && mistake.skill_signature_hash == replay.skill_signature_hash
        && mistake.ability_signature_hash == replay.ability_signature_hash
        && mistake.membership_signature_hash == replay.membership_signature_hash
}

fn verdict_from_bias(value: f32) -> Verdict {
    if value >= 0.0 {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

fn target_sign(verdict: Verdict) -> Result<f32, TrainerError> {
    match verdict {
        Verdict::Pass => Ok(1.0),
        Verdict::Fail => Ok(-1.0),
        Verdict::OutOfDistribution | Verdict::Abstain | Verdict::GuardRejected => {
            Err(non_binary_verdict("ground_truth_verdict"))
        }
    }
}

fn validate_binary_verdict(field: &'static str, verdict: Verdict) -> Result<(), TrainerError> {
    match verdict {
        Verdict::Pass | Verdict::Fail => Ok(()),
        Verdict::OutOfDistribution | Verdict::Abstain | Verdict::GuardRejected => {
            Err(non_binary_verdict(field))
        }
    }
}

fn non_binary_verdict(field: &'static str) -> TrainerError {
    invalid(field, "online binary head only accepts pass/fail rows")
}
