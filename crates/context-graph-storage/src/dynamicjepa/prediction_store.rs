use context_graph_core::dynamicjepa::{DynamicJepaResult, PredictionId, PredictionRecord};
use rocksdb::DB;

use crate::dynamicjepa::column_families::CF_DJ_PREDICTIONS;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

pub fn put_prediction(db: &DB, record: &PredictionRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_PREDICTIONS,
        record.prediction_id.into_bytes(),
        record,
    )
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
