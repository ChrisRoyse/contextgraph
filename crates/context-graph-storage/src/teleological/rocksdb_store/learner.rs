//! UTL learner-state persistence.
//!
//! These records are the learner namespace from `docs/07_context_graph_integration.md`.
//! They intentionally do not alter the content `TeleologicalFingerprint` layout.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::learner::{
    GoalCentroid, LearnerAuditEntry, LearnerConstellation, LearnerDeltaLog, LearnerFingerprint,
    LearnerGoalState, LearnerKSleep, LearnerMTrace, LearnerProfile, LearnerRetrievalLog,
    LearnerStateVector, LEARNER_RECORD_VERSION,
};
use rocksdb::{ColumnFamily, IteratorMode};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{error, warn};
use uuid::Uuid;

use crate::teleological::column_families::{
    CF_FINGERPRINTS_LEARNER, CF_GOAL_CENTROIDS, CF_LEARNER_AUDIT, CF_LEARNER_CONSTELLATIONS,
    CF_LEARNER_DELTA_LOG, CF_LEARNER_GOAL_STATES, CF_LEARNER_K_SLEEP, CF_LEARNER_M_PER_TRACE,
    CF_LEARNER_PROFILE, CF_LEARNER_RETRIEVAL_LOG, CF_LEARNER_STATE_HISTORY,
};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    #[inline]
    fn learner_cf(&self, name: &'static str) -> &ColumnFamily {
        self.db
            .cf_handle(name)
            .unwrap_or_else(|| panic!("{name} CF must exist - database initialization failed"))
    }

    pub async fn store_learner_profile(&self, profile: &LearnerProfile) -> CoreResult<()> {
        let payload = encode_learner_profile(profile)?;
        self.put_learner_bytes(CF_LEARNER_PROFILE, profile.learner_id.as_bytes(), &payload)
    }

    pub async fn get_learner_profile(
        &self,
        learner_id: Uuid,
    ) -> CoreResult<Option<LearnerProfile>> {
        self.get_learner_bytes(CF_LEARNER_PROFILE, learner_id.as_bytes())?
            .map(|bytes| decode_learner_profile(&bytes))
            .transpose()
    }

    pub async fn store_learner_fingerprint(
        &self,
        fingerprint: &LearnerFingerprint,
    ) -> CoreResult<()> {
        let payload = encode_learner_fingerprint(fingerprint)?;
        let key = learner_session_key(fingerprint.learner_id, fingerprint.session_ts);
        self.put_learner_bytes(CF_FINGERPRINTS_LEARNER, &key, &payload)?;

        let state_payload = encode_learner_state_vector(&fingerprint.state_vector)?;
        self.put_learner_bytes(CF_LEARNER_STATE_HISTORY, &key, &state_payload)
    }

    pub async fn get_learner_fingerprint(
        &self,
        learner_id: Uuid,
        session_ts: u64,
    ) -> CoreResult<Option<LearnerFingerprint>> {
        let key = learner_session_key(learner_id, session_ts);
        self.get_learner_bytes(CF_FINGERPRINTS_LEARNER, &key)?
            .map(|bytes| decode_learner_fingerprint(&bytes))
            .transpose()
    }

    pub async fn get_learner_state_vector(
        &self,
        learner_id: Uuid,
        session_ts: u64,
    ) -> CoreResult<Option<LearnerStateVector>> {
        let key = learner_session_key(learner_id, session_ts);
        self.get_learner_bytes(CF_LEARNER_STATE_HISTORY, &key)?
            .map(|bytes| decode_learner_state_vector(&bytes))
            .transpose()
    }

    pub async fn list_learner_fingerprint_keys(&self) -> CoreResult<Vec<(Uuid, u64)>> {
        self.list_learner_session_keys(CF_FINGERPRINTS_LEARNER)
    }

    pub async fn store_learner_delta_log(&self, log: &LearnerDeltaLog) -> CoreResult<()> {
        let payload = encode_learner_delta_log(log)?;
        let key = learner_session_key(log.learner_id, log.session_ts);
        self.put_learner_bytes(CF_LEARNER_DELTA_LOG, &key, &payload)
    }

    pub async fn get_learner_delta_log(
        &self,
        learner_id: Uuid,
        session_ts: u64,
    ) -> CoreResult<Option<LearnerDeltaLog>> {
        let key = learner_session_key(learner_id, session_ts);
        self.get_learner_bytes(CF_LEARNER_DELTA_LOG, &key)?
            .map(|bytes| decode_learner_delta_log(&bytes))
            .transpose()
    }

    pub async fn list_learner_delta_log_keys(&self) -> CoreResult<Vec<(Uuid, u64)>> {
        self.list_learner_session_keys(CF_LEARNER_DELTA_LOG)
    }

    pub async fn store_learner_m_trace(&self, trace: &LearnerMTrace) -> CoreResult<()> {
        let payload = encode_learner_m_trace(trace)?;
        let key = learner_trace_key(trace.learner_id, trace.trace_id);
        self.put_learner_bytes(CF_LEARNER_M_PER_TRACE, &key, &payload)
    }

    pub async fn get_learner_m_trace(
        &self,
        learner_id: Uuid,
        trace_id: Uuid,
    ) -> CoreResult<Option<LearnerMTrace>> {
        let key = learner_trace_key(learner_id, trace_id);
        self.get_learner_bytes(CF_LEARNER_M_PER_TRACE, &key)?
            .map(|bytes| decode_learner_m_trace(&bytes))
            .transpose()
    }

    pub async fn store_learner_constellation(
        &self,
        constellation: &LearnerConstellation,
    ) -> CoreResult<()> {
        let payload = encode_learner_constellation(constellation)?;
        let key = learner_constellation_key(constellation.learner_id, constellation.selector_kind);
        self.put_learner_bytes(CF_LEARNER_CONSTELLATIONS, &key, &payload)
    }

    pub async fn get_learner_constellation(
        &self,
        learner_id: Uuid,
        selector_kind: u8,
    ) -> CoreResult<Option<LearnerConstellation>> {
        let key = learner_constellation_key(learner_id, selector_kind);
        self.get_learner_bytes(CF_LEARNER_CONSTELLATIONS, &key)?
            .map(|bytes| decode_learner_constellation(&bytes))
            .transpose()
    }

    pub async fn store_learner_goal_state(&self, goal: &LearnerGoalState) -> CoreResult<()> {
        let payload = encode_learner_goal_state(goal)?;
        let key = learner_skill_key(goal.learner_id, goal.skill_id);
        self.put_learner_bytes(CF_LEARNER_GOAL_STATES, &key, &payload)
    }

    pub async fn get_learner_goal_state(
        &self,
        learner_id: Uuid,
        skill_id: Uuid,
    ) -> CoreResult<Option<LearnerGoalState>> {
        let key = learner_skill_key(learner_id, skill_id);
        self.get_learner_bytes(CF_LEARNER_GOAL_STATES, &key)?
            .map(|bytes| decode_learner_goal_state(&bytes))
            .transpose()
    }

    pub async fn store_learner_retrieval_log(&self, log: &LearnerRetrievalLog) -> CoreResult<()> {
        let payload = encode_learner_retrieval_log(log)?;
        let key = learner_trace_ts_key(log.learner_id, log.trace_id, log.ts);
        self.put_learner_bytes(CF_LEARNER_RETRIEVAL_LOG, &key, &payload)
    }

    pub async fn get_learner_retrieval_log(
        &self,
        learner_id: Uuid,
        trace_id: Uuid,
        ts: u64,
    ) -> CoreResult<Option<LearnerRetrievalLog>> {
        let key = learner_trace_ts_key(learner_id, trace_id, ts);
        self.get_learner_bytes(CF_LEARNER_RETRIEVAL_LOG, &key)?
            .map(|bytes| decode_learner_retrieval_log(&bytes))
            .transpose()
    }

    pub async fn list_learner_retrieval_log_keys(&self) -> CoreResult<Vec<(Uuid, Uuid, u64)>> {
        let cf = self.learner_cf(CF_LEARNER_RETRIEVAL_LOG);
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, _) = item.map_err(|e| {
                error!(
                    cf = CF_LEARNER_RETRIEVAL_LOG,
                    error = %e,
                    "ROCKSDB ERROR: Failed to iterate learner retrieval log keys"
                );
                TeleologicalStoreError::rocksdb_op(
                    "iterate_learner_retrieval_log_keys",
                    CF_LEARNER_RETRIEVAL_LOG,
                    None,
                    e,
                )
            })?;
            if key.len() != 40 {
                warn!(
                    cf = CF_LEARNER_RETRIEVAL_LOG,
                    len = key.len(),
                    "Skipping learner retrieval log with unexpected key length"
                );
                continue;
            }
            let mut learner = [0u8; 16];
            let mut trace = [0u8; 16];
            let mut ts = [0u8; 8];
            learner.copy_from_slice(&key[..16]);
            trace.copy_from_slice(&key[16..32]);
            ts.copy_from_slice(&key[32..40]);
            out.push((
                Uuid::from_bytes(learner),
                Uuid::from_bytes(trace),
                u64::from_be_bytes(ts),
            ));
        }
        Ok(out)
    }

    pub async fn store_learner_k_sleep(&self, value: &LearnerKSleep) -> CoreResult<()> {
        let payload = encode_learner_k_sleep(value)?;
        let key = learner_session_key(value.learner_id, value.session_ts);
        self.put_learner_bytes(CF_LEARNER_K_SLEEP, &key, &payload)
    }

    pub async fn get_learner_k_sleep(
        &self,
        learner_id: Uuid,
        session_ts: u64,
    ) -> CoreResult<Option<LearnerKSleep>> {
        let key = learner_session_key(learner_id, session_ts);
        self.get_learner_bytes(CF_LEARNER_K_SLEEP, &key)?
            .map(|bytes| decode_learner_k_sleep(&bytes))
            .transpose()
    }

    pub async fn store_goal_centroid(&self, centroid: &GoalCentroid) -> CoreResult<()> {
        let payload = encode_goal_centroid(centroid)?;
        let mut key = Vec::with_capacity(17);
        key.extend_from_slice(centroid.skill_id.as_bytes());
        key.push(centroid.modality as u8);
        self.put_learner_bytes(CF_GOAL_CENTROIDS, &key, &payload)
    }

    pub async fn get_goal_centroid(
        &self,
        skill_id: Uuid,
        modality: context_graph_core::learner::LearnerModality,
    ) -> CoreResult<Option<GoalCentroid>> {
        let mut key = Vec::with_capacity(17);
        key.extend_from_slice(skill_id.as_bytes());
        key.push(modality as u8);
        self.get_learner_bytes(CF_GOAL_CENTROIDS, &key)?
            .map(|bytes| decode_goal_centroid(&bytes))
            .transpose()
    }

    pub async fn store_learner_audit_entry(&self, entry: &LearnerAuditEntry) -> CoreResult<()> {
        let payload = encode_learner_audit_entry(entry)?;
        let key = learner_audit_key(entry.learner_id, entry.ts, entry.audit_id);
        self.put_learner_bytes(CF_LEARNER_AUDIT, &key, &payload)
    }

    pub async fn count_learner_profiles(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_PROFILE)
    }

    pub async fn count_learner_fingerprints(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_FINGERPRINTS_LEARNER)
    }

    pub async fn count_learner_state_history(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_STATE_HISTORY)
    }

    pub async fn count_learner_delta_logs(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_DELTA_LOG)
    }

    pub async fn count_learner_m_traces(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_M_PER_TRACE)
    }

    pub async fn count_learner_constellations(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_CONSTELLATIONS)
    }

    pub async fn count_learner_goal_states(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_GOAL_STATES)
    }

    pub async fn count_learner_retrieval_logs(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_RETRIEVAL_LOG)
    }

    pub async fn count_learner_k_sleep(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_K_SLEEP)
    }

    pub async fn count_goal_centroids(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_GOAL_CENTROIDS)
    }

    pub async fn count_learner_audit_entries(&self) -> CoreResult<usize> {
        self.count_learner_cf(CF_LEARNER_AUDIT)
    }

    pub async fn clear_all_learner_state_records(&self) -> CoreResult<usize> {
        let cfs = [
            CF_LEARNER_PROFILE,
            CF_LEARNER_CONSTELLATIONS,
            CF_FINGERPRINTS_LEARNER,
            CF_LEARNER_M_PER_TRACE,
            CF_LEARNER_STATE_HISTORY,
            CF_LEARNER_GOAL_STATES,
            CF_LEARNER_RETRIEVAL_LOG,
            CF_LEARNER_K_SLEEP,
            CF_GOAL_CENTROIDS,
            CF_LEARNER_DELTA_LOG,
            CF_LEARNER_AUDIT,
        ];
        let mut deleted = 0usize;
        for cf_name in cfs {
            deleted += self.clear_learner_cf(cf_name)?;
        }
        Ok(deleted)
    }

    fn put_learner_bytes(
        &self,
        cf_name: &'static str,
        key: &[u8],
        payload: &[u8],
    ) -> CoreResult<()> {
        let cf = self.learner_cf(cf_name);
        self.db.put_cf(cf, key, payload).map_err(|e| {
            error!(cf = cf_name, error = %e, "ROCKSDB ERROR: Failed to store learner record");
            TeleologicalStoreError::rocksdb_op("put_learner_record", cf_name, None, e)
        })?;
        Ok(())
    }

    fn get_learner_bytes(&self, cf_name: &'static str, key: &[u8]) -> CoreResult<Option<Vec<u8>>> {
        let cf = self.learner_cf(cf_name);
        self.db.get_cf(cf, key).map_err(|e| {
            error!(cf = cf_name, error = %e, "ROCKSDB ERROR: Failed to read learner record");
            TeleologicalStoreError::rocksdb_op("get_learner_record", cf_name, None, e).into()
        })
    }

    fn count_learner_cf(&self, cf_name: &'static str) -> CoreResult<usize> {
        let cf = self.learner_cf(cf_name);
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    fn list_learner_session_keys(&self, cf_name: &'static str) -> CoreResult<Vec<(Uuid, u64)>> {
        let cf = self.learner_cf(cf_name);
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, _) = item.map_err(|e| {
                error!(cf = cf_name, error = %e, "ROCKSDB ERROR: Failed to iterate learner session keys");
                TeleologicalStoreError::rocksdb_op(
                    "iterate_learner_session_keys",
                    cf_name,
                    None,
                    e,
                )
            })?;
            if key.len() != 24 {
                warn!(
                    cf = cf_name,
                    len = key.len(),
                    "Skipping learner session record with unexpected key length"
                );
                continue;
            }
            let mut learner = [0u8; 16];
            let mut ts = [0u8; 8];
            learner.copy_from_slice(&key[..16]);
            ts.copy_from_slice(&key[16..24]);
            out.push((Uuid::from_bytes(learner), u64::from_be_bytes(ts)));
        }
        Ok(out)
    }

    fn clear_learner_cf(&self, cf_name: &'static str) -> CoreResult<usize> {
        let cf = self.learner_cf(cf_name);
        let keys = self
            .db
            .iterator_cf(cf, IteratorMode::Start)
            .map(|item| item.map(|(key, _)| key.to_vec()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                error!(cf = cf_name, error = %e, "ROCKSDB ERROR: Failed to iterate learner CF");
                TeleologicalStoreError::rocksdb_op("iterate_learner_cf", cf_name, None, e)
            })?;
        for key in &keys {
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(cf = cf_name, error = %e, "ROCKSDB ERROR: Failed to clear learner CF");
                TeleologicalStoreError::rocksdb_op("clear_learner_cf", cf_name, None, e)
            })?;
        }
        Ok(keys.len())
    }
}

