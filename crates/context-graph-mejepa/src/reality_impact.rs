use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_REALITY_IMPACT};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::{
    ObservedTestOutcome, PredictedTestOutcome, PredictionCorrectness, RealityImpact,
    RealityPrediction, ShiftEntry, TestId, TestOutcome,
};

pub const REALITY_IMPACT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS: i64 = 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealityImpactItemKind {
    FileChange,
    FailedTest,
    EdgeCase,
    DeadCode,
    PerfRegression,
    SecurityConcern,
    AccuracyDegradation,
    CostRegression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealityImpactClassification {
    Confirmed,
    Missed,
    NotYetObserved,
    Surprise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactPredictedItem {
    pub item_id: String,
    pub kind: RealityImpactItemKind,
    pub target: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactObservedItem {
    pub shift_id: String,
    pub kind: RealityImpactItemKind,
    pub target: String,
    pub outcome: Option<TestOutcome>,
    pub file: Option<PathBuf>,
    pub timestamp_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactMatch {
    pub predicted: RealityImpactPredictedItem,
    pub observed: RealityImpactObservedItem,
    pub classification: RealityImpactClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactMiss {
    pub predicted: RealityImpactPredictedItem,
    pub classification: RealityImpactClassification,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactSurprise {
    pub observed: RealityImpactObservedItem,
    pub classification: RealityImpactClassification,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpactRecord {
    pub schema_version: u32,
    pub prediction_id: [u8; 16],
    pub session_id: [u8; 16],
    pub window_start_unix_ms: i64,
    pub window_end_unix_ms: i64,
    pub source_shift_log_path: PathBuf,
    pub source_prediction_cf: String,
    pub source_impact_cf: String,
    pub matched: Vec<RealityImpactMatch>,
    pub missed: Vec<RealityImpactMiss>,
    pub not_yet_observed: Vec<RealityImpactMiss>,
    pub surprises: Vec<RealityImpactSurprise>,
    pub shift_count: usize,
    pub observed_test_count: usize,
    pub created_at_unix_ms: i64,
}

impl RealityImpactRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != REALITY_IMPACT_SCHEMA_VERSION {
            return Err(invalid_input(
                "reality_impact.schema_version",
                format!(
                    "expected {}, got {}",
                    REALITY_IMPACT_SCHEMA_VERSION, self.schema_version
                ),
            ));
        }
        if self.prediction_id.iter().all(|byte| *byte == 0) {
            return Err(invalid_input(
                "reality_impact.prediction_id",
                "prediction_id must be non-zero",
            ));
        }
        if self.session_id.iter().all(|byte| *byte == 0) {
            return Err(invalid_input(
                "reality_impact.session_id",
                "session_id must be non-zero",
            ));
        }
        if self.window_end_unix_ms < self.window_start_unix_ms {
            return Err(invalid_input(
                "reality_impact.window",
                "window_end_unix_ms must be >= window_start_unix_ms",
            ));
        }
        if self.source_prediction_cf != CF_MEJEPA_LIVE_PREDICTIONS {
            return Err(invalid_input(
                "reality_impact.source_prediction_cf",
                format!("expected {CF_MEJEPA_LIVE_PREDICTIONS}"),
            ));
        }
        if self.source_impact_cf != CF_MEJEPA_REALITY_IMPACT {
            return Err(invalid_input(
                "reality_impact.source_impact_cf",
                format!("expected {CF_MEJEPA_REALITY_IMPACT}"),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParsedShiftLogEntry {
    shift_id: String,
    timestamp_unix_ms: i64,
    file: Option<PathBuf>,
    before_sha: Option<[u8; 32]>,
    after_sha: Option<[u8; 32]>,
    test_outcome: Option<ObservedTestOutcome>,
    declared_sha256: Option<String>,
    prev_sha256: Option<String>,
}

pub fn reconcile_reality_impact(
    prediction: &RealityPrediction,
    observed_shifts: Vec<ShiftEntry>,
    observed_test_outcomes: Vec<ObservedTestOutcome>,
) -> Result<RealityImpact, MejepaInferError> {
    let predicted_files_changed = prediction_covered_files(prediction);
    let observed_files_changed = observed_shifts
        .iter()
        .map(|shift| shift.file.clone())
        .collect::<Vec<_>>();
    let predicted_set = predicted_files_changed.iter().collect::<BTreeSet<_>>();
    let unexpected_files_changed = observed_files_changed
        .iter()
        .filter(|file| !predicted_set.contains(file))
        .cloned()
        .collect::<Vec<_>>();
    let prediction_correctness = classify_correctness(
        &prediction.predicted_failed_tests,
        &observed_test_outcomes,
        unexpected_files_changed.is_empty(),
    );
    let impact = RealityImpact {
        observed_shifts,
        predicted_files_changed,
        observed_files_changed,
        unexpected_files_changed,
        predicted_test_outcomes: prediction.predicted_failed_tests.clone(),
        observed_test_outcomes,
        prediction_correctness,
    };
    impact.validate()?;
    Ok(impact)
}

pub fn prediction_covered_files(prediction: &RealityPrediction) -> Vec<PathBuf> {
    let mut out = prediction
        .covered_chunks
        .iter()
        .filter_map(|chunk| chunk.0.split('#').next())
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

pub fn replay_and_persist_reality_impact(
    db: &DB,
    prediction_id: [u8; 16],
    runtime_or_shift_log_root: impl AsRef<Path>,
    replay_window_ms: i64,
    created_at_unix_ms: i64,
) -> Result<RealityImpactRecord, MejepaInferError> {
    let prediction = read_live_prediction_by_id(db, prediction_id)?;
    let record = replay_reality_impact_for_prediction(
        &prediction,
        runtime_or_shift_log_root,
        replay_window_ms,
        created_at_unix_ms,
    )?;
    write_reality_impact_record(db, &record)?;
    read_reality_impact_record(db, prediction_id)?.ok_or_else(|| {
        invalid_input(
            "reality_impact.readback",
            "missing CF_MEJEPA_REALITY_IMPACT row after write",
        )
    })
}

pub fn replay_reality_impact_for_prediction(
    prediction: &RealityPrediction,
    runtime_or_shift_log_root: impl AsRef<Path>,
    replay_window_ms: i64,
    created_at_unix_ms: i64,
) -> Result<RealityImpactRecord, MejepaInferError> {
    if replay_window_ms <= 0 {
        return Err(invalid_input(
            "reality_impact.replay_window_ms",
            format!("replay_window_ms must be positive, got {replay_window_ms}"),
        ));
    }
    let shift_log_path =
        shift_log_path_for_session(runtime_or_shift_log_root, prediction.session_id);
    let entries = read_shift_log_entries(&shift_log_path)?;
    let window_start = prediction.created_at_unix_ms;
    let window_end = window_start.saturating_add(replay_window_ms);
    let entries = entries
        .into_iter()
        .filter(|entry| {
            entry.timestamp_unix_ms >= window_start && entry.timestamp_unix_ms <= window_end
        })
        .collect::<Vec<_>>();
    let predicted_items = predicted_impact_items(prediction);
    let observed_items = observed_impact_items(&entries);
    let observed_test_count = entries
        .iter()
        .filter(|entry| entry.test_outcome.is_some())
        .count();

    let mut matched = Vec::new();
    let mut missed = Vec::new();
    let mut not_yet_observed = Vec::new();
    let mut used_observations = BTreeSet::new();

    for predicted in predicted_items {
        let maybe_match = observed_items.iter().enumerate().find(|(idx, observed)| {
            !used_observations.contains(idx) && observation_matches(&predicted, observed)
        });
        if let Some((idx, observed)) = maybe_match {
            used_observations.insert(idx);
            matched.push(RealityImpactMatch {
                predicted,
                observed: observed.clone(),
                classification: RealityImpactClassification::Confirmed,
            });
        } else if entries.is_empty() {
            not_yet_observed.push(RealityImpactMiss {
                predicted,
                classification: RealityImpactClassification::NotYetObserved,
                reason: "REPLAY_WINDOW_EMPTY".to_string(),
            });
        } else {
            missed.push(RealityImpactMiss {
                reason: miss_reason(predicted.kind).to_string(),
                predicted,
                classification: RealityImpactClassification::Missed,
            });
        }
    }

    let predicted_targets = matched
        .iter()
        .map(|item| (item.predicted.kind, item.predicted.target.as_str()))
        .chain(
            missed
                .iter()
                .map(|item| (item.predicted.kind, item.predicted.target.as_str())),
        )
        .chain(
            not_yet_observed
                .iter()
                .map(|item| (item.predicted.kind, item.predicted.target.as_str())),
        )
        .collect::<BTreeSet<_>>();
    let surprises = observed_items
        .iter()
        .enumerate()
        .filter(|(idx, observed)| {
            !used_observations.contains(idx)
                && !predicted_targets.contains(&(observed.kind, observed.target.as_str()))
        })
        .map(|(_, observed)| RealityImpactSurprise {
            observed: observed.clone(),
            classification: RealityImpactClassification::Surprise,
            reason: surprise_reason(observed.kind).to_string(),
        })
        .collect::<Vec<_>>();

    let record = RealityImpactRecord {
        schema_version: REALITY_IMPACT_SCHEMA_VERSION,
        prediction_id: prediction.prediction_id,
        session_id: prediction.session_id,
        window_start_unix_ms: window_start,
        window_end_unix_ms: window_end,
        source_shift_log_path: shift_log_path,
        source_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_impact_cf: CF_MEJEPA_REALITY_IMPACT.to_string(),
        matched,
        missed,
        not_yet_observed,
        surprises,
        shift_count: entries.len(),
        observed_test_count,
        created_at_unix_ms,
    };
    record.validate()?;
    Ok(record)
}

pub fn write_reality_impact_record(
    db: &DB,
    record: &RealityImpactRecord,
) -> Result<(), MejepaInferError> {
    record.validate()?;
    let cf = cf(db, CF_MEJEPA_REALITY_IMPACT)?;
    let value = bincode::serialize(record)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, record.prediction_id, &value, &opts)?;
    let readback = db.get_cf(cf, record.prediction_id)?.ok_or_else(|| {
        invalid_input(
            "reality_impact.readback",
            "missing CF_MEJEPA_REALITY_IMPACT row after write",
        )
    })?;
    if readback != value {
        return Err(invalid_input(
            "reality_impact.readback",
            "CF_MEJEPA_REALITY_IMPACT readback bytes differ",
        ));
    }
    let decoded: RealityImpactRecord = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *record {
        return Err(invalid_input(
            "reality_impact.readback",
            "CF_MEJEPA_REALITY_IMPACT decoded readback differs",
        ));
    }
    Ok(())
}

pub fn read_reality_impact_record(
    db: &DB,
    prediction_id: [u8; 16],
) -> Result<Option<RealityImpactRecord>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_REALITY_IMPACT)?;
    let Some(value) = db.get_cf(cf, prediction_id)? else {
        return Ok(None);
    };
    let record: RealityImpactRecord = bincode::deserialize(&value)?;
    record.validate()?;
    if record.prediction_id != prediction_id {
        return Err(invalid_input(
            "reality_impact.prediction_id",
            "CF_MEJEPA_REALITY_IMPACT key does not match payload prediction_id",
        ));
    }
    Ok(Some(record))
}

pub fn read_live_prediction_by_id(
    db: &DB,
    prediction_id: [u8; 16],
) -> Result<RealityPrediction, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.len() != 40 {
            return Err(MejepaInferError::DimMismatch {
                expected: 40,
                actual: key.len(),
                context: "live prediction key must be session_id || created_at || prediction_id"
                    .to_string(),
            });
        }
        if key[24..40] != prediction_id {
            continue;
        }
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate()?;
        if prediction.prediction_id != prediction_id {
            return Err(invalid_input(
                "live_predictions.prediction_id",
                "payload prediction_id does not match key suffix",
            ));
        }
        if prediction.session_id != key[0..16] {
            return Err(invalid_input(
                "live_predictions.session_id",
                "payload session_id does not match key prefix",
            ));
        }
        let mut created_at = [0u8; 8];
        created_at.copy_from_slice(&key[16..24]);
        if prediction.created_at_unix_ms != i64::from_be_bytes(created_at) {
            return Err(invalid_input(
                "live_predictions.created_at_unix_ms",
                "payload created_at_unix_ms does not match key timestamp",
            ));
        }
        return Ok(prediction);
    }
    Err(invalid_input(
        "prediction_id",
        format!(
            "prediction_id {} was not found in {CF_MEJEPA_LIVE_PREDICTIONS}",
            hex::encode(prediction_id)
        ),
    ))
}

fn classify_correctness(
    predicted: &[PredictedTestOutcome],
    observed: &[ObservedTestOutcome],
    files_aligned: bool,
) -> PredictionCorrectness {
    let predicted_failures = predicted
        .iter()
        .filter(|item| {
            matches!(
                item.predicted_outcome,
                TestOutcome::Fail | TestOutcome::Error
            )
        })
        .count();
    let observed_failures = observed
        .iter()
        .filter(|item| matches!(item.outcome, TestOutcome::Fail | TestOutcome::Error))
        .count();
    if !files_aligned || observed_failures > predicted_failures + 2 {
        return PredictionCorrectness::Surprise;
    }
    match predicted_failures.cmp(&observed_failures) {
        std::cmp::Ordering::Equal => PredictionCorrectness::Aligned,
        std::cmp::Ordering::Less => PredictionCorrectness::UnderPredicted,
        std::cmp::Ordering::Greater => PredictionCorrectness::OverPredicted,
    }
}

fn predicted_impact_items(prediction: &RealityPrediction) -> Vec<RealityImpactPredictedItem> {
    let mut items = Vec::new();
    let mut ids = BTreeSet::new();
    for file in prediction_covered_files(prediction) {
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("file:{}", file.display()),
                kind: RealityImpactItemKind::FileChange,
                target: file.display().to_string(),
                detail: "covered chunk file expected to be touched by this prediction".to_string(),
            },
        );
    }
    for item in &prediction.predicted_failed_tests {
        if matches!(
            item.predicted_outcome,
            TestOutcome::Fail | TestOutcome::Error
        ) {
            push_predicted_item(
                &mut items,
                &mut ids,
                RealityImpactPredictedItem {
                    item_id: format!("failed_test:{}", item.test_id.0),
                    kind: RealityImpactItemKind::FailedTest,
                    target: item.test_id.0.clone(),
                    detail: format!("predicted_outcome={:?}", item.predicted_outcome),
                },
            );
        }
    }
    for item in &prediction.predicted_edge_cases {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("edge_case:{target}:{:?}", item.edge_class),
                kind: RealityImpactItemKind::EdgeCase,
                target,
                detail: format!("edge_class={:?}", item.edge_class),
            },
        );
    }
    for item in &prediction.predicted_dead_code {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("dead_code:{target}:{:?}", item.kind),
                kind: RealityImpactItemKind::DeadCode,
                target,
                detail: format!("kind={:?}", item.kind),
            },
        );
    }
    for item in &prediction.predicted_perf_regressions {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("perf:{target}:{:?}", item.axis),
                kind: RealityImpactItemKind::PerfRegression,
                target,
                detail: format!(
                    "axis={:?} delta_pct={}",
                    item.axis, item.predicted_delta_pct
                ),
            },
        );
    }
    for item in &prediction.predicted_security_concerns {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("security:{target}:{:?}", item.class),
                kind: RealityImpactItemKind::SecurityConcern,
                target,
                detail: format!("class={:?}", item.class),
            },
        );
    }
    for item in &prediction.predicted_accuracy_degradations {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("accuracy:{target}:{:?}", item.metric),
                kind: RealityImpactItemKind::AccuracyDegradation,
                target,
                detail: format!("metric={:?} delta={}", item.metric, item.predicted_delta),
            },
        );
    }
    for item in &prediction.predicted_cost_regressions {
        let target = chunk_file_target(&item.chunk.0);
        push_predicted_item(
            &mut items,
            &mut ids,
            RealityImpactPredictedItem {
                item_id: format!("cost:{target}:{:?}", item.axis),
                kind: RealityImpactItemKind::CostRegression,
                target,
                detail: format!("axis={:?} delta={}", item.axis, item.predicted_delta),
            },
        );
    }
    items
}

