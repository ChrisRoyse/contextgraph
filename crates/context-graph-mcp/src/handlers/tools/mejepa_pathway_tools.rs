//! TASK-PY-G-119 MCP surfaces for binary-leaf consequence pathways.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result as AnyhowResult};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use context_graph_mejepa::{
    pathway_leaf_credit_assignment, persist_pathway_surface, read_operator_pathway_choices,
    read_pathway_tree, read_surfaced_pathway, surface_pathways, OperatorPathwayChoiceRecord,
    PathwayLeafOutcome, PathwaySurfaceInput,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_OPERATOR_PATHWAY_CHOICES, CF_MEJEPA_PATHWAY_TREES, CF_MEJEPA_SURFACED_PATHWAYS,
};

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const ENV_TRAIN_DB: &str = "CONTEXTGRAPH_MEJEPA_TRAIN_DB";
const ENV_INFER_DB: &str = "CONTEXTGRAPH_MEJEPA_INFER_DB";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathwaySurfaceRequest {
    db_path: Option<PathBuf>,
    input: PathwaySurfaceInput,
    #[serde(default)]
    create_if_missing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathwayInspectRequest {
    db_path: Option<PathBuf>,
    pathway_id: Option<String>,
    tree_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathwayRecordChoiceRequest {
    db_path: Option<PathBuf>,
    choice: OperatorPathwayChoiceRecord,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathwayHistoryRequest {
    db_path: Option<PathBuf>,
    prediction_id_hex: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

impl Handlers {
    pub(crate) async fn call_mejepa_pathway_surface(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_PATHWAY_SURFACE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_pathway_surface(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => pathway_error(self, id, "MEJEPA_PATHWAY_SURFACE_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_pathway_inspect(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_PATHWAY_INSPECT) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_pathway_inspect(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => pathway_error(self, id, "MEJEPA_PATHWAY_INSPECT_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_pathway_record_choice(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_PATHWAY_RECORD_CHOICE) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_pathway_record_choice(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => pathway_error(self, id, "MEJEPA_PATHWAY_RECORD_CHOICE_FAILED", err),
        }
    }

    pub(crate) async fn call_mejepa_pathway_history(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request = match parse_tool_request(args, tool_names::MEJEPA_PATHWAY_HISTORY) {
            Ok(value) => value,
            Err(message) => return self.tool_error_typed(id, ToolErrorKind::Validation, &message),
        };
        match run_pathway_history(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => pathway_error(self, id, "MEJEPA_PATHWAY_HISTORY_FAILED", err),
        }
    }
}

fn run_pathway_surface(request: PathwaySurfaceRequest) -> AnyhowResult<Value> {
    let db_path = resolve_pathway_db_path(request.db_path)?;
    let db = open_pathway_rocksdb(&db_path, request.create_if_missing)
        .with_context(|| format!("open pathway DB {}", db_path.display()))?;
    let report = surface_pathways(request.input)?;
    persist_pathway_surface(&db, &report)?;
    Ok(json!({
        "tool": tool_names::MEJEPA_PATHWAY_SURFACE,
        "sourceOfTruth": source_of_truth(&db_path),
        "report": report
    }))
}

fn run_pathway_inspect(request: PathwayInspectRequest) -> AnyhowResult<Value> {
    let db_path = resolve_pathway_db_path(request.db_path)?;
    let db = open_pathway_rocksdb(&db_path, false)
        .with_context(|| format!("open pathway DB {}", db_path.display()))?;
    if request.pathway_id.is_none() && request.tree_id.is_none() {
        bail!("pathwayId or treeId is required");
    }
    let pathway = request
        .pathway_id
        .as_deref()
        .map(|id| read_surfaced_pathway(&db, id))
        .transpose()?
        .flatten();
    let tree_id = request
        .tree_id
        .as_deref()
        .or_else(|| pathway.as_ref().map(|value| value.tree_id.as_str()));
    let tree = tree_id
        .map(|id| read_pathway_tree(&db, id))
        .transpose()?
        .flatten();
    let credit_assignments_if_refuted = pathway
        .as_ref()
        .map(credit_assignments_if_refuted)
        .transpose()?
        .unwrap_or_default();
    Ok(json!({
        "tool": tool_names::MEJEPA_PATHWAY_INSPECT,
        "sourceOfTruth": source_of_truth(&db_path),
        "tree": tree,
        "pathway": pathway,
        "creditAssignmentsIfRefuted": credit_assignments_if_refuted
    }))
}

fn run_pathway_record_choice(request: PathwayRecordChoiceRequest) -> AnyhowResult<Value> {
    let db_path = resolve_pathway_db_path(request.db_path)?;
    let db = open_pathway_rocksdb(&db_path, false)
        .with_context(|| format!("open pathway DB {}", db_path.display()))?;
    let inserted = context_graph_mejepa::write_operator_pathway_choice(&db, &request.choice)?;
    Ok(json!({
        "tool": tool_names::MEJEPA_PATHWAY_RECORD_CHOICE,
        "sourceOfTruth": source_of_truth(&db_path),
        "inserted": inserted,
        "choice": request.choice
    }))
}

fn run_pathway_history(request: PathwayHistoryRequest) -> AnyhowResult<Value> {
    if request.limit == 0 || request.limit > 10_000 {
        bail!("limit must be in 1..=10000");
    }
    let db_path = resolve_pathway_db_path(request.db_path)?;
    let db = open_pathway_rocksdb(&db_path, false)
        .with_context(|| format!("open pathway DB {}", db_path.display()))?;
    let choices = read_operator_pathway_choices(&db, request.limit)?;
    let choices = if let Some(prediction_id_hex) = request.prediction_id_hex {
        choices
            .into_iter()
            .filter(|choice| choice.prediction_id_hex == prediction_id_hex)
            .collect::<Vec<_>>()
    } else {
        choices
    };
    let mut surfaced = Vec::new();
    for choice in &choices {
        if let Some(pathway) = read_surfaced_pathway(&db, &choice.pathway_id)? {
            surfaced.push(pathway);
        }
    }
    Ok(json!({
        "tool": tool_names::MEJEPA_PATHWAY_HISTORY,
        "sourceOfTruth": source_of_truth(&db_path),
        "choices": choices,
        "surfacedPathways": surfaced
    }))
}

fn open_pathway_rocksdb(path: &Path, create_if_missing: bool) -> AnyhowResult<DB> {
    validate_path_not_mntd(path)?;
    let mut opts = Options::default();
    opts.create_if_missing(create_if_missing);
    opts.create_missing_column_families(create_if_missing);
    let mut cf_names = BTreeSet::<String>::new();
    cf_names.insert("default".to_string());
    for cf in pathway_cfs() {
        cf_names.insert(cf.to_string());
    }
    if path.exists() {
        for cf in DB::list_cf(&opts, path).unwrap_or_default() {
            cf_names.insert(cf);
        }
    }
    let descriptors = cf_names
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()))
        .collect::<Vec<_>>();
    let db = DB::open_cf_descriptors(&opts, path, descriptors)?;
    for cf in pathway_cfs() {
        if db.cf_handle(cf).is_none() {
            bail!("missing pathway column family {cf}");
        }
    }
    Ok(db)
}

fn pathway_cfs() -> [&'static str; 3] {
    [
        CF_MEJEPA_PATHWAY_TREES,
        CF_MEJEPA_SURFACED_PATHWAYS,
        CF_MEJEPA_OPERATOR_PATHWAY_CHOICES,
    ]
}

fn resolve_pathway_db_path(input: Option<PathBuf>) -> AnyhowResult<PathBuf> {
    match input {
        Some(path) if !path.as_os_str().is_empty() => {
            validate_path_not_mntd(&path)?;
            Ok(path)
        }
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
            let path = PathBuf::from(raw);
            validate_path_not_mntd(&path)?;
            Ok(path)
        }
    }
}

fn validate_path_not_mntd(path: &Path) -> AnyhowResult<()> {
    let value = path.display().to_string();
    if value == "/mnt/d" || value.starts_with("/mnt/d/") {
        bail!("legacy /mnt/d roots are not valid for pathway source-of-truth");
    }
    Ok(())
}

fn source_of_truth(db_path: &Path) -> Value {
    json!({
        "dbPath": db_path.display().to_string(),
        "cfs": pathway_cfs(),
        "binaryLeafOnly": true,
        "ambiguousLeafRejectCode": context_graph_mejepa::PATHWAY_AMBIGUOUS_LEAF_REJECTED
    })
}

fn credit_assignments_if_refuted(
    pathway: &context_graph_mejepa::SurfacedPathwayRecord,
) -> AnyhowResult<Vec<context_graph_mejepa::PathwayLeafCreditAssignment>> {
    pathway
        .leaf_chain
        .iter()
        .map(|leaf| {
            let observed = opposite_outcome(leaf.predicted_outcome);
            pathway_leaf_credit_assignment(pathway, &leaf.leaf_id, observed).map_err(Into::into)
        })
        .collect()
}

fn opposite_outcome(outcome: PathwayLeafOutcome) -> PathwayLeafOutcome {
    match outcome {
        PathwayLeafOutcome::Yes => PathwayLeafOutcome::No,
        PathwayLeafOutcome::No => PathwayLeafOutcome::Yes,
        PathwayLeafOutcome::Pass => PathwayLeafOutcome::Fail,
        PathwayLeafOutcome::Fail => PathwayLeafOutcome::Pass,
    }
}

fn default_limit() -> usize {
    100
}

fn parse_tool_request<T: DeserializeOwned>(
    args: serde_json::Value,
    tool_name: &str,
) -> Result<T, String> {
    serde_json::from_value(args)
        .map_err(|err| format!("{tool_name} schema validation failed: {err}"))
}

fn pathway_error(
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
        json!({"toolFamily": "mejepa_pathway"}),
    )
}
