//! Daemon tool definitions for multi-agent observability.
//!
//! Tools:
//! - daemon_status: Returns daemon health, connection count, and background task state

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns daemon tool definitions (1 tool).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition::new(
        "daemon_status",
        "Returns the daemon's health metrics for multi-agent observability. \
         Shows active connection count, model loading state, background task status \
         (GC, HNSW persist, graph builder), uptime, and PID. \
         Use this to diagnose connection issues or verify multi-agent setup is working.",
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_definitions_count() {
        assert_eq!(definitions().len(), 1);
    }

    #[test]
    fn test_daemon_status_definition() {
        let tools = definitions();
        let status = &tools[0];
        assert_eq!(status.name, "daemon_status");
        assert!(status.description.contains("connection count"));
        let props = status.input_schema.get("properties").unwrap();
        assert!(props.as_object().unwrap().is_empty());
    }
}
