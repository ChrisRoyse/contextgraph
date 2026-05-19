//! ME-JEPA reward-signal completeness MCP audit tool.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{bail, Context, Result as AnyhowResult};
use context_graph_mejepa::{audit_reward_signals, open_reward_signal_audit_rocksdb};
use context_graph_mejepa_cf::CF_MEJEPA_SIGNAL_DROP_LOG;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RewardSignalAuditRequest {
    db_path: Option<PathBuf>,
    #[serde(default = "default_min_coverage")]
    min_coverage: f32,
    #[serde(default = "default_signal_drop_sample_limit")]
    signal_drop_sample_limit: usize,
}

impl Handlers {
    pub(crate) async fn call_mejepa_audit_reward_signals(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_AUDIT_REWARD_SIGNALS) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_reward_signal_audit(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.tool_error_structured(
                id,
                ToolErrorKind::Storage,
                "MEJEPA_REWARD_SIGNAL_AUDIT_FAILED",
                &err.to_string(),
                json!({"toolFamily": "mejepa_reward_signal_audit"}),
            ),
        }
    }
}

fn run_reward_signal_audit(request: RewardSignalAuditRequest) -> AnyhowResult<Value> {
    let db_path = resolve_infer_db_path(request.db_path)?;
    let db = open_reward_signal_audit_rocksdb(&db_path).context("open reward audit RocksDB")?;
    let report = audit_reward_signals(
        db.as_ref(),
        request.min_coverage,
        request.signal_drop_sample_limit,
    )
    .context("audit reward signals")?;
    let incomplete_tiers = report
        .per_tier
        .iter()
        .filter(|tier| !tier.passed)
        .map(|tier| tier.tier)
        .collect::<Vec<_>>();
    let missing_cfs = report
        .per_tier
        .iter()
        .flat_map(|tier| tier.signals.iter())
        .flat_map(|signal| signal.missing_cfs.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Ok(json!({
        "tool": tool_names::MEJEPA_AUDIT_REWARD_SIGNALS,
        "status": if report.acceptance_passed { "green" } else { "blocked" },
        "acceptancePassed": report.acceptance_passed,
        "incompleteTiers": incomplete_tiers,
        "missingCfs": missing_cfs,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "signalDropCf": CF_MEJEPA_SIGNAL_DROP_LOG
        },
        "report": report
    }))
}

fn default_min_coverage() -> f32 {
    0.95
}

fn default_signal_drop_sample_limit() -> usize {
    10
}

fn parse_tool_request<T: DeserializeOwned>(
    args: serde_json::Value,
    tool_name: &str,
) -> Result<T, String> {
    serde_json::from_value(args)
        .map_err(|err| format!("{tool_name} schema validation failed: {err}"))
}

fn resolve_infer_db_path(input: Option<PathBuf>) -> AnyhowResult<PathBuf> {
    match input {
        Some(path) if !path.as_os_str().is_empty() => Ok(path),
        Some(_) => bail!("dbPath must be a non-empty path"),
        None => {
            let raw = std::env::var(ENV_INFER_DB)
                .with_context(|| format!("dbPath or {ENV_INFER_DB} is required"))?;
            if raw.trim().is_empty() {
                bail!("{ENV_INFER_DB} must not be empty");
            }
            Ok(PathBuf::from(raw))
        }
    }
}

#[cfg(test)]
pub(in crate::handlers::tools) fn run_reward_signal_audit_write_fsv_artifact() {
    test_support::reward_signal_audit_write_fsv_artifact();
}

#[cfg(test)]
mod test_support {
    use super::*;
    use context_graph_mejepa::{
        audit_reward_signals, persist_signal_drop_log_entry, reward_signal_definitions,
        SignalDropLogEntry, SignalDropSeverity,
    };
    use rocksdb::{Options, WriteOptions, DB};
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::TempDir;

