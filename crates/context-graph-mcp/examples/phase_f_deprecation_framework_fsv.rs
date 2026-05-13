use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use context_graph_mcp::deprecation::{
    apply_deprecation, apply_retired_cgreality_deprecation, is_retired_cgreality_tool,
    DeprecationNotice, RETIRED_CGREALITY_REMOVE_AFTER, RETIRED_CGREALITY_TOOLS,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-deprecation-framework-fsv";
const ENV_LEVEL: &str = "CONTEXT_GRAPH_DEPRECATION_LEVEL";
const CALL_SITE_INVENTORY: &str = "scripts/ci/phase_f_deprecation_call_sites.md";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&root)?;
    let artifact_path = root.join("deprecation_framework_fsv.json");
    let notice = DeprecationNotice {
        tool: "legacy-tool",
        deprecated_since: "2026-05-13",
        remove_after: "2026-08-13",
        replacement: Some("modern_tool"),
        reason: "superseded by modern_tool",
    };

    std::env::remove_var(ENV_LEVEL);
    let object_warn =
        apply_deprecation(&notice, json!({"ok": true})).map_err(anyhow::Error::msg)?;
    let scalar_warn = apply_deprecation(&notice, json!(42)).map_err(anyhow::Error::msg)?;
    let existing_warn = apply_deprecation(&notice, json!({"warnings": [{"code": "ALREADY"}]}))
        .map_err(anyhow::Error::msg)?;

    std::env::set_var(ENV_LEVEL, "error");
    let error_escalation = apply_deprecation(&notice, json!({"ok": true}))
        .expect_err("error escalation must block deprecated tool");

    std::env::set_var(ENV_LEVEL, "maybe");
    let malformed_level = apply_deprecation(&notice, json!({"ok": true}))
        .expect_err("malformed level must fail closed");

    std::env::remove_var(ENV_LEVEL);
    let bad_warnings = apply_deprecation(&notice, json!({"warnings": "bad"}))
        .expect_err("non-array warnings field must fail closed");

    let retired_tool = "mcp__cgreality__reality_run_attempt";
    let retired_warn = apply_retired_cgreality_deprecation(
        retired_tool,
        json!({
            "structuredContent": {
                "error_code": "CCREALITY_ENGINE_RETIRED",
                "attempted_tool": retired_tool
            },
            "isError": true,
        }),
    )
    .map_err(anyhow::Error::msg)?;

    std::env::set_var(ENV_LEVEL, "error");
    let retired_error = apply_retired_cgreality_deprecation(
        retired_tool,
        json!({
            "structuredContent": {
                "error_code": "CCREALITY_ENGINE_RETIRED",
                "attempted_tool": retired_tool
            },
            "isError": true,
        }),
    )
    .expect_err("retired tool error escalation must block the call");
    std::env::remove_var(ENV_LEVEL);

    let missing_notice =
        apply_retired_cgreality_deprecation("mcp__cgreality__mejepa_verify", json!({"ok": true}))
            .expect_err("non-retired tool must not have a retired-tool notice");

    let inventory = fs::read_to_string(CALL_SITE_INVENTORY)?;
    let inventory_sha256 = sha256_file(Path::new(CALL_SITE_INVENTORY))?;
    let inventory_lists_every_retired_tool = RETIRED_CGREALITY_TOOLS
        .iter()
        .all(|tool| inventory.contains(tool) && inventory.contains(RETIRED_CGREALITY_REMOVE_AFTER));

    let report = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-API-006",
        "build_release_sha": git_head_short(),
        "happy_path": [
            {
                "case": "warn_injects_object_response",
                "sot": "tool response JSON",
                "before": {"ok": true},
                "trigger": "apply_deprecation warn/default",
                "expected": "response has warnings[0].code",
                "actual": object_warn,
                "pass": object_warn["warnings"][0]["code"] == "MEJEPA_TOOL_DEPRECATED_LEGACY_TOOL",
            },
            {
                "case": "warn_wraps_scalar_response",
                "sot": "tool response JSON",
                "before": 42,
                "trigger": "apply_deprecation warn/default",
                "expected": "scalar moved to result and warning attached",
                "actual": scalar_warn,
                "pass": scalar_warn["result"] == 42
                    && scalar_warn["warnings"][0]["replacement"] == "modern_tool",
            },
            {
                "case": "warn_appends_existing_warning_array",
                "sot": "tool response JSON",
                "before": {"warnings": [{"code": "ALREADY"}]},
                "trigger": "apply_deprecation warn/default",
                "expected": "two warning entries",
                "actual": existing_warn,
                "pass": existing_warn["warnings"].as_array().map(|items| items.len()) == Some(2),
            },
            {
                "case": "retired_call_site_warn_injects_response_warning",
                "sot": "retired MCP response JSON",
                "before": {"error_code": "CCREALITY_ENGINE_RETIRED", "attempted_tool": retired_tool},
                "trigger": "apply_retired_cgreality_deprecation warn/default",
                "expected": "retired response has warnings[0] and preserves isError",
                "actual": retired_warn,
                "pass": retired_warn["isError"] == true
                    && retired_warn["structuredContent"]["error_code"] == "CCREALITY_ENGINE_RETIRED"
                    && retired_warn["warnings"][0]["tool"] == retired_tool,
            },
            {
                "case": "inventory_lists_all_wrapped_call_sites",
                "sot": CALL_SITE_INVENTORY,
                "before": null,
                "trigger": "read inventory file",
                "expected": "all retired tools and remove_after date listed",
                "actual": {
                    "path": CALL_SITE_INVENTORY,
                    "sha256": inventory_sha256,
                    "retired_tools": RETIRED_CGREALITY_TOOLS,
                    "remove_after": RETIRED_CGREALITY_REMOVE_AFTER,
                },
                "pass": inventory_lists_every_retired_tool
                    && inventory.contains("mejepa_phase7_tools.rs")
                    && inventory.contains("main.rs"),
            }
        ],
        "boundary_cases": [
            {
                "case": "error_level_blocks_call",
                "before": {"CONTEXT_GRAPH_DEPRECATION_LEVEL": "error"},
                "trigger": "apply_deprecation",
                "expected": "MEJEPA_TOOL_DEPRECATED_LEGACY_TOOL",
                "actual": error_escalation,
                "pass": error_escalation.contains("MEJEPA_TOOL_DEPRECATED_LEGACY_TOOL"),
            },
            {
                "case": "malformed_level_fails_closed",
                "before": {"CONTEXT_GRAPH_DEPRECATION_LEVEL": "maybe"},
                "trigger": "apply_deprecation",
                "expected": "MEJEPA_DEPRECATION_LEVEL_INVALID",
                "actual": malformed_level,
                "pass": malformed_level.contains("MEJEPA_DEPRECATION_LEVEL_INVALID"),
            },
            {
                "case": "non_array_warnings_fails_closed",
                "before": {"warnings": "bad"},
                "trigger": "apply_deprecation",
                "expected": "MEJEPA_DEPRECATION_WARNINGS_SHAPE_INVALID",
                "actual": bad_warnings,
                "pass": bad_warnings.contains("MEJEPA_DEPRECATION_WARNINGS_SHAPE_INVALID"),
            },
            {
                "case": "retired_call_site_error_level_blocks_call",
                "before": {"CONTEXT_GRAPH_DEPRECATION_LEVEL": "error", "tool": retired_tool},
                "trigger": "apply_retired_cgreality_deprecation",
                "expected": "MEJEPA_TOOL_DEPRECATED_MCP__CGREALITY__REALITY_RUN_ATTEMPT",
                "actual": retired_error,
                "pass": retired_error.contains("MEJEPA_TOOL_DEPRECATED_MCP__CGREALITY__REALITY_RUN_ATTEMPT"),
            },
            {
                "case": "non_retired_tool_notice_missing_fails_closed",
                "before": {"tool": "mcp__cgreality__mejepa_verify"},
                "trigger": "apply_retired_cgreality_deprecation",
                "expected": "MEJEPA_DEPRECATION_NOTICE_MISSING",
                "actual": missing_notice,
                "pass": missing_notice.contains("MEJEPA_DEPRECATION_NOTICE_MISSING")
                    && !is_retired_cgreality_tool("mcp__cgreality__mejepa_verify"),
            }
        ],
        "source_of_truth": {
            "response_json": "apply_deprecation return value",
            "env_var": ENV_LEVEL,
            "retired_tools": RETIRED_CGREALITY_TOOLS,
            "call_site_inventory": CALL_SITE_INVENTORY,
        },
        "readback_equal": true,
    });
    let all_passed = report["happy_path"]
        .as_array()
        .expect("happy_path array")
        .iter()
        .chain(report["boundary_cases"].as_array().expect("boundary array"))
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let mut report = report;
    report["all_passed"] = Value::Bool(all_passed);
    write_json_artifact(&artifact_path, &report)?;
    let artifact_sha256 = sha256_file(&artifact_path)?;

    if !all_passed {
        anyhow::bail!(
            "MEJEPA_DEPRECATION_FRAMEWORK_FSV_FAILED: {}",
            artifact_path.display()
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": artifact_sha256,
            "all_passed": all_passed,
            "readback_equal": true,
        }))?
    );
    Ok(())
}

fn write_json_artifact(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_synced(path, &bytes)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    if readback != bytes {
        anyhow::bail!(
            "MEJEPA_DEPRECATION_FRAMEWORK_READBACK_MISMATCH: {}",
            path.display()
        );
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
