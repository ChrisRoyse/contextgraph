//! Learning-as-UTL event persistence.
//!
//! Records are stored in `CF_LEARNING_EVENTS` with the canonical
//! `[LEARNING_EVENT_VERSION: u8][bincode-encoded LearningEvent]` layout.
//! Decode rejects version mismatches and malformed event state; no automatic
//! migration is attempted.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::learning::{LearningEvent, LEARNING_EVENT_VERSION};
use rocksdb::{ColumnFamily, IteratorMode};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::teleological::column_families::CF_LEARNING_EVENTS;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    /// Get the learning_events CF handle.
    #[inline]
    fn cf_learning_events(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LEARNING_EVENTS)
            .expect("CF_LEARNING_EVENTS must exist — database initialization failed")
    }

    /// Store or replace a learning event keyed by `event.event_id`.
    pub async fn store_learning_event(&self, event: &LearningEvent) -> CoreResult<()> {
        let payload = encode_learning_event(event)?;
        let cf = self.cf_learning_events();
        let key = event.event_id.as_bytes();

        self.db.put_cf(cf, key, &payload).map_err(|e| {
            error!(
                event_id = %event.event_id,
                error = %e,
                "ROCKSDB ERROR: Failed to store learning event"
            );
            TeleologicalStoreError::rocksdb_op(
                "put_learning_event",
                CF_LEARNING_EVENTS,
                Some(event.event_id),
                e,
            )
        })?;

        debug!(
            event_id = %event.event_id,
            bytes = payload.len(),
            "Stored learning event"
        );
        Ok(())
    }

    /// Retrieve a learning event by UUID.
    pub async fn get_learning_event(&self, id: Uuid) -> CoreResult<Option<LearningEvent>> {
        let cf = self.cf_learning_events();
        match self.db.get_cf(cf, id.as_bytes()) {
            Ok(Some(bytes)) => decode_learning_event(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(event_id = %id, error = %e, "ROCKSDB ERROR: Failed to read learning event");
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_learning_event",
                    CF_LEARNING_EVENTS,
                    Some(id),
                    e,
                )
                .into())
            }
        }
    }

    /// Enumerate all learning event ids.
    pub async fn list_learning_event_ids(&self) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_learning_events();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            match item {
                Ok((key, _value)) => {
                    if key.len() != 16 {
                        warn!(
                            len = key.len(),
                            "Skipping learning event with non-UUID key length"
                        );
                        continue;
                    }
                    let mut buf = [0u8; 16];
                    buf.copy_from_slice(&key);
                    out.push(Uuid::from_bytes(buf));
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in list_learning_event_ids");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "iterate_learning_events",
                        CF_LEARNING_EVENTS,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// Count rows in `CF_LEARNING_EVENTS`.
    pub async fn count_learning_events(&self) -> CoreResult<usize> {
        let cf = self.cf_learning_events();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Delete a learning event. Returns true if a row existed.
    pub async fn delete_learning_event(&self, id: Uuid) -> CoreResult<bool> {
        let cf = self.cf_learning_events();
        let existed = matches!(self.db.get_cf(cf, id.as_bytes()), Ok(Some(_)));
        if !existed {
            return Ok(false);
        }

        self.db.delete_cf(cf, id.as_bytes()).map_err(|e| {
            error!(event_id = %id, error = %e, "ROCKSDB ERROR: Failed to delete learning event");
            TeleologicalStoreError::rocksdb_op(
                "delete_learning_event",
                CF_LEARNING_EVENTS,
                Some(id),
                e,
            )
        })?;
        Ok(true)
    }

    /// Delete every learning event. Returns deleted count.
    pub async fn clear_all_learning_events(&self) -> CoreResult<usize> {
        let ids = self.list_learning_event_ids().await?;
        let cf = self.cf_learning_events();
        for id in &ids {
            self.db.delete_cf(cf, id.as_bytes()).map_err(|e| {
                error!(event_id = %id, error = %e, "ROCKSDB ERROR: Failed to delete learning event during clear");
                TeleologicalStoreError::rocksdb_op(
                    "delete_learning_event",
                    CF_LEARNING_EVENTS,
                    Some(*id),
                    e,
                )
            })?;
        }
        Ok(ids.len())
    }
}

/// Encode a `LearningEvent` with the version-byte prefix.
pub fn encode_learning_event(event: &LearningEvent) -> CoreResult<Vec<u8>> {
    event.validate()?;
    let mut bytes = bincode::serialize(event).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize LearningEvent: {}", e))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(LEARNING_EVENT_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

/// Decode a `LearningEvent`, rejecting version mismatches and invalid state.
pub fn decode_learning_event(bytes: &[u8]) -> CoreResult<LearningEvent> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "learning event payload is empty (missing version byte)".into(),
        ));
    }
    let version = bytes[0];
    if version != LEARNING_EVENT_VERSION {
        return Err(CoreError::SerializationError(format!(
            "learning event version mismatch: got {}, expected {}. No automatic migration is supported.",
            version, LEARNING_EVENT_VERSION
        )));
    }
    let event: LearningEvent = bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize LearningEvent: {}", e))
    })?;
    event.validate()?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::learning::{
        LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
    };
    use context_graph_core::training::NUM_CROSS_CORRELATIONS;

    fn state(value: f32) -> LearningStateSnapshot {
        LearningStateSnapshot {
            topic_profile: [value; 14],
            cross_correlations: vec![0.1; NUM_CROSS_CORRELATIONS],
            retrieval_rank: Some(5),
            embedder_scores: [0.2; 14],
            contradiction_pressure: 0.0,
            integration_confidence: 0.5,
            recurrence_count: 1,
            stability_score: 0.5,
            domain: Some("docs".into()),
            successful_transfer_count: 0,
        }
    }

    fn event() -> LearningEvent {
        LearningEvent::new(
            Uuid::new_v4(),
            vec![Uuid::new_v4()],
            Some("session".into()),
            Some("response".into()),
            Some("task".into()),
            "query".into(),
            "context".into(),
            "response".into(),
            state(0.2),
            state(0.4),
            LearningOutcome {
                label: LearningOutcomeLabel::Useful,
                utility_delta: 0.3,
                correction_required: false,
                reuse_observed: true,
            },
        )
        .unwrap()
    }

    #[test]
    fn encode_decode_learning_event_roundtrip() {
        let event = event();
        let bytes = encode_learning_event(&event).unwrap();
        assert_eq!(bytes[0], LEARNING_EVENT_VERSION);
        let decoded = decode_learning_event(&bytes).unwrap();
        println!(
            "event_id={}, signals={}, delta_e_scalar={}",
            decoded.event_id,
            decoded.signals.len(),
            decoded.features.delta_e_scalar
        );
        assert_eq!(decoded.event_id, event.event_id);
        assert_eq!(decoded.before.topic_profile.len(), 14);
        assert_eq!(
            decoded.before.cross_correlations.len(),
            NUM_CROSS_CORRELATIONS
        );
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let event = event();
        let mut bytes = encode_learning_event(&event).unwrap();
        bytes[0] = LEARNING_EVENT_VERSION + 1;
        let err = decode_learning_event(&bytes).unwrap_err();
        println!("wrong version error={err}");
        assert!(format!("{err}").contains("version mismatch"));
    }

    #[test]
    fn decode_rejects_empty_payload() {
        let err = decode_learning_event(&[]).unwrap_err();
        println!("empty payload error={err}");
        assert!(format!("{err}").contains("empty"));
    }
}
