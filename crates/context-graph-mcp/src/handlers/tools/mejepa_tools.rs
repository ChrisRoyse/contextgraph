//! ME-JEPA Phase 4 inference MCP handlers.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, CalibrationStore, MeJepaInferConfig,
    PatchBundle, RocksDbInferStore, TaskContext,
};
use context_graph_mejepa_tct::{
    build_inspect_summary, open_tct_rocksdb, ConstellationStore, EmbedderId,
};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_TCT_DB: &str = "CONTEXTGRAPH_MEJEPA_TCT_DB";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VerifyRequest {
    patch: PatchBundle,
    context: TaskContext,
    #[serde(default)]
    include_provenance: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PredictLatestRequest {
    session_id: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConstellationInspectRequest {
    db_path: Option<PathBuf>,
    version_id: Option<String>,
    runtime_embedder_versions: std::collections::BTreeMap<String, String>,
}

fn default_limit() -> u32 {
    10
}

impl Handlers {
    pub(crate) async fn call_mejepa_verify(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: VerifyRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_VERIFY
                    ),
                );
            }
        };
        if request.context.problem_statement.len() > 65_536 {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "problem_statement exceeds 65536 bytes",
            );
        }
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let repo_root = request.context.environment.repo_root.clone();
        info!(
            tool = tool_names::MEJEPA_VERIFY,
            task_id = %request.context.task_id.0,
            db_path = %db_path.display(),
            "ME-JEPA verify"
        );
        let result = (|| {
            let db = open_infer_rocksdb(&db_path)?;
            let calibration = CalibrationStore::new(db.clone(), 30)?;
            let store = Arc::new(RocksDbInferStore::new(db));
            let compiler = build_deterministic_compiler(
                repo_root,
                store,
                calibration,
                MeJepaInferConfig::default(),
            )?;
            let verdict = compiler.verify(&request.patch, &request.context)?;
            let mut response = serde_json::to_value(verdict)?;
            if request.include_provenance {
                response["provenance"] = json!({
                    "codeVersion": env!("CARGO_PKG_VERSION"),
                    "dbPath": db_path,
                    "mode": "phase4_inference"
                });
            }
            Ok::<serde_json::Value, context_graph_mejepa::MejepaInferError>(response)
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_predict_latest(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PredictLatestRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_PREDICT_LATEST
                    ),
                );
            }
        };
        if request.session_id.len() != 32
            || !request.session_id.chars().all(|ch| ch.is_ascii_hexdigit())
        {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                "sessionId must be exactly 32 hexadecimal characters",
            );
        }
        let mut session_id = [0u8; 16];
        if let Err(err) = hex::decode_to_slice(&request.session_id, &mut session_id) {
            return self.tool_error_typed(
                id,
                ToolErrorKind::Validation,
                &format!("sessionId decode failed: {err}"),
            );
        }
        let db_path = match infer_db_path() {
            Ok(path) => path,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result = (|| {
            let db = open_infer_rocksdb(&db_path)?;
            let store = RocksDbInferStore::new(db);
            let predictions = context_graph_mejepa::MejepaStore::read_live_predictions(
                &store,
                session_id,
                request.limit.clamp(1, 1000),
            )?;
            Ok::<serde_json::Value, context_graph_mejepa::MejepaInferError>(json!({
                "predictions": predictions
            }))
        })();
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Storage,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_constellation_inspect(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: ConstellationInspectRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_CONSTELLATION_INSPECT
                    ),
                );
            }
        };
        let db_path = match request.db_path {
            path @ Some(_) => match resolve_tct_db_path(path) {
                Ok(path) => path,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            },
            None => match resolve_tct_db_path(None) {
                Ok(path) => path,
                Err(message) => {
                    return self.tool_error_typed(id, ToolErrorKind::Validation, &message)
                }
            },
        };
        let runtime_versions = match parse_runtime_versions(&request.runtime_embedder_versions) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        let result =
            run_constellation_inspect(&db_path, request.version_id.as_deref(), &runtime_versions);
        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code()),
                )
            }
        }
    }
}

fn resolve_tct_db_path(input: Option<PathBuf>) -> Result<PathBuf, String> {
    match input {
        Some(path) => Ok(path),
        None => match std::env::var(ENV_TCT_DB) {
            Ok(path) => Ok(PathBuf::from(path)),
            Err(std::env::VarError::NotPresent) => Err(format!(
                "dbPath or {ENV_TCT_DB} is required; refusing to guess a TCT database path"
            )),
            Err(err) => Err(format!("{ENV_TCT_DB} must be readable UTF-8: {err}")),
        },
    }
}

