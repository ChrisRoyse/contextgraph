//! Bind Claude Code Bash test outcomes back to ME-JEPA predictions.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Args;
use context_graph_mejepa::{
    open_infer_rocksdb, ActiveLearningKind, ActiveLearningQueueEntry, ActiveLearningQueueState,
    Language, MejepaStore, PredictionId, RealityPrediction, RocksDbEvalStore, RocksDbInferStore,
    SurpriseSeverity, TaskId, TestOutcome, Verdict,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::mejepa_active_learning::DEFAULT_MEJEPA_INFER_DB;

#[derive(Args, Debug, Clone)]
pub struct BindTestOutcomesArgs {
    /// Inference RocksDB path containing live predictions and verification rows.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// JSONL file emitted by scripts/cc/parse_test_output.py.
    #[arg(long)]
    pub events_jsonl: PathBuf,

    /// Durable source-of-truth path for the raw hook/test-output evidence.
    #[arg(long)]
    pub evidence_path: PathBuf,

    /// Repo-relative edited file path used to bind predictions by covered chunk.
    #[arg(long = "edited-file")]
    pub edited_files: Vec<String>,

    /// Record an inconclusive verification when the command was a test runner
    /// but no concrete test markers were parsed.
    #[arg(long, default_value_t = false)]
    pub ambiguous: bool,

    /// Session id used only for ambiguous rows; normal rows use the event file.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Tool use id used only for ambiguous rows.
    #[arg(long)]
    pub tool_use_id: Option<String>,

    /// Bash command used only for ambiguous rows.
    #[arg(long)]
    pub command: Option<String>,

    /// Deterministic timestamp override for FSV.
    #[arg(long)]
    pub now_ms: Option<i64>,
}

#[derive(Args, Debug, Clone)]
pub struct StopSelfVerifyArgs {
    /// Inference RocksDB path containing live predictions and verification rows.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Current Claude session id as 32 lowercase hex chars.
    #[arg(long)]
    pub session_id: String,

    /// Number of recent predictions to inspect for the session.
    #[arg(long, default_value_t = 50)]
    pub max_predictions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestOutcomeEvent {
    event_id: String,
    ts: i64,
    session_id: String,
    tool_use_id: String,
    command: String,
    framework: String,
    test_id: String,
    outcome: String,
    duration_ms: Option<u64>,
    error_log: String,
    source: String,
    sequence: u64,
    line_no: u64,
    #[serde(default)]
    summary_count: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Agreement {
    Confirmed,
    Refuted,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionVerificationRecord {
    pub schema_version: u32,
    pub verification_id: String,
    pub prediction_id: String,
    pub ts: i64,
    pub session_id: String,
    pub task_id: String,
    pub test_id: String,
    pub tool_use_id: String,
    pub command: String,
    pub observed_outcome: String,
    pub predicted_outcome: String,
    pub agreement: String,
    pub evidence_path: String,
    pub event_id: String,
    pub created_at_unix_ms: i64,
    pub source: String,
    pub source_prediction_cf: String,
    pub source_verification_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindTestOutcomesOutput {
    pub tool: String,
    pub source_of_truth: serde_json::Value,
    pub events_scanned: usize,
    pub candidate_predictions_seen: usize,
    pub verifications_written: usize,
    pub refuted_queue_entries: usize,
    pub rows: Vec<PredictionVerificationRecord>,
    pub boundary: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopSelfVerifyPredictionView {
    pub prediction_id: String,
    pub task_id: String,
    pub verdict: String,
    pub language: String,
    pub created_at_unix_ms: i64,
    pub severity_score: f32,
    pub recommended_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopSelfVerifyOutput {
    pub tool: String,
    pub source_of_truth: serde_json::Value,
    pub session_id: String,
    pub predictions_scanned: usize,
    pub verified_prediction_count: usize,
    pub abstained_prediction_count: usize,
    pub unverified_count: usize,
    pub unverified_predictions: Vec<StopSelfVerifyPredictionView>,
    pub recommended_commands: Vec<String>,
    pub truncated_recommended_command_count: usize,
    pub block: bool,
}

pub fn bind_test_outcomes(args: BindTestOutcomesArgs) -> Result<BindTestOutcomesOutput> {
    let now_ms = args
        .now_ms
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let events = load_events(&args, now_ms)?;
    if events.is_empty() {
        return Ok(BindTestOutcomesOutput {
            tool: "context-graph-cli mejepa bind-test-outcomes".to_string(),
            source_of_truth: source_of_truth(&args.db_path, &args.evidence_path),
            events_scanned: 0,
            candidate_predictions_seen: 0,
            verifications_written: 0,
            refuted_queue_entries: 0,
            rows: Vec::new(),
            boundary: json!({"reason": "no_test_outcome_events"}),
        });
    }

    let db = open_infer_rocksdb(&args.db_path).with_context(|| {
        format!(
            "MEJEPA_VERIFICATION_DB_OPEN_FAILED: {}",
            args.db_path.display()
        )
    })?;
    let infer_store = RocksDbInferStore::new(db.clone());
    let eval_store =
        RocksDbEvalStore::new(db.clone()).context("MEJEPA_VERIFICATION_EVAL_STORE_OPEN_FAILED")?;
    let mut rows = Vec::new();
    let mut candidate_predictions_seen = 0usize;
    let mut refuted_queue_entries = 0usize;
    let edited_files = normalize_edited_files(&args.edited_files);

    for event in &events {
        validate_event(event)?;
        let session_id = decode_hex16(&event.session_id)?;
        let predictions = infer_store
            .read_live_predictions(session_id, 50)
            .with_context(|| {
                format!(
                    "MEJEPA_VERIFICATION_PREDICTION_LOOKUP_FAILED: {}",
                    event.session_id
                )
            })?;
        candidate_predictions_seen += predictions.len();
        let matches = matching_predictions(&predictions, &edited_files);
        for prediction in matches {
            let record = verification_record(prediction, event, &args.evidence_path, now_ms);
            write_verification_readback(db.as_ref(), &record)?;
            if record.agreement == "refuted" {
                enqueue_refuted_prediction(&eval_store, prediction)?;
                refuted_queue_entries += 1;
            }
            rows.push(record);
        }
    }

    Ok(BindTestOutcomesOutput {
        tool: "context-graph-cli mejepa bind-test-outcomes".to_string(),
        source_of_truth: source_of_truth(&args.db_path, &args.evidence_path),
        events_scanned: events.len(),
        candidate_predictions_seen,
        verifications_written: rows.len(),
        refuted_queue_entries,
        rows,
        boundary: json!({
            "edited_files": edited_files,
            "ambiguous": args.ambiguous,
        }),
    })
}

pub fn stop_self_verify(args: StopSelfVerifyArgs) -> Result<StopSelfVerifyOutput> {
    if !(1..=1000).contains(&args.max_predictions) {
        return Err(anyhow!(
            "MEJEPA_STOP_SELF_VERIFY_LIMIT_INVALID: {}",
            args.max_predictions
        ));
    }
    let session_id_bytes = decode_hex16(&args.session_id)?;
    let db = open_infer_rocksdb(&args.db_path).with_context(|| {
        format!(
            "MEJEPA_STOP_SELF_VERIFY_DB_OPEN_FAILED: {}",
            args.db_path.display()
        )
    })?;
    let infer_store = RocksDbInferStore::new(db.clone());
    let predictions = infer_store
        .read_live_predictions(session_id_bytes, args.max_predictions)
        .with_context(|| {
            format!(
                "MEJEPA_STOP_SELF_VERIFY_PREDICTION_LOOKUP_FAILED: {}",
                args.session_id
            )
        })?;
    let verified = confirmed_or_refuted_prediction_ids(db.as_ref(), &args.session_id)?;

    let mut unverified_predictions = Vec::new();
    let mut verified_prediction_count = 0usize;
    let mut abstained_prediction_count = 0usize;
    for prediction in &predictions {
        let prediction_id = hex::encode(prediction.prediction_id);
        if prediction.verdict == Verdict::Abstain {
            abstained_prediction_count += 1;
            continue;
        }
        if verified.contains(&prediction_id) {
            verified_prediction_count += 1;
            continue;
        }
        unverified_predictions.push(stop_prediction_view(prediction));
    }
    unverified_predictions.sort_by(|left, right| {
        right
            .severity_score
            .total_cmp(&left.severity_score)
            .then_with(|| left.prediction_id.cmp(&right.prediction_id))
    });
    let all_recommended = top_recommended_commands(&unverified_predictions);
    let top_recommended = all_recommended.iter().take(10).cloned().collect::<Vec<_>>();
    let block = !unverified_predictions.is_empty();
    Ok(StopSelfVerifyOutput {
        tool: "context-graph-cli mejepa stop-self-verify".to_string(),
        source_of_truth: json!({
            "db_path": args.db_path,
            "live_prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
            "verification_cf": context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
            "session_id": args.session_id,
            "max_predictions": args.max_predictions,
        }),
        session_id: args.session_id,
        predictions_scanned: predictions.len(),
        verified_prediction_count,
        abstained_prediction_count,
        unverified_count: unverified_predictions.len(),
        unverified_predictions,
        truncated_recommended_command_count: all_recommended.len().saturating_sub(10),
        recommended_commands: top_recommended,
        block,
    })
}

fn load_events(args: &BindTestOutcomesArgs, now_ms: i64) -> Result<Vec<TestOutcomeEvent>> {
    if args.ambiguous {
        let session_id = args
            .session_id
            .as_deref()
            .ok_or_else(|| anyhow!("MEJEPA_VERIFICATION_AMBIGUOUS_SESSION_REQUIRED"))?;
        let command = args.command.clone().unwrap_or_default();
        let event_id = stable_id(&[
            "ambiguous",
            session_id,
            args.tool_use_id.as_deref().unwrap_or_default(),
            &command,
            &args.evidence_path.display().to_string(),
            &now_ms.to_string(),
        ]);
        return Ok(vec![TestOutcomeEvent {
            event_id,
            ts: now_ms,
            session_id: session_id.to_string(),
            tool_use_id: args.tool_use_id.clone().unwrap_or_default(),
            command,
            framework: "unknown".to_string(),
            test_id: "ambiguous::no-clear-test-marker".to_string(),
            outcome: "unknown".to_string(),
            duration_ms: None,
            error_log: String::new(),
            source: "claude_post_tool_use_bash_ambiguous".to_string(),
            sequence: 0,
            line_no: 0,
            summary_count: None,
        }]);
    }
    let raw = fs::read_to_string(&args.events_jsonl).with_context(|| {
        format!(
            "MEJEPA_VERIFICATION_EVENTS_READ_FAILED: {}",
            args.events_jsonl.display()
        )
    })?;
    let mut events = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: TestOutcomeEvent = serde_json::from_str(line).with_context(|| {
            format!(
                "MEJEPA_VERIFICATION_EVENT_JSON_INVALID: {}:{}",
                args.events_jsonl.display(),
                idx + 1
            )
        })?;
        events.push(event);
    }
    Ok(events)
}

fn validate_event(event: &TestOutcomeEvent) -> Result<()> {
    if event.event_id.len() != 32 || !event.event_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("MEJEPA_VERIFICATION_EVENT_ID_INVALID"));
    }
    if event.session_id.len() != 32 || !event.session_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("MEJEPA_VERIFICATION_SESSION_ID_INVALID"));
    }
    if event.test_id.trim().is_empty() {
        return Err(anyhow!("MEJEPA_VERIFICATION_TEST_ID_EMPTY"));
    }
    if !matches!(
        event.outcome.as_str(),
        "pass" | "fail" | "skipped" | "unknown"
    ) {
        return Err(anyhow!(
            "MEJEPA_VERIFICATION_OUTCOME_INVALID: {}",
            event.outcome
        ));
    }
    Ok(())
}

fn matching_predictions<'a>(
    predictions: &'a [RealityPrediction],
    edited_files: &BTreeSet<String>,
) -> Vec<&'a RealityPrediction> {
    if edited_files.is_empty() {
        return predictions.first().into_iter().collect();
    }
    let matches = predictions
        .iter()
        .filter(|prediction| {
            prediction.covered_chunks.iter().any(|chunk| {
                let raw = chunk.0.as_str();
                let path = raw.split('#').next().unwrap_or(raw);
                edited_files
                    .iter()
                    .any(|edited| path == edited || raw.starts_with(edited) || raw.contains(edited))
            })
        })
        .collect::<Vec<_>>();
    matches
}

