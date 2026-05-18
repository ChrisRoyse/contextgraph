use crate::synthetic_stress::{
    synthetic_stress_invalid, SyntheticStressError, SyntheticStressResult,
};
use context_graph_mejepa_cf::CF_MEJEPA_SYNTHETIC_STRESS_RESULTS;
use rocksdb::{IteratorMode, WriteOptions, DB};

pub fn synthetic_stress_result_key(case_id: &str) -> Vec<u8> {
    format!("python-synthetic-stress-v1:{case_id}").into_bytes()
}

pub fn persist_synthetic_stress_result(
    db: &DB,
    result: &SyntheticStressResult,
) -> Result<(), SyntheticStressError> {
    let cf = db
        .cf_handle(CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        .ok_or_else(|| {
            synthetic_stress_invalid("rocksdb.column_family", CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        })?;
    let bytes = bincode::serialize(result)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(
        cf,
        synthetic_stress_result_key(&result.case_id),
        &bytes,
        &opts,
    )?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, synthetic_stress_result_key(&result.case_id))?
        .ok_or_else(|| {
            synthetic_stress_invalid("synthetic_stress_result.readback", "missing row after put")
        })?;
    if readback != bytes {
        return Err(synthetic_stress_invalid(
            "synthetic_stress_result.readback",
            "readback bytes differ from written result",
        ));
    }
    Ok(())
}

pub fn read_synthetic_stress_result(
    db: &DB,
    case_id: &str,
) -> Result<Option<SyntheticStressResult>, SyntheticStressError> {
    let cf = db
        .cf_handle(CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        .ok_or_else(|| {
            synthetic_stress_invalid("rocksdb.column_family", CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        })?;
    db.get_cf(cf, synthetic_stress_result_key(case_id))?
        .map(|bytes| bincode::deserialize(&bytes).map_err(SyntheticStressError::from))
        .transpose()
}

pub fn count_synthetic_stress_results(db: &DB) -> Result<usize, SyntheticStressError> {
    let cf = db
        .cf_handle(CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        .ok_or_else(|| {
            synthetic_stress_invalid("rocksdb.column_family", CF_MEJEPA_SYNTHETIC_STRESS_RESULTS)
        })?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}
