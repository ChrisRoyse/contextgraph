use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use context_graph_mejepa::calibration::{count_cf, CF_MEJEPA_LIVE_PREDICTIONS, MEJEPA_INFER_CFS};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::heal::{
    all_referenced_cf_names, bootstrap_pipeline_for_path, CF_MEJEPA_DRIFT_WINDOW,
    CF_MEJEPA_SHIFT_WATERMARK,
};
use context_graph_mejepa::{
    build_deterministic_compiler, materialize_inference_panels, open_infer_rocksdb, ActualOutcome,
    AgentId, AstDiff, CalibrationStore, DiffHunk, FeedbackId, FeedbackKind, Language,
    MeJepaInferConfig, MejepaStore, OracleOutcome, PatchBundle, PredictionId, RealityPrediction,
    RocksDbInferStore, SurpriseEvent, SurpriseSeverity, TaskContext, TaskEnvironment, TaskId,
    TestId, VerifyVerdict, WitnessHash,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_AGENT_FEEDBACK, CF_MEJEPA_ORACLE_VERDICTS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E2eFsvReport {
    pub fsv_root: String,
    pub task_id: String,
    pub build_release_sha: String,
    pub source_of_truth: SourceOfTruth,
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
pub struct SourceOfTruth {
    pub d_root: String,
    pub infer_db_path: String,
    pub heal_db_path: String,
    pub shift_log_path: String,
    pub test_outcome_path: String,
    pub injection_markdown_path: String,
    pub live_prediction_cf: String,
    pub feedback_cf: String,
    pub oracle_cf: String,
    pub observe_watermark_cf: String,
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

struct E2ePaths {
    fsv_root: PathBuf,
    d_root: PathBuf,
    repo_root: PathBuf,
    infer_db_path: PathBuf,
    heal_root: PathBuf,
    heal_db_path: PathBuf,
    shift_log_path: PathBuf,
    test_outcome_path: PathBuf,
    injection_markdown_path: PathBuf,
}

struct E2eHarness {
    paths: E2ePaths,
    session_id: [u8; 16],
}

struct EditArtifact {
    shift_id: String,
    patch: PatchBundle,
    context: TaskContext,
    pre_sha_hex: String,
    post_sha_hex: String,
}

struct FeedbackArtifact {
    feedback_id: FeedbackId,
    feedback_key: Vec<u8>,
    event: SurpriseEvent,
    rows_before: u64,
    rows_after: u64,
}

struct OutcomeArtifact {
    rows_before: u64,
    rows_after: u64,
    line_sha256: String,
}

struct ObserveArtifact {
    output: context_graph_mejepa::heal::ObserveOutput,
    drift_rows_before: u64,
    drift_rows_after: u64,
    watermark_value: u64,
}

pub fn run_e2e_happy_path_fsv(fsv_root: &Path) -> Result<E2eFsvReport> {
    let harness = E2eHarness::create(fsv_root)?;
    let mut happy_path = Vec::new();
    let mut boundary_cases = Vec::new();
    let build_release_sha = git_head_short();

    let cf_counts_before = harness.initial_counts()?;

    let (first_prediction, second_prediction, final_counts) = {
        let db = open_infer_rocksdb(&harness.paths.infer_db_path)
            .context("open E2E inference RocksDB")?;
        let calibration_store = seed_calibration(db.clone())?;
        let store = Arc::new(RocksDbInferStore::new(db.clone()));
        let compiler = build_deterministic_compiler(
            harness.paths.repo_root.clone(),
            store.clone(),
            calibration_store,
            MeJepaInferConfig {
                outcome_set_max: 4,
                ..MeJepaInferConfig::default()
            },
        )?;

        let edit = harness.make_edit("problematic", "", BAD_SOURCE)?;
        happy_path.push(StepEvidence {
            step: "edit_to_shift_log".to_string(),
            trigger: "make_edit(problematic)".to_string(),
            source_of_truth: harness.paths.shift_log_path.display().to_string(),
            before: json!({"shiftRows": 0}),
            after: json!({
                "shiftRows": shift_log_line_count(&harness.paths.shift_log_path)?,
                "shiftId": edit.shift_id,
                "preSha": edit.pre_sha_hex,
                "postSha": edit.post_sha_hex
            }),
            expected: json!({"shiftRows": 1}),
            actual: json!({"shiftRows": shift_log_line_count(&harness.paths.shift_log_path)?}),
            pass: shift_log_line_count(&harness.paths.shift_log_path)? == 1,
        });

        let predictions_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        let first_prediction = verify_approved(&compiler, &store, &edit.patch, &edit.context)
            .context("first verify should approve and persist")?;
        let predictions_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        ensure!(
            predictions_after == predictions_before + 1,
            "first prediction count did not increment"
        );
        happy_path.push(StepEvidence {
            step: "predict_readback".to_string(),
            trigger: "MeJepaCompiler::verify(problematic patch)".to_string(),
            source_of_truth: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            before: json!({"rows": predictions_before}),
            after: prediction_summary(&first_prediction, predictions_after),
            expected: json!({"rows": predictions_before + 1, "persisted": true}),
            actual: json!({"rows": predictions_after, "persisted": true}),
            pass: true,
        });

        let markdown = render_prediction_markdown(&first_prediction);
        write_bytes_readback(&harness.paths.injection_markdown_path, markdown.as_bytes())?;
        let injected = fs::read_to_string(&harness.paths.injection_markdown_path)?;
        ensure!(
            injected.contains("Reality prediction") && injected.contains("Prediction ID"),
            "injection markdown missing prediction block"
        );
        happy_path.push(StepEvidence {
            step: "inject_markdown".to_string(),
            trigger: "render latest RealityPrediction into UserPromptSubmit-style markdown"
                .to_string(),
            source_of_truth: harness.paths.injection_markdown_path.display().to_string(),
            before: json!(null),
            after: json!({"bytes": injected.len(), "containsPredictionId": injected.contains("Prediction ID")}),
            expected: json!({"containsPredictionId": true}),
            actual: json!({"containsPredictionId": injected.contains("Prediction ID")}),
            pass: true,
        });

        let feedback = harness.persist_feedback(db.as_ref(), &first_prediction)?;
        happy_path.push(StepEvidence {
            step: "feedback_persistence".to_string(),
            trigger: "persist SurpriseEvent for first prediction".to_string(),
            source_of_truth: CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            before: json!({"rows": feedback.rows_before}),
            after: json!({
                "rows": feedback.rows_after,
                "feedbackId": hex::encode(feedback.feedback_id.0),
                "feedbackKeyHex": hex::encode(&feedback.feedback_key),
                "severity": format!("{:?}", feedback.event.severity)
            }),
            expected: json!({"rows": feedback.rows_before + 1}),
            actual: json!({"rows": feedback.rows_after}),
            pass: feedback.rows_after == feedback.rows_before + 1,
        });

        let outcome = harness.persist_test_outcome(db.as_ref(), &first_prediction)?;
        happy_path.push(StepEvidence {
            step: "test_outcome_to_oracle".to_string(),
            trigger: "append measured test outcome JSONL and persist oracle verdict".to_string(),
            source_of_truth: format!(
                "{} + {}",
                harness.paths.test_outcome_path.display(),
                CF_MEJEPA_ORACLE_VERDICTS
            ),
            before: json!({"oracleRows": outcome.rows_before}),
            after: json!({
                "oracleRows": outcome.rows_after,
                "testOutcomeLineSha256": outcome.line_sha256
            }),
            expected: json!({"oracleRows": outcome.rows_before + 1}),
            actual: json!({"oracleRows": outcome.rows_after}),
            pass: outcome.rows_after == outcome.rows_before + 1,
        });

        let observe = harness.observe(&compiler, &edit.patch, &edit.context)?;
        happy_path.push(StepEvidence {
            step: "observe_heal_pipeline".to_string(),
            trigger: "MeJepaCompiler::observe(panel_t2, OracleOutcome::Fail)".to_string(),
            source_of_truth: format!("{CF_MEJEPA_DRIFT_WINDOW} + {CF_MEJEPA_SHIFT_WATERMARK}"),
            before: json!({"driftRows": observe.drift_rows_before}),
            after: json!({
                "driftRows": observe.drift_rows_after,
                "watermark": observe.watermark_value,
                "observeOutput": observe.output
            }),
            expected: json!({"watermark": 1, "driftRowsAtLeast": observe.drift_rows_before + 1}),
            actual: json!({"watermark": observe.watermark_value, "driftRows": observe.drift_rows_after}),
            pass: observe.watermark_value == 1 && observe.drift_rows_after > observe.drift_rows_before,
        });

        let improved_edit = harness.make_edit("improved", BAD_SOURCE, GOOD_SOURCE)?;
        let second_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        let second_prediction = verify_approved(
            &compiler,
            &store,
            &improved_edit.patch,
            &improved_edit.context,
        )
        .context("second verify should approve and persist")?;
        let second_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        let first_risk = prediction_risk_score(&first_prediction);
        let second_risk = prediction_risk_score(&second_prediction);
        ensure!(
            second_after == second_before + 1,
            "second prediction did not persist"
        );
        ensure!(
            second_risk < first_risk,
            "improved edit did not reduce predicted risk score: first={first_risk}, second={second_risk}"
        );
        happy_path.push(StepEvidence {
            step: "improved_prediction_readback".to_string(),
            trigger: "make improving edit then verify/readback second prediction".to_string(),
            source_of_truth: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            before: json!({"rows": second_before, "riskScore": first_risk}),
            after: json!({
                "rows": second_after,
                "riskScore": second_risk,
                "predictionId": hex::encode(second_prediction.prediction_id)
            }),
            expected: json!({"rows": second_before + 1, "riskReduced": true}),
            actual: json!({"rows": second_after, "riskReduced": second_risk < first_risk}),
            pass: true,
        });

        boundary_cases.push(harness.boundary_empty_patch(db.as_ref())?);
        boundary_cases.push(harness.boundary_zero_session_context(db.as_ref())?);
        boundary_cases.push(harness.boundary_unknown_feedback_prediction(db.as_ref())?);
        boundary_cases.push(harness.boundary_observe_nan_signal()?);

        flush_infer_db(db.as_ref())?;
        let final_counts = harness.count_all_open(db.as_ref())?;
        (first_prediction, second_prediction, final_counts)
    };

    let reopened_counts = harness.reopen_counts()?;
    ensure!(
        reopened_counts.get(CF_MEJEPA_LIVE_PREDICTIONS)
            == final_counts.get(CF_MEJEPA_LIVE_PREDICTIONS),
        "live prediction count changed after reopen"
    );
    ensure!(
        reopened_counts.get(CF_MEJEPA_AGENT_FEEDBACK) == final_counts.get(CF_MEJEPA_AGENT_FEEDBACK),
        "feedback count changed after reopen"
    );

    ensure!(
        first_prediction.prediction_id != second_prediction.prediction_id,
        "second prediction reused first prediction id"
    );

    let all_passed =
        happy_path.iter().all(|step| step.pass) && boundary_cases.iter().all(|case| case.pass);
    ensure!(
        all_passed,
        "E2E report contains failed happy-path or boundary evidence"
    );

    let report = E2eFsvReport {
        fsv_root: harness.paths.fsv_root.display().to_string(),
        task_id: "TASK-TEST-001".to_string(),
        build_release_sha,
        source_of_truth: harness.source_of_truth(),
        happy_path,
        boundary_cases,
        cf_counts_before,
        cf_counts_after: reopened_counts,
        physical_artifacts: collect_file_artifacts(&harness.paths.fsv_root)?,
        readback_equal: true,
        all_passed,
    };
    Ok(report)
}

#[allow(dead_code)]
pub fn write_report_with_readback(report: &E2eFsvReport, path: &Path) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    write_bytes_readback(path, &bytes)?;
    let readback_bytes = fs::read(path)?;
    let readback: E2eFsvReport = serde_json::from_slice(&readback_bytes)?;
    let expected_json: Value = serde_json::from_slice(&bytes)?;
    let actual_json = serde_json::to_value(&readback)?;
    ensure!(
        actual_json == expected_json,
        "FSV evidence semantic readback did not match"
    );
    Ok(())
}

impl E2eHarness {
    fn create(fsv_root: &Path) -> Result<Self> {
        remove_existing_fsv_root(fsv_root)?;
        fs::create_dir_all(fsv_root)?;
        let d_root = fsv_root.join("d-root");
        let repo_root = d_root.join("repo");
        let infer_db_path = d_root.join("state/mejepa-infer-rocksdb");
        let heal_root = d_root.join("state/heal");
        let heal_db_path = heal_root.join("heal-rocksdb");
        let shift_log_path = d_root.join("runtime/cgreality-shift-log/e2e-session.jsonl");
        let test_outcome_path = d_root.join("runtime/test-outcomes/e2e-session.jsonl");
        let injection_markdown_path = d_root.join("runtime/injected-prediction.md");
        fs::create_dir_all(repo_root.join("src"))?;
        fs::create_dir_all(shift_log_path.parent().context("shift log parent")?)?;
        fs::create_dir_all(test_outcome_path.parent().context("test outcome parent")?)?;
        fs::create_dir_all(heal_root.parent().context("heal root parent")?)?;
        Ok(Self {
            paths: E2ePaths {
                fsv_root: fsv_root.to_path_buf(),
                d_root,
                repo_root,
                infer_db_path,
                heal_root,
                heal_db_path,
                shift_log_path,
                test_outcome_path,
                injection_markdown_path,
            },
            session_id: [0x64; 16],
        })
    }

    fn source_of_truth(&self) -> SourceOfTruth {
        SourceOfTruth {
            d_root: self.paths.d_root.display().to_string(),
            infer_db_path: self.paths.infer_db_path.display().to_string(),
            heal_db_path: self.paths.heal_db_path.display().to_string(),
            shift_log_path: self.paths.shift_log_path.display().to_string(),
            test_outcome_path: self.paths.test_outcome_path.display().to_string(),
            injection_markdown_path: self.paths.injection_markdown_path.display().to_string(),
            live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            feedback_cf: CF_MEJEPA_AGENT_FEEDBACK.to_string(),
            oracle_cf: CF_MEJEPA_ORACLE_VERDICTS.to_string(),
            observe_watermark_cf: CF_MEJEPA_SHIFT_WATERMARK.to_string(),
        }
    }

    fn initial_counts(&self) -> Result<BTreeMap<String, u64>> {
        let db = open_infer_rocksdb(&self.paths.infer_db_path)?;
        self.count_all_open(db.as_ref())
    }

    fn reopen_counts(&self) -> Result<BTreeMap<String, u64>> {
        let db = open_infer_rocksdb(&self.paths.infer_db_path)?;
        self.count_all_open(db.as_ref())
    }

    fn count_all_open(&self, db: &DB) -> Result<BTreeMap<String, u64>> {
        let mut counts = BTreeMap::new();
        for cf in [
            CF_MEJEPA_LIVE_PREDICTIONS,
            CF_MEJEPA_AGENT_FEEDBACK,
            CF_MEJEPA_ORACLE_VERDICTS,
            CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
        ] {
            counts.insert(cf.to_string(), count_cf_any(db, cf)?);
        }
        if self.paths.heal_db_path.exists() {
            let storage =
                context_graph_mejepa::heal::HealRocksStore::open(&self.paths.heal_db_path)?;
            counts.insert(
                CF_MEJEPA_DRIFT_WINDOW.to_string(),
                storage.count_cf(CF_MEJEPA_DRIFT_WINDOW)?,
            );
            counts.insert(
                CF_MEJEPA_SHIFT_WATERMARK.to_string(),
                storage.count_cf(CF_MEJEPA_SHIFT_WATERMARK)?,
            );
        } else {
            counts.insert(CF_MEJEPA_DRIFT_WINDOW.to_string(), 0);
            counts.insert(CF_MEJEPA_SHIFT_WATERMARK.to_string(), 0);
        }
        Ok(counts)
    }

    fn make_edit(&self, name: &str, before: &str, after: &str) -> Result<EditArtifact> {
        let rel_path = PathBuf::from("src/e2e_target.py");
        let abs_path = self.paths.repo_root.join(&rel_path);
        let pre_sha = sha256_bytes(before.as_bytes());
        let post_sha = sha256_bytes(after.as_bytes());
        write_bytes_readback(&abs_path, after.as_bytes())?;
        let observed = sha256_bytes(&fs::read(&abs_path)?);
        ensure!(observed == post_sha, "edit post-sha readback mismatch");
        let shift_id = format!("shift-{name}-{}", hex::encode(&post_sha[..4]));
        let shift = json!({
            "shiftId": shift_id,
            "sessionId": hex::encode(self.session_id),
            "file": rel_path,
            "preSha": hex::encode(pre_sha),
            "postSha": hex::encode(post_sha),
            "beforeLen": before.len(),
            "afterLen": after.len()
        });
        append_json_line(&self.paths.shift_log_path, &shift)?;
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: rel_path,
                    pre_sha,
                    post_sha,
                    before: before.to_string(),
                    after: after.to_string(),
                }],
            },
            valid_witness_segment(),
            format!("Phase F E2E {name} edit"),
            sha256_bytes(format!("phase-f-e2e:{name}:{}", hex::encode(post_sha)).as_bytes()),
        )?;
        let context = TaskContext {
            task_id: TaskId("phase-f-e2e-happy-path".to_string()),
            session_id: self.session_id,
            language: Language::Python,
            problem_statement: "Phase F E2E harness validates the live prediction loop".to_string(),
            tests: vec![TestId("test_normalize_happy_path".to_string())],
            environment: TaskEnvironment {
                repo_root: self.paths.repo_root.clone(),
                python_version: Some("3.11".to_string()),
                os: std::env::consts::OS.to_string(),
            },
            claim_graph: None,
            skill_citations: vec![],
        };
        context.validate()?;
        Ok(EditArtifact {
            shift_id,
            patch,
            context,
            pre_sha_hex: hex::encode(pre_sha),
            post_sha_hex: hex::encode(post_sha),
        })
    }

    fn persist_feedback(
        &self,
        db: &DB,
        prediction: &RealityPrediction,
    ) -> Result<FeedbackArtifact> {
        let rows_before = count_cf_any(db, CF_MEJEPA_AGENT_FEEDBACK)?;
        let event = self.feedback_event(prediction, prediction.created_at_unix_ms + 1)?;
        let feedback_key =
            encode_feedback_key(&event.prediction_id, &event.agent_id, event.ts_millis)?;
        write_feedback_event(db, &feedback_key, &event)?;
        let rows_after = count_cf_any(db, CF_MEJEPA_AGENT_FEEDBACK)?;
        Ok(FeedbackArtifact {
            feedback_id: event.feedback_id,
            feedback_key,
            event,
            rows_before,
            rows_after,
        })
    }

    fn feedback_event(
        &self,
        prediction: &RealityPrediction,
        ts_millis: i64,
    ) -> Result<SurpriseEvent> {
        let prediction_id = PredictionId(prediction.prediction_id);
        let agent_id = AgentId::try_new("phase-f-e2e-agent")?;
        let feedback_id = feedback_id_for(prediction_id, &agent_id, ts_millis);
        SurpriseEvent::try_new(SurpriseEvent {
            feedback_id,
            prediction_id,
            agent_id,
            ts_millis,
            feedback_kind: FeedbackKind::Surprise,
            agent_explanation: "E2E measured failing test after first prediction".to_string(),
            actual_outcome: Some(ActualOutcome {
                oracle_outcome: OracleOutcome::Fail,
                failed_tests: vec![TestId("test_normalize_happy_path".to_string())],
                runtime_ms: Some(37),
                notes: "synthetic but persisted measured outcome from the E2E harness".to_string(),
            }),
            severity: SurpriseSeverity::High,
            extra_structured_data: json!({"source": "TASK-TEST-001", "shiftLog": self.paths.shift_log_path}),
            witness_hash: WitnessHash(prediction.source_panel_sha),
        })
        .context("construct SurpriseEvent")
    }

    fn persist_test_outcome(
        &self,
        db: &DB,
        prediction: &RealityPrediction,
    ) -> Result<OutcomeArtifact> {
        let rows_before = count_cf_any(db, CF_MEJEPA_ORACLE_VERDICTS)?;
        let outcome = json!({
            "sessionId": hex::encode(self.session_id),
            "predictionId": hex::encode(prediction.prediction_id),
            "taskId": prediction.task_id.0,
            "command": "pytest tests/test_normalize.py",
            "status": "failed",
            "oracleOutcome": "fail",
            "failedTests": ["test_normalize_happy_path"],
            "runtimeMs": 37
        });
        append_json_line(&self.paths.test_outcome_path, &outcome)?;
        let line_sha256 = hex::encode(Sha256::digest(serde_json::to_vec(&outcome)?));
        let cf = cf_handle(db, CF_MEJEPA_ORACLE_VERDICTS)?;
        let key = format!("phase-f-e2e:{}", hex::encode(prediction.prediction_id));
        let bytes = serde_json::to_vec(&outcome)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        db.put_cf_opt(cf, key.as_bytes(), &bytes, &opts)?;
        let readback = db
            .get_cf(cf, key.as_bytes())?
            .ok_or_else(|| anyhow!("oracle verdict readback missing"))?;
        ensure!(readback == bytes, "oracle verdict readback mismatch");
        let rows_after = count_cf_any(db, CF_MEJEPA_ORACLE_VERDICTS)?;
        Ok(OutcomeArtifact {
            rows_before,
            rows_after,
            line_sha256,
        })
    }

    fn observe(
        &self,
        compiler: &context_graph_mejepa::MeJepaCompiler,
        patch: &PatchBundle,
        context: &TaskContext,
    ) -> Result<ObserveArtifact> {
        let mut pipeline = bootstrap_pipeline_for_path(&self.paths.heal_root)?;
        let drift_rows_before = pipeline.storage.count_cf(CF_MEJEPA_DRIFT_WINDOW)?;
        let (_, _, panel_t2) = materialize_inference_panels(patch, context)?;
        let session = hex::encode(self.session_id);
        let output = compiler
            .observe(
                &mut pipeline,
                &panel_t2,
                &OracleOutcome::Fail,
                0.9,
                0,
                &session,
            )
            .context("observe failed")?;
        let drift_rows_after = pipeline.storage.count_cf(CF_MEJEPA_DRIFT_WINDOW)?;
        let watermark_bytes = pipeline
            .storage
            .get_cf(CF_MEJEPA_SHIFT_WATERMARK, session.as_bytes())?
            .ok_or_else(|| anyhow!("observe watermark missing after observe"))?;
        ensure!(
            watermark_bytes.len() == 8,
            "observe watermark length mismatch: {}",
            watermark_bytes.len()
        );
        let mut value = [0u8; 8];
        value.copy_from_slice(&watermark_bytes);
        let watermark_value = u64::from_be_bytes(value);
        flush_heal_db(pipeline.storage.db().as_ref())?;
        Ok(ObserveArtifact {
            output,
            drift_rows_before,
            drift_rows_after,
            watermark_value,
        })
    }

    fn boundary_empty_patch(&self, db: &DB) -> Result<BoundaryEvidence> {
        let before = self.count_all_open(db)?;
        let err = PatchBundle::try_new(
            AstDiff { hunks: vec![] },
            valid_witness_segment(),
            "empty patch".to_string(),
            [0; 32],
        )
        .expect_err("empty patch must fail closed");
        let after = self.count_all_open(db)?;
        Ok(BoundaryEvidence {
            case: "empty_patch_rejected".to_string(),
            trigger: "PatchBundle::try_new with zero hunks".to_string(),
            expected_error_code: "MEJEPA_INFER_INVALID_INPUT".to_string(),
            actual_error_code: err.code().to_string(),
            before: json!(before),
            after: json!(after),
            pass: err.code() == "MEJEPA_INFER_INVALID_INPUT" && before == after,
        })
    }

    fn boundary_zero_session_context(&self, db: &DB) -> Result<BoundaryEvidence> {
        let before = self.count_all_open(db)?;
        let mut context = TaskContext {
            task_id: TaskId("phase-f-e2e-boundary".to_string()),
            session_id: [0; 16],
            language: Language::Python,
            problem_statement: "boundary".to_string(),
            tests: vec![TestId("test_boundary".to_string())],
            environment: TaskEnvironment {
                repo_root: self.paths.repo_root.clone(),
                python_version: Some("3.11".to_string()),
                os: std::env::consts::OS.to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        let err = context.validate().expect_err("zero session must fail");
        context.session_id = self.session_id;
        context.validate()?;
        let after = self.count_all_open(db)?;
        Ok(BoundaryEvidence {
            case: "zero_session_context_rejected".to_string(),
            trigger: "TaskContext::validate with zero session id".to_string(),
            expected_error_code: "MEJEPA_INFER_INVALID_INPUT".to_string(),
            actual_error_code: err.code().to_string(),
            before: json!(before),
            after: json!(after),
            pass: err.code() == "MEJEPA_INFER_INVALID_INPUT" && before == after,
        })
    }

    fn boundary_unknown_feedback_prediction(&self, db: &DB) -> Result<BoundaryEvidence> {
        let before = self.count_all_open(db)?;
        let prediction_id = PredictionId([0xEE; 16]);
        let agent_id = AgentId::try_new("phase-f-e2e-agent")?;
        let err = persist_feedback_for_prediction_id(db, prediction_id, &agent_id, 1)
            .expect_err("unknown prediction feedback must fail closed");
        let after = self.count_all_open(db)?;
        Ok(BoundaryEvidence {
            case: "unknown_prediction_feedback_rejected".to_string(),
            trigger: "persist feedback for absent prediction id".to_string(),
            expected_error_code: "MEJEPA_E2E_PREDICTION_NOT_FOUND".to_string(),
            actual_error_code: err.to_string(),
            before: json!(before),
            after: json!(after),
            pass: err.to_string() == "MEJEPA_E2E_PREDICTION_NOT_FOUND" && before == after,
        })
    }

    fn boundary_observe_nan_signal(&self) -> Result<BoundaryEvidence> {
        let mut pipeline =
            bootstrap_pipeline_for_path(&self.paths.fsv_root.join("edge-nan-observe"))?;
        let before_drift = pipeline.storage.count_cf(CF_MEJEPA_DRIFT_WINDOW)?;
        let patch_context = self.make_patch_context_only("nan-observe", GOOD_SOURCE)?;
        let (_, _, panel_t2) = materialize_inference_panels(&patch_context.0, &patch_context.1)?;
        let err = pipeline
            .observe(
                &panel_t2,
                &OracleOutcome::Pass,
                f32::NAN,
                0,
                "phase-f-e2e-nan",
            )
            .expect_err("NaN signal clarity must fail closed");
        let after_drift = pipeline.storage.count_cf(CF_MEJEPA_DRIFT_WINDOW)?;
        Ok(BoundaryEvidence {
            case: "observe_invalid_input_rejected".to_string(),
            trigger: "SelfHealingPipeline::observe with invalid synthetic observe input"
                .to_string(),
            expected_error_code: "MEJEPA_HEAL_INVALID_STATE".to_string(),
            actual_error_code: err.code().to_string(),
            before: json!({"driftRows": before_drift}),
            after: json!({"driftRows": after_drift}),
            pass: err.code() == "MEJEPA_HEAL_INVALID_STATE" && before_drift == after_drift,
        })
    }

    fn make_patch_context_only(
        &self,
        name: &str,
        after: &str,
    ) -> Result<(PatchBundle, TaskContext)> {
        let rel_path = PathBuf::from(format!("src/{name}.py"));
        let abs_path = self.paths.repo_root.join(&rel_path);
        write_bytes_readback(&abs_path, after.as_bytes())?;
        let post_sha = sha256_bytes(after.as_bytes());
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: rel_path,
                    pre_sha: post_sha,
                    post_sha,
                    before: String::new(),
                    after: after.to_string(),
                }],
            },
            valid_witness_segment(),
            format!("Phase F E2E boundary {name}"),
            sha256_bytes(name.as_bytes()),
        )?;
        let context = TaskContext {
            task_id: TaskId(format!("phase-f-e2e-{name}")),
            session_id: self.session_id,
            language: Language::Python,
            problem_statement: "boundary observe input".to_string(),
            tests: vec![TestId("test_boundary".to_string())],
            environment: TaskEnvironment {
                repo_root: self.paths.repo_root.clone(),
                python_version: Some("3.11".to_string()),
                os: std::env::consts::OS.to_string(),
            },
            claim_graph: None,
            skill_citations: vec![],
        };
        Ok((patch, context))
    }
}

