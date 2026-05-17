//! Phase G failure-shape fingerprint MCP tools.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result as AnyhowResult};
use context_graph_mejepa::{
    classify_failure_fingerprint_observation, open_infer_rocksdb, read_fingerprint,
    read_fingerprint_dormancy, read_fingerprint_fisher, read_fingerprint_reference,
    write_fingerprint_audit_sync_readback, write_fingerprint_calibration_sync_readback,
    write_fingerprint_reference_sync_readback, write_fingerprint_sync_readback, ActiveLearningKind,
    ActiveLearningLabel, ActiveLearningQueueState, ChunkId, FailureModeClass,
    FailureShapeFingerprint, FingerprintAuditAction, FingerprintAuditEntry,
    FingerprintCalibrationRecord, FingerprintCalibrationState, FingerprintCbpReset,
    FingerprintClassifierConfig, FingerprintConfidence, FingerprintId, FingerprintKind,
    FingerprintReference, LabelMethod, MutationCategory, OracleOutcome, RocksDbEvalStore, TaskId,
    DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_LABELS, CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FINGERPRINT_AUDIT, CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_DORMANCY, CF_MEJEPA_FINGERPRINT_FISHER, CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
};
use rocksdb::{IteratorMode, DB};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";
const DEFAULT_LIST_LIMIT: usize = 1000;
const DEFAULT_REFERENCE_LIMIT: usize = 100;
const DEFAULT_CALIBRATION_LIMIT: usize = 20;
const DEFAULT_MIN_CLUSTER_SIZE: usize = 3;
const DEFAULT_CLUSTER_THRESHOLD: f32 = 0.98;
const DEFAULT_SOURCE_CORPUS: &str = "mcp-fingerprint-label-v1";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FingerprintKindFilter {
    KnownGood,
    KnownBad,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintListRequest {
    db_path: Option<PathBuf>,
    kind: Option<FingerprintKindFilter>,
    #[serde(default = "default_list_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintInspectRequest {
    db_path: Option<PathBuf>,
    fingerprint_id: String,
    #[serde(default = "default_reference_limit")]
    reference_limit: usize,
    #[serde(default = "default_calibration_limit")]
    calibration_limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintClassifyRequest {
    db_path: Option<PathBuf>,
    observation_by_embedder: BTreeMap<String, Vec<f32>>,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintSuggestNewRequest {
    db_path: Option<PathBuf>,
    #[serde(default = "default_min_cluster_size")]
    min_cluster_size: usize,
    #[serde(default = "default_cluster_threshold")]
    cosine_threshold: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LabelCatalogKind {
    Unknown,
    KnownGood,
    KnownBad,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintLabelRequest {
    db_path: Option<PathBuf>,
    candidate_id: String,
    oracle_outcome: OracleOutcome,
    #[serde(default = "default_label_method")]
    method: LabelMethod,
    operator_id: String,
    #[serde(default = "default_label_catalog_kind")]
    catalog_kind: LabelCatalogKind,
    name: Option<String>,
    repo: Option<String>,
    mutation_category: Option<String>,
    failure_mode: Option<String>,
    reference_chunk_id: String,
    reference_id: Option<String>,
    witness_hash: String,
    source_manifest_sha256: String,
    #[serde(default = "default_source_corpus")]
    source_corpus: String,
    tau_by_embedder: Option<BTreeMap<String, f32>>,
    #[serde(default)]
    allow_overwrite: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintPromoteCanonicalRequest {
    db_path: Option<PathBuf>,
    fingerprint_id: String,
    task_id: String,
    repo: String,
    mutation_category: String,
    chunk_id: String,
    reference_id: Option<String>,
    oracle_outcome: OracleOutcome,
    witness_hash: String,
    source_manifest_sha256: String,
    operator_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintRecalibrateRequest {
    db_path: Option<PathBuf>,
    fingerprint_id: String,
    tau_by_embedder: BTreeMap<String, f32>,
    #[serde(default = "default_same_session_band_percentile")]
    same_session_band_percentile: f32,
    sample_count: usize,
    operator_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FingerprintCatalogStabilityRequest {
    db_path: Option<PathBuf>,
    #[serde(default = "default_catalog_stability_window_limit")]
    window_limit: usize,
    #[serde(default = "default_max_catalog_stability_drift")]
    max_accuracy_drift: f32,
    #[serde(default = "default_max_catalog_stability_drift")]
    max_precision_drift: f32,
    #[serde(default = "default_dormancy_threshold")]
    dormancy_threshold: f32,
}

impl Handlers {
    pub(crate) async fn call_mejepa_fingerprint_list(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_LIST) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_list(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_LIST_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_inspect(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_INSPECT) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_inspect(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_INSPECT_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_classify(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_CLASSIFY) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_classify(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_CLASSIFY_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_suggest_new(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_SUGGEST_NEW) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_suggest_new(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_SUGGEST_NEW_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_label(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_LABEL) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_label(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_LABEL_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_promote_canonical(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request =
            match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_PROMOTE_CANONICAL) {
                Ok(value) => value,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            };
        match run_fingerprint_promote_canonical(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                self.fingerprint_error(id, "MEJEPA_FINGERPRINT_PROMOTE_CANONICAL_FAILED", err)
            }
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_recalibrate(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_RECALIBRATE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_fingerprint_recalibrate(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => self.fingerprint_error(id, "MEJEPA_FINGERPRINT_RECALIBRATE_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_fingerprint_catalog_stability(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request =
            match parse_tool_request(args, tool_names::MEJEPA_FINGERPRINT_CATALOG_STABILITY) {
                Ok(value) => value,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            };
        match run_fingerprint_catalog_stability(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                self.fingerprint_error(id, "MEJEPA_FINGERPRINT_CATALOG_STABILITY_FAILED", err)
            }
        }
    }

    fn fingerprint_error(
        &self,
        id: Option<JsonRpcId>,
        code: &'static str,
        err: anyhow::Error,
    ) -> JsonRpcResponse {
        self.tool_error_structured(
            id,
            ToolErrorKind::Storage,
            code,
            &err.to_string(),
            json!({"toolFamily": "mejepa_fingerprint"}),
        )
    }
}

fn run_fingerprint_list(request: FingerprintListRequest) -> AnyhowResult<Value> {
    ensure_limit("limit", request.limit, 10_000)?;
    let db_path = resolve_infer_db_path(request.db_path.clone())?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let mut catalog = read_catalog(db.as_ref()).context("read fingerprint catalog")?;
    catalog.retain(|fingerprint| match request.kind {
        Some(FingerprintKindFilter::KnownGood) => {
            matches!(fingerprint.kind, FingerprintKind::KnownGood { .. })
        }
        Some(FingerprintKindFilter::KnownBad) => {
            matches!(fingerprint.kind, FingerprintKind::KnownBad { .. })
        }
        Some(FingerprintKindFilter::Unknown) => {
            matches!(fingerprint.kind, FingerprintKind::Unknown { .. })
        }
        None => true,
    });
    catalog.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.fingerprint_id.cmp(&right.fingerprint_id))
    });
    let total_count = catalog.len();
    let fingerprints = catalog
        .iter()
        .take(request.limit)
        .map(fingerprint_summary_json)
        .collect::<Vec<_>>();
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_LIST,
        "catalogCount": total_count,
        "returnedCount": fingerprints.len(),
        "fingerprints": fingerprints,
        "sourceOfTruth": fingerprint_sot(&db_path)
    }))
}

fn run_fingerprint_inspect(request: FingerprintInspectRequest) -> AnyhowResult<Value> {
    ensure_limit("referenceLimit", request.reference_limit, 10_000)?;
    ensure_limit("calibrationLimit", request.calibration_limit, 1_000)?;
    let db_path = resolve_infer_db_path(request.db_path)?;
    let fingerprint_id = parse_fingerprint_id(&request.fingerprint_id)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let fingerprint = read_fingerprint(db.as_ref(), fingerprint_id)
        .context("read fingerprint")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_NOT_FOUND"))?;
    let references =
        scan_cf::<FingerprintReference>(db.as_ref(), CF_MEJEPA_FINGERPRINT_REFERENCES)?
            .into_iter()
            .filter(|reference| reference.fingerprint_id == fingerprint_id)
            .take(request.reference_limit)
            .collect::<Vec<_>>();
    let calibration_records =
        scan_cf::<FingerprintCalibrationRecord>(db.as_ref(), CF_MEJEPA_FINGERPRINT_CALIBRATION)?
            .into_iter()
            .filter(|record| record.fingerprint_id == fingerprint_id)
            .take(request.calibration_limit)
            .collect::<Vec<_>>();
    let audit_entries = scan_cf::<FingerprintAuditEntry>(db.as_ref(), CF_MEJEPA_FINGERPRINT_AUDIT)?
        .into_iter()
        .filter(|entry| entry.fingerprint_id == fingerprint_id)
        .collect::<Vec<_>>();
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_INSPECT,
        "fingerprint": fingerprint,
        "references": references,
        "calibrationRecords": calibration_records,
        "auditEntries": audit_entries,
        "sourceOfTruth": fingerprint_sot(&db_path)
    }))
}

fn run_fingerprint_classify(request: FingerprintClassifyRequest) -> AnyhowResult<Value> {
    ensure_limit("topK", request.top_k, 100)?;
    let db_path = resolve_infer_db_path(request.db_path)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let catalog = read_catalog(db.as_ref()).context("read fingerprint catalog")?;
    let observation_by_embedder = embedder_vector_map(request.observation_by_embedder);
    let classification = classify_failure_fingerprint_observation(
        &catalog,
        &observation_by_embedder,
        FingerprintClassifierConfig {
            top_k: request.top_k,
            ..FingerprintClassifierConfig::default()
        },
    )
    .context("classify fingerprint observation")?;
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_CLASSIFY,
        "classification": classification,
        "sourceOfTruth": fingerprint_sot(&db_path)
    }))
}

fn run_fingerprint_suggest_new(request: FingerprintSuggestNewRequest) -> AnyhowResult<Value> {
    let db_path = resolve_infer_db_path(request.db_path)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let eval_store = RocksDbEvalStore::new(db).context("construct eval store")?;
    let queue = eval_store
        .load_queue()
        .context("load active-learning queue")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_ACTIVE_LEARNING_QUEUE_MISSING"))?;
    let suggestions = queue
        .suggest_unknown_fingerprint_clusters(request.min_cluster_size, request.cosine_threshold)
        .context("suggest unknown fingerprint clusters")?;
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_SUGGEST_NEW,
        "suggestionCount": suggestions.len(),
        "suggestions": suggestions,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "queueCf": CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            "queueKeyHex": hex::encode(b"active")
        }
    }))
}