fn verification_record(
    prediction: &RealityPrediction,
    event: &TestOutcomeEvent,
    evidence_path: &Path,
    now_ms: i64,
) -> PredictionVerificationRecord {
    let observed = observed_outcome(event);
    let predicted = predicted_outcome(prediction, &event.test_id);
    let agreement = agreement(predicted.as_deref(), observed.as_deref());
    let prediction_id = hex::encode(prediction.prediction_id);
    let verification_id = stable_id(&[
        &event.session_id,
        &event.event_id,
        &prediction_id,
        &event.test_id,
        &now_ms.to_string(),
    ]);
    PredictionVerificationRecord {
        schema_version: 1,
        verification_id,
        prediction_id,
        ts: now_ms,
        session_id: event.session_id.clone(),
        task_id: prediction.task_id.0.clone(),
        test_id: event.test_id.clone(),
        tool_use_id: event.tool_use_id.clone(),
        command: event.command.clone(),
        observed_outcome: observed.unwrap_or_else(|| "unknown".to_string()),
        predicted_outcome: predicted.unwrap_or_else(|| "unknown".to_string()),
        agreement: match agreement {
            Agreement::Confirmed => "confirmed",
            Agreement::Refuted => "refuted",
            Agreement::Inconclusive => "inconclusive",
        }
        .to_string(),
        evidence_path: evidence_path.display().to_string(),
        event_id: event.event_id.clone(),
        created_at_unix_ms: now_ms,
        source: "claude_post_tool_use_bash".to_string(),
        source_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_verification_cf: context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS
            .to_string(),
    }
}

