use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};

use crate::calibration::cf;
use crate::error::MejepaInferError;

pub const PARK_LIST_FAILURE_THRESHOLD: u32 = 3;
pub const PARK_LIST_DURATION_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParkListEntry {
    pub prediction_id: [u8; 16],
    pub attempt_count: u32,
    pub last_attempted_at_unix_ms: i64,
    pub park_until_unix_ms: Option<i64>,
    pub last_error_code: String,
}

impl ParkListEntry {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.attempt_count == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "park_list.attempt_count".to_string(),
                detail: "attempt_count must be greater than zero".to_string(),
            });
        }
        if self.last_attempted_at_unix_ms < 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "park_list.last_attempted_at_unix_ms".to_string(),
                detail: "last_attempted_at_unix_ms must be non-negative".to_string(),
            });
        }
        if self.last_error_code.trim().is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "park_list.last_error_code".to_string(),
                detail: "last_error_code must be non-empty".to_string(),
            });
        }
        if let Some(park_until) = self.park_until_unix_ms {
            if park_until <= self.last_attempted_at_unix_ms {
                return Err(MejepaInferError::InvalidInput {
                    field: "park_list.park_until_unix_ms".to_string(),
                    detail: "park_until_unix_ms must be after last_attempted_at_unix_ms"
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    pub fn is_parked_at(&self, now_unix_ms: i64) -> bool {
        self.park_until_unix_ms
            .map(|park_until| now_unix_ms < park_until)
            .unwrap_or(false)
    }
}

pub fn read_park_list_entry(
    db: &DB,
    prediction_id: [u8; 16],
) -> Result<Option<ParkListEntry>, MejepaInferError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)?;
    let Some(bytes) = db.get_cf(cf, prediction_id)? else {
        return Ok(None);
    };
    let entry: ParkListEntry = serde_json::from_slice(&bytes)?;
    entry.validate()?;
    if entry.prediction_id != prediction_id {
        return Err(MejepaInferError::InvalidInput {
            field: "park_list.prediction_id".to_string(),
            detail: "park-list payload prediction_id does not match key".to_string(),
        });
    }
    Ok(Some(entry))
}

pub fn record_park_list_failure(
    db: &DB,
    prediction_id: [u8; 16],
    now_unix_ms: i64,
    error_code: &str,
) -> Result<ParkListEntry, MejepaInferError> {
    if now_unix_ms < 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "park_list.now_unix_ms".to_string(),
            detail: "now_unix_ms must be non-negative".to_string(),
        });
    }
    if error_code.trim().is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: "park_list.error_code".to_string(),
            detail: "error_code must be non-empty".to_string(),
        });
    }
    let previous = read_park_list_entry(db, prediction_id)?;
    let attempt_count = previous
        .map(|entry| entry.attempt_count.saturating_add(1))
        .unwrap_or(1);
    let park_until_unix_ms = if attempt_count >= PARK_LIST_FAILURE_THRESHOLD {
        Some(now_unix_ms.saturating_add(PARK_LIST_DURATION_MS))
    } else {
        None
    };
    let entry = ParkListEntry {
        prediction_id,
        attempt_count,
        last_attempted_at_unix_ms: now_unix_ms,
        park_until_unix_ms,
        last_error_code: error_code.to_string(),
    };
    entry.validate()?;
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)?;
    let value = serde_json::to_vec(&entry)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, prediction_id, &value, &opts)?;
    let readback = db
        .get_cf(cf, prediction_id)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "park_list".to_string(),
            detail: "read-after-write could not find park-list row".to_string(),
        })?;
    if readback != value {
        return Err(MejepaInferError::InvalidInput {
            field: "park_list".to_string(),
            detail: "read-after-write bytes differ from park-list payload".to_string(),
        });
    }
    Ok(entry)
}

pub fn clear_park_list_entry(db: &DB, prediction_id: [u8; 16]) -> Result<(), MejepaInferError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)?;
    db.delete_cf(cf, prediction_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;

    #[test]
    fn third_failure_parks_for_twenty_four_hours() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let prediction_id = [9u8; 16];
        record_park_list_failure(&db, prediction_id, 1_000, "MEJEPA_TEST_FAILURE").unwrap();
        record_park_list_failure(&db, prediction_id, 2_000, "MEJEPA_TEST_FAILURE").unwrap();
        let third =
            record_park_list_failure(&db, prediction_id, 3_000, "MEJEPA_TEST_FAILURE").unwrap();
        assert_eq!(third.attempt_count, PARK_LIST_FAILURE_THRESHOLD);
        assert_eq!(
            third.park_until_unix_ms,
            Some(3_000 + PARK_LIST_DURATION_MS)
        );
        assert!(third.is_parked_at(3_000 + PARK_LIST_DURATION_MS - 1));
        assert!(!third.is_parked_at(3_000 + PARK_LIST_DURATION_MS));
    }
}
