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
use std::cmp::Ordering;

const ONLINE_HEAD_STATE_SCHEMA_VERSION: u32 = 1;
const ONLINE_HEAD_MAX_NEIGHBORS_CAP: usize = 128;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnlineHeadProtectionMode {
    ExactPanelKeyIsolation,
    FisherEwc,
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
    pub protection_mode: OnlineHeadProtectionMode,
    pub claims_fisher_ewc_protection: bool,
    pub fisher_ewc_row_key: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadNeighborConfig {
    pub max_distance: f32,
    pub max_neighbors: usize,
}

impl Default for OnlineHeadNeighborConfig {
    fn default() -> Self {
        Self {
            max_distance: 0.05,
            max_neighbors: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadNeighbor {
    pub panel_signature_hash: String,
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadNeighborContext {
    pub replay_cell_id: String,
    pub label_signature_hash: String,
    pub skill_signature_hash: Option<String>,
    pub ability_signature_hash: Option<String>,
    pub membership_signature_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnlineHeadPredictionSource {
    BaseNoCorrection,
    ExactPanel,
    NeighborPanel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OnlineHeadPredictionReport {
    pub schema_version: u32,
    pub base_verdict: Verdict,
    pub corrected_verdict: Verdict,
    pub source: OnlineHeadPredictionSource,
    pub query_panel_signature_hash: String,
    pub matched_panel_signature_hash: Option<String>,
    pub matched_distance: Option<f32>,
    pub matched_head_state: Option<OnlineHeadStateRow>,
    pub neighbor_count_considered: usize,
    pub neighbor_max_distance: f32,
    pub context_matched: bool,
    pub flat_vector_concat_used: bool,
    pub claims_fisher_ewc_protection: bool,
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

impl OnlineHeadNeighborConfig {
    fn validate(&self) -> Result<(), TrainerError> {
        if !self.max_distance.is_finite() || self.max_distance < 0.0 {
            return Err(invalid("max_distance", "must be finite and non-negative"));
        }
        if self.max_neighbors == 0 || self.max_neighbors > ONLINE_HEAD_MAX_NEIGHBORS_CAP {
            return Err(invalid(
                "max_neighbors",
                format!("must be in 1..={ONLINE_HEAD_MAX_NEIGHBORS_CAP}"),
            ));
        }
        Ok(())
    }
}

impl OnlineHeadNeighbor {
    fn validate(&self) -> Result<(), TrainerError> {
        validate_id("neighbor.panel_signature_hash", &self.panel_signature_hash)?;
        if !self.distance.is_finite() || self.distance < 0.0 {
            return Err(invalid(
                "neighbor.distance",
                "must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

impl OnlineHeadNeighborContext {
    pub fn from_replay_row(row: &ReplayBufferRow) -> Result<Self, TrainerError> {
        row.validate()?;
        Ok(Self {
            replay_cell_id: row.cell_id.clone(),
            label_signature_hash: row.label_signature_hash.clone().ok_or_else(|| {
                invalid(
                    "label_signature_hash",
                    "neighbor correction requires label-aware replay context",
                )
            })?,
            skill_signature_hash: row.skill_signature_hash.clone(),
            ability_signature_hash: row.ability_signature_hash.clone(),
            membership_signature_hash: row.membership_signature_hash.clone(),
        })
    }

    pub fn validate(&self) -> Result<(), TrainerError> {
        validate_id("replay_cell_id", &self.replay_cell_id)?;
        validate_id("label_signature_hash", &self.label_signature_hash)?;
        validate_optional_id("skill_signature_hash", &self.skill_signature_hash)?;
        validate_optional_id("ability_signature_hash", &self.ability_signature_hash)?;
        validate_optional_id("membership_signature_hash", &self.membership_signature_hash)?;
        Ok(())
    }

    fn matches_row(&self, row: &OnlineHeadStateRow) -> bool {
        self.replay_cell_id == row.replay_cell_id
            && self.label_signature_hash == row.label_signature_hash
            && self.skill_signature_hash == row.skill_signature_hash
            && self.ability_signature_hash == row.ability_signature_hash
            && self.membership_signature_hash == row.membership_signature_hash
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
    let previous_metric = read_repeat_metric_row(db, &metric_key)?;
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
    let previous_cell_mistakes = previous_metric
        .as_ref()
        .map(|row| row.mistake_count)
        .unwrap_or(0);
    let previous_cell_repeats = previous_metric
        .as_ref()
        .map(|row| row.repeated_mistake_count)
        .unwrap_or(0);
    let observed = previous_cell_mistakes
        .saturating_add(1)
        .min(config.repeat_window_size);
    let repeated = if previous_cell_mistakes == 0 {
        0
    } else {
        previous_cell_repeats.saturating_add(1).min(observed)
    };
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
        protection_mode: OnlineHeadProtectionMode::ExactPanelKeyIsolation,
        claims_fisher_ewc_protection: false,
        fisher_ewc_row_key: None,
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

pub fn online_head_report_fisher_claim_guard(
    db: &DB,
    report: &OnlineHeadUpdateReport,
) -> Result<bool, TrainerError> {
    if !report.claims_fisher_ewc_protection {
        return Ok(
            report.protection_mode == OnlineHeadProtectionMode::ExactPanelKeyIsolation
                && report.fisher_ewc_row_key.is_none(),
        );
    }
    if report.protection_mode != OnlineHeadProtectionMode::FisherEwc {
        return Ok(false);
    }
    let Some(row_key) = report.fisher_ewc_row_key.as_deref() else {
        return Ok(false);
    };
    validate_id("fisher_ewc_row_key", row_key)?;
    Ok(db
        .get_cf(online_head_cf(db)?, row_key.as_bytes())
        .map_err(map_rocksdb_error)?
        .is_some())
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

pub fn predict_with_online_head_neighbors(
    db: &DB,
    panel_signature_hash: &str,
    base_verdict: Verdict,
    context: &OnlineHeadNeighborContext,
    neighbors: &[OnlineHeadNeighbor],
    config: OnlineHeadNeighborConfig,
) -> Result<OnlineHeadPredictionReport, TrainerError> {
    validate_id("panel_signature_hash", panel_signature_hash)?;
    validate_binary_verdict("base_verdict", base_verdict)?;
    context.validate()?;
    config.validate()?;
    if neighbors.len() > config.max_neighbors {
        return Err(invalid(
            "neighbors",
            format!(
                "got {} neighbors, max {}",
                neighbors.len(),
                config.max_neighbors
            ),
        ));
    }
    for neighbor in neighbors {
        neighbor.validate()?;
    }

    let exact_key = online_head_key(panel_signature_hash)?;
    if let Some(row) = read_online_head_state_row(db, &exact_key)? {
        if context.matches_row(&row) {
            return Ok(OnlineHeadPredictionReport {
                schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
                base_verdict,
                corrected_verdict: row.corrected_verdict,
                source: OnlineHeadPredictionSource::ExactPanel,
                query_panel_signature_hash: panel_signature_hash.to_string(),
                matched_panel_signature_hash: Some(row.panel_signature_hash.clone()),
                matched_distance: Some(0.0),
                matched_head_state: Some(row),
                neighbor_count_considered: 0,
                neighbor_max_distance: config.max_distance,
                context_matched: true,
                flat_vector_concat_used: false,
                claims_fisher_ewc_protection: false,
            });
        }
    }

    let mut candidates = neighbors.to_vec();
    candidates.sort_by(compare_neighbors);
    let mut considered = 0_usize;
    for neighbor in candidates {
        if neighbor.panel_signature_hash == panel_signature_hash {
            continue;
        }
        if neighbor.distance > config.max_distance {
            continue;
        }
        considered += 1;
        let head_key = online_head_key(&neighbor.panel_signature_hash)?;
        let Some(row) = read_online_head_state_row(db, &head_key)? else {
            continue;
        };
        if !context.matches_row(&row) {
            continue;
        }
        return Ok(OnlineHeadPredictionReport {
            schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
            base_verdict,
            corrected_verdict: row.corrected_verdict,
            source: OnlineHeadPredictionSource::NeighborPanel,
            query_panel_signature_hash: panel_signature_hash.to_string(),
            matched_panel_signature_hash: Some(row.panel_signature_hash.clone()),
            matched_distance: Some(neighbor.distance),
            matched_head_state: Some(row),
            neighbor_count_considered: considered,
            neighbor_max_distance: config.max_distance,
            context_matched: true,
            flat_vector_concat_used: false,
            claims_fisher_ewc_protection: false,
        });
    }

    Ok(OnlineHeadPredictionReport {
        schema_version: ONLINE_HEAD_STATE_SCHEMA_VERSION,
        base_verdict,
        corrected_verdict: base_verdict,
        source: OnlineHeadPredictionSource::BaseNoCorrection,
        query_panel_signature_hash: panel_signature_hash.to_string(),
        matched_panel_signature_hash: None,
        matched_distance: None,
        matched_head_state: None,
        neighbor_count_considered: considered,
        neighbor_max_distance: config.max_distance,
        context_matched: false,
        flat_vector_concat_used: false,
        claims_fisher_ewc_protection: false,
    })
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

fn compare_neighbors(a: &OnlineHeadNeighbor, b: &OnlineHeadNeighbor) -> Ordering {
    a.distance
        .partial_cmp(&b.distance)
        .unwrap_or(Ordering::Greater)
        .then_with(|| a.panel_signature_hash.cmp(&b.panel_signature_hash))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability_resolver::open_ability_resolver_rocksdb;
    use crate::label_bridge::{
        ability_signature_hash, accepted_label_signature_hash, membership_signature_hash,
        skill_signature_hash,
    };
    use crate::mistake_log::{mistake_id_from_evidence_parts, MistakeTruthSource};
    use crate::replay_buffer::{
        ability_aware_replay_cell_id, ReplayBufferSource, ReplayRetentionTier,
    };

    const NOW: i64 = 1_780_470_000_000;

    fn rows(byte: u8, suffix: &str) -> (MistakeLogRow, ReplayBufferRow) {
        let prediction_id = PredictionId([byte; 16]);
        let accepted_label_ids = vec!["ast_surface:function".to_string()];
        let active_skill_ids = vec!["skill:unit_sequence".to_string()];
        let active_higher_ability_ids = vec!["ability:boundary_sequence".to_string()];
        let source_membership_keys = vec![
            "chunk_skill::chunk:fsv689::python:before:fsv689::skill:unit_sequence".to_string(),
        ];
        let label_signature_hash = accepted_label_signature_hash(&accepted_label_ids).unwrap();
        let skill_signature_hash = Some(skill_signature_hash(&active_skill_ids).unwrap());
        let ability_signature_hash =
            Some(ability_signature_hash(&active_higher_ability_ids).unwrap());
        let membership_signature_hash =
            Some(membership_signature_hash(&source_membership_keys).unwrap());
        let code_state_key = format!("python:before:fsv689:{suffix}");
        let named_failure_mode = "failure:neighbor_mistake_loop".to_string();
        let replay_row_key = hex::encode(prediction_id.0);
        let mistake_id = mistake_id_from_evidence_parts(
            prediction_id,
            &code_state_key,
            &label_signature_hash,
            skill_signature_hash.as_deref(),
            ability_signature_hash.as_deref(),
            membership_signature_hash.as_deref(),
            Verdict::Fail,
        )
        .unwrap();
        let cell_id = ability_aware_replay_cell_id(
            "python",
            "fsv689_mistake_loop",
            &code_state_key,
            &named_failure_mode,
            &label_signature_hash,
            skill_signature_hash.as_deref(),
            ability_signature_hash.as_deref(),
            membership_signature_hash.as_deref(),
        )
        .unwrap();
        (
            MistakeLogRow {
                schema_version: 1,
                mistake_id,
                prediction_id,
                predicted_verdict: Verdict::Pass,
                ground_truth_verdict: Verdict::Fail,
                truth_source: MistakeTruthSource::SwebenchDockerOracle,
                code_state_key: code_state_key.clone(),
                named_failure_mode: Some(named_failure_mode),
                accepted_label_ids: accepted_label_ids.clone(),
                active_skill_ids: active_skill_ids.clone(),
                label_signature_hash: label_signature_hash.clone(),
                skill_signature_hash: skill_signature_hash.clone(),
                failure_evidence_set_ids: vec![code_state_key],
                replay_row_key,
                accepted_registry_sha256: Some("sha256:fsv689-accepted-registry".to_string()),
                usefulness_metrics_sha256: Some("sha256:fsv689-usefulness".to_string()),
                learning_bridge_manifest_sha256: Some("sha256:fsv689-bridge".to_string()),
                created_at_unix_ms: NOW,
                active_higher_ability_ids: active_higher_ability_ids.clone(),
                source_membership_keys: source_membership_keys.clone(),
                ability_signature_hash: ability_signature_hash.clone(),
                membership_signature_hash: membership_signature_hash.clone(),
            },
            ReplayBufferRow {
                prediction_id,
                surprise_z: 2.0,
                cell_id,
                coverage_gap_score: 0.75,
                last_replayed_ts: None,
                replay_count: 0,
                retention_weight: 1.0,
                protected: true,
                retention_tier: ReplayRetentionTier::Hot,
                source: ReplayBufferSource::AcceptedLabelMistake,
                created_at_unix_ms: NOW,
                updated_at_unix_ms: NOW,
                accepted_label_ids,
                active_skill_ids,
                active_higher_ability_ids,
                source_membership_keys,
                label_signature_hash: Some(label_signature_hash),
                skill_signature_hash,
                ability_signature_hash,
                membership_signature_hash,
            },
        )
    }

    fn record_panel(
        db: &DB,
        panel_signature_hash: &str,
        mistake_row: MistakeLogRow,
        replay_row: ReplayBufferRow,
    ) {
        apply_online_mistake_update_sync_readback(
            db,
            OnlineHeadUpdateInput {
                panel_signature_hash: panel_signature_hash.to_string(),
                mistake_row,
                replay_row,
                base_verdict_before_update: Verdict::Pass,
                now_unix_ms: NOW + 1,
            },
            OnlineHeadUpdateConfig {
                learning_rate: 1.0,
                repeat_window_size: 4,
            },
        )
        .unwrap();
    }

    #[test]
    fn neighbor_prediction_propagates_to_non_identical_panel_when_context_matches() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
        let (mistake_row, replay_row) = rows(0x68, "source");
        let context = OnlineHeadNeighborContext::from_replay_row(&replay_row).unwrap();
        record_panel(&db, "panel:fsv689:source", mistake_row, replay_row);

        assert_eq!(
            predict_with_online_head(&db, "panel:fsv689:heldout", Verdict::Pass).unwrap(),
            Verdict::Pass
        );

        let report = predict_with_online_head_neighbors(
            &db,
            "panel:fsv689:heldout",
            Verdict::Pass,
            &context,
            &[OnlineHeadNeighbor {
                panel_signature_hash: "panel:fsv689:source".to_string(),
                distance: 0.01,
            }],
            OnlineHeadNeighborConfig::default(),
        )
        .unwrap();

        assert_eq!(report.source, OnlineHeadPredictionSource::NeighborPanel);
        assert_eq!(report.corrected_verdict, Verdict::Fail);
        assert_eq!(
            report.matched_panel_signature_hash.as_deref(),
            Some("panel:fsv689:source")
        );
        assert_eq!(report.matched_distance, Some(0.01));
        assert!(report.context_matched);
        assert!(!report.flat_vector_concat_used);
        assert!(!report.claims_fisher_ewc_protection);
    }

    #[test]
    fn neighbor_prediction_does_not_cross_distance_or_context_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
        let (mistake_row, replay_row) = rows(0x69, "source");
        let context = OnlineHeadNeighborContext::from_replay_row(&replay_row).unwrap();
        record_panel(&db, "panel:fsv689:source", mistake_row, replay_row);

        let distant = predict_with_online_head_neighbors(
            &db,
            "panel:fsv689:heldout",
            Verdict::Pass,
            &context,
            &[OnlineHeadNeighbor {
                panel_signature_hash: "panel:fsv689:source".to_string(),
                distance: 0.25,
            }],
            OnlineHeadNeighborConfig::default(),
        )
        .unwrap();
        assert_eq!(distant.source, OnlineHeadPredictionSource::BaseNoCorrection);
        assert_eq!(distant.corrected_verdict, Verdict::Pass);

        let mut wrong_context = context;
        wrong_context.label_signature_hash = "labels:not_the_recorded_context".to_string();
        let mismatch = predict_with_online_head_neighbors(
            &db,
            "panel:fsv689:heldout",
            Verdict::Pass,
            &wrong_context,
            &[OnlineHeadNeighbor {
                panel_signature_hash: "panel:fsv689:source".to_string(),
                distance: 0.01,
            }],
            OnlineHeadNeighborConfig::default(),
        )
        .unwrap();
        assert_eq!(
            mismatch.source,
            OnlineHeadPredictionSource::BaseNoCorrection
        );
        assert_eq!(mismatch.corrected_verdict, Verdict::Pass);
    }

    #[test]
    fn neighbor_prediction_fails_closed_on_invalid_neighbor_input() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
        let (_mistake_row, replay_row) = rows(0x70, "source");
        let context = OnlineHeadNeighborContext::from_replay_row(&replay_row).unwrap();

        let err = predict_with_online_head_neighbors(
            &db,
            "panel:fsv689:heldout",
            Verdict::Pass,
            &context,
            &[OnlineHeadNeighbor {
                panel_signature_hash: "panel:fsv689:source".to_string(),
                distance: f32::NAN,
            }],
            OnlineHeadNeighborConfig::default(),
        )
        .unwrap_err();

        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn repeat_metric_aggregates_non_identical_panels_by_cell_context() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_ability_resolver_rocksdb(temp.path(), true).unwrap();
        let (first_mistake, first_replay) = rows(0x71, "shared-cell");
        let (second_mistake, second_replay) = rows(0x72, "shared-cell");
        let metric_key = repeat_metric_key(
            &first_replay.cell_id,
            &first_mistake.label_signature_hash,
            first_mistake.skill_signature_hash.as_deref(),
            first_mistake.ability_signature_hash.as_deref(),
            first_mistake.membership_signature_hash.as_deref(),
        )
        .unwrap();

        record_panel(&db, "panel:fsv689:first", first_mistake, first_replay);
        record_panel(&db, "panel:fsv689:second", second_mistake, second_replay);

        let metric = read_repeat_metric_row(&db, &metric_key).unwrap().unwrap();
        assert_eq!(metric.mistake_count, 2);
        assert_eq!(metric.repeated_mistake_count, 1);
        assert!((metric.mistake_repeat_rate - 0.5).abs() < f32::EPSILON);
    }
}