pub fn learner_session_key(learner_id: Uuid, session_ts: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(learner_id.as_bytes());
    key.extend_from_slice(&session_ts.to_be_bytes());
    key
}

pub fn learner_trace_key(learner_id: Uuid, trace_id: Uuid) -> Vec<u8> {
    let mut key = Vec::with_capacity(32);
    key.extend_from_slice(learner_id.as_bytes());
    key.extend_from_slice(trace_id.as_bytes());
    key
}

pub fn learner_constellation_key(learner_id: Uuid, selector_kind: u8) -> Vec<u8> {
    let mut key = Vec::with_capacity(17);
    key.extend_from_slice(learner_id.as_bytes());
    key.push(selector_kind);
    key
}

pub fn learner_skill_key(learner_id: Uuid, skill_id: Uuid) -> Vec<u8> {
    let mut key = Vec::with_capacity(32);
    key.extend_from_slice(learner_id.as_bytes());
    key.extend_from_slice(skill_id.as_bytes());
    key
}

pub fn learner_trace_ts_key(learner_id: Uuid, trace_id: Uuid, ts: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(learner_id.as_bytes());
    key.extend_from_slice(trace_id.as_bytes());
    key.extend_from_slice(&ts.to_be_bytes());
    key
}

pub fn learner_audit_key(learner_id: Uuid, ts: u64, audit_id: Uuid) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(learner_id.as_bytes());
    key.extend_from_slice(&ts.to_be_bytes());
    key.extend_from_slice(audit_id.as_bytes());
    key
}