fn run_fingerprint_label(request: FingerprintLabelRequest) -> AnyhowResult<Value> {
    validate_operator("operatorId", &request.operator_id)?;
    let db_path = resolve_infer_db_path(request.db_path.clone())?;
    let candidate_id = parse_hex_array::<16>("candidateId", &request.candidate_id)?;
    let witness_hash = parse_hex_array::<32>("witnessHash", &request.witness_hash)?;
    let source_manifest_sha256 =
        parse_hex_array::<32>("sourceManifestSha256", &request.source_manifest_sha256)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let eval_store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    let queue = eval_store
        .load_queue()
        .context("load active-learning queue")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_ACTIVE_LEARNING_QUEUE_MISSING"))?;
    let candidate = find_unknown_candidate(&queue, candidate_id)
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_CANDIDATE_NOT_FOUND"))?;
    let now = chrono::Utc::now().timestamp_millis();
    let label = ActiveLearningLabel {
        task_id: candidate.task_id.clone(),
        oracle_outcome: request.oracle_outcome,
        method: request.method,
        labeled_at_unix_ms: now,
    };
    eval_store
        .persist_label(&label)
        .context("persist active-learning label")?;
    flush_cf(db.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_LABELS)?;

    let kind = label_catalog_kind(&request, candidate)?;
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, &request.source_corpus)
        .context("derive canonical fingerprint id")?;
    if !request.allow_overwrite && read_fingerprint(db.as_ref(), fingerprint_id)?.is_some() {
        bail!("MEJEPA_FINGERPRINT_ALREADY_EXISTS");
    }
    let reference_chunk = ChunkId(request.reference_chunk_id.clone());
    let tau_by_embedder = request
        .tau_by_embedder
        .map(embedder_scalar_map)
        .unwrap_or_else(|| default_tau_by_embedder(&candidate.observation_by_embedder));
    let fingerprint = FailureShapeFingerprint {
        schema_version: context_graph_mejepa::FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name: request.name.unwrap_or_else(|| {
            format!(
                "operator-labeled-unknown-{}",
                hex::encode(&candidate.candidate_id[..8])
            )
        }),
        source_corpus: request.source_corpus.clone(),
        source_manifest_sha256: Some(source_manifest_sha256),
        centroid_by_embedder: candidate.observation_by_embedder.clone(),
        variance_by_embedder: zero_variance_by_embedder(&candidate.observation_by_embedder),
        tau_by_embedder,
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: vec![reference_chunk.clone()],
        n_references: 1,
        oracle_outcome: match request.catalog_kind {
            LabelCatalogKind::KnownGood | LabelCatalogKind::KnownBad => {
                Some(request.oracle_outcome)
            }
            LabelCatalogKind::Unknown => None,
        },
        is_canonical: false,
        frozen_at_unix_ms: now,
        confidence: FingerprintConfidence::default(),
    };
    write_fingerprint_sync_readback(db.as_ref(), &fingerprint).context("write fingerprint")?;
    let reference = fingerprint_reference_from_request(FingerprintReferenceInput {
        fingerprint_id: fingerprint.fingerprint_id,
        fingerprint: &fingerprint,
        task_id: &candidate.task_id,
        repo: request
            .repo
            .as_deref()
            .unwrap_or("operator_labeled_unknown"),
        mutation_category_slug: request
            .mutation_category
            .as_deref()
            .unwrap_or(MutationCategory::KnownGood.slug()),
        chunk_id: &reference_chunk,
        reference_id: request
            .reference_id
            .as_deref()
            .unwrap_or("operator-labeled-reference"),
        oracle_outcome: request.oracle_outcome,
        witness_hash,
        source_manifest_sha256,
    })?;
    write_fingerprint_reference_sync_readback(db.as_ref(), &reference)
        .context("write fingerprint reference")?;
    let audit = audit_entry(
        fingerprint.fingerprint_id,
        FingerprintAuditAction::Created,
        &request.operator_id,
        now,
        "operator labeled Unknown/OOD candidate into fingerprint catalog",
    );
    write_fingerprint_audit_sync_readback(db.as_ref(), &audit)
        .context("write fingerprint audit")?;
    let label_readback = eval_store
        .load_label(&candidate.task_id)
        .context("load label readback")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_LABEL_READBACK_MISSING"))?;
    let fingerprint_readback = read_fingerprint(db.as_ref(), fingerprint.fingerprint_id)?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_READBACK_MISSING"))?;
    if label_readback.oracle_outcome != label.oracle_outcome || fingerprint_readback != fingerprint
    {
        bail!("MEJEPA_FINGERPRINT_LABEL_READBACK_MISMATCH");
    }
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_LABEL,
        "status": "recorded",
        "candidateId": hex::encode(candidate.candidate_id),
        "fingerprintId": fingerprint.fingerprint_id.hex(),
        "activeLearningLabel": label_readback,
        "fingerprint": fingerprint_readback,
        "reference": reference,
        "sourceOfTruth": fingerprint_write_sot(&db_path)
    }))
}

