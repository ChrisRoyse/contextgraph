//! Tool name aliases for backward compatibility.
//!
//! TASK-MCP-P1-001: Provides alias resolution for legacy tool names.
//!
//! ## Design
//!
//! - Aliases use simple match for O(1) lookup
//! - Resolution is transparent to clients
//! - Adding aliases requires updating resolve_alias() function
//!
//! ## Aliases
//!
//! | Legacy Name | Canonical Name |
//! |-------------|----------------|
//! | consolidate_memories | trigger_consolidation |
//! | inject_context | store_memory |

/// Resolve a tool name to its canonical form.
///
/// If the name has an alias, returns the canonical name.
/// Otherwise returns the original name unchanged.
///
/// # Arguments
/// * `name` - The tool name to resolve
///
/// # Returns
/// The canonical tool name (may be same as input if no alias exists)
///
/// # TASK-MCP-P1-001
#[inline]
pub fn resolve_alias(name: &str) -> &str {
    match name {
        "consolidate_memories" => "trigger_consolidation",
        // Merged tools - backward compatibility
        "inject_context" => "store_memory",
        "mejepa_project_ingest" => crate::tools::names::MEJEPA_PROJECT_INGEST,
        "mejepa_project_report" => crate::tools::names::MEJEPA_PROJECT_REPORT,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidate_memories_alias() {
        assert_eq!(
            resolve_alias("consolidate_memories"),
            "trigger_consolidation"
        );
    }

    #[test]
    fn test_inject_context_alias() {
        // inject_context merged into store_memory
        assert_eq!(resolve_alias("inject_context"), "store_memory");
    }

    #[test]
    fn test_project_ingest_alias() {
        assert_eq!(
            resolve_alias("mejepa_project_ingest"),
            crate::tools::names::MEJEPA_PROJECT_INGEST
        );
    }

    #[test]
    fn test_project_report_alias() {
        assert_eq!(
            resolve_alias("mejepa_project_report"),
            crate::tools::names::MEJEPA_PROJECT_REPORT
        );
    }

    #[test]
    fn test_canonical_name_unchanged() {
        // Canonical names should pass through unchanged
        assert_eq!(
            resolve_alias("trigger_consolidation"),
            "trigger_consolidation"
        );
        assert_eq!(resolve_alias("store_memory"), "store_memory");
        assert_eq!(
            resolve_alias("get_workspace_status"),
            "get_workspace_status"
        );
    }

    #[test]
    fn test_unknown_name_unchanged() {
        // Unknown names should pass through unchanged
        assert_eq!(resolve_alias("unknown_tool"), "unknown_tool");
        assert_eq!(resolve_alias(""), "");
    }
}
