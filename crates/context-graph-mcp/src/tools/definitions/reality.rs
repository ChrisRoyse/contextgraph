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
            run_attempt_summary_schema(),
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
            "dynamicjepa_reality_for_attempt",
            "Read the persisted DynamicJEPA reality block for an attempt.",
            run_attempt_schema(),
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
            "reality_query_ledger",
            "Run a constrained read-only ledger query.",
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
            json!({"type":"object","properties":{"scope":{"type":"string","enum":["mejepa_loop_only","reality_loop_only","full"],"default":"mejepa_loop_only"}},"additionalProperties":false}),
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
            "optimizer_bandit_select",
            "Select a generic optimizer arm through persisted Thompson sampling.",
            json!({"type":"object","properties":{"decision_point":{"type":"string"},"context_bucket":{"type":"string"},"arms":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":64},"cost_weight":{"type":"number","minimum":0}},"required":["decision_point","context_bucket","arms"],"additionalProperties":false}),
        ),
        def(
            "optimizer_bandit_record_reward",
            "Record verified [0,1] reward for a previously selected optimizer arm.",
            json!({"type":"object","properties":{"decision_point":{"type":"string"},"context_bucket":{"type":"string"},"arm":{"type":"string"},"reward":{"type":"number","minimum":0,"maximum":1},"cost":{"type":"number","minimum":0}},"required":["decision_point","context_bucket","arm","reward"],"additionalProperties":false}),
        ),
        def(
            "optimizer_bandit_state",
            "Read persisted SolverBandit state from the active run.",
            json!({"type":"object","properties":{"decision_point":{"type":"string"},"context_bucket":{"type":"string"}},"additionalProperties":false}),
        ),
        def(
            "optimizer_recall_recommendations",
            "Rank persisted optimizer recommendations with MMR over alpha*similarity + beta*uplift - gamma*cost, then persist and witness the recall audit.",
            json!({"type":"object","properties":{"failure_summary":{"type":"string"},"k":{"type":"integer","minimum":1,"maximum":50,"default":5},"alpha":{"type":"number","minimum":0,"default":0.7},"beta":{"type":"number","minimum":0,"default":0.2},"gamma":{"type":"number","minimum":0,"default":0.1},"lambda":{"type":"number","minimum":0,"maximum":1,"default":0.7}},"required":["failure_summary"],"additionalProperties":false}),
        ),
        def(
            "optimizer_compute_influence",
            "Compute graph-backed influence ranking from persisted attempts and recommendations using Forward-Push PPR, then persist and witness the computation audit.",
            json!({"type":"object","properties":{"failure_tag":{"type":"string"},"run_id":{"type":"string"},"k":{"type":"integer","minimum":1,"maximum":100,"default":5},"alpha":{"type":"number","exclusiveMinimum":0,"exclusiveMaximum":1,"default":0.15},"tolerance":{"type":"number","exclusiveMinimum":0,"default":1e-8},"max_pushes":{"type":"integer","minimum":1,"default":1000000}},"required":["failure_tag"],"additionalProperties":false}),
        ),
        def(
            "optimizer_witness_chain_verify",
            "Verify the SHAKE-256 optimizer witness chain for a run.",
            json!({"type":"object","properties":{"run_id":{"type":"string"}},"additionalProperties":false}),
        ),
        def(
            "optimizer_witness_chain_diff",
            "List optimizer witness-chain entries since an offset.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"since_offset":{"type":"integer","minimum":0}},"required":["since_offset"],"additionalProperties":false}),
        ),
        def(
            "optimizer_witness_chain_repair_legacy",
            "Explicitly reconcile a pre-canonical optimizer witness chain into the canonical layout after SHA-256 confirmation.",
            json!({"type":"object","properties":{"run_id":{"type":"string"},"expected_legacy_sha256":{"type":"string","pattern":"^(sha256:)?[0-9a-fA-F]{64}$"}},"required":["expected_legacy_sha256"],"additionalProperties":false}),
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
        // Phase 15: autoresearch engine
        def(
            "experiment_registry_list",
            "List experiments tracked in <run_root>/reality-optimizer/experiment-registry.json.",
            json!({"type":"object","properties":{"status_filter":{"type":"string","enum":["pending","kept","discarded","escalated","promoted"]},"limit":{"type":"integer","minimum":1,"maximum":500,"default":100}},"additionalProperties":false}),
        ),
        def(
            "experiment_registry_get",
            "Read a single experiment record by experiment_id.",
            json!({"type":"object","properties":{"experiment_id":{"type":"string"}},"required":["experiment_id"],"additionalProperties":false}),
        ),
        def(
            "champion_state_get",
            "Read champion-state.json (best (model, task) result so far).",
            json!({"type":"object","properties":{"model":{"type":"string"},"task":{"type":"string"}},"additionalProperties":false}),
        ),
        def(
            "attempts_history_query",
            "Read recent rows from <run_root>/reality-optimizer/attempts.jsonl.",
            json!({"type":"object","properties":{"model":{"type":"string"},"instance_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":500,"default":100},"metadata_filter":{"type":"object"}},"additionalProperties":false}),
        ),
        def(
            "attempts_query_reflexion",
            "Query Reflexion-style attempt episodes with filters and MMR reranking.",
            json!({"type":"object","properties":{"query":{"type":"string"},"model":{"type":"string"},"instance_id":{"type":"string"},"only_failures":{"type":"boolean"},"only_successes":{"type":"boolean"},"min_reward":{"type":"number"},"tag":{"type":"string"},"outcome_kind":{"type":"string"},"metadata_filter":{"type":"object"},"k":{"type":"integer","minimum":1,"maximum":100,"default":10},"lambda":{"type":"number","minimum":0,"maximum":1,"default":0.7}},"additionalProperties":false}),
        ),
        def(
            "attempts_critique_summary",
            "Summarize recurring Reflexion critique tags and outcomes.",
            json!({"type":"object","properties":{"model":{"type":"string"},"instance_id":{"type":"string"},"only_failures":{"type":"boolean"},"only_successes":{"type":"boolean"},"tag":{"type":"string"},"outcome_kind":{"type":"string"},"metadata_filter":{"type":"object"},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20}},"additionalProperties":false}),
        ),
        def(
            "attempts_success_strategies",
            "List successful attempt strategies from Reflexion fields.",
            json!({"type":"object","properties":{"model":{"type":"string"},"instance_id":{"type":"string"},"tag":{"type":"string"},"metadata_filter":{"type":"object"},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20}},"additionalProperties":false}),
        ),
        def(
            "attempts_synthesize",
            "Build a deterministic narrative summary from persisted attempt rows.",
            json!({"type":"object","properties":{"model":{"type":"string"},"instance_id":{"type":"string"},"only_failures":{"type":"boolean"},"only_successes":{"type":"boolean"},"tag":{"type":"string"},"outcome_kind":{"type":"string"},"metadata_filter":{"type":"object"}},"additionalProperties":false}),
        ),
        def(
            "experiment_registry_propose",
            "Append a candidate harness-change experiment to the registry.",
            json!({"type":"object","properties":{"harness_change_summary":{"type":"string"},"harness_change_recommendation_path":{"type":["string","null"]},"harness_change_shift_ids":{"type":"array","items":{"type":"string"}},"files_changed":{"type":"array","items":{"type":"string"}},"before_attempt_count":{"type":"integer","minimum":0},"claude_session_id":{"type":"string"}},"required":["harness_change_summary","claude_session_id"],"additionalProperties":true}),
        ),
        def(
            "experiment_registry_update_outcome",
            "Record kept/discarded/escalated/promoted on an existing experiment.",
            json!({"type":"object","properties":{"experiment_id":{"type":"string"},"outcome":{"type":"string","enum":["pending","kept","discarded","escalated","promoted"]},"reasoning":{"type":"string"},"claude_session_id":{"type":"string"}},"required":["experiment_id","outcome","claude_session_id"],"additionalProperties":true}),
        ),
        def(
            "champion_state_promote",
            "Promote a candidate champion. Requires official_resolved=true.",
            json!({"type":"object","properties":{"model":{"type":"string"},"task":{"type":"string"},"justification":{"type":"string"},"claude_session_id":{"type":"string"}},"required":["model","task","justification","claude_session_id"],"additionalProperties":true}),
        ),
    ]
}

fn def(name: &str, description: &str, schema: serde_json::Value) -> ToolDefinition {
    ToolDefinition::new(name, description, schema)
}

fn run_attempt_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0}},"required":["run_id","attempt"],"additionalProperties":false})
}

fn run_attempt_summary_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"compact":{"type":"boolean","default":false}},"required":["run_id","attempt"],"additionalProperties":false})
}

fn run_attempt_compact_schema() -> serde_json::Value {
    json!({"type":"object","properties":{"run_id":{"type":"string"},"attempt":{"type":"integer","minimum":0},"compact":{"type":"boolean","default":false}},"required":["run_id","attempt"],"additionalProperties":false})
}