fn run_fingerprint_promote_canonical(
    request: FingerprintPromoteCanonicalRequest,
) -> AnyhowResult<Value> {
    validate_operator("operatorId", &request.operator_id)?;
    let db_path = resolve_infer_db_path(request.db_path)?;
    let fingerprint_id = parse_fingerprint_id(&request.fingerprint_id)?;
    let witness_hash = parse_hex_array::<32>("witnessHash", &request.witness_hash)?;
    let source_manifest_sha256 =
        parse_hex_array::<32>("sourceManifestSha256", &request.source_manifest_sha256)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let mut fingerprint = read_fingerprint(db.as_ref(), fingerprint_id)
        .context("read fingerprint")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_NOT_FOUND"))?;
    if let Some(expected) = fingerprint.oracle_outcome {
        if expected != request.oracle_outcome {
            bail!("MEJEPA_FINGERPRINT_ORACLE_OUTCOME_MISMATCH");
        }
    }
    let chunk_id = ChunkId(request.chunk_id.clone());
    let reference_id = request
        .reference_id
        .unwrap_or_else(|| format!("canonical-{}", chunk_id.0));
    let task_id = TaskId(request.task_id.clone());
    let reference = fingerprint_reference_from_request(FingerprintReferenceInput {
        fingerprint_id,
        fingerprint: &fingerprint,
        task_id: &task_id,
        repo: &request.repo,
        mutation_category_slug: &request.mutation_category,
        chunk_id: &chunk_id,
        reference_id: &reference_id,
        oracle_outcome: request.oracle_outcome,
        witness_hash,
        source_manifest_sha256,
    })?;
    if read_fingerprint_reference(db.as_ref(), fingerprint_id, &reference_id)?.is_none() {
        write_fingerprint_reference_sync_readback(db.as_ref(), &reference)
            .context("write canonical fingerprint reference")?;
    }
    if !fingerprint.reference_chunks.contains(&chunk_id) {
        fingerprint.reference_chunks.push(chunk_id);
        fingerprint.n_references = fingerprint.reference_chunks.len();
    }
    fingerprint.is_canonical = true;
    write_fingerprint_sync_readback(db.as_ref(), &fingerprint)
        .context("write canonical fingerprint")?;
    let now = chrono::Utc::now().timestamp_millis();
    let audit = audit_entry(
        fingerprint_id,
        FingerprintAuditAction::PromotedCanonical,
        &request.operator_id,
        now,
        "operator promoted fingerprint reference as canonical",
    );
    write_fingerprint_audit_sync_readback(db.as_ref(), &audit).context("write promote audit")?;
    let readback = read_fingerprint(db.as_ref(), fingerprint_id)?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_READBACK_MISSING"))?;
    if !readback.is_canonical || readback.n_references != fingerprint.n_references {
        bail!("MEJEPA_FINGERPRINT_PROMOTE_READBACK_MISMATCH");
    }
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_PROMOTE_CANONICAL,
        "status": "promoted",
        "fingerprintId": fingerprint_id.hex(),
        "fingerprint": readback,
        "reference": reference,
        "sourceOfTruth": fingerprint_write_sot(&db_path)
    }))
}

fn run_fingerprint_recalibrate(request: FingerprintRecalibrateRequest) -> AnyhowResult<Value> {
    validate_operator("operatorId", &request.operator_id)?;
    if request.sample_count == 0 {
        bail!("sampleCount must be greater than zero");
    }
    if !request.same_session_band_percentile.is_finite()
        || !(0.0..=1.0).contains(&request.same_session_band_percentile)
    {
        bail!("sameSessionBandPercentile must be in [0,1]");
    }
    let db_path = resolve_infer_db_path(request.db_path)?;
    let fingerprint_id = parse_fingerprint_id(&request.fingerprint_id)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let mut fingerprint = read_fingerprint(db.as_ref(), fingerprint_id)
        .context("read fingerprint")?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_NOT_FOUND"))?;
    let tau_by_embedder = embedder_scalar_map(request.tau_by_embedder);
    if tau_by_embedder.keys().collect::<Vec<_>>()
        != fingerprint.centroid_by_embedder.keys().collect::<Vec<_>>()
    {
        bail!("MEJEPA_FINGERPRINT_TAU_EMBEDDER_SET_MISMATCH");
    }
    let now = chrono::Utc::now().timestamp_millis();
    fingerprint.tau_by_embedder = tau_by_embedder.clone();
    fingerprint.confidence.calibration_state = FingerprintCalibrationState::Calibrated;
    fingerprint.confidence.calibration_observations = request.sample_count;
    write_fingerprint_sync_readback(db.as_ref(), &fingerprint)
        .context("write recalibrated fingerprint")?;
    let record = FingerprintCalibrationRecord {
        fingerprint_id,
        calibrated_at_unix_ms: now,
        tau_by_embedder,
        same_session_band_percentile: request.same_session_band_percentile,
        sample_count: request.sample_count,
    };
    write_fingerprint_calibration_sync_readback(db.as_ref(), &record)
        .context("write calibration record")?;
    let audit = audit_entry(
        fingerprint_id,
        FingerprintAuditAction::Calibrated,
        &request.operator_id,
        now,
        "operator forced per-fingerprint tau recalibration",
    );
    write_fingerprint_audit_sync_readback(db.as_ref(), &audit)
        .context("write calibration audit")?;
    let readback = read_fingerprint(db.as_ref(), fingerprint_id)?
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_READBACK_MISSING"))?;
    if readback.tau_by_embedder != record.tau_by_embedder {
        bail!("MEJEPA_FINGERPRINT_RECALIBRATE_READBACK_MISMATCH");
    }
    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_RECALIBRATE,
        "status": "recalibrated",
        "fingerprintId": fingerprint_id.hex(),
        "fingerprint": readback,
        "calibrationRecord": record,
        "sourceOfTruth": fingerprint_write_sot(&db_path)
    }))
}