    pub(super) fn reward_signal_audit_write_fsv_artifact() {
        let temp = TempDir::new().expect("tempdir");
        let db_path = temp.path().join("reward-audit.rocksdb");
        let db = open_reward_signal_audit_rocksdb(&db_path).expect("open audit db");
        let seeded_cfs = seed_all_reward_signal_cfs(db.as_ref()).expect("seed reward signal cfs");
        let before_report =
            audit_reward_signals(db.as_ref(), 0.95, 0).expect("before signal-drop report");
        let before_signal_drop_rows = before_report.signal_drop_log.row_count;

        let entry = SignalDropLogEntry::new(
            1_779_056_000_000,
            4,
            "static_analysis_mypy_pyright_ruff",
            "static_analysis",
            "task-rwd-315-row-1",
            "STATIC_ANALYSIS_RUNTIME_EXCEEDED",
            "synthetic analyzer runner timeout",
            SignalDropSeverity::Error,
            "quarantine row and inspect toolchain",
            None,
            BTreeMap::from([
                ("task_id".to_string(), "TASK-RWD-315".to_string()),
                ("fsv_case".to_string(), "signal_drop_readback".to_string()),
            ]),
        )
        .expect("build signal-drop entry");
        persist_signal_drop_log_entry(db.as_ref(), &entry).expect("persist signal-drop entry");
        let after_report =
            audit_reward_signals(db.as_ref(), 0.95, 10).expect("after signal-drop report");

        let invalid_min_coverage = audit_reward_signals(db.as_ref(), 1.01, 0).is_err();
        let invalid_sample_limit = audit_reward_signals(db.as_ref(), 0.95, 1001).is_err();
        let invalid_signal_drop_entry = {
            let mut bad = entry.clone();
            bad.fail_closed = false;
            persist_signal_drop_log_entry(db.as_ref(), &bad).is_err()
        };

        flush_seeded_cfs(db.as_ref(), &seeded_cfs).expect("flush seeded cfs");
        db.cancel_all_background_work(true);
        drop(db);
        std::thread::sleep(std::time::Duration::from_millis(100));

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let tool_result = runtime.block_on(async {
            let (handlers, _handler_tempdir) =
                crate::handlers::tests::create_protocol_test_handlers().await;
            let response = handlers
                .call_mejepa_audit_reward_signals(
                    Some(JsonRpcId::Number(315)),
                    json!({
                        "dbPath": db_path,
                        "minCoverage": 0.95,
                        "signalDropSampleLimit": 5
                    }),
                )
                .await;
            assert!(response.error.is_none());
            response.result.expect("tool result")
        });
        let structured = tool_result["structuredContent"].clone();

        let missing_cf_report = missing_cf_boundary(temp.path());
        let corrupt_signal_drop_failed = corrupt_signal_drop_boundary(temp.path());
        let reopened = open_reward_signal_audit_rocksdb(&db_path).expect("reopen audit db");
        let reopened_report =
            audit_reward_signals(reopened.as_ref(), 0.95, 10).expect("reopened audit report");

        let boundary_cases = vec![
            boundary_case("invalid_min_coverage_rejected", invalid_min_coverage),
            boundary_case("invalid_sample_limit_rejected", invalid_sample_limit),
            boundary_case(
                "non_fail_closed_signal_drop_rejected",
                invalid_signal_drop_entry,
            ),
            boundary_case(
                "missing_cfs_block_acceptance",
                !missing_cf_report.acceptance_passed
                    && !missing_cf_report.signal_drop_log.registered
                    && missing_cf_report.captured_signal_count == 0,
            ),
            boundary_case(
                "corrupt_signal_drop_fails_closed",
                corrupt_signal_drop_failed,
            ),
        ];
        let all_passed = structured["acceptancePassed"] == json!(true)
            && structured["status"] == json!("green")
            && structured["report"]["overallCoverageRatio"] == json!(1.0)
            && structured["report"]["fingerprintFeatureSpan"]["spansAllEightTiers"] == json!(true)
            && structured["report"]["signalDropLog"]["registered"] == json!(true)
            && structured["report"]["signalDropLog"]["rowCount"] == json!(1)
            && before_signal_drop_rows == 0
            && after_report.signal_drop_log.row_count == 1
            && reopened_report.acceptance_passed
            && reopened_report.signal_drop_log.samples.len() == 1
            && boundary_cases
                .iter()
                .all(|case| case["passed"] == json!(true));

        let artifact = json!({
            "task_id": "TASK-RWD-315",
            "issue": 315,
            "tool": tool_names::MEJEPA_AUDIT_REWARD_SIGNALS,
            "source_of_truth": {
                "db_path": db_path.display().to_string(),
                "signal_drop_cf": CF_MEJEPA_SIGNAL_DROP_LOG,
                "seeded_cfs": seeded_cfs
            },
            "trigger": "cargo test -p context-graph-mcp reward_signal_audit_writes_fsv_artifact -- --nocapture",
            "before_signal_drop_rows": before_signal_drop_rows,
            "after_signal_drop_report": after_report,
            "mcp_tool_result": structured,
            "reopened_readback": {
                "acceptance_passed": reopened_report.acceptance_passed,
                "overall_coverage_ratio": reopened_report.overall_coverage_ratio,
                "signal_drop_rows": reopened_report.signal_drop_log.row_count,
                "signal_drop_event_ids": reopened_report.signal_drop_log.samples.iter().map(|entry| entry.event_id_hex()).collect::<Vec<_>>()
            },
            "boundary_cases": boundary_cases,
            "all_passed": all_passed
        });
        let run_root = PathBuf::from(format!(
            "/zfs/archive/contextgraph/fsv/task-rwd-315-reward-audit-fsv/run-{}-{}",
            chrono::Utc::now().timestamp_millis(),
            std::process::id()
        ));
        std::fs::create_dir_all(&run_root).expect("create fsv dir");
        let output = run_root.join("reward_signal_audit_fsv.json");
        std::fs::write(&output, serde_json::to_vec_pretty(&artifact).expect("json"))
            .expect("write fsv");
        assert!(all_passed, "FSV artifact: {}", output.display());
    }

