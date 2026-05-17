use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use bincode::Options;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::constellation::{bincode_options, TctConstellation};
use crate::error::TctError;
use crate::freshness::DEFAULT_MAX_AGE_DAYS;
use crate::shrinkage::ShrinkageOrigin;
use crate::types::{validate_cos, EmbedderId, EntityType, Language, MutationCategory};

pub const TCT_REFRESH_LOG_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_MAX_NEW_EXAMPLES_SINCE_REFRESH: u32 = 100;
pub const DEFAULT_MAX_TAU_M_DRIFT_PCT: f32 = 3.0;
pub const DEFAULT_SHRUNK_CELL_SUPPORT_THRESHOLD: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCellKey {
    pub mutation: MutationCategory,
    pub entity_type: EntityType,
    pub language: Language,
    pub embedder: EmbedderId,
}

impl ConstellationCellKey {
    pub fn stable_id(&self) -> String {
        format!(
            "{:?}/{:?}/{:?}/{}",
            self.mutation, self.entity_type, self.language, self.embedder
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshPolicyConfig {
    pub max_new_examples_since_refresh: u32,
    pub max_tau_m_drift_pct: f32,
    pub max_age_days: u32,
    pub shrunk_cell_support_threshold: u32,
}

impl Default for RefreshPolicyConfig {
    fn default() -> Self {
        Self {
            max_new_examples_since_refresh: DEFAULT_MAX_NEW_EXAMPLES_SINCE_REFRESH,
            max_tau_m_drift_pct: DEFAULT_MAX_TAU_M_DRIFT_PCT,
            max_age_days: DEFAULT_MAX_AGE_DAYS,
            shrunk_cell_support_threshold: DEFAULT_SHRUNK_CELL_SUPPORT_THRESHOLD,
        }
    }
}

impl RefreshPolicyConfig {
    pub fn validate(&self) -> Result<(), TctError> {
        if self.max_new_examples_since_refresh == 0 {
            return Err(TctError::invalid(
                "refresh_policy.max_new_examples_since_refresh",
                "threshold must be greater than zero",
            ));
        }
        if !self.max_tau_m_drift_pct.is_finite() || self.max_tau_m_drift_pct <= 0.0 {
            return Err(TctError::nan(
                "refresh_policy.max_tau_m_drift_pct",
                format!(
                    "threshold must be finite and greater than zero, got {}",
                    self.max_tau_m_drift_pct
                ),
            ));
        }
        if self.max_age_days == 0 {
            return Err(TctError::invalid(
                "refresh_policy.max_age_days",
                "threshold must be greater than zero",
            ));
        }
        if self.shrunk_cell_support_threshold == 0 {
            return Err(TctError::invalid(
                "refresh_policy.shrunk_cell_support_threshold",
                "threshold must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCell {
    pub key: ConstellationCellKey,
    pub n_supporting: u32,
    pub bayesian_shrunk: bool,
    pub tau_m: f32,
    pub last_refresh_ts: SystemTime,
    pub examples_since_last_refresh: u32,
    pub tau_m_drift_pct_rolling: f32,
    pub refresh_failed: bool,
}

impl ConstellationCell {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        key: ConstellationCellKey,
        n_supporting: u32,
        bayesian_shrunk: bool,
        tau_m: f32,
        last_refresh_ts: SystemTime,
        examples_since_last_refresh: u32,
        tau_m_drift_pct_rolling: f32,
        refresh_failed: bool,
    ) -> Result<Self, TctError> {
        let value = Self {
            key,
            n_supporting,
            bayesian_shrunk,
            tau_m,
            last_refresh_ts,
            examples_since_last_refresh,
            tau_m_drift_pct_rolling,
            refresh_failed,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), TctError> {
        if self.n_supporting == 0 {
            return Err(TctError::InsufficientSamples {
                cell: self.key.stable_id(),
                observed: 0,
                required: 1,
            });
        }
        validate_cos("constellation_cell.tau_m", self.tau_m)?;
        if !self.tau_m_drift_pct_rolling.is_finite() || self.tau_m_drift_pct_rolling < 0.0 {
            return Err(TctError::nan(
                "constellation_cell.tau_m_drift_pct_rolling",
                format!(
                    "drift percent must be finite and non-negative, got {}",
                    self.tau_m_drift_pct_rolling
                ),
            ));
        }
        Ok(())
    }

    pub fn age_days_at(&self, now: SystemTime) -> Result<u32, TctError> {
        age_days_between(
            now,
            self.last_refresh_ts,
            "constellation_cell.last_refresh_ts",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCellFreshnessOverride {
    pub last_refresh_ts: Option<SystemTime>,
    pub examples_since_last_refresh: Option<u32>,
    pub tau_m_drift_pct_rolling: Option<f32>,
    pub refresh_failed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RefreshReason {
    NewExamples {
        observed: u32,
        threshold: u32,
    },
    TauMDrift {
        observed_pct: f32,
        threshold_pct: f32,
    },
    FreshnessAge {
        age_days: u32,
        threshold_days: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RefreshDecision {
    Skip {
        reason: String,
    },
    Refit {
        reason: RefreshReason,
        shrunk_cell: bool,
    },
}

impl RefreshDecision {
    pub fn is_refit(&self) -> bool {
        matches!(self, Self::Refit { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefreshActionStatus {
    Skipped,
    RefitSucceeded,
    RefreshFailed,
    ManualOverrideFreshnessBypass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCellAuditRow {
    pub cell: ConstellationCell,
    pub age_days: u32,
    pub decision: RefreshDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FreshnessHistogram {
    pub age_0_to_30_days: usize,
    pub age_31_to_90_days: usize,
    pub age_over_90_days: usize,
}

impl FreshnessHistogram {
    fn record(&mut self, age_days: u32) {
        if age_days <= 30 {
            self.age_0_to_30_days += 1;
        } else if age_days <= 90 {
            self.age_31_to_90_days += 1;
        } else {
            self.age_over_90_days += 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessAuditReport {
    pub generated_at: SystemTime,
    pub constellation_version_id: [u8; 32],
    pub total_cells: usize,
    pub refit_required_count: usize,
    pub skip_count: usize,
    pub failed_cell_count: usize,
    pub shrunk_refit_count: usize,
    pub histogram: FreshnessHistogram,
    pub rows: Vec<ConstellationCellAuditRow>,
}

impl FreshnessAuditReport {
    pub fn validate(&self) -> Result<(), TctError> {
        if self.constellation_version_id == [0; 32] {
            return Err(TctError::invalid(
                "freshness_audit.constellation_version_id",
                "version id must be non-zero",
            ));
        }
        if self.total_cells == 0 {
            return Err(TctError::InsufficientSamples {
                cell: "freshness_audit.rows".to_string(),
                observed: 0,
                required: 1,
            });
        }
        if self.total_cells != self.rows.len() {
            return Err(TctError::dim(
                self.total_cells,
                self.rows.len(),
                "freshness_audit total_cells must match row count",
            ));
        }
        if self.refit_required_count + self.skip_count != self.total_cells {
            return Err(TctError::invalid(
                "freshness_audit.counts",
                "refit_required_count + skip_count must equal total_cells",
            ));
        }
        for row in &self.rows {
            row.cell.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationRefreshLogEntryInput {
    pub constellation_version_id: [u8; 32],
    pub cell: ConstellationCell,
    pub decision: RefreshDecision,
    pub status: RefreshActionStatus,
    pub generated_at: SystemTime,
    pub after_last_refresh_ts: Option<SystemTime>,
    pub operator_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationRefreshLogEntry {
    pub schema_version: u16,
    pub event_id: [u8; 32],
    pub constellation_version_id: [u8; 32],
    pub cell_key: ConstellationCellKey,
    pub n_supporting: u32,
    pub bayesian_shrunk: bool,
    pub tau_m: f32,
    pub before_last_refresh_ts: SystemTime,
    pub after_last_refresh_ts: Option<SystemTime>,
    pub examples_since_last_refresh: u32,
    pub tau_m_drift_pct_rolling: f32,
    pub age_days_before: u32,
    pub decision: RefreshDecision,
    pub status: RefreshActionStatus,
    pub generated_at: SystemTime,
    pub operator_id: Option<String>,
    pub detail: String,
}

impl ConstellationRefreshLogEntry {
    pub fn try_new(input: ConstellationRefreshLogEntryInput) -> Result<Self, TctError> {
        let age_days_before = input.cell.age_days_at(input.generated_at)?;
        let mut value = Self {
            schema_version: TCT_REFRESH_LOG_SCHEMA_VERSION,
            event_id: [0; 32],
            constellation_version_id: input.constellation_version_id,
            cell_key: input.cell.key,
            n_supporting: input.cell.n_supporting,
            bayesian_shrunk: input.cell.bayesian_shrunk,
            tau_m: input.cell.tau_m,
            before_last_refresh_ts: input.cell.last_refresh_ts,
            after_last_refresh_ts: input.after_last_refresh_ts,
            examples_since_last_refresh: input.cell.examples_since_last_refresh,
            tau_m_drift_pct_rolling: input.cell.tau_m_drift_pct_rolling,
            age_days_before,
            decision: input.decision,
            status: input.status,
            generated_at: input.generated_at,
            operator_id: input.operator_id,
            detail: input.detail,
        };
        value.validate_without_event_id()?;
        value.event_id = value.compute_event_id()?;
        value.validate_integrity()?;
        Ok(value)
    }

    pub fn validate_integrity(&self) -> Result<(), TctError> {
        self.validate_without_event_id()?;
        let observed = self.compute_event_id()?;
        if observed != self.event_id {
            return Err(TctError::FrozenViolation {
                detail: format!(
                    "refresh log event_id mismatch: stored={} recomputed={}",
                    hex::encode(self.event_id),
                    hex::encode(observed)
                ),
            });
        }
        Ok(())
    }

    pub fn compute_event_id(&self) -> Result<[u8; 32], TctError> {
        let payload = RefreshLogPayload {
            schema_version: self.schema_version,
            constellation_version_id: self.constellation_version_id,
            cell_key: self.cell_key,
            n_supporting: self.n_supporting,
            bayesian_shrunk: self.bayesian_shrunk,
            tau_m: self.tau_m,
            before_last_refresh_ts: self.before_last_refresh_ts,
            after_last_refresh_ts: self.after_last_refresh_ts,
            examples_since_last_refresh: self.examples_since_last_refresh,
            tau_m_drift_pct_rolling: self.tau_m_drift_pct_rolling,
            age_days_before: self.age_days_before,
            decision: &self.decision,
            status: self.status,
            generated_at: self.generated_at,
            operator_id: self.operator_id.as_deref(),
            detail: &self.detail,
        };
        let bytes = bincode_options().serialize(&payload)?;
        Ok(Sha256::digest(bytes).into())
    }

    fn validate_without_event_id(&self) -> Result<(), TctError> {
        if self.schema_version != TCT_REFRESH_LOG_SCHEMA_VERSION {
            return Err(TctError::invalid(
                "refresh_log.schema_version",
                format!(
                    "unsupported schema version {}; expected {TCT_REFRESH_LOG_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        if self.constellation_version_id == [0; 32] {
            return Err(TctError::invalid(
                "refresh_log.constellation_version_id",
                "version id must be non-zero",
            ));
        }
        if self.n_supporting == 0 {
            return Err(TctError::InsufficientSamples {
                cell: self.cell_key.stable_id(),
                observed: 0,
                required: 1,
            });
        }
        validate_cos("refresh_log.tau_m", self.tau_m)?;
        if !self.tau_m_drift_pct_rolling.is_finite() || self.tau_m_drift_pct_rolling < 0.0 {
            return Err(TctError::nan(
                "refresh_log.tau_m_drift_pct_rolling",
                format!(
                    "drift percent must be finite and non-negative, got {}",
                    self.tau_m_drift_pct_rolling
                ),
            ));
        }
        if let Some(after) = self.after_last_refresh_ts {
            after
                .duration_since(self.before_last_refresh_ts)
                .map_err(|_| {
                    TctError::invalid(
                        "refresh_log.after_last_refresh_ts",
                        "after_last_refresh_ts must be >= before_last_refresh_ts",
                    )
                })?;
        }
        if matches!(
            self.status,
            RefreshActionStatus::ManualOverrideFreshnessBypass
        ) && self.operator_id.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(TctError::invalid(
                "refresh_log.operator_id",
                "manual override bypass rows require operator_id",
            ));
        }
        if let Some(operator_id) = &self.operator_id {
            validate_log_text("refresh_log.operator_id", operator_id)?;
        }
        validate_log_text("refresh_log.detail", &self.detail)?;
        Ok(())
    }
}

#[derive(Serialize)]
struct RefreshLogPayload<'a> {
    schema_version: u16,
    constellation_version_id: [u8; 32],
    cell_key: ConstellationCellKey,
    n_supporting: u32,
    bayesian_shrunk: bool,
    tau_m: f32,
    before_last_refresh_ts: SystemTime,
    after_last_refresh_ts: Option<SystemTime>,
    examples_since_last_refresh: u32,
    tau_m_drift_pct_rolling: f32,
    age_days_before: u32,
    decision: &'a RefreshDecision,
    status: RefreshActionStatus,
    generated_at: SystemTime,
    operator_id: Option<&'a str>,
    detail: &'a str,
}

pub fn should_refresh(
    cell: &ConstellationCell,
    now: SystemTime,
    config: RefreshPolicyConfig,
) -> Result<RefreshDecision, TctError> {
    config.validate()?;
    cell.validate()?;
    let shrunk_cell =
        cell.bayesian_shrunk || cell.n_supporting < config.shrunk_cell_support_threshold;
    if cell.examples_since_last_refresh > config.max_new_examples_since_refresh {
        return Ok(RefreshDecision::Refit {
            reason: RefreshReason::NewExamples {
                observed: cell.examples_since_last_refresh,
                threshold: config.max_new_examples_since_refresh,
            },
            shrunk_cell,
        });
    }
    if cell.tau_m_drift_pct_rolling > config.max_tau_m_drift_pct {
        return Ok(RefreshDecision::Refit {
            reason: RefreshReason::TauMDrift {
                observed_pct: cell.tau_m_drift_pct_rolling,
                threshold_pct: config.max_tau_m_drift_pct,
            },
            shrunk_cell,
        });
    }
    let age_days = cell.age_days_at(now)?;
    if age_days > config.max_age_days {
        return Ok(RefreshDecision::Refit {
            reason: RefreshReason::FreshnessAge {
                age_days,
                threshold_days: config.max_age_days,
            },
            shrunk_cell,
        });
    }
    Ok(RefreshDecision::Skip {
        reason: "all_refresh_policy_clauses_below_threshold".to_string(),
    })
}

pub fn materialize_constellation_cells(
    constellation: &TctConstellation,
    overrides: &BTreeMap<ConstellationCellKey, ConstellationCellFreshnessOverride>,
) -> Result<Vec<ConstellationCell>, TctError> {
    constellation.validate_integrity()?;
    let mut cells = Vec::with_capacity(constellation.per_chunk_type_centroids.len());
    for ((mutation, entity_type, language, embedder), centroid) in
        &constellation.per_chunk_type_centroids
    {
        let key = ConstellationCellKey {
            mutation: *mutation,
            entity_type: *entity_type,
            language: *language,
            embedder: *embedder,
        };
        let override_row = overrides.get(&key).copied().unwrap_or_default();
        let n_supporting = u32::try_from(centroid.sample_count).map_err(|_| {
            TctError::invalid(
                "constellation_cell.n_supporting",
                format!("sample_count {} exceeds u32::MAX", centroid.sample_count),
            )
        })?;
        let tau_m = constellation.threshold(*embedder, Some(*entity_type))?;
        let cell = ConstellationCell::try_new(
            key,
            n_supporting,
            centroid.origin != ShrinkageOrigin::OwnCell
                || n_supporting < DEFAULT_SHRUNK_CELL_SUPPORT_THRESHOLD,
            tau_m,
            override_row
                .last_refresh_ts
                .unwrap_or(constellation.frozen_at),
            override_row.examples_since_last_refresh.unwrap_or(0),
            override_row.tau_m_drift_pct_rolling.unwrap_or(0.0),
            override_row.refresh_failed.unwrap_or(false),
        )?;
        cells.push(cell);
    }
    if cells.is_empty() {
        return Err(TctError::MissingCentroid {
            detail: "constellation has no per-chunk cells to audit".to_string(),
        });
    }
    Ok(cells)
}

pub fn materialize_target_cells(
    constellation: &TctConstellation,
    mutation: MutationCategory,
    language: Language,
    entity_type: EntityType,
    overrides: &BTreeMap<ConstellationCellKey, ConstellationCellFreshnessOverride>,
) -> Result<Vec<ConstellationCell>, TctError> {
    let cells = materialize_constellation_cells(constellation, overrides)?;
    let target = cells
        .into_iter()
        .filter(|cell| {
            cell.key.mutation == mutation
                && cell.key.language == language
                && cell.key.entity_type == entity_type
        })
        .collect::<Vec<_>>();
    if target.is_empty() {
        return Err(TctError::MissingCentroid {
            detail: format!("no constellation cells for {mutation:?}/{language:?}/{entity_type:?}"),
        });
    }
    Ok(target)
}

pub fn build_freshness_audit(
    constellation_version_id: [u8; 32],
    cells: &[ConstellationCell],
    now: SystemTime,
    config: RefreshPolicyConfig,
) -> Result<FreshnessAuditReport, TctError> {
    config.validate()?;
    if constellation_version_id == [0; 32] {
        return Err(TctError::invalid(
            "freshness_audit.constellation_version_id",
            "version id must be non-zero",
        ));
    }
    if cells.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "freshness_audit.cells".to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mut rows = Vec::with_capacity(cells.len());
    let mut histogram = FreshnessHistogram::default();
    let mut refit_required_count = 0usize;
    let mut skip_count = 0usize;
    let mut failed_cell_count = 0usize;
    let mut shrunk_refit_count = 0usize;
    for cell in cells {
        let age_days = cell.age_days_at(now)?;
        histogram.record(age_days);
        let decision = should_refresh(cell, now, config)?;
        match &decision {
            RefreshDecision::Refit { shrunk_cell, .. } => {
                refit_required_count += 1;
                if *shrunk_cell {
                    shrunk_refit_count += 1;
                }
            }
            RefreshDecision::Skip { .. } => skip_count += 1,
        }
        if cell.refresh_failed {
            failed_cell_count += 1;
        }
        rows.push(ConstellationCellAuditRow {
            cell: *cell,
            age_days,
            decision,
        });
    }
    let report = FreshnessAuditReport {
        generated_at: now,
        constellation_version_id,
        total_cells: cells.len(),
        refit_required_count,
        skip_count,
        failed_cell_count,
        shrunk_refit_count,
        histogram,
        rows,
    };
    report.validate()?;
    Ok(report)
}

pub fn overrides_from_refresh_log(
    version_id: [u8; 32],
    entries: &[ConstellationRefreshLogEntry],
) -> BTreeMap<ConstellationCellKey, ConstellationCellFreshnessOverride> {
    let mut out = BTreeMap::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.constellation_version_id == version_id)
    {
        let override_row = out
            .entry(entry.cell_key)
            .or_insert_with(ConstellationCellFreshnessOverride::default);
        match entry.status {
            RefreshActionStatus::RefitSucceeded => {
                override_row.last_refresh_ts = entry.after_last_refresh_ts;
                override_row.examples_since_last_refresh = Some(0);
                override_row.tau_m_drift_pct_rolling = Some(0.0);
                override_row.refresh_failed = Some(false);
            }
            RefreshActionStatus::RefreshFailed => {
                override_row.refresh_failed = Some(true);
            }
            RefreshActionStatus::Skipped | RefreshActionStatus::ManualOverrideFreshnessBypass => {}
        }
    }
    out
}

pub fn freshness_bypass_entry(
    constellation_version_id: [u8; 32],
    cell: ConstellationCell,
    generated_at: SystemTime,
    operator_id: impl Into<String>,
    detail: impl Into<String>,
) -> Result<ConstellationRefreshLogEntry, TctError> {
    ConstellationRefreshLogEntry::try_new(ConstellationRefreshLogEntryInput {
        constellation_version_id,
        cell,
        decision: RefreshDecision::Skip {
            reason: "operator_override_freshness_bypass".to_string(),
        },
        status: RefreshActionStatus::ManualOverrideFreshnessBypass,
        generated_at,
        after_last_refresh_ts: None,
        operator_id: Some(operator_id.into()),
        detail: detail.into(),
    })
}

pub fn age_days_between(
    now: SystemTime,
    then: SystemTime,
    field: &'static str,
) -> Result<u32, TctError> {
    let age = now.duration_since(then).map_err(|_| {
        TctError::invalid(field, "timestamp is in the future relative to audit time")
    })?;
    Ok(duration_days(age))
}

fn duration_days(age: Duration) -> u32 {
    let days = age.as_secs() / 86_400;
    u32::try_from(days).unwrap_or(u32::MAX)
}

fn validate_log_text(field: &str, value: &str) -> Result<(), TctError> {
    if value.trim().is_empty() {
        return Err(TctError::invalid(field, "value must be non-empty"));
    }
    if value
        .bytes()
        .any(|byte| byte == 0 || (byte < 0x20 && byte != b'\n'))
    {
        return Err(TctError::invalid(
            field,
            "value must not contain NUL or control bytes",
        ));
    }
    Ok(())
}