fn push_predicted_item(
    items: &mut Vec<RealityImpactPredictedItem>,
    ids: &mut BTreeSet<String>,
    item: RealityImpactPredictedItem,
) {
    if ids.insert(item.item_id.clone()) {
        items.push(item);
    }
}

fn observed_impact_items(entries: &[ParsedShiftLogEntry]) -> Vec<RealityImpactObservedItem> {
    let mut out = Vec::new();
    for entry in entries {
        if let Some(file) = &entry.file {
            out.push(RealityImpactObservedItem {
                shift_id: entry.shift_id.clone(),
                kind: RealityImpactItemKind::FileChange,
                target: file.display().to_string(),
                outcome: None,
                file: Some(file.clone()),
                timestamp_unix_ms: entry.timestamp_unix_ms,
            });
        }
        if let Some(test) = &entry.test_outcome {
            if matches!(test.outcome, TestOutcome::Fail | TestOutcome::Error) {
                out.push(RealityImpactObservedItem {
                    shift_id: entry.shift_id.clone(),
                    kind: RealityImpactItemKind::FailedTest,
                    target: test.test_id.0.clone(),
                    outcome: Some(test.outcome),
                    file: entry.file.clone(),
                    timestamp_unix_ms: entry.timestamp_unix_ms,
                });
            }
        }
    }
    out
}

