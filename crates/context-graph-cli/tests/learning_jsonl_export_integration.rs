//! Full-state verification for Learning-as-UTL JSONL exports.
//!
//! These tests use real RocksDB stores under an issue-scoped local FSV root and
//! then inspect both the JSONL file and the source column family after export.
//! No mocks.

use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use context_graph_cli::commands::learning::{
    handle_learning_command, LearningCommands, LearningExportDatasetJsonlArgs,
    LearningExportEventsJsonlArgs,
};
use context_graph_core::learner_training::{
    learning_event_feature_schema, learning_event_feature_vector, LearnerTrainingDataset,
    LearnerTrainingRow, LearnerTrainingTask,
};
use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
};
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde_json::Value;
use serial_test::serial;
use std::collections::BTreeMap;
use tempfile::TempDir;
use uuid::Uuid;

const FSV_ROOT: &str = "/tmp/contextgraph/fsv/issue-259-learning-jsonl-export-fsv";

struct DataRootGuard {
    prior: Option<String>,
}

impl Drop for DataRootGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(value) => std::env::set_var(context_graph_paths::ENV_DATA_ROOT, value),
            None => std::env::remove_var(context_graph_paths::ENV_DATA_ROOT),
        }
    }
}

fn configure_local_data_root() -> DataRootGuard {
    fs::create_dir_all(FSV_ROOT).expect("create local FSV data root");
    let prior = std::env::var(context_graph_paths::ENV_DATA_ROOT).ok();
    std::env::set_var(context_graph_paths::ENV_DATA_ROOT, FSV_ROOT);
    DataRootGuard { prior }
}

fn state(value: f32, rank: u32, domain: &str) -> LearningStateSnapshot {
    LearningStateSnapshot {
        topic_profile: [value; NUM_EMBEDDERS],
        cross_correlations: vec![value * 0.25; NUM_CROSS_CORRELATIONS],
        retrieval_rank: Some(rank),
        embedder_scores: [value; NUM_EMBEDDERS],
        contradiction_pressure: 0.2,
        integration_confidence: 0.5,
        recurrence_count: 3,
        stability_score: 0.6,
        domain: Some(domain.into()),
        successful_transfer_count: 1,
    }
}

fn event(
    event_id: Uuid,
    after_value: f32,
    label: LearningOutcomeLabel,
    utility: f32,
) -> LearningEvent {
    LearningEvent::new(
        event_id,
        vec![Uuid::from_u128(0x11111111_2222_4333_8444_555555555555)],
        Some("jsonl-export-fsv".into()),
        Some(format!("response-{event_id}")),
        Some("shared-task".into()),
        "Which response keeps the learner in the productive window?".into(),
        "Controlled context shared by two real persisted event rows.".into(),
        format!("response body utility={utility}"),
        state(0.2, 9, "docs"),
        state(after_value, 2, "code"),
        LearningOutcome {
            label,
            utility_delta: utility,
            correction_required: utility < 0.0,
            reuse_observed: utility > 0.0,
        },
    )
    .unwrap()
}

async fn open_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default())
        .expect("open real RocksDB store")
}

fn read_jsonl(path: &std::path::Path) -> Vec<Value> {
    let text = fs::read_to_string(path).expect("read jsonl");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse jsonl row"))
        .collect()
}