pub fn encode_learner_profile(value: &LearnerProfile) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerProfile")
}

pub fn decode_learner_profile(bytes: &[u8]) -> CoreResult<LearnerProfile> {
    let value: LearnerProfile = decode_versioned(bytes, "LearnerProfile")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_fingerprint(value: &LearnerFingerprint) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerFingerprint")
}

pub fn decode_learner_fingerprint(bytes: &[u8]) -> CoreResult<LearnerFingerprint> {
    let value: LearnerFingerprint = decode_versioned(bytes, "LearnerFingerprint")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_state_vector(value: &LearnerStateVector) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerStateVector")
}

pub fn decode_learner_state_vector(bytes: &[u8]) -> CoreResult<LearnerStateVector> {
    let value: LearnerStateVector = decode_versioned(bytes, "LearnerStateVector")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_delta_log(value: &LearnerDeltaLog) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerDeltaLog")
}

pub fn decode_learner_delta_log(bytes: &[u8]) -> CoreResult<LearnerDeltaLog> {
    let value: LearnerDeltaLog = decode_versioned(bytes, "LearnerDeltaLog")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_m_trace(value: &LearnerMTrace) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerMTrace")
}

pub fn decode_learner_m_trace(bytes: &[u8]) -> CoreResult<LearnerMTrace> {
    let value: LearnerMTrace = decode_versioned(bytes, "LearnerMTrace")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_constellation(value: &LearnerConstellation) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerConstellation")
}