fn observation_matches(
    predicted: &RealityImpactPredictedItem,
    observed: &RealityImpactObservedItem,
) -> bool {
    predicted.kind == observed.kind && predicted.target == observed.target
}

fn miss_reason(kind: RealityImpactItemKind) -> &'static str {
    match kind {
        RealityImpactItemKind::FileChange => "FILE_NEVER_CHANGED",
        RealityImpactItemKind::FailedTest => "TEST_NEVER_RAN",
        RealityImpactItemKind::EdgeCase => "EDGE_CASE_NEVER_OBSERVED",
        RealityImpactItemKind::DeadCode => "DEAD_CODE_NEVER_OBSERVED",
        RealityImpactItemKind::PerfRegression => "PERF_REGRESSION_NEVER_OBSERVED",
        RealityImpactItemKind::SecurityConcern => "SECURITY_CONCERN_NEVER_OBSERVED",
        RealityImpactItemKind::AccuracyDegradation => "ACCURACY_DEGRADATION_NEVER_OBSERVED",
        RealityImpactItemKind::CostRegression => "COST_REGRESSION_NEVER_OBSERVED",
    }
}

fn surprise_reason(kind: RealityImpactItemKind) -> &'static str {
    match kind {
        RealityImpactItemKind::FileChange => "UNPREDICTED_FILE_SHIFT",
        RealityImpactItemKind::FailedTest => "UNPREDICTED_TEST_FAILURE",
        _ => "UNPREDICTED_REALITY_SHIFT",
    }
}

