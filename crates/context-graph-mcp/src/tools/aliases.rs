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
//! | discover_goals | discover_sub_goals |
//! | consolidate_memories | trigger_consolidation |

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
        "discover_goals" => "discover_sub_goals",
        "consolidate_memories" => "trigger_consolidation",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_goals_alias() {
        assert_eq!(resolve_alias("discover_goals"), "discover_sub_goals");
    }

    #[test]
    fn test_consolidate_memories_alias() {
        assert_eq!(resolve_alias("consolidate_memories"), "trigger_consolidation");
    }

    #[test]
    fn test_canonical_name_unchanged() {
        // Canonical names should pass through unchanged
        assert_eq!(resolve_alias("discover_sub_goals"), "discover_sub_goals");
        assert_eq!(resolve_alias("trigger_consolidation"), "trigger_consolidation");
        assert_eq!(resolve_alias("inject_context"), "inject_context");
        assert_eq!(resolve_alias("get_consciousness_state"), "get_consciousness_state");
    }

    #[test]
    fn test_unknown_name_unchanged() {
        // Unknown names should pass through unchanged
        assert_eq!(resolve_alias("unknown_tool"), "unknown_tool");
        assert_eq!(resolve_alias(""), "");
    }
}