fn run_fingerprint_catalog_stability(
    request: FingerprintCatalogStabilityRequest,
) -> AnyhowResult<Value> {
    ensure_limit("windowLimit", request.window_limit, 32)?;
    if request.window_limit < 2 {
        bail!("windowLimit must be at least 2");
    }
    validate_probability_arg("maxAccuracyDrift", request.max_accuracy_drift)?;
    validate_probability_arg("maxPrecisionDrift", request.max_precision_drift)?;
    validate_probability_arg("dormancyThreshold", request.dormancy_threshold)?;

    let db_path = resolve_infer_db_path(request.db_path)?;
    let db = open_infer_rocksdb(&db_path).context("open inference RocksDB")?;
    let eval_store = RocksDbEvalStore::new(db.clone()).context("construct eval store")?;
    let windows = eval_store
        .load_fingerprint_ship_gate_windows_chronological()
        .context("load fingerprint ship-gate windows")?;
    if windows.len() < request.window_limit {
        bail!(
            "MEJEPA_FINGERPRINT_STABILITY_INSUFFICIENT_WINDOWS: expected at least {} got {}",
            request.window_limit,
            windows.len()
        );
    }
    let recent = &windows[windows.len() - request.window_limit..];
    let latest = recent
        .last()
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_STABILITY_NO_WINDOWS"))?;
    let earliest = recent
        .first()
        .ok_or_else(|| anyhow!("MEJEPA_FINGERPRINT_STABILITY_NO_WINDOWS"))?;

    let mut entries = Vec::new();
    let mut blockers = Vec::new();
    for (fingerprint_id_hex, latest_metrics) in &latest.per_fingerprint {
        let fingerprint_id = parse_fingerprint_id(fingerprint_id_hex)?;
        let earliest_metrics = earliest.per_fingerprint.get(fingerprint_id_hex);
        let Some(earliest_metrics) = earliest_metrics else {
            blockers.push(format!(
                "{fingerprint_id_hex}: missing earliest-window metrics in {}",
                earliest.window_id
            ));
            entries.push(json!({
                "fingerprintId": fingerprint_id_hex,
                "stable": false,
                "blockers": ["missing_earliest_window_metrics"]
            }));
            continue;
        };
        let accuracy_drift = (latest_metrics.accuracy - earliest_metrics.accuracy).abs();
        let precision_drift = (latest_metrics.precision - earliest_metrics.precision).abs();
        let fisher = read_fingerprint_fisher(db.as_ref(), fingerprint_id)
            .context("read fingerprint Fisher snapshot")?;
        let fisher_present = match &fisher {
            Some(snapshot) => {
                snapshot
                    .validate()
                    .map_err(|err| anyhow!("invalid Fisher snapshot: {err}"))?;
                true
            }
            None => false,
        };
        let dormancy = read_fingerprint_dormancy(db.as_ref(), fingerprint_id)
            .context("read fingerprint dormancy snapshot")?;
        let (dormancy_present, cbp_reset_dimensions, cbp_protected_dimensions) = match &dormancy {
            Some(snapshot) => {
                snapshot
                    .validate()
                    .map_err(|err| anyhow!("invalid dormancy snapshot: {err}"))?;
                let plan = FingerprintCbpReset::plan(snapshot, request.dormancy_threshold)
                    .map_err(|err| anyhow!("invalid CBP reset plan: {err}"))?;
                (true, plan.reset_dimensions, plan.protected_dimensions)
            }
            None => (false, Vec::new(), Vec::new()),
        };
        let mut entry_blockers = Vec::new();
        if accuracy_drift > request.max_accuracy_drift {
            entry_blockers.push("accuracy_drift_exceeded");
        }
        if precision_drift > request.max_precision_drift {
            entry_blockers.push("precision_drift_exceeded");
        }
        if !fisher_present {
            entry_blockers.push("fisher_missing");
        }
        if !dormancy_present {
            entry_blockers.push("dormancy_missing");
        }
        let stable = entry_blockers.is_empty();
        if !stable {
            blockers.push(format!("{fingerprint_id_hex}:{}", entry_blockers.join(",")));
        }
        entries.push(json!({
            "fingerprintId": fingerprint_id_hex,
            "stable": stable,
            "accuracy": {
                "earliest": earliest_metrics.accuracy,
                "latest": latest_metrics.accuracy,
                "absoluteDrift": accuracy_drift,
                "maxAllowedDrift": request.max_accuracy_drift
            },
            "precision": {
                "earliest": earliest_metrics.precision,
                "latest": latest_metrics.precision,
                "absoluteDrift": precision_drift,
                "maxAllowedDrift": request.max_precision_drift
            },
            "fisherSnapshotPresent": fisher_present,
            "dormancySnapshotPresent": dormancy_present,
            "cbpResetDimensions": cbp_reset_dimensions,
            "cbpProtectedDimensions": cbp_protected_dimensions,
            "blockers": entry_blockers
        }));
    }

    Ok(json!({
        "tool": tool_names::MEJEPA_FINGERPRINT_CATALOG_STABILITY,
        "stable": blockers.is_empty(),
        "windowCount": recent.len(),
        "earliestWindowId": earliest.window_id,
        "latestWindowId": latest.window_id,
        "fingerprintCount": entries.len(),
        "fingerprints": entries,
        "blockers": blockers,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "shipGateWindowsCf": CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
            "fisherCf": CF_MEJEPA_FINGERPRINT_FISHER,
            "dormancyCf": CF_MEJEPA_FINGERPRINT_DORMANCY
        }
    }))
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

fn read_catalog(db: &DB) -> AnyhowResult<Vec<FailureShapeFingerprint>> {
    let cf = cf_handle(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let fingerprint: FailureShapeFingerprint = bincode::deserialize(&value)?;
        fingerprint.validate().map_err(|err| anyhow!("{err}"))?;
        out.push(fingerprint);
    }
    Ok(out)
}

fn scan_cf<T>(db: &DB, cf_name: &str) -> AnyhowResult<Vec<T>>
where
    T: DeserializeOwned + FingerprintValidatable,
{
    let cf = cf_handle(db, cf_name)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let decoded: T = bincode::deserialize(&value)?;
        decoded.validate_for_fingerprint_tool()?;
        out.push(decoded);
    }
    Ok(out)
}

trait FingerprintValidatable {
    fn validate_for_fingerprint_tool(&self) -> AnyhowResult<()>;
}

impl FingerprintValidatable for FingerprintReference {
    fn validate_for_fingerprint_tool(&self) -> AnyhowResult<()> {
        self.validate().map_err(|err| anyhow!("{err}"))
    }
}

