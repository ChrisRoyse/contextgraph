//! DynamicJEPA MCP handlers (5090jepa Phase 9).

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tracing::{error, info};

use context_graph_cli::commands::dynamicjepa::{
    run_dynamicjepa_command, BindArgs, CompileDatasetArgs, CompileTrajectoriesArgs,
    DynamicJepaCommands, GetArtifactArgs, GetDatasetShardArgs, GetDomainArgs, GetPanelArgs,
    GetPlanTraceArgs, GetPredictionArgs, GetSurpriseArgs, GetTrainingRunArgs, GetTrajectoryArgs,
    IngestEventArgs, InspectCfArgs, InspectCountsArgs, InspectDatasetArgs, ListBindingsArgs,
    ListDomainsArgs, ListInstrumentReadingsArgs, ListTrajectoriesArgs, MaterializePanelArgs,
    PlanArgs, PredictArgs, RecordSurpriseArgs, RegisterDomainArgs, RunAdapterArgs, TrainArgs,
};

use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::dynamicjepa_dtos::*;
use super::helpers::ToolErrorKind;

impl Handlers {
    pub(crate) async fn call_dynamicjepa_register_domain_pack(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: RegisterDomainPackRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_register_domain_pack", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::RegisterDomain(RegisterDomainArgs {
                db: req.db_path,
                file: req.file,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_list_domain_packs(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: ListDomainPacksRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_list_domain_packs", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::ListDomains(ListDomainsArgs {
                db: req.db_path,
                limit: req.limit,
                offset: req.offset,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_domain_pack(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetDomainPackRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_domain_pack", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetDomain(GetDomainArgs {
                db: req.db_path,
                id: req.id,
                domain_version: req.domain_version,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_ingest_event(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: IngestEventRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_ingest_event", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::IngestEvent(IngestEventArgs {
                db: req.db_path,
                domain: req.domain,
                adapter: req.adapter,
                file: req.file,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_run_adapter(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: RunAdapterRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_run_adapter", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::RunAdapter(RunAdapterArgs {
                db: req.db_path,
                event_id: req.event_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_materialize_panel(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: MaterializePanelRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_materialize_panel", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::MaterializePanel(MaterializePanelArgs {
                db: req.db_path,
                transition_id: req.transition_id,
                all_pending: req.all_pending,
                domain: req.domain,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_panel(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetPanelRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_panel", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetPanel(GetPanelArgs {
                db: req.db_path,
                panel_id: req.panel_id,
                include_readings: req.include_readings,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_list_instrument_readings(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: ListInstrumentReadingsRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_list_instrument_readings", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::ListInstrumentReadings(ListInstrumentReadingsArgs {
                db: req.db_path,
                event_id: req.event_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_create_binding(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: CreateBindingRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_create_binding", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::Bind(BindArgs {
                db: req.db_path,
                left_cf: req.left_cf,
                left_key: req.left_key,
                right_cf: req.right_cf,
                right_key: req.right_key,
                method: req.method,
                kind: req.kind,
                score: req.score,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_list_bindings(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: ListBindingsRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_list_bindings", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::ListBindings(ListBindingsArgs {
                db: req.db_path,
                entity: req.entity,
                limit: req.limit,
                offset: req.offset,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_compile_trajectories(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: CompileTrajectoriesRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_compile_trajectories", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::CompileTrajectories(CompileTrajectoriesArgs {
                db: req.db_path,
                domain: req.domain,
                policy: req.policy,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_trajectory(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetTrajectoryRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_trajectory", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetTrajectory(GetTrajectoryArgs {
                db: req.db_path,
                id: req.id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_list_trajectories(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: ListTrajectoriesRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_list_trajectories", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::ListTrajectories(ListTrajectoriesArgs {
                db: req.db_path,
                domain: req.domain,
                limit: req.limit,
                offset: req.offset,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_compile_dataset(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: CompileDatasetRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_compile_dataset", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::CompileDataset(CompileDatasetArgs {
                db: req.db_path,
                domain: req.domain,
                policy: req.policy,
                split: req.split,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_dataset_shard(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetDatasetShardRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_dataset_shard", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetDatasetShard(GetDatasetShardArgs {
                db: req.db_path,
                dataset_id: req.dataset_id,
                shard_id: req.shard_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_inspect_dataset_row(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: InspectDatasetRowRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_inspect_dataset_row", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::InspectDataset(InspectDatasetArgs {
                db: req.db_path,
                dataset_id: req.dataset_id,
                shard_id: req.shard_id,
                row: req.row,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_train(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: TrainRequest = match parse_dynamicjepa_args(&id, args, "dynamicjepa_train", self) {
            Ok(req) => req,
            Err(resp) => return resp,
        };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::Train(TrainArgs {
                db: req.db_path,
                dataset_id: req.dataset_id,
                config: req.config,
                artifact_root: req.artifact_root,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_training_run(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetTrainingRunRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_training_run", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetTrainingRun(GetTrainingRunArgs {
                db: req.db_path,
                id: req.id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_artifact(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetArtifactRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_artifact", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetArtifact(GetArtifactArgs {
                db: req.db_path,
                id: req.id,
                verify_files: req.verify_files,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_predict(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: PredictRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_predict", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::Predict(PredictArgs {
                db: req.db_path,
                artifact_id: req.artifact_id,
                panel_id: req.panel_id,
                action_id: req.action_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_plan(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: PlanRequest = match parse_dynamicjepa_args(&id, args, "dynamicjepa_plan", self) {
            Ok(req) => req,
            Err(resp) => return resp,
        };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::Plan(PlanArgs {
                db: req.db_path,
                artifact_id: req.artifact_id,
                panel_id: req.panel_id,
                skill_id: req.skill_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_record_surprise(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: RecordSurpriseRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_record_surprise", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::RecordSurprise(RecordSurpriseArgs {
                db: req.db_path,
                prediction_id: req.prediction_id,
                observed_outcome_id: req.observed_outcome_id,
                observed_panel_id: req.observed_panel_id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_prediction(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetPredictionRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_prediction", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetPrediction(GetPredictionArgs {
                db: req.db_path,
                id: req.id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_plan_trace(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetPlanTraceRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_plan_trace", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetPlanTrace(GetPlanTraceArgs {
                db: req.db_path,
                id: req.id,
                include_predictions: req.include_predictions,
                include_guards: req.include_guards,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_get_surprise(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: GetSurpriseRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_get_surprise", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::GetSurprise(GetSurpriseArgs {
                db: req.db_path,
                id: req.id,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_inspect_counts(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: InspectCountsRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_inspect_counts", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::InspectCounts(InspectCountsArgs {
                db: req.db_path,
                allow_missing: false,
                json: true,
            }),
        )
        .await
    }

    pub(crate) async fn call_dynamicjepa_inspect_cf(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        let req: InspectCfRequest =
            match parse_dynamicjepa_args(&id, args, "dynamicjepa_inspect_cf", self) {
                Ok(req) => req,
                Err(resp) => return resp,
            };
        self.run_dynamicjepa_mcp(
            id,
            DynamicJepaCommands::InspectCf(InspectCfArgs {
                db: req.db_path,
                cf: req.cf,
                limit: req.limit,
                offset: req.offset,
                json: true,
            }),
        )
        .await
    }

    async fn run_dynamicjepa_mcp(
        &self,
        id: Option<JsonRpcId>,
        action: DynamicJepaCommands,
    ) -> JsonRpcResponse {
        match run_dynamicjepa_command(action).await {
            Ok(outcome) => {
                let is_error = outcome.exit_code != 0;
                if is_error {
                    error!(
                        error_code = outcome
                            .value
                            .get("error_code")
                            .and_then(|value| value.as_str())
                            .unwrap_or("DYNAMICJEPA_ERROR"),
                        "DynamicJEPA MCP tool returned structured error"
                    );
                } else {
                    info!("DynamicJEPA MCP tool completed");
                }
                dynamicjepa_tool_response(id, outcome.value, is_error)
            }
            Err(err) => {
                error!(
                    error_code = err.code(),
                    error_message = %err,
                    "DynamicJEPA MCP tool failed before structured command output"
                );
                self.tool_error_typed(
                    id,
                    dynamicjepa_error_kind(err.code()),
                    &format!("{}: {}", err.code(), err),
                )
            }
        }
    }
}

fn parse_dynamicjepa_args<T: DeserializeOwned>(
    id: &Option<JsonRpcId>,
    args: Value,
    tool_name: &str,
    handlers: &Handlers,
) -> Result<T, JsonRpcResponse> {
    ensure_no_empty_strings(&args, tool_name).map_err(|message| {
        error!(tool_name, error_message = %message, "DynamicJEPA MCP argument validation failed");
        handlers.tool_error_typed(id.clone(), ToolErrorKind::Validation, &message)
    })?;
    serde_json::from_value(args).map_err(|err| {
        let message = format!("{tool_name} arguments are invalid: {err}");
        error!(tool_name, error_message = %message, "DynamicJEPA MCP argument parse failed");
        handlers.tool_error_typed(id.clone(), ToolErrorKind::Validation, &message)
    })
}

fn ensure_no_empty_strings(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::String(text) if text.trim().is_empty() => {
            Err(format!("{path} must not contain empty string arguments"))
        }
        Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                ensure_no_empty_strings(item, &format!("{path}[{idx}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map {
                ensure_no_empty_strings(item, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn dynamicjepa_tool_response(
    id: Option<JsonRpcId>,
    data: Value,
    is_error: bool,
) -> JsonRpcResponse {
    let text = serde_json::to_string(&data).unwrap_or_else(|err| {
        json!({
            "operation": "dynamicjepa_mcp_serialize",
            "status": "error",
            "error_code": "MCP_SERIALIZE",
            "error_message": format!("failed to serialize DynamicJEPA response: {err}")
        })
        .to_string()
    });
    let mut result = json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": data,
        "isError": is_error
    });
    if is_error {
        let error_code = result["structuredContent"]["error_code"]
            .as_str()
            .unwrap_or("DYNAMICJEPA_ERROR")
            .to_string();
        result["errorCode"] = json!(error_codes::INTERNAL_ERROR);
        result["dynamicJepaErrorCode"] = json!(error_code);
    }
    JsonRpcResponse::success(id, result)
}

fn dynamicjepa_error_kind(code: &str) -> ToolErrorKind {
    match code {
        "VALIDATION" | "SCHEMA_VALIDATION_FAILED" | "PREDICTION_INPUT_MISSING" => {
            ToolErrorKind::Validation
        }
        "SOURCE_OF_TRUTH_MISSING" | "DOMAIN_PACK_NOT_FOUND" => ToolErrorKind::NotFound,
        "STORAGE" | "CODEC" | "ARTIFACT_HASH_MISMATCH" | "STORAGE_INVARIANT_VIOLATION" => {
            ToolErrorKind::Storage
        }
        _ => ToolErrorKind::Execution,
    }
}