fn run_constellation_inspect(
    db_path: &std::path::Path,
    version_id: Option<&str>,
    runtime_versions: &std::collections::BTreeMap<EmbedderId, [u8; 32]>,
) -> Result<serde_json::Value, context_graph_mejepa_tct::TctError> {
    let db = open_tct_rocksdb(db_path)?;
    let store = ConstellationStore::new(db)?;
    let version_id = match version_id {
        Some(value) => parse_version_id(value).map_err(|err| {
            context_graph_mejepa_tct::TctError::InvalidInput {
                field: "versionId".to_string(),
                detail: err,
            }
        })?,
        None => store.latest_version()?,
    };
    let constellation = store.load(version_id, runtime_versions)?;
    let summary = build_inspect_summary(&constellation, None);
    serde_json::to_value(summary).map_err(context_graph_mejepa_tct::TctError::from)
}

fn infer_db_path() -> Result<PathBuf, String> {
    std::env::var("CONTEXTGRAPH_MEJEPA_INFER_DB")
        .map(PathBuf::from)
        .map_err(|_| {
            "CONTEXTGRAPH_MEJEPA_INFER_DB must point to the Phase 4 inference RocksDB; refusing to guess a database path".to_string()
        })
}

fn parse_runtime_versions(
    input: &std::collections::BTreeMap<String, String>,
) -> Result<std::collections::BTreeMap<EmbedderId, [u8; 32]>, String> {
    if input.len() != EmbedderId::all().len() {
        return Err(format!(
            "runtimeEmbedderVersions must contain exactly {} entries, got {}",
            EmbedderId::all().len(),
            input.len()
        ));
    }
    let mut out = std::collections::BTreeMap::new();
    for (key, value) in input {
        let embedder = EmbedderId::from_str(key)
            .map_err(|err| format!("invalid embedder id {key:?}: {err}"))?;
        let digest = parse_version_id(value)
            .map_err(|err| format!("invalid digest for {embedder}: {err}"))?;
        out.insert(embedder, digest);
    }
    for embedder in EmbedderId::all() {
        if !out.contains_key(&embedder) {
            return Err(format!("runtimeEmbedderVersions missing {embedder}"));
        }
    }
    Ok(out)
}

fn parse_version_id(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "expected exactly 64 hexadecimal characters, got {value:?}"
        ));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(value, &mut out)
        .map_err(|err| format!("hex decode failed for {value:?}: {err}"))?;
    Ok(out)
}

