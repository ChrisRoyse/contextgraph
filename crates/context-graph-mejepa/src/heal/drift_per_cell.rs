use std::collections::BTreeMap;
use std::sync::Arc;

use rocksdb::DB;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use rocksdb::IteratorMode;

use crate::eval::{cell_key, EvalReport, MutationCategory};
use crate::heal::drift::{DriftSeverity, SeverityTable};
use crate::heal::drift_bayesian::{jeffreys_posterior_below_threshold, BayesianDriftDecision};
use crate::heal::errors::HealError;
use crate::heal::lora_refresh::{refresh, CorpusSlice, LoraRefresher};
use crate::heal::policy::{persist_policy_record, scan_policy_records};
use crate::heal::promote::TriggerReason;
use crate::heal::promote_approval::{queue_pending_retrain_request, PendingPromotionKind};
use crate::heal::store::HealRocksStore;
use crate::types::Language;

const CELL_OBSERVATION_PREFIX: &[u8] = b"phase-e/cell-observation/";
const CELL_REPORT_PREFIX: &[u8] = b"phase-e/per-cell-drift-report/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftCell {
    pub mutation_category: MutationCategory,
    pub language: Language,
}

impl DriftCell {
    pub fn key(self) -> String {
        cell_key(self.mutation_category, self.language)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerCellDriftObservation {
    pub cell: DriftCell,
    pub covered: bool,
    pub surprise_weight: u8,
    pub witness_chain_offset: u64,
    pub observed_at_unix_ms: i64,
    pub source: String,
    pub sample_hash: [u8; 32],
}

impl PerCellDriftObservation {
    pub fn try_new(
        cell: DriftCell,
        covered: bool,
        surprise_weight: u8,
        witness_chain_offset: u64,
        observed_at_unix_ms: i64,
        source: impl Into<String>,
    ) -> Result<Self, HealError> {
        if !matches!(surprise_weight, 1 | 2 | 4) {
            return Err(HealError::invalid(
                "per_cell_drift.surprise_weight",
                format!("surprise_weight must be 1, 2, or 4, got {surprise_weight}"),
            ));
        }
        let source = source.into();
        if source.trim().is_empty() {
            return Err(HealError::invalid(
                "per_cell_drift.source",
                "source must be non-empty",
            ));
        }
        let sample_hash = observation_sample_hash(
            cell,
            covered,
            surprise_weight,
            witness_chain_offset,
            observed_at_unix_ms,
            &source,
        );
        Ok(Self {
            cell,
            covered,
            surprise_weight,
            witness_chain_offset,
            observed_at_unix_ms,
            source,
            sample_hash,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerCellDriftConfig {
    pub max_window_observations: usize,
    pub small_window_cutoff: usize,
    pub min_large_window_samples: usize,
    pub bayesian_confidence: f32,
    pub severity_table: SeverityTable,
    pub catastrophic_threshold: f32,
}

impl Default for PerCellDriftConfig {
    fn default() -> Self {
        Self {
            max_window_observations: 1000,
            small_window_cutoff: 50,
            min_large_window_samples: 50,
            bayesian_confidence: 0.95,
            severity_table: SeverityTable::default(),
            catastrophic_threshold: 0.80,
        }
    }
}

impl PerCellDriftConfig {
    pub fn validate(&self) -> Result<(), HealError> {
        if self.max_window_observations == 0
            || self.small_window_cutoff == 0
            || self.min_large_window_samples == 0
        {
            return Err(HealError::invalid(
                "per_cell_drift.window",
                "window sizes must be greater than zero",
            ));
        }
        if !self.bayesian_confidence.is_finite() || !(0.5..1.0).contains(&self.bayesian_confidence)
        {
            return Err(HealError::invalid(
                "per_cell_drift.bayesian_confidence",
                "bayesian confidence must be finite in [0.5,1)",
            ));
        }
        if !self.catastrophic_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.catastrophic_threshold)
        {
            return Err(HealError::invalid(
                "per_cell_drift.catastrophic_threshold",
                "catastrophic threshold must be finite in [0,1]",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PerCellIntervention {
    NoChange,
    QueueLoraRefresh,
    QueueFullRetrainApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerCellDriftCellReport {
    pub cell_key: String,
    pub sample_count: usize,
    pub weighted_successes: u64,
    pub weighted_trials: u64,
    pub metric_value: Option<f32>,
    pub metric_source: String,
    pub severity: DriftSeverity,
    pub bayesian: Option<BayesianDriftDecision>,
    pub intervention: PerCellIntervention,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerCellDriftReport {
    pub generated_at_unix_ms: i64,
    pub source_of_truth_cf: String,
    pub observation_count: usize,
    pub latest_eval_report_seen: bool,
    pub cells: BTreeMap<String, PerCellDriftCellReport>,
    pub lora_refresh_reports: Vec<crate::heal::lora_refresh::LoraRefreshReport>,
    pub queued_catastrophic_promotions: Vec<String>,
}

pub fn persist_per_cell_observation(
    storage: &HealRocksStore,
    observation: &PerCellDriftObservation,
) -> Result<Vec<u8>, HealError> {
    let key = observation_key(observation);
    persist_policy_record(storage, &key, observation)?;
    Ok(key)
}

pub fn detect_and_act_per_cell_drift(
    _db: Arc<DB>,
    storage: Arc<HealRocksStore>,
    lora_refresher: &mut LoraRefresher,
    config: &PerCellDriftConfig,
) -> Result<PerCellDriftReport, HealError> {
    config.validate()?;
    let mut report = build_per_cell_drift_report(storage.as_ref(), config)?;
    for cell in report.cells.values_mut() {
        match cell.intervention {
            PerCellIntervention::NoChange => {}
            PerCellIntervention::QueueLoraRefresh => {
                let corpus =
                    corpus_slice_for_cell(storage.as_ref(), &cell.cell_key, cell.weighted_trials)?;
                let lora_report = refresh(lora_refresher, 7, &corpus, storage.clone())?;
                report.lora_refresh_reports.push(lora_report);
            }
            PerCellIntervention::QueueFullRetrainApproval => {
                let queued = queue_pending_retrain_request(
                    storage.as_ref(),
                    PendingPromotionKind::CatastrophicFullRetrainRequired {
                        cell_key: cell.cell_key.clone(),
                        metric_value: cell.metric_value.unwrap_or(0.0),
                    },
                    TriggerReason::DriftCatastrophic,
                    "per-cell catastrophic drift requires full retrain before promotion approval",
                )?;
                report.queued_catastrophic_promotions.push(queued);
            }
        }
    }
    persist_per_cell_report(storage.as_ref(), &report)?;
    Ok(report)
}

pub fn build_per_cell_drift_report(
    storage: &HealRocksStore,
    config: &PerCellDriftConfig,
) -> Result<PerCellDriftReport, HealError> {
    config.validate()?;
    let observations = latest_observations(storage, config.max_window_observations)?;
    let mut by_cell = BTreeMap::<String, Vec<PerCellDriftObservation>>::new();
    for observation in &observations {
        by_cell
            .entry(observation.cell.key())
            .or_default()
            .push(observation.clone());
    }
    let mut cells = BTreeMap::new();
    for (key, window) in by_cell {
        let report = classify_observation_window(&key, &window, config)?;
        cells.insert(key, report);
    }

    let latest_eval = latest_eval_report(storage.db().as_ref())?;
    if let Some(eval) = &latest_eval {
        for (key, value) in &eval.per_cell_correlation {
            let metric = value.map(correlation_to_unit);
            let report = classify_eval_cell(key, metric, config)?;
            cells.entry(key.clone()).or_insert(report);
        }
    }

    Ok(PerCellDriftReport {
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        observation_count: observations.len(),
        latest_eval_report_seen: latest_eval.is_some(),
        cells,
        lora_refresh_reports: Vec::new(),
        queued_catastrophic_promotions: Vec::new(),
    })
}

fn classify_observation_window(
    key: &str,
    window: &[PerCellDriftObservation],
    config: &PerCellDriftConfig,
) -> Result<PerCellDriftCellReport, HealError> {
    let weighted_trials = window
        .iter()
        .map(|obs| obs.surprise_weight as u64)
        .sum::<u64>();
    let weighted_successes = window
        .iter()
        .filter(|obs| obs.covered)
        .map(|obs| obs.surprise_weight as u64)
        .sum::<u64>();
    let metric = if weighted_trials == 0 {
        None
    } else {
        Some(weighted_successes as f32 / weighted_trials as f32)
    };
    let (severity, bayesian) = classify_metric(
        weighted_successes,
        weighted_trials,
        window.len(),
        metric,
        config,
    )?;
    Ok(PerCellDriftCellReport {
        cell_key: key.to_string(),
        sample_count: window.len(),
        weighted_successes,
        weighted_trials,
        metric_value: metric,
        metric_source: "CF_MEJEPA_MODEL_PROMOTIONS phase-e/cell-observation".to_string(),
        severity,
        bayesian,
        intervention: intervention_for(severity),
    })
}

fn classify_eval_cell(
    key: &str,
    metric: Option<f32>,
    config: &PerCellDriftConfig,
) -> Result<PerCellDriftCellReport, HealError> {
    let severity = match metric {
        Some(value) => config.severity_table.classify(value),
        None => DriftSeverity::WarmupNotReady,
    };
    Ok(PerCellDriftCellReport {
        cell_key: key.to_string(),
        sample_count: 0,
        weighted_successes: 0,
        weighted_trials: 0,
        metric_value: metric,
        metric_source: "CF_MEJEPA_EVAL_REPORTS per_cell_correlation".to_string(),
        severity,
        bayesian: None,
        intervention: intervention_for(severity),
    })
}

fn classify_metric(
    successes: u64,
    trials: u64,
    sample_count: usize,
    metric: Option<f32>,
    config: &PerCellDriftConfig,
) -> Result<(DriftSeverity, Option<BayesianDriftDecision>), HealError> {
    if trials == 0 || metric.is_none() {
        return Ok((DriftSeverity::WarmupNotReady, None));
    }
    if sample_count < config.small_window_cutoff {
        let hard = jeffreys_posterior_below_threshold(
            successes,
            trials,
            config.catastrophic_threshold,
            config.bayesian_confidence,
        )?;
        if hard.fires {
            return Ok((DriftSeverity::Catastrophic, Some(hard)));
        }
        let soft = jeffreys_posterior_below_threshold(
            successes,
            trials,
            config.severity_table.soft_min,
            config.bayesian_confidence,
        )?;
        if soft.fires {
            return Ok((DriftSeverity::Hard, Some(soft)));
        }
        return Ok((DriftSeverity::Healthy, Some(soft)));
    }
    if sample_count < config.min_large_window_samples {
        return Ok((DriftSeverity::WarmupNotReady, None));
    }
    Ok((
        config.severity_table.classify(metric.unwrap_or_default()),
        None,
    ))
}

fn intervention_for(severity: DriftSeverity) -> PerCellIntervention {
    match severity {
        DriftSeverity::Soft | DriftSeverity::Hard => PerCellIntervention::QueueLoraRefresh,
        DriftSeverity::Catastrophic => PerCellIntervention::QueueFullRetrainApproval,
        DriftSeverity::WarmupNotReady | DriftSeverity::Healthy => PerCellIntervention::NoChange,
    }
}

fn latest_observations(
    storage: &HealRocksStore,
    max_window_observations: usize,
) -> Result<Vec<PerCellDriftObservation>, HealError> {
    let mut records =
        scan_policy_records::<PerCellDriftObservation>(storage, CELL_OBSERVATION_PREFIX)?;
    if records.len() > max_window_observations {
        records.drain(0..records.len() - max_window_observations);
    }
    Ok(records.into_iter().map(|(_, obs)| obs).collect())
}

fn latest_eval_report(db: &DB) -> Result<Option<EvalReport>, HealError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS)
        .ok_or_else(|| {
            HealError::invalid("per_cell_drift.eval_cf", "missing CF_MEJEPA_EVAL_REPORTS")
        })?;
    let mut iter = db.iterator_cf(cf, IteratorMode::End);
    let Some(item) = iter.next() else {
        return Ok(None);
    };
    let (_key, value) = item?;
    let report: EvalReport = bincode::deserialize(&value)?;
    report.validate().map_err(|err| {
        HealError::invalid(
            "per_cell_drift.eval_report",
            format!("{}: {err}", err.code()),
        )
    })?;
    Ok(Some(report))
}

fn correlation_to_unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn persist_per_cell_report(
    storage: &HealRocksStore,
    report: &PerCellDriftReport,
) -> Result<(), HealError> {
    let key = report_key(
        CELL_REPORT_PREFIX,
        report.generated_at_unix_ms,
        &report.cells,
    )?;
    persist_policy_record(storage, &key, report)
}

fn observation_key(observation: &PerCellDriftObservation) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(observation.cell.key().as_bytes());
    hasher.update([observation.covered as u8]);
    hasher.update([observation.surprise_weight]);
    hasher.update(observation.witness_chain_offset.to_be_bytes());
    hasher.update(observation.observed_at_unix_ms.to_be_bytes());
    hasher.update(observation.source.as_bytes());
    format!(
        "{}{:020}-{}",
        std::str::from_utf8(CELL_OBSERVATION_PREFIX).expect("ascii prefix"),
        observation.observed_at_unix_ms,
        hex::encode(hasher.finalize())
    )
    .into_bytes()
}

fn report_key<T: Serialize>(prefix: &[u8], ts: i64, value: &T) -> Result<Vec<u8>, HealError> {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(value)?);
    Ok(format!(
        "{}{:020}-{}",
        std::str::from_utf8(prefix).expect("ascii prefix"),
        ts,
        hex::encode(hasher.finalize())
    )
    .into_bytes())
}

fn observation_sample_hash(
    cell: DriftCell,
    covered: bool,
    surprise_weight: u8,
    witness_chain_offset: u64,
    observed_at_unix_ms: i64,
    source: &str,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(cell.key().as_bytes());
    hasher.update([covered as u8]);
    hasher.update([surprise_weight]);
    hasher.update(witness_chain_offset.to_be_bytes());
    hasher.update(observed_at_unix_ms.to_be_bytes());
    hasher.update(source.as_bytes());
    hasher.finalize().into()
}

fn corpus_slice_for_cell(
    storage: &HealRocksStore,
    cell_key: &str,
    weighted_trials: u64,
) -> Result<CorpusSlice, HealError> {
    let mut observations = latest_observations(storage, weighted_trials.clamp(1, 128) as usize)?;
    observations.retain(|observation| observation.cell.key() == cell_key);
    if observations.is_empty() {
        return Err(HealError::invalid(
            "per_cell_drift.corpus_slice",
            format!("no persisted observations found for cell {cell_key}"),
        ));
    }
    let hashes = observations
        .into_iter()
        .rev()
        .map(|observation| observation.sample_hash)
        .collect::<Vec<_>>();
    CorpusSlice::try_new(
        hashes,
        format!("per-cell-drift-observations::{cell_key}::{weighted_trials}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::{ColumnFamilyDescriptor, Options};

    fn open_db(path: &std::path::Path) -> Arc<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        Arc::new(
            DB::open_cf_descriptors(
                &opts,
                path,
                context_graph_mejepa_cf::all_hygiene_referenced_cfs()
                    .into_iter()
                    .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn per_cell_small_window_noisy_sample_does_not_fire() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_db(&temp.path().join("db"));
        let storage = HealRocksStore::from_db(db.clone()).unwrap();
        let cell = DriftCell {
            mutation_category: MutationCategory::OffByOne,
            language: Language::Python,
        };
        for idx in 0..10 {
            persist_per_cell_observation(
                storage.as_ref(),
                &PerCellDriftObservation::try_new(
                    cell,
                    idx < 7,
                    1,
                    idx,
                    1_778_000_000_000 + idx as i64,
                    "real-rocksdb-readback",
                )
                .unwrap(),
            )
            .unwrap();
        }
        let report =
            build_per_cell_drift_report(storage.as_ref(), &PerCellDriftConfig::default()).unwrap();
        let cell_report = report.cells.get(&cell.key()).unwrap();
        assert_eq!(cell_report.intervention, PerCellIntervention::NoChange);
    }

    #[test]
    fn per_cell_small_window_catastrophic_sample_queues_retrain() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_db(&temp.path().join("db"));
        let storage = HealRocksStore::from_db(db.clone()).unwrap();
        let cell = DriftCell {
            mutation_category: MutationCategory::CompileError,
            language: Language::Rust,
        };
        for idx in 0..10 {
            persist_per_cell_observation(
                storage.as_ref(),
                &PerCellDriftObservation::try_new(
                    cell,
                    false,
                    1,
                    idx,
                    1_778_000_001_000 + idx as i64,
                    "real-rocksdb-readback",
                )
                .unwrap(),
            )
            .unwrap();
        }
        let report =
            build_per_cell_drift_report(storage.as_ref(), &PerCellDriftConfig::default()).unwrap();
        let cell_report = report.cells.get(&cell.key()).unwrap();
        assert_eq!(cell_report.severity, DriftSeverity::Catastrophic);
        assert_eq!(
            cell_report.intervention,
            PerCellIntervention::QueueFullRetrainApproval
        );
    }
}
