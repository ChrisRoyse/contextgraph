//! File watcher tool definitions per PRD v6 Section 10.
//!
//! Tools (4):
//! - list_watched_files: List all files with embeddings
//! - get_file_watcher_stats: Get statistics about file watcher content
//! - delete_file_content: Delete all embeddings for a specific file
//! - reconcile_files: Find orphaned files and optionally delete them
//!
//! Constitution Compliance:
//! - SEC-06: Soft delete 30-day recovery for delete_file_content
//! - FAIL FAST: All tools error on failures, no fallbacks

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns file watcher tool definitions (4 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // list_watched_files
        ToolDefinition::new(
            "list_watched_files",
            "List all files that have embeddings in the knowledge graph from the file watcher. \
             Returns file paths with chunk counts and last update times.",
            json!({
                "type": "object",
                "properties": {
                    "include_counts": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include chunk counts per file"
                    },
                    "path_filter": {
                        "type": "string",
                        "description": "Optional glob pattern to filter paths (e.g., '**/docs/*.md')"
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
        // get_file_watcher_stats
        ToolDefinition::new(
            "get_file_watcher_stats",
            "Get statistics about file watcher content in the knowledge graph. \
             Returns total files, total chunks, average chunks per file, and min/max values.",
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        ),
        // delete_file_content
        ToolDefinition::new(
            "delete_file_content",
            "Delete all embeddings for a specific file path. Use for manual cleanup. \
             Supports soft delete with 30-day recovery (per SEC-06).",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Absolute path to the file"
                    },
                    "soft_delete": {
                        "type": "boolean",
                        "default": true,
                        "description": "Use soft delete with 30-day recovery (default true per SEC-06)"
                    }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }),
        ),
        // reconcile_files
        ToolDefinition::new(
            "reconcile_files",
            "Find orphaned files (embeddings exist but file doesn't on disk) and optionally delete them. \
             Use dry_run=true to preview changes without modifying data.",
            json!({
                "type": "object",
                "properties": {
                    "dry_run": {
                        "type": "boolean",
                        "default": true,
                        "description": "If true, only report orphans without deleting"
                    },
                    "base_path": {
                        "type": "string",
                        "description": "Optional base path to limit reconciliation scope"
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_exist_with_required_fields() {
        let tools = definitions();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"list_watched_files"));
        assert!(names.contains(&"get_file_watcher_stats"));
        assert!(names.contains(&"delete_file_content"));
        assert!(names.contains(&"reconcile_files"));
        // delete_file_content: soft_delete defaults to true per SEC-06
        let delete = tools
            .iter()
            .find(|t| t.name == "delete_file_content")
            .unwrap();
        assert!(delete.input_schema["properties"]["soft_delete"]["default"]
            .as_bool()
            .unwrap());
        // reconcile_files: dry_run defaults to true
        let reconcile = tools.iter().find(|t| t.name == "reconcile_files").unwrap();
        assert!(reconcile.input_schema["properties"]["dry_run"]["default"]
            .as_bool()
            .unwrap());
    }

    #[test]
    fn test_synthetic_valid_input() {
        let tools = definitions();
        // delete_file_content requires file_path
        let delete = tools
            .iter()
            .find(|t| t.name == "delete_file_content")
            .unwrap();
        let required: Vec<&str> = delete.input_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(required.contains(&"file_path"));
        // All tools have type: object
        for tool in &tools {
            assert_eq!(tool.input_schema["type"].as_str().unwrap(), "object");
            assert!(!tool.description.is_empty());
        }
    }
}