fn durable_case_root(case_name: &str) -> PathBuf {
    let root = PathBuf::from(FSV_ROOT).join(format!("{case_name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create D-root FSV case directory");
    root
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn export_events_jsonl_happy_path_and_edges() {
    let _data_root = configure_local_data_root();
    let root = durable_case_root("events");
    let storage = root.join("store");
    let out = root.join("events.jsonl");

    println!("SOURCE OF TRUTH: RocksDB CF_LEARNING_EVENTS");
    {
        let store = open_store(&storage).await;
        println!(
            "HAPPY BEFORE count={}",
            store.count_learning_events().await.unwrap()
        );
        store
            .store_learning_event(&event(
                Uuid::from_u128(0xaaaaaaaa_bbbb_4ccc_8ddd_eeeeeeeeeeee),
                0.4,
                LearningOutcomeLabel::Useful,
                0.7,
            ))
            .await
            .unwrap();
        println!(
            "HAPPY AFTER seed count={}",
            store.count_learning_events().await.unwrap()
        );
    }

    let code = handle_learning_command(LearningCommands::ExportEventsJsonl(
        LearningExportEventsJsonlArgs {
            storage: storage.clone(),
            out: out.clone(),
            limit: None,
            offset: 0,
            include_text: true,
            overwrite: false,
        },
    ))
    .await;
    assert_eq!(code, 0);

    let rows = read_jsonl(&out);
    let row = &rows[0];
    println!(
        "HAPPY AFTER export rows={} event_id={} tensor_len={} first_feature={}",
        rows.len(),
        row["event_id"],
        row["feature_tensor_len"],
        row["feature_tensor"][0]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(row["record_kind"], "learning_event_tensor");
    assert_eq!(row["source_of_truth"]["column_family"], "learning_events");
    assert_eq!(
        row["feature_tensor_len"].as_u64().unwrap() as usize,
        learning_event_feature_schema().len()
    );
    let first_feature = row["feature_tensor"].as_array().unwrap()[0]
        .as_f64()
        .unwrap();
    assert!((first_feature - 0.2).abs() < 1e-6);
    assert_eq!(row["outcome"]["label"], "useful");

    {
        let store = open_store(&storage).await;
        let count = store.count_learning_events().await.unwrap();
        let readback = store
            .get_learning_event(Uuid::from_u128(0xaaaaaaaa_bbbb_4ccc_8ddd_eeeeeeeeeeee))
            .await
            .unwrap()
            .expect("event still present after export");
        println!(
            "HAPPY READBACK count={} delta_e_scalar={} query={}",
            count, readback.features.delta_e_scalar, readback.query
        );
        assert_eq!(count, 1);
    }

    let empty_root = root.join("empty");
    fs::create_dir_all(&empty_root).unwrap();
    let empty_storage = empty_root.join("store");
    let empty_out = empty_root.join("empty.jsonl");
    {
        let store = open_store(&empty_storage).await;
        println!(
            "EDGE EMPTY BEFORE count={}",
            store.count_learning_events().await.unwrap()
        );
    }
    let code = handle_learning_command(LearningCommands::ExportEventsJsonl(
        LearningExportEventsJsonlArgs {
            storage: empty_storage.clone(),
            out: empty_out.clone(),
            limit: None,
            offset: 0,
            include_text: true,
            overwrite: false,
        },
    ))
    .await;
    assert_eq!(code, 0);
    println!(
        "EDGE EMPTY AFTER rows={} bytes={}",
        read_jsonl(&empty_out).len(),
        fs::metadata(&empty_out).unwrap().len()
    );
    assert_eq!(read_jsonl(&empty_out).len(), 0);

    {
        let store = open_store(&storage).await;
        println!(
            "EDGE LIMIT BEFORE count={}",
            store.count_learning_events().await.unwrap()
        );
        store
            .store_learning_event(&event(
                Uuid::from_u128(0xbbbbbbbb_bbbb_4bbb_8bbb_bbbbbbbbbbbb),
                0.1,
                LearningOutcomeLabel::Harmful,
                -0.3,
            ))
            .await
            .unwrap();
    }
    let limit_out = root.join("limit.jsonl");
    let code = handle_learning_command(LearningCommands::ExportEventsJsonl(
        LearningExportEventsJsonlArgs {
            storage: storage.clone(),
            out: limit_out.clone(),
            limit: Some(1),
            offset: 0,
            include_text: true,
            overwrite: false,
        },
    ))
    .await;
    assert_eq!(code, 0);
    println!("EDGE LIMIT AFTER rows={}", read_jsonl(&limit_out).len());
    assert_eq!(read_jsonl(&limit_out).len(), 1);

    let bad_out = root.join("missing-parent").join("bad.jsonl");
    println!("EDGE INVALID BEFORE output_exists={}", bad_out.exists());
    let code = handle_learning_command(LearningCommands::ExportEventsJsonl(
        LearningExportEventsJsonlArgs {
            storage: storage.clone(),
            out: bad_out.clone(),
            limit: None,
            offset: 0,
            include_text: true,
            overwrite: false,
        },
    ))
    .await;
    println!(
        "EDGE INVALID AFTER exit_code={} output_exists={}",
        code,
        bad_out.exists()
    );
    assert_eq!(code, 1);
    assert!(!bad_out.exists());

    let outside_temp = TempDir::new().unwrap();
    let outside_storage = outside_temp.path().join("store");
    let outside_out = outside_temp.path().join("events.jsonl");
    println!(
        "EDGE OUTSIDE ROOT BEFORE storage={} out={} output_exists={}",
        outside_storage.display(),
        outside_out.display(),
        outside_out.exists()
    );
    let code = handle_learning_command(LearningCommands::ExportEventsJsonl(
        LearningExportEventsJsonlArgs {
            storage: outside_storage.clone(),
            out: outside_out.clone(),
            limit: None,
            offset: 0,
            include_text: true,
            overwrite: false,
        },
    ))
    .await;
    println!(
        "EDGE OUTSIDE ROOT AFTER exit_code={} output_exists={}",
        code,
        outside_out.exists()
    );
    assert_eq!(code, 1);
    assert!(!outside_out.exists());
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn export_dataset_jsonl_reads_persisted_matrix_rows() {
    let _data_root = configure_local_data_root();
    let root = durable_case_root("dataset");
    let storage = root.join("store");
    let out = root.join("dataset.jsonl");
    let dataset_id = Uuid::from_u128(0xcccccccc_cccc_4ccc_8ccc_cccccccccccc);
    let event_id = Uuid::from_u128(0xdddddddd_dddd_4ddd_8ddd_dddddddddddd);

    println!("SOURCE OF TRUTH: RocksDB CF_LEARNER_TRAINING_DATASETS");
    {
        let store = open_store(&storage).await;
        let learning_event = event(event_id, 0.5, LearningOutcomeLabel::Useful, 0.8);
        let tensor = learning_event_feature_vector(&learning_event).unwrap();
        let row = LearnerTrainingRow {
            row_id: event_id,
            source_cf: "learning_events".into(),
            source_key: event_id.to_string(),
            event_id: Some(event_id),
            learner_id: None,
            session_ts: Some(Utc::now().timestamp() as u64),
            label_scalar: Some(0.8),
            label_class: Some("useful".into()),
            split_key: "jsonl-export-fsv".into(),
            provenance_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
        };
        let dataset = LearnerTrainingDataset::new(
            dataset_id,
            LearnerTrainingTask::RewardModel,
            learning_event_feature_schema(),
            vec!["utility_delta".into(), "outcome_label".into()],
            vec![row],
            tensor,
            BTreeMap::from([("learning_events".into(), 1)]),
            BTreeMap::from([("case".into(), "dataset-jsonl".into())]),
        )
        .unwrap();
        println!(
            "HAPPY BEFORE datasets={}",
            store.count_learner_training_datasets().await.unwrap()
        );
        store
            .store_learner_training_dataset(&dataset)
            .await
            .unwrap();
        println!(
            "HAPPY AFTER seed datasets={}",
            store.count_learner_training_datasets().await.unwrap()
        );
    }

    let code = handle_learning_command(LearningCommands::ExportDatasetJsonl(
        LearningExportDatasetJsonlArgs {
            storage: storage.clone(),
            out: out.clone(),
            dataset_id: Some(dataset_id),
            limit: None,
            offset: 0,
            overwrite: false,
        },
    ))
    .await;
    assert_eq!(code, 0);
    let rows = read_jsonl(&out);
    println!(
        "HAPPY AFTER export rows={} dataset_id={} feature_len={} label={}",
        rows.len(),
        rows[0]["dataset_id"],
        rows[0]["feature_tensor_len"],
        rows[0]["labels"]["label_scalar"]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["record_kind"], "learner_training_row");
    assert_eq!(rows[0]["dataset_id"], dataset_id.to_string());
    assert_eq!(
        rows[0]["features"].as_array().unwrap().len(),
        learning_event_feature_schema().len()
    );

    {
        let store = open_store(&storage).await;
        let readback = store
            .get_learner_training_dataset(dataset_id)
            .await
            .unwrap()
            .expect("dataset still physically present");
        println!(
            "HAPPY READBACK datasets={} rows={} cols={} sha={}",
            store.count_learner_training_datasets().await.unwrap(),
            readback.rows_len,
            readback.cols_len,
            readback.row_major_sha256
        );
        assert_eq!(readback.rows_len, 1);
        assert_eq!(readback.rows[0].event_id, Some(event_id));
    }
}