impl FingerprintValidatable for FingerprintCalibrationRecord {
    fn validate_for_fingerprint_tool(&self) -> AnyhowResult<()> {
        self.validate().map_err(|err| anyhow!("{err}"))
    }
}

impl FingerprintValidatable for FingerprintAuditEntry {
    fn validate_for_fingerprint_tool(&self) -> AnyhowResult<()> {
        self.validate().map_err(|err| anyhow!("{err}"))
    }
}

fn fingerprint_summary_json(fingerprint: &FailureShapeFingerprint) -> Value {
    json!({
        "fingerprintId": fingerprint.fingerprint_id.hex(),
        "name": fingerprint.name,
        "kind": fingerprint_kind_slug(&fingerprint.kind),
        "kindDetail": fingerprint.kind,
        "sourceCorpus": fingerprint.source_corpus,
        "nReferences": fingerprint.n_references,
        "isCanonical": fingerprint.is_canonical,
        "calibrationState": fingerprint.confidence.calibration_state,
        "calibrationObservations": fingerprint.confidence.calibration_observations,
        "embedderCount": fingerprint.centroid_by_embedder.len(),
        "oracleOutcome": fingerprint.oracle_outcome
    })
}

fn fingerprint_kind_slug(kind: &FingerprintKind) -> &'static str {
    match kind {
        FingerprintKind::KnownGood { .. } => "known_good",
        FingerprintKind::KnownBad { .. } => "known_bad",
        FingerprintKind::Unknown { .. } => "unknown",
    }
}

fn embedder_vector_map(
    raw: BTreeMap<String, Vec<f32>>,
) -> BTreeMap<context_graph_mejepa::EmbedderId, Vec<f32>> {
    raw.into_iter()
        .map(|(embedder, vector)| (context_graph_mejepa::EmbedderId(embedder), vector))
        .collect()
}

fn embedder_scalar_map(
    raw: BTreeMap<String, f32>,
) -> BTreeMap<context_graph_mejepa::EmbedderId, f32> {
    raw.into_iter()
        .map(|(embedder, value)| (context_graph_mejepa::EmbedderId(embedder), value))
        .collect()
}

fn default_tau_by_embedder(
    observation: &BTreeMap<context_graph_mejepa::EmbedderId, Vec<f32>>,
) -> BTreeMap<context_graph_mejepa::EmbedderId, f32> {
    observation
        .keys()
        .cloned()
        .map(|embedder| (embedder, 1.0_f32))
        .collect()
}

fn zero_variance_by_embedder(
    observation: &BTreeMap<context_graph_mejepa::EmbedderId, Vec<f32>>,
) -> BTreeMap<context_graph_mejepa::EmbedderId, f32> {
    observation
        .keys()
        .cloned()
        .map(|embedder| (embedder, 0.0_f32))
        .collect()
}

fn find_unknown_candidate(
    queue: &ActiveLearningQueueState,
    candidate_id: [u8; 16],
) -> Option<&context_graph_mejepa::UnknownFingerprintCandidate> {
    queue.entries.values().find_map(|entry| match &entry.kind {
        ActiveLearningKind::UnknownFingerprint { candidate }
            if candidate.candidate_id == candidate_id =>
        {
            Some(candidate.as_ref())
        }
        _ => None,
    })
}

fn label_catalog_kind(
    request: &FingerprintLabelRequest,
    candidate: &context_graph_mejepa::UnknownFingerprintCandidate,
) -> AnyhowResult<FingerprintKind> {
    match request.catalog_kind {
        LabelCatalogKind::Unknown => Ok(FingerprintKind::Unknown {
            observed_at_unix_ms: candidate.observed_at_unix_ms,
            observed_by_session: candidate.session_id,
            ood_score: candidate.ood_score,
            embedder_disagreement_score: candidate.embedder_disagreement_score,
            active_learning_priority: candidate.active_learning_priority,
        }),
        LabelCatalogKind::KnownGood => {
            if request.oracle_outcome != OracleOutcome::Pass {
                bail!("known_good catalog labels require oracleOutcome=pass");
            }
            Ok(FingerprintKind::KnownGood {
                repo: request.repo.clone(),
                gold_patch_count: 1,
            })
        }
        LabelCatalogKind::KnownBad => {
            if request.oracle_outcome != OracleOutcome::Fail {
                bail!("known_bad catalog labels require oracleOutcome=fail");
            }
            let repo = request
                .repo
                .clone()
                .ok_or_else(|| anyhow!("repo is required for known_bad labels"))?;
            let mutation_category =
                mutation_category_from_slug(request.mutation_category.as_deref().ok_or_else(
                    || anyhow!("mutationCategory is required for known_bad labels"),
                )?)?;
            if mutation_category == MutationCategory::KnownGood {
                bail!("known_bad labels cannot use mutationCategory=known_good");
            }
            let failure_mode = failure_mode_from_slug(
                request
                    .failure_mode
                    .as_deref()
                    .ok_or_else(|| anyhow!("failureMode is required for known_bad labels"))?,
            )?;
            Ok(FingerprintKind::KnownBad {
                repo,
                mutation_category,
                failure_mode,
                exception_class: None,
            })
        }
    }
}

struct FingerprintReferenceInput<'a> {
    fingerprint_id: FingerprintId,
    fingerprint: &'a FailureShapeFingerprint,
    task_id: &'a TaskId,
    repo: &'a str,
    mutation_category_slug: &'a str,
    chunk_id: &'a ChunkId,
    reference_id: &'a str,
    oracle_outcome: OracleOutcome,
    witness_hash: [u8; 32],
    source_manifest_sha256: [u8; 32],
}

fn fingerprint_reference_from_request(
    input: FingerprintReferenceInput<'_>,
) -> AnyhowResult<FingerprintReference> {
    let FingerprintReferenceInput {
        fingerprint_id,
        fingerprint,
        task_id,
        repo,
        mutation_category_slug,
        chunk_id,
        reference_id,
        oracle_outcome,
        witness_hash,
        source_manifest_sha256,
    } = input;
    if let Some(expected) = fingerprint.oracle_outcome {
        if expected != oracle_outcome {
            bail!("reference oracleOutcome does not match fingerprint oracle_outcome");
        }
    }
    let mutation_category = mutation_category_from_slug(mutation_category_slug)?;
    let reference = FingerprintReference {
        fingerprint_id,
        reference_id: reference_id.to_string(),
        task_id: task_id.clone(),
        repo: repo.to_string(),
        mutation_category,
        chunk_id: chunk_id.clone(),
        embedder_ids: fingerprint.centroid_by_embedder.keys().cloned().collect(),
        oracle_outcome,
        witness_hash,
        source_manifest_sha256,
    };
    reference.validate().map_err(|err| anyhow!("{err}"))?;
    Ok(reference)
}

fn mutation_category_from_slug(slug: &str) -> AnyhowResult<MutationCategory> {
    MutationCategory::all()
        .into_iter()
        .find(|category| category.slug() == slug)
        .ok_or_else(|| anyhow!("unknown mutationCategory {slug}"))
}