fn chunk_file_target(chunk_id: &str) -> String {
    chunk_id
        .split('#')
        .next()
        .filter(|raw| !raw.is_empty())
        .unwrap_or(chunk_id)
        .to_string()
}

fn shift_log_path_for_session(root: impl AsRef<Path>, session_id: [u8; 16]) -> PathBuf {
    let root = root.as_ref();
    let shift_log_dir = if root
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "cgreality-shift-log")
    {
        root.to_path_buf()
    } else {
        root.join("cgreality-shift-log")
    };
    shift_log_dir.join(format!("{}.jsonl", hex::encode(session_id)))
}

fn read_shift_log_entries(path: &Path) -> Result<Vec<ParsedShiftLogEntry>, MejepaInferError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|err| MejepaInferError::io("read", path, err))?;
    let mut out = Vec::new();
    let mut previous_timestamp = None;
    let mut previous_effective_sha = None::<String>;
    for (line_idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let entry = parse_shift_log_entry(&value, path, line_idx + 1)?;
        if let Some(prev_ts) = previous_timestamp {
            if entry.timestamp_unix_ms < prev_ts {
                return Err(invalid_input(
                    "shift_log.timestamp",
                    format!(
                        "SHIFT_LOG_OUT_OF_ORDER: line {} timestamp {} < previous {} at {}",
                        line_idx + 1,
                        entry.timestamp_unix_ms,
                        prev_ts,
                        path.display()
                    ),
                ));
            }
        }
        if line_idx == 0 {
            if entry
                .prev_sha256
                .as_deref()
                .is_some_and(|sha| !sha.is_empty() && sha != "GENESIS")
            {
                return Err(invalid_input(
                    "shift_log.prev_sha256",
                    format!(
                        "SHIFT_LOG_CHAIN_BROKEN: first line has non-genesis prev_sha256 at {}",
                        path.display()
                    ),
                ));
            }
        } else if let Some(prev) = &entry.prev_sha256 {
            let expected = previous_effective_sha.as_deref().ok_or_else(|| {
                invalid_input(
                    "shift_log.prev_sha256",
                    format!(
                        "SHIFT_LOG_CHAIN_BROKEN: line {} has prev_sha256 but previous line has no effective sha at {}",
                        line_idx + 1,
                        path.display()
                    ),
                )
            })?;
            if prev != expected {
                return Err(invalid_input(
                    "shift_log.prev_sha256",
                    format!(
                        "SHIFT_LOG_CHAIN_BROKEN: line {} prev_sha256={} expected={} at {}",
                        line_idx + 1,
                        prev,
                        expected,
                        path.display()
                    ),
                ));
            }
        }
        previous_timestamp = Some(entry.timestamp_unix_ms);
        previous_effective_sha = Some(
            entry
                .declared_sha256
                .clone()
                .unwrap_or_else(|| hex::encode(Sha256::digest(line.as_bytes()))),
        );
        out.push(entry);
    }
    Ok(out)
}

