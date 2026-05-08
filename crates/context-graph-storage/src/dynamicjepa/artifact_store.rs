use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, ModelArtifactId, ModelArtifactRecord, TrainingRunId,
    TrainingRunRecord,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{CF_DJ_MODEL_ARTIFACTS, CF_DJ_TRAINING_RUNS};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;

pub fn put_training_run(db: &DB, record: &TrainingRunRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_TRAINING_RUNS,
        record.training_run_id.into_bytes(),
        record,
    )
}

pub fn put_training_run_with_audit_batch(
    db: &DB,
    record: &TrainingRunRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_TRAINING_RUNS)?,
        record.training_run_id.into_bytes(),
        encode_record(record)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_training_run_with_audit_batch")
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

pub fn put_model_artifact_completion_batch(
    db: &DB,
    artifact: &ModelArtifactRecord,
    completed_run: &TrainingRunRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    artifact.validate_record()?;
    completed_run.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_MODEL_ARTIFACTS)?,
        artifact.artifact_id.into_bytes(),
        encode_record(artifact)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_TRAINING_RUNS)?,
        completed_run.training_run_id.into_bytes(),
        encode_record(completed_run)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_model_artifact_completion_batch")
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
