//! ME-JEPA UTML MCP handlers.

use std::collections::BTreeMap;
use std::path::PathBuf;

use context_graph_mejepa_train::learning_signal::PairwiseMiAuditor;
use serde::Deserialize;
use serde_json::json;

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairwiseMiAuditRequest {
    output_dir: PathBuf,
    step: u64,
    #[serde(default = "default_period_steps")]
    period_steps: u64,
    series_by_slot: BTreeMap<String, Vec<f32>>,
}

fn default_period_steps() -> u64 {
    1
}

impl PairwiseMiAuditRequest {
    fn validate(&self) -> Result<(), String> {
        if self.period_steps == 0 {
            return Err("periodSteps must be greater than zero".to_string());
        }
        if self.series_by_slot.len() < 2 {
            return Err("seriesBySlot must contain at least two slots".to_string());
        }
        for (slot, values) in &self.series_by_slot {
            if slot.trim().is_empty() {
                return Err("seriesBySlot contains an empty slot name".to_string());
            }
            if values.len() < 2 {
                return Err(format!(
                    "seriesBySlot.{slot} must contain at least two samples"
                ));
            }
        }
        Ok(())
    }
}

impl Handlers {
    pub(crate) async fn call_mejepa_audit_pairwise_mi(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: PairwiseMiAuditRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(
                    error_code = "MEJEPA_AUDIT_PAIRWISE_MI_SCHEMA_INVALID",
                    error = %err,
                    tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                    "pairwise MI MCP request schema validation failed"
                );
                return self.tool_error_structured(
                    id,
                    ToolErrorKind::Validation,
                    "MEJEPA_AUDIT_PAIRWISE_MI_SCHEMA_INVALID",
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_AUDIT_PAIRWISE_MI
                    ),
                    json!({"tool": tool_names::MEJEPA_AUDIT_PAIRWISE_MI}),
                );
            }
        };
        if let Err(err) = request.validate() {
            tracing::error!(
                error_code = "MEJEPA_AUDIT_PAIRWISE_MI_INVALID_INPUT",
                error = %err,
                tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                "pairwise MI MCP request validation failed"
            );
            return self.tool_error_structured(
                id,
                ToolErrorKind::Validation,
                "MEJEPA_AUDIT_PAIRWISE_MI_INVALID_INPUT",
                &err,
                json!({"tool": tool_names::MEJEPA_AUDIT_PAIRWISE_MI}),
            );
        }

        let output_dir =
            match context_graph_paths::require_under_data_root(&request.output_dir, "outputDir") {
                Ok(path) => path,
                Err(err) => {
                    tracing::error!(
                        error_code = err.code,
                        error = %err,
                        output_dir = %request.output_dir.display(),
                        tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                        "pairwise MI output path rejected"
                    );
                    return self.tool_error_structured(
                        id,
                        ToolErrorKind::Validation,
                        err.code,
                        &err.to_string(),
                        json!({"outputDir": request.output_dir}),
                    );
                }
            };

        let result = run_pairwise_mi_audit(&request, output_dir.clone());

        match result {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                tracing::error!(
                    error_code = err.code(),
                    error = %err,
                    output_dir = %output_dir.display(),
                    step = request.step,
                    tool = tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
                    "pairwise MI audit failed"
                );
                self.tool_error_structured(
                    id,
                    ToolErrorKind::Execution,
                    err.code(),
                    &err.to_string(),
                    json!({
                        "outputDir": output_dir,
                        "step": request.step,
                        "sourceOfTruth": "pairwise_mi_csv"
                    }),
                )
            }
        }
    }
}

fn run_pairwise_mi_audit(
    request: &PairwiseMiAuditRequest,
    output_dir: PathBuf,
) -> Result<serde_json::Value, context_graph_mejepa_train::learning_signal::UtmlError> {
    let auditor = PairwiseMiAuditor::new(request.period_steps, &output_dir)?;
    let should_run = auditor.should_run(request.step);
    let summary = auditor.run_from_slot_series(&request.series_by_slot, request.step)?;
    Ok(json!({
        "step": summary.step,
        "periodSteps": request.period_steps,
        "shouldRunAtStep": should_run,
        "sampleCount": summary.sample_count,
        "slotCount": summary.matrix.slots.len(),
        "slots": summary.matrix.slots,
        "maxOffDiagonal": summary.max_off_diagonal,
        "meanOffDiagonal": summary.mean_off_diagonal,
        "csvPath": summary.path,
        "readbackPath": summary.readback_path,
        "matrix": summary.matrix.values,
        "sourceOfTruth": "pairwise_mi_csv",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pairwise_mi_audit_writes_and_reads_csv_source_of_truth() {
        let root = context_graph_paths::ensure_subdir("fsv/mcp-pairwise-mi-unit").unwrap();
        let output_dir = root.join("audit");
        let _ = fs::remove_dir_all(&output_dir);
        let request = PairwiseMiAuditRequest {
            output_dir: output_dir.clone(),
            step: 42,
            period_steps: 7,
            series_by_slot: BTreeMap::from([
                ("e_ast".to_string(), vec![0.1, 0.2, 0.3, 0.4]),
                ("e_diff".to_string(), vec![0.4, 0.3, 0.2, 0.1]),
                ("e_test".to_string(), vec![0.1, 0.1, 0.9, 0.9]),
            ]),
        };
        request.validate().unwrap();
        let output = run_pairwise_mi_audit(&request, output_dir).unwrap();
        assert_eq!(output["sourceOfTruth"], json!("pairwise_mi_csv"));
        assert_eq!(output["step"], json!(42));
        assert_eq!(output["shouldRunAtStep"], json!(true));
        let csv_path = output["csvPath"].as_str().unwrap();
        let text = fs::read_to_string(csv_path).unwrap();
        assert!(text.starts_with("slot,e_ast,e_diff,e_test"));
        let readback =
            context_graph_mejepa_train::learning_signal::load_pairwise_mi_matrix_csv(csv_path)
                .unwrap();
        assert_eq!(readback.slots.len(), 3);
        assert_eq!(readback.values.len(), 3);
    }

    #[test]
    fn pairwise_mi_audit_rejects_empty_slot_series() {
        let request = PairwiseMiAuditRequest {
            output_dir: PathBuf::from("/mnt/d/contextgraph/fsv/mcp-pairwise-mi-unit/edge"),
            step: 1,
            period_steps: 1,
            series_by_slot: BTreeMap::from([("e_ast".to_string(), vec![0.1])]),
        };
        assert!(request.validate().unwrap_err().contains("at least two"));
    }
}
