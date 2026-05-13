//! TASK-API-004 — Deprecation framework for MCP tools.
//!
//! Provides typed deprecation metadata and warning-injection helpers so
//! a tool can be marked "deprecated" with a documented removal date,
//! optional replacement, and opt-in escalation (warn -> hard error)
//! controlled by the operator via the `CONTEXT_GRAPH_DEPRECATION_LEVEL`
//! env-var.
//!
//! Per `24_API_STABILITY.md §3` the framework keeps deprecated tools
//! available behind a warning until the published removal date so we
//! never need a backwards-compat shim — the warning IS the contract.

use std::env;

use serde::Serialize;
use serde_json::{json, Map, Value};

const ENV_LEVEL: &str = "CONTEXT_GRAPH_DEPRECATION_LEVEL";
pub const RETIRED_CGREALITY_DEPRECATED_SINCE: &str = "2026-05-09";
pub const RETIRED_CGREALITY_REMOVE_AFTER: &str = "2026-08-09";
pub const RETIRED_CGREALITY_REPLACEMENT: &str =
    "mcp__cgreality__mejepa_verify, mcp__cgreality__mejepa_predict_latest, or mcp__cgreality__mejepa_observe_shift";
pub const RETIRED_CGREALITY_REASON: &str =
    "retired by the 2026-05-09 ME-JEPA pivot to the single outer-loop truth system";
pub const RETIRED_CGREALITY_TOOLS: &[&str] = &[
    "mcp__cgreality__reality_run_attempt",
    "mcp__cgreality__reality_alter_file",
    "mcp__cgreality__reality_alter_run",
    "mcp__cgreality__reality_alter_apply",
    "mcp__cgreality__reality_drip_attempt",
];

/// Severity level the operator chose at process start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeprecationLevel {
    /// Default — record the warning into the response payload but allow the call.
    Warn,
    /// Treat any deprecated-tool call as a fail-closed error.
    Error,
}

impl DeprecationLevel {
    pub fn from_env() -> Result<Self, String> {
        match env::var(ENV_LEVEL) {
            Ok(raw) => match raw.to_ascii_lowercase().as_str() {
                "warn" | "warning" | "0" | "false" => Ok(DeprecationLevel::Warn),
                "error" | "1" | "true" => Ok(DeprecationLevel::Error),
                _ => Err(format!(
                    "MEJEPA_DEPRECATION_LEVEL_INVALID: {ENV_LEVEL} must be warn or error"
                )),
            },
            Err(env::VarError::NotPresent) => Ok(DeprecationLevel::Warn),
            Err(env::VarError::NotUnicode(_)) => Err(format!(
                "MEJEPA_DEPRECATION_LEVEL_INVALID: {ENV_LEVEL} must be valid Unicode"
            )),
        }
    }
}

/// Typed deprecation metadata for a single tool.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeprecationNotice {
    pub tool: &'static str,
    pub deprecated_since: &'static str,
    pub remove_after: &'static str,
    pub replacement: Option<&'static str>,
    pub reason: &'static str,
}

impl DeprecationNotice {
    pub fn as_warning_value(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "code".into(),
            Value::String(format!(
                "MEJEPA_TOOL_DEPRECATED_{}",
                error_code_fragment(self.tool)
            )),
        );
        map.insert("tool".into(), Value::String(self.tool.to_string()));
        map.insert(
            "deprecatedSince".into(),
            Value::String(self.deprecated_since.to_string()),
        );
        map.insert(
            "removeAfter".into(),
            Value::String(self.remove_after.to_string()),
        );
        map.insert("reason".into(), Value::String(self.reason.to_string()));
        if let Some(replacement) = self.replacement {
            map.insert("replacement".into(), Value::String(replacement.to_string()));
        }
        Value::Object(map)
    }
}

/// Wrap a successful tool response with the deprecation warning, or
/// convert it into a fail-closed error when the operator has escalated.
pub fn apply_deprecation(notice: &DeprecationNotice, response: Value) -> Result<Value, String> {
    match DeprecationLevel::from_env()? {
        DeprecationLevel::Warn => inject_warning(notice, response),
        DeprecationLevel::Error => Err(format!(
            "MEJEPA_TOOL_DEPRECATED_{}: {} (remove_after={}, replacement={})",
            error_code_fragment(notice.tool),
            notice.reason,
            notice.remove_after,
            notice.replacement.unwrap_or("none")
        )),
    }
}

pub fn is_retired_cgreality_tool(tool_name: &str) -> bool {
    RETIRED_CGREALITY_TOOLS.contains(&tool_name)
}

pub fn retired_cgreality_notice(tool_name: &str) -> Option<DeprecationNotice> {
    let tool = RETIRED_CGREALITY_TOOLS
        .iter()
        .copied()
        .find(|candidate| *candidate == tool_name)?;
    Some(DeprecationNotice {
        tool,
        deprecated_since: RETIRED_CGREALITY_DEPRECATED_SINCE,
        remove_after: RETIRED_CGREALITY_REMOVE_AFTER,
        replacement: Some(RETIRED_CGREALITY_REPLACEMENT),
        reason: RETIRED_CGREALITY_REASON,
    })
}

