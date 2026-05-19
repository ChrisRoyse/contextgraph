//! Reward-signal completeness audit and fail-closed signal-drop log.
//!
//! This module is deliberately storage-first: every audit claim is derived from
//! RocksDB column-family presence/counts, and every skipped reward computation
//! can be persisted as a `SignalDropLogEntry` in `CF_MEJEPA_SIGNAL_DROP_LOG`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use context_graph_mejepa_cf::{
    all_hygiene_referenced_cfs, CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_AGENT_FEEDBACK,
    CF_MEJEPA_CHURN_SIGNALS, CF_MEJEPA_COMPLEXITY_SIGNALS, CF_MEJEPA_CONSTELLATION,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG, CF_MEJEPA_COVERAGE_SIGNALS, CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS, CF_MEJEPA_DOC_COVERAGE_SIGNALS, CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_CALIBRATION, CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_LIVE_SESSION_SIGNALS, CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
    CF_MEJEPA_OPERATOR_OVERRIDES, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_PANELS,
    CF_MEJEPA_PREDICTION_VERIFICATIONS, CF_MEJEPA_Q4_ACCURACY_LABELS, CF_MEJEPA_Q4_COST_LABELS,
    CF_MEJEPA_Q4_PERF_LABELS, CF_MEJEPA_Q4_SECURITY_LABELS, CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_SIGNAL_DROP_LOG, CF_MEJEPA_STATIC_ANALYSIS_SIGNALS, CF_MEJEPA_TELEMETRY_SIGNALS,
    CF_MEJEPA_TYPE_GRAPH_SIGNALS, CF_MEJEPA_WITNESS_CHAIN,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;