fn parse_shift_log_entry(
    value: &Value,
    path: &Path,
    line: usize,
) -> Result<ParsedShiftLogEntry, MejepaInferError> {
    let shift_id = string_at_any(value, &["/shift_id", "/shiftId"])
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| {
            invalid_input(
                "shift_log.shift_id",
                format!("missing shift_id at {} line {line}", path.display()),
            )
        })?
        .to_string();
    let timestamp_unix_ms = timestamp_ms(value).ok_or_else(|| {
        invalid_input(
            "shift_log.timestamp",
            format!("missing timestamp at {} line {line}", path.display()),
        )
    })?;
    let file = file_path_from_shift(value);
    let before_sha = string_at_any(
        value,
        &[
            "/before/sha256",
            "/before_sha256",
            "/delta_summary/before_sha256",
            "/deltaSummary/beforeSha256",
        ],
    )
    .and_then(decode_sha256);
    let after_sha = string_at_any(
        value,
        &[
            "/after/sha256",
            "/after_sha256",
            "/delta_summary/after_sha256",
            "/deltaSummary/afterSha256",
        ],
    )
    .and_then(decode_sha256);
    let test_outcome = test_outcome_from_shift(value)?;
    let declared_sha256 = string_at_any(
        value,
        &[
            "/shift_sha256",
            "/shiftSha256",
            "/line_sha256",
            "/lineSha256",
            "/sha256",
        ],
    )
    .map(normalize_sha_text);
    let prev_sha256 = string_at_any(value, &["/prev_sha256", "/prevSha256"])
        .filter(|raw| !raw.is_empty())
        .map(normalize_sha_text);
    Ok(ParsedShiftLogEntry {
        shift_id,
        timestamp_unix_ms,
        file,
        before_sha,
        after_sha,
        test_outcome,
        declared_sha256,
        prev_sha256,
    })
}