pub fn apply_retired_cgreality_deprecation(
    tool_name: &str,
    response: Value,
) -> Result<Value, String> {
    let notice = retired_cgreality_notice(tool_name).ok_or_else(|| {
        format!("MEJEPA_DEPRECATION_NOTICE_MISSING: no retired cgreality notice for {tool_name}")
    })?;
    apply_deprecation(&notice, response)
}

fn inject_warning(notice: &DeprecationNotice, response: Value) -> Result<Value, String> {
    let warning = notice.as_warning_value();
    match response {
        Value::Object(mut map) => {
            let entry = map
                .entry("warnings")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(arr) = entry {
                arr.push(warning);
            } else {
                return Err(
                    "MEJEPA_DEPRECATION_WARNINGS_SHAPE_INVALID: existing warnings field must be an array"
                        .to_string(),
                );
            }
            Ok(Value::Object(map))
        }
        other => Ok(json!({
            "result": other,
            "warnings": [warning],
        })),
    }
}

fn error_code_fragment(tool: &str) -> String {
    tool.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn notice() -> DeprecationNotice {
        DeprecationNotice {
            tool: "legacy_thing",
            deprecated_since: "2026-05-13",
            remove_after: "2026-08-13",
            replacement: Some("modern_thing"),
            reason: "superseded by modern_thing",
        }
    }

    #[test]
    fn warning_attached_to_object_response() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        let result = apply_deprecation(&notice(), json!({"ok": true})).unwrap();
        let warnings = result.get("warnings").unwrap().as_array().unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0]["code"].as_str().unwrap(),
            "MEJEPA_TOOL_DEPRECATED_LEGACY_THING"
        );
    }

    #[test]
    fn warning_wraps_non_object_response() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        let result = apply_deprecation(&notice(), json!(42)).unwrap();
        assert_eq!(result["result"].as_i64().unwrap(), 42);
        assert_eq!(result["warnings"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn error_level_blocks_call() {
        let _guard = env_guard();
        std::env::set_var(ENV_LEVEL, "error");
        let err = apply_deprecation(&notice(), json!({"ok": true})).unwrap_err();
        assert!(err.contains("MEJEPA_TOOL_DEPRECATED_LEGACY_THING"));
        std::env::remove_var(ENV_LEVEL);
    }

    #[test]
    fn warn_is_default_level() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        assert_eq!(
            DeprecationLevel::from_env().unwrap(),
            DeprecationLevel::Warn
        );
    }

    #[test]
    fn malformed_level_fails_closed() {
        let _guard = env_guard();
        std::env::set_var(ENV_LEVEL, "maybe");
        let err = apply_deprecation(&notice(), json!({"ok": true})).unwrap_err();
        assert!(err.contains("MEJEPA_DEPRECATION_LEVEL_INVALID"));
        std::env::remove_var(ENV_LEVEL);
    }

    #[test]
    fn existing_non_array_warnings_fail_closed() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        let err = apply_deprecation(&notice(), json!({"warnings": "bad"})).unwrap_err();
        assert!(err.contains("MEJEPA_DEPRECATION_WARNINGS_SHAPE_INVALID"));
    }

    #[test]
    fn existing_array_warnings_are_preserved() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        let result =
            apply_deprecation(&notice(), json!({"warnings": [{"code": "ALREADY"}]})).unwrap();
        assert_eq!(result["warnings"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn retired_cgreality_tools_have_structured_notices() {
        for tool in RETIRED_CGREALITY_TOOLS {
            let notice = retired_cgreality_notice(tool).expect("retired tool notice");
            assert_eq!(notice.deprecated_since, RETIRED_CGREALITY_DEPRECATED_SINCE);
            assert_eq!(notice.remove_after, RETIRED_CGREALITY_REMOVE_AFTER);
            assert_eq!(notice.replacement, Some(RETIRED_CGREALITY_REPLACEMENT));
        }
        assert!(retired_cgreality_notice("mcp__cgreality__mejepa_verify").is_none());
    }

    #[test]
    fn retired_cgreality_warn_and_error_paths_match_framework() {
        let _guard = env_guard();
        std::env::remove_var(ENV_LEVEL);
        let warn = apply_retired_cgreality_deprecation(
            "mcp__cgreality__reality_run_attempt",
            json!({"isError": true}),
        )
        .unwrap();
        assert_eq!(
            warn["warnings"][0]["code"],
            "MEJEPA_TOOL_DEPRECATED_MCP__CGREALITY__REALITY_RUN_ATTEMPT"
        );

        std::env::set_var(ENV_LEVEL, "error");
        let err = apply_retired_cgreality_deprecation(
            "mcp__cgreality__reality_run_attempt",
            json!({"isError": true}),
        )
        .unwrap_err();
        assert!(err.contains("MEJEPA_TOOL_DEPRECATED_MCP__CGREALITY__REALITY_RUN_ATTEMPT"));
        std::env::remove_var(ENV_LEVEL);
    }
}