pub fn decode_learner_constellation(bytes: &[u8]) -> CoreResult<LearnerConstellation> {
    let value: LearnerConstellation = decode_versioned(bytes, "LearnerConstellation")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_goal_state(value: &LearnerGoalState) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerGoalState")
}

pub fn decode_learner_goal_state(bytes: &[u8]) -> CoreResult<LearnerGoalState> {
    let value: LearnerGoalState = decode_versioned(bytes, "LearnerGoalState")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_retrieval_log(value: &LearnerRetrievalLog) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerRetrievalLog")
}

pub fn decode_learner_retrieval_log(bytes: &[u8]) -> CoreResult<LearnerRetrievalLog> {
    let value: LearnerRetrievalLog = decode_versioned(bytes, "LearnerRetrievalLog")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_k_sleep(value: &LearnerKSleep) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerKSleep")
}

pub fn decode_learner_k_sleep(bytes: &[u8]) -> CoreResult<LearnerKSleep> {
    let value: LearnerKSleep = decode_versioned(bytes, "LearnerKSleep")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_goal_centroid(value: &GoalCentroid) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "GoalCentroid")
}

pub fn decode_goal_centroid(bytes: &[u8]) -> CoreResult<GoalCentroid> {
    let value: GoalCentroid = decode_versioned(bytes, "GoalCentroid")?;
    value.validate()?;
    Ok(value)
}