#[cfg(test)]
mod mejepa_constellation_inspect_tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, SystemTime};

    use context_graph_mejepa_tct::{
        Centroid, CorpusProvenance, EntityType, Language, MutationCategory, OracleOutcome,
        ShrinkageOrigin, TctConstellation, Thresholds,
    };
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_guard() -> MutexGuard<'static, ()> {
        let guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var(ENV_TCT_DB);
        std::env::remove_var(context_graph_mejepa_tct::ENV_ALLOW_STALE);
        std::env::remove_var(context_graph_mejepa_tct::ENV_MAX_AGE_DAYS);
        guard
    }

    fn versions(seed: u8) -> BTreeMap<EmbedderId, [u8; 32]> {
        EmbedderId::all()
            .into_iter()
            .enumerate()
            .map(|(idx, embedder)| (embedder, [seed.wrapping_add(idx as u8); 32]))
            .collect()
    }

    fn centroid(sample_count: usize) -> Centroid {
        Centroid::try_new(
            vec![1.0],
            sample_count,
            ShrinkageOrigin::OwnCell,
            "mcp-test",
        )
        .unwrap()
    }

    fn by_embedder(sample_count: usize) -> BTreeMap<EmbedderId, Centroid> {
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, centroid(sample_count)))
            .collect()
    }

    fn thresholds() -> Thresholds {
        let panel_level = EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.8))
            .collect();
        let per_chunk_type = EmbedderId::all()
            .into_iter()
            .map(|embedder| ((EntityType::Function, embedder), 0.9))
            .collect();
        Thresholds::try_new(panel_level, per_chunk_type).unwrap()
    }

    fn constellation(frozen_at: SystemTime, seed: u8) -> TctConstellation {
        let embedder_versions = versions(seed);
        let provenance =
            CorpusProvenance::try_new([seed; 32], embedder_versions, frozen_at, "a".repeat(40))
                .unwrap();
        let mut per_category = BTreeMap::new();
        per_category.insert(MutationCategory::KnownGood, by_embedder(50));
        let mut per_language = BTreeMap::new();
        per_language.insert(
            (Language::Python, MutationCategory::KnownGood),
            by_embedder(50),
        );
        let mut outcomes = BTreeMap::new();
        outcomes.insert(OracleOutcome::Pass, by_embedder(50));
        let per_chunk = EmbedderId::all()
            .into_iter()
            .map(|embedder| {
                (
                    (
                        MutationCategory::KnownGood,
                        EntityType::Function,
                        Language::Python,
                        embedder,
                    ),
                    centroid(50),
                )
            })
            .collect();
        TctConstellation::try_new(
            per_category,
            per_language,
            outcomes,
            per_chunk,
            thresholds(),
            provenance,
            frozen_at,
        )
        .unwrap()
    }

    fn persist(temp: &TempDir, constellation: &TctConstellation) {
        let db = open_tct_rocksdb(temp.path()).unwrap();
        let store = ConstellationStore::new(db).unwrap();
        store.persist(constellation).unwrap();
    }

    fn contains_key(value: &serde_json::Value, needle: &str) -> bool {
        match value {
            serde_json::Value::Object(map) => map
                .iter()
                .any(|(key, child)| key == needle || contains_key(child, needle)),
            serde_json::Value::Array(items) => {
                items.iter().any(|child| contains_key(child, needle))
            }
            _ => false,
        }
    }

    #[test]
    fn mejepa_constellation_inspect_returns_summary_for_loaded_version() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 1);
        persist(&temp, &constellation);
        let output = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert_eq!(
            output["version_id"],
            hex::encode(constellation.version_id())
        );
        assert_eq!(output["cell_counts"]["panel_level_cells"], 63);
        assert_eq!(output["cell_counts"]["per_chunk_type_cells"], 21);
        assert_eq!(output["sample_support_histogram"]["n_50_plus"], 21);
        assert_eq!(
            output["per_chunk_type_threshold_summary"]["function"]["n_cells"],
            21
        );
    }

    #[test]
    fn mejepa_constellation_inspect_does_not_leak_raw_centroid_vectors() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 2);
        persist(&temp, &constellation);
        let output = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert!(!contains_key(&output, "values"));
        assert!(!contains_key(&output, "per_category_centroids"));
        assert!(!contains_key(&output, "per_chunk_type_centroids"));
    }

    #[test]
    fn mejepa_constellation_inspect_loads_newest_frozen_at_when_version_omitted() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let older = constellation(SystemTime::now() - Duration::from_secs(60), 3);
        let newer = constellation(SystemTime::now(), 4);
        persist(&temp, &older);
        persist(&temp, &newer);
        let output = run_constellation_inspect(
            temp.path(),
            None,
            &newer.corpus_provenance.embedder_versions,
        )
        .unwrap();
        assert_eq!(output["version_id"], hex::encode(newer.version_id()));
    }

    #[test]
    fn mejepa_constellation_inspect_surfaces_provenance_mismatch() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 5);
        persist(&temp, &constellation);
        let mut runtime = constellation.corpus_provenance.embedder_versions.clone();
        runtime.insert(EmbedderId::E3, [99; 32]);
        let err = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(constellation.version_id())),
            &runtime,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_PROVENANCE_MISMATCH");
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_stale_constellation() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let stale = constellation(SystemTime::now() - Duration::from_secs(91 * 86_400), 6);
        persist(&temp, &stale);
        let err = run_constellation_inspect(
            temp.path(),
            Some(&hex::encode(stale.version_id())),
            &stale.corpus_provenance.embedder_versions,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_GTAU_STALE_CONSTELLATION");
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_malformed_version_id() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let constellation = constellation(SystemTime::now(), 7);
        persist(&temp, &constellation);
        let err = run_constellation_inspect(
            temp.path(),
            Some("not-a-64-byte-hex-digest"),
            &constellation.corpus_provenance.embedder_versions,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_INVALID_INPUT");
    }

    #[test]
    fn mejepa_constellation_inspect_requires_db_path_or_env() {
        let _guard = env_guard();
        let err = resolve_tct_db_path(None).unwrap_err();
        assert!(err.contains(ENV_TCT_DB));
        assert!(err.contains("refusing to guess"));
    }

    #[test]
    fn mejepa_constellation_inspect_rejects_empty_store() {
        let _guard = env_guard();
        let temp = TempDir::new().unwrap();
        let runtime = versions(8);
        let err = run_constellation_inspect(temp.path(), None, &runtime).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TCT_MISSING_CENTROID");
    }
}