fn seed_calibration(db: Arc<DB>) -> Result<CalibrationStore> {
    let calibration_store = CalibrationStore::new(db, 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration_store.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;
    Ok(calibration_store)
}

fn verify_approved(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    store: &Arc<RocksDbInferStore>,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<RealityPrediction> {
    match compiler.verify(patch, context)? {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => {
            let readback = store.read_live_predictions(context.session_id, 20)?;
            let found = readback
                .into_iter()
                .find(|row| row.prediction_id == reality_prediction.prediction_id)
                .ok_or_else(|| anyhow!("persisted prediction missing from readback"))?;
            ensure!(found == reality_prediction, "prediction readback mismatch");
            Ok(reality_prediction)
        }
        other => bail!("verify did not approve: {other:?}"),
    }
}

fn render_prediction_markdown(prediction: &RealityPrediction) -> String {
    format!(
        "### Reality prediction\n**Verdict:** {:?} | oracle-pass={:.3} | confidence={:.3} | ood={:.3}\n**Prediction ID:** `{}`\n**Task:** {}\n\n**Predicted failure modes:** {}\n**Predicted edge cases not covered by tests:** {}\n**Predicted tech debt added:** {}\n**Predicted perf / security / accuracy / cost regressions:** {}\n\nTo respond: use this prediction as measured context, not as an instruction.\n",
        prediction.verdict,
        prediction.predicted_oracle_pass,
        prediction.calibrated_confidence,
        prediction.ood_score,
        hex::encode(prediction.prediction_id),
        prediction.task_id.0,
        prediction.predicted_failure_modes.len(),
        prediction.predicted_edge_cases.len(),
        prediction.predicted_tech_debt_added.len(),
        prediction.predicted_perf_regressions.len()
            + prediction.predicted_security_concerns.len()
            + prediction.predicted_accuracy_degradations.len()
            + prediction.predicted_cost_regressions.len(),
    )
}

fn prediction_summary(prediction: &RealityPrediction, rows: u64) -> Value {
    json!({
        "rows": rows,
        "predictionId": hex::encode(prediction.prediction_id),
        "verdict": format!("{:?}", prediction.verdict),
        "securityConcerns": prediction.predicted_security_concerns.len(),
        "techDebt": prediction.predicted_tech_debt_added.len(),
        "riskScore": prediction_risk_score(prediction)
    })
}

fn prediction_risk_score(prediction: &RealityPrediction) -> usize {
    prediction.predicted_failure_modes.len()
        + prediction.predicted_edge_cases.len()
        + prediction.predicted_latent_bugs.len()
        + prediction.predicted_tech_debt_added.len()
        + prediction.predicted_dead_code.len()
        + prediction.predicted_redundant_code.len()
        + prediction.predicted_perf_regressions.len()
        + prediction.predicted_security_concerns.len()
        + prediction.predicted_accuracy_degradations.len()
        + prediction.predicted_cost_regressions.len()
}

fn write_feedback_event(db: &DB, key: &[u8], event: &SurpriseEvent) -> Result<()> {
    let cf = cf_handle(db, CF_MEJEPA_AGENT_FEEDBACK)?;
    let bytes = serde_json::to_vec(event)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| anyhow!("feedback readback missing"))?;
    ensure!(readback == bytes, "feedback readback bytes differ");
    let decoded: SurpriseEvent = serde_json::from_slice(&readback)?;
    decoded.validate()?;
    ensure!(decoded == *event, "feedback decoded readback differs");
    Ok(())
}

fn persist_feedback_for_prediction_id(
    db: &DB,
    prediction_id: PredictionId,
    agent_id: &AgentId,
    ts_millis: i64,
) -> Result<()> {
    if !prediction_exists(db, prediction_id)? {
        bail!("MEJEPA_E2E_PREDICTION_NOT_FOUND");
    }
    let event = SurpriseEvent::try_new(SurpriseEvent {
        feedback_id: feedback_id_for(prediction_id, agent_id, ts_millis),
        prediction_id,
        agent_id: agent_id.clone(),
        ts_millis,
        feedback_kind: FeedbackKind::Confirmed,
        agent_explanation: "boundary".to_string(),
        actual_outcome: None,
        severity: SurpriseSeverity::Low,
        extra_structured_data: json!({}),
        witness_hash: WitnessHash([0; 32]),
    })?;
    let key = encode_feedback_key(&event.prediction_id, &event.agent_id, ts_millis)?;
    write_feedback_event(db, &key, &event)
}

fn prediction_exists(db: &DB, prediction_id: PredictionId) -> Result<bool> {
    let cf = cf_handle(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate()?;
        if prediction.prediction_id == prediction_id.0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn feedback_id_for(prediction_id: PredictionId, agent_id: &AgentId, ts_millis: i64) -> FeedbackId {
    let mut hasher = Sha256::new();
    hasher.update(prediction_id.0);
    hasher.update(agent_id.0.as_bytes());
    hasher.update(ts_millis.to_be_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    FeedbackId(out)
}

fn encode_feedback_key(
    prediction_id: &PredictionId,
    agent_id: &AgentId,
    ts_millis: i64,
) -> Result<Vec<u8>> {
    bincode::serialize(&(prediction_id, agent_id, ts_millis)).context("encode feedback key")
}

fn append_json_line(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open append {}", path.display()))?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn shift_log_line_count(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::read_to_string(path)?.lines().count())
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
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
        "refusing to remove existing E2E FSV root outside /mnt/d/contextgraph/fsv or temp dir: {}",
        path.display()
    );
    fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))
}

fn count_cf_any(db: &DB, cf_name: &str) -> Result<u64> {
    let cf = cf_handle(db, cf_name)?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn cf_handle<'a>(db: &'a DB, cf_name: &str) -> Result<&'a rocksdb::ColumnFamily> {
    db.cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing column family {cf_name}"))
}

fn flush_infer_db(db: &DB) -> Result<()> {
    for cf_name in MEJEPA_INFER_CFS {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
}

fn flush_heal_db(db: &DB) -> Result<()> {
    for cf_name in all_referenced_cf_names() {
        if let Some(cf) = db.cf_handle(cf_name) {
            db.flush_cf(cf)?;
        }
    }
    Ok(())
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

const BAD_SOURCE: &str = "API_KEY = \"sk-phasef12345678901234567890\"\n# TODO: add owner before release\n\
def normalize(values):\n    digest = \"md5\"\n    if values == [] or len(values) == 0:\n        return \"empty\"\n\
    return [v.clone() for v in values]\n";

const GOOD_SOURCE: &str =
    "def normalize(values):\n    if not values:\n        return []\n    return list(values)\n";