pub fn encode_learner_audit_entry(value: &LearnerAuditEntry) -> CoreResult<Vec<u8>> {
    value.validate()?;
    encode_versioned(value, "LearnerAuditEntry")
}

fn encode_versioned<T: Serialize>(value: &T, type_name: &str) -> CoreResult<Vec<u8>> {
    let mut bytes = bincode::serialize(value).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize {type_name}: {e}"))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(LEARNER_RECORD_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

fn decode_versioned<T: DeserializeOwned>(bytes: &[u8], type_name: &str) -> CoreResult<T> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(format!(
            "{type_name} payload is empty (missing version byte)"
        )));
    }
    if bytes[0] != LEARNER_RECORD_VERSION {
        warn!(
            got = bytes[0],
            expected = LEARNER_RECORD_VERSION,
            type_name,
            "Rejecting learner record with unsupported version"
        );
        return Err(CoreError::SerializationError(format!(
            "{type_name} version mismatch: got {}, expected {}. No automatic migration is supported.",
            bytes[0], LEARNER_RECORD_VERSION
        )));
    }
    bincode::deserialize(&bytes[1..])
        .map_err(|e| CoreError::SerializationError(format!("bincode deserialize {type_name}: {e}")))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use context_graph_core::learner::{
        compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_json,
        ComputationEnvelope, LearnerModality, LearnerStateComponents, LearnerStateVector,
    };

    use super::*;

    fn profile() -> LearnerProfile {
        LearnerProfile::new(
            Uuid::from_u128(1),
            "synthetic".into(),
            "consented".into(),
            BTreeSet::from([LearnerModality::AffectText, LearnerModality::SelfReport]),
            Some(1_700_000_000),
        )
        .unwrap()
    }

    fn state_vector(learner_id: Uuid, session_ts: u64) -> LearnerStateVector {
        LearnerStateVector {
            learner_id,
            session_ts,
            values: vec![0.1, 0.2, 0.3, 0.4],
            components: LearnerStateComponents {
                plasticity_window: 0.8,
                hrv_coherence: 0.7,
                valence: 0.2,
                arousal: 0.0,
                stress_floor: 0.9,
                k_sleep: 1.0,
            },
            context: BTreeMap::new(),
        }
    }

    #[test]
    fn profile_roundtrip_is_versioned() {
        let profile = profile();
        let bytes = encode_learner_profile(&profile).unwrap();
        assert_eq!(bytes[0], LEARNER_RECORD_VERSION);
        let decoded = decode_learner_profile(&bytes).unwrap();
        println!(
            "profile learner_id={} version={} modalities={}",
            decoded.learner_id,
            bytes[0],
            decoded.modalities_enabled.len()
        );
        assert_eq!(decoded, profile);
    }

    #[test]
    fn delta_log_roundtrip_has_expected_state() {
        let learner_id = Uuid::from_u128(1);
        let session_ts = 1_700_000_010;
        let delta_s = compute_delta_s_from_text(
            "temporary concept",
            "temporary concept refined",
            None,
            0.2,
            None,
        )
        .unwrap();
        let delta_c = compute_delta_c(&[0.7, 0.8, 0.9], 0.7, 0.8, 0.0, None).unwrap();
        let delta_e = compute_delta_e(&state_vector(learner_id, session_ts).components).unwrap();
        let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None).unwrap();
        let output_hash = sha256_json(&computation).unwrap();
        let provenance = ComputationEnvelope::new(
            Uuid::from_u128(2),
            learner_id,
            session_ts,
            Vec::new(),
            "thresholds-v1".into(),
            output_hash,
        )
        .unwrap();
        let log = LearnerDeltaLog {
            learner_id,
            session_ts,
            computation,
            provenance,
        };
        let bytes = encode_learner_delta_log(&log).unwrap();
        let decoded = decode_learner_delta_log(&bytes).unwrap();
        println!(
            "delta_log l={} state={}",
            decoded.computation.l,
            decoded.computation.diagnostic_state.as_str()
        );
        assert_eq!(decoded.learner_id, learner_id);
    }

    #[test]
    fn wrong_version_is_rejected() {
        let profile = profile();
        let mut bytes = encode_learner_profile(&profile).unwrap();
        bytes[0] = LEARNER_RECORD_VERSION + 1;
        let err = decode_learner_profile(&bytes).unwrap_err();
        println!("wrong version error={err}");
        assert!(format!("{err}").contains("version mismatch"));
    }
}
