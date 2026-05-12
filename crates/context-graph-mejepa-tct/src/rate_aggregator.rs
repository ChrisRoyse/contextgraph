use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bincode::Options;
use rocksdb::{Direction, IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};

use crate::constellation::bincode_options;
use crate::error::TctError;
use crate::rate::RollingWindow;
use crate::store::cf;
use crate::verdict::VerdictGuardRejected;

pub use context_graph_mejepa_cf::CF_MEJEPA_GUARD_DECISIONS;
pub const MIN_SAMPLES_FOR_RATE: usize = 30;
pub const ENV_RATE_WINDOW_SIZE: &str = "MEJEPA_TCT_RATE_WINDOW_SIZE";
pub const ENV_RATE_WINDOW_HOURS: &str = "MEJEPA_TCT_RATE_WINDOW_HOURS";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictRecord {
    Approve(VerdictApproveSummary),
    GuardRejected(VerdictGuardRejectedSummary),
}

impl VerdictRecord {
    fn timestamp(&self) -> SystemTime {
        match self {
            Self::Approve(value) => value.timestamp,
            Self::GuardRejected(value) => value.timestamp,
        }
    }

    fn is_rejected(&self) -> bool {
        matches!(self, Self::GuardRejected(_))
    }

    fn validate(&self) -> Result<(), TctError> {
        let _ = unix_secs(self.timestamp())?;
        match self {
            Self::Approve(value) => {
                if value.constellation_version_id == [0u8; 32] {
                    return Err(TctError::invalid(
                        "constellation_version_id",
                        "approve verdict version id must be non-zero",
                    ));
                }
            }
            Self::GuardRejected(value) => {
                if value.payload.constellation_version_id == [0u8; 32] {
                    return Err(TctError::invalid(
                        "constellation_version_id",
                        "guard-rejected verdict version id must be non-zero",
                    ));
                }
                if value.payload.violating_embedders.is_empty() {
                    return Err(TctError::ConstellationViolation {
                        detail: "guard-rejected verdict requires at least one violating embedder"
                            .to_string(),
                    });
                }
                if value
                    .payload
                    .predictor_predicted
                    .per_test_probabilities
                    .is_empty()
                {
                    return Err(TctError::InsufficientSamples {
                        cell: "predictor_predicted.per_test_probabilities".to_string(),
                        observed: 0,
                        required: 1,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictApproveSummary {
    pub timestamp: SystemTime,
    pub constellation_version_id: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictGuardRejectedSummary {
    pub timestamp: SystemTime,
    pub payload: VerdictGuardRejected,
}

#[derive(Clone)]
pub struct ViolationRateAggregator {
    db: Arc<DB>,
}

impl ViolationRateAggregator {
    pub fn new(db: Arc<DB>) -> Result<Self, TctError> {
        if db.cf_handle(CF_MEJEPA_GUARD_DECISIONS).is_none() {
            return Err(TctError::store(
                "open",
                CF_MEJEPA_GUARD_DECISIONS,
                "column family missing",
            ));
        }
        Ok(Self { db })
    }

    pub fn record_verdict(&self, record: &VerdictRecord) -> Result<Vec<u8>, TctError> {
        record.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_GUARD_DECISIONS)?;
        let key = verdict_key(record.timestamp())?;
        let bytes = bincode_options().serialize(record)?;
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(cf, &key, bytes, &write_opts)
            .map_err(|err| TctError::store("put", CF_MEJEPA_GUARD_DECISIONS, err.to_string()))?;
        let raw = self
            .db
            .get_cf(cf, &key)
            .map_err(|err| TctError::store("get", CF_MEJEPA_GUARD_DECISIONS, err.to_string()))?
            .ok_or_else(|| {
                TctError::store(
                    "read_after_write",
                    CF_MEJEPA_GUARD_DECISIONS,
                    "missing persisted guard decision row",
                )
            })?;
        let readback: VerdictRecord = bincode_options().deserialize(&raw)?;
        readback.validate()?;
        if readback != *record {
            return Err(TctError::FrozenViolation {
                detail: "guard decision read-after-write payload mismatch".to_string(),
            });
        }
        Ok(key)
    }

    pub fn constellation_violation_rate(
        &self,
        window: RollingWindow,
        now: SystemTime,
    ) -> Result<Option<f32>, TctError> {
        let cutoff = now
            .checked_sub(Duration::from_secs(window.window_hours as u64 * 3600))
            .ok_or_else(|| {
                TctError::invalid(
                    "RollingWindow.window_hours",
                    "window cutoff underflowed UNIX time",
                )
            })?;
        let cutoff_secs = unix_secs(cutoff)?;
        let now_secs = unix_secs(now)?;
        let mut end_key = Vec::with_capacity(24);
        end_key.extend_from_slice(&now_secs.to_be_bytes());
        end_key.extend_from_slice(&[u8::MAX; 16]);
        let cf = cf(&self.db, CF_MEJEPA_GUARD_DECISIONS)?;
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&end_key, Direction::Reverse));
        let mut total = 0usize;
        let mut rejected = 0usize;
        for item in iter {
            if total == window.window_size_calls as usize {
                break;
            }
            let (key, value) = item.map_err(|err| {
                TctError::store("iterate", CF_MEJEPA_GUARD_DECISIONS, err.to_string())
            })?;
            if key.len() < 8 {
                return Err(TctError::dim(
                    8,
                    key.len(),
                    "guard decision key timestamp prefix",
                ));
            }
            let mut ts = [0u8; 8];
            ts.copy_from_slice(&key[..8]);
            let ts_secs = u64::from_be_bytes(ts);
            if ts_secs < cutoff_secs {
                break;
            }
            let record: VerdictRecord = bincode_options().deserialize(&value)?;
            record.validate()?;
            if unix_secs(record.timestamp())? != ts_secs {
                return Err(TctError::FrozenViolation {
                    detail: "guard decision key timestamp does not match payload timestamp"
                        .to_string(),
                });
            }
            if record.is_rejected() {
                rejected += 1;
            }
            total += 1;
        }
        if total < MIN_SAMPLES_FOR_RATE {
            return Ok(None);
        }
        Ok(Some(rejected as f32 / total as f32))
    }

    pub fn count_rows(&self) -> Result<usize, TctError> {
        let cf = cf(&self.db, CF_MEJEPA_GUARD_DECISIONS)?;
        let mut count = 0usize;
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            item.map_err(|err| {
                TctError::store("iterate", CF_MEJEPA_GUARD_DECISIONS, err.to_string())
            })?;
            count += 1;
        }
        Ok(count)
    }
}

pub fn read_window_config() -> Result<RollingWindow, TctError> {
    let default = RollingWindow::default();
    let window_size_calls = match std::env::var(ENV_RATE_WINDOW_SIZE) {
        Ok(value) => value.parse::<u32>().map_err(|err| {
            TctError::invalid(
                ENV_RATE_WINDOW_SIZE,
                format!("must parse as u32, got {value:?}: {err}"),
            )
        })?,
        Err(std::env::VarError::NotPresent) => default.window_size_calls,
        Err(err) => {
            return Err(TctError::invalid(
                ENV_RATE_WINDOW_SIZE,
                format!("failed to read env var: {err}"),
            ));
        }
    };
    let window_hours = match std::env::var(ENV_RATE_WINDOW_HOURS) {
        Ok(value) => value.parse::<u32>().map_err(|err| {
            TctError::invalid(
                ENV_RATE_WINDOW_HOURS,
                format!("must parse as u32, got {value:?}: {err}"),
            )
        })?,
        Err(std::env::VarError::NotPresent) => default.window_hours,
        Err(err) => {
            return Err(TctError::invalid(
                ENV_RATE_WINDOW_HOURS,
                format!("failed to read env var: {err}"),
            ));
        }
    };
    RollingWindow::try_new(window_size_calls, window_hours)
}

fn verdict_key(timestamp: SystemTime) -> Result<Vec<u8>, TctError> {
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&unix_secs(timestamp)?.to_be_bytes());
    key.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    Ok(key)
}

fn unix_secs(timestamp: SystemTime) -> Result<u64, TctError> {
    timestamp
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| TctError::invalid("timestamp", "timestamp is before UNIX_EPOCH"))
}