pub const REWARD_SIGNAL_AUDIT_SCHEMA_VERSION: u32 = 1;
pub const REWARD_SIGNAL_TIER_COUNT: u8 = 8;
pub const SIGNAL_DROP_LOG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDropSeverity {
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignalDropLogEntry {
    pub schema_version: u32,
    pub event_id: [u8; 32],
    pub occurred_at_unix_ms: i64,
    pub tier: u8,
    pub signal_name: String,
    pub source_stage: String,
    pub artifact_id: String,
    pub error_code: String,
    pub error_detail: String,
    pub fail_closed: bool,
    pub severity: SignalDropSeverity,
    pub recovery_hint: String,
    pub input_sha256: Option<[u8; 32]>,
    pub context: BTreeMap<String, String>,
}

impl SignalDropLogEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        occurred_at_unix_ms: i64,
        tier: u8,
        signal_name: impl Into<String>,
        source_stage: impl Into<String>,
        artifact_id: impl Into<String>,
        error_code: impl Into<String>,
        error_detail: impl Into<String>,
        severity: SignalDropSeverity,
        recovery_hint: impl Into<String>,
        input_sha256: Option<[u8; 32]>,
        context: BTreeMap<String, String>,
    ) -> Result<Self, MejepaInferError> {
        let signal_name = signal_name.into();
        let source_stage = source_stage.into();
        let artifact_id = artifact_id.into();
        let error_code = error_code.into();
        let error_detail = error_detail.into();
        let recovery_hint = recovery_hint.into();
        let event_id = signal_drop_event_id(
            occurred_at_unix_ms,
            tier,
            &signal_name,
            &source_stage,
            &artifact_id,
            &error_code,
            input_sha256.as_ref(),
        );
        let entry = Self {
            schema_version: SIGNAL_DROP_LOG_SCHEMA_VERSION,
            event_id,
            occurred_at_unix_ms,
            tier,
            signal_name,
            source_stage,
            artifact_id,
            error_code,
            error_detail,
            fail_closed: true,
            severity,
            recovery_hint,
            input_sha256,
            context,
        };
        entry.validate()?;
        Ok(entry)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != SIGNAL_DROP_LOG_SCHEMA_VERSION {
            return invalid_input(
                "signal_drop.schema_version",
                format!(
                    "expected {SIGNAL_DROP_LOG_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        if self.event_id.iter().all(|byte| *byte == 0) {
            return invalid_input("signal_drop.event_id", "event id must be non-zero");
        }
        if self.occurred_at_unix_ms <= 0 {
            return invalid_input(
                "signal_drop.occurred_at_unix_ms",
                "timestamp must be positive",
            );
        }
        validate_tier(self.tier)?;
        validate_text("signal_drop.signal_name", &self.signal_name, 256)?;
        validate_text("signal_drop.source_stage", &self.source_stage, 128)?;
        validate_text("signal_drop.artifact_id", &self.artifact_id, 512)?;
        validate_text("signal_drop.error_code", &self.error_code, 128)?;
        validate_text("signal_drop.error_detail", &self.error_detail, 2048)?;
        validate_text("signal_drop.recovery_hint", &self.recovery_hint, 512)?;
        if !self.fail_closed {
            return invalid_input(
                "signal_drop.fail_closed",
                "signal drops must be logged only after the failing path has failed closed",
            );
        }
        for (key, value) in &self.context {
            validate_text("signal_drop.context.key", key, 128)?;
            validate_text("signal_drop.context.value", value, 1024)?;
        }
        Ok(())
    }

    pub fn event_id_hex(&self) -> String {
        hex::encode(self.event_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardSignalEvidence {
    pub tier: u8,
    pub signal: String,
    pub source: String,
    pub storage: String,
    pub required_cfs: Vec<String>,
    pub missing_cfs: Vec<String>,
    pub row_count: u64,
    pub captured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardSignalTierCoverage {
    pub tier: u8,
    pub captured_signal_count: usize,
    pub total_signal_count: usize,
    pub coverage_ratio: f32,
    pub passed: bool,
    pub signals: Vec<RewardSignalEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintFeatureSpanStatus {
    pub catalog_cf: String,
    pub catalog_row_count: u64,
    pub feature_tiers_present: Vec<u8>,
    pub spans_all_eight_tiers: bool,
    pub join_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardSignalLoopStatus {
    pub replay_buffer_ready: bool,
    pub replay_buffer_rows: u64,
    pub operator_override_multiplier: u8,
    pub active_learning_ready: bool,
    pub active_learning_queue_rows: u64,
    pub prediction_verification_ready: bool,
    pub prediction_verification_rows: u64,
    pub ontology_growth_rows: u64,
    pub constellation_freshness_ready: bool,
    pub constellation_refresh_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalDropLogStatus {
    pub cf: String,
    pub registered: bool,
    pub row_count: u64,
    pub sample_limit: usize,
    pub samples: Vec<SignalDropLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardSignalAuditReport {
    pub schema_version: u32,
    pub min_coverage: f32,
    pub acceptance_passed: bool,
    pub overall_coverage_ratio: f32,
    pub total_signal_count: usize,
    pub captured_signal_count: usize,
    pub per_tier: Vec<RewardSignalTierCoverage>,
    pub fingerprint_feature_span: FingerprintFeatureSpanStatus,
    pub lifelong_learning: RewardSignalLoopStatus,
    pub signal_drop_log: SignalDropLogStatus,
    pub source_of_truth_cfs: Vec<String>,
}

#[derive(Clone, Copy)]
struct SignalDefinition {
    tier: u8,
    signal: &'static str,
    source: &'static str,
    storage: &'static str,
    required_cfs: &'static [&'static str],
}

macro_rules! signal {
    ($tier:literal, $name:literal, $source:literal, $storage:literal, [$($cf:expr),+ $(,)?]) => {
        SignalDefinition {
            tier: $tier,
            signal: $name,
            source: $source,
            storage: $storage,
            required_cfs: &[$($cf),+],
        }
    };
}

pub fn open_reward_signal_audit_rocksdb(
    path: impl AsRef<Path>,
) -> Result<Arc<DB>, MejepaInferError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(
        &opts,
        path.as_ref(),
        descriptors,
    )?))
}

pub fn persist_signal_drop_log_entry(
    db: &DB,
    entry: &SignalDropLogEntry,
) -> Result<(), MejepaInferError> {
    entry.validate()?;
    let cf = cf(db, CF_MEJEPA_SIGNAL_DROP_LOG)?;
    let key = signal_drop_key(entry);
    let bytes = bincode::serialize(entry)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &bytes, &opts)?;
    let readback = db
        .get_cf(cf, &key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "signal_drop_log.readback".to_string(),
            detail: "missing signal-drop readback after sync write".to_string(),
        })?;
    if readback != bytes {
        return invalid_input(
            "signal_drop_log.readback",
            "readback bytes differ from written signal-drop row",
        );
    }
    let decoded: SignalDropLogEntry = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *entry {
        return invalid_input(
            "signal_drop_log.readback",
            "decoded signal-drop row differs from input",
        );
    }
    Ok(())
}

pub fn load_signal_drop_log_entries(
    db: &DB,
    limit: usize,
) -> Result<Vec<SignalDropLogEntry>, MejepaInferError> {
    if limit > 1000 {
        return invalid_input("signal_drop.limit", "limit must be <= 1000");
    }
    let cf = cf(db, CF_MEJEPA_SIGNAL_DROP_LOG)?;
    let mut entries = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        if entries.len() >= limit {
            break;
        }
        let (_key, value) = item?;
        let entry: SignalDropLogEntry = bincode::deserialize(&value)?;
        entry.validate()?;
        entries.push(entry);
    }
    Ok(entries)
}

pub fn reward_signal_definitions() -> Vec<RewardSignalEvidence> {
    reward_signal_definition_table()
        .into_iter()
        .map(|definition| RewardSignalEvidence {
            tier: definition.tier,
            signal: definition.signal.to_string(),
            source: definition.source.to_string(),
            storage: definition.storage.to_string(),
            required_cfs: definition
                .required_cfs
                .iter()
                .map(|cf| (*cf).to_string())
                .collect(),
            missing_cfs: Vec::new(),
            row_count: 0,
            captured: false,
        })
        .collect()
}

pub fn audit_reward_signals(
    db: &DB,
    min_coverage: f32,
    signal_drop_sample_limit: usize,
) -> Result<RewardSignalAuditReport, MejepaInferError> {
    if !min_coverage.is_finite() || !(0.0..=1.0).contains(&min_coverage) {
        return invalid_input("reward_signal_audit.min_coverage", "must be in [0,1]");
    }
    if signal_drop_sample_limit > 1000 {
        return invalid_input(
            "reward_signal_audit.signal_drop_sample_limit",
            "must be <= 1000",
        );
    }

    let definitions = reward_signal_definition_table();
    let mut count_cache = BTreeMap::<String, Option<u64>>::new();
    let mut evidence_by_tier = BTreeMap::<u8, Vec<RewardSignalEvidence>>::new();
    for definition in definitions {
        let mut missing_cfs = Vec::new();
        let mut row_count = 0u64;
        for cf_name in definition.required_cfs {
            let count = match count_cache.get(*cf_name) {
                Some(count) => *count,
                None => {
                    let count = count_cf_optional(db, cf_name)?;
                    count_cache.insert((*cf_name).to_string(), count);
                    count
                }
            };
            match count {
                Some(count) => row_count = row_count.saturating_add(count),
                None => missing_cfs.push((*cf_name).to_string()),
            }
        }
        let captured = missing_cfs.is_empty() && row_count > 0;
        evidence_by_tier
            .entry(definition.tier)
            .or_default()
            .push(RewardSignalEvidence {
                tier: definition.tier,
                signal: definition.signal.to_string(),
                source: definition.source.to_string(),
                storage: definition.storage.to_string(),
                required_cfs: definition
                    .required_cfs
                    .iter()
                    .map(|cf| (*cf).to_string())
                    .collect(),
                missing_cfs,
                row_count,
                captured,
            });
    }

    let mut per_tier = Vec::new();
    for tier in 1..=REWARD_SIGNAL_TIER_COUNT {
        let signals = evidence_by_tier.remove(&tier).unwrap_or_default();
        let total = signals.len();
        let captured = signals.iter().filter(|signal| signal.captured).count();
        let coverage_ratio = if total == 0 {
            0.0
        } else {
            captured as f32 / total as f32
        };
        per_tier.push(RewardSignalTierCoverage {
            tier,
            captured_signal_count: captured,
            total_signal_count: total,
            coverage_ratio,
            passed: total > 0 && coverage_ratio >= min_coverage,
            signals,
        });
    }

    let total_signal_count = per_tier
        .iter()
        .map(|tier| tier.total_signal_count)
        .sum::<usize>();
    let captured_signal_count = per_tier
        .iter()
        .map(|tier| tier.captured_signal_count)
        .sum::<usize>();
    let overall_coverage_ratio = if total_signal_count == 0 {
        0.0
    } else {
        captured_signal_count as f32 / total_signal_count as f32
    };

    let captured_tiers = per_tier
        .iter()
        .filter(|tier| tier.captured_signal_count == tier.total_signal_count)
        .map(|tier| tier.tier)
        .collect::<Vec<_>>();
    let fingerprint_feature_span = FingerprintFeatureSpanStatus {
        catalog_cf: CF_MEJEPA_FAILURE_FINGERPRINTS.to_string(),
        catalog_row_count: count_cf_optional(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?.unwrap_or(0),
        feature_tiers_present: captured_tiers.clone(),
        spans_all_eight_tiers: captured_tiers == (1..=REWARD_SIGNAL_TIER_COUNT).collect::<Vec<_>>()
            && db.cf_handle(CF_MEJEPA_FAILURE_FINGERPRINTS).is_some(),
        join_strategy:
            "fingerprint references join CF_MEJEPA_FAILURE_FINGERPRINTS to tiered reward CFs"
                .to_string(),
    };

    let lifelong_learning = RewardSignalLoopStatus {
        replay_buffer_ready: db.cf_handle(CF_MEJEPA_REPLAY_BUFFER).is_some(),
        replay_buffer_rows: count_cf_optional(db, CF_MEJEPA_REPLAY_BUFFER)?.unwrap_or(0),
        operator_override_multiplier: 6,
        active_learning_ready: db.cf_handle(CF_MEJEPA_ACTIVE_LEARNING_QUEUE).is_some()
            && db.cf_handle(CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT).is_some(),
        active_learning_queue_rows: count_cf_optional(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?
            .unwrap_or(0),
        prediction_verification_ready: db.cf_handle(CF_MEJEPA_PREDICTION_VERIFICATIONS).is_some(),
        prediction_verification_rows: count_cf_optional(db, CF_MEJEPA_PREDICTION_VERIFICATIONS)?
            .unwrap_or(0),
        ontology_growth_rows: count_cf_optional(db, CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT)?.unwrap_or(0),
        constellation_freshness_ready: db.cf_handle(CF_MEJEPA_CONSTELLATION_REFRESH_LOG).is_some(),
        constellation_refresh_rows: count_cf_optional(db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)?
            .unwrap_or(0),
    };

    let signal_drop_registered = db.cf_handle(CF_MEJEPA_SIGNAL_DROP_LOG).is_some();
    let signal_drop_log = SignalDropLogStatus {
        cf: CF_MEJEPA_SIGNAL_DROP_LOG.to_string(),
        registered: signal_drop_registered,
        row_count: count_cf_optional(db, CF_MEJEPA_SIGNAL_DROP_LOG)?.unwrap_or(0),
        sample_limit: signal_drop_sample_limit,
        samples: if signal_drop_registered && signal_drop_sample_limit > 0 {
            load_signal_drop_log_entries(db, signal_drop_sample_limit)?
        } else {
            Vec::new()
        },
    };

    let acceptance_passed = per_tier.iter().all(|tier| tier.passed)
        && fingerprint_feature_span.spans_all_eight_tiers
        && lifelong_learning.replay_buffer_ready
        && lifelong_learning.active_learning_ready
        && lifelong_learning.prediction_verification_ready
        && lifelong_learning.constellation_freshness_ready
        && signal_drop_log.registered;

    let source_of_truth_cfs = source_of_truth_cfs(&per_tier);
    Ok(RewardSignalAuditReport {
        schema_version: REWARD_SIGNAL_AUDIT_SCHEMA_VERSION,
        min_coverage,
        acceptance_passed,
        overall_coverage_ratio,
        total_signal_count,
        captured_signal_count,
        per_tier,
        fingerprint_feature_span,
        lifelong_learning,
        signal_drop_log,
        source_of_truth_cfs,
    })
}

pub fn signal_drop_key(entry: &SignalDropLogEntry) -> Vec<u8> {
    format!(
        "{:020}::{}",
        entry.occurred_at_unix_ms,
        entry.event_id_hex()
    )
    .into_bytes()
}

fn reward_signal_definition_table() -> Vec<SignalDefinition> {
    vec![
        signal!(
            1,
            "docker_oracle_pass_fail",
            "SWE-bench harness",
            "E_Oracle / CF_MEJEPA_ORACLE_VERDICTS",
            [CF_MEJEPA_ORACLE_VERDICTS]
        ),
        signal!(
            1,
            "swebench_test_sets",
            "SWE-bench report.json",
            "E_Oracle / CF_MEJEPA_ORACLE_VERDICTS",
            [CF_MEJEPA_ORACLE_VERDICTS]
        ),
        signal!(
            1,
            "exception_class",
            "SWE-bench harness",
            "E_Oracle / CF_MEJEPA_ORACLE_VERDICTS",
            [CF_MEJEPA_ORACLE_VERDICTS]
        ),
        signal!(
            1,
            "post_tool_test_outcomes",
            "PostToolUse Bash hook",
            "CF_MEJEPA_ORACLE_VERDICTS",
            [CF_MEJEPA_ORACLE_VERDICTS]
        ),
        signal!(
            1,
            "operator_override",
            "mejepa_record_agent_feedback / override MCP",
            "CF_MEJEPA_OPERATOR_OVERRIDES",
            [CF_MEJEPA_OPERATOR_OVERRIDES]
        ),
        signal!(
            2,
            "per_embedder_cosine_to_centroid",
            "DDA pipeline",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            2,
            "pairwise_embedder_cosine",
            "DDA pipeline",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            2,
            "pairwise_mutual_information",
            "DDA MI audit",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            2,
            "blind_spot_z_scores",
            "DDA pipeline",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            2,
            "embedder_agreement_vectors",
            "DDA pipeline",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            3,
            "ast_chunk_metadata",
            "python_ast_chunks.py",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            3,
            "file_sha_before_after",
            "Pre/PostToolUse hooks",
            "CF_MEJEPA_WITNESS_CHAIN",
            [CF_MEJEPA_WITNESS_CHAIN]
        ),
        signal!(
            3,
            "ast_diff",
            "python_diff_chunk_map_core.py",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            3,
            "static_data_flow",
            "python_static_graph.py",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            3,
            "type_graph_call_sites",
            "E_TypeGraph",
            "CF_MEJEPA_TYPE_GRAPH_SIGNALS",
            [CF_MEJEPA_TYPE_GRAPH_SIGNALS]
        ),
        signal!(
            3,
            "import_graph_deltas",
            "python_static_graph.py",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            3,
            "call_graph_reachability",
            "python_static_graph.py",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            3,
            "diff_chunk_mapping",
            "python_diff_chunk_map",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            4,
            "cyclomatic_complexity_delta",
            "E_Cyclomatic",
            "CF_MEJEPA_COMPLEXITY_SIGNALS",
            [CF_MEJEPA_COMPLEXITY_SIGNALS]
        ),
        signal!(
            4,
            "cognitive_complexity_halstead",
            "E_Cyclomatic",
            "CF_MEJEPA_COMPLEXITY_SIGNALS",
            [CF_MEJEPA_COMPLEXITY_SIGNALS]
        ),
        signal!(
            4,
            "depth_of_nesting_delta",
            "E_Cyclomatic",
            "CF_MEJEPA_COMPLEXITY_SIGNALS",
            [CF_MEJEPA_COMPLEXITY_SIGNALS]
        ),
        signal!(
            4,
            "line_coverage_delta",
            "E_Coverage",
            "CF_MEJEPA_COVERAGE_SIGNALS",
            [CF_MEJEPA_COVERAGE_SIGNALS]
        ),
        signal!(
            4,
            "diff_topology",
            "E_DiffTopology",
            "CF_MEJEPA_PANELS",
            [CF_MEJEPA_PANELS]
        ),
        signal!(
            4,
            "static_analysis_mypy_pyright_ruff",
            "static analysis reward runner",
            "CF_MEJEPA_STATIC_ANALYSIS_SIGNALS",
            [CF_MEJEPA_STATIC_ANALYSIS_SIGNALS]
        ),
        signal!(
            4,
            "security_bandit_semgrep",
            "E_SecuritySignals",
            "CF_MEJEPA_Q4_SECURITY_LABELS",
            [CF_MEJEPA_Q4_SECURITY_LABELS]
        ),
        signal!(
            4,
            "perf_pytest_benchmark_cprofile",
            "Q4 perf label producer",
            "CF_MEJEPA_Q4_PERF_LABELS",
            [CF_MEJEPA_Q4_PERF_LABELS]
        ),
        signal!(
            4,
            "accuracy_data_metric_delta",
            "Q4 accuracy label producer",
            "CF_MEJEPA_Q4_ACCURACY_LABELS",
            [CF_MEJEPA_Q4_ACCURACY_LABELS]
        ),
        signal!(
            4,
            "cost_ci_dependency_wheel_delta",
            "Q4 cost label producer",
            "CF_MEJEPA_Q4_COST_LABELS",
            [CF_MEJEPA_Q4_COST_LABELS]
        ),
        signal!(
            4,
            "dependency_cve_count",
            "local advisory-feed reward signal",
            "CF_MEJEPA_DEP_CVE_SIGNALS",
            [CF_MEJEPA_DEP_CVE_SIGNALS]
        ),
        signal!(
            4,
            "git_churn",
            "git blame/log analyzer",
            "CF_MEJEPA_CHURN_SIGNALS",
            [CF_MEJEPA_CHURN_SIGNALS]
        ),
        signal!(
            4,
            "documentation_delta",
            "Ruff docstring analyzer",
            "CF_MEJEPA_DOC_COVERAGE_SIGNALS",
            [CF_MEJEPA_DOC_COVERAGE_SIGNALS]
        ),
        signal!(
            5,
            "predicted_latency_delta",
            "E_Telemetry",
            "CF_MEJEPA_TELEMETRY_SIGNALS",
            [CF_MEJEPA_TELEMETRY_SIGNALS]
        ),
        signal!(
            5,
            "predicted_vram_delta",
            "E_Telemetry",
            "CF_MEJEPA_TELEMETRY_SIGNALS",
            [CF_MEJEPA_TELEMETRY_SIGNALS]
        ),
        signal!(
            5,
            "predicted_cost_delta",
            "E_Telemetry",
            "CF_MEJEPA_TELEMETRY_SIGNALS",
            [CF_MEJEPA_TELEMETRY_SIGNALS]
        ),
        signal!(
            5,
            "per_symbol_call_frequency",
            "E_Telemetry",
            "CF_MEJEPA_TELEMETRY_SIGNALS",
            [CF_MEJEPA_TELEMETRY_SIGNALS]
        ),
        signal!(
            5,
            "big_o_signature_shift",
            "E_Telemetry",
            "CF_MEJEPA_TELEMETRY_SIGNALS",
            [CF_MEJEPA_TELEMETRY_SIGNALS]
        ),
        signal!(
            6,
            "per_cell_centroid_variance",
            "TCT builder",
            "CF_MEJEPA_CONSTELLATION",
            [CF_MEJEPA_CONSTELLATION]
        ),
        signal!(
            6,
            "per_cell_gtau_threshold",
            "TCT calibrator",
            "CF_MEJEPA_CONSTELLATION",
            [CF_MEJEPA_CONSTELLATION]
        ),
        signal!(
            6,
            "per_cell_oracle_outcome_distribution",
            "TCT",
            "CF_MEJEPA_ORACLE_VERDICTS",
            [CF_MEJEPA_ORACLE_VERDICTS]
        ),
        signal!(
            6,
            "per_cell_pairwise_mi",
            "TCT / DDA",
            "CF_MEJEPA_DDA_SIGNALS",
            [CF_MEJEPA_DDA_SIGNALS]
        ),
        signal!(
            6,
            "cell_membership_per_chunk",
            "TCT",
            "CF_MEJEPA_CONSTELLATION",
            [CF_MEJEPA_CONSTELLATION]
        ),
        signal!(
            6,
            "failure_fingerprint_catalog",
            "fingerprint builder",
            "CF_MEJEPA_FAILURE_FINGERPRINTS",
            [
                CF_MEJEPA_FAILURE_FINGERPRINTS,
                CF_MEJEPA_FINGERPRINT_REFERENCES
            ]
        ),
        signal!(
            6,
            "per_fingerprint_conformal_residuals",
            "fingerprint calibrator",
            "CF_MEJEPA_FINGERPRINT_CALIBRATION",
            [CF_MEJEPA_FINGERPRINT_CALIBRATION]
        ),
        signal!(
            6,
            "novel_pattern_detection",
            "active learning ontology gate",
            "CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT",
            [CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT]
        ),
        signal!(
            7,
            "session_state",
            "E_LiveSession",
            "CF_MEJEPA_LIVE_SESSION_SIGNALS",
            [CF_MEJEPA_LIVE_SESSION_SIGNALS]
        ),
        signal!(
            7,
            "prompt_history_session_context",
            "E_LiveSession",
            "CF_MEJEPA_LIVE_SESSION_SIGNALS",
            [CF_MEJEPA_LIVE_SESSION_SIGNALS]
        ),
        signal!(
            7,
            "unknown_ood_active_learning",
            "classifier OOD path",
            "CF_MEJEPA_ACTIVE_LEARNING_QUEUE",
            [CF_MEJEPA_ACTIVE_LEARNING_QUEUE]
        ),
        signal!(
            7,
            "prediction_actual_reconciliation",
            "PostToolUse prediction verification + agent/operator feedback",
            "CF_MEJEPA_PREDICTION_VERIFICATIONS + CF_MEJEPA_AGENT_FEEDBACK + CF_MEJEPA_OPERATOR_OVERRIDES",
            [
                CF_MEJEPA_PREDICTION_VERIFICATIONS,
                CF_MEJEPA_AGENT_FEEDBACK,
                CF_MEJEPA_OPERATOR_OVERRIDES
            ]
        ),
        signal!(
            7,
            "prioritized_replay_buffer",
            "BatchSampler M(t)",
            "CF_MEJEPA_REPLAY_BUFFER",
            [CF_MEJEPA_REPLAY_BUFFER]
        ),
        signal!(
            8,
            "witness_merkle_root",
            "E_Witness",
            "CF_MEJEPA_WITNESS_CHAIN",
            [CF_MEJEPA_WITNESS_CHAIN]
        ),
        signal!(
            8,
            "parent_commit_hash",
            "E_Witness",
            "CF_MEJEPA_WITNESS_CHAIN",
            [CF_MEJEPA_WITNESS_CHAIN]
        ),
        signal!(
            8,
            "author_timestamp",
            "E_Witness",
            "CF_MEJEPA_WITNESS_CHAIN",
            [CF_MEJEPA_WITNESS_CHAIN]
        ),
        signal!(
            8,
            "prediction_id",
            "RealityPrediction",
            "CF_MEJEPA_LIVE_PREDICTIONS",
            [CF_MEJEPA_LIVE_PREDICTIONS]
        ),
        signal!(
            8,
            "tool_use_id",
            "MCP/hooks",
            "CF_MEJEPA_AGENT_FEEDBACK",
            [CF_MEJEPA_AGENT_FEEDBACK]
        ),
    ]
}

fn source_of_truth_cfs(per_tier: &[RewardSignalTierCoverage]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for tier in per_tier {
        for signal in &tier.signals {
            for cf in &signal.required_cfs {
                set.insert(cf.clone());
            }
        }
    }
    set.insert(CF_MEJEPA_SIGNAL_DROP_LOG.to_string());
    set.into_iter().collect()
}

fn count_cf_optional(db: &DB, name: &str) -> Result<Option<u64>, MejepaInferError> {
    let Some(cf) = db.cf_handle(name) else {
        return Ok(None);
    };
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(Some(count))
}

fn cf<'a>(db: &'a DB, name: &str) -> Result<&'a rocksdb::ColumnFamily, MejepaInferError> {
    db.cf_handle(name)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "rocksdb.column_family".to_string(),
            detail: format!("missing column family {name}"),
        })
}

