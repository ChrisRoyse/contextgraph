use context_graph_mejepa_cf::{
    CF_MEJEPA_OPERATOR_PATHWAY_CHOICES, CF_MEJEPA_PATHWAY_TREES, CF_MEJEPA_SURFACED_PATHWAYS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};

use super::error::{require, validate_hex, validate_id, PathwayError, PathwayResult};
use super::{
    OperatorPathwayChoiceRecord, PathwaySurfaceReport, PathwayTreeRecord, SurfacedPathwayRecord,
};

pub fn write_pathway_tree(db: &DB, record: &PathwayTreeRecord) -> PathwayResult<()> {
    record.validate()?;
    put_readback(
        db,
        CF_MEJEPA_PATHWAY_TREES,
        record.key().as_bytes(),
        &bincode::serialize(record).map_err(PathwayError::from_err)?,
    )
}

pub fn write_surfaced_pathway(db: &DB, record: &SurfacedPathwayRecord) -> PathwayResult<()> {
    record.validate()?;
    put_readback(
        db,
        CF_MEJEPA_SURFACED_PATHWAYS,
        record.key().as_bytes(),
        &bincode::serialize(record).map_err(PathwayError::from_err)?,
    )
}

pub fn write_operator_pathway_choice(
    db: &DB,
    record: &OperatorPathwayChoiceRecord,
) -> PathwayResult<bool> {
    record.validate()?;
    let key = record.key();
    let value = bincode::serialize(record).map_err(PathwayError::from_err)?;
    let cf = cf(db, CF_MEJEPA_OPERATOR_PATHWAY_CHOICES)?;
    if let Some(existing) = db
        .get_cf(cf, key.as_bytes())
        .map_err(PathwayError::from_err)?
    {
        require(
            existing.as_slice() == value.as_slice(),
            "operator pathway choice is already recorded with different payload",
        )?;
        return Ok(false);
    }
    put_readback(
        db,
        CF_MEJEPA_OPERATOR_PATHWAY_CHOICES,
        key.as_bytes(),
        &value,
    )?;
    Ok(true)
}

pub fn read_pathway_tree(db: &DB, tree_id: &str) -> PathwayResult<Option<PathwayTreeRecord>> {
    validate_id("tree_id", tree_id)?;
    let Some(bytes) = db
        .get_cf(cf(db, CF_MEJEPA_PATHWAY_TREES)?, tree_id.as_bytes())
        .map_err(PathwayError::from_err)?
    else {
        return Ok(None);
    };
    let record: PathwayTreeRecord = bincode::deserialize(&bytes).map_err(PathwayError::from_err)?;
    record.validate()?;
    require(record.key() == tree_id, "pathway tree key mismatch")?;
    Ok(Some(record))
}

pub fn read_surfaced_pathway(
    db: &DB,
    pathway_id: &str,
) -> PathwayResult<Option<SurfacedPathwayRecord>> {
    validate_id("pathway_id", pathway_id)?;
    let Some(bytes) = db
        .get_cf(cf(db, CF_MEJEPA_SURFACED_PATHWAYS)?, pathway_id.as_bytes())
        .map_err(PathwayError::from_err)?
    else {
        return Ok(None);
    };
    let record: SurfacedPathwayRecord =
        bincode::deserialize(&bytes).map_err(PathwayError::from_err)?;
    record.validate()?;
    require(record.key() == pathway_id, "surfaced pathway key mismatch")?;
    Ok(Some(record))
}

pub fn read_surfaced_pathways_for_prediction(
    db: &DB,
    prediction_id_hex: &str,
    limit: usize,
) -> PathwayResult<Vec<SurfacedPathwayRecord>> {
    validate_hex("prediction_id_hex", prediction_id_hex)?;
    require(
        limit > 0 && limit <= 10_000,
        "limit must be within 1..=10000",
    )?;
    let cf = cf(db, CF_MEJEPA_SURFACED_PATHWAYS)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, bytes) = item.map_err(PathwayError::from_err)?;
        let record: SurfacedPathwayRecord =
            bincode::deserialize(&bytes).map_err(PathwayError::from_err)?;
        record.validate()?;
        if record.prediction_id_hex == prediction_id_hex {
            out.push(record);
        }
    }
    out.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| left.pathway_id.cmp(&right.pathway_id))
    });
    out.truncate(limit);
    Ok(out)
}

pub fn read_operator_pathway_choices(
    db: &DB,
    limit: usize,
) -> PathwayResult<Vec<OperatorPathwayChoiceRecord>> {
    require(
        limit > 0 && limit <= 10_000,
        "limit must be within 1..=10000",
    )?;
    let cf = cf(db, CF_MEJEPA_OPERATOR_PATHWAY_CHOICES)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, bytes) = item.map_err(PathwayError::from_err)?;
        let record: OperatorPathwayChoiceRecord =
            bincode::deserialize(&bytes).map_err(PathwayError::from_err)?;
        record.validate()?;
        out.push(record);
    }
    out.sort_by(|left, right| {
        right
            .chosen_at_unix_ms
            .cmp(&left.chosen_at_unix_ms)
            .then_with(|| left.choice_id.cmp(&right.choice_id))
    });
    out.truncate(limit);
    Ok(out)
}

pub fn persist_pathway_surface(db: &DB, report: &PathwaySurfaceReport) -> PathwayResult<()> {
    write_pathway_tree(db, &report.tree)?;
    for pathway in &report.surfaced_pathways {
        write_surfaced_pathway(db, pathway)?;
    }
    Ok(())
}

fn put_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> PathwayResult<()> {
    let cf = cf(db, cf_name)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)
        .map_err(PathwayError::from_err)?;
    let readback = db
        .get_cf(cf, key)
        .map_err(PathwayError::from_err)?
        .ok_or_else(|| PathwayError::new(format!("missing readback from {cf_name}")))?;
    require(readback.as_slice() == value, "CF readback changed payload")
}

fn cf<'a>(db: &'a DB, cf_name: &str) -> PathwayResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(cf_name)
        .ok_or_else(|| PathwayError::new(format!("missing column family {cf_name}")))
}
