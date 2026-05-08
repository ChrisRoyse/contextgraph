use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, PredictionId, PredictionRecord,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::CF_DJ_PREDICTIONS;
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;

pub fn put_prediction(db: &DB, record: &PredictionRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_PREDICTIONS,
        record.prediction_id.into_bytes(),
        record,
    )
}

pub fn put_prediction_with_audit_batch(
    db: &DB,
    record: &PredictionRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_PREDICTIONS)?,
        record.prediction_id.into_bytes(),
        encode_record(record)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_prediction_with_audit_batch")
}

pub fn get_prediction(db: &DB, id: PredictionId) -> DynamicJepaResult<Option<PredictionRecord>> {
    get_record(db, CF_DJ_PREDICTIONS, id.into_bytes())
}

pub fn list_predictions(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<PredictionRecord>> {
    list_records(db, CF_DJ_PREDICTIONS, limit, offset)
}

pub fn count_predictions(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_PREDICTIONS)
}