fn failure_mode_from_slug(slug: &str) -> AnyhowResult<FailureModeClass> {
    FailureModeClass::all()
        .into_iter()
        .find(|mode| mode.slug() == slug)
        .ok_or_else(|| anyhow!("unknown failureMode {slug}"))
}

fn parse_fingerprint_id(raw: &str) -> AnyhowResult<FingerprintId> {
    Ok(FingerprintId(parse_hex_array::<32>("fingerprintId", raw)?))
}

fn parse_hex_array<const N: usize>(field: &str, raw: &str) -> AnyhowResult<[u8; N]> {
    let bytes = hex::decode(raw).with_context(|| format!("{field} must be valid hex"))?;
    if bytes.len() != N {
        bail!("{field} must decode to {N} bytes");
    }
    let mut out = [0_u8; N];
    out.copy_from_slice(&bytes);
    if out.iter().all(|byte| *byte == 0) {
        bail!("{field} must be non-zero");
    }
    Ok(out)
}

fn audit_entry(
    fingerprint_id: FingerprintId,
    action: FingerprintAuditAction,
    actor: &str,
    created_at_unix_ms: i64,
    detail: &str,
) -> FingerprintAuditEntry {
    FingerprintAuditEntry {
        fingerprint_id,
        action,
        actor: actor.to_string(),
        created_at_unix_ms,
        detail: detail.to_string(),
    }
}

fn cf_handle<'a>(db: &'a DB, name: &str) -> AnyhowResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| anyhow!("missing RocksDB column family {name}"))
}

fn flush_cf(db: &DB, name: &str) -> AnyhowResult<()> {
    db.flush_cf(cf_handle(db, name)?)
        .with_context(|| format!("flush {name}"))
}

fn ensure_limit(field: &str, value: usize, max: usize) -> AnyhowResult<()> {
    if value == 0 || value > max {
        bail!("{field} must be in 1..={max}");
    }
    Ok(())
}

fn validate_operator(field: &str, value: &str) -> AnyhowResult<()> {
    if value.trim().is_empty() {
        bail!("{field} must be non-empty");
    }
    if value.len() > 256 {
        bail!("{field} exceeds 256 bytes");
    }
    Ok(())
}

fn validate_probability_arg(field: &str, value: f32) -> AnyhowResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("{field} must be finite and in [0,1]");
    }
    Ok(())
}

fn fingerprint_sot(db_path: &Path) -> Value {
    json!({
        "dbPath": db_path.display().to_string(),
        "catalogCf": CF_MEJEPA_FAILURE_FINGERPRINTS
    })
}

fn fingerprint_write_sot(db_path: &Path) -> Value {
    json!({
        "dbPath": db_path.display().to_string(),
        "catalogCf": CF_MEJEPA_FAILURE_FINGERPRINTS,
        "referencesCf": CF_MEJEPA_FINGERPRINT_REFERENCES,
        "reverseIndexCf": CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
        "calibrationCf": CF_MEJEPA_FINGERPRINT_CALIBRATION,
        "auditCf": CF_MEJEPA_FINGERPRINT_AUDIT,
        "activeLearningLabelsCf": CF_MEJEPA_ACTIVE_LEARNING_LABELS
    })
}

fn default_list_limit() -> usize {
    DEFAULT_LIST_LIMIT
}

fn default_reference_limit() -> usize {
    DEFAULT_REFERENCE_LIMIT
}

fn default_calibration_limit() -> usize {
    DEFAULT_CALIBRATION_LIMIT
}

fn default_top_k() -> usize {
    context_graph_mejepa::DEFAULT_FINGERPRINT_MATCH_TOP_K
}

fn default_min_cluster_size() -> usize {
    DEFAULT_MIN_CLUSTER_SIZE
}

fn default_cluster_threshold() -> f32 {
    DEFAULT_CLUSTER_THRESHOLD
}

fn default_label_method() -> LabelMethod {
    LabelMethod::Human
}

fn default_label_catalog_kind() -> LabelCatalogKind {
    LabelCatalogKind::Unknown
}

fn default_source_corpus() -> String {
    DEFAULT_SOURCE_CORPUS.to_string()
}

fn default_same_session_band_percentile() -> f32 {
    0.10
}

fn default_catalog_stability_window_limit() -> usize {
    4
}

fn default_max_catalog_stability_drift() -> f32 {
    0.02
}

fn default_dormancy_threshold() -> f32 {
    DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD
}

#[cfg(test)]
pub(in crate::handlers::tools) fn run_fingerprint_mcp_tools_write_fsv_artifact() {
    test_support::fingerprint_mcp_tools_write_fsv_artifact();
}

#[cfg(test)]
mod test_support {
    use super::*;
    use context_graph_mejepa::ActiveLearningQueueEntry;
    use std::sync::Arc;
    use tempfile::TempDir;