fn observed_outcome(event: &TestOutcomeEvent) -> Option<String> {
    match event.outcome.as_str() {
        "pass" => Some("pass".to_string()),
        "fail" => Some("fail".to_string()),
        "skipped" | "unknown" => None,
        _ => None,
    }
}

fn predicted_outcome(prediction: &RealityPrediction, test_id: &str) -> Option<String> {
    for item in &prediction.predicted_failed_tests {
        if item.test_id.0 == test_id {
            return test_outcome_to_hard_label(item.predicted_outcome);
        }
    }
    match prediction.verdict {
        Verdict::Pass => Some("pass".to_string()),
        Verdict::Fail | Verdict::GuardRejected => Some("fail".to_string()),
        Verdict::OutOfDistribution | Verdict::Abstain => None,
    }
}

fn test_outcome_to_hard_label(outcome: TestOutcome) -> Option<String> {
    match outcome {
        TestOutcome::Pass => Some("pass".to_string()),
        TestOutcome::Fail | TestOutcome::Error => Some("fail".to_string()),
        TestOutcome::Skip | TestOutcome::Flaky => None,
    }
}

fn agreement(predicted: Option<&str>, observed: Option<&str>) -> Agreement {
    match (predicted, observed) {
        (Some(predicted), Some(observed)) if predicted == observed => Agreement::Confirmed,
        (Some(_), Some(_)) => Agreement::Refuted,
        _ => Agreement::Inconclusive,
    }
}

fn write_verification_readback(db: &DB, record: &PredictionVerificationRecord) -> Result<()> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
        .ok_or_else(|| anyhow!("MEJEPA_VERIFICATION_CF_MISSING"))?;
    let key = verification_key(record);
    let value = serde_json::to_vec(record).context("MEJEPA_VERIFICATION_ENCODE_FAILED")?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &opts)
        .context("MEJEPA_VERIFICATION_WRITE_FAILED")?;
    db.flush_wal(true)
        .context("MEJEPA_VERIFICATION_WAL_FLUSH_FAILED")?;
    db.flush_cf(cf)
        .context("MEJEPA_VERIFICATION_CF_FLUSH_FAILED")?;
    let readback = db
        .get_cf(cf, &key)
        .context("MEJEPA_VERIFICATION_READBACK_FAILED")?
        .ok_or_else(|| anyhow!("MEJEPA_VERIFICATION_READBACK_MISSING"))?;
    if readback != value {
        return Err(anyhow!("MEJEPA_VERIFICATION_READBACK_BYTES_DIFFER"));
    }
    let decoded: PredictionVerificationRecord =
        serde_json::from_slice(&readback).context("MEJEPA_VERIFICATION_READBACK_DECODE_FAILED")?;
    if decoded.verification_id != record.verification_id {
        return Err(anyhow!("MEJEPA_VERIFICATION_READBACK_RECORD_DIFFERS"));
    }
    Ok(())
}

