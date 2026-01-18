//! Dream tool definitions (trigger_dream, get_dream_status).

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns dream tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // trigger_dream - Execute NREM/REM dream consolidation cycle
        ToolDefinition::new(
            "trigger_dream",
            "Execute NREM/REM dream consolidation cycle. \
             NREM phase (3 min): Hebbian replay strengthening high-importance connections. \
             REM phase (2 min): Hyperbolic random walk in Poincare ball discovering blind spots. \
             Trigger conditions (AP-70): entropy > 0.7 AND churn > 0.5. \
             Constitution constraints: 100 max queries, <100ms wake latency, <30% GPU. \
             Returns dream_id, status, phase reports, and recommendations.",
            json!({
                "type": "object",
                "properties": {
                    "blocking": {
                        "type": "boolean",
                        "description": "Wait for dream cycle to complete (default: true). If false, returns immediately with dream_id for status polling.",
                        "default": true
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "Simulate dream cycle without modifying graph (default: false). Returns projected effects.",
                        "default": false
                    },
                    "skip_nrem": {
                        "type": "boolean",
                        "description": "Skip NREM phase, run REM only (default: false).",
                        "default": false
                    },
                    "skip_rem": {
                        "type": "boolean",
                        "description": "Skip REM phase, run NREM only (default: false).",
                        "default": false
                    },
                    "max_duration_secs": {
                        "type": "integer",
                        "description": "Maximum total duration in seconds (default: 300 = 5 min). Phases may be truncated.",
                        "default": 300,
                        "minimum": 10,
                        "maximum": 600
                    }
                },
                "additionalProperties": false
            }),
        ),

        // get_dream_status - Get status of running or completed dream cycle
        ToolDefinition::new(
            "get_dream_status",
            "Get status of a dream cycle. Returns current phase, progress percentage, \
             elapsed time, and partial results if available. Use after trigger_dream with \
             blocking=false to poll for completion.",
            json!({
                "type": "object",
                "properties": {
                    "dream_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Dream cycle ID from trigger_dream response. If omitted, returns status of most recent dream."
                    }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dream_tool_count() {
        assert_eq!(definitions().len(), 2);
    }

    #[test]
    fn test_trigger_dream_schema() {
        let tools = definitions();
        let trigger = tools.iter().find(|t| t.name == "trigger_dream").unwrap();

        let props = trigger.input_schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("blocking"));
        assert!(props.contains_key("dry_run"));
        assert!(props.contains_key("skip_nrem"));
        assert!(props.contains_key("skip_rem"));
        assert!(props.contains_key("max_duration_secs"));
    }

    #[test]
    fn test_get_dream_status_schema() {
        let tools = definitions();
        let status = tools.iter().find(|t| t.name == "get_dream_status").unwrap();

        let props = status.input_schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("dream_id"));
    }
}