    fn seed_all_reward_signal_cfs(db: &DB) -> AnyhowResult<Vec<String>> {
        let mut cfs = reward_signal_definitions()
            .into_iter()
            .flat_map(|signal| signal.required_cfs)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        cfs.sort();
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        for (idx, cf_name) in cfs.iter().enumerate() {
            let cf = db
                .cf_handle(cf_name)
                .with_context(|| format!("missing seeded cf {cf_name}"))?;
            let key = format!("reward-audit-seed::{idx:04}::{cf_name}");
            let value = format!("task-rwd-315-fsv::{cf_name}");
            db.put_cf_opt(cf, key.as_bytes(), value.as_bytes(), &opts)?;
        }
        Ok(cfs)
    }

    fn flush_seeded_cfs(db: &DB, cfs: &[String]) -> AnyhowResult<()> {
        for cf_name in cfs {
            if let Some(cf) = db.cf_handle(cf_name) {
                db.flush_cf(cf)?;
            }
        }
        if let Some(cf) = db.cf_handle(CF_MEJEPA_SIGNAL_DROP_LOG) {
            db.flush_cf(cf)?;
        }
        Ok(())
    }

    fn missing_cf_boundary(root: &Path) -> context_graph_mejepa::RewardSignalAuditReport {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, root.join("missing-cfs.rocksdb")).expect("open missing cf db");
        audit_reward_signals(&db, 0.95, 0).expect("missing cf audit")
    }

    fn corrupt_signal_drop_boundary(root: &Path) -> bool {
        let db_path = root.join("corrupt-signal-drop.rocksdb");
        let db: Arc<DB> = open_reward_signal_audit_rocksdb(&db_path).expect("open corrupt db");
        let cf = db
            .cf_handle(CF_MEJEPA_SIGNAL_DROP_LOG)
            .expect("signal drop cf");
        db.put_cf(cf, b"corrupt", b"not-bincode")
            .expect("write corrupt row");
        audit_reward_signals(db.as_ref(), 0.95, 1).is_err()
    }

    fn boundary_case(name: &str, passed: bool) -> Value {
        json!({
            "case": name,
            "passed": passed
        })
    }
}