fn enqueue_refuted_prediction(
    eval_store: &RocksDbEvalStore,
    prediction: &RealityPrediction,
) -> Result<()> {
    let mut queue = eval_store
        .load_queue()
        .context("MEJEPA_VERIFICATION_QUEUE_READ_FAILED")?
        .unwrap_or(ActiveLearningQueueState::new(1024)?);
    let task_id = TaskId(format!(
        "verification-refuted::{}",
        hex::encode(prediction.prediction_id)
    ));
    let entry = ActiveLearningQueueEntry {
        task_id: task_id.clone(),
        score: 6.0,
        outcome_set_len: prediction.outcome_set.outcomes.len(),
        ood_score: prediction.ood_score,
        curiosity_score: 0.0,
        reason: "prediction_verification_refuted".to_string(),
        kind: ActiveLearningKind::AgentSurprise {
            prediction_id: PredictionId(prediction.prediction_id),
            severity_score: SurpriseSeverity::High.severity_score(),
        },
    };
    queue.entries.insert(task_id, entry);
    eval_store
        .persist_queue(&queue)
        .context("MEJEPA_VERIFICATION_QUEUE_WRITE_FAILED")?;
    Ok(())
}

fn verification_key(record: &PredictionVerificationRecord) -> Vec<u8> {
    format!(
        "{}:{}:{}",
        record.session_id, record.event_id, record.prediction_id
    )
    .into_bytes()
}

fn confirmed_or_refuted_prediction_ids(db: &DB, session_id: &str) -> Result<BTreeSet<String>> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
        .ok_or_else(|| anyhow!("MEJEPA_STOP_SELF_VERIFY_VERIFICATION_CF_MISSING"))?;
    let mut ids = BTreeSet::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item.context("MEJEPA_STOP_SELF_VERIFY_VERIFICATION_ITER_FAILED")?;
        let row: PredictionVerificationRecord = serde_json::from_slice(&value)
            .context("MEJEPA_STOP_SELF_VERIFY_VERIFICATION_DECODE_FAILED")?;
        if row.session_id == session_id && matches!(row.agreement.as_str(), "confirmed" | "refuted")
        {
            ids.insert(row.prediction_id);
        }
    }
    Ok(ids)
}

fn stop_prediction_view(prediction: &RealityPrediction) -> StopSelfVerifyPredictionView {
    StopSelfVerifyPredictionView {
        prediction_id: hex::encode(prediction.prediction_id),
        task_id: prediction.task_id.0.clone(),
        verdict: verdict_label(prediction.verdict).to_string(),
        language: language_label(prediction.language).to_string(),
        created_at_unix_ms: prediction.created_at_unix_ms,
        severity_score: stop_prediction_severity_score(prediction),
        recommended_commands: recommended_commands_for_prediction(prediction),
    }
}

fn stop_prediction_severity_score(prediction: &RealityPrediction) -> f32 {
    let verdict_weight = match prediction.verdict {
        Verdict::GuardRejected => 10.0,
        Verdict::Fail => 9.0,
        Verdict::OutOfDistribution => 8.0,
        Verdict::Pass => 4.0,
        Verdict::Abstain => 0.0,
    };
    let predicted_failure_weight = (prediction.predicted_failed_tests.len().min(8) as f32) * 0.25;
    let oracle_fail_weight = (1.0 - prediction.predicted_oracle_pass).clamp(0.0, 1.0);
    verdict_weight
        + predicted_failure_weight
        + prediction.ood_score.clamp(0.0, 1.0)
        + oracle_fail_weight
}

fn top_recommended_commands(views: &[StopSelfVerifyPredictionView]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for view in views {
        for command in &view.recommended_commands {
            if seen.insert(command.clone()) {
                out.push(command.clone());
            }
        }
    }
    out
}

fn recommended_commands_for_prediction(prediction: &RealityPrediction) -> Vec<String> {
    let mut commands = BTreeSet::new();
    for predicted in &prediction.predicted_failed_tests {
        if let Some(command) = command_for_test_id(prediction.language, &predicted.test_id.0) {
            commands.insert(command);
        }
    }
    if commands.is_empty() {
        commands.insert(fallback_test_command(prediction));
    }
    commands.into_iter().collect()
}

fn command_for_test_id(language: Language, test_id: &str) -> Option<String> {
    let test_id = test_id.trim();
    if test_id.is_empty() {
        return None;
    }
    let command = match language {
        Language::Python => format!("pytest {test_id} -q"),
        Language::Rust => {
            let test_name = test_id.rsplit("::").next().unwrap_or(test_id);
            format!("cargo test {test_name}")
        }
        Language::Javascript | Language::Typescript => format!("npm test -- {test_id}"),
        Language::Go => format!("go test ./... -run {test_id}"),
        Language::Java => format!("mvn test -Dtest={test_id}"),
        Language::C | Language::Cpp | Language::CSharp | Language::Ruby | Language::Php => {
            format!("run project tests for {test_id}")
        }
    };
    Some(command)
}

