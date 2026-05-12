use context_graph_mejepa::open_infer_rocksdb;
use context_graph_mejepa::types::{ChunkId, PanelId};
use context_graph_mejepa_train::dda::{
    compute_dda_signals, count_persisted_dda_signals, persist_dda_signals, read_dda_signals,
    DdaPairwiseBaseline, DdaVectorInput, DDA_PAIRWISE_PROJECTION_DIM,
    DDA_PAIRWISE_PROJECTION_SCHEMA,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-d-fsv";
const CHUNK_COUNT: usize = 30;
const EMBEDDER_COUNT: usize = 12;
const VECTOR_DIM: usize = 16;

#[derive(Debug, Serialize)]
struct StateSnapshot {
    label: String,
    dda_row_count: usize,
    db_path: String,
}

#[derive(Debug, Serialize)]
struct EdgeCaseReport {
    case: String,
    before: StateSnapshot,
    error_code: String,
    error_message: String,
    after: StateSnapshot,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    source_of_truth: serde_json::Value,
    projection_schema: String,
    projection_dim: usize,
    chunk_count: usize,
    embedder_count: usize,
    expected_rows: usize,
    before: StateSnapshot,
    after: StateSnapshot,
    first_row: serde_json::Value,
    last_row: serde_json::Value,
    edge_cases: Vec<EdgeCaseReport>,
    evidence_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(FSV_ROOT)?;
    let run_id = unix_ms()?;
    let db_path = PathBuf::from(FSV_ROOT).join(format!("dda-rocks-{run_id}"));
    let evidence_path = PathBuf::from(FSV_ROOT).join("dda-signal-compute.json");

    let db = open_infer_rocksdb(&db_path)?;
    let before = snapshot("before_happy_path", &db_path, db.as_ref())?;
    let panel_id = panel_id("phase-d-dda-fsv-panel");

    for chunk_idx in 0..CHUNK_COUNT {
        let chunk_id = ChunkId(format!("phase-d-dda-chunk-{chunk_idx:02}"));
        let inputs = known_inputs(chunk_idx);
        let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
        let signals = compute_dda_signals(&inputs, &baseline)?;
        if signals
            .per_embedder_cosine
            .iter()
            .any(|value| (*value - 1.0).abs() > 1e-6)
        {
            return Err(format!("per-embedder cosine was not 1.0 for {chunk_idx}").into());
        }
        persist_dda_signals(db.as_ref(), &panel_id, &chunk_id, &signals)?;
    }

    let after = snapshot("after_happy_path_same_handle", &db_path, db.as_ref())?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_after = snapshot("after_happy_path_reopened", &db_path, reopened.as_ref())?;
    if reopened_after.dda_row_count != CHUNK_COUNT {
        return Err(format!(
            "DDA RocksDB row count mismatch: got {}, expected {}",
            reopened_after.dda_row_count, CHUNK_COUNT
        )
        .into());
    }

    let first_chunk = ChunkId("phase-d-dda-chunk-00".to_string());
    let last_chunk = ChunkId(format!("phase-d-dda-chunk-{:02}", CHUNK_COUNT - 1));
    let first = read_dda_signals(reopened.as_ref(), &panel_id, &first_chunk)?
        .ok_or("first DDA row missing after reopen")?;
    let last = read_dda_signals(reopened.as_ref(), &panel_id, &last_chunk)?
        .ok_or("last DDA row missing after reopen")?;
    assert_signal_shape(&first)?;
    assert_signal_shape(&last)?;

    let edge_cases = vec![
        edge_case("empty_inputs_rejected", &db_path, reopened.as_ref(), || {
            let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(0);
            compute_dda_signals(&[], &baseline).map(|_| ())
        })?,
        edge_case(
            "unsorted_embedder_ids_rejected",
            &db_path,
            reopened.as_ref(),
            || {
                let inputs = vec![
                    DdaVectorInput {
                        embedder_id: "E02".to_string(),
                        vector: vec![1.0],
                        centroid: vec![1.0],
                    },
                    DdaVectorInput {
                        embedder_id: "E01".to_string(),
                        vector: vec![1.0],
                        centroid: vec![1.0],
                    },
                ];
                let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
                compute_dda_signals(&inputs, &baseline).map(|_| ())
            },
        )?,
        edge_case("zero_vector_rejected", &db_path, reopened.as_ref(), || {
            let inputs = vec![DdaVectorInput {
                embedder_id: "E01".to_string(),
                vector: vec![0.0, 0.0],
                centroid: vec![1.0, 0.0],
            }];
            let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
            compute_dda_signals(&inputs, &baseline).map(|_| ())
        })?,
    ];

    let report = FsvReport {
        source_of_truth: json!({
            "type": "rocksdb_column_family",
            "db_path": db_path,
            "column_family": context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
            "verification": "write via persist_dda_signals, drop handle, reopen with open_infer_rocksdb, read rows through read_dda_signals"
        }),
        projection_schema: DDA_PAIRWISE_PROJECTION_SCHEMA.to_string(),
        projection_dim: DDA_PAIRWISE_PROJECTION_DIM,
        chunk_count: CHUNK_COUNT,
        embedder_count: EMBEDDER_COUNT,
        expected_rows: CHUNK_COUNT,
        before,
        after,
        first_row: serde_json::to_value(&first)?,
        last_row: serde_json::to_value(&last)?,
        edge_cases,
        evidence_path: evidence_path.display().to_string(),
    };
    fs::write(&evidence_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn known_inputs(chunk_idx: usize) -> Vec<DdaVectorInput> {
    (0..EMBEDDER_COUNT)
        .map(|embedder_idx| {
            let mut vector = Vec::with_capacity(VECTOR_DIM);
            for dim in 0..VECTOR_DIM {
                let value = ((chunk_idx + 1) as f32 * 0.031)
                    + ((embedder_idx + 1) as f32 * 0.071)
                    + ((dim + 1) as f32 * 0.013);
                vector.push(value);
            }
            DdaVectorInput {
                embedder_id: format!("E{:02}", embedder_idx + 1),
                centroid: vector.clone(),
                vector,
            }
        })
        .collect()
}

fn assert_signal_shape(
    signals: &context_graph_mejepa::types::DdaSignals,
) -> Result<(), Box<dyn Error>> {
    let pairwise = EMBEDDER_COUNT * (EMBEDDER_COUNT - 1) / 2;
    if signals.per_embedder_cosine.len() != EMBEDDER_COUNT {
        return Err("per_embedder_cosine length mismatch".into());
    }
    if signals.pairwise_cosine_upper.len() != pairwise
        || signals.pairwise_mi_upper.len() != pairwise
        || signals.blind_spot_z_scores.len() != pairwise
    {
        return Err("pairwise signal length mismatch".into());
    }
    Ok(())
}

fn edge_case<F>(
    case: &str,
    db_path: &Path,
    db: &rocksdb::DB,
    trigger: F,
) -> Result<EdgeCaseReport, Box<dyn Error>>
where
    F: FnOnce() -> Result<(), context_graph_mejepa_train::TrainerError>,
{
    let before = snapshot(&format!("{case}_before"), db_path, db)?;
    let err = trigger().expect_err("edge case unexpectedly succeeded");
    let after = snapshot(&format!("{case}_after"), db_path, db)?;
    if before.dda_row_count != after.dda_row_count {
        return Err(format!(
            "{case} mutated DDA RocksDB row count: before={} after={}",
            before.dda_row_count, after.dda_row_count
        )
        .into());
    }
    Ok(EdgeCaseReport {
        case: case.to_string(),
        before,
        error_code: err.code().to_string(),
        error_message: err.to_string(),
        after,
    })
}

fn snapshot(
    label: &str,
    db_path: &Path,
    db: &rocksdb::DB,
) -> Result<StateSnapshot, Box<dyn Error>> {
    Ok(StateSnapshot {
        label: label.to_string(),
        dda_row_count: count_persisted_dda_signals(db)?,
        db_path: db_path.display().to_string(),
    })
}

fn panel_id(label: &str) -> PanelId {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    PanelId(hasher.finalize().into())
}

fn unix_ms() -> Result<u128, Box<dyn Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}