fn timestamp_ms(value: &Value) -> Option<i64> {
    if let Some(ms) = value
        .pointer("/timestamp_unix_ms")
        .or_else(|| value.pointer("/timestampUnixMs"))
        .and_then(Value::as_i64)
    {
        return Some(ms);
    }
    let ns = value
        .pointer("/timestamp_unix_ns")
        .or_else(|| value.pointer("/timestampUnixNs"))
        .and_then(Value::as_u64)?;
    Some((ns / 1_000_000) as i64)
}

fn file_path_from_shift(value: &Value) -> Option<PathBuf> {
    string_at_any(
        value,
        &[
            "/subject/path",
            "/subject/file_path",
            "/subject/filePath",
            "/delta_summary/path",
            "/delta_summary/file",
            "/delta_summary/file_path",
            "/deltaSummary/path",
            "/deltaSummary/file",
            "/deltaSummary/filePath",
            "/before/path",
            "/after/path",
        ],
    )
    .or_else(|| {
        string_at_any(
            value,
            &["/delta_summary/artifact", "/deltaSummary/artifact"],
        )
        .and_then(|raw| raw.strip_prefix("file:").or(Some(raw)))
    })
    .map(normalize_path)
}

fn test_outcome_from_shift(value: &Value) -> Result<Option<ObservedTestOutcome>, MejepaInferError> {
    let Some(test_id) = string_at_any(
        value,
        &[
            "/test_id",
            "/testId",
            "/test",
            "/test_name",
            "/testName",
            "/subject/test_id",
            "/subject/testId",
            "/delta_summary/test_id",
            "/delta_summary/testId",
            "/delta_summary/test_name",
            "/deltaSummary/testId",
            "/deltaSummary/testName",
            "/verification/test_id",
            "/verification/testId",
        ],
    )
    .filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    let Some(outcome_raw) = string_at_any(
        value,
        &[
            "/outcome",
            "/test_outcome",
            "/testOutcome",
            "/delta_summary/outcome",
            "/delta_summary/test_outcome",
            "/deltaSummary/outcome",
            "/deltaSummary/testOutcome",
            "/verification/outcome",
            "/verification/test_outcome",
            "/verification/testOutcome",
        ],
    ) else {
        return Ok(None);
    };
    let outcome = parse_test_outcome(outcome_raw)?;
    let duration_ms = value
        .pointer("/duration_ms")
        .or_else(|| value.pointer("/durationMs"))
        .or_else(|| value.pointer("/verification/duration_ms"))
        .or_else(|| value.pointer("/verification/durationMs"))
        .and_then(Value::as_u64);
    Ok(Some(ObservedTestOutcome {
        test_id: TestId(test_id.to_string()),
        outcome,
        duration_ms,
    }))
}

