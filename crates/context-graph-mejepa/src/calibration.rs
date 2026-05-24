use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use sha2::Digest;

use crate::calibration_types::CalibrationRecord;
use crate::compiler::MejepaStore;
use crate::conformal::{self, CalibrationExample};
use crate::error::MejepaInferError;
use crate::ood;
use crate::types::EmbedderId;

pub use context_graph_mejepa_cf::{
    CF_MEJEPA_ADVERSARIAL_CORPUS, CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_HIERARCHICAL_PREDICTIONS, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS,
    CF_MEJEPA_TRAIN_CERTS,
};
pub const MEJEPA_INFER_CFS: &[&str] = context_graph_mejepa_cf::INFER_CFS;

#[derive(Clone)]
pub struct CalibrationStore {
    db: Arc<DB>,
    max_age_days: u32,
}

impl CalibrationStore {
    pub fn new(db: Arc<DB>, max_age_days: u32) -> Result<Self, MejepaInferError> {
        if max_age_days == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "max_age_days".to_string(),
                detail: "max_age_days must be >= 1".to_string(),
            });
        }
        for cf in MEJEPA_INFER_CFS {
            if db.cf_handle(cf).is_none() {
                return Err(MejepaInferError::InvalidInput {
                    field: "rocksdb.column_family".to_string(),
                    detail: format!("missing inference column family {cf}"),
                });
            }
        }
        Ok(Self { db, max_age_days })
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn calibrate(
        &self,
        examples: &[CalibrationExample],
        norm_sq_per_example: &[f32],
        alpha: f32,
        min_samples_per_stratum: usize,
        target_mean_ood: f32,
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap<EmbedderId, String>,
    ) -> Result<CalibrationRecord, MejepaInferError> {
        if examples.len() != norm_sq_per_example.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: examples.len(),
                actual: norm_sq_per_example.len(),
                context: "calibrate examples/norm_sq length mismatch".to_string(),
            });
        }
        let sigma_squared = ood::tune_sigma_squared(norm_sq_per_example, target_mean_ood)?;
        let record = conformal::calibrate(
            examples,
            alpha,
            min_samples_per_stratum,
            sigma_squared,
            corpus_sha,
            embedder_versions,
        )?;
        self.persist(&record)?;
        Ok(record)
    }

    pub fn persist(&self, record: &CalibrationRecord) -> Result<(), MejepaInferError> {
        record.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_CALIBRATION_HISTORY)?;
        let key = calibration_key(record);
        let value = bincode::serialize(record)?;
        self.db.put_cf(cf, &key, &value)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "calibration_history".to_string(),
                detail: "read-after-write could not find persisted calibration row".to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "calibration_history".to_string(),
                detail: "read-after-write bytes differ from persisted calibration payload"
                    .to_string(),
            });
        }
        let decoded = decode_calibration_record(&readback)?;
        decoded.validate()?;
        if decoded != *record {
            return Err(MejepaInferError::InvalidInput {
                field: "calibration_history".to_string(),
                detail: "read-after-write decoded calibration does not match input".to_string(),
            });
        }
        Ok(())
    }

    pub fn load_active(&self) -> Result<Arc<CalibrationRecord>, MejepaInferError> {
        let mut records = self.list_history(1)?;
        let record = records
            .pop()
            .ok_or_else(|| MejepaInferError::CalibrationStale {
                version: "none".to_string(),
                age_days: u32::MAX,
            })?;
        let now = chrono::Utc::now().timestamp();
        let age_days = ((now - record.frozen_at).max(0) / 86_400) as u32;
        if age_days > self.max_age_days {
            return Err(MejepaInferError::CalibrationStale {
                version: record.version,
                age_days,
            });
        }
        Ok(Arc::new(record))
    }

    pub fn recompute_sliding_window(
        &self,
        recent_attempts: usize,
        store: &dyn MejepaStore,
        alpha: f32,
        target_mean_ood: f32,
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap<EmbedderId, String>,
    ) -> Result<CalibrationRecord, MejepaInferError> {
        let examples = store.read_recent_calibration_examples(recent_attempts)?;
        if examples.is_empty() {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                language: None,
                expected: 1,
                actual: 0,
            });
        }
        let norm_sq = examples
            .iter()
            .map(|example| {
                let score = conformal::non_conformity_score(
                    &example.predicted_test_pass,
                    &example.actual_test_pass,
                )?;
                Ok(score * score)
            })
            .collect::<Result<Vec<_>, MejepaInferError>>()?;
        self.calibrate(
            &examples,
            &norm_sq,
            alpha,
            1,
            target_mean_ood,
            corpus_sha,
            embedder_versions,
        )
    }

    pub fn list_history(&self, limit: usize) -> Result<Vec<CalibrationRecord>, MejepaInferError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf = cf(&self.db, CF_MEJEPA_CALIBRATION_HISTORY)?;
        let iter = self.db.iterator_cf(cf, IteratorMode::End);
        let mut out = Vec::with_capacity(limit);
        for item in iter {
            let (_key, value) = item?;
            let record = decode_calibration_record(&value)?;
            record.validate()?;
            out.push(record);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn count_history(&self) -> Result<u64, MejepaInferError> {
        count_cf(&self.db, CF_MEJEPA_CALIBRATION_HISTORY)
    }
}