fn fallback_test_command(prediction: &RealityPrediction) -> String {
    match prediction.language {
        Language::Python => first_chunk_path(prediction)
            .filter(|path| path.starts_with("tests/") && path.ends_with(".py"))
            .map(|path| format!("pytest {path} -q"))
            .unwrap_or_else(|| "pytest -q".to_string()),
        Language::Rust => "cargo test".to_string(),
        Language::Javascript | Language::Typescript => "npm test".to_string(),
        Language::Go => "go test ./...".to_string(),
        Language::Java => "mvn test".to_string(),
        Language::C | Language::Cpp | Language::CSharp | Language::Ruby | Language::Php => {
            "run project test suite".to_string()
        }
    }
}

fn first_chunk_path(prediction: &RealityPrediction) -> Option<&str> {
    prediction.covered_chunks.first().map(|chunk| {
        let raw = chunk.0.as_str();
        raw.split('#').next().unwrap_or(raw)
    })
}

fn verdict_label(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
        Verdict::OutOfDistribution => "out_of_distribution",
        Verdict::Abstain => "abstain",
        Verdict::GuardRejected => "guard_rejected",
    }
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Rust => "rust",
        Language::Python => "python",
        Language::Javascript => "javascript",
        Language::Typescript => "typescript",
        Language::Go => "go",
        Language::Java => "java",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::CSharp => "csharp",
        Language::Ruby => "ruby",
        Language::Php => "php",
    }
}

#[cfg(test)]
fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing CF {cf_name}"))?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count())
}

fn decode_hex16(value: &str) -> Result<[u8; 16]> {
    let mut out = [0u8; 16];
    hex::decode_to_slice(value, &mut out)
        .with_context(|| format!("MEJEPA_VERIFICATION_SESSION_DECODE_FAILED: {value}"))?;
    Ok(out)
}

fn normalize_edited_files(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.trim().trim_start_matches("./").to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn stable_id(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())[..32].to_string()
}