fn parse_test_outcome(raw: &str) -> Result<TestOutcome, MejepaInferError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pass" | "passed" | "ok" => Ok(TestOutcome::Pass),
        "fail" | "failed" | "failure" => Ok(TestOutcome::Fail),
        "error" | "errored" => Ok(TestOutcome::Error),
        "skip" | "skipped" => Ok(TestOutcome::Skip),
        "flaky" => Ok(TestOutcome::Flaky),
        other => Err(invalid_input(
            "shift_log.test_outcome",
            format!("unsupported test outcome {other}"),
        )),
    }
}

fn string_at_any<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

fn normalize_path(raw: &str) -> PathBuf {
    let raw = raw.strip_prefix("file:").unwrap_or(raw);
    let raw = raw.trim_start_matches('/');
    PathBuf::from(raw)
}

fn normalize_sha_text(raw: &str) -> String {
    raw.strip_prefix("sha256:").unwrap_or(raw).to_string()
}

fn decode_sha256(raw: &str) -> Option<[u8; 32]> {
    let raw = normalize_sha_text(raw);
    let mut out = [0u8; 32];
    hex::decode_to_slice(raw, &mut out).ok()?;
    Some(out)
}

fn invalid_input(field: impl Into<String>, detail: impl Into<String>) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;
    use crate::compiler::MejepaStore;
    use crate::store::RocksDbInferStore;
    use crate::types::{
        ChunkId, ConformalSet, EmbedderId, FailureModeClass, Language, OracleOutcome,
        PredictedFailureMode, RealityPrediction, RootCauseClass, Severity, TaskId, TestDeltaKind,
        TestId,
    };
    use std::collections::BTreeMap;

    #[test]
    fn covered_files_dedup_by_chunk_prefix() {
        let prediction = prediction(
            [1; 16],
            [3; 16],
            1,
            vec!["src/lib.rs#0", "src/lib.rs#1"],
            Vec::new(),
        );
        assert_eq!(
            prediction_covered_files(&prediction),
            vec![PathBuf::from("src/lib.rs")]
        );
    }

    #[test]
    fn replay_confirms_failed_test_and_persists_readback() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let prediction_id = [9; 16];
        let session_id = [8; 16];
        let prediction = prediction(
            prediction_id,
            session_id,
            1_000,
            vec!["src/lib.py#0"],
            vec!["tests/test_lib.py::test_handles_edges"],
        );
        store.write_live_prediction(&prediction).unwrap();
        write_shift(
            temp.path(),
            session_id,
            r#"{"shift_id":"shift-1","timestamp_unix_ms":1010,"subject":{"path":"src/lib.py"},"before":{"sha256":"sha256:1111111111111111111111111111111111111111111111111111111111111111"},"after":{"sha256":"sha256:2222222222222222222222222222222222222222222222222222222222222222"},"test_id":"tests/test_lib.py::test_handles_edges","outcome":"failed"}"#,
        );
        let record = replay_and_persist_reality_impact(
            db.as_ref(),
            prediction_id,
            temp.path(),
            10_000,
            2_000,
        )
        .unwrap();
        assert_eq!(record.matched.len(), 2);
        assert_eq!(record.missed.len(), 0);
        assert_eq!(record.surprises.len(), 0);
        assert_eq!(
            read_reality_impact_record(db.as_ref(), prediction_id)
                .unwrap()
                .unwrap(),
            record
        );
    }

    #[test]
    fn replay_empty_window_marks_not_yet_observed() {
        let prediction = prediction(
            [2; 16],
            [3; 16],
            1_000,
            vec!["src/lib.py#0"],
            vec!["tests/test_lib.py::test_missing"],
        );
        let temp = tempfile::tempdir().unwrap();
        let record =
            replay_reality_impact_for_prediction(&prediction, temp.path(), 10_000, 2_000).unwrap();
        assert_eq!(record.matched.len(), 0);
        assert_eq!(record.missed.len(), 0);
        assert_eq!(record.not_yet_observed.len(), 2);
        assert!(record.surprises.is_empty());
    }

    #[test]
    fn replay_shift_sha_chain_break_fails_closed() {
        let prediction = prediction([4; 16], [5; 16], 1_000, vec!["src/lib.py#0"], Vec::new());
        let temp = tempfile::tempdir().unwrap();
        write_shift(
            temp.path(),
            [5; 16],
            r#"{"shift_id":"shift-1","timestamp_unix_ms":1010,"subject":{"path":"src/lib.py"},"shift_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        );
        write_shift(
            temp.path(),
            [5; 16],
            r#"{"shift_id":"shift-2","timestamp_unix_ms":1020,"subject":{"path":"src/lib.py"},"prev_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
        );
        let err = replay_reality_impact_for_prediction(&prediction, temp.path(), 10_000, 2_000)
            .unwrap_err();
        assert!(err.to_string().contains("SHIFT_LOG_CHAIN_BROKEN"));
    }

    #[test]
    fn replay_records_missed_test_and_surprise_file_shift() {
        let prediction = prediction(
            [6; 16],
            [7; 16],
            1_000,
            vec!["src/lib.py#0"],
            vec!["tests/test_lib.py::test_never_ran"],
        );
        let temp = tempfile::tempdir().unwrap();
        write_shift(
            temp.path(),
            [7; 16],
            r#"{"shift_id":"shift-1","timestamp_unix_ms":1010,"subject":{"path":"src/unexpected.py"}}"#,
        );
        let record =
            replay_reality_impact_for_prediction(&prediction, temp.path(), 10_000, 2_000).unwrap();
        assert_eq!(record.matched.len(), 0);
        assert_eq!(record.missed.len(), 2);
        assert!(record
            .missed
            .iter()
            .any(|item| item.reason == "TEST_NEVER_RAN"));
        assert_eq!(record.surprises.len(), 1);
        assert_eq!(record.surprises[0].reason, "UNPREDICTED_FILE_SHIFT");
    }

    fn prediction(
        prediction_id: [u8; 16],
        session_id: [u8; 16],
        created_at_unix_ms: i64,
        covered_chunks: Vec<&str>,
        failed_tests: Vec<&str>,
    ) -> RealityPrediction {
        let failure_mode = PredictedFailureMode {
            failure_class: FailureModeClass::AssertionMismatch,
            chunk: ChunkId("src/lib.py#0".to_string()),
            line_range: (1, 3),
            confidence: 0.9,
            severity: Severity::High,
            explanation: "synthetic known failure".to_string(),
            contributing_embedders: vec![EmbedderId("e1".to_string())],
            root_cause_class: RootCauseClass::LogicError,
        };
        RealityPrediction::try_new(RealityPrediction {
            prediction_id,
            witness_hash: crate::types::WitnessHash([2; 32]),
            task_id: TaskId("task".to_string()),
            session_id,
            language: Language::Python,
            covered_chunks: covered_chunks
                .into_iter()
                .map(|chunk| ChunkId(chunk.to_string()))
                .collect(),
            verdict: crate::types::Verdict::Pass,
            confidence_interval: crate::types::ConformalInterval::default(),
            predicted_oracle_pass: 0.9,
            predicted_test_pass: vec![0.9],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.8,
            degraded_status: false,
            granger_attestations: BTreeMap::new(),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: failed_tests
                .into_iter()
                .map(|test| PredictedTestOutcome {
                    test_id: TestId(test.to_string()),
                    current_outcome: TestOutcome::Pass,
                    predicted_outcome: TestOutcome::Fail,
                    delta_kind: TestDeltaKind::PassToFail,
                    confidence: 0.9,
                    why: failure_mode.clone(),
                })
                .collect(),
            predicted_works: Vec::new(),
            predicted_uncovered_paths: Vec::new(),
            predicted_flaky_tests: Vec::new(),
            guard_violations: Vec::new(),
            per_slot_ood_reasons: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: crate::types::ReasoningClass::Mute,
            agent_claim_graph: crate::types::AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: crate::types::PredictionProvenance::default(),
            source_panel_sha: [4; 32],
            calibration_version: "cal-v1".to_string(),
            created_at_unix_ms,
            matched_fingerprint: None,
            unknown_candidate_id: None,
        })
        .unwrap()
    }

    fn write_shift(root: &Path, session_id: [u8; 16], line: &str) {
        let dir = root.join("cgreality-shift-log");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.jsonl", hex::encode(session_id)));
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }
}
