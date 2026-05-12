//! `context-graph export dynamicjepa-episodes --bundle <dir> --out episodes.jsonl`
//!
//! Converts a passed DynamicJEPA v2 bundle into strict JSON Lines training
//! episodes. The exporter is fail-closed: it opens existing RocksDB sources
//! without creating missing databases or column families, validates every
//! linked plan/prediction/guard/verification row through independent reads,
//! writes through temporary files, and re-reads the final JSONL artifact before
//! reporting success.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use context_graph_core::dynamicjepa::{VerificationRunRecord, VerificationStatus};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_GUARD_DECISIONS, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS, CF_DJ_VERIFICATION_RUNS,
};
use context_graph_storage::dynamicjepa::{
    count_plan_traces, count_verification_runs, get_guard_decision, get_plan_trace, get_prediction,
    list_plan_traces, list_verification_runs, snapshot_dj_counts,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;

const EPISODE_SCHEMA_VERSION: u32 = 1;
const REQUIRED_BUNDLE_SCHEMA_VERSION: u64 = 2;

/// CLI arguments for `export dynamicjepa-episodes`.
#[derive(Args, Debug, Clone)]
pub struct ExportDynamicJepaEpisodesArgs {
    /// Export target. Currently only `dynamicjepa-episodes`.
    #[arg(long, default_value = "dynamicjepa-episodes", value_parser = ["dynamicjepa-episodes"])]
    pub kind: String,

    /// Output format. Currently only JSON Lines.
    #[arg(long, default_value = "jsonl", value_parser = ["jsonl"])]
    pub format: String,

    /// DynamicJEPA bundle root containing run_manifest.json, fsv_report.json,
    /// db_counts_after.json, edge_case_results.json, and dbs/.
    #[arg(long)]
    pub bundle: PathBuf,

    /// Output JSONL file path. Parent directory must already exist.
    #[arg(long)]
    pub out: PathBuf,

    /// Optional exporter manifest path. Defaults next to --out.
    #[arg(long)]
    pub manifest_out: Option<PathBuf>,

    /// Override run_manifest.db_path. The path must already exist.
    #[arg(long)]
    pub db_root: Option<PathBuf>,

    /// Restrict export to one or more domains from db_counts_after.json.
    #[arg(long = "domain")]
    pub domains: Vec<String>,

    /// Do not include edge-case verification episodes from edge_case_results.json.
    #[arg(long)]
    pub no_edge_cases: bool,

    /// Replace existing --out / --manifest-out files.
    #[arg(long)]
    pub overwrite: bool,

    /// Permit a zero-row export. Default refuses empty corpora.
    #[arg(long)]
    pub allow_empty: bool,
}

/// Summary of a successful DynamicJEPA episode export.
#[derive(Debug, Clone, Serialize)]
pub struct ExportDynamicJepaEpisodesSummary {
    pub status: &'static str,
    pub output: String,
    pub manifest: String,
    pub total_records: usize,
    pub plan_episode_count: usize,
    pub edge_case_episode_count: usize,
    pub domains: Vec<DomainExportSummary>,
    pub output_sha256: String,
    pub output_bytes: u64,
    pub readback: JsonlReadback,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainExportSummary {
    pub domain: String,
    pub db_path: String,
    pub plan_episode_count: usize,
    pub verification_run_id: String,
    pub verification_status: String,
    pub db_counts_before: BTreeMap<String, u64>,
    pub db_counts_after: BTreeMap<String, u64>,
    pub manifest_count_drift: BTreeMap<String, CountDrift>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountDrift {
    pub manifest: u64,
    pub actual_db: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonlReadback {
    pub line_count: usize,
    pub plan_episode_count: usize,
    pub edge_case_episode_count: usize,
    pub first_episode_id: Option<String>,
    pub last_episode_id: Option<String>,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
struct EpisodeRow {
    schema_version: u32,
    record_kind: &'static str,
    episode_id: String,
    bundle: Value,
    source_of_truth: Value,
    state: Value,
    action: Value,
    expected_persisted_delta: Value,
    actual_readback: Value,
    verifier_status: Value,
    error_code: Option<String>,
    remediation: Option<String>,
}

struct BundleContext {
    bundle_root: PathBuf,
    run_manifest_path: PathBuf,
    fsv_report_path: PathBuf,
    db_counts_after_path: PathBuf,
    edge_case_results_path: PathBuf,
    run_manifest: Value,
    fsv_report: Value,
    db_counts_after: Value,
    edge_case_results: Vec<Value>,
    run_id: String,
    bundle_schema_version: u64,
    db_root: PathBuf,
    source_hashes: BTreeMap<String, String>,
}

struct DomainRows {
    summary: DomainExportSummary,
    rows: Vec<EpisodeRow>,
}

/// Execute the DynamicJEPA bundle-to-episodes export.
pub async fn run(args: ExportDynamicJepaEpisodesArgs) -> Result<ExportDynamicJepaEpisodesSummary> {
    validate_args(&args)?;
    let start = Instant::now();
    let manifest_out = derive_manifest_path(&args.out, args.manifest_out.clone())?;
    validate_output_paths(&args.out, &manifest_out, args.overwrite)?;

    let context = load_bundle_context(&args)?;
    let selected_domains = selected_domains(&context, &args.domains)?;
    let bundle_ref = bundle_ref(&context);

    let mut domain_summaries = Vec::new();
    let mut rows = Vec::new();
    for domain in selected_domains {
        let domain_rows = build_domain_rows(&context, &bundle_ref, &domain)?;
        rows.extend(domain_rows.rows);
        domain_summaries.push(domain_rows.summary);
    }

    let mut edge_case_episode_count = 0usize;
    if !args.no_edge_cases {
        let edge_rows = build_edge_case_rows(&context, &bundle_ref)?;
        edge_case_episode_count = edge_rows.len();
        rows.extend(edge_rows);
    }

    if rows.is_empty() && !args.allow_empty {
        bail!(
            "dynamicjepa-episodes export produced zero rows; refusing to create an empty corpus \
             without --allow-empty"
        );
    }

    let tmp_out = temp_path(&args.out);
    let tmp_manifest = temp_path(&manifest_out);
    cleanup_temp(&tmp_out);
    cleanup_temp(&tmp_manifest);

    if let Err(err) = write_jsonl(&tmp_out, &rows)
        .with_context(|| format!("failed to write JSONL temp file {}", tmp_out.display()))
    {
        cleanup_temp(&tmp_out);
        return Err(err);
    }
    let tmp_readback = match readback_jsonl(&tmp_out)
        .with_context(|| format!("failed to read back JSONL temp file {}", tmp_out.display()))
    {
        Ok(readback) => readback,
        Err(err) => {
            cleanup_temp(&tmp_out);
            return Err(err);
        }
    };
    if tmp_readback.line_count != rows.len() {
        cleanup_temp(&tmp_out);
        bail!(
            "JSONL temp readback count mismatch: expected {} rows, got {}",
            rows.len(),
            tmp_readback.line_count
        );
    }

    promote_temp(&tmp_out, &args.out)
        .with_context(|| format!("failed to promote {}", args.out.display()))?;
    let final_readback = readback_jsonl(&args.out)
        .with_context(|| format!("failed to read back final JSONL {}", args.out.display()))?;
    if final_readback.sha256 != tmp_readback.sha256 {
        bail!(
            "final JSONL hash changed after promotion: temp={} final={}",
            tmp_readback.sha256,
            final_readback.sha256
        );
    }

    let output_bytes = fs::metadata(&args.out)
        .with_context(|| format!("failed to stat {}", args.out.display()))?
        .len();
    let plan_episode_count = rows
        .iter()
        .filter(|row| row.record_kind == "dynamicjepa_plan_episode")
        .count();
    let summary = ExportDynamicJepaEpisodesSummary {
        status: "ok",
        output: args.out.display().to_string(),
        manifest: manifest_out.display().to_string(),
        total_records: rows.len(),
        plan_episode_count,
        edge_case_episode_count,
        domains: domain_summaries,
        output_sha256: final_readback.sha256.clone(),
        output_bytes,
        readback: final_readback,
        elapsed_ms: start.elapsed().as_millis() as u64,
    };

    if let Err(err) = write_export_manifest(&tmp_manifest, &context, &summary) {
        cleanup_temp(&tmp_manifest);
        return Err(err);
    }
    let manifest_json = match read_required_json(&tmp_manifest, "export manifest temp") {
        Ok(value) => value,
        Err(err) => {
            cleanup_temp(&tmp_manifest);
            return Err(err);
        }
    };
    if manifest_json["status"] != "ok" {
        cleanup_temp(&tmp_manifest);
        bail!("export manifest temp readback did not preserve status=ok");
    }
    promote_temp(&tmp_manifest, &manifest_out)
        .with_context(|| format!("failed to promote {}", manifest_out.display()))?;
    let final_manifest = read_required_json(&manifest_out, "export manifest final")?;
    if final_manifest["output"]["sha256"] != summary.output_sha256 {
        bail!("final export manifest SHA-256 does not match JSONL readback");
    }

    info!(
        out = %args.out.display(),
        manifest = %manifest_out.display(),
        total_records = summary.total_records,
        output_sha256 = %summary.output_sha256,
        "DynamicJEPA episode export finished"
    );
    Ok(summary)
}

pub fn summary_to_exit_code(result: Result<ExportDynamicJepaEpisodesSummary>) -> i32 {
    match result {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::to_string(&summary)
                    .expect("ExportDynamicJepaEpisodesSummary must serialize")
            );
            0
        }
        Err(err) => {
            tracing::error!(error = %err, "export dynamicjepa-episodes: FAILED");
            let value = json!({
                "status": "error",
                "operation": "export_dynamicjepa_episodes",
                "error_code": "DYNAMICJEPA_EPISODE_EXPORT_FAILED",
                "error_message": format!("{err:#}"),
                "remediation": "read the error context, inspect the named bundle file or RocksDB CF, fix the source-of-truth mismatch, and rerun the export",
            });
            println!("{value}");
            1
        }
    }
}

fn validate_args(args: &ExportDynamicJepaEpisodesArgs) -> Result<()> {
    if args.kind != "dynamicjepa-episodes" {
        bail!("unsupported export kind: {}", args.kind);
    }
    if args.format != "jsonl" {
        bail!("unsupported dynamicjepa episode format: {}", args.format);
    }
    Ok(())
}

fn validate_output_paths(out: &Path, manifest_out: &Path, overwrite: bool) -> Result<()> {
    for (label, path) in [("--out", out), ("--manifest-out", manifest_out)] {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("{label} path has no parent: {}", path.display()))?;
        if !parent.as_os_str().is_empty() && !parent.exists() {
            bail!(
                "{label} parent directory does not exist: {}",
                parent.display()
            );
        }
        if path.exists() && !overwrite {
            bail!(
                "{label} already exists: {} (use --overwrite to replace it)",
                path.display()
            );
        }
    }
    if out == manifest_out {
        bail!("--out and --manifest-out must be different files");
    }
    Ok(())
}

fn load_bundle_context(args: &ExportDynamicJepaEpisodesArgs) -> Result<BundleContext> {
    let bundle_root = canonical_existing_dir(&args.bundle, "--bundle")?;
    let run_manifest_path = bundle_root.join("run_manifest.json");
    let fsv_report_path = bundle_root.join("fsv_report.json");
    let db_counts_after_path = bundle_root.join("db_counts_after.json");
    let edge_case_results_path = bundle_root.join("edge_case_results.json");

    let run_manifest = read_required_json(&run_manifest_path, "run_manifest.json")?;
    let fsv_report = read_required_json(&fsv_report_path, "fsv_report.json")?;
    let db_counts_after = read_required_json(&db_counts_after_path, "db_counts_after.json")?;
    let edge_case_results_value =
        read_required_json(&edge_case_results_path, "edge_case_results.json")?;
    let edge_case_results = edge_case_results_value
        .as_array()
        .ok_or_else(|| anyhow!("edge_case_results.json must be a JSON array"))?
        .clone();

    let bundle_schema_version = run_manifest
        .get("bundle_schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("run_manifest.json missing numeric bundle_schema_version"))?;
    if bundle_schema_version != REQUIRED_BUNDLE_SCHEMA_VERSION {
        bail!(
            "bundle schema version mismatch: expected {} found {} manifest={}",
            REQUIRED_BUNDLE_SCHEMA_VERSION,
            bundle_schema_version,
            run_manifest_path.display()
        );
    }
    let status = required_str(&run_manifest, "status", "run_manifest.json")?;
    if status != "passed" {
        bail!("run_manifest.json status must be passed, got {status:?}");
    }
    let fsv_status = required_str(&fsv_report, "status", "fsv_report.json")?;
    if fsv_status != "passed" {
        bail!("fsv_report.json status must be passed, got {fsv_status:?}");
    }
    if !db_counts_after.is_object() {
        bail!("db_counts_after.json must be an object keyed by domain");
    }

    let run_id = required_str(&run_manifest, "run_id", "run_manifest.json")?.to_string();
    let db_root = match &args.db_root {
        Some(path) => canonical_existing_dir(path, "--db-root")?,
        None => {
            let raw_db_path = required_str(&run_manifest, "db_path", "run_manifest.json")?;
            resolve_existing_path(&bundle_root, raw_db_path, "run_manifest.db_path")?
        }
    };

    let mut source_hashes = BTreeMap::new();
    for path in [
        &run_manifest_path,
        &fsv_report_path,
        &db_counts_after_path,
        &edge_case_results_path,
    ] {
        source_hashes.insert(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_string(),
            sha256_file(path)?,
        );
    }

    Ok(BundleContext {
        bundle_root,
        run_manifest_path,
        fsv_report_path,
        db_counts_after_path,
        edge_case_results_path,
        run_manifest,
        fsv_report,
        db_counts_after,
        edge_case_results,
        run_id,
        bundle_schema_version,
        db_root,
        source_hashes,
    })
}

fn selected_domains(context: &BundleContext, requested: &[String]) -> Result<Vec<String>> {
    let counts_obj = context
        .db_counts_after
        .as_object()
        .ok_or_else(|| anyhow!("db_counts_after.json must be an object keyed by domain"))?;
    let mut domains = if requested.is_empty() {
        counts_obj.keys().cloned().collect::<Vec<_>>()
    } else {
        requested.to_vec()
    };
    domains.sort();
    domains.dedup();
    for domain in &domains {
        if !counts_obj.contains_key(domain) {
            bail!("requested domain {domain:?} is absent from db_counts_after.json");
        }
    }
    Ok(domains)
}

fn build_domain_rows(context: &BundleContext, bundle: &Value, domain: &str) -> Result<DomainRows> {
    let db_path = domain_db_path(context, domain)?;
    let store = open_existing_db(&db_path)?;
    let db = store.dynamicjepa_db();
    let db_counts_before = snapshot_dj_counts(db)
        .with_context(|| format!("snapshot DynamicJEPA counts before export for {domain}"))?;

    let plan_count = count_plan_traces(db)
        .with_context(|| format!("count plan traces for domain {domain}"))?
        as usize;
    if plan_count == 0 {
        bail!(
            "domain {domain} has zero persisted plan traces in {}",
            db_path.display()
        );
    }
    let verification_count = count_verification_runs(db)
        .with_context(|| format!("count verification runs for domain {domain}"))?
        as usize;
    if verification_count == 0 {
        bail!(
            "domain {domain} has no verification runs in {}; refusing unverified corpus",
            db_path.display()
        );
    }
    let verification_runs = list_verification_runs(db, verification_count, 0)
        .with_context(|| format!("list verification runs for domain {domain}"))?;
    let latest_verification = latest_passed_verification(domain, &verification_runs)?;

    let plans =
        list_plan_traces(db, plan_count, 0).with_context(|| format!("list plans for {domain}"))?;
    let mut rows = Vec::with_capacity(plans.len());
    for plan in plans {
        rows.push(plan_episode_row(
            db,
            bundle,
            domain,
            &db_path,
            &plan,
            latest_verification,
            &db_counts_before,
        )?);
    }

    let db_counts_after = snapshot_dj_counts(db)
        .with_context(|| format!("snapshot DynamicJEPA counts after export for {domain}"))?;
    if db_counts_after != db_counts_before {
        bail!(
            "read-only export mutated source DB counts for domain {domain}; before={:?} after={:?}",
            db_counts_before,
            db_counts_after
        );
    }

    let manifest_count_drift = manifest_count_drift(context, domain, &db_counts_after)
        .with_context(|| {
            format!("compare manifest db_counts_after with physical DB for domain {domain}")
        })?;
    Ok(DomainRows {
        summary: DomainExportSummary {
            domain: domain.to_string(),
            db_path: db_path.display().to_string(),
            plan_episode_count: rows.len(),
            verification_run_id: latest_verification.verification_run_id.to_string(),
            verification_status: "Passed".to_string(),
            db_counts_before,
            db_counts_after,
            manifest_count_drift,
        },
        rows,
    })
}

fn plan_episode_row(
    db: &rocksdb::DB,
    bundle: &Value,
    domain: &str,
    db_path: &Path,
    plan: &context_graph_core::dynamicjepa::PlanTraceRecord,
    verification: &VerificationRunRecord,
    counts: &BTreeMap<String, u64>,
) -> Result<EpisodeRow> {
    let readback_plan = get_plan_trace(db, plan.plan_trace_id)
        .with_context(|| format!("read back plan {}", plan.plan_trace_id))?
        .ok_or_else(|| anyhow!("plan {} disappeared during export", plan.plan_trace_id))?;
    if readback_plan != *plan {
        bail!(
            "plan {} changed between list and get readback",
            plan.plan_trace_id
        );
    }

    let mut predictions = Vec::new();
    for (idx, prediction_id) in plan.prediction_ids.iter().enumerate() {
        let prediction = get_prediction(db, *prediction_id)
            .with_context(|| format!("read back prediction {prediction_id}"))?
            .ok_or_else(|| {
                anyhow!(
                    "plan {} references missing prediction {prediction_id}",
                    plan.plan_trace_id
                )
            })?;
        let expected_action = plan.candidate_action_ids.get(idx).ok_or_else(|| {
            anyhow!(
                "plan {} missing candidate action at index {idx}",
                plan.plan_trace_id
            )
        })?;
        if prediction.candidate_action_id != *expected_action {
            bail!(
                "plan {} prediction {} candidate action mismatch: expected {} actual {}",
                plan.plan_trace_id,
                prediction_id,
                expected_action,
                prediction.candidate_action_id
            );
        }
        if prediction.input_panel_id != plan.current_panel_id {
            bail!(
                "plan {} prediction {} input panel mismatch: expected {} actual {}",
                plan.plan_trace_id,
                prediction_id,
                plan.current_panel_id,
                prediction.input_panel_id
            );
        }
        if prediction.model_artifact_hash_at_inference != plan.model_artifact_hash_at_plan {
            bail!(
                "plan {} prediction {} model artifact hash drift",
                plan.plan_trace_id,
                prediction_id
            );
        }
        predictions.push(prediction);
    }

    let mut guards = Vec::new();
    for guard_id in &plan.guard_decision_ids {
        let guard = get_guard_decision(db, (*guard_id).into())
            .with_context(|| format!("read back guard decision {guard_id}"))?
            .ok_or_else(|| {
                anyhow!(
                    "plan {} references missing guard {guard_id}",
                    plan.plan_trace_id
                )
            })?;
        if guard.plan_trace_id != plan.plan_trace_id.0 {
            bail!(
                "guard {} points to plan {} but episode plan is {}",
                guard_id,
                guard.plan_trace_id,
                plan.plan_trace_id
            );
        }
        if !plan
            .candidate_action_ids
            .contains(&guard.candidate_action_id)
        {
            bail!(
                "guard {} candidate action {} is absent from plan {}",
                guard_id,
                guard.candidate_action_id,
                plan.plan_trace_id
            );
        }
        guards.push(guard);
    }

    let prediction_values = predictions
        .iter()
        .map(prediction_json)
        .collect::<Result<Vec<_>>>()?;
    let guard_values = guards
        .iter()
        .map(|guard| to_value(guard, "GuardDecisionRecord"))
        .collect::<Result<Vec<_>>>()?;

    let error_code = plan_status_error_code(plan);
    let remediation = error_code
        .as_ref()
        .map(|_| "inspect PlanTraceRecord.status and the linked guard decisions before training on this failed episode".to_string());

    Ok(EpisodeRow {
        schema_version: EPISODE_SCHEMA_VERSION,
        record_kind: "dynamicjepa_plan_episode",
        episode_id: format!("{domain}:plan:{}", plan.plan_trace_id),
        bundle: bundle.clone(),
        source_of_truth: json!({
            "db_path": db_path,
            "column_families": [
                CF_DJ_PLAN_TRACES,
                CF_DJ_PREDICTIONS,
                CF_DJ_GUARD_DECISIONS,
                CF_DJ_VERIFICATION_RUNS
            ],
            "plan_trace_key_hex": hex(plan.plan_trace_id.into_bytes()),
            "prediction_key_hexes": plan.prediction_ids.iter().map(|id| hex(id.into_bytes())).collect::<Vec<_>>(),
            "guard_decision_key_hexes": plan.guard_decision_ids.iter().map(|id| hex(id.as_bytes())).collect::<Vec<_>>(),
        }),
        state: json!({
            "domain_pack_id": plan.domain_pack_id,
            "current_panel_id": plan.current_panel_id,
            "model_artifact_id": plan.model_artifact_id,
            "model_artifact_hash_at_plan": hex(plan.model_artifact_hash_at_plan),
            "skill_policy_id": plan.skill_policy_id,
            "created_at_unix_ms": plan.created_at_unix_ms,
        }),
        action: json!({
            "candidate_action_ids": plan.candidate_action_ids,
            "prediction_ids": plan.prediction_ids,
            "guard_decision_ids": plan.guard_decision_ids,
            "utility_scores": plan.utility_scores,
            "selected_action_id": plan.selected_action_id,
            "no_accepted_candidate": plan.no_accepted_candidate,
            "constellation_uuid_used": plan.constellation_uuid_used,
            "status": plan.status,
        }),
        expected_persisted_delta: json!({
            "dj_plan_traces": 1,
            "dj_predictions": plan.prediction_ids.len(),
            "dj_guard_decisions": plan.guard_decision_ids.len(),
        }),
        actual_readback: json!({
            "plan_trace_present": true,
            "plan_trace": to_value(plan, "PlanTraceRecord")?,
            "linked_prediction_count": predictions.len(),
            "linked_guard_decision_count": guards.len(),
            "predictions": prediction_values,
            "guard_decisions": guard_values,
            "db_counts": counts,
        }),
        verifier_status: json!({
            "verification_run_id": verification.verification_run_id,
            "verification_status": "Passed",
            "verification_created_at_unix_ms": verification.created_at_unix_ms,
            "verification_test_name": verification.test_name,
        }),
        error_code,
        remediation,
    })
}

fn latest_passed_verification<'a>(
    domain: &str,
    runs: &'a [VerificationRunRecord],
) -> Result<&'a VerificationRunRecord> {
    let latest = runs
        .iter()
        .max_by_key(|run| run.created_at_unix_ms)
        .ok_or_else(|| anyhow!("domain {domain} has no verification runs"))?;
    match &latest.status {
        VerificationStatus::Passed => Ok(latest),
        VerificationStatus::Failed { failure_details } => bail!(
            "latest verification run for domain {domain} failed: run={} details={failure_details}",
            latest.verification_run_id
        ),
    }
}

fn build_edge_case_rows(context: &BundleContext, bundle: &Value) -> Result<Vec<EpisodeRow>> {
    let mut rows = Vec::with_capacity(context.edge_case_results.len());
    for edge in &context.edge_case_results {
        let domain = required_str(edge, "domain", "edge_case_results[]")?;
        let edge_index = edge
            .get("edge_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("edge_case_results[] missing numeric edge_index"))?;
        let edge_obj = edge.get("edge").and_then(Value::as_object).ok_or_else(|| {
            anyhow!("edge_case_results[{domain}/{edge_index}] missing edge object")
        })?;
        let edge_value = Value::Object(edge_obj.clone());
        let name = required_str(&edge_value, "name", "edge_case_results[].edge")?;
        let fixture = required_str(&edge_value, "fixture", "edge_case_results[].edge")?;
        let error = edge_value
            .get("error")
            .ok_or_else(|| anyhow!("edge case {domain}/{name} missing error readback"))?;
        let error_code = required_str(error, "error_code", "edge_case_results[].edge.error")?;
        let error_status = required_str(error, "status", "edge_case_results[].edge.error")?;
        if error_status != "error" {
            bail!("edge case {domain}/{name} expected error status but got {error_status:?}");
        }

        let expected_delta = edge_value
            .get("expected_delta")
            .ok_or_else(|| anyhow!("edge case {domain}/{name} missing expected_delta"))?
            .clone();
        verify_edge_delta(domain, name, &edge_value, &expected_delta)?;

        let evidence_file = required_str(edge, "evidence_file", "edge_case_results[]")?;
        let evidence_path = PathBuf::from(evidence_file);
        if !evidence_path.exists() {
            bail!(
                "edge case evidence file does not exist: {}",
                evidence_path.display()
            );
        }
        let evidence_sha256 = sha256_file(&evidence_path)?;

        rows.push(EpisodeRow {
            schema_version: EPISODE_SCHEMA_VERSION,
            record_kind: "dynamicjepa_edge_case_episode",
            episode_id: format!("{domain}:edge:{edge_index}:{name}"),
            bundle: bundle.clone(),
            source_of_truth: json!({
                "edge_case_results_file": context.edge_case_results_path,
                "evidence_file": evidence_path,
                "evidence_sha256": evidence_sha256,
                "source_of_truth": error.get("source_of_truth").cloned().unwrap_or(Value::Null),
            }),
            state: json!({
                "domain": domain,
                "edge_index": edge_index,
                "edge_name": name,
                "fixture": fixture,
                "before_counts": edge_value.get("before_counts").cloned().unwrap_or(Value::Null),
            }),
            action: json!({
                "operation": error.get("operation").cloned().unwrap_or(Value::Null),
                "fixture": fixture,
                "trigger_event_id": error.get("event_id").cloned().unwrap_or(Value::Null),
            }),
            expected_persisted_delta: expected_delta,
            actual_readback: json!({
                "after_counts": edge_value.get("after_counts").cloned().unwrap_or(Value::Null),
                "count_delta": error.get("count_delta").cloned().unwrap_or(Value::Null),
                "created_ids": error.get("created_ids").cloned().unwrap_or(Value::Null),
                "decoded_records": error.get("decoded_records").cloned().unwrap_or(Value::Null),
                "error_message": error.get("error_message").cloned().unwrap_or(Value::Null),
            }),
            verifier_status: json!({
                "status": "passed_edge_case",
                "expected_error_code": error_code,
                "observed_error_code": error_code,
            }),
            error_code: Some(error_code.to_string()),
            remediation: Some(edge_remediation(error)),
        });
    }
    Ok(rows)
}

fn verify_edge_delta(domain: &str, name: &str, edge: &Value, expected_delta: &Value) -> Result<()> {
    let before = edge
        .get("before_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("edge case {domain}/{name} missing before_counts object"))?;
    let after = edge
        .get("after_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("edge case {domain}/{name} missing after_counts object"))?;
    let expected = expected_delta
        .as_object()
        .ok_or_else(|| anyhow!("edge case {domain}/{name} expected_delta must be object"))?;
    for (cf, value) in expected {
        let expected_value = value
            .as_i64()
            .ok_or_else(|| anyhow!("edge case {domain}/{name} expected_delta.{cf} not integer"))?;
        let before_value = before
            .get(cf)
            .and_then(Value::as_i64)
            .ok_or_else(|| anyhow!("edge case {domain}/{name} before_counts.{cf} missing"))?;
        let after_value = after
            .get(cf)
            .and_then(Value::as_i64)
            .ok_or_else(|| anyhow!("edge case {domain}/{name} after_counts.{cf} missing"))?;
        let actual = after_value - before_value;
        if actual != expected_value {
            bail!(
                "edge case {domain}/{name} delta mismatch for {cf}: expected {expected_value} actual {actual}"
            );
        }
    }
    Ok(())
}

fn write_jsonl(path: &Path, rows: &[EpisodeRow]) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new temp output {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)
            .with_context(|| format!("serialize episode {}", row.episode_id))?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn readback_jsonl(path: &Path) -> Result<JsonlReadback> {
    let sha256 = sha256_file(path)?;
    let file = File::open(path).with_context(|| format!("open JSONL {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut line_count = 0usize;
    let mut plan_episode_count = 0usize;
    let mut edge_case_episode_count = 0usize;
    let mut first_episode_id = None;
    let mut last_episode_id = None;
    for (idx, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read JSONL line {}", idx + 1))?;
        if line.trim().is_empty() {
            bail!(
                "JSONL line {} is blank; blank lines are invalid records",
                idx + 1
            );
        }
        let value: Value =
            serde_json::from_str(&line).with_context(|| format!("parse JSONL line {}", idx + 1))?;
        if !value.is_object() {
            bail!("JSONL line {} is not a JSON object", idx + 1);
        }
        if value.get("schema_version").and_then(Value::as_u64)
            != Some(EPISODE_SCHEMA_VERSION as u64)
        {
            bail!("JSONL line {} has wrong schema_version", idx + 1);
        }
        for key in [
            "record_kind",
            "episode_id",
            "state",
            "action",
            "expected_persisted_delta",
            "actual_readback",
            "verifier_status",
        ] {
            if value.get(key).is_none() {
                bail!("JSONL line {} missing required field {key}", idx + 1);
            }
        }
        let episode_id = value["episode_id"]
            .as_str()
            .ok_or_else(|| anyhow!("JSONL line {} episode_id must be string", idx + 1))?
            .to_string();
        if first_episode_id.is_none() {
            first_episode_id = Some(episode_id.clone());
        }
        last_episode_id = Some(episode_id);
        match value["record_kind"].as_str() {
            Some("dynamicjepa_plan_episode") => plan_episode_count += 1,
            Some("dynamicjepa_edge_case_episode") => edge_case_episode_count += 1,
            Some(other) => bail!(
                "JSONL line {} has unsupported record_kind {other:?}",
                idx + 1
            ),
            None => bail!("JSONL line {} record_kind must be string", idx + 1),
        }
        line_count += 1;
    }
    Ok(JsonlReadback {
        line_count,
        plan_episode_count,
        edge_case_episode_count,
        first_episode_id,
        last_episode_id,
        sha256,
    })
}

fn write_export_manifest(
    path: &Path,
    context: &BundleContext,
    summary: &ExportDynamicJepaEpisodesSummary,
) -> Result<()> {
    let manifest = json!({
        "status": "ok",
        "operation": "export_dynamicjepa_episodes",
        "schema_version": EPISODE_SCHEMA_VERSION,
        "bundle": bundle_ref(context),
        "source_files": {
            "run_manifest": context.run_manifest_path,
            "fsv_report": context.fsv_report_path,
            "db_counts_after": context.db_counts_after_path,
            "edge_case_results": context.edge_case_results_path,
            "sha256": context.source_hashes,
        },
        "output": {
            "path": summary.output,
            "sha256": summary.output_sha256,
            "bytes": summary.output_bytes,
            "readback": summary.readback,
        },
        "domains": summary.domains,
        "counts": {
            "total_records": summary.total_records,
            "plan_episode_count": summary.plan_episode_count,
            "edge_case_episode_count": summary.edge_case_episode_count,
        },
    });
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new temp manifest {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &manifest)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn bundle_ref(context: &BundleContext) -> Value {
    json!({
        "run_id": context.run_id,
        "bundle_schema_version": context.bundle_schema_version,
        "bundle_root": context.bundle_root,
        "db_root": context.db_root,
        "run_manifest_status": context.run_manifest.get("status").cloned().unwrap_or(Value::Null),
        "fsv_report_status": context.fsv_report.get("status").cloned().unwrap_or(Value::Null),
        "source_hashes": context.source_hashes,
    })
}

fn open_existing_db(path: &Path) -> Result<RocksDbTeleologicalStore> {
    let config = TeleologicalStoreConfig {
        create_if_missing: false,
        create_missing_column_families: false,
        ..TeleologicalStoreConfig::default()
    };
    RocksDbTeleologicalStore::open_with_config(path, config)
        .with_context(|| format!("open existing DynamicJEPA RocksDB {}", path.display()))
}

fn domain_db_path(context: &BundleContext, domain: &str) -> Result<PathBuf> {
    let path = context
        .db_root
        .join(format!("{}_{}_rocksdb", context.run_id, domain));
    if !path.join("CURRENT").exists() {
        bail!(
            "expected domain DB is missing or incomplete for domain {domain}: {}",
            path.display()
        );
    }
    path.canonicalize()
        .with_context(|| format!("canonicalize domain DB {}", path.display()))
}

fn manifest_count_drift(
    context: &BundleContext,
    domain: &str,
    actual_counts: &BTreeMap<String, u64>,
) -> Result<BTreeMap<String, CountDrift>> {
    let manifest_counts = context
        .db_counts_after
        .get(domain)
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("db_counts_after.json missing object for domain {domain}"))?;
    let mut drift = BTreeMap::new();
    for (cf, actual) in actual_counts {
        let Some(manifest_value) = manifest_counts.get(cf).and_then(Value::as_u64) else {
            drift.insert(
                cf.clone(),
                CountDrift {
                    manifest: 0,
                    actual_db: *actual,
                },
            );
            continue;
        };
        if manifest_value != *actual {
            drift.insert(
                cf.clone(),
                CountDrift {
                    manifest: manifest_value,
                    actual_db: *actual,
                },
            );
        }
    }
    Ok(drift)
}

fn prediction_json(
    prediction: &context_graph_core::dynamicjepa::PredictionRecord,
) -> Result<Value> {
    let mut value = to_value(prediction, "PredictionRecord")?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow!("PredictionRecord JSON must be an object"))?;
    object.insert(
        "predicted_next_panel_vec_len".to_string(),
        json!(prediction.predicted_next_panel_vec.len()),
    );
    object.insert(
        "predicted_next_panel_vec_sha256".to_string(),
        json!(sha256_f32_slice(&prediction.predicted_next_panel_vec)),
    );
    Ok(value)
}

fn plan_status_error_code(
    plan: &context_graph_core::dynamicjepa::PlanTraceRecord,
) -> Option<String> {
    match &plan.status {
        context_graph_core::dynamicjepa::PlanTraceStatus::Failed { .. } => {
            Some("PLAN_FAILED".to_string())
        }
        _ => None,
    }
}

fn edge_remediation(error: &Value) -> String {
    if let Some(remediation) = error.get("remediation").and_then(Value::as_str) {
        return remediation.to_string();
    }
    "inspect the edge evidence file, source-of-truth counts, and typed error_code before rerunning the DynamicJEPA command"
        .to_string()
}

fn to_value<T: Serialize>(value: &T, label: &str) -> Result<Value> {
    serde_json::to_value(value).with_context(|| format!("serialize {label} to JSON"))
}

fn required_str<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{label} missing string field {key}"))
}

fn read_required_json(path: &Path, label: &str) -> Result<Value> {
    let file =
        File::open(path).with_context(|| format!("open required {label}: {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("parse required {label}: {}", path.display()))
}

fn canonical_existing_dir(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.exists() {
        bail!("{label} does not exist: {}", path.display());
    }
    if !path.is_dir() {
        bail!("{label} is not a directory: {}", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("canonicalize {label} {}", path.display()))
}

fn resolve_existing_path(bundle_root: &Path, raw: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw);
    let candidate = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("read current working directory")?
            .join(&path)
    };
    if candidate.exists() {
        return canonical_existing_dir(&candidate, label);
    }
    let bundle_relative = bundle_root.join(raw);
    if bundle_relative.exists() {
        return canonical_existing_dir(&bundle_relative, label);
    }
    bail!(
        "{label} does not exist. Tried {} and {}",
        candidate.display(),
        bundle_relative.display()
    );
}

fn derive_manifest_path(out: &Path, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let parent = out
        .parent()
        .ok_or_else(|| anyhow!("--out path has no parent: {}", out.display()))?;
    let stem = out
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("episodes");
    Ok(parent.join(format!("{stem}.manifest.json")))
}

fn temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("dynamicjepa_export");
    path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
}

fn cleanup_temp(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

fn promote_temp(temp: &Path, final_path: &Path) -> Result<()> {
    fs::rename(temp, final_path)
        .with_context(|| format!("rename {} to {}", temp.display(), final_path.display()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("sha256:{}", sha256_bytes(&bytes)))
}

fn sha256_f32_slice(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{}", hex(hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(hasher.finalize())
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
