use context_graph_core::dynamicjepa::{
    DynamicJepaResult, ModelArtifactId, ModelArtifactRecord, TrainingRunId, TrainingRunRecord,
};
use rocksdb::DB;

use crate::dynamicjepa::column_families::{CF_DJ_MODEL_ARTIFACTS, CF_DJ_TRAINING_RUNS};
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

pub fn put_training_run(db: &DB, record: &TrainingRunRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_TRAINING_RUNS,
        record.training_run_id.into_bytes(),
        record,
    )
}

pub fn get_training_run(
    db: &DB,
    id: TrainingRunId,
) -> DynamicJepaResult<Option<TrainingRunRecord>> {
    get_record(db, CF_DJ_TRAINING_RUNS, id.into_bytes())
}

pub fn list_training_runs(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<TrainingRunRecord>> {
    list_records(db, CF_DJ_TRAINING_RUNS, limit, offset)
}

pub fn count_training_runs(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_TRAINING_RUNS)
}

pub fn put_model_artifact(db: &DB, record: &ModelArtifactRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_MODEL_ARTIFACTS,
        record.artifact_id.into_bytes(),
        record,
    )
}

pub fn get_model_artifact(
    db: &DB,
    id: ModelArtifactId,
) -> DynamicJepaResult<Option<ModelArtifactRecord>> {
    get_record(db, CF_DJ_MODEL_ARTIFACTS, id.into_bytes())
}

pub fn list_model_artifacts(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<ModelArtifactRecord>> {
    list_records(db, CF_DJ_MODEL_ARTIFACTS, limit, offset)
}

pub fn count_model_artifacts(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_MODEL_ARTIFACTS)
}
