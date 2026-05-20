//! ME-JEPA compression-progress MCP readback tool.

use std::path::PathBuf;

use anyhow::{bail, Context, Result as AnyhowResult};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_TRAIN_DB: &str = "CONTEXTGRAPH_MEJEPA_TRAIN_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompressionProgressRequest {
    db_path: Option<PathBuf>,
    #[serde(default = "default_window")]
    window: u64,
    #[serde(default = "default_epsilon_bits")]
    epsilon_bits: f64,
}

impl Handlers {
    pub(crate) async fn call_mejepa_compression_progress(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_COMPRESSION_PROGRESS) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_compression_progress(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_COMPRESSION_PROGRESS_FAILED",
                &err.to_string(),
                json!({"toolFamily": "mejepa_compression_progress"}),
            ),
        }
    }
}

fn run_compression_progress(request: CompressionProgressRequest) -> AnyhowResult<Value> {
    let db_path = resolve_train_db_path(request.db_path)?;
    let report = context_graph_mejepa_train::compression_progress_report_from_path(
        &db_path,
        request.window,
        request.epsilon_bits,
    )
    .with_context(|| format!("read compression progress from {}", db_path.display()))?;
    let status = match report.monotonicity_passed {
        Some(true) => "green",
        Some(false) => "regressing",
        None => match report.state {
            context_graph_mejepa_train::CompressionProgressState::Empty => "empty",
            context_graph_mejepa_train::CompressionProgressState::SingleCertificate => {
                "indeterminate"
            }
            context_graph_mejepa_train::CompressionProgressState::Ready => "indeterminate",
        },
    };
    Ok(json!({
        "tool": tool_names::MEJEPA_COMPRESSION_PROGRESS,
        "status": status,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "trainCertCf": context_graph_mejepa_train::CF_MEJEPA_TRAIN_CERTS
        },
        "report": report
    }))
}

fn default_window() -> u64 {
    context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_WINDOW
}

fn default_epsilon_bits() -> f64 {
    context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS
}

fn parse_tool_request<T: DeserializeOwned>(
    args: serde_json::Value,
    tool_name: &str,
) -> Result<T, String> {
    serde_json::from_value(args)
        .map_err(|err| format!("{tool_name} schema validation failed: {err}"))
}

fn resolve_train_db_path(input: Option<PathBuf>) -> AnyhowResult<PathBuf> {
    match input {
        Some(path) if !path.as_os_str().is_empty() => Ok(path),
        Some(_) => bail!("dbPath must be a non-empty path"),
        None => {
            let raw = std::env::var(ENV_TRAIN_DB)
                .or_else(|_| std::env::var(ENV_INFER_DB))
                .with_context(|| {
                    format!("dbPath, {ENV_TRAIN_DB}, or {ENV_INFER_DB} is required")
                })?;
            if raw.trim().is_empty() {
                bail!("{ENV_TRAIN_DB}/{ENV_INFER_DB} must not be empty");
            }
            Ok(PathBuf::from(raw))
        }
    }
}

#[cfg(test)]
pub(in crate::handlers::tools) fn run_compression_progress_write_fsv_artifact() {
    test_support::compression_progress_write_fsv_artifact();
}

#[cfg(test)]
mod test_support {
    use super::*;
    use anyhow::{ensure, Context, Result};
    use context_graph_mejepa::HeadId;
    use context_graph_mejepa::{ActiveLearningSummary, EvalProvenance, EvalReport};
    use context_graph_mejepa_train::cert::{
        open_train_rocksdb, TrainCertWriter, TrainingCertificate, CF_MEJEPA_TRAIN_CERTS,
    };
    use context_graph_mejepa_train::{
        compression_progress_report, compute_l_step, render_compression_progress_weekly_section,
        DeltaKComponents, DeltaOmegaComponents, DeltaPComponents, DeltaXiComponents,
    };
    use rocksdb::DB;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    pub(super) fn compression_progress_write_fsv_artifact() {
        let report = run_fsv().expect("compression progress FSV");
        assert_eq!(report["all_passed"], true);
    }

