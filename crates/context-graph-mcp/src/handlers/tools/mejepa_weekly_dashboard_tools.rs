//! ME-JEPA Phase F weekly evaluation dashboard MCP handler.

use std::path::PathBuf;

use serde::Deserialize;

use super::mejepa_weekly_dashboard_status::weekly_eval_dashboard;
use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

const DEFAULT_DASHBOARD_MAX_CELLS: usize = 64;
const DASHBOARD_MAX_CELLS_CAP: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WeeklyEvalDashboardRequest {
    db_path: PathBuf,
    d_root: Option<PathBuf>,
    exports_root: Option<PathBuf>,
    #[serde(default = "default_dashboard_max_cells")]
    max_cells: usize,
}

fn default_dashboard_max_cells() -> usize {
    DEFAULT_DASHBOARD_MAX_CELLS
}

impl Handlers {
    pub(crate) async fn call_mejepa_weekly_eval_dashboard(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: WeeklyEvalDashboardRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD
                    ),
                );
            }
        };
        if let Err(message) = validate_dashboard_request(&request) {
            return self.tool_error_typed(id, ToolErrorKind::Validation, &message);
        }
        let result = weekly_eval_dashboard(
            &request.db_path,
            request.d_root,
            request.exports_root,
            request.max_cells,
        );
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

fn validate_dashboard_request(request: &WeeklyEvalDashboardRequest) -> Result<(), String> {
    if request.db_path.as_os_str().is_empty() {
        return Err("dbPath must be a non-empty path".to_string());
    }
    if let Some(exports_root) = &request.exports_root {
        if exports_root.as_os_str().is_empty() {
            return Err("exportsRoot must be a non-empty path".to_string());
        }
    }
    if let Some(d_root) = &request.d_root {
        if d_root.as_os_str().is_empty() {
            return Err("dRoot must be a non-empty path".to_string());
        }
    }
    if request.max_cells == 0 || request.max_cells > DASHBOARD_MAX_CELLS_CAP {
        return Err(format!("maxCells must be in 1..={DASHBOARD_MAX_CELLS_CAP}"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "mejepa_weekly_dashboard_regression_tests.rs"]
mod fsv_tests;
