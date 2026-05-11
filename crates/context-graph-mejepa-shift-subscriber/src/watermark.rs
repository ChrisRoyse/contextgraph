// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::sync::Arc;

use rocksdb::{IteratorMode, WriteOptions, DB};

use crate::models::{encode_session_hex, WatermarkRecord};
use crate::{Result, SubscriberError};

pub struct WatermarkWriter {
    db: Arc<DB>,
}

impl WatermarkWriter {
    pub fn new(db: Arc<DB>) -> Result<Self> {
        if db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
            .is_none()
        {
            return Err(SubscriberError::invalid(
                "rocksdb.column_family",
                "missing CF_MEJEPA_SHIFT_WATERMARK",
            ));
        }
        Ok(Self { db })
    }

    pub fn key_for_session_hex(session_hex: &str) -> String {
        format!("wm:{session_hex}")
    }

    pub fn read(&self, session_id: [u8; 16]) -> Result<Option<WatermarkRecord>> {
        let session_hex = encode_session_hex(session_id);
        let key = Self::key_for_session_hex(&session_hex);
        let cf = self.cf()?;
        let Some(bytes) = self.db.get_cf(cf, key.as_bytes())? else {
            return Ok(None);
        };
        let record: WatermarkRecord = serde_json::from_slice(&bytes)?;
        record.validate()?;
        Ok(Some(record))
    }

    pub fn read_all(&self) -> Result<BTreeMap<String, WatermarkRecord>> {
        let cf = self.cf()?;
        let mut out = BTreeMap::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                SubscriberError::invalid("watermark.key", format!("key is not UTF-8: {err}"))
            })?;
            let record: WatermarkRecord = serde_json::from_slice(&value)?;
            record.validate()?;
            let expected = Self::key_for_session_hex(&record.session_id);
            if key != expected {
                return Err(SubscriberError::invalid(
                    "watermark.key",
                    format!("key {key:?} does not match record session key {expected:?}"),
                ));
            }
            out.insert(record.session_id.clone(), record);
        }
        Ok(out)
    }

    pub fn write_watermark(&self, record: &WatermarkRecord) -> Result<String> {
        record.validate()?;
        let existing = self.read(crate::models::decode_session_hex32(&record.session_id)?)?;
        if let Some(existing) = existing {
            if record.last_consumed_byte_offset < existing.last_consumed_byte_offset {
                return Err(SubscriberError::WatermarkBackwards {
                    session_id: record.session_id.clone(),
                    existing_offset: existing.last_consumed_byte_offset,
                    requested_offset: record.last_consumed_byte_offset,
                });
            }
        }
        let key = Self::key_for_session_hex(&record.session_id);
        let value = serde_json::to_vec(record)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &opts)?;
        let readback = self.db.get_cf(self.cf()?, key.as_bytes())?.ok_or_else(|| {
            SubscriberError::invalid("watermark.readback", "write returned but row is absent")
        })?;
        if readback != value {
            return Err(SubscriberError::invalid(
                "watermark.readback",
                "read-after-write bytes differ from serialized watermark",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> Result<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
            .ok_or_else(|| {
                SubscriberError::invalid(
                    "rocksdb.column_family",
                    "missing CF_MEJEPA_SHIFT_WATERMARK",
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa::open_infer_rocksdb;

    fn record(offset: u64) -> WatermarkRecord {
        WatermarkRecord {
            session_id: "11111111111111111111111111111111".to_string(),
            last_consumed_shift_id: "01J0123456789ABCDEF0123".to_string(),
            last_consumed_byte_offset: offset,
            last_advanced_at_unix_seconds: 1,
            producer_tool_name: Some("unit".to_string()),
            source_log_path: Some("/tmp/unit.jsonl".to_string()),
        }
    }

    #[test]
    fn watermark_write_is_sync_and_monotonic() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let writer = WatermarkWriter::new(db).unwrap();
        let key = writer.write_watermark(&record(10)).unwrap();
        assert_eq!(key, "wm:11111111111111111111111111111111");
        assert_eq!(
            writer
                .read(
                    crate::models::decode_session_hex32("11111111111111111111111111111111")
                        .unwrap()
                )
                .unwrap()
                .unwrap()
                .last_consumed_byte_offset,
            10
        );
        assert_eq!(
            writer.write_watermark(&record(9)).unwrap_err().code(),
            "MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS"
        );
    }
}