fn source_of_truth(db_path: &Path, evidence_path: &Path) -> serde_json::Value {
    json!({
        "db_path": db_path,
        "prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
        "verification_cf": context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
        "active_learning_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
        "evidence_path": evidence_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa::{
        ChunkId, ConformalInterval, ConformalMethod, ConformalSet, FailureModeClass, Language,
        OracleOutcome, PredictedFailureMode, PredictedTestOutcome, PredictionProvenance,
        RealityPrediction, ReasoningClass, RocksDbInferStore, RootCauseClass, Severity, TaskId,
        TestDeltaKind, TestId, WitnessHash,
    };
    use tempfile::TempDir;

    fn prediction(
        session_id: [u8; 16],
        prediction_id: [u8; 16],
        chunk: &str,
        pass: f32,
    ) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id,
            witness_hash: WitnessHash([0x77; 32]),
            task_id: TaskId(format!("task-{}", hex::encode(prediction_id))),
            session_id,
            language: Language::Python,
            covered_chunks: vec![ChunkId(chunk.to_string())],
            verdict: if pass >= 0.5 {
                Verdict::Pass
            } else {
                Verdict::Fail
            },
            confidence_interval: ConformalInterval {
                lower: 0.1,
                upper: 0.9,
                method: ConformalMethod::SplitConformal,
                coverage_target: 0.90,
                empirical_coverage: 0.89,
            },
            predicted_oracle_pass: pass,
            predicted_test_pass: vec![pass],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(
                vec![if pass >= 0.5 {
                    OracleOutcome::Pass
                } else {
                    OracleOutcome::Fail
                }],
                0.1,
                0.2,
            )
            .unwrap(),
            calibrated_confidence: 0.8,
            degraded_status: false,
            granger_attestations: Default::default(),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            guard_violations: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: ReasoningClass::MostlyCorrect,
            agent_claim_graph: Default::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: PredictionProvenance {
                predictor_version: "verification-test".to_string(),
                constellation_version: "verification-test".to_string(),
                calibration_version: "verification-test".to_string(),
                active_pointer: hex::encode(prediction_id),
            },
            source_panel_sha: [0x42; 32],
            calibration_version: "verification-test".to_string(),
            created_at_unix_ms: 1_779_000_000_000 + i64::from(prediction_id[0]),
            matched_fingerprint: None,
            unknown_candidate_id: None,
        })
        .unwrap()
    }

    fn prediction_with_verdict(
        session_id: [u8; 16],
        prediction_id: [u8; 16],
        chunk: &str,
        pass: f32,
        verdict: Verdict,
        predicted_failed_tests: Vec<PredictedTestOutcome>,
    ) -> RealityPrediction {
        let mut prediction = prediction(session_id, prediction_id, chunk, pass);
        prediction.verdict = verdict;
        prediction.outcome_set = ConformalSet::try_new(
            vec![match verdict {
                Verdict::Pass => OracleOutcome::Pass,
                Verdict::Fail | Verdict::GuardRejected => OracleOutcome::Fail,
                Verdict::OutOfDistribution => OracleOutcome::OutOfDistribution,
                Verdict::Abstain => OracleOutcome::Abstain,
            }],
            0.1,
            0.2,
        )
        .unwrap();
        prediction.predicted_failed_tests = predicted_failed_tests;
        RealityPrediction::try_new(prediction).unwrap()
    }

    fn predicted_failed_test(test_id: &str, chunk: &str) -> PredictedTestOutcome {
        PredictedTestOutcome {
            test_id: TestId(test_id.to_string()),
            current_outcome: TestOutcome::Pass,
            predicted_outcome: TestOutcome::Fail,
            delta_kind: TestDeltaKind::PassToFail,
            confidence: 0.91,
            why: PredictedFailureMode {
                failure_class: FailureModeClass::WrongAlgorithm,
                chunk: ChunkId(chunk.to_string()),
                line_range: (1, 2),
                confidence: 0.91,
                severity: Severity::High,
                explanation: "synthetic stop self-verify predicted failure".to_string(),
                contributing_embedders: Vec::new(),
                root_cause_class: RootCauseClass::LogicError,
            },
        }
    }

    fn event_json(
        session_id: [u8; 16],
        event_id: &str,
        command: &str,
        test_id: &str,
        outcome: &str,
    ) -> String {
        serde_json::to_string(&TestOutcomeEvent {
            event_id: event_id.to_string(),
            ts: 1,
            session_id: hex::encode(session_id),
            tool_use_id: "tool-1".to_string(),
            command: command.to_string(),
            framework: "pytest".to_string(),
            test_id: test_id.to_string(),
            outcome: outcome.to_string(),
            duration_ms: None,
            error_log: if outcome == "fail" {
                "failed".to_string()
            } else {
                String::new()
            },
            source: "unit".to_string(),
            sequence: 0,
            line_no: 1,
            summary_count: None,
        })
        .unwrap()
    }

    fn read_verification_rows(db: &DB) -> Result<Vec<PredictionVerificationRecord>> {
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
            .ok_or_else(|| anyhow!("missing verification CF"))?;
        let mut rows = Vec::new();
        for item in db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item?;
            rows.push(serde_json::from_slice(&value)?);
        }
        Ok(rows)
    }

    #[test]
    fn bind_test_outcomes_writes_verifications_and_queue() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("infer-db");
        let evidence_path = temp.path().join("events.jsonl");
        let db = open_infer_rocksdb(&db_path).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let p1 = prediction([0x44; 16], [0x01; 16], "src/demo.py#fn#foo", 0.1);
        let p2 = prediction([0x44; 16], [0x02; 16], "src/demo.py#fn#bar", 0.2);
        store.write_live_prediction(&p1).unwrap();
        store.write_live_prediction(&p2).unwrap();
        drop(store);
        drop(db);
        fs::write(
            &evidence_path,
            format!(
                "{}\n",
                serde_json::to_string(&TestOutcomeEvent {
                    event_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    ts: 1,
                    session_id: hex::encode([0x44; 16]),
                    tool_use_id: "tool-1".to_string(),
                    command: "pytest tests/test_demo.py".to_string(),
                    framework: "pytest".to_string(),
                    test_id: "tests/test_demo.py::test_foo".to_string(),
                    outcome: "fail".to_string(),
                    duration_ms: None,
                    error_log: "failed".to_string(),
                    source: "unit".to_string(),
                    sequence: 0,
                    line_no: 1,
                    summary_count: None,
                })
                .unwrap()
            ),
        )
        .unwrap();
        let out = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: evidence_path.clone(),
            evidence_path,
            edited_files: vec!["src/demo.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_000_123_000),
        })
        .unwrap();
        assert_eq!(out.events_scanned, 1);
        assert_eq!(out.verifications_written, 2);
        assert!(out.rows.iter().all(|row| row.agreement == "confirmed"));
        let reopened = open_infer_rocksdb(&db_path).unwrap();
        assert_eq!(
            count_cf(
                reopened.as_ref(),
                context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn task_py_g_069_posttooluse_capture_fsv() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("infer-db");
        let evidence_dir = temp.path().join("evidence");
        fs::create_dir_all(&evidence_dir).unwrap();
        let session_id = [0x55; 16];
        let db = open_infer_rocksdb(&db_path).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        store
            .write_live_prediction(&prediction(
                session_id,
                [0x10; 16],
                "src/refute.py#fn#x",
                0.9,
            ))
            .unwrap();
        store
            .write_live_prediction(&prediction(
                session_id,
                [0x11; 16],
                "src/confirmed.py#fn#a",
                0.1,
            ))
            .unwrap();
        store
            .write_live_prediction(&prediction(
                session_id,
                [0x12; 16],
                "src/confirmed.py#fn#b",
                0.2,
            ))
            .unwrap();
        store
            .write_live_prediction(&prediction(
                session_id,
                [0x13; 16],
                "src/ambiguous.py#fn#z",
                0.8,
            ))
            .unwrap();
        drop(store);
        drop(db);

        let refuted_events = evidence_dir.join("refuted.jsonl");
        fs::write(
            &refuted_events,
            format!(
                "{}\n",
                event_json(
                    session_id,
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "pytest tests/test_refute.py",
                    "tests/test_refute.py::test_refute",
                    "fail",
                )
            ),
        )
        .unwrap();
        let refuted = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: refuted_events.clone(),
            evidence_path: refuted_events.clone(),
            edited_files: vec!["src/refute.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_001_000_001),
        })
        .unwrap();
        assert_eq!(refuted.verifications_written, 1);
        assert_eq!(refuted.refuted_queue_entries, 1);
        assert_eq!(refuted.rows[0].agreement, "refuted");
        assert_eq!(refuted.rows[0].ts, 1_779_001_000_001);

        let confirmed_events = evidence_dir.join("confirmed.jsonl");
        fs::write(
            &confirmed_events,
            format!(
                "{}\n",
                event_json(
                    session_id,
                    "cccccccccccccccccccccccccccccccc",
                    "pytest tests/test_confirmed.py",
                    "tests/test_confirmed.py::test_confirmed",
                    "fail",
                )
            ),
        )
        .unwrap();
        let confirmed = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: confirmed_events.clone(),
            evidence_path: confirmed_events.clone(),
            edited_files: vec!["src/confirmed.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_001_000_002),
        })
        .unwrap();
        assert_eq!(confirmed.verifications_written, 2);
        assert!(confirmed
            .rows
            .iter()
            .all(|row| row.agreement == "confirmed"));

        let ambiguous_events = evidence_dir.join("ambiguous.jsonl");
        let ambiguous = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: ambiguous_events.clone(),
            evidence_path: ambiguous_events.clone(),
            edited_files: vec!["src/ambiguous.py".to_string()],
            ambiguous: true,
            session_id: Some(hex::encode(session_id)),
            tool_use_id: Some("tool-ambiguous".to_string()),
            command: Some("pytest -q".to_string()),
            now_ms: Some(1_779_001_000_003),
        })
        .unwrap();
        assert_eq!(ambiguous.verifications_written, 1);
        assert_eq!(ambiguous.rows[0].agreement, "inconclusive");

        let empty_events = evidence_dir.join("empty.jsonl");
        fs::write(&empty_events, "").unwrap();
        let empty = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: empty_events.clone(),
            evidence_path: empty_events,
            edited_files: vec!["src/non_test.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_001_000_004),
        })
        .unwrap();
        assert_eq!(empty.verifications_written, 0);

        let reopened = open_infer_rocksdb(&db_path).unwrap();
        let rows = read_verification_rows(reopened.as_ref()).unwrap();
        let eval_store = RocksDbEvalStore::new(reopened.clone()).unwrap();
        let queue = eval_store.load_queue().unwrap().unwrap();
        let refuted_score = queue
            .entries
            .values()
            .find(|entry| entry.reason == "prediction_verification_refuted")
            .map(|entry| entry.score)
            .unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(refuted_score, 6.0);

        let all_passed = rows.len() == 4
            && rows.iter().any(|row| row.agreement == "refuted")
            && rows.iter().any(|row| row.agreement == "inconclusive")
            && rows
                .iter()
                .filter(|row| row.agreement == "confirmed")
                .count()
                == 2
            && (refuted_score - 6.0).abs() < f32::EPSILON;
        let fsv_root = std::env::var("CONTEXTGRAPH_FSV_ROOT")
            .unwrap_or_else(|_| "/mnt/d/contextgraph/fsv".to_string());
        let fsv_dir = PathBuf::from(fsv_root)
            .join("task-py-g-069-posttooluse-capture-fsv")
            .join(format!(
                "run-{}-{}",
                chrono::Utc::now().timestamp_millis(),
                std::process::id()
            ));
        fs::create_dir_all(&fsv_dir).unwrap();
        let report_path = fsv_dir.join("posttooluse_capture_fsv.json");
        let report = json!({
            "task": "TASK-PY-G-069",
            "issue": 288,
            "all_passed": all_passed,
            "source_of_truth": {
                "db_path": db_path,
                "verification_cf": context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
                "live_prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                "active_learning_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
                "fsv_path": report_path.display().to_string(),
                "hook_capture_log": "/mnt/d/contextgraph/runtime/posttooluse-mejepa-capture.jsonl"
            },
            "cases": {
                "refuted_queue_weight_6x": {
                    "verifications_written": refuted.verifications_written,
                    "queue_entries_written": refuted.refuted_queue_entries,
                    "queue_score": refuted_score
                },
                "multiple_predictions_by_covered_chunk_overlap": {
                    "verifications_written": confirmed.verifications_written,
                    "agreements": confirmed.rows.iter().map(|row| row.agreement.clone()).collect::<Vec<_>>()
                },
                "ambiguous_output_inconclusive": {
                    "verifications_written": ambiguous.verifications_written,
                    "agreement": ambiguous.rows[0].agreement.clone()
                },
                "empty_non_test_event_file_no_write": {
                    "verifications_written": empty.verifications_written
                },
                "reopen_readback": {
                    "verification_rows": rows.len(),
                    "agreements": rows.iter().map(|row| row.agreement.clone()).collect::<Vec<_>>()
                }
            },
            "rows": rows
        });
        fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        assert!(all_passed, "FSV report: {}", report_path.display());
    }

    #[test]
    fn stop_self_verify_blocks_until_confirmed_or_refuted() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("infer-db");
        let evidence_path = temp.path().join("events.jsonl");
        let session_id = [0x66; 16];
        let db = open_infer_rocksdb(&db_path).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        store
            .write_live_prediction(&prediction_with_verdict(
                session_id,
                [0x21; 16],
                "src/stop_gate.py#fn#broken",
                0.1,
                Verdict::Fail,
                vec![predicted_failed_test(
                    "tests/test_stop_gate.py::test_broken",
                    "src/stop_gate.py#fn#broken",
                )],
            ))
            .unwrap();
        drop(store);
        drop(db);

        let before = stop_self_verify(StopSelfVerifyArgs {
            db_path: db_path.clone(),
            session_id: hex::encode(session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(before.block);
        assert_eq!(before.unverified_count, 1);
        assert_eq!(
            before.recommended_commands,
            vec!["pytest tests/test_stop_gate.py::test_broken -q".to_string()]
        );

        fs::write(
            &evidence_path,
            format!(
                "{}\n",
                event_json(
                    session_id,
                    "dddddddddddddddddddddddddddddddd",
                    "pytest tests/test_stop_gate.py::test_broken -q",
                    "tests/test_stop_gate.py::test_broken",
                    "fail",
                )
            ),
        )
        .unwrap();
        bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: evidence_path.clone(),
            evidence_path,
            edited_files: vec!["src/stop_gate.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_002_000_001),
        })
        .unwrap();

        let after = stop_self_verify(StopSelfVerifyArgs {
            db_path,
            session_id: hex::encode(session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(!after.block);
        assert_eq!(after.unverified_count, 0);
        assert_eq!(after.verified_prediction_count, 1);
    }

    #[test]
    fn task_py_g_070_stop_self_verify_fsv() {
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("infer-db");
        let evidence_path = temp.path().join("verified.jsonl");
        let session_id = [0x70; 16];
        let abstain_session_id = [0x71; 16];
        let bulk_session_id = [0x72; 16];
        let db = open_infer_rocksdb(&db_path).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        store
            .write_live_prediction(&prediction_with_verdict(
                session_id,
                [0x30; 16],
                "src/stop_fsv.py#fn#broken",
                0.1,
                Verdict::Fail,
                vec![predicted_failed_test(
                    "tests/test_stop_fsv.py::test_broken",
                    "src/stop_fsv.py#fn#broken",
                )],
            ))
            .unwrap();
        store
            .write_live_prediction(&prediction_with_verdict(
                abstain_session_id,
                [0x31; 16],
                "src/abstain.py#fn#x",
                0.5,
                Verdict::Abstain,
                Vec::new(),
            ))
            .unwrap();
        for idx in 1u8..=50 {
            let test_id = format!("tests/test_bulk_{idx:02}.py::test_case_{idx:02}");
            let chunk = format!("src/bulk_{idx:02}.py#fn#case");
            store
                .write_live_prediction(&prediction_with_verdict(
                    bulk_session_id,
                    [idx; 16],
                    &chunk,
                    0.1,
                    Verdict::Fail,
                    vec![predicted_failed_test(&test_id, &chunk)],
                ))
                .unwrap();
        }
        drop(store);
        drop(db);

        let blocked_before = stop_self_verify(StopSelfVerifyArgs {
            db_path: db_path.clone(),
            session_id: hex::encode(session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(blocked_before.block);
        assert_eq!(blocked_before.unverified_count, 1);

        fs::write(
            &evidence_path,
            format!(
                "{}\n",
                event_json(
                    session_id,
                    "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                    "pytest tests/test_stop_fsv.py::test_broken -q",
                    "tests/test_stop_fsv.py::test_broken",
                    "fail",
                )
            ),
        )
        .unwrap();
        let bind = bind_test_outcomes(BindTestOutcomesArgs {
            db_path: db_path.clone(),
            events_jsonl: evidence_path.clone(),
            evidence_path: evidence_path.clone(),
            edited_files: vec!["src/stop_fsv.py".to_string()],
            ambiguous: false,
            session_id: None,
            tool_use_id: None,
            command: None,
            now_ms: Some(1_779_003_000_001),
        })
        .unwrap();

        let allowed_after = stop_self_verify(StopSelfVerifyArgs {
            db_path: db_path.clone(),
            session_id: hex::encode(session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(!allowed_after.block);
        assert_eq!(allowed_after.unverified_count, 0);

        let abstain = stop_self_verify(StopSelfVerifyArgs {
            db_path: db_path.clone(),
            session_id: hex::encode(abstain_session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(!abstain.block);
        assert_eq!(abstain.abstained_prediction_count, 1);

        let bulk = stop_self_verify(StopSelfVerifyArgs {
            db_path: db_path.clone(),
            session_id: hex::encode(bulk_session_id),
            max_predictions: 50,
        })
        .unwrap();
        assert!(bulk.block);
        assert_eq!(bulk.unverified_count, 50);
        assert_eq!(bulk.unverified_predictions.len(), 50);
        assert_eq!(bulk.recommended_commands.len(), 10);
        assert_eq!(bulk.truncated_recommended_command_count, 40);

        let all_passed = blocked_before.block
            && blocked_before.unverified_count == 1
            && bind.verifications_written == 1
            && allowed_after.unverified_count == 0
            && !allowed_after.block
            && !abstain.block
            && abstain.abstained_prediction_count == 1
            && bulk.unverified_predictions.len() == 50
            && bulk.recommended_commands.len() == 10
            && bulk.truncated_recommended_command_count == 40;
        let fsv_root = std::env::var("CONTEXTGRAPH_FSV_ROOT")
            .unwrap_or_else(|_| "/mnt/d/contextgraph/fsv".to_string());
        let fsv_dir = PathBuf::from(fsv_root)
            .join("task-py-g-070-stop-self-verify-fsv")
            .join(format!(
                "run-{}-{}",
                chrono::Utc::now().timestamp_millis(),
                std::process::id()
            ));
        fs::create_dir_all(&fsv_dir).unwrap();
        let report_path = fsv_dir.join("stop_self_verify_fsv.json");
        let report = json!({
            "task": "TASK-PY-G-070",
            "issue": 289,
            "all_passed": all_passed,
            "source_of_truth": {
                "db_path": db_path,
                "live_prediction_cf": context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                "verification_cf": context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
                "hook_log": "/mnt/d/contextgraph/runtime/stop-mejepa-self-verify.jsonl",
                "fsv_path": report_path.display().to_string()
            },
            "cases": {
                "fail_prediction_blocks_before_test": blocked_before,
                "posttooluse_verification_row_written": {
                    "verifications_written": bind.verifications_written,
                    "agreements": bind.rows.iter().map(|row| row.agreement.clone()).collect::<Vec<_>>()
                },
                "same_prediction_allows_after_confirmed_verification": allowed_after,
                "all_abstain_allows": abstain,
                "fifty_unverified_lists_all_predictions_truncates_commands": bulk
            }
        });
        fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        assert!(all_passed, "FSV report: {}", report_path.display());
    }
}