    pub(super) fn fingerprint_mcp_tools_write_fsv_artifact() {
        let temp = TempDir::new().expect("tempdir");
        let db_path = temp.path().join("infer.rocksdb");
        let db = open_infer_rocksdb(&db_path).expect("open db");
        let known_good = seed_known_good(db.as_ref()).expect("seed known good");
        let queue = seed_unknown_queue(db.clone()).expect("seed queue");
        let candidate_id = match &queue.entries.values().next().expect("queue entry").kind {
            ActiveLearningKind::UnknownFingerprint { candidate } => candidate.candidate_id,
            _ => panic!("expected unknown fingerprint"),
        };
        flush_cf(db.as_ref(), CF_MEJEPA_FAILURE_FINGERPRINTS).expect("flush seeded catalog");
        flush_cf(db.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_QUEUE).expect("flush seeded queue");
        // RocksDB holds an in-process LOCK on the data dir; cancel background work
        // so the LOCK fcntl record is released before we re-open the same dir below.
        db.cancel_all_background_work(true);
        drop(db);
        std::thread::sleep(std::time::Duration::from_millis(100));

        let list = run_fingerprint_list(FingerprintListRequest {
            db_path: Some(db_path.clone()),
            kind: None,
            limit: 100,
        })
        .expect("list");
        let inspect = run_fingerprint_inspect(FingerprintInspectRequest {
            db_path: Some(db_path.clone()),
            fingerprint_id: known_good.fingerprint_id.hex(),
            reference_limit: 10,
            calibration_limit: 10,
        })
        .expect("inspect");
        let classify = run_fingerprint_classify(FingerprintClassifyRequest {
            db_path: Some(db_path.clone()),
            observation_by_embedder: BTreeMap::from([("e1".to_string(), vec![1.0, 0.0])]),
            top_k: 3,
        })
        .expect("classify");
        let suggest = run_fingerprint_suggest_new(FingerprintSuggestNewRequest {
            db_path: Some(db_path.clone()),
            min_cluster_size: 2,
            cosine_threshold: 0.95,
        })
        .expect("suggest");
        let label = run_fingerprint_label(FingerprintLabelRequest {
            db_path: Some(db_path.clone()),
            candidate_id: hex::encode(candidate_id),
            oracle_outcome: OracleOutcome::Pass,
            method: LabelMethod::Human,
            operator_id: "operator-fsv".to_string(),
            catalog_kind: LabelCatalogKind::Unknown,
            name: Some("operator-unknown-fsv".to_string()),
            repo: Some("operator/repo".to_string()),
            mutation_category: Some("known_good".to_string()),
            failure_mode: None,
            reference_chunk_id: "chunk-unknown-reference".to_string(),
            reference_id: Some("unknown-reference-1".to_string()),
            witness_hash: "11".repeat(32),
            source_manifest_sha256: "22".repeat(32),
            source_corpus: "mcp-fingerprint-fsv".to_string(),
            tau_by_embedder: None,
            allow_overwrite: false,
        })
        .expect("label");
        let promote = run_fingerprint_promote_canonical(FingerprintPromoteCanonicalRequest {
            db_path: Some(db_path.clone()),
            fingerprint_id: known_good.fingerprint_id.hex(),
            task_id: "task-known-good-canonical".to_string(),
            repo: "repo/fsv".to_string(),
            mutation_category: "known_good".to_string(),
            chunk_id: "chunk-known-good-canonical".to_string(),
            reference_id: Some("canonical-reference-1".to_string()),
            oracle_outcome: OracleOutcome::Pass,
            witness_hash: "33".repeat(32),
            source_manifest_sha256: "44".repeat(32),
            operator_id: "operator-fsv".to_string(),
        })
        .expect("promote");
        let recalibrate = run_fingerprint_recalibrate(FingerprintRecalibrateRequest {
            db_path: Some(db_path.clone()),
            fingerprint_id: known_good.fingerprint_id.hex(),
            tau_by_embedder: BTreeMap::from([("e1".to_string(), 0.75)]),
            same_session_band_percentile: 0.10,
            sample_count: 9,
            operator_id: "operator-fsv".to_string(),
        })
        .expect("recalibrate");
        seed_catalog_stability_rows(&db_path, known_good.fingerprint_id)
            .expect("seed catalog stability rows");
        let catalog_stability =
            run_fingerprint_catalog_stability(FingerprintCatalogStabilityRequest {
                db_path: Some(db_path.clone()),
                window_limit: 2,
                max_accuracy_drift: 0.02,
                max_precision_drift: 0.02,
                dormancy_threshold: DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD,
            })
            .expect("catalog stability");

        let boundaries = vec![
            boundary_case(
                "inspect_missing_fingerprint",
                run_fingerprint_inspect(FingerprintInspectRequest {
                    db_path: Some(db_path.clone()),
                    fingerprint_id: "55".repeat(32),
                    reference_limit: 10,
                    calibration_limit: 10,
                })
                .is_err(),
            ),
            boundary_case(
                "classify_missing_embedder",
                run_fingerprint_classify(FingerprintClassifyRequest {
                    db_path: Some(db_path.clone()),
                    observation_by_embedder: BTreeMap::from([(
                        "missing".to_string(),
                        vec![1.0, 0.0],
                    )]),
                    top_k: 3,
                })
                .is_err(),
            ),
            boundary_case(
                "suggest_zero_min_cluster_size",
                run_fingerprint_suggest_new(FingerprintSuggestNewRequest {
                    db_path: Some(db_path.clone()),
                    min_cluster_size: 0,
                    cosine_threshold: 0.95,
                })
                .is_err(),
            ),
            boundary_case(
                "label_missing_candidate",
                run_fingerprint_label(FingerprintLabelRequest {
                    db_path: Some(db_path.clone()),
                    candidate_id: "66".repeat(16),
                    oracle_outcome: OracleOutcome::Abstain,
                    method: LabelMethod::Human,
                    operator_id: "operator-fsv".to_string(),
                    catalog_kind: LabelCatalogKind::Unknown,
                    name: Some("missing-candidate".to_string()),
                    repo: Some("operator/repo".to_string()),
                    mutation_category: Some("known_good".to_string()),
                    failure_mode: None,
                    reference_chunk_id: "chunk-missing".to_string(),
                    reference_id: None,
                    witness_hash: "77".repeat(32),
                    source_manifest_sha256: "88".repeat(32),
                    source_corpus: "mcp-fingerprint-fsv".to_string(),
                    tau_by_embedder: None,
                    allow_overwrite: false,
                })
                .is_err(),
            ),
        ];

        let reopened = open_infer_rocksdb(&db_path).expect("reopen db");
        let reopened_catalog = read_catalog(reopened.as_ref()).expect("reopened catalog");
        let all_passed = list["catalogCount"] == json!(1)
            && inspect["references"].as_array().expect("refs").len() == 1
            && classify["classification"]["verdict"] == json!("pass")
            && classify["classification"]["reason"] == json!("known_good_only")
            && suggest["suggestionCount"] == json!(1)
            && label["status"] == json!("recorded")
            && promote["fingerprint"]["is_canonical"] == json!(true)
            && recalibrate["fingerprint"]["confidence"]["calibration_state"] == json!("calibrated")
            && catalog_stability["stable"] == json!(true)
            && reopened_catalog.len() == 2
            && boundaries.iter().all(|case| case["passed"] == json!(true));

        let artifact = json!({
            "task_id": "TASK-FP-006",
            "tool_surface": [
                tool_names::MEJEPA_FINGERPRINT_LIST,
                tool_names::MEJEPA_FINGERPRINT_INSPECT,
                tool_names::MEJEPA_FINGERPRINT_CLASSIFY,
                tool_names::MEJEPA_FINGERPRINT_SUGGEST_NEW,
                tool_names::MEJEPA_FINGERPRINT_LABEL,
                tool_names::MEJEPA_FINGERPRINT_PROMOTE_CANONICAL,
                tool_names::MEJEPA_FINGERPRINT_RECALIBRATE,
                tool_names::MEJEPA_FINGERPRINT_CATALOG_STABILITY
            ],
            "source_of_truth": fingerprint_write_sot(&db_path),
            "trigger": "cargo test -p context-graph-mcp fingerprint_mcp_tools_write_fsv_artifact -- --nocapture",
            "happy_path": {
                "list": list,
                "inspect": inspect,
                "classify": classify,
                "suggestNew": suggest,
                "label": label,
                "promoteCanonical": promote,
                "recalibrate": recalibrate,
                "catalogStability": catalog_stability
            },
            "readback": {
                "reopenedCatalogCount": reopened_catalog.len(),
                "reopenedFingerprintIds": reopened_catalog.iter().map(|fingerprint| fingerprint.fingerprint_id.hex()).collect::<Vec<_>>()
            },
            "boundary_cases": boundaries,
            "all_passed": all_passed
        });
        let run_root = PathBuf::from(format!(
            "/mnt/d/contextgraph/fsv/phase-g-fingerprint-mcp-tools-fsv/run-{}",
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::create_dir_all(&run_root).expect("create fsv dir");
        let output = run_root.join("fingerprint_mcp_tools_fsv.json");
        std::fs::write(&output, serde_json::to_vec_pretty(&artifact).expect("json"))
            .expect("write fsv");
        assert!(all_passed, "FSV artifact: {}", output.display());
    }

    fn seed_known_good(db: &DB) -> AnyhowResult<FailureShapeFingerprint> {
        let kind = FingerprintKind::KnownGood {
            repo: Some("repo/fsv".to_string()),
            gold_patch_count: 1,
        };
        let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, "mcp-fingerprint-fsv")?;
        let fingerprint = FailureShapeFingerprint {
            schema_version: context_graph_mejepa::FAILURE_FINGERPRINT_SCHEMA_VERSION,
            fingerprint_id,
            kind,
            name: "known-good-fsv".to_string(),
            source_corpus: "mcp-fingerprint-fsv".to_string(),
            source_manifest_sha256: Some([2; 32]),
            centroid_by_embedder: BTreeMap::from([(
                context_graph_mejepa::EmbedderId("e1".to_string()),
                vec![1.0, 0.0],
            )]),
            variance_by_embedder: BTreeMap::from([(
                context_graph_mejepa::EmbedderId("e1".to_string()),
                0.0,
            )]),
            tau_by_embedder: BTreeMap::from([(
                context_graph_mejepa::EmbedderId("e1".to_string()),
                0.80,
            )]),
            pairwise_cosine: Vec::new(),
            pairwise_mutual_information: Vec::new(),
            reference_chunks: vec![ChunkId("chunk-known-good-reference".to_string())],
            n_references: 1,
            oracle_outcome: Some(OracleOutcome::Pass),
            is_canonical: true,
            frozen_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            confidence: FingerprintConfidence {
                classification_accuracy: None,
                classification_precision: None,
                unknown_recall: None,
                calibration_observations: 7,
                calibration_state: FingerprintCalibrationState::Calibrated,
            },
        };
        write_fingerprint_sync_readback(db, &fingerprint)?;
        let reference = FingerprintReference {
            fingerprint_id,
            reference_id: "known-good-reference-1".to_string(),
            task_id: TaskId("task-known-good".to_string()),
            repo: "repo/fsv".to_string(),
            mutation_category: MutationCategory::KnownGood,
            chunk_id: ChunkId("chunk-known-good-reference".to_string()),
            embedder_ids: fingerprint.centroid_by_embedder.keys().cloned().collect(),
            oracle_outcome: OracleOutcome::Pass,
            witness_hash: [3; 32],
            source_manifest_sha256: [4; 32],
        };
        write_fingerprint_reference_sync_readback(db, &reference)?;
        Ok(fingerprint)
    }

    fn seed_unknown_queue(db: Arc<DB>) -> AnyhowResult<ActiveLearningQueueState> {
        let mut queue = ActiveLearningQueueState::new(10)?;
        queue.entries.insert(
            TaskId("unknown-task-1".to_string()),
            unknown_entry(1, "unknown-task-1", vec![0.0, 1.0]),
        );
        queue.entries.insert(
            TaskId("unknown-task-2".to_string()),
            unknown_entry(2, "unknown-task-2", vec![0.0, 0.98]),
        );
        RocksDbEvalStore::new(db)?.persist_queue(&queue)?;
        Ok(queue)
    }

    fn seed_catalog_stability_rows(
        db_path: &Path,
        fingerprint_id: FingerprintId,
    ) -> AnyhowResult<()> {
        let db = open_infer_rocksdb(db_path)?;
        let store = RocksDbEvalStore::new(db.clone())?;
        let fp = fingerprint_id.hex();
        for (idx, (window_id, accuracy, precision, tp, tn, fp_count, fn_count)) in [
            ("s1", 0.980000, 0.980000, 49, 49, 1, 1),
            ("s2", 0.970000, 0.980000, 49, 48, 1, 2),
        ]
        .into_iter()
        .enumerate()
        {
            store.persist_fingerprint_ship_gate_window(
                &context_graph_mejepa::FingerprintShipGateWindow {
                    window_id: window_id.to_string(),
                    report_date: format!("2026-05-1{}", idx + 6),
                    generated_at_unix_ms: 1_779_000_000_000 + idx as i64,
                    per_fingerprint: BTreeMap::from([(
                        fp.clone(),
                        context_graph_mejepa::FingerprintClassificationMetrics {
                            fingerprint_id: fp.clone(),
                            sample_count: 100,
                            true_positive_count: tp,
                            true_negative_count: tn,
                            false_positive_count: fp_count,
                            false_negative_count: fn_count,
                            accuracy,
                            precision,
                            accuracy_threshold: 0.95,
                            precision_threshold: 0.95,
                            passed_threshold: true,
                        },
                    )]),
                    unknown_ood_recall: context_graph_mejepa::UnknownOodRecallMetrics {
                        actual_unknown_count: 100,
                        detected_unknown_count: 95,
                        missed_unknown_count: 5,
                        recall: 0.95,
                        recall_threshold: 0.90,
                        passed_threshold: true,
                    },
                    passed_window: true,
                    failures: Vec::new(),
                },
            )?;
        }
        context_graph_mejepa::write_fingerprint_fisher_sync_readback(
            db.as_ref(),
            &context_graph_mejepa::FingerprintFisherSnapshot {
                fingerprint_id,
                calibrated_at_unix_ms: 1_779_000_001_000,
                sample_count: 32,
                theta_star_by_dimension: BTreeMap::from([(0, 0.1), (1, -0.1)]),
                fisher_by_dimension: BTreeMap::from([(0, 0.5), (1, 0.7)]),
            },
        )?;
        context_graph_mejepa::write_fingerprint_dormancy_sync_readback(
            db.as_ref(),
            &context_graph_mejepa::FingerprintDormancySnapshot {
                fingerprint_id,
                recorded_at_unix_ms: 1_779_000_001_001,
                window_steps: 32,
                dormancy_ema_by_dimension: BTreeMap::from([(0, 0.80), (1, 0.75)]),
                dormancy_reset_count: 0,
            },
        )?;
        Ok(())
    }

    fn unknown_entry(id: u8, task_id: &str, vector: Vec<f32>) -> ActiveLearningQueueEntry {
        let prediction_id = context_graph_mejepa::PredictionId([id; 16]);
        let candidate = context_graph_mejepa::UnknownFingerprintCandidate {
            candidate_id: [id; 16],
            prediction_id,
            task_id: TaskId(task_id.to_string()),
            session_id: [9; 16],
            observed_at_unix_ms: 1_779_000_000_000 + i64::from(id),
            ood_score: 0.92,
            embedder_disagreement_score: 0.25,
            active_learning_priority: 2,
            observation_by_embedder: BTreeMap::from([(
                context_graph_mejepa::EmbedderId("e1".to_string()),
                vector,
            )]),
            nearest_fingerprints: Vec::new(),
        };
        ActiveLearningQueueEntry {
            task_id: TaskId(task_id.to_string()),
            score: 0.8,
            outcome_set_len: 1,
            ood_score: 0.92,
            reason: "fingerprint_unknown_ood".to_string(),
            kind: ActiveLearningKind::UnknownFingerprint {
                candidate: Box::new(candidate),
            },
        }
    }

    fn boundary_case(name: &str, passed: bool) -> Value {
        json!({
            "case": name,
            "passed": passed
        })
    }
}
