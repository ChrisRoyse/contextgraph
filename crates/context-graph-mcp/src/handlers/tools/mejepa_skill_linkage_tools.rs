//! TASK-PY-G-120 MCP read surface for skill <-> code linkage.

use std::path::{Path, PathBuf};

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
struct SkillToCodeRequest {
    skill_id: String,
    db_path: Option<PathBuf>,
    chunks_jsonl: Option<PathBuf>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    require_source_text: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CodeToSkillRequest {
    chunk_id: String,
    code_state_key: Option<String>,
    db_path: Option<PathBuf>,
    chunks_jsonl: Option<PathBuf>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    require_source_text: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillSetQueryRequest {
    must_have: Vec<String>,
    #[serde(default)]
    must_not_have: Vec<String>,
    db_path: Option<PathBuf>,
    #[serde(default = "default_set_query_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillCoverageAuditRequest {
    db_path: Option<PathBuf>,
    chunks_jsonl: Option<PathBuf>,
    #[serde(default = "default_set_query_limit")]
    sample_limit: usize,
}

impl Handlers {
    pub(crate) async fn call_mejepa_skill_to_code(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_TO_CODE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_to_code(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_TO_CODE_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_code_to_skill(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_CODE_TO_SKILL) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_code_to_skill(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_CODE_TO_SKILL_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_skill_set_query(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_SET_QUERY) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_set_query(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_SET_QUERY_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_skill_coverage_audit(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_COVERAGE_AUDIT) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_coverage_audit(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_COVERAGE_AUDIT_FAILED", err),
        }
    }
}

fn run_skill_to_code(request: SkillToCodeRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let source_index = optional_source_index(request.chunks_jsonl.as_deref())?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_to_code(
        &db,
        &request.skill_id,
        source_index.as_ref(),
        context_graph_mejepa_train::SkillLinkageOptions {
            limit: request.limit,
            require_source_text: request.require_source_text,
        },
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_TO_CODE,
        "sourceOfTruth": source_of_truth(&db_path, request.chunks_jsonl.as_deref()),
        "report": report
    }))
}

fn run_code_to_skill(request: CodeToSkillRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let source_index = optional_source_index(request.chunks_jsonl.as_deref())?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::code_to_skill(
        &db,
        &request.chunk_id,
        request.code_state_key.as_deref(),
        source_index.as_ref(),
        context_graph_mejepa_train::SkillLinkageOptions {
            limit: request.limit,
            require_source_text: request.require_source_text,
        },
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_CODE_TO_SKILL,
        "sourceOfTruth": source_of_truth(&db_path, request.chunks_jsonl.as_deref()),
        "report": report
    }))
}

fn run_skill_set_query(request: SkillSetQueryRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_set_query(
        &db,
        &request.must_have,
        &request.must_not_have,
        request.limit,
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_SET_QUERY,
        "sourceOfTruth": source_of_truth(&db_path, None),
        "report": report
    }))
}

fn run_skill_coverage_audit(request: SkillCoverageAuditRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let source_index = optional_source_index(request.chunks_jsonl.as_deref())?;
    let chunk_universe = source_index
        .as_ref()
        .map(context_graph_mejepa_train::ChunkSourceIndex::chunk_ids)
        .unwrap_or_default();
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_coverage_audit(
        &db,
        &chunk_universe,
        request.sample_limit,
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_COVERAGE_AUDIT,
        "sourceOfTruth": source_of_truth(&db_path, request.chunks_jsonl.as_deref()),
        "report": report
    }))
}

fn optional_source_index(
    path: Option<&Path>,
) -> AnyhowResult<Option<context_graph_mejepa_train::ChunkSourceIndex>> {
    path.map(context_graph_mejepa_train::load_chunk_source_index_jsonl)
        .transpose()
        .map_err(anyhow::Error::from)
}

fn default_limit() -> usize {
    20
}

fn default_set_query_limit() -> usize {
    100
}

fn parse_tool_request<T: DeserializeOwned>(
    args: serde_json::Value,
    tool_name: &str,
) -> Result<T, String> {
    serde_json::from_value(args)
        .map_err(|err| format!("{tool_name} schema validation failed: {err}"))
}

fn resolve_skill_db_path(input: Option<PathBuf>) -> AnyhowResult<PathBuf> {
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

fn source_of_truth(db_path: &Path, chunks_jsonl: Option<&Path>) -> Value {
    json!({
        "dbPath": db_path.display().to_string(),
        "chunksJsonl": chunks_jsonl.map(|path| path.display().to_string()),
        "cfs": context_graph_mejepa_train::skill_linkage_cfs(),
        "noNewPredictionHeadIntroduced": true
    })
}

fn skill_linkage_error(
    handlers: &Handlers,
    id: Option<JsonRpcId>,
    code: &str,
    err: anyhow::Error,
) -> JsonRpcResponse {
    handlers.tool_error_structured(
        id,
        ToolErrorKind::Storage,
        code,
        &err.to_string(),
        json!({"toolFamily": "mejepa_skill_linkage"}),
    )
}