    fn run_fsv() -> Result<Value> {
        let temp = TempDir::new().context("tempdir")?;
        let db_path = temp.path().join("compression-progress.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open train db")?;
        seed_cert_chain(db.clone(), &[8.0, 6.0, 4.0, 2.0]).context("seed cert chain")?;
        let direct_report = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )
        .context("direct report")?;
        ensure!(
            direct_report.monotonicity_passed == Some(true),
            "direct monotonicity should pass"
        );
        db.flush().context("flush db")?;
        db.cancel_all_background_work(true);
        drop(db);

        let runtime = tokio::runtime::Runtime::new().context("runtime")?;
        let tool_result = runtime.block_on(async {
            let (handlers, _handler_tempdir) =
                crate::handlers::tests::create_protocol_test_handlers().await;
            let response = handlers
                .call_mejepa_compression_progress(
                    Some(JsonRpcId::Number(322)),
                    json!({
                        "dbPath": db_path,
                        "window": 4,
                        "epsilonBits": context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS
                    }),
                )
                .await;
            assert!(response.error.is_none());
            response.result.expect("tool result")
        });
        let structured = tool_result["structuredContent"].clone();
        ensure!(
            structured["status"] == "green",
            "MCP status should be green"
        );
        ensure!(
            structured["report"]["rollingMeanCpPhiBits"]
                .as_f64()
                .unwrap_or(0.0)
                > 0.0,
            "MCP rolling mean should be positive"
        );

        let cli_output = context_graph_cli::commands::mejepa_active_learning::compression_progress(
            context_graph_cli::commands::mejepa_active_learning::CompressionProgressArgs {
                db_path: db_path.clone(),
                window: 4,
                epsilon_bits: context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
            },
        )
        .context("CLI compression-progress")?;
        ensure!(
            cli_output.report.rolling_mean_cp_phi_bits == direct_report.rolling_mean_cp_phi_bits,
            "CLI rolling mean should match direct report"
        );

        let weekly_section = render_compression_progress_weekly_section(&direct_report);
        ensure!(
            weekly_section.contains("## Compression Progress")
                && weekly_section.contains("monotonicity_badge: monotone")
                && weekly_section.contains("sparkline:"),
            "weekly section should render compression progress"
        );

        let empty_report = empty_boundary(temp.path())?;
        let single_report = single_boundary(temp.path())?;
        let regression_report = regression_boundary(temp.path())?;
        let missing_bits_error = missing_bits_boundary(temp.path())?;
        let non_finite_error = non_finite_boundary(temp.path())?;

