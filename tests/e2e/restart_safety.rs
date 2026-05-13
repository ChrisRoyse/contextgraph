use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa::{
    open_infer_rocksdb, CalibrationExample, CalibrationStore, EmbedderId, MejepaStore,
    RocksDbInferStore, TrainCertSummary,
};
use context_graph_mejepa_instruments::{PanelStore, CF_MEJEPA_PANELS, CF_MEJEPA_PANEL_META};
use context_graph_mejepa_shift_subscriber::{
    decode_session_hex32, MeJepaShiftSubscriberConfig, ShiftLogTail, ShiftOutcome, ShiftSubscriber,
    SkipReason, WatermarkRecord, WatermarkWriter,
};
use context_graph_mejepa_train::dda::count_persisted_dda_signals;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const TASK_ID: &str = "TASK-TEST-002";
const SESSION_HEX: &str = "24242424242424242424242424242424";
const FIRST_SHIFT_INDEX: u64 = 1;
const LAST_SHIFT_INDEX: u64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestartSafetyFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: RestartSafetySourceOfTruth,
    pub happy_path: Vec<StepEvidence>,
    pub boundary_cases: Vec<BoundaryEvidence>,
    pub cf_counts_before: BTreeMap<String, u64>,
    pub cf_counts_after: BTreeMap<String, u64>,
    pub physical_artifacts: Vec<FileArtifact>,
    pub readback_equal: bool,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestartSafetySourceOfTruth {
    pub infer_db_path: String,
    pub panel_db_path: String,
    pub shift_log_path: String,
    pub watermark_cf: String,
    pub live_prediction_cf: String,
    pub dda_signal_cf: String,
    pub final_watermark_offset: u64,
    pub report_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepEvidence {
    pub step: String,
    pub trigger: String,
    pub source_of_truth: String,
    pub before: Value,
    pub after: Value,
    pub expected: Value,
    pub actual: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoundaryEvidence {
    pub case: String,
    pub trigger: String,
    pub expected_error_code: String,
    pub actual_error_code: String,
    pub before: Value,
    pub after: Value,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileArtifact {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

struct Harness {
    fsv_root: PathBuf,
    infer_db_path: PathBuf,
    panel_db_path: PathBuf,
    repo_root: PathBuf,
    shift_log_dir: PathBuf,
    shift_log_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ShiftLine {
    shift_id: String,
    line: String,
    byte_offset: u64,
    next_byte_offset: u64,
}

#[derive(Debug, Clone)]
struct RestartState {
    watermark: WatermarkRecord,
    live_predictions: u64,
    oracle_verdicts: u64,
    train_certs: u64,
    dda_signals: u64,
    panel_rows: u64,
    panel_meta_rows: u64,
}

pub fn default_fsv_root() -> PathBuf {
    PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-restart-safety-fsv")
}

pub fn default_report_path(fsv_root: &Path) -> PathBuf {
    fsv_root.join("restart_safety_fsv.json")
}

pub async fn run_restart_safety_fsv(fsv_root: &Path) -> Result<RestartSafetyFsvReport> {
    remove_existing_fsv_root(fsv_root)?;
    fs::create_dir_all(fsv_root)?;
    let harness = Harness::new(fsv_root);
    fs::create_dir_all(&harness.repo_root)?;
    fs::create_dir_all(&harness.shift_log_dir)?;
    seed_infer_db(&harness.infer_db_path)?;

    let report_path = default_report_path(fsv_root);
    let build_release_sha = git_head_short();
    let cf_counts_before = read_infer_counts(&harness.infer_db_path)?;
    let shift_lines = build_shift_lines()?;
    write_shift_log(&harness.shift_log_path, &shift_lines[..1])?;

    let mut happy_path = Vec::new();
    let before_restart_state = {
        let subscriber = harness.open_subscriber()?;
        let summary = subscriber.run_until_idle().await?;
        drop(subscriber);
        let state =
            read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
        let first_expected_offset = shift_lines[0].next_byte_offset;
        let pass = summary.processed == 1
            && summary.observed == 1
            && state.watermark.last_consumed_shift_id == shift_lines[0].shift_id
            && state.watermark.last_consumed_byte_offset == first_expected_offset
            && state.live_predictions == 1
            && state.dda_signals == 1
            && state.panel_rows == 3
            && state.panel_meta_rows == 3;
        happy_path.push(StepEvidence {
            step: "pre_restart_first_shift".to_string(),
            trigger: "run subscriber until idle on first JSONL shift".to_string(),
            source_of_truth: format!(
                "{} + {} + {}",
                context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
                context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                CF_MEJEPA_PANELS
            ),
            before: json!({"cfCounts": cf_counts_before}),
            after: json!({"summary": summary, "state": state_as_json(&state)}),
            expected: json!({
                "processed": 1,
                "observed": 1,
                "watermarkShiftId": shift_lines[0].shift_id,
                "watermarkOffset": first_expected_offset,
                "livePredictions": 1,
                "ddaSignals": 1,
                "panelRows": 3,
                "panelMetaRows": 3
            }),
            actual: json!({
                "processed": summary.processed,
                "observed": summary.observed,
                "watermarkShiftId": state.watermark.last_consumed_shift_id,
                "watermarkOffset": state.watermark.last_consumed_byte_offset,
                "livePredictions": state.live_predictions,
                "ddaSignals": state.dda_signals,
                "panelRows": state.panel_rows,
                "panelMetaRows": state.panel_meta_rows
            }),
            pass,
        });
        state
    };

    write_shift_log(&harness.shift_log_path, &shift_lines)?;
    let after_restart_state = {
        let subscriber = harness.open_subscriber()?;
        let summary = subscriber.run_until_idle().await?;
        drop(subscriber);
        flush_infer_db(&harness.infer_db_path)?;
        let state =
            read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
        let final_expected_offset = shift_lines
            .last()
            .map(|line| line.next_byte_offset)
            .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_EMPTY_SHIFT_LINES"))?;
        let prediction_ids = prediction_ids_for_session(&harness.infer_db_path, SESSION_HEX)?;
        let pass = summary.processed == 2
            && summary.observed == 2
            && state.watermark.last_consumed_shift_id
                == shift_lines
                    .last()
                    .map(|line| line.shift_id.as_str())
                    .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_EMPTY_SHIFT_LINES"))?
            && state.watermark.last_consumed_byte_offset == final_expected_offset
            && state.live_predictions == 3
            && state.dda_signals == 3
            && state.panel_rows == 9
            && state.panel_meta_rows == 9
            && state.live_predictions == before_restart_state.live_predictions + 2;
        happy_path.push(StepEvidence {
            step: "restart_resume_remaining_shifts".to_string(),
            trigger: "drop subscriber, reopen subscriber, run until idle on full JSONL".to_string(),
            source_of_truth: format!(
                "{} + {} + {}",
                context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
                context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS
            ),
            before: json!(state_as_json(&before_restart_state)),
            after: json!({
                "summary": summary,
                "state": state_as_json(&state),
                "distinctPredictionIds": prediction_ids
            }),
            expected: json!({
                "processedAfterRestart": 2,
                "observedAfterRestart": 2,
                "finalWatermarkOffset": final_expected_offset,
                "livePredictions": 3,
                "ddaSignals": 3,
                "panelRows": 9,
                "panelMetaRows": 9
            }),
            actual: json!({
                "processedAfterRestart": summary.processed,
                "observedAfterRestart": summary.observed,
                "finalWatermarkOffset": state.watermark.last_consumed_byte_offset,
                "livePredictions": state.live_predictions,
                "ddaSignals": state.dda_signals,
                "panelRows": state.panel_rows,
                "panelMetaRows": state.panel_meta_rows,
                "distinctPredictionIds": prediction_ids.len()
            }),
            pass,
        });
        state
    };

    let reopened_state =
        read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
    let readback_pass = restarted_state_matches(&after_restart_state, &reopened_state);
    happy_path.push(StepEvidence {
        step: "independent_reopen_readback".to_string(),
        trigger: "drop subscriber handle, reopen RocksDB/PanelStore, read source of truth"
            .to_string(),
        source_of_truth: format!(
            "{} + {} + {}",
            context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
            context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            CF_MEJEPA_PANEL_META
        ),
        before: json!(state_as_json(&after_restart_state)),
        after: json!(state_as_json(&reopened_state)),
        expected: json!({"stateStableAfterReopen": true}),
        actual: json!({"stateStableAfterReopen": readback_pass}),
        pass: readback_pass,
    });

    let boundary_cases = vec![
        boundary_duplicate_shift_skipped(&harness, &shift_lines[0]).await?,
        boundary_backwards_watermark_rejected(&harness, &reopened_state)?,
        boundary_malformed_jsonl_fails_closed(&harness.fsv_root.join("boundary-malformed-jsonl"))
            .await?,
        boundary_zero_session_watermark_rejected(&harness.fsv_root.join("boundary-zero-session"))?,
    ];

    let cf_counts_after = read_infer_counts(&harness.infer_db_path)?;
    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    let failed_steps = happy_path
        .iter()
        .filter(|step| !step.pass)
        .map(|step| step.step.as_str())
        .collect::<Vec<_>>();
    let failed_boundaries = boundary_cases
        .iter()
        .filter(|case| !case.pass)
        .map(|case| case.case.as_str())
        .collect::<Vec<_>>();
    ensure!(
        all_passed,
        "restart-safety FSV contains failed evidence: steps={failed_steps:?} boundaries={failed_boundaries:?}"
    );

    Ok(RestartSafetyFsvReport {
        fsv_root: fsv_root.display().to_string(),
        task_id: TASK_ID.to_string(),
        build_release_sha,
        source_of_truth: RestartSafetySourceOfTruth {
            infer_db_path: harness.infer_db_path.display().to_string(),
            panel_db_path: harness.panel_db_path.display().to_string(),
            shift_log_path: harness.shift_log_path.display().to_string(),
            watermark_cf: context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK.to_string(),
            live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            dda_signal_cf: context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS.to_string(),
            final_watermark_offset: reopened_state.watermark.last_consumed_byte_offset,
            report_path: report_path.display().to_string(),
        },
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after,
        physical_artifacts: collect_file_artifacts(fsv_root)?,
        readback_equal: true,
        all_passed,
    })
}

pub fn write_report_with_readback(report: &RestartSafetyFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback: RestartSafetyFsvReport = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *report, "restart-safety FSV readback mismatch");
    Ok(())
}

impl Harness {
    fn new(fsv_root: &Path) -> Self {
        let shift_log_dir = fsv_root.join("cgreality-shift-log");
        Self {
            fsv_root: fsv_root.to_path_buf(),
            infer_db_path: fsv_root.join("infer-rocksdb"),
            panel_db_path: fsv_root.join("panel-rocksdb"),
            repo_root: fsv_root.join("repo"),
            shift_log_path: shift_log_dir.join(format!("{SESSION_HEX}.jsonl")),
            shift_log_dir,
        }
    }

    fn config(&self) -> MeJepaShiftSubscriberConfig {
        MeJepaShiftSubscriberConfig {
            infer_db_path: self.infer_db_path.clone(),
            panel_db_path: self.panel_db_path.clone(),
            repo_root: self.repo_root.clone(),
            shift_log_dir: self.shift_log_dir.clone(),
            l_step_observe_threshold: 0.05,
            max_concurrent_shifts: 1,
            lag_alert_threshold_shifts: 500,
            lag_alert_sustain_seconds: 60,
            tail_poll_interval_ms: 10,
        }
    }

    fn open_subscriber(&self) -> Result<ShiftSubscriber> {
        Ok(ShiftSubscriber::open(self.config())?)
    }
}

fn build_shift_lines() -> Result<Vec<ShiftLine>> {
    let mut out = Vec::new();
    let mut offset = 0u64;
    for idx in FIRST_SHIFT_INDEX..=LAST_SHIFT_INDEX {
        let shift_id = format!("01J{idx:020X}");
        let before = format!("def restart_answer_{idx}():\n    return {idx}\n");
        let after = format!("def restart_answer_{idx}():\n    return {}\n", idx + 1);
        let line = serde_json::to_string(&json!({
            "shift_id": shift_id,
            "timestamp_unix_ns": 1_772_000_000_000_000_000u128 + u128::from(idx * 1_000_000),
            "tool_name": "harness_apply_line_window_edit",
            "tool_use_id": format!("toolu-restart-{idx}"),
            "session_id": SESSION_HEX,
            "subject": {
                "task_id": format!("restart_safety_attempt_{idx}"),
                "path": format!("restart_safety_{idx}.py"),
                "language": "python",
                "tests": [format!("test_restart_safety_{idx}")],
                "problem_statement": "restart-safety harness shift",
                "os": std::env::consts::OS
            },
            "before": {
                "text": before,
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(before.as_bytes())))
            },
            "after": {
                "text": after,
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(after.as_bytes())))
            },
            "delta_summary": {"commit_message": format!("restart safety shift {idx}")},
            "verification": {
                "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment()),
                "l_step": 0.8,
                "delta_p": 0.9,
                "delta_k": 0.8,
                "delta_omega": 0.7,
                "delta_xi": 0.6,
                "actual_test_pass": [1.0]
            },
            "harness_transition_path": null
        }))?;
        let next_byte_offset = offset + line.len() as u64 + 1;
        out.push(ShiftLine {
            shift_id,
            line,
            byte_offset: offset,
            next_byte_offset,
        });
        offset = next_byte_offset;
    }
    Ok(out)
}

