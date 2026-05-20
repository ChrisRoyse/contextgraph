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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChunkAsStarRequest {
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
struct ConstellationMembershipRequest {
    chunk_id: String,
    code_state_key: Option<String>,
    db_path: Option<PathBuf>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillImpactRequest {
    chunk_id: String,
    code_state_key: Option<String>,
    db_path: Option<PathBuf>,
    #[serde(default = "default_impact_depth")]
    depth: u32,
    #[serde(default = "default_set_query_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillGraphInspectRequest {
    skill_id: Option<String>,
    db_path: Option<PathBuf>,
    #[serde(default = "default_set_query_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillConflictGraphRequest {
    db_path: Option<PathBuf>,
    #[serde(default = "default_set_query_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SkillBrowseRequest {
    db_path: Option<PathBuf>,
    filter: Option<String>,
    #[serde(default = "default_set_query_limit")]
    limit: usize,
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

    pub(crate) async fn call_mejepa_chunk_as_star(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_CHUNK_AS_STAR) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_chunk_as_star(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_CHUNK_AS_STAR_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_constellation_membership(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_CONSTELLATION_MEMBERSHIP) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_constellation_membership(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                skill_linkage_error(self, id, "MEJEPA_CONSTELLATION_MEMBERSHIP_FAILED", err)
            }
        }
    }

    pub(crate) async fn call_mejepa_skill_impact(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_IMPACT) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_impact(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_IMPACT_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_skill_graph_inspect(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_GRAPH_INSPECT) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_graph_inspect(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_GRAPH_INSPECT_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_skill_conflict_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_CONFLICT_GRAPH) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_conflict_graph(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_CONFLICT_GRAPH_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_skill_browse(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_SKILL_BROWSE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_skill_browse(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => skill_linkage_error(self, id, "MEJEPA_SKILL_BROWSE_FAILED", err),
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
    // F-029 / #481: make the universe-resolution semantics explicit.
    //
    // `skill_coverage_audit` interprets an empty `chunk_universe` slice as
    // "use the universe derived from CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP rows"
    // (see skill_linkage.rs:288). When the caller supplies a `chunks_jsonl`
    // file, the audit is bounded to that explicit universe; when omitted,
    // the audit derives the universe from the CF rows.
    //
    // Encoding this as `unwrap_or_default()` collapsed the two distinct
    // modes into a single Vec<String> with no audit trail. We now branch
    // explicitly so the MCP response payload distinguishes the two modes
    // via `chunk_universe_source`, restoring SoT readback integrity.
    let (chunk_universe, chunk_universe_source) = match source_index.as_ref() {
        Some(index) => (
            context_graph_mejepa_train::ChunkSourceIndex::chunk_ids(index),
            "explicit_jsonl",
        ),
        None => (Vec::new(), "cf_derived"),
    };
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
        "chunkUniverseSource": chunk_universe_source,
        "chunkUniverseSize": chunk_universe.len(),
        "report": report
    }))
}

fn run_chunk_as_star(request: ChunkAsStarRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let source_index = optional_source_index(request.chunks_jsonl.as_deref())?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::chunk_as_star(
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
        "tool": tool_names::MEJEPA_CHUNK_AS_STAR,
        "sourceOfTruth": source_of_truth(&db_path, request.chunks_jsonl.as_deref()),
        "report": report
    }))
}

fn run_constellation_membership(request: ConstellationMembershipRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::constellation_membership(
        &db,
        &request.chunk_id,
        request.code_state_key.as_deref(),
        request.limit,
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_CONSTELLATION_MEMBERSHIP,
        "sourceOfTruth": source_of_truth(&db_path, None),
        "report": report
    }))
}

fn run_skill_impact(request: SkillImpactRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_impact(
        &db,
        &request.chunk_id,
        request.code_state_key.as_deref(),
        request.depth,
        request.limit,
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_IMPACT,
        "sourceOfTruth": source_of_truth(&db_path, None),
        "report": report
    }))
}

fn run_skill_graph_inspect(request: SkillGraphInspectRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_graph_inspect(
        &db,
        request.skill_id.as_deref(),
        request.limit,
    )?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_GRAPH_INSPECT,
        "sourceOfTruth": source_of_truth(&db_path, None),
        "report": report
    }))
}

fn run_skill_conflict_graph(request: SkillConflictGraphRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report = context_graph_mejepa_train::skill_conflict_graph(&db, request.limit)?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_CONFLICT_GRAPH,
        "sourceOfTruth": source_of_truth(&db_path, None),
        "report": report
    }))
}

fn run_skill_browse(request: SkillBrowseRequest) -> AnyhowResult<Value> {
    let db_path = resolve_skill_db_path(request.db_path)?;
    let db = context_graph_mejepa_train::open_skill_linkage_rocksdb(&db_path, false)
        .with_context(|| format!("open skill linkage DB {}", db_path.display()))?;
    let report =
        context_graph_mejepa_train::skill_browse(&db, request.filter.as_deref(), request.limit)?;
    Ok(json!({
        "tool": tool_names::MEJEPA_SKILL_BROWSE,
        "sourceOfTruth": source_of_truth(&db_path, None),
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

fn default_impact_depth() -> u32 {
    2
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