        let fsv_root =
            Path::new("/zfs/archive/contextgraph/fsv/task-ek-001-compression-progress-fsv");
        let run_root = fsv_root.join(format!(
            "run-{}-{}",
            chrono::Utc::now().timestamp_millis(),
            std::process::id()
        ));
        fs::create_dir_all(&run_root).context("create fsv run root")?;
        let weekly_path = run_root.join("weekly-compression-progress.md");
        crate::daemon::write_weekly_eval_markdown(
            &weekly_path,
            &synthetic_eval_report(),
            "task-ek-001-compression-progress-fsv",
            &crate::daemon::WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0)
                .with_compression_progress_section(weekly_section.clone()),
        )
        .context("write weekly markdown")?;
        let weekly_readback = fs::read_to_string(&weekly_path).context("read weekly section")?;
        ensure!(
            weekly_readback.contains(&weekly_section),
            "weekly section missing"
        );

        let fsv = json!({
            "fsvName": "task-ek-001-compression-progress-fsv",
            "issue": 322,
            "all_passed": true,
            "sourceOfTruth": {
                "trainCertCf": CF_MEJEPA_TRAIN_CERTS,
                "syntheticDb": db_path,
                "weeklyMarkdown": weekly_path,
            },
            "directReport": direct_report,
            "mcpStructuredContent": structured,
            "cliReport": cli_output.report,
            "weeklyMarkdown": {
                "path": weekly_path,
                "sha256": sha256_path(&weekly_path)?,
                "containsCompressionProgress": weekly_readback.contains("## Compression Progress"),
                "containsMonotoneBadge": weekly_readback.contains("monotonicity_badge: monotone"),
            },
            "boundaryCases": {
                "emptyWindowReason": empty_report.status_reason,
                "singleWindowMonotonicity": single_report.monotonicity,
                "regressionMonotonicity": regression_report.monotonicity,
                "missingBitsError": missing_bits_error,
                "nonFiniteError": non_finite_error,
            },
        });
        let fsv_path = run_root.join("compression_progress_fsv.json");
        fs::write(&fsv_path, serde_json::to_vec_pretty(&fsv)?).context("write fsv")?;
        let readback: Value = serde_json::from_slice(&fs::read(&fsv_path).context("read fsv")?)?;
        ensure!(readback == fsv, "FSV readback mismatch");
        let fsv_sha256 = sha256_path(&fsv_path)?;
        let mut with_artifact = fsv;
        with_artifact["fsvPath"] = json!(fsv_path);
        with_artifact["fsvSha256"] = json!(fsv_sha256);
        Ok(with_artifact)
    }

    fn seed_cert_chain(db: std::sync::Arc<DB>, bits: &[f64]) -> Result<()> {
        let mut writer = TrainCertWriter::new(
            db,
            CF_MEJEPA_TRAIN_CERTS.to_string(),
            "task-ek-001-fsv".to_string(),
            "synthetic-compression-progress".to_string(),
            HashMap::new(),
            "2026-05-17T00:00:00Z".to_string(),
        )
        .context("train cert writer")?;
        for (step, bits) in bits.iter().copied().enumerate() {
            let mut cert = synthetic_cert(step as u64, bits)?;
            writer.emit(&mut cert).context("emit cert")?;
        }
        Ok(())
    }

    fn synthetic_eval_report() -> EvalReport {
        EvalReport {
            report_date: "2026-05-17-task-ek-001-fsv".to_string(),
            generated_at_unix_ms: 1_779_055_200_000,
            rolling_window_size: 4,
            holdout_count: 4,
            overall_correlation: Some(0.9),
            per_category_correlation: BTreeMap::new(),
            per_language_correlation: BTreeMap::new(),
            per_cell_correlation: BTreeMap::new(),
            cell_exemptions: BTreeMap::new(),
            bayesian_shrinkage: BTreeMap::new(),
            conformal_coverage_health: BTreeMap::new(),
            ood_calibration_health: BTreeMap::new(),
            gtau_pass_rate: BTreeMap::new(),
            per_prediction_class_calibration: BTreeMap::new(),
            per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
                4,
                &context_graph_mejepa::EvalConfig::default(),
            ),
            per_cell_convergence_eta: BTreeMap::new(),
            active_learning: ActiveLearningSummary {
                queued_count: 0,
                evicted_count: 0,
                ood_escalation_count: 0,
            },
            state_transfer_diagnostic: None,
            per_cell_state_transfer: BTreeMap::new(),
            failing_cell_classifications: BTreeMap::new(),
            aux_head_distillation: None,
            regression_checks: Vec::new(),
            open_research_questions: Vec::new(),
            q1_pass_rate: 1.0,
            q2_report_correlation: Some(0.9),
            q3_side_effect_agreement: Some(1.0),
            ship_gate_passed: false,
            ship_gate_failures: vec!["synthetic compression-progress FSV".to_string()],
            provenance: EvalProvenance {
                corpus_sha: "task-ek-001-compression-progress-fsv".to_string(),
                eval_code_version: "task-ek-001-fsv".to_string(),
                calibration_version: "task-ek-001-fsv".to_string(),
                generated_by: "compression_progress_write_fsv_artifact".to_string(),
            },
            wall_clock_seconds: 0.01,
        }
    }

    fn synthetic_cert(step: u64, bits: f64) -> Result<TrainingCertificate> {
        let probability = 2f64.powf(-bits).clamp(0.0, 1.0) as f32;
        let signal = compute_l_step(
            DeltaPComponents {
                delta_p_real: probability,
                per_chunk_values: vec![probability],
                ..DeltaPComponents::default()
            },
            DeltaKComponents::default(),
            DeltaOmegaComponents::default(),
            DeltaXiComponents {
                target_collapse: 0.0,
                predictor_redundancy: 0.01,
                constellation_violation_rate: 0.01,
            },
        )?;
        let delta_xi_global_min = signal.delta_xi;
        let per_head_l_step = HeadId::ALL
            .into_iter()
            .map(|head| (head.as_str().to_string(), signal.l_step))
            .collect();
        Ok(TrainingCertificate {
            step,
            epoch: 0,
            signal,
            per_head_l_step,
            delta_xi_global_min,
            loss_components: HashMap::from([("predict_nll_bits".to_string(), bits as f32)]),
            conditional_description_length_bits: Some(bits),
            inverse_map_quality_bits: None,
            l_entropy_nats: None,
            l_entropy_weighted: None,
            l_entropy_lambda: None,
            l_entropy_sample_count: None,
            l_entropy_estimator: None,
            training_mode: "task-ek-001-fsv".to_string(),
            distillation_cycle_id: None,
            distillation_loss_mean: None,
            distillation_skipped_count: None,
            counterfactual_smoothness: None,
            counterfactual_smoothness_anomaly: None,
            adversarial_mix_target_ratio: 0.20,
            adversarial_mix_count: 0,
            adversarial_mix_example_indices: Vec::new(),
            adversarial_mix_fallback_count: 0,
            cross_task_transfer_indices: Vec::new(),
            cross_task_transfer_fallback_count: 0,
            predictor_redundancy_pairwise_mi: 0.01,
            predictor_redundancy_pairwise_mi_source: "synthetic_pairwise_mi".to_string(),
            holdout_promotion: None,
            generic_only_warning: None,
            phase3_dod_passed: None,
            grad_norm_running_mean: 0.0,
            parent_witness_hash: String::new(),
            self_hash: String::new(),
            merkle_root: String::new(),
            code_version: String::new(),
            corpus_sha: String::new(),
            embedder_versions: HashMap::new(),
            frozen_at: String::new(),
        })
    }

    fn empty_boundary(
        root: &Path,
    ) -> Result<context_graph_mejepa_train::CompressionProgressReport> {
        let db_path = root.join("empty.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open empty db")?;
        let report = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )?;
        ensure!(
            report.status_reason.as_deref() == Some("MEJEPA_NO_CERT_HISTORY"),
            "empty boundary should report no cert history"
        );
        Ok(report)
    }

    fn single_boundary(
        root: &Path,
    ) -> Result<context_graph_mejepa_train::CompressionProgressReport> {
        let db_path = root.join("single.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open single db")?;
        seed_cert_chain(db.clone(), &[3.0])?;
        let report = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )?;
        ensure!(
            report.monotonicity
                == context_graph_mejepa_train::CompressionProgressMonotonicity::Indeterminate,
            "single boundary should be indeterminate"
        );
        Ok(report)
    }

    fn regression_boundary(
        root: &Path,
    ) -> Result<context_graph_mejepa_train::CompressionProgressReport> {
        let db_path = root.join("regression.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open regression db")?;
        seed_cert_chain(db.clone(), &[3.0, 4.0, 5.0])?;
        let report = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )?;
        ensure!(
            report.monotonicity
                == context_graph_mejepa_train::CompressionProgressMonotonicity::Decreasing,
            "regression boundary should be decreasing"
        );
        Ok(report)
    }

    fn missing_bits_boundary(root: &Path) -> Result<String> {
        let db_path = root.join("missing-bits.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open missing bits db")?;
        let cf = db
            .cf_handle(CF_MEJEPA_TRAIN_CERTS)
            .context("missing train cert cf")?;
        let mut cert = synthetic_cert(0, 3.0)?;
        cert.conditional_description_length_bits = None;
        let bytes = serde_json::to_vec(&cert)?;
        db.put_cf(cf, 0u64.to_be_bytes(), bytes)
            .context("write missing bits cert")?;
        let err = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )
        .unwrap_err();
        Ok(err.code().to_string())
    }

    fn non_finite_boundary(root: &Path) -> Result<String> {
        let db_path = root.join("non-finite.rocksdb");
        let db = open_train_rocksdb(&db_path).context("open non finite db")?;
        let cf = db
            .cf_handle(CF_MEJEPA_TRAIN_CERTS)
            .context("missing train cert cf")?;
        let mut cert = synthetic_cert(0, 3.0)?;
        cert.conditional_description_length_bits = Some(-1.0);
        db.put_cf(cf, 0u64.to_be_bytes(), serde_json::to_vec(&cert)?)
            .context("write invalid cert")?;
        let err = compression_progress_report(
            db.as_ref(),
            4,
            context_graph_mejepa_train::DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
        )
        .unwrap_err();
        Ok(err.code().to_string())
    }

    fn sha256_path(path: impl AsRef<Path>) -> Result<String> {
        let bytes = fs::read(path.as_ref())
            .with_context(|| format!("read sha256 path {}", path.as_ref().display()))?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}