fn write_shift_log(path: &Path, lines: &[ShiftLine]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options
        .open(path)
        .with_context(|| format!("open write {}", path.display()))?;
    for line in lines {
        file.write_all(line.line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    Ok(())
}

fn seed_infer_db(path: &Path) -> Result<()> {
    let db = open_infer_rocksdb(path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: context_graph_mejepa::Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    let norms = vec![0.01; examples.len()];
    calibration.calibrate(
        &examples,
        &norms,
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::<EmbedderId, String>::new(),
    )?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_TRAIN_CERT_CF_MISSING"))?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 44,
    };
    let value = bincode::serialize(&cert)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, b"cert:restart-safety:0001", &value, &opts)?;
    let readback = db
        .get_cf(cf, b"cert:restart-safety:0001")?
        .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_TRAIN_CERT_READBACK_MISSING"))?;
    ensure!(readback == value, "train cert readback mismatch");
    flush_db_cfs(db.as_ref(), context_graph_mejepa::MEJEPA_INFER_CFS)?;
    drop(db);
    Ok(())
}

fn read_restart_state(
    infer_db_path: &Path,
    panel_db_path: &Path,
    session_hex: &str,
) -> Result<RestartState> {
    let db = open_infer_rocksdb(infer_db_path)?;
    let writer = WatermarkWriter::new(db.clone())?;
    let session = decode_session_hex32(session_hex)?;
    let watermark = writer
        .read(session)?
        .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_WATERMARK_MISSING"))?;
    let store = RocksDbInferStore::new(db.clone());
    let live_predictions = store.read_live_predictions(session, 1000)?.len() as u64;
    let oracle_verdicts = context_graph_mejepa::calibration::count_cf(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
    )?;
    let train_certs = context_graph_mejepa::calibration::count_cf(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
    )?;
    let dda_signals = count_persisted_dda_signals(db.as_ref())? as u64;
    drop(db);
    let panels = PanelStore::open(panel_db_path)?;
    let panel_rows = panels.count_cf(CF_MEJEPA_PANELS)? as u64;
    let panel_meta_rows = panels.count_cf(CF_MEJEPA_PANEL_META)? as u64;
    Ok(RestartState {
        watermark,
        live_predictions,
        oracle_verdicts,
        train_certs,
        dda_signals,
        panel_rows,
        panel_meta_rows,
    })
}

fn read_infer_counts(infer_db_path: &Path) -> Result<BTreeMap<String, u64>> {
    let db = open_infer_rocksdb(infer_db_path)?;
    let mut out = BTreeMap::new();
    for cf_name in [
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
        context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
        context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS,
    ] {
        out.insert(
            cf_name.to_string(),
            context_graph_mejepa::calibration::count_cf(&db, cf_name)?,
        );
    }
    Ok(out)
}

fn prediction_ids_for_session(infer_db_path: &Path, session_hex: &str) -> Result<BTreeSet<String>> {
    let db = open_infer_rocksdb(infer_db_path)?;
    let store = RocksDbInferStore::new(db);
    let predictions = store.read_live_predictions(decode_session_hex32(session_hex)?, 1000)?;
    Ok(predictions
        .into_iter()
        .map(|prediction| hex::encode(prediction.prediction_id))
        .collect())
}

async fn boundary_duplicate_shift_skipped(
    harness: &Harness,
    first_line: &ShiftLine,
) -> Result<BoundaryEvidence> {
    let before = read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
    let mut tail = ShiftLogTail::new(&harness.shift_log_path, first_line.byte_offset);
    let entry = tail
        .poll_next_line()
        .await?
        .ok_or_else(|| anyhow!("MEJEPA_RESTART_SAFETY_FIRST_ENTRY_MISSING"))?;
    let subscriber = harness.open_subscriber()?;
    let outcome = subscriber.process_shift(&entry, false).await?;
    drop(subscriber);
    let after = read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
    let skipped = matches!(
        outcome,
        ShiftOutcome::Skipped {
            reason: SkipReason::AlreadyConsumed,
            ..
        }
    );
    Ok(BoundaryEvidence {
        case: "duplicate_shift_skipped_without_double_counting".to_string(),
        trigger: "direct process_shift of first entry after final watermark".to_string(),
        expected_error_code: "none".to_string(),
        actual_error_code: "none".to_string(),
        before: json!(state_as_json(&before)),
        after: json!({"outcome": outcome, "state": state_as_json(&after)}),
        pass: skipped && restarted_state_matches(&before, &after),
    })
}

fn boundary_backwards_watermark_rejected(
    harness: &Harness,
    state: &RestartState,
) -> Result<BoundaryEvidence> {
    let db = open_infer_rocksdb(&harness.infer_db_path)?;
    let writer = WatermarkWriter::new(db)?;
    let requested = WatermarkRecord {
        session_id: SESSION_HEX.to_string(),
        last_consumed_shift_id: "01J00000000000000000001".to_string(),
        last_consumed_byte_offset: state.watermark.last_consumed_byte_offset - 1,
        last_advanced_at_unix_seconds: state.watermark.last_advanced_at_unix_seconds + 1,
        producer_tool_name: Some("restart-boundary".to_string()),
        source_log_path: state.watermark.source_log_path.clone(),
    };
    let err = writer
        .write_watermark(&requested)
        .expect_err("backwards watermark must fail closed");
    drop(writer);
    let after = read_restart_state(&harness.infer_db_path, &harness.panel_db_path, SESSION_HEX)?;
    Ok(BoundaryEvidence {
        case: "backwards_watermark_rejected".to_string(),
        trigger: "WatermarkWriter::write_watermark(offset=current-1)".to_string(),
        expected_error_code: "MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS".to_string(),
        actual_error_code: err.code().to_string(),
        before: json!(state_as_json(state)),
        after: json!({"state": state_as_json(&after), "error": err.to_string()}),
        pass: err.code() == "MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS"
            && restarted_state_matches(state, &after),
    })
}

async fn boundary_malformed_jsonl_fails_closed(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let harness = Harness::new(root);
    fs::create_dir_all(&harness.repo_root)?;
    fs::create_dir_all(&harness.shift_log_dir)?;
    seed_infer_db(&harness.infer_db_path)?;
    write_bytes_readback(&harness.shift_log_path, b"{not-json}\n")?;
    let before = read_infer_counts(&harness.infer_db_path)?;
    let subscriber = harness.open_subscriber()?;
    let err = subscriber
        .run_until_idle()
        .await
        .expect_err("malformed JSONL must fail closed");
    drop(subscriber);
    let after = read_infer_counts(&harness.infer_db_path)?;
    let watermarks = {
        let db = open_infer_rocksdb(&harness.infer_db_path)?;
        WatermarkWriter::new(db)?.read_all()?
    };
    Ok(BoundaryEvidence {
        case: "malformed_jsonl_fails_before_watermark".to_string(),
        trigger: "run_until_idle on malformed shift-log JSONL".to_string(),
        expected_error_code: "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL".to_string(),
        actual_error_code: err.code().to_string(),
        before: json!({"cfCounts": before}),
        after: json!({"cfCounts": after, "watermarkRows": watermarks.len(), "error": err.to_string()}),
        pass: err.code() == "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL"
            && before == after
            && watermarks.is_empty(),
    })
}

fn boundary_zero_session_watermark_rejected(root: &Path) -> Result<BoundaryEvidence> {
    fs::create_dir_all(root)?;
    let db = open_infer_rocksdb(root.join("infer-rocksdb"))?;
    let writer = WatermarkWriter::new(db.clone())?;
    let record = WatermarkRecord {
        session_id: "00000000000000000000000000000000".to_string(),
        last_consumed_shift_id: "01J00000000000000000001".to_string(),
        last_consumed_byte_offset: 1,
        last_advanced_at_unix_seconds: 1,
        producer_tool_name: Some("restart-boundary".to_string()),
        source_log_path: Some(root.join("zero.jsonl").display().to_string()),
    };
    let err = writer
        .write_watermark(&record)
        .expect_err("zero session watermark must fail closed");
    let count = context_graph_mejepa::calibration::count_cf(
        &db,
        context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
    )?;
    Ok(BoundaryEvidence {
        case: "zero_session_watermark_rejected".to_string(),
        trigger: "WatermarkWriter::write_watermark(session_id=zero)".to_string(),
        expected_error_code: "MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT".to_string(),
        actual_error_code: err.code().to_string(),
        before: json!({"watermarkRows": 0}),
        after: json!({"watermarkRows": count, "error": err.to_string()}),
        pass: err.code() == "MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT" && count == 0,
    })
}

fn state_as_json(state: &RestartState) -> Value {
    json!({
        "watermark": state.watermark,
        "livePredictions": state.live_predictions,
        "oracleVerdicts": state.oracle_verdicts,
        "trainCerts": state.train_certs,
        "ddaSignals": state.dda_signals,
        "panelRows": state.panel_rows,
        "panelMetaRows": state.panel_meta_rows
    })
}

fn restarted_state_matches(left: &RestartState, right: &RestartState) -> bool {
    left.watermark == right.watermark
        && left.live_predictions == right.live_predictions
        && left.oracle_verdicts == right.oracle_verdicts
        && left.train_certs == right.train_certs
        && left.dda_signals == right.dda_signals
        && left.panel_rows == right.panel_rows
        && left.panel_meta_rows == right.panel_meta_rows
}

fn flush_infer_db(path: &Path) -> Result<()> {
    let db = open_infer_rocksdb(path)?;
    flush_db_cfs(db.as_ref(), context_graph_mejepa::MEJEPA_INFER_CFS)
}

fn flush_db_cfs(db: &DB, cfs: &[&str]) -> Result<()> {
    for cf_name in cfs {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    let mut file = options
        .open(path)
        .with_context(|| format!("open write {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    ensure!(
        readback == bytes,
        "file readback mismatch for {}",
        path.display()
    );
    Ok(())
}

fn remove_existing_fsv_root(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let temp_dir = std::env::temp_dir();
    let allowed = path.starts_with("/mnt/d/contextgraph/fsv/") || path.starts_with(&temp_dir);
    ensure!(
        allowed,
        "refusing to remove existing restart-safety FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    let mut last_err = None;
    for attempt in 0..5 {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(Duration::from_millis(50 * (attempt + 1) as u64));
            }
        }
    }
    if !path.exists() {
        return Ok(());
    }
    Err(last_err.expect("remove_dir_all attempted at least once"))
        .with_context(|| format!("remove {}", path.display()))
}

fn collect_file_artifacts(root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_file_artifacts_inner(path: &Path, out: &mut Vec<FileArtifact>) -> Result<()> {
    if path.is_file() {
        let bytes = fs::read(path)?;
        out.push(FileArtifact {
            path: path.display().to_string(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
        return Ok(());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect_file_artifacts_inner(&entry?.path(), out)?;
        }
    }
    Ok(())
}

fn git_head_short() -> String {
    match Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn task_test_002_restart_safety_resumes_without_loss_or_duplicate() {
        let fsv_root = default_fsv_root();
        let report_path = default_report_path(&fsv_root);
        let report = run_restart_safety_fsv(&fsv_root)
            .await
            .expect("restart-safety FSV");
        write_report_with_readback(&report, &report_path).expect("write restart-safety report");
        assert!(report.all_passed);
        assert!(report.readback_equal);
        assert!(report.happy_path.len() >= 3);
        assert!(report.boundary_cases.len() >= 3);
        assert!(report.source_of_truth.final_watermark_offset > 0);
    }
}