pub fn open_infer_rocksdb(path: impl AsRef<Path>) -> Result<Arc<DB>, MejepaInferError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = MEJEPA_INFER_CFS
        .iter()
        .chain(context_graph_mejepa_cf::TCT_CFS.iter())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(
        &opts,
        path.as_ref(),
        descriptors,
    )?))
}

pub fn count_cf(db: &DB, cf_name: &str) -> Result<u64, MejepaInferError> {
    let cf = cf(db, cf_name)?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

pub(crate) fn cf<'a>(
    db: &'a DB,
    name: &str,
) -> Result<&'a rocksdb::ColumnFamily, MejepaInferError> {
    db.cf_handle(name)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "rocksdb.column_family".to_string(),
            detail: format!("missing column family {name}"),
        })
}

fn calibration_key(record: &CalibrationRecord) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&record.frozen_at.to_be_bytes());
    let digest = sha2::Sha256::digest(record.version.as_bytes());
    key.extend_from_slice(&digest[..16]);
    key
}

fn decode_calibration_record(bytes: &[u8]) -> Result<CalibrationRecord, MejepaInferError> {
    match bincode::deserialize::<CalibrationRecord>(bytes) {
        Ok(record) => Ok(record),
        Err(current_err) => match bincode::deserialize::<LegacyCalibrationRecord>(bytes) {
            Ok(legacy) => Ok(legacy.into_current()),
            Err(_) => Err(current_err.into()),
        },
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCalibrationRecord {
    version: String,
    alpha: f32,
    tau: f32,
    sigma_squared: f32,
    empirical_coverage: f32,
    min_samples_per_stratum: usize,
    sample_count: usize,
    per_language_counts: BTreeMap<crate::types::Language, usize>,
    corpus_sha: [u8; 32],
    embedder_versions: BTreeMap<EmbedderId, String>,
    frozen_at: i64,
}

impl LegacyCalibrationRecord {
    fn into_current(self) -> CalibrationRecord {
        CalibrationRecord {
            version: self.version,
            alpha: self.alpha,
            target_coverage: 1.0 - self.alpha,
            tau: self.tau,
            sigma_squared: self.sigma_squared,
            empirical_coverage: self.empirical_coverage,
            min_samples_per_stratum: self.min_samples_per_stratum,
            sample_count: self.sample_count,
            per_language_counts: self.per_language_counts,
            per_slot_sigma_squared: None,
            corpus_sha: self.corpus_sha,
            embedder_versions: self.embedder_versions,
            frozen_at: self.frozen_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Language;
    use context_graph_mejepa_instruments::InstrumentSlot;

    fn example(i: usize) -> CalibrationExample {
        CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![if i.is_multiple_of(10) { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if i.is_multiple_of(10) { 0.0 } else { 1.0 }],
        }
    }

    #[test]
    fn persist_load_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = CalibrationStore::new(db, 30).unwrap();
        let examples = (0..40).map(example).collect::<Vec<_>>();
        let norms = vec![0.01; examples.len()];
        let record = store
            .calibrate(&examples, &norms, 0.10, 30, 0.30, [7; 32], BTreeMap::new())
            .unwrap();
        let loaded = store.load_active().unwrap();
        assert_eq!(loaded.version, record.version);
        assert_eq!(
            loaded.per_slot_sigma_squared.as_ref().unwrap().len(),
            InstrumentSlot::all().len()
        );
        assert_eq!(store.count_history().unwrap(), 1);
    }
}
