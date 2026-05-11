use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::heal::cf::DriftHistoryRecord;
use crate::heal::errors::HealError;
use crate::types::OracleOutcome;

pub const DEFAULT_DRIFT_WINDOW_SIZE: usize = 1000;
pub const DEFAULT_DRIFT_MIN_DETECTION_SAMPLES: usize = 700;
pub const DEFAULT_HYSTERESIS_WINDOWS: usize = 3;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    #[default]
    WarmupNotReady,
    Healthy,
    Soft,
    Hard,
    Catastrophic,
}

impl DriftSeverity {
    pub fn band_index(&self) -> Option<u8> {
        match self {
            Self::WarmupNotReady => None,
            Self::Healthy => Some(0),
            Self::Soft => Some(1),
            Self::Hard => Some(2),
            Self::Catastrophic => Some(3),
        }
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self, Self::Hard | Self::Catastrophic)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct OracleOutcomeRef(pub [u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftSample {
    pub predicted_set: Vec<OracleOutcome>,
    pub actual_oracle: OracleOutcome,
    pub witness_chain_offset: u64,
    pub ood_score: f32,
    pub signal_clarity: f32,
}

impl DriftSample {
    pub fn try_new(
        predicted_set: Vec<OracleOutcome>,
        actual_oracle: OracleOutcome,
        witness_chain_offset: u64,
        ood_score: f32,
        signal_clarity: f32,
    ) -> Result<Self, HealError> {
        if predicted_set.is_empty() {
            return Err(HealError::invalid(
                "drift_sample.predicted_set",
                "predicted set must be non-empty",
            ));
        }
        if !ood_score.is_finite() || !(0.0..=1.0).contains(&ood_score) {
            return Err(HealError::invalid(
                "drift_sample.ood_score",
                format!("ood_score must be finite in [0,1], got {ood_score}"),
            ));
        }
        if !signal_clarity.is_finite() || !(0.0..=1.0).contains(&signal_clarity) {
            return Err(HealError::invalid(
                "drift_sample.signal_clarity",
                format!("signal_clarity must be finite in [0,1], got {signal_clarity}"),
            ));
        }
        Ok(Self {
            predicted_set,
            actual_oracle,
            witness_chain_offset,
            ood_score,
            signal_clarity,
        })
    }

    pub fn covered(&self) -> bool {
        self.predicted_set.contains(&self.actual_oracle)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SeverityTable {
    pub healthy_min: f32,
    pub healthy_max: f32,
    pub soft_min: f32,
    pub hard_min: f32,
}

impl SeverityTable {
    pub fn try_new(
        healthy_min: f32,
        healthy_max: f32,
        soft_min: f32,
        hard_min: f32,
    ) -> Result<Self, HealError> {
        for (name, value) in [
            ("healthy_min", healthy_min),
            ("healthy_max", healthy_max),
            ("soft_min", soft_min),
            ("hard_min", hard_min),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(HealError::invalid(
                    format!("severity_table.{name}"),
                    format!("threshold must be finite in [0,1], got {value}"),
                ));
            }
        }
        if !(hard_min < soft_min && soft_min < healthy_min && healthy_min < healthy_max) {
            return Err(HealError::invalid(
                "severity_table.order",
                "thresholds must satisfy hard_min < soft_min < healthy_min < healthy_max",
            ));
        }
        Ok(Self {
            healthy_min,
            healthy_max,
            soft_min,
            hard_min,
        })
    }

    pub fn classify(&self, coverage: f32) -> DriftSeverity {
        if coverage >= self.healthy_min && coverage <= self.healthy_max {
            DriftSeverity::Healthy
        } else if coverage >= self.soft_min && coverage < self.healthy_min {
            DriftSeverity::Soft
        } else if coverage >= self.hard_min && coverage < self.soft_min {
            DriftSeverity::Hard
        } else if coverage < self.hard_min {
            DriftSeverity::Catastrophic
        } else {
            DriftSeverity::Healthy
        }
    }
}

impl Default for SeverityTable {
    fn default() -> Self {
        Self::try_new(0.88, 0.92, 0.85, 0.80).expect("default severity table is valid")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RingBuffer<T, const N: usize> {
    data: VecDeque<T>,
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn try_new() -> Result<Self, HealError> {
        if N == 0 {
            return Err(HealError::invalid("ring_buffer.capacity", "N must be > 0"));
        }
        Ok(Self {
            data: VecDeque::with_capacity(N),
        })
    }

    pub fn push(&mut self, item: T) -> Option<T> {
        let evicted = if self.data.len() == N {
            self.data.pop_front()
        } else {
            None
        };
        self.data.push_back(item);
        evicted
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.data.len() == N
    }

    pub fn capacity(&self) -> usize {
        N
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SeverityHistoryEntry {
    pub timestamp_seconds: i64,
    pub severity: DriftSeverity,
    pub empirical_coverage: f32,
    pub window_start_offset: u64,
    pub window_end_offset: u64,
}

impl SeverityHistoryEntry {
    pub fn try_new(
        timestamp_seconds: i64,
        severity: DriftSeverity,
        empirical_coverage: f32,
        window_start_offset: u64,
        window_end_offset: u64,
    ) -> Result<Self, HealError> {
        if !empirical_coverage.is_finite() || !(0.0..=1.0).contains(&empirical_coverage) {
            return Err(HealError::invalid(
                "severity_history.empirical_coverage",
                format!("coverage must be finite in [0,1], got {empirical_coverage}"),
            ));
        }
        if window_end_offset < window_start_offset {
            return Err(HealError::invalid(
                "severity_history.window",
                "end offset must be >= start offset",
            ));
        }
        Ok(Self {
            timestamp_seconds,
            severity,
            empirical_coverage,
            window_start_offset,
            window_end_offset,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftDetector {
    pub window: RingBuffer<DriftSample, DEFAULT_DRIFT_WINDOW_SIZE>,
    pub stated_coverage: f32,
    pub severity_thresholds: SeverityTable,
    pub hysteresis_windows: usize,
    pub min_detection_samples: usize,
    pub last_severity: DriftSeverity,
    pub severity_history: Vec<SeverityHistoryEntry>,
    candidate_severity: Option<DriftSeverity>,
    candidate_count: usize,
    pub last_empirical_coverage: Option<f32>,
}

impl DriftDetector {
    pub fn try_new(
        stated_coverage: f32,
        severity_thresholds: SeverityTable,
    ) -> Result<Self, HealError> {
        if !stated_coverage.is_finite() || !(0.0..=1.0).contains(&stated_coverage) {
            return Err(HealError::invalid(
                "drift_detector.stated_coverage",
                format!("stated_coverage must be in [0,1], got {stated_coverage}"),
            ));
        }
        Ok(Self {
            window: RingBuffer::try_new()?,
            stated_coverage,
            severity_thresholds,
            hysteresis_windows: DEFAULT_HYSTERESIS_WINDOWS,
            min_detection_samples: DEFAULT_DRIFT_MIN_DETECTION_SAMPLES,
            last_severity: DriftSeverity::WarmupNotReady,
            severity_history: Vec::new(),
            candidate_severity: None,
            candidate_count: 0,
            last_empirical_coverage: None,
        })
    }

    pub fn push<S: DriftStore>(
        &mut self,
        sample: DriftSample,
        store: &S,
    ) -> Result<Option<DriftSample>, HealError> {
        store.persist_drift_sample(self.window.len() as u64, &sample)?;
        let evicted = self.window.push(sample);
        Ok(evicted)
    }

    pub fn empirical_coverage(&self) -> Option<f32> {
        let len = self.window.len();
        if len < self.min_detection_samples {
            return None;
        }
        let covered = self.window.iter().filter(|sample| sample.covered()).count();
        Some(covered as f32 / len as f32)
    }

    pub fn detect_drift<H: DriftHistoryStore, F: DriftSurface>(
        &mut self,
        store: &H,
        surface: &F,
    ) -> Result<DriftSeverity, HealError> {
        let coverage = match self.empirical_coverage() {
            Some(value) => value,
            None => return Ok(DriftSeverity::WarmupNotReady),
        };
        self.last_empirical_coverage = Some(coverage);
        let raw = self.severity_thresholds.classify(coverage);
        let next = self.apply_hysteresis(raw);
        if next != self.last_severity {
            let samples = self.window.iter().cloned().collect::<Vec<_>>();
            let record = DriftHistoryRecord::from_sample_window(next, coverage, &samples)?;
            store.persist_drift_history(&record)?;
            let entry = SeverityHistoryEntry::try_new(
                chrono::Utc::now().timestamp(),
                next,
                coverage,
                record.window_start_offset,
                record.window_end_offset,
            )?;
            self.severity_history.push(entry);
            surface.surface_drift(next, coverage)?;
            self.last_severity = next;
        }
        Ok(next)
    }

    fn apply_hysteresis(&mut self, raw: DriftSeverity) -> DriftSeverity {
        if self.last_severity == DriftSeverity::WarmupNotReady {
            return raw;
        }
        let old = self.last_severity.band_index().unwrap_or(0);
        let new = raw.band_index().unwrap_or(old);
        if new.abs_diff(old) >= 2 {
            self.candidate_severity = None;
            self.candidate_count = 0;
            return raw;
        }
        if raw == self.last_severity {
            self.candidate_severity = None;
            self.candidate_count = 0;
            return raw;
        }
        if self.candidate_severity == Some(raw) {
            self.candidate_count += 1;
        } else {
            self.candidate_severity = Some(raw);
            self.candidate_count = 1;
        }
        if self.candidate_count >= self.hysteresis_windows {
            self.candidate_severity = None;
            self.candidate_count = 0;
            raw
        } else {
            self.last_severity
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

pub trait DriftStore {
    fn persist_drift_sample(&self, offset: u64, sample: &DriftSample) -> Result<(), HealError>;
}

pub trait DriftHistoryStore {
    fn persist_drift_history(&self, record: &DriftHistoryRecord) -> Result<(), HealError>;
}

pub trait DriftSurface {
    fn surface_drift(&self, severity: DriftSeverity, coverage: f32) -> Result<(), HealError>;
}

#[derive(Debug, Clone)]
pub struct SmapDriftSurface {
    pub memory_root: PathBuf,
}

impl Default for SmapDriftSurface {
    fn default() -> Self {
        Self {
            memory_root: PathBuf::from("memory"),
        }
    }
}

impl DriftSurface for SmapDriftSurface {
    fn surface_drift(&self, severity: DriftSeverity, coverage: f32) -> Result<(), HealError> {
        fs::create_dir_all(self.memory_root.join("journal")).map_err(|err| {
            HealError::io("create_dir_all", self.memory_root.join("journal"), err)
        })?;
        let ts = chrono::Utc::now();
        let slug =
            format!("{}--mejepa-drift-{:?}.md", ts.format("%Y-%m-%d"), severity).to_lowercase();
        let path = self.memory_root.join("journal").join(slug);
        let body = format!(
            "---\nnamespace: journal\ncreated: {}\nupdated: {}\nstatus: active\ntags: mejepa, phase5, drift\n---\n\n# ME-JEPA Drift Event\n\nseverity: {:?}\nempirical_coverage: {:.6}\n",
            ts.to_rfc3339(),
            ts.to_rfc3339(),
            severity,
            coverage
        );
        fs::write(&path, body).map_err(|err| HealError::io("write", &path, err))?;
        if severity.is_actionable() {
            fs::create_dir_all(self.memory_root.join("blockers")).map_err(|err| {
                HealError::io("create_dir_all", self.memory_root.join("blockers"), err)
            })?;
            let path = self.memory_root.join("blockers").join(
                format!("{}--mejepa-drift-{:?}.md", ts.format("%Y-%m-%d"), severity).to_lowercase(),
            );
            let sev = if severity == DriftSeverity::Catastrophic {
                "critical"
            } else {
                "high"
            };
            let body = format!(
                "---\nnamespace: blockers\ncreated: {}\nupdated: {}\nstatus: active\nseverity: {}\ntags: mejepa, phase5, drift\n---\n\n# ME-JEPA Drift Requires Healing\n\nseverity: {:?}\nempirical_coverage: {:.6}\n",
                ts.to_rfc3339(),
                ts.to_rfc3339(),
                sev,
                severity,
                coverage
            );
            fs::write(&path, body).map_err(|err| HealError::io("write", &path, err))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopDriftSurface;

impl DriftSurface for NoopDriftSurface {
    fn surface_drift(&self, _severity: DriftSeverity, _coverage: f32) -> Result<(), HealError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryDriftStore {
    pub samples: std::sync::Arc<std::sync::Mutex<Vec<DriftSample>>>,
    pub history: std::sync::Arc<std::sync::Mutex<Vec<DriftHistoryRecord>>>,
}

impl DriftStore for MemoryDriftStore {
    fn persist_drift_sample(&self, _offset: u64, sample: &DriftSample) -> Result<(), HealError> {
        self.samples.lock().unwrap().push(sample.clone());
        Ok(())
    }
}

impl DriftHistoryStore for MemoryDriftStore {
    fn persist_drift_history(&self, record: &DriftHistoryRecord) -> Result<(), HealError> {
        self.history.lock().unwrap().push(record.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(covered: bool, offset: u64) -> DriftSample {
        let predicted = if covered {
            vec![OracleOutcome::Pass]
        } else {
            vec![OracleOutcome::Fail]
        };
        DriftSample::try_new(predicted, OracleOutcome::Pass, offset, 0.1, 0.9).unwrap()
    }

    #[test]
    fn ring_buffer_evicts_fifo() {
        let mut buf = RingBuffer::<u8, 2>::try_new().unwrap();
        assert_eq!(buf.push(1), None);
        assert_eq!(buf.push(2), None);
        assert_eq!(buf.push(3), Some(1));
        assert_eq!(buf.iter().copied().collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn severity_table_maps_coverage() {
        let table = SeverityTable::default();
        assert_eq!(table.classify(0.9), DriftSeverity::Healthy);
        assert_eq!(table.classify(0.86), DriftSeverity::Soft);
        assert_eq!(table.classify(0.82), DriftSeverity::Hard);
        assert_eq!(table.classify(0.70), DriftSeverity::Catastrophic);
    }

    #[test]
    fn drift_detects_hard_after_min_window() {
        let store = MemoryDriftStore::default();
        let surface = NoopDriftSurface;
        let mut detector = DriftDetector::try_new(0.9, SeverityTable::default()).unwrap();
        detector.min_detection_samples = 10;
        detector.hysteresis_windows = 1;
        for i in 0..10 {
            detector.push(sample(i < 8, i), &store).unwrap();
        }
        assert_eq!(
            detector.detect_drift(&store, &surface).unwrap(),
            DriftSeverity::Hard
        );
        assert_eq!(store.history.lock().unwrap().len(), 1);
    }

    #[test]
    fn drift_warmup_refuses_early_detection() {
        let store = MemoryDriftStore::default();
        let surface = NoopDriftSurface;
        let mut detector = DriftDetector::try_new(0.9, SeverityTable::default()).unwrap();
        detector.min_detection_samples = 10;
        for i in 0..9 {
            detector.push(sample(false, i), &store).unwrap();
        }
        assert_eq!(
            detector.detect_drift(&store, &surface).unwrap(),
            DriftSeverity::WarmupNotReady
        );
    }
}
