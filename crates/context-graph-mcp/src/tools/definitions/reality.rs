use crate::tools::types::ToolDefinition;
use serde_json::json;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        def(
            "reality_latest_root",
            "Read the active ccreality runtime root.",
            json!({"type": "object", "properties": {}, "additionalProperties": false}),
        ),
        def(
            "reality_attempt_summary",
            "Read an attempt-summary artifact.",
            run_attempt_schema(),
        ),
        def(
            "reality_official_report",
            "Read official SWE-bench evidence for an attempt.",
            run_attempt_schema(),
        ),
        def(
            "reality_problem_packet",
            "Read the problem-reality packet for a run.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"compact":{"type":"boolean","default":false}},"required":["run_id"],"additionalProperties":false}),
        ),
        def(
            "reality_signal",
            "Read the reality-signal packet for an attempt.",
            run_attempt_compact_schema(),
        ),
        def(
            "reality_failure",
            "Read compact failure evidence for an attempt.",
            run_attempt_schema(),
        ),
        def(
            "reality_trigger_decision",
            "Read the trigger-decision artifact for an attempt.",
            run_attempt_schema(),
        ),
        def(
            "reality_harness_transitions",
            "List or read harness transition artifacts.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"round":{"type":["integer","null"],"minimum":0}},"required":["run_id","attempt"],"additionalProperties":false}),
        ),
        def(
            "reality_compare_attempts",
            "Compare two attempt summaries.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt_a":{"type":"integer","minimum":0},"attempt_b":{"type":"integer","minimum":0}},"required":["run_id","attempt_a","attempt_b"],"additionalProperties":false}),
        ),
        def(
            "reality_audit_trail",
            "Read ContextGraph audit trail for an entity.",
            json!({"type":"object","properties":{"entity_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":500,"default":50}},"required":["entity_id"],"additionalProperties":false}),
        ),
        def(
            "reality_replay_artifact",
            "Read any file artifact with SHA-256 readback.",
            json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
        ),
        def(
            "reality_run_attempt",
            "Spawn reality-loop attempt.",
            json!({"type":"object","properties":{"task_manifest":{"type":"string"},"runtime_root":{"type":"string"},"max_attempts":{"type":"integer","minimum":1,"default":1},"official_eval":{"type":"boolean","default":true},"model":{"type":"string"}},"required":["task_manifest"],"additionalProperties":false}),
        ),
        def(
            "reality_run_smoke",
            "Spawn reality-loop smoke.",
            json!({"type":"object","properties":{"runtime_root":{"type":"string"},"label":{"type":"string"},"session_id":{"type":"string"}},"additionalProperties":false}),
        ),
        def(
            "reality_run_official_eval",
            "Run official evaluation for an existing attempt.",
            run_attempt_schema(),
        ),
        def(
            "reality_run_targeted_test",
            "Run one targeted test for an attempt.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"test_id":{"type":"string"},"timeout_secs":{"type":"integer","minimum":1,"maximum":1800}},"required":["run_id","attempt","test_id"],"additionalProperties":false}),
        ),
        def(
            "reality_recompile_reward",
            "Replay an attempt signal packet after reward changes.",
            run_attempt_schema(),
        ),
        def(
            "reality_query_ledger",
            "Run a constrained read-only BlackBox ledger query.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"where":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":500}},"required":["run_id","where"],"additionalProperties":false}),
        ),
        def(
            "harness_open_window",
            "Open a governed project file window and return SHA-256.",
            json!({"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer","minimum":1},"end_line":{"type":"integer","minimum":1}},"required":["path","start_line","end_line"],"additionalProperties":false}),
        ),
        def(
            "harness_apply_line_window_edit",
            "Apply a SHA-guarded governed line-window edit.",
            json!({"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer","minimum":1},"end_line":{"type":"integer","minimum":1},"observed_sha256":{"type":"string"},"replace":{"type":"string"},"session_id":{"type":"string"},"tool_use_id":{"type":"string"}},"required":["path","start_line","end_line","observed_sha256","replace"],"additionalProperties":false}),
        ),
        def(
            "harness_run_command",
            "Run an allowlisted governed command in the project root.",
            json!({"type":"object","properties":{"command":{"type":"string"},"timeout_secs":{"type":"integer","minimum":1,"maximum":1800},"session_id":{"type":"string"}},"required":["command"],"additionalProperties":false}),
        ),
        def(
            "harness_git_diff",
            "Read git diff for the project or one governed path.",
            json!({"type":"object","properties":{"path":{"type":["string","null"]},"base":{"type":"string"}},"additionalProperties":false}),
        ),
        def(
            "harness_git_status",
            "Read git status --porcelain as structured JSON.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "harness_verify_state",
            "Run governed rustfmt/check/test verification.",
            json!({"type":"object","properties":{"scope":{"type":"string","enum":["reality_loop_only","full"],"default":"reality_loop_only"}},"additionalProperties":false}),
        ),
        def(
            "optimizer_record_decision",
            "Write optimizer trigger-decision artifacts.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"policy":{"type":"string"},"should_run":{"type":"boolean"},"reasons":{"type":"array"},"claude_session_id":{"type":"string"},"claude_model":{"type":"string"}},"required":["run_id","attempt","policy","should_run","claude_session_id","claude_model"],"additionalProperties":true}),
        ),
        def(
            "optimizer_record_recommendation",
            "Validate and write optimizer recommendation artifact.",
            json!({"type":"object","additionalProperties":true}),
        ),
        def(
            "optimizer_record_harness_transition",
            "Write a harness-transition artifact.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"tool_use_id":{"type":"string"},"file_path":{"type":"string"},"before_sha256":{"type":"string"},"after_sha256":{"type":"string"},"lines_added":{"type":"integer","minimum":0},"lines_removed":{"type":"integer","minimum":0},"cargo_check":{"type":"object"},"git_diff_stat":{"type":"string"},"session_id":{"type":"string"}},"required":["run_id","attempt","file_path","before_sha256","after_sha256","lines_added","lines_removed"],"additionalProperties":true}),
        ),
        def(
            "reality_shift_log",
            "Read ccreality per-session shift log.",
            json!({"type":"object","properties":{"session_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":500},"since_shift_id":{"type":["string","null"]}},"required":["session_id"],"additionalProperties":false}),
        ),
        def(
            "reality_shift_compare_to_my_view",
            "Compare current file SHAs to a session's shift-log view.",
            json!({"type":"object","properties":{"session_id":{"type":"string"},"files":{"type":"array","items":{"type":"string"}}},"required":["session_id","files"],"additionalProperties":false}),
        ),
    ]
}

fn def(name: &str, description: &str, schema: serde_json::Value) -> ToolDefinition {
    ToolDefinition::new(name, description, schema)
}

fn run_attempt_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0}},"required":["run_id","attempt"],"additionalProperties":false})
}

fn run_attempt_compact_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"compact":{"type":"boolean","default":false}},"required":["run_id","attempt"],"additionalProperties":false})
}