fn signal_drop_event_id(
    occurred_at_unix_ms: i64,
    tier: u8,
    signal_name: &str,
    source_stage: &str,
    artifact_id: &str,
    error_code: &str,
    input_sha256: Option<&[u8; 32]>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_SIGNAL_DROP_LOG_V1");
    hasher.update(occurred_at_unix_ms.to_be_bytes());
    hasher.update([tier]);
    for value in [signal_name, source_stage, artifact_id, error_code] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    if let Some(input_sha256) = input_sha256 {
        hasher.update(input_sha256);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn validate_tier(tier: u8) -> Result<(), MejepaInferError> {
    if !(1..=REWARD_SIGNAL_TIER_COUNT).contains(&tier) {
        return invalid_input(
            "signal_drop.tier",
            format!("tier must be in 1..={REWARD_SIGNAL_TIER_COUNT}"),
        );
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid_input(field, "must be non-empty");
    }
    if value.len() > max_len {
        return invalid_input(field, format!("exceeds {max_len} bytes"));
    }
    if value.chars().any(char::is_control) {
        return invalid_input(field, "must not contain control characters");
    }
    Ok(())
}

fn invalid_input<T>(
    field: impl Into<String>,
    detail: impl Into<String>,
) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_drop_requires_fail_closed() {
        let mut entry = SignalDropLogEntry::new(
            1,
            4,
            "static_analysis_mypy_pyright_ruff",
            "static_analysis",
            "row-1",
            "STATIC_ANALYSIS_RUNTIME_EXCEEDED",
            "runner exceeded its budget",
            SignalDropSeverity::Error,
            "quarantine row and inspect toolchain",
            None,
            BTreeMap::new(),
        )
        .unwrap();
        entry.fail_closed = false;
        assert!(entry.validate().is_err());
    }

    #[test]
    fn reward_signal_definitions_span_all_eight_tiers() {
        let tiers = reward_signal_definition_table()
            .into_iter()
            .map(|definition| definition.tier)
            .collect::<BTreeSet<_>>();
        assert_eq!(tiers, (1..=REWARD_SIGNAL_TIER_COUNT).collect());
    }
}
